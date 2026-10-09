//! Hosts and sites: which part of an address names the organisation behind it.
//!
//! `ads.tracker.co.uk` and `www.tracker.co.uk` are one site, `tracker.co.uk`; `a.github.io` and
//! `b.github.io` are two. Only the Public Suffix List knows where the line falls, so it travels
//! inside the binary (`data/public_suffix_list.dat`, MPL-2.0, see `data/LEEME.md`). Nothing is
//! downloaded: a newer list comes with a newer version.

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

/// The host of an address, normalised, or `None` when it has none (`about:blank`, `data:`).
#[must_use]
pub fn host_de(direccion: &str) -> Option<String> {
    let u = url::Url::parse(direccion).ok()?;
    u.host_str().map(normaliza_host).filter(|h| !h.is_empty())
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
    fn hosts_come_out_of_addresses() {
        assert_eq!(
            host_de("https://WWW.Example.com:8443/a?b=1").as_deref(),
            Some("www.example.com")
        );
        assert_eq!(host_de("http://[::1]:80/").as_deref(), Some("::1"));
        assert_eq!(host_de("about:blank"), None);
        assert_eq!(host_de("no es una dirección"), None);
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
