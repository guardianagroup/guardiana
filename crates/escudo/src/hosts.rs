//! «Hosts» (phase A3; the owner, 1 Nov 2026, 18:55: «que vea todo lo posible»): the hosts file sends a name to an
//! address before any DNS is asked, so a harmful program that writes into it can send a bank's or an update's name to
//! a server of its own, and no DNS protection sees it.
//!
//! The service reads the file every minute. A name that now goes to an address on the internet, and did not before, is
//! a grave warning. Addresses that only block (`0.0.0.0`, this computer itself) and the local network's (a printer, a
//! NAS) are not: blocklists and home setups write those. The first reading is the starting point.

use std::collections::BTreeMap;
use std::net::IpAddr;

use crate::primera_vez::Aviso;

/// The most names named in one warning.
const NOMBRADOS: usize = 3;

/// The file's names and the address each goes to, as Windows reads it: comments out, the first address of a name kept.
#[must_use]
pub fn entradas(texto: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for linea in texto.lines().take(200_000) {
        let linea = linea.split('#').next().unwrap_or_default();
        let mut partes = linea.split_whitespace();
        let Some(direccion) = partes.next() else {
            continue;
        };
        for nombre in partes {
            out.entry(nombre.to_ascii_lowercase())
                .or_insert_with(|| direccion.to_owned());
        }
    }
    out
}

/// Whether an address is on the internet: not one that blocks, not this computer, not the local network.
fn de_internet(direccion: &str) -> bool {
    match direccion.parse::<IpAddr>() {
        Ok(IpAddr::V4(a)) => {
            !(a.is_unspecified()
                || a.is_loopback()
                || a.is_private()
                || a.is_link_local()
                || a.is_broadcast()
                || a.is_documentation())
        }
        Ok(IpAddr::V6(a)) => {
            let primero = a.segments()[0];
            // fc00::/7 (the local network's own) and fe80::/10 (the link's)
            !(a.is_unspecified()
                || a.is_loopback()
                || (primero & 0xfe00) == 0xfc00
                || (primero & 0xffc0) == 0xfe80)
        }
        Err(_) => false,
    }
}

/// What changed from `antes` to `ahora`, as one grave warning: the names that now go to an address on the internet
/// they did not go to before.
#[must_use]
pub fn cambios(
    antes: &BTreeMap<String, String>,
    ahora: &BTreeMap<String, String>,
    cuando_ms: u64,
) -> Option<Aviso> {
    let nuevos: Vec<(&String, &String)> = ahora
        .iter()
        .filter(|(n, d)| de_internet(d) && antes.get(*n) != Some(*d))
        .collect();
    if nuevos.is_empty() {
        return None;
    }
    let nombrados: Vec<String> = nuevos
        .iter()
        .take(NOMBRADOS)
        .map(|(n, d)| format!("{n} a {d}"))
        .collect();
    let resto = nuevos.len().saturating_sub(NOMBRADOS);
    let mas = if resto > 0 {
        format!(" (y {resto} más)")
    } else {
        String::new()
    };
    let texto = format!(
        "El archivo hosts de Windows ha cambiado: ahora manda {}{mas}, antes de preguntar a ningún DNS. Si no lo hiciste \
         tú, alguien puede estar desviando esas webs a su propio servidor: revisa el archivo y quita esas líneas.",
        nombrados.join(", ")
    );
    let nombre = nuevos
        .iter()
        .map(|(n, d)| format!("{n}={d}"))
        .collect::<Vec<_>>()
        .join(",");
    Some(Aviso {
        regla: "hosts",
        programa: "hosts".to_owned(),
        ruta: String::new(),
        sitio: String::new(),
        nombre: nombre.chars().take(500).collect(),
        cuando_ms,
        fuente: None,
        texto,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn lee_como_windows() {
        let e = entradas("# comentario\n127.0.0.1 localhost\n  0.0.0.0  ads.example  tracker.example # bloqueo\n\n1.2.3.4 Banco.Example\n5.6.7.8 banco.example\n");
        assert_eq!(e.get("localhost").map(String::as_str), Some("127.0.0.1"));
        assert_eq!(
            e.get("tracker.example").map(String::as_str),
            Some("0.0.0.0")
        );
        // la primera dirección de un nombre es la que vale
        assert_eq!(e.get("banco.example").map(String::as_str), Some("1.2.3.4"));
    }

    #[test]
    fn solo_avisa_de_lo_que_desvia_a_internet() {
        let antes = entradas("127.0.0.1 localhost\n");
        // bloquear, o la red de casa: no
        let bloqueo = entradas("127.0.0.1 localhost\n0.0.0.0 ads.example\n192.168.1.20 nas.casa\nfe80::1 router.casa\n");
        assert_eq!(cambios(&antes, &bloqueo, 1), None);
        // desviar un nombre a una dirección de internet: grave
        let desvio = entradas("127.0.0.1 localhost\n203.0.113.9 doc.example\n198.51.100.7 banco.example\n8.8.4.4 actualiza.example\n");
        let a = cambios(&antes, &desvio, 1).unwrap();
        assert_eq!(a.regla, "hosts");
        assert!(
            a.texto.contains("actualiza.example a 8.8.4.4"),
            "{}",
            a.texto
        );
        // las direcciones de documentación (203.0.113.x, 198.51.100.x) no cuentan
        assert!(!a.texto.contains("doc.example"), "{}", a.texto);
        // lo mismo otra vez: nada nuevo
        assert_eq!(cambios(&desvio, &desvio, 2), None);
    }
}
