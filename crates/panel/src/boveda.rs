//! The vault's own page (DECISIONES #131): a small local server the person starts themselves
//! (`guardiana boveda panel`), separate from the panel that runs inside the service. The vault
//! belongs to a person and the service to the machine, so the keys live in this process — wiped
//! when the vault is locked — and the server dies with the page's "Salir" or after half an hour
//! idle. Same rules as the main panel: loopback only, a random token per run carried in a header,
//! `Host` checked, no third-party code, no cookies.
#![allow(clippy::result_large_err)] // handlers return axum responses as errors, like api.rs

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, FromRequestParts, Path as ObjPath, Query, State};
use axum::http::request::Parts;
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use guardiana_core::i18n;
use guardiana_vault::{self as vault, KdfParams, Vault};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::sync::Notify;

use crate::auth::{constant_time_eq, strip_port, Lang};
use crate::{add_security_headers, static_response, Error};

/// Who asked, as written in the vault's log.
const QUIEN: &str = "panel";
/// Idle time after which the open vault is locked (keys wiped).
const IDLE_LOCK: Duration = Duration::from_secs(10 * 60);
/// Idle time after which the server stops altogether.
const IDLE_EXIT: Duration = Duration::from_secs(30 * 60);
/// Shortest master password accepted (same as the terminal).
const MIN_PASSWORD: usize = 10;
/// Largest file accepted in one upload (2 GiB).
const MAX_UPLOAD: usize = 2 * 1024 * 1024 * 1024;

struct Estado {
    dir: PathBuf,
    token: String,
    kdf: KdfParams,
    vault: Mutex<Option<Vault>>,
    last: Mutex<Instant>,
    stop: Notify,
}

impl Estado {
    fn touch(&self) {
        if let Ok(mut l) = self.last.lock() {
            *l = Instant::now();
        }
    }

    fn idle(&self) -> Duration {
        self.last.lock().map(|l| l.elapsed()).unwrap_or_default()
    }

    fn lock_vault(&self) {
        if let Ok(mut v) = self.vault.lock() {
            *v = None; // the master key is zeroized on drop
        }
    }
}

/// A bound, not yet running, vault page.
pub struct Servidor {
    /// Loopback address actually bound.
    pub addr: SocketAddr,
    /// The token of this run.
    pub token: String,
    listener: TcpListener,
    estado: Arc<Estado>,
}

impl Servidor {
    /// `http://127.0.0.1:<port>/?t=<token>`: what the browser opens.
    #[must_use]
    pub fn url(&self) -> String {
        format!("http://{}/?t={}", self.addr, self.token)
    }

    /// Serve until the page says "Salir" or the server has been idle for half an hour.
    pub async fn run(self) {
        let estado = self.estado.clone();
        let app = router(estado.clone());
        let watchdog = {
            let estado = estado.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(15)).await;
                    let idle = estado.idle();
                    if idle >= IDLE_LOCK {
                        estado.lock_vault();
                    }
                    if idle >= IDLE_EXIT {
                        estado.stop.notify_waiters();
                        return;
                    }
                }
            })
        };
        let stop = estado.clone();
        let _ = axum::serve(
            self.listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async move { stop.stop.notified().await })
        .await;
        watchdog.abort();
        estado.lock_vault();
    }
}

/// Bind the vault page on a free loopback port with a fresh token. `dir` is the vault folder
/// (it may not exist yet: the page offers to create it).
pub async fn bind(dir: PathBuf) -> Result<Servidor, Error> {
    bind_with(dir, KdfParams::default()).await
}

/// Same, with explicit Argon2id parameters (tests use small ones).
pub async fn bind_with(dir: PathBuf, kdf: KdfParams) -> Result<Servidor, Error> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| Error::Token(std::io::Error::other(e.to_string())))?;
    let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let listen: SocketAddr = ([127, 0, 0, 1], 0).into();
    let listener = TcpListener::bind(listen)
        .await
        .map_err(|e| Error::Bind(listen, e))?;
    let addr = listener.local_addr().map_err(|e| Error::Bind(listen, e))?;
    let estado = Arc::new(Estado {
        dir,
        token: token.clone(),
        kdf,
        vault: Mutex::new(None),
        last: Mutex::new(Instant::now()),
        stop: Notify::new(),
    });
    Ok(Servidor {
        addr,
        token,
        listener,
        estado,
    })
}

fn router(estado: Arc<Estado>) -> Router {
    Router::new()
        .route(
            "/",
            get(|| async { Html(include_str!("../static/boveda.html")) }),
        )
        .route(
            "/static/style.css",
            get(|| async {
                static_response(
                    include_str!("../static/style.css"),
                    "text/css; charset=utf-8",
                )
            }),
        )
        .route(
            "/static/boveda.js",
            get(|| async {
                static_response(
                    include_str!("../static/boveda.js"),
                    "application/javascript; charset=utf-8",
                )
            }),
        )
        .route(
            "/favicon.ico",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "image/x-icon")],
                    include_bytes!("../static/favicon.ico").as_slice(),
                )
                    .into_response()
            }),
        )
        .route("/api/textos", get(crate::api::textos))
        .route("/api/estado", get(estado_api))
        .route("/api/crear", post(crear))
        .route("/api/abrir", post(abrir))
        .route("/api/cerrar", post(cerrar))
        .route("/api/contrasena", post(contrasena))
        .route("/api/recuperar", post(recuperar))
        .route("/api/registro", get(registro))
        .route("/api/salir", post(salir))
        .route(
            "/api/objetos",
            get(objetos)
                .post(subir)
                .layer(DefaultBodyLimit::max(MAX_UPLOAD)),
        )
        .route("/api/objetos/{id}", get(bajar))
        .layer(axum::middleware::from_fn(add_security_headers))
        .with_state(estado)
}

/// Extractor: the request carries this run's token and a loopback `Host`.
struct Llave;

impl FromRequestParts<Arc<Estado>> for Llave {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<Estado>,
    ) -> Result<Self, Self::Rejection> {
        let host = parts
            .headers
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            .map(strip_port)
            .unwrap_or_default();
        if host != "127.0.0.1" && host != "localhost" {
            return Err((StatusCode::MISDIRECTED_REQUEST, "host not recognised").into_response());
        }
        let from_header = parts
            .headers
            .get("x-guardiana-token")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let from_query = parts.uri.query().and_then(|q| {
            q.split('&')
                .find_map(|kv| kv.strip_prefix("t=").map(str::to_owned))
        });
        let given = from_header.or(from_query).unwrap_or_default();
        if !given.is_empty() && constant_time_eq(given.as_bytes(), state.token.as_bytes()) {
            state.touch();
            Ok(Self)
        } else {
            Err((StatusCode::UNAUTHORIZED, "session token required").into_response())
        }
    }
}

/// Vault errors in the person's words (the same sentences the terminal uses).
fn plain(t: &i18n::Texts, e: &vault::Error) -> Response {
    let (code, key, arg) = match e {
        vault::Error::WrongKey => (StatusCode::UNAUTHORIZED, "boveda_err_clave", None),
        vault::Error::Tampered(q) => (
            StatusCode::CONFLICT,
            "boveda_err_alterado",
            Some(("{que}", q.clone())),
        ),
        vault::Error::BadWords(q) => (
            StatusCode::BAD_REQUEST,
            "boveda_err_palabras",
            Some(("{que}", q.clone())),
        ),
        vault::Error::BadShare(q) => (
            StatusCode::BAD_REQUEST,
            "boveda_err_trozo",
            Some(("{que}", q.clone())),
        ),
        vault::Error::Exists(p) => (
            StatusCode::CONFLICT,
            "boveda_err_existe",
            Some(("{ruta}", p.display().to_string())),
        ),
        vault::Error::NotFound(p) => (
            StatusCode::NOT_FOUND,
            "boveda_err_no_hay",
            Some(("{ruta}", p.display().to_string())),
        ),
        vault::Error::NoObject(q) => (
            StatusCode::NOT_FOUND,
            "boveda_err_objeto",
            Some(("{que}", q.clone())),
        ),
        vault::Error::ChainBroken(n) => (
            StatusCode::CONFLICT,
            "boveda_err_cadena",
            Some(("{n}", n.to_string())),
        ),
        other => return (StatusCode::INTERNAL_SERVER_ERROR, other.to_string()).into_response(),
    };
    let mut text = t.cli(key).to_owned();
    if let Some((k, v)) = arg {
        text = text.replace(k, &v);
    }
    (code, text).into_response()
}

fn closed(t: &i18n::Texts) -> Response {
    (StatusCode::UNAUTHORIZED, t.panel("bov_cerrada").to_owned()).into_response()
}

fn poisoned() -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, "vault lock poisoned").into_response()
}

#[derive(Serialize)]
struct EstadoJson {
    existe: bool,
    abierta: bool,
    ruta: String,
    id: Option<String>,
}

async fn estado_api(State(s): State<Arc<Estado>>, _: Llave) -> Result<Json<EstadoJson>, Response> {
    let existe = s.dir.join("boveda.json").exists();
    let abierta = s.vault.lock().map_err(|_| poisoned())?.is_some();
    let id = if existe {
        Vault::peek(&s.dir).ok().map(|h| h.id)
    } else {
        None
    };
    Ok(Json(EstadoJson {
        existe,
        abierta,
        ruta: s.dir.display().to_string(),
        id,
    }))
}

#[derive(Deserialize)]
struct Clave {
    contrasena: String,
}

#[derive(Serialize)]
struct Palabras {
    palabras: Vec<String>,
}

fn check_password(t: &i18n::Texts, p: &str) -> Result<(), Response> {
    if p.len() < MIN_PASSWORD {
        return Err((
            StatusCode::BAD_REQUEST,
            t.cli("boveda_clave_corta").to_owned(),
        )
            .into_response());
    }
    Ok(())
}

async fn crear(
    State(s): State<Arc<Estado>>,
    _: Llave,
    Lang(t): Lang,
    Json(c): Json<Clave>,
) -> Result<Json<Palabras>, Response> {
    check_password(t, &c.contrasena)?;
    let created =
        Vault::create(&s.dir, c.contrasena.as_bytes(), s.kdf, QUIEN).map_err(|e| plain(t, &e))?;
    let palabras = created.words.iter().map(|w| (*w).to_owned()).collect();
    *s.vault.lock().map_err(|_| poisoned())? = Some(created.vault);
    Ok(Json(Palabras { palabras }))
}

async fn abrir(
    State(s): State<Arc<Estado>>,
    _: Llave,
    Lang(t): Lang,
    Json(c): Json<Clave>,
) -> Result<Json<EstadoJson>, Response> {
    let v = Vault::open(&s.dir, c.contrasena.as_bytes(), QUIEN).map_err(|e| plain(t, &e))?;
    let id = v.header.id.clone();
    *s.vault.lock().map_err(|_| poisoned())? = Some(v);
    Ok(Json(EstadoJson {
        existe: true,
        abierta: true,
        ruta: s.dir.display().to_string(),
        id: Some(id),
    }))
}

async fn cerrar(State(s): State<Arc<Estado>>, _: Llave) -> StatusCode {
    s.lock_vault();
    StatusCode::NO_CONTENT
}

async fn salir(State(s): State<Arc<Estado>>, _: Llave) -> StatusCode {
    s.lock_vault();
    s.stop.notify_waiters();
    StatusCode::NO_CONTENT
}

#[derive(Deserialize)]
struct Nueva {
    nueva: String,
}

async fn contrasena(
    State(s): State<Arc<Estado>>,
    _: Llave,
    Lang(t): Lang,
    Json(n): Json<Nueva>,
) -> Result<StatusCode, Response> {
    check_password(t, &n.nueva)?;
    let mut guard = s.vault.lock().map_err(|_| poisoned())?;
    let v = guard.as_mut().ok_or_else(|| closed(t))?;
    v.set_password(n.nueva.as_bytes(), QUIEN)
        .map_err(|e| plain(t, &e))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct Recuperar {
    palabras: String,
    nueva: String,
}

async fn recuperar(
    State(s): State<Arc<Estado>>,
    _: Llave,
    Lang(t): Lang,
    Json(r): Json<Recuperar>,
) -> Result<StatusCode, Response> {
    check_password(t, &r.nueva)?;
    let words: Vec<String> = r
        .palabras
        .split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphabetic())
                .to_ascii_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect();
    let refs: Vec<&str> = words.iter().map(String::as_str).collect();
    let mut v = Vault::open_with_words(&s.dir, &refs, QUIEN).map_err(|e| plain(t, &e))?;
    v.set_password(r.nueva.as_bytes(), QUIEN)
        .map_err(|e| plain(t, &e))?;
    *s.vault.lock().map_err(|_| poisoned())? = Some(v);
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
struct Objeto {
    id: String,
    nombre: String,
    guardado: String,
    tamano: u64,
    sha256: String,
}

async fn objetos(
    State(s): State<Arc<Estado>>,
    _: Llave,
    Lang(t): Lang,
) -> Result<Json<Vec<Objeto>>, Response> {
    let guard = s.vault.lock().map_err(|_| poisoned())?;
    let v = guard.as_ref().ok_or_else(|| closed(t))?;
    let items = v.list().map_err(|e| plain(t, &e))?;
    Ok(Json(
        items
            .into_iter()
            .map(|i| Objeto {
                id: i.id,
                nombre: i.meta.nombre,
                guardado: i.meta.guardado,
                tamano: i.meta.tamano,
                sha256: i.meta.sha256,
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
struct Nombre {
    nombre: String,
}

async fn subir(
    State(s): State<Arc<Estado>>,
    _: Llave,
    Lang(t): Lang,
    Query(q): Query<Nombre>,
    body: Bytes,
) -> Result<Json<Objeto>, Response> {
    let nombre = q.nombre.trim();
    if nombre.is_empty() || nombre.contains(['/', '\\']) {
        return Err((StatusCode::BAD_REQUEST, "nombre").into_response());
    }
    let guard = s.vault.lock().map_err(|_| poisoned())?;
    let v = guard.as_ref().ok_or_else(|| closed(t))?;
    let size = body.len() as u64;
    let item = v
        .add(nombre, &mut body.as_ref(), size, QUIEN)
        .map_err(|e| plain(t, &e))?;
    Ok(Json(Objeto {
        id: item.id,
        nombre: item.meta.nombre,
        guardado: item.meta.guardado,
        tamano: item.meta.tamano,
        sha256: item.meta.sha256,
    }))
}

async fn bajar(
    State(s): State<Arc<Estado>>,
    _: Llave,
    Lang(t): Lang,
    ObjPath(id): ObjPath<String>,
) -> Result<Response, Response> {
    let guard = s.vault.lock().map_err(|_| poisoned())?;
    let v = guard.as_ref().ok_or_else(|| closed(t))?;
    let mut out = Vec::new();
    let meta = v.extract(&id, &mut out, QUIEN).map_err(|e| plain(t, &e))?;
    let ascii: String = meta
        .nombre
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let utf8: String = meta
        .nombre
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_' {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{ascii}\"; filename*=UTF-8''{utf8}"),
            ),
        ],
        out,
    )
        .into_response())
}

#[derive(Serialize)]
struct Registro {
    lineas: Vec<vault::log::Entry>,
    cadena_ok: bool,
    rota_en: Option<u64>,
    total: u64,
}

async fn registro(
    State(s): State<Arc<Estado>>,
    _: Llave,
    Lang(t): Lang,
) -> Result<Json<Registro>, Response> {
    let (lineas, check) = Vault::log(&s.dir).map_err(|e| plain(t, &e))?;
    let (cadena_ok, rota_en, total) = match check {
        Ok(n) => (true, None, n),
        Err(n) => (false, Some(n), lineas.len() as u64),
    };
    Ok(Json(Registro {
        lineas,
        cadena_ok,
        rota_en,
        total,
    }))
}
