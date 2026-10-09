//! Who is behind each address a page asks for: the same lists GUARDIANA uses for the whole home
//! (categories, owners and countries, registered data brokers, AI services, delivery networks
//! and clouds), applied to every request of every tab.

use std::sync::OnceLock;

use guardiana_core::Category;
use guardiana_lists::Catalog;
use serde::Serialize;

use crate::dominio::{normaliza_host, sitio};

static CATALOGO: OnceLock<Catalog> = OnceLock::new();

fn catalogo() -> &'static Catalog {
    CATALOGO.get_or_init(Catalog::bundled)
}

/// A company registered as a data broker, as it wrote itself in the public registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Corredor {
    /// The name in the registry.
    pub nombre: &'static str,
    /// Its country, when it wrote one.
    pub pais: Option<&'static str>,
    /// Letters for what it ticked (see `corredores.txt`).
    pub declara: &'static str,
    /// The page it declared for deletion requests.
    pub url_derechos: Option<&'static str>,
}

/// What GUARDIANA knows about one host. A label, never a verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Destino {
    /// The host, normalised.
    pub host: String,
    /// Its registrable site.
    pub sitio: String,
    /// Category from the lists: `rastreador`, `publicidad`, `telemetria`, `esperado` or
    /// `desconocido`.
    pub categoria: &'static str,
    /// Who owns the name, when `empresas.txt` knows.
    pub empresa: Option<&'static str>,
    /// The owner's country (two letters), when known.
    pub pais: Option<&'static str>,
    /// The registered data broker the name belongs to.
    pub corredor: Option<Corredor>,
    /// The AI service the name belongs to.
    pub ia: Option<&'static str>,
    /// The delivery network, when the name only delivers what others serve.
    pub reparto: Option<&'static str>,
    /// The cloud, when the name is rented space on it.
    pub nube: Option<&'static str>,
}

impl Destino {
    /// Whether the lists say this name tracks or advertises: what «Bloquear rastreadores» cuts.
    #[must_use]
    pub fn sigue(&self) -> bool {
        matches!(self.categoria, "rastreador" | "publicidad")
    }

    /// The name people recognise: the company, the AI service, the broker, or the site.
    #[must_use]
    pub fn quien(&self) -> &str {
        self.ia
            .or(self.empresa)
            .or(self.corredor.as_ref().map(|c| c.nombre))
            .unwrap_or(&self.sitio)
    }
}

/// Everything the lists say about `host`.
#[must_use]
pub fn clasifica(host: &str) -> Destino {
    let host = normaliza_host(host);
    let categoria = match catalogo().category(&host) {
        Category::Rastreador => "rastreador",
        Category::Publicidad => "publicidad",
        Category::Telemetria => "telemetria",
        Category::Esperado => "esperado",
        Category::Desconocido => "desconocido",
    };
    let corredor = guardiana_lists::data_broker_of(&host).map(|b| Corredor {
        nombre: b.name,
        pais: b.country,
        declara: b.declared,
        url_derechos: b.rights_url,
    });
    Destino {
        sitio: sitio(&host),
        categoria,
        empresa: guardiana_lists::company_of(&host),
        pais: guardiana_lists::country_of(&host),
        corredor,
        ia: guardiana_lists::ai_service_of(&host),
        reparto: guardiana_lists::delivery_of(&host),
        nube: guardiana_lists::hosting_of(&host),
        host,
    }
}

/// Whether a request to `host` from a page of `sitio_pagina` goes to someone else: another site
/// that is not owned by the same company. `gstatic.com` on `google.com` is Google talking to
/// itself; `doubleclick.net` on `eltiempo.com` is a third party.
#[must_use]
pub fn es_tercero(host: &str, sitio_pagina: &str) -> bool {
    let s = sitio(host);
    if s == sitio_pagina {
        return false;
    }
    // `ana.github.io` and `luis.github.io` are both GitHub's, but not each other's.
    if crate::dominio::bajo_sufijo_privado(&s) || crate::dominio::bajo_sufijo_privado(sitio_pagina)
    {
        return true;
    }
    match (
        guardiana_lists::company_of(&s),
        guardiana_lists::company_of(sitio_pagina),
    ) {
        (Some(a), Some(b)) => a != b,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lists_say_who_is_behind_a_name() {
        let d = clasifica("stats.g.doubleclick.net");
        assert_eq!(d.sitio, "doubleclick.net");
        assert_eq!(d.empresa, Some("Google"));
        assert_eq!(d.pais, Some("US"));
        assert!(d.sigue());
        let c = clasifica("chatgpt.com");
        assert_eq!(c.ia, Some("OpenAI (ChatGPT)"));
        assert_eq!(c.quien(), "OpenAI (ChatGPT)");
    }

    #[test]
    fn third_parties_are_other_owners() {
        assert!(!es_tercero("fonts.gstatic.com", "google.com"));
        assert!(!es_tercero("img.eltiempo.com", "eltiempo.com"));
        assert!(es_tercero("stats.g.doubleclick.net", "eltiempo.com"));
        assert!(es_tercero("cdn.otra.co", "eltiempo.com"));
    }

    #[test]
    fn a_registered_broker_is_named_as_it_registered() {
        let d = clasifica("33across.com");
        assert_eq!(
            d.corredor.as_ref().map(|c| c.nombre),
            Some("33Across, Inc.")
        );
    }
}
