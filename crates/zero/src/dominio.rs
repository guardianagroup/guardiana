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
