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
/// The Brazilian Portuguese texts (decision 175): same keys again, because Brazil is the largest
/// market of the region and the site speaks it since 19 Sep 2026.
pub const PT_JSON: &str = include_str!("../../panel/i18n/pt.json");

/// All interface texts, grouped as in the JSON file.
#[derive(Debug, Clone, Deserialize)]
pub struct Texts {
    /// Category labels, keyed by the stored text (`rastreador`, ...).
    pub categorias: HashMap<String, String>,
    /// The two-word label of each signal, for the filter dropdown and the chips. Lived hardcoded in
    /// app.js until 19 Sep 2026: both languages were there, but interface text in code is exactly
    /// what the project rule forbids, because the next one added is the one that ships untranslated.
    #[serde(default)]
    pub senales_corto: HashMap<String, String>,
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
    /// What each company does for a living, in one sentence, keyed by the company name of the
    /// "empresas" list (decision 149). Not a verdict: AppsFlyer selling attribution is its
    /// trade, and saying so is what stops the panel from calling it "unknown".
    #[serde(default)]
    pub oficios: HashMap<String, String>,
}

static TEXTS: OnceLock<Texts> = OnceLock::new();
static TEXTS_EN: OnceLock<Texts> = OnceLock::new();
static TEXTS_PT: OnceLock<Texts> = OnceLock::new();

/// The Spanish texts. The embedded file is validated by a test, so a
/// malformed file fails the build's tests rather than the user's session.
pub fn es() -> &'static Texts {
    TEXTS.get_or_init(|| serde_json::from_str(ES_JSON).unwrap_or_else(|_| Texts::empty()))
}

/// The English texts.
pub fn en() -> &'static Texts {
    TEXTS_EN.get_or_init(|| serde_json::from_str(EN_JSON).unwrap_or_else(|_| Texts::empty()))
}

/// The Portuguese texts.
pub fn pt() -> &'static Texts {
    TEXTS_PT.get_or_init(|| serde_json::from_str(PT_JSON).unwrap_or_else(|_| Texts::empty()))
}

/// Texts for a language code (`en`, `en-US`, `pt-BR`, `es`...): English when it starts with `en`,
/// Portuguese when it starts with `pt`, Spanish otherwise. Spanish stays the fallback because it
/// is what the person installed when nothing says otherwise (the Windows service has no locale).
#[must_use]
pub fn by_code(code: &str) -> &'static Texts {
    let code = code.trim().to_ascii_lowercase();
    if code.starts_with("en") {
        en()
    } else if code.starts_with("pt") {
        pt()
    } else {
        es()
    }
}

/// The raw JSON behind a `Texts`, for whoever has to hand the whole file over instead of a
/// sentence — the panel gives it to the browser. It lives here, next to the three constants, so
/// that adding a language is one place and not a chain of "is it English?" in every caller: that
/// exact shortcut is what left the panel answering in Spanish to a Portuguese browser.
#[must_use]
pub fn json_of(t: &'static Texts) -> &'static str {
    if std::ptr::eq(t, en()) {
        EN_JSON
    } else if std::ptr::eq(t, pt()) {
        PT_JSON
    } else {
        ES_JSON
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
            senales_corto: HashMap::new(),
            senales: HashMap::new(),
            veredictos: HashMap::new(),
            decidido_por: HashMap::new(),
            cli: HashMap::new(),
            panel: HashMap::new(),
            oficios: HashMap::new(),
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

    /// What a company does, in one sentence, or `None` when it is not written yet. Unlike the
    /// other lookups this does not fall back to the key: a company name is not a sentence, and
    /// printing it as if it were would be worse than saying nothing.
    #[must_use]
    pub fn oficio(&self, empresa: &str) -> Option<&str> {
        self.oficios.get(empresa).map(String::as_str)
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
            assert!(t.senales_corto.contains_key(k.as_str()), "corto: {k}");
        }
        assert_eq!(
            es().signal(&Signal::Baliza { minutes: 7 }),
            "Contacta el mismo destino cada 7 minutos, como un latido."
        );
        assert!(!es().signal(&Signal::Volumen).contains('{'));
    }

    #[test]
    fn english_file_has_exactly_the_same_keys() {
        // And Portuguese too: a third language is exactly when a missing key stops being noticed,
        // so the same test guards all three.
        let es_v: serde_json::Value = serde_json::from_str(ES_JSON).unwrap();
        for (otro, json) in [("en", EN_JSON), ("pt", PT_JSON)] {
            let v: serde_json::Value = serde_json::from_str(json).unwrap();
            for (section, es_map) in es_v.as_object().unwrap() {
                assert!(
                    v.get(section).is_some(),
                    "{otro}: section {section} missing"
                );
                let map = v[section].as_object().unwrap();
                let mut a: Vec<&String> = es_map.as_object().unwrap().keys().collect();
                let mut b: Vec<&String> = map.keys().collect();
                a.sort();
                b.sort();
                assert_eq!(a, b, "keys of {section} in {otro}");
            }
        }
        assert_eq!(en().category(Category::Rastreador), "tracker");
        assert_eq!(
            en().signal(&Signal::Baliza { minutes: 7 }),
            "Contacts the same destination every 7 minutes, like a heartbeat."
        );
        assert_eq!(pt().category(Category::Rastreador), "rastreador");
        assert_eq!(
            pt().signal(&Signal::Baliza { minutes: 7 }),
            "Contata o mesmo destino a cada 7 minutos, como uma batida."
        );
        assert!(std::ptr::eq(by_code("en-GB"), en()));
        assert!(std::ptr::eq(by_code("es-CO"), es()));
        assert!(std::ptr::eq(by_code("pt-BR"), pt()));
        // Whoever hands the whole file over must hand the right one (the panel does).
        assert_eq!(json_of(by_code("pt-BR")), PT_JSON);
        assert_eq!(json_of(by_code("en-US")), EN_JSON);
        assert_eq!(json_of(by_code("es")), ES_JSON);
        assert!(std::ptr::eq(by_code("PT"), pt()));
        assert!(std::ptr::eq(by_code(""), es()));
    }
}
