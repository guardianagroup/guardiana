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
use guardiana_core::rules::{cutting_rules, decide, RuleInput};
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
const MAX_DESTINOS: usize = 800;
/// Besides the antivirus and the browsers, how many other programs (the ones that move most).
const MAX_OTROS: usize = 128;

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
    /// The VPNs the traffic is leaving through now, by name (15 Oct 2026). `None` where it
    /// cannot be known (not Windows, or Windows did not answer): the page then says nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    vpn: Option<Vec<String>>,
}

/// The VPN answer, kept for a while: asking Windows takes about a second and the page asks
/// every five, so Windows is asked in its own thread and the page gets the last answer (none
/// the first time, which the page shows as nothing).
fn vpn_ahora() -> Option<Vec<String>> {
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};
    struct Estado {
        cuando: Option<Instant>,
        valor: Option<Vec<String>>,
        preguntando: bool,
    }
    static CACHE: OnceLock<Mutex<Estado>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| {
        Mutex::new(Estado {
            cuando: None,
            valor: None,
            preguntando: false,
        })
    });
    let Ok(mut c) = cache.lock() else {
        return None;
    };
    let caducado = c
        .cuando
        .is_none_or(|t| t.elapsed() > Duration::from_secs(120));
    if caducado && !c.preguntando {
        c.preguntando = true;
        std::thread::spawn(move || {
            let v = guardiana_apps::vpn_activas()
                .ok()
                .map(|l| l.into_iter().map(|x| x.nombre).collect::<Vec<_>>());
            if let Ok(mut c) = cache.lock() {
                c.valor = v;
                c.cuando = Some(Instant::now());
                c.preguntando = false;
            }
        });
    }
    c.valor.clone()
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
    /// What it does inside the computer now, as Windows counts it (1.0.24, antivirus only).
    #[serde(skip_serializing_if = "Option::is_none")]
    actividad: Option<ActividadAv>,
}

/// An antivirus inside the computer: processor, memory and input/output, measured each time the
/// page asks (every 5 s), as rates between two takes. Nothing estimated: until there are two
/// takes, the rates are absent and the page says it is measuring.
#[derive(Serialize, Clone, Default)]
struct ActividadAv {
    /// Its processes running now.
    procesos: u32,
    /// Share of the whole computer's processor in the last interval, 0–100.
    cpu: Option<f64>,
    /// Memory it holds in RAM now, bytes.
    memoria: u64,
    /// Bytes per second it read and wrote in the last interval.
    lee: Option<u64>,
    escribe: Option<u64>,
    /// The last five minutes, one value per take: processor % and bytes/s read + written.
    serie_cpu: Vec<f64>,
    serie_es: Vec<u64>,
}

/// The takes of each antivirus product: (time ms, cpu ms, read, written).
type Tomas = HashMap<String, std::collections::VecDeque<(i64, u64, u64, u64)>>;

fn actividad_antivirus(now: i64) -> HashMap<String, ActividadAv> {
    static TOMAS: std::sync::Mutex<Option<Tomas>> = std::sync::Mutex::new(None);
    let nombres: Vec<String> = antivirus::antivirus()
        .iter()
        .flat_map(|a| a.programas.iter().map(|p| (*p).to_owned()))
        .collect();
    let mut por_av: HashMap<String, (u32, u64, u64, u64, u64)> = HashMap::new();
    for a in guardiana_apps::actividad(&nombres) {
        if let Some(av) = antivirus::antivirus_de_programa(&a.nombre) {
            let e = por_av.entry(av.nombre.to_owned()).or_default();
            e.0 += a.procesos;
            e.1 += a.cpu_ms;
            e.2 += a.leido;
            e.3 += a.escrito;
            e.4 += a.memoria;
        }
    }
    let cpus = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
    let Ok(mut g) = TOMAS.lock() else {
        return HashMap::new();
    };
    let tomas = g.get_or_insert_with(HashMap::new);
    tomas.retain(|k, _| por_av.contains_key(k));
    let mut out = HashMap::new();
    for (av, (procesos, cpu_ms, lee, escribe, memoria)) in por_av {
        let t = tomas.entry(av.clone()).or_default();
        t.push_back((now, cpu_ms, lee, escribe));
        while t.len() > 61 || t.front().is_some_and(|x| now - x.0 > 6 * 60_000) {
            t.pop_front();
        }
        // Rates between consecutive takes; a counter that went back (a process restarted) gives
        // no rate for that step rather than a wrong one.
        let pasos: Vec<(f64, Option<u64>, Option<u64>)> = t
            .iter()
            .zip(t.iter().skip(1))
            .filter_map(|(a, b)| {
                let dt = b.0 - a.0;
                if dt <= 0 {
                    return None;
                }
                let dt_f = dt as f64;
                let cpu = b.1.checked_sub(a.1).map_or(0.0, |d| {
                    (d as f64 / (dt_f * cpus as f64) * 100.0).min(100.0)
                });
                let por_s = |x: u64, y: u64| {
                    y.checked_sub(x)
                        .map(|d| d * 1000 / u64::try_from(dt).unwrap_or(1).max(1))
                };
                Some((cpu, por_s(a.2, b.2), por_s(a.3, b.3)))
            })
            .collect();
        let ultimo = pasos.last().copied();
        out.insert(
            av,
            ActividadAv {
                procesos,
                cpu: ultimo.map(|u| (u.0 * 10.0).round() / 10.0),
                memoria,
                lee: ultimo.and_then(|u| u.1),
                escribe: ultimo.and_then(|u| u.2),
                serie_cpu: pasos.iter().map(|p| (p.0 * 10.0).round() / 10.0).collect(),
                serie_es: pasos
                    .iter()
                    .map(|p| p.1.unwrap_or(0) + p.2.unwrap_or(0))
                    .collect(),
            },
        );
    }
    out
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
    /// The company the name belongs to (`Microsoft`), from GUARDIANA's open list; empty when the
    /// list does not know it. Not the same as the network: a server on Amazon's network does not
    /// mean Amazon uses what is sent (13 Oct 2026).
    empresa: String,
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
    /// Every active rule that cuts the name (the deciding one included): «dejarlo pasar» undoes
    /// them all. Empty when `regla` is `None`.
    reglas: Vec<i64>,
    /// Who made the deciding rule (`usuario`, or «protección máxima»).
    regla_por: Option<String>,
    /// The name is cut by a category rule (the person cut a whole kind of traffic, or «Cortar
    /// todo lo que envía el antivirus»): undoing names does nothing; the category must go.
    regla_categoria: Option<String>,
    /// True when nothing left: the name was stopped, so `pais` is its company's seat.
    sede: bool,
    /// Where the server is, to the region or the city, when its own operator publishes it
    /// (1.0.24); `None` when only the country is known.
    lugar: Option<LugarServidor>,
    /// Who is behind the destination, how sure and why, apart from the network (1.0.45; fields
    /// documented in `identificacion.rs`).
    identidad: Option<guardiana_lists::atribucion::Atribucion>,
    /// Where its server is, as precisely as the evidence says (1.0.45). `None` for a name that
    /// was stopped before it left.
    #[serde(skip_serializing_if = "Option::is_none")]
    ubicacion: Option<guardiana_lists::ubicacion::Ubicacion>,
}

/// The place key most of a destination's bytes went to.
fn clave_lugar(lugares: &HashMap<String, u64>) -> Option<&String> {
    lugares
        .iter()
        .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
        .map(|(k, _)| k)
}

/// The place an operator publishes for its servers, as the page draws and tells it.
#[derive(Serialize)]
struct LugarServidor {
    /// `Virginia del Norte (Ashburn)`.
    ciudad: String,
    pais: String,
    lat: f64,
    lon: f64,
    /// `aws`, `gcp`, `azure`, `oracle`, `do`, `linode`, `vultr`.
    operador: String,
    /// The region's code (`us-east-1`), empty for a geofeed's city.
    region: String,
    /// `region` (a cloud region) or `geofeed` (a city the operator declares).
    fuente: &'static str,
}

fn lugar_servidor(lugares: &HashMap<String, u64>) -> Option<LugarServidor> {
    let (clave, _) = lugares
        .iter()
        .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))?;
    if let Some(c) = guardiana_lists::ciudad_desde_clave(clave) {
        return Some(LugarServidor {
            ciudad: c.ciudad,
            pais: c.pais,
            lat: c.lat,
            lon: c.lon,
            operador: "dbip".to_owned(),
            region: c.region,
            fuente: "base",
        });
    }
    let l = guardiana_lists::lugar_por_clave(clave)?;
    let geofeed = l.region.contains('|');
    Some(LugarServidor {
        ciudad: l.ciudad.clone(),
        pais: l.pais.clone(),
        lat: l.lat,
        lon: l.lon,
        operador: l.proveedor.clone(),
        region: if geofeed {
            String::new()
        } else {
            l.region.clone()
        },
        fuente: if geofeed { "geofeed" } else { "region" },
    })
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

fn entrada<'a>(nombre: &'a str, categoria: Category) -> RuleInput<'a> {
    RuleInput {
        device_id: SELF_DEVICE_ID,
        name: nombre,
        category: categoria,
        observed_ms: i64::MAX / 4,
    }
}

fn reglas_de(
    rules: &[guardiana_core::Rule],
    nombre: &str,
    categoria: Category,
    now: i64,
) -> Vec<i64> {
    if nombre.is_empty() {
        return Vec::new();
    }
    cutting_rules(rules, entrada(nombre, categoria), now)
        .into_iter()
        .map(|r| r.id)
        .collect()
}

fn regla_por(
    rules: &[guardiana_core::Rule],
    nombre: &str,
    categoria: Category,
    now: i64,
) -> Option<String> {
    if nombre.is_empty() {
        return None;
    }
    decide(rules, entrada(nombre, categoria), now)
        .filter(|r| r.action == Action::Cortar)
        .map(|r| r.created_by.clone())
}

fn regla_categoria(
    rules: &[guardiana_core::Rule],
    nombre: &str,
    categoria: Category,
    now: i64,
) -> Option<String> {
    if nombre.is_empty() {
        return None;
    }
    decide(rules, entrada(nombre, categoria), now)
        .filter(|r| r.action == Action::Cortar && r.match_kind == MatchKind::Category)
        .map(|r| r.pattern.clone())
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
    /// Bytes per place its operator published (`aws:us-east-1`).
    lugares: HashMap<String, u64>,
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
        if !f.lugar.is_empty() {
            *a.lugares.entry(f.lugar.clone()).or_default() += f.enviados + f.recibidos + 1;
        }
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
        let empresa = guardiana_lists::company_of(&destino)
            .unwrap_or("")
            .to_owned();
        destinos.push(Destino {
            identidad: Some(guardiana_lists::atribucion::atribuir(
                &guardiana_lists::atribucion::Pistas {
                    qname: &destino,
                    asn: a.asn,
                    ..Default::default()
                },
            )),
            ubicacion: Some(guardiana_lists::ubicacion::ubicacion_de_clave(
                clave_lugar(&a.lugares).map_or("", String::as_str),
                &pais,
            )),
            lugar: lugar_servidor(&a.lugares),
            empresa,
            regla: regla_de(&rules, &destino, categoria, now),
            reglas: reglas_de(&rules, &destino, categoria, now),
            regla_por: regla_por(&rules, &destino, categoria, now),
            regla_categoria: regla_categoria(&rules, &destino, categoria, now),
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
            reglas: reglas_de(&rules, nombre, *categoria, now),
            regla_por: regla_por(&rules, nombre, *categoria, now),
            regla_categoria: regla_categoria(&rules, nombre, *categoria, now),
            programa: prog.clone(),
            destino: nombre.clone(),
            pais,
            red: empresa.unwrap_or("").to_owned(),
            empresa: empresa.unwrap_or("").to_owned(),
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
            lugar: None,
            identidad: Some(guardiana_lists::atribucion::atribuir(
                &guardiana_lists::atribucion::Pistas {
                    qname: nombre,
                    ..Default::default()
                },
            )),
            ubicacion: None,
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
    // 1.0.45: the alias sites the resolver kept say whose service answered under a name.
    let alias = crate::identificacion::alias_de(
        &state,
        destinos
            .iter()
            .filter(|d| !d.destino.is_empty())
            .map(|d| d.destino.clone())
            .collect(),
    )?;
    for d in &mut destinos {
        if let Some(a) = alias.get(&d.destino) {
            let visto = guardiana_core::time::day_utc(a.last_seen);
            d.identidad = Some(guardiana_lists::atribucion::atribuir(
                &guardiana_lists::atribucion::Pistas {
                    qname: &d.destino,
                    alias: &a.sites,
                    alias_visto: Some(&visto),
                    asn: d.asn,
                },
            ));
        }
    }

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

    // An antivirus that is running is shown even when it sent nothing in the window: what it does
    // inside the computer is measured too (1.0.24).
    let mut actividad = actividad_antivirus(now);
    for av in actividad.keys() {
        progs.entry(av.clone()).or_insert_with(|| ProgAcc {
            tipo: "antivirus",
            procesos: antivirus::antivirus()
                .iter()
                .find(|a| a.nombre == av)
                .map(|a| a.programas.iter().map(|p| (*p).to_owned()).collect())
                .unwrap_or_default(),
            firma: String::new(),
            enviados: 0,
            recibidos: 0,
        });
    }
    let mut programas: Vec<Programa> = progs
        .into_iter()
        .filter(|(id, p)| vale(id, p.tipo))
        .map(|(id, p)| Programa {
            actividad: actividad.remove(&id),
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
        vpn: vpn_ahora(),
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

/// `/api/secure-en-directo` (16 Oct 2026): the GUARDIANA SECURE conversations open on this
/// computer right now, as GUARDIANA ZERO leaves them next to the ledger: the contact's name,
/// where each end is by time zone (never an address), and whether a call is on. Only this
/// computer's own panel reads it, behind its session; nothing is sent anywhere. A file older
/// than fifteen seconds is a ZERO that closed: nothing is open.
pub(crate) async fn secure_en_directo(
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<serde_json::Value> {
    let vacio = serde_json::json!({ "conversaciones": [] });
    let Some(carpeta) = state.db_path.parent() else {
        return Ok(axum::Json(vacio));
    };
    let leido = std::fs::read(carpeta.join("secure-en-directo.json"))
        .ok()
        .filter(|b| b.len() < 64 * 1024)
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok());
    let Some(v) = leido else {
        return Ok(axum::Json(vacio));
    };
    let ahora = (now_ms() / 1000) as u64;
    let fresco = v["actualizado"]
        .as_u64()
        .is_some_and(|t| ahora.saturating_sub(t) <= 15);
    if !fresco {
        return Ok(axum::Json(vacio));
    }
    // Only the fields the globe draws, whatever else the file holds.
    let conversaciones: Vec<serde_json::Value> = v["conversaciones"]
        .as_array()
        .map(|a| {
            a.iter()
                .take(8)
                .map(|c| {
                    serde_json::json!({
                        "contacto": c["contacto"].as_str().unwrap_or_default().chars().take(60).collect::<String>(),
                        "zona": c["zona"].as_str().unwrap_or_default().chars().take(64).collect::<String>(),
                        "llamada": c["llamada"].clone(),
                        "rele": c["rele"].as_bool().unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(axum::Json(serde_json::json!({
        "zona": v["zona"].as_str().unwrap_or_default().chars().take(64).collect::<String>(),
        "conversaciones": conversaciones,
    })))
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
