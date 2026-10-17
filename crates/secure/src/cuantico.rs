//! The post-quantum layer (16 Oct 2026): a second lock on every message that Olm carries,
//! with a key no quantum computer can recover.
//!
//! Olm's keys are Curve25519: safe today, but a quantum computer able to run Shor's algorithm
//! would break them, and traffic captured now could be opened then («harvest now, decrypt
//! later»). ML-KEM-768 (FIPS 203) is the lattice scheme NIST standardised against that; Signal
//! added it to its protocol in 2023 (PQXDH). Here it is a hybrid: the message stays inside
//! Olm exactly as before, and is first sealed with a key both sides share only through ML-KEM.
//! An attacker must break both.
//!
//! Shape: once the two people have verified each other, side 0 (by the order of their keys)
//! sends an encapsulation key inside Olm; side 1 encapsulates a secret to it and sends the
//! ciphertext back, also inside Olm. From that secret each direction derives a chain; every
//! message takes a fresh key from the chain and moves it forward (forward secrecy within the
//! layer), and carries its number so a message lost in a dropped link does not stall the rest.
//! A number seen before is refused: a replay. A new encapsulation runs at every link.
//!
//! Chosen with Grok and ChatGPT on 16 Oct 2026: «híbrido X25519 + ML-KEM-768 como Signal; no
//! sustituir Olm, añadir la capa encima; ratchet simétrico por dirección».

use chacha20poly1305::aead::{Aead as _, KeyInit as _, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use ml_kem::kem::{Decapsulate as _, Encapsulate as _, Kem as _, KeyExport as _};
use ml_kem::{Ciphertext, EncapsulationKey768, MlKem768};

use crate::sframe::hkdf;
use crate::Error;

const DOMINIO: &[u8] = b"GUARDIANA_SECURE_CUANTICO_V1";
/// Bytes of an ML-KEM-768 encapsulation key and of its ciphertext, as FIPS 203 fixes them.
pub const LLAVE_BYTES: usize = 1184;
/// See [`LLAVE_BYTES`].
pub const CIFRADO_BYTES: usize = 1088;

/// Side 0's half: the decapsulation key, kept until side 1 answers.
pub struct Oferta {
    dk: ml_kem::DecapsulationKey768,
}

impl Oferta {
    /// A fresh key pair; the bytes go to the other side inside Olm.
    #[must_use]
    pub fn nueva() -> (Self, Vec<u8>) {
        let (dk, ek) = MlKem768::generate_keypair();
        (Self { dk }, ek.to_bytes().to_vec())
    }

    /// The shared secret, from the ciphertext the other side sent back.
    pub fn abre(&self, cifrado: &[u8]) -> Result<[u8; 32], Error> {
        let ct = Ciphertext::<MlKem768>::try_from(cifrado).map_err(|_| Error::CannotOpen)?;
        let ss = self.dk.decapsulate(&ct);
        let mut k = [0u8; 32];
        k.copy_from_slice(ss.as_slice());
        Ok(k)
    }
}

/// Side 1's step: a secret for the holder of `llave`, and its ciphertext to send back.
pub fn encapsula(llave: &[u8]) -> Result<(Vec<u8>, [u8; 32]), Error> {
    let bytes =
        ml_kem::Key::<EncapsulationKey768>::try_from(llave).map_err(|_| Error::CannotOpen)?;
    let ek = EncapsulationKey768::new(&bytes).map_err(|_| Error::CannotOpen)?;
    let (ct, ss) = ek.encapsulate();
    let mut k = [0u8; 32];
    k.copy_from_slice(ss.as_slice());
    Ok((ct.to_vec(), k))
}

/// One direction of the layer: a chain key that moves forward with every message.
struct Cadena {
    clave: [u8; 32],
    n: u64,
}

impl Cadena {
    fn nueva(secreto: &[u8; 32], etiqueta: &[u8]) -> Self {
        let mut clave = [0u8; 32];
        hkdf(DOMINIO, secreto, etiqueta, &mut clave);
        Self { clave, n: 0 }
    }

    /// The key for message `n` (the chain's current one), then the chain moves on.
    fn siguiente(&mut self) -> ([u8; 32], u64) {
        let mut mensaje = [0u8; 32];
        hkdf(DOMINIO, &self.clave, b"mensaje", &mut mensaje);
        let mut nueva = [0u8; 32];
        hkdf(DOMINIO, &self.clave, b"cadena", &mut nueva);
        let n = self.n;
        self.clave = nueva;
        self.n += 1;
        (mensaje, n)
    }
}

/// The two directions, from the shared secret and who is who (0 or 1, agreed by the keys).
pub struct Capa {
    envio: Cadena,
    recibo: Cadena,
}

impl Capa {
    /// From the secret ML-KEM gave both sides.
    #[must_use]
    pub fn nueva(secreto: &[u8; 32], yo: u8) -> Self {
        let (a, b) = (b"de 0 a 1".as_slice(), b"de 1 a 0".as_slice());
        let (mio, suyo) = if yo == 0 { (a, b) } else { (b, a) };
        Self {
            envio: Cadena::nueva(secreto, mio),
            recibo: Cadena::nueva(secreto, suyo),
        }
    }

    fn nonce(n: u64) -> Nonce {
        let mut b = [0u8; 12];
        b[4..].copy_from_slice(&n.to_be_bytes());
        Nonce::from(b)
    }

    /// Seal a message: its number (8 bytes) then the ciphertext.
    pub fn sella(&mut self, plano: &[u8]) -> Result<Vec<u8>, Error> {
        let (clave, n) = self.envio.siguiente();
        let c = ChaCha20Poly1305::new((&clave).into())
            .encrypt(
                &Self::nonce(n),
                Payload {
                    msg: plano,
                    aad: &n.to_be_bytes(),
                },
            )
            .map_err(|_| Error::CannotOpen)?;
        let mut out = n.to_be_bytes().to_vec();
        out.extend_from_slice(&c);
        Ok(out)
    }

    /// Open a sealed message. A number already used (a replay) or one too far ahead is refused;
    /// numbers skipped by a dropped link are passed over.
    pub fn abre(&mut self, sellado: &[u8]) -> Result<Vec<u8>, Error> {
        if sellado.len() < 8 {
            return Err(Error::CannotOpen);
        }
        let mut nb = [0u8; 8];
        nb.copy_from_slice(&sellado[..8]);
        let n = u64::from_be_bytes(nb);
        if n < self.recibo.n || n.saturating_sub(self.recibo.n) > 4096 {
            return Err(Error::CannotOpen);
        }
        let mut clave = [0u8; 32];
        let mut m = self.recibo.n;
        while m <= n {
            let (k, _) = self.recibo.siguiente();
            clave = k;
            m += 1;
        }
        ChaCha20Poly1305::new((&clave).into())
            .decrypt(
                &Self::nonce(n),
                Payload {
                    msg: &sellado[8..],
                    aad: &nb,
                },
            )
            .map_err(|_| Error::CannotOpen)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn both_sides_reach_the_same_secret_and_the_layer_opens_in_order() {
        let (oferta, ek) = Oferta::nueva();
        assert_eq!(ek.len(), LLAVE_BYTES);
        let (ct, s1) = encapsula(&ek).unwrap();
        assert_eq!(ct.len(), CIFRADO_BYTES);
        let s0 = oferta.abre(&ct).unwrap();
        assert_eq!(s0, s1);
        let (mut a, mut b) = (Capa::nueva(&s0, 0), Capa::nueva(&s1, 1));
        let x = a.sella(b"hola").unwrap();
        let y = a.sella(b"otra").unwrap();
        assert_ne!(&x[8..], &y[8..]);
        assert_eq!(b.abre(&x).unwrap(), b"hola");
        assert_eq!(b.abre(&y).unwrap(), b"otra");
        // The other direction has its own chain.
        let z = b.sella(b"vuelta").unwrap();
        assert_eq!(a.abre(&z).unwrap(), b"vuelta");
    }

    #[test]
    fn a_replay_and_a_changed_byte_are_refused_and_a_gap_is_passed_over() {
        let (oferta, ek) = Oferta::nueva();
        let (ct, s1) = encapsula(&ek).unwrap();
        let s0 = oferta.abre(&ct).unwrap();
        let (mut a, mut b) = (Capa::nueva(&s0, 0), Capa::nueva(&s1, 1));
        let x = a.sella(b"uno").unwrap();
        let _perdido = a.sella(b"dos").unwrap();
        let z = a.sella(b"tres").unwrap();
        assert_eq!(b.abre(&x).unwrap(), b"uno");
        assert_eq!(b.abre(&z).unwrap(), b"tres", "the lost one is skipped");
        assert!(b.abre(&x).is_err(), "replayed");
        let mut w = a.sella(b"cuatro").unwrap();
        w[12] ^= 1;
        assert!(b.abre(&w).is_err());
    }

    #[test]
    fn a_wrong_key_or_ciphertext_is_refused() {
        assert!(encapsula(&[0u8; 10]).is_err());
        let (oferta, _) = Oferta::nueva();
        assert!(oferta.abre(&[0u8; 10]).is_err());
    }
}
