//! Four things customers asked for (18 Oct 2026), each small and on its own:
//!
//! - «Limpiar archivo»: what a photo's or a document's metadata says about the person, and a
//!   copy without it (`guardiana_core::huellas`). The file comes from the page to this program on
//!   this computer, is read in memory and is not kept.
//! - «Antes de instalar»: who signed an installer, as Windows checks it
//!   (`guardiana_apps::instalador`). The file is written for the check in GUARDIANA's own folder
//!   (which only the system can write) and removed right after; it is never sent anywhere.
//! - «Qué se rompe si corto»: before cutting a company or a site, which programs of this computer
//!   used it in the last seven days and how often, from the per-program attribution already in
//!   the ledger. For the «Cortar» component to call (`static/que-se-rompe.js`).
//! - «Modo apagón»: one switch that cuts everything this computer asks for except the essential
//!   list and what the person approves one by one (`guardiana_core::apagon`).
//!
//! Its own routes, page and scripts, so the rest of the panel only gains one `merge` line.

// Handlers answer with the same error type as the rest of the API (see api.rs).
#![allow(clippy::result_large_err)]

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::body::{Body, Bytes, HttpBody as _};
use axum::extract::{DefaultBodyLimit, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use guardiana_core::apagon::{self, Apagon, Esencial};
use guardiana_core::huellas::{self, Informe};
use guardiana_core::time::{now_ms, DAY_MS, HOUR_MS};
use guardiana_core::{DecidedBy, EventFilter, NameGroup, Verdict, SELF_DEVICE_ID};
use serde::{Deserialize, Serialize};

use crate::api::{with_ledger, ApiResult};
use crate::auth::Session;
use crate::AppState;

const HERRAMIENTAS_HTML: &str = include_str!("../static/herramientas.html");
const HERRAMIENTAS_JS: &str = include_str!("../static/herramientas.js");
const QUE_SE_ROMPE_JS: &str = include_str!("../static/que-se-rompe.js");
const HERRAMIENTAS_CSS: &str = include_str!("../static/herramientas.css");

/// The largest installer checked. The biggest common ones (an office suite, a game launcher)
/// are well under it.
const INSTALADOR_MAXIMO: u64 = 4 * 1024 * 1024 * 1024;

/// The routes of this module, merged into the panel's router.
pub(crate) fn rutas() -> Router<Arc<AppState>> {
    let pagina = || async { crate::pagina(HERRAMIENTAS_HTML) };
    Router::new()
        .route("/herramientas", get(pagina))
        .route("/herramientas.html", get(pagina))
        .route(
            "/static/herramientas.js",
            get(|| async {
                crate::static_response(HERRAMIENTAS_JS, "application/javascript; charset=utf-8")
            }),
        )
        .route(
            "/static/herramientas.css",
            get(|| async { crate::static_response(HERRAMIENTAS_CSS, "text/css; charset=utf-8") }),
        )
        .route(
            "/static/que-se-rompe.js",
            get(|| async {
                crate::static_response(QUE_SE_ROMPE_JS, "application/javascript; charset=utf-8")
            }),
        )
        .route(
            "/api/huellas/mirar",
            post(huellas_mirar).layer(DefaultBodyLimit::max(huellas::TAMANO_MAXIMO + 1024)),
        )
        .route(
            "/api/huellas/limpiar",
            post(huellas_limpiar).layer(DefaultBodyLimit::max(huellas::TAMANO_MAXIMO + 1024)),
        )
        .route(
            "/api/instalador/firma",
            post(instalador_firma).layer(DefaultBodyLimit::disable()),
        )
        .route("/api/que-se-rompe", get(que_se_rompe))
        .route("/api/apagon", get(apagon_ver).post(apagon_cambiar))
        .route("/api/apagon/aprobar", post(apagon_aprobar))
        .route("/api/apagon/quitar", post(apagon_quitar))
}

// ---------- Limpiar archivo ----------

#[derive(Serialize)]
struct Mirada {
    #[serde(flatten)]
    informe: Informe,
    bytes: usize,
}

async fn huellas_mirar(_s: Session, cuerpo: Bytes) -> ApiResult<Mirada> {
    Ok(Json(Mirada {
        informe: huellas::mirar(&cuerpo),
        bytes: cuerpo.len(),
    }))
}

async fn huellas_limpiar(_s: Session, cuerpo: Bytes) -> Response {
    match huellas::limpiar(&cuerpo) {
        Ok(limpio) => {
            ([(header::CONTENT_TYPE, "application/octet-stream")], limpio).into_response()
        }
        Err(f) => {
            let (codigo, texto) = match f {
                huellas::Fallo::NoSoportado => (StatusCode::UNPROCESSABLE_ENTITY, "no_soportado"),
                huellas::Fallo::Grande => (StatusCode::PAYLOAD_TOO_LARGE, "grande"),
                huellas::Fallo::Danado => (StatusCode::UNPROCESSABLE_ENTITY, "danado"),
            };
            (codigo, texto).into_response()
        }
    }
}

// ---------- Antes de instalar ----------

/// The extension Windows needs to pick the right signature check; anything else is `.exe`.
fn extension(cabeceras: &HeaderMap) -> &'static str {
    let pedida = cabeceras
        .get("x-guardiana-extension")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .trim_start_matches('.')
        .to_ascii_lowercase();
    match pedida.as_str() {
        "msi" => "msi",
        "msix" => "msix",
        "msixbundle" => "msixbundle",
        "appx" => "appx",
        "cab" => "cab",
        "dll" => "dll",
        _ => "exe",
    }
}

#[derive(Serialize)]
struct Comprobacion {
    #[serde(flatten)]
    firma: guardiana_apps::instalador::Firma,
    bytes: u64,
}

async fn instalador_firma(
    State(state): State<Arc<AppState>>,
    _s: Session,
    cabeceras: HeaderMap,
    mut cuerpo: Body,
) -> Result<Json<Comprobacion>, Response> {
    if !cfg!(windows) {
        return Ok(Json(Comprobacion {
            firma: guardiana_apps::instalador::comprobar(std::path::Path::new("")),
            bytes: 0,
        }));
    }
    let carpeta = state
        .db_path
        .parent()
        .map(|p| p.join("antes-de-instalar"))
        .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, "sin carpeta").into_response())?;
    let mut nombre = [0u8; 12];
    getrandom::fill(&mut nombre)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "sin azar").into_response())?;
    let nombre: String = nombre.iter().map(|b| format!("{b:02x}")).collect();
    let ruta = carpeta.join(format!("{nombre}.{}", extension(&cabeceras)));
    let error =
        |e: std::io::Error| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    std::fs::create_dir_all(&carpeta).map_err(error)?;
    // Written piece by piece as it arrives, so a large installer never sits in memory. Each
    // piece is a few kilobytes: writing it in place costs the resolver nothing it would notice.
    let escrito = async {
        use std::io::Write as _;
        let mut f = std::fs::File::create(&ruta)?;
        let mut total: u64 = 0;
        while let Some(trozo) =
            std::future::poll_fn(|cx| std::pin::Pin::new(&mut cuerpo).poll_frame(cx)).await
        {
            let trozo = trozo.map_err(std::io::Error::other)?;
            if let Ok(datos) = trozo.into_data() {
                total += u64::try_from(datos.len()).unwrap_or(u64::MAX);
                if total > INSTALADOR_MAXIMO {
                    return Err(std::io::Error::other("demasiado grande"));
                }
                f.write_all(&datos)?;
            }
        }
        f.flush()?;
        Ok::<u64, std::io::Error>(total)
    }
    .await;
    let resultado = match escrito {
        Ok(bytes) => {
            let r = ruta.clone();
            tokio::task::spawn_blocking(move || guardiana_apps::instalador::comprobar(&r))
                .await
                .map(|firma| Comprobacion { firma, bytes })
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())
        }
        Err(e) => Err(error(e)),
    };
    // Never kept: the copy goes as soon as Windows has looked at it, whatever it said.
    let _ = std::fs::remove_file(&ruta);
    resultado.map(Json)
}

// ---------- Qué se rompe si corto ----------

#[derive(Deserialize)]
struct Pregunta {
    #[serde(default)]
    empresa: Option<String>,
    #[serde(default)]
    dominio: Option<String>,
}

/// One program of this computer that asked for the names about to be cut.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct ProgramaUso {
    /// File name of the program (`Teams.exe`).
    programa: String,
    /// How many times it asked, in the window.
    consultas: u64,
    /// The last time, Unix ms.
    ultima: i64,
    /// Up to five of the names it asked for, most asked first.
    nombres: Vec<String>,
}

/// What the ledger says about the names a cut would take.
#[derive(Debug, Serialize, PartialEq, Eq, Default)]
pub(crate) struct Uso {
    /// Programs of this computer, most queries first.
    programas: Vec<ProgramaUso>,
    /// Queries of this computer for those names when Windows did not say which program asked.
    sin_programa: u64,
    /// Queries of other devices of the house for them (which program, only this computer says).
    otros_dispositivos: u64,
    /// Distinct names involved.
    nombres: usize,
}

/// Fold the grouped names of the window into what the cut would touch. Pure, for the tests.
pub(crate) fn agrupa(grupos: &[NameGroup], coincide: impl Fn(&str) -> bool) -> Uso {
    let mut por: BTreeMap<String, (u64, i64, BTreeMap<String, u64>)> = BTreeMap::new();
    let mut uso = Uso::default();
    let mut nombres = std::collections::BTreeSet::new();
    for g in grupos.iter().filter(|g| coincide(&g.qname)) {
        nombres.insert(g.qname.as_str());
        if g.device_id != SELF_DEVICE_ID {
            uso.otros_dispositivos += g.queries;
            continue;
        }
        match &g.process {
            Some(p) => {
                let e = por.entry(p.clone()).or_default();
                e.0 += g.queries;
                e.1 = e.1.max(g.last_ts);
                *e.2.entry(g.qname.clone()).or_default() += g.queries;
            }
            None => uso.sin_programa += g.queries,
        }
    }
    uso.nombres = nombres.len();
    uso.programas = por
        .into_iter()
        .map(|(programa, (consultas, ultima, n))| {
            let mut n: Vec<(String, u64)> = n.into_iter().collect();
            n.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            ProgramaUso {
                programa,
                consultas,
                ultima,
                nombres: n.into_iter().take(5).map(|(q, _)| q).collect(),
            }
        })
        .collect();
    uso.programas.sort_by(|a, b| {
        b.consultas
            .cmp(&a.consultas)
            .then_with(|| a.programa.cmp(&b.programa))
    });
    uso
}

/// `name` is `dominio` or under it.
fn bajo(name: &str, dominio: &str) -> bool {
    name == dominio || name.strip_suffix(dominio).is_some_and(|r| r.ends_with('.'))
}

/// Whether a name is one of those the cut would take.
type Coincide = Box<dyn Fn(&str) -> bool + Send>;

#[derive(Serialize)]
struct QueSeRompe {
    /// What was asked about, as understood.
    objetivo: String,
    /// The window: the last seven days, or less when the ledger keeps less detail.
    desde: i64,
    hasta: i64,
    /// The oldest detail this computer's ledger still holds (the free plan keeps a day): the page
    /// says «desde» honestly instead of «7 días» when this is later.
    detalle_desde: Option<i64>,
    #[serde(flatten)]
    uso: Uso,
}

async fn que_se_rompe(
    State(state): State<Arc<AppState>>,
    _s: Session,
    Query(p): Query<Pregunta>,
) -> ApiResult<QueSeRompe> {
    let (objetivo, coincide): (String, Coincide) = if let Some(d) = p
        .dominio
        .as_deref()
        .and_then(guardiana_core::rules::normalizar_nombre)
    {
        let d = d.0;
        (d.clone(), Box::new(move |q: &str| bajo(q, &d)))
    } else if let Some(e) = p.empresa.as_deref().and_then(crate::empresa::nombre_pedido) {
        let dominios: Vec<String> = guardiana_lists::company_record(&e)
            .map(|r| r.domains.iter().map(|d| d.to_ascii_lowercase()).collect())
            .unwrap_or_default();
        let nombre = guardiana_lists::company_record(&e).map_or(e.clone(), |r| r.name.to_owned());
        let n = nombre.clone();
        (
            nombre,
            Box::new(move |q: &str| {
                guardiana_lists::company_of(q) == Some(n.as_str())
                    || guardiana_lists::group_of(q) == Some(n.as_str())
                    || dominios.iter().any(|d| bajo(q, d))
            }),
        )
    } else {
        return Err((StatusCode::BAD_REQUEST, "empresa o dominio").into_response());
    };
    let hasta = now_ms();
    let desde = hasta - 7 * DAY_MS;
    let (grupos, detalle_desde) = with_ledger(&state, |l| {
        let g = l.names_grouped(desde, hasta + 1)?;
        let min: Option<i64> = l
            .connection()
            .query_row(
                "SELECT MIN(ts) FROM events WHERE device_id = ?1",
                [SELF_DEVICE_ID],
                |r| r.get(0),
            )
            .unwrap_or(None);
        Ok((g, min))
    })?;
    Ok(Json(QueSeRompe {
        objetivo,
        desde,
        hasta,
        detalle_desde,
        uso: agrupa(&grupos, coincide),
    }))
}

// ---------- Modo apagón ----------

#[derive(Serialize)]
struct Cortado {
    nombre: String,
    veces: u64,
    ultima: i64,
    programa: Option<String>,
}

#[derive(Serialize)]
struct EstadoApagon {
    #[serde(flatten)]
    apagon: Apagon,
    /// What it never cuts, shown whole.
    esenciales: &'static [Esencial],
    /// What it cut in the last hour, most recent first, for approving one by one.
    cortados: Vec<Cortado>,
    /// The essential list's door is the only one: the panel itself is on `127.0.0.1`.
    panel_local: bool,
}

fn lee(l: &guardiana_core::Ledger) -> Apagon {
    Apagon::de_ajuste(l.setting(apagon::AJUSTE_APAGON).ok().flatten().as_deref())
}

fn estado(state: &AppState) -> Result<EstadoApagon, Response> {
    let ahora = now_ms();
    with_ledger(state, |l| {
        let a = lee(l);
        let eventos = l.own_events(
            SELF_DEVICE_ID,
            &EventFilter {
                verdict: Some(Verdict::Cortado),
                since: Some(ahora - HOUR_MS),
                limit: Some(5_000),
                ..EventFilter::default()
            },
        )?;
        let mut por: BTreeMap<String, Cortado> = BTreeMap::new();
        for e in eventos
            .into_iter()
            .filter(|e| e.decided_by == DecidedBy::ModoApagon)
        {
            let c = por.entry(e.qname.clone()).or_insert_with(|| Cortado {
                nombre: e.qname.clone(),
                veces: 0,
                ultima: 0,
                programa: None,
            });
            c.veces += 1;
            if e.ts >= c.ultima {
                c.ultima = e.ts;
                if let Some(p) = e.process.filter(|p| !p.nombre.is_empty()) {
                    c.programa = Some(p.nombre);
                }
            }
        }
        let mut cortados: Vec<Cortado> = por.into_values().collect();
        cortados.sort_by_key(|c| std::cmp::Reverse(c.ultima));
        cortados.truncate(100);
        Ok(EstadoApagon {
            apagon: a,
            esenciales: apagon::ESENCIALES,
            cortados,
            panel_local: true,
        })
    })
}

async fn apagon_ver(State(state): State<Arc<AppState>>, _s: Session) -> ApiResult<EstadoApagon> {
    Ok(Json(estado(&state)?))
}

#[derive(Deserialize)]
struct Cambio {
    activo: bool,
}

async fn apagon_cambiar(
    State(state): State<Arc<AppState>>,
    _s: Session,
    Json(c): Json<Cambio>,
) -> ApiResult<EstadoApagon> {
    let ahora = now_ms();
    with_ledger(&state, |l| {
        let mut a = lee(l);
        if a.cambia(c.activo, ahora) {
            l.set_setting(apagon::AJUSTE_APAGON, &a.a_ajuste())?;
            // In the chained ledger: when it went on and off cannot be quietly rewritten.
            l.append(apagon::evento_cambio(c.activo, ahora))?;
        }
        Ok(())
    })?;
    Ok(Json(estado(&state)?))
}

#[derive(Deserialize)]
struct Nombre {
    nombre: String,
}

async fn apagon_aprobar(
    State(state): State<Arc<AppState>>,
    _s: Session,
    Json(n): Json<Nombre>,
) -> ApiResult<EstadoApagon> {
    let hecho = with_ledger(&state, |l| {
        let mut a = lee(l);
        let r = a.aprueba(&n.nombre);
        if r.is_some() {
            l.set_setting(apagon::AJUSTE_APAGON, &a.a_ajuste())?;
        }
        Ok(r)
    })?;
    if hecho.is_none() {
        return Err((StatusCode::BAD_REQUEST, "nombre_no_valido").into_response());
    }
    Ok(Json(estado(&state)?))
}

async fn apagon_quitar(
    State(state): State<Arc<AppState>>,
    _s: Session,
    Json(n): Json<Nombre>,
) -> ApiResult<EstadoApagon> {
    with_ledger(&state, |l| {
        let mut a = lee(l);
        if a.quita(&n.nombre) {
            l.set_setting(apagon::AJUSTE_APAGON, &a.a_ajuste())?;
        }
        Ok(())
    })?;
    Ok(Json(estado(&state)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(device: &str, q: &str, p: Option<&str>, n: u64, t: i64) -> NameGroup {
        NameGroup {
            device_id: device.to_owned(),
            qname: q.to_owned(),
            process: p.map(str::to_owned),
            queries: n,
            last_ts: t,
        }
    }

    #[test]
    fn what_breaks_counts_each_program_of_this_computer_and_only_the_names_cut() {
        let grupos = [
            g("self", "teams.microsoft.com", Some("ms-teams.exe"), 40, 10),
            g(
                "self",
                "login.microsoftonline.com",
                Some("ms-teams.exe"),
                5,
                20,
            ),
            g("self", "outlook.office.com", Some("olk.exe"), 12, 15),
            g("self", "teams.microsoft.com", None, 3, 5),
            g("tel-1", "teams.microsoft.com", None, 7, 30),
            g("self", "www.google.com", Some("chrome.exe"), 99, 40),
        ];
        let u = agrupa(&grupos, |q| {
            bajo(q, "microsoft.com") || bajo(q, "office.com") || bajo(q, "microsoftonline.com")
        });
        assert_eq!(u.nombres, 3);
        assert_eq!(u.sin_programa, 3);
        assert_eq!(u.otros_dispositivos, 7);
        assert_eq!(u.programas.len(), 2);
        assert_eq!(u.programas[0].programa, "ms-teams.exe");
        assert_eq!(u.programas[0].consultas, 45);
        assert_eq!(u.programas[0].ultima, 20);
        assert_eq!(
            u.programas[0].nombres,
            vec!["teams.microsoft.com", "login.microsoftonline.com"]
        );
        assert_eq!(u.programas[1].programa, "olk.exe");
        assert!(u.programas.iter().all(|p| p.programa != "chrome.exe"));
    }

    #[test]
    fn nothing_matching_is_an_empty_answer_not_an_error() {
        let u = agrupa(&[g("self", "a.example.com", Some("x.exe"), 1, 1)], |q| {
            bajo(q, "otro.com")
        });
        assert_eq!(u, Uso::default());
    }

    #[test]
    fn a_suffix_is_a_whole_label() {
        assert!(bajo("api.github.com", "github.com"));
        assert!(bajo("github.com", "github.com"));
        assert!(!bajo("notgithub.com", "github.com"));
    }

    #[test]
    fn the_installer_extension_is_one_windows_knows_or_exe() {
        let mut h = HeaderMap::new();
        assert_eq!(extension(&h), "exe");
        h.insert(
            "x-guardiana-extension",
            ".MSI".parse().unwrap_or_else(|_| unreachable!()),
        );
        assert_eq!(extension(&h), "msi");
        h.insert(
            "x-guardiana-extension",
            "..\\..\\x".parse().unwrap_or_else(|_| unreachable!()),
        );
        assert_eq!(extension(&h), "exe");
    }

    /// Every text the page and its two scripts ask for, and every one built from what the
    /// program says (`hx_h_<clave>`, `hx_para_<para>`, `hx_nota_<nota>`), is in the three files.
    #[test]
    #[allow(clippy::unwrap_used)]
    fn every_text_of_the_tools_exists_in_three_languages() {
        let mut claves: Vec<String> = Vec::new();
        for attr in ["data-t=\"", "data-t-placeholder=\""] {
            for parte in HERRAMIENTAS_HTML.split(attr).skip(1) {
                claves.push(parte.split('"').next().unwrap().to_owned());
            }
        }
        for js in [HERRAMIENTAS_JS, QUE_SE_ROMPE_JS] {
            for pref in ["t('", "tn('"] {
                for (i, _) in js.match_indices(pref) {
                    // `t(` itself, not the end of a longer name (`createElement('a')`).
                    let antes = js
                        .as_bytes()
                        .get(i.wrapping_sub(1))
                        .copied()
                        .unwrap_or(b' ');
                    if antes.is_ascii_alphanumeric() || antes == b'_' || antes == b'.' {
                        continue;
                    }
                    let k = js[i + pref.len()..].split('\'').next().unwrap();
                    if !k.is_empty()
                        && !k.ends_with('_')
                        && k.chars().all(|c| c.is_ascii_lowercase() || c == '_')
                    {
                        claves.push(k.to_owned());
                    }
                }
            }
        }
        let texto = |v: serde_json::Value| v.as_str().unwrap().to_owned();
        for c in [
            huellas::Clave::Gps,
            huellas::Clave::Dispositivo,
            huellas::Clave::Serie,
            huellas::Clave::Autor,
            huellas::Clave::UltimoAutor,
            huellas::Clave::Empresa,
            huellas::Clave::Responsable,
            huellas::Clave::Fecha,
            huellas::Clave::Programa,
            huellas::Clave::Titulo,
            huellas::Clave::Descripcion,
            huellas::Clave::Derechos,
            huellas::Clave::Plantilla,
            huellas::Clave::Propiedades,
        ] {
            claves.push(format!("hx_h_{}", texto(serde_json::to_value(c).unwrap())));
        }
        for e in apagon::ESENCIALES {
            claves.push(format!(
                "hx_para_{}",
                texto(serde_json::to_value(e.para).unwrap())
            ));
        }
        for n in ["pdf_comprimido", "heic_no_seguido"] {
            claves.push(format!("hx_nota_{n}"));
        }
        assert!(claves.len() > 60, "{}", claves.len());
        for (idioma, json) in [
            ("es", guardiana_core::i18n::ES_JSON),
            ("en", guardiana_core::i18n::EN_JSON),
            ("pt", guardiana_core::i18n::PT_JSON),
        ] {
            let v: serde_json::Value = serde_json::from_str(json).unwrap();
            let panel = v["panel"].as_object().unwrap();
            let faltan: Vec<&String> = claves.iter().filter(|k| !panel.contains_key(*k)).collect();
            assert!(faltan.is_empty(), "{idioma}: faltan {faltan:?}");
        }
    }
}
