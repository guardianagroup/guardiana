//! «GUARDIANA por dentro» (1.0.22): what the watched programs of this computer send and receive,
//! for the live globe. The owner asked for a view that shows «cómo trabaja GUARDIANA dentro del
//! ordenador», with the antivirus and the browser and «tantos datos como seas capaz», live.
//!
//! Only measurements and facts, never guesses: per program and destination, the bytes, packets,
//! protocol and port Windows counted (the traffic channel of 1.0.21); the country and network of
//! the server from the open tables; whether the maker documents the name (antivirus); how many
//! times the program asked for the name and how many of those GUARDIANA cut. The server's address
//! is never stored nor sent (decision 203). A name stopped before it left has no server: it is
//! placed at its company's seat, and the answer says so.

// The handler answers with the same error type as the rest of the API (see api.rs).
#![allow(clippy::result_large_err)]

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::State;
use guardiana_core::rules::{decide, RuleInput};
use guardiana_core::time::{now_ms, HOUR_MS};
use guardiana_core::{Action, Category, MatchKind, SELF_DEVICE_ID};
use guardiana_lists::antivirus;
use serde::Serialize;

use crate::api::{with_ledger, ApiResult};
use crate::auth::{Session, Zona};
use crate::AppState;

/// How many hours of measurements the globe adds up (the free plan keeps a day of detail).
const VENTANA_H: i64 = 24;
/// The latest minutes, row by row, for the live particles and the log.
const RECIENTES_MIN: i64 = 3;
/// The most destinations sent; the totals count all of them.
const MAX_DESTINOS: usize = 400;
/// Besides the antivirus and the browsers, how many other programs (the ones that move most).
const MAX_OTROS: usize = 8;

/// Browsers by their process file name (lowercase), with the name people know.
const NAVEGADORES: &[(&str, &str)] = &[
    ("chrome.exe", "Chrome"),
    ("msedge.exe", "Edge"),
    ("firefox.exe", "Firefox"),
    ("brave.exe", "Brave"),
    ("opera.exe", "Opera"),
    ("vivaldi.exe", "Vivaldi"),
    ("arc.exe", "Arc"),
    ("chromium.exe", "Chromium"),
    ("librewolf.exe", "LibreWolf"),
    ("waterfox.exe", "Waterfox"),
    ("iexplore.exe", "Internet Explorer"),
    ("guardiana zero.exe", "GUARDIANA ZERO"),
    ("guardiana-zero.exe", "GUARDIANA ZERO"),
];

#[derive(Serialize)]
pub(crate) struct Universo {
    programas: Vec<Programa>,
    destinos: Vec<Destino>,
    recientes: Vec<Reciente>,
    /// Destinations left out for being too many (they still count in no total here).
    destinos_omitidos: usize,
    /// Programs left out (beyond the antivirus, the browsers and the ones that move most).
    programas_omitidos: usize,
    /// Cutting a whole antivirus is available here (Windows, as the system).
    cortar_disponible: bool,
    /// GUARDIANA is counting what the antivirus sends.
    vigilando: bool,
    /// «Cortar todo lo que envía el antivirus» is on (called «Protección máxima del antivirus»
    /// until 13 Oct 2026; the field keeps its name).
    maxima: bool,
    /// This system measures the bytes of each program (Windows' network channel, 1.0.22).
    /// Elsewhere nothing will ever arrive, and the page says so instead of waiting.
    mide_bytes: bool,
    ventana_h: i64,
    ahora: i64,
}

#[derive(Serialize)]
struct Programa {
    /// Stable key: the product (`Microsoft Defender`), the browser (`Chrome`) or the file name.
    id: String,
    tipo: &'static str,
    /// The file names seen for it.
    procesos: Vec<String>,
    /// Who signs it, as Windows said; empty when unknown.
    firma: String,
    /// Its whole outgoing traffic can be cut in the firewall (only an antivirus, by design).
    cortable: bool,
    cortado: bool,
    enviados: u64,
    recibidos: u64,
}

#[derive(Serialize)]
struct Destino {
    programa: String,
    /// The name asked for this server, empty when none was seen.
    destino: String,
    /// Two-letter country of the server; a mark (`@local`, `@anycast`); or, for a name stopped
    /// before leaving, the country of its company's seat (see `sede`).
    pais: String,
    /// The network that announces the address (`Amazon.com, Inc.`), or the company of the name.
    red: String,
    asn: u32,
    /// The way most of its bytes went (`tcp`/`udp` and the port at the other end).
    proto: String,
    puerto: u16,
    /// Every way seen, most bytes first (`udp/443`).
    vias: Vec<String>,
    enviados: u64,
    recibidos: u64,
    paquetes: u64,
    /// Of the bytes sent, between midnight and six in the person's day.
    noche: u64,
    primera: i64,
    ultima: i64,
    /// Antivirus only: whether the maker documents the name, and what for.
    documentado: Option<bool>,
    uso: Option<&'static str>,
    /// Times the program asked for the name in the window, and of those, cut.
    consultas: u64,
    paradas: u64,
    /// The active rule of GUARDIANA that cuts the name on this computer, to undo it.
    regla: Option<i64>,
    /// True when nothing left: the name was stopped, so `pais` is its company's seat.
    sede: bool,
}

#[derive(Serialize)]
struct Reciente {
    minuto: i64,
    programa: String,
    destino: String,
    pais: String,
    red: String,
    proto: String,
    puerto: u16,
    enviados: u64,
    recibidos: u64,
    paquetes: u64,
}

/// Which watched program a process belongs to: an antivirus product, a browser, or itself.
fn programa_de(proceso: &str) -> (String, &'static str) {
    if let Some(av) = antivirus::antivirus_de_programa(proceso) {
        return (av.nombre.to_owned(), "antivirus");
    }
    let archivo = proceso
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(proceso)
        .to_ascii_lowercase();
    if let Some((_, n)) = NAVEGADORES.iter().find(|(p, _)| *p == archivo) {
        return ((*n).to_owned(), "navegador");
    }
    (
        proceso
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(proceso)
            .to_owned(),
        "programa",
    )
}

/// The rule that cuts `nombre` on this computer right now, when it is one picked for names (not a
/// whole category, which the person undoes elsewhere).
fn regla_de(
    rules: &[guardiana_core::Rule],
    nombre: &str,
    categoria: Category,
    now: i64,
) -> Option<i64> {
    if nombre.is_empty() {
        return None;
    }
    decide(
        rules,
        RuleInput {
            device_id: SELF_DEVICE_ID,
            name: nombre,
            category: categoria,
            observed_ms: i64::MAX / 4,
        },
        now,
    )
    .filter(|r| r.action == Action::Cortar && r.match_kind != MatchKind::Category)
    .map(|r| r.id)
}

#[derive(Default)]
struct Acc {
    enviados: u64,
    recibidos: u64,
    paquetes: u64,
    noche: u64,
    primera: i64,
    ultima: i64,
    asn: u32,
    vias: HashMap<(String, u16), u64>,
}

/// `/api/por-dentro`: the live universe of this computer.
pub(crate) async fn universo(
    Zona(zona): Zona,
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<Universo> {
    let now = now_ms();
    let desde = now - VENTANA_H * HOUR_MS;
    let (filas, nombres, rules, vigilando, con_ruta) = with_ledger(&state, |l| {
        Ok((
            l.trafico(desde, now + 1, &[])?,
            l.program_names(desde, now + 1, zona)?,
            l.rules()?,
            l.vigilando_antivirus(),
            crate::antivirus::CortesAntivirus::rutas_por_producto(l)
                .into_iter()
                .map(|(p, _)| p)
                .collect::<Vec<_>>(),
        ))
    })?;
    let cortados = state.cortes.cortados();
    let maxima = !con_ruta.is_empty() && con_ruta.iter().all(|p| cortados.iter().any(|c| c == p));

    // Programs: who moved what.
    struct ProgAcc {
        tipo: &'static str,
        procesos: Vec<String>,
        firma: String,
        enviados: u64,
        recibidos: u64,
    }
    let mut progs: HashMap<String, ProgAcc> = HashMap::new();
    let mut por_destino: HashMap<(String, String, String, String), Acc> = HashMap::new();
    for f in &filas {
        let (id, tipo) = programa_de(&f.programa);
        let p = progs.entry(id.clone()).or_insert_with(|| ProgAcc {
            tipo,
            procesos: Vec::new(),
            firma: String::new(),
            enviados: 0,
            recibidos: 0,
        });
        let archivo = f
            .programa
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(&f.programa)
            .to_owned();
        if !p.procesos.contains(&archivo) {
            p.procesos.push(archivo);
        }
        if p.firma.is_empty() && !f.firma.is_empty() {
            p.firma.clone_from(&f.firma);
        }
        p.enviados += f.enviados;
        p.recibidos += f.recibidos;
        // A country, or the network's mark when the address has none (local, anycast).
        let pais = if f.pais.is_empty() && f.red.starts_with('@') {
            f.red.clone()
        } else {
            f.pais.clone()
        };
        let a = por_destino
            .entry((id, f.destino.clone(), pais, f.red.clone()))
            .or_default();
        a.enviados += f.enviados;
        a.recibidos += f.recibidos;
        a.paquetes += f.paquetes;
        if crate::antivirus::es_noche(f.minuto, zona) {
            a.noche += f.enviados;
        }
        a.primera = if a.primera == 0 {
            f.minuto
        } else {
            a.primera.min(f.minuto)
        };
        a.ultima = a.ultima.max(f.minuto);
        a.asn = a.asn.max(f.asn);
        *a.vias.entry((f.proto.clone(), f.puerto)).or_default() += f.enviados + f.recibidos;
    }

    // The DNS side: how many times each program asked for each name, and how many were cut.
    let mut dns: HashMap<(String, String), (u64, u64, Category, i64)> = HashMap::new();
    for n in nombres.iter().filter(|n| n.device_id == SELF_DEVICE_ID) {
        let (id, tipo) = programa_de(&n.program);
        let e = dns
            .entry((id.clone(), n.qname.clone()))
            .or_insert((0, 0, n.category, 0));
        e.0 += n.queries;
        e.1 += n.cut;
        e.3 = e.3.max(n.last_ts);
        progs.entry(id).or_insert_with(|| ProgAcc {
            tipo,
            procesos: vec![n
                .program
                .rsplit(['\\', '/'])
                .next()
                .unwrap_or(&n.program)
                .to_owned()],
            firma: n.signer.clone(),
            enviados: 0,
            recibidos: 0,
        });
    }

    // Keep the antivirus and the browsers, and the other programs that moved most.
    let mut otros: Vec<(&String, u64)> = progs
        .iter()
        .filter(|(_, p)| p.tipo == "programa")
        .map(|(k, p)| (k, p.enviados + p.recibidos))
        .collect();
    otros.sort_by_key(|(k, b)| (std::cmp::Reverse(*b), (*k).clone()));
    let quedan: Vec<String> = otros
        .iter()
        .take(MAX_OTROS)
        .map(|(k, _)| (*k).clone())
        .collect();
    let programas_omitidos = otros.len().saturating_sub(MAX_OTROS);
    let vale = |id: &str, tipo: &str| tipo != "programa" || quedan.iter().any(|q| q == id);

    let mut destinos: Vec<Destino> = Vec::new();
    let mut con_trafico: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();
    for ((prog, destino, pais, red), a) in por_destino {
        let tipo = progs.get(&prog).map_or("programa", |p| p.tipo);
        if !vale(&prog, tipo) {
            continue;
        }
        let mut vias: Vec<((String, u16), u64)> = a.vias.into_iter().collect();
        vias.sort_by_key(|(k, b)| (std::cmp::Reverse(*b), k.clone()));
        let (proto, puerto) = vias.first().map(|(k, _)| k.clone()).unwrap_or_default();
        let (consultas, paradas, categoria, _) = dns
            .get(&(prog.clone(), destino.clone()))
            .copied()
            .unwrap_or((0, 0, Category::Desconocido, 0));
        let (documentado, uso) = documentado(&prog, tipo, &destino);
        con_trafico.insert((prog.clone(), destino.clone()));
        destinos.push(Destino {
            regla: regla_de(&rules, &destino, categoria, now),
            programa: prog,
            destino,
            pais,
            red,
            asn: a.asn,
            proto,
            puerto,
            vias: vias
                .iter()
                .take(3)
                .map(|((p, n), _)| format!("{p}/{n}"))
                .collect(),
            enviados: a.enviados,
            recibidos: a.recibidos,
            paquetes: a.paquetes,
            noche: a.noche,
            primera: a.primera,
            ultima: a.ultima,
            documentado,
            uso,
            consultas,
            paradas,
            sede: false,
        });
    }
    // Names stopped before they left: no server, so the company's seat, said as such.
    for ((prog, nombre), (consultas, paradas, categoria, ultima)) in &dns {
        if *paradas == 0 || con_trafico.contains(&(prog.clone(), nombre.clone())) {
            continue;
        }
        let tipo = progs.get(prog).map_or("programa", |p| p.tipo);
        if !vale(prog, tipo) {
            continue;
        }
        let empresa = guardiana_lists::company_of(nombre);
        let pais = empresa
            .and_then(guardiana_lists::company_record)
            .and_then(|r| r.country)
            .unwrap_or("")
            .to_owned();
        let (documentado, uso) = documentado(prog, tipo, nombre);
        destinos.push(Destino {
            regla: regla_de(&rules, nombre, *categoria, now),
            programa: prog.clone(),
            destino: nombre.clone(),
            pais,
            red: empresa.unwrap_or("").to_owned(),
            asn: 0,
            proto: String::new(),
            puerto: 0,
            vias: Vec::new(),
            enviados: 0,
            recibidos: 0,
            paquetes: 0,
            noche: 0,
            primera: *ultima,
            ultima: *ultima,
            documentado,
            uso,
            consultas: *consultas,
            paradas: *paradas,
            sede: true,
        });
    }
    destinos.sort_by(|a, b| {
        (b.enviados + b.recibidos)
            .cmp(&(a.enviados + a.recibidos))
            .then(b.paradas.cmp(&a.paradas))
            .then(a.destino.cmp(&b.destino))
    });
    let destinos_omitidos = destinos.len().saturating_sub(MAX_DESTINOS);
    destinos.truncate(MAX_DESTINOS);

    let recientes: Vec<Reciente> = filas
        .iter()
        .filter(|f| f.minuto >= now - RECIENTES_MIN * 60_000)
        .filter_map(|f| {
            let (id, tipo) = programa_de(&f.programa);
            if !vale(&id, tipo) {
                return None;
            }
            Some(Reciente {
                minuto: f.minuto,
                programa: id,
                destino: f.destino.clone(),
                pais: if f.pais.is_empty() && f.red.starts_with('@') {
                    f.red.clone()
                } else {
                    f.pais.clone()
                },
                red: f.red.clone(),
                proto: f.proto.clone(),
                puerto: f.puerto,
                enviados: f.enviados,
                recibidos: f.recibidos,
                paquetes: f.paquetes,
            })
        })
        .collect();

    let mut programas: Vec<Programa> = progs
        .into_iter()
        .filter(|(id, p)| vale(id, p.tipo))
        .map(|(id, p)| Programa {
            cortable: p.tipo == "antivirus",
            cortado: cortados.iter().any(|c| c == &id),
            id,
            tipo: p.tipo,
            procesos: p.procesos,
            firma: p.firma,
            enviados: p.enviados,
            recibidos: p.recibidos,
        })
        .collect();
    // The antivirus first, then the browsers, then the rest; inside, most bytes first.
    let orden = |t: &str| match t {
        "antivirus" => 0,
        "navegador" => 1,
        _ => 2,
    };
    programas.sort_by(|a, b| {
        orden(a.tipo)
            .cmp(&orden(b.tipo))
            .then((b.enviados + b.recibidos).cmp(&(a.enviados + a.recibidos)))
            .then(a.id.cmp(&b.id))
    });

    Ok(axum::Json(Universo {
        programas,
        destinos,
        recientes,
        destinos_omitidos,
        programas_omitidos,
        cortar_disponible: state.cortes.disponible(),
        vigilando,
        maxima,
        mide_bytes: cfg!(windows),
        ventana_h: VENTANA_H,
        ahora: now,
    }))
}

/// For an antivirus, whether its maker documents the name and what for; for anything else, the
/// question does not apply.
fn documentado(prog: &str, tipo: &str, nombre: &str) -> (Option<bool>, Option<&'static str>) {
    if tipo != "antivirus" || nombre.is_empty() {
        return (None, None);
    }
    let Some(av) = antivirus::antivirus().iter().find(|a| a.nombre == prog) else {
        return (None, None);
    };
    match antivirus::uso_documentado(av, nombre) {
        Some(d) => (Some(true), Some(d.uso)),
        None => (Some(false), None),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_process_belongs_to_its_antivirus_its_browser_or_itself() {
        assert_eq!(
            programa_de("MsMpEng.exe"),
            ("Microsoft Defender".to_owned(), "antivirus")
        );
        assert_eq!(
            programa_de("C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe"),
            ("Chrome".to_owned(), "navegador")
        );
        assert_eq!(
            programa_de("GUARDIANA ZERO.exe"),
            ("GUARDIANA ZERO".to_owned(), "navegador")
        );
        assert_eq!(
            programa_de("Spotify.exe"),
            ("Spotify.exe".to_owned(), "programa")
        );
    }
}
