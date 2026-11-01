//! The tunnel's post-quantum preshared key.
//!
//! At every sign-up the computer makes a fresh ML-KEM-768 key pair and sends the encapsulation key; the server
//! encapsulates a secret to it and sends back the ciphertext. From that secret both sides derive, with HKDF-SHA256 and
//! both WireGuard public keys as context, the 32-byte preshared key WireGuard mixes into its handshake. The ML-KEM key is
//! thrown away after use, so a later theft of the computer or of the server does not open recorded tunnels.

use ml_kem::kem::{Decapsulate as _, Encapsulate as _, Kem as _, KeyExport as _};
use ml_kem::{Ciphertext, EncapsulationKey768, MlKem768};

use crate::Error;

/// Bytes of an ML-KEM-768 encapsulation key, as FIPS 203 fixes them.
pub const LLAVE_BYTES: usize = 1184;
/// Bytes of an ML-KEM-768 ciphertext.
pub const CIFRADO_BYTES: usize = 1088;
const DOMINIO: &[u8] = b"GUARDIANA_VPN_PSK_V1";

/// The computer's half: the decapsulation key, kept only until the server answers.
pub struct Oferta {
    dk: ml_kem::DecapsulationKey768,
}

impl Oferta {
    /// A fresh ML-KEM-768 key pair; the bytes go to the server with the sign-up.
    #[must_use]
    pub fn nueva() -> (Self, Vec<u8>) {
        let (dk, ek) = MlKem768::generate_keypair();
        (Self { dk }, ek.to_bytes().to_vec())
    }

    /// The shared secret, from the ciphertext the server sent back.
    pub fn abre(&self, cifrado: &[u8]) -> Result<[u8; 32], Error> {
        let ct = Ciphertext::<MlKem768>::try_from(cifrado)
            .map_err(|_| Error::Formato("cifrado ML-KEM"))?;
        let ss = self.dk.decapsulate(&ct);
        let mut k = [0u8; 32];
        k.copy_from_slice(ss.as_slice());
        Ok(k)
    }
}

/// The server's step (also used by the tests): a secret for the holder of `llave`, and its ciphertext.
pub fn encapsula(llave: &[u8]) -> Result<(Vec<u8>, [u8; 32]), Error> {
    let bytes = ml_kem::Key::<EncapsulationKey768>::try_from(llave)
        .map_err(|_| Error::Formato("llave ML-KEM"))?;
    let ek = EncapsulationKey768::new(&bytes).map_err(|_| Error::Formato("llave ML-KEM"))?;
    let (ct, ss) = ek.encapsulate();
    let mut k = [0u8; 32];
    k.copy_from_slice(ss.as_slice());
    Ok((ct.to_vec(), k))
}

/// The preshared key both sides derive: HKDF-SHA256 over the ML-KEM secret, bound to both WireGuard public keys.
pub fn psk(secreto: &[u8; 32], cliente: &[u8; 32], servidor: &[u8; 32]) -> Result<[u8; 32], Error> {
    let mut info = Vec::with_capacity(DOMINIO.len() + 64);
    info.extend_from_slice(DOMINIO);
    info.extend_from_slice(cliente);
    info.extend_from_slice(servidor);
    let prk = ring::hkdf::Salt::new(ring::hkdf::HKDF_SHA256, DOMINIO).extract(secreto);
    let mut k = [0u8; 32];
    prk.expand(&[info.as_slice()], ring::hkdf::HKDF_SHA256)
        .and_then(|okm| okm.fill(&mut k))
        .map_err(|_| Error::Formato("HKDF"))?;
    Ok(k)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn los_dos_lados_llegan_a_la_misma_llave() {
        let (oferta, ek) = Oferta::nueva();
        assert_eq!(ek.len(), LLAVE_BYTES);
        let (ct, del_servidor) = encapsula(&ek).unwrap();
        assert_eq!(ct.len(), CIFRADO_BYTES);
        let del_equipo = oferta.abre(&ct).unwrap();
        assert_eq!(del_equipo, del_servidor);
        let (c, s) = ([1u8; 32], [2u8; 32]);
        assert_eq!(
            psk(&del_equipo, &c, &s).unwrap(),
            psk(&del_servidor, &c, &s).unwrap()
        );
    }

    #[test]
    fn la_llave_depende_de_las_dos_partes() {
        let ss = [7u8; 32];
        let base = psk(&ss, &[1; 32], &[2; 32]).unwrap();
        assert_ne!(base, psk(&ss, &[2; 32], &[1; 32]).unwrap());
        assert_ne!(base, psk(&[8u8; 32], &[1; 32], &[2; 32]).unwrap());
        assert_ne!(base, ss);
    }

    #[test]
    fn lo_que_no_es_ml_kem_no_entra() {
        let (oferta, _) = Oferta::nueva();
        assert!(oferta.abre(&[0u8; 10]).is_err());
        assert!(encapsula(&[0u8; 10]).is_err());
    }
}
