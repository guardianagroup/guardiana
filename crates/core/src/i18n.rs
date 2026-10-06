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
    /// Una línea que dice qué significa cada categoría, para enseñarla justo donde está la
    /// palabra. La leyenda del pie de la página existía, pero está lejos de la tabla: el
    /// responsable leyó «sin clasificar» y «entrega» y tuvo que preguntar (21 sep 2026).
    #[serde(default)]
    pub categorias_que: HashMap<String, String>,
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
    /// El nombre de cada país, por su código ISO de dos letras, en el idioma de esta ficha.
    /// La lista `empresas` guarda el código para no repetir el nombre tres veces, y aquí se dice
    /// como lo diría una persona de ese idioma.
    #[serde(default)]
    pub paises: HashMap<String, String>,
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

/// Texts for a language code (`es-CO`, `en-US`, `pt-BR`, `de-DE`...): Spanish, Portuguese or
/// English when it starts with `es`, `pt` or `en`; English for any other language, because a
/// person reading German or French is far likelier to read English than Spanish (28 Sep 2026:
/// a browser in German got the whole panel in Spanish). Spanish stays the answer only when
/// nothing is said at all — an empty code, or the `C`/`POSIX` locale of a bare system or of the
/// Windows service — because that is the language the program was written in.
#[must_use]
pub fn by_code(code: &str) -> &'static Texts {
    let code = code.trim().to_ascii_lowercase();
    let sin_idioma = code.is_empty() || code == "c" || code.starts_with("c.") || code == "posix";
    if code.starts_with("es") || sin_idioma {
        es()
    } else if code.starts_with("pt") {
        pt()
    } else {
        en()
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
/// or `LANG`. Windows sets none of those, so there it is the language of the Windows account
/// (`HKCU\Control Panel\International\LocaleName`): a German Windows answers in English, a
/// Colombian one in Spanish. Spanish when nothing says anything (see [`by_code`]).
pub fn current() -> &'static Texts {
    for var in ["GUARDIANA_LANG", "LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(var) {
            if !v.trim().is_empty() {
                return by_code(&v);
            }
        }
    }
    #[cfg(windows)]
    if let Some(code) = windows_locale() {
        return by_code(code);
    }
    es()
}

/// The Windows account's locale name (`es-CO`, `de-DE`...), read once per run with `reg`, the
/// tool Windows ships for it: no extra dependency and no `unsafe` for one string.
#[cfg(windows)]
fn windows_locale() -> Option<&'static str> {
    static LOCALE: OnceLock<Option<String>> = OnceLock::new();
    LOCALE
        .get_or_init(|| {
            use std::os::windows::process::CommandExt;
            // The reg.exe of Windows itself, by full path, never one found elsewhere on PATH.
            // CREATE_NO_WINDOW: no console flashing when this runs from the service.
            let raiz = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_owned());
            let out = std::process::Command::new(format!(r"{raiz}\System32\reg.exe"))
                .args([
                    "query",
                    r"HKCU\Control Panel\International",
                    "/v",
                    "LocaleName",
                ])
                .creation_flags(0x0800_0000)
                .output()
                .ok()?;
            locale_de_reg(&String::from_utf8_lossy(&out.stdout))
        })
        .as_deref()
}

/// The value of `LocaleName` in the output of `reg query` (`    LocaleName    REG_SZ    es-CO`).
#[cfg_attr(not(windows), allow(dead_code))]
fn locale_de_reg(salida: &str) -> Option<String> {
    salida.lines().find_map(|l| {
        let mut partes = l.split_whitespace();
        if partes.next()? != "LocaleName" {
            return None;
        }
        partes.nth(1).map(str::to_owned)
    })
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
            categorias_que: HashMap::new(),
            oficios: HashMap::new(),
            paises: HashMap::new(),
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

    /// A CLI text that counts: `key_uno` when there is one thing and that text exists, `key`
    /// otherwise, with `{n}` replaced. Until 1.0.2 the program said «1 reglas deshechas» or
    /// «quedan 1 días» (review of 5 Oct 2026).
    #[must_use]
    pub fn cli_n(&self, key: &str, n: i64) -> String {
        let uno = format!("{key}_uno");
        let elegida = if n == 1 && self.cli.contains_key(&uno) {
            uno.as_str()
        } else {
            key
        };
        Self::get(&self.cli, elegida).replace("{n}", &n.to_string())
    }

    /// The same for a panel text.
    #[must_use]
    pub fn panel_n(&self, key: &str, n: i64) -> String {
        let uno = format!("{key}_uno");
        let elegida = if n == 1 && self.panel.contains_key(&uno) {
            uno.as_str()
        } else {
            key
        };
        Self::get(&self.panel, elegida).replace("{n}", &n.to_string())
    }

    /// «1 dispositivo» or «3 dispositivos», in this language: the weekly report's share text,
    /// which read «1 dispositivos» in a home with only this PC until 1.0.1.
    #[must_use]
    pub fn cuantos_dispositivos(&self, n: usize) -> String {
        if n == 1 {
            self.panel("informe_dispositivos_uno").to_owned()
        } else {
            self.panel("informe_dispositivos_n")
                .replace("{n}", &n.to_string())
        }
    }

    /// What a company does, in one sentence, or `None` when it is not written yet. Unlike the
    /// other lookups this does not fall back to the key: a company name is not a sentence, and
    /// printing it as if it were would be worse than saying nothing.
    #[must_use]
    pub fn oficio(&self, empresa: &str) -> Option<&str> {
        self.oficios.get(empresa).map(String::as_str)
    }

    /// El nombre del país por su código de dos letras, o vacío si no está escrito. Como con el
    /// oficio, no se cae al identificador: «CN» a secas no le dice nada a nadie.
    #[must_use]
    pub fn pais<'a>(&'a self, codigo: &str) -> &'a str {
        self.paises.get(codigo).map_or("", String::as_str)
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
        // Any other language reads English, not Spanish; a locale that names no language stays
        // in Spanish.
        assert!(std::ptr::eq(by_code("de-DE,de;q=0.9,en;q=0.8"), en()));
        assert!(std::ptr::eq(by_code("fr"), en()));
        assert!(std::ptr::eq(by_code("it_IT.UTF-8"), en()));
        assert!(std::ptr::eq(by_code("C.UTF-8"), es()));
        assert!(std::ptr::eq(by_code("POSIX"), es()));
        assert!(std::ptr::eq(by_code("es_ES.UTF-8"), es()));
        let reg = "\r\nHKEY_CURRENT_USER\\Control Panel\\International\r\n    LocaleName    REG_SZ    de-DE\r\n\r\n";
        assert_eq!(locale_de_reg(reg).as_deref(), Some("de-DE"));
        assert_eq!(locale_de_reg("ERROR: nada"), None);
    }
}
