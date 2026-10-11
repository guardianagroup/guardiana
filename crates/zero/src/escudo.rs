//! The shield: what each tab is being saved from, in real time, and the running totals of the
//! day and the month. Every figure is a count of requests the browser itself decided: nothing is
//! estimated, nothing is rounded, nothing is invented (CLAUDE.md: never an invented counter).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::chivatos::{Chivato, Evento, Red};
use crate::decision::{Ajustes, Decision, Motivo};

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
    /// Cuts by reason.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub motivos: BTreeMap<Motivo, u32>,
    /// What each request was, to tell what holds for the site now, whatever switch changes
    /// (review of 10 Oct 2026, media 8). Each request counts once, in the first that applies:
    /// cut for being outside the mandate or for carrying its decoy (no rule of the person's
    /// undoes those)…
    #[serde(default, skip_serializing_if = "is_zero")]
    pub fijos: u32,
    /// …carrying a marked value of the person to it…
    #[serde(default, skip_serializing_if = "is_zero")]
    pub con_dato: u32,
    /// …one the open lists cut with the protection on…
    #[serde(default, skip_serializing_if = "is_zero")]
    pub de_lista: u32,
    /// …or one only maximum protection cuts (telemetry, pings and beacons).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub de_maxima: u32,
    /// The lists' reason for the last request of it they know.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motivo_lista: Option<Motivo>,
}

/// What holds now for one site of the shield.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Estado {
    /// `cortado` (everything it asked for is cut now), `parcial` (part of it) or `pasa`.
    pub ahora: &'static str,
    /// The rule behind it: `tuya` (the person cut it), `permitido` (the person let it
    /// through), `lista` (the open lists cut it), or none.
    pub regla: Option<&'static str>,
    /// Why it is cut, when it is.
    pub por: Option<Motivo>,
    /// «Desbloquear» would let through something that is cut now: the person's own cut, or
    /// what the lists cut. Not marked data, nor what the task's limits cut.
    pub deshace: bool,
    /// Part of what is cut now carried the person's marked data: only «Enviar» in the form
    /// guard sends it.
    pub dato: bool,
}

impl Tercero {
    /// A site seen through the lists alone, when no shield shows it any more: what its name
    /// says, as one request.
    #[must_use]
    pub fn de_sitio(sitio: &str) -> Self {
        let d = crate::destino::clasifica(sitio);
        let lista = match d.categoria {
            "rastreador" => Some(Motivo::Rastreador),
            "publicidad" => Some(Motivo::Publicidad),
            _ if d.corredor.is_some() => Some(Motivo::Corredor),
            "telemetria" => Some(Motivo::Telemetria),
            _ => None,
        };
        Self {
            quien: d.quien().to_string(),
            sitio: sitio.to_string(),
            pais: None,
            categoria: d.categoria.to_string(),
            corredor: d.corredor.as_ref().map(|c| c.nombre.to_string()),
            vistas: 1,
            cortadas: 0,
            motivo: None,
            motivos: BTreeMap::new(),
            fijos: 0,
            con_dato: 0,
            de_lista: u32::from(matches!(
                lista,
                Some(Motivo::Rastreador | Motivo::Publicidad | Motivo::Corredor)
            )),
            de_maxima: u32::from(lista == Some(Motivo::Telemetria)),
            motivo_lista: lista,
        }
    }

    /// How many of its requests would be cut now with these settings: (by the mandate's
    /// limits, for carrying marked data, by the lists). With `reglas` false, leaving out the
    /// person's own rules. «Desbloquear» only undoes what the lists cut: marked data never
    /// goes to another company by a rule (the owner, 10 Oct 2026).
    fn cortaria(&self, a: &Ajustes, reglas: bool) -> (u32, u32, u32) {
        let permitido = reglas && a.permitidos.contains(&self.sitio);
        let lista = if a.cortar_seguimiento && !permitido {
            self.de_lista
                .saturating_add(if a.maxima { self.de_maxima } else { 0 })
        } else {
            0
        };
        (self.fijos, self.con_dato, lista)
    }

    /// What the lists, the marked data and the mandate cut of it by themselves, without any
    /// rule of the person's: `(all of it, some of it)`. «Bloquear» needs no rule of its own when
    /// they already cut it all, and «Volver a bloquear» leaves it to them when they cut a part.
    #[must_use]
    pub fn sin_reglas(&self, a: &Ajustes) -> (bool, bool) {
        let (f, d, l) = self.cortaria(a, false);
        let n = f.saturating_add(d).saturating_add(l);
        (self.vistas > 0 && n >= self.vistas, n > 0)
    }

    /// What holds now for the site with the person's settings (`protege` false: nothing is
    /// cut, the trial or the subscription is over).
    #[must_use]
    pub fn estado(&self, a: &Ajustes, protege: bool) -> Estado {
        if !protege {
            return Estado {
                ahora: "pasa",
                regla: None,
                por: None,
                deshace: false,
                dato: false,
            };
        }
        if a.cortados.contains(&self.sitio) {
            return Estado {
                ahora: "cortado",
                regla: Some("tuya"),
                por: Some(Motivo::CorteTuyo),
                deshace: true,
                dato: self.con_dato > 0,
            };
        }
        let permitido = a.permitidos.contains(&self.sitio);
        let (f, d, l) = self.cortaria(a, true);
        let n = f.saturating_add(d).saturating_add(l).min(self.vistas);
        let ahora = if n == 0 {
            "pasa"
        } else if n >= self.vistas {
            "cortado"
        } else {
            "parcial"
        };
        let por = if f > 0 {
            Some(if self.motivos.contains_key(&Motivo::Senuelo) {
                Motivo::Senuelo
            } else {
                Motivo::FueraDeMandato
            })
        } else if d > 0 {
            Some(Motivo::Tinta)
        } else if l > 0 {
            self.motivo_lista
        } else {
            None
        };
        let regla = if permitido {
            Some("permitido")
        } else if l > 0 {
            Some("lista")
        } else {
            None
        };
        Estado {
            ahora,
            regla,
            por,
            deshace: l > 0,
            dato: d > 0,
        }
    }

    /// Nothing the person's rules decide changes it: all of it was cut by the mandate's limits.
    #[must_use]
    pub const fn fijo(&self) -> bool {
        self.vistas > 0 && self.fijos >= self.vistas
    }
}

/// One thing a page's pixels tried to tell, as the shield lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChivatoVisto {
    /// What it said.
    #[serde(flatten)]
    pub chivato: Chivato,
    /// Every time it was sent, it was cut.
    pub cortado: bool,
    /// How many times it was sent (the same event, by its id, is one).
    pub veces: u32,
}

/// What a shareable card says about the pixels of a page that were cut: `n` companies were told
/// `evento` (and all of them with the person's hashed email, when `correo`), and every one of
/// those attempts was cut. Nothing else: no amount, no product, no email.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Tarjeta {
    /// Companies.
    pub empresas: u32,
    /// The event, the one that says most among those cut.
    pub evento: Evento,
    /// All of them carried the person's own email, hashed.
    pub correo: bool,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// Third parties kept per tab, sites per day: bounds against a page that calls endless names.
const MAX_TERCEROS: usize = 2_000;
const MAX_SITIOS_DIA: usize = 20_000;
/// Pixel events kept per tab.
const MAX_CHIVATOS: usize = 50;

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
    /// What its pixels tried to tell (in memory only: amounts and products never reach the
    /// disk).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chivatos: Vec<ChivatoVisto>,
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
        if self.terceros.len() >= MAX_TERCEROS && !self.terceros.contains_key(&d.destino.sitio) {
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
                motivos: BTreeMap::new(),
                fijos: 0,
                con_dato: 0,
                de_lista: 0,
                de_maxima: 0,
                motivo_lista: None,
            });
        // One name of a site may say more than the first one did: `adservice.google.com` after
        // `www.google.com`, the pixel after the like button.
        if matches!(d.destino.categoria, "rastreador" | "publicidad")
            && !matches!(e.categoria.as_str(), "rastreador" | "publicidad")
        {
            e.categoria = d.destino.categoria.to_string();
        }
        if e.quien == e.sitio && d.destino.quien() != d.destino.sitio {
            e.quien = d.destino.quien().to_string();
        }
        e.vistas += 1;
        if d.cortar {
            e.cortadas += 1;
            e.motivo = d.motivo;
            if let Some(m) = d.motivo {
                *e.motivos.entry(m).or_insert(0) += 1;
            }
        }
        if d.lista.is_some() {
            e.motivo_lista = d.lista;
        }
        if d.cortar && matches!(d.motivo, Some(Motivo::FueraDeMandato | Motivo::Senuelo)) {
            e.fijos += 1;
        } else if d.otro && !d.hallazgos.is_empty() {
            e.con_dato += 1;
        } else {
            match d.lista {
                Some(Motivo::Telemetria | Motivo::Baliza) => e.de_maxima += 1,
                Some(_) => e.de_lista += 1,
                None => {}
            }
        }
    }

    /// Keep what a page's pixel tried to tell. The same event sent again (by its id) is one;
    /// without an id, the same thing again counts one more time. Returns whether it was new.
    pub fn chivato(&mut self, c: Chivato, cortado: bool) -> bool {
        let mismo = |x: &Chivato| {
            x.red == c.red
                && match (&x.id, &c.id) {
                    (Some(a), Some(b)) => a == b,
                    (None, None) => {
                        x.evento == c.evento
                            && x.nombre == c.nombre
                            && x.importe == c.importe
                            && x.moneda == c.moneda
                    }
                    _ => false,
                }
        };
        if let Some(e) = self.chivatos.iter_mut().find(|e| mismo(&e.chivato)) {
            if c.id.is_none() {
                e.veces = e.veces.saturating_add(1);
            }
            // If it got through once, it got through.
            e.cortado &= cortado;
            // A copy may say what the first did not (the email in the second request).
            if e.chivato.correo.is_none_or(|d| !d.tuyo) && c.correo.is_some() {
                e.chivato.correo = c.correo;
            }
            if e.chivato.telefono.is_none_or(|d| !d.tuyo) && c.telefono.is_some() {
                e.chivato.telefono = c.telefono;
            }
            return false;
        }
        if self.chivatos.len() >= MAX_CHIVATOS {
            return false;
        }
        self.chivatos.push(ChivatoVisto {
            chivato: c,
            cortado,
            veces: 1,
        });
        true
    }

    /// The card to share, when something the pixels tried to tell was cut: the event that says
    /// most among those cut, told to how many companies, every attempt of theirs cut. A pixel
    /// that says it was sent from another web (a beacon of the page before) is left out.
    #[must_use]
    pub fn tarjeta(&self) -> Option<Tarjeta> {
        let de_aqui =
            |c: &&ChivatoVisto| c.chivato.pagina.as_deref().is_none_or(|p| p == self.sitio);
        let evento = self
            .chivatos
            .iter()
            .filter(de_aqui)
            .filter(|c| c.cortado)
            .map(|c| c.chivato.evento)
            .max_by_key(|e| e.peso())?;
        let mut por: BTreeMap<&str, (bool, bool)> = BTreeMap::new();
        for c in self
            .chivatos
            .iter()
            .filter(de_aqui)
            .filter(|c| c.chivato.evento == evento)
        {
            let e = por.entry(c.chivato.red.empresa()).or_insert((true, false));
            e.0 &= c.cortado;
            e.1 |= c.chivato.correo.is_some_and(|d| d.tuyo && d.cifrado);
        }
        let cortadas: Vec<bool> = por.values().filter(|(c, _)| *c).map(|(_, m)| *m).collect();
        if cortadas.is_empty() {
            return None;
        }
        Some(Tarjeta {
            empresas: u32::try_from(cortadas.len()).unwrap_or(u32::MAX),
            evento,
            correo: cortadas.iter().all(|m| *m),
        })
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
    /// What pages' pixels tried to tell, counted by network and event: never an amount, a
    /// product or a site. In memory only: on disk it would say which days the person bought
    /// something.
    #[serde(skip)]
    pub chivatos: BTreeMap<String, BTreeMap<String, u32>>,
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
        // «a company from outside». A whole page that was cut is in the day too, as it is in
        // the list of cuts.
        if !d.anotable() {
            return;
        }
        // A whole page the person (or the AI, a step outside its task) opened and that was cut
        // is a request cut, not «a company from outside» that tried anything.
        if d.pagina && !d.de_fuera() {
            e.cortadas += 1;
            return;
        }
        if e.terceros.len() < MAX_SITIOS_DIA {
            e.terceros
                .entry(d.destino.sitio.clone())
                .or_insert_with(|| d.destino.quien().to_string());
        }
        if let Some(c) = &d.destino.corredor {
            e.corredores.insert(c.nombre.to_string());
        }
        if d.cortar {
            e.cortadas += 1;
            if e.cortados.len() < MAX_SITIOS_DIA {
                e.cortados.insert(d.destino.sitio.clone());
            }
        }
        let sigue = !d.pagina && (d.destino.sigue() || d.destino.corredor.is_some());
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

    /// Count one thing a page's pixel tried to tell, on `dia`: its network and event, nothing
    /// more.
    pub fn anota_chivato(&mut self, dia: &str, red: Red, evento: Evento) {
        let e = self.dias.entry(dia.to_string()).or_default();
        let n = e
            .chivatos
            .entry(red.clave().to_string())
            .or_default()
            .entry(evento.clave().to_string())
            .or_insert(0);
        *n = n.saturating_add(1);
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
    fn the_card_only_says_what_was_cut_for_every_company_it_names() {
        use crate::chivatos::{Chivato, Dato, Evento, Red};
        let c = |red: Red, evento: Evento, tuyo: bool| Chivato {
            red,
            evento,
            nombre: String::new(),
            importe: Some("1".into()),
            moneda: None,
            correo: Some(Dato {
                tuyo,
                cifrado: true,
            }),
            telefono: None,
            id: None,
            servidor: None,
            pagina: None,
        };
        let mut p = Pestana::nueva("tienda.co");
        assert!(p.chivato(c(Red::Meta, Evento::Compra, true), true));
        // Google twice is one company.
        assert!(p.chivato(c(Red::GoogleAds, Evento::Compra, true), true));
        assert!(p.chivato(c(Red::GoogleAnalytics, Evento::Compra, true), true));
        assert!(p.chivato(c(Red::Tiktok, Evento::VerPagina, false), true));
        let t = p.tarjeta();
        assert_eq!(
            t,
            Some(Tarjeta {
                empresas: 2,
                evento: Evento::Compra,
                correo: true
            })
        );
        // A company that got the purchase through is not «cut» on the card.
        assert!(p.chivato(c(Red::Snap, Evento::Compra, false), false));
        let t = p.tarjeta();
        assert_eq!(t.map(|t| (t.empresas, t.correo)), Some((2, true)));
        // The same again without an id is one more time, and if it got through once, it did.
        assert!(!p.chivato(c(Red::Meta, Evento::Compra, true), false));
        assert_eq!((p.chivatos[0].veces, p.chivatos[0].cortado), (2, false));
        assert_eq!(p.tarjeta().map(|t| t.empresas), Some(1));
        // And not everyone with the person's email: the card does not say «with my email».
        let mut q = Pestana::nueva("tienda.co");
        assert!(q.chivato(c(Red::Meta, Evento::Compra, true), true));
        assert!(q.chivato(c(Red::Pinterest, Evento::Compra, false), true));
        assert_eq!(q.tarjeta().map(|t| t.correo), Some(false));
        assert_eq!(Pestana::nueva("x.co").tarjeta(), None);
        // A beacon of the page before (its `dl` names another web) is not this page's story.
        let mut r = Pestana::nueva("tienda.co");
        let mut antes = c(Red::Meta, Evento::Compra, true);
        antes.pagina = Some("otra-tienda.co".into());
        assert!(r.chivato(antes, true));
        assert_eq!(r.tarjeta(), None);
        let mut aqui = c(Red::Tiktok, Evento::Compra, true);
        aqui.pagina = Some("tienda.co".into());
        assert!(r.chivato(aqui, true));
        assert_eq!(r.tarjeta().map(|t| t.empresas), Some(1));
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
