//! The browser's texts in Spanish, English and Portuguese (`i18n/*.json`), never inside the code.
//! The language follows the system's: Spanish and Portuguese by their own name, English for
//! everything else, the same rule as guardianagroup.com and the GUARDIANA panel.

use std::collections::BTreeMap;
use std::sync::OnceLock;

const ES: &str = include_str!("../i18n/es.json");
const EN: &str = include_str!("../i18n/en.json");
const PT: &str = include_str!("../i18n/pt.json");

/// A language of the browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Idioma {
    /// Spanish.
    Es,
    /// English.
    En,
    /// Portuguese.
    Pt,
}

impl Idioma {
    /// From a system or browser tag (`es-CO`, `pt-BR`, `en-US`, `fr-FR`...).
    #[must_use]
    pub fn de_etiqueta(tag: &str) -> Self {
        let t = tag.trim().to_ascii_lowercase();
        if t.starts_with("es") {
            Self::Es
        } else if t.starts_with("pt") {
            Self::Pt
        } else {
            Self::En
        }
    }

    /// Two-letter code.
    #[must_use]
    pub const fn codigo(self) -> &'static str {
        match self {
            Self::Es => "es",
            Self::En => "en",
            Self::Pt => "pt",
        }
    }
}

type Mapa = BTreeMap<String, String>;

static MAPAS: OnceLock<[Mapa; 3]> = OnceLock::new();

fn mapas() -> &'static [Mapa; 3] {
    MAPAS.get_or_init(|| {
        let lee = |s: &str| serde_json::from_str::<Mapa>(s).unwrap_or_default();
        [lee(ES), lee(EN), lee(PT)]
    })
}

/// The texts of one language.
#[derive(Debug, Clone, Copy)]
pub struct Textos {
    idioma: Idioma,
}

impl Textos {
    /// Texts in `idioma`.
    #[must_use]
    pub const fn de(idioma: Idioma) -> Self {
        Self { idioma }
    }

    /// The language.
    #[must_use]
    pub const fn idioma(&self) -> Idioma {
        self.idioma
    }

    /// The text for `clave`, falling back to Spanish and then to the key itself (which the tests
    /// make sure never happens).
    #[must_use]
    pub fn t(&self, clave: &str) -> String {
        let [es, en, pt] = mapas();
        let m = match self.idioma {
            Idioma::Es => es,
            Idioma::En => en,
            Idioma::Pt => pt,
        };
        m.get(clave)
            .or_else(|| es.get(clave))
            .cloned()
            .unwrap_or_else(|| clave.to_string())
    }

    /// Whether the language (or Spanish, its fallback) has a text for `clave`.
    fn tiene(&self, clave: &str) -> bool {
        let [es, en, pt] = mapas();
        let m = match self.idioma {
            Idioma::Es => es,
            Idioma::En => en,
            Idioma::Pt => pt,
        };
        m.contains_key(clave) || es.contains_key(clave)
    }

    /// The text for `clave` with every `{hueco}` filled.
    #[must_use]
    pub fn tf(&self, clave: &str, huecos: &[(&str, &str)]) -> String {
        let mut s = self.t(clave);
        for (k, v) in huecos {
            s = s.replace(&format!("{{{k}}}"), v);
        }
        s
    }

    /// [`Self::tf`] with the count's own form, as the browser's pages do (`puente.js`):
    /// `<clave>_uno` for one and `<clave>_cero` for none, when the texts have them («Se cortó 1
    /// petición», never «Se cortaron 1 peticiones»).
    #[must_use]
    pub fn tn(&self, clave: &str, n: u64, huecos: &[(&str, &str)]) -> String {
        let propia = match n {
            1 => Some(format!("{clave}_uno")),
            0 => Some(format!("{clave}_cero")),
            _ => None,
        };
        match propia {
            Some(k) if self.tiene(&k) => self.tf(&k, huecos),
            _ => self.tf(clave, huecos),
        }
    }

    /// A count written the person's way: `12.345` in Spanish and Portuguese, `12,345` in English
    /// (four digits stay together in Spanish and Portuguese, as the pages write them).
    #[must_use]
    pub fn numero(&self, n: u64) -> String {
        let s = n.to_string();
        let (sep, desde) = match self.idioma {
            Idioma::En => (',', 4),
            Idioma::Es | Idioma::Pt => ('.', 5),
        };
        if s.len() < desde {
            return s;
        }
        let mut out = String::with_capacity(s.len() + s.len() / 3);
        for (i, c) in s.chars().enumerate() {
            if i > 0 && (s.len() - i) % 3 == 0 {
                out.push(sep);
            }
            out.push(c);
        }
        out
    }

    /// Every text of the language, for the interface (sent once when it opens).
    #[must_use]
    pub fn todos(&self) -> &'static Mapa {
        let [es, en, pt] = mapas();
        match self.idioma {
            Idioma::Es => es,
            Idioma::En => en,
            Idioma::Pt => pt,
        }
    }
}

impl crate::tachon::Etiquetas for Textos {
    fn etiqueta(&self, clase: crate::datos::Clase) -> String {
        self.t(&format!("marca_{}", clase.clave()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_languages_have_the_same_keys() {
        let [es, en, pt] = mapas();
        assert!(!es.is_empty());
        let falta = |a: &Mapa, b: &Mapa| -> Vec<String> {
            a.keys().filter(|k| !b.contains_key(*k)).cloned().collect()
        };
        assert_eq!(falta(es, en), Vec::<String>::new(), "en English");
        assert_eq!(falta(es, pt), Vec::<String>::new(), "en portugués");
        assert_eq!(falta(en, es), Vec::<String>::new(), "sobran en English");
        assert_eq!(falta(pt, es), Vec::<String>::new(), "sobran en portugués");
    }

    #[test]
    fn every_kind_of_data_and_every_law_has_its_words() {
        for idioma in [Idioma::Es, Idioma::En, Idioma::Pt] {
            let t = Textos::de(idioma);
            for c in [
                crate::datos::Clase::Correo,
                crate::datos::Clase::Telefono,
                crate::datos::Clase::Tarjeta,
                crate::datos::Clase::Cuenta,
                crate::datos::Clase::Documento,
                crate::datos::Clase::Nombre,
                crate::datos::Clase::Otro,
            ] {
                assert_ne!(t.t(c.clave()), c.clave());
                assert_ne!(
                    t.t(&format!("marca_{}", c.clave())),
                    format!("marca_{}", c.clave())
                );
            }
            for ley in ["co", "ue", "ca", "br", "otro"] {
                for parte in ["asunto", "cuerpo"] {
                    let k = format!("carta_{ley}_{parte}");
                    assert_ne!(t.t(&k), k);
                }
            }
        }
    }

    #[test]
    fn counts_take_their_own_form_and_their_own_separators() {
        let es = Textos::de(Idioma::Es);
        assert_eq!(
            es.tn("escudo_resumen_cortadas", 1, &[("cortadas", "1")]),
            "Se cortó 1 petición."
        );
        assert_eq!(
            es.tn("escudo_resumen_cortadas", 0, &[("cortadas", "0")]),
            "No se cortó ninguna petición."
        );
        assert_eq!(
            es.tn("escudo_resumen_cortadas", 7, &[("cortadas", "7")]),
            "Se cortaron 7 peticiones."
        );
        // A text without a form for none takes the general one.
        assert_eq!(es.tn("escudo_peticiones", 0, &[("n", "0")]), "0 peticiones");
        assert_eq!(es.numero(1234), "1234");
        assert_eq!(es.numero(12_345), "12.345");
        assert_eq!(es.numero(1_234_567), "1.234.567");
        assert_eq!(Textos::de(Idioma::En).numero(1234), "1,234");
        assert_eq!(Textos::de(Idioma::Pt).numero(999), "999");
    }

    #[test]
    fn languages_follow_the_system() {
        assert_eq!(Idioma::de_etiqueta("es-CO"), Idioma::Es);
        assert_eq!(Idioma::de_etiqueta("pt-BR"), Idioma::Pt);
        assert_eq!(Idioma::de_etiqueta("fr-FR"), Idioma::En);
    }
}
