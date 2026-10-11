//! The one place that decides whether a request may leave. The shell asks for every request of
//! every tab, before the engine sends it, and does what the answer says; the answer also says
//! why, so the person sees it in real time («cortado: rastreador de Google»).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::destino::{clasifica_url, es_tercero_de, Destino};
use crate::mandato::{Mandato, Recurso};
use crate::tinta::{busca, Forma, Hallazgo};

/// The person's choices that decide requests.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ajustes {
    /// Cut third parties that the open lists know as trackers or advertising, and registered
    /// data brokers. Asked on first run; off until the person says yes.
    pub cortar_seguimiento: bool,
    /// Sites the person cut by hand, everywhere.
    pub cortados: BTreeSet<String>,
    /// Sites the person let through by hand even if a list knows them (an «undo»).
    pub permitidos: BTreeSet<String>,
    /// «Protección máxima» (the owner, 10 Oct 2026): on top of the above, also cut the
    /// telemetry the lists know at other companies, and every ping or beacon to another company
    /// (a request whose only job is to carry information out). Only with `cortar_seguimiento`.
    #[serde(default)]
    pub maxima: bool,
}

/// One request the engine is about to send.
#[derive(Debug, Clone)]
pub struct Peticion<'a> {
    /// Full address.
    pub url: &'a str,
    /// What is sent with it (empty for most).
    pub cuerpo: &'a [u8],
    /// What kind of resource.
    pub recurso: Recurso,
    /// The registrable site of the page the tab shows (for a top-level navigation, the site
    /// being opened).
    pub sitio_pagina: &'a str,
}

/// Why a request was cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Motivo {
    /// A tracker in the open lists.
    Rastreador,
    /// Advertising in the open lists.
    Publicidad,
    /// A company registered as a data broker.
    Corredor,
    /// A site the person cut.
    CorteTuyo,
    /// Outside the mandate of this tab.
    FueraDeMandato,
    /// A marked value of the person leaving to a third party.
    Tinta,
    /// The mandate's decoy leaving: proof that something tried to take data out.
    Senuelo,
    /// Telemetry at another company (maximum protection).
    Telemetria,
    /// A ping or beacon to another company: it only carries information out (maximum
    /// protection).
    Baliza,
}

/// The answer for one request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Decision {
    /// Cut it (the shell answers with an empty 403 and the request never leaves).
    pub cortar: bool,
    /// Why, when cut.
    pub motivo: Option<Motivo>,
    /// Who is behind it.
    pub destino: Destino,
    /// Another owner than the page's.
    pub tercero: bool,
    /// The request goes to another site than the page's (delivery networks included).
    pub otro: bool,
    /// Marked values found in it (also when allowed: to the page's own site, they go in the
    /// person's data book).
    pub hallazgos: Vec<Hallazgo>,
    /// What the open lists (and our list of pixels) would cut it for, whatever the person's
    /// switches and rules say now: `Rastreador`, `Publicidad` or `Corredor` with the protection
    /// on, `Telemetria` or `Baliza` only with maximum protection. `None` for the page's own
    /// site. The shield derives from it what holds for a site when a switch changes.
    pub lista: Option<Motivo>,
    /// It is the tab's own page (a top-level navigation), not something the page asked for.
    pub pagina: bool,
}

impl Decision {
    /// Whether it counts as «a company from outside» on the shield: another party, or another
    /// site that was cut. The page's own site never does, even when a part of it is cut, except
    /// when it takes a mandate's decoy: the decoy belongs to nobody, so whoever takes it is
    /// shown (and logged) like any outsider, and «datos detenidos» always matches the cut list.
    #[must_use]
    pub const fn de_fuera(&self) -> bool {
        self.tercero || (self.cortar && (self.otro || matches!(self.motivo, Some(Motivo::Senuelo))))
    }

    /// Whether it is written down in the day's figures and in «Lo que se cortó»: what counts as
    /// «from outside», and also a whole page that was cut (a site the person cut, a step outside
    /// the mandate), which otherwise vanished from both (review of 10 Oct 2026, media 10).
    #[must_use]
    pub const fn anotable(&self) -> bool {
        self.de_fuera() || (self.cortar && self.pagina)
    }
}

/// Decide one request. `senuelo` is the index, in `tinta`, of the current mandate's decoy.
#[must_use]
pub fn decide(
    p: &Peticion<'_>,
    ajustes: &Ajustes,
    tinta: &[Forma],
    senuelo: Option<usize>,
    mandato: Option<&Mandato>,
) -> Decision {
    // The name, and what its path adds: `www.facebook.com/tr` is Meta's pixel (grave 1).
    decide_con(p, clasifica_url(p.url), ajustes, tinta, senuelo, mandato)
}

/// [`decide`] for a request whose destination the caller already classified
/// ([`crate::destino::clasifica_en`]): the browser reads each address once.
#[must_use]
pub fn decide_con(
    p: &Peticion<'_>,
    destino: Destino,
    ajustes: &Ajustes,
    tinta: &[Forma],
    senuelo: Option<usize>,
    mandato: Option<&Mandato>,
) -> Decision {
    // A delivery network carries the page's own things (decision 148 of GUARDIANA): it is the
    // road, not someone else. Counting Akamai as «an outside company that tried to connect»
    // would inflate the shield with the page's own images. A name on it that the lists know as
    // a tracker, though, is a tracker whatever road it takes (review of 10 Oct 2026, grave 2).
    // Your marked data never goes to anyone but the page's own site, road or not.
    let otro = p.recurso != Recurso::Documento && es_tercero_de(&destino, p.sitio_pagina);
    let tercero = otro
        && (destino.reparto.is_none()
            || matches!(
                destino.categoria,
                "rastreador" | "publicidad" | "telemetria"
            ));
    let hallazgos = if tinta.is_empty() {
        Vec::new()
    } else {
        busca(tinta, p.url, p.cuerpo)
    };
    // What the lists say, before the person's switches and rules.
    let lista = if tercero {
        match destino.categoria {
            "rastreador" => Some(Motivo::Rastreador),
            "publicidad" => Some(Motivo::Publicidad),
            _ if destino.corredor.is_some() => Some(Motivo::Corredor),
            "telemetria" => Some(Motivo::Telemetria),
            _ if p.recurso == Recurso::Aviso => Some(Motivo::Baliza),
            _ => None,
        }
    } else {
        None
    };
    let pagina = p.recurso == Recurso::Documento;
    let corta = |motivo: Motivo, destino: Destino, hallazgos: Vec<Hallazgo>| Decision {
        cortar: true,
        motivo: Some(motivo),
        destino,
        tercero,
        otro,
        hallazgos,
        lista,
        pagina,
    };
    // The decoy belongs to nobody: wherever it goes, it is being taken.
    if senuelo.is_some_and(|i| hallazgos.iter().any(|h| h.origen == i)) {
        return corta(Motivo::Senuelo, destino, hallazgos);
    }
    if let Some(m) = mandato {
        if !m.permite(&destino, p.recurso) {
            return corta(Motivo::FueraDeMandato, destino, hallazgos);
        }
    }
    // The person's marked data never goes to another company, whatever rule a site has:
    // «Desbloquear» undoes what the lists cut, not this. What the person sends with «Enviar» in
    // the form guard is let through for a few minutes before this is asked (`sesion`).
    if otro && !hallazgos.is_empty() {
        return corta(Motivo::Tinta, destino, hallazgos);
    }
    let permitido = ajustes.permitidos.contains(&destino.sitio);
    if ajustes.cortados.contains(&destino.sitio) {
        return corta(Motivo::CorteTuyo, destino, hallazgos);
    }
    // A page the person opens is never cut by a list: only what it pulls in from others.
    if ajustes.cortar_seguimiento && !permitido {
        let motivo = match lista {
            Some(Motivo::Telemetria | Motivo::Baliza) if !ajustes.maxima => None,
            m => m,
        };
        if let Some(m) = motivo {
            return corta(m, destino, hallazgos);
        }
    }
    Decision {
        cortar: false,
        motivo: None,
        destino,
        tercero,
        otro,
        hallazgos,
        lista,
        pagina,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tinta::{formas, Marcado, Tipo};

    fn pet<'a>(url: &'a str, cuerpo: &'a [u8], recurso: Recurso) -> Peticion<'a> {
        Peticion {
            url,
            cuerpo,
            recurso,
            sitio_pagina: "eltiempo.com",
        }
    }

    #[test]
    fn trackers_are_cut_only_when_the_person_said_so() {
        let mut a = Ajustes::default();
        let p = pet(
            "https://stats.g.doubleclick.net/g/collect",
            b"",
            Recurso::Imagen,
        );
        assert!(!decide(&p, &a, &[], None, None).cortar);
        a.cortar_seguimiento = true;
        let d = decide(&p, &a, &[], None, None);
        assert!(d.cortar);
        assert!(matches!(
            d.motivo,
            Some(Motivo::Rastreador | Motivo::Publicidad)
        ));
        // The page's own resources are never cut by a list.
        let own = pet("https://img.eltiempo.com/a.png", b"", Recurso::Imagen);
        assert!(!decide(&own, &a, &[], None, None).cortar);
        // An «undo» lets it through again.
        a.permitidos.insert("doubleclick.net".into());
        assert!(!decide(&p, &a, &[], None, None).cortar);
    }

    #[test]
    fn a_marked_email_never_reaches_a_third_party() {
        let f = formas(&[Marcado {
            tipo: Tipo::Correo,
            valor: "ana@correo.co".into(),
        }]);
        let a = Ajustes::default();
        let fuera = pet(
            "https://collect.otra-empresa.io/e",
            b"{\"email\":\"ana@correo.co\"}",
            Recurso::Datos,
        );
        let d = decide(&fuera, &a, &f, None, None);
        assert!(d.cortar);
        assert_eq!(d.motivo, Some(Motivo::Tinta));
        // To the page's own site it goes (it was typed there), and it is written down.
        let propia = pet(
            "https://www.eltiempo.com/login",
            b"email=ana%40correo.co",
            Recurso::Datos,
        );
        let d = decide(&propia, &a, &f, None, None);
        assert!(!d.cortar);
        assert_eq!(d.hallazgos.len(), 1);
    }

    #[test]
    fn the_mandate_and_its_decoy_hold() {
        let senuelo = crate::tinta::senuelo(b"m1");
        let m = Mandato::nuevo(
            "m1".into(),
            "",
            &["eltiempo.com".into()],
            senuelo.clone(),
            0,
        );
        let f = formas(&[Marcado {
            tipo: Tipo::Senuelo,
            valor: senuelo.clone(),
        }]);
        let a = Ajustes::default();
        let fuera = pet("https://evil.example/x", b"", Recurso::Documento);
        assert_eq!(
            decide(&fuera, &a, &f, Some(0), Some(&m)).motivo,
            Some(Motivo::FueraDeMandato)
        );
        let robo = format!("https://www.eltiempo.com/buscar?q={senuelo}");
        assert_eq!(
            decide(&pet(&robo, b"", Recurso::Datos), &a, &f, Some(0), Some(&m)).motivo,
            Some(Motivo::Senuelo)
        );
    }

    #[test]
    fn known_pixels_are_cut_by_their_path_only_from_other_sites() {
        let mut a = Ajustes {
            cortar_seguimiento: true,
            ..Ajustes::default()
        };
        let corta = |a: &Ajustes, url: &str, cuerpo: &[u8], recurso: Recurso| {
            let d = decide(&pet(url, cuerpo, recurso), a, &[], None, None);
            (d.cortar, d.motivo, d.destino.quien().to_string())
        };
        for (url, quien) in [
            (
                "https://www.facebook.com/tr?id=1&ev=Purchase&cd[value]=89900",
                "Meta",
            ),
            ("https://connect.facebook.net/en_US/fbevents.js", "Meta"),
            (
                "https://connect.facebook.net/signals/config/1?v=2.9",
                "Meta",
            ),
            (
                "https://www.google.com/pagead/1p-conversion/123/?value=1",
                "Google",
            ),
            (
                "https://www.google.com/pagead/1p-user-list/123/?random=1",
                "Google",
            ),
            ("https://www.google.com/ccm/collect?en=page_view", "Google"),
            ("https://analytics.twitter.com/i/adsct?txn_id=o1", "X"),
            ("https://t.co/i/adsct?txn_id=o1", "X"),
            ("https://static.ads-twitter.com/uwt.js", "X"),
            (
                "https://snap.licdn.com/li.lms-analytics/insight.min.js",
                "LinkedIn",
            ),
            ("https://px.ads.linkedin.com/collect?pid=1", "LinkedIn"),
            ("https://api.segment.io/v1/t", "Segment"),
            (
                "https://cdn.segment.com/analytics.js/v1/k/analytics.min.js",
                "Segment",
            ),
            (
                "https://cdn.mxpnl.com/libs/mixpanel-2-latest.min.js",
                "Mixpanel",
            ),
            ("https://api-js.mixpanel.com/track/?ip=1", "Mixpanel"),
            (
                "https://bat.bing.com/action/0?ti=1&evt=pageLoad",
                "Microsoft",
            ),
        ] {
            let (cortar, motivo, q) = corta(&a, url, b"", Recurso::Imagen);
            assert!(cortar, "{url}");
            assert!(
                matches!(motivo, Some(Motivo::Rastreador | Motivo::Publicidad)),
                "{url}"
            );
            assert_eq!(q, quien, "{url}");
        }
        // The pixel by POST, as `fbevents.js` sends it when the data is long.
        assert!(
            corta(
                &a,
                "https://www.facebook.com/tr/",
                b"ev=Purchase",
                Recurso::Datos
            )
            .0
        );
        // The rest of those names is not a pixel: the like button, the login SDK, reCAPTCHA.
        assert!(
            !corta(
                &a,
                "https://www.facebook.com/plugins/like.php",
                b"",
                Recurso::Marco
            )
            .0
        );
        assert!(
            !corta(
                &a,
                "https://connect.facebook.net/en_US/sdk.js",
                b"",
                Recurso::Script
            )
            .0
        );
        assert!(
            !corta(
                &a,
                "https://www.google.com/recaptcha/api.js",
                b"",
                Recurso::Script
            )
            .0
        );
        // Telemetry (Sentry) only with maximum protection.
        let sentry = "https://o4505.ingest.sentry.io/api/1/envelope/";
        assert!(!corta(&a, sentry, b"{}", Recurso::Datos).0);
        a.maxima = true;
        assert_eq!(
            corta(&a, sentry, b"{}", Recurso::Datos),
            (true, Some(Motivo::Telemetria), "Sentry".into())
        );
        // On Facebook itself, its own pixel is the page's own business.
        let propia = Peticion {
            url: "https://www.facebook.com/tr?id=1&ev=PageView",
            cuerpo: b"",
            recurso: Recurso::Imagen,
            sitio_pagina: "facebook.com",
        };
        assert!(!decide(&propia, &a, &[], None, None).cortar);
        // And «Desbloquear» lets it through like any cut of the lists.
        a.permitidos.insert("facebook.com".into());
        assert!(!corta(&a, "https://www.facebook.com/tr?id=1", b"", Recurso::Imagen).0);
    }

    #[test]
    fn a_tracker_on_a_delivery_network_is_still_a_tracker() {
        let mut a = Ajustes {
            cortar_seguimiento: true,
            ..Ajustes::default()
        };
        // A CloudFront distribution EasyPrivacy lists as a tracker.
        let p = pet(
            "https://d10lpsik1i8c69.cloudfront.net/w.js",
            b"",
            Recurso::Script,
        );
        let d = decide(&p, &a, &[], None, None);
        assert!(d.tercero && d.cortar, "{d:?}");
        assert_eq!(d.motivo, Some(Motivo::Rastreador));
        // The road itself, carrying the page's own images, is still the road.
        let img = pet(
            "https://d1abc.cloudfront.net/logo.png",
            b"",
            Recurso::Imagen,
        );
        let d = decide(&img, &a, &[], None, None);
        assert!(!d.tercero && !d.cortar, "{d:?}");
        // Even if the lists still called its name «the road».
        a.maxima = true;
        let mut cdn = crate::destino::clasifica_url("https://c.akstat.io/b");
        cdn.reparto = Some("Akamai");
        let d = decide_con(
            &pet("https://c.akstat.io/b", b"x", Recurso::Aviso),
            cdn,
            &a,
            &[],
            None,
            None,
        );
        assert!(d.tercero && d.cortar, "{d:?}");
    }

    #[test]
    fn unblocking_a_site_never_lets_marked_data_go_there() {
        let f = formas(&[Marcado {
            tipo: Tipo::Correo,
            valor: "ana@correo.co".into(),
        }]);
        let mut a = Ajustes::default();
        let p = pet(
            "https://checkout.pagos-ejemplo.com/pay",
            b"email=ana%40correo.co",
            Recurso::Datos,
        );
        assert_eq!(decide(&p, &a, &f, None, None).motivo, Some(Motivo::Tinta));
        // «Desbloquear» undoes the lists' cuts, not this one.
        a.permitidos.insert("pagos-ejemplo.com".into());
        let d = decide(&p, &a, &f, None, None);
        assert_eq!((d.cortar, d.motivo), (true, Some(Motivo::Tinta)));
        assert_eq!(d.hallazgos.len(), 1);
        // The mandate's decoy is never anybody's to unblock.
        let senuelo = crate::tinta::senuelo(b"m");
        let fs = formas(&[Marcado {
            tipo: Tipo::Senuelo,
            valor: senuelo.clone(),
        }]);
        let robo = format!("https://checkout.pagos-ejemplo.com/p?e={senuelo}");
        assert_eq!(
            decide(&pet(&robo, b"", Recurso::Datos), &a, &fs, Some(0), None).motivo,
            Some(Motivo::Senuelo)
        );
    }

    #[test]
    fn maximum_protection_also_cuts_telemetry_and_beacons_but_never_the_page() {
        let mut a = Ajustes {
            cortar_seguimiento: true,
            ..Ajustes::default()
        };
        let baliza = pet("https://cdn.otra.io/ping", b"", Recurso::Aviso);
        let script = pet("https://cdn.otra.io/app.js", b"", Recurso::Script);
        assert!(!decide(&baliza, &a, &[], None, None).cortar);
        a.maxima = true;
        let d = decide(&baliza, &a, &[], None, None);
        assert_eq!((d.cortar, d.motivo), (true, Some(Motivo::Baliza)));
        // The same company's other requests still load: only what carries information out.
        assert!(!decide(&script, &a, &[], None, None).cortar);
        // The page's own beacons are the page's.
        let propia = pet("https://www.eltiempo.com/ping", b"", Recurso::Aviso);
        assert!(!decide(&propia, &a, &[], None, None).cortar);
        // Unblocked by hand, it passes again; and without the cut on, nothing of this applies.
        a.permitidos.insert("otra.io".into());
        assert!(!decide(&baliza, &a, &[], None, None).cortar);
        a.permitidos.clear();
        a.cortar_seguimiento = false;
        assert!(!decide(&baliza, &a, &[], None, None).cortar);
    }
}
