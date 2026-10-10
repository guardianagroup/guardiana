//! Favourites: the pages the person keeps, shown as quick links on the new tab. They live only on
//! this computer (`favoritos.json` in the browser's data folder), are never sent anywhere, and
//! «Borrar todo» leaves them alone, as other browsers do with bookmarks.
//!
//! They can also be brought over from Chrome, Edge or Brave on this same computer: their
//! `Bookmarks` file is read once, when the person presses the button, and nothing else of those
//! browsers is touched (no history, no passwords, no cookies).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// At most this many favourites are kept.
pub const MAXIMO: usize = 500;
/// A site icon kept with a favourite, at most this long (a `data:` PNG).
const ICONO_MAX: usize = 16 * 1024;

/// One favourite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Favorito {
    /// The address, without its fragment.
    pub url: String,
    /// What it is called.
    pub titulo: String,
    /// Its icon as a `data:` PNG, or empty.
    #[serde(default)]
    pub icono: String,
}

/// The person's favourites, in their order (newest last; imported ones in the other browser's
/// order).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Favoritos {
    /// The list.
    pub lista: Vec<Favorito>,
}

fn sin_fragmento(url: &str) -> &str {
    url.split('#').next().unwrap_or(url)
}

/// Only web addresses become favourites.
fn es_web(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

impl Favoritos {
    /// Whether `url` is kept.
    #[must_use]
    pub fn contiene(&self, url: &str) -> bool {
        let u = sin_fragmento(url);
        self.lista.iter().any(|f| f.url == u)
    }

    /// Keep `url`. Returns whether it was added (not already there, a web address, room left).
    pub fn anade(&mut self, url: &str, titulo: &str, icono: &str) -> bool {
        let u = sin_fragmento(url);
        if !es_web(u) || u.len() > 2048 || self.contiene(u) || self.lista.len() >= MAXIMO {
            return false;
        }
        let titulo: String = titulo.trim().chars().take(200).collect();
        let icono = if icono.starts_with("data:image/") && icono.len() <= ICONO_MAX {
            icono.to_string()
        } else {
            String::new()
        };
        self.lista.push(Favorito {
            url: u.to_string(),
            titulo,
            icono,
        });
        true
    }

    /// Forget `url`. Returns whether it was there.
    pub fn quita(&mut self, url: &str) -> bool {
        let u = sin_fragmento(url);
        let antes = self.lista.len();
        self.lista.retain(|f| f.url != u);
        self.lista.len() != antes
    }

    /// Bring in a list (from another browser): the new ones go at the end, in order. Returns how
    /// many were added.
    pub fn importa(&mut self, nuevos: &[(String, String)]) -> usize {
        nuevos
            .iter()
            .filter(|(url, titulo)| self.anade(url, titulo, ""))
            .count()
    }
}

/// The web addresses in a Chromium `Bookmarks` file (Chrome, Edge, Brave), in the order the
/// browser shows them: the bookmarks bar first, then the other folders.
#[must_use]
pub fn de_chromium(texto: &str) -> Vec<(String, String)> {
    fn recorre(n: &Value, out: &mut Vec<(String, String)>) {
        match n.get("type").and_then(Value::as_str) {
            Some("url") => {
                let url = n.get("url").and_then(Value::as_str).unwrap_or("");
                if es_web(url) {
                    let titulo = n.get("name").and_then(Value::as_str).unwrap_or(url);
                    out.push((url.to_string(), titulo.to_string()));
                }
            }
            Some("folder") => {
                for h in n
                    .get("children")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    recorre(h, out);
                }
            }
            _ => {}
        }
    }
    let Ok(v) = serde_json::from_str::<Value>(texto) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for raiz in ["bookmark_bar", "other", "synced"] {
        if let Some(n) = v.get("roots").and_then(|r| r.get(raiz)) {
            recorre(n, &mut out);
        }
    }
    out
}

/// The other browsers on this computer whose favourites can be brought over: their name and
/// their `Bookmarks` file (the default profile), only those that exist.
#[must_use]
pub fn otros_navegadores() -> Vec<(&'static str, PathBuf)> {
    let Some(local) = std::env::var_os("LOCALAPPDATA") else {
        return Vec::new();
    };
    otros_navegadores_en(Path::new(&local))
}

/// [`otros_navegadores`] under a given `%LOCALAPPDATA%`.
#[must_use]
pub fn otros_navegadores_en(local: &Path) -> Vec<(&'static str, PathBuf)> {
    [
        ("Chrome", "Google/Chrome/User Data/Default/Bookmarks"),
        ("Edge", "Microsoft/Edge/User Data/Default/Bookmarks"),
        (
            "Brave",
            "BraveSoftware/Brave-Browser/User Data/Default/Bookmarks",
        ),
    ]
    .into_iter()
    .map(|(n, r)| (n, local.join(r)))
    .filter(|(_, p)| p.is_file())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_favourite_is_a_web_address_kept_once() {
        let mut f = Favoritos::default();
        assert!(f.anade(
            "https://www.eltiempo.com/#arriba",
            "El Tiempo",
            "data:image/png;base64,AA"
        ));
        assert!(f.contiene("https://www.eltiempo.com/"));
        assert!(!f.anade("https://www.eltiempo.com/", "otra vez", ""));
        assert!(!f.anade("file:///C:/secreto.txt", "no", ""));
        assert!(!f.anade("javascript:alert(1)", "no", ""));
        assert!(f.anade("https://x.example/", "X", "<svg onload=1>"));
        assert_eq!(f.lista[1].icono, "");
        assert!(f.quita("https://www.eltiempo.com/"));
        assert!(!f.contiene("https://www.eltiempo.com/"));
    }

    #[test]
    fn chrome_bookmarks_come_in_their_order_and_only_web_addresses() {
        let texto = r#"{"roots":{
          "bookmark_bar":{"type":"folder","children":[
            {"type":"url","name":"El Tiempo","url":"https://www.eltiempo.com/"},
            {"type":"folder","name":"Trabajo","children":[{"type":"url","name":"Correo","url":"https://mail.example/"}]},
            {"type":"url","name":"Ajustes","url":"chrome://settings/"}]},
          "other":{"type":"folder","children":[{"type":"url","name":"Wiki","url":"https://es.wikipedia.org/"}]},
          "synced":{"type":"folder","children":[]}}}"#;
        let l = de_chromium(texto);
        let urls: Vec<&str> = l.iter().map(|(u, _)| u.as_str()).collect();
        assert_eq!(
            urls,
            [
                "https://www.eltiempo.com/",
                "https://mail.example/",
                "https://es.wikipedia.org/"
            ]
        );
        let mut f = Favoritos::default();
        f.anade("https://mail.example/", "ya estaba", "");
        assert_eq!(f.importa(&l), 2);
        assert_eq!(f.lista.len(), 3);
        assert!(de_chromium("no es json").is_empty());
    }

    #[test]
    fn other_browsers_are_found_only_when_their_file_is_there() {
        let d = std::env::temp_dir().join(format!("zero-otros-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let p = d.join("Microsoft/Edge/User Data/Default");
        std::fs::create_dir_all(&p).unwrap_or_default();
        std::fs::write(p.join("Bookmarks"), "{}").unwrap_or_default();
        let n: Vec<&str> = otros_navegadores_en(&d)
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        assert_eq!(n, ["Edge"]);
        let _ = std::fs::remove_dir_all(&d);
    }
}
