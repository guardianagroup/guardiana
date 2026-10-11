//! Local HTTP panel (brief §8): static pages plus a JSON API on
//! `127.0.0.1:7443`, served by the same process as the resolver.
//!
//! Security, as the brief states it: a random token per installation kept
//! in a file only the user can read; the launcher opens
//! `http://127.0.0.1:7443/?t=<token>` and the page keeps the token in
//! its tab (`sessionStorage`, gone when the tab closes), never as a cookie. Every request must carry a `Host` the panel
//! recognises (against DNS rebinding). Without the token there is no
//! writing and no reading of the home panel. No third-party JavaScript, no
//! external fonts, no cookies.

mod api;
mod auth;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::extract::{Path as RutaUrl, RawQuery};
use axum::http::{header, HeaderValue, StatusCode};
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
    /// One change of the system DNS at a time: two «apply» clicks in the same second could
    /// take the second snapshot after the first apply and keep Guardiana's own address as
    /// «the original» (review of 8 Oct 2026).
    pub(crate) dns_cambio: tokio::sync::Mutex<()>,
    /// One activation at a time: two clicks within the gateway's 20 seconds activated twice
    /// (two of the five activations spent) and wrote two secrets (second pass, 8 Oct 2026).
    pub(crate) licencia_cambio: tokio::sync::Mutex<()>,
    pub(crate) token: String,
    pub(crate) allowed_hosts: Vec<String>,
    pub(crate) info: RuntimeInfo,
    pub(crate) db_path: PathBuf,
    /// The chain's genesis, to open a connection of one's own for a call that must not hold
    /// the panel's lock (activating a key waits on the network).
    pub(crate) genesis: Hash,
    /// The one reader of the neighbour table (who is which phone), shared by every handler.
    pub(crate) vecinos: guardiana_devices::Resolver,
    /// The last answer to "does this computer ask the guardian first?", and when. On Windows the
    /// question is a PowerShell, and every page asked it on every load while the DNS was not
    /// pointed yet (review of 10 Oct 2026, panel 15): it is asked again after thirty seconds.
    pub(crate) primario: Mutex<Option<(std::time::Instant, Option<bool>)>>,
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

/// The site's own typefaces, carried inside the binary. The panel never asks the internet for
/// anything (brief, section 6), so looking like guardianagroup.com cannot mean fetching a font
/// from a CDN: the six files travel with the program. Latin subset only, which is all Spanish,
/// English and Portuguese need.
async fn fuente(RutaUrl(archivo): RutaUrl<String>) -> Response {
    let cuerpo: &'static [u8] = match archivo.as_str() {
        "IBMPlexSans-400-latin.woff2" => {
            include_bytes!("../static/fonts/IBMPlexSans-400-latin.woff2")
        }
        "IBMPlexSans-500-latin.woff2" => {
            include_bytes!("../static/fonts/IBMPlexSans-500-latin.woff2")
        }
        "IBMPlexSans-600-latin.woff2" => {
            include_bytes!("../static/fonts/IBMPlexSans-600-latin.woff2")
        }
        "IBMPlexMono-400-latin.woff2" => {
            include_bytes!("../static/fonts/IBMPlexMono-400-latin.woff2")
        }
        "IBMPlexMono-500-latin.woff2" => {
            include_bytes!("../static/fonts/IBMPlexMono-500-latin.woff2")
        }
        "Unbounded-latin.woff2" => include_bytes!("../static/fonts/Unbounded-latin.woff2"),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    (
        [
            (header::CONTENT_TYPE, "font/woff2"),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        cuerpo,
    )
        .into_response()
}

pub(crate) fn static_response(body: &'static str, content_type: &'static str) -> Response {
    ([(header::CONTENT_TYPE, content_type)], body).into_response()
}

/// The panel's script and style sheet, as the pages ask for them.
const APP_JS: &str = include_str!("../static/app.js");
const STYLE_CSS: &str = include_str!("../static/style.css");

/// A short tag of the script and the style sheet, so the pages ask for them by a name that
/// changes when they do and the browser may keep them for a while. Until 1.0.12 every navigation
/// fetched the 113 KB script and the style sheet again, which a phone on the Wi-Fi notices
/// (review of 10 Oct 2026, panel 25). Only needs to be stable within one binary.
pub(crate) fn version_estaticos() -> &'static str {
    static TAG: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    TAG.get_or_init(|| {
        use std::hash::{Hash as _, Hasher as _};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        APP_JS.hash(&mut h);
        STYLE_CSS.hash(&mut h);
        format!("{:016x}", h.finish())
    })
}

/// A page, with its script and style sheet named by their tag (see [`version_estaticos`]). The
/// page itself is never stored, so it always names the current ones.
pub(crate) fn pagina(html: &'static str) -> Html<String> {
    let v = version_estaticos();
    Html(
        html.replace("/static/app.js\"", &format!("/static/app.js?v={v}\""))
            .replace("/static/style.css\"", &format!("/static/style.css?v={v}\"")),
    )
}

/// The script or the style sheet: kept for ten minutes when asked for by the current tag, never
/// stored otherwise.
fn estatico(body: &'static str, content_type: &'static str, query: Option<&str>) -> Response {
    let mut res = static_response(body, content_type);
    let v = version_estaticos();
    if query.is_some_and(|q| q.split('&').any(|kv| kv.strip_prefix("v=") == Some(v))) {
        res.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=600"),
        );
    }
    res
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
    // The fonts say `immutable` for themselves, and the script and the style sheet asked for by
    // their tag keep ten minutes; everything else is never stored (review of 8 Oct 2026: `insert`
    // overwrote the fonts' header and 700 KB came down on every page).
    h.entry(header::CACHE_CONTROL)
        .or_insert(HeaderValue::from_static("no-store"));
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
        dns_cambio: tokio::sync::Mutex::new(()),
        licencia_cambio: tokio::sync::Mutex::new(()),
        token: token.clone(),
        allowed_hosts,
        info: config.info,
        db_path: config.db_path,
        genesis: config.genesis,
        vecinos: guardiana_devices::Resolver::default(),
        primario: Mutex::new(None),
    });

    let app = Router::new()
        .route("/", get(api::root_page))
        .route(
            "/licencia",
            get(|| async { pagina(include_str!("../static/licencia.html")) }),
        )
        .route("/api/licencia", get(api::licencia))
        .route("/api/licencia/activar-clave", post(api::licencia_clave))
        .route(
            "/verify",
            get(|| async { pagina(include_str!("../static/verify.html")) }),
        )
        .route("/api/verify", get(api::verify))
        .route(
            "/reglas",
            get(|| async { pagina(include_str!("../static/reglas.html")) }),
        )
        .route("/api/reglas", get(api::reglas).post(api::nueva_regla))
        .route("/api/reglas/deshacer-hoy", post(api::deshacer_hoy))
        .route("/api/reglas/{id}/deshacer", post(api::deshacer_regla))
        .route("/api/ajustes/bloqueo", post(api::modo_bloqueo))
        .route(
            "/api/proteccion-maxima",
            get(api::maxima).post(api::cambia_maxima),
        )
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
            get(|| async { pagina(include_str!("../static/hogar.html")) }),
        )
        .route(
            "/mi-dispositivo",
            get(|| async { pagina(include_str!("../static/mi-dispositivo.html")) }),
        )
        .route(
            "/extracto",
            get(|| async { pagina(include_str!("../static/extracto.html")) }),
        )
        .route(
            "/dispositivos",
            get(|| async { pagina(include_str!("../static/dispositivos.html")) }),
        )
        .route(
            "/sabe-de-ti",
            get(|| async { pagina(include_str!("../static/sabe-de-ti.html")) }),
        )
        .route(
            "/estado",
            get(|| async { pagina(include_str!("../static/estado.html")) }),
        )
        .route(
            "/ia",
            get(|| async { pagina(include_str!("../static/ia.html")) }),
        )
        .route(
            "/static/style.css",
            get(|RawQuery(q): RawQuery| async move {
                estatico(STYLE_CSS, "text/css; charset=utf-8", q.as_deref())
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
        // Alias: la dirección con «.html» al final lleva a la misma página. Escribirla así es
        // lo natural cuando alguien copia el nombre del archivo, y el 404 que salía a cambio
        // parecía que el programa estaba roto (20 sep 2026).
        .route(
            "/licencia.html",
            get(|| async { pagina(include_str!("../static/licencia.html")) }),
        )
        .route(
            "/verify.html",
            get(|| async { pagina(include_str!("../static/verify.html")) }),
        )
        .route(
            "/reglas.html",
            get(|| async { pagina(include_str!("../static/reglas.html")) }),
        )
        .route(
            "/hogar.html",
            get(|| async { pagina(include_str!("../static/hogar.html")) }),
        )
        .route(
            "/mi-dispositivo.html",
            get(|| async { pagina(include_str!("../static/mi-dispositivo.html")) }),
        )
        .route(
            "/extracto.html",
            get(|| async { pagina(include_str!("../static/extracto.html")) }),
        )
        .route(
            "/dispositivos.html",
            get(|| async { pagina(include_str!("../static/dispositivos.html")) }),
        )
        .route(
            "/sabe-de-ti.html",
            get(|| async { pagina(include_str!("../static/sabe-de-ti.html")) }),
        )
        .route(
            "/estado.html",
            get(|| async { pagina(include_str!("../static/estado.html")) }),
        )
        .route(
            "/ia.html",
            get(|| async { pagina(include_str!("../static/ia.html")) }),
        )
        .route(
            "/informe.html",
            get(|| async { pagina(include_str!("../static/informe.html")) }),
        )
        .route("/static/fonts/{archivo}", get(fuente))
        .route(
            "/static/arranque.js",
            get(|| async {
                static_response(
                    include_str!("../static/arranque.js"),
                    "application/javascript; charset=utf-8",
                )
            }),
        )
        .route(
            "/static/app.js",
            get(|RawQuery(q): RawQuery| async move {
                estatico(
                    APP_JS,
                    "application/javascript; charset=utf-8",
                    q.as_deref(),
                )
            }),
        )
        .route("/api/textos", get(api::textos))
        .route("/api/estado", get(api::estado))
        .route("/api/cambios", get(api::cambios))
        .route("/api/ia", get(api::ia))
        .route("/api/ia/alcance", post(api::alcance))
        .route("/api/ia/alcance/anadir", post(api::alcance_anadir))
        .route("/api/dns/aplicar", post(api::dns_aplicar))
        .route("/api/dns/restaurar", post(api::dns_restaurar))
        .route("/api/radiografia", get(api::radiografia))
        .route("/api/rafagas", get(api::rafagas))
        .route("/api/recibo", get(api::recibo))
        .route("/api/extracto", get(api::extracto))
        .route("/api/extracto/comprobar", get(api::comprobar))
        .route("/api/extracto/exportar", get(api::exportar))
        .route("/api/dispositivos", get(api::dispositivos))
        .route("/api/dispositivos/{id}/nombre", post(api::renombrar))
        .route("/api/sabe-de-ti", get(api::sabe_de_ti))
        .route("/api/sabe-de-ti/borrar", post(api::borrar))
        .route(
            "/informe",
            get(|| async { pagina(include_str!("../static/informe.html")) }),
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod estaticos {
    use super::*;

    /// The pages name the script and the style sheet by their tag, and only a request with the
    /// current tag may be kept by the browser (review of 10 Oct 2026, panel 25).
    #[test]
    fn the_pages_ask_for_the_current_script_and_only_that_one_is_kept() {
        let v = version_estaticos();
        let Html(pagina) = pagina(include_str!("../static/index.html"));
        assert!(
            pagina.contains(&format!("/static/app.js?v={v}\"")),
            "{pagina}"
        );
        assert!(
            pagina.contains(&format!("/static/style.css?v={v}\"")),
            "{pagina}"
        );
        let guardado = |q: Option<&str>| {
            estatico(APP_JS, "application/javascript", q)
                .headers()
                .get(header::CACHE_CONTROL)
                .map(|h| h.to_str().unwrap_or_default().to_owned())
        };
        assert_eq!(
            guardado(Some(&format!("v={v}"))).as_deref(),
            Some("public, max-age=600")
        );
        assert_eq!(guardado(Some("v=otra")), None);
        assert_eq!(guardado(None), None);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod textos {
    /// Every text the panel's pages and script ask for must exist in the `panel` section of the
    /// three files. It is not enough for the three to agree with each other: `t()` falls back to
    /// the key itself, so a text that lives only in the `cli` section reaches the screen as
    /// `trampa_etiqueta`, which is exactly what happened with the trap tag until 21 Sep 2026.
    #[test]
    fn every_text_the_screen_asks_for_exists() {
        const PAGES: &[&str] = &[
            include_str!("../static/index.html"),
            include_str!("../static/extracto.html"),
            include_str!("../static/dispositivos.html"),
            include_str!("../static/mi-dispositivo.html"),
            include_str!("../static/hogar.html"),
            include_str!("../static/informe.html"),
            include_str!("../static/licencia.html"),
            include_str!("../static/reglas.html"),
            include_str!("../static/sabe-de-ti.html"),
            include_str!("../static/estado.html"),
            include_str!("../static/verify.html"),
            include_str!("../static/ia.html"),
            include_str!("../static/comprobador.html"),
        ];
        const SCRIPTS: &[&str] = &[include_str!("../static/app.js")];
        // The keys asked for with a literal: `data-t="x"` in the pages, `t('x')` in the scripts.
        // The ones built at run time (`t('cambio_' + x)`) cannot be read here and are covered by
        // the tests of whatever produces them.
        let mut asked: Vec<String> = Vec::new();
        for page in PAGES {
            for part in page.split("data-t=\"").skip(1) {
                if let Some(k) = part.split('"').next() {
                    asked.push(k.to_owned());
                }
            }
        }
        for script in SCRIPTS {
            // `t('clave')` and nothing else: the character before the `t` must not be part of a
            // longer name, or `createElement('div')` would look like a text of ours. A key that
            // ends in `_` is the start of one built at run time (`t('cambio_' + x)`) and is left
            // to whoever builds it.
            // `tn('clave', n)` too, the counting form.
            let bytes = script.as_bytes();
            let llamadas = script
                .match_indices("t('")
                .map(|(i, _)| (i, 3))
                .chain(script.match_indices("tn('").map(|(i, _)| (i, 4)));
            for (i, largo) in llamadas {
                let before = if i == 0 { b' ' } else { bytes[i - 1] };
                if before.is_ascii_alphanumeric() || before == b'_' || before == b'.' {
                    continue;
                }
                let rest = &script[i + largo..];
                if let Some(k) = rest.split('\'').next() {
                    if !k.is_empty()
                        && !k.ends_with('_')
                        && k.chars()
                            .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit())
                    {
                        asked.push(k.to_owned());
                    }
                }
            }
        }
        assert!(
            asked.len() > 100,
            "se leyeron pocas claves: {}",
            asked.len()
        );
        for (name, json) in [
            ("es", guardiana_core::i18n::ES_JSON),
            ("en", guardiana_core::i18n::EN_JSON),
            ("pt", guardiana_core::i18n::PT_JSON),
        ] {
            let v: serde_json::Value = serde_json::from_str(json).unwrap();
            let panel = v["panel"].as_object().unwrap();
            let missing: Vec<&String> = asked.iter().filter(|k| !panel.contains_key(*k)).collect();
            assert!(missing.is_empty(), "{name}: faltan en «panel» {missing:?}");
        }
    }

    /// The list of changes builds its key at run time (`t('quien_' + c.quien)`), which the test
    /// above cannot read. Every value the program can write as "who" must have its text: in
    /// 1.0.1 "licencia" had none and the panel printed `quien_licencia` when a trial ended.
    #[test]
    fn every_who_of_a_change_has_its_text() {
        for (name, json) in [
            ("es", guardiana_core::i18n::ES_JSON),
            ("en", guardiana_core::i18n::EN_JSON),
            ("pt", guardiana_core::i18n::PT_JSON),
        ] {
            let v: serde_json::Value = serde_json::from_str(json).unwrap();
            let panel = v["panel"].as_object().unwrap();
            for who in guardiana_core::ChangeWho::ALL {
                let key = format!("quien_{}", who.as_str());
                assert!(panel.contains_key(&key), "{name}: falta «{key}»");
            }
            for kind in ["hogar_on", "hogar_off", "dns_on", "dns_off"] {
                let key = format!("cambio_{kind}");
                assert!(panel.contains_key(&key), "{name}: falta «{key}»");
            }
        }
    }
}
