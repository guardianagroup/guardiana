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

/// The engines on offer, in the order the box and the settings show them. Since 1.0.58 (the owner,
/// 15 Nov 2026: only Google, Brave and DuckDuckGo) there are three: Google, the default; Brave
/// Search and DuckDuckGo, which say they keep no record of who searches what. Whoever had chosen
/// another one goes back to Google (`buscador` falls back to the default). Tor is not here: this
/// browser has no way to reach the Tor network, and an entry that only says «Tor» without going
/// through it would be a lie.
pub const BUSCADORES: &[Buscador] = &[
    Buscador {
        id: "google",
        nombre: "Google",
        plantilla: "https://www.google.com/search?q={}",
        privado: false,
    },
    Buscador {
        id: "brave",
        nombre: "Brave Search",
        plantilla: "https://search.brave.com/search?q={}",
        privado: true,
    },
    Buscador {
        id: "duckduckgo",
        nombre: "DuckDuckGo",
        plantilla: "https://duckduckgo.com/?q={}",
        privado: true,
    },
];

/// The engine for whoever has not chosen one (2 Nov 2026; the owner, 09:10: «hazlo dentro del
/// programa para que la gente vaya libremente por todo Google»). ZERO runs on the person's own
/// computer and connection, so Google answers it like any browser: all of Google, no box of 50
/// sites. Whoever chose one of the three keeps it.
pub const POR_DEFECTO: &str = "google";

/// The engines offered until the 1.0.57 that are no longer (1.0.58: only three, the ones that work
/// everywhere). All of them were private ones: whoever had chosen one goes on with a private
/// engine, DuckDuckGo, and not with Google (second voice of the 1.0.58, 15 Nov 2026).
const RETIRADOS_PRIVADOS: &[&str] = &[
    "startpage",
    "ecosia",
    "qwant",
    "mojeek",
    "swisscows",
    "metager",
];

/// The engine with `id`; for one of the engines no longer offered, DuckDuckGo; else the default
/// one.
#[must_use]
pub fn buscador(id: &str) -> &'static Buscador {
    let id = if RETIRADOS_PRIVADOS.contains(&id) {
        "duckduckgo"
    } else {
        id
    };
    BUSCADORES
        .iter()
        .find(|b| b.id == id)
        .or_else(|| BUSCADORES.iter().find(|b| b.id == POR_DEFECTO))
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
        assert_eq!(d.id, "google");
        assert_eq!(buscador("duckduckgo").id, "duckduckgo");
        assert_eq!(buscador("no-existe").id, "google");
        // los que se quitaron en la 1.0.58 eran privados: quien tenía uno sigue con uno privado
        for viejo in [
            "startpage",
            "ecosia",
            "qwant",
            "mojeek",
            "swisscows",
            "metager",
        ] {
            assert_eq!(buscador(viejo).id, "duckduckgo", "{viejo}");
        }
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
            "https://www.google.com/search?q=node.js"
        );
        assert_eq!(
            a_direccion("real madrid hoy", d),
            "https://www.google.com/search?q=real%20madrid%20hoy"
        );
        assert_eq!(
            a_direccion("¿qué es?", buscador("google")),
            "https://www.google.com/search?q=%C2%BFqu%C3%A9%20es%3F"
        );
        assert_eq!(
            a_direccion("javascript:alert(1)", d),
            "https://www.google.com/search?q=javascript%3Aalert%281%29"
        );
    }

    #[test]
    fn every_engine_is_https_with_one_query_in_the_owners_order() {
        let mut vistos = std::collections::BTreeSet::new();
        for b in BUSCADORES {
            assert!(vistos.insert(b.id), "{} twice", b.id);
            assert!(b.plantilla.starts_with("https://"), "{}", b.id);
            assert_eq!(b.plantilla.matches("{}").count(), 1, "{}", b.id);
            assert_eq!(buscador(b.id).id, b.id);
        }
        // 15 Nov 2026 (the owner): only Google, Brave and DuckDuckGo, in that order.
        let ids: Vec<&str> = BUSCADORES.iter().map(|b| b.id).collect();
        assert_eq!(ids, ["google", "brave", "duckduckgo"]);
        // An engine chosen before 1.0.58 and gone since goes back to the default.
        for viejo in ["bing", "qwant", "startpage", "yandex"] {
            assert_eq!(buscador(viejo).id, POR_DEFECTO);
        }
        // Nothing that claims the Tor network: this browser cannot reach it.
        assert!(BUSCADORES
            .iter()
            .all(|b| !b.id.contains("tor") && !b.plantilla.contains(".onion")));
    }
}
