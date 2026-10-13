//! What a request carries, read from the *names* of its fields.
//!
//! The browser never keeps the values of a request's parameters: they are the data. But their
//! names are not, and many are published by the companies themselves: Google documents that
//! `dl` is the page's address and `sr` the screen size; Meta documents `ud[em]` as a hashed
//! e-mail. So a cut can say «it carried the page you were on, an identifier of yours and your
//! screen size» without reading anything personal (the owner, 13 Oct 2026: «intenta dar algún
//! dato más»). A name the tables do not know is counted, never guessed.

use std::collections::BTreeSet;

/// How many names a cut keeps, and how long each may be.
pub const MAX_CAMPOS: usize = 24;
const MAX_LARGO: usize = 40;

/// What a field means, as a text key of the interface (`campo_pagina`…).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Lectura {
    /// The interface text: `campo_pagina`, `campo_id`…
    pub clave: &'static str,
    /// Who published the meaning, as an interface text key (`fuente_google_analytics`), or
    /// `None` when it is only the usual convention for that name.
    pub fuente: Option<&'static str>,
}

/// The names of the parameters of `url`, lower-cased, each once, at most [`MAX_CAMPOS`];
/// never their values. A form's fields in a POST body are not read.
#[must_use]
pub fn nombres(url: &str) -> Vec<String> {
    let Ok(u) = url::Url::parse(url) else {
        return Vec::new();
    };
    let mut vistos = BTreeSet::new();
    let mut out = Vec::new();
    for (k, _) in u.query_pairs() {
        let k: String = k
            .trim()
            .to_ascii_lowercase()
            .chars()
            .take(MAX_LARGO)
            .collect();
        if k.is_empty() || !vistos.insert(k.clone()) {
            continue;
        }
        out.push(k);
        if out.len() >= MAX_CAMPOS {
            break;
        }
    }
    out
}

const GOOGLE: Option<&str> = Some("fuente_google_analytics");
const META: Option<&str> = Some("fuente_meta_pixel");

/// What a name means on `host`, when it is documented or plain.
#[must_use]
pub fn lee(host: &str, nombre: &str) -> Option<Lectura> {
    let h = host.trim_end_matches('.').to_ascii_lowercase();
    let google = h.ends_with("google-analytics.com")
        || h.ends_with("analytics.google.com")
        || h.ends_with("googletagmanager.com")
        || h.ends_with("doubleclick.net")
        || h.ends_with(".google.com");
    let meta = h.ends_with("facebook.com") || h.ends_with("facebook.net");
    let l = |clave: &'static str, fuente: Option<&'static str>| Some(Lectura { clave, fuente });
    // Google's Measurement Protocol (Universal Analytics and GA4) names, as Google documents
    // them: https://developers.google.com/analytics/devguides/collection/protocol/v1/parameters
    if google {
        match nombre {
            "dl" | "dp" => return l("campo_pagina", GOOGLE),
            "dr" => return l("campo_referente", GOOGLE),
            "dt" => return l("campo_titulo", GOOGLE),
            "cid" | "uid" | "_gid" => return l("campo_id", GOOGLE),
            "sid" | "_s" => return l("campo_sesion", GOOGLE),
            "sr" | "vp" | "sd" => return l("campo_pantalla", GOOGLE),
            "ul" => return l("campo_idioma", GOOGLE),
            "ua" | "uap" | "uamb" | "uam" | "uafvl" => return l("campo_navegador", GOOGLE),
            "tid" => return l("campo_cuenta", GOOGLE),
            "t" | "en" | "ec" | "ea" | "el" | "ev" => return l("campo_evento", GOOGLE),
            "gtm" | "v" | "_p" | "z" | "cb" | "_r" => return l("campo_tecnico", GOOGLE),
            "aip" => return l("campo_ip_anonima", GOOGLE),
            "uip" => return l("campo_ip", GOOGLE),
            "geoid" => return l("campo_lugar", GOOGLE),
            _ => {}
        }
    }
    // Meta's pixel, as Meta documents it: https://developers.facebook.com/docs/meta-pixel/reference
    if meta {
        match nombre {
            "dl" => return l("campo_pagina", META),
            "rl" => return l("campo_referente", META),
            "id" => return l("campo_cuenta", META),
            "ev" => return l("campo_evento", META),
            "sw" | "sh" => return l("campo_pantalla", META),
            "fbp" | "fbc" => return l("campo_id", META),
            "ud[em]" => return l("campo_correo_cifrado", META),
            "ud[ph]" => return l("campo_telefono_cifrado", META),
            "ud[fn]" | "ud[ln]" | "ud[ge]" | "ud[db]" | "ud[ct]" | "ud[st]" | "ud[zp]"
            | "ud[country]" | "ud[external_id]" => return l("campo_persona_cifrada", META),
            "ts" | "it" => return l("campo_hora", META),
            "v" | "r" | "if" | "o" | "coo" | "ec" | "tm" | "es" | "eid" => {
                return l("campo_tecnico", META)
            }
            _ => {}
        }
    }
    // The names most of the trade shares, by convention (no single publisher):
    match nombre {
        "gdpr" | "gdpr_consent" | "gdpr_pd" | "us_privacy" | "gpp" | "gpp_sid" | "tcf"
        | "consent" | "tcstring" => l("campo_consentimiento", None),
        "url" | "u" | "page" | "page_url" | "pageurl" | "href" | "loc" | "location" | "p" => {
            l("campo_pagina", None)
        }
        "ref" | "referrer" | "referer" | "r" => l("campo_referente", None),
        "uid" | "uuid" | "user_id" | "userid" | "puid" | "cuid" | "id" | "guid" | "visitor"
        | "visitor_id" | "vid" | "did" | "device_id" | "client_id" | "clientid" | "cid"
        | "d_mid" | "d_cid" | "partner_uid" | "external_id" => l("campo_id", None),
        "sid" | "session" | "session_id" | "sessionid" => l("campo_sesion", None),
        "sr" | "res" | "screen" | "sw" | "sh" | "vp" | "viewport" => l("campo_pantalla", None),
        "lang" | "language" | "locale" | "hl" => l("campo_idioma", None),
        "ua" | "user_agent" | "useragent" | "browser" => l("campo_navegador", None),
        "ts" | "t" | "time" | "timestamp" | "tz" | "tzo" => l("campo_hora", None),
        "ev" | "event" | "e" | "ea" | "action" => l("campo_evento", None),
        "email" | "em" | "mail" | "e_mail" => l("campo_correo", None),
        "phone" | "ph" | "tel" => l("campo_telefono", None),
        "ip" | "ipaddress" | "ip_address" => l("campo_ip", None),
        "lat" | "lon" | "lng" | "geo" | "country" | "city" => l("campo_lugar", None),
        "cb" | "rnd" | "random" | "nocache" | "_" | "v" | "ver" | "version" | "callback"
        | "format" | "cachebuster" => l("campo_tecnico", None),
        _ => None,
    }
}

/// What a cut carried, as the distinct meanings of its names (each once, in a fixed order),
/// and how many names stay unread.
#[must_use]
pub fn resumen(host: &str, campos: &[String]) -> (Vec<Lectura>, usize) {
    let mut vistos = BTreeSet::new();
    let mut sin = 0;
    for c in campos {
        match lee(host, c) {
            Some(l) => {
                vistos.insert(l);
            }
            None => sin += 1,
        }
    }
    (vistos.into_iter().collect(), sin)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn names_are_kept_without_their_values() {
        let n = nombres("https://www.google-analytics.com/g/collect?v=2&tid=G-1&cid=55.66&dl=https%3A%2F%2Feltiempo.com%2Fx&sr=1920x1080&ul=es-co&CID=again");
        assert_eq!(n, vec!["v", "tid", "cid", "dl", "sr", "ul"]);
        assert!(nombres("nada").is_empty());
        let muchos: String = (0..40).map(|i| format!("k{i}=1&")).collect();
        assert_eq!(
            nombres(&format!("https://a.example/?{muchos}")).len(),
            MAX_CAMPOS
        );
    }

    #[test]
    fn google_and_meta_names_read_as_documented_and_the_rest_by_convention() {
        let (que, sin) = resumen(
            "www.google-analytics.com",
            &["v", "tid", "cid", "dl", "sr", "ul", "zzz"].map(String::from),
        );
        let claves: Vec<&str> = que.iter().map(|l| l.clave).collect();
        assert!(claves.contains(&"campo_pagina") && claves.contains(&"campo_id"));
        assert!(claves.contains(&"campo_pantalla") && claves.contains(&"campo_idioma"));
        assert!(que.iter().all(|l| l.fuente == GOOGLE));
        assert_eq!(sin, 1);
        let (que, _) = resumen(
            "www.facebook.com",
            &["id", "ev", "dl", "ud[em]"].map(String::from),
        );
        let claves: Vec<&str> = que.iter().map(|l| l.clave).collect();
        assert!(claves.contains(&"campo_correo_cifrado") && claves.contains(&"campo_cuenta"));
        // Elsewhere, `id` is an identifier by convention and carries no publisher.
        let l = lee("sync.richaudience.com", "id").unwrap();
        assert_eq!((l.clave, l.fuente), ("campo_id", None));
        assert_eq!(
            lee("sync.richaudience.com", "gdpr_consent").unwrap().clave,
            "campo_consentimiento"
        );
        assert!(lee("sync.richaudience.com", "qwerty").is_none());
    }
}
