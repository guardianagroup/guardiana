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

/// The two directions of the layer, from the shared secret, who is who (0 or 1, agreed by the
/// keys) and the transcript of the exchange (both identities, the encapsulation key and the
/// ciphertext): a secret reached through any other exchange opens nothing here.
pub struct Capa {
    /// The root every chain comes from; a renewal mixes the next ML-KEM secret into it.
    raiz: [u8; 32],
    yo: u8,
    /// Grows by one with every renewal; the first byte of every sealed message.
    epoca: u8,
    envio: Cadena,
    recibo: Cadena,
    /// The previous epoch's receiving chain, kept until the first message of the new one
    /// arrives: what the other side sealed before it switched still opens.
    anterior: Option<Cadena>,
}

/// Sealed messages per epoch before side 0 offers a new key. 18 Oct 2026, the owner: «quiero la
/// máxima tecnología disponible actualmente en el chat»: from 500 down to 50, and a clock as well
/// ([`RENUEVA_S`]), whichever comes first.
///
/// This is post-compromise security for the post-quantum layer, in the spirit of Signal's sparse
/// post-quantum ratchet (SPQR, the «triple ratchet» of 2025): a fresh ML-KEM-768 encapsulation is
/// mixed into the root inside a live conversation, not only when a link opens, so a state that
/// leaked heals at the next epoch even against an attacker with a quantum computer. It is not
/// SPQR itself: SPQR splits the encapsulation key into erasure-coded chunks carried on every
/// message; here the whole key (1184 bytes) and ciphertext (1088 bytes) travel inside Olm in two
/// messages of their own. Olm's own Curve25519 ratchet goes on underneath, unchanged.
pub const RENUEVA_CADA: u64 = 50;

/// Seconds an epoch may last before side 0 offers a new key, even in a quiet conversation.
pub const RENUEVA_S: u64 = 10 * 60;

/// Whether an epoch that sealed `sellados` messages and began at `desde` (unix s) is due for a
/// new encapsulation at `ahora`.
#[must_use]
pub fn toca_renovar(sellados: u64, desde: u64, ahora: u64) -> bool {
    sellados >= RENUEVA_CADA || ahora.saturating_sub(desde) >= RENUEVA_S
}

/// The transcript the layer is bound to.
#[must_use]
pub fn transcript(
    identidad_0: &[u8],
    identidad_1: &[u8],
    llave: &[u8],
    cifrado: &[u8],
) -> [u8; 32] {
    use sha2::Digest as _;
    let h = sha2::Sha256::new()
        .chain_update(DOMINIO)
        .chain_update(identidad_0)
        .chain_update(identidad_1)
        .chain_update(llave)
        .chain_update(cifrado)
        .finalize();
    let mut t = [0u8; 32];
    t.copy_from_slice(&h);
    t
}

impl Capa {
    /// From the secret ML-KEM gave both sides and the transcript of that exchange.
    #[must_use]
    pub fn nueva(secreto: &[u8; 32], transcript: &[u8; 32], yo: u8) -> Self {
        let mut raiz = [0u8; 32];
        hkdf(transcript, secreto, b"raiz", &mut raiz);
        let (envio, recibo) = Self::cadenas(&raiz, yo);
        Self {
            raiz,
            yo,
            epoca: 0,
            envio,
            recibo,
            anterior: None,
        }
    }

    fn cadenas(raiz: &[u8; 32], yo: u8) -> (Cadena, Cadena) {
        let (a, b) = (b"de 0 a 1".as_slice(), b"de 1 a 0".as_slice());
        let (mio, suyo) = if yo == 0 { (a, b) } else { (b, a) };
        (Cadena::nueva(raiz, mio), Cadena::nueva(raiz, suyo))
    }

    /// A new ML-KEM secret (and its transcript) mixed into the root: a new epoch with fresh
    /// chains. The other side's previous chain stays open for what it sealed before switching.
    pub fn renueva(&mut self, secreto: &[u8; 32], transcript: &[u8; 32]) {
        let mut mezcla = [0u8; 64];
        mezcla[..32].copy_from_slice(&self.raiz);
        mezcla[32..].copy_from_slice(secreto);
        let mut raiz = [0u8; 32];
        hkdf(transcript, &mezcla, b"raiz", &mut raiz);
        self.raiz = raiz;
        self.epoca = self.epoca.wrapping_add(1);
        let (envio, recibo) = Self::cadenas(&raiz, self.yo);
        self.envio = envio;
        self.anterior = Some(std::mem::replace(&mut self.recibo, recibo));
    }

    /// Messages sealed in this epoch so far.
    #[must_use]
    pub fn sellados(&self) -> u64 {
        self.envio.n
    }

    /// The current epoch.
    #[must_use]
    pub fn epoca(&self) -> u8 {
        self.epoca
    }

    fn nonce(epoca: u8, n: u64) -> Nonce {
        let mut b = [0u8; 12];
        b[0] = epoca;
        b[4..].copy_from_slice(&n.to_be_bytes());
        Nonce::from(b)
    }

    /// Seal a message: the epoch (1 byte), its number (8 bytes), then the ciphertext.
    pub fn sella(&mut self, plano: &[u8]) -> Result<Vec<u8>, Error> {
        let (clave, n) = self.envio.siguiente();
        let mut cabecera = vec![self.epoca];
        cabecera.extend_from_slice(&n.to_be_bytes());
        let c = ChaCha20Poly1305::new((&clave).into())
            .encrypt(
                &Self::nonce(self.epoca, n),
                Payload {
                    msg: plano,
                    aad: &cabecera,
                },
            )
            .map_err(|_| Error::CannotOpen)?;
        cabecera.extend_from_slice(&c);
        Ok(cabecera)
    }

    /// Open a sealed message. A number already used (a replay), one too far ahead, or an
    /// epoch that is neither the current one nor the one just left, is refused; numbers
    /// skipped by a dropped link are passed over.
    pub fn abre(&mut self, sellado: &[u8]) -> Result<Vec<u8>, Error> {
        if sellado.len() < 9 {
            return Err(Error::CannotOpen);
        }
        let epoca = sellado[0];
        let mut nb = [0u8; 8];
        nb.copy_from_slice(&sellado[1..9]);
        let n = u64::from_be_bytes(nb);
        let actual = epoca == self.epoca;
        let cadena = if actual {
            &mut self.recibo
        } else if epoca.wrapping_add(1) == self.epoca {
            self.anterior.as_mut().ok_or(Error::CannotOpen)?
        } else {
            return Err(Error::CannotOpen);
        };
        if n < cadena.n || n.saturating_sub(cadena.n) > 4096 {
            return Err(Error::CannotOpen);
        }
        let mut clave = [0u8; 32];
        while cadena.n <= n {
            let (k, _) = cadena.siguiente();
            clave = k;
        }
        let plano = ChaCha20Poly1305::new((&clave).into())
            .decrypt(
                &Self::nonce(epoca, n),
                Payload {
                    msg: &sellado[9..],
                    aad: &sellado[..9],
                },
            )
            .map_err(|_| Error::CannotOpen)?;
        if actual {
            self.anterior = None;
        }
        Ok(plano)
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
        let t = transcript(b"id0", b"id1", &ek, &ct);
        let (mut a, mut b) = (Capa::nueva(&s0, &t, 0), Capa::nueva(&s1, &t, 1));
        let x = a.sella(b"hola").unwrap();
        let y = a.sella(b"otra").unwrap();
        assert_ne!(&x[9..], &y[9..]);
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
        let t = transcript(b"id0", b"id1", &ek, &ct);
        // A different transcript (another exchange) opens nothing, even with the same secret.
        let mut a = Capa::nueva(&s0, &t, 0);
        let mut c = Capa::nueva(&s1, &transcript(b"id0", b"idX", &ek, &ct), 1);
        assert!(c.abre(&a.sella(b"cero").unwrap()).is_err());
        let (mut a, mut b) = (Capa::nueva(&s0, &t, 0), Capa::nueva(&s1, &t, 1));
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
    fn a_renewal_opens_a_new_epoch_and_what_was_sealed_before_it_still_opens() {
        let (oferta, ek) = Oferta::nueva();
        let (ct, s1) = encapsula(&ek).unwrap();
        let s0 = oferta.abre(&ct).unwrap();
        let t = transcript(b"id0", b"id1", &ek, &ct);
        let (mut a, mut b) = (Capa::nueva(&s0, &t, 0), Capa::nueva(&s1, &t, 1));
        // Side 0 sealed one in epoch 0 that is still on its way when side 1 switches.
        let en_vuelo = a.sella(b"antes").unwrap();
        let (oferta2, ek2) = Oferta::nueva();
        let (ct2, s1b) = encapsula(&ek2).unwrap();
        let s0b = oferta2.abre(&ct2).unwrap();
        let t2 = transcript(b"id0", b"id1", &ek2, &ct2);
        b.renueva(&s1b, &t2);
        assert_eq!(b.epoca(), 1);
        assert_eq!(
            b.abre(&en_vuelo).unwrap(),
            b"antes",
            "the old epoch still opens"
        );
        a.renueva(&s0b, &t2);
        let y = a.sella(b"despues").unwrap();
        assert_eq!(y[0], 1);
        assert_eq!(b.abre(&y).unwrap(), b"despues");
        // Once the new epoch spoke, the old one is closed for good.
        assert!(b.abre(&en_vuelo).is_err());
        let z = b.sella(b"vuelta").unwrap();
        assert_eq!(a.abre(&z).unwrap(), b"vuelta");
    }

    #[test]
    fn an_epoch_is_renewed_after_fifty_messages_or_ten_minutes() {
        assert!(!toca_renovar(0, 1000, 1000));
        assert!(!toca_renovar(RENUEVA_CADA - 1, 1000, 1000 + RENUEVA_S - 1));
        assert!(toca_renovar(RENUEVA_CADA, 1000, 1000));
        assert!(toca_renovar(3, 1000, 1000 + RENUEVA_S));
        assert_eq!((RENUEVA_CADA, RENUEVA_S), (50, 600));
    }

    #[test]
    fn a_wrong_key_or_ciphertext_is_refused() {
        assert!(encapsula(&[0u8; 10]).is_err());
        let (oferta, _) = Oferta::nueva();
        assert!(oferta.abre(&[0u8; 10]).is_err());
    }
}
