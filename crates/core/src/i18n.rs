//! Interface texts (brief §2): every sentence the user sees lives in
//! `crates/panel/i18n/es.json`, never in code. This module loads that file
//! (embedded at build time) so the CLI and the panel print the same words
//! (decision 14).

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::model::{Category, DecidedBy, Signal, Verdict};

/// The Spanish texts, embedded from `crates/panel/i18n/es.json`.
pub const ES_JSON: &str = include_str!("../../panel/i18n/es.json");
/// The English texts, embedded from `crates/panel/i18n/en.json` (decision 45): same keys, never text in code.
pub const EN_JSON: &str = include_str!("../../panel/i18n/en.json");

/// All interface texts, grouped as in the JSON file.
#[derive(Debug, Clone, Deserialize)]
pub struct Texts {
    /// Category labels, keyed by the stored text (`rastreador`, ...).
    pub categorias: HashMap<String, String>,
    /// One sentence per signal, keyed by signal name; `{n}` is replaced for `baliza`.
    pub senales: HashMap<String, String>,
    /// Verdict labels.
    pub veredictos: HashMap<String, String>,
    /// "Who decided" labels.
    pub decidido_por: HashMap<String, String>,
    /// Free-form texts used by the CLI, keyed by identifier.
    pub cli: HashMap<String, String>,
    /// Texts of the panel pages, keyed by identifier.
    #[serde(default)]
    pub panel: HashMap<String, String>,
}

static TEXTS: OnceLock<Texts> = OnceLock::new();
static TEXTS_EN: OnceLock<Texts> = OnceLock::new();

/// The Spanish texts. The embedded file is validated by a test, so a
/// malformed file fails the build's tests rather than the user's session.
pub fn es() -> &'static Texts {
    TEXTS.get_or_init(|| serde_json::from_str(ES_JSON).unwrap_or_else(|_| Texts::empty()))
}

/// The English texts.
pub fn en() -> &'static Texts {
    TEXTS_EN.get_or_init(|| serde_json::from_str(EN_JSON).unwrap_or_else(|_| Texts::empty()))
}

/// Texts for a language code (`en`, `en-US`, `es`...): English when it starts with `en`, Spanish otherwise.
#[must_use]
pub fn by_code(code: &str) -> &'static Texts {
    if code.trim().to_ascii_lowercase().starts_with("en") {
        en()
    } else {
        es()
    }
}

/// The language the CLI speaks: `GUARDIANA_LANG` first, then the system's `LC_ALL`, `LC_MESSAGES`
/// or `LANG`; Spanish unless one of them starts with `en`. The Windows service has none of them
/// and stays in Spanish, which is what the person installed.
pub fn current() -> &'static Texts {
    for var in ["GUARDIANA_LANG", "LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(var) {
            if !v.trim().is_empty() {
                return by_code(&v);
            }
        }
    }
    es()
}

impl Texts {
    fn empty() -> Self {
        Self {
            categorias: HashMap::new(),
            senales: HashMap::new(),
            veredictos: HashMap::new(),
            decidido_por: HashMap::new(),
            cli: HashMap::new(),
            panel: HashMap::new(),
        }
    }

    fn get<'a>(map: &'a HashMap<String, String>, key: &'a str) -> &'a str {
        map.get(key).map_or(key, String::as_str)
    }

    /// Label for a category; falls back to the stored text.
    #[must_use]
    pub fn category(&self, c: Category) -> &str {
        Self::get(&self.categorias, c.as_str())
    }

    /// Label for a verdict.
    #[must_use]
    pub fn verdict(&self, v: Verdict) -> &str {
        Self::get(&self.veredictos, v.as_str())
    }

    /// Label for who decided.
    #[must_use]
    pub fn decided_by(&self, d: DecidedBy) -> &str {
        Self::get(&self.decidido_por, d.as_str())
    }

    /// The one-sentence explanation of a signal (brief §5).
    #[must_use]
    pub fn signal(&self, s: &Signal) -> String {
        let template = Self::get(&self.senales, s.as_str());
        match s {
            Signal::Baliza { minutes } => template.replace("{n}", &minutes.to_string()),
            _ => template.to_owned(),
        }
    }

    /// A CLI text by identifier; falls back to the identifier.
    #[must_use]
    pub fn cli<'a>(&'a self, key: &'a str) -> &'a str {
        Self::get(&self.cli, key)
    }

    /// A panel text by identifier; falls back to the identifier.
    #[must_use]
    pub fn panel<'a>(&'a self, key: &'a str) -> &'a str {
        Self::get(&self.panel, key)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn embedded_file_is_valid_and_complete() {
        let t: Texts = serde_json::from_str(ES_JSON).unwrap();
        for c in Category::ALL {
            assert!(t.categorias.contains_key(c.as_str()), "{c}");
        }
        for v in Verdict::ALL {
            assert!(t.veredictos.contains_key(v.as_str()), "{v}");
        }
        for d in DecidedBy::ALL {
            assert!(t.decidido_por.contains_key(d.as_str()), "{d}");
        }
        for k in crate::model::SignalKind::ALL {
            assert!(t.senales.contains_key(k.as_str()), "{k}");
        }
        assert_eq!(
            es().signal(&Signal::Baliza { minutes: 7 }),
            "Contacta el mismo destino cada 7 minutos, como un latido."
        );
        assert!(!es().signal(&Signal::Volumen).contains('{'));
    }

    #[test]
    fn english_file_has_exactly_the_same_keys() {
        let es_v: serde_json::Value = serde_json::from_str(ES_JSON).unwrap();
        let en_v: serde_json::Value = serde_json::from_str(EN_JSON).unwrap();
        for (section, es_map) in es_v.as_object().unwrap() {
            let en_map = en_v[section].as_object().unwrap();
            let mut a: Vec<&String> = es_map.as_object().unwrap().keys().collect();
            let mut b: Vec<&String> = en_map.keys().collect();
            a.sort();
            b.sort();
            assert_eq!(a, b, "keys of {section}");
        }
        assert_eq!(en().category(Category::Rastreador), "tracker");
        assert_eq!(
            en().signal(&Signal::Baliza { minutes: 7 }),
            "Contacts the same destination every 7 minutes, like a heartbeat."
        );
        assert!(std::ptr::eq(by_code("en-GB"), en()));
        assert!(std::ptr::eq(by_code("es-CO"), es()));
        assert!(std::ptr::eq(by_code(""), es()));
    }
}
