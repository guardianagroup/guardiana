//! What the screen shows the whole time SECURE is in use. Francisco, 15 Oct 2026:
//! «monitoreo en pantalla constante … para que el cliente vea todos los datos posibles».
//!
//! Every field here is either a fact this crate produced (keys, counts) or `None`: the screen
//! prints «—» for `None`, never a guess. Nothing in it is an address, a key secret or a message.
//! Transport figures and the watch over other programs are filled by the later parts of the
//! plan (calls, vigilance); the shape is fixed here so the screen is the same from day one.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

/// How sure an entry is. Never «you are being spied on».
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Nivel {
    /// Something this crate did: a verification, a key rotation, a message.
    Info,
    /// Another program was seen doing something concrete (the name of the program is in the text).
    Observada,
    /// Something changed that does not prove anything on its own (a new connection, say).
    Inusual,
    /// Windows gives no way to see this without a driver; the screen says so.
    NoVerificable,
}

/// One line of the history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Suceso {
    /// Unix seconds.
    pub cuando: u64,
    /// How sure the entry is.
    pub nivel: Nivel,
    /// What happened, in the person's language, naming the program when one was seen.
    pub texto: String,
}

/// Who this computer is talking to, as verified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Identidad {
    /// The name the owner gave the contact.
    pub nombre: String,
    /// The safety number both people compared ([`crate::agenda::huella`]).
    pub huella: String,
    /// Short form of this computer's own signing key.
    pub propia: String,
    /// Short form of the contact's signing key.
    pub contacto: String,
    /// Unix seconds of the short-code check.
    pub verificado_en: u64,
    /// `verificado` or `clave_cambiada`.
    pub estado: String,
}

/// The cipher at work.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Cifrado {
    /// Always Olm (double ratchet) in this version; printed so the person sees it.
    pub algoritmo: &'static str,
    /// Messages encrypted here since the session opened.
    pub enviados: u64,
    /// Messages opened here since the session opened.
    pub recibidos: u64,
    /// Unix seconds of the last message either way.
    pub ultimo: Option<u64>,
    /// Whether the media frames carry their own layer (SFrame). `None` until there is a call.
    pub sframe: Option<bool>,
}

/// The connection, once there is one. Never an address.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Transporte {
    /// `true`: the two computers talk directly; `false`: through a relay that sees only ciphertext.
    pub directo: Option<bool>,
    /// Whether the DTLS fingerprint matched the one signed inside the session.
    pub dtls_verificado: Option<bool>,
    /// Round trip, milliseconds.
    pub latencia_ms: Option<u32>,
    /// Packets lost, percent.
    pub perdida_pct: Option<f32>,
    /// Jitter, milliseconds.
    pub jitter_ms: Option<u32>,
    /// Audio bitrate, kilobits per second.
    pub kbps_audio: Option<u32>,
    /// Video bitrate, kilobits per second.
    pub kbps_video: Option<u32>,
    /// Audio codec name (Opus).
    pub codec_audio: Option<String>,
    /// Video codec name (AV1, VP9…).
    pub codec_video: Option<String>,
    /// Video size, as `1920×1080`.
    pub resolucion: Option<String>,
}

/// The whole screen, as one value the page can draw every second.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Monitor {
    /// The verified contact of the open conversation, if any.
    pub identidad: Option<Identidad>,
    /// The cipher at work.
    pub cifrado: Cifrado,
    /// The connection, once there is one.
    pub transporte: Transporte,
    /// Running, or stopped with its reason ([`crate::pausa`]).
    pub sesion: crate::pausa::Sesion,
    /// What other programs were seen doing during the session, newest last.
    pub vigilancia: Vec<Suceso>,
    /// The last [`Monitor::HISTORICO`] events of every kind, oldest first.
    pub historico: VecDeque<Suceso>,
}

impl Monitor {
    /// How many events the history keeps.
    pub const HISTORICO: usize = 50;

    /// An empty screen: no contact, no messages, nothing seen.
    pub fn new() -> Self {
        Self {
            cifrado: Cifrado {
                algoritmo: "Olm · doble trinquete (vodozemac)",
                ..Cifrado::default()
            },
            ..Self::default()
        }
    }

    /// Add to the history, dropping the oldest past the limit. A vigilance entry
    /// (`Observada`, `Inusual`, `NoVerificable`) is also kept in its own list.
    pub fn anota(&mut self, cuando: u64, nivel: Nivel, texto: impl Into<String>) {
        let s = Suceso {
            cuando,
            nivel,
            texto: texto.into(),
        };
        if nivel != Nivel::Info {
            self.vigilancia.push(s.clone());
            // The newest 200 are enough to read; the list goes in every state the screens get.
            if self.vigilancia.len() > 200 {
                let sobra = self.vigilancia.len() - 200;
                self.vigilancia.drain(..sobra);
            }
        }
        self.historico.push_back(s);
        while self.historico.len() > Self::HISTORICO {
            self.historico.pop_front();
        }
    }

    /// One message left this computer.
    pub fn enviado(&mut self, cuando: u64) {
        self.cifrado.enviados += 1;
        self.cifrado.ultimo = Some(cuando);
    }

    /// One message was opened here.
    pub fn recibido(&mut self, cuando: u64) {
        self.cifrado.recibidos += 1;
        self.cifrado.ultimo = Some(cuando);
    }

    /// The screen as JSON for the page.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_owned())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn the_history_keeps_the_last_fifty_and_the_watch_keeps_only_what_was_seen() {
        let mut m = Monitor::new();
        for i in 0..60 {
            m.anota(i, Nivel::Info, format!("mensaje {i}"));
        }
        assert_eq!(m.historico.len(), Monitor::HISTORICO);
        assert_eq!(m.historico.front().unwrap().cuando, 10);
        assert!(m.vigilancia.is_empty());
        m.anota(61, Nivel::Observada, "ejemplo.exe usó el micrófono");
        m.anota(62, Nivel::NoVerificable, "quién leyó el portapapeles");
        assert_eq!(m.vigilancia.len(), 2);
        m.enviado(63);
        m.recibido(64);
        let json = m.to_json();
        assert!(json.contains("\"enviados\":1") && json.contains("\"recibidos\":1"));
        assert!(json.contains("\"no_verificable\""));
        assert!(json.contains("\"latencia_ms\":null"));
        assert!(!json.contains("127.0.0.1"));
    }
}
