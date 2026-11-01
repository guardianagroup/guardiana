//! The computer's WireGuard key pair (Curve25519). The private half never leaves the program: it goes to the Windows
//! adapter in memory and, between runs, is kept encrypted by the program like its other secrets.

use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::{Zeroize as _, Zeroizing};

use crate::{b64, de_b64, Error};

/// A WireGuard key pair.
pub struct Llaves {
    secreta: StaticSecret,
    publica: PublicKey,
}

impl std::fmt::Debug for Llaves {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // never the private half, not even in a log
        f.debug_struct("Llaves")
            .field("publica", &self.publica_base64())
            .finish_non_exhaustive()
    }
}

impl Llaves {
    /// A fresh key pair, from the system's randomness.
    pub fn nuevas() -> Result<Self, Error> {
        let mut b = [0u8; 32];
        getrandom::fill(&mut b).map_err(|_| Error::SinAzar)?;
        let llaves = Self::desde_bytes(b);
        b.zeroize();
        Ok(llaves)
    }

    /// The pair from its private half (32 bytes; Curve25519 clamps it as WireGuard does).
    #[must_use]
    pub fn desde_bytes(secreta: [u8; 32]) -> Self {
        let secreta = StaticSecret::from(secreta);
        let publica = PublicKey::from(&secreta);
        Self { secreta, publica }
    }

    /// The pair from its private half in base64, as WireGuard writes it.
    pub fn desde_base64(s: &str) -> Result<Self, Error> {
        let v = Zeroizing::new(de_b64(s).ok_or(Error::Formato("llave privada"))?);
        let mut b: [u8; 32] = v
            .as_slice()
            .try_into()
            .map_err(|_| Error::Formato("llave privada"))?;
        let llaves = Self::desde_bytes(b);
        b.zeroize();
        Ok(llaves)
    }

    /// The public half, 32 bytes.
    #[must_use]
    pub fn publica(&self) -> [u8; 32] {
        self.publica.to_bytes()
    }

    /// The public half in base64, as WireGuard and the server write it.
    #[must_use]
    pub fn publica_base64(&self) -> String {
        b64(self.publica.as_bytes())
    }

    /// The private half in base64, for the adapter's configuration only; wiped from memory when dropped.
    #[must_use]
    pub fn secreta_base64(&self) -> Zeroizing<String> {
        Zeroizing::new(b64(&self.secreta.to_bytes()))
    }
}

/// A public key in base64 (44 characters), as 32 bytes.
pub fn publica_de_base64(s: &str) -> Result<[u8; 32], Error> {
    de_b64(s)
        .and_then(|v| <[u8; 32]>::try_from(v.as_slice()).ok())
        .ok_or(Error::Formato("llave pública"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn la_misma_cuenta_que_wireguard() {
        // RFC 7748 §6.1: la llave de Alice y su parte pública
        let alice: [u8; 32] = [
            0x77, 0x07, 0x6d, 0x0a, 0x73, 0x18, 0xa5, 0x7d, 0x3c, 0x16, 0xc1, 0x72, 0x51, 0xb2,
            0x66, 0x45, 0xdf, 0x4c, 0x2f, 0x87, 0xeb, 0xc0, 0x99, 0x2a, 0xb1, 0x77, 0xfb, 0xa5,
            0x1d, 0xb9, 0x2c, 0x2a,
        ];
        let publica: [u8; 32] = [
            0x85, 0x20, 0xf0, 0x09, 0x89, 0x30, 0xa7, 0x54, 0x74, 0x8b, 0x7d, 0xdc, 0xb4, 0x3e,
            0xf7, 0x5a, 0x0d, 0xbf, 0x3a, 0x0d, 0x26, 0x38, 0x1a, 0xf4, 0xeb, 0xa4, 0xa9, 0x8e,
            0xaa, 0x9b, 0x4e, 0x6a,
        ];
        assert_eq!(Llaves::desde_bytes(alice).publica(), publica);
    }

    #[test]
    fn ida_y_vuelta_en_base64_sin_ensenar_la_privada() {
        let l = Llaves::nuevas().unwrap();
        let otra = Llaves::desde_base64(&l.secreta_base64()).unwrap();
        assert_eq!(otra.publica(), l.publica());
        assert_eq!(l.publica_base64().len(), 44);
        assert_eq!(publica_de_base64(&l.publica_base64()).unwrap(), l.publica());
        let visto = format!("{l:?}");
        assert!(!visto.contains(l.secreta_base64().as_str()));
        assert!(Llaves::desde_base64("no es una llave").is_err());
        assert!(publica_de_base64("AAAA").is_err());
    }

    #[test]
    fn dos_llaves_nuevas_no_se_parecen() {
        assert_ne!(
            Llaves::nuevas().unwrap().publica(),
            Llaves::nuevas().unwrap().publica()
        );
    }
}
