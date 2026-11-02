//! The browser's texts in the 42 languages of guardianagroup.com (`i18n/<code>.json`), never inside the code. The
//! language follows the program's choice or the system's; a text not translated yet into a language reads in English,
//! and in Spanish (the language it is written in) only when English lacks it too. The list of languages is the one of
//! `guardiana_core::i18n::IDIOMAS` (2 Nov 2026, the owner: «mete los 42 idiomas en el programa»).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use guardiana_core::i18n::IDIOMAS;

macro_rules! fuentes {
    ($($codigo:literal),* $(,)?) => {
        /// Each language's file, in the order of `IDIOMAS`.
        const FUENTES: &[&str] = &[$(include_str!(concat!("../i18n/", $codigo, ".json"))),*];
        /// Their codes, to check the order against `IDIOMAS`.
        const CODIGOS: &[&str] = &[$($codigo),*];
    };
}

fuentes!(
    "es", "en", "pt", "fr", "de", "it", "nl", "sv", "da", "nb", "fi", "is", "pl", "uk", "ru", "tr",
    "zh", "ja", "ko", "hi", "bn", "id", "vi", "th", "ar", "fa", "he", "ur", "zh-tw", "cs", "ro",
    "hu", "el", "sk", "bg", "hr", "ms", "fil", "sw", "ta", "te", "pt-pt",
);

const N: usize = 42;
const _: () = assert!(FUENTES.len() == N && CODIGOS.len() == N && IDIOMAS.len() == N);

/// A language of the browser: its place in `guardiana_core::i18n::IDIOMAS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Idioma(usize);

#[allow(non_upper_case_globals)]
impl Idioma {
    /// Spanish.
    pub const Es: Self = Self(0);
    /// English.
    pub const En: Self = Self(1);
    /// Brazilian Portuguese.
    pub const Pt: Self = Self(2);

    /// From a system or browser tag (`es-CO`, `pt-BR`, `en-US`, `fr-FR`, `zh-TW`…): each of the 42 by its own code;
    /// any other language reads English (`guardiana_core::i18n::indice`).
    #[must_use]
    pub fn de_etiqueta(tag: &str) -> Self {
        Self(guardiana_core::i18n::indice(tag))
    }

    /// Its code (`es`, `zh-tw`, `pt-pt`…).
    #[must_use]
    pub fn codigo(self) -> &'static str {
        IDIOMAS.get(self.0).map_or("en", |x| x.codigo)
    }

    /// Its `lang` attribute (BCP 47).
    #[must_use]
    pub fn lang(self) -> &'static str {
        IDIOMAS.get(self.0).map_or("en", |x| x.lang)
    }

    /// Written right to left.
    #[must_use]
    pub fn rtl(self) -> bool {
        IDIOMAS.get(self.0).is_some_and(|x| x.rtl)
    }

    /// How it writes numbers: the thousands' separator, the decimal one, and from how many digits it groups them (the
    /// common usage of each language: a dot or a space for thousands and a comma for decimals in most of Europe,
    /// the other way round in English and in Asia; Spanish and Portuguese keep four digits together).
    #[must_use]
    pub fn separadores(self) -> (char, char, usize) {
        match self.codigo() {
            "es" | "pt" => ('.', ',', 5),
            "de" | "it" | "nl" | "da" | "id" | "tr" | "el" | "ro" | "hr" | "vi" | "is" => {
                ('.', ',', 4)
            }
            "fr" | "pt-pt" | "pl" | "fi" | "sv" | "nb" | "uk" | "ru" | "cs" | "sk" | "hu"
            | "bg" => ('\u{202f}', ',', 5),
            _ => (',', '.', 4),
        }
    }
}

type Mapa = BTreeMap<String, String>;

static MAPAS: [OnceLock<Mapa>; N] = [const { OnceLock::new() }; N];
static COMPLETOS: [OnceLock<Mapa>; N] = [const { OnceLock::new() }; N];

/// The texts a language's file says.
fn propio(i: usize) -> &'static Mapa {
    MAPAS[i].get_or_init(|| serde_json::from_str::<Mapa>(FUENTES[i]).unwrap_or_default())
}

/// A language's texts over English, over Spanish: every key, as the interface gets them.
fn completo(i: usize) -> &'static Mapa {
    COMPLETOS[i].get_or_init(|| {
        let mut m = propio(0).clone();
        m.extend(propio(1).iter().map(|(k, v)| (k.clone(), v.clone())));
        m.extend(
            propio(i)
                .iter()
                .filter(|(_, v)| !v.trim().is_empty())
                .map(|(k, v)| (k.clone(), v.clone())),
        );
        m
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

    fn i(&self) -> usize {
        self.idioma.0.min(N - 1)
    }

    /// The text for `clave`, in the language, else in English, else in Spanish, and as a last resort the key itself
    /// (which the tests make sure never happens).
    #[must_use]
    pub fn t(&self, clave: &str) -> String {
        completo(self.i())
            .get(clave)
            .cloned()
            .unwrap_or_else(|| clave.to_string())
    }

    /// Whether the language (or English or Spanish, its fallbacks) has a text for `clave`.
    fn tiene(&self, clave: &str) -> bool {
        completo(self.i()).contains_key(clave)
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

    /// A count written the person's way: `12.345` in Spanish and Portuguese, `12,345` in English, `12 345` in French
    /// (see [`Idioma::separadores`]).
    #[must_use]
    pub fn numero(&self, n: u64) -> String {
        let s = n.to_string();
        let (sep, _, desde) = self.idioma.separadores();
        if s.len() < desde {
            return s;
        }
        let mut out = String::with_capacity(s.len() + s.len() / 3 * 3);
        for (i, c) in s.chars().enumerate() {
            if i > 0 && (s.len() - i) % 3 == 0 {
                out.push(sep);
            }
            out.push(c);
        }
        out
    }

    /// Every text of the language, for the interface (sent once when it opens): over English and Spanish, so a text
    /// not translated yet still reads in English.
    #[must_use]
    pub fn todos(&self) -> &'static Mapa {
        completo(self.i())
    }
}

impl crate::tachon::Etiquetas for Textos {
    fn etiqueta(&self, clase: crate::datos::Clase) -> String {
        self.t(&format!("marca_{}", clase.clave()))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn the_three_languages_have_the_same_keys() {
        let (es, en, pt) = (propio(0), propio(1), propio(2));
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
        assert_eq!(Idioma::de_etiqueta("fr-FR").codigo(), "fr");
        assert_eq!(Idioma::de_etiqueta("zh-TW").codigo(), "zh-tw");
        assert!(Idioma::de_etiqueta("ar-SA").rtl());
        assert_eq!(Idioma::de_etiqueta("eu-ES"), Idioma::En);
    }

    #[test]
    fn the_42_languages_read_every_key() {
        // the files in the order of the program's list; each one JSON with only keys that exist; read over English, every
        // key of Spanish (2 Nov 2026)
        for (i, c) in CODIGOS.iter().enumerate() {
            assert_eq!(*c, IDIOMAS[i].codigo);
            let suyo: Mapa = serde_json::from_str(FUENTES[i])
                .map_err(|e| format!("{c}: {e}"))
                .unwrap();
            for k in suyo.keys() {
                assert!(
                    propio(0).contains_key(k),
                    "{c}: {k} ya no existe en español"
                );
            }
            let t = Textos::de(Idioma(i));
            for k in propio(0).keys() {
                assert_ne!(t.t(k), *k, "{c}: falta {k}");
            }
        }
        assert_eq!(
            Textos::de(Idioma::de_etiqueta("fr")).numero(12_345),
            "12\u{202f}345"
        );
        assert_eq!(Textos::de(Idioma::de_etiqueta("de")).numero(1234), "1.234");
    }
}
