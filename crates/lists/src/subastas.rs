//! «Tu visita, a subasta» (1.0.46): the part each name plays in the advertising auction that
//! starts when a web page opens, from `data/subastas.txt` (open, ours, with its sources).
//!
//! A label, never a verdict, and never the owner or the country: those come from
//! `empresas.txt` (see [`crate::company_of`]), checked one by one. Nothing here touches the
//! network; the PC's DNS and GUARDIANA ZERO both read it.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Serialize;

/// The list as shipped.
pub const SUBASTAS: &str = include_str!("../data/subastas.txt");

/// Identifier of the list in `list_source` and in the panel's list of lists.
pub const OWN_SOURCE_SUBASTAS: &str = "guardiana-subastas";

/// The part a name plays in the auction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Papel {
    /// An identifier that joins your visit across companies (UID2, ID5, RampID…).
    Id,
    /// Only there to cross identifiers between companies («cookie sync»).
    Sincroniza,
    /// The page's ad program: asks for the bids or receives them and picks the ad.
    Servidor,
    /// Where Prebid.js sends a bid request.
    Puja,
}

impl Papel {
    /// The key in texts and messages.
    #[must_use]
    pub const fn clave(self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::Sincroniza => "sincroniza",
            Self::Servidor => "servidor",
            Self::Puja => "puja",
        }
    }
}

/// An identifier system, with the page where a person can opt out of it (when one is published).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SistemaId {
    /// Its usual name (`UID2`).
    pub nombre: &'static str,
    /// The opt-out page, as the list writes it with its source; `None` when there is none.
    pub baja: Option<&'static str>,
}

/// What the list says about one name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Rol {
    /// Its part.
    pub papel: Papel,
    /// The entry that matched (the name or a parent of it).
    pub coincide: &'static str,
    /// For an identifier: which one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sistema: Option<SistemaId>,
}

#[derive(Debug, Default)]
struct Tabla {
    nombres: HashMap<&'static str, (Papel, Option<SistemaId>)>,
    pujadores: HashMap<&'static str, &'static str>,
}

fn lee(texto: &'static str) -> Tabla {
    let mut t = Tabla::default();
    let mut papel: Option<Papel> = None;
    let mut sistema: Option<SistemaId> = None;
    let mut en_pujadores = false;
    for linea in texto.lines() {
        let linea = linea.split('#').next().unwrap_or("").trim();
        if linea.is_empty() {
            continue;
        }
        if let Some(cab) = linea.strip_prefix('@') {
            let mut c = cab.split('|');
            let nombre = c.next().unwrap_or("").trim();
            en_pujadores = nombre == "pujadores";
            sistema = None;
            papel = match nombre {
                "id" => {
                    let s = c.next().unwrap_or("").trim();
                    let baja = c
                        .next()
                        .map(str::trim)
                        .filter(|b| b.starts_with("https://"));
                    sistema = Some(SistemaId { nombre: s, baja });
                    Some(Papel::Id)
                }
                "sincroniza" => Some(Papel::Sincroniza),
                "servidor" => Some(Papel::Servidor),
                "puja" => Some(Papel::Puja),
                _ => None,
            };
            continue;
        }
        if en_pujadores {
            if let Some(resto) = linea.strip_prefix('=') {
                let mut p = resto.split_whitespace();
                if let (Some(codigo), Some(sitio)) = (p.next(), p.next()) {
                    t.pujadores.entry(codigo).or_insert(sitio);
                }
            }
            continue;
        }
        let Some(p) = papel else { continue };
        if !linea.contains('.') || linea.contains(char::is_whitespace) {
            continue;
        }
        // The first section that names it wins: id, then sync, then the ad program, then bids.
        t.nombres.entry(linea).or_insert((p, sistema.clone()));
    }
    t
}

fn tabla() -> &'static Tabla {
    static T: OnceLock<Tabla> = OnceLock::new();
    T.get_or_init(|| lee(SUBASTAS))
}

/// The part `qname` (or a parent of it) plays in the auction, when the list knows it.
#[must_use]
pub fn papel_de(qname: &str) -> Option<Rol> {
    let n = qname.trim().trim_end_matches('.').to_ascii_lowercase();
    let t = tabla();
    let mut resto = n.as_str();
    loop {
        if let Some((k, (papel, sistema))) = t.nombres.get_key_value(resto) {
            return Some(Rol {
                papel: *papel,
                coincide: k,
                sistema: sistema.clone(),
            });
        }
        match resto.find('.') {
            Some(i) if resto[i + 1..].contains('.') => resto = &resto[i + 1..],
            _ => return None,
        }
    }
}

/// The site of the names of a Prebid.js bidder (`criteo` → `criteo.com`), to know whose it is.
#[must_use]
pub fn sitio_de_pujador(codigo: &str) -> Option<&'static str> {
    let c = codigo.trim().to_ascii_lowercase();
    tabla().pujadores.get(c.as_str()).copied()
}

/// Every name of the list, with its part: what the DNS cuts with «No me subastes».
pub fn nombres() -> impl Iterator<Item = (&'static str, Papel)> {
    tabla().nombres.iter().map(|(k, (p, _))| (*k, *p))
}

/// How many names and bidders the list has.
#[must_use]
pub fn cuantos() -> (usize, usize) {
    let t = tabla();
    (t.nombres.len(), t.pujadores.len())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn the_list_reads_whole_and_each_name_has_one_part() {
        let (n, p) = cuantos();
        assert!(n > 500, "{n}");
        assert!(p > 300, "{p}");
        assert_eq!(papel_de("prod.uidapi.com").unwrap().papel, Papel::Id);
        assert_eq!(papel_de("x.id5-sync.com").unwrap().papel, Papel::Id);
        assert_eq!(
            papel_de("match.adsrvr.org").unwrap().papel,
            Papel::Sincroniza
        );
        assert_eq!(
            papel_de("grid-bidder.criteo.com").unwrap().papel,
            Papel::Puja
        );
        assert_eq!(
            papel_de("securepubads.g.doubleclick.net").unwrap().papel,
            Papel::Servidor
        );
        // The parent of a bid name is not a bid name: criteo.com is the company's own web.
        assert!(papel_de("criteo.com").is_none());
        assert!(papel_de("www.eltiempo.com").is_none());
    }

    #[test]
    fn the_opt_out_pages_are_kept_and_never_made_up() {
        let uid2 = papel_de("uidapi.com").unwrap().sistema.unwrap();
        assert_eq!(uid2.nombre, "UID2");
        assert_eq!(uid2.baja, Some("https://www.transparentadvertising.com/"));
        let panorama = papel_de("crwdcntrl.net").unwrap().sistema.unwrap();
        assert_eq!(panorama.baja, None);
    }

    #[test]
    fn the_privacy_and_opt_out_pages_are_never_in_the_list() {
        for (n, _) in nombres() {
            for malo in ["privacy.", "optout.", "console.", "docs."] {
                assert!(!n.starts_with(malo), "{n}");
            }
            assert_ne!(n, "s.yimg.com");
        }
    }

    #[test]
    fn the_bidders_of_the_eltiempo_test_are_known_by_their_owner() {
        // 18 Oct 2026, eltiempo.com: six bidders in one auction.
        for (codigo, sitio, dueno) in [
            ("mathildeads", "mathilde-ads.com", None),
            ("criteo", "criteo.com", Some("Criteo")),
            ("rubicon", "rubiconproject.com", Some("Magnite")),
            ("teads", "teads.tv", Some("Teads")),
            ("smartadserver", "smartadserver.com", Some("Equativ")),
            ("richaudience", "richaudience.com", Some("Rich Audience")),
        ] {
            assert_eq!(sitio_de_pujador(codigo), Some(sitio), "{codigo}");
            assert_eq!(crate::company_of(sitio), dueno, "{codigo}");
        }
    }
}
