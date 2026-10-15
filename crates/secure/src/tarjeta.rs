//! The signed card: one computer's two public keys, the day it was issued and a signature
//! made with its own Ed25519 key. Shown as a QR code and read in person, it pins a contact
//! without any server saying who is who. A card whose signature does not check against the key
//! inside it is refused; a card does not prove a civil identity, only that whoever shows it
//! holds that private key.

use vodozemac::{Curve25519PublicKey, Ed25519PublicKey, Ed25519Signature};

use crate::Error;

const PREFIJO: &str = "GSEC1";
const DOMINIO: &str = "GUARDIANA_SECURE_TARJETA_V1";

/// A card that was read and whose signature checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tarjeta {
    curve: Curve25519PublicKey,
    signing: Ed25519PublicKey,
    emitida: u64,
}

impl Tarjeta {
    /// The key messages are encrypted to.
    pub fn curve(&self) -> Curve25519PublicKey {
        self.curve
    }

    /// The key that signed the card.
    pub fn signing(&self) -> Ed25519PublicKey {
        self.signing
    }

    /// Unix seconds when the card was issued.
    pub fn emitida(&self) -> u64 {
        self.emitida
    }
}

fn cuerpo(curve: Curve25519PublicKey, signing: Ed25519PublicKey, emitida: u64) -> String {
    format!(
        "{DOMINIO}|{}|{}|{emitida}",
        curve.to_base64(),
        signing.to_base64()
    )
}

/// The text behind the QR: `GSEC1|curve|ed25519|unix|signature`.
pub(crate) fn escribe(identity: &crate::Identity, emitida: u64) -> String {
    let curve = identity.public_key();
    let signing = identity.signing_key();
    let firma = identity.sign(cuerpo(curve, signing, emitida).as_bytes());
    format!(
        "{PREFIJO}|{}|{}|{emitida}|{}",
        curve.to_base64(),
        signing.to_base64(),
        firma.to_base64()
    )
}

/// Read a card. Anything that is not a card signed by the key it carries is
/// [`Error::CannotOpen`], and nothing is pinned.
pub fn lee(texto: &str) -> Result<Tarjeta, Error> {
    let mut partes = texto.trim().split('|');
    if partes.next() != Some(PREFIJO) {
        return Err(Error::CannotOpen);
    }
    let curve = partes
        .next()
        .and_then(|s| Curve25519PublicKey::from_base64(s).ok())
        .ok_or(Error::CannotOpen)?;
    let signing = partes
        .next()
        .and_then(|s| Ed25519PublicKey::from_base64(s).ok())
        .ok_or(Error::CannotOpen)?;
    let emitida: u64 = partes
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or(Error::CannotOpen)?;
    let firma = partes
        .next()
        .and_then(|s| Ed25519Signature::from_base64(s).ok())
        .ok_or(Error::CannotOpen)?;
    if partes.next().is_some() {
        return Err(Error::CannotOpen);
    }
    signing
        .verify(cuerpo(curve, signing, emitida).as_bytes(), &firma)
        .map_err(|_| Error::CannotOpen)?;
    Ok(Tarjeta {
        curve,
        signing,
        emitida,
    })
}

/// The card as an SVG QR code, for the screen or a printed sheet. No image crate: text only.
pub fn qr_svg(texto: &str) -> Result<String, Error> {
    use qrcode::render::svg;
    let code = qrcode::QrCode::new(texto.as_bytes()).map_err(|_| Error::CannotOpen)?;
    Ok(code
        .render::<svg::Color>()
        .min_dimensions(220, 220)
        .quiet_zone(true)
        .build())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::Identity;

    #[test]
    fn a_card_reads_back_and_a_touched_card_does_not() {
        let yo = Identity::new();
        let texto = yo.tarjeta(1_700_000_000);
        let t = lee(&texto).unwrap();
        assert_eq!(t.curve(), yo.public_key());
        assert_eq!(t.signing(), yo.signing_key());
        assert_eq!(t.emitida(), 1_700_000_000);

        // Another date under the same signature: refused.
        let otra = texto.replacen("1700000000", "1700000001", 1);
        assert_eq!(lee(&otra).unwrap_err(), Error::CannotOpen);
        // Another computer's curve key under this signature: refused.
        let impostor = Identity::new();
        let cambiada = texto.replacen(
            &yo.public_key().to_base64(),
            &impostor.public_key().to_base64(),
            1,
        );
        assert_eq!(lee(&cambiada).unwrap_err(), Error::CannotOpen);
        assert_eq!(lee("hola").unwrap_err(), Error::CannotOpen);
        assert_eq!(lee(&format!("{texto}|x")).unwrap_err(), Error::CannotOpen);

        let svg = qr_svg(&texto).unwrap();
        assert!(
            svg.starts_with("<svg") || svg.starts_with("<?xml"),
            "{}",
            &svg[..40]
        );
    }
}
