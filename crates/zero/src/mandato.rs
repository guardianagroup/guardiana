//! «Mandato»: the person says the task and which sites it needs; GUARDIANA ZERO keeps the AI's
//! tab inside them. The limits are enforced here, in the browser, outside the AI: a page that
//! talks the AI into going elsewhere has talked to the wrong one. When the mandate ends it is
//! written down, signed, with every site visited and every attempt cut.

use serde::{Deserialize, Serialize};

use crate::destino::Destino;
use crate::dominio::sitio;

/// What kind of resource a request asks for, as the engine says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Recurso {
    /// The page itself (top-level navigation).
    Documento,
    /// A frame inside the page.
    Marco,
    /// A script.
    Script,
    /// An image.
    Imagen,
    /// A style sheet.
    Estilo,
    /// A font.
    Fuente,
    /// Audio or video.
    Media,
    /// A request made by the page's code (fetch, XHR).
    Datos,
    /// A ping or beacon: a request that only carries information out.
    Aviso,
    /// A WebSocket.
    Socket,
    /// Anything else.
    Otro,
}

impl Recurso {
    /// Resources that only bring something to show: what a delivery network may serve inside a
    /// strict mandate without being named in it.
    #[must_use]
    pub const fn solo_trae(self) -> bool {
        matches!(
            self,
            Self::Script | Self::Imagen | Self::Estilo | Self::Fuente | Self::Media
        )
    }
}

/// One line of what happened inside a mandate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Paso {
    /// Milliseconds since the Unix epoch.
    pub ts: i64,
    /// `visita` (a page opened), `cortado` (an attempt outside), `dato` (personal data sent,
    /// asked first), `tinta` (a marked value or the decoy caught leaving).
    pub que: String,
    /// The site it was about.
    pub sitio: String,
    /// Extra detail: the kinds of data, the form a marked value travelled in...
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detalle: String,
    /// How many times it happened (identical steps within a second are one line).
    #[serde(default = "uno")]
    pub veces: u32,
}

const fn uno() -> u32 {
    1
}

/// A task given to an AI, with its limits and its log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mandato {
    /// Unique id (hex).
    pub id: String,
    /// The task in the person's words (may be empty).
    pub tarea: String,
    /// Sites the AI may use (registrable sites, `google.com`).
    pub permitidos: Vec<String>,
    /// Strict: only those sites; delivery networks only for images, scripts, styles and fonts,
    /// and never a customer's own name on them (`x.cloudfront.net`, a bucket).
    pub estricto: bool,
    /// A decoy for this mandate (see `tinta::senuelo`).
    pub senuelo: String,
    /// Start and end, ms since the epoch (`fin` 0 while it runs).
    pub inicio: i64,
    /// End.
    pub fin: i64,
    /// What happened.
    pub pasos: Vec<Paso>,
    /// When it ends on its own, ms since the epoch (0: when the person ends it).
    #[serde(default)]
    pub caduca: i64,
    /// Steps beyond the log's limit, by kind: still counted in the receipt.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub desbordados: std::collections::BTreeMap<String, u32>,
}

/// The sites a mandate allows for what the person wrote: each entry becomes its registrable site
/// (`maps.google.com` → `google.com`, as the limit really applies), once. An entry that is not a
/// web address with a dot (`flights`, `a.com;`) is left out rather than allowed as written. The
/// panel shows exactly this list before the mandate starts.
#[must_use]
pub fn sitios_de(direcciones: &[String]) -> Vec<String> {
    let mut permitidos: Vec<String> = Vec::new();
    for d in direcciones {
        let d = d.trim();
        let host = crate::dominio::host_de(d)
            .or_else(|| crate::dominio::host_de(&format!("https://{d}")))
            .unwrap_or_default();
        let valido = host.contains('.')
            && !host.starts_with('.')
            && !host.ends_with('.')
            && host
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.');
        if !valido {
            continue;
        }
        let s = sitio(&host);
        if !s.is_empty() && !permitidos.contains(&s) {
            permitidos.push(s);
        }
        if permitidos.len() >= 40 {
            break;
        }
    }
    permitidos
}

impl Mandato {
    /// A new mandate for `tarea` limited to the sites of `direcciones` (addresses or names).
    #[must_use]
    pub fn nuevo(
        id: String,
        tarea: &str,
        direcciones: &[String],
        senuelo: String,
        ahora: i64,
    ) -> Self {
        let permitidos = sitios_de(direcciones);
        Self {
            id,
            tarea: tarea.trim().to_string(),
            permitidos,
            estricto: true,
            senuelo,
            inicio: ahora,
            fin: 0,
            pasos: Vec::new(),
            caduca: 0,
            desbordados: std::collections::BTreeMap::new(),
        }
    }

    /// Whether a request to `destino` for `recurso` stays inside the mandate.
    #[must_use]
    pub fn permite(&self, destino: &Destino, recurso: Recurso) -> bool {
        if self.permitidos.contains(&destino.sitio) {
            return true;
        }
        if self.estricto {
            // A distribution, a bucket or a repository on a shared service is somebody's:
            // anyone can rent one and read what reaches it (review of 10 Oct 2026, grave 4).
            destino.reparto.is_some() && !destino.de_cliente && recurso.solo_trae()
        } else {
            recurso.solo_trae() || destino.reparto.is_some()
        }
    }

    /// Write down one step; repeated identical steps within a second are folded.
    pub fn anota(&mut self, ts: i64, que: &str, sitio: &str, detalle: &str) {
        if let Some(u) = self.pasos.last_mut() {
            if u.que == que && u.sitio == sitio && u.detalle == detalle && ts - u.ts < 1000 {
                u.veces += 1;
                return;
            }
        }
        if self.pasos.len() < 5000 {
            self.pasos.push(Paso {
                ts,
                que: que.into(),
                sitio: sitio.into(),
                detalle: detalle.into(),
                veces: 1,
            });
        } else {
            *self.desbordados.entry(que.to_string()).or_insert(0) += 1;
        }
    }

    /// Counts for the receipt: (pages visited, distinct sites, attempts cut, data sent).
    #[must_use]
    pub fn cuentas(&self) -> (usize, usize, usize, usize) {
        let mut sitios: Vec<&str> = Vec::new();
        let (mut visitas, mut cortes, mut datos) = (0, 0, 0);
        let pasos = self
            .pasos
            .iter()
            .map(|p| (p.que.as_str(), p.veces as usize))
            .chain(
                self.desbordados
                    .iter()
                    .map(|(q, n)| (q.as_str(), *n as usize)),
            );
        for (que, n) in pasos {
            match que {
                "visita" => visitas += n,
                "cortado" | "tinta" => cortes += n,
                "dato" => datos += n,
                _ => {}
            }
        }
        for p in &self.pasos {
            if !sitios.contains(&p.sitio.as_str()) {
                sitios.push(&p.sitio);
            }
        }
        (visitas, sitios.len(), cortes, datos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::destino::{clasifica, clasifica_url};

    #[test]
    fn a_mandate_keeps_the_ai_inside_its_sites() {
        let m = Mandato::nuevo(
            "1".into(),
            "Busca un vuelo",
            &["https://www.avianca.com/".into(), "google.com".into()],
            "x".into(),
            0,
        );
        assert_eq!(m.permitidos, vec!["avianca.com", "google.com"]);
        assert!(m.permite(&clasifica("www.avianca.com"), Recurso::Documento));
        assert!(m.permite(&clasifica("fonts.google.com"), Recurso::Fuente));
        assert!(!m.permite(&clasifica("evil.example"), Recurso::Documento));
        assert!(!m.permite(&clasifica("evil.example"), Recurso::Datos));
        // A delivery network may bring an image, not take data out.
        let cdn = clasifica("r1---sn-abc.gvt1.com");
        assert!(m.permite(&cdn, Recurso::Imagen));
        let cdn = clasifica_url("https://cdn.jsdelivr.net/npm/vue@3/dist/vue.js");
        assert!(m.permite(&cdn, Recurso::Script));
        assert!(!m.permite(&cdn, Recurso::Datos));
    }

    #[test]
    fn a_strict_mandate_never_takes_a_rented_name_for_the_road() {
        let m = Mandato::nuevo("1".into(), "", &["avianca.com".into()], "x".into(), 0);
        assert!(m.estricto);
        for u in [
            "https://dattacker.cloudfront.net/p.gif?d=secreto",
            "https://cdn.jsdelivr.net/gh/atacante/x@main/a.js?d=secreto",
            "https://atacante.r2.cloudflarestorage.com/p.png?d=secreto",
            "https://ninja.akamaized.net/p.png?d=secreto",
            "https://raw.githubusercontent.com/atacante/x/main/a.js",
            "https://storage.googleapis.com/atacante/p.png",
        ] {
            let d = clasifica_url(u);
            assert!(!m.permite(&d, Recurso::Imagen), "{u}");
            assert!(!m.permite(&d, Recurso::Script), "{u}");
        }
        // Named in the mandate, a rented name is allowed like any other site.
        let m = Mandato::nuevo(
            "2".into(),
            "",
            &["dattacker.cloudfront.net".into()],
            "x".into(),
            0,
        );
        assert!(m.permite(
            &clasifica_url("https://dattacker.cloudfront.net/p.gif"),
            Recurso::Imagen
        ));
    }

    #[test]
    fn steps_are_counted_for_the_receipt() {
        let mut m = Mandato::nuevo("1".into(), "", &["a.com".into()], "x".into(), 0);
        m.anota(1, "visita", "a.com", "");
        m.anota(2, "cortado", "b.com", "");
        m.anota(3, "cortado", "b.com", "");
        m.anota(2000, "dato", "a.com", "correo");
        // Two attempts within a second are one line of the log, and two attempts in the count.
        assert_eq!(m.pasos.len(), 3);
        assert_eq!(m.cuentas(), (1, 2, 2, 1));
    }

    #[test]
    fn the_sites_are_what_the_limit_really_allows() {
        let w = |l: &[&str]| sitios_de(&l.iter().map(|s| (*s).to_string()).collect::<Vec<_>>());
        assert_eq!(
            w(&[
                "https://www.maps.google.com/x",
                "a.com;",
                "b.com",
                "flights",
                "B.COM"
            ]),
            ["google.com", "b.com"]
        );
        assert!(w(&["javascript:alert(1)", ".com", "x."]).is_empty());
    }
}
