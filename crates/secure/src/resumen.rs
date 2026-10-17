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

/// What one computer's watch saw over the microphone, the camera and the screen (Francisco,
/// 17 Oct 2026: the summary must say «si ha habido espionaje, si ha habido grabación»). Only
/// what Windows recorded, by program name; `vigilado` is `false` where nothing could be watched
/// (another system, a phone's browser), and the summary says so instead of «nothing».
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vigilancia {
    /// Whether that computer could watch at all.
    pub vigilado: bool,
    /// Other programs that used the microphone during the session, by name.
    #[serde(default)]
    pub microfono: Vec<String>,
    /// Other programs that used the camera.
    #[serde(default)]
    pub camara: Vec<String>,
    /// Other programs that captured the screen through Windows.
    #[serde(default)]
    pub pantalla: Vec<String>,
    /// Whether the window was kept out of screenshots and screen recordings the whole time.
    #[serde(default)]
    pub pantalla_protegida: bool,
}

impl Vigilancia {
    /// Nothing else used the microphone, the camera or the screen.
    #[must_use]
    pub fn limpio(&self) -> bool {
        self.microfono.is_empty() && self.camara.is_empty() && self.pantalla.is_empty()
    }

    /// Read what the other side said, with limits (it is never trusted for more than it says).
    #[must_use]
    pub fn de_json(v: &serde_json::Value) -> Option<Self> {
        let lista = |k: &str| -> Vec<String> {
            v[k].as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(serde_json::Value::as_str)
                        .take(20)
                        .map(|s| s.chars().filter(|c| !c.is_control()).take(120).collect())
                        .collect()
                })
                .unwrap_or_default()
        };
        Some(Self {
            vigilado: v["vigilado"].as_bool()?,
            microfono: lista("microfono"),
            camara: lista("camara"),
            pantalla: lista("pantalla"),
            pantalla_protegida: v["pantalla_protegida"].as_bool().unwrap_or(false),
        })
    }
}

/// The final judgement of a session: was anyone listening, watching or recording.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Veredicto {
    /// This computer.
    pub este: Vigilancia,
    /// The other side, as its own GUARDIANA reported it inside the encrypted session; `None`
    /// when it said nothing (an older version).
    pub otro: Option<Vigilancia>,
    /// The other side talked from a phone's browser, which cannot watch its own microphone.
    pub otro_movil: bool,
    /// The contact's identity matched the safety number the whole session.
    pub identidad_verificada: bool,
    /// Signs of interference (unusual entries in the chain) during the session.
    pub interferencias: u64,
}

impl Veredicto {
    /// No sign of anyone listening, watching, recording or interfering, on either side that
    /// could tell.
    #[must_use]
    pub fn sin_espionaje(&self) -> bool {
        self.identidad_verificada
            && self.interferencias == 0
            && self.este.limpio()
            && self.otro.as_ref().is_none_or(Vigilancia::limpio)
    }
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
    /// The final judgement (17 Oct 2026). Absent in older summaries, whose signatures must still
    /// check: an absent field is not written, so their signed text is unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub veredicto: Option<Veredicto>,
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
            veredicto: None,
            firmante: identity.signing_key().to_base64(),
            firma: String::new(),
        };
        r.firma = identity.sign(r.transcript().as_bytes()).to_base64();
        Some(r)
    }

    /// The same for a part of the session (a call): the counts are of what the chain recorded
    /// between `inicio` and `fin`, and the verdict goes in, signed with the rest.
    #[allow(clippy::too_many_arguments)]
    pub fn cerrar_parte(
        identity: &Identity,
        programa: &str,
        clase: Clase,
        monitor: &Monitor,
        cadena: &Cadena,
        inicio: u64,
        fin: u64,
        archivos: Archivos,
        conexion: Conexion,
        veredicto: Veredicto,
    ) -> Option<Self> {
        let mut r = Self::cerrar(
            identity, programa, clase, monitor, cadena, inicio, fin, archivos, conexion,
        )?;
        let cuenta = |nivel: Nivel| {
            cadena
                .registros
                .iter()
                .filter(|x| x.nivel == nivel && x.cuando >= inicio && x.cuando <= fin)
                .count() as u64
        };
        r.observadas = cuenta(Nivel::Observada);
        r.inusuales = cuenta(Nivel::Inusual);
        r.no_verificables = cuenta(Nivel::NoVerificable);
        r.veredicto = Some(veredicto);
        r.firma = identity.sign(r.transcript().as_bytes()).to_base64();
        Some(r)
    }

    /// Put the verdict in and sign again (the whole conversation's summary).
    pub fn con_veredicto(mut self, identity: &Identity, veredicto: Veredicto) -> Self {
        self.veredicto = Some(veredicto);
        self.firma = identity.sign(self.transcript().as_bytes()).to_base64();
        self
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

    /// One CSV line (with its header) for a spreadsheet of sessions. The verdict and the call's
    /// figures come last, so older sheets keep their first columns.
    pub fn to_csv(&self) -> String {
        let clase = serde_json::to_string(&self.clase).unwrap_or_default();
        let v = self.veredicto.as_ref();
        let si_no = |b: bool| if b { "si" } else { "no" };
        let lista = |l: Option<&Vec<String>>| csv(&l.map(|l| l.join(" ")).unwrap_or_default());
        let num = |n: Option<u32>| n.map(|n| n.to_string()).unwrap_or_default();
        let otro = match v {
            Some(v) if v.otro_movil => "movil".to_owned(),
            Some(Veredicto { otro: Some(o), .. }) if o.vigilado => si_no(o.limpio()).to_owned(),
            Some(Veredicto { otro: Some(_), .. }) => "no_vigilado".to_owned(),
            _ => String::new(),
        };
        format!(
            "clase,contacto,verificado_en,estado_identidad,inicio,fin,duracion_s,mensajes_enviados,mensajes_recibidos,archivos_enviados,archivos_recibidos,bytes,observadas,inusuales,no_verificables,hash_cadena,firmante,sin_espionaje,vigilado,microfono_otros,camara_otros,pantalla_otros,pantalla_protegida,otro_lado_limpio,latencia_media_ms,perdida_max_pct,kbps_audio_medio,kbps_video_medio,resolucion\n{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            clase.trim_matches('"'),
            csv(&self.contacto),
            self.verificado_en,
            csv(&self.estado_identidad),
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
            self.firmante,
            v.map(|v| si_no(v.sin_espionaje())).unwrap_or_default(),
            v.map(|v| si_no(v.este.vigilado)).unwrap_or_default(),
            lista(v.map(|v| &v.este.microfono)),
            lista(v.map(|v| &v.este.camara)),
            lista(v.map(|v| &v.este.pantalla)),
            v.map(|v| si_no(v.este.pantalla_protegida)).unwrap_or_default(),
            otro,
            num(self.conexion.latencia_media_ms),
            self.conexion
                .perdida_max_pct
                .map(|p| format!("{p:.1}"))
                .unwrap_or_default(),
            num(self.conexion.kbps_audio_medio),
            num(self.conexion.kbps_video_medio),
            csv(self.conexion.resolucion.as_deref().unwrap_or_default()),
        )
    }
}

fn csv(s: &str) -> String {
    // A name that starts like a formula would run as one in a spreadsheet (17 Oct 2026 review):
    // a leading apostrophe keeps it text.
    let s = if s.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{s}")
    } else {
        s.to_owned()
    };
    let s = s.as_str();
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

    #[test]
    fn a_call_summary_carries_a_signed_verdict_and_older_ones_still_check() {
        let yo = Identity::new();
        let mut m = Monitor::new();
        m.identidad = Some(Identidad {
            nombre: "=Marta".to_owned(),
            huella: "1".to_owned(),
            propia: "a".to_owned(),
            contacto: "c".to_owned(),
            verificado_en: 900,
            estado: "verificado".to_owned(),
        });
        let mut c = Cadena::new("s", 1_000);
        // Before the call: not counted in the call's summary.
        c.anota(
            1_010,
            Nivel::Inusual,
            Hecho {
                texto: "antes".to_owned(),
                ..Hecho::default()
            },
        );
        c.anota(
            1_210,
            Nivel::Observada,
            Hecho {
                texto: "Zoom.exe usó el micrófono".to_owned(),
                ..Hecho::default()
            },
        );
        let mut v = Veredicto {
            este: Vigilancia {
                vigilado: true,
                pantalla_protegida: true,
                ..Vigilancia::default()
            },
            otro: Some(Vigilancia {
                vigilado: true,
                ..Vigilancia::default()
            }),
            otro_movil: false,
            identidad_verificada: true,
            interferencias: 0,
        };
        assert!(v.sin_espionaje());
        v.este.microfono.push("Zoom.exe".to_owned());
        assert!(!v.sin_espionaje());
        let r = Resumen::cerrar_parte(
            &yo,
            "1.0.41",
            Clase::Videollamada,
            &m,
            &c,
            1_200,
            1_500,
            Archivos::default(),
            Conexion {
                latencia_media_ms: Some(96),
                perdida_max_pct: Some(1.1),
                ..Conexion::default()
            },
            v,
        )
        .unwrap();
        assert_eq!((r.observadas, r.inusuales), (1, 0));
        assert!(r.verificar().is_ok());
        let mut t = r.clone();
        if let Some(v) = &mut t.veredicto {
            v.este.microfono.clear();
        }
        assert!(t.verificar().is_err(), "the verdict is signed");
        let fila = r.to_csv().lines().nth(1).unwrap().to_owned();
        assert!(fila.starts_with("videollamada,'=Marta,"), "{fila}");
        assert!(
            fila.ends_with(",no,si,Zoom.exe,,,si,si,96,1.1,,,"),
            "{fila}"
        );

        // A summary written before the verdict existed keeps its signature.
        let viejo = Resumen::cerrar(
            &yo,
            "1.0.31",
            Clase::Chat,
            &m,
            &c,
            1_000,
            1_100,
            Archivos::default(),
            Conexion::default(),
        )
        .unwrap();
        let texto = viejo.to_json();
        assert!(!texto.contains("veredicto"));
        assert!(Resumen::from_json(&texto).unwrap().verificar().is_ok());
    }
}
