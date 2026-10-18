//! The window and its web views: the bar at the top, the side panel, and one view per tab.
//!
//! Two rules keep this layer simple and safe:
//! - No `RefCell` borrow is ever held while calling into the engine. The engine may raise an
//!   event from inside a call; its handler must find everything free.
//! - What the session orders from inside an engine event is queued and done right after, from
//!   the window's own message loop, never re-entering the engine from its own callback. Only the
//!   answers the engine needs on the spot (cancel this navigation, send this response instead)
//!   are given inside the event.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::ffi::c_void;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use guardiana_zero::lanzador::{self, Pedido};
use guardiana_zero::licencia::{Estado as EstadoLicencia, Fallo, Lugar as LugarLicencia};
use guardiana_zero::sesion::{self, Arranque, Orden, Origen, Perfil, Sesion};
use guardiana_zero::textos::{Idioma, Textos};
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::{
    take_pwstr, AcceleratorKeyPressedEventHandler,
    AddScriptToExecuteOnDocumentCreatedCompletedHandler, ClearBrowsingDataCompletedHandler,
    ContainsFullScreenElementChangedEventHandler, ContentLoadingEventHandler,
    ContextMenuRequestedEventHandler, CoreWebView2EnvironmentOptions,
    CreateCoreWebView2ControllerCompletedHandler, CreateCoreWebView2EnvironmentCompletedHandler,
    DocumentTitleChangedEventHandler, ExecuteScriptCompletedHandler, FaviconChangedEventHandler,
    FrameCreatedEventHandler, FrameWebMessageReceivedEventHandler, GetFaviconCompletedHandler,
    HistoryChangedEventHandler, NavigationCompletedEventHandler, NavigationStartingEventHandler,
    NewWindowRequestedEventHandler, PermissionRequestedEventHandler, PrintToPdfCompletedHandler,
    ProcessFailedEventHandler, SourceChangedEventHandler, WebMessageReceivedEventHandler,
    WebResourceRequestedEventHandler,
};
use windows::core::{w, Interface, BOOL, PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    GetLastError, COLORREF, ERROR_ALREADY_EXISTS, ERROR_SUCCESS, E_POINTER, HINSTANCE, HWND,
    LPARAM, LRESULT, POINT, RECT, S_FALSE, WAIT_ABANDONED, WAIT_OBJECT_0, WAIT_TIMEOUT, WPARAM,
};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
};
use windows::Win32::Graphics::Gdi::{
    CreateSolidBrush, GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::Com::{
    CoInitializeEx, CoTaskMemFree, COINIT_APARTMENTTHREADED, STREAM_SEEK_SET,
};
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER,
    HKEY_LOCAL_MACHINE, KEY_READ, RRF_RT_REG_QWORD, RRF_RT_REG_SZ,
};
use windows::Win32::System::Threading::{CreateMutexW, WaitForSingleObject};
use windows::Win32::UI::HiDpi::{
    GetDpiForWindow, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState;
use windows::Win32::UI::Shell::{
    FOLDERID_Downloads, SHCreateMemStream, SHGetKnownFolderPath, ShellExecuteW, KF_FLAG_DEFAULT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconFromResourceEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    FindWindowW, GetClientRect, GetMessageW, GetWindowLongPtrW, GetWindowRect,
    GetWindowThreadProcessId, IsIconic, KillTimer, LoadCursorW, MessageBoxW, PostMessageW,
    PostQuitMessage, RegisterClassExW, SendMessageW, SetForegroundWindow, SetTimer,
    SetWindowDisplayAffinity, SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow,
    SystemParametersInfoW, TranslateMessage, GWL_STYLE, HICON, HWND_TOP, ICON_BIG, ICON_SMALL,
    IDC_ARROW, LR_DEFAULTCOLOR, MB_ICONWARNING, MB_OK, MINMAXINFO, MSG, SIZE_MINIMIZED,
    SPI_GETWORKAREA, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_NOZORDER, SW_RESTORE,
    SW_SHOWMAXIMIZED, SW_SHOWNORMAL, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WDA_EXCLUDEFROMCAPTURE,
    WDA_MONITOR, WDA_NONE, WINDOW_EX_STYLE, WM_APP, WM_CLOSE, WM_COPYDATA, WM_DESTROY,
    WM_DPICHANGED, WM_GETMINMAXINFO, WM_MOVE, WM_MOVING, WM_SETFOCUS, WM_SETICON, WM_SIZE,
    WM_TIMER, WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
};

/// Before the session says otherwise (1.0.46, [`Orden::Marco`]): the browser's whole bar (top
/// row 56, tabs 40, address 44) and no side view; see `interfaz/barra.html`.
const MARCO_INICIAL: (i32, i32) = (sesion::ALTO_ZERO, 0);
/// The window class, also used to find a running copy.
const CLASE: PCWSTR = w!("GuardianaZeroVentana");
/// «The session queued orders».
const WM_ORDENES: u32 = WM_APP + 1;
/// The session's clock (batches, figures, saves).
const TIC: usize = 1;
/// The conversations' fast clock, only while a call is going on.
const TIC_LLAMADA: usize = 2;
/// Whether the fast clock is running.
static RAPIDO: AtomicBool = AtomicBool::new(false);
/// Shown once, when a page has loaded: not empty, then white, then the page.
static VENTANA_LISTA: AtomicBool = AtomicBool::new(false);
static TICS_SIN_PAGINA: AtomicU32 = AtomicU32::new(0);
/// Ticks while a SECURE conversation is open: every tenth (two seconds) the watch looks.
static TICS_VIGIA: AtomicU32 = AtomicU32::new(0);
/// Whether the window asked to be kept out of screen captures (while a conversation is open).
static OCULTA_A_CAPTURAS: AtomicBool = AtomicBool::new(false);
/// Whether Windows accepted.
static PROTEGIDA: AtomicBool = AtomicBool::new(false);

/// A string the way Win32 wants it: UTF-16, ending in zero, alive while it is used.
struct Ancho(Vec<u16>);

impl Ancho {
    fn de(s: &str) -> Self {
        Self(s.encode_utf16().chain(std::iter::once(0)).collect())
    }
    fn p(&self) -> PCWSTR {
        PCWSTR(self.0.as_ptr())
    }
}

/// Read a string the engine hands out (and free it).
fn texto_de(f: impl FnOnce(*mut PWSTR) -> windows::core::Result<()>) -> String {
    let mut p = PWSTR::null();
    if f(&mut p).is_err() {
        return String::new();
    }
    take_pwstr(p)
}

#[derive(Clone)]
struct Vista {
    controlador: ICoreWebView2Controller,
    web: ICoreWebView2,
}

struct PestanaW {
    vista: Option<Vista>,
    /// The engine profile it lives in («general», «aislada-3», «mandato-5»).
    perfil: String,
    privada: bool,
    /// Where to go once the view exists.
    pendiente: Option<String>,
}

struct Colores {
    oscuro: bool,
    marco: COLORREF,
    hoja: COREWEBVIEW2_COLOR,
    fondo_marco: COREWEBVIEW2_COLOR,
}

struct Vistas {
    hwnd: HWND,
    entorno: Option<ICoreWebView2Environment>,
    barra: Option<Vista>,
    panel: Option<Vista>,
    pestanas: BTreeMap<u32, PestanaW>,
    activa: u32,
    /// The bar's height and the side view's width, in CSS pixels, as the session last said.
    marco: (i32, i32),
    /// Tabs opened by a page (a new window): handed to the engine when their view exists.
    ventanas: HashMap<
        u32,
        (
            ICoreWebView2NewWindowRequestedEventArgs,
            ICoreWebView2Deferral,
        ),
    >,
    carpeta_ui: PathBuf,
    estricto: bool,
    borrar_pendiente: bool,
    /// Full screen (a video): the tab that asked, and the window's style and place before.
    completa: Option<(u32, isize, RECT)>,
    /// Pages reloaded after their process failed: when, per tab (a page that keeps crashing
    /// is not reloaded forever).
    recargas: HashMap<u32, Vec<i64>>,
}

thread_local! {
    static SESION: RefCell<Option<Sesion>> = const { RefCell::new(None) };
    static VISTAS: RefCell<Option<Vistas>> = const { RefCell::new(None) };
    static COLA: RefCell<VecDeque<Orden>> = const { RefCell::new(VecDeque::new()) };
    static REGISTRO: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    static DEPURA: bool = std::env::var_os("GUARDIANA_ZERO_DEPURA").is_some();
}

/// Where the subscription lives, for the worker threads that talk to the payment gateway.
static LUGAR: std::sync::OnceLock<LugarLicencia> = std::sync::OnceLock::new();
/// What those threads found, picked up by the window on its next tick.
static LICENCIA: std::sync::Mutex<VecDeque<DeLicencia>> = std::sync::Mutex::new(VecDeque::new());
/// One periodic check at a time.
static COMPROBANDO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether a thread is waiting for GUARDIANA's service (`--panel`, `guardiana://activar`).
static ESPERANDO_PANEL: AtomicBool = AtomicBool::new(false);
/// The licence key the last request asked to fill in, taken by that thread when the panel
/// answers. Kept in memory only, never written to the log.
static CLAVE_PEDIDA: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
/// What that thread found (the panel's address, or the wait page saying what failed), picked up
/// by the window on its next tick.
static DEL_PANEL: std::sync::Mutex<VecDeque<String>> = std::sync::Mutex::new(VecDeque::new());

/// A result from the payment gateway, done away from the window so it never freezes.
enum DeLicencia {
    Activada(Result<EstadoLicencia, Fallo>),
    Comprobada(EstadoLicencia),
    /// The newest version in the public ledger, read when the person asked.
    Version(Result<String, String>),
}

fn de_licencia(r: DeLicencia) {
    LICENCIA
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push_back(r);
}

/// Hand what the gateway threads found to the session.
fn recoge_licencia() {
    let hechos: Vec<DeLicencia> = LICENCIA
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .drain(..)
        .collect();
    for r in hechos {
        let o = match r {
            DeLicencia::Activada(r) => con_sesion(|s| s.licencia_activada(r)),
            DeLicencia::Comprobada(e) => con_sesion(|s| s.pon_licencia(e)),
            DeLicencia::Version(r) => con_sesion(|s| s.version_encontrada(r)),
        };
        encola(o.unwrap_or_default());
    }
}

/// Write ZERO's own wait page for the panel into the browser's folder, in the person's language.
fn escribe_espera(carpeta: &Path, textos: &Textos) {
    let ruta = carpeta.join(lanzador::ARCHIVO_ESPERA);
    let html = lanzador::pagina_espera(textos);
    if std::fs::read_to_string(&ruta).ok().as_deref() != Some(html.as_str()) {
        if let Err(e) = std::fs::write(&ruta, html) {
            registra("página de espera", &e);
        }
    }
}

/// `--panel` and `guardiana://activar` (1.0.44): the panel at once when the service already
/// answers; otherwise GUARDIANA's tab on ZERO's own wait page, and a thread that waits for the
/// service (up to [`lanzador::ESPERA_MAXIMA`]) and hands the panel's address to the window. The
/// person never sees a console, nor a white or navy page in between.
fn abre_panel(s: &mut Sesion, clave: Option<String>) -> Vec<Orden> {
    if let Some(url) = lanzador::enlace_si_listo(clave.as_deref()) {
        return s.abre_guardiana(&url);
    }
    *CLAVE_PEDIDA
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = clave;
    let o = s.abre_guardiana(lanzador::ESPERA);
    if !ESPERANDO_PANEL.swap(true, Ordering::SeqCst) {
        let lanzado = std::thread::Builder::new().name("panel".into()).spawn(|| {
            let r = lanzador::espera_llave(&lanzador::ruta_llave(), lanzador::ESPERA_MAXIMA);
            let clave = CLAVE_PEDIDA
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            let url = match r {
                Ok(llave) => lanzador::enlace(&llave, clave.as_deref()),
                Err(f) => f.pagina().to_owned(),
            };
            DEL_PANEL
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push_back(url);
            ESPERANDO_PANEL.store(false, Ordering::SeqCst);
        });
        if let Err(e) = lanzado {
            registra("esperar el panel", &e);
            ESPERANDO_PANEL.store(false, Ordering::SeqCst);
        }
    }
    o
}

/// Open in GUARDIANA's tab what the waiting thread found.
fn recoge_panel() {
    let hechos: Vec<String> = DEL_PANEL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .drain(..)
        .collect();
    for url in hechos {
        encola(con_sesion(|s| s.abre_guardiana(&url)).unwrap_or_default());
    }
}

/// Hand this start's request to the copy that is already running (its own window does it):
/// `dwData` 1 is an address for GUARDIANA's tab, 2 the panel, with a licence key or empty.
fn pasa_pedido(otra: HWND, pedido: &Pedido) {
    let (tipo, texto) = match pedido {
        Pedido::AbreGuardiana(url) => (1, url.as_str()),
        Pedido::Panel => (2, ""),
        Pedido::Activar(clave) => (2, clave.as_str()),
    };
    let ancho = Ancho::de(texto);
    let copia = COPYDATASTRUCT {
        dwData: tipo,
        cbData: u32::try_from(ancho.0.len() * 2).unwrap_or_default(),
        lpData: ancho.0.as_ptr().cast_mut().cast::<c_void>(),
    };
    // SAFETY: `copia` and the text it points to live until SendMessageW returns, which is when
    // the other window has copied them (WM_COPYDATA is synchronous).
    unsafe {
        let _ = SendMessageW(
            otra,
            WM_COPYDATA,
            Some(WPARAM(0)),
            Some(LPARAM((&copia as *const COPYDATASTRUCT) as isize)),
        );
    }
}

fn con_sesion<R>(f: impl FnOnce(&mut Sesion) -> R) -> Option<R> {
    SESION.with(|s| s.try_borrow_mut().ok()?.as_mut().map(f))
}

fn con_vistas<R>(f: impl FnOnce(&mut Vistas) -> R) -> Option<R> {
    VISTAS.with(|v| v.try_borrow_mut().ok()?.as_mut().map(f))
}

fn hwnd() -> HWND {
    con_vistas(|v| v.hwnd).unwrap_or(HWND(std::ptr::null_mut()))
}

/// One appearance. Called when the first page has loaded, or after a few seconds if it has not.
fn muestra_ventana() {
    if VENTANA_LISTA.swap(true, Ordering::SeqCst) {
        return;
    }
    let h = hwnd();
    // Maximised from the first moment (the owner, 18 Oct 2026: «nada más abrir… debe hacerse a
    // pantalla completa»). The 85 % size above stays as the size it returns to when restored.
    unsafe {
        let _ = ShowWindow(h, SW_SHOWMAXIMIZED);
        let _ = SetForegroundWindow(h);
    }
}

/// A line in the browser's own log (`datos/registro.txt`): what failed, for whoever has to look.
/// It stays on this computer.
fn registra(que: &str, e: &dyn std::fmt::Display) {
    REGISTRO.with(|r| {
        let Some(ruta) = r.borrow().clone() else {
            return;
        };
        if std::fs::metadata(&ruta).map(|m| m.len()).unwrap_or(0) > 512 * 1024 {
            let _ = std::fs::remove_file(&ruta);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&ruta)
        {
            let _ = writeln!(f, "{} {que}: {e}", sesion::ahora_sistema());
        }
    });
}

fn vale(que: &str, r: windows::core::Result<()>) {
    if let Err(e) = r {
        registra(que, &e);
    }
}

// --- orders ----------------------------------------------------------------------------------

/// Queue what the session ordered; the window does it on its next turn.
fn encola(o: Vec<Orden>) {
    if o.is_empty() {
        return;
    }
    let avisar = COLA.with(|c| {
        let mut c = c.borrow_mut();
        let vacia = c.is_empty();
        c.extend(o);
        vacia
    });
    if avisar {
        let h = hwnd();
        if !h.0.is_null() {
            let _ = unsafe { PostMessageW(Some(h), WM_ORDENES, WPARAM(0), LPARAM(0)) };
        }
    }
}

fn drena() {
    // Orders need the engine; before it exists they wait in the queue.
    if con_vistas(|v| v.entorno.is_some()) != Some(true) {
        return;
    }
    while let Some(o) = COLA.with(|c| c.borrow_mut().pop_front()) {
        ejecuta(o);
    }
    // 1.0.46: the layout for what is in front now (the bar, the glass strip, the side view),
    // when it changed.
    for o in con_sesion(Sesion::marco_nuevo).unwrap_or_default() {
        ejecuta(o);
    }
}

fn vista_de(a: Origen) -> Option<Vista> {
    con_vistas(|v| match a {
        Origen::Barra => v.barra.clone(),
        Origen::Panel => v.panel.clone(),
        Origen::Pestana(id) => v.pestanas.get(&id).and_then(|p| p.vista.clone()),
    })
    .flatten()
}

fn ejecuta(o: Orden) {
    match o {
        Orden::CreaPestana {
            id,
            perfil,
            url,
            de,
        } => crea_pestana(id, perfil, url, de),
        Orden::CierraPestana { id } => {
            if con_vistas(|v| v.completa.is_some_and(|c| c.0 == id)) == Some(true) {
                pantalla_completa(id, false);
            }
            let (quitada, ventana) =
                con_vistas(|v| (v.pestanas.remove(&id), v.ventanas.remove(&id)))
                    .unwrap_or((None, None));
            if let Some((args, aplazado)) = ventana {
                // A window a page asked for and that will not open: the engine must not open
                // its own instead.
                unsafe {
                    let _ = args.SetHandled(true);
                    let _ = aplazado.Complete();
                }
            }
            if let Some(Vista { controlador, .. }) = quitada.and_then(|p| p.vista) {
                vale("cerrar pestaña", unsafe { controlador.Close() });
            }
            coloca();
        }
        Orden::Activa { id } => {
            // Another tab never shows in the full screen a video asked for.
            if let Some(c) = con_vistas(|v| v.completa.map(|c| c.0)).flatten() {
                if c != id {
                    pantalla_completa(c, false);
                }
            }
            con_vistas(|v| v.activa = id);
            coloca();
        }
        Orden::Navega { id, url } => {
            match vista_de(Origen::Pestana(id)) {
                Some(v) => {
                    let u = Ancho::de(&url);
                    vale("navegar", unsafe { v.web.Navigate(u.p()) });
                }
                None => {
                    con_vistas(|v| {
                        if let Some(p) = v.pestanas.get_mut(&id) {
                            p.pendiente = Some(url);
                        }
                    });
                }
            };
        }
        Orden::Atras { id } => {
            if let Some(v) = vista_de(Origen::Pestana(id)) {
                let _ = unsafe { v.web.GoBack() };
            }
        }
        Orden::Adelante { id } => {
            if let Some(v) = vista_de(Origen::Pestana(id)) {
                let _ = unsafe { v.web.GoForward() };
            }
        }
        Orden::Recarga { id } => {
            if let Some(v) = vista_de(Origen::Pestana(id)) {
                let _ = unsafe { v.web.Reload() };
            }
        }
        Orden::Detiene { id } => {
            if let Some(v) = vista_de(Origen::Pestana(id)) {
                let _ = unsafe { v.web.Stop() };
            }
        }
        // The side view's place comes with the layout (`Orden::Marco`), given after every turn.
        Orden::Panel { .. } => {}
        Orden::Marco { barra, lateral } => {
            con_vistas(|v| v.marco = (barra, lateral));
            coloca();
        }
        Orden::PideGuardiana => {
            encola(con_sesion(|s| abre_panel(s, None)).unwrap_or_default());
        }
        Orden::Envia { a, json } => envia(a, &json),
        Orden::RespondeFormulario { id, json } => {
            // The answer to the page's own form guard; only its closure listens to it.
            if let Some(v) = vista_de(Origen::Pestana(id)) {
                let j = Ancho::de(&json);
                vale("respuesta al formulario", unsafe {
                    v.web.PostWebMessageAsJson(j.p())
                });
            }
        }
        Orden::GuardaPdf { id, ruta } => guarda_pdf(id, ruta),
        Orden::Ejecuta {
            id,
            script,
            etiqueta,
        } => {
            if let Some(v) = vista_de(Origen::Pestana(id)) {
                let s = Ancho::de(&script);
                let h = ExecuteScriptCompletedHandler::create(Box::new(
                    move |r: windows::core::Result<()>, resultado: String| {
                        if let (Ok(()), Some(e)) = (r, etiqueta) {
                            encola(
                                con_sesion(|s| s.script_hecho(e, &resultado)).unwrap_or_default(),
                            );
                        } else if let Some(e) = etiqueta {
                            encola(con_sesion(|s| s.script_hecho(e, "null")).unwrap_or_default());
                        }
                        Ok(())
                    },
                ));
                vale("script", unsafe { v.web.ExecuteScript(s.p(), &h) });
            }
        }
        Orden::Foco { a } => {
            if let Some(v) = vista_de(a) {
                let _ = unsafe {
                    v.controlador
                        .MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC)
                };
            }
        }
        Orden::AbreFuera { url } => {
            // Only mail (the letter opens in the person's own mail program) and the two pages of
            // Windows' privacy settings for the microphone and the camera; never a web address.
            if url.starts_with("mailto:")
                || url == "ms-settings:privacy-microphone"
                || url == "ms-settings:privacy-webcam"
            {
                let u = Ancho::de(&url);
                unsafe {
                    ShellExecuteW(
                        Some(hwnd()),
                        w!("open"),
                        u.p(),
                        PCWSTR::null(),
                        PCWSTR::null(),
                        SW_SHOWNORMAL,
                    );
                }
            }
        }
        Orden::AbreArchivo { ruta } => {
            // A file that arrived in SECURE, with its own program (the session never asks
            // this for a program or a script; Windows also sees it marked as from outside).
            let r = Ancho::de(&ruta);
            unsafe {
                ShellExecuteW(
                    Some(hwnd()),
                    w!("open"),
                    r.p(),
                    PCWSTR::null(),
                    PCWSTR::null(),
                    SW_SHOWNORMAL,
                );
            }
        }
        Orden::MuestraArchivo { ruta } => {
            // The folder, with the file selected.
            let argumentos = Ancho::de(&format!("/select,\"{ruta}\""));
            unsafe {
                ShellExecuteW(
                    Some(hwnd()),
                    w!("open"),
                    w!("explorer.exe"),
                    argumentos.p(),
                    PCWSTR::null(),
                    SW_SHOWNORMAL,
                );
            }
        }
        Orden::BorraNavegacion => borra_navegacion(),
        Orden::Seguimiento { estricto } => {
            con_vistas(|v| v.estricto = estricto);
            let vistas: Vec<Vista> = con_vistas(|v| {
                v.pestanas
                    .values()
                    .filter_map(|p| p.vista.clone())
                    .collect()
            })
            .unwrap_or_default();
            for v in vistas {
                pon_seguimiento(&v.web, estricto);
            }
        }
        Orden::Tema { .. } => {
            // 1.0.44: the program is dark only (the website's palette); the frame is repainted
            // all the same, and every tab gets the colour of what it shows.
            let col = colores();
            pinta_marco_ventana(hwnd(), &col);
            let vistas: Vec<(ICoreWebView2Controller, ICoreWebView2)> = con_vistas(|v| {
                v.pestanas
                    .values()
                    .filter_map(|p| p.vista.as_ref())
                    .map(|x| (x.controlador.clone(), x.web.clone()))
                    .collect()
            })
            .unwrap_or_default();
            for (c, w) in vistas {
                let url = texto_de(|p| unsafe { w.Source(p) });
                color_fondo(&c, fondo_para(&url));
            }
        }
        Orden::Titulo { texto } => {
            let t = Ancho::de(&texto);
            let _ = unsafe { SetWindowTextW(hwnd(), t.p()) };
        }
        Orden::CierraVentana => {
            let _ = unsafe { PostMessageW(Some(hwnd()), WM_CLOSE, WPARAM(0), LPARAM(0)) };
        }
        Orden::ActivaLicencia { clave } => {
            let Some(lugar) = LUGAR.get().cloned() else {
                de_licencia(DeLicencia::Activada(Err(Fallo::Disco)));
                return;
            };
            let _ = std::thread::Builder::new()
                .name("licencia".into())
                .spawn(move || {
                    let r = lugar.activar(&clave, sesion::ahora_sistema());
                    de_licencia(DeLicencia::Activada(r));
                });
        }
        Orden::BuscaVersion => {
            let Some(lugar) = LUGAR.get().cloned() else {
                de_licencia(DeLicencia::Version(Err("sin carpeta de datos".into())));
                return;
            };
            let lanzado = std::thread::Builder::new()
                .name("version".into())
                .spawn(move || {
                    let r = lugar.ultima_version(sesion::ahora_sistema());
                    de_licencia(DeLicencia::Version(r));
                });
            if lanzado.is_err() {
                de_licencia(DeLicencia::Version(Err("no se pudo empezar".into())));
            }
        }
        Orden::ComprobarLicencia => {
            let Some(lugar) = LUGAR.get().cloned() else {
                return;
            };
            if COMPROBANDO.swap(true, std::sync::atomic::Ordering::SeqCst) {
                return;
            }
            let lanzado = std::thread::Builder::new()
                .name("licencia".into())
                .spawn(move || {
                    if let Some(e) = lugar.comprobar(sesion::ahora_sistema()) {
                        de_licencia(DeLicencia::Comprobada(e));
                    }
                    COMPROBANDO.store(false, std::sync::atomic::Ordering::SeqCst);
                });
            if lanzado.is_err() {
                COMPROBANDO.store(false, std::sync::atomic::Ordering::SeqCst);
            }
        }
    }
}

/// Post a message to one of the browser's own pages, and only to them: a tab gets it only while
/// the engine says it shows `https://zero.guardiana/…` (the session checked it too).
fn envia(a: Origen, json: &str) {
    let Some(v) = vista_de(a) else {
        return;
    };
    let fuente = texto_de(|p| unsafe { v.web.Source(p) });
    if !sesion::es_interna(&fuente) {
        return;
    }
    let j = Ancho::de(json);
    vale("mensaje", unsafe { v.web.PostWebMessageAsJson(j.p()) });
}

/// Print one of the browser's own pages to a PDF in the person's Downloads.
fn guarda_pdf(id: u32, ruta: PathBuf) {
    let (Some(v), Some(entorno)) = (
        vista_de(Origen::Pestana(id)),
        con_vistas(|v| v.entorno.clone()).flatten(),
    ) else {
        // The page waits for an answer (the summary keeps its list hidden until then).
        encola(con_sesion(|s| s.pdf_hecho(id, &ruta, false)).unwrap_or_default());
        return;
    };
    let r = (|| unsafe {
        let ajustes = entorno
            .cast::<ICoreWebView2Environment6>()?
            .CreatePrintSettings()?;
        ajustes.SetShouldPrintBackgrounds(true)?;
        ajustes.SetShouldPrintHeaderAndFooter(false)?;
        let destino = ruta.clone();
        let h = PrintToPdfCompletedHandler::create(Box::new(
            move |r: windows::core::Result<()>, ok: bool| {
                let bien = r.is_ok() && ok;
                encola(con_sesion(|s| s.pdf_hecho(id, &destino, bien)).unwrap_or_default());
                Ok(())
            },
        ));
        let c = Ancho::de(&ruta.to_string_lossy());
        v.web
            .cast::<ICoreWebView2_7>()?
            .PrintToPdf(c.p(), &ajustes, &h)
    })();
    if let Err(e) = r {
        registra("pdf", &e);
        encola(con_sesion(|s| s.pdf_hecho(id, &ruta, false)).unwrap_or_default());
    }
}

fn pon_seguimiento(web: &ICoreWebView2, estricto: bool) {
    let nivel = if estricto {
        COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_STRICT
    } else {
        COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_BALANCED
    };
    let r = (|| unsafe {
        let p = web.cast::<ICoreWebView2_13>()?.Profile()?;
        p.cast::<ICoreWebView2Profile3>()?
            .SetPreferredTrackingPreventionLevel(nivel)
    })();
    vale("seguimiento", r);
}

fn borra_navegacion() {
    let general = con_vistas(|v| {
        v.pestanas
            .values()
            .find(|p| p.perfil == "general")
            .and_then(|p| p.vista.clone())
    })
    .flatten();
    let Some(v) = general else {
        con_vistas(|v| v.borrar_pendiente = true);
        return;
    };
    let r = (|| unsafe {
        let p = v.web.cast::<ICoreWebView2_13>()?.Profile()?;
        let h =
            ClearBrowsingDataCompletedHandler::create(Box::new(|r: windows::core::Result<()>| {
                vale("borrar datos de navegación", r);
                Ok(())
            }));
        p.cast::<ICoreWebView2Profile2>()?.ClearBrowsingDataAll(&h)
    })();
    vale("borrar", r);
}

// --- layout ----------------------------------------------------------------------------------

fn escala(h: HWND) -> f64 {
    let dpi = unsafe { GetDpiForWindow(h) };
    if dpi == 0 {
        1.0
    } else {
        f64::from(dpi) / 96.0
    }
}

fn px(css: i32, s: f64) -> i32 {
    (f64::from(css) * s).round() as i32
}

/// Place every view: the bar on top, the panel on the right when open, the active tab in the
/// rest. A tab in full screen takes the whole window.
fn coloca() {
    let Some((h, barra, panel, pestanas, activa, marco, completa)) = con_vistas(|v| {
        (
            v.hwnd,
            v.barra.clone(),
            v.panel.clone(),
            v.pestanas
                .iter()
                .filter_map(|(id, p)| p.vista.clone().map(|x| (*id, x)))
                .collect::<Vec<_>>(),
            v.activa,
            v.marco,
            v.completa.map(|c| c.0),
        )
    }) else {
        return;
    };
    let mut r = RECT::default();
    if unsafe { GetClientRect(h, &mut r) }.is_err() {
        return;
    }
    let s = escala(h);
    let (ancho, alto) = (r.right - r.left, r.bottom - r.top);
    let alto_barra = px(marco.0, s).min(alto);
    let ancho_panel = px(marco.1, s).min(ancho / 2);
    let abierto = ancho_panel > 0;
    let en_completa = completa.is_some();
    let contenido = if en_completa {
        RECT {
            left: 0,
            top: 0,
            right: ancho,
            bottom: alto,
        }
    } else {
        RECT {
            left: 0,
            top: alto_barra,
            right: ancho - ancho_panel,
            bottom: alto,
        }
    };
    unsafe {
        if let Some(b) = &barra {
            let _ = b.controlador.SetBounds(RECT {
                left: 0,
                top: 0,
                right: ancho,
                bottom: alto_barra,
            });
            let _ = b.controlador.SetIsVisible(!en_completa && alto_barra > 0);
        }
        if let Some(p) = &panel {
            let _ = p.controlador.SetBounds(RECT {
                left: ancho - ancho_panel,
                top: alto_barra,
                right: ancho,
                bottom: alto,
            });
            let _ = p.controlador.SetIsVisible(abierto && !en_completa);
        }
        for (id, v) in &pestanas {
            if *id == activa {
                let _ = v.controlador.SetBounds(contenido);
                let _ = v.controlador.SetIsVisible(true);
            } else {
                let _ = v.controlador.SetIsVisible(false);
            }
        }
    }
}

fn avisa_movimiento() {
    let vistas: Vec<Vista> = con_vistas(|v| {
        let mut l: Vec<Vista> = v
            .pestanas
            .values()
            .filter_map(|p| p.vista.clone())
            .collect();
        l.extend(v.barra.clone());
        l.extend(v.panel.clone());
        l
    })
    .unwrap_or_default();
    for v in vistas {
        let _ = unsafe { v.controlador.NotifyParentWindowPositionChanged() };
    }
}

/// The maximised window on the monitor it is on: its work area (the monitor minus the taskbar,
/// on any edge), at that monitor's scale, a pixel free on the edge of a taskbar that hides
/// itself (the owner, 18 Oct 2026: «abajo se ve cortado»; see `guardiana_zero::ventana`).
fn maximo_en_area(h: HWND) -> Option<(POINT, POINT)> {
    use guardiana_zero::ventana::{maximizada, Ocultas, Rect};
    use windows::Win32::UI::Shell::{
        SHAppBarMessage, ABE_BOTTOM, ABE_LEFT, ABE_RIGHT, ABE_TOP, ABM_GETAUTOHIDEBAREX, APPBARDATA,
    };
    let monitor = unsafe { MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST) };
    let mut mi = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !unsafe { GetMonitorInfoW(monitor, &mut mi) }.as_bool() {
        return None;
    }
    let r = |x: RECT| Rect::new(x.left, x.top, x.right, x.bottom);
    let oculta = |borde: u32| {
        let mut abd = APPBARDATA {
            cbSize: std::mem::size_of::<APPBARDATA>() as u32,
            uEdge: borde,
            rc: mi.rcMonitor,
            ..Default::default()
        };
        unsafe { SHAppBarMessage(ABM_GETAUTOHIDEBAREX, &mut abd) != 0 }
    };
    let ocultas = Ocultas {
        izquierda: oculta(ABE_LEFT),
        arriba: oculta(ABE_TOP),
        derecha: oculta(ABE_RIGHT),
        abajo: oculta(ABE_BOTTOM),
    };
    let dpi = unsafe { GetDpiForWindow(h) }.max(96);
    let borde = unsafe {
        windows::Win32::UI::HiDpi::GetSystemMetricsForDpi(
            windows::Win32::UI::WindowsAndMessaging::SM_CXSIZEFRAME,
            dpi,
        ) + windows::Win32::UI::HiDpi::GetSystemMetricsForDpi(
            windows::Win32::UI::WindowsAndMessaging::SM_CXPADDEDBORDER,
            dpi,
        )
    };
    let ((x, y), (w, al)) = maximizada(r(mi.rcMonitor), r(mi.rcWork), borde, ocultas);
    Some((POINT { x, y }, POINT { x: w, y: al }))
}

/// Maximised and the taskbar moved, appeared or changed size: fit the work area again.
fn reajusta_maximizada(h: HWND) {
    if !unsafe { windows::Win32::UI::WindowsAndMessaging::IsZoomed(h) }.as_bool()
        || con_vistas(|v| v.completa.is_some()) == Some(true)
    {
        return;
    }
    let monitor = unsafe { MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST) };
    let mut mi = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !unsafe { GetMonitorInfoW(monitor, &mut mi) }.as_bool() {
        return;
    }
    if let Some((pos, tam)) = maximo_en_area(h) {
        let _ = unsafe {
            SetWindowPos(
                h,
                None,
                mi.rcMonitor.left + pos.x,
                mi.rcMonitor.top + pos.y,
                tam.x,
                tam.y,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
            )
        };
    }
}

/// Tell the taskbar that the window is in full screen (a video, the globe's «Full screen»), so
/// it steps aside instead of covering the bottom of the page, the way browsers do it.
fn marca_completa(h: HWND, si: bool) {
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
    use windows::Win32::UI::Shell::{ITaskbarList2, TaskbarList};
    let r = (|| unsafe {
        let t: ITaskbarList2 = CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER)?;
        t.HrInit()?;
        t.MarkFullscreenWindow(h, si)
    })();
    vale("barra de tareas", r);
}

/// A video (or any page) asks for the whole screen, or gives it back.
fn pantalla_completa(id: u32, si: bool) {
    let h = hwnd();
    let antes = con_vistas(|v| v.completa).flatten();
    unsafe {
        if si && antes.is_none() {
            let estilo = GetWindowLongPtrW(h, GWL_STYLE);
            let mut sitio = RECT::default();
            let _ = GetWindowRect(h, &mut sitio);
            con_vistas(|v| v.completa = Some((id, estilo, sitio)));
            // A popup without a frame: what the taskbar recognises as full screen.
            SetWindowLongPtrW(
                h,
                GWL_STYLE,
                (estilo & !(WS_OVERLAPPEDWINDOW.0 as isize))
                    | windows::Win32::UI::WindowsAndMessaging::WS_POPUP.0 as isize,
            );
            let mut mi = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            let _ = GetMonitorInfoW(MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST), &mut mi);
            let m = mi.rcMonitor;
            let _ = SetWindowPos(
                h,
                Some(HWND_TOP),
                m.left,
                m.top,
                m.right - m.left,
                m.bottom - m.top,
                SWP_NOOWNERZORDER | SWP_FRAMECHANGED,
            );
            marca_completa(h, true);
        } else if !si {
            if let Some((_, estilo, r)) = antes {
                con_vistas(|v| v.completa = None);
                marca_completa(h, false);
                SetWindowLongPtrW(h, GWL_STYLE, estilo);
                let _ = SetWindowPos(
                    h,
                    None,
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOOWNERZORDER | SWP_FRAMECHANGED | SWP_NOZORDER,
                );
            }
        }
    }
    coloca();
}

// --- creating views ----------------------------------------------------------------------------

fn crea_controlador(
    perfil: &str,
    privada: bool,
    hecho: impl FnOnce(ICoreWebView2Controller) + 'static,
) -> windows::core::Result<()> {
    let (entorno, h) = con_vistas(|v| (v.entorno.clone(), v.hwnd))
        .ok_or_else(|| windows::core::Error::from(E_POINTER))?;
    let entorno = entorno.ok_or_else(|| windows::core::Error::from(E_POINTER))?;
    unsafe {
        let e10: ICoreWebView2Environment10 = entorno.cast()?;
        let opciones = e10.CreateCoreWebView2ControllerOptions()?;
        let nombre = Ancho::de(perfil);
        opciones.SetProfileName(nombre.p())?;
        opciones.SetIsInPrivateModeEnabled(privada)?;
        let manejador = CreateCoreWebView2ControllerCompletedHandler::create(Box::new(
            move |r: windows::core::Result<()>, c: Option<ICoreWebView2Controller>| {
                match (r, c) {
                    (Ok(()), Some(c)) => hecho(c),
                    (Err(e), _) => registra("crear vista", &e),
                    _ => registra("crear vista", &"sin controlador"),
                }
                Ok(())
            },
        ));
        e10.CreateCoreWebView2ControllerWithOptions(h, &opciones, &manejador)
    }
}

fn ajustes_comunes(web: &ICoreWebView2, propia: bool) -> windows::core::Result<()> {
    unsafe {
        let s = web.Settings()?;
        // SmartScreen sends addresses to Microsoft to check them: not from this browser.
        if let Ok(s8) = s.cast::<ICoreWebView2Settings8>() {
            s8.SetIsReputationCheckingRequired(false)?;
        }
        // Nothing typed is kept unless the person keeps it: no saved passwords, no autofill.
        if let Ok(s4) = s.cast::<ICoreWebView2Settings4>() {
            s4.SetIsPasswordAutosaveEnabled(false)?;
            s4.SetIsGeneralAutofillEnabled(false)?;
        }
        if propia {
            // The bar and the panel are the browser, not pages: no zoom, no reload keys, and a
            // menu with only what editing text needs (the owner, 12 Oct 2026: cut, copy and
            // paste in the address bar). Without that menu, none at all.
            if menu_de_texto(web).is_err() {
                s.SetAreDefaultContextMenusEnabled(false)?;
            }
            s.SetAreDevToolsEnabled(std::env::var_os("GUARDIANA_ZERO_ARGS").is_some())?;
            s.SetIsZoomControlEnabled(false)?;
            s.SetIsStatusBarEnabled(false)?;
            if let Ok(s3) = s.cast::<ICoreWebView2Settings3>() {
                s3.SetAreBrowserAcceleratorKeysEnabled(false)?;
            }
            if let Ok(s5) = s.cast::<ICoreWebView2Settings5>() {
                s5.SetIsPinchZoomEnabled(false)?;
            }
            if let Ok(s6) = s.cast::<ICoreWebView2Settings6>() {
                s6.SetIsSwipeNavigationEnabled(false)?;
            }
        }
    }
    Ok(())
}

/// What the menu of the bar and the panel keeps: editing text and copying what is selected.
const MENU_DE_TEXTO: [&str; 8] = [
    "undo",
    "redo",
    "cut",
    "copy",
    "paste",
    "pasteAndMatchStyle",
    "selectAll",
    "emoji",
];

/// The right-click (or long-press) menu of the bar and the panel: only [`MENU_DE_TEXTO`], with
/// no separator at its ends or two together; off a field and with nothing selected, no menu.
fn menu_de_texto(web: &ICoreWebView2) -> windows::core::Result<()> {
    let w11 = web.cast::<ICoreWebView2_11>()?;
    let h = ContextMenuRequestedEventHandler::create(Box::new(
        |_w, args: Option<ICoreWebView2ContextMenuRequestedEventArgs>| {
            let Some(args) = args else {
                return Ok(());
            };
            unsafe {
                let lista = args.MenuItems()?;
                let separador = |i: u32| -> windows::core::Result<bool> {
                    let mut k = COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND::default();
                    lista.GetValueAtIndex(i)?.Kind(&mut k)?;
                    Ok(k == COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_SEPARATOR)
                };
                let cuantos = || -> windows::core::Result<u32> {
                    let mut n = 0u32;
                    lista.Count(&mut n)?;
                    Ok(n)
                };
                // From the end, so the places still to read do not move.
                let mut i = cuantos()?;
                while i > 0 {
                    i -= 1;
                    let item = lista.GetValueAtIndex(i)?;
                    let nombre = texto_de(|p| item.Name(p));
                    if !separador(i)? && !MENU_DE_TEXTO.contains(&nombre.as_str()) {
                        lista.RemoveValueAtIndex(i)?;
                    }
                }
                let mut i = 0u32;
                let mut tras_separador = true;
                while i < cuantos()? {
                    let sep = separador(i)?;
                    if sep && tras_separador {
                        lista.RemoveValueAtIndex(i)?;
                        continue;
                    }
                    tras_separador = sep;
                    i += 1;
                }
                let n = cuantos()?;
                if n > 0 && separador(n - 1)? {
                    lista.RemoveValueAtIndex(n - 1)?;
                }
                if cuantos()? == 0 {
                    args.SetHandled(true)?;
                }
            }
            Ok(())
        },
    ));
    let mut t = 0i64;
    unsafe { w11.add_ContextMenuRequested(&h, &mut t) }
}

fn mapea_interfaz(web: &ICoreWebView2, acceso: COREWEBVIEW2_HOST_RESOURCE_ACCESS_KIND) {
    let carpeta = con_vistas(|v| v.carpeta_ui.clone()).unwrap_or_default();
    let c = Ancho::de(&carpeta.to_string_lossy());
    let r = (|| unsafe {
        web.cast::<ICoreWebView2_3>()?
            .SetVirtualHostNameToFolderMapping(w!("zero.guardiana"), c.p(), acceso)
    })();
    vale("carpeta de la interfaz", r);
}

/// Messages from a view, with the address of the document that sent them as the engine says.
fn escucha_mensajes(web: &ICoreWebView2, origen: Origen) -> windows::core::Result<()> {
    let h = WebMessageReceivedEventHandler::create(Box::new(
        move |_w, args: Option<ICoreWebView2WebMessageReceivedEventArgs>| {
            let Some(args) = args else {
                return Ok(());
            };
            let fuente = texto_de(|p| unsafe { args.Source(p) });
            let json = texto_de(|p| unsafe { args.WebMessageAsJson(p) });
            encola(con_sesion(|s| s.mensaje(origen, &fuente, &json)).unwrap_or_default());
            Ok(())
        },
    ));
    let mut t = 0i64;
    unsafe { web.add_WebMessageReceived(&h, &mut t) }
}

/// The page script inside the frames of a tab (decision 206). WebView2 runs it in child frames
/// too, and a frame talks to the browser on its own channel (`CoreWebView2Frame`, for the frames
/// directly inside the page). The session reads from it only what it measured and its question;
/// the answer goes back to that frame alone. An engine without frames' channel: nothing here.
fn escucha_marcos(web: &ICoreWebView2, id: u32) {
    let Ok(w4) = web.cast::<ICoreWebView2_4>() else {
        return;
    };
    let h = FrameCreatedEventHandler::create(Box::new(
        move |_w, args: Option<ICoreWebView2FrameCreatedEventArgs>| {
            let Some(args) = args else {
                return Ok(());
            };
            let Ok(marco) = (unsafe { args.Frame() }) else {
                return Ok(());
            };
            let Ok(marco) = marco.cast::<ICoreWebView2Frame2>() else {
                return Ok(());
            };
            let destino = marco.clone();
            let hm = FrameWebMessageReceivedEventHandler::create(Box::new(
                move |_f, a: Option<ICoreWebView2WebMessageReceivedEventArgs>| {
                    let Some(a) = a else {
                        return Ok(());
                    };
                    let fuente = texto_de(|p| unsafe { a.Source(p) });
                    let json = texto_de(|p| unsafe { a.WebMessageAsJson(p) });
                    let (respuesta, o) = con_sesion(|s| s.mensaje_de_marco(id, &fuente, &json))
                        .unwrap_or((None, Vec::new()));
                    if let Some(r) = respuesta {
                        let j = Ancho::de(&r);
                        vale("respuesta al marco", unsafe {
                            destino.PostWebMessageAsJson(j.p())
                        });
                    }
                    encola(o);
                    Ok(())
                },
            ));
            let mut t = 0i64;
            vale("mensajes del marco", unsafe {
                marco.add_WebMessageReceived(&hm, &mut t)
            });
            Ok(())
        },
    ));
    let mut t = 0i64;
    vale("marcos", unsafe { w4.add_FrameCreated(&h, &mut t) });
}

fn pulsada(vk: i32) -> bool {
    unsafe { GetKeyState(vk) < 0 }
}

fn escucha_teclas(c: &ICoreWebView2Controller) -> windows::core::Result<()> {
    let h = AcceleratorKeyPressedEventHandler::create(Box::new(
        |_c, args: Option<ICoreWebView2AcceleratorKeyPressedEventArgs>| {
            let Some(args) = args else {
                return Ok(());
            };
            let mut tipo = COREWEBVIEW2_KEY_EVENT_KIND::default();
            unsafe { args.KeyEventKind(&mut tipo)? };
            if tipo != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN
                && tipo != COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN
            {
                return Ok(());
            }
            let mut estado = COREWEBVIEW2_PHYSICAL_KEY_STATUS::default();
            unsafe { args.PhysicalKeyStatus(&mut estado)? };
            let mut vk = 0u32;
            unsafe { args.VirtualKey(&mut vk)? };
            // F7 turns on caret browsing in WebView2: a blinking text bar wherever one clicks,
            // on text that is not meant to be edited (the owner, 18 Oct 2026: «donde no hay enlace
            // no debe salir nada»). It is swallowed here, in the bar, the panel and every tab.
            if vk == 0x76 {
                unsafe { args.SetHandled(true)? };
                return Ok(());
            }
            let (ctrl, shift, alt) = (pulsada(0x11), pulsada(0x10), pulsada(0x12));
            if let Some(o) = con_sesion(|s| s.tecla(vk, ctrl, shift, alt)).flatten() {
                unsafe { args.SetHandled(true)? };
                // A held key repeats: one new tab per press, not one per repeat.
                if !estado.WasKeyDown.as_bool() || vk == 0x09 {
                    encola(o);
                }
            }
            Ok(())
        },
    ));
    let mut t = 0i64;
    unsafe { c.add_AcceleratorKeyPressed(&h, &mut t) }
}

/// The background of the program's own pages (--bg in interfaz/diseno.css): a tab showing one
/// of them is painted in it between a reload and the first paint, never white (the owner,
/// 17 Oct 2026: reloading GUARDIANA «hace una luz blanca … un fogonazo»). Dark only since
/// 1.0.44, the website's #06080C: nothing flashes navy or white while a page loads.
const fn fondo_propio() -> COREWEBVIEW2_COLOR {
    COREWEBVIEW2_COLOR {
        A: 255,
        R: 0x06,
        G: 0x08,
        B: 0x0C,
    }
}

/// What a tab shows before its page paints: the program's colour for its own pages (ZERO's,
/// SECURE, GUARDIANA's panel and its wait page), white for the web, as every browser does.
fn fondo_para(url: &str) -> COREWEBVIEW2_COLOR {
    if sesion::es_interna(url)
        || url.starts_with("http://127.0.0.1:7443/")
        || url.contains("guardiana-arrancando.html")
    {
        fondo_propio()
    } else {
        COREWEBVIEW2_COLOR {
            A: 255,
            R: 255,
            G: 255,
            B: 255,
        }
    }
}

fn color_fondo(c: &ICoreWebView2Controller, color: COREWEBVIEW2_COLOR) {
    if let Ok(c2) = c.cast::<ICoreWebView2Controller2>() {
        let _ = unsafe { c2.SetDefaultBackgroundColor(color) };
    }
}

/// The bar and the panel: the browser's own pages, in a profile of their own.
fn crea_propia(origen: Origen, colores_fondo: COREWEBVIEW2_COLOR) {
    let r = crea_controlador("interfaz", false, move |c| {
        let r = (|| -> windows::core::Result<()> {
            let web = unsafe { c.CoreWebView2()? };
            color_fondo(&c, colores_fondo);
            ajustes_comunes(&web, true)?;
            mapea_interfaz(&web, COREWEBVIEW2_HOST_RESOURCE_ACCESS_KIND_ALLOW);
            escucha_mensajes(&web, origen)?;
            escucha_teclas(&c)?;
            // A link in the panel (a company's deletion page) opens as a tab, never inside it.
            let h = NewWindowRequestedEventHandler::create(Box::new(
                |_w, args: Option<ICoreWebView2NewWindowRequestedEventArgs>| {
                    if let Some(args) = args {
                        let url = texto_de(|p| unsafe { args.Uri(p) });
                        unsafe { args.SetHandled(true)? };
                        if url.starts_with("https://") {
                            encola(vec![Orden::Navega {
                                id: con_sesion(|s| s.activa()).unwrap_or(0),
                                url,
                            }]);
                        }
                    }
                    Ok(())
                },
            ));
            let mut t = 0i64;
            unsafe { web.add_NewWindowRequested(&h, &mut t)? };
            // The bar and the panel never leave the browser's own pages.
            let guarda = NavigationStartingEventHandler::create(Box::new(
                |_w, args: Option<ICoreWebView2NavigationStartingEventArgs>| {
                    if let Some(args) = args {
                        let url = texto_de(|p| unsafe { args.Uri(p) });
                        if !sesion::es_interna(&url) {
                            unsafe { args.SetCancel(true)? };
                        }
                    }
                    Ok(())
                },
            ));
            unsafe { web.add_NavigationStarting(&guarda, &mut t)? };
            let vista = Vista {
                controlador: c.clone(),
                web: web.clone(),
            };
            con_vistas(|v| match origen {
                Origen::Barra => v.barra = Some(vista),
                _ => v.panel = Some(vista),
            });
            coloca();
            let url = if origen == Origen::Barra {
                sesion::BARRA
            } else {
                sesion::PANEL
            };
            let u = Ancho::de(url);
            unsafe { web.Navigate(u.p()) }
        })();
        vale("vista propia", r);
    });
    vale("crear vista propia", r);
}

fn crea_pestana(id: u32, perfil: Perfil, url: Option<String>, de: Option<u32>) {
    let (nombre, privada) = con_vistas(|v| {
        match de.and_then(|d| v.pestanas.get(&d)) {
            // A window a page opens shares that page's session (a sign-in popup needs it).
            Some(p) => (p.perfil.clone(), p.privada),
            None => match perfil {
                Perfil::General => ("general".to_string(), false),
                Perfil::Aislada => (format!("aislada-{id}"), true),
                Perfil::Mandato => (format!("mandato-{id}"), true),
            },
        }
    })
    .unwrap_or_else(|| ("general".to_string(), false));
    con_vistas(|v| {
        v.pestanas.insert(
            id,
            PestanaW {
                vista: None,
                perfil: nombre.clone(),
                privada,
                pendiente: url,
            },
        );
    });
    let r = crea_controlador(&nombre, privada, move |c| {
        if con_vistas(|v| v.pestanas.contains_key(&id)) != Some(true) {
            // Closed before it existed.
            let _ = unsafe { c.Close() };
            return;
        }
        if let Err(e) = prepara_pestana(id, &c) {
            registra("preparar pestaña", &e);
        }
    });
    vale("crear pestaña", r);
}

fn prepara_pestana(id: u32, c: &ICoreWebView2Controller) -> windows::core::Result<()> {
    let web = unsafe { c.CoreWebView2()? };
    ajustes_comunes(&web, false)?;
    mapea_interfaz(&web, COREWEBVIEW2_HOST_RESOURCE_ACCESS_KIND_DENY);
    let estricto = con_vistas(|v| v.estricto).unwrap_or(false);
    pon_seguimiento(&web, estricto);
    unsafe {
        // Every request of every kind, from the page and its frames and workers, passes here.
        match web.cast::<ICoreWebView2_22>() {
            Ok(w22) => {
                w22.AddWebResourceRequestedFilterWithRequestSourceKinds(
                    w!("*"),
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                    COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL,
                )?;
                con_sesion(|s| s.pon_trabajadores(true));
            }
            Err(_) => {
                web.AddWebResourceRequestedFilter(w!("*"), COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL)?;
                con_sesion(|s| s.pon_trabajadores(false));
            }
        }
    }
    // Most tabs open on one of the program's own pages: their colour from the start.
    color_fondo(c, fondo_propio());
    eventos_pestana(id, &web, c.clone())?;
    escucha_mensajes(&web, Origen::Pestana(id))?;
    escucha_marcos(&web, id);
    escucha_teclas(c)?;
    let vista = Vista {
        controlador: c.clone(),
        web: web.clone(),
    };
    let borrar = con_vistas(|v| {
        let mut borrar = false;
        if let Some(p) = v.pestanas.get_mut(&id) {
            p.vista = Some(vista.clone());
            borrar = v.borrar_pendiente && p.perfil == "general";
        }
        if borrar {
            v.borrar_pendiente = false;
        }
        borrar
    })
    .unwrap_or(false);
    if borrar {
        borra_navegacion();
    }
    coloca();
    // The page script goes in before the first page; then the tab opens (or is handed to the
    // page that asked for a new window). A mandate's tab gets its own (WebRTC and WebTransport
    // off), and so do the windows its pages open, which are mandate tabs too.
    let guion = con_sesion(|s| s.guion_pestana(id)).unwrap_or_default();
    let g = Ancho::de(&guion);
    let web2 = web.clone();
    let hecho = AddScriptToExecuteOnDocumentCreatedCompletedHandler::create(Box::new(
        move |r: windows::core::Result<()>, _id: String| {
            vale("guion de páginas", r);
            let ventana = con_vistas(|v| v.ventanas.remove(&id)).flatten();
            if let Some((args, aplazado)) = ventana {
                unsafe {
                    vale("ventana nueva", args.SetNewWindow(&web2));
                    vale("ventana nueva", aplazado.Complete());
                }
                return Ok(());
            }
            let destino =
                con_vistas(|v| v.pestanas.get_mut(&id).and_then(|p| p.pendiente.take())).flatten();
            if let Some(u) = destino {
                let u = Ancho::de(&u);
                vale("abrir", unsafe { web2.Navigate(u.p()) });
            }
            Ok(())
        },
    ));
    unsafe { web.AddScriptToExecuteOnDocumentCreated(g.p(), &hecho) }
}

fn lee_cuerpo(req: &ICoreWebView2WebResourceRequest) -> Vec<u8> {
    let Ok(flujo) = (unsafe { req.Content() }) else {
        return Vec::new();
    };
    let out = lee_flujo(&flujo, guardiana_zero::medidas::MAX_CUERPO);
    // The engine reads the body again to send it: put it back at the start.
    let _ = unsafe { flujo.Seek(0, STREAM_SEEK_SET, None) };
    out
}

/// Read up to `max` bytes of a stream.
fn lee_flujo(flujo: &windows::Win32::System::Com::IStream, max: usize) -> Vec<u8> {
    let mut out = Vec::new();
    let mut buf = vec![0u8; 16 * 1024];
    loop {
        let mut leidos = 0u32;
        let hr = unsafe {
            flujo.Read(
                buf.as_mut_ptr().cast::<c_void>(),
                buf.len() as u32,
                Some(&mut leidos as *mut u32),
            )
        };
        if hr.is_err() || leidos == 0 {
            break;
        }
        out.extend_from_slice(&buf[..leidos as usize]);
        if hr == S_FALSE || out.len() >= max {
            break;
        }
    }
    out
}

fn eventos_pestana(
    id: u32,
    web: &ICoreWebView2,
    controlador: ICoreWebView2Controller,
) -> windows::core::Result<()> {
    let mut t = 0i64;
    unsafe {
        web.add_NavigationStarting(
            &NavigationStartingEventHandler::create(Box::new(
                move |_w, args: Option<ICoreWebView2NavigationStartingEventArgs>| {
                    let Some(args) = args else {
                        return Ok(());
                    };
                    let url = texto_de(|p| args.Uri(p));
                    // A reload or a new page: the right colour behind it until it paints.
                    color_fondo(&controlador, fondo_para(&url));
                    let mut redirigida = BOOL::default();
                    let _ = args.IsRedirected(&mut redirigida);
                    let r = con_sesion(|s| s.navegacion_empieza(id, &url, redirigida.as_bool()));
                    if DEPURA.with(|d| *d) {
                        // Never the fragment: a licence key from a link travels there.
                        let sin = url.split('#').next().unwrap_or_default();
                        registra(
                            "navegación",
                            &format!("{id} {sin} {:?}", r.as_ref().map(|x| x.0)),
                        );
                    }
                    let (cancela, o) = r.unwrap_or((false, Vec::new()));
                    if cancela {
                        args.SetCancel(true)?;
                    }
                    encola(o);
                    Ok(())
                },
            )),
            &mut t,
        )?;
        web.add_ContentLoading(
            &ContentLoadingEventHandler::create(Box::new(move |w: Option<ICoreWebView2>, _| {
                if let Some(w) = w {
                    let url = texto_de(|p| w.Source(p));
                    encola(con_sesion(|s| s.pagina_nueva(id, &url)).unwrap_or_default());
                }
                Ok(())
            })),
            &mut t,
        )?;
        web.add_NavigationCompleted(
            &NavigationCompletedEventHandler::create(Box::new(
                move |w: Option<ICoreWebView2>, _args| {
                    let mut o = con_sesion(|s| s.navegacion_termina(id)).unwrap_or_default();
                    if let Some(w) = w {
                        let (mut a, mut b) = (BOOL::default(), BOOL::default());
                        let _ = w.CanGoBack(&mut a);
                        let _ = w.CanGoForward(&mut b);
                        o.extend(
                            con_sesion(|s| s.historial(id, a.as_bool(), b.as_bool()))
                                .unwrap_or_default(),
                        );
                        let url = texto_de(|p| w.Source(p));
                        if !(url.is_empty() || url.starts_with("about:")) {
                            muestra_ventana();
                        }
                    }
                    encola(o);
                    Ok(())
                },
            )),
            &mut t,
        )?;
        web.add_SourceChanged(
            &SourceChangedEventHandler::create(Box::new(move |w: Option<ICoreWebView2>, _| {
                if let Some(w) = w {
                    let url = texto_de(|p| w.Source(p));
                    encola(con_sesion(|s| s.fuente(id, &url)).unwrap_or_default());
                }
                Ok(())
            })),
            &mut t,
        )?;
        web.add_DocumentTitleChanged(
            &DocumentTitleChangedEventHandler::create(Box::new(
                move |w: Option<ICoreWebView2>, _| {
                    if let Some(w) = w {
                        let titulo = texto_de(|p| w.DocumentTitle(p));
                        encola(con_sesion(|s| s.titulo(id, &titulo)).unwrap_or_default());
                    }
                    Ok(())
                },
            )),
            &mut t,
        )?;
        web.add_HistoryChanged(
            &HistoryChangedEventHandler::create(Box::new(move |w: Option<ICoreWebView2>, _| {
                if let Some(w) = w {
                    let (mut a, mut b) = (BOOL::default(), BOOL::default());
                    let _ = w.CanGoBack(&mut a);
                    let _ = w.CanGoForward(&mut b);
                    encola(
                        con_sesion(|s| s.historial(id, a.as_bool(), b.as_bool()))
                            .unwrap_or_default(),
                    );
                }
                Ok(())
            })),
            &mut t,
        )?;
        web.add_NewWindowRequested(
            &NewWindowRequestedEventHandler::create(Box::new(
                move |_w, args: Option<ICoreWebView2NewWindowRequestedEventArgs>| {
                    let Some(args) = args else {
                        return Ok(());
                    };
                    // Windows a page opens on its own (not from a click) are not opened: a page
                    // in the background cannot flood tabs or take the front one.
                    let mut persona = BOOL::default();
                    let _ = args.IsUserInitiated(&mut persona);
                    if !persona.as_bool() {
                        args.SetHandled(true)?;
                        return Ok(());
                    }
                    // Where it goes (empty or `about:blank` when the page fills it in later): a
                    // pop-up to an advertising or tracking company is not opened.
                    let destino = texto_de(|p| args.Uri(p));
                    let aplazado = args.GetDeferral()?;
                    let Some((nueva, o)) = con_sesion(|s| s.ventana_nueva(id, &destino)).flatten()
                    else {
                        args.SetHandled(true)?;
                        aplazado.Complete()?;
                        return Ok(());
                    };
                    con_vistas(|v| v.ventanas.insert(nueva, (args.clone(), aplazado)));
                    encola(o);
                    Ok(())
                },
            )),
            &mut t,
        )?;
        // A page asking to show notifications as it loads, without a click: answered «no»
        // without a prompt. Asked after a click, the engine asks the person as always.
        web.add_PermissionRequested(
            &PermissionRequestedEventHandler::create(Box::new(
                move |_w, args: Option<ICoreWebView2PermissionRequestedEventArgs>| {
                    let Some(args) = args else {
                        return Ok(());
                    };
                    let mut tipo = COREWEBVIEW2_PERMISSION_KIND::default();
                    args.PermissionKind(&mut tipo)?;
                    // The microphone and the camera for GUARDIANA SECURE's own page (16 Oct
                    // 2026): the person already chose by pressing «Llamar» or «Contestar»; a
                    // second question from the engine (or a «no» saved once by mistake) only
                    // left the other person in silence. Any other page asks as always.
                    if tipo == COREWEBVIEW2_PERMISSION_KIND_MICROPHONE
                        || tipo == COREWEBVIEW2_PERMISSION_KIND_CAMERA
                    {
                        let url = texto_de(|p| args.Uri(p));
                        if url.starts_with(guardiana_zero::sesion::SECURE) {
                            args.SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW)?;
                        }
                        return Ok(());
                    }
                    if tipo == COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS {
                        let mut persona = BOOL::default();
                        let _ = args.IsUserInitiated(&mut persona);
                        if con_sesion(|s| s.pide_notificaciones(id, persona.as_bool()))
                            == Some(true)
                        {
                            args.SetState(COREWEBVIEW2_PERMISSION_STATE_DENY)?;
                            // This «no» is for this time only: by default WebView2 keeps the
                            // answer in the profile, and the site could never ask again, not
                            // even when the person wants its notifications (review of 12 Oct
                            // 2026). Older engines without the interface keep the old behaviour.
                            if let Ok(a3) =
                                args.cast::<ICoreWebView2PermissionRequestedEventArgs3>()
                            {
                                let _ = a3.SetSavesInProfile(false);
                            }
                        }
                    }
                    Ok(())
                },
            )),
            &mut t,
        )?;
        web.add_WebResourceRequested(
            &WebResourceRequestedEventHandler::create(Box::new(
                move |_w, args: Option<ICoreWebView2WebResourceRequestedEventArgs>| {
                    let Some(args) = args else {
                        return Ok(());
                    };
                    let req = args.Request()?;
                    let url = texto_de(|p| req.Uri(p));
                    if !["http://", "https://", "ws://", "wss://"]
                        .iter()
                        .any(|e| url.starts_with(e))
                    {
                        return Ok(());
                    }
                    let metodo = texto_de(|p| req.Method(p));
                    let mut contexto = COREWEBVIEW2_WEB_RESOURCE_CONTEXT::default();
                    args.ResourceContext(&mut contexto)?;
                    // The body is read when marked data or the decoy might travel in it, when it
                    // goes to a known pixel, which puts there what it tells, or when it goes to
                    // another site, whose size the shield says (decision 206).
                    let cuerpo = if metodo != "GET"
                        && metodo != "HEAD"
                        && con_sesion(|s| s.necesita_cuerpo(id, &url)) == Some(true)
                    {
                        lee_cuerpo(&req)
                    } else {
                        Vec::new()
                    };
                    // If the session cannot answer, nothing leaves (it never happens; if it did, the
                    // safe side is the cut).
                    // The names of its cookies go with it (never their values): a cut says how
                    // the destination knows you from one site to the next.
                    let cookies = req
                        .Headers()
                        .ok()
                        .filter(|h| {
                            let mut hay = BOOL::default();
                            h.Contains(w!("Cookie"), &mut hay).is_ok() && hay.as_bool()
                        })
                        .map(|h| texto_de(|p| h.GetHeader(w!("Cookie"), p)))
                        .unwrap_or_default();
                    // 1.0.46 (subastas-1.0.46): Global Privacy Control and the strict
                    // fingerprint mode, as the person set them.
                    let (r, gpc, uniforme) = con_sesion(|s| {
                        (
                            s.peticion_con(id, &url, &metodo, &cuerpo, &cookies, contexto.0),
                            s.gpc(),
                            s.huella_uniforme(),
                        )
                    })
                    .unwrap_or((
                        sesion::Respuesta {
                            cortar: true,
                            pagina: None,
                            nota: String::new(),
                        },
                        true,
                        false,
                    ));
                    if !r.nota.is_empty() {
                        registra("petición", &format!("{metodo} {url} {}", r.nota));
                    }
                    if !r.cortar {
                        // Global Privacy Control: «do not sell or share my data», on every request
                        // (unless the person turned it off, 1.0.46). With the strict fingerprint
                        // mode, only the first language goes in `Accept-Language`, as the page
                        // script says in `navigator.languages`.
                        if url.starts_with("http") && !sesion::es_interna(&url) {
                            if let Ok(h) = req.Headers() {
                                if gpc {
                                    let _ = h.SetHeader(w!("Sec-GPC"), w!("1"));
                                }
                                if uniforme {
                                    let mut hay = BOOL::default();
                                    if h.Contains(w!("Accept-Language"), &mut hay).is_ok()
                                        && hay.as_bool()
                                    {
                                        let todas =
                                            texto_de(|p| h.GetHeader(w!("Accept-Language"), p));
                                        let una = todas
                                            .split(',')
                                            .next()
                                            .and_then(|x| x.split(';').next())
                                            .map(str::trim)
                                            .unwrap_or_default();
                                        if !una.is_empty() && una.len() < 40 {
                                            let v = Ancho::de(una);
                                            let _ = h.SetHeader(w!("Accept-Language"), v.p());
                                        }
                                    }
                                }
                            }
                        }
                        return Ok(());
                    }
                    let Some(entorno) = con_vistas(|v| v.entorno.clone()).flatten() else {
                        return Ok(());
                    };
                    let respuesta = match &r.pagina {
                        Some(html) => {
                            let flujo = SHCreateMemStream(Some(html.as_bytes()));
                            entorno.CreateWebResourceResponse(
                                flujo.as_ref(),
                                200,
                                w!("OK"),
                                w!("Content-Type: text/html; charset=utf-8\r\nCache-Control: no-store"),
                            )?
                        }
                        None => entorno.CreateWebResourceResponse(
                            None::<&windows::Win32::System::Com::IStream>,
                            403,
                            w!("Cut by GUARDIANA ZERO"),
                            w!("Cache-Control: no-store"),
                        )?,
                    };
                    args.SetResponse(&respuesta)?;
                    Ok(())
                },
            )),
            &mut t,
        )?;
        // The site's icon for the tab: the engine fetches it through the same request checks.
        if let Ok(w15) = web.cast::<ICoreWebView2_15>() {
            w15.add_FaviconChanged(
                &FaviconChangedEventHandler::create(Box::new(
                    move |w: Option<ICoreWebView2>, _| {
                        let Some(w15) = w.and_then(|w| w.cast::<ICoreWebView2_15>().ok()) else {
                            return Ok(());
                        };
                        let h = GetFaviconCompletedHandler::create(Box::new(
                        move |r: windows::core::Result<()>,
                              flujo: Option<windows::Win32::System::Com::IStream>| {
                            let png = match (r, flujo) {
                                (Ok(()), Some(f)) => lee_flujo(&f, 64 * 1024),
                                _ => Vec::new(),
                            };
                            encola(con_sesion(|s| s.icono(id, &png)).unwrap_or_default());
                            Ok(())
                        },
                    ));
                        w15.GetFavicon(COREWEBVIEW2_FAVICON_IMAGE_FORMAT_PNG, &h)
                    },
                )),
                &mut t,
            )?;
        }
        web.add_ContainsFullScreenElementChanged(
            &ContainsFullScreenElementChangedEventHandler::create(Box::new(
                move |w: Option<ICoreWebView2>, _| {
                    if let Some(w) = w {
                        let mut si = BOOL::default();
                        let _ = w.ContainsFullScreenElement(&mut si);
                        pantalla_completa(id, si.as_bool());
                    }
                    Ok(())
                },
            )),
            &mut t,
        )?;
        web.add_ProcessFailed(
            &ProcessFailedEventHandler::create(Box::new(
                move |w: Option<ICoreWebView2>,
                      args: Option<ICoreWebView2ProcessFailedEventArgs>| {
                    let mut tipo = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
                    if let Some(a) = args {
                        let _ = a.ProcessFailedKind(&mut tipo);
                    }
                    registra("proceso del motor", &tipo.0);
                    if tipo == COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED {
                        // Without the engine there is no browser: close cleanly, saving first.
                        let _ = PostMessageW(Some(hwnd()), WM_CLOSE, WPARAM(0), LPARAM(0));
                        return Ok(());
                    }
                    // A page that hung or crashed is loaded again, at most twice a minute.
                    if tipo == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED
                        || tipo == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_UNRESPONSIVE
                    {
                        let ahora = sesion::ahora_sistema();
                        let otra = con_vistas(|v| {
                            let l = v.recargas.entry(id).or_default();
                            l.retain(|t| ahora - t < 60_000);
                            l.push(ahora);
                            l.len() <= 2
                        })
                        .unwrap_or(false);
                        if otra {
                            if let Some(w) = w {
                                let _ = w.Reload();
                            }
                        }
                    }
                    Ok(())
                },
            )),
            &mut t,
        )?;
    }
    Ok(())
}

// --- the window ----------------------------------------------------------------------------------

extern "system" fn proc_ventana(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_SIZE => {
            if wp.0 as u32 != SIZE_MINIMIZED {
                coloca();
            }
            LRESULT(0)
        }
        WM_MOVE | WM_MOVING => {
            avisa_movimiento();
            unsafe { DefWindowProcW(h, msg, wp, lp) }
        }
        WM_DPICHANGED => {
            let r = lp.0 as *const RECT;
            if !r.is_null() {
                let r = unsafe { *r };
                let _ = unsafe {
                    SetWindowPos(
                        h,
                        None,
                        r.left,
                        r.top,
                        r.right - r.left,
                        r.bottom - r.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    )
                };
            }
            // Moved to a monitor at another scale while maximised: that monitor's work area.
            reajusta_maximizada(h);
            coloca();
            LRESULT(0)
        }
        WM_GETMINMAXINFO => {
            let m = lp.0 as *mut MINMAXINFO;
            if !m.is_null() {
                let s = escala(h);
                unsafe {
                    (*m).ptMinTrackSize = POINT {
                        x: px(560, s),
                        y: px(420, s),
                    };
                    // Maximised: the work area of this window's monitor, never under the taskbar.
                    if let Some((pos, tam)) = maximo_en_area(h) {
                        (*m).ptMaxPosition = pos;
                        (*m).ptMaxSize = tam;
                    }
                }
            }
            LRESULT(0)
        }
        // The taskbar moved, appeared, changed size, or a monitor came or went.
        windows::Win32::UI::WindowsAndMessaging::WM_SETTINGCHANGE
        | windows::Win32::UI::WindowsAndMessaging::WM_DISPLAYCHANGE => {
            reajusta_maximizada(h);
            unsafe { DefWindowProcW(h, msg, wp, lp) }
        }
        WM_TIMER if wp.0 == TIC_LLAMADA => {
            encola(con_sesion(Sesion::tic_llamada).unwrap_or_default());
            drena();
            LRESULT(0)
        }
        WM_TIMER => {
            recoge_licencia();
            recoge_panel();
            encola(con_sesion(Sesion::tic).unwrap_or_default());
            drena();
            // During a call the conversations also run on a 20 ms clock; it stops with the call.
            let llamada = con_sesion(|s| s.en_llamada()).unwrap_or(false);
            if llamada != RAPIDO.swap(llamada, Ordering::Relaxed) {
                unsafe {
                    if llamada {
                        SetTimer(Some(h), TIC_LLAMADA, 20, None);
                    } else {
                        let _ = KillTimer(Some(h), TIC_LLAMADA);
                    }
                }
            }
            // The watch (17 Oct 2026): while a conversation is open the window keeps itself out of
            // screenshots and screen recordings (they get black), and every two seconds reads
            // Windows' record of who is using the microphone, the camera and the screen.
            let conversando = con_sesion(|s| s.conversando()).unwrap_or(false);
            if conversando != OCULTA_A_CAPTURAS.load(Ordering::Relaxed) {
                // Windows before 10 2004 may refuse both ways of hiding: the report then says the
                // screen was not protected; nothing else changes.
                let ok = oculta_a_capturas(h, conversando);
                OCULTA_A_CAPTURAS.store(conversando, Ordering::Relaxed);
                PROTEGIDA.store(conversando && ok, Ordering::Relaxed);
                TICS_VIGIA.store(0, Ordering::Relaxed);
            }
            if conversando && TICS_VIGIA.fetch_add(1, Ordering::Relaxed) % 10 == 0 {
                let usos = lee_usos();
                let protegida = PROTEGIDA.load(Ordering::Relaxed);
                let _ = con_sesion(|s| {
                    s.pantalla_protegida(protegida);
                    s.vigia(&usos);
                });
            }
            // A page that never arrives still has to appear: three seconds, then the window.
            if !VENTANA_LISTA.load(Ordering::Relaxed)
                && TICS_SIN_PAGINA.fetch_add(1, Ordering::Relaxed) >= 15
            {
                muestra_ventana();
            }
            LRESULT(0)
        }
        WM_ORDENES => {
            drena();
            LRESULT(0)
        }
        WM_COPYDATA => {
            let copia = lp.0 as *const COPYDATASTRUCT;
            if copia.is_null() {
                return LRESULT(0);
            }
            let copia = unsafe { &*copia };
            if copia.cbData < 2 || copia.lpData.is_null() || copia.cbData > 16_384 {
                return LRESULT(0);
            }
            let chars = unsafe {
                std::slice::from_raw_parts(copia.lpData.cast::<u16>(), copia.cbData as usize / 2)
            };
            let fin = chars.iter().position(|c| *c == 0).unwrap_or(chars.len());
            let texto = String::from_utf16_lossy(&chars[..fin]);
            if copia.dwData == 2 {
                // The panel, with a licence key to fill in when it carries one (checked again
                // here: the window takes nothing on trust).
                if !texto.is_empty() && !lanzador::es_clave_valida(&texto) {
                    return LRESULT(0);
                }
                let clave = (!texto.is_empty()).then_some(texto);
                if let (Some(carpeta), Some(t)) = (
                    con_vistas(|v| v.carpeta_ui.clone()),
                    con_sesion(|s| s.textos()),
                ) {
                    escribe_espera(&carpeta, &t);
                }
                encola(con_sesion(|s| abre_panel(s, clave)).unwrap_or_default());
            } else {
                encola(con_sesion(|s| s.abre_guardiana(&texto)).unwrap_or_default());
            }
            drena();
            LRESULT(1)
        }
        WM_SETFOCUS => {
            let activa = con_vistas(|v| v.activa).unwrap_or(0);
            if let Some(v) = vista_de(Origen::Pestana(activa)) {
                let _ = unsafe {
                    v.controlador
                        .MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC)
                };
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            con_sesion(Sesion::cierra);
            let todas: Vec<Vista> = con_vistas(|v| {
                let mut l: Vec<Vista> = std::mem::take(&mut v.pestanas)
                    .into_values()
                    .filter_map(|p| p.vista)
                    .collect();
                l.extend(v.barra.take());
                l.extend(v.panel.take());
                l
            })
            .unwrap_or_default();
            for v in todas {
                let _ = unsafe { v.controlador.Close() };
            }
            let _ = unsafe { DestroyWindow(h) };
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(h, msg, wp, lp) },
    }
}

/// Keep the window out of screenshots and screen recordings (they show it black), or let it
/// back in. Windows 10 2004 and later exclude it outright; before that, «monitor only» does the
/// same for captures. Returns whether Windows accepted.
fn oculta_a_capturas(h: HWND, si: bool) -> bool {
    unsafe {
        if !si {
            return SetWindowDisplayAffinity(h, WDA_NONE).is_ok();
        }
        SetWindowDisplayAffinity(h, WDA_EXCLUDEFROMCAPTURE).is_ok()
            || SetWindowDisplayAffinity(h, WDA_MONITOR).is_ok()
    }
}

/// The registry path as Windows wants it: UTF-16, ending in zero.
fn ancho(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The names of a key's subkeys (at most 512; none if it cannot be opened).
fn subclaves(raiz: HKEY, ruta: &str) -> Vec<String> {
    let r = ancho(ruta);
    let mut clave = HKEY(std::ptr::null_mut());
    if unsafe { RegOpenKeyExW(raiz, PCWSTR(r.as_ptr()), None, KEY_READ, &mut clave) }
        != ERROR_SUCCESS
    {
        return Vec::new();
    }
    let mut out = Vec::new();
    for i in 0..512u32 {
        let mut buf = [0u16; 512];
        let mut largo = buf.len() as u32;
        let e = unsafe {
            RegEnumKeyExW(
                clave,
                i,
                Some(PWSTR(buf.as_mut_ptr())),
                &mut largo,
                None,
                None,
                None,
                None,
            )
        };
        if e != ERROR_SUCCESS {
            break;
        }
        let n = (largo as usize).min(buf.len());
        out.push(String::from_utf16_lossy(&buf[..n]));
    }
    unsafe {
        let _ = RegCloseKey(clave);
    }
    out
}

/// A QWORD value of a key (zero when absent).
fn qword(raiz: HKEY, ruta: &[u16], valor: PCWSTR) -> u64 {
    let mut v = 0u64;
    let mut largo = std::mem::size_of::<u64>() as u32;
    let e = unsafe {
        RegGetValueW(
            raiz,
            PCWSTR(ruta.as_ptr()),
            valor,
            RRF_RT_REG_QWORD,
            None,
            Some((&mut v as *mut u64).cast::<c_void>()),
            Some(&mut largo as *mut u32),
        )
    };
    if e == ERROR_SUCCESS {
        v
    } else {
        0
    }
}

/// Windows' own record of who used the microphone, the camera and programmatic screen capture,
/// and when (the privacy settings' «recent activity»: `CapabilityAccessManager\ConsentStore`),
/// for this account and for the computer. Read only; nothing is written or kept here.
fn lee_usos() -> Vec<guardiana_zero::vigia::Uso> {
    use guardiana_zero::vigia::{de_registro, Recurso};
    let mut v = Vec::new();
    for raiz in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        for (cap, recurso) in [
            ("microphone", Recurso::Microfono),
            ("webcam", Recurso::Camara),
            ("graphicsCaptureProgrammatic", Recurso::Pantalla),
        ] {
            let base = format!(
                "Software\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\{cap}"
            );
            let mut una = |ruta: String, nombre: &str| {
                let r = ancho(&ruta);
                let inicio = qword(raiz, &r, w!("LastUsedTimeStart"));
                let fin = qword(raiz, &r, w!("LastUsedTimeStop"));
                if let Some(u) = de_registro(nombre, recurso, inicio, fin) {
                    v.push(u);
                }
            };
            for sub in subclaves(raiz, &base) {
                if sub.eq_ignore_ascii_case("NonPackaged") {
                    let np = format!("{base}\\NonPackaged");
                    for app in subclaves(raiz, &np) {
                        una(format!("{np}\\{app}"), &app);
                    }
                } else {
                    una(format!("{base}\\{sub}"), &sub);
                }
            }
        }
    }
    v
}

/// The frame's colours. Dark only since 1.0.44, the website's palette, whatever day or night
/// the person or Windows chose: the same tokens as interfaz/diseno.css, --marco and --card.
fn colores() -> Colores {
    let oscuro = true;
    let (m, h) = ((0x06, 0x08, 0x0C), (0x0C, 0x11, 0x18));
    let rgb = |(r, g, b): (u8, u8, u8)| {
        COLORREF(u32::from(r) | (u32::from(g) << 8) | (u32::from(b) << 16))
    };
    let wv = |(r, g, b): (u8, u8, u8)| COREWEBVIEW2_COLOR {
        A: 255,
        R: r,
        G: g,
        B: b,
    };
    Colores {
        oscuro,
        marco: rgb(m),
        hoja: wv(h),
        fondo_marco: wv(m),
    }
}

/// The icon of the given size from the `.ico` inside the program (its PNG images).
fn icono(tam: i32) -> Option<HICON> {
    let ico = crate::interfaz::ICONO;
    let n = usize::from(u16::from_le_bytes([*ico.get(4)?, *ico.get(5)?]));
    let mut mejor: Option<(i32, &[u8])> = None;
    for i in 0..n {
        let e = ico.get(6 + 16 * i..22 + 16 * i)?;
        let lado = if e[0] == 0 { 256 } else { i32::from(e[0]) };
        let largo = u32::from_le_bytes([e[8], e[9], e[10], e[11]]) as usize;
        let desde = u32::from_le_bytes([e[12], e[13], e[14], e[15]]) as usize;
        let datos = ico.get(desde..desde + largo)?;
        if lado >= tam && mejor.is_none_or(|(l, _)| lado < l) {
            mejor = Some((lado, datos));
        }
    }
    let (_, datos) = mejor?;
    unsafe { CreateIconFromResourceEx(datos, true, 0x0003_0000, tam, tam, LR_DEFAULTCOLOR).ok() }
}

fn carpeta_descargas() -> PathBuf {
    unsafe {
        if let Ok(p) = SHGetKnownFolderPath(&FOLDERID_Downloads, KF_FLAG_DEFAULT, None) {
            let s = p.to_string().unwrap_or_default();
            CoTaskMemFree(Some(p.0 as *const c_void));
            if !s.is_empty() {
                return PathBuf::from(s);
            }
        }
    }
    std::env::var_os("USERPROFILE")
        .map(|u| PathBuf::from(u).join("Downloads"))
        .unwrap_or_default()
}

/// The language ZERO starts in until the person picks one in its settings (18 Oct 2026, the
/// owner: «English is the first language on screen»). The Spanish and Portuguese installers were
/// an explicit choice, and they leave it in the registry (`idioma`); anything else is English.
/// Windows's own language is not asked any more: it says where the computer was bought, not what
/// the person chose.
fn idioma_sistema() -> String {
    for ruta in [
        "Software\\GUARDIANA GROUP\\GUARDIANA ZERO",
        "Software\\Guardiana",
    ] {
        let r = ancho(ruta);
        let mut buf = [0u16; 16];
        let mut largo = std::mem::size_of_val(&buf) as u32;
        let e = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                PCWSTR(r.as_ptr()),
                w!("idioma"),
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr().cast::<c_void>()),
                Some(&mut largo as *mut u32),
            )
        };
        if e == ERROR_SUCCESS {
            let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            let v = String::from_utf16_lossy(&buf[..n]);
            if matches!(v.as_str(), "es" | "en" | "pt") {
                return v;
            }
        }
    }
    "en".into()
}

fn aviso(textos: &Textos, titulo: &str, cuerpo: &str) {
    let t = Ancho::de(&textos.t(titulo));
    let c = Ancho::de(&textos.t(cuerpo));
    unsafe {
        MessageBoxW(None, c.p(), t.p(), MB_OK | MB_ICONWARNING);
    }
}

/// Start: one window per person (a second start brings the first one forward), the engine, the
/// bar, the panel and the first tab.
pub fn arranca() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    let base = std::env::var_os("GUARDIANA_ZERO_DATOS")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("LOCALAPPDATA").map(|l| PathBuf::from(l).join("GUARDIANA ZERO"))
        })
        .unwrap_or_else(|| PathBuf::from("GUARDIANA ZERO"));
    let datos = base.join("datos");
    let _ = std::fs::create_dir_all(&datos);
    REGISTRO.with(|r| *r.borrow_mut() = Some(datos.join("registro.txt")));
    let idioma = idioma_sistema();
    let textos = Textos::de(Idioma::de_etiqueta(&idioma));
    // `--abrir-guardiana <url>` (the CLI), `--panel` (the shortcuts and the end of the install)
    // and `--url guardiana://activar?clave=…` (the licence email), see `lanzador`.
    let pedido = {
        let args: Vec<String> = std::env::args().skip(1).collect();
        lanzador::pedido_de_argumentos(&args)
    };

    // One window: a second start of the same version brings the first one forward; a newer
    // version closes the older one (it saves as on its own X) and takes its place, so updating
    // is just opening the new file (the owner, 10 Oct 2026: «que se abra automática la última,
    // así el cliente será más fácil»). Before, the new copy brought the old window forward and
    // left without a word.
    let nombre_mutex = Ancho::de(&format!(
        "Local\\GuardianaZero-{}",
        base.to_string_lossy().replace('\\', "/")
    ));
    let marca = datos.join(EN_MARCHA);
    unsafe {
        // The handle stays open for the life of the process: that is what marks it as running.
        let mutex = CreateMutexW(None, true, nombre_mutex.p());
        if GetLastError() == ERROR_ALREADY_EXISTS {
            // The first one may still be opening its window: give it a few seconds. A copy that
            // runs with no window at all used to make every new start end in silence (the
            // owner, 10 Oct 2026: «no se me abre el navegador»); now the person is told.
            let mut otra = FindWindowW(CLASE, PCWSTR::null());
            for _ in 0..20 {
                if otra.is_ok() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(250));
                otra = FindWindowW(CLASE, PCWSTR::null());
            }
            let Ok(otra) = otra else {
                registra("arranque", &"otra copia en marcha sin ventana");
                aviso(&textos, "error_ya_abierto_titulo", "error_ya_abierto");
                return;
            };
            let suya = version_en_marcha(&marca, otra);
            let mia = version_de(env!("CARGO_PKG_VERSION")).unwrap_or_default();
            // Up to 1.0.4 nothing wrote the mark: no mark means an older copy.
            if suya.is_some_and(|v| v >= mia) {
                if let Some(p) = &pedido {
                    pasa_pedido(otra, p);
                }
                if IsIconic(otra).as_bool() {
                    let _ = ShowWindow(otra, SW_RESTORE);
                }
                let _ = SetForegroundWindow(otra);
                return;
            }
            registra(
                "arranque",
                &format!(
                    "cierra la versión {} para abrir la {}",
                    suya.map_or_else(
                        || "anterior".to_string(),
                        |(a, b, c)| format!("{a}.{b}.{c}")
                    ),
                    env!("CARGO_PKG_VERSION")
                ),
            );
            let _ = PostMessageW(Some(otra), WM_CLOSE, WPARAM(0), LPARAM(0));
            // The old copy owns the mutex until its process ends; waiting on it is waiting for
            // that end (WAIT_ABANDONED), after its session is saved.
            let fin = match &mutex {
                Ok(h) => WaitForSingleObject(*h, 30_000),
                Err(_) => WAIT_TIMEOUT,
            };
            if fin != WAIT_OBJECT_0 && fin != WAIT_ABANDONED {
                registra("arranque", &"la versión anterior no se cerró");
                aviso(&textos, "error_otra_version_titulo", "error_otra_version");
                return;
            }
            // Its WebView2 processes share the data folder: let them finish.
            std::thread::sleep(std::time::Duration::from_millis(1500));
        }
        // Kept open on purpose until the process ends (see above).
        std::mem::forget(mutex);
        registra(
            "arranque",
            &format!("GUARDIANA ZERO {}", env!("CARGO_PKG_VERSION")),
        );
    }
    let _ = std::fs::write(
        &marca,
        format!("{} {}", env!("CARGO_PKG_VERSION"), std::process::id()),
    );

    // The engine must be on the system (Windows 10 and 11 bring it; Windows Update keeps it).
    let mut version = PWSTR::null();
    let hay_motor =
        unsafe { GetAvailableCoreWebView2BrowserVersionString(PCWSTR::null(), &mut version) }
            .is_ok()
            && !version.is_null();
    let _ = take_pwstr(version);
    if !hay_motor {
        aviso(&textos, "motor_falta_titulo", "motor_falta");
        unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                w!("https://go.microsoft.com/fwlink/p/?LinkId=2124703"),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            );
        }
        return;
    }

    let carpeta_ui = base.join(format!("interfaz-{}", env!("CARGO_PKG_VERSION")));
    if let Err(e) = crate::interfaz::prepara(&carpeta_ui) {
        registra("interfaz", &e);
        aviso(&textos, "error_arranque_titulo", "error_arranque");
        return;
    }

    let (s, iniciales) = Sesion::abre(Arranque {
        datos: datos.clone(),
        descargas: carpeta_descargas(),
        idioma_sistema: idioma.clone(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        reloj: sesion::ahora_sistema,
    });
    let mut s = s;
    s.pon_depuracion(DEPURA.with(|d| *d));
    s.pon_resolutor(Some(guardiana_zero::servidor::del_sistema));
    // The subscription: seven days of the browser's own, kept in its data folder and, a second
    // time, in a folder outside it (Roaming), so emptying one does not start the week again.
    let ancla = std::env::var_os("GUARDIANA_ZERO_DATOS")
        .map(|_| base.join("marca"))
        .or_else(|| {
            std::env::var_os("APPDATA")
                .map(|a| PathBuf::from(a).join("GUARDIANA ZERO").join("marca"))
        })
        .unwrap_or_else(|| base.join("marca"));
    let lugar = LugarLicencia::nuevo(&datos, ancla);
    let _ = LUGAR.set(lugar.clone());
    escribe_espera(&carpeta_ui, &s.textos());
    let mut iniciales = iniciales;
    let o = match &pedido {
        Some(Pedido::AbreGuardiana(url)) => s.abre_guardiana(url),
        Some(p) => abre_panel(&mut s, p.clave().map(str::to_owned)),
        None => Vec::new(),
    };
    if !o.is_empty() {
        iniciales = o;
    }
    iniciales.extend(s.pon_lugar_licencia(lugar));
    let estricto = s.seguimiento_estricto();
    SESION.with(|c| *c.borrow_mut() = Some(s));

    let col = colores();
    let h = match crea_ventana(&col) {
        Ok(h) => h,
        Err(e) => {
            registra("ventana", &e);
            aviso(&textos, "error_arranque_titulo", "error_arranque");
            return;
        }
    };
    VISTAS.with(|v| {
        *v.borrow_mut() = Some(Vistas {
            hwnd: h,
            entorno: None,
            barra: None,
            panel: None,
            pestanas: BTreeMap::new(),
            activa: 0,
            marco: MARCO_INICIAL,
            ventanas: HashMap::new(),
            carpeta_ui,
            estricto,
            borrar_pendiente: false,
            completa: None,
            recargas: HashMap::new(),
        });
    });
    encola(iniciales);
    // Which relay is nearest, measured once in the background at start: the phone hand-off and
    // the house's room use it too, not only an invitation (17 Oct 2026 review: from Colombia
    // they always went through Germany).
    guardiana_zero::rele::mide_cercania();

    let marco = col.fondo_marco;
    let hoja = col.hoja;
    if let Err(e) = crea_entorno(&base.join("motor"), &idioma, marco, hoja) {
        registra("motor", &e);
        aviso(&textos, "error_arranque_titulo", "error_arranque");
        return;
    }
    unsafe {
        SetTimer(Some(h), TIC, 200, None);
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    // Only our own mark: a newer copy that took over has written its own.
    if std::fs::read_to_string(&marca)
        .is_ok_and(|t| t.split_whitespace().nth(1) == Some(&std::process::id().to_string()))
    {
        let _ = std::fs::remove_file(&marca);
    }
}

/// The file in the data folder that says which version is running and in which process.
const EN_MARCHA: &str = "en-marcha.txt";

/// «1.0.5» as numbers, to compare versions.
fn version_de(texto: &str) -> Option<(u32, u32, u32)> {
    let mut p = texto.trim().split('.').map(|x| x.parse::<u32>().ok());
    Some((p.next()??, p.next()??, p.next()??))
}

/// The version of the running copy that owns `ventana`, from its mark; `None` when there is no
/// mark, or the mark is another process's (a copy up to 1.0.4, or a stale mark after a crash).
fn version_en_marcha(marca: &std::path::Path, ventana: HWND) -> Option<(u32, u32, u32)> {
    let texto = std::fs::read_to_string(marca).ok()?;
    let mut partes = texto.split_whitespace();
    let version = version_de(partes.next()?)?;
    let pid: u32 = partes.next()?.parse().ok()?;
    let mut suyo = 0u32;
    // SAFETY: reads the owner process id of a window handle into a local.
    unsafe { GetWindowThreadProcessId(ventana, Some(std::ptr::addr_of_mut!(suyo))) };
    (suyo == pid).then_some(version)
}

fn crea_ventana(col: &Colores) -> windows::core::Result<HWND> {
    unsafe {
        let instancia: HINSTANCE = GetModuleHandleW(PCWSTR::null())?.into();
        let grande = icono(32);
        let chico = icono(16);
        let clase = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(proc_ventana),
            hInstance: instancia,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: CreateSolidBrush(col.marco),
            lpszClassName: CLASE,
            hIcon: grande.unwrap_or_default(),
            hIconSm: chico.unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassExW(&clase);
        // 85 % of the work area, centred.
        let mut area = RECT::default();
        let _ = SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some((&mut area as *mut RECT).cast::<c_void>()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        let (aw, ah) = (area.right - area.left, area.bottom - area.top);
        let (w, h) = if aw > 0 && ah > 0 {
            (aw * 85 / 100, ah * 88 / 100)
        } else {
            (1280, 820)
        };
        let x = area.left + (aw - w) / 2;
        let y = area.top + (ah - h) / 2;
        let titulo = w!("GUARDIANA ZERO");
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASE,
            titulo,
            WS_OVERLAPPEDWINDOW,
            x,
            y,
            w,
            h,
            None,
            None,
            Some(instancia),
            None,
        )?;
        if let Some(i) = grande {
            SendMessageW(
                hwnd,
                WM_SETICON,
                Some(WPARAM(ICON_BIG as usize)),
                Some(LPARAM(i.0 as isize)),
            );
        }
        if let Some(i) = chico {
            SendMessageW(
                hwnd,
                WM_SETICON,
                Some(WPARAM(ICON_SMALL as usize)),
                Some(LPARAM(i.0 as isize)),
            );
        }
        pinta_marco_ventana(hwnd, col);
        Ok(hwnd)
    }
}

/// The title bar takes the colour of the browser's frame (Windows 11) and its day or night.
fn pinta_marco_ventana(hwnd: HWND, col: &Colores) {
    // SAFETY: plain attribute calls on our own window with pointers to locals that outlive them.
    unsafe {
        let oscuro = BOOL::from(col.oscuro);
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&oscuro as *const BOOL).cast::<c_void>(),
            std::mem::size_of::<BOOL>() as u32,
        );
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_CAPTION_COLOR,
            (&col.marco as *const COLORREF).cast::<c_void>(),
            std::mem::size_of::<COLORREF>() as u32,
        );
        // 1.0.46, the name once: the caption's words take the caption's own colour, so the
        // window's name is read in GUARDIANA's top row and in the taskbar, not twice on screen.
        let texto = col.marco;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_TEXT_COLOR,
            (&texto as *const COLORREF).cast::<c_void>(),
            std::mem::size_of::<COLORREF>() as u32,
        );
    }
}

fn crea_entorno(
    carpeta: &Path,
    idioma: &str,
    marco: COREWEBVIEW2_COLOR,
    hoja: COREWEBVIEW2_COLOR,
) -> windows::core::Result<()> {
    let opciones = CoreWebView2EnvironmentOptions::default();
    let mut argumentos = sesion::ARGUMENTOS_MOTOR.to_string();
    // Only for the automatic tests: a local debugging port and test host names.
    if let Ok(extra) = std::env::var("GUARDIANA_ZERO_ARGS") {
        argumentos.push(' ');
        argumentos.push_str(&extra);
    }
    unsafe {
        opciones.set_additional_browser_arguments(argumentos);
        opciones.set_language(idioma.to_string());
        opciones.set_are_browser_extensions_enabled(false);
        opciones.set_enable_tracking_prevention(true);
        opciones.set_allow_single_sign_on_using_os_primary_account(false);
    }
    let opciones: ICoreWebView2EnvironmentOptions = opciones.into();
    let c = Ancho::de(&carpeta.to_string_lossy());
    let manejador = CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(
        move |r: windows::core::Result<()>, e: Option<ICoreWebView2Environment>| {
            let entorno = match (r, e) {
                (Ok(()), Some(e)) => e,
                (Err(err), _) => {
                    registra("crear motor", &err);
                    return Ok(());
                }
                _ => return Ok(()),
            };
            let version = texto_de(|p| unsafe { entorno.BrowserVersionString(p) });
            con_sesion(|s| s.pon_motor(&version));
            con_vistas(|v| v.entorno = Some(entorno));
            crea_propia(Origen::Barra, marco);
            crea_propia(Origen::Panel, hoja);
            drena();
            Ok(())
        },
    ));
    unsafe {
        CreateCoreWebView2EnvironmentWithOptions(PCWSTR::null(), c.p(), &opciones, &manejador)
    }
}
