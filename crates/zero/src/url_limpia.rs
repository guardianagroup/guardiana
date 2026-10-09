//! Links that carry a tracking tag (`?utm_source=…&fbclid=…`) tell the page you open who sent
//! you and, with `fbclid` or `gclid`, which click of which person it was. They are not needed to
//! open the page: GUARDIANA ZERO opens the same address without them, and counts each one.

/// Parameters that only identify a campaign or a click. Kept short and explained: each one is
/// documented by the company that adds it as an ad or analytics identifier.
const PARAMETROS: &[&str] = &[
    // Google Analytics and Ads
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_term",
    "utm_content",
    "utm_id",
    "utm_source_platform",
    "utm_creative_format",
    "utm_marketing_tactic",
    "gclid",
    "gclsrc",
    "dclid",
    "gbraid",
    "wbraid",
    "_ga",
    "_gl",
    // Meta (Facebook, Instagram)
    "fbclid",
    "igshid",
    "igsh",
    // Microsoft, X, TikTok, LinkedIn, Yandex
    "msclkid",
    "twclid",
    "ttclid",
    "li_fat_id",
    "yclid",
    // Mailchimp, HubSpot, Marketo, Oracle Eloqua, Vero, Olytics, Pinterest
    "mc_cid",
    "mc_eid",
    "_hsenc",
    "_hsmi",
    "__hssc",
    "__hstc",
    "__hsfp",
    "mkt_tok",
    "elqtrackid",
    "vero_id",
    "vero_conv",
    "oly_anon_id",
    "oly_enc_id",
    "epik",
];

/// The address without its tracking parameters, and how many were taken out; `None` when it
/// had none (or is not an address with a query).
#[must_use]
pub fn limpia(direccion: &str) -> Option<(String, usize)> {
    let mut u = url::Url::parse(direccion).ok()?;
    if !matches!(u.scheme(), "http" | "https") {
        return None;
    }
    u.query()?;
    let pares: Vec<(String, String)> = u
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    let quedan: Vec<&(String, String)> = pares
        .iter()
        .filter(|(k, _)| !PARAMETROS.contains(&k.to_ascii_lowercase().as_str()))
        .collect();
    let quitados = pares.len() - quedan.len();
    if quitados == 0 {
        return None;
    }
    if quedan.is_empty() {
        u.set_query(None);
    } else {
        u.query_pairs_mut()
            .clear()
            .extend_pairs(quedan.iter().map(|(k, v)| (k.as_str(), v.as_str())));
    }
    Some((u.to_string(), quitados))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracking_tags_go_and_the_rest_stays() {
        assert_eq!(
            limpia("https://tienda.co/p?id=7&utm_source=fb&fbclid=AbC123"),
            Some(("https://tienda.co/p?id=7".to_string(), 2))
        );
        assert_eq!(
            limpia("https://x.com/a?gclid=1"),
            Some(("https://x.com/a".to_string(), 1))
        );
        assert_eq!(limpia("https://x.com/a?q=utm_source"), None);
        assert_eq!(limpia("https://x.com/a"), None);
        assert_eq!(limpia("about:blank"), None);
    }

    #[test]
    fn the_fragment_survives() {
        assert_eq!(
            limpia("https://x.com/a?utm_medium=mail#seccion"),
            Some(("https://x.com/a#seccion".to_string(), 1))
        );
    }
}
