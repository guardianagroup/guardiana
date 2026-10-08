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
use guardiana_core::{
    write_csv_for_spreadsheet, write_json, Action, Category, DecidedBy, Device, Event, EventFilter,
    Ledger, MatchKind, NewRule, Rule, Scope, Signal, Verdict, SELF_DEVICE_ID,
};
use guardiana_core::{ChangeKind, ChangeWho};
use guardiana_service::home::{
    self, SETTING_HOME_APARCADO, SETTING_HOME_IP, SETTING_HOME_MODE, SETTING_HOME_RED,
    SETTING_HOME_SINCE,
};
use guardiana_service::sysdns::{SETTING_BACKUP, SETTING_BACKUP_APARCADA};
use serde::{Deserialize, Serialize};

use crate::auth::{Lang, PorDireccion, Session, Zona};
use crate::AppState;

type ApiResult<T> = Result<Json<T>, Response>;

fn internal(e: impl std::fmt::Display) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
}

/// Run `f` on the panel's ledger connection.
///
/// The panel shares the runtime with the resolver, and SQLite blocks: an export of 600,000 rows
/// held a worker thread for seconds and a DNS answer waited 5.9 s behind it (review of 5 Oct
/// 2026, serious 12). On the service's multi-threaded runtime the work is done "in place":
/// tokio hands this worker's other tasks, the resolver's among them, to another thread first.
/// Waiting for the lock counts as blocking too, so it is inside. A runtime with one thread (the
/// tests) cannot do that and runs it as before.
fn with_ledger<T>(
    state: &AppState,
    f: impl FnOnce(&mut Ledger) -> guardiana_core::Result<T>,
) -> Result<T, Response> {
    let run = || {
        let mut guard = state
            .ledger
            .lock()
            .map_err(|_| internal("ledger lock poisoned"))?;
        f(&mut guard).map_err(internal)
    };
    en_hilo_aparte(run)
}

/// `f`, which blocks (a database, a process, the network), without holding up the resolver that
/// shares these threads: "in place" on a multi-threaded runtime, as it is on a runtime with one
/// thread (the tests), which cannot hand its other tasks away.
fn en_hilo_aparte<T>(f: impl FnOnce() -> T) -> T {
    match tokio::runtime::Handle::try_current().map(|h| h.runtime_flavor()) {
        Ok(tokio::runtime::RuntimeFlavor::MultiThread) => tokio::task::block_in_place(f),
        _ => f(),
    }
}

/// An event as the pages show it: with device name and the signal sentences.
#[derive(Serialize, Clone)]
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
    /// The country that company answers to, as its name in the language of the panel.
    ///
    /// Es el país de la EMPRESA, no el del servidor que responde: casi todo lo grande va por una
    /// red de reparto y el servidor suele estar en el país de quien pregunta, así que un país
    /// sacado de la dirección IP diría «Colombia» de algo cuyos datos acaban en Estados Unidos.
    pais: Option<String>,
    /// La ciudad de la sede de la empresa dueña del nombre, cuando está comprobada. Va debajo
    /// del país: «Estados Unidos» sitúa bajo qué leyes está; «Menlo Park» lo hace concreto.
    ciudad: Option<String>,
    /// The artificial-intelligence service the name belongs to (decision 62), or `None`.
    ia: Option<&'static str>,
    /// The delivery network that serves the name (decision 148), or `None`. Used to say
    /// "delivery" instead of "unknown" for a name that is the road, not the destination.
    entrega: Option<&'static str>,
    /// Whether the name is the home network talking to itself (decision 148): reverse
    /// lookups, mDNS and service discovery, which are not a destination at all.
    local: bool,
    /// What the company behind the name does for a living, in one sentence (decision 149).
    /// `None` when it is not written: a company name is not a trade, and guessing one would be
    /// exactly the invented verdict this program refuses to give.
    oficio: Option<String>,
    /// The program that asked, when the system said so (Windows, this computer). `None` on the
    /// phones of Home Mode, on the other systems, and whenever the system did not say: an empty
    /// space, never a guess.
    programa: Option<guardiana_core::Proceso>,
    /// Identifier of the trap file, when this name is one. A query for it means that file was
    /// read: Guardiana never looked at the file, it only ever saw the name.
    trampa: Option<String>,
    /// The registered data broker that owns the name (decision 194), when one does: a company
    /// that told the State of California it collects and sells personal data of people it has no
    /// relationship with. Copied from that public register; never a verdict about this query.
    corredor: Option<CorredorView>,
}

/// A registered data broker as the pages show it: who, from where, what it ticked in the
/// register, and where it said people can ask for their data back.
#[derive(Serialize, Clone)]
pub(crate) struct CorredorView {
    nombre: &'static str,
    /// The country the registrant wrote for itself, named in the language of the panel.
    pais: Option<String>,
    /// What the registrant ticked, one short sentence each, already in the panel's language.
    declara: Vec<String>,
    /// The page it declared for people to ask that their data be deleted or not sold.
    derechos: Option<&'static str>,
}

/// The letters of `corredores.txt` and the text key that says each one. The order is the order
/// the sentences are shown in: what it collects first, who it sold to after.
const DECLARA: [(char, &str); 7] = [
    ('g', "corredor_declara_g"),
    ('m', "corredor_declara_m"),
    ('b', "corredor_declara_b"),
    ('x', "corredor_declara_x"),
    ('f', "corredor_declara_f"),
    ('p', "corredor_declara_p"),
    ('i', "corredor_declara_i"),
];

fn corredor_de(t: &Texts, qname: &str) -> Option<CorredorView> {
    let b = guardiana_lists::data_broker_of(qname)?;
    Some(CorredorView {
        nombre: b.name,
        pais: b
            .country
            .map(|c| t.pais(c))
            .filter(|n| !n.is_empty())
            .map(str::to_owned),
        declara: DECLARA
            .iter()
            .filter(|(letra, _)| b.declares(*letra))
            .map(|(_, clave)| t.panel(clave).to_owned())
            .collect(),
        derechos: b.rights_url,
    })
}

/// Who owns the name: the company of `empresas.txt`, or, when that list does not know it, the
/// registrant of `corredores.txt`. A registered broker's own name is as good an owner as any.
fn empresa_de(qname: &str) -> Option<&'static str> {
    guardiana_lists::company_of(qname)
        .or_else(|| guardiana_lists::data_broker_of(qname).map(|b| b.name))
}

/// The two-letter country of the owner, the same way round: the company's, else the registrant's.
fn pais_codigo_de(qname: &str) -> Option<&'static str> {
    guardiana_lists::country_of(qname)
        .or_else(|| guardiana_lists::data_broker_of(qname).and_then(|b| b.country))
}

/// The sentence for a name: first the name itself and its parent domains, then the company
/// that owns it. `app-measurement.com` belongs to Google, whose trade does not fit in one
/// sentence, but that name alone does: Firebase measurement. Looking at the name first is what
/// lets a broad owner still carry a precise line where one is true.
fn oficio_de(t: &Texts, qname: &str) -> Option<String> {
    let name = qname.trim_end_matches('.').to_ascii_lowercase();
    let mut rest = name.as_str();
    loop {
        if let Some(f) = t.oficio(rest) {
            return Some(f.to_owned());
        }
        match rest.split_once('.') {
            Some((_, r)) if r.contains('.') => rest = r,
            _ => break,
        }
    }
    if let Some(f) = guardiana_lists::company_of(qname).and_then(|c| t.oficio(c)) {
        return Some(f.to_owned());
    }
    // No trade written, but the register says what the owner is registered as: that sentence is
    // the trade, and it is the company's own declaration rather than ours.
    let b = guardiana_lists::data_broker_of(qname)?;
    let pais = b.country.map(|c| t.pais(c)).filter(|n| !n.is_empty());
    Some(
        t.panel("corredor_frase")
            .replace("{nombre}", b.name)
            .replace(
                "{pais}",
                &pais.map(|p| format!(" · {p}")).unwrap_or_default(),
            ),
    )
}

/// El país de la empresa dueña del nombre, ya dicho en el idioma del panel. Si el código no
/// tiene nombre escrito en ese idioma, no se enseña nada: dos letras sueltas no informan.
fn pais_de(t: &Texts, qname: &str) -> Option<String> {
    let codigo = pais_codigo_de(qname)?;
    let nombre = t.pais(codigo);
    (!nombre.is_empty()).then(|| nombre.to_owned())
}

fn view(t: &Texts, e: Event, names: &HashMap<String, Option<String>>) -> EventView {
    EventView {
        frases: e.signals.iter().map(|s| t.signal(s)).collect(),
        device_name: names.get(&e.device_id).cloned().flatten(),
        empresa: empresa_de(&e.qname),
        pais: pais_de(t, &e.qname),
        corredor: corredor_de(t, &e.qname),
        ciudad: guardiana_lists::city_of(&e.qname).map(str::to_owned),
        ia: guardiana_lists::ai_service_of(&e.qname),
        entrega: guardiana_lists::delivery_of(&e.qname),
        local: guardiana_lists::is_local_name(&e.qname),
        oficio: oficio_de(t, &e.qname),
        programa: e.process,
        trampa: guardiana_core::trampas::id_de(&e.qname),
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
        qname: None,
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
    let body = i18n::json_of(t);
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
    _p: PorDireccion,
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
    // With the trial over and no Plus the guardian is not answering: pointing the system at it
    // would leave the computer on its secondary resolver, or on none (review of 28 Sep 2026, G6).
    let puede = with_ledger(&state, |l| {
        Ok(guardiana_license::status(l, &state.token, now_ms()).ok())
    })?
    .is_none_or(|s| s.puede_funcionar);
    if !puede {
        return Err((StatusCode::CONFLICT, t.panel("caducado_dns").to_owned()).into_response());
    }
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
            let _ = l.record_change(now_ms(), ChangeKind::DnsOn, ChangeWho::Panel, &originals);
        }
        let mut msg = t
            .cli("dns.aplicado_panel")
            .replace("{originales}", &originals);
        msg.push(' ');
        msg.push_str(t.cli(guardiana_service::sysdns::sole_or_secondary_key()));
        // Not what Windows was told, but whether a query really arrives: the settings said
        // "127.0.0.1 first" on 27 Sep 2026 and nothing came, and this screen said "done".
        if cfg!(target_os = "windows") {
            std::thread::sleep(std::time::Duration::from_millis(800));
            msg.push(' ');
            msg.push_str(t.cli(
                if guardiana_service::sysdns::system_reaches_guardian(
                    std::time::Duration::from_secs(4),
                ) {
                    "dns.camino_ok"
                } else {
                    "dns.camino_no"
                },
            ));
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
    // Undoing by hand is also saying no to the copy parked when the trial ended: it is not
    // applied again when the licence comes back.
    let aparcada =
        with_ledger(&state, |l| l.setting(SETTING_BACKUP_APARCADA))?.is_some_and(|v| !v.is_empty());
    if aparcada {
        with_ledger(&state, |l| l.set_setting(SETTING_BACKUP_APARCADA, ""))?;
    }
    let Some(json) = stored else {
        return Ok(Json(DnsCambio {
            dns_aplicado: false,
            mensaje: t
                .cli(if aparcada {
                    "dns.aparcada_borrada"
                } else {
                    "dns.no_hay_copia"
                })
                .to_owned(),
        }));
    };
    let backup: sysdns::Backup = serde_json::from_str(&json).map_err(internal)?;
    let st = state.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<(), String> {
        // La copia se borra ANTES de deshacer: mientras exista, el vigilante del
        // servicio vuelve a aplicar el cambio y el «restaurado» queda a medias
        // (decisión 118). Si deshacer falla, la copia se devuelve.
        {
            let l = st
                .ledger
                .lock()
                .map_err(|_| "ledger lock poisoned".to_owned())?;
            l.set_setting(SETTING_BACKUP, "")
                .map_err(|e| e.to_string())?;
        }
        if let Err(e) = sysdns::restore(&backup) {
            if let Ok(l) = st.ledger.lock() {
                if let Ok(json) = serde_json::to_string(&backup) {
                    let _ = l.set_setting(SETTING_BACKUP, &json);
                }
            }
            return Err(e.to_string());
        }
        let l = st
            .ledger
            .lock()
            .map_err(|_| "ledger lock poisoned".to_owned())?;
        l.record_change(now_ms(), ChangeKind::DnsOff, ChangeWho::Panel, "")
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
    /// Only when asked (`?detalle=1`, the Estado page; on Windows asking is a PowerShell, too
    /// slow for the banner every page loads): whether every connection of this machine asks the
    /// guardian right now. With the change applied and this `false`, the page says that one
    /// connection is not covered yet instead of a flat "pointed at Guardiana" (review of 5 Oct
    /// 2026, serious 10).
    dns_cubre: Option<bool>,
    listas: Vec<ListaInfo>,
    panel_puerto: u16,
    /// Which system this is running on. The panel uses it to say, on a Mac, that Home Mode is
    /// not part of 1.0 there: the site says so and the program must not offer it silently.
    so: &'static str,
    /// True when the trial or the subscription is over: Guardiana has stood down, given the
    /// system DNS back and stopped watching. Every page says so, and only activating and taking
    /// your own extract away still make sense.
    caducado: bool,
    /// Stood down because a subscription ended, not the trial: the banner says which, so a
    /// customer who paid is not told "the trial is over".
    caducado_plus: bool,
    /// While stood down: Home Mode was on, so the router still sends the house here and a relay
    /// passes their queries on without looking. The panel says so, and asks for the router back.
    hogar_aparcado: bool,
    /// Days left of the trial while it runs, so every page can count down honestly instead of
    /// letting the day arrive as a surprise. `None` with a subscription.
    prueba_dias: Option<i64>,
}

#[derive(Serialize)]
struct ListaInfo {
    id: String,
    entries: u64,
    fetched: String,
}

pub(crate) async fn estado(
    State(state): State<Arc<AppState>>,
    Query(q): Query<HashMap<String, String>>,
    _s: Session,
) -> ApiResult<Estado> {
    // Applied by Guardiana (a backup exists) or pointed here by other means (the
    // system says 127.0.0.1 is the primary): either way this PC passes through it.
    let copia = with_ledger(&state, |l| l.setting(SETTING_BACKUP))?.is_some_and(|v| !v.is_empty());
    let primario = if !copia || q.contains_key("detalle") {
        tokio::task::spawn_blocking(guardiana_service::sysdns::guardian_is_primary)
            .await
            .map_err(internal)?
    } else {
        None
    };
    let dns_aplicado = copia || primario == Some(true);
    let dns_cubre = if q.contains_key("detalle") {
        primario
    } else {
        None
    };
    let licencia = with_ledger(&state, |l| {
        Ok(guardiana_license::status(l, &state.token, now_ms()).ok())
    })?;
    let hogar_aparcado =
        with_ledger(&state, |l| l.setting(SETTING_HOME_APARCADO))?.is_some_and(|v| v == "1");
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
        dns_cubre,
        listas,
        panel_puerto: crate::DEFAULT_PORT,
        so: if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else {
            "linux"
        },
        caducado: !licencia.as_ref().is_none_or(|s| s.puede_funcionar),
        caducado_plus: licencia
            .as_ref()
            .is_some_and(|s| matches!(s.plan, guardiana_license::Plan::PlusTerminado { .. })),
        hogar_aparcado,
        prueba_dias: licencia.as_ref().and_then(|s| match s.plan {
            guardiana_license::Plan::Prueba { dias_restantes, .. } => Some(dias_restantes),
            _ => None,
        }),
    }))
}

#[derive(Serialize)]
pub(crate) struct Radiografia {
    desde: i64,
    hasta: i64,
    servicios: i64,
    rastreadores: i64,
    publicidad: i64,
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
    /// (país, consultas) de las últimas 24 h, el que más primero, como mucho seis. Es el país de
    /// la EMPRESA dueña del nombre, no el del servidor: ver `guardiana_lists::country_of`.
    paises: Vec<(String, u64)>,
    /// Minutes since its last query when it has been quiet for a while but the house has not.
    callado_min: Option<i64>,
    /// Queried iCloud Private Relay names in the last 24 h.
    relay: bool,
    /// Queries carrying the DNS-evasion signal in the last 24 h.
    evasiones: u64,
}

const SILENCE_MIN: i64 = 30;

/// Who is asking for the reading: the household panel, or the device about itself.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Lector {
    /// The home panel (token). A device that did not share its detail yields an empty reading:
    /// companies and countries are names read sideways, not totals (review of 1 Oct 2026,
    /// entry 1). Its figures are in `totales`, which do not pass through here.
    Casa,
    /// The device itself, from its own screen, identified by its IP.
    ElMismo,
}

fn lectura(
    t: &Texts,
    l: &Ledger,
    id: &str,
    last_seen: i64,
    house_last: i64,
    now: i64,
    lector: Lector,
) -> guardiana_core::Result<Lectura> {
    let filter = EventFilter {
        device_id: Some(id.to_owned()),
        since: Some(now - 24 * guardiana_core::time::HOUR_MS),
        // A normal PC asks ~6 000 names a day; 5 000 cut the "24 h" short (review of 8 Oct 2026).
        limit: Some(50_000),
        ..EventFilter::default()
    };
    let events = match lector {
        Lector::Casa => l.events(&filter)?,
        Lector::ElMismo => l.own_events(id, &filter)?,
    };
    let mut by_company: HashMap<&'static str, u64> = HashMap::new();
    let mut by_country: HashMap<&'static str, u64> = HashMap::new();
    let mut relay = false;
    let mut evasiones = 0;
    for e in &events {
        if let Some(c) = empresa_de(&e.qname) {
            *by_company.entry(c).or_insert(0) += 1;
        }
        if let Some(p) = pais_codigo_de(&e.qname) {
            *by_country.entry(p).or_insert(0) += 1;
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
    let mut paises: Vec<(String, u64)> = by_country
        .into_iter()
        .filter_map(|(c, n)| {
            let nombre = t.pais(c);
            (!nombre.is_empty()).then(|| (nombre.to_owned(), n))
        })
        .collect();
    paises.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    paises.truncate(6);
    let quiet_for = (now - last_seen) / 60_000;
    let callado_min =
        (!events.is_empty() && quiet_for >= SILENCE_MIN && now - house_last < 5 * 60_000)
            .then_some(quiet_for);
    Ok(Lectura {
        empresas,
        empresas_total,
        paises,
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

/// Whether the declared scope cuts or only reports, per device. The engine reads this
/// same key on its own beat (`crates/cli/src/engine.rs`).
fn scope_mode_key(device_id: &str) -> String {
    format!("alcance_modo:{device_id}")
}

/// A name is inside the declared scope when it equals a pattern or hangs below it.
fn in_scope(patterns: &[String], qname: &str) -> bool {
    let name = qname.trim_end_matches('.').to_ascii_lowercase();
    patterns.iter().any(|p| {
        guardiana_core::rules::normalizar_nombre(p)
            .is_some_and(|(p, _)| name == p || name.ends_with(&format!(".{p}")))
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
    /// Qué programas hicieron esas consultas, de más a menos, cuando el sistema lo dijo (Windows,
    /// este equipo). Es la pregunta que de verdad importa en esta pantalla: no solo que el equipo
    /// habló con Anthropic, sino que quien habló fue el agente y no el navegador. Vacío en los
    /// teléfonos y en los sistemas donde no se puede saber.
    programas: Vec<(String, u64)>,
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
    /// Whether the person asked for what falls outside to be cut, not just listed
    /// (decision 154). Off unless they turn it on.
    cortar: bool,
    /// When a temporary pass is running, the moment it ends and the cut comes back.
    /// For the person who sends a long job and walks away (decision 155).
    pase_hasta: Option<i64>,
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
        type PorServicio =
            HashMap<(String, &'static str), (HashMap<String, u64>, i64, HashMap<String, u64>)>;
        let mut servicios: PorServicio = HashMap::new();
        let mut alcances = Vec::new();
        let mut sin_alcance = Vec::new();
        // The whole week, grouped in the database: no cap, so "last 7 days" is the last 7 days.
        let semana = l.names_grouped(now - 7 * 24 * guardiana_core::time::HOUR_MS, now + 1)?;
        for g in &semana {
            let Some(servicio) = guardiana_lists::ai_service_of(&g.qname) else {
                continue;
            };
            if !devices.iter().any(|d| d.id == g.device_id) {
                continue;
            }
            let slot = servicios
                .entry((g.device_id.clone(), servicio))
                .or_insert_with(|| (HashMap::new(), 0, HashMap::new()));
            *slot.0.entry(g.qname.clone()).or_insert(0) += g.queries;
            slot.1 = slot.1.max(g.last_ts);
            if let Some(p) = &g.process {
                *slot.2.entry(p.clone()).or_insert(0) += g.queries;
            }
        }
        for d in &devices {
            let modo = l.setting(&scope_mode_key(&d.id))?.unwrap_or_default();
            let pase_hasta = modo
                .strip_prefix("observar:")
                .and_then(|x| x.parse::<i64>().ok());
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
            // The scope reads one day of this device; the category is needed to leave the
            // expected traffic out, so these are the rows themselves (a day, not a week).
            let dia = l.events(&EventFilter {
                device_id: Some(d.id.clone()),
                since: Some(now - 24 * guardiana_core::time::HOUR_MS),
                limit: Some(50_000),
                ..EventFilter::default()
            })?;
            let mut fuera: HashMap<String, u64> = HashMap::new();
            let mut dentro: std::collections::HashSet<String> = std::collections::HashSet::new();
            for e in dia.iter().filter(|e| e.category != Category::Esperado) {
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
                cortar: modo == "cortar" || pase_hasta.is_some_and(|t| now >= t),
                pase_hasta: pase_hasta.filter(|t| now < *t),
            });
        }
        let names = names_of(&devices);
        let mut servicios: Vec<IaServicio> = servicios
            .into_iter()
            .map(|((device_id, servicio), (nombres, ultima, por_programa))| {
                let mut programas: Vec<(String, u64)> = por_programa.into_iter().collect();
                programas.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
                programas.truncate(4);
                IaServicio {
                    servicio,
                    device_name: names.get(&device_id).cloned().flatten(),
                    device_id,
                    nombres: nombres.len(),
                    consultas: nombres.values().sum(),
                    ultima,
                    programas,
                }
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
    /// "cortar" to stop what falls outside; anything else only reports it.
    #[serde(default)]
    modo: Option<String>,
    /// Hours of temporary pass: nothing is cut until they are up, then the cut is back
    /// on its own. For sending a long job and walking away.
    #[serde(default)]
    pase_horas: Option<i64>,
    /// The declared destinations as typed, one per line. Empty clears the declaration.
    patrones: String,
}

/// Declare (or clear) what a device may talk to. Nothing is cut: Guardiana reports what went
/// beyond, and cutting stays a separate, explicit decision in Rules.
pub(crate) async fn alcance(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Json(body): Json<AlcanceBody>,
) -> ApiResult<IaView> {
    if body.patrones.len() > 8_000 {
        return Err((StatusCode::BAD_REQUEST, "scope too long").into_response());
    }
    // Each line as the name a query carries (`https://github.com/x` is `github.com`). A line that is
    // no name at all is said, not dropped: with Guard mode on, a line lost in silence is a service
    // the agent needs and that gets cut (G4, 28 Sep 2026).
    let mut cleaned: Vec<String> = Vec::new();
    let mut malas: Vec<String> = Vec::new();
    for linea in body
        .patrones
        .lines()
        .map(str::trim)
        .filter(|x| !x.is_empty())
    {
        match guardiana_core::rules::normalizar_nombre(linea) {
            Some((n, _)) if !cleaned.contains(&n) => cleaned.push(n),
            Some(_) => {}
            None => malas.push(linea.chars().take(80).collect()),
        }
    }
    if !malas.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            t.panel("alcance_lineas_malas")
                .replace("{lineas}", &malas.join(", ")),
        )
            .into_response());
    }
    // El Modo Vigilante corta TODO lo que no esté en la lista, así que la regla 6 del brief le
    // vale igual que a un corte suelto: antes de 24 horas observando, no. Sin esto era la manera
    // más fácil de saltársela, y encima la más dañina —y el aviso que enseña el panel se calcula
    // sobre las últimas 24 horas, o sea que en un equipo recién instalado dice un número
    // tranquilizador porque todavía no hay historia. Visto en el repaso del 20 sep 2026.
    if body.modo.as_deref() == Some("cortar") && body.pase_horas.is_none() {
        let dev = with_ledger(&state, |l| l.device(&body.device_id))?;
        let observado = dev.map_or(0, |d| now_ms() - d.first_seen);
        if !observation_complete(observado) {
            return Err((StatusCode::CONFLICT, frase_observando(t, observado)).into_response());
        }
    }
    with_ledger(&state, |l| {
        l.set_setting(&scope_key(&body.device_id), &cleaned.join("\n"))?;
        if let Some(h) = body.pase_horas {
            let hasta = now_ms() + h.clamp(1, 24) * 3600 * 1000;
            l.set_setting(
                &scope_mode_key(&body.device_id),
                &format!("observar:{hasta}"),
            )?;
        } else if let Some(m) = &body.modo {
            let modo = if m == "cortar" { "cortar" } else { "observar" };
            l.set_setting(&scope_mode_key(&body.device_id), modo)?;
        }
        Ok(())
    })?;
    ia(State(state), _s).await
}

/// Body of "add this name to the scope".
#[derive(Deserialize)]
pub(crate) struct AnadirBody {
    device_id: String,
    nombre: String,
}

/// Add one name to a device's declared scope. This is the answer to a name that was cut
/// for falling outside it: from now on it goes through. What was already cut stays cut in
/// the ledger, because it happened.
pub(crate) async fn alcance_anadir(
    State(state): State<Arc<AppState>>,
    _s: Session,
    Json(body): Json<AnadirBody>,
) -> ApiResult<IaView> {
    let Some((nombre, _)) = guardiana_core::rules::normalizar_nombre(&body.nombre) else {
        return Err((StatusCode::BAD_REQUEST, "bad name").into_response());
    };
    with_ledger(&state, |l| {
        let key = scope_key(&body.device_id);
        let mut lineas: Vec<String> = l
            .setting(&key)?
            .unwrap_or_default()
            .lines()
            .map(|x| x.trim().to_owned())
            .filter(|x| !x.is_empty())
            .collect();
        if !lineas.iter().any(|x| x == &nombre) {
            lineas.push(nombre.clone());
        }
        l.set_setting(&key, &lineas.join("\n"))
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

/// The live counters and the latest names. The counters add up every device of the house; the
/// names come through `Ledger::events`, which leaves out the devices whose owner did not share
/// their detail (review of 1 Oct 2026, entry 1): totals yes, names no.
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
        publicidad: c.ads,
        destinos_nuevos: c.new_destinations,
        esperados: c.expected,
        cortados: c.blocked,
        eventos,
    }))
}

/// A burst: several different names asked by the same device within a couple of seconds.
///
/// A row on its own says little; the cluster is the story. When a phone asks for the shop, two
/// of the shop's own tracking hosts, an attribution company and a product-analytics company in
/// the same second, that is one act, not six. Nothing here is inferred about content: it is the
/// same events the ledger already holds, read together instead of one by one.
#[derive(Serialize)]
pub(crate) struct Rafaga {
    /// When the first name of the burst was asked.
    ts: i64,
    device_id: String,
    device_name: Option<String>,
    /// How long the burst lasted, in milliseconds.
    duracion_ms: i64,
    /// The distinct names, in the order they were asked.
    nombres: Vec<EventView>,
    /// The distinct companies behind them, as far as the lists know.
    empresas: Vec<String>,
    /// How many of those companies have a written trade (measurement, attribution, ads...).
    con_oficio: usize,
}

/// Bursts inside one device's events, oldest first: groups separated by less than two seconds
/// with at least `minimo` distinct names. Shared by the bursts list and the receipt.
fn rafagas_de(evs: &[EventView], minimo: usize) -> Vec<Rafaga> {
    const HUECO_MS: i64 = 2000;
    let mut salida: Vec<Rafaga> = Vec::new();
    let mut grupo: Vec<&EventView> = Vec::new();
    let cerrar = |grupo: &Vec<&EventView>, salida: &mut Vec<Rafaga>| {
        let mut vistos: Vec<&EventView> = Vec::new();
        for e in grupo {
            if !vistos.iter().any(|v| v.qname == e.qname) {
                vistos.push(e);
            }
        }
        if vistos.len() < minimo {
            return;
        }
        let mut empresas: Vec<String> = Vec::new();
        for e in &vistos {
            if let Some(c) = e.empresa {
                if !empresas.iter().any(|x| x == c) {
                    empresas.push(c.to_owned());
                }
            }
        }
        let ts = vistos.first().map_or(0, |e| e.ts);
        let fin = vistos.last().map_or(ts, |e| e.ts);
        salida.push(Rafaga {
            ts,
            device_id: vistos
                .first()
                .map_or(String::new(), |e| e.device_id.clone()),
            device_name: vistos.first().and_then(|e| e.device_name.clone()),
            duracion_ms: fin - ts,
            con_oficio: vistos.iter().filter(|e| e.oficio.is_some()).count(),
            empresas,
            nombres: vistos.iter().map(|e| (**e).clone()).collect(),
        });
    };
    for e in evs {
        if grupo.last().is_some_and(|u| e.ts - u.ts > HUECO_MS) {
            cerrar(&grupo, &mut salida);
            grupo.clear();
        }
        grupo.push(e);
    }
    cerrar(&grupo, &mut salida);
    salida
}

/// Bursts of the last hours, newest first. `horas` (default 24) and `min` (default 4 names).
pub(crate) async fn rafagas(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Vec<Rafaga>> {
    let horas: i64 = q.get("horas").and_then(|s| s.parse().ok()).unwrap_or(24);
    let minimo: usize = q.get("min").and_then(|s| s.parse().ok()).unwrap_or(4);
    let since = now_ms() - horas.clamp(1, 24 * 30) * 3600 * 1000;
    let (events, devices) = with_ledger(&state, |l| {
        let events = l.events(&EventFilter {
            since: Some(since),
            limit: Some(5000),
            ..EventFilter::default()
        })?;
        Ok((events, l.devices()?))
    })?;
    let names = names_of(&devices);
    // Oldest first, so a burst is read in the order it happened. Sorted, not reversed: the
    // order the ledger hands them back is not part of its contract, and assuming it produced
    // one burst of 817 names with a negative duration.
    let mut evs: Vec<EventView> = events.into_iter().map(|e| view(t, e, &names)).collect();
    evs.sort_by_key(|e| e.ts);

    let mut por_aparato: HashMap<String, Vec<EventView>> = HashMap::new();
    for e in evs {
        por_aparato.entry(e.device_id.clone()).or_default().push(e);
    }
    let mut salida: Vec<Rafaga> = Vec::new();
    for (_, evs) in por_aparato {
        salida.extend(rafagas_de(&evs, minimo));
    }
    salida.sort_by_key(|r| std::cmp::Reverse(r.ts));
    salida.truncate(20);
    Ok(Json(salida))
}

/// The receipt of one device over a window: the few lines a person can repeat out loud.
///
/// The ledger of ten thousand rows convinces nobody; this does. Every figure here is counted
/// from the same events, and the window is published with it because on the free plan the
/// detail only goes back a day. Nothing about content: the last line says so.
#[derive(Serialize)]
pub(crate) struct Recibo {
    device_id: String,
    device_name: Option<String>,
    desde: i64,
    hasta: i64,
    /// Distinct names asked in the window.
    nombres: usize,
    /// Distinct companies behind them, as far as the lists know.
    empresas: usize,
    /// Of those, how many have a written trade (measurement, attribution, ads...).
    empresas_con_oficio: usize,
    /// The names of those companies, for the sentence.
    oficios: Vec<String>,
    /// Heartbeats seen: name and period in minutes.
    latidos: Vec<(String, u32)>,
    /// Attempts to leave through encrypted DNS, which this ledger cannot see inside.
    evasiones: usize,
    /// Queries answered with a cut in the window.
    cortadas: usize,
    /// The largest burst of the window, when there was one.
    rafaga: Option<Rafaga>,
}

/// One receipt per device. `dias` (default 7) is trimmed to what the ledger actually holds.
pub(crate) async fn recibo(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Vec<Recibo>> {
    let dias: i64 = q.get("dias").and_then(|s| s.parse().ok()).unwrap_or(7);
    let hasta = now_ms();
    let desde = hasta - dias.clamp(1, 90) * 24 * 3600 * 1000;
    let (events, devices) = with_ledger(&state, |l| {
        let events = l.events(&EventFilter {
            since: Some(desde),
            // One PC asks ~40 000 names a week; the old cap of 20 000 made "7 days" read as
            // three or four (review of 8 Oct 2026). A very busy house can still hit this one,
            // and then `desde` says from when the receipt really counts.
            limit: Some(150_000),
            ..EventFilter::default()
        })?;
        Ok((events, l.devices()?))
    })?;
    let names = names_of(&devices);
    let mut evs: Vec<EventView> = events.into_iter().map(|e| view(t, e, &names)).collect();
    evs.sort_by_key(|e| e.ts);

    let mut por_aparato: HashMap<String, Vec<EventView>> = HashMap::new();
    for e in evs {
        por_aparato.entry(e.device_id.clone()).or_default().push(e);
    }
    let mut salida: Vec<Recibo> = Vec::new();
    for (device_id, evs) in por_aparato {
        let mut nombres: Vec<&str> = Vec::new();
        let mut empresas: Vec<&str> = Vec::new();
        let mut oficios: Vec<String> = Vec::new();
        let mut latidos: Vec<(String, u32)> = Vec::new();
        let mut evasiones = 0usize;
        let mut cortadas = 0usize;
        for e in &evs {
            if !nombres.contains(&e.qname.as_str()) {
                nombres.push(&e.qname);
            }
            if let Some(c) = e.empresa {
                if !empresas.contains(&c) {
                    empresas.push(c);
                    if e.oficio.is_some() {
                        oficios.push(c.to_owned());
                    }
                }
            }
            for s in &e.signals {
                match s {
                    Signal::Baliza { minutes } if !latidos.iter().any(|(n, _)| n == &e.qname) => {
                        latidos.push((e.qname.clone(), *minutes));
                    }
                    Signal::EvasionDns => evasiones += 1,
                    _ => {}
                }
            }
            if e.verdict == "cortado" {
                cortadas += 1;
            }
        }
        let rafaga = rafagas_de(&evs, 4)
            .into_iter()
            .max_by_key(|r| r.nombres.len());
        salida.push(Recibo {
            device_name: evs.first().and_then(|e| e.device_name.clone()),
            device_id,
            desde: evs.first().map_or(desde, |e| e.ts),
            hasta,
            nombres: nombres.len(),
            empresas: empresas.len(),
            empresas_con_oficio: oficios.len(),
            oficios,
            latidos,
            evasiones,
            cortadas,
            rafaga,
        });
    }
    salida.sort_by_key(|r| std::cmp::Reverse(r.nombres));
    Ok(Json(salida))
}

#[derive(Serialize)]
pub(crate) struct Extracto {
    /// Stored rows the household may read: what the filters can ever reach.
    total: u64,
    /// Stored rows of devices that do not share their detail: said as a number, never listed.
    ocultos: u64,
    eventos: Vec<EventView>,
    /// Periods in which the service was not watching, newest first.
    huecos: Vec<HuecoView>,
}

/// The household's extract. `?device_id=` of a device whose owner did not share its detail
/// returns no rows: the consent check lives in `Ledger::events`, not here, so the export, the
/// radiography and the report cannot drift from it (review of 1 Oct 2026, entry 1). `total`
/// counts the rows the household may read and `ocultos` the rest: «Mostrando 200 de 5.000» used
/// to count rows no filter could ever show (review of 5 Oct 2026, privacy item 3).
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
    let (events, total, todos, devices, huecos) = with_ledger(&state, |l| {
        Ok((
            l.events(&filter)?,
            l.shared_event_count()?,
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
        ocultos: todos.saturating_sub(total),
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
            // The newest rows are gone (review of 1 Oct 2026, entry 17).
            guardiana_core::ChainFault::Truncated { .. } => t.cli("ledger.fallo.recortado"),
            guardiana_core::ChainFault::HeadMissing => t.cli("ledger.fallo.sin_cabeza"),
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
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Response, Response> {
    // Everything the filters select, not only the rows the page is showing: the page asks for
    // the newest 200, and exporting sent those 200 and nothing else, while the panel promises
    // that the whole extract is yours to take (found exporting 441 events from 1.0.0, 28 Sep
    // 2026: the CSV and the JSON had 200). "Yours" is the household's: a phone that did not
    // share its detail is not in the file either, by the same `Ledger::events` the page uses
    // (review of 1 Oct 2026, entry 1).
    let mut filter = filter_from(&q)?;
    filter.limit = None;
    let events = with_ledger(&state, |l| l.events(&filter))?;
    let json = q.get("formato").is_some_and(|f| f == "json");
    let mut body = Vec::new();
    if json {
        write_json(&events, &mut body).map_err(internal)?;
    } else {
        // The separator of the person's spreadsheet: `;` where decimals are written with a
        // comma (the Spanish- and Portuguese-speaking world), `,` in English.
        let sep = if std::ptr::eq(t, i18n::en()) {
            ','
        } else {
            ';'
        };
        write_csv_for_spreadsheet(&events, sep, &mut body).map_err(internal)?;
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
    Lang(t): Lang,
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
                lectura(t, l, &d.id, d.last_seen, house_last, now, Lector::Casa)?,
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
    let (counts, outbound) = with_ledger(&state, |l| Ok((l.table_counts()?, l.outbound()?)))?;
    // Un solo plan y ninguna poda: mientras Guardiana mire, lo guarda todo, y si deja de mirar
    // lo anotado sigue en el disco hasta que la persona lo borre. No hay nada que matizar.
    let retencion = t.panel("retencion_ilimitada").to_owned();
    Ok(Json(SabeDeTi {
        eventos: counts.events,
        dispositivos: counts.devices,
        reglas: counts.rules,
        nombres_vistos: counts.seen_domains,
        ruta: state.db_path.display().to_string(),
        retencion,
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
    /// Home Mode was on when the trial ended: off for now, a relay keeps the house resolving,
    /// and it comes back on with the licence. Switching it off clears that.
    aparcado: bool,
    /// On, but the computer is on another network than the one it was switched on in: no port
    /// is open there until it is back home (review of 5 Oct 2026).
    fuera_de_casa: bool,
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
    let (on, ip, since, devices, aparcado, red) = with_ledger(state, |l| {
        Ok((
            l.setting(SETTING_HOME_MODE)?.is_some_and(|v| v == "1"),
            l.setting(SETTING_HOME_IP)?.filter(|v| !v.is_empty()),
            l.setting(SETTING_HOME_SINCE)?
                .and_then(|v| v.parse::<i64>().ok()),
            l.devices()?.len(),
            l.setting(SETTING_HOME_APARCADO)?.is_some_and(|v| v == "1"),
            l.setting(SETTING_HOME_RED)?.unwrap_or_default(),
        ))
    })?;
    let fuera_de_casa = on
        && match (
            guardiana_devices::Red::de_texto(&red),
            en_hilo_aparte(guardiana_service::sysdns::default_gateway)
                .map(guardiana_devices::Red::de_puerta),
        ) {
            (Some(casa), Some(ahora)) => !casa.misma(&ahora),
            _ => false,
        };
    let lan = guardiana_devices::local_lan_ipv4();
    let shown_ip = if on { ip } else { lan.map(|i| i.to_string()) };
    let parsed: Option<std::net::Ipv4Addr> = shown_ip.as_deref().and_then(|s| s.parse().ok());
    let url = parsed.map(home::home_url);
    Ok(Hogar {
        licencia: String::new(),
        encendido: on,
        ip_dinamica: parsed.and_then(|ip| en_hilo_aparte(|| home::ip_is_dynamic(ip))),
        suspension: en_hilo_aparte(home::sleep_after_minutes),
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
        aparcado: aparcado && !on,
        fuera_de_casa,
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
    // Home Mode is free (decision 52): no plan of its own. But with the trial over and no Plus
    // the program has stood aside and nothing listens on the LAN: switching it on would show the
    // QR and the router steps for a port nobody answers (review of 5 Oct 2026, serious 14).
    let puede = with_ledger(&state, |l| {
        Ok(guardiana_license::status(l, &state.token, now_ms()).ok())
    })?
    .is_none_or(|s| s.puede_funcionar);
    if !puede {
        return Err((StatusCode::CONFLICT, t.panel("caducado_hogar").to_owned()).into_response());
    }
    let aviso = match home::firewall_allow() {
        Ok(()) => None,
        Err(home::FirewallError::Manual) => Some(t.panel("hogar_firewall_manual").to_owned()),
        Err(home::FirewallError::Command(_)) => Some(t.panel("hogar_no_admin").to_owned()),
    };
    // The network it is switched on in is home: on any other no port opens (5 Oct 2026).
    let red = tokio::task::spawn_blocking(|| {
        guardiana_service::sysdns::default_gateway()
            .map(|p| guardiana_devices::Red::de_puerta(p).texto())
            .unwrap_or_default()
    })
    .await
    .map_err(internal)?;
    with_ledger(&state, |l| {
        l.set_setting(SETTING_HOME_MODE, "1")?;
        l.set_setting(SETTING_HOME_IP, &lan.to_string())?;
        l.set_setting(SETTING_HOME_RED, &red)?;
        l.set_setting(SETTING_HOME_SINCE, &now_ms().to_string())?;
        l.record_change(
            now_ms(),
            ChangeKind::HogarOn,
            ChangeWho::Panel,
            &lan.to_string(),
        )
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
        // Off is off: not even the relay of a stood-down program, nor back on with the licence.
        l.set_setting(SETTING_HOME_APARCADO, "")?;
        l.record_change(now_ms(), ChangeKind::HogarOff, ChangeWho::Panel, "")
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

pub(crate) fn identity_of(peer: SocketAddr) -> String {
    if peer.ip().is_loopback() {
        SELF_DEVICE_ID.to_owned()
    } else {
        // Reads the neighbour table: a process on Windows and on Linux, and a phone asks this
        // every five seconds.
        en_hilo_aparte(|| {
            guardiana_devices::Resolver::default()
                .identify(peer.ip())
                .id
        })
    }
}

pub(crate) async fn mi_dispositivo(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    _p: PorDireccion,
) -> ApiResult<MiDispositivo> {
    let id = identity_of(peer);
    let now = now_ms();
    let (device, totals, events, lectura) = with_ledger(&state, |l| {
        let device = l.device(&id)?;
        // This device's rows only, through the (device, time) index: the totals of the whole
        // house took 2.6 s with 600,000 rows, and every phone asks every 5 seconds.
        let totals = l.device_totals_of(&id, None)?;
        // The device reading about itself: its rows in full, whatever its sharing flag says.
        // The identity comes from the caller's IP, so a phone only ever reaches its own.
        let events = l.own_events(
            &id,
            &EventFilter {
                limit: Some(100),
                ..EventFilter::default()
            },
        )?;
        let house_last = l.devices()?.iter().map(|d| d.last_seen).max().unwrap_or(0);
        let lectura = match &device {
            Some(d) => lectura(t, l, &id, d.last_seen, house_last, now, Lector::ElMismo)?,
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
    _p: PorDireccion,
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
    _p: PorDireccion,
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
    /// The same week, seven days earlier, so the report can say what changed. Only with Plus:
    /// the free plan has already deleted a week-old total.
    anterior: Option<InformeFila>,
    /// Destinations asked for this week and never before, busiest first. Only with Plus, for
    /// the same reason: counting them needs the detail the free plan drops after a day.
    novedades: Vec<InformeNovedad>,
    /// When the week before this one was not watched whole, the moment from which it will be:
    /// until then there is nothing to compare with and nothing is "new" (review of 5 Oct 2026:
    /// the first week compared with zeros and called every destination new).
    comparar_desde: Option<i64>,
    /// The registered data brokers some device of the house talked to this week, most asked
    /// first (decision 194): a company's own entry in a public register, shown with its country
    /// and what it ticked there. Whether to cut them is the person's call; the report only says
    /// who they are. Only with Plus, like everything else here.
    corredores: Vec<InformeCorredor>,
    /// Queries of the week that went to those registrants, all of them together.
    corredores_consultas: i64,
    texto_whatsapp: String,
}

#[derive(Serialize)]
struct InformeCorredor {
    nombre: &'static str,
    pais: Option<String>,
    /// Queries of the week to names this registrant owns.
    consultas: i64,
    /// How many distinct names of its the house asked for.
    nombres: i64,
    declara: Vec<String>,
    derechos: Option<&'static str>,
}

/// The week's names grouped by the registered data broker that owns them, most queried first.
fn corredores_de(t: &Texts, nombres: &[guardiana_core::NameCount]) -> Vec<InformeCorredor> {
    let mut por: HashMap<&'static str, InformeCorredor> = HashMap::new();
    for n in nombres {
        let Some(c) = corredor_de(t, &n.qname) else {
            continue;
        };
        let e = por.entry(c.nombre).or_insert_with(|| InformeCorredor {
            nombre: c.nombre,
            pais: c.pais,
            consultas: 0,
            nombres: 0,
            declara: c.declara,
            derechos: c.derechos,
        });
        e.consultas += n.queries;
        e.nombres += 1;
    }
    let mut v: Vec<InformeCorredor> = por.into_values().collect();
    v.sort_by(|a, b| b.consultas.cmp(&a.consultas).then(a.nombre.cmp(b.nombre)));
    v
}

#[derive(Serialize)]
struct InformeNovedad {
    nombre: String,
    empresa: String,
    categoria: String,
    dispositivo: String,
    consultas: i64,
    visto: i64,
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
    let ahora = now_ms();
    const SEMANA: i64 = 7 * 24 * 60 * 60 * 1000;
    let (w, anterior, novedades, plus, comparar_desde, nombres) = with_ledger(&state, |l| {
        let plus = guardiana_license::status(l, &state.token, ahora)
            .map(|s| s.plus_activo)
            .unwrap_or(false);
        let semana = l.week_summary(ahora)?;
        // Both comparisons need the previous week watched whole: since the first device was seen.
        let mirando_desde = l.devices()?.iter().map(|d| d.first_seen).min();
        let comparable = mirando_desde.is_some_and(|d| d <= ahora - 2 * SEMANA);
        let comparar_desde = (!comparable).then(|| mirando_desde.unwrap_or(ahora) + 2 * SEMANA);
        // Only asked for with Plus: without the memory Plus keeps, both answers are empty by
        // construction and printing an empty one would read as "nothing changed". The week is
        // figures per device, so every device is in it; the new destinations carry names, so a
        // device whose owner did not share its detail is left out of them by the ledger itself
        // (review of 1 Oct 2026, entry 1).
        let (anterior, novedades) = if plus && comparable {
            (
                Some(l.week_summary(ahora - SEMANA)?),
                l.new_destinations(ahora, 12)?,
            )
        } else {
            (None, Vec::new())
        };
        // The week's names, for the registered data brokers among them: only with Plus, like
        // the rest of the report, and the detail a device kept to itself is not in them.
        let nombres = if plus {
            l.week_names(ahora)?
        } else {
            Vec::new()
        };
        Ok((semana, anterior, novedades, plus, comparar_desde, nombres))
    })?;
    if !plus {
        // There is no free plan any more: reaching this means the trial or the subscription is
        // over and Guardiana has stood down. It shows nothing rather than an invented example —
        // the work is what is paid for. Taking your own raw data away still works, from the
        // extract page, because that data is yours.
        return Ok(Json(Informe {
            plus: false,
            ejemplo: false,
            anterior: None,
            novedades: Vec::new(),
            comparar_desde: None,
            corredores: Vec::new(),
            corredores_consultas: 0,
            desde: w.since,
            hasta: w.until,
            dispositivos: Vec::new(),
            total: InformeFila::default(),
            texto_whatsapp: String::new(),
        }));
    }
    let corredores = corredores_de(t, &nombres);
    let corredores_consultas: i64 = corredores.iter().map(|c| c.consultas).sum();
    let mut texto_whatsapp = t
        .panel("informe_texto")
        .replace("{dispositivos}", &t.cuantos_dispositivos(w.devices.len()))
        .replace("{consultas}", &w.total.queries.to_string())
        .replace("{rastreadores}", &w.total.trackers.to_string())
        .replace("{publicidad}", &w.total.ads.to_string())
        .replace("{cortados}", &w.total.blocked.to_string());
    if !corredores.is_empty() {
        // One more sentence, only when there is something to say: how many queries went to
        // companies registered as data brokers, and how many of them.
        texto_whatsapp.push(' ');
        texto_whatsapp.push_str(
            &t.panel_n("informe_corredores_texto", corredores.len() as i64)
                .replace("{consultas}", &corredores_consultas.to_string()),
        );
    }
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
        anterior: anterior.as_ref().map(|a| fila(&a.total)),
        novedades: novedades
            .into_iter()
            .map(|n| InformeNovedad {
                empresa: empresa_de(&n.qname).unwrap_or_default().to_owned(),
                nombre: n.qname,
                categoria: n.category,
                dispositivo: n.name.unwrap_or(n.device_id),
                consultas: n.queries,
                visto: n.first_seen,
            })
            .collect(),
        comparar_desde,
        corredores,
        corredores_consultas,
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
    /// Rules left out of this list because a device that does not share made them for itself.
    ocultas: usize,
    modo_bloqueo: String,
}

/// `created_by` of a rule made in the household panel.
const CREADA_EN_PANEL: &str = "usuario (panel)";
/// `created_by` of a rule a device made from its own page.
const CREADA_EN_DISPOSITIVO: &str = "usuario (dispositivo)";

/// The rules as `only_device` may read them: a device's own page gets its device rules; the
/// household gets everything except the rules a device that does not share made for itself.
/// Those name what the phone wanted to stop seeing, which is its detail: the household learns
/// how many there are, not which (review of 5 Oct 2026, privacy item 5). Rules the household
/// itself set on a device stay listed, so it can always undo them.
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
    let comparte = |id: &str| {
        id == SELF_DEVICE_ID
            || devices
                .iter()
                .any(|d| d.id == id && d.share_detail_with_home)
    };
    let mut ocultas = 0;
    let reglas = rules
        .into_iter()
        .filter(|r| match only_device {
            Some(d) => r.scope == Scope::Device && r.device_id.as_deref() == Some(d),
            None => {
                let suya_y_callada = r.scope == Scope::Device
                    && r.created_by == CREADA_EN_DISPOSITIVO
                    && !r.device_id.as_deref().is_some_and(comparte);
                if suya_y_callada {
                    ocultas += 1;
                }
                !suya_y_callada
            }
        })
        .map(|r| rule_view(r, &names, now))
        .collect();
    Ok(Reglas {
        reglas,
        ocultas,
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

/// Cuánto lleva GUARDIANA mirando **esta casa**: el propio computador si ya está en el extracto y,
/// si no, el aparato más antiguo que haya. Un corte que vale para toda la casa se mide con esto,
/// igual que el de un aparato se mide con el suyo.
///
/// Sin esto, la regla 6 del brief —«antes de 24 horas de observación el botón cortar no existe»—
/// se saltaba por la puerta de delante: bastaba elegir «toda la casa» en el formulario de reglas,
/// que además es la opción que viene puesta. Visto en el repaso del 20 sep 2026.
fn observado_casa(state: &AppState) -> Result<i64, Response> {
    let devices = with_ledger(state, |l| l.devices())?;
    let desde = devices
        .iter()
        .find(|d| d.id == SELF_DEVICE_ID)
        .map(|d| d.first_seen)
        .or_else(|| devices.iter().map(|d| d.first_seen).min());
    Ok(desde.map_or(0, |f| now_ms() - f))
}

/// «Guardiana está observando: N horas de 24», con la frase en singular cuando toca.
fn frase_observando(t: &Texts, observado: i64) -> String {
    let h = observed_hours(observado);
    if h == 1 {
        t.panel("observando_una").to_owned()
    } else {
        t.panel("observando").replace("{h}", &h.to_string())
    }
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
    let mut match_kind: MatchKind = body
        .match_kind
        .parse()
        .map_err(|e: guardiana_core::Error| bad(e.to_string()))?;
    let action: Action = body
        .action
        .parse()
        .map_err(|e: guardiana_core::Error| bad(e.to_string()))?;
    let mut pattern = body
        .pattern
        .trim()
        .trim_end_matches('.')
        .to_ascii_lowercase();
    // A pasted address can be long (tracking parameters): the length is that of the name, below.
    if pattern.is_empty() || pattern.len() > 2048 {
        return Err(bad(t.panel("regla_patron_invalido").to_owned()));
    }
    if match_kind == MatchKind::Category {
        if pattern.parse::<Category>().is_err() {
            return Err(bad(t.panel("regla_patron_invalido").to_owned()));
        }
    } else {
        // What a query carries, whatever was typed: `https://www.x.com/…` is `www.x.com`, and
        // `*.x.com` is `x.com` and everything below it (G4, 28 Sep 2026). Until 1.0.2 such a rule
        // was listed as cutting and never matched anything.
        let Some((nombre, comodin)) = guardiana_core::rules::normalizar_nombre(&pattern) else {
            return Err(bad(t.panel("regla_patron_invalido").to_owned()));
        };
        if nombre.len() > 253 {
            return Err(bad(t.panel("regla_patron_invalido").to_owned()));
        }
        pattern = nombre;
        if comodin {
            match_kind = MatchKind::Suffix;
        }
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
        // La espera de 24 horas, con la línea donde el responsable la puso el 20 sep 2026:
        // **un nombre concreto que elige la persona se corta desde el primer minuto**, porque esa
        // es su decisión sobre una cosa que puede señalar con el dedo y deshacer en un clic. Lo
        // que sigue esperando el día entero es todo lo ANCHO —una categoría entera, toda la casa
        // y el Modo Vigilante—, porque ahí nadie puede saber qué se lleva por delante hasta que
        // Guardiana haya visto un día de esta casa. Antes de las 24 h, el corte de un nombre pide
        // confirmación y dice cuánto lleva mirando: la persona decide sabiendo lo que no se sabe.
        let ancho = scope == Scope::Home || match_kind == MatchKind::Category;
        let observed = if let Some(d) = device_id.as_deref() {
            let dev = with_ledger(state, |l| l.device(d))?;
            let Some(dev) = dev else {
                return Err(bad(t.panel("regla_patron_invalido").to_owned()));
            };
            now_ms() - dev.first_seen
        } else {
            observado_casa(state)?
        };
        if ancho && !observation_complete(observed) {
            return Ok(AltaRegla {
                creada: None,
                necesita: None,
                mensaje: Some(frase_observando(t, observed)),
            });
        }
        // Cortar un nombre no pregunta nada: el botón se convierte en «desbloquear» y deshacerlo
        // es el mismo clic, así que la ventana de «¿seguro?» solo estorbaba (el responsable la
        // quitó el 21 sep 2026: «quita el primer aviso al cortar, no es necesario»).
        //
        // Quedan las dos que NO son formalidad y no se quitan:
        //   · un nombre de los que el equipo necesita —actualizaciones, hora, mensajería—, que es
        //     palabra por palabra la regla 6 del brief y puede dejar el aparato sin ellas;
        //   · y el aparato con menos de un día mirado, que es la decisión 187 de él mismo.
        //
        // The first one also for a rule for the whole home: a name the device needs is the same
        // name whoever the rule is for, and the resolver never cuts it without the "yes". Until
        // 1.0.2 a home rule on one skipped the question, was created unconfirmed, and was listed
        // as cutting while it cut nothing (G4, 28 Sep 2026).
        if !body.confirmed && match_kind != MatchKind::Category {
            let catalog = guardiana_lists::Catalog::bundled();
            // A wildcard (`*.microsoft.com`) reaches whatever is under it: the same question when
            // something the computer needs is there, or it was listed as cutting a family whose
            // update servers it never cut (second pass of 5 Oct 2026).
            let toca_esperado = catalog.category(&pattern) == Category::Esperado
                || (match_kind == MatchKind::Suffix && catalog.has_expected_at_or_under(&pattern));
            let frase = if toca_esperado {
                Some(t.panel("regla_esperado_confirmar"))
            } else if ancho || observation_complete(observed) {
                None
            } else {
                // Sin el número de horas: el responsable leyó la frase larga en pantalla y no la
                // entendió («eso no hay quien lo entienda»). Dos frases y el nombre.
                Some(t.panel("cortar_pronto_confirmar"))
            };
            if let Some(frase) = frase {
                return Ok(AltaRegla {
                    creada: None,
                    necesita: Some("confirmar".to_owned()),
                    mensaje: Some(frase.replace("{nombre}", &pattern)),
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
    Ok(Json(create_rule(t, &state, body, None, CREADA_EN_PANEL)?))
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
        "nxdomain" => "nxdomain",
        _ => return Err((StatusCode::BAD_REQUEST, "unknown block mode").into_response()),
    };
    with_ledger(&state, |l| l.set_setting("block_mode", modo))?;
    Ok(Json(modo.to_owned()))
}

// Phone: its own rules, by IP, no token.

pub(crate) async fn mi_reglas(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    _p: PorDireccion,
) -> ApiResult<Reglas> {
    let id = identity_of(peer);
    Ok(Json(list_rules(&state, Some(&id))?))
}

pub(crate) async fn mi_nueva_regla(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    _p: PorDireccion,
    Json(body): Json<NuevaRegla>,
) -> ApiResult<AltaRegla> {
    let id = identity_of(peer);
    Ok(Json(create_rule(
        t,
        &state,
        body,
        Some(&id),
        CREADA_EN_DISPOSITIVO,
    )?))
}

pub(crate) async fn mi_deshacer_regla(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    _p: PorDireccion,
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
        texto: guardiana_verify::render_para_panel(t, &informe),
        informe,
    }))
}

// ----- licence (token) ------------------------------------------------------

#[derive(Serialize)]
pub(crate) struct LicenciaView {
    estado: guardiana_license::Status,
    texto: String,
    /// Dónde está la marca que impide que borrar el extracto devuelva los siete días. Se enseña
    /// porque no se esconde: el programa no deja nada en el equipo que no cuente.
    marca_prueba: String,
    /// Whether the trial mark is really there: written as best effort, read back before it is
    /// named (review of 1 Oct 2026, entry 24; app.js claimed it without looking until 1.0.2).
    marca_puesta: bool,
    /// Where the copy of a paid licence lives (key, activation number, product), for the same
    /// reason the trial mark is named.
    marca_licencia: String,
    clave_dev: bool,
    host_activacion: &'static str,
    /// Hours observed so far (the trial needs 24).
    horas_observadas: i64,
    conexiones: Vec<OutboundView>,
}

fn licencia_error(t: &Texts, e: &guardiana_license::Error) -> String {
    match e {
        guardiana_license::Error::Malformed(_) => t.panel("licencia_err_formato").to_owned(),
        guardiana_license::Error::KeyRejected(why) => {
            t.panel("licencia_err_clave").replace("{motivo}", why)
        }
        // Refused by the licence crate before anything leaves the machine (review of 1 Oct
        // 2026, entry 26); `licencia_clave` below already answers 400 for it.
        guardiana_license::Error::EmptyKey => t.panel("licencia_err_clave_vacia").to_owned(),
        guardiana_license::Error::Network(why) => {
            t.panel("licencia_err_red").replace("{motivo}", why)
        }
        guardiana_license::Error::AlreadyPlus => t.panel("licencia_ya_plus").to_owned(),
        guardiana_license::Error::Ledger(err) => err.to_string(),
    }
}

fn licencia_texto(t: &Texts, s: &guardiana_license::Status, zona: i64) -> String {
    use guardiana_license::{Comprobacion, Plan};
    let day = |ms: i64| guardiana_core::time::local_day(ms, zona);
    match &s.plan {
        Plan::Prueba {
            termina,
            dias_restantes,
            ..
        } => t
            .panel(if *dias_restantes == 1 {
                "licencia_prueba_uno"
            } else {
                "licencia_prueba"
            })
            .replace("{d}", &dias_restantes.to_string())
            .replace("{fecha}", &day(*termina)),
        Plan::PruebaAgotada { termino } => t
            .panel("licencia_prueba_agotada")
            .replace("{fecha}", &day(*termino)),
        Plan::Plus {
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
                .replace("{origen}", t.panel("licencia_origen_clave"))
                .replace("{fecha}", &day(*desde))
                .replace("{titular}", titular.as_deref().unwrap_or("-"));
            match (periodo_dias, proxima_comprobacion, comprobacion) {
                // Una licencia comprada una vez no tiene periodo ni próxima comprobación, así
                // que sin esta rama el panel se quedaba callado justo con quien más pagó. Se
                // dice lo bueno y lo otro: no caduca por estar sin conexión, pero una
                // devolución sí la termina.
                (Some(p), _, _) if *p == guardiana_license::DE_POR_VIDA => {
                    text.push(' ');
                    text.push_str(t.panel("licencia_de_por_vida"));
                    // The gateway turned the key down: from when it stops, said with its date.
                    if let Some(c) = caduca_ms {
                        text.push(' ');
                        text.push_str(&t.panel("licencia_caduca").replace("{fecha}", &day(*c)));
                    }
                }
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
                            // The deadline exists only once a check has failed: before that it
                            // was the date the check fell due, already past (licence item 2).
                            .replace("{limite}", &caduca_ms.map(day).unwrap_or_default()),
                    );
                }
                _ => {
                    if let Some(c) = caduca_ms {
                        text.push(' ');
                        text.push_str(&t.panel("licencia_caduca").replace("{fecha}", &day(*c)));
                    }
                }
            }
            text
        }
        Plan::PlusTerminado { termino, motivo } => t
            .panel(match motivo.as_str() {
                "cancelada" => "licencia_plus_cancelada",
                "rechazada" => "licencia_plus_rechazada",
                _ => "licencia_plus_sin_comprobar",
            })
            .replace("{fecha}", &day(*termino)),
    }
}

fn licencia_view(t: &Texts, state: &AppState, zona: i64) -> Result<LicenciaView, Response> {
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
        texto: licencia_texto(t, &estado, zona),
        marca_prueba: guardiana_license::ancla::donde(),
        marca_puesta: guardiana_license::ancla::leer().is_some(),
        marca_licencia: guardiana_license::ancla::donde_licencia(),
        clave_dev: guardiana_core::identity::public_key_is_dev(),
        host_activacion: guardiana_license::gateway_host(),
        horas_observadas: estado.observado_ms / guardiana_core::time::HOUR_MS,
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

pub(crate) async fn licencia(
    Lang(t): Lang,
    Zona(zona): Zona,
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<LicenciaView> {
    Ok(Json(licencia_view(t, &state, zona)?))
}

#[derive(Deserialize)]
pub(crate) struct ClaveBody {
    clave: String,
}

pub(crate) async fn licencia_clave(
    Lang(t): Lang,
    Zona(zona): Zona,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Json(body): Json<ClaveBody>,
) -> ApiResult<LicenciaView> {
    let key = body.clave.trim().to_owned();
    if key.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            t.panel("licencia_err_clave_vacia").to_owned(),
        )
            .into_response());
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
    Ok(Json(licencia_view(t, &state, zona)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use guardiana_license::{Plan, Status};

    fn estado(plan: Plan, puede_funcionar: bool) -> Status {
        Status {
            plan,
            plus_activo: puede_funcionar,
            puede_funcionar,
            hogar_permitido: true,
            observado_ms: 0,
        }
    }

    /// Cuando la prueba se acaba, la pantalla de licencia lo dice con su fecha y no promete
    /// nada más. Antes hablaba de un plan gratis al que se volvía; ese plan ya no existe.
    #[test]
    fn al_acabarse_la_prueba_la_pantalla_lo_dice_con_su_fecha() {
        let t = guardiana_core::i18n::es();
        let fin = 1_760_000_000_000;
        let texto = licencia_texto(t, &estado(Plan::PruebaAgotada { termino: fin }, false), 0);
        let dia = guardiana_core::time::rfc3339_utc(fin)[..10].to_owned();
        assert!(texto.contains(&dia), "{texto}");
    }

    /// Mientras la prueba corre, la pantalla dice cuántos días quedan: que se acabe no puede
    /// ser una sorpresa.
    #[test]
    fn mientras_corre_la_prueba_se_dicen_los_dias_que_quedan() {
        let t = guardiana_core::i18n::es();
        let texto = licencia_texto(
            t,
            &estado(
                Plan::Prueba {
                    empieza: 0,
                    termina: 7 * guardiana_core::time::DAY_MS,
                    dias_restantes: 3,
                },
                true,
            ),
            0,
        );
        assert!(texto.contains('3'), "{texto}");
    }

    /// The end of the trial is the person's own day: 21:55 in Bogotá on the 4th is the 5th in UTC
    /// (review of 5 Oct 2026, licence medium).
    #[test]
    fn la_fecha_es_la_del_dia_de_quien_mira() {
        let t = guardiana_core::i18n::es();
        let fin = 1_791_168_900_000; // 2026-10-05T02:55Z
        let bogota = licencia_texto(
            t,
            &estado(Plan::PruebaAgotada { termino: fin }, false),
            -300,
        );
        assert!(bogota.contains("2026-10-04"), "{bogota}");
        let madrid = licencia_texto(t, &estado(Plan::PruebaAgotada { termino: fin }, false), 120);
        assert!(madrid.contains("2026-10-05"), "{madrid}");
    }
}
