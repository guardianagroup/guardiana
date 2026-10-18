//! Destruction on an anomaly (18 Oct 2026, the owner: «si detectamos espionaje o anomalía,
//! destrucción del contenido automáticamente»).
//!
//! During a conversation or a delivery, any of these ends it at once: the verification code does
//! not match, a device that was not expected joins, another program starts capturing the screen
//! or using the microphone or the camera (the watch of `guardiana-zero`'s `vigia`), or what
//! arrives was tampered with (a bad authentication tag in a burst, a replay, a downgrade of the
//! post-quantum layer). Then this side wipes the session's keys and everything opened in memory
//! and on screen, tells the other side to do the same, and writes the event in the signed chain
//! ([`crate::prueba`]).
//!
//! The notice to the other side is signed with this computer's Ed25519 identity key over the
//! session, the reason and the second: the other side checks it against the card it verified,
//! so nobody on the road (the relay included) can forge a «destroy» to cut a conversation, and
//! nobody can replay one into another session. A notice that does not check is ignored.
//!
//! What it cannot do, said on the screen too: erase what was already saved, copied or
//! photographed on a device that was compromised.

use base64::Engine as _;
use vodozemac::{Ed25519PublicKey, Ed25519Signature};

use crate::Identity;

const DOMINIO: &str = "GUARDIANA_SECURE_DESTRUCCION_V1";

/// Why a conversation or a delivery was destroyed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Motivo {
    /// The verification numbers or the delivery code did not match.
    CodigoDistinto,
    /// A device that was not expected joined (a changed identity key, a second browser).
    DispositivoNuevo,
    /// Another program started capturing the screen.
    Captura(String),
    /// Another program started using the microphone.
    Microfono(String),
    /// Another program started using the camera.
    Camara(String),
    /// What arrived did not authenticate (several frames, or a downgrade).
    Manipulada,
    /// A message already seen arrived again.
    Repeticion,
    /// The other side destroyed it and said so (signed).
    OtroLado(String),
}

impl Motivo {
    /// The short key the page translates and the chain records.
    #[must_use]
    pub fn clave(&self) -> &'static str {
        match self {
            Self::CodigoDistinto => "codigo_distinto",
            Self::DispositivoNuevo => "dispositivo_nuevo",
            Self::Captura(_) => "captura_pantalla",
            Self::Microfono(_) => "microfono",
            Self::Camara(_) => "camara",
            Self::Manipulada => "manipulada",
            Self::Repeticion => "repeticion",
            Self::OtroLado(_) => "otro_lado",
        }
    }

    /// The program involved, when there is one (its file name, never a path).
    #[must_use]
    pub fn programa(&self) -> Option<&str> {
        match self {
            Self::Captura(p) | Self::Microfono(p) | Self::Camara(p) => Some(p),
            _ => None,
        }
    }

    /// The line for the chain, in Spanish as the chain's other lines.
    #[must_use]
    pub fn texto(&self) -> String {
        match self {
            Self::CodigoDistinto => "destruida: el código de verificación no coincidió".to_owned(),
            Self::DispositivoNuevo => "destruida: entró un dispositivo no esperado".to_owned(),
            Self::Captura(p) => format!("destruida: «{p}» empezó a capturar la pantalla"),
            Self::Microfono(p) => format!("destruida: «{p}» empezó a usar el micrófono"),
            Self::Camara(p) => format!("destruida: «{p}» empezó a usar la cámara"),
            Self::Manipulada => "destruida: llegaron datos alterados".to_owned(),
            Self::Repeticion => "destruida: llegó un mensaje repetido".to_owned(),
            Self::OtroLado(m) => format!("destruida por la otra parte: {m}"),
        }
    }

    /// From a key the other side sent (only the known ones; anything else is «otro_lado»).
    #[must_use]
    pub fn de_clave(clave: &str) -> &'static str {
        match clave {
            "codigo_distinto" => "codigo_distinto",
            "dispositivo_nuevo" => "dispositivo_nuevo",
            "captura_pantalla" => "captura_pantalla",
            "microfono" => "microfono",
            "camara" => "camara",
            "manipulada" => "manipulada",
            "repeticion" => "repeticion",
            _ => "otro_lado",
        }
    }
}

fn transcript(sesion: &str, motivo: &str, cuando: u64) -> Vec<u8> {
    format!("{DOMINIO}|{sesion}|{motivo}|{cuando}").into_bytes()
}

/// This side's signature on the notice (base64): session, reason key and second.
#[must_use]
pub fn firma(identidad: &Identity, sesion: &str, motivo: &str, cuando: u64) -> String {
    identidad
        .sign(&transcript(sesion, motivo, cuando))
        .to_base64()
}

/// Whether a notice was signed by `firmante` for this session. A notice for another session, a
/// changed reason or second, or another key, does not check.
#[must_use]
pub fn comprueba(
    firmante: Ed25519PublicKey,
    sesion: &str,
    motivo: &str,
    cuando: u64,
    firma_b64: &str,
) -> bool {
    let Ok(f) = Ed25519Signature::from_base64(firma_b64) else {
        return false;
    };
    firmante
        .verify(&transcript(sesion, motivo, cuando), &f)
        .is_ok()
}

/// A name for a session both sides compute the same way: the hash of both identity keys (in
/// order) and the second the conversation opened. Neither key nor time is secret; the name only
/// binds a notice to one conversation.
#[must_use]
pub fn sesion(identidad_0: &[u8], identidad_1: &[u8], inicio: u64) -> String {
    use sha2::Digest as _;
    let h = sha2::Sha256::new()
        .chain_update(DOMINIO.as_bytes())
        .chain_update(identidad_0)
        .chain_update(identidad_1)
        .chain_update(inicio.to_be_bytes())
        .finalize();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&h[..16])
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_notice_checks_only_for_its_session_reason_time_and_signer() {
        let yo = Identity::new();
        let otro = Identity::new();
        let s = sesion(b"a", b"b", 100);
        assert_eq!(s, sesion(b"a", b"b", 100));
        assert_ne!(s, sesion(b"a", b"b", 101));
        let f = firma(&yo, &s, "microfono", 200);
        assert!(comprueba(yo.signing_key(), &s, "microfono", 200, &f));
        assert!(!comprueba(otro.signing_key(), &s, "microfono", 200, &f));
        assert!(!comprueba(yo.signing_key(), &s, "camara", 200, &f));
        assert!(!comprueba(yo.signing_key(), &s, "microfono", 201, &f));
        assert!(!comprueba(
            yo.signing_key(),
            &sesion(b"a", b"c", 100),
            "microfono",
            200,
            &f
        ));
        assert!(!comprueba(yo.signing_key(), &s, "microfono", 200, "basura"));
    }

    #[test]
    fn reasons_have_keys_and_lines() {
        let m = Motivo::Microfono("Grabadora.exe".into());
        assert_eq!(m.clave(), "microfono");
        assert_eq!(m.programa(), Some("Grabadora.exe"));
        assert!(m.texto().contains("Grabadora.exe"));
        assert_eq!(Motivo::de_clave("camara"), "camara");
        assert_eq!(Motivo::de_clave("<script>"), "otro_lado");
        assert_eq!(Motivo::CodigoDistinto.programa(), None);
    }
}
