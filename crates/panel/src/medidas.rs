//! The weekly report's measurements (decision 205), worked out here from rows the ledger
//! already keeps; nothing new is stored for them:
//!
//! - **Mientras dormías**: per device, queries to tracker, advertising and telemetry names between
//!   00:00 and 06:00 local time, and the first of those each morning after 06:00.
//! - **Empresas que pudieron saber que estabas conectado**: distinct company groups each device
//!   asked for (and was not cut) on each day.
//! - **Cada cuánto llaman**: for the busiest trackers, the median interval between contacts,
//!   folded exactly as the beacon signal folds them; «llama cada» only when that signal fired.
//! - **Nombres disfrazados**: the names that answered as another site's tracker.
//!
//! Only the household's rows, as everywhere a name is shown: this computer and the devices whose
//! owner shares their detail. A device that does not share is not in any of it.

use std::collections::{BTreeMap, HashMap};

use guardiana_core::i18n::Texts;
use guardiana_core::time::HOUR_MS;
use guardiana_core::{local_day_index, local_midnight, CloakedCount, NameDay, TrackerRow};
use serde::Serialize;

use crate::destinos::grupo_de;

/// Days the measurements cover: today and the six before, whole local days.
pub(crate) const DIAS: i64 = 7;
/// The night: from 00:00 to 06:00 local time.
const FIN_NOCHE_H: i64 = 6;
/// The morning, for the first tracker of the day: from 06:00 to 12:00 local time.
const FIN_MANANA_H: i64 = 12;
/// How many trackers the cadence table names.
const TOP_CADENCIAS: usize = 8;
/// How many companies a device's night or a device's day names.
const TOP_EMPRESAS: usize = 3;
/// Fewer intervals than this and there is no «typical» interval to speak of.
const MIN_INTERVALOS: usize = 3;

/// The window of the measurements: local midnight six days ago until `ahora`.
#[must_use]
pub(crate) fn ventana(ahora: i64, zona_min: i64) -> (i64, i64) {
    let hoy = local_day_index(ahora, zona_min);
    (local_midnight(hoy - (DIAS - 1), zona_min), ahora)
}

/// One device's nights.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub(crate) struct NocheAparato {
    pub(crate) id: String,
    pub(crate) name: Option<String>,
    /// Queries to tracker, advertising or telemetry names between 00:00 and 06:00.
    pub(crate) consultas: i64,
    /// Of those, the ones GUARDIANA cut (they never left the house).
    pub(crate) cortadas: i64,
    /// Nights with at least one.
    pub(crate) noches: i64,
    /// The groups most asked for at night, at most three.
    pub(crate) empresas: Vec<&'static str>,
    /// Per day of [`Medidas::dias`], minutes after local midnight of the first query to such a
    /// name between 06:00 and 12:00; `None` when there was none.
    pub(crate) mananas: Vec<Option<i64>>,
}

/// One device's companies per day.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub(crate) struct EmpresasAparato {
    pub(crate) id: String,
    pub(crate) name: Option<String>,
    /// Distinct groups per day of [`Medidas::dias`].
    pub(crate) por_dia: Vec<i64>,
    /// Today's, most asked first.
    pub(crate) hoy: Vec<&'static str>,
}

/// How often one tracker calls.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub(crate) struct Cadencia {
    pub(crate) nombre: String,
    pub(crate) empresa: Option<&'static str>,
    pub(crate) categoria: String,
    /// Contacts in the window, folded as the beacon signal folds them (A and AAAA together).
    pub(crate) contactos: i64,
    /// Median interval between contacts of the same device, pooled over the devices.
    pub(crate) mediana_ms: Option<i64>,
    /// Whether the beacon signal fired for it in the window: «llama cada» is said only then.
    pub(crate) baliza: bool,
    /// Contacts between 00:00 and 06:00.
    pub(crate) noche: i64,
    /// The sentence the row shows.
    pub(crate) frase: String,
}

/// A name that answered as another site's tracker, as the report shows it.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub(crate) struct Disfrazado {
    pub(crate) nombre: String,
    pub(crate) sitio: String,
    pub(crate) empresa: Option<&'static str>,
    pub(crate) consultas: i64,
    pub(crate) aparatos: i64,
    pub(crate) frase: String,
}

/// All of it, for `/api/informe`.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub(crate) struct Medidas {
    /// Minutes east of UTC the days were cut with.
    pub(crate) zona_min: i64,
    pub(crate) desde: i64,
    pub(crate) hasta: i64,
    /// Unix ms of each local midnight, oldest first; the last one is today.
    pub(crate) dias: Vec<i64>,
    pub(crate) noche: Vec<NocheAparato>,
    /// The household's night queries, all devices together.
    pub(crate) noche_total: i64,
    pub(crate) empresas: Vec<EmpresasAparato>,
    /// Groups any of the household's devices asked for today, most asked first.
    pub(crate) empresas_hoy: Vec<&'static str>,
    pub(crate) cadencias: Vec<Cadencia>,
    pub(crate) disfrazados: Vec<Disfrazado>,
    pub(crate) disfrazados_consultas: i64,
}

/// «~5 min», «~3 h», «menos de 1 min».
pub(crate) fn duracion(t: &Texts, ms: i64) -> String {
    let min = (ms + 30_000) / 60_000;
    if ms < 60_000 {
        t.panel("dur_menos_min").to_owned()
    } else if min < 90 {
        t.panel("dur_min").replace("{n}", &min.to_string())
    } else {
        let h = (ms + HOUR_MS / 2) / HOUR_MS;
        t.panel("dur_h").replace("{n}", &h.to_string())
    }
}

/// Who answers behind a disguised name: the company of `empresas.txt` that owns the site.
fn empresa_del_sitio(sitio: &str) -> Option<&'static str> {
    crate::api::empresa_de(sitio)
}

/// The sentence of a disguised name: «se presenta como metricas.ejemplo.com, pero responde Adobe
/// (omtrdc.net)».
pub(crate) fn frase_disfraz(t: &Texts, nombre: &str, sitio: &str) -> String {
    match empresa_del_sitio(sitio) {
        Some(e) => t
            .panel("disfraz_frase")
            .replace("{nombre}", nombre)
            .replace("{empresa}", e)
            .replace("{sitio}", sitio),
        None => t
            .panel("disfraz_frase_sin_empresa")
            .replace("{nombre}", nombre)
            .replace("{sitio}", sitio),
    }
}

fn hora_local(ts: i64, zona_min: i64) -> (i64, i64) {
    let dia = local_day_index(ts, zona_min);
    (dia, (ts - local_midnight(dia, zona_min)) / 60_000)
}

/// The group a query adds up to: the name's own, or, for a disguised name, the group of the
/// tracker that answered behind it (decision 205).
fn grupo_o_disfraz(qname: &str, target: &str) -> Option<&'static str> {
    grupo_de(qname).or_else(|| {
        (!target.is_empty())
            .then(|| guardiana_lists::group_or_company_of(target))
            .flatten()
    })
}

/// The groups of a count, most first, then by name; at most `n`.
fn primeras(mut v: Vec<(&'static str, i64)>, n: usize) -> Vec<&'static str> {
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    v.into_iter().take(n).map(|(g, _)| g).collect()
}

/// Work everything out from the ledger's rows. `nombres` names the devices.
pub(crate) fn calcula(
    t: &Texts,
    ahora: i64,
    zona_min: i64,
    rastreo: &[TrackerRow],
    por_dia: &[NameDay],
    disfraz: &[CloakedCount],
    nombres: &HashMap<String, Option<String>>,
) -> Medidas {
    let (desde, hasta) = ventana(ahora, zona_min);
    let hoy = local_day_index(ahora, zona_min);
    let primero = hoy - (DIAS - 1);
    let dias: Vec<i64> = (primero..=hoy)
        .map(|d| local_midnight(d, zona_min))
        .collect();
    let pos = |dia: i64| {
        usize::try_from(dia - primero)
            .ok()
            .filter(|&i| i < dias.len())
    };

    // ----- the nights and the mornings, per device
    let mut noche: BTreeMap<&str, NocheAparato> = BTreeMap::new();
    let mut empresas_noche: HashMap<&str, HashMap<&'static str, i64>> = HashMap::new();
    let mut noches_vistas: HashMap<&str, std::collections::BTreeSet<i64>> = HashMap::new();
    for r in rastreo {
        let (dia, minuto) = hora_local(r.ts, zona_min);
        let Some(i) = pos(dia) else { continue };
        let n = noche
            .entry(r.device_id.as_str())
            .or_insert_with(|| NocheAparato {
                id: r.device_id.clone(),
                name: nombres.get(&r.device_id).cloned().flatten(),
                consultas: 0,
                cortadas: 0,
                noches: 0,
                empresas: Vec::new(),
                mananas: vec![None; dias.len()],
            });
        if minuto < FIN_NOCHE_H * 60 {
            n.consultas += 1;
            if r.cut {
                n.cortadas += 1;
            }
            noches_vistas
                .entry(r.device_id.as_str())
                .or_default()
                .insert(dia);
            if let Some(g) = grupo_o_disfraz(&r.qname, &r.target) {
                *empresas_noche
                    .entry(r.device_id.as_str())
                    .or_default()
                    .entry(g)
                    .or_insert(0) += 1;
            }
        } else if minuto < FIN_MANANA_H * 60 {
            let m = &mut n.mananas[i];
            if m.is_none_or(|x| minuto < x) {
                *m = Some(minuto);
            }
        }
    }
    for (id, n) in &mut noche {
        n.noches = noches_vistas
            .get(id)
            .map_or(0, |s| i64::try_from(s.len()).unwrap_or(0));
        n.empresas = empresas_noche
            .remove(id)
            .map(|m| primeras(m.into_iter().collect(), TOP_EMPRESAS))
            .unwrap_or_default();
    }
    let noche: Vec<NocheAparato> = noche.into_values().collect();
    let noche_total = noche.iter().map(|n| n.consultas).sum();

    // ----- companies per day, per device
    let mut grupos: BTreeMap<&str, Vec<HashMap<&'static str, i64>>> = BTreeMap::new();
    let mut hoy_casa: HashMap<&'static str, i64> = HashMap::new();
    for f in por_dia {
        let (Some(i), Some(g)) = (pos(f.day), grupo_o_disfraz(&f.qname, &f.target)) else {
            continue;
        };
        if guardiana_lists::is_local_name(&f.qname) {
            continue;
        }
        let v = grupos
            .entry(f.device_id.as_str())
            .or_insert_with(|| vec![HashMap::new(); dias.len()]);
        *v[i].entry(g).or_insert(0) += f.queries;
        if f.day == hoy {
            *hoy_casa.entry(g).or_insert(0) += f.queries;
        }
    }
    let empresas: Vec<EmpresasAparato> = grupos
        .into_iter()
        .map(|(id, v)| EmpresasAparato {
            id: id.to_owned(),
            name: nombres.get(id).cloned().flatten(),
            por_dia: v
                .iter()
                .map(|m| i64::try_from(m.len()).unwrap_or(0))
                .collect(),
            hoy: v
                .last()
                .map(|m| primeras(m.iter().map(|(g, n)| (*g, *n)).collect(), usize::MAX))
                .unwrap_or_default(),
        })
        .collect();
    let empresas_hoy = primeras(hoy_casa.into_iter().collect(), usize::MAX);

    // ----- how often the busiest trackers call
    let mut series: HashMap<(&str, &str), Vec<i64>> = HashMap::new();
    let mut baliza: HashMap<&str, bool> = HashMap::new();
    let mut categoria: HashMap<&str, &str> = HashMap::new();
    let mut disfraz_de: HashMap<&str, &str> = HashMap::new();
    for r in rastreo {
        if pos(local_day_index(r.ts, zona_min)).is_none() {
            continue;
        }
        series
            .entry((r.qname.as_str(), r.device_id.as_str()))
            .or_default()
            .push(r.ts);
        *baliza.entry(r.qname.as_str()).or_insert(false) |= r.beacon_minutes.is_some();
        categoria
            .entry(r.qname.as_str())
            .or_insert(r.category.as_str());
        if !r.target.is_empty() {
            disfraz_de.insert(r.qname.as_str(), r.target.as_str());
        }
    }
    // Per name: contacts, contacts at night, and every interval of every device's own series.
    let mut por_nombre: HashMap<&str, (i64, i64, Vec<i64>)> = HashMap::new();
    for ((nombre, _), tiempos) in &series {
        let e = por_nombre.entry(nombre).or_insert((0, 0, Vec::new()));
        let mut ultimo: Option<i64> = None;
        for &ts in tiempos {
            if ultimo.is_some_and(|u| guardiana_classify::same_contact(u, ts)) {
                continue;
            }
            ultimo = Some(ts);
            e.0 += 1;
            if hora_local(ts, zona_min).1 < FIN_NOCHE_H * 60 {
                e.1 += 1;
            }
        }
        e.2.extend(guardiana_classify::contact_intervals(tiempos));
    }
    let mut cadencias: Vec<Cadencia> = por_nombre
        .into_iter()
        .map(|(nombre, (contactos, de_noche, mut iv))| {
            iv.sort_unstable();
            let mediana_ms = (iv.len() >= MIN_INTERVALOS)
                .then(|| guardiana_classify::median_of_sorted(&iv))
                .flatten();
            let es_baliza = baliza.get(nombre).copied().unwrap_or(false);
            let mut frase = match mediana_ms {
                Some(ms) if es_baliza => t.panel("cadencia_cada").replace("{t}", &duracion(t, ms)),
                Some(ms) => t.panel("cadencia_mediana").replace("{t}", &duracion(t, ms)),
                None => t.panel_n("cadencia_pocos", contactos),
            };
            if de_noche > 0 {
                frase.push_str(t.panel("cadencia_noche"));
            }
            Cadencia {
                nombre: nombre.to_owned(),
                // A disguised name is the company that answered behind it.
                empresa: crate::api::empresa_de(nombre)
                    .or_else(|| disfraz_de.get(nombre).and_then(|s| empresa_del_sitio(s))),
                categoria: categoria.get(nombre).copied().unwrap_or("").to_owned(),
                contactos,
                mediana_ms,
                baliza: es_baliza,
                noche: de_noche,
                frase,
            }
        })
        .collect();
    cadencias.sort_by(|a, b| b.contactos.cmp(&a.contactos).then(a.nombre.cmp(&b.nombre)));
    cadencias.truncate(TOP_CADENCIAS);

    // ----- the disguised names
    let disfrazados: Vec<Disfrazado> = disfraz
        .iter()
        .map(|d| Disfrazado {
            frase: frase_disfraz(t, &d.qname, &d.target),
            empresa: empresa_del_sitio(&d.target),
            nombre: d.qname.clone(),
            sitio: d.target.clone(),
            consultas: d.queries,
            aparatos: d.devices,
        })
        .collect();
    let disfrazados_consultas = disfrazados.iter().map(|d| d.consultas).sum();

    Medidas {
        zona_min,
        desde,
        hasta,
        dias,
        noche,
        noche_total,
        empresas,
        empresas_hoy,
        cadencias,
        disfrazados,
        disfrazados_consultas,
    }
}

/// The sentences for the report's text to share, only when there is something to say: the
/// household's night queries, and the disguised names. Short, and without a single name of a
/// device or a site.
pub(crate) fn frases_para_compartir(t: &Texts, m: &Medidas) -> Vec<String> {
    let mut out = Vec::new();
    if m.noche_total > 0 {
        out.push(t.panel_n("compartir_noche", m.noche_total));
    }
    if !m.disfrazados.is_empty() {
        out.push(t.panel_n(
            "compartir_disfrazados",
            i64::try_from(m.disfrazados.len()).unwrap_or(i64::MAX),
        ));
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use guardiana_core::i18n;
    use guardiana_core::time::DAY_MS;

    const M: i64 = 60_000;

    fn fila(dev: &str, ts: i64, q: &str, baliza: bool) -> TrackerRow {
        TrackerRow {
            device_id: dev.to_owned(),
            ts,
            qname: q.to_owned(),
            category: "rastreador".to_owned(),
            cut: false,
            beacon_minutes: baliza.then_some(5),
            target: String::new(),
        }
    }

    #[test]
    fn durations_read_like_a_person_says_them() {
        let t = i18n::es();
        assert_eq!(duracion(t, 20_000), "menos de 1 min");
        assert_eq!(duracion(t, 5 * M + 10_000), "~5 min");
        assert_eq!(duracion(t, 3 * HOUR_MS + 10 * M), "~3 h");
    }

    #[test]
    fn nights_mornings_companies_and_cadence_from_rows() {
        let t = i18n::es();
        let zona = -300; // Bogotá
                         // Today, 15:00 in Bogotá.
        let hoy = local_day_index(1_800_000_000_000, zona);
        let medianoche = local_midnight(hoy, zona);
        let ahora = medianoche + 15 * HOUR_MS;
        let ayer = medianoche - DAY_MS;
        let mut rastreo = Vec::new();
        // Last night, every five minutes from 01:00 to 02:00, A and AAAA: 13 contacts, beacon.
        for i in 0..13 {
            let ts = ayer + DAY_MS + HOUR_MS + i * 5 * M;
            rastreo.push(fila("self", ts, "pixel.tracker.test", i > 4));
            rastreo.push(fila("self", ts + 1, "pixel.tracker.test", false));
        }
        // The morning: 07:12 and 06:40 yesterday, 08:05 today.
        rastreo.push(fila(
            "self",
            ayer + 7 * HOUR_MS + 12 * M,
            "graph.facebook.com",
            false,
        ));
        rastreo.push(fila(
            "self",
            ayer + 6 * HOUR_MS + 40 * M,
            "graph.facebook.com",
            false,
        ));
        rastreo.push(fila(
            "self",
            medianoche + 8 * HOUR_MS + 5 * M,
            "graph.facebook.com",
            false,
        ));
        // Outside the window: eight days ago.
        rastreo.push(fila(
            "self",
            medianoche - 8 * DAY_MS,
            "viejo.tracker.test",
            false,
        ));
        rastreo.sort_by_key(|r| r.ts);
        let dia = |offset: i64| local_day_index(medianoche + offset * DAY_MS + HOUR_MS, zona);
        let por_dia = vec![
            NameDay {
                device_id: "self".into(),
                day: dia(0),
                qname: "graph.facebook.com".into(),
                target: String::new(),
                queries: 4,
            },
            NameDay {
                device_id: "self".into(),
                day: dia(0),
                qname: "www.youtube.com".into(),
                target: String::new(),
                queries: 2,
            },
            NameDay {
                device_id: "self".into(),
                day: dia(0),
                qname: "e1.dsce9.akamaiedge.net".into(),
                target: String::new(),
                queries: 9,
            },
            NameDay {
                device_id: "self".into(),
                day: dia(-1),
                qname: "metricas.tienda.example".into(),
                target: "omtrdc.net".into(),
                queries: 1,
            },
        ];
        let m = calcula(t, ahora, zona, &rastreo, &por_dia, &[], &HashMap::new());
        assert_eq!(m.dias.len(), 7);
        assert_eq!(*m.dias.last().unwrap(), medianoche);
        let n = &m.noche[0];
        assert_eq!(n.consultas, 26, "every query counts, A and AAAA alike");
        assert_eq!(n.noches, 1);
        assert_eq!(n.mananas[6], Some(8 * 60 + 5));
        assert_eq!(
            n.mananas[5],
            Some(6 * 60 + 40),
            "the earliest of the morning"
        );
        assert_eq!(n.mananas[4], None);
        assert_eq!(m.noche_total, 26);
        // Companies: today Meta and Alphabet (the delivery network is nobody); yesterday Adobe,
        // behind a disguised name.
        let e = &m.empresas[0];
        assert_eq!(e.por_dia[6], 2);
        assert_eq!(e.por_dia[5], 1);
        assert_eq!(m.empresas[0].por_dia.len(), 7);
        assert_eq!(e.hoy, vec!["Meta", "Alphabet"]);
        assert_eq!(m.empresas_hoy, vec!["Meta", "Alphabet"]);
        // Cadence: the pixel, 13 contacts, every five minutes, at night, with its beacon.
        let c = &m.cadencias[0];
        assert_eq!(c.nombre, "pixel.tracker.test");
        assert_eq!(c.contactos, 13);
        assert_eq!(c.mediana_ms, Some(5 * M));
        assert!(c.baliza);
        assert_eq!(c.frase, "llama cada ~5 min, también de noche");
        // Facebook: three contacts, two intervals, too few to speak of a typical one.
        let fb = m
            .cadencias
            .iter()
            .find(|c| c.nombre == "graph.facebook.com")
            .unwrap();
        assert_eq!(fb.mediana_ms, None);
        assert_eq!(fb.frase, "3 contactos en estos 7 días");
        assert!(!m.cadencias.iter().any(|c| c.nombre == "viejo.tracker.test"));
    }

    #[test]
    fn a_disguise_says_who_answers() {
        let t = i18n::es();
        assert_eq!(
            frase_disfraz(t, "metricas.ejemplo.com", "omtrdc.net"),
            "se presenta como metricas.ejemplo.com, pero responde Adobe (omtrdc.net)"
        );
        assert_eq!(
            frase_disfraz(t, "m.ejemplo.com", "medidor.example"),
            "se presenta como m.ejemplo.com, pero responde medidor.example, que las listas abiertas dan como rastreo"
        );
    }
}
