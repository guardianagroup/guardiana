//! Local HTTP panel (brief §8): static pages plus a JSON API on
//! `127.0.0.1:7443`, served by the same process as the resolver.
//!
//! Security, as the brief states it: a random token per installation kept
//! in a file only the user can read; the launcher opens
//! `http://127.0.0.1:7443/?t=<token>` and the page keeps the token in
//! memory, never as a cookie. Every request must carry a `Host` the panel
//! recognises (against DNS rebinding). Without the token there is no
//! writing and no reading of the home panel. No third-party JavaScript, no
//! external fonts, no cookies.

mod api;
mod auth;
pub mod boveda;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::http::{header, HeaderValue};
use axum::middleware;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use guardiana_core::{Hash, Ledger};
use tokio::net::TcpListener;
use tokio::sync::Notify;

pub use auth::{load_or_create_token, TOKEN_FILE};

/// Default panel port (brief §0).
pub const DEFAULT_PORT: u16 = 7443;
/// The checker name the panel also answers to (brief §4).
pub const CHECKER_HOST: &str = "comprobar.guardiana.hogar";

/// Facts about the running resolver that the panel shows on `/estado`.
#[derive(Debug, Clone, Default)]
pub struct RuntimeInfo {
    /// Program version.
    pub version: String,
    /// DNS listen addresses, as text.
    pub listen_dns: Vec<String>,
    /// Upstream resolvers, as text.
    pub upstream: Vec<String>,
    /// Whether the binary carries the development key.
    pub dev_key: bool,
}

/// Panel configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// Addresses to bind: loopback always; the LAN address too in Home Mode.
    pub listen: Vec<SocketAddr>,
    /// Addresses that are nice to have (the LAN port 80 for the checker
    /// page); a bind failure there is reported, not fatal.
    pub optional_listen: Vec<SocketAddr>,
    /// Ledger database (the panel opens its own connection).
    pub db_path: PathBuf,
    /// Genesis hash the database was created with.
    pub genesis: Hash,
    /// Where the token file lives.
    pub token_path: PathBuf,
    /// Extra `Host` values to accept (the LAN IP in Home Mode).
    pub extra_hosts: Vec<String>,
    /// Shown on `/estado`.
    pub info: RuntimeInfo,
}

/// Errors starting the panel.
#[derive(Debug)]
pub enum Error {
    /// Binding the listener failed.
    Bind(SocketAddr, std::io::Error),
    /// The token file could not be created or read.
    Token(std::io::Error),
    /// The ledger could not be opened.
    Ledger(guardiana_core::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bind(a, e) => write!(f, "cannot bind panel on {a}: {e}"),
            Self::Token(e) => write!(f, "panel token file: {e}"),
            Self::Ledger(e) => write!(f, "panel ledger: {e}"),
        }
    }
}

impl std::error::Error for Error {}

/// Shared state of every handler.
pub(crate) struct AppState {
    pub(crate) ledger: Mutex<Ledger>,
    pub(crate) token: String,
    pub(crate) allowed_hosts: Vec<String>,
    pub(crate) info: RuntimeInfo,
    pub(crate) db_path: PathBuf,
}

/// A running panel.
pub struct Running {
    /// Addresses actually bound; the first one is loopback.
    pub addrs: Vec<SocketAddr>,
    /// Optional addresses that could not be bound, with the reason.
    pub failed_optional: Vec<(SocketAddr, String)>,
    /// The session token, for the launcher URL.
    pub token: String,
    stop: Arc<Notify>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl Running {
    /// `http://<loopback addr>/?t=<token>`: what the launcher opens.
    #[must_use]
    pub fn url(&self) -> String {
        self.addrs
            .first()
            .map(|a| panel_url(*a, &self.token))
            .unwrap_or_default()
    }

    /// Ask every listener to stop and wait for them.
    pub async fn shutdown(self) {
        self.stop.notify_waiters();
        for t in self.tasks {
            let _ = t.await;
        }
    }
}

/// `http://<addr>/?t=<token>`.
#[must_use]
pub fn panel_url(addr: SocketAddr, token: &str) -> String {
    format!("http://{addr}/?t={token}")
}

pub(crate) fn static_response(body: &'static str, content_type: &'static str) -> Response {
    ([(header::CONTENT_TYPE, content_type)], body).into_response()
}

pub(crate) async fn add_security_headers(
    req: axum::extract::Request,
    next: middleware::Next,
) -> Response {
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'",
        ),
    );
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

/// Bind and serve the panel until [`Running::shutdown`].
pub async fn start(config: Config) -> Result<Running, Error> {
    let token = load_or_create_token(&config.token_path).map_err(Error::Token)?;
    let ledger = Ledger::open(&config.db_path, config.genesis).map_err(Error::Ledger)?;
    let mut allowed_hosts = vec![
        "127.0.0.1".to_owned(),
        "localhost".to_owned(),
        CHECKER_HOST.to_owned(),
    ];
    allowed_hosts.extend(config.extra_hosts.iter().cloned());
    let state = Arc::new(AppState {
        ledger: Mutex::new(ledger),
        token: token.clone(),
        allowed_hosts,
        info: config.info,
        db_path: config.db_path,
    });

    let app = Router::new()
        .route("/", get(api::root_page))
        .route(
            "/licencia",
            get(|| async { Html(include_str!("../static/licencia.html")) }),
        )
        .route("/api/licencia", get(api::licencia))
        .route("/api/licencia/activar-clave", post(api::licencia_clave))
        .route("/api/licencia/activar-archivo", post(api::licencia_archivo))
        .route("/api/licencia/probar", post(api::licencia_probar))
        .route(
            "/verify",
            get(|| async { Html(include_str!("../static/verify.html")) }),
        )
        .route("/api/verify", get(api::verify))
        .route(
            "/reglas",
            get(|| async { Html(include_str!("../static/reglas.html")) }),
        )
        .route("/api/reglas", get(api::reglas).post(api::nueva_regla))
        .route("/api/reglas/deshacer-hoy", post(api::deshacer_hoy))
        .route("/api/reglas/{id}/deshacer", post(api::deshacer_regla))
        .route("/api/ajustes/bloqueo", post(api::modo_bloqueo))
        .route(
            "/api/mi-dispositivo/reglas",
            get(api::mi_reglas).post(api::mi_nueva_regla),
        )
        .route(
            "/api/mi-dispositivo/reglas/{id}/deshacer",
            post(api::mi_deshacer_regla),
        )
        .route(
            "/hogar",
            get(|| async { Html(include_str!("../static/hogar.html")) }),
        )
        .route(
            "/mi-dispositivo",
            get(|| async { Html(include_str!("../static/mi-dispositivo.html")) }),
        )
        .route(
            "/extracto",
            get(|| async { Html(include_str!("../static/extracto.html")) }),
        )
        .route(
            "/dispositivos",
            get(|| async { Html(include_str!("../static/dispositivos.html")) }),
        )
        .route(
            "/sabe-de-ti",
            get(|| async { Html(include_str!("../static/sabe-de-ti.html")) }),
        )
        .route(
            "/estado",
            get(|| async { Html(include_str!("../static/estado.html")) }),
        )
        .route(
            "/ia",
            get(|| async { Html(include_str!("../static/ia.html")) }),
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
        // The site's own icon, so browsers stop asking for /favicon.ico (a 404 in the console).
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
        .route(
            "/static/app.js",
            get(|| async {
                static_response(
                    include_str!("../static/app.js"),
                    "application/javascript; charset=utf-8",
                )
            }),
        )
        .route("/api/textos", get(api::textos))
        .route("/api/me", get(api::me))
        .route("/api/estado", get(api::estado))
        .route("/api/cambios", get(api::cambios))
        .route("/api/ia", get(api::ia))
        .route("/api/ia/alcance", post(api::alcance))
        .route("/api/dns/aplicar", post(api::dns_aplicar))
        .route("/api/dns/restaurar", post(api::dns_restaurar))
        .route("/api/radiografia", get(api::radiografia))
        .route("/api/extracto", get(api::extracto))
        .route("/api/extracto/comprobar", get(api::comprobar))
        .route("/api/extracto/exportar", get(api::exportar))
        .route("/api/dispositivos", get(api::dispositivos))
        .route("/api/dispositivos/{id}/nombre", post(api::renombrar))
        .route("/api/sabe-de-ti", get(api::sabe_de_ti))
        .route("/api/sabe-de-ti/borrar", post(api::borrar))
        .route(
            "/informe",
            get(|| async { Html(include_str!("../static/informe.html")) }),
        )
        .route("/api/informe", get(api::informe))
        .route("/api/hogar", get(api::hogar))
        .route("/api/hogar/activar", post(api::hogar_activar))
        .route("/api/hogar/desactivar", post(api::hogar_desactivar))
        .route("/api/mi-dispositivo", get(api::mi_dispositivo))
        .route("/api/mi-dispositivo/nombre", post(api::mi_nombre))
        .route("/api/mi-dispositivo/compartir", post(api::mi_compartir))
        .layer(middleware::from_fn(add_security_headers))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::check_host,
        ))
        .with_state(state);

    let stop = Arc::new(Notify::new());
    let mut addrs = Vec::new();
    let mut tasks = Vec::new();
    let mut failed_optional = Vec::new();
    let required = config.listen.iter().map(|a| (*a, true));
    let optional = config.optional_listen.iter().map(|a| (*a, false));
    for (listen, must) in required.chain(optional) {
        let listener = match TcpListener::bind(listen).await {
            Ok(l) => l,
            Err(e) if must => return Err(Error::Bind(listen, e)),
            Err(e) => {
                failed_optional.push((listen, e.to_string()));
                continue;
            }
        };
        addrs.push(listener.local_addr().map_err(|e| Error::Bind(listen, e))?);
        let stop_signal = stop.clone();
        let app = app.clone();
        tasks.push(tokio::spawn(async move {
            let _ = axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(async move { stop_signal.notified().await })
            .await;
        }));
    }
    Ok(Running {
        addrs,
        failed_optional,
        token,
        stop,
        tasks,
    })
}
