//! What the screen shows the whole time SECURE is in use. Francisco, 15 Oct 2026:
//! «monitoreo en pantalla constante … para que el cliente vea todos los datos posibles».
//!
//! Every field here is either a fact this crate produced (keys, counts) or `None`: the screen
//! prints «—» for `None`, never a guess. Nothing in it is an address, a key secret or a message.
//! Transport figures and the watch over other programs are filled by the later parts of the
//! plan (calls, vigilance); the shape is fixed here so the screen is the same from day one.

use std::collections::VecDeque;

use serde::Serialize;

/// How sure an entry is. Never «you are being spied on».
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
    pub nivel: Nivel,
    pub texto: String,
}

/// Who this computer is talking to, as verified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Identidad {
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
    pub enviados: u64,
    pub recibidos: u64,
    /// Unix seconds of the last message either way.
    pub ultimo: Option<u64>,
    /// Whether the media frames carry their own layer (SFrame). `None` until there is a call.
    pub sframe: Option<bool>,
}

/// The connection, once there is one. Never an address.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Transporte {
    pub directo: Option<bool>,
    pub dtls_verificado: Option<bool>,
    pub latencia_ms: Option<u32>,
    pub perdida_pct: Option<f32>,
    pub jitter_ms: Option<u32>,
    pub kbps_audio: Option<u32>,
    pub kbps_video: Option<u32>,
    pub codec_audio: Option<String>,
    pub codec_video: Option<String>,
    pub resolucion: Option<String>,
}

/// The whole screen, as one value the page can draw every second.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Monitor {
    pub identidad: Option<Identidad>,
    pub cifrado: Cifrado,
    pub transporte: Transporte,
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
