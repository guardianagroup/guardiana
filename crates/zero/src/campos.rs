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

/// The names of the fields of a request's body, never their values: the keys of a JSON
/// object (one level down too, as `user.email`), or the names of a form (`a=1&b=2`). Anything
/// else (a file, text, a binary) gives nothing. At most [`MAX_CAMPOS`], each once.
#[must_use]
pub fn nombres_cuerpo(cuerpo: &[u8]) -> Vec<String> {
    let Ok(texto) = std::str::from_utf8(cuerpo) else {
        return Vec::new();
    };
    let texto = texto.trim_start();
    let mut vistos = BTreeSet::new();
    let mut out = Vec::new();
    let mut mete = |k: String| {
        let k: String = k
            .trim()
            .to_ascii_lowercase()
            .chars()
            .take(MAX_LARGO)
            .collect();
        if !k.is_empty() && out.len() < MAX_CAMPOS && vistos.insert(k.clone()) {
            out.push(k);
        }
    };
    if texto.starts_with('{') || texto.starts_with('[') {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(texto) {
            claves_json(&v, "", 0, &mut mete);
        }
        return out;
    }
    if texto.starts_with('<') || texto.contains('\0') {
        return Vec::new();
    }
    // A form: `name=value&other=value`, or one `name=value` line.
    if texto.contains('=') {
        for par in texto.split('&').take(200) {
            if let Some((k, _)) = par.split_once('=') {
                mete(percent_decode(k));
            }
        }
    }
    out
}

fn claves_json(v: &serde_json::Value, prefijo: &str, nivel: u8, mete: &mut impl FnMut(String)) {
    match v {
        serde_json::Value::Object(m) => {
            for (k, hijo) in m {
                let nombre = if prefijo.is_empty() {
                    k.clone()
                } else {
                    format!("{prefijo}.{k}")
                };
                mete(nombre.clone());
                if nivel < 1 {
                    claves_json(hijo, &nombre, nivel + 1, mete);
                }
            }
        }
        serde_json::Value::Array(a) => {
            // A list of events: the keys of the first one say what each carries.
            if let Some(primero) = a.first() {
                claves_json(primero, prefijo, nivel, mete);
            }
        }
        _ => {}
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(h) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(h);
                i += 3;
                continue;
            }
        }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The names of the cookies a request carries (its `Cookie` header), never their values.
#[must_use]
pub fn nombres_cookies(cabecera: &str) -> Vec<String> {
    let mut vistos = BTreeSet::new();
    let mut out = Vec::new();
    for par in cabecera.split(';') {
        let k = par.split('=').next().unwrap_or("").trim();
        let k: String = k.chars().take(MAX_LARGO).collect();
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

/// What a cookie is, by its name, when the trade documents it. The cookies a tracker reads
/// back are how it knows you from one site to the next.
#[must_use]
pub fn lee_cookie(nombre: &str) -> Option<Lectura> {
    let l = |clave: &'static str, fuente: Option<&'static str>| Some(Lectura { clave, fuente });
    match nombre {
        "_ga" | "_gid" | "_gat" | "_gcl_au" | "_gcl_aw" | "__utma" | "__utmb" | "__utmc"
        | "__utmz" => l("cookie_google_analytics", GOOGLE),
        "IDE" | "DSID" | "__gads" | "__gpi" | "test_cookie" | "NID" | "ANID" | "1P_JAR" => {
            l("cookie_google_ads", GOOGLE)
        }
        "_fbp" | "_fbc" | "fr" | "datr" | "sb" | "c_user" | "xs" => l("cookie_meta", META),
        "_tt_enable_cookie" | "_ttp" | "ttwid" => l("cookie_tiktok", None),
        "_hjid" | "_hjSessionUser_" | "_hjSession_" | "_hjFirstSeen" => l("cookie_hotjar", None),
        "_pin_unauth" | "_pinterest_ct_ua" => l("cookie_pinterest", None),
        "li_sugr" | "bcookie" | "lidc" | "UserMatchHistory" | "AnalyticsSyncHistory" => {
            l("cookie_linkedin", None)
        }
        "uuid2" | "anj" | "sess" | "usersync" | "icu" => l("cookie_appnexus", None),
        "tuuid" | "tuuid_lu" | "c" | "ssi" => l("cookie_adtech", None),
        "_uetsid" | "_uetvid" | "MUID" | "MR" => l("cookie_microsoft_ads", None),
        "criteo_write_test" | "uid" | "cto_bundle" | "cto_bidid" => l("cookie_criteo", None),
        "sessionid" | "session_id" | "PHPSESSID" | "JSESSIONID" | "ASP.NET_SessionId"
        | "connect.sid" | "_session_id" => l("cookie_sesion", None),
        "euconsent-v2" | "eupubconsent-v2" | "OptanonConsent" | "CookieConsent"
        | "cookieyes-consent" | "didomi_token" | "usprivacy" => l("cookie_consentimiento", None),
        _ => {
            let n = nombre.to_ascii_lowercase();
            if n.contains("uuid") || n.contains("_id") || n.ends_with("id") && n.len() <= 12 {
                l("cookie_id", None)
            } else {
                None
            }
        }
    }
}

/// What a request's cookies say, each meaning once, and how many names stay unread.
#[must_use]
pub fn resumen_cookies(cookies: &[String]) -> (Vec<Lectura>, usize) {
    let mut vistos = BTreeSet::new();
    let mut sin = 0;
    for c in cookies {
        match lee_cookie(c) {
            Some(l) => {
                vistos.insert(l);
            }
            None => sin += 1,
        }
    }
    (vistos.into_iter().collect(), sin)
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
        "email" | "em" | "mail" | "e_mail" | "user.email" | "email_address" | "user_email"
        | "customer.email" | "hashed_email" | "em_hash" | "email_sha256" => l("campo_correo", None),
        "name" | "first_name" | "last_name" | "firstname" | "lastname" | "full_name"
        | "user.name" | "customer.name" | "fn" | "ln" => l("campo_nombre", None),
        "address" | "street" | "address1" | "zip" | "zipcode" | "postal_code" | "postcode"
        | "ct" | "st" | "zp" | "user.address" => l("campo_direccion", None),
        "birthday" | "birthdate" | "dob" | "date_of_birth" | "db" | "age" | "gender" | "ge"
        | "sex" => l("campo_persona", None),
        "items" | "products" | "product_id" | "sku" | "content_ids" | "contents" | "cart"
        | "line_items" | "order_id" | "transaction_id" | "value" | "currency" | "price"
        | "total" | "revenue" => l("campo_compra", None),
        "query" | "q" | "search" | "search_term" | "keyword" | "keywords" => {
            l("campo_busqueda", None)
        }
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
    let mut vistos: Vec<Lectura> = Vec::new();
    let mut sin = 0;
    for c in campos {
        match lee(host, c) {
            Some(l) => match vistos.iter_mut().find(|v| v.clave == l.clave) {
                // One meaning once; a documented reading beats a conventional one.
                Some(v) => {
                    if v.fuente.is_none() {
                        v.fuente = l.fuente;
                    }
                }
                None => vistos.push(l),
            },
            None => sin += 1,
        }
    }
    vistos.sort();
    (vistos, sin)
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
    fn body_keys_and_cookie_names_are_read_without_values() {
        let j = br#"{"event":"purchase","user":{"email":"ana@secreto.co","name":"Ana"},"items":[{"sku":"X1","price":9}],"value":9}"#;
        let n = nombres_cuerpo(j);
        assert!(n.contains(&"user.email".into()) && n.contains(&"items".into()));
        assert!(!n.iter().any(|k| k.contains("secreto") || k.contains("Ana")));
        let f = nombres_cuerpo(b"email=ana%40secreto.co&phone=123&gdpr_consent=abc");
        assert_eq!(f, vec!["email", "phone", "gdpr_consent"]);
        assert!(nombres_cuerpo(b"\x89PNG\0binary").is_empty());
        assert!(nombres_cuerpo(b"just some text").is_empty());
        let c = nombres_cookies("_ga=GA1.2.3; _fbp=fb.1.2; PHPSESSID=abc; weird=1");
        assert_eq!(c, vec!["_ga", "_fbp", "PHPSESSID", "weird"]);
        let (que, sin) = resumen_cookies(&c);
        let claves: Vec<&str> = que.iter().map(|l| l.clave).collect();
        assert!(claves.contains(&"cookie_google_analytics") && claves.contains(&"cookie_meta"));
        assert!(claves.contains(&"cookie_sesion"));
        assert_eq!(sin, 1);
        let (que, _) = resumen("collect.otra-empresa.io", &nombres_cuerpo(j));
        let claves: Vec<&str> = que.iter().map(|l| l.clave).collect();
        assert!(claves.contains(&"campo_correo") && claves.contains(&"campo_compra"));
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
