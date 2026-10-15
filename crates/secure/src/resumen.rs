//! The summary shown when a conversation, a call or a transfer ends, and what the person can
//! download from it. Francisco, 15 Oct 2026: «resumen detallado en pantalla y descargable
//! después de la llamada o envíos o chats».
//!
//! It is built from the monitor and the chain of what was seen, so it says only what was
//! measured: who (as verified), how long, how many messages and files (never their contents or
//! names), how the connection behaved, and what other programs did. It is signed like the
//! chain, so a sheet handed to a lawyer or a client can be checked.

use serde::{Deserialize, Serialize};
use vodozemac::{Ed25519PublicKey, Ed25519Signature};

use crate::monitor::{Monitor, Nivel};
use crate::prueba::Cadena;
use crate::{Error, Identity};

const DOMINIO: &str = "GUARDIANA_SECURE_RESUMEN_V1";

/// What kind of session the summary is of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Clase {
    /// Text messages.
    Chat,
    /// A voice call.
    Llamada,
    /// A video call (Francisco, 15 Oct 2026: «videollamada también»).
    Videollamada,
    /// Files sent or received.
    Envio,
}

/// Files that travelled: how many and how much, never which.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Archivos {
    /// Files sent from this computer.
    pub enviados: u64,
    /// Files received here.
    pub recibidos: u64,
    /// Bytes, both ways, as encrypted on the wire.
    pub bytes: u64,
}

/// How the connection behaved over the session. `None` where there was no call.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Conexion {
    /// Whether the two computers talked directly the whole time.
    pub directa: Option<bool>,
    /// Mean round trip, milliseconds.
    pub latencia_media_ms: Option<u32>,
    /// Worst packet loss seen, percent.
    pub perdida_max_pct: Option<f32>,
    /// Mean audio bitrate, kilobits per second.
    pub kbps_audio_medio: Option<u32>,
    /// Mean video bitrate, kilobits per second.
    pub kbps_video_medio: Option<u32>,
    /// Audio codec.
    pub codec_audio: Option<String>,
    /// Video codec.
    pub codec_video: Option<String>,
    /// Video size most of the time.
    pub resolucion: Option<String>,
}

/// The summary itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Resumen {
    /// Format version.
    pub version: u32,
    /// The GUARDIANA version that wrote it.
    pub programa: String,
    /// What kind of session.
    pub clase: Clase,
    /// The name the owner gave the contact, as pinned.
    pub contacto: String,
    /// The safety number both people compared.
    pub huella: String,
    /// Unix seconds of the short-code check that pinned the contact.
    pub verificado_en: u64,
    /// `verificado` or `clave_cambiada` at the end of the session.
    pub estado_identidad: String,
    /// The cipher at work.
    pub cifrado: String,
    /// Unix seconds, start and end.
    pub inicio: u64,
    /// Unix seconds, end.
    pub fin: u64,
    /// Messages sent from here.
    pub mensajes_enviados: u64,
    /// Messages opened here.
    pub mensajes_recibidos: u64,
    /// Files, counted only.
    pub archivos: Archivos,
    /// The connection, when there was a call.
    pub conexion: Conexion,
    /// How many things were seen, per level.
    pub observadas: u64,
    /// Unusual ones.
    pub inusuales: u64,
    /// Not verifiable ones.
    pub no_verificables: u64,
    /// The last hash of the chain of what was seen, so this sheet and the chain match.
    pub hash_cadena: String,
    /// How many entries that chain has.
    pub registros_cadena: u64,
    /// The signing computer's Ed25519 key, base64.
    pub firmante: String,
    /// Ed25519 signature over the fields above (see [`Resumen::verificar`]).
    pub firma: String,
}

impl Resumen {
    /// Build and sign the summary of a session that just ended. `archivos` and `conexion` come
    /// from the parts that moved files and media; until they exist they are the defaults.
    #[allow(clippy::too_many_arguments)]
    pub fn cerrar(
        identity: &Identity,
        programa: &str,
        clase: Clase,
        monitor: &Monitor,
        cadena: &Cadena,
        inicio: u64,
        fin: u64,
        archivos: Archivos,
        conexion: Conexion,
    ) -> Option<Self> {
        let id = monitor.identidad.as_ref()?;
        let cuenta =
            |nivel: Nivel| cadena.registros.iter().filter(|r| r.nivel == nivel).count() as u64;
        let mut r = Self {
            version: 1,
            programa: programa.to_owned(),
            clase,
            contacto: id.nombre.clone(),
            huella: id.huella.clone(),
            verificado_en: id.verificado_en,
            estado_identidad: id.estado.clone(),
            cifrado: monitor.cifrado.algoritmo.to_owned(),
            inicio,
            fin,
            mensajes_enviados: monitor.cifrado.enviados,
            mensajes_recibidos: monitor.cifrado.recibidos,
            archivos,
            conexion,
            observadas: cuenta(Nivel::Observada),
            inusuales: cuenta(Nivel::Inusual),
            no_verificables: cuenta(Nivel::NoVerificable),
            hash_cadena: cadena.ultimo_hash(),
            registros_cadena: cadena.registros.len() as u64,
            firmante: identity.signing_key().to_base64(),
            firma: String::new(),
        };
        r.firma = identity.sign(r.transcript().as_bytes()).to_base64();
        Some(r)
    }

    /// Seconds the session lasted.
    pub fn duracion_s(&self) -> u64 {
        self.fin.saturating_sub(self.inicio)
    }

    fn transcript(&self) -> String {
        // Everything but the signature, as one deterministic text.
        let sin_firma = Self {
            firma: String::new(),
            ..self.clone()
        };
        format!(
            "{DOMINIO}|{}",
            serde_json::to_string(&sin_firma).unwrap_or_default()
        )
    }

    /// Check the signature. Passing means this exact summary was signed by the holder of
    /// `firmante`'s private key.
    pub fn verificar(&self) -> Result<Ed25519PublicKey, Error> {
        let clave = Ed25519PublicKey::from_base64(&self.firmante).map_err(|_| Error::CannotOpen)?;
        let firma = Ed25519Signature::from_base64(&self.firma).map_err(|_| Error::CannotOpen)?;
        clave
            .verify(self.transcript().as_bytes(), &firma)
            .map_err(|_| Error::CannotOpen)?;
        Ok(clave)
    }

    /// The file: JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Read a file back. Nothing is trusted until [`Resumen::verificar`] passes.
    pub fn from_json(texto: &str) -> Result<Self, Error> {
        serde_json::from_str(texto).map_err(|_| Error::CannotRestore)
    }

    /// One CSV line (with its header) for a spreadsheet of sessions.
    pub fn to_csv(&self) -> String {
        let clase = serde_json::to_string(&self.clase).unwrap_or_default();
        format!(
            "clase,contacto,verificado_en,estado_identidad,inicio,fin,duracion_s,mensajes_enviados,mensajes_recibidos,archivos_enviados,archivos_recibidos,bytes,observadas,inusuales,no_verificables,hash_cadena,firmante\n{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            clase.trim_matches('"'),
            csv(&self.contacto),
            self.verificado_en,
            self.estado_identidad,
            self.inicio,
            self.fin,
            self.duracion_s(),
            self.mensajes_enviados,
            self.mensajes_recibidos,
            self.archivos.enviados,
            self.archivos.recibidos,
            self.archivos.bytes,
            self.observadas,
            self.inusuales,
            self.no_verificables,
            self.hash_cadena,
            self.firmante
        )
    }
}

fn csv(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::monitor::Identidad;
    use crate::prueba::Hecho;

    #[test]
    fn a_summary_counts_what_was_measured_and_its_signature_holds() {
        let yo = Identity::new();
        let mut m = Monitor::new();
        assert!(Resumen::cerrar(
            &yo,
            "1.0.31",
            Clase::Chat,
            &m,
            &Cadena::new("s", 1),
            1,
            2,
            Archivos::default(),
            Conexion::default()
        )
        .is_none());
        m.identidad = Some(Identidad {
            nombre: "Marta".to_owned(),
            huella: "12345 67890".to_owned(),
            propia: "a…b".to_owned(),
            contacto: "c…d".to_owned(),
            verificado_en: 900,
            estado: "verificado".to_owned(),
        });
        m.enviado(1_001);
        m.enviado(1_002);
        m.recibido(1_003);
        let mut c = Cadena::new("s", 1_000);
        c.anota(
            1_004,
            Nivel::Observada,
            Hecho {
                texto: "x usó el micrófono".to_owned(),
                ..Hecho::default()
            },
        );
        c.anota(
            1_005,
            Nivel::NoVerificable,
            Hecho {
                texto: "portapapeles".to_owned(),
                ..Hecho::default()
            },
        );
        let r = Resumen::cerrar(
            &yo,
            "1.0.31",
            Clase::Llamada,
            &m,
            &c,
            1_000,
            1_600,
            Archivos {
                enviados: 1,
                recibidos: 0,
                bytes: 2_048,
            },
            Conexion {
                directa: Some(true),
                codec_audio: Some("Opus".to_owned()),
                ..Conexion::default()
            },
        )
        .unwrap();
        assert_eq!(r.duracion_s(), 600);
        assert_eq!((r.mensajes_enviados, r.mensajes_recibidos), (2, 1));
        assert_eq!((r.observadas, r.inusuales, r.no_verificables), (1, 0, 1));
        assert_eq!(r.hash_cadena, c.ultimo_hash());
        assert_eq!(r.verificar().unwrap(), yo.signing_key());

        let back = Resumen::from_json(&r.to_json()).unwrap();
        assert!(back.verificar().is_ok());
        let mut t = r.clone();
        t.mensajes_enviados = 200;
        assert!(t.verificar().is_err());
        let mut t = r.clone();
        t.contacto = "Otra".to_owned();
        assert!(t.verificar().is_err());

        let csv = r.to_csv();
        assert_eq!(csv.lines().count(), 2);
        assert!(csv
            .lines()
            .nth(1)
            .unwrap()
            .starts_with("llamada,Marta,900,verificado,1000,1600,600,2,1,1,0,2048,1,0,1,"));
        assert!(!r.to_json().contains("127.0.0.1"));
    }
}
