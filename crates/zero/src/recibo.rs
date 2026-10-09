//! Signed receipts. Each browser makes its own key (ECDSA P-256) the first time and keeps it in
//! its folder; a receipt carries its content as a string, the public key and the signature, so
//! anyone can check it with WebCrypto in any browser (guardianagroup.com/verificar does exactly
//! that, on the visitor's own machine). What a signature proves, and no more: that this receipt
//! was not changed after this browser wrote it.

use base64::Engine as _;
use ring::rand::SystemRandom;
use ring::signature::{
    EcdsaKeyPair, KeyPair, UnparsedPublicKey, ECDSA_P256_SHA256_FIXED,
    ECDSA_P256_SHA256_FIXED_SIGNING,
};
use serde::{Deserialize, Serialize};

/// The algorithm, as the receipt names it.
pub const ALGORITMO: &str = "ECDSA-P256-SHA256";

/// A receipt: content, key and signature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recibo {
    /// Always [`ALGORITMO`].
    pub algoritmo: String,
    /// The content, a JSON document kept as the exact string that was signed.
    pub contenido: String,
    /// The public key, base64 of the uncompressed point (65 bytes).
    pub clave: String,
    /// The signature, base64 of r‖s (64 bytes).
    pub firma: String,
}

/// Errors making or checking a receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The stored key could not be read.
    Clave,
    /// The system could not give random numbers or sign.
    Firma,
    /// The signature does not match.
    NoCoincide,
}

/// A new key, as PKCS#8 bytes to keep in the browser's folder.
///
/// # Errors
/// When the system's random generator fails.
pub fn clave_nueva() -> Result<Vec<u8>, Error> {
    let rng = SystemRandom::new();
    EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng)
        .map(|d| d.as_ref().to_vec())
        .map_err(|_| Error::Firma)
}

/// Sign `contenido` (a JSON document as a string) with the key in `pkcs8`.
///
/// # Errors
/// When the key cannot be read or the signature cannot be made.
pub fn firma(pkcs8: &[u8], contenido: String) -> Result<Recibo, Error> {
    let rng = SystemRandom::new();
    let par = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8, &rng)
        .map_err(|_| Error::Clave)?;
    let sig = par
        .sign(&rng, contenido.as_bytes())
        .map_err(|_| Error::Firma)?;
    let b64 = base64::engine::general_purpose::STANDARD;
    Ok(Recibo {
        algoritmo: ALGORITMO.into(),
        clave: b64.encode(par.public_key().as_ref()),
        firma: b64.encode(sig.as_ref()),
        contenido,
    })
}

/// The fingerprint of a public key: the first 16 bytes of its SHA-256 in groups of four hex
/// digits. Whoever checks a receipt compares it with the one this computer shows in Settings.
#[must_use]
pub fn huella_publica(publica: &[u8]) -> String {
    let h = ring::digest::digest(&ring::digest::SHA256, publica);
    let hexa: String = h.as_ref()[..16]
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect();
    hexa.as_bytes()
        .chunks(4)
        .map(|c| String::from_utf8_lossy(c).into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The fingerprint of the key kept in PKCS#8 form.
///
/// # Errors
/// When the key cannot be read.
pub fn huella(pkcs8: &[u8]) -> Result<String, Error> {
    let rng = SystemRandom::new();
    let par = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8, &rng)
        .map_err(|_| Error::Clave)?;
    Ok(huella_publica(par.public_key().as_ref()))
}

/// The fingerprint of the key a receipt was signed with.
#[must_use]
pub fn huella_de(r: &Recibo) -> String {
    base64::engine::general_purpose::STANDARD
        .decode(&r.clave)
        .map(|k| huella_publica(&k))
        .unwrap_or_default()
}

/// Check a receipt.
///
/// # Errors
/// [`Error::NoCoincide`] when the content, the key or the signature were changed.
pub fn comprueba(r: &Recibo) -> Result<(), Error> {
    let b64 = base64::engine::general_purpose::STANDARD;
    let clave = b64.decode(&r.clave).map_err(|_| Error::NoCoincide)?;
    let firma = b64.decode(&r.firma).map_err(|_| Error::NoCoincide)?;
    if r.algoritmo != ALGORITMO {
        return Err(Error::NoCoincide);
    }
    UnparsedPublicKey::new(&ECDSA_P256_SHA256_FIXED, clave)
        .verify(r.contenido.as_bytes(), &firma)
        .map_err(|_| Error::NoCoincide)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_receipt_checks_until_someone_touches_it() {
        let k = clave_nueva().unwrap();
        let r = firma(&k, r#"{"tipo":"mandato","cortes":3}"#.into()).unwrap();
        assert_eq!(comprueba(&r), Ok(()));
        // The receipt names the key that signed it, as Settings shows it.
        assert_eq!(huella_de(&r), huella(&k).unwrap());
        assert_eq!(huella_de(&r).len(), 39);
        let mut tocado = r.clone();
        tocado.contenido = r#"{"tipo":"mandato","cortes":0}"#.into();
        assert_eq!(comprueba(&tocado), Err(Error::NoCoincide));
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(&r.clave)
                .unwrap()
                .len(),
            65
        );
    }
}
