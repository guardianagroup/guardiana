//! The shield: what each tab is being saved from, in real time, and the running totals of the
//! day and the month. Every figure is a count of requests the browser itself decided: nothing is
//! estimated, nothing is rounded, nothing is invented (CLAUDE.md: never an invented counter).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::decision::{Decision, Motivo};

/// One company or site that a page tried to send you to or pull in, as the shield lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tercero {
    /// The name people recognise (company, AI service, registered broker, or the site).
    pub quien: String,
    /// The registrable site.
    pub sitio: String,
    /// The owner's country, two letters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pais: Option<String>,
    /// The lists' category.
    pub categoria: String,
    /// Registered as a data broker (its name in the registry).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corredor: Option<String>,
    /// Requests seen.
    pub vistas: u32,
    /// Requests cut.
    pub cortadas: u32,
    /// Why the last cut happened.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motivo: Option<Motivo>,
}

/// What the shield of one tab shows.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pestana {
    /// The site the tab shows.
    pub sitio: String,
    /// Every third party of this page, by site.
    pub terceros: BTreeMap<String, Tercero>,
    /// Tracking tags taken out of the address.
    pub parametros_quitados: u32,
    /// Marked values cut on their way out.
    pub datos_salvados: u32,
    /// The page's cookie notice: its consent manager and what was done (`rechazado`, or
    /// `escondido` when it had no «reject all»).
    pub cookies: Option<(String, String)>,
}

impl Pestana {
    /// A clean shield for a page of `sitio` (on every top-level navigation).
    #[must_use]
    pub fn nueva(sitio: &str) -> Self {
        Self {
            sitio: sitio.into(),
            ..Self::default()
        }
    }

    /// Count one decided request.
    pub fn anota(&mut self, d: &Decision) {
        if matches!(d.motivo, Some(Motivo::Tinta | Motivo::Senuelo)) {
            self.datos_salvados += 1;
        }
        // Outside companies only: the page's own site, even when you cut a part of it, is not
        // «a company from outside».
        if !d.de_fuera() {
            return;
        }
        let e = self
            .terceros
            .entry(d.destino.sitio.clone())
            .or_insert_with(|| Tercero {
                quien: d.destino.quien().to_string(),
                sitio: d.destino.sitio.clone(),
                pais: d.destino.pais.map(str::to_string),
                categoria: d.destino.categoria.to_string(),
                corredor: d.destino.corredor.as_ref().map(|c| c.nombre.to_string()),
                vistas: 0,
                cortadas: 0,
                motivo: None,
            });
        e.vistas += 1;
        if d.cortar {
            e.cortadas += 1;
            e.motivo = d.motivo;
        }
    }

    /// The numbers on the button and at the top of the panel.
    #[must_use]
    pub fn resumen(&self) -> Resumen {
        let mut r = Resumen {
            terceros: self.terceros.len() as u32,
            parametros_quitados: self.parametros_quitados,
            datos_salvados: self.datos_salvados,
            ..Resumen::default()
        };
        let mut empresas = BTreeSet::new();
        let mut cortadas = BTreeSet::new();
        for t in self.terceros.values() {
            empresas.insert(t.quien.as_str());
            r.cortadas += t.cortadas;
            if t.cortadas > 0 {
                r.terceros_cortados += 1;
                cortadas.insert(t.quien.as_str());
            }
            if t.corredor.is_some() {
                r.corredores += 1;
            }
            if matches!(t.categoria.as_str(), "rastreador" | "publicidad") {
                r.rastreadores += 1;
            }
        }
        r.empresas = empresas.len() as u32;
        r.empresas_cortadas = cortadas.len() as u32;
        r
    }
}

/// Totals of a tab, a day or a month.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resumen {
    /// Distinct third parties (sites).
    pub terceros: u32,
    /// Distinct companies behind them.
    pub empresas: u32,
    /// Third parties with at least one request cut.
    pub terceros_cortados: u32,
    /// Distinct companies with at least one request cut: the unit the person reads («a 2
    /// empresas se les cortó el paso»), the same as `empresas`.
    pub empresas_cortadas: u32,
    /// Requests cut.
    pub cortadas: u32,
    /// Third parties the open lists call trackers or advertising.
    pub rastreadores: u32,
    /// Registered data brokers.
    pub corredores: u32,
    /// Tracking tags taken out of addresses.
    pub parametros_quitados: u32,
    /// Marked values cut on their way out.
    pub datos_salvados: u32,
    /// Cookie notices answered «no» (or hidden without accepting anything).
    #[serde(default)]
    pub avisos_cookies: u32,
}

/// The days the browser was used, each with its totals: what the home page and «your month in
/// data» read. Kept in the browser's folder; nothing is sent anywhere.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diario {
    /// `AAAA-MM-DD` → that day's totals.
    pub dias: BTreeMap<String, Dia>,
}

/// One day: the third parties met (by site), and the counters.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dia {
    /// Sites of the third parties met, with the company behind each.
    pub terceros: BTreeMap<String, String>,
    /// Sites of the third parties cut at least once.
    pub cortados: BTreeSet<String>,
    /// Registered data brokers met.
    pub corredores: BTreeSet<String>,
    /// Requests cut.
    pub cortadas: u32,
    /// Tracking tags taken out.
    pub parametros_quitados: u32,
    /// Marked values cut.
    pub datos_salvados: u32,
    /// Pages opened.
    pub paginas: u32,
    /// Cookie notices answered «no» (or hidden without accepting anything), one per page.
    #[serde(default)]
    pub avisos_cookies: u32,
    /// The sites the person had open (never in an isolated tab): how many webs the day had.
    /// Kept only for the last [`DIAS_WEBS`] days.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub webs: BTreeSet<String>,
    /// The companies that tried to follow the person from site to site: on which of those webs
    /// each one showed up as a tracker, advertiser or data broker, and on which it got through.
    /// Kept only for the last [`DIAS_WEBS`] days.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub siguen: BTreeMap<String, Sigue>,
}

/// Where one company tried to follow the person on one day.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sigue {
    /// The sites of the pages it showed up on.
    pub webs: BTreeSet<String>,
    /// Those where at least one of its requests got through.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub paso: BTreeSet<String>,
}

/// Days for which the webs of each day are kept (the same as the list of cuts).
pub const DIAS_WEBS: usize = 31;
/// Webs kept per day, and companies per day: bounds, not expectations.
const MAX_WEBS: usize = 5_000;
const MAX_SIGUEN: usize = 2_000;

/// One company that followed the person across sites, as the shield and the new tab show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seguidor {
    /// The company.
    pub quien: String,
    /// On how many distinct webs it showed up.
    pub webs: u32,
    /// On how many of them something of it got through.
    pub paso: u32,
}

/// Who followed the person across the most sites in a period.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rastro {
    /// Distinct webs the person had open.
    pub webs: u32,
    /// The companies that showed up on two webs or more, most webs first, at most `n`.
    pub seguidores: Vec<Seguidor>,
    /// How many companies showed up on two webs or more (the list may be shorter).
    pub total: u32,
}

impl Diario {
    /// Count one decided request on `dia`. `pagina` is the site of the page that made it, or
    /// `None` in an isolated tab or on the browser's own pages (they are not webs of the day).
    pub fn anota(&mut self, dia: &str, d: &Decision, pagina: Option<&str>) {
        let e = self.dias.entry(dia.to_string()).or_default();
        let pagina = pagina.filter(|p| !p.is_empty());
        if let Some(p) = pagina {
            if e.webs.len() < MAX_WEBS && !e.webs.contains(p) {
                e.webs.insert(p.to_string());
            }
        }
        if matches!(d.motivo, Some(Motivo::Tinta | Motivo::Senuelo)) {
            e.datos_salvados += 1;
        }
        // Outside companies only: the page's own site, even when you cut a part of it, is not
        // «a company from outside».
        if !d.de_fuera() {
            return;
        }
        if e.terceros.len() < 20_000 {
            e.terceros
                .entry(d.destino.sitio.clone())
                .or_insert_with(|| d.destino.quien().to_string());
        }
        if let Some(c) = &d.destino.corredor {
            e.corredores.insert(c.nombre.to_string());
        }
        if d.cortar {
            e.cortadas += 1;
            e.cortados.insert(d.destino.sitio.clone());
        }
        let sigue = d.destino.sigue() || d.destino.corredor.is_some();
        if let (true, Some(p)) = (sigue, pagina) {
            let quien = d.destino.quien();
            if e.siguen.len() < MAX_SIGUEN || e.siguen.contains_key(quien) {
                let s = e.siguen.entry(quien.to_string()).or_default();
                if s.webs.len() < MAX_WEBS {
                    s.webs.insert(p.to_string());
                    if !d.cortar {
                        s.paso.insert(p.to_string());
                    }
                }
            }
        }
    }

    /// Forget which webs were open on the days before `desde` (`AAAA-MM-DD`): the totals stay,
    /// the sites go, as the list of cuts does after [`DIAS_WEBS`] days.
    pub fn olvida_webs(&mut self, desde: &str) -> bool {
        let mut algo = false;
        for (_, d) in self.dias.range_mut(..desde.to_string()) {
            if !d.webs.is_empty() || !d.siguen.is_empty() {
                d.webs.clear();
                d.siguen.clear();
                algo = true;
            }
        }
        algo
    }

    /// Who followed the person across the most webs from `desde` to `hasta` (inclusive): the
    /// companies seen on two webs or more, most webs first, at most `n` of them.
    #[must_use]
    pub fn rastro(&self, desde: &str, hasta: &str, n: usize) -> Rastro {
        let mut webs = BTreeSet::new();
        let mut por: BTreeMap<&str, (BTreeSet<&str>, BTreeSet<&str>)> = BTreeMap::new();
        for (_, d) in self.dias.range(desde.to_string()..=hasta.to_string()) {
            webs.extend(d.webs.iter().map(String::as_str));
            for (q, s) in &d.siguen {
                let e = por.entry(q.as_str()).or_default();
                e.0.extend(s.webs.iter().map(String::as_str));
                e.1.extend(s.paso.iter().map(String::as_str));
            }
        }
        let mut seguidores: Vec<Seguidor> = por
            .into_iter()
            .filter(|(_, (w, _))| w.len() >= 2)
            .map(|(q, (w, p))| Seguidor {
                quien: q.to_string(),
                webs: u32::try_from(w.len()).unwrap_or(u32::MAX),
                paso: u32::try_from(p.len()).unwrap_or(u32::MAX),
            })
            .collect();
        seguidores.sort_by(|a, b| b.webs.cmp(&a.webs).then_with(|| a.quien.cmp(&b.quien)));
        let total = u32::try_from(seguidores.len()).unwrap_or(u32::MAX);
        seguidores.truncate(n);
        Rastro {
            webs: u32::try_from(webs.len()).unwrap_or(u32::MAX),
            seguidores,
            total,
        }
    }

    /// Totals from `desde` to `hasta` (inclusive, `AAAA-MM-DD`), distinct counts across days.
    #[must_use]
    pub fn total(&self, desde: &str, hasta: &str) -> Resumen {
        let mut sitios = BTreeSet::new();
        let mut empresas = BTreeSet::new();
        let mut cortados = BTreeSet::new();
        let mut corredores = BTreeSet::new();
        let mut r = Resumen::default();
        for (_, d) in self.dias.range(desde.to_string()..=hasta.to_string()) {
            for (s, q) in &d.terceros {
                sitios.insert(s.as_str());
                empresas.insert(q.as_str());
            }
            cortados.extend(d.cortados.iter().map(String::as_str));
            corredores.extend(d.corredores.iter().map(String::as_str));
            r.cortadas += d.cortadas;
            r.parametros_quitados += d.parametros_quitados;
            r.datos_salvados += d.datos_salvados;
            r.avisos_cookies += d.avisos_cookies;
        }
        r.terceros = sitios.len() as u32;
        r.empresas = empresas.len() as u32;
        r.terceros_cortados = cortados.len() as u32;
        // A cut site's company is in the same day's map (cuts are written there too).
        let mut empresas_cortadas = BTreeSet::new();
        for (_, d) in self.dias.range(desde.to_string()..=hasta.to_string()) {
            for s in &d.cortados {
                empresas_cortadas.insert(d.terceros.get(s).map_or(s.as_str(), String::as_str));
            }
        }
        r.empresas_cortadas = empresas_cortadas.len() as u32;
        r.corredores = corredores.len() as u32;
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decision::{decide, Ajustes, Peticion};
    use crate::mandato::Recurso;

    #[test]
    fn the_shield_counts_what_was_decided_and_nothing_else() {
        let a = Ajustes {
            cortar_seguimiento: true,
            ..Ajustes::default()
        };
        let mut p = Pestana::nueva("eltiempo.com");
        let mut diario = Diario::default();
        for url in [
            "https://stats.g.doubleclick.net/a",
            "https://stats.g.doubleclick.net/b",
            "https://www.google-analytics.com/g/collect",
            "https://img.eltiempo.com/x.png",
            "https://cdn.otra.io/y.js",
        ] {
            let d = decide(
                &Peticion {
                    url,
                    cuerpo: b"",
                    recurso: Recurso::Script,
                    sitio_pagina: "eltiempo.com",
                },
                &a,
                &[],
                None,
                None,
            );
            p.anota(&d);
            diario.anota("2026-10-09", &d, Some("eltiempo.com"));
        }
        let r = p.resumen();
        assert_eq!(r.terceros, 3);
        assert_eq!(r.cortadas, 3);
        // Two Google sites cut: two sites, one company — the person reads companies.
        assert_eq!((r.terceros_cortados, r.empresas_cortadas), (2, 1));
        assert_eq!(r.empresas, 2);
        let t = diario.total("2026-10-01", "2026-10-31");
        assert_eq!((t.terceros, t.cortadas), (3, 3));
        assert_eq!((t.empresas, t.empresas_cortadas), (2, 1));
    }

    #[test]
    fn the_trail_counts_on_how_many_webs_each_company_showed_up() {
        fn pide(diario: &mut Diario, dia: &str, pagina: Option<&str>, url: &str, a: &Ajustes) {
            let d = decide(
                &Peticion {
                    url,
                    cuerpo: b"",
                    recurso: Recurso::Script,
                    sitio_pagina: pagina.unwrap_or("aislada.example"),
                },
                a,
                &[],
                None,
                None,
            );
            diario.anota(dia, &d, pagina);
        }
        let a = Ajustes {
            cortar_seguimiento: true,
            ..Ajustes::default()
        };
        let mut diario = Diario::default();
        for web in ["eltiempo.com", "elpais.com", "semana.com"] {
            pide(
                &mut diario,
                "2026-10-09",
                Some(web),
                "https://www.google-analytics.com/g/collect",
                &a,
            );
        }
        // A tracker seen on one web only is not following anyone from site to site.
        pide(
            &mut diario,
            "2026-10-09",
            Some("elpais.com"),
            "https://stats.g.doubleclick.net/a",
            &a,
        );
        pide(
            &mut diario,
            "2026-10-09",
            Some("eltiempo.com"),
            "https://cdn.otra.io/y.js",
            &a,
        );
        // An isolated tab leaves no web behind.
        pide(
            &mut diario,
            "2026-10-09",
            None,
            "https://www.google-analytics.com/g/collect",
            &a,
        );
        // With cutting off, what gets through is told apart.
        let sin = Ajustes::default();
        pide(
            &mut diario,
            "2026-10-10",
            Some("eltiempo.com"),
            "https://www.google-analytics.com/g/collect",
            &sin,
        );
        pide(
            &mut diario,
            "2026-10-10",
            Some("bbc.com"),
            "https://www.google-analytics.com/g/collect",
            &a,
        );
        let r = diario.rastro("2026-10-09", "2026-10-09", 5);
        assert_eq!(r.webs, 3);
        assert_eq!(r.total, 1);
        assert_eq!(r.seguidores[0].quien, "Google");
        assert_eq!((r.seguidores[0].webs, r.seguidores[0].paso), (3, 0));
        let r = diario.rastro("2026-10-09", "2026-10-10", 5);
        assert_eq!(r.webs, 4);
        assert_eq!((r.seguidores[0].webs, r.seguidores[0].paso), (4, 1));
        // After the days the cuts are kept, the webs go and the totals stay.
        assert!(diario.olvida_webs("2026-10-10"));
        assert_eq!(
            diario.rastro("2026-10-09", "2026-10-09", 5),
            Rastro::default()
        );
        assert_eq!(diario.total("2026-10-09", "2026-10-09").cortadas, 5);
        assert!(!diario.olvida_webs("2026-10-10"));
    }
}
