//! «Contratos» (1.0.45, «contrato vivo»): the page and the API over `guardiana_core::contratos`.
//!
//! The owner, 18 Oct 2026: «la experiencia del cliente debe ser sentir el monitoreo constante…
//! para saber que todo está bien y nada está vigilando o extrayendo sus datos». This is the part
//! that says «todo sigue como se aprendió» most of the time, and, the day a program updates and
//! starts contacting companies it never contacted, says that and nothing more: «empezó a
//! contactar», never an accusation. A name asked for is not data sent, and every notice says so.
//!
//! What lives here: reading a name as a company (the lists' owner, else its site), the watcher
//! that runs a pass over the new extract rows every few seconds, the sentences in the person's
//! language, and the routes. The learning, the notices, the cuts and the chained register are in
//! the core crate, with their tests.

// Handlers return a full HTTP `Response` as their error, as axum intends (as in api.rs).
#![allow(clippy::result_large_err)]

use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use guardiana_core::contratos::{
    Aviso, ComprobacionRegistro, Contrato, EstadoAviso, EstadoContrato, Lectura, Registro,
    APRENDE_DIAS, APRENDE_LANZAMIENTOS,
};
use guardiana_core::i18n::Texts;
use guardiana_core::time::{now_ms, DAY_MS};
use serde::{Deserialize, Serialize};

use crate::api::{with_ledger, ApiResult};
use crate::auth::{Lang, Session};
use crate::AppState;

/// Seconds between two passes of the watcher.
const CADA_S: u64 = 15;
/// Notices listed on the page.
const MAX_AVISOS_PAGINA: usize = 100;
/// Register rows listed on the page.
const MAX_REGISTRO_PAGINA: usize = 300;

const HTML: &str = include_str!("../static/contratos.html");
const JS: &str = include_str!("../static/contratos.js");

/// How a name is read for a contract: its company (the lists' owner, else its site) and its
/// site. Local names, reverse lookups, single labels and GUARDIANA's own self-check say nothing
/// about a program's contract and are skipped.
pub(crate) fn lee(qname: &str) -> Option<Lectura> {
    let q = qname.trim_end_matches('.').to_ascii_lowercase();
    if q.is_empty()
        || !q.contains('.')
        || guardiana_lists::is_local_name(&q)
        || q.ends_with(guardiana_core::SELF_CHECK_SUFFIX)
    {
        return None;
    }
    let sitio = guardiana_lists::dominio::sitio(&q);
    let empresa = crate::api::empresa_de(&q).map_or_else(|| sitio.clone(), str::to_owned);
    Some(Lectura { empresa, sitio })
}

/// The routes of the page and its API, merged into the panel's router.
pub(crate) fn rutas() -> Router<Arc<AppState>> {
    Router::new()
        .route("/contratos", get(|| async { crate::pagina(HTML) }))
        .route("/contratos.html", get(|| async { crate::pagina(HTML) }))
        .route(
            "/static/contratos.js",
            get(|| async { crate::static_response(JS, "application/javascript; charset=utf-8") }),
        )
        .route("/api/contratos", get(resumen))
        .route("/api/contratos/atencion", get(atencion))
        .route("/api/contratos/registro", get(registro))
        .route("/api/contratos/comprobar", get(comprobar))
        .route("/api/contratos/fijar", post(fijar))
        .route("/api/contratos/avisos/{id}/aceptar", post(aceptar))
        .route("/api/contratos/avisos/{id}/cortar", post(cortar))
        .route("/api/contratos/avisos/{id}/deshacer", post(deshacer))
}

/// The watcher: a pass every [`CADA_S`] seconds, on the panel's own connection, so a fixed
/// contract's cut happens even with no page open. It stops with the panel.
pub(crate) fn vigia(
    state: Arc<AppState>,
    stop: Arc<tokio::sync::Notify>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        // Registered before the first pass, so a stop asked while a pass runs is not missed.
        let parar = stop.notified();
        tokio::pin!(parar);
        parar.as_mut().enable();
        let mut cada = tokio::time::interval(Duration::from_secs(CADA_S));
        loop {
            tokio::select! {
                () = &mut parar => break,
                _ = cada.tick() => {}
            }
            // Catch up in batches when there is a backlog (the first pass after an update reads
            // the history already in the extract), yielding between them.
            for _ in 0..20 {
                let leidas = with_ledger(&state, |l| l.contratos_pasada(&lee, now_ms()))
                    .map(|p| p.leidas)
                    .unwrap_or(0);
                if leidas < 5_000 {
                    break;
                }
                tokio::task::yield_now().await;
            }
        }
    })
}

/// `Zoom.exe` → `Zoom`.
fn visible(nombre: &str) -> String {
    let n = nombre.trim();
    if n.len() > 4 && n[n.len() - 4..].eq_ignore_ascii_case(".exe") {
        n[..n.len() - 4].to_owned()
    } else {
        n.to_owned()
    }
}

/// `6.2.11 (49849)` → `6.2.11`: the number people know.
fn version_corta(v: &str) -> &str {
    v.split(' ').next().unwrap_or(v)
}

/// The notice's sentence: «Zoom 6.2 ha cambiado: empezó a contactar con 3 empresas nuevas».
pub(crate) fn titulo(t: &Texts, a: &Aviso) -> String {
    let n = i64::try_from(a.nuevas.len()).unwrap_or(i64::MAX);
    let programa = visible(&a.programa);
    let clave = if !a.version_cambio() {
        "ct_aviso"
    } else if a.version_ahora.starts_with("huella ") {
        "ct_aviso_archivo"
    } else {
        "ct_aviso_version"
    };
    t.panel_n(clave, n)
        .replace("{programa}", &programa)
        .replace("{version}", version_corta(&a.version_ahora))
}

fn estado_texto(t: &Texts, e: EstadoContrato) -> &str {
    t.panel(match e {
        EstadoContrato::Aprendiendo => "ct_estado_aprendiendo",
        EstadoContrato::Vigilando => "ct_estado_vigilando",
        EstadoContrato::Fijado => "ct_estado_fijado",
        EstadoContrato::NoAplica => "ct_estado_no_aplica",
    })
}

fn estado_aviso(e: EstadoAviso) -> &'static str {
    match e {
        EstadoAviso::Abierto => "abierto",
        EstadoAviso::Aceptado => "aceptado",
        EstadoAviso::Cortado => "cortado",
        EstadoAviso::CortadoAuto => "cortado_auto",
        EstadoAviso::Deshecho => "deshecho",
    }
}

#[derive(Serialize)]
struct EmpresaVistaApi {
    empresa: String,
    nombres: Vec<String>,
    sitios: Vec<String>,
    veces: u64,
    desde: i64,
    ultimo: i64,
    version: String,
    esperado: bool,
}

fn empresas_api(
    m: &std::collections::BTreeMap<String, guardiana_core::contratos::EmpresaVista>,
) -> Vec<EmpresaVistaApi> {
    m.iter()
        .map(|(empresa, e)| EmpresaVistaApi {
            empresa: empresa.clone(),
            nombres: e.nombres.iter().cloned().collect(),
            sitios: e.sitios.iter().cloned().collect(),
            veces: e.veces,
            desde: e.desde,
            ultimo: e.ultimo,
            version: e.version.clone(),
            esperado: e.esperado,
        })
        .collect()
}

/// A notice as the page shows it.
#[derive(Serialize)]
pub(crate) struct AvisoApi {
    id: i64,
    clave: String,
    programa: String,
    titulo: String,
    nota: String,
    version_antes: String,
    version_ahora: String,
    version_cambio: bool,
    desde: i64,
    ultimo: i64,
    estado: &'static str,
    estado_texto: String,
    antes: Vec<String>,
    nuevas: Vec<EmpresaVistaApi>,
    reglas: usize,
    reglas_texto: String,
    sin_cortar: Vec<String>,
    resuelto: Option<i64>,
}

fn aviso_api(t: &Texts, a: &Aviso) -> AvisoApi {
    let estado = estado_aviso(a.estado);
    let n_reglas = a.reglas.len();
    AvisoApi {
        id: a.id,
        clave: a.clave.clone(),
        programa: visible(&a.programa),
        titulo: titulo(t, a),
        nota: t.panel("ct_aviso_nota").to_owned(),
        version_antes: a.version_antes.clone(),
        version_ahora: a.version_ahora.clone(),
        version_cambio: a.version_cambio(),
        desde: a.desde,
        ultimo: a.ultimo,
        estado,
        estado_texto: t.panel(&format!("ct_aviso_estado_{estado}")).to_owned(),
        antes: a.antes.clone(),
        nuevas: empresas_api(&a.nuevas),
        reglas: n_reglas,
        reglas_texto: if n_reglas == 0 {
            String::new()
        } else {
            t.panel_n(
                "ct_reglas_hechas",
                i64::try_from(n_reglas).unwrap_or(i64::MAX),
            )
        },
        sin_cortar: a
            .sin_cortar
            .iter()
            .map(|s| {
                t.panel(&format!("ct_sin_cortar_{}", s.motivo))
                    .replace("{empresa}", &s.empresa)
                    .replace("{con}", &visible(&s.con))
            })
            .collect(),
        resuelto: a.resuelto,
    }
}

/// A program's contract as the page shows it.
#[derive(Serialize)]
struct ContratoApi {
    clave: String,
    programa: String,
    nombre: String,
    ruta: String,
    version: String,
    firmado_por: String,
    estado: EstadoContrato,
    estado_texto: String,
    motivo_texto: String,
    desde: i64,
    ultimo: i64,
    aprendido: Option<i64>,
    /// When the learning ends at the latest (the day count), while it is learning.
    aprende_hasta: Option<i64>,
    lanzamientos: u32,
    empresas: Vec<EmpresaVistaApi>,
    cortadas: Vec<String>,
    versiones: Vec<String>,
    pendientes: usize,
}

fn contrato_api(t: &Texts, c: &Contrato) -> ContratoApi {
    ContratoApi {
        clave: c.clave.clone(),
        programa: visible(&c.nombre),
        nombre: c.nombre.clone(),
        ruta: c.ruta.clone(),
        version: c.etiqueta(),
        firmado_por: c.firmado_por.clone(),
        estado: c.estado,
        estado_texto: estado_texto(t, c.estado).to_owned(),
        motivo_texto: if c.motivo.is_empty() {
            String::new()
        } else {
            t.panel(&format!("ct_motivo_{}", c.motivo)).to_owned()
        },
        desde: c.desde,
        ultimo: c.ultimo,
        aprendido: c.aprendido,
        aprende_hasta: (c.estado == EstadoContrato::Aprendiendo)
            .then_some(c.desde + APRENDE_DIAS * DAY_MS),
        lanzamientos: c.lanzamientos,
        empresas: empresas_api(&c.empresas),
        cortadas: c.cortadas.iter().cloned().collect(),
        versiones: c.versiones.iter().map(|v| v.etiqueta.clone()).collect(),
        pendientes: c.pendientes.len(),
    }
}

/// One register row as the page shows it.
#[derive(Serialize)]
pub(crate) struct RegistroApi {
    id: i64,
    ts: i64,
    clave: String,
    tipo: String,
    frase: String,
    hash: String,
    cabeza_extracto: String,
}

fn lista(v: &serde_json::Value) -> String {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

/// The sentence of a register row, from its detail.
fn frase_registro(t: &Texts, r: &Registro) -> String {
    let d: serde_json::Value = serde_json::from_str(&r.detalle).unwrap_or_default();
    let s = |k: &str| d[k].as_str().unwrap_or_default().to_owned();
    let n_empresas = d["empresas"].as_array().map_or(0, Vec::len);
    let clave = format!("ct_reg_{}", r.tipo);
    let base = if r.tipo == "aprendido" {
        t.panel(&clave).replace("{n}", &n_empresas.to_string())
    } else {
        t.panel(&clave).to_owned()
    };
    base.replace("{programa}", &visible(&s("programa")))
        .replace("{version}", &s("version"))
        .replace("{antes}", &s("antes"))
        .replace("{ahora}", &s("ahora"))
        .replace("{empresa}", &s("empresa"))
        .replace("{nombre}", &s("nombre"))
        .replace("{empresas}", &lista(&d["empresas"]))
}

fn registro_api(t: &Texts, r: &Registro) -> RegistroApi {
    RegistroApi {
        id: r.id,
        ts: r.ts,
        clave: r.clave.clone(),
        tipo: r.tipo.clone(),
        frase: frase_registro(t, r),
        hash: r.hash.clone(),
        cabeza_extracto: r.cabeza_extracto.clone(),
    }
}

/// `/api/contratos`: every program, the notices and the rules of the learning.
#[derive(Serialize)]
pub(crate) struct Resumen {
    /// Windows can say which program asked; elsewhere there are no contracts.
    disponible: bool,
    aprende_dias: i64,
    aprende_lanzamientos: u32,
    contratos: Vec<ContratoApi>,
    avisos: Vec<AvisoApi>,
    abiertos: usize,
}

pub(crate) async fn resumen(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<Resumen> {
    let (contratos, avisos) = with_ledger(&state, |l| {
        l.contratos_pasada(&lee, now_ms())?;
        Ok((l.contratos()?, l.avisos_contrato(false, MAX_AVISOS_PAGINA)?))
    })?;
    let mut contratos: Vec<ContratoApi> = contratos.iter().map(|c| contrato_api(t, c)).collect();
    // Waiting for the person first, then fixed, watched, learning, and those with no contract.
    let orden = |e: EstadoContrato| match e {
        EstadoContrato::Fijado => 1,
        EstadoContrato::Vigilando => 2,
        EstadoContrato::Aprendiendo => 3,
        EstadoContrato::NoAplica => 4,
    };
    contratos.sort_by(|a, b| {
        (b.pendientes > 0)
            .cmp(&(a.pendientes > 0))
            .then(orden(a.estado).cmp(&orden(b.estado)))
            .then(b.ultimo.cmp(&a.ultimo))
    });
    let avisos: Vec<AvisoApi> = avisos.iter().map(|a| aviso_api(t, a)).collect();
    let abiertos = avisos.iter().filter(|a| a.estado == "abierto").count();
    Ok(Json(Resumen {
        disponible: cfg!(windows),
        aprende_dias: APRENDE_DIAS,
        aprende_lanzamientos: APRENDE_LANZAMIENTOS,
        contratos,
        avisos,
        abiertos,
    }))
}

/// `/api/contratos/atencion`: what the top bar's status pill needs to say «Atención».
#[derive(Serialize)]
pub(crate) struct Atencion {
    /// Some program started contacting new companies and the person has not decided yet.
    atencion: bool,
    /// How many notices are waiting.
    abiertos: usize,
    /// The sentence for the pill, empty when there is nothing.
    texto: String,
    /// Where it leads.
    enlace: &'static str,
    /// The waiting notices, newest first: id, program and sentence.
    avisos: Vec<AtencionAviso>,
}

#[derive(Serialize)]
struct AtencionAviso {
    id: i64,
    programa: String,
    titulo: String,
    ultimo: i64,
}

pub(crate) fn atencion_de(t: &Texts, abiertos: &[Aviso]) -> Atencion {
    let mut programas: Vec<String> = abiertos.iter().map(|a| visible(&a.programa)).collect();
    programas.sort();
    programas.dedup();
    let texto = match programas.len() {
        0 => String::new(),
        1 => t
            .panel("ct_pill_atencion_uno")
            .replace("{programa}", &programas[0]),
        n => t.panel_n("ct_pill_atencion", i64::try_from(n).unwrap_or(i64::MAX)),
    };
    Atencion {
        atencion: !abiertos.is_empty(),
        abiertos: abiertos.len(),
        texto,
        enlace: "/contratos",
        avisos: abiertos
            .iter()
            .map(|a| AtencionAviso {
                id: a.id,
                programa: visible(&a.programa),
                titulo: titulo(t, a),
                ultimo: a.ultimo,
            })
            .collect(),
    }
}

pub(crate) async fn atencion(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<Atencion> {
    let abiertos = with_ledger(&state, |l| l.avisos_contrato(true, 50))?;
    Ok(Json(atencion_de(t, &abiertos)))
}

#[derive(Deserialize)]
pub(crate) struct RegistroQuery {
    #[serde(default)]
    clave: Option<String>,
}

/// `/api/contratos/registro`: the chained register, newest first, as sentences.
pub(crate) async fn registro(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Query(q): Query<RegistroQuery>,
) -> ApiResult<Vec<RegistroApi>> {
    let filas = with_ledger(&state, |l| {
        l.registro_contratos(q.clave.as_deref(), MAX_REGISTRO_PAGINA)
    })?;
    Ok(Json(filas.iter().map(|r| registro_api(t, r)).collect()))
}

/// `/api/contratos/comprobar`: recomputes the register's chain.
#[derive(Serialize)]
pub(crate) struct Comprobado {
    #[serde(flatten)]
    r: ComprobacionRegistro,
    texto: String,
}

pub(crate) async fn comprobar(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<Comprobado> {
    let r = with_ledger(&state, |l| l.comprueba_contratos())?;
    let texto = match r.fallo {
        None => t
            .panel("ct_comprobado_ok")
            .replace("{n}", &r.filas.to_string()),
        Some(0) => t.panel("ct_comprobado_cola").to_owned(),
        Some(id) => t
            .panel("ct_comprobado_mal")
            .replace("{id}", &id.to_string()),
    };
    Ok(Json(Comprobado { r, texto }))
}

#[derive(Deserialize)]
pub(crate) struct FijarBody {
    clave: String,
    fijar: bool,
}

fn no_encontrado() -> Response {
    (StatusCode::NOT_FOUND, "no existe o ya está resuelto").into_response()
}

/// `/api/contratos/fijar`: fix a contract or let go of it.
pub(crate) async fn fijar(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Json(body): Json<FijarBody>,
) -> Result<Json<serde_json::Value>, Response> {
    let ahora = now_ms();
    let c = with_ledger(&state, |l| {
        let c = l.contrato_fijar(&body.clave, body.fijar, ahora)?;
        // A fixed contract cuts what is already waiting right away, not in the next pass.
        if c.is_some() {
            l.contratos_pasada(&lee, ahora)?;
        }
        Ok(match c {
            Some(c) => l.contrato(&c.clave)?,
            None => None,
        })
    })?
    .ok_or_else(no_encontrado)?;
    Ok(Json(serde_json::json!({ "contrato": contrato_api(t, &c) })))
}

fn resuelto(
    t: &'static Texts,
    state: &AppState,
    id: i64,
    hacer: impl FnOnce(&mut guardiana_core::Ledger, i64, i64) -> guardiana_core::Result<Option<Aviso>>,
    clave_texto: &str,
) -> Result<Json<serde_json::Value>, Response> {
    let a = with_ledger(state, |l| hacer(l, id, now_ms()))?.ok_or_else(no_encontrado)?;
    let texto = t
        .panel(clave_texto)
        .replace("{programa}", &visible(&a.programa));
    Ok(Json(
        serde_json::json!({ "aviso": aviso_api(t, &a), "texto": texto }),
    ))
}

/// «Aceptar como normal».
pub(crate) async fn aceptar(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, Response> {
    resuelto(
        t,
        &state,
        id,
        |l, id, ahora| l.contrato_aceptar(id, ahora),
        "ct_hecho_aceptar",
    )
}

/// «Cortar solo lo nuevo».
pub(crate) async fn cortar(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, Response> {
    resuelto(
        t,
        &state,
        id,
        |l, id, ahora| l.contrato_cortar(id, ahora),
        "ct_hecho_cortar",
    )
}

/// Undo a cut.
pub(crate) async fn deshacer(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, Response> {
    resuelto(
        t,
        &state,
        id,
        |l, id, ahora| l.contrato_deshacer(id, ahora),
        "ct_hecho_deshacer",
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::collections::BTreeMap;

    use guardiana_core::contratos::{Aviso, EmpresaVista, EstadoAviso};
    use guardiana_core::i18n;

    fn aviso(antes: &str, ahora: &str, n: usize) -> Aviso {
        let mut nuevas = BTreeMap::new();
        for i in 0..n {
            nuevas.insert(format!("Empresa {i}"), EmpresaVista::default());
        }
        Aviso {
            id: 1,
            clave: "zoom".into(),
            programa: "Zoom.exe".into(),
            version_antes: antes.into(),
            version_ahora: ahora.into(),
            desde: 0,
            ultimo: 0,
            antes: vec!["Zoom".into()],
            nuevas,
            estado: EstadoAviso::Abierto,
            resuelto: None,
            reglas: Vec::new(),
            sin_cortar: Vec::new(),
        }
    }

    #[test]
    fn the_notice_says_started_contacting_and_never_more() {
        let t = i18n::by_code("es");
        assert_eq!(
            super::titulo(t, &aviso("6.1.0", "6.2.0 (4567)", 3)),
            "Zoom 6.2.0 ha cambiado: empezó a contactar con 3 empresas nuevas"
        );
        assert_eq!(
            super::titulo(t, &aviso("6.2.0", "6.2.0", 1)),
            "Zoom empezó a contactar con una empresa nueva"
        );
        assert!(
            super::titulo(t, &aviso("huella aaaaaaaa", "huella bbbbbbbb", 2))
                .starts_with("Zoom se actualizó")
        );
        // In every language, the notice and the pill speak of contacting, never of sending.
        for code in ["es", "en", "pt"] {
            let t = i18n::by_code(code);
            for k in [
                "ct_aviso",
                "ct_aviso_version",
                "ct_pill_atencion",
                "ct_reg_novedad",
            ] {
                let s = t.panel(k).to_lowercase();
                for prohibida in [
                    "envió", "envía", "sent ", "sends", "enviou", "roba", "steal", "espía", "spy",
                ] {
                    assert!(!s.contains(prohibida), "{code} {k}: {s}");
                }
            }
        }
    }

    #[test]
    fn the_pill_names_the_program_when_there_is_one() {
        let t = i18n::by_code("es");
        let a = super::atencion_de(t, &[aviso("6.1", "6.2", 2)]);
        assert!(a.atencion);
        assert_eq!(
            a.texto,
            "Atención: Zoom empezó a contactar con empresas nuevas"
        );
        let mut b = aviso("1", "2", 1);
        b.programa = "Teams.exe".into();
        let a = super::atencion_de(t, &[aviso("6.1", "6.2", 2), b]);
        assert_eq!(
            a.texto,
            "Atención: 2 programas empezaron a contactar con empresas nuevas"
        );
        assert!(!super::atencion_de(t, &[]).atencion);
    }

    #[test]
    fn names_that_say_nothing_about_a_program_are_skipped() {
        assert!(super::lee("1.0.0.127.in-addr.arpa").is_none());
        assert!(super::lee("impresora.local").is_none());
        assert!(super::lee("localhost").is_none());
        let l = super::lee("api.ejemplo-que-nadie-conoce.com").unwrap();
        assert_eq!(l.sitio, "ejemplo-que-nadie-conoce.com");
        assert_eq!(l.empresa, "ejemplo-que-nadie-conoce.com");
    }

    /// Every text the page, its script and the sentences built here ask for, in the three files.
    #[test]
    fn every_text_of_the_page_exists_in_the_three_languages() {
        let mut claves: Vec<String> = Vec::new();
        for part in super::HTML.split("data-t=\"").skip(1) {
            if let Some(k) = part.split('"').next() {
                claves.push(k.to_owned());
            }
        }
        for (i, _) in super::JS.match_indices("t('") {
            let antes = super::JS
                .as_bytes()
                .get(i.wrapping_sub(1))
                .copied()
                .unwrap_or(b' ');
            if antes.is_ascii_alphanumeric() || antes == b'_' || antes == b'.' {
                continue;
            }
            if let Some(k) = super::JS[i + 3..].split('\'').next() {
                if !k.ends_with('_') {
                    claves.push(k.to_owned());
                }
            }
        }
        for (i, _) in super::JS.match_indices("tn('") {
            if let Some(k) = super::JS[i + 4..].split('\'').next() {
                claves.push(k.to_owned());
            }
        }
        // Built at run time here.
        for e in ["aprendiendo", "vigilando", "fijado", "no_aplica"] {
            claves.push(format!("ct_estado_{e}"));
        }
        for e in ["abierto", "aceptado", "cortado", "cortado_auto", "deshecho"] {
            claves.push(format!("ct_aviso_estado_{e}"));
        }
        for m in ["navegador", "compartido", "variado"] {
            claves.push(format!("ct_motivo_{m}"));
        }
        for m in ["esperado", "compartido"] {
            claves.push(format!("ct_sin_cortar_{m}"));
        }
        for r in [
            "aprendiendo",
            "aprendido",
            "no_aplica",
            "version",
            "novedad",
            "aceptado",
            "cortado",
            "cortado_auto",
            "deshecho",
            "fijado",
            "soltado",
        ] {
            claves.push(format!("ct_reg_{r}"));
        }
        for k in [
            "ct_aviso",
            "ct_aviso_uno",
            "ct_aviso_version",
            "ct_aviso_version_uno",
            "ct_aviso_archivo",
            "ct_aviso_archivo_uno",
            "ct_aviso_nota",
            "ct_pill_atencion",
            "ct_pill_atencion_uno",
            "ct_reglas_hechas",
            "ct_comprobado_ok",
            "ct_comprobado_mal",
            "ct_comprobado_cola",
            "ct_hecho_aceptar",
            "ct_hecho_cortar",
            "ct_hecho_deshacer",
        ] {
            claves.push(k.to_owned());
        }
        assert!(claves.len() > 40, "{}", claves.len());
        for (nombre, json) in [
            ("es", i18n::ES_JSON),
            ("en", i18n::EN_JSON),
            ("pt", i18n::PT_JSON),
        ] {
            let v: serde_json::Value = serde_json::from_str(json).unwrap();
            let panel = v["panel"].as_object().unwrap();
            let faltan: Vec<&String> = claves.iter().filter(|k| !panel.contains_key(*k)).collect();
            assert!(faltan.is_empty(), "{nombre}: faltan {faltan:?}");
        }
    }
}
