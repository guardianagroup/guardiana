//! One message on the screen of a conversation, with everything the big chat apps show about
//! it (Francisco, 16 Oct 2026: «copia el sistema de Telegram o WhatsApp»): who, when, whether
//! it left, arrived or was seen (✓, ✓✓, ✓✓ green), whether it was edited or deleted, what it
//! answers, the reactions it got, and — for a call — how it went.
//!
//! Every message has an id both sides know, so «delivered», «seen», «edit», «delete» and
//! «react» can point at it. The id is random: the two sides never collide and nobody can
//! guess the next one.

use serde_json::{json, Value};

/// Where a message of ours stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estado {
    /// Not sent yet (no link, or the lock is paused).
    Pendiente,
    /// Left this computer.
    Enviado,
    /// The other computer took it.
    Entregado,
    /// The other person had it on screen.
    Visto,
}

impl Estado {
    fn nombre(self) -> &'static str {
        match self {
            Self::Pendiente => "pendiente",
            Self::Enviado => "enviado",
            Self::Entregado => "entregado",
            Self::Visto => "visto",
        }
    }
}

/// A call, as a line in the conversation (16 Oct 2026: «regístrame también el historial de
/// llamadas para que se refleje en el chat»).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Llamada {
    /// With camera.
    pub video: bool,
    /// Started from here.
    pub saliente: bool,
    /// How long it lasted once connected; `None` when it never connected.
    pub segundos: Option<u64>,
    /// How it ended when it never connected: `perdida` (rang here, nobody answered),
    /// `rechazada`, `sin_respuesta` (we called, nobody answered), `cancelada` (we hung up
    /// before the answer).
    pub fin: &'static str,
}

/// A message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mensaje {
    /// Shared with the other side.
    pub id: u64,
    /// Unix seconds.
    pub t: u64,
    /// Written here.
    pub mio: bool,
    /// The text (empty once deleted).
    pub texto: String,
    /// Who wrote it, for a group; empty for the other end of a two-person link.
    pub de: String,
    /// Where it stands (ours only; for the other side's it is always `Visto`).
    pub estado: Estado,
    /// The text was changed after sending.
    pub editado: bool,
    /// Deleted for everyone: the line stays as «message deleted».
    pub borrado: bool,
    /// The id of the message it answers.
    pub re: Option<u64>,
    /// (who, emoji); one per person.
    pub reacciones: Vec<(String, String)>,
    /// A call line instead of a text.
    pub llamada: Option<Llamada>,
    /// A file line: the name shown.
    pub archivo: Option<String>,
    /// The file's id on the phone link (0: none handed over).
    pub archivo_id: u32,
}

impl Mensaje {
    /// A fresh id: 63 random bits (never 0, which means «none» on the wire).
    pub fn nuevo_id() -> u64 {
        use ring::rand::{SecureRandom, SystemRandom};
        let mut b = [0u8; 8];
        let _ = SystemRandom::new().fill(&mut b);
        (u64::from_le_bytes(b) >> 1).max(1)
    }

    /// A text written here, about to be sent.
    pub fn mio(texto: String, re: Option<u64>) -> Self {
        Self::texto(Self::nuevo_id(), ahora(), true, texto, String::new(), re)
    }

    /// A text from the other side.
    pub fn del_otro(id: u64, texto: String, de: String, re: Option<u64>) -> Self {
        Self::texto(id, ahora(), false, texto, de, re)
    }

    /// Any text.
    pub fn texto(id: u64, t: u64, mio: bool, texto: String, de: String, re: Option<u64>) -> Self {
        Self {
            id,
            t,
            mio,
            texto,
            de,
            estado: if mio { Estado::Enviado } else { Estado::Visto },
            editado: false,
            borrado: false,
            re,
            reacciones: Vec::new(),
            llamada: None,
            archivo: None,
            archivo_id: 0,
        }
    }

    /// A file that went or came.
    pub fn archivo(nombre: String, mio: bool) -> Self {
        let mut m = Self::texto(
            Self::nuevo_id(),
            ahora(),
            mio,
            String::new(),
            String::new(),
            None,
        );
        m.archivo = Some(nombre);
        m.estado = Estado::Visto;
        m
    }

    /// A call that just ended, as a line.
    pub fn llamada(l: Llamada) -> Self {
        let mut m = Self::texto(
            Self::nuevo_id(),
            ahora(),
            l.saliente,
            String::new(),
            String::new(),
            None,
        );
        m.estado = Estado::Visto;
        m.llamada = Some(l);
        m
    }

    /// Mark ours as taken by the other computer (never backwards).
    pub fn entregado(&mut self) {
        if self.mio && self.estado != Estado::Visto {
            self.estado = Estado::Entregado;
        }
    }

    /// Mark ours as seen by the other person.
    pub fn visto(&mut self) {
        if self.mio {
            self.estado = Estado::Visto;
        }
    }

    /// Change the text (either side; who may is checked by the caller).
    pub fn edita(&mut self, texto: String) {
        if !self.borrado && self.llamada.is_none() && self.archivo.is_none() {
            self.texto = texto;
            self.editado = true;
        }
    }

    /// Delete for everyone: the text goes, the line says so.
    pub fn borra(&mut self) {
        self.texto.clear();
        self.archivo = None;
        self.borrado = true;
        self.reacciones.clear();
    }

    /// One reaction per person: the same emoji again takes it away; another replaces it.
    pub fn reacciona(&mut self, quien: &str, emoji: &str) {
        let emoji: String = emoji.chars().take(8).collect();
        if let Some(i) = self.reacciones.iter().position(|(q, _)| q == quien) {
            if self.reacciones[i].1 == emoji {
                self.reacciones.remove(i);
            } else {
                self.reacciones[i].1 = emoji;
            }
        } else if !emoji.is_empty() {
            self.reacciones.push((quien.to_owned(), emoji));
        }
    }

    /// For the screen.
    pub fn json(&self) -> Value {
        let mut v = json!({
            "id": self.id.to_string(),
            "t": self.t,
            "mio": self.mio,
            "texto": self.texto,
            "de": self.de,
            "estado": self.estado.nombre(),
        });
        if self.editado {
            v["editado"] = json!(true);
        }
        if self.borrado {
            v["borrado"] = json!(true);
        }
        if let Some(re) = self.re {
            v["re"] = json!(re.to_string());
        }
        if !self.reacciones.is_empty() {
            v["reacciones"] = json!(self
                .reacciones
                .iter()
                .map(|(q, e)| json!({ "de": q, "emoji": e }))
                .collect::<Vec<_>>());
        }
        if let Some(l) = &self.llamada {
            v["llamada"] = json!({
                "video": l.video,
                "saliente": l.saliente,
                "segundos": l.segundos,
                "fin": l.fin,
            });
        }
        if let Some(a) = &self.archivo {
            v["archivo"] = json!({ "nombre": a, "id": self.archivo_id, "mime": mime_de(a) });
        }
        v
    }
}

/// The media type a picture or sound name suggests (what the phone needs to show or play it).
/// A voice note (`nota-de-voz-…`, 17 Oct 2026) is sound whatever its container.
pub fn mime_de(nombre: &str) -> &'static str {
    let voz = nombre.to_ascii_lowercase().starts_with("nota-de-voz");
    match nombre
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "webm" if voz => "audio/webm",
        "mp4" if voz => "audio/mp4",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "aac" => "audio/aac",
        "oga" => "audio/ogg",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "heic" => "image/heic",
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "m4a" => "audio/mp4",
        "ogg" | "opus" => "audio/ogg",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

/// An id as it travels: a decimal string (JSON numbers lose bits past 2^53).
pub fn id_de(v: &Value) -> Option<u64> {
    match v {
        Value::String(s) => s.parse().ok().filter(|&n| n != 0),
        Value::Number(n) => n.as_u64().filter(|&n| n != 0),
        _ => None,
    }
}

fn ahora() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estados_y_reacciones() {
        let mut m = Mensaje::mio("hola".into(), None);
        assert_eq!(m.estado, Estado::Enviado);
        m.entregado();
        assert_eq!(m.estado, Estado::Entregado);
        m.visto();
        assert_eq!(m.estado, Estado::Visto);
        m.entregado();
        assert_eq!(m.estado, Estado::Visto, "never backwards");
        m.reacciona("Bea", "❤️");
        m.reacciona("Bea", "❤️");
        assert!(
            m.reacciones.is_empty(),
            "the same emoji again takes it away"
        );
        m.reacciona("Bea", "👍");
        m.reacciona("Bea", "😂");
        assert_eq!(m.reacciones, vec![("Bea".to_owned(), "😂".to_owned())]);
        m.edita("hola!".into());
        assert!(m.editado);
        m.borra();
        assert!(m.borrado && m.texto.is_empty() && m.reacciones.is_empty());
        let j = m.json();
        assert_eq!(j["borrado"], true);
        assert_eq!(id_de(&j["id"]), Some(m.id));
    }

    #[test]
    fn ids_distintos_y_no_cero() {
        let a = Mensaje::nuevo_id();
        let b = Mensaje::nuevo_id();
        assert_ne!(a, b);
        assert_ne!(a, 0);
        assert_eq!(id_de(&json!("0")), None);
        assert_eq!(id_de(&json!(7)), Some(7));
    }
}
