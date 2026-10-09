//! What the address bar does with what the person types: an address opens, anything else is a
//! search with the search engine the person chose. Typed names go out over HTTPS.

use std::net::IpAddr;

use crate::dominio::dominio_conocido;

/// A search engine the person can choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Buscador {
    /// Stable id, kept in the settings.
    pub id: &'static str,
    /// Its name, as the company writes it.
    pub nombre: &'static str,
    /// The search address; `{}` is the query, already encoded.
    pub plantilla: &'static str,
    /// It says it keeps no record of who searches what (the search box says so, or says the
    /// opposite).
    pub privado: bool,
}

/// The engines on offer, the ones that say they keep no record first (the settings list them
/// apart). The first is the default: it does not build a profile of the person.
pub const BUSCADORES: &[Buscador] = &[
    Buscador {
        id: "duckduckgo",
        nombre: "DuckDuckGo",
        plantilla: "https://duckduckgo.com/?q={}",
        privado: true,
    },
    Buscador {
        id: "startpage",
        nombre: "Startpage",
        plantilla: "https://www.startpage.com/do/search?query={}",
        privado: true,
    },
    Buscador {
        id: "brave",
        nombre: "Brave Search",
        plantilla: "https://search.brave.com/search?q={}",
        privado: true,
    },
    Buscador {
        id: "ecosia",
        nombre: "Ecosia",
        plantilla: "https://www.ecosia.org/search?q={}",
        privado: true,
    },
    Buscador {
        id: "qwant",
        nombre: "Qwant",
        plantilla: "https://www.qwant.com/?q={}",
        privado: true,
    },
    Buscador {
        id: "mojeek",
        nombre: "Mojeek",
        plantilla: "https://www.mojeek.com/search?q={}",
        privado: true,
    },
    Buscador {
        id: "swisscows",
        nombre: "Swisscows",
        plantilla: "https://swisscows.com/web?query={}",
        privado: true,
    },
    Buscador {
        id: "google",
        nombre: "Google",
        plantilla: "https://www.google.com/search?q={}",
        privado: false,
    },
    Buscador {
        id: "bing",
        nombre: "Bing",
        plantilla: "https://www.bing.com/search?q={}",
        privado: false,
    },
    Buscador {
        id: "yahoo",
        nombre: "Yahoo",
        plantilla: "https://search.yahoo.com/search?p={}",
        privado: false,
    },
    Buscador {
        id: "perplexity",
        nombre: "Perplexity",
        plantilla: "https://www.perplexity.ai/search?q={}",
        privado: false,
    },
    Buscador {
        id: "yandex",
        nombre: "Yandex",
        plantilla: "https://yandex.com/search/?text={}",
        privado: false,
    },
    Buscador {
        id: "baidu",
        nombre: "Baidu",
        plantilla: "https://www.baidu.com/s?wd={}",
        privado: false,
    },
];

/// The engine with `id`, or the default one.
#[must_use]
pub fn buscador(id: &str) -> &'static Buscador {
    BUSCADORES
        .iter()
        .find(|b| b.id == id)
        .unwrap_or(&BUSCADORES[0])
}

/// Percent-encoding for a query or a mail field: everything but unreserved characters, spaces
/// as `%20` (what both a search address and a `mailto:` understand).
#[must_use]
pub fn codifica(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(b));
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The address to open for `texto`.
#[must_use]
pub fn a_direccion(texto: &str, motor: &Buscador) -> String {
    let t = texto.trim();
    let busca = || motor.plantilla.replace("{}", &codifica(t));
    if t.is_empty() {
        return busca();
    }
    let minus = t.to_ascii_lowercase();
    if minus.starts_with("http://") || minus.starts_with("https://") {
        return match url::Url::parse(t) {
            Ok(u) if u.host_str().is_some() => u.to_string(),
            _ => busca(),
        };
    }
    if t.chars().any(char::is_whitespace) || t.contains("://") {
        return busca();
    }
    // A name without a scheme: «elpais.com», «localhost:8080/x», «192.168.1.1».
    let Ok(u) = url::Url::parse(&format!("http://{t}")) else {
        return busca();
    };
    let Some(host) = u.host_str() else {
        return busca();
    };
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let local = host == "localhost" || host.ends_with(".localhost");
    if local || host.parse::<IpAddr>().is_ok() {
        // The person's own devices and routers rarely have a certificate.
        return u.to_string();
    }
    if dominio_conocido(host) {
        let mut s = u;
        if s.set_scheme("https").is_ok() {
            return s.to_string();
        }
    }
    busca()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_open_and_words_search() {
        let d = buscador("");
        assert_eq!(d.id, "duckduckgo");
        assert_eq!(a_direccion("elpais.com", d), "https://elpais.com/");
        assert_eq!(
            a_direccion("www.realmadrid.com/es", d),
            "https://www.realmadrid.com/es"
        );
        assert_eq!(
            a_direccion("http://example.com/a", d),
            "http://example.com/a"
        );
        assert_eq!(
            a_direccion("localhost:8080/x", d),
            "http://localhost:8080/x"
        );
        assert_eq!(a_direccion("192.168.1.1", d), "http://192.168.1.1/");
        assert_eq!(
            a_direccion("node.js", d),
            "https://duckduckgo.com/?q=node.js"
        );
        assert_eq!(
            a_direccion("real madrid hoy", d),
            "https://duckduckgo.com/?q=real%20madrid%20hoy"
        );
        assert_eq!(
            a_direccion("¿qué es?", buscador("google")),
            "https://www.google.com/search?q=%C2%BFqu%C3%A9%20es%3F"
        );
        assert_eq!(
            a_direccion("javascript:alert(1)", d),
            "https://duckduckgo.com/?q=javascript%3Aalert%281%29"
        );
    }

    #[test]
    fn every_engine_is_https_with_one_query_and_the_private_ones_come_first() {
        let mut vistos = std::collections::BTreeSet::new();
        for b in BUSCADORES {
            assert!(vistos.insert(b.id), "{} twice", b.id);
            assert!(b.plantilla.starts_with("https://"), "{}", b.id);
            assert_eq!(b.plantilla.matches("{}").count(), 1, "{}", b.id);
            assert_eq!(buscador(b.id).id, b.id);
        }
        let primer_otro = BUSCADORES
            .iter()
            .position(|b| !b.privado)
            .unwrap_or(BUSCADORES.len());
        assert!(BUSCADORES[primer_otro..].iter().all(|b| !b.privado));
        assert!(BUSCADORES[0].privado);
    }
}
