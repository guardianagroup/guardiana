//! Hosts and sites: which part of an address names the organisation behind it.
//!
//! The Public Suffix List and the rules that read it live in `guardiana-lists` (moved there for
//! decision 205, so the resolver and ZERO read the same copy); this module re-exports them and
//! adds what only a browser needs: the host of a full address.

pub use guardiana_lists::dominio::{bajo_sufijo_privado, dominio_conocido, normaliza_host, sitio};

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
    fn hosts_come_out_of_addresses() {
        assert_eq!(
            host_de("https://WWW.Example.com:8443/a?b=1").as_deref(),
            Some("www.example.com")
        );
        assert_eq!(host_de("http://[::1]:80/").as_deref(), Some("::1"));
        assert_eq!(host_de("about:blank"), None);
        assert_eq!(host_de("no es una dirección"), None);
    }
}

/// Two-letter endings sold to anyone for their look (`.io`, `.co`, `.tv`, `.me`…): they say
/// nothing about a country.
const TERMINACIONES_DE_ADORNO: [&str; 42] = [
    "ac", "ad", "ag", "ai", "am", "as", "bz", "cc", "cd", "cf", "cm", "co", "cx", "dj", "eu", "fm",
    "ga", "gd", "gg", "gl", "gq", "gs", "im", "io", "je", "la", "ly", "me", "ml", "ms", "mu", "nu",
    "pw", "sc", "sh", "so", "st", "su", "tk", "to", "tv", "ws",
];

/// The country of a site's two-letter ending (`seznam.cz` → `CZ`, `bbc.co.uk` → `GB`), as an
/// ISO 3166 code; `None` for `.com` and the like, and for endings used for their look. It says
/// where the name is registered, not where the company is: shown only as that, for a company
/// the lists do not know (the owner, 12 Oct 2026: «que a nadie le salga desconocido»).
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
mod pruebas_del_pais {
    use super::pais_del_dominio;

    #[test]
    fn the_ending_gives_a_country_only_when_it_is_one() {
        assert_eq!(pais_del_dominio("seznam.cz").as_deref(), Some("CZ"));
        assert_eq!(pais_del_dominio("bbc.co.uk").as_deref(), Some("GB"));
        assert_eq!(pais_del_dominio("gemius.pl").as_deref(), Some("PL"));
        assert_eq!(pais_del_dominio("ad.gt").as_deref(), Some("GT"));
        for no in ["criteo.com", "x.io", "t.co", "a.tv", "y.eu", "cz", ""] {
            assert!(pais_del_dominio(no).is_none(), "{no}");
        }
    }
}
