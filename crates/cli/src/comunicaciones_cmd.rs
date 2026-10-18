//! `guardiana comunicaciones` (also `guardiana secure`): GUARDIANA SECURE, chat and calls, in the
//! person's own browser on Linux and macOS (17 Oct 2026).
//!
//! A friend of the owner runs GUARDIANA on Linux and saw only the radiographies: Comunicaciones
//! lived inside GUARDIANA ZERO, which is Windows only. The page is the same one ZERO shows
//! (`zero/navegador/interfaz/secure.html`, embedded here byte for byte) and so is the engine
//! (`guardiana_zero::comunicaciones`); only who carries the page's messages changes. This command
//! runs as the person, never inside the DNS service: the service is root and its unit has
//! `ProtectHome=yes`, and a conversation's keys belong to the person, in their own folder.
//!
//! What protects it:
//! - It listens on 127.0.0.1 only, and answers only requests whose `Host` is this address and
//!   port (DNS rebinding) and whose `Origin`, when there is one, is this same page.
//! - Every request carries this run's key: a cookie (`HttpOnly`, `SameSite=Strict`) set when the
//!   browser comes in with a one-use, two-minute key in its address. The key that opens the
//!   browser is useless once used, so a glance at the process list is not a way in.
//! - The person's folder is theirs alone (0700), like `%LOCALAPPDATA%` on Windows.
//!
//! Not built on Windows: there Comunicaciones is a tab of GUARDIANA ZERO, on the same identity
//! folder, and two programs on one identity would step on each other.

use std::collections::VecDeque;
use std::error::Error;
use std::future::IntoFuture as _;
use std::io::{IsTerminal as _, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, RawQuery, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use guardiana_core::i18n;
use guardiana_zero::comunicaciones::{Comunicaciones, Salida};
use guardiana_zero::textos::{Idioma, Textos};
use serde_json::{json, Value};

use crate::args::Opts;

/// The port it tries first (the panel is 7443); a free one when it is taken.
const PUERTO: u16 = 7444;
/// The engine's clock, as in ZERO (`zero/navegador/src/app.rs`, `TIC`).
const TIC: Duration = Duration::from_millis(200);
/// During a call, as in ZERO (`TIC_LLAMADA`): a piece of voice goes on at once.
const TIC_LLAMADA: Duration = Duration::from_millis(20);
/// How long `/eventos` waits for something before answering with nothing.
const ESPERA_EVENTOS: Duration = Duration::from_secs(25);
/// The largest request: the page sends files in 64 KB pieces and a call in small ones, a few
/// MB at a time (`puente.js`). Never unbounded.
const MAX_CUERPO: usize = 8 * 1024 * 1024;
/// The most one answer of `/eventos` carries (a picture for the chat is at most 12 MB, in
/// base64); what is left goes in the next one.
const MAX_RESPUESTA: usize = 24 * 1024 * 1024;
/// What waits for a page that does not come to read it, at most.
const MAX_BUZON: usize = 64 * 1024 * 1024;
const MAX_MENSAJES: usize = 20_000;
/// A page counts as there while it asked within this time (it asks again at least every 25 s).
const PAGINA_VIVA: Duration = Duration::from_secs(40);
/// With no page for this long it closes: the person closed the tab, as closing ZERO's window.
const SIN_PAGINA: Duration = Duration::from_secs(90);
/// The first page has longer to arrive (a browser that starts cold).
const SIN_PRIMERA_PAGINA: Duration = Duration::from_secs(180);
/// How long the key in the browser's address is good for, once.
const ENTRADA_VALE: Duration = Duration::from_secs(120);
/// The cookie with this run's key.
const COOKIE: &str = "guardiana_com";
/// The file that tells a second `guardiana comunicaciones` where the first one is.
const FICHA: &str = "comunicaciones.json";

const HTML: &str = "text/html; charset=utf-8";
const JS: &str = "text/javascript; charset=utf-8";
const CSS: &str = "text/css; charset=utf-8";
const WOFF2: &str = "font/woff2";

/// The page itself, as ZERO shows it.
const PORTADA: &[u8] = include_bytes!("../../../zero/navegador/interfaz/secure.html");

/// Everything `secure.html` asks for (its scripts, its style sheets and the fonts those name).
const ARCHIVOS: &[(&str, &str, &[u8])] = &[
    (
        "/secure.js",
        JS,
        include_bytes!("../../../zero/navegador/interfaz/secure.js"),
    ),
    (
        "/puente.js",
        JS,
        include_bytes!("../../../zero/navegador/interfaz/puente.js"),
    ),
    (
        "/relevo.js",
        JS,
        include_bytes!("../../../zero/navegador/interfaz/relevo.js"),
    ),
    (
        "/voz.js",
        JS,
        include_bytes!("../../../zero/navegador/interfaz/voz.js"),
    ),
    (
        "/interruptor.js",
        JS,
        include_bytes!("../../../zero/navegador/interfaz/interruptor.js"),
    ),
    (
        "/opus.js",
        JS,
        include_bytes!("../../../zero/navegador/interfaz/opus.js"),
    ),
    (
        "/voz-eleccion.js",
        JS,
        include_bytes!("../../../zero/navegador/interfaz/voz-eleccion.js"),
    ),
    (
        "/escudo-directo.js",
        JS,
        include_bytes!("../../../zero/navegador/interfaz/escudo-directo.js"),
    ),
    (
        "/entrega.js",
        JS,
        include_bytes!("../../../zero/navegador/interfaz/entrega.js"),
    ),
    (
        "/entrega.css",
        CSS,
        include_bytes!("../../../zero/navegador/interfaz/entrega.css"),
    ),
    (
        "/diseno.css",
        CSS,
        include_bytes!("../../../zero/navegador/interfaz/diseno.css"),
    ),
    (
        "/comun.css",
        CSS,
        include_bytes!("../../../zero/navegador/interfaz/comun.css"),
    ),
    (
        "/fuentes/IBMPlexSans-400-latin.woff2",
        WOFF2,
        include_bytes!("../../../zero/navegador/interfaz/fuentes/IBMPlexSans-400-latin.woff2"),
    ),
    (
        "/fuentes/IBMPlexSans-500-latin.woff2",
        WOFF2,
        include_bytes!("../../../zero/navegador/interfaz/fuentes/IBMPlexSans-500-latin.woff2"),
    ),
    (
        "/fuentes/IBMPlexSans-600-latin.woff2",
        WOFF2,
        include_bytes!("../../../zero/navegador/interfaz/fuentes/IBMPlexSans-600-latin.woff2"),
    ),
    (
        "/fuentes/IBMPlexMono-400-latin.woff2",
        WOFF2,
        include_bytes!("../../../zero/navegador/interfaz/fuentes/IBMPlexMono-400-latin.woff2"),
    ),
    (
        "/fuentes/IBMPlexMono-500-latin.woff2",
        WOFF2,
        include_bytes!("../../../zero/navegador/interfaz/fuentes/IBMPlexMono-500-latin.woff2"),
    ),
    (
        "/fuentes/Unbounded-latin.woff2",
        WOFF2,
        include_bytes!("../../../zero/navegador/interfaz/fuentes/Unbounded-latin.woff2"),
    ),
];

/// The page may load only what it brings, draw what arrives (pictures and voice notes come as
/// `data:`, a video as `blob:`) and talk only to this server; nobody may frame it. Inline style
/// is allowed because `secure.html` carries its own `<style>`.
const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; media-src 'self' data: blob:; font-src 'self'; connect-src 'self'; object-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'";

/// Kinds of file that run something when opened on Linux or macOS. The engine's own list
/// (`Recibido::abrible`) thinks of Windows; one of these is shown in its folder instead.
const EJECUTABLES: &[&str] = &[
    "desktop",
    "appimage",
    "run",
    "bin",
    "sh",
    "bash",
    "zsh",
    "csh",
    "ksh",
    "fish",
    "py",
    "pyc",
    "pyw",
    "pl",
    "rb",
    "php",
    "lua",
    "jar",
    "deb",
    "rpm",
    "snap",
    "flatpak",
    "flatpakref",
    "flatpakrepo",
    "command",
    "tool",
    "terminal",
    "workflow",
    "scpt",
    "applescript",
    "app",
    "pkg",
    "dmg",
    "jnlp",
    "so",
    "ko",
    "elf",
    "out",
    "x86_64",
];

fn cerrojo<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A new random key, 64 hexadecimal characters.
fn llave_nueva() -> std::io::Result<String> {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

/// What the page says, for the engine's thread: one request's messages, in order, and how to
/// say they were taken (the request answers only then, so the page never runs ahead of it).
struct Entrada {
    pagina: String,
    mensajes: Vec<Value>,
    hecho: tokio::sync::oneshot::Sender<()>,
}

/// What waits for the page, numbered: the page asks from the number it has, so an answer lost
/// on the way is asked for again and nothing comes twice.
struct Buzon {
    /// The number the next message gets.
    siguiente: u64,
    cola: VecDeque<(u64, String)>,
    bytes: usize,
    /// The page that said «listo» last: it is the one Comunicaciones talks to.
    pagina: Option<String>,
    /// Where its messages start.
    inicio: u64,
    /// When that page last asked.
    vista: Instant,
    /// Whether a page has come at all.
    hubo_pagina: bool,
}

/// What a request for `/eventos` finds.
enum Recogida {
    /// Another page took over (or this one never said «listo»).
    Otra,
    /// Nothing yet after this number.
    Nada(u64),
    /// This answer.
    Lista(String),
}

/// What the server's requests and the engine's thread share.
struct Compartido {
    llave: String,
    /// `127.0.0.1:<port>` and `localhost:<port>`, the only `Host` it answers.
    hosts: [String; 2],
    entradas: Mutex<Vec<(String, Instant)>>,
    buzon: Mutex<Buzon>,
    /// The number of the next message, to wake whoever waits on `/eventos`.
    aviso: tokio::sync::watch::Sender<u64>,
    motor: mpsc::Sender<Entrada>,
}

impl Compartido {
    fn nuevo(llave: String, puerto: u16, motor: mpsc::Sender<Entrada>) -> Self {
        let (aviso, _) = tokio::sync::watch::channel(0);
        Self {
            llave,
            hosts: [format!("127.0.0.1:{puerto}"), format!("localhost:{puerto}")],
            entradas: Mutex::new(Vec::new()),
            buzon: Mutex::new(Buzon {
                siguiente: 0,
                cola: VecDeque::new(),
                bytes: 0,
                pagina: None,
                inicio: 0,
                vista: Instant::now(),
                hubo_pagina: false,
            }),
            aviso,
            motor,
        }
    }

    /// A one-use key for the browser's address.
    fn entrada_nueva(&self) -> std::io::Result<String> {
        let e = llave_nueva()?;
        let mut v = cerrojo(&self.entradas);
        v.retain(|(_, t)| t.elapsed() < ENTRADA_VALE);
        // A handful at most: each `guardiana comunicaciones` asks for one.
        if v.len() >= 16 {
            v.remove(0);
        }
        v.push((e.clone(), Instant::now()));
        Ok(e)
    }

    /// Whether `t` is a key for the browser's address, still good; it is spent here.
    fn entra(&self, t: &str) -> bool {
        let mut v = cerrojo(&self.entradas);
        v.retain(|(_, cuando)| cuando.elapsed() < ENTRADA_VALE);
        let i = v
            .iter()
            .position(|(e, _)| guardiana_panel::constant_time_eq(e.as_bytes(), t.as_bytes()));
        i.map(|i| v.remove(i)).is_some()
    }

    /// Whether the request carries this run's key: the cookie, or the header the second
    /// `guardiana comunicaciones` sends.
    fn autorizada(&self, h: &HeaderMap) -> bool {
        let de_cookie = h
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|v| v.split(';'))
            .find_map(|kv| {
                kv.trim()
                    .strip_prefix(COOKIE)
                    .and_then(|r| r.strip_prefix('='))
            });
        let de_cabecera = h.get("x-guardiana-token").and_then(|v| v.to_str().ok());
        [de_cookie, de_cabecera].into_iter().flatten().any(|dada| {
            !dada.is_empty()
                && guardiana_panel::constant_time_eq(dada.as_bytes(), self.llave.as_bytes())
        })
    }

    /// Whether a request of `pagina` is taken: from the page Comunicaciones talks to, from any
    /// page while there is none, and from one that says «listo» (it takes over).
    fn acepta(&self, pagina: &str, listo: bool) -> bool {
        let mut b = cerrojo(&self.buzon);
        if b.pagina.as_deref() == Some(pagina) {
            b.vista = Instant::now();
            return true;
        }
        b.pagina.is_none() || listo
    }

    /// `pagina` said «listo»: from now on it is the one, and what waited for another is gone.
    fn nueva_pagina(&self, pagina: &str) {
        let siguiente = {
            let mut b = cerrojo(&self.buzon);
            b.pagina = Some(pagina.to_owned());
            b.inicio = b.siguiente;
            b.cola.clear();
            b.bytes = 0;
            b.vista = Instant::now();
            b.hubo_pagina = true;
            b.siguiente
        };
        // The page that was there wakes up and hears that another one took over.
        self.aviso.send_replace(siguiente);
    }

    /// Messages for the page, in order.
    fn publica(&self, mensajes: Vec<Value>) {
        if mensajes.is_empty() {
            return;
        }
        let siguiente = {
            let mut b = cerrojo(&self.buzon);
            for v in mensajes {
                let s = v.to_string();
                b.bytes += s.len();
                let n = b.siguiente;
                b.cola.push_back((n, s));
                b.siguiente += 1;
            }
            while (b.bytes > MAX_BUZON || b.cola.len() > MAX_MENSAJES) && b.cola.len() > 1 {
                if let Some((_, s)) = b.cola.pop_front() {
                    b.bytes = b.bytes.saturating_sub(s.len());
                }
            }
            b.siguiente
        };
        self.aviso.send_replace(siguiente);
    }

    /// What `pagina` has to read from `desde` on. What it read before is let go.
    fn recoge(&self, pagina: &str, desde: u64) -> Recogida {
        let mut b = cerrojo(&self.buzon);
        if b.pagina.as_deref() != Some(pagina) {
            return Recogida::Otra;
        }
        b.vista = Instant::now();
        let efectivo = desde.max(b.inicio).min(b.siguiente);
        while b.cola.front().is_some_and(|(n, _)| *n < efectivo) {
            if let Some((_, s)) = b.cola.pop_front() {
                b.bytes = b.bytes.saturating_sub(s.len());
            }
        }
        if b.cola.is_empty() {
            return Recogida::Nada(b.siguiente);
        }
        let mut partes: Vec<&str> = Vec::new();
        let mut tam = 0;
        let mut sig = efectivo;
        for (n, s) in &b.cola {
            if !partes.is_empty() && tam + s.len() > MAX_RESPUESTA {
                break;
            }
            partes.push(s);
            tam += s.len();
            sig = n + 1;
        }
        Recogida::Lista(format!("{{\"sig\":{sig},\"m\":[{}]}}", partes.join(",")))
    }

    /// Whether a page is there to hear the engine (it asked a moment ago).
    fn pagina_viva(&self) -> bool {
        let b = cerrojo(&self.buzon);
        b.pagina.is_some() && b.vista.elapsed() < PAGINA_VIVA
    }

    /// Whether nobody is looking any more: time to close.
    fn abandonada(&self, arranque: Instant) -> bool {
        let b = cerrojo(&self.buzon);
        if b.hubo_pagina {
            b.vista.elapsed() > SIN_PAGINA
        } else {
            arranque.elapsed() > SIN_PRIMERA_PAGINA
        }
    }
}

/// The engine's side: everything SECURE does runs on this one thread, as in ZERO's window.
struct Motor {
    datos: PathBuf,
    descargas: PathBuf,
    textos: Textos,
    /// The person's day or night from ZERO's settings, if this folder has them; empty follows
    /// the system (the page's `data-sigue`).
    tema: String,
}

impl Motor {
    fn corre(&self, entradas: &mpsc::Receiver<Entrada>, c: &Compartido) {
        let mut com = Comunicaciones::abre(&self.datos, &self.descargas, env!("CARGO_PKG_VERSION"));
        // 1.0.42: others can call this computer by its GUARDIANA number.
        com.abre_buzon();
        let arranque = Instant::now();
        let mut proximo = Instant::now();
        loop {
            if Instant::now() >= proximo {
                c.publica(com.tic(c.pagina_viva()));
                if c.abandonada(arranque) {
                    return;
                }
                proximo = Instant::now() + if com.en_llamada() { TIC_LLAMADA } else { TIC };
            }
            match entradas.recv_timeout(proximo.saturating_duration_since(Instant::now())) {
                Ok(e) => {
                    for m in &e.mensajes {
                        self.atiende(&mut com, c, &e.pagina, m);
                    }
                    let _ = e.hecho.send(());
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
    }

    /// One message of the page, as ZERO's session takes it from its SECURE tab: «listo» gets the
    /// day or night, the texts and the state, in that order (`Sesion::listo`); «secure» is an
    /// action. Nothing else is for this page.
    fn atiende(&self, com: &mut Comunicaciones, c: &Compartido, pagina: &str, m: &Value) {
        match m.get("tipo").and_then(Value::as_str) {
            Some("listo") => {
                c.nueva_pagina(pagina);
                let mut o = vec![
                    json!({ "tipo": "tema", "tema": self.tema }),
                    json!({
                        "tipo": "textos",
                        "textos": self.textos.todos(),
                        "idioma": self.textos.idioma().codigo(),
                    }),
                ];
                if m.get("vista").and_then(Value::as_str) == Some("secure") {
                    o.push(com.estado_json());
                }
                c.publica(o);
            }
            Some("secure") => {
                let mut o = Vec::new();
                for s in com.accion(m) {
                    match s {
                        Salida::Pagina(v) => o.push(v),
                        Salida::AbreArchivo { ruta } => abre_archivo(Path::new(&ruta)),
                        Salida::MuestraArchivo { ruta } => muestra_archivo(Path::new(&ruta)),
                        // Windows' settings page for the microphone or the camera: here the
                        // browser is who asks, and the page says where.
                        Salida::AbreFuera { url } if url.starts_with("ms-settings:") => o.push(
                            json!({ "tipo": "secure_aviso", "clave": "sec_permiso_navegador", "texto": "" }),
                        ),
                        Salida::AbreFuera { .. } => {}
                    }
                }
                c.publica(o);
            }
            _ => {}
        }
    }
}

/// Run the system's opener (`xdg-open`, or `open` on the Mac) on a file or a folder, and let it
/// go: it ends by itself, and a thread waits for it so it leaves nothing behind.
fn abre_con_sistema(args: &[&std::ffi::OsStr]) {
    let programa = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    if let Ok(mut hijo) = std::process::Command::new(programa)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        std::thread::spawn(move || {
            let _ = hijo.wait();
        });
    }
}

/// Whether a file that arrived may be opened with a double click here: never one that runs.
fn abrible_aqui(ruta: &Path) -> bool {
    let ext = ruta
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if EJECUTABLES.contains(&ext.as_str()) {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if std::fs::metadata(ruta).is_ok_and(|m| m.permissions().mode() & 0o111 != 0) {
            return false;
        }
    }
    true
}

fn abre_archivo(ruta: &Path) {
    if abrible_aqui(ruta) {
        abre_con_sistema(&[ruta.as_os_str()]);
    } else {
        muestra_archivo(ruta);
    }
}

/// The file in its folder: selected on the Mac (`open -R`), the folder itself elsewhere.
fn muestra_archivo(ruta: &Path) {
    if cfg!(target_os = "macos") {
        abre_con_sistema(&[std::ffi::OsStr::new("-R"), ruta.as_os_str()]);
    } else if let Some(carpeta) = ruta.parent() {
        abre_con_sistema(&[carpeta.as_os_str()]);
    }
}

// --- the server ---------------------------------------------------------------------------------

fn sirve_archivo(tipo: &'static str, datos: &'static [u8]) -> Response {
    ([(header::CONTENT_TYPE, tipo)], datos).into_response()
}

/// The page for a request without the key: what to do, in the person's language.
fn sin_llave() -> Response {
    let t = i18n::current();
    let esc = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    };
    let html = format!(
        "<!doctype html><html lang=\"{}\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><title>GUARDIANA</title></head><body style=\"font-family:system-ui,sans-serif;max-width:560px;margin:15vh auto;padding:0 24px;line-height:1.5;color:#0B1020;background:#F4F6FA\"><h1 style=\"font-size:20px;letter-spacing:.12em\">GUARDIANA</h1><p>{}</p></body></html>",
        esc(i18n::code_of(t)),
        esc(t.cli("comunicaciones.sin_llave"))
    );
    (
        StatusCode::UNAUTHORIZED,
        [(header::CONTENT_TYPE, HTML)],
        html,
    )
        .into_response()
}

/// Every request: its `Host` must be this server, its `Origin` (when there is one) this page,
/// and every one but the door (`/`, which checks for itself) must carry the key.
async fn guardia(State(c): State<Arc<Compartido>>, req: Request, next: Next) -> Response {
    let rechazo = {
        let h = req.headers();
        let host = h
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        if !c.hosts.iter().any(|x| x.eq_ignore_ascii_case(host)) {
            Some((StatusCode::MISDIRECTED_REQUEST, "host not recognised").into_response())
        } else if h.get(header::ORIGIN).is_some_and(|o| {
            !guardiana_panel::origin_es_este_panel(o.to_str().unwrap_or("null"), host)
        }) || h.get("sec-fetch-site").and_then(|v| v.to_str().ok()) == Some("cross-site")
        {
            Some((StatusCode::FORBIDDEN, "cross-site request refused").into_response())
        } else if req.uri().path() != "/" && !c.autorizada(h) {
            Some(sin_llave())
        } else {
            None
        }
    };
    match rechazo {
        Some(r) => r,
        None => next.run(req).await,
    }
}

/// The same headers as the panel's (`add_security_headers`), with this page's policy.
async fn cabeceras(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CSP),
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
    h.insert(
        header::HeaderName::from_static("cross-origin-resource-policy"),
        HeaderValue::from_static("same-origin"),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

/// The door: with a one-use key in the address it sets the cookie and goes to `/` without the
/// key (so it stays out of the history); with the cookie, the page.
async fn portada(
    State(c): State<Arc<Compartido>>,
    RawQuery(q): RawQuery,
    h: HeaderMap,
) -> Response {
    if let Some(t) = q
        .as_deref()
        .and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("t=")))
    {
        if !c.entra(t) {
            return sin_llave();
        }
        let cookie = format!("{COOKIE}={}; Path=/; HttpOnly; SameSite=Strict", c.llave);
        return (
            StatusCode::SEE_OTHER,
            [
                (header::LOCATION, "/".to_owned()),
                (header::SET_COOKIE, cookie),
            ],
        )
            .into_response();
    }
    if !c.autorizada(&h) {
        return sin_llave();
    }
    sirve_archivo(HTML, PORTADA)
}

async fn archivo(uri: Uri) -> Response {
    ARCHIVOS
        .iter()
        .find(|(ruta, _, _)| *ruta == uri.path())
        .map_or_else(
            || StatusCode::NOT_FOUND.into_response(),
            |&(_, tipo, datos)| sirve_archivo(tipo, datos),
        )
}

/// The page's own name (`puente.js`): 1 to 64 hexadecimal characters.
fn pagina_de(h: &HeaderMap) -> Option<String> {
    h.get("x-guardiana-pagina")
        .and_then(|v| v.to_str().ok())
        .filter(|p| !p.is_empty() && p.len() <= 64 && p.bytes().all(|b| b.is_ascii_hexdigit()))
        .map(str::to_owned)
}

/// What the page says: one message or a list of them, handed to the engine in order. It
/// answers once the engine has taken them.
async fn mensaje(State(c): State<Arc<Compartido>>, h: HeaderMap, cuerpo: Bytes) -> Response {
    let Some(pagina) = pagina_de(&h) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(v) = serde_json::from_slice::<Value>(&cuerpo) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let mensajes = match v {
        Value::Array(a) => a,
        otro => vec![otro],
    };
    let listo = mensajes
        .iter()
        .any(|m| m.get("tipo").and_then(Value::as_str) == Some("listo"));
    if !c.acepta(&pagina, listo) {
        return StatusCode::CONFLICT.into_response();
    }
    let (hecho, tomado) = tokio::sync::oneshot::channel();
    if c.motor
        .send(Entrada {
            pagina,
            mensajes,
            hecho,
        })
        .is_err()
    {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let _ = tomado.await;
    StatusCode::NO_CONTENT.into_response()
}

/// What the program says, from the number the page has on: at once if there is something,
/// otherwise as soon as there is, or nothing after [`ESPERA_EVENTOS`].
async fn eventos(
    State(c): State<Arc<Compartido>>,
    RawQuery(q): RawQuery,
    h: HeaderMap,
) -> Response {
    let Some(pagina) = pagina_de(&h) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let desde = q
        .as_deref()
        .and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("desde=")))
        .and_then(|d| d.parse::<u64>().ok())
        .unwrap_or(0);
    let fin = tokio::time::Instant::now() + ESPERA_EVENTOS;
    let mut aviso = c.aviso.subscribe();
    loop {
        match c.recoge(&pagina, desde) {
            Recogida::Otra => return StatusCode::CONFLICT.into_response(),
            Recogida::Lista(cuerpo) => {
                return ([(header::CONTENT_TYPE, "application/json")], cuerpo).into_response()
            }
            Recogida::Nada(sig) => {
                let llego = matches!(
                    tokio::time::timeout_at(fin, aviso.wait_for(|n| *n > sig)).await,
                    Ok(Ok(_))
                );
                if !llego {
                    return Json(json!({ "sig": sig, "m": [] })).into_response();
                }
            }
        }
    }
}

/// A one-use key for the browser, for a second `guardiana comunicaciones` (which has the key of
/// this run from the file only this person can read).
async fn entrada(State(c): State<Arc<Compartido>>) -> Response {
    match c.entrada_nueva() {
        Ok(t) => Json(json!({ "t": t })).into_response(),
        Err(_) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

fn rutas(c: Arc<Compartido>) -> Router {
    Router::new()
        .route("/", get(portada))
        .route("/mensaje", post(mensaje))
        .route("/eventos", get(eventos))
        .route("/entrada", post(entrada))
        .fallback(archivo)
        .layer(DefaultBodyLimit::max(MAX_CUERPO))
        .layer(middleware::from_fn_with_state(c.clone(), guardia))
        // Outermost, so a refusal carries the same headers as the page.
        .layer(middleware::from_fn(cabeceras))
        .with_state(c)
}

/// The shared state and the engine's thread; `fin` fires when the engine stops (nobody looking).
fn arranca(
    puerto: u16,
    motor: Motor,
) -> std::io::Result<(Arc<Compartido>, tokio::sync::oneshot::Receiver<()>)> {
    let (a_motor, del_servidor) = mpsc::channel();
    let c = Arc::new(Compartido::nuevo(llave_nueva()?, puerto, a_motor));
    let (fin, fin_rx) = tokio::sync::oneshot::channel::<()>();
    let c2 = Arc::clone(&c);
    std::thread::Builder::new()
        .name("comunicaciones".into())
        .spawn(move || {
            motor.corre(&del_servidor, &c2);
            let _ = fin.send(());
        })?;
    Ok((c, fin_rx))
}

// --- the person's folders -------------------------------------------------------------------------

fn casa() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
}

/// Where Comunicaciones keeps this person's identity and book: the platform's place for a
/// program's own data, as `%LOCALAPPDATA%\GUARDIANA ZERO` is on Windows. `GUARDIANA_ZERO_DATOS`
/// moves it, as it moves ZERO's.
fn carpeta_base() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("GUARDIANA_ZERO_DATOS") {
        return Some(PathBuf::from(p));
    }
    if cfg!(target_os = "macos") {
        return casa().map(|h| h.join("Library/Application Support/guardiana/zero"));
    }
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| casa().map(|h| h.join(".local/share")))
        .map(|d| d.join("guardiana").join("zero"))
}

/// A folder only this person can open: the conversation's keys are inside.
fn carpeta_propia(d: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(d)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(d, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// The person's downloads: `xdg-user-dir DOWNLOAD` on Linux, then the usual names.
fn carpeta_descargas() -> PathBuf {
    let Some(h) = casa() else {
        return std::env::temp_dir();
    };
    if cfg!(target_os = "linux") {
        if let Ok(out) = std::process::Command::new("xdg-user-dir")
            .arg("DOWNLOAD")
            .stderr(std::process::Stdio::null())
            .output()
        {
            let d = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
            // Without its own folder it says the home folder itself: not that.
            if out.status.success() && d.is_absolute() && d != h {
                return d;
            }
        }
    }
    ["Descargas", "Downloads", "Transferências"]
        .iter()
        .map(|n| h.join(n))
        .find(|d| d.is_dir())
        .unwrap_or_else(|| h.join("Downloads"))
}

/// Day or night, if ZERO's settings are in this folder (a Mac or Linux copy of them).
fn tema_guardado(datos: &Path) -> String {
    std::fs::read(datos.join("preferencias.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v.get("tema").and_then(Value::as_str).map(str::to_owned))
        .filter(|t| t == "dia" || t == "noche")
        .unwrap_or_default()
}

/// The file that says where this run is: its port and its key, readable by this person only
/// (created new with 0600, as the panel's wait page).
fn escribe_ficha(ruta: &Path, puerto: u16, llave: &str) -> std::io::Result<()> {
    let texto = json!({ "puerto": puerto, "llave": llave }).to_string();
    let _ = std::fs::remove_file(ruta);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(ruta)?;
        f.write_all(texto.as_bytes())
    }
    #[cfg(not(unix))]
    {
        std::fs::write(ruta, texto)
    }
}

/// A plain HTTP/1.1 request to this server on this computer: for a second run and for the tests.
/// Returns the status, the head and the body.
fn pide(
    puerto: u16,
    metodo: &str,
    ruta: &str,
    cabeceras: &[(&str, &str)],
    cuerpo: &[u8],
) -> std::io::Result<(u16, String, Vec<u8>)> {
    let dir = std::net::SocketAddr::from(([127, 0, 0, 1], puerto));
    let mut s = std::net::TcpStream::connect_timeout(&dir, Duration::from_secs(2))?;
    s.set_read_timeout(Some(ESPERA_EVENTOS + Duration::from_secs(10)))?;
    let mut p = format!("{metodo} {ruta} HTTP/1.1\r\n");
    if !cabeceras
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("host"))
    {
        p.push_str(&format!("Host: 127.0.0.1:{puerto}\r\n"));
    }
    for (k, v) in cabeceras {
        p.push_str(&format!("{k}: {v}\r\n"));
    }
    p.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        cuerpo.len()
    ));
    // Head and body in one write: a refusal answered before the body arrived, on a connection
    // then closed, came back as «connection reset» on macOS instead of its status.
    let mut todo = p.into_bytes();
    todo.extend_from_slice(cuerpo);
    s.write_all(&todo)?;
    let mut r = Vec::new();
    // What arrived before a reset is still the answer.
    if let Err(e) = s.read_to_end(&mut r) {
        if r.is_empty() {
            return Err(e);
        }
    }
    let corte = r
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| std::io::Error::other("no head"))?;
    let cabeza = String::from_utf8_lossy(&r[..corte]).into_owned();
    let estado = cabeza
        .split_whitespace()
        .nth(1)
        .and_then(|e| e.parse().ok())
        .ok_or_else(|| std::io::Error::other("no status"))?;
    Ok((estado, cabeza, r[corte + 4..].to_vec()))
}

/// The run that is already open, if there is one: its port and a fresh key for the browser.
fn ya_abierto(ficha: &Path) -> Option<(u16, String)> {
    let v: Value = serde_json::from_slice(&std::fs::read(ficha).ok()?).ok()?;
    let puerto = u16::try_from(v.get("puerto")?.as_u64()?).ok()?;
    let llave = v.get("llave")?.as_str()?;
    let (estado, _, cuerpo) = pide(
        puerto,
        "POST",
        "/entrada",
        &[("X-Guardiana-Token", llave)],
        b"",
    )
    .ok()?;
    if estado != 200 {
        return None;
    }
    let j: Value = serde_json::from_slice(&cuerpo).ok()?;
    Some((puerto, j.get("t")?.as_str()?.to_owned()))
}

/// Opens the page in the person's browser, and says it in the terminal when there is one (from
/// the applications menu there is none, and the link must not end up in a log).
fn abre_navegador(t: &'static i18n::Texts, clave: &str, puerto: u16, entrada: &str) {
    let url = format!("http://127.0.0.1:{puerto}/?t={entrada}");
    if std::io::stdout().is_terminal() {
        println!("{}", t.cli(clave).replace("{url}", &url));
    }
    crate::engine::open_in_browser(&url);
}

/// Through `sudo` the identity would be root's and the browser root's, or none.
fn por_sudo() -> bool {
    std::env::var_os("SUDO_USER").is_some()
        || std::env::var("USER").is_ok_and(|u| u == "root")
        || std::env::var("LOGNAME").is_ok_and(|u| u == "root")
}

/// Listen, start the engine, open the browser, and serve until nobody is looking (or Ctrl+C).
async fn sirve(t: &'static i18n::Texts, motor: Motor, ficha: &Path) -> Result<(), Box<dyn Error>> {
    let escucha = match tokio::net::TcpListener::bind(("127.0.0.1", PUERTO)).await {
        Ok(l) => l,
        Err(_) => tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|e| {
                t.cli("comunicaciones.sin_puerto")
                    .replace("{motivo}", &e.to_string())
            })?,
    };
    let puerto = escucha.local_addr()?.port();
    let (c, fin) = arranca(puerto, motor)?;
    // Without the file a second run would start a second engine on the same folder: say it.
    if let Err(e) = escribe_ficha(ficha, puerto, &c.llave) {
        eprintln!(
            "{}",
            t.cli("comunicaciones.sin_carpeta_motivo")
                .replace("{ruta}", &ficha.display().to_string())
                .replace("{motivo}", &e.to_string())
        );
    }
    let entrada = c.entrada_nueva()?;
    abre_navegador(t, "comunicaciones.abriendo", puerto, &entrada);
    let servidor = axum::serve(escucha, rutas(c)).into_future();
    tokio::select! {
        r = servidor => r.map_err(Box::<dyn Error>::from),
        _ = fin => {
            if std::io::stdout().is_terminal() {
                println!("{}", t.cli("comunicaciones.cerrado"));
            }
            Ok(())
        }
        _ = tokio::signal::ctrl_c() => Ok(()),
    }
}

pub fn run(_opts: &Opts) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    if por_sudo() {
        return Err(t.cli("comunicaciones.sin_sudo").into());
    }
    let base = carpeta_base().ok_or_else(|| t.cli("comunicaciones.sin_carpeta").to_owned())?;
    let falla = |e: std::io::Error| {
        t.cli("comunicaciones.sin_carpeta_motivo")
            .replace("{ruta}", &base.display().to_string())
            .replace("{motivo}", &e.to_string())
    };
    carpeta_propia(&base).map_err(falla)?;
    let ficha = base.join(FICHA);
    if let Some((puerto, entrada)) = ya_abierto(&ficha) {
        abre_navegador(t, "comunicaciones.ya_abierto", puerto, &entrada);
        return Ok(());
    }
    let datos = base.join("datos");
    carpeta_propia(&datos).map_err(falla)?;
    let descargas = carpeta_descargas();
    let _ = std::fs::create_dir_all(&descargas);
    let motor = Motor {
        tema: tema_guardado(&datos),
        textos: Textos::de(Idioma::de_etiqueta(i18n::code_of(t))),
        datos,
        descargas,
    };
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let r = rt.block_on(sirve(t, motor, &ficha));
    let _ = std::fs::remove_file(&ficha);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    /// A server on a free port with the real engine in a folder of its own, as the command
    /// starts it.
    fn servidor() -> (tokio::runtime::Runtime, Arc<Compartido>, u16, PathBuf) {
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let d = std::env::temp_dir().join(format!(
            "guardiana-comunicaciones-cli-{}-{n}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::create_dir_all(d.join("descargas"));
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap_or_else(|e| unreachable!("runtime: {e}"));
        let escucha = rt
            .block_on(tokio::net::TcpListener::bind(("127.0.0.1", 0)))
            .unwrap_or_else(|e| unreachable!("bind: {e}"));
        let puerto = escucha.local_addr().map(|a| a.port()).unwrap_or_default();
        let motor = Motor {
            datos: d.join("datos"),
            descargas: d.join("descargas"),
            textos: Textos::de(Idioma::Es),
            tema: String::new(),
        };
        let (c, _fin) = arranca(puerto, motor).unwrap_or_else(|e| unreachable!("engine: {e}"));
        let app = rutas(Arc::clone(&c));
        rt.spawn(async move {
            let _ = axum::serve(escucha, app).await;
        });
        (rt, c, puerto, d)
    }

    fn cookie(c: &Compartido) -> String {
        format!("{COOKIE}={}", c.llave)
    }

    fn json_de(cuerpo: &[u8]) -> Value {
        serde_json::from_slice(cuerpo).unwrap_or_default()
    }

    /// A request as the page makes it: with the cookie, its own name and its own origin.
    fn lado(
        puerto: u16,
        k: &str,
        pagina: &str,
        metodo: &str,
        ruta: &str,
        cuerpo: &[u8],
    ) -> (u16, String, Vec<u8>) {
        let origen = format!("http://127.0.0.1:{puerto}");
        pide(
            puerto,
            metodo,
            ruta,
            &[
                ("Cookie", k),
                ("X-Guardiana-Pagina", pagina),
                ("Origin", origen.as_str()),
            ],
            cuerpo,
        )
        .unwrap_or_default()
    }

    #[test]
    fn every_request_needs_this_runs_key_and_the_address_key_works_once() {
        let (_rt, c, puerto, d) = servidor();
        // Without the key: neither the page nor its scripts nor the messages.
        for (m, r) in [
            ("GET", "/"),
            ("GET", "/secure.js"),
            ("GET", "/eventos"),
            ("POST", "/mensaje"),
        ] {
            let (e, _, _) = pide(puerto, m, r, &[], b"").unwrap_or_default();
            assert_eq!(e, 401, "{m} {r}");
        }
        let mala = format!("{COOKIE}={}", "0".repeat(64));
        let (e, _, _) = pide(
            puerto,
            "GET",
            "/secure.js",
            &[("Cookie", mala.as_str())],
            b"",
        )
        .unwrap_or_default();
        assert_eq!(e, 401, "a wrong key");
        // The key in the address sets the cookie and leaves the address clean, once.
        let entrada = c.entrada_nueva().unwrap_or_default();
        let con_entrada = format!("/?t={entrada}");
        let (e, cabeza, _) = pide(puerto, "GET", &con_entrada, &[], b"").unwrap_or_default();
        assert_eq!(e, 303);
        let cabeza = cabeza.to_ascii_lowercase();
        let esperada = format!("set-cookie: {}", cookie(&c));
        assert!(cabeza.contains(&esperada), "{cabeza}");
        assert!(cabeza.contains("httponly") && cabeza.contains("samesite=strict"));
        assert!(cabeza.contains("location: /\r\n") || cabeza.ends_with("location: /"));
        let (e, _, _) = pide(puerto, "GET", &con_entrada, &[], b"").unwrap_or_default();
        assert_eq!(e, 401, "spent");
        // The run's own key is never good in the address.
        let con_llave = format!("/?t={}", c.llave);
        let (e, _, _) = pide(puerto, "GET", &con_llave, &[], b"").unwrap_or_default();
        assert_eq!(e, 401);
        // With the cookie: the page, its scripts and its fonts.
        let k = cookie(&c);
        let (e, cabeza, cuerpo) =
            pide(puerto, "GET", "/", &[("Cookie", k.as_str())], b"").unwrap_or_default();
        assert_eq!(e, 200);
        assert!(String::from_utf8_lossy(&cuerpo).contains("secure.js"));
        assert!(cabeza
            .to_ascii_lowercase()
            .contains("frame-ancestors 'none'"));
        for &(r, _, datos) in ARCHIVOS {
            let (e, _, cuerpo) =
                pide(puerto, "GET", r, &[("Cookie", k.as_str())], b"").unwrap_or_default();
            assert_eq!(e, 200, "{r}");
            assert!(cuerpo.as_slice() == datos, "{r}");
        }
        // And the header the second run uses.
        let (e, _, cuerpo) = pide(
            puerto,
            "POST",
            "/entrada",
            &[("X-Guardiana-Token", c.llave.as_str())],
            b"",
        )
        .unwrap_or_default();
        assert_eq!(e, 200);
        assert_eq!(
            json_de(&cuerpo)["t"].as_str().map(str::len),
            Some(64),
            "a fresh key for the browser"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn another_host_or_another_origin_is_refused() {
        let (_rt, c, puerto, d) = servidor();
        let k = cookie(&c);
        let k = k.as_str();
        // DNS rebinding: a name that points here, with the right key even.
        let otro = format!("evil.example:{puerto}");
        let (e, _, _) = pide(
            puerto,
            "GET",
            "/secure.js",
            &[("Host", otro.as_str()), ("Cookie", k)],
            b"",
        )
        .unwrap_or_default();
        assert_eq!(e, 421);
        let (e, _, _) = pide(
            puerto,
            "GET",
            "/",
            &[("Host", "127.0.0.1:1"), ("Cookie", k)],
            b"",
        )
        .unwrap_or_default();
        assert_eq!(e, 421, "another port");
        // Another site's page posting here.
        let (e, _, _) = pide(
            puerto,
            "POST",
            "/mensaje",
            &[
                ("Origin", "http://evil.example"),
                ("Cookie", k),
                ("X-Guardiana-Pagina", "ab"),
            ],
            br#"{"tipo":"secure","accion":"cerrar"}"#,
        )
        .unwrap_or_default();
        assert_eq!(e, 403);
        let (e, _, _) = pide(
            puerto,
            "POST",
            "/mensaje",
            &[
                ("Origin", "null"),
                ("Cookie", k),
                ("X-Guardiana-Pagina", "ab"),
            ],
            b"[]",
        )
        .unwrap_or_default();
        assert_eq!(e, 403, "an opaque origin");
        let (e, _, _) = pide(
            puerto,
            "GET",
            "/secure.js",
            &[("Sec-Fetch-Site", "cross-site"), ("Cookie", k)],
            b"",
        )
        .unwrap_or_default();
        assert_eq!(e, 403);
        // localhost, with its own origin, is this page too.
        let host = format!("localhost:{puerto}");
        let origen = format!("http://localhost:{puerto}");
        let (e, _, _) = pide(
            puerto,
            "POST",
            "/mensaje",
            &[
                ("Host", host.as_str()),
                ("Origin", origen.as_str()),
                ("Cookie", k),
                ("X-Guardiana-Pagina", "ab"),
            ],
            b"[]",
        )
        .unwrap_or_default();
        assert_eq!(e, 204);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn listo_gets_the_texts_then_the_state_and_a_second_tab_takes_over() {
        let (_rt, c, puerto, d) = servidor();
        let k = cookie(&c);
        let k = k.as_str();
        let listo: &[u8] = br#"[{"tipo":"listo","vista":"secure"}]"#;
        let (e, _, _) = lado(puerto, k, "a1", "POST", "/mensaje", listo);
        assert_eq!(e, 204);
        let (e, _, cuerpo) = lado(puerto, k, "a1", "GET", "/eventos?desde=0", b"");
        assert_eq!(e, 200);
        let j = json_de(&cuerpo);
        let tipos: Vec<&str> = j["m"]
            .as_array()
            .map(|a| a.iter().filter_map(|m| m["tipo"].as_str()).collect())
            .unwrap_or_default();
        // As ZERO's session answers its SECURE tab: day or night, the texts, then the state.
        assert_eq!(
            tipos.iter().take(3).copied().collect::<Vec<_>>(),
            vec!["tema", "textos", "secure_estado"]
        );
        assert_eq!(j["m"][1]["idioma"], "es");
        assert_eq!(j["m"][1]["textos"]["pestana_secure"], "Comunicaciones");
        assert!(j["m"][1]["textos"]["sec_permiso_navegador"].is_string());
        let sig = j["sig"].as_u64().unwrap_or(0);
        assert!(sig >= 3);
        // An action comes back as the new state.
        let (e, _, _) = lado(
            puerto,
            k,
            "a1",
            "POST",
            "/mensaje",
            br#"{"tipo":"secure","accion":"escuchar","puerto":0}"#,
        );
        assert_eq!(e, 204);
        let ruta = format!("/eventos?desde={sig}");
        let (_, _, cuerpo) = lado(puerto, k, "a1", "GET", &ruta, b"");
        let j = json_de(&cuerpo);
        let estado = j["m"]
            .as_array()
            .and_then(|a| a.iter().rev().find(|m| m["tipo"] == "secure_estado"))
            .cloned()
            .unwrap_or_default();
        assert_eq!(estado["fase"], "escuchando");
        // Windows' privacy settings do not exist here: the page says where the browser asks.
        let sig = j["sig"].as_u64().unwrap_or(0);
        let _ = lado(
            puerto,
            k,
            "a1",
            "POST",
            "/mensaje",
            br#"{"tipo":"secure","accion":"ajustes_privacidad","que":"camara"}"#,
        );
        let ruta = format!("/eventos?desde={sig}");
        let (_, _, cuerpo) = lado(puerto, k, "a1", "GET", &ruta, b"");
        assert!(json_de(&cuerpo)["m"]
            .as_array()
            .is_some_and(|a| a.iter().any(|m| m["clave"] == "sec_permiso_navegador")));
        // A second tab says «listo»: it is the one now, and the first is told so.
        let (e, _, _) = lado(puerto, k, "b2", "POST", "/mensaje", listo);
        assert_eq!(e, 204);
        let (e, _, _) = lado(puerto, k, "a1", "GET", "/eventos?desde=0", b"");
        assert_eq!(e, 409);
        let (e, _, _) = lado(
            puerto,
            k,
            "a1",
            "POST",
            "/mensaje",
            br#"{"tipo":"secure","accion":"visto"}"#,
        );
        assert_eq!(e, 409);
        let (e, _, cuerpo) = lado(puerto, k, "b2", "GET", "/eventos?desde=0", b"");
        assert_eq!(e, 200);
        assert_eq!(json_de(&cuerpo)["m"][1]["tipo"], "textos");
        let _ = lado(
            puerto,
            k,
            "b2",
            "POST",
            "/mensaje",
            br#"{"tipo":"secure","accion":"cerrar"}"#,
        );
        // Without the page's own name nothing is taken.
        let (e, _, _) = pide(puerto, "GET", "/eventos", &[("Cookie", k)], b"").unwrap_or_default();
        assert_eq!(e, 400);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn nothing_that_runs_is_opened_with_a_double_click() {
        assert!(!abrible_aqui(Path::new("/tmp/x/instalar.desktop")));
        assert!(!abrible_aqui(Path::new("/tmp/x/Juego.AppImage")));
        assert!(!abrible_aqui(Path::new("/tmp/x/a.sh")));
        assert!(abrible_aqui(Path::new("/tmp/x/no-existe-foto.jpg")));
    }

    #[test]
    fn the_folder_follows_the_platform() {
        if std::env::var_os("GUARDIANA_ZERO_DATOS").is_some() || casa().is_none() {
            return;
        }
        let b = carpeta_base().unwrap_or_default();
        assert!(
            b.ends_with(Path::new("guardiana").join("zero")),
            "{}",
            b.display()
        );
    }
}
