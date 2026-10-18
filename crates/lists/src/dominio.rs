//! Hosts and sites: which part of a name names the organisation behind it.
//!
//! `ads.tracker.co.uk` and `www.tracker.co.uk` are one site, `tracker.co.uk`; `a.github.io` and
//! `b.github.io` are two. Only the Public Suffix List knows where the line falls, so it travels
//! inside the binary (`data/public_suffix_list.dat`, MPL-2.0, see `data/LEEME.md`). Nothing is
//! downloaded: a newer list comes with a newer version. It lives here, with the other bundled
//! lists, so the resolver (decision 205: a name dressed as another) and GUARDIANA ZERO read the
//! same one.

use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::OnceLock;

const PSL: &str = include_str!("../data/public_suffix_list.dat");

#[derive(Default)]
struct Reglas {
    normales: HashSet<String>,
    /// `*.ck` is kept as `ck`: every label directly under it is a public suffix.
    comodines: HashSet<String>,
    /// `!www.ck` is kept as `www.ck`: the exception is registrable.
    excepciones: HashSet<String>,
    /// Rules from the list's private section (`github.io`, `cloudfront.net`): each name under
    /// them belongs to a different customer, whoever runs the service.
    privados: HashSet<String>,
}

static REGLAS: OnceLock<Reglas> = OnceLock::new();

fn a_ascii(regla: &str) -> String {
    if regla.is_ascii() {
        regla.to_ascii_lowercase()
    } else {
        idna::domain_to_ascii(regla).unwrap_or_else(|_| regla.to_lowercase())
    }
}

fn reglas() -> &'static Reglas {
    REGLAS.get_or_init(|| {
        let mut r = Reglas::default();
        let mut privado = false;
        for linea in PSL.lines() {
            if linea.contains("===BEGIN PRIVATE DOMAINS===") {
                privado = true;
            } else if linea.contains("===END PRIVATE DOMAINS===") {
                privado = false;
            }
            let linea = linea.split_whitespace().next().unwrap_or("");
            if linea.is_empty() || linea.starts_with("//") {
                continue;
            }
            if let Some(resto) = linea.strip_prefix('!') {
                r.excepciones.insert(a_ascii(resto));
            } else if let Some(resto) = linea.strip_prefix("*.") {
                if privado {
                    r.privados.insert(a_ascii(resto));
                }
                r.comodines.insert(a_ascii(resto));
            } else {
                if privado {
                    r.privados.insert(a_ascii(linea));
                }
                r.normales.insert(a_ascii(linea));
            }
        }
        r
    })
}

/// How many labels at the right of `etiquetas` are the public suffix (at least one).
fn largo_sufijo(etiquetas: &[&str]) -> usize {
    let r = reglas();
    let n = etiquetas.len();
    for i in 0..n {
        let candidato = etiquetas[i..].join(".");
        if r.excepciones.contains(&candidato) {
            return n - i - 1;
        }
        if r.normales.contains(&candidato) {
            return n - i;
        }
        if i + 1 < n && r.comodines.contains(&etiquetas[i + 1..].join(".")) {
            return n - i;
        }
    }
    1
}

/// A host the way every list in GUARDIANA writes it: lowercase, no trailing dot, no brackets.
#[must_use]
pub fn normaliza_host(host: &str) -> String {
    host.trim()
        .trim_end_matches('.')
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase()
}

/// The registrable site of a host: `ads.tracker.co.uk` → `tracker.co.uk`. An IP address, a
/// single label (`localhost`) or a public suffix by itself is its own site.
#[must_use]
pub fn sitio(host: &str) -> String {
    let host = normaliza_host(host);
    if host.parse::<IpAddr>().is_ok() || !host.contains('.') {
        return host;
    }
    let etiquetas: Vec<&str> = host.split('.').filter(|e| !e.is_empty()).collect();
    let sufijo = largo_sufijo(&etiquetas);
    if sufijo >= etiquetas.len() {
        return host;
    }
    etiquetas[etiquetas.len() - sufijo - 1..].join(".")
}

/// Whether `host` sits under a suffix from the list's private section: a name rented on a
/// shared service (`ana.github.io`, `d1.cloudfront.net`), not the service's own site.
#[must_use]
pub fn bajo_sufijo_privado(host: &str) -> bool {
    let host = normaliza_host(host);
    if host.parse::<IpAddr>().is_ok() {
        return false;
    }
    let etiquetas: Vec<&str> = host.split('.').filter(|e| !e.is_empty()).collect();
    let r = reglas();
    (1..etiquetas.len()).any(|i| r.privados.contains(&etiquetas[i..].join(".")))
}

/// The site an alias stands for when GUARDIANA keeps it (1.0.45): its registrable site, except
/// under a suffix of the list's private section (`d1x.cloudfront.net`, `tienda.myshopify.com`),
/// where it is the service's suffix itself (`cloudfront.net`): which customer of the service it
/// was is not needed to say whose road it is, so it is not kept.
#[must_use]
pub fn sitio_de_alias(host: &str) -> String {
    let host = normaliza_host(host);
    if host.parse::<IpAddr>().is_ok() {
        return host;
    }
    let etiquetas: Vec<&str> = host.split('.').filter(|e| !e.is_empty()).collect();
    let r = reglas();
    for i in 1..etiquetas.len() {
        let s = etiquetas[i..].join(".");
        if r.privados.contains(&s) {
            return s;
        }
    }
    sitio(&host)
}

/// Whether the last label of `host` is a top-level domain the Public Suffix List knows
/// (`com`, `co`, `xn--p1ai`): what tells «elpais.com» (an address) from «node.js» (a search).
#[must_use]
pub fn dominio_conocido(host: &str) -> bool {
    let host = normaliza_host(host);
    let Some(tld) = host.rsplit('.').next().filter(|t| !t.is_empty()) else {
        return false;
    };
    let r = reglas();
    host.contains('.') && (r.normales.contains(tld) || r.comodines.contains(tld))
}

/// Two-letter endings sold to anyone for their look (`.io`, `.co`, `.tv`, `.me`…): they say
/// nothing about a country.
const TERMINACIONES_DE_ADORNO: [&str; 51] = [
    "ac", "ad", "ag", "ai", "am", "as", "bi", "bz", "cc", "cd", "cf", "cm", "co", "cx", "dj", "eu",
    "ki", "lc", "nr", "sx", "tc", "tl", "vc", "vg", "fm", "ga", "gd", "gg", "gl", "gq", "gs", "im",
    "io", "je", "la", "ly", "me", "ml", "ms", "mu", "nu", "pw", "sc", "sh", "so", "st", "su", "tk",
    "to", "tv", "ws",
];

/// The country of a site's two-letter ending (`seznam.cz` → `CZ`, `bbc.co.uk` → `GB`), as an
/// ISO 3166 code; `None` for `.com` and the like, and for endings used for their look. It says
/// where the name is registered, not where the company is: shown only as that, for a company
/// the lists do not know (the owner, 12 Oct 2026; shared with the GUARDIANA panel since 1.0.16: «que a nadie le salga desconocido»).
#[must_use]
pub fn pais_del_dominio(sitio: &str) -> Option<String> {
    let fin = sitio.trim_end_matches('.').rsplit('.').next()?;
    if fin.len() != 2
        || !fin.bytes().all(|b| b.is_ascii_lowercase())
        || TERMINACIONES_DE_ADORNO.contains(&fin)
        || !sitio.contains('.')
    {
        return None;
    }
    Some(if fin == "uk" {
        "GB".to_string()
    } else {
        fin.to_ascii_uppercase()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sites_follow_the_public_suffix_list() {
        assert_eq!(sitio("ads.tracker.co.uk"), "tracker.co.uk");
        assert_eq!(sitio("www.Example.COM."), "example.com");
        assert_eq!(sitio("a.b.github.io"), "b.github.io");
        assert_eq!(sitio("bancolombia.com.co"), "bancolombia.com.co");
        assert_eq!(sitio("www.gov.co"), "www.gov.co");
        assert_eq!(sitio("cdn.globo.com.br"), "globo.com.br");
        assert_eq!(sitio("localhost"), "localhost");
        assert_eq!(sitio("127.0.0.1"), "127.0.0.1");
        assert_eq!(sitio("co.uk"), "co.uk");
        // Wildcard and exception rules: `*.ck` and `!www.ck`.
        assert_eq!(sitio("a.b.ck"), "a.b.ck");
        assert_eq!(sitio("x.www.ck"), "www.ck");
    }

    #[test]
    fn unicode_rules_match_their_ascii_hosts() {
        // `个人.hk` is in the list; hosts arrive in their xn-- form.
        let ascii = idna::domain_to_ascii("个人.hk").unwrap_or_default();
        assert_eq!(sitio(&format!("a.b.{ascii}")), format!("b.{ascii}"));
    }

    #[test]
    fn rented_names_are_told_apart() {
        assert!(bajo_sufijo_privado("ana.github.io"));
        assert!(bajo_sufijo_privado("d1234.cloudfront.net"));
        assert!(!bajo_sufijo_privado("www.github.com"));
        assert!(!bajo_sufijo_privado("bancolombia.com.co"));
        assert!(!bajo_sufijo_privado("127.0.0.1"));
    }

    #[test]
    fn known_top_level_domains() {
        assert!(dominio_conocido("elpais.com"));
        assert!(dominio_conocido("www.realmadrid.com"));
        assert!(dominio_conocido("bancolombia.com.co"));
        assert!(!dominio_conocido("node.js"));
        assert!(!dominio_conocido("localhost"));
        assert!(!dominio_conocido("hola"));
    }
}

#[cfg(test)]
mod pruebas_del_pais {
    use super::pais_del_dominio;

    #[test]
    fn the_ending_gives_a_country_only_when_it_is_one() {
        assert_eq!(pais_del_dominio("seznam.cz").as_deref(), Some("CZ"));
        assert_eq!(pais_del_dominio("bbc.co.uk").as_deref(), Some("GB"));
        assert_eq!(pais_del_dominio("gemius.pl").as_deref(), Some("PL"));
        assert_eq!(pais_del_dominio("ad.gt").as_deref(), Some("GT"));
        // «.bi» is sold as «business intelligence» (newsroom.bi is Deep.BI, from Poland).
        assert!(pais_del_dominio("newsroom.bi").is_none());
        for no in ["criteo.com", "x.io", "t.co", "a.tv", "y.eu", "cz", ""] {
            assert!(pais_del_dominio(no).is_none(), "{no}");
        }
    }
}
