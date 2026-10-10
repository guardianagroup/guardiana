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
}

impl Diario {
    /// Count one decided request on `dia`.
    pub fn anota(&mut self, dia: &str, d: &Decision) {
        let e = self.dias.entry(dia.to_string()).or_default();
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
            diario.anota("2026-10-09", &d);
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
}
