//! JSON API behind the pages. Every number comes straight from the ledger.

// Handlers return a full HTTP `Response` as their error, as axum intends; the
// size of that type is not worth boxing on a loopback panel.
#![allow(clippy::result_large_err)]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::Json;
use guardiana_core::i18n::{self, Texts};
use guardiana_core::rules::{observation_complete, observed_hours};
use guardiana_core::time::now_ms;
use guardiana_core::ChangeKind;
use guardiana_core::{
    write_csv, write_json, Action, Category, DecidedBy, Device, Event, EventFilter, Ledger,
    MatchKind, NewRule, Retention, Rule, Scope, Signal, Verdict, SELF_DEVICE_ID,
};
use guardiana_service::home::{self, SETTING_HOME_IP, SETTING_HOME_MODE, SETTING_HOME_SINCE};
use guardiana_service::sysdns::SETTING_BACKUP;
use serde::{Deserialize, Serialize};

use crate::auth::{Lang, Session};
use crate::AppState;

type ApiResult<T> = Result<Json<T>, Response>;

fn internal(e: impl std::fmt::Display) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
}

fn with_ledger<T>(
    state: &AppState,
    f: impl FnOnce(&mut Ledger) -> guardiana_core::Result<T>,
) -> Result<T, Response> {
    let mut guard = state
        .ledger
        .lock()
        .map_err(|_| internal("ledger lock poisoned"))?;
    f(&mut guard).map_err(internal)
}

/// An event as the pages show it: with device name and the signal sentences.
#[derive(Serialize)]
pub(crate) struct EventView {
    id: i64,
    ts: i64,
    qname: String,
    qtype: String,
    category: String,
    signals: Vec<Signal>,
    verdict: String,
    decided_by: String,
    rule_id: Option<i64>,
    device_id: String,
    device_name: Option<String>,
    frases: Vec<String>,
    /// Who owns the name, from the "empresas" list (decision 61); a label, never a verdict.
    empresa: Option<&'static str>,
    /// The artificial-intelligence service the name belongs to (decision 62), or `None`.
    ia: Option<&'static str>,
}

fn view(t: &Texts, e: Event, names: &HashMap<String, Option<String>>) -> EventView {
    EventView {
        frases: e.signals.iter().map(|s| t.signal(s)).collect(),
        device_name: names.get(&e.device_id).cloned().flatten(),
        empresa: guardiana_lists::company_of(&e.qname),
        ia: guardiana_lists::ai_service_of(&e.qname),
        id: e.id,
        ts: e.ts,
        qname: e.qname,
        qtype: e.qtype,
        category: e.category.as_str().to_owned(),
        signals: e.signals,
        verdict: e.verdict.as_str().to_owned(),
        decided_by: e.decided_by.as_str().to_owned(),
        rule_id: e.rule_id,
        device_id: e.device_id,
    }
}

fn names_of(devices: &[Device]) -> HashMap<String, Option<String>> {
    devices
        .iter()
        .map(|d| (d.id.clone(), d.name.clone()))
        .collect()
}

fn filter_from(q: &HashMap<String, String>) -> Result<EventFilter, Response> {
    let bad = |e: guardiana_core::Error| (StatusCode::BAD_REQUEST, e.to_string()).into_response();
    Ok(EventFilter {
        device_id: q.get("device_id").filter(|s| !s.is_empty()).cloned(),
        category: q
            .get("category")
            .filter(|s| !s.is_empty())
            .map(|s| s.parse())
            .transpose()
            .map_err(bad)?,
        signal: q
            .get("signal")
            .filter(|s| !s.is_empty())
            .map(|s| s.parse())
            .transpose()
            .map_err(bad)?,
        verdict: q
            .get("verdict")
            .filter(|s| !s.is_empty())
            .map(|s| s.parse())
            .transpose()
            .map_err(bad)?,
        since: q.get("since").and_then(|s| s.parse().ok()),
        until: q.get("until").and_then(|s| s.parse().ok()),
        limit: q.get("limit").and_then(|s| s.parse().ok()),
    })
}

// ----- public (no token) ----------------------------------------------------

/// The whole texts file, so pages and CLI share every sentence.
pub(crate) async fn textos(Lang(t): Lang) -> Response {
    let body = if std::ptr::eq(t, i18n::en()) {
        i18n::EN_JSON
    } else {
        i18n::ES_JSON
    };
    (
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        body,
    )
        .into_response()
}

/// The calling device's own record, by IP. Loopback is this computer.
pub(crate) async fn me(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
) -> ApiResult<Option<Device>> {
    let id = if peer.ip().is_loopback() {
        SELF_DEVICE_ID.to_owned()
    } else {
        format!("ip:{}", peer.ip())
    };
    Ok(Json(with_ledger(&state, |l| l.device(&id))?))
}

// ----- home panel (token) ---------------------------------------------------

#[derive(Serialize)]
pub(crate) struct DnsCambio {
    dns_aplicado: bool,
    mensaje: String,
}

/// Point this PC's system DNS at Guardiana (primary 127.0.0.1, the previous
/// resolver as secondary), keeping an exact backup first (brief §4). The
/// panel runs inside the service, so no extra rights are needed.
pub(crate) async fn dns_aplicar(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<DnsCambio> {
    use guardiana_service::sysdns;
    let already =
        with_ledger(&state, |l| l.setting(SETTING_BACKUP))?.is_some_and(|v| !v.is_empty());
    if already {
        return Ok(Json(DnsCambio {
            dns_aplicado: true,
            mensaje: t.cli("dns.ya_aplicado").to_owned(),
        }));
    }
    let st = state.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let backup = match sysdns::snapshot(now_ms()) {
            Ok(b) => b,
            Err(sysdns::Error::Unsupported) => return Err(t.cli("dns.no_soportado").to_owned()),
            Err(e) => return Err(e.to_string()),
        };
        let json = serde_json::to_string(&backup).map_err(|e| e.to_string())?;
        {
            let l = st
                .ledger
                .lock()
                .map_err(|_| "ledger lock poisoned".to_owned())?;
            l.set_setting(SETTING_BACKUP, &json)
                .map_err(|e| e.to_string())?;
        }
        let guardian = std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);
        if let Err(e) = sysdns::apply(&backup, guardian) {
            let _ = sysdns::restore(&backup);
            if let Ok(l) = st.ledger.lock() {
                let _ = l.set_setting(SETTING_BACKUP, "");
            }
            return Err(e.to_string());
        }
        let originals = backup
            .original_servers()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        if let Ok(l) = st.ledger.lock() {
            let _ = l.record_change(now_ms(), ChangeKind::DnsOn, "panel", &originals);
        }
        let mut msg = t
            .cli("dns.aplicado_panel")
            .replace("{originales}", &originals);
        msg.push(' ');
        msg.push_str(
            t.cli(if guardiana_service::sysdns::guardian_is_sole_resolver() {
                "dns.solo_guardiana"
            } else {
                "dns.reserva_secundario"
            }),
        );
        if cfg!(target_os = "windows") {
            msg.push(' ');
            msg.push_str(t.cli("dns.limite_windows"));
        }
        Ok(msg)
    })
    .await
    .map_err(internal)?;
    match result {
        Ok(mensaje) => Ok(Json(DnsCambio {
            dns_aplicado: true,
            mensaje,
        })),
        Err(e) => Err((StatusCode::CONFLICT, e).into_response()),
    }
}

/// Put the system DNS back exactly as it was.
pub(crate) async fn dns_restaurar(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<DnsCambio> {
    use guardiana_service::sysdns;
    let stored = with_ledger(&state, |l| l.setting(SETTING_BACKUP))?.filter(|v| !v.is_empty());
    let Some(json) = stored else {
        return Ok(Json(DnsCambio {
            dns_aplicado: false,
            mensaje: t.cli("dns.no_hay_copia").to_owned(),
        }));
    };
    let backup: sysdns::Backup = serde_json::from_str(&json).map_err(internal)?;
    let st = state.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<(), String> {
        sysdns::restore(&backup).map_err(|e| e.to_string())?;
        let l = st
            .ledger
            .lock()
            .map_err(|_| "ledger lock poisoned".to_owned())?;
        l.set_setting(SETTING_BACKUP, "")
            .map_err(|e| e.to_string())?;
        l.record_change(now_ms(), ChangeKind::DnsOff, "panel", "")
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(internal)?;
    match result {
        Ok(()) => Ok(Json(DnsCambio {
            dns_aplicado: false,
            mensaje: t.cli("dns.restaurado").to_owned(),
        })),
        Err(e) => Err((StatusCode::CONFLICT, e).into_response()),
    }
}

#[derive(Serialize)]
pub(crate) struct Estado {
    version: String,
    clave_dev: bool,
    escucha: Vec<String>,
    upstream: Vec<String>,
    dns_aplicado: bool,
    listas: Vec<ListaInfo>,
    panel_puerto: u16,
}

#[derive(Serialize)]
struct ListaInfo {
    id: String,
    entries: u64,
    fetched: String,
}

pub(crate) async fn estado(State(state): State<Arc<AppState>>, _s: Session) -> ApiResult<Estado> {
    // Applied by Guardiana (a backup exists) or pointed here by other means (the
    // system says 127.0.0.1 is the primary): either way this PC passes through it.
    let mut dns_aplicado =
        with_ledger(&state, |l| l.setting(SETTING_BACKUP))?.is_some_and(|v| !v.is_empty());
    if !dns_aplicado {
        dns_aplicado = tokio::task::spawn_blocking(guardiana_service::sysdns::guardian_is_primary)
            .await
            .map_err(internal)?
            == Some(true);
    }
    let manifest = guardiana_lists::manifest().map_err(internal)?;
    let mut listas: Vec<ListaInfo> = manifest
        .lists
        .iter()
        .map(|l| ListaInfo {
            id: l.id.clone(),
            entries: l.entries,
            fetched: manifest.fetched.clone(),
        })
        .collect();
    listas.push(ListaInfo {
        id: guardiana_lists::OWN_SOURCE_ESPERADO.to_owned(),
        entries: guardiana_lists::parse_guardiana(guardiana_lists::ESPERADO).count() as u64,
        fetched: String::new(),
    });
    listas.push(ListaInfo {
        id: guardiana_lists::OWN_SOURCE_TELEMETRIA.to_owned(),
        entries: guardiana_lists::parse_guardiana(guardiana_lists::TELEMETRIA).count() as u64,
        fetched: String::new(),
    });
    Ok(Json(Estado {
        version: state.info.version.clone(),
        clave_dev: state.info.dev_key,
        escucha: state.info.listen_dns.clone(),
        upstream: state.info.upstream.clone(),
        dns_aplicado,
        listas,
        panel_puerto: crate::DEFAULT_PORT,
    }))
}

#[derive(Serialize)]
pub(crate) struct Radiografia {
    desde: i64,
    hasta: i64,
    servicios: i64,
    rastreadores: i64,
    destinos_nuevos: i64,
    esperados: i64,
    cortados: i64,
    eventos: Vec<EventView>,
    /// Latest period (last 7 days) in which the service was not watching.
    hueco: Option<HuecoView>,
}

/// A gap in the watch, for the panel.
#[derive(Serialize)]
pub(crate) struct HuecoView {
    desde: i64,
    hasta: i64,
}

#[derive(Serialize)]
pub(crate) struct CambioView {
    ts: i64,
    que: String,
    quien: String,
    detalle: String,
}

/// The changes Guardiana made on someone's order (Home Mode, system DNS), newest first.
pub(crate) async fn cambios(
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<Vec<CambioView>> {
    let v = with_ledger(&state, |l| l.changes(50))?;
    Ok(Json(
        v.into_iter()
            .map(|c| CambioView {
                ts: c.ts,
                que: c.kind.as_str().to_owned(),
                quien: c.who,
                detalle: c.detail,
            })
            .collect(),
    ))
}

/// What a device's last 24 hours say beyond the totals (decision 61): the companies behind its
/// names, whether it went silent while the house kept talking, and whether it checks a service
/// that would take its traffic out of Guardiana's sight.
#[derive(Serialize, Default)]
pub(crate) struct Lectura {
    /// (company, queries) of the last 24 h, most first, at most eight.
    empresas: Vec<(String, u64)>,
    /// Distinct companies in the last 24 h.
    empresas_total: usize,
    /// Minutes since its last query when it has been quiet for a while but the house has not.
    callado_min: Option<i64>,
    /// Queried iCloud Private Relay names in the last 24 h.
    relay: bool,
    /// Queries carrying the DNS-evasion signal in the last 24 h.
    evasiones: u64,
}

const SILENCE_MIN: i64 = 30;

fn lectura(
    l: &Ledger,
    id: &str,
    last_seen: i64,
    house_last: i64,
    now: i64,
) -> guardiana_core::Result<Lectura> {
    let events = l.events(&EventFilter {
        device_id: Some(id.to_owned()),
        since: Some(now - 24 * guardiana_core::time::HOUR_MS),
        limit: Some(5000),
        ..EventFilter::default()
    })?;
    let mut by_company: HashMap<&'static str, u64> = HashMap::new();
    let mut relay = false;
    let mut evasiones = 0;
    for e in &events {
        if let Some(c) = guardiana_lists::company_of(&e.qname) {
            *by_company.entry(c).or_insert(0) += 1;
        }
        let q = e.qname.to_ascii_lowercase();
        if q == "mask.icloud.com"
            || q == "mask-api.icloud.com"
            || q.ends_with(".mask.icloud.com")
            || q == "mask.apple-dns.net"
            || q == "mask-api.fe2.apple-dns.net"
        {
            relay = true;
        }
        if e.signals.iter().any(|s| s.as_str() == "evasion_dns") {
            evasiones += 1;
        }
    }
    let mut empresas: Vec<(String, u64)> = by_company
        .into_iter()
        .map(|(c, n)| (c.to_owned(), n))
        .collect();
    empresas.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let empresas_total = empresas.len();
    empresas.truncate(8);
    let quiet_for = (now - last_seen) / 60_000;
    let callado_min =
        (!events.is_empty() && quiet_for >= SILENCE_MIN && now - house_last < 5 * 60_000)
            .then_some(quiet_for);
    Ok(Lectura {
        empresas,
        empresas_total,
        callado_min,
        relay,
        evasiones,
    })
}

// ----- the AI layer (decision 62) -------------------------------------------

/// Settings key holding the destinations declared for a device, one per line.
fn scope_key(device_id: &str) -> String {
    format!("alcance:{device_id}")
}

/// A name is inside the declared scope when it equals a pattern or hangs below it.
fn in_scope(patterns: &[String], qname: &str) -> bool {
    let name = qname.trim_end_matches('.').to_ascii_lowercase();
    patterns.iter().any(|p| {
        let p = p.trim().trim_start_matches("*.").trim_end_matches('.');
        !p.is_empty() && (name == p || name.ends_with(&format!(".{p}")))
    })
}

#[derive(Serialize)]
pub(crate) struct IaServicio {
    servicio: &'static str,
    device_id: String,
    device_name: Option<String>,
    nombres: usize,
    consultas: u64,
    ultima: i64,
}

#[derive(Serialize)]
pub(crate) struct AlcanceView {
    device_id: String,
    device_name: Option<String>,
    /// What the person declared this device may talk to, one per line.
    patrones: Vec<String>,
    /// Names seen in the last 24 h outside that list, most-queried first, at most twenty.
    fuera: Vec<(String, u64)>,
    /// How many distinct names went beyond the declared list.
    fuera_total: usize,
    /// Distinct names inside it.
    dentro_total: usize,
}

#[derive(Serialize)]
pub(crate) struct IaView {
    /// AI services seen per device in the last seven days.
    servicios: Vec<IaServicio>,
    /// One entry per device that has a declared scope.
    alcances: Vec<AlcanceView>,
    /// Devices with no declared scope yet, to offer it.
    sin_alcance: Vec<(String, Option<String>)>,
}

/// What the AI page shows: which artificial-intelligence services each device talks to, and
/// whether each device stays inside the destinations its owner declared. Guardiana checks where
/// an agent talks, never what it reads: that limit is written on the page itself.
pub(crate) async fn ia(State(state): State<Arc<AppState>>, _s: Session) -> ApiResult<IaView> {
    let now = now_ms();
    let view = with_ledger(&state, |l| {
        let devices = l.devices()?;
        // Per (device, AI service): how many queries for each distinct name, and the last time.
        type PorServicio = HashMap<(String, &'static str), (HashMap<String, u64>, i64)>;
        let mut servicios: PorServicio = HashMap::new();
        let mut alcances = Vec::new();
        let mut sin_alcance = Vec::new();
        for d in &devices {
            let week = l.events(&EventFilter {
                device_id: Some(d.id.clone()),
                since: Some(now - 7 * 24 * guardiana_core::time::HOUR_MS),
                limit: Some(20_000),
                ..EventFilter::default()
            })?;
            for e in &week {
                if let Some(servicio) = guardiana_lists::ai_service_of(&e.qname) {
                    let slot = servicios
                        .entry((d.id.clone(), servicio))
                        .or_insert_with(|| (HashMap::new(), 0));
                    *slot.0.entry(e.qname.clone()).or_insert(0) += 1;
                    slot.1 = slot.1.max(e.ts);
                }
            }
            let patrones: Vec<String> = l
                .setting(&scope_key(&d.id))?
                .unwrap_or_default()
                .lines()
                .map(|x| x.trim().to_owned())
                .filter(|x| !x.is_empty())
                .collect();
            if patrones.is_empty() {
                sin_alcance.push((d.id.clone(), d.name.clone()));
                continue;
            }
            // Expected traffic (updates, time, resolvers, messaging) never counts as "beyond":
            // it is the machine keeping itself alive, not the agent going somewhere else.
            let mut fuera: HashMap<String, u64> = HashMap::new();
            let mut dentro: std::collections::HashSet<String> = std::collections::HashSet::new();
            for e in week
                .iter()
                .filter(|e| e.ts >= now - 24 * guardiana_core::time::HOUR_MS)
                .filter(|e| e.category != Category::Esperado)
            {
                if in_scope(&patrones, &e.qname) {
                    dentro.insert(e.qname.clone());
                } else {
                    *fuera.entry(e.qname.clone()).or_insert(0) += 1;
                }
            }
            let fuera_total = fuera.len();
            let mut fuera: Vec<(String, u64)> = fuera.into_iter().collect();
            fuera.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            fuera.truncate(20);
            alcances.push(AlcanceView {
                device_id: d.id.clone(),
                device_name: d.name.clone(),
                patrones,
                fuera,
                fuera_total,
                dentro_total: dentro.len(),
            });
        }
        let names = names_of(&devices);
        let mut servicios: Vec<IaServicio> = servicios
            .into_iter()
            .map(|((device_id, servicio), (nombres, ultima))| IaServicio {
                servicio,
                device_name: names.get(&device_id).cloned().flatten(),
                device_id,
                nombres: nombres.len(),
                consultas: nombres.values().sum(),
                ultima,
            })
            .collect();
        servicios.sort_by_key(|s| std::cmp::Reverse(s.ultima));
        Ok(IaView {
            servicios,
            alcances,
            sin_alcance,
        })
    })?;
    Ok(Json(view))
}

#[derive(Deserialize)]
pub(crate) struct AlcanceBody {
    device_id: String,
    /// The declared destinations as typed, one per line. Empty clears the declaration.
    patrones: String,
}

/// Declare (or clear) what a device may talk to. Nothing is cut: Guardiana reports what went
/// beyond, and cutting stays a separate, explicit decision in Rules.
pub(crate) async fn alcance(
    State(state): State<Arc<AppState>>,
    _s: Session,
    Json(body): Json<AlcanceBody>,
) -> ApiResult<IaView> {
    if body.patrones.len() > 8_000 {
        return Err((StatusCode::BAD_REQUEST, "scope too long").into_response());
    }
    let cleaned: Vec<String> = body
        .patrones
        .lines()
        .map(|x| {
            x.trim()
                .trim_start_matches("*.")
                .trim_end_matches('.')
                .to_ascii_lowercase()
        })
        .filter(|x| !x.is_empty() && x.contains('.') && !x.contains(' ') && !x.contains('/'))
        .collect();
    with_ledger(&state, |l| {
        l.set_setting(&scope_key(&body.device_id), &cleaned.join("\n"))
    })?;
    ia(State(state), _s).await
}

/// Gaps newer than `since`, newest first, at most `limit`.
fn huecos(l: &Ledger, since: i64, limit: usize) -> guardiana_core::Result<Vec<HuecoView>> {
    Ok(l.gaps()?
        .into_iter()
        .filter(|g| g.to_ts >= since)
        .take(limit)
        .map(|g| HuecoView {
            desde: g.from_ts,
            hasta: g.to_ts,
        })
        .collect())
}

pub(crate) async fn radiografia(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Radiografia> {
    let seconds: i64 = q.get("segundos").and_then(|s| s.parse().ok()).unwrap_or(60);
    let until = now_ms();
    let since = until - seconds.clamp(1, 3600) * 1000;
    let (c, events, devices, hueco) = with_ledger(&state, |l| {
        let c = l.counters(since, until)?;
        let events = l.events(&EventFilter {
            since: Some(since),
            limit: Some(50),
            ..EventFilter::default()
        })?;
        let hueco = huecos(l, until - 7 * 24 * 3600 * 1000, 1)?.pop();
        Ok((c, events, l.devices()?, hueco))
    })?;
    let names = names_of(&devices);
    let mut eventos: Vec<EventView> = events.into_iter().map(|e| view(t, e, &names)).collect();
    eventos.reverse();
    Ok(Json(Radiografia {
        hueco,
        desde: since,
        hasta: until,
        servicios: c.services,
        rastreadores: c.trackers,
        destinos_nuevos: c.new_destinations,
        esperados: c.expected,
        cortados: c.blocked,
        eventos,
    }))
}

#[derive(Serialize)]
pub(crate) struct Extracto {
    total: u64,
    eventos: Vec<EventView>,
    /// Periods in which the service was not watching, newest first.
    huecos: Vec<HuecoView>,
}

pub(crate) async fn extracto(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Extracto> {
    let mut filter = filter_from(&q)?;
    if filter.limit.is_none() {
        filter.limit = Some(200);
    }
    let (events, total, devices, huecos) = with_ledger(&state, |l| {
        Ok((
            l.events(&filter)?,
            l.event_count()?,
            l.devices()?,
            huecos(l, 0, 20)?,
        ))
    })?;
    let names = names_of(&devices);
    let mut eventos: Vec<EventView> = events.into_iter().map(|e| view(t, e, &names)).collect();
    eventos.reverse();
    Ok(Json(Extracto {
        total,
        eventos,
        huecos,
    }))
}

#[derive(Serialize)]
pub(crate) struct Comprobacion {
    ok: bool,
    checked: u64,
    pruned: u64,
    anchor_is_genesis: bool,
    fallo: Option<Fallo>,
}

#[derive(Serialize)]
struct Fallo {
    id: i64,
    motivo: String,
}

pub(crate) async fn comprobar(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<Comprobacion> {
    let r = with_ledger(&state, |l| l.check())?;
    let fallo = r.first_fault.as_ref().map(|(id, f)| Fallo {
        id: *id,
        motivo: match f {
            guardiana_core::ChainFault::BrokenLink { .. } => t.cli("ledger.fallo.enlace"),
            guardiana_core::ChainFault::AlteredRow { .. } => t.cli("ledger.fallo.alterado"),
            guardiana_core::ChainFault::Unreadable(_) => t.cli("ledger.fallo.ilegible"),
        }
        .to_owned(),
    });
    Ok(Json(Comprobacion {
        ok: r.is_ok(),
        checked: r.checked,
        pruned: r.pruned,
        anchor_is_genesis: r.anchor_is_genesis,
        fallo,
    }))
}

pub(crate) async fn exportar(
    State(state): State<Arc<AppState>>,
    _s: Session,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Response, Response> {
    let filter = filter_from(&q)?;
    let events = with_ledger(&state, |l| l.events(&filter))?;
    let json = q.get("formato").is_some_and(|f| f == "json");
    let mut body = Vec::new();
    if json {
        write_json(&events, &mut body).map_err(internal)?;
    } else {
        write_csv(&events, &mut body).map_err(internal)?;
    }
    let (ctype, name) = if json {
        ("application/json; charset=utf-8", "guardiana-extracto.json")
    } else {
        ("text/csv; charset=utf-8", "guardiana-extracto.csv")
    };
    Ok((
        [
            (header::CONTENT_TYPE, ctype.to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{name}\""),
            ),
        ],
        body,
    )
        .into_response())
}

#[derive(Serialize)]
pub(crate) struct DispositivoView {
    id: String,
    name: Option<String>,
    last_ip: Option<String>,
    first_seen: i64,
    last_seen: i64,
    share_detail_with_home: bool,
    detalle_visible: bool,
    /// Hours observed so far, capped at 24 (brief §6).
    horas_observadas: i64,
    /// Whether cut rules may be created for this device.
    puede_cortar: bool,
    totales: Totales,
    lectura: Lectura,
}

#[derive(Serialize, Default)]
struct Totales {
    consultas: i64,
    rastreadores: i64,
    publicidad: i64,
    telemetria: i64,
    esperados: i64,
    desconocidos: i64,
    cortados: i64,
}

pub(crate) async fn dispositivos(
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<Vec<DispositivoView>> {
    let now = now_ms();
    let (devices, totals, lecturas) = with_ledger(&state, |l| {
        let devices = l.devices()?;
        let totals = l.device_totals(None)?;
        let house_last = devices.iter().map(|d| d.last_seen).max().unwrap_or(0);
        let mut lecturas = HashMap::new();
        for d in &devices {
            lecturas.insert(
                d.id.clone(),
                lectura(l, &d.id, d.last_seen, house_last, now)?,
            );
        }
        Ok((devices, totals, lecturas))
    })?;
    let mut lecturas = lecturas;
    let out = devices
        .into_iter()
        .map(|d| {
            let t = totals.iter().find(|t| t.device_id == d.id);
            let lectura = lecturas.remove(&d.id).unwrap_or_default();
            let totales = t.map_or_else(Totales::default, |t| Totales {
                consultas: t.queries,
                rastreadores: t.rastreador,
                publicidad: t.publicidad,
                telemetria: t.telemetria,
                esperados: t.esperado,
                desconocidos: t.desconocido,
                cortados: t.cortado,
            });
            let observed = now_ms() - d.first_seen;
            DispositivoView {
                detalle_visible: d.id == SELF_DEVICE_ID || d.share_detail_with_home,
                horas_observadas: observed_hours(observed),
                puede_cortar: observation_complete(observed),
                id: d.id,
                name: d.name,
                last_ip: d.last_ip,
                first_seen: d.first_seen,
                last_seen: d.last_seen,
                share_detail_with_home: d.share_detail_with_home,
                totales,
                lectura,
            }
        })
        .collect();
    Ok(Json(out))
}

#[derive(Deserialize)]
pub(crate) struct Nombre {
    name: String,
}

pub(crate) async fn renombrar(
    State(state): State<Arc<AppState>>,
    _s: Session,
    Path(id): Path<String>,
    Json(body): Json<Nombre>,
) -> ApiResult<Option<Device>> {
    let name = body.name.trim().to_owned();
    if name.is_empty() || name.chars().count() > 60 {
        return Err((StatusCode::BAD_REQUEST, "name must be 1-60 characters").into_response());
    }
    Ok(Json(with_ledger(&state, |l| {
        l.rename_device(&id, &name)?;
        l.device(&id)
    })?))
}

#[derive(Serialize)]
pub(crate) struct SabeDeTi {
    eventos: i64,
    dispositivos: i64,
    reglas: i64,
    nombres_vistos: i64,
    ruta: String,
    retencion: String,
    outbound: Vec<OutboundView>,
}

#[derive(Serialize)]
struct OutboundView {
    ts: i64,
    purpose: String,
    host: String,
    bytes: i64,
    initiated_by_user: bool,
}

pub(crate) async fn sabe_de_ti(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<SabeDeTi> {
    let (counts, outbound, plus) = with_ledger(&state, |l| {
        let plus = guardiana_license::status(l, &state.token, now_ms())
            .map(|s| s.plus_activo)
            .unwrap_or(false);
        Ok((l.table_counts()?, l.outbound()?, plus))
    })?;
    let retention_key = if plus || Retention::FREE.detail_ms.is_none() {
        "retencion_ilimitada"
    } else {
        "retencion_gratis"
    };
    Ok(Json(SabeDeTi {
        eventos: counts.events,
        dispositivos: counts.devices,
        reglas: counts.rules,
        nombres_vistos: counts.seen_domains,
        ruta: state.db_path.display().to_string(),
        retencion: t.panel(retention_key).to_owned(),
        outbound: outbound
            .into_iter()
            .map(|o| OutboundView {
                ts: o.ts,
                purpose: o.purpose.as_str().to_owned(),
                host: o.host,
                bytes: o.bytes,
                initiated_by_user: o.initiated_by_user,
            })
            .collect(),
    }))
}

#[derive(Deserialize)]
pub(crate) struct Confirmacion {
    confirmacion: String,
}

pub(crate) async fn borrar(
    State(state): State<Arc<AppState>>,
    _s: Session,
    Json(body): Json<Confirmacion>,
) -> ApiResult<TableCountsView> {
    if body.confirmacion != "BORRAR" {
        return Err((StatusCode::BAD_REQUEST, "confirmation word required").into_response());
    }
    let counts = with_ledger(&state, |l| {
        l.wipe()?;
        l.table_counts()
    })?;
    Ok(Json(TableCountsView {
        eventos: counts.events,
        dispositivos: counts.devices,
    }))
}

#[derive(Serialize)]
pub(crate) struct TableCountsView {
    eventos: i64,
    dispositivos: i64,
}

// Keep the enum imports used for documentation of stored values.
#[allow(dead_code)]
const _: (Verdict, DecidedBy) = (Verdict::Observado, DecidedBy::Nadie);

// ----- pages that depend on the Host ----------------------------------------

/// `/`: the radiography for the computer; the checker page when reached
/// through `comprobar.guardiana.hogar` (brief §4).
pub(crate) async fn root_page(headers: HeaderMap) -> Html<&'static str> {
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if host.starts_with(crate::CHECKER_HOST) {
        Html(include_str!("../static/comprobador.html"))
    } else {
        Html(include_str!("../static/index.html"))
    }
}

// ----- Home Mode (token) ----------------------------------------------------

#[derive(Serialize)]
pub(crate) struct Hogar {
    encendido: bool,
    ip: Option<String>,
    desde: Option<i64>,
    ip_dinamica: Option<bool>,
    /// Minutes before the system suspends itself (mains, battery); 0 = never.
    suspension: Option<(u32, u32)>,
    url: Option<String>,
    qr_svg: Option<String>,
    escuchando_en_lan: bool,
    dispositivos: usize,
    licencia: String,
}

fn qr_svg(text: &str) -> Option<String> {
    let code = qrcode::QrCode::new(text.as_bytes()).ok()?;
    Some(
        code.render::<qrcode::render::svg::Color<'_>>()
            .min_dimensions(180, 180)
            .quiet_zone(true)
            .build(),
    )
}

fn hogar_view(state: &AppState) -> Result<Hogar, Response> {
    let (on, ip, since, devices) = with_ledger(state, |l| {
        Ok((
            l.setting(SETTING_HOME_MODE)?.is_some_and(|v| v == "1"),
            l.setting(SETTING_HOME_IP)?.filter(|v| !v.is_empty()),
            l.setting(SETTING_HOME_SINCE)?
                .and_then(|v| v.parse::<i64>().ok()),
            l.devices()?.len(),
        ))
    })?;
    let lan = guardiana_devices::local_lan_ipv4();
    let shown_ip = if on { ip } else { lan.map(|i| i.to_string()) };
    let parsed: Option<std::net::Ipv4Addr> = shown_ip.as_deref().and_then(|s| s.parse().ok());
    let url = parsed.map(home::home_url);
    Ok(Hogar {
        licencia: String::new(),
        encendido: on,
        ip_dinamica: parsed.and_then(home::ip_is_dynamic),
        suspension: home::sleep_after_minutes(),
        qr_svg: if on {
            url.as_deref().and_then(qr_svg)
        } else {
            None
        },
        url,
        ip: shown_ip,
        desde: since,
        escuchando_en_lan: state.info.listen_dns.iter().any(|a| !a.starts_with("127.")),
        dispositivos: devices,
    })
}

pub(crate) async fn hogar(State(state): State<Arc<AppState>>, _s: Session) -> ApiResult<Hogar> {
    Ok(Json(hogar_view(&state)?))
}

#[derive(Serialize)]
pub(crate) struct HogarCambio {
    hogar: Hogar,
    aviso: Option<String>,
}

pub(crate) async fn hogar_activar(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<HogarCambio> {
    let Some(lan) = guardiana_devices::local_lan_ipv4() else {
        return Err((StatusCode::CONFLICT, t.panel("hogar_sin_lan").to_owned()).into_response());
    };
    if !guardiana_devices::is_private_lan(std::net::IpAddr::V4(lan)) {
        return Err((StatusCode::CONFLICT, t.panel("hogar_sin_lan").to_owned()).into_response());
    }
    // Home Mode is free (decision 52): no trial, no licence gate.
    let aviso = match home::firewall_allow() {
        Ok(()) => None,
        Err(home::FirewallError::Manual) => Some(t.panel("hogar_firewall_manual").to_owned()),
        Err(home::FirewallError::Command(_)) => Some(t.panel("hogar_no_admin").to_owned()),
    };
    with_ledger(&state, |l| {
        l.set_setting(SETTING_HOME_MODE, "1")?;
        l.set_setting(SETTING_HOME_IP, &lan.to_string())?;
        l.set_setting(SETTING_HOME_SINCE, &now_ms().to_string())?;
        l.record_change(now_ms(), ChangeKind::HogarOn, "panel", &lan.to_string())
    })?;
    Ok(Json(HogarCambio {
        hogar: hogar_view(&state)?,
        aviso,
    }))
}

pub(crate) async fn hogar_desactivar(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<HogarCambio> {
    let aviso = match home::firewall_remove() {
        Ok(()) | Err(home::FirewallError::Manual) => None,
        Err(home::FirewallError::Command(_)) => Some(t.panel("hogar_no_admin").to_owned()),
    };
    with_ledger(&state, |l| {
        l.set_setting(SETTING_HOME_MODE, "0")?;
        l.set_setting(SETTING_HOME_IP, "")?;
        l.record_change(now_ms(), ChangeKind::HogarOff, "panel", "")
    })?;
    Ok(Json(HogarCambio {
        hogar: hogar_view(&state)?,
        aviso,
    }))
}

// ----- the calling device's own page (no token; by IP) ----------------------

#[derive(Serialize)]
pub(crate) struct MiDispositivo {
    es_este_computador: bool,
    dispositivo: Option<Device>,
    totales: Totales,
    eventos: Vec<EventView>,
    lectura: Lectura,
}

fn identity_of(peer: SocketAddr) -> String {
    if peer.ip().is_loopback() {
        SELF_DEVICE_ID.to_owned()
    } else {
        guardiana_devices::Resolver::default()
            .identify(peer.ip())
            .id
    }
}

pub(crate) async fn mi_dispositivo(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
) -> ApiResult<MiDispositivo> {
    let id = identity_of(peer);
    let now = now_ms();
    let (device, totals, events, lectura) = with_ledger(&state, |l| {
        let device = l.device(&id)?;
        let totals = l
            .device_totals(None)?
            .into_iter()
            .find(|t| t.device_id == id);
        let events = l.events(&EventFilter {
            device_id: Some(id.clone()),
            limit: Some(100),
            ..EventFilter::default()
        })?;
        let house_last = l.devices()?.iter().map(|d| d.last_seen).max().unwrap_or(0);
        let lectura = match &device {
            Some(d) => lectura(l, &id, d.last_seen, house_last, now)?,
            None => Lectura::default(),
        };
        Ok((device, totals, events, lectura))
    })?;
    let names = HashMap::new();
    let mut eventos: Vec<EventView> = events.into_iter().map(|e| view(t, e, &names)).collect();
    eventos.reverse();
    Ok(Json(MiDispositivo {
        es_este_computador: id == SELF_DEVICE_ID,
        dispositivo: device,
        totales: totals.map_or_else(Totales::default, |t| Totales {
            consultas: t.queries,
            rastreadores: t.rastreador,
            publicidad: t.publicidad,
            telemetria: t.telemetria,
            esperados: t.esperado,
            desconocidos: t.desconocido,
            cortados: t.cortado,
        }),
        eventos,
        lectura,
    }))
}

pub(crate) async fn mi_nombre(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(body): Json<Nombre>,
) -> ApiResult<Option<Device>> {
    let name = body.name.trim().to_owned();
    if name.is_empty() || name.chars().count() > 60 {
        return Err((StatusCode::BAD_REQUEST, "name must be 1-60 characters").into_response());
    }
    // A phone can name itself before its DNS points here: create its row if needed,
    // so the name is not lost (the panel reloads every few seconds).
    let (id, mac, ip) = if peer.ip().is_loopback() {
        (SELF_DEVICE_ID.to_owned(), None, None)
    } else {
        let who = guardiana_devices::Resolver::default().identify(peer.ip());
        (who.id, who.mac, Some(peer.ip().to_string()))
    };
    Ok(Json(with_ledger(&state, |l| {
        if l.device(&id)?.is_none() {
            l.upsert_device(&id, mac.as_deref(), ip.as_deref(), now_ms())?;
        }
        l.rename_device(&id, &name)?;
        l.device(&id)
    })?))
}

#[derive(Deserialize)]
pub(crate) struct Compartir {
    compartir: bool,
}

pub(crate) async fn mi_compartir(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(body): Json<Compartir>,
) -> ApiResult<Option<Device>> {
    let id = identity_of(peer);
    Ok(Json(with_ledger(&state, |l| {
        l.set_share_detail(&id, body.compartir)?;
        l.device(&id)
    })?))
}

// ----- weekly report (token) ------------------------------------------------

#[derive(Serialize)]
pub(crate) struct Informe {
    /// Whether Plus is on: the rows are the household's real week.
    plus: bool,
    /// Free plan: the rows are a fixed example, not readings.
    ejemplo: bool,
    desde: i64,
    hasta: i64,
    dispositivos: Vec<InformeDispositivo>,
    total: InformeFila,
    texto_whatsapp: String,
}

#[derive(Serialize)]
struct InformeDispositivo {
    id: String,
    name: Option<String>,
    fila: InformeFila,
}

#[derive(Serialize, Default)]
struct InformeFila {
    consultas: i64,
    rastreadores: i64,
    publicidad: i64,
    telemetria: i64,
    esperados: i64,
    desconocidos: i64,
    cortados: i64,
}

fn fila(d: &guardiana_core::DeviceWeek) -> InformeFila {
    InformeFila {
        consultas: d.queries,
        rastreadores: d.trackers,
        publicidad: d.ads,
        telemetria: d.telemetry,
        esperados: d.expected,
        desconocidos: d.unknown,
        cortados: d.blocked,
    }
}

pub(crate) async fn informe(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<Informe> {
    let (w, plus) = with_ledger(&state, |l| {
        let plus = guardiana_license::status(l, &state.token, now_ms())
            .map(|s| s.plus_activo)
            .unwrap_or(false);
        Ok((l.week_summary(now_ms())?, plus))
    })?;
    if !plus {
        // Free plan: a sample clearly marked as an example, never the household's data.
        let sample = |q, tr, ads, tel, exp, unk, blk| InformeFila {
            consultas: q,
            rastreadores: tr,
            publicidad: ads,
            telemetria: tel,
            esperados: exp,
            desconocidos: unk,
            cortados: blk,
        };
        let rows = vec![
            (
                t.panel("informe_ejemplo_pc").to_owned(),
                sample(412, 31, 22, 9, 297, 53, 4),
            ),
            (
                t.panel("informe_ejemplo_tele").to_owned(),
                sample(188, 44, 12, 61, 63, 8, 6),
            ),
            (
                t.panel("informe_ejemplo_telefono").to_owned(),
                sample(255, 39, 27, 14, 158, 17, 2),
            ),
        ];
        let total = sample(855, 114, 61, 84, 518, 78, 12);
        return Ok(Json(Informe {
            plus: false,
            ejemplo: true,
            desde: w.since,
            hasta: w.until,
            dispositivos: rows
                .into_iter()
                .enumerate()
                .map(|(i, (name, fila))| InformeDispositivo {
                    id: format!("ejemplo-{i}"),
                    name: Some(name),
                    fila,
                })
                .collect(),
            total,
            texto_whatsapp: String::new(),
        }));
    }
    let texto_whatsapp = t
        .panel("informe_texto")
        .replace("{dispositivos}", &w.devices.len().to_string())
        .replace("{consultas}", &w.total.queries.to_string())
        .replace("{rastreadores}", &w.total.trackers.to_string())
        .replace("{publicidad}", &w.total.ads.to_string())
        .replace("{cortados}", &w.total.blocked.to_string());
    Ok(Json(Informe {
        plus: true,
        ejemplo: false,
        desde: w.since,
        hasta: w.until,
        dispositivos: w
            .devices
            .iter()
            .map(|d| InformeDispositivo {
                id: d.device_id.clone(),
                name: d.name.clone(),
                fila: fila(d),
            })
            .collect(),
        total: fila(&w.total),
        texto_whatsapp,
    }))
}

// ----- rules (brief §6) -----------------------------------------------------

/// A rule as the pages show it.
#[derive(Serialize)]
pub(crate) struct ReglaView {
    id: i64,
    scope: String,
    device_id: Option<String>,
    device_name: Option<String>,
    match_kind: String,
    pattern: String,
    action: String,
    created_at: i64,
    created_by: String,
    undone_at: Option<i64>,
    confirmed: bool,
    activa: bool,
}

fn rule_view(r: Rule, names: &HashMap<String, Option<String>>, now: i64) -> ReglaView {
    ReglaView {
        activa: r.is_active(now),
        device_name: r
            .device_id
            .as_ref()
            .and_then(|d| names.get(d).cloned().flatten()),
        id: r.id,
        scope: r.scope.as_str().to_owned(),
        device_id: r.device_id,
        match_kind: r.match_kind.as_str().to_owned(),
        pattern: r.pattern,
        action: r.action.as_str().to_owned(),
        created_at: r.created_at,
        created_by: r.created_by,
        undone_at: r.undone_at,
        confirmed: r.confirmed,
    }
}

#[derive(Serialize)]
pub(crate) struct Reglas {
    reglas: Vec<ReglaView>,
    modo_bloqueo: String,
}

fn list_rules(state: &AppState, only_device: Option<&str>) -> Result<Reglas, Response> {
    let now = now_ms();
    let (rules, devices, mode) = with_ledger(state, |l| {
        Ok((
            l.rules()?,
            l.devices()?,
            l.setting("block_mode")?
                .unwrap_or_else(|| "nxdomain".to_owned()),
        ))
    })?;
    let names = names_of(&devices);
    let reglas = rules
        .into_iter()
        .filter(|r| match only_device {
            Some(d) => r.scope == Scope::Device && r.device_id.as_deref() == Some(d),
            None => true,
        })
        .map(|r| rule_view(r, &names, now))
        .collect();
    Ok(Reglas {
        reglas,
        modo_bloqueo: mode,
    })
}

pub(crate) async fn reglas(State(state): State<Arc<AppState>>, _s: Session) -> ApiResult<Reglas> {
    Ok(Json(list_rules(&state, None)?))
}

#[derive(Deserialize)]
pub(crate) struct NuevaRegla {
    scope: String,
    #[serde(default)]
    device_id: Option<String>,
    match_kind: String,
    pattern: String,
    action: String,
    #[serde(default)]
    confirmed: bool,
}

/// Outcome of trying to create a rule: created, or a plain-language reason it
/// needs confirmation or cannot exist (brief §6).
#[derive(Serialize)]
pub(crate) struct AltaRegla {
    creada: Option<ReglaView>,
    /// `confirmar`: resend with `confirmed: true` to accept the warning.
    necesita: Option<String>,
    mensaje: Option<String>,
}

fn create_rule(
    t: &Texts,
    state: &AppState,
    body: NuevaRegla,
    forced_device: Option<&str>,
    created_by: &str,
) -> Result<AltaRegla, Response> {
    let bad = |msg: String| (StatusCode::BAD_REQUEST, msg).into_response();
    let scope: Scope = body
        .scope
        .parse()
        .map_err(|e: guardiana_core::Error| bad(e.to_string()))?;
    let match_kind: MatchKind = body
        .match_kind
        .parse()
        .map_err(|e: guardiana_core::Error| bad(e.to_string()))?;
    let action: Action = body
        .action
        .parse()
        .map_err(|e: guardiana_core::Error| bad(e.to_string()))?;
    let pattern = body
        .pattern
        .trim()
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if pattern.is_empty() || pattern.len() > 253 {
        return Err(bad(t.panel("regla_patron_invalido").to_owned()));
    }
    if match_kind == MatchKind::Category && pattern.parse::<Category>().is_err() {
        return Err(bad(t.panel("regla_patron_invalido").to_owned()));
    }
    let device_id = match (scope, forced_device) {
        (Scope::Device, Some(d)) => Some(d.to_owned()),
        (Scope::Device, None) => body.device_id.filter(|d| !d.is_empty()),
        (Scope::Home, Some(_)) => {
            return Err((
                StatusCode::FORBIDDEN,
                t.panel("regla_casa_solo_panel").to_owned(),
            )
                .into_response());
        }
        (Scope::Home, None) => None,
    };
    if scope == Scope::Device && device_id.is_none() {
        return Err(bad(t.panel("regla_patron_invalido").to_owned()));
    }

    if action == Action::Cortar {
        // Inviolable: expected traffic cannot be cut by category at all.
        if match_kind == MatchKind::Category && pattern == Category::Esperado.as_str() {
            return Ok(AltaRegla {
                creada: None,
                necesita: None,
                mensaje: Some(t.panel("regla_esperado_prohibido").to_owned()),
            });
        }
        // The 24-hour gate for a device rule.
        if let Some(d) = device_id.as_deref() {
            let dev = with_ledger(state, |l| l.device(d))?;
            let Some(dev) = dev else {
                return Err(bad(t.panel("regla_patron_invalido").to_owned()));
            };
            let observed = now_ms() - dev.first_seen;
            if !observation_complete(observed) {
                return Ok(AltaRegla {
                    creada: None,
                    necesita: None,
                    mensaje: Some(
                        t.panel("observando")
                            .replace("{h}", &observed_hours(observed).to_string()),
                    ),
                });
            }
        }
        // Explicit rule on expected traffic: needs the confirmation sentence.
        if match_kind != MatchKind::Category && !body.confirmed {
            let catalog = guardiana_lists::Catalog::bundled();
            let protected = catalog.category(&pattern) == Category::Esperado;
            if protected {
                return Ok(AltaRegla {
                    creada: None,
                    necesita: Some("confirmar".to_owned()),
                    mensaje: Some(t.panel("regla_esperado_confirmar").to_owned()),
                });
            }
        }
    }

    let now = now_ms();
    let rule = with_ledger(state, |l| {
        l.add_rule(NewRule {
            scope,
            device_id: device_id.clone(),
            match_kind,
            pattern: pattern.clone(),
            action,
            created_at: now,
            created_by: created_by.to_owned(),
            expires_at: None,
            confirmed: body.confirmed,
        })
    })?;
    let devices = with_ledger(state, |l| l.devices())?;
    Ok(AltaRegla {
        creada: Some(rule_view(rule, &names_of(&devices), now)),
        necesita: None,
        mensaje: None,
    })
}

pub(crate) async fn nueva_regla(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Json(body): Json<NuevaRegla>,
) -> ApiResult<AltaRegla> {
    Ok(Json(create_rule(t, &state, body, None, "usuario (panel)")?))
}

pub(crate) async fn deshacer_regla(
    State(state): State<Arc<AppState>>,
    _s: Session,
    Path(id): Path<i64>,
) -> ApiResult<bool> {
    Ok(Json(with_ledger(&state, |l| l.undo_rule(id, now_ms()))?))
}

pub(crate) async fn deshacer_hoy(
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<usize> {
    let now = now_ms();
    Ok(Json(with_ledger(&state, |l| {
        l.undo_rules_since(now - guardiana_core::time::DAY_MS, now)
    })?))
}

#[derive(Deserialize)]
pub(crate) struct ModoBloqueo {
    modo: String,
}

pub(crate) async fn modo_bloqueo(
    State(state): State<Arc<AppState>>,
    _s: Session,
    Json(body): Json<ModoBloqueo>,
) -> ApiResult<String> {
    let modo = match body.modo.as_str() {
        "zero" => "zero",
        _ => "nxdomain",
    };
    with_ledger(&state, |l| l.set_setting("block_mode", modo))?;
    Ok(Json(modo.to_owned()))
}

// Phone: its own rules, by IP, no token.

pub(crate) async fn mi_reglas(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
) -> ApiResult<Reglas> {
    let id = identity_of(peer);
    Ok(Json(list_rules(&state, Some(&id))?))
}

pub(crate) async fn mi_nueva_regla(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(body): Json<NuevaRegla>,
) -> ApiResult<AltaRegla> {
    let id = identity_of(peer);
    Ok(Json(create_rule(
        t,
        &state,
        body,
        Some(&id),
        "usuario (dispositivo)",
    )?))
}

pub(crate) async fn mi_deshacer_regla(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Path(rule_id): Path<i64>,
) -> ApiResult<bool> {
    let me = identity_of(peer);
    let now = now_ms();
    Ok(Json(with_ledger(&state, |l| {
        let own = l.rules()?.into_iter().any(|r| {
            r.id == rule_id
                && r.scope == Scope::Device
                && r.device_id.as_deref() == Some(me.as_str())
        });
        if own {
            l.undo_rule(rule_id, now)
        } else {
            Ok(false)
        }
    })?))
}

// ----- verify (token) -------------------------------------------------------

#[derive(Serialize)]
pub(crate) struct VerifyView {
    texto: String,
    informe: guardiana_verify::Report,
}

pub(crate) async fn verify(Lang(t): Lang, _s: Session) -> ApiResult<VerifyView> {
    let informe = tokio::task::spawn_blocking(guardiana_verify::run)
        .await
        .map_err(internal)?;
    Ok(Json(VerifyView {
        texto: guardiana_verify::render_with(t, &informe),
        informe,
    }))
}

// ----- licence (token) ------------------------------------------------------

#[derive(Serialize)]
pub(crate) struct LicenciaView {
    estado: guardiana_license::Status,
    texto: String,
    clave_dev: bool,
    host_activacion: &'static str,
    /// Hours observed so far (the trial needs 24).
    horas_observadas: i64,
    /// Show the "try Plus" invitation (decision 53): free, trial unused, 24 h observed.
    invitar: bool,
    conexiones: Vec<OutboundView>,
}

fn licencia_error(t: &Texts, e: &guardiana_license::Error) -> String {
    match e {
        guardiana_license::Error::DevKey => t.panel("licencia_dev").to_owned(),
        guardiana_license::Error::Malformed(_) => t.panel("licencia_err_formato").to_owned(),
        guardiana_license::Error::BadSignature => t.panel("licencia_err_firma").to_owned(),
        guardiana_license::Error::Expired => t.panel("licencia_err_caducada").to_owned(),
        guardiana_license::Error::KeyRejected(why) => {
            t.panel("licencia_err_clave").replace("{motivo}", why)
        }
        guardiana_license::Error::Network(why) => {
            t.panel("licencia_err_red").replace("{motivo}", why)
        }
        guardiana_license::Error::TrialTooEarly { horas } => t
            .panel("licencia_prueba_pronto")
            .replace("{h}", &horas.to_string()),
        guardiana_license::Error::TrialUsed => t.panel("licencia_prueba_usada").to_owned(),
        guardiana_license::Error::AlreadyPlus => t.panel("licencia_ya_plus").to_owned(),
        guardiana_license::Error::Ledger(err) => err.to_string(),
    }
}

fn licencia_texto(t: &Texts, s: &guardiana_license::Status) -> String {
    use guardiana_core::time::{rfc3339_utc, HOUR_MS};
    use guardiana_license::{Comprobacion, Plan};
    let day = |ms: i64| rfc3339_utc(ms)[..10].to_owned();
    match &s.plan {
        Plan::Gratis => {
            if s.puede_probar {
                t.panel("licencia_gratis_puede_probar").to_owned()
            } else {
                t.panel("licencia_gratis")
                    .replace("{h}", &(s.observado_ms / HOUR_MS).to_string())
            }
        }
        Plan::Prueba {
            termina,
            dias_restantes,
            ..
        } => t
            .panel("licencia_prueba")
            .replace("{d}", &dias_restantes.to_string())
            .replace("{fecha}", &day(*termina)),
        Plan::PruebaAgotada { termino } => t
            .panel("licencia_prueba_agotada")
            .replace("{fecha}", &day(*termino)),
        Plan::Plus {
            origen,
            desde,
            titular,
            periodo_dias,
            proxima_comprobacion,
            caduca_ms,
            comprobacion,
            ..
        } => {
            let mut text = t
                .panel("licencia_plus")
                .replace(
                    "{origen}",
                    if origen == "clave" {
                        t.panel("licencia_origen_clave")
                    } else {
                        t.panel("licencia_origen_archivo")
                    },
                )
                .replace("{fecha}", &day(*desde))
                .replace("{titular}", titular.as_deref().unwrap_or("-"));
            match (periodo_dias, proxima_comprobacion, comprobacion) {
                (Some(p), Some(next), Some(c)) => {
                    let key = match c {
                        Comprobacion::AlDia => "licencia_comprobacion_al_dia",
                        Comprobacion::Pendiente => "licencia_comprobacion_pendiente",
                        Comprobacion::Fallida => "licencia_comprobacion_fallida",
                    };
                    text.push(' ');
                    text.push_str(
                        &t.panel(key)
                            .replace(
                                "{periodo}",
                                if *p >= guardiana_license::YEAR_DAYS {
                                    t.panel("licencia_periodo_anual")
                                } else {
                                    t.panel("licencia_periodo_mensual")
                                },
                            )
                            .replace("{fecha}", &day(*next))
                            .replace("{limite}", &day(caduca_ms.unwrap_or(*next))),
                    );
                }
                _ => {
                    if let Some(c) = caduca_ms {
                        text.push(' ');
                        text.push_str(
                            &t.panel("licencia_archivo_caduca")
                                .replace("{fecha}", &day(*c)),
                        );
                    }
                }
            }
            text
        }
        Plan::PlusTerminado { termino, motivo } => t
            .panel(match motivo.as_str() {
                "cancelada" => "licencia_plus_cancelada",
                "caducada" => "licencia_plus_caducada",
                _ => "licencia_plus_sin_comprobar",
            })
            .replace("{fecha}", &day(*termino)),
    }
}

fn licencia_view(t: &Texts, state: &AppState) -> Result<LicenciaView, Response> {
    let (estado, outbound) = with_ledger(state, |l| {
        let s = guardiana_license::status(l, &state.token, now_ms()).map_err(|e| {
            guardiana_core::Error::UnknownValue {
                kind: "licencia",
                value: e.to_string(),
            }
        })?;
        Ok((s, l.outbound()?))
    })?;
    Ok(LicenciaView {
        texto: licencia_texto(t, &estado),
        clave_dev: guardiana_core::identity::public_key_is_dev(),
        host_activacion: guardiana_license::gateway_host(),
        horas_observadas: estado.observado_ms / guardiana_core::time::HOUR_MS,
        invitar: estado.puede_probar,
        conexiones: outbound
            .into_iter()
            .filter(|o| o.purpose == guardiana_core::Purpose::Licencia)
            .map(|o| OutboundView {
                ts: o.ts,
                purpose: o.purpose.as_str().to_owned(),
                host: o.host,
                bytes: o.bytes,
                initiated_by_user: o.initiated_by_user,
            })
            .collect(),
        estado,
    })
}

/// Start the 7-day Plus trial (decision 53): the user asks for it, never before 24 h.
pub(crate) async fn licencia_probar(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<LicenciaView> {
    let result = with_ledger(&state, |l| {
        Ok(guardiana_license::start_trial(l, &state.token, now_ms())
            .map(|_| ())
            .map_err(|e| licencia_error(t, &e)))
    })?;
    if let Err(e) = result {
        return Err((StatusCode::CONFLICT, e).into_response());
    }
    Ok(Json(licencia_view(t, &state)?))
}

pub(crate) async fn licencia(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<LicenciaView> {
    Ok(Json(licencia_view(t, &state)?))
}

#[derive(Deserialize)]
pub(crate) struct ClaveBody {
    clave: String,
}

pub(crate) async fn licencia_clave(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Json(body): Json<ClaveBody>,
) -> ApiResult<LicenciaView> {
    let key = body.clave.trim().to_owned();
    if key.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "clave vacía").into_response());
    }
    let st = state.clone();
    // The network call blocks: keep it off the async workers.
    let result = tokio::task::spawn_blocking(move || {
        let Ok(mut guard) = st.ledger.lock() else {
            return Err("ledger lock poisoned".to_owned());
        };
        guardiana_license::activate_with_key(&mut guard, &key, &st.token, now_ms())
            .map(|_| ())
            .map_err(|e| licencia_error(t, &e))
    })
    .await
    .map_err(internal)?;
    if let Err(e) = result {
        return Err((StatusCode::CONFLICT, e).into_response());
    }
    Ok(Json(licencia_view(t, &state)?))
}

#[derive(Deserialize)]
pub(crate) struct ArchivoBody {
    texto: String,
}

pub(crate) async fn licencia_archivo(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Json(body): Json<ArchivoBody>,
) -> ApiResult<LicenciaView> {
    let result = with_ledger(&state, |l| {
        Ok(
            guardiana_license::activate_with_file(l, &body.texto, &state.token, now_ms())
                .map(|_| ())
                .map_err(|e| licencia_error(t, &e)),
        )
    })?;
    if let Err(e) = result {
        return Err((StatusCode::CONFLICT, e).into_response());
    }
    Ok(Json(licencia_view(t, &state)?))
}
