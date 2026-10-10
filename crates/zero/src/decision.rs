//! The one place that decides whether a request may leave. The shell asks for every request of
//! every tab, before the engine sends it, and does what the answer says; the answer also says
//! why, so the person sees it in real time («cortado: rastreador de Google»).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::destino::{clasifica, es_tercero, Destino};
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    let host = crate::dominio::host_de(p.url).unwrap_or_default();
    let destino = clasifica(&host);
    // A delivery network carries the page's own things (decision 148 of GUARDIANA): it is the
    // road, not someone else. Counting Akamai as «an outside company that tried to connect»
    // would inflate the shield with the page's own images.
    // Your marked data, though, never goes to anyone but the page's own site, road or not.
    let otro = p.recurso != Recurso::Documento && es_tercero(&host, p.sitio_pagina);
    let tercero = otro && destino.reparto.is_none();
    let hallazgos = if tinta.is_empty() {
        Vec::new()
    } else {
        busca(tinta, p.url, p.cuerpo)
    };
    let corta = |motivo: Motivo, destino: Destino, hallazgos: Vec<Hallazgo>| Decision {
        cortar: true,
        motivo: Some(motivo),
        destino,
        tercero,
        otro,
        hallazgos,
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
    if otro && !hallazgos.is_empty() {
        return corta(Motivo::Tinta, destino, hallazgos);
    }
    if ajustes.cortados.contains(&destino.sitio) {
        return corta(Motivo::CorteTuyo, destino, hallazgos);
    }
    // A page the person opens is never cut by a list: only what it pulls in from others.
    if ajustes.cortar_seguimiento && tercero && !ajustes.permitidos.contains(&destino.sitio) {
        let motivo = match destino.categoria {
            "rastreador" => Some(Motivo::Rastreador),
            "publicidad" => Some(Motivo::Publicidad),
            _ if destino.corredor.is_some() => Some(Motivo::Corredor),
            "telemetria" if ajustes.maxima => Some(Motivo::Telemetria),
            _ if ajustes.maxima && p.recurso == Recurso::Aviso => Some(Motivo::Baliza),
            _ => None,
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
