//! Who is behind each address a page asks for: the same lists GUARDIANA uses for the whole home
//! (categories, owners and countries, registered data brokers, AI services, delivery networks
//! and clouds), applied to every request of every tab; and, because the browser sees the whole
//! address and not only the name, a short list of our own for the tracking pixels that live on a
//! name that also does other things (`www.facebook.com/tr`, `data/pixeles.txt`).

use std::sync::OnceLock;

use guardiana_core::Category;
use guardiana_lists::Catalog;
use serde::Serialize;

use crate::dominio::{bajo_sufijo_privado, normaliza_host, sitio};

static CATALOGO: OnceLock<Catalog> = OnceLock::new();

fn catalogo() -> &'static Catalog {
    CATALOGO.get_or_init(Catalog::bundled)
}

const PIXELES: &str = include_str!("../data/pixeles.txt");

/// One line of `pixeles.txt`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pixel {
    /// The exact host, or the suffix when `bajo` is set (`*.ingest.sentry.io`).
    host: String,
    /// Any name under `host`, not `host` itself.
    bajo: bool,
    /// Path segments; `*` stands for any one segment. Empty: the whole name.
    ruta: Vec<String>,
    empresa: &'static str,
    pais: Option<&'static str>,
    categoria: &'static str,
}

static PIXELES_LEIDOS: OnceLock<Vec<Pixel>> = OnceLock::new();

fn pixeles() -> &'static [Pixel] {
    PIXELES_LEIDOS.get_or_init(|| {
        let mut out = Vec::new();
        for linea in PIXELES.lines() {
            let linea = linea.trim();
            if linea.is_empty() || linea.starts_with('#') {
                continue;
            }
            let partes: Vec<&'static str> = linea.split('|').map(str::trim).collect();
            let [direccion, empresa, pais, categoria] = partes[..] else {
                continue;
            };
            let categoria = match categoria {
                "rastreador" => "rastreador",
                "publicidad" => "publicidad",
                "telemetria" => "telemetria",
                _ => continue,
            };
            let (host, ruta) = direccion.split_once('/').unwrap_or((direccion, ""));
            let (host, bajo) = host
                .strip_prefix("*.")
                .map_or((host, false), |resto| (resto, true));
            out.push(Pixel {
                host: normaliza_host(host),
                bajo,
                ruta: ruta
                    .split('/')
                    .filter(|t| !t.is_empty())
                    .map(str::to_string)
                    .collect(),
                empresa,
                pais: Some(pais).filter(|p| !p.is_empty()),
                categoria,
            });
        }
        out
    })
}

/// Whether `host` is a name under `sufijo` (`www.facebook.com` under `facebook.com`), without
/// building any string: this runs for every request.
fn debajo(host: &str, sufijo: &str) -> bool {
    host.strip_suffix(sufijo)
        .is_some_and(|r| r.len() > 1 && r.ends_with('.'))
}

/// The pixel of our own list that `host` and `ruta` (the address's path) are, if any.
fn pixel(host: &str, ruta: &str) -> Option<&'static Pixel> {
    pixeles().iter().find(|p| {
        let nombre = if p.bajo {
            debajo(host, &p.host)
        } else {
            host == p.host
        };
        let mut tramos = ruta.split('/').filter(|t| !t.is_empty());
        nombre
            && p.ruta.iter().all(|a| {
                tramos
                    .next()
                    .is_some_and(|b| a == "*" || a.eq_ignore_ascii_case(b))
            })
    })
}

/// Names on a shared service that belong to whoever rents them, besides those in the private
/// section of the Public Suffix List: a bucket of Cloudflare R2.
const CUENTAS: &[&str] = &["r2.cloudflarestorage.com"];

/// Whether `host`, which is not under a private suffix of the list, or its path, is still a
/// customer's account on a shared service: anyone can have one, so it is someone's, never «the
/// road» (review of 10 Oct 2026, grave 4).
fn de_cliente(host: &str, ruta: &str) -> bool {
    CUENTAS.iter().any(|c| debajo(host, c))
        // `cdn.jsdelivr.net/gh/<anyone>/<repo>`: a GitHub repository of whoever made it.
        || (host == "cdn.jsdelivr.net" && (ruta == "/gh" || ruta.starts_with("/gh/")))
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
    /// The address is a known tracking pixel by its path (`data/pixeles.txt`): the category and
    /// the company say so even when the name alone does not.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub pixel: bool,
    /// The name is a customer's account on a shared service (`d1.cloudfront.net`, a bucket):
    /// whoever rents it, not the service, receives what goes there.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub de_cliente: bool,
    /// The name sits under a suffix of the list's private section (worked out once per
    /// request: [`es_tercero_de`] needs it too).
    #[serde(skip)]
    pub privado: bool,
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
    clasifica_en(host, "/")
}

/// Everything the lists say about the host of `direccion`, and what its path adds: a known
/// pixel (`www.facebook.com/tr` is Meta's tracker even though `www.facebook.com` is also
/// Facebook) and a customer's repository on jsDelivr.
#[must_use]
pub fn clasifica_url(direccion: &str) -> Destino {
    let u = url::Url::parse(direccion).ok();
    let host = u.as_ref().and_then(url::Url::host_str).unwrap_or_default();
    clasifica_en(host, u.as_ref().map_or("", url::Url::path))
}

/// The same for an address the caller already read: its host and its path. The browser reads
/// each request's address once and hands the parts here.
#[must_use]
pub fn clasifica_en(host: &str, ruta: &str) -> Destino {
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
    let privado = bajo_sufijo_privado(&host);
    let mut d = Destino {
        sitio: sitio(&host),
        categoria,
        empresa: guardiana_lists::company_of(&host),
        pais: guardiana_lists::country_of(&host),
        corredor,
        ia: guardiana_lists::ai_service_of(&host),
        reparto: guardiana_lists::delivery_of(&host),
        nube: guardiana_lists::hosting_of(&host),
        pixel: false,
        de_cliente: privado || de_cliente(&host, ruta),
        privado,
        host,
    };
    if let Some(p) = pixel(&d.host, ruta) {
        // The lists' own word stays when they already call the name a tracker or advertising.
        if !matches!(d.categoria, "rastreador" | "publicidad") {
            d.categoria = p.categoria;
        }
        d.empresa = Some(p.empresa);
        if d.pais.is_none() {
            d.pais = p.pais;
        }
        d.pixel = true;
    }
    d
}

/// Whether a request to `host` from a page of `sitio_pagina` goes to someone else: another site
/// that is not owned by the same company. `gstatic.com` on `google.com` is Google talking to
/// itself; `doubleclick.net` on `eltiempo.com` is a third party.
#[must_use]
pub fn es_tercero(host: &str, sitio_pagina: &str) -> bool {
    let s = sitio(host);
    tercero(&s, bajo_sufijo_privado(&s), sitio_pagina)
}

/// [`es_tercero`] for a destination already classified (what it knows is not worked out again).
#[must_use]
pub fn es_tercero_de(d: &Destino, sitio_pagina: &str) -> bool {
    tercero(&d.sitio, d.privado, sitio_pagina)
}

fn tercero(s: &str, privado: bool, sitio_pagina: &str) -> bool {
    if s == sitio_pagina {
        return false;
    }
    // `ana.github.io` and `luis.github.io` are both GitHub's, but not each other's.
    if privado || bajo_sufijo_privado(sitio_pagina) {
        return true;
    }
    match (
        guardiana_lists::company_of(s),
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
    fn known_pixels_are_told_by_their_path() {
        let px = |u: &str| {
            let d = clasifica_url(u);
            (d.categoria, d.quien().to_string())
        };
        assert_eq!(
            px("https://www.facebook.com/tr?id=1&ev=Purchase"),
            ("rastreador", "Meta".into())
        );
        assert_eq!(
            px("https://www.facebook.com/tr/"),
            ("rastreador", "Meta".into())
        );
        assert_eq!(px("https://facebook.com/tr"), ("rastreador", "Meta".into()));
        assert_eq!(
            px("https://connect.facebook.net/en_US/fbevents.js"),
            ("rastreador", "Meta".into())
        );
        assert_eq!(
            px("https://connect.facebook.net/signals/config/123?v=2"),
            ("rastreador", "Meta".into())
        );
        assert_eq!(
            px("https://www.google.com/pagead/1p-conversion/123/?value=1"),
            ("publicidad", "Google".into())
        );
        assert_eq!(
            px("https://www.google.com/ccm/collect?en=page_view"),
            ("publicidad", "Google".into())
        );
        assert_eq!(
            px("https://static.ads-twitter.com/uwt.js"),
            ("publicidad", "X".into())
        );
        assert_eq!(
            px("https://analytics.twitter.com/i/adsct?txn_id=1"),
            ("publicidad", "X".into())
        );
        assert_eq!(
            px("https://snap.licdn.com/li.lms-analytics/insight.min.js"),
            ("rastreador", "LinkedIn".into())
        );
        assert_eq!(
            px("https://api.segment.io/v1/t"),
            ("rastreador", "Segment".into())
        );
        assert_eq!(
            px("https://cdn.mxpnl.com/libs/mixpanel-2-latest.min.js"),
            ("rastreador", "Mixpanel".into())
        );
        assert_eq!(
            px("https://o4505.ingest.sentry.io/api/1/envelope/"),
            ("telemetria", "Sentry".into())
        );
        assert_eq!(
            px("https://o4505.ingest.us.sentry.io/api/1/envelope/"),
            ("telemetria", "Sentry".into())
        );
        // What else those names do is not a pixel: the page, the login, «I am not a robot».
        assert_eq!(px("https://www.facebook.com/tracking").0, "desconocido");
        assert_eq!(
            px("https://www.facebook.com/plugins/like.php").0,
            "desconocido"
        );
        assert_eq!(
            px("https://connect.facebook.net/en_US/sdk.js").0,
            "desconocido"
        );
        assert_eq!(
            px("https://www.google.com/recaptcha/api.js").0,
            "desconocido"
        );
        // All of `/pagead`, on google.com and on its country names; never reCAPTCHA.
        for u in [
            "https://www.google.com/pagead/form-data/1?em=x",
            "https://www.google.com.co/pagead/1p-user-list/1/?x=1",
            "https://www.google.com.mx/pagead/1p-conversion/1/",
            "https://www.google.co.uk/pagead/landing",
            "https://google.es/pagead/1p-user-list/1/",
        ] {
            assert_eq!(px(u), ("publicidad", "Google".into()), "{u}");
        }
        for u in [
            "https://www.google.com.co/recaptcha/api2/anchor",
            "https://www.google.com/recaptcha/enterprise.js",
            "https://www.google.com.br/search?q=x",
        ] {
            assert!(!clasifica_url(u).pixel, "{u}");
        }
        assert_eq!(
            px("https://www.facebook.com/privacy_sandbox/pixel/register/trigger/?id=1"),
            ("rastreador", "Meta".into())
        );
        assert!(!clasifica_url("https://sentry.io/").pixel);
        assert!(!clasifica_url("https://ingest.sentry.io/").pixel);
    }

    #[test]
    fn rented_accounts_are_nobody_s_road() {
        for h in [
            "https://d1234.cloudfront.net/p.gif",
            "https://atacante.r2.cloudflarestorage.com/p.png",
            "https://cdn.jsdelivr.net/gh/atacante/x@main/a.js",
            "https://raw.githubusercontent.com/atacante/x/main/a.js",
            "https://cubo.s3.amazonaws.com/a.png",
            "https://storage.googleapis.com/cubo/a.png",
            "https://cuenta.blob.core.windows.net/a/b.png",
            "https://x.atacante.workers.dev/",
            "https://atacante.pages.dev/",
            "https://atacante.netlify.app/",
            "https://atacante.vercel.app/",
            "https://ninja.akamaized.net/a.js",
        ] {
            assert!(clasifica_url(h).de_cliente, "{h}");
        }
        for h in [
            "https://cdn.jsdelivr.net/npm/vue@3/dist/vue.js",
            "https://fonts.gstatic.com/a.woff2",
            "https://r1---sn-x.gvt1.com/a",
        ] {
            assert!(!clasifica_url(h).de_cliente, "{h}");
        }
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
