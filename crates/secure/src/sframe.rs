//! The media layer: every audio or video frame encrypted and authenticated on its own, with a
//! key both computers share only through Olm. The transport (DTLS-SRTP) keeps its own keys;
//! this layer sits above it, so a relay, a media server or a library bug in the transport never
//! sees a frame in the clear. This is the shape of SFrame (RFC 9605): a per-frame counter, a
//! key derived per purpose, AEAD over the frame with the header as associated data, and a
//! ratchet that moves the key forward every so many frames.
//!
//! Chosen on 15 Oct 2026 with Grok and ChatGPT: «SFrame sobre los fotogramas, claves frescas
//! dentro de Olm, derivación por propósito con HKDF; no tocar las claves de DTLS-SRTP».
//!
//! HKDF (RFC 5869) and HMAC (RFC 2104) are written here over the SHA-256 the crate already
//! has, and checked against the RFCs' own test vectors below.

use chacha20poly1305::aead::{Aead as _, KeyInit as _, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use sha2::{Digest as _, Sha256};

use crate::Error;

const DOMINIO: &[u8] = b"GUARDIANA_SECURE_SFRAME_V1";
/// Frames per key before the ratchet moves forward.
pub const RATCHET_CADA: u64 = 4096;
/// How far behind the current ratchet a receiver still opens frames (late or reordered ones).
const RATCHET_ATRAS: u64 = 2;

fn hmac(key: &[u8], msg: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        k[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let ipad: Vec<u8> = k.iter().map(|b| b ^ 0x36).collect();
    let opad: Vec<u8> = k.iter().map(|b| b ^ 0x5c).collect();
    let inner = Sha256::new()
        .chain_update(ipad)
        .chain_update(msg)
        .finalize();
    let outer = Sha256::new()
        .chain_update(opad)
        .chain_update(inner)
        .finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&outer);
    out
}

/// HKDF-SHA256: extract with `salt`, expand `info` to `out.len()` bytes (at most 255 × 32).
pub(crate) fn hkdf(salt: &[u8], ikm: &[u8], info: &[u8], out: &mut [u8]) {
    let prk = hmac(salt, ikm);
    let mut previo: Vec<u8> = Vec::new();
    let mut hecho = 0;
    let mut i: u8 = 1;
    while hecho < out.len() {
        let mut m = previo.clone();
        m.extend_from_slice(info);
        m.push(i);
        let t = hmac(&prk, &m);
        let n = (out.len() - hecho).min(32);
        out[hecho..hecho + n].copy_from_slice(&t[..n]);
        previo = t.to_vec();
        hecho += n;
        i = i.wrapping_add(1);
    }
}

/// What a frame carries in the clear: who sent it, which key generation, which frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cabecera {
    /// The sender's number in the call (0 or 1 in a two-computer call).
    pub emisor: u8,
    /// Ratchet generation of the key.
    pub generacion: u64,
    /// Frame counter within this sender's stream.
    pub contador: u64,
}

impl Cabecera {
    fn bytes(self) -> [u8; 17] {
        let mut b = [0u8; 17];
        b[0] = self.emisor;
        b[1..9].copy_from_slice(&self.generacion.to_be_bytes());
        b[9..].copy_from_slice(&self.contador.to_be_bytes());
        b
    }

    fn lee(b: &[u8]) -> Option<(Self, &[u8])> {
        if b.len() < 17 {
            return None;
        }
        let gen = u64::from_be_bytes(b[1..9].try_into().ok()?);
        let cnt = u64::from_be_bytes(b[9..17].try_into().ok()?);
        Some((
            Self {
                emisor: b[0],
                generacion: gen,
                contador: cnt,
            },
            &b[17..],
        ))
    }
}

/// The key material for one direction, as derived from the media key both sides got in Olm.
struct Generacion {
    cipher: ChaCha20Poly1305,
    salt: [u8; 12],
}

fn deriva(base: &[u8; 32], emisor: u8, generacion: u64) -> Generacion {
    let mut info = DOMINIO.to_vec();
    info.push(emisor);
    info.extend_from_slice(&generacion.to_be_bytes());
    let mut k = [0u8; 32];
    let mut s = [0u8; 12];
    hkdf(b"clave", base, &info, &mut k);
    hkdf(b"sal", base, &info, &mut s);
    Generacion {
        cipher: ChaCha20Poly1305::new((&k).into()),
        salt: s,
    }
}

fn avanza(base: &[u8; 32]) -> [u8; 32] {
    let mut siguiente = [0u8; 32];
    hkdf(b"ratchet", base, DOMINIO, &mut siguiente);
    siguiente
}

fn nonce(salt: &[u8; 12], contador: u64) -> Nonce {
    let mut n = *salt;
    for (i, b) in contador.to_be_bytes().iter().enumerate() {
        n[4 + i] ^= b;
    }
    Nonce::from(n)
}

/// One sender's side: encrypts frames, moving the key forward every [`RATCHET_CADA`].
pub struct Emisor {
    emisor: u8,
    base: [u8; 32],
    generacion: u64,
    contador: u64,
    actual: Generacion,
}

impl Emisor {
    /// From the 32-byte media key carried in Olm.
    pub fn new(clave_medios: &[u8; 32], emisor: u8) -> Self {
        Self {
            emisor,
            base: *clave_medios,
            generacion: 0,
            contador: 0,
            actual: deriva(clave_medios, emisor, 0),
        }
    }

    /// Encrypt one frame. The output is the header in the clear followed by the ciphertext.
    pub fn cifra(&mut self, frame: &[u8]) -> Result<Vec<u8>, Error> {
        if self.contador >= RATCHET_CADA {
            self.base = avanza(&self.base);
            self.generacion += 1;
            self.contador = 0;
            self.actual = deriva(&self.base, self.emisor, self.generacion);
        }
        let cab = Cabecera {
            emisor: self.emisor,
            generacion: self.generacion,
            contador: self.contador,
        };
        let h = cab.bytes();
        let c = self
            .actual
            .cipher
            .encrypt(
                &nonce(&self.actual.salt, cab.contador),
                Payload {
                    msg: frame,
                    aad: &h,
                },
            )
            .map_err(|_| Error::CannotOpen)?;
        self.contador += 1;
        let mut out = h.to_vec();
        out.extend_from_slice(&c);
        Ok(out)
    }

    /// Which generation the key is in now (for the screen).
    pub fn generacion(&self) -> u64 {
        self.generacion
    }
}

/// The other side: opens frames from one sender, following its ratchet and refusing replays.
pub struct Receptor {
    emisor: u8,
    bases: Vec<(u64, [u8; 32])>,
    ultimo: Option<(u64, u64)>,
    vistos: std::collections::BTreeSet<(u64, u64)>,
}

impl Receptor {
    /// From the same media key, for the given sender.
    pub fn new(clave_medios: &[u8; 32], emisor: u8) -> Self {
        Self {
            emisor,
            bases: vec![(0, *clave_medios)],
            ultimo: None,
            vistos: std::collections::BTreeSet::new(),
        }
    }

    fn base_de(&mut self, generacion: u64) -> Option<[u8; 32]> {
        let (mut g, mut b) = *self.bases.last()?;
        if generacion + RATCHET_ATRAS < g {
            return None;
        }
        if let Some((_, base)) = self.bases.iter().find(|(x, _)| *x == generacion) {
            return Some(*base);
        }
        if generacion > g + 64 {
            return None;
        }
        while g < generacion {
            b = avanza(&b);
            g += 1;
            self.bases.push((g, b));
        }
        let min = g.saturating_sub(RATCHET_ATRAS);
        self.bases.retain(|(x, _)| *x >= min);
        Some(b)
    }

    /// Open one frame. A frame already seen, from another sender, from a generation too far
    /// back or forward, or touched on the road is [`Error::CannotOpen`].
    pub fn abre(&mut self, bytes: &[u8]) -> Result<Vec<u8>, Error> {
        let (cab, cuerpo) = Cabecera::lee(bytes).ok_or(Error::CannotOpen)?;
        if cab.emisor != self.emisor {
            return Err(Error::CannotOpen);
        }
        if self.vistos.contains(&(cab.generacion, cab.contador)) {
            return Err(Error::CannotOpen);
        }
        let base = self.base_de(cab.generacion).ok_or(Error::CannotOpen)?;
        let g = deriva(&base, self.emisor, cab.generacion);
        let plano = g
            .cipher
            .decrypt(
                &nonce(&g.salt, cab.contador),
                Payload {
                    msg: cuerpo,
                    aad: &cab.bytes(),
                },
            )
            .map_err(|_| Error::CannotOpen)?;
        self.vistos.insert((cab.generacion, cab.contador));
        if self.vistos.len() > 8192 {
            if let Some(primero) = self.vistos.iter().next().copied() {
                self.vistos.remove(&primero);
            }
        }
        self.ultimo = Some((cab.generacion, cab.contador));
        Ok(plano)
    }

    /// The last frame opened, for the screen.
    pub fn ultimo(&self) -> Option<Cabecera> {
        self.ultimo.map(|(generacion, contador)| Cabecera {
            emisor: self.emisor,
            generacion,
            contador,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    #[test]
    fn hmac_and_hkdf_match_the_rfc_test_vectors() {
        // RFC 4231, test case 2.
        assert_eq!(
            hex(&hmac(b"Jefe", b"what do ya want for nothing?")),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        // RFC 5869, test case 1.
        let ikm = [0x0bu8; 22];
        let salt: Vec<u8> = (0u8..=0x0c).collect();
        let info: Vec<u8> = (0xf0u8..=0xf9).collect();
        let mut okm = [0u8; 42];
        hkdf(&salt, &ikm, &info, &mut okm);
        assert_eq!(
            hex(&okm),
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"
        );
    }

    #[test]
    fn frames_open_in_order_out_of_order_and_across_the_ratchet_but_never_twice() {
        let clave = [7u8; 32];
        let mut tx = Emisor::new(&clave, 0);
        let mut rx = Receptor::new(&clave, 0);
        let f1 = tx.cifra(b"uno").unwrap();
        let f2 = tx.cifra(b"dos").unwrap();
        assert!(!f1.windows(3).any(|w| w == b"uno"));
        assert_eq!(rx.abre(&f2).unwrap(), b"dos");
        assert_eq!(rx.abre(&f1).unwrap(), b"uno");
        assert_eq!(rx.abre(&f1).unwrap_err(), Error::CannotOpen);
        // A touched byte.
        let mut t = f2.clone();
        *t.last_mut().unwrap() ^= 1;
        assert!(rx.abre(&t).is_err());
        // Another sender's number under the same bytes.
        let mut t = f2.clone();
        t[0] = 1;
        assert!(rx.abre(&t).is_err());
        // Across the ratchet: the receiver follows on its own.
        let mut guardado = Vec::new();
        for i in 0..(RATCHET_CADA + 5) {
            let f = tx.cifra(format!("f{i}").as_bytes()).unwrap();
            if i == 3 || i == RATCHET_CADA || i == RATCHET_CADA + 4 {
                guardado.push((i, f));
            }
        }
        assert_eq!(tx.generacion(), 1);
        for (i, f) in guardado {
            assert_eq!(rx.abre(&f).unwrap(), format!("f{i}").as_bytes());
        }
        assert_eq!(rx.ultimo().unwrap().generacion, 1);
        // A different media key opens nothing.
        let mut otro = Receptor::new(&[8u8; 32], 0);
        assert!(otro.abre(&tx.cifra(b"x").unwrap()).is_err());
        // The other direction uses its own derived key.
        let mut tx1 = Emisor::new(&clave, 1);
        let f = tx1.cifra(b"vuelta").unwrap();
        assert!(rx.abre(&f).is_err());
        assert_eq!(Receptor::new(&clave, 1).abre(&f).unwrap(), b"vuelta");
    }
}
