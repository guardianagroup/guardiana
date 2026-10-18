//! «Quién vende tus datos dentro de tu PC» (1.0.46): the open list behind the page
//! (`data/quien-vende.json`). It adds to the California registry of data brokers
//! (`corredores.txt`, [`crate::data_broker_of`]) the names a program, a browser extension or a
//! TV of the house may call and that a regulator or a public investigation documented selling
//! or passing on data: each entry names its case, with date and source. A label about who owns
//! the name, never a verdict about what one query carried.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// The list, as it travels inside the binary.
pub const QUIEN_VENDE_JSON: &str = include_str!("../data/quien-vende.json");

/// A documented case: who said it, when, and where to read it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Caso {
    /// Its key (`ftc-avast-2024`); the page's sentence about it is the text `qv_caso_<key>`.
    pub id: String,
    /// The date of the decision, the complaint or the report, `AAAA-MM-DD`.
    pub fecha: String,
    /// Who published it.
    pub medio: String,
    /// Where to read it.
    pub url: String,
}

/// A name and the case that documents its owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dominio {
    /// The domain; it matches itself and every subdomain.
    pub dominio: String,
    /// Its owner, as the case names it.
    pub empresa: String,
    /// `venta` (sold browsing data), `ubicacion` (sold location data), `acr` (a TV reporting
    /// what is on screen).
    pub tipo: String,
    /// The case.
    pub caso: String,
}

/// A browser extension a case named, by its store id when known and by its name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Extension {
    /// Its id in the Chrome Web Store, when the case or the store gives it.
    #[serde(default)]
    pub id: Option<String>,
    /// Its name in the store.
    pub nombre: String,
    /// The case.
    pub caso: String,
}

/// An antivirus whose maker has a documented case about selling data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Antivirus {
    /// A word its name contains, lower case.
    pub contiene: String,
    /// The case.
    pub caso: String,
}

/// A computer maker, by the signer of its programs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fabricante {
    /// What the signer's name contains, lower case (`hp inc`).
    pub firmante: String,
    /// The brand people know.
    pub marca: String,
}

/// An AI service and the guide to turn off training on the person's chats.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuiaIa {
    /// The service as [`crate::ai_service_of`] names it.
    pub servicio: String,
    /// The guide's key: its text is `qv_ia_guia_<key>`.
    pub guia: String,
    /// A documented case about it, when there is one.
    #[serde(default)]
    pub caso: Option<String>,
}

/// The whole list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Catalogo {
    /// Every case.
    pub casos: Vec<Caso>,
    /// Names with a documented owner.
    pub dominios: Vec<Dominio>,
    /// Extensions a case named.
    pub extensiones: Vec<Extension>,
    /// Antivirus makers with a case.
    pub antivirus: Vec<Antivirus>,
    /// Computer makers, by signer.
    pub fabricantes: Vec<Fabricante>,
    /// AI services and their guides.
    pub ia: Vec<GuiaIa>,
    /// The web addresses where people chat with an AI: an extension that may read them may read
    /// those chats.
    pub ia_dominios_chat: Vec<String>,
}

static CATALOGO: std::sync::OnceLock<Catalogo> = std::sync::OnceLock::new();
static POR_DOMINIO: std::sync::OnceLock<HashMap<String, usize>> = std::sync::OnceLock::new();

/// The list. A list that does not parse is empty (a test makes sure it parses).
#[must_use]
pub fn catalogo() -> &'static Catalogo {
    CATALOGO.get_or_init(|| serde_json::from_str(QUIEN_VENDE_JSON).unwrap_or_default())
}

/// The case with this key.
#[must_use]
pub fn caso(id: &str) -> Option<&'static Caso> {
    catalogo().casos.iter().find(|c| c.id == id)
}

/// The documented owner of a queried name (`acr0.samsungacr.com` → Samsung ACR), or `None`.
#[must_use]
pub fn dominio_de(qname: &str) -> Option<&'static Dominio> {
    let c = catalogo();
    let mapa = POR_DOMINIO.get_or_init(|| {
        c.dominios
            .iter()
            .enumerate()
            .map(|(i, d)| (d.dominio.to_ascii_lowercase(), i))
            .collect()
    });
    let nombre = qname.trim_end_matches('.').to_ascii_lowercase();
    let mut resto = nombre.as_str();
    loop {
        if let Some(i) = mapa.get(resto) {
            return c.dominios.get(*i);
        }
        let p = resto.find('.')?;
        resto = &resto[p + 1..];
    }
}

/// A case naming this extension: by its id first, then by its exact name (case aside).
#[must_use]
pub fn extension_conocida(id: &str, nombre: &str) -> Option<&'static Extension> {
    let c = catalogo();
    c.extensiones
        .iter()
        .find(|e| e.id.as_deref().is_some_and(|i| i.eq_ignore_ascii_case(id)))
        .or_else(|| {
            let n = nombre.trim();
            c.extensiones
                .iter()
                .find(|e| e.id.is_none() && !n.is_empty() && e.nombre.eq_ignore_ascii_case(n))
        })
}

/// A case about this antivirus's maker, by a word of its name (`Avast Free Antivirus`).
#[must_use]
pub fn antivirus_con_caso(nombre: &str) -> Option<&'static Antivirus> {
    let n = nombre.to_ascii_lowercase();
    catalogo().antivirus.iter().find(|a| {
        n.split(|c: char| !c.is_ascii_alphanumeric())
            .any(|palabra| palabra == a.contiene)
    })
}

/// The computer maker that signs a program (`HP Inc.` → HP), or `None`.
#[must_use]
pub fn fabricante_de(firmante: &str) -> Option<&'static Fabricante> {
    let f = firmante.to_ascii_lowercase();
    if f.is_empty() {
        return None;
    }
    catalogo()
        .fabricantes
        .iter()
        .find(|m| f.contains(m.firmante.as_str()))
}

/// The guide for an AI service as [`crate::ai_service_of`] names it.
#[must_use]
pub fn guia_ia(servicio: &str) -> Option<&'static GuiaIa> {
    catalogo().ia.iter().find(|g| g.servicio == servicio)
}

/// Whether a web pattern of an extension (`<all_urls>`, `https://*/*`, `*://chatgpt.com/*`)
/// reaches every website.
#[must_use]
pub fn patron_todas_las_webs(p: &str) -> bool {
    let p = p.trim();
    p == "<all_urls>"
        || p == "*://*/*"
        || p == "http://*/*"
        || p == "https://*/*"
        || p == "*://*/"
        || p == "https://*/"
        || p == "http://*/"
}

/// Whether a web pattern reaches one of the addresses where people chat with an AI.
#[must_use]
pub fn patron_chat_ia(p: &str) -> bool {
    if patron_todas_las_webs(p) {
        return true;
    }
    let sin = p
        .trim()
        .split_once("://")
        .map_or(p.trim(), |(_, r)| r)
        .split('/')
        .next()
        .unwrap_or("")
        .trim_start_matches("*.")
        .to_ascii_lowercase();
    !sin.is_empty()
        && catalogo().ia_dominios_chat.iter().any(|d| {
            sin == *d || d.ends_with(&format!(".{sin}")) || sin.ends_with(&format!(".{d}"))
        })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn the_list_parses_and_every_entry_names_a_case_that_exists() {
        let c: Catalogo = serde_json::from_str(QUIEN_VENDE_JSON).unwrap();
        assert!(!c.casos.is_empty());
        for d in &c.dominios {
            assert!(caso(&d.caso).is_some(), "{}", d.dominio);
            assert!(
                matches!(d.tipo.as_str(), "venta" | "ubicacion" | "acr"),
                "{}",
                d.tipo
            );
        }
        for e in &c.extensiones {
            assert!(caso(&e.caso).is_some(), "{}", e.nombre);
        }
        for a in &c.antivirus {
            assert!(caso(&a.caso).is_some());
        }
        for g in &c.ia {
            assert!(g.caso.as_deref().is_none_or(|k| caso(k).is_some()));
            // Every guide is about a service the AI list knows.
            assert!(
                crate::parse_guardiana(crate::IA).any(|(s, _)| s == Some(g.servicio.as_str())),
                "{}",
                g.servicio
            );
        }
        for k in &c.casos {
            assert!(k.url.starts_with("https://"), "{}", k.id);
            assert_eq!(k.fecha.len(), 10, "{}", k.id);
        }
    }

    #[test]
    fn names_extensions_makers_and_patterns_are_recognised() {
        assert_eq!(dominio_de("acr0.samsungacr.com").unwrap().tipo, "acr");
        assert_eq!(dominio_de("api.kochava.com.").unwrap().empresa, "Kochava");
        assert!(dominio_de("notkochava.com").is_none());
        assert!(extension_conocida("eppiocemhmnlbhjplcgkofciiegomcon", "x").is_some());
        assert!(extension_conocida("otro", "urban vpn proxy").is_some());
        assert!(extension_conocida("otro", "uBlock Origin").is_none());
        assert_eq!(fabricante_de("HP Inc.").unwrap().marca, "HP");
        assert!(fabricante_de("").is_none());
        assert!(antivirus_con_caso("Avast Free Antivirus").is_some());
        assert!(antivirus_con_caso("Bitdefender").is_none());
        assert!(patron_todas_las_webs("<all_urls>"));
        assert!(patron_chat_ia("https://chatgpt.com/*"));
        assert!(patron_chat_ia("*://*.claude.ai/*"));
        assert!(!patron_chat_ia("https://example.com/*"));
    }
}
