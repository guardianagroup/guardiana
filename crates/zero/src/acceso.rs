//! The services that check that a visitor is a person before letting them in: captchas and anti-fraud checks
//! (reCAPTCHA, hCaptcha, Cloudflare Turnstile, DataDome, HUMAN, Arkose Labs, GeeTest, Friendly Captcha, MTCaptcha).
//!
//! The owner, 2 Nov 2026, 07:57: «nos detectan enseguida que somos un robot… de esta forma no está funcionando
//! correctamente». Two things of ZERO's own made a person look like a robot to these services: the lists cut some of
//! their requests, and the fingerprint noise meant for other companies' scripts reached theirs too, so the values they
//! read did not add up. Here they get the browser exactly as it is: never cut by the lists and never given noise. That
//! does not disguise anything; it stops confusing the check. What the person cut by hand, their mandate and their
//! marked data still hold, as everywhere.
//!
//! Only these companies' own names, and reCAPTCHA by its path on Google's names (the rest of `google.com` is judged
//! as always). The same list lives in the page script (`paginas.rs`, `ACCESO`), and a test keeps both equal.

/// The registrable names (and their subdomains) of the services, each one only an access check.
pub const SITIOS: &[&str] = &[
    "recaptcha.net",
    "hcaptcha.com",
    "challenges.cloudflare.com",
    "datadome.co",
    "captcha-delivery.com",
    "px-cloud.net",
    "px-cdn.net",
    "px-client.net",
    "perimeterx.net",
    "arkoselabs.com",
    "funcaptcha.com",
    "geetest.com",
    "friendlycaptcha.com",
    "frcapi.com",
    "mtcaptcha.com",
];

/// Whether a request to `host` with `ruta` is one of these checks.
#[must_use]
pub fn es_verificacion(host: &str, ruta: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    let bajo = |s: &str| host == s || host.strip_suffix(s).is_some_and(|r| r.ends_with('.'));
    if SITIOS.iter().any(|s| bajo(s)) {
        return true;
    }
    // reCAPTCHA on Google's own names: only its path, never the rest of google.com or gstatic.com.
    let google = host == "google.com"
        || host == "www.google.com"
        || host.starts_with("www.google.")
        || host == "gstatic.com"
        || host == "www.gstatic.com";
    google && (ruta.starts_with("/recaptcha/") || ruta == "/recaptcha")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solo_las_verificaciones() {
        for (h, r) in [
            ("www.google.com", "/recaptcha/api.js"),
            ("www.google.com.co", "/recaptcha/api2/anchor"),
            ("www.gstatic.com", "/recaptcha/releases/x/recaptcha__es.js"),
            ("www.recaptcha.net", "/recaptcha/api.js"),
            ("js.hcaptcha.com", "/1/api.js"),
            ("challenges.cloudflare.com", "/turnstile/v0/api.js"),
            ("js.datadome.co", "/tags.js"),
            ("geo.captcha-delivery.com", "/captcha/"),
            ("client.px-cloud.net", "/PX123/main.min.js"),
            ("client-api.arkoselabs.com", "/v2/api.js"),
            ("static.geetest.com", "/v4/gt4.js"),
        ] {
            assert!(es_verificacion(h, r), "{h}{r}");
        }
        for (h, r) in [
            ("www.google.com", "/pagead/1p-user-list/1"),
            ("www.google-analytics.com", "/g/collect"),
            ("stats.g.doubleclick.net", "/g/collect"),
            ("www.gstatic.com", "/firebasejs/9/firebase-app.js"),
            ("notdatadome.co", "/tags.js"),
            ("datadome.co.evil.com", "/tags.js"),
            ("cloudflare.com", "/cdn-cgi/trace"),
        ] {
            assert!(!es_verificacion(h, r), "{h}{r}");
        }
    }

    #[test]
    fn la_misma_lista_en_el_guion_de_las_paginas() {
        let guion = crate::paginas::guion_de_pruebas();
        for s in SITIOS {
            assert!(
                guion.contains(&format!("'{s}'")),
                "{s} falta en ACCESO de paginas.rs"
            );
        }
    }
}
