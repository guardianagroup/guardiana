//! Windows' notices of el escudo's grave warnings (1.0.52; the owner, 1 Nov 2026, 19:29: «Me parece bien la propuesta,
//! sí, buena idea»): `GUARDIANA ZERO.exe --avisos`, started when the person signs in (the installer's Run entry) and by
//! every start of ZERO, with no window and one per person. What to show is decided by `guardiana_zero::avisos`, tested
//! on every system; this only shows it, under GUARDIANA's name and mark.
//!
//! 5 Nov 2026 (the owner, 3 Nov, on the icon and the notices of the VPN: «Hazlo claro»): the same process carries the
//! VPN's icon next to Windows' clock, so it is there from the sign-in and with ZERO closed too. What it says, its menu
//! and its notices are decided by `guardiana_zero::bandeja`; this asks GUARDIANA's service on its own threads (never on
//! the window's, so the icon always answers), draws the icon with `Shell_NotifyIconW`, shows its menu and opens ZERO.

use std::cell::RefCell;
use std::ffi::c_void;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use guardiana_zero::avisos::{self, AUMID};
use guardiana_zero::bandeja::{self, Color, Memoria, Orden, Respuesta, Vpn};
use guardiana_zero::lanzador;
use guardiana_zero::textos::Textos;
use windows::core::{w, HRESULT, HSTRING, PCWSTR};
use windows::Data::Xml::Dom::XmlDocument;
use windows::Win32::Foundation::{
    GetLastError, ERROR_ALREADY_EXISTS, ERROR_INSUFFICIENT_BUFFER, ERROR_SERVICE_DOES_NOT_EXIST,
    HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM,
};
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, TCP_TABLE_OWNER_PID_LISTENER,
};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Services::{
    CloseServiceHandle, OpenSCManagerW, OpenServiceW, QueryServiceStatusEx, SC_MANAGER_CONNECT,
    SC_STATUS_PROCESS_INFO, SERVICE_QUERY_STATUS,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE,
    NIM_MODIFY, NIM_SETVERSION, NINF_KEY, NIN_SELECT, NOTIFYICONDATAW, NOTIFYICON_VERSION_4,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIconFromResourceEx, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
    DestroyIcon, DestroyMenu, DispatchMessageW, GetCursorPos, GetMessageW, GetSystemMetrics,
    KillTimer, PostMessageW, PostQuitMessage, RegisterClassExW, RegisterWindowMessageW,
    SetForegroundWindow, SetMenuDefaultItem, SetTimer, TrackPopupMenu, TranslateMessage, HICON,
    LR_DEFAULTCOLOR, MF_GRAYED, MF_SEPARATOR, MF_STRING, MSG, SM_CXSMICON, TPM_NONOTIFY,
    TPM_RETURNCMD, TPM_RIGHTBUTTON, WINDOW_EX_STYLE, WM_APP, WM_CLOSE, WM_CONTEXTMENU,
    WM_ENDSESSION, WM_LBUTTONUP, WM_NULL, WM_QUERYENDSESSION, WM_RBUTTONUP, WM_TIMER, WNDCLASSEXW,
    WS_OVERLAPPED,
};
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};

/// GUARDIANA's mark, for the notices' header.
const ICONO: &[u8] = include_bytes!("../../../build/wix/guardiana.ico");
/// `CREATE_NO_WINDOW`: the registry tool runs without a console flashing.
const SIN_VENTANA: u32 = 0x0800_0000;
/// The hidden window's class.
const CLASE: PCWSTR = w!("GuardianaAvisos");

/// The VPN's four icons: GUARDIANA's mark with a dot in the VPN's colour (build/wix/iconos_vpn.py).
const VPN_VERDE: &[u8] = include_bytes!("../../../build/wix/vpn-verde.ico");
const VPN_AMBAR: &[u8] = include_bytes!("../../../build/wix/vpn-ambar.ico");
const VPN_ROJO: &[u8] = include_bytes!("../../../build/wix/vpn-rojo.ico");
const VPN_GRIS: &[u8] = include_bytes!("../../../build/wix/vpn-gris.ico");
/// The icon's own messages: the mouse and the keyboard on it.
const WM_ICONO: u32 = WM_APP + 1;
/// A reading of the VPN, or an order of its menu, finished on its own thread.
const WM_VPN: u32 = WM_APP + 2;
/// The icon's number for Windows: there is one.
const ID_ICONO: u32 = 1;
/// The timers: el escudo's warnings, and the VPN.
const RELOJ_AVISOS: usize = 1;
const RELOJ_VPN: usize = 2;
/// `NIN_KEYSELECT`: the icon chosen with the keyboard.
const NIN_KEYSELECT: u32 = NIN_SELECT | NINF_KEY;

fn ahora_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(0))
        .unwrap_or(0)
}

/// The automatic tests (zero/prueba/prueba.js) run with their own data folder: on a computer without GUARDIANA's service
/// their own panel on the port is taken as it (`bandeja::llave_al_puerto`; never where the service exists), and the
/// icon writes what it shows to `bandeja.txt` for them to read.
fn en_prueba() -> bool {
    std::env::var_os("GUARDIANA_ZERO_DATOS").is_some()
}

/// The person's own data folder of ZERO (the same one the window uses).
fn carpeta_datos() -> PathBuf {
    std::env::var_os("GUARDIANA_ZERO_DATOS")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("LOCALAPPDATA").map(|l| PathBuf::from(l).join("GUARDIANA ZERO"))
        })
        .unwrap_or_else(|| PathBuf::from("GUARDIANA ZERO"))
        .join("datos")
}

/// The name and the mark Windows shows the notices under, for this person's account
/// (`HKCU\Software\Classes\AppUserModelId\…`), written with Windows' own registry tool.
fn registra(icono: &Path) {
    let clave = format!(r"HKCU\Software\Classes\AppUserModelId\{AUMID}");
    let reg = std::env::var_os("SystemRoot")
        .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from)
        .join("System32")
        .join("reg.exe");
    let icono = icono.to_string_lossy().into_owned();
    for (nombre, valor) in [("DisplayName", "GUARDIANA"), ("IconUri", icono.as_str())] {
        let _ = Command::new(&reg)
            .args([
                "add", &clave, "/v", nombre, "/t", "REG_SZ", "/d", valor, "/f",
            ])
            .creation_flags(SIN_VENTANA)
            .status();
    }
}

/// One notice on the screen.
fn muestra(xml: &str) -> windows::core::Result<()> {
    let doc = XmlDocument::new()?;
    doc.LoadXml(&HSTRING::from(xml))?;
    let aviso = ToastNotification::CreateToastNotification(&doc)?;
    ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(AUMID))?.Show(&aviso)
}

/// What the loop keeps between looks.
struct Estado {
    archivo: PathBuf,
    ruta_visto: PathBuf,
    visto: u64,
    cambiado: Option<SystemTime>,
}

thread_local! {
    static ESTADO: RefCell<Option<Estado>> = const { RefCell::new(None) };
    static BANDEJA: RefCell<Option<Bandeja>> = const { RefCell::new(None) };
}

/// One look at the warnings' file: a notice for each new grave warning, when the file changed.
fn mira() {
    ESTADO.with(|e| {
        let Ok(mut e) = e.try_borrow_mut() else {
            return;
        };
        let Some(e) = e.as_mut() else {
            return;
        };
        let cambiado = std::fs::metadata(&e.archivo)
            .and_then(|m| m.modified())
            .ok();
        if cambiado.is_none() || cambiado == e.cambiado {
            return;
        }
        let Ok(texto) = std::fs::read_to_string(&e.archivo) else {
            return;
        };
        e.cambiado = cambiado;
        let (graves, marca) = avisos::nuevos(&texto, e.visto);
        for xml in avisos::notificaciones(&graves) {
            let _ = muestra(&xml);
        }
        if marca != e.visto {
            e.visto = marca;
            let _ = std::fs::write(&e.ruta_visto, e.visto.to_string());
        }
    });
}

// --- the VPN next to the clock (5 Nov 2026) ------------------------------------------------------------------------------

/// What the icon keeps on the window's thread.
struct Bandeja {
    /// ZERO's data folder: the language the person chose, and the tests' `bandeja.txt`.
    datos: PathBuf,
    /// The last reading and the last order of the menu.
    memoria: Memoria,
    /// Windows has the icon.
    puesto: bool,
    /// Windows took the icon's fourth version (`NIN_SELECT` for a click, `WM_CONTEXTMENU` for the menu).
    version4: bool,
    /// What it shows: so it is only drawn again when it changes.
    color: Option<Color>,
    linea: String,
    /// The image it shows (destroyed when another takes its place).
    icono: Option<HICON>,
}

/// The last reading of the VPN, from its thread to the window's.
static LEIDA: Mutex<Option<Vpn>> = Mutex::new(None);
/// A reading is under way.
static LEYENDO: AtomicBool = AtomicBool::new(false);
/// The order of the menu under way (its number; 0, none).
static ORDENANDO: AtomicU32 = AtomicU32::new(0);
/// The last order of the menu that finished, and what the service answered.
static HECHA: Mutex<Option<(Orden, Respuesta)>> = Mutex::new(None);
/// The message Windows sends every window when the taskbar is created again (Explorer restarted): `TaskbarCreated`.
static BARRA_NUEVA: AtomicU32 = AtomicU32::new(0);
/// The process is closing (the installer, or the end of the session): nothing puts the icon back any more.
static CERRANDO: AtomicBool = AtomicBool::new(false);

/// The icon's texts: the language the person chose in ZERO, else the one ZERO starts in.
fn textos(datos: &Path) -> Textos {
    Textos::de(bandeja::idioma(datos, &crate::app::idioma_sistema()))
}

/// A string for Windows, ending in NUL.
fn ancho(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// GUARDIANA's service as Windows' service manager says it: whether Windows answered that it is not installed (only
/// that answer: a failure to ask is not taken for it), and the process it runs in, while it runs.
fn servicio() -> (bool, Option<u32>) {
    let nombre = ancho(bandeja::SERVICIO);
    // SAFETY: opens Windows' service manager and GUARDIANA's service only to read its state into a buffer of ours that
    // lives through the call, and closes both handles.
    unsafe {
        let Ok(gestor) = OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT) else {
            return (false, None);
        };
        let r = match OpenServiceW(gestor, PCWSTR(nombre.as_ptr()), SERVICE_QUERY_STATUS) {
            Ok(s) => {
                let mut estado = [0u8; 64];
                let mut hace_falta = 0u32;
                let leido = QueryServiceStatusEx(
                    s,
                    SC_STATUS_PROCESS_INFO,
                    Some(&mut estado[..]),
                    &mut hace_falta,
                );
                let _ = CloseServiceHandle(s);
                let pid = if leido.is_ok() {
                    bandeja::pid_del_servicio(&estado)
                } else {
                    None
                };
                (false, pid)
            }
            Err(e) => (
                e.code() == HRESULT::from_win32(ERROR_SERVICE_DOES_NOT_EXIST.0),
                None,
            ),
        };
        let _ = CloseServiceHandle(gestor);
        r
    }
}

/// The process that listens on 127.0.0.1 at the panel's port, without doubt (`bandeja::pid_que_escucha`), from Windows'
/// table of listening sockets.
fn pid_del_puerto() -> Option<u32> {
    let mut tam: u32 = 0;
    // SAFETY: with no buffer, Windows only says in `tam` how much it needs.
    unsafe {
        let _ = GetExtendedTcpTable(None, &mut tam, false, 2, TCP_TABLE_OWNER_PID_LISTENER, 0);
    }
    // The table can grow between one call and the next: three tries, with room.
    for _ in 0..3 {
        let palabras = usize::try_from(tam).unwrap_or(0) / 4 + 1 + 6 * 64;
        let mut buf = vec![0u32; palabras];
        let mut largo = u32::try_from(palabras * 4).unwrap_or(u32::MAX);
        // SAFETY: `buf` is ours, aligned to 4 bytes, and `largo` says its size in bytes.
        let r = unsafe {
            GetExtendedTcpTable(
                Some(buf.as_mut_ptr().cast::<c_void>()),
                &mut largo,
                false,
                2,
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        };
        if r == 0 {
            return bandeja::pid_que_escucha(&buf, [127, 0, 0, 1], lanzador::PUERTO_PANEL);
        }
        if r != ERROR_INSUFFICIENT_BUFFER.0 {
            return None;
        }
        tam = largo;
    }
    None
}

/// Whether the panel's key may go to the one listening on the panel's port: only GUARDIANA's service (the CLI's rule);
/// the automatic tests' own panel only on a computer where the service does not exist (`bandeja::llave_al_puerto`).
fn al_servicio() -> bool {
    let (sin_servicio, pid) = servicio();
    bandeja::llave_al_puerto(en_prueba(), sin_servicio, pid, pid_del_puerto())
}

/// The VPN as the service says it now; unknown when the one listening is not the service or the key cannot be read.
fn lectura() -> Vpn {
    if !al_servicio() {
        return Vpn::default();
    }
    bandeja::lee(lanzador::lee_llave(&lanzador::ruta_llave()).as_deref())
}

/// A message to this process's own hidden window from another thread (its handle travels as a number).
fn avisa_ventana(ventana: isize) {
    // SAFETY: posts a message to the hidden window of this process; once it is gone, the call fails and nothing happens.
    unsafe {
        let _ = PostMessageW(
            Some(HWND(ventana as *mut c_void)),
            WM_VPN,
            WPARAM(0),
            LPARAM(0),
        );
    }
}

/// A reading of the VPN on its own thread, when none is under way; the window hears of it with [`WM_VPN`].
fn lee_vpn(h: HWND) {
    if LEYENDO.swap(true, Ordering::SeqCst) {
        return;
    }
    let ventana = h.0 as isize;
    let lanzado = std::thread::Builder::new()
        .name("vpn".into())
        .spawn(move || {
            let v = lectura();
            *LEIDA.lock().unwrap_or_else(PoisonError::into_inner) = Some(v);
            LEYENDO.store(false, Ordering::SeqCst);
            avisa_ventana(ventana);
        });
    if lanzado.is_err() {
        LEYENDO.store(false, Ordering::SeqCst);
    }
}

/// «Conectar» or «Desconectar» from the menu, on its own thread (closing the tunnel can take seconds), one at a time,
/// with the icon's language (`idioma`) for the words of the service's answer.
fn ordena(h: HWND, o: Orden, idioma: &'static str) {
    if CERRANDO.load(Ordering::SeqCst) {
        return;
    }
    if ORDENANDO
        .compare_exchange(0, o.id(), Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    con_bandeja(|b| b.memoria.ordenada(o, Instant::now()));
    let ventana = h.0 as isize;
    let lanzado = std::thread::Builder::new()
        .name("vpn-orden".into())
        .spawn(move || {
            let llave = al_servicio()
                .then(|| lanzador::lee_llave(&lanzador::ruta_llave()))
                .flatten();
            let r = llave.map_or(Respuesta::No, |llave| bandeja::ordena(o, &llave, idioma));
            *HECHA.lock().unwrap_or_else(PoisonError::into_inner) = Some((o, r));
            ORDENANDO.store(0, Ordering::SeqCst);
            avisa_ventana(ventana);
        });
    if lanzado.is_err() {
        ORDENANDO.store(0, Ordering::SeqCst);
    }
}

fn con_bandeja<R>(f: impl FnOnce(&mut Bandeja) -> R) -> Option<R> {
    BANDEJA.with(|b| b.try_borrow_mut().ok()?.as_mut().map(f))
}

/// ZERO itself with `args`: Órbita on the VPN (`--url guardiana://vpn`) or GUARDIANA (`--panel`). A copy already open
/// takes the request.
fn abre_zero(args: &[&str]) {
    if let Ok(yo) = std::env::current_exe() {
        let _ = Command::new(yo).args(args).spawn();
    }
}

/// One of the VPN's icons at the size of the notification area.
fn imagen(color: Color) -> Option<HICON> {
    let ico = match color {
        Color::Verde => VPN_VERDE,
        Color::Ambar => VPN_AMBAR,
        Color::Rojo => VPN_ROJO,
        Color::Gris => VPN_GRIS,
    };
    // SAFETY: reads one of the system's measures.
    let lado = unsafe { GetSystemMetrics(SM_CXSMICON) };
    let lado = if lado > 0 { lado } else { 16 };
    let bits = bandeja::imagen_ico(ico, u32::try_from(lado).unwrap_or(16))?;
    // SAFETY: an icon made from the bytes of one image of our own icon files, which Windows copies.
    unsafe { CreateIconFromResourceEx(bits, true, 0x0003_0000, lado, lado, LR_DEFAULTCOLOR) }.ok()
}

/// The icon's identity for Windows: this window, icon number 1.
fn identidad(h: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: h,
        uID: ID_ICONO,
        ..Default::default()
    }
}

/// The icon as the last reading says it: put when Windows does not have it yet, changed when its colour or its lines
/// changed (`forzar`: again in any case, after Explorer started again).
fn pon_icono(h: HWND, b: &mut Bandeja, t: &Textos, forzar: bool) {
    if CERRANDO.load(Ordering::SeqCst) {
        return;
    }
    let color = bandeja::color(&b.memoria.vpn);
    let linea = bandeja::linea(&b.memoria.vpn, t);
    if b.puesto && !forzar && b.color == Some(color) && b.linea == linea {
        return;
    }
    let nueva = if b.color == Some(color) && b.icono.is_some() {
        None
    } else {
        imagen(color)
    };
    let Some(icono) = nueva.or(b.icono) else {
        return;
    };
    let mut d = identidad(h);
    d.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;
    d.uCallbackMessage = WM_ICONO;
    d.hIcon = icono;
    for (hueco, c) in d
        .szTip
        .iter_mut()
        .zip(linea.encode_utf16().take(bandeja::LARGO_LINEA))
    {
        *hueco = c;
    }
    // SAFETY: hands Windows the icon of this process's own window, with a structure that lives through the calls.
    let puesto = unsafe {
        if b.puesto && Shell_NotifyIconW(NIM_MODIFY, &d).as_bool() {
            true
        } else if Shell_NotifyIconW(NIM_ADD, &d).as_bool() {
            d.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            b.version4 = Shell_NotifyIconW(NIM_SETVERSION, &d).as_bool();
            true
        } else {
            // Windows already had it (the taskbar came back without its message): changed instead
            Shell_NotifyIconW(NIM_MODIFY, &d).as_bool()
        }
    };
    if nueva.is_some() {
        if let Some(vieja) = b.icono.replace(icono) {
            // SAFETY: an icon this process made, which Windows no longer shows.
            unsafe {
                let _ = DestroyIcon(vieja);
            }
        }
    }
    b.puesto = puesto;
    b.color = Some(color);
    if en_prueba() {
        let _ = std::fs::write(
            b.datos.join("bandeja.txt"),
            format!("{} {}\n{linea}", color.nombre(), u8::from(puesto)),
        );
    }
    b.linea = linea;
}

/// Before the process ends: its timers stopped, the icon out of the notification area whatever this process believes
/// of it (Windows only needs the window and the icon's number), and the icon's state emptied. A reading, an order or a
/// `TaskbarCreated` that arrives after this (the quit message comes only once the queue is empty) finds nothing to draw
/// and does not put the icon back (review of bandeja-vpn-2, finding 2).
fn cierra(h: HWND) {
    CERRANDO.store(true, Ordering::SeqCst);
    // SAFETY: stops the two timers of this thread's own window.
    unsafe {
        let _ = KillTimer(Some(h), RELOJ_AVISOS);
        let _ = KillTimer(Some(h), RELOJ_VPN);
    }
    let d = identidad(h);
    // SAFETY: takes away the icon of this process's own window; when Windows does not have it, nothing happens.
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &d);
    }
    let fuera = BANDEJA.with(|b| b.try_borrow_mut().ok().and_then(|mut b| b.take()));
    if let Some(icono) = fuera.and_then(|b| b.icono) {
        // SAFETY: an icon this process made, no longer shown.
        unsafe {
            let _ = DestroyIcon(icono);
        }
    }
}

/// A reading or an order finished: the notice it deserves, and the icon as it is now.
fn al_leer(h: HWND) {
    if CERRANDO.load(Ordering::SeqCst) {
        return;
    }
    // taken out only once the icon's state is at hand, so neither is lost
    let ordenada = con_bandeja(|b| {
        let nueva = LEIDA.lock().unwrap_or_else(PoisonError::into_inner).take();
        let hecha = HECHA.lock().unwrap_or_else(PoisonError::into_inner).take();
        let t = textos(&b.datos);
        if let Some((o, r)) = &hecha {
            if let Some(a) = bandeja::aviso_de_orden(*o, r, &b.memoria.vpn, &t) {
                let _ = muestra(&bandeja::notificacion(&a, &t));
            }
        }
        if let Some(v) = nueva {
            if let Some(a) = b.memoria.lectura(v, Instant::now(), &t) {
                let _ = muestra(&bandeja::notificacion(&a, &t));
            }
        }
        pon_icono(h, b, &t, false);
        hecha.is_some()
    });
    // what the order did, at once
    if ordenada == Some(true) {
        lee_vpn(h);
    }
}

/// The menu at the pointer, and what was chosen in it. Nothing of the icon's state stays borrowed while it is open:
/// Windows runs the menu's own loop, and the readings go on meanwhile.
fn menu(h: HWND) {
    let Some((vpn, t)) = con_bandeja(|b| (b.memoria.vpn.clone(), textos(&b.datos))) else {
        return;
    };
    let desconectando = ORDENANDO.load(Ordering::SeqCst) == Orden::Desconectar.id();
    let entradas = bandeja::menu(&vpn, desconectando, &t);
    let anchos: Vec<Vec<u16>> = entradas.iter().map(|e| ancho(&e.texto)).collect();
    let mut punto = POINT::default();
    // SAFETY: a menu of this thread, its texts alive until it is destroyed; shown at the pointer for this process's own
    // window, which comes to the front first so the menu closes when the person clicks elsewhere (and the empty message
    // after it is what Windows asks for that).
    let elegida = unsafe {
        let Ok(m) = CreatePopupMenu() else {
            return;
        };
        for (e, texto) in entradas.iter().zip(&anchos) {
            if e.raya {
                let _ = AppendMenuW(m, MF_SEPARATOR, 0, PCWSTR::null());
            }
            let marcas = if e.activa {
                MF_STRING
            } else {
                MF_STRING | MF_GRAYED
            };
            let _ = AppendMenuW(m, marcas, e.orden.id() as usize, PCWSTR(texto.as_ptr()));
        }
        let _ = SetMenuDefaultItem(m, Orden::AbrirVpn.id(), 0);
        let _ = GetCursorPos(&mut punto);
        let _ = SetForegroundWindow(h);
        let r = TrackPopupMenu(
            m,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY,
            punto.x,
            punto.y,
            None,
            h,
            None,
        );
        let _ = PostMessageW(Some(h), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(m);
        Orden::de_id(u32::try_from(r.0).unwrap_or(0))
    };
    // closed while the menu was open (its own loop handed the closing on): nothing chosen is done
    if CERRANDO.load(Ordering::SeqCst) {
        return;
    }
    match elegida {
        Some(Orden::AbrirVpn) => abre_zero(&["--url", bandeja::ENLACE]),
        Some(Orden::AbrirGuardiana) => abre_zero(&["--panel"]),
        Some(o @ (Orden::Conectar | Orden::Desconectar)) => ordena(h, o, t.idioma().codigo()),
        _ => {}
    }
}

/// The mouse or the keyboard on the icon: a click opens Órbita on the VPN, the right button (or the menu key) its menu.
fn en_el_icono(h: HWND, lp: LPARAM) {
    // the event, in the low word (the version 4 of the icon puts the icon's number in the high one)
    let evento = (lp.0 as u32) & 0xFFFF;
    let version4 = con_bandeja(|b| b.version4).unwrap_or(false);
    let (abrir, menu_) = if version4 {
        (
            evento == NIN_SELECT || evento == NIN_KEYSELECT,
            evento == WM_CONTEXTMENU,
        )
    } else {
        (evento == WM_LBUTTONUP, evento == WM_RBUTTONUP)
    };
    if abrir {
        abre_zero(&["--url", bandeja::ENLACE]);
    } else if menu_ {
        menu(h);
    }
}

/// The hidden window's messages: el escudo's look and the VPN's reading at each tick; the icon's mouse; closing (the
/// installer, before it replaces this program) or the end of the session takes the icon away and ends the loop.
extern "system" fn proc_avisos(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_TIMER => {
            if wp.0 == RELOJ_VPN {
                lee_vpn(h);
            } else {
                mira();
            }
            LRESULT(0)
        }
        WM_VPN => {
            al_leer(h);
            LRESULT(0)
        }
        WM_ICONO => {
            en_el_icono(h, lp);
            LRESULT(0)
        }
        WM_QUERYENDSESSION => LRESULT(1),
        // A wParam of 0: the end of the session was called off (another program said no) and the session goes on, so
        // do the notices and the icon. The installers close this process with WM_CLOSE (taskkill without /F), and
        // Windows' Restart Manager with a WM_ENDSESSION of wParam 1.
        WM_ENDSESSION if wp.0 == 0 => LRESULT(0),
        WM_CLOSE | WM_ENDSESSION => {
            cierra(h);
            // SAFETY: posts the quit message to this thread's own queue.
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        m if m != 0 && m == BARRA_NUEVA.load(Ordering::SeqCst) => {
            // Explorer started again (it stopped, or was restarted): the icon is put back, unless the process is closing.
            con_bandeja(|b| {
                b.puesto = false;
                let t = textos(&b.datos);
                pon_icono(h, b, &t, true);
            });
            LRESULT(0)
        }
        // SAFETY: the default handling of a message of this thread's own window, with its own arguments.
        _ => unsafe { DefWindowProcW(h, msg, wp, lp) },
    }
}

/// The notices' loop and the VPN's icon, until the person signs out or the installer closes it. A hidden window carries
/// them, so the installer's «close GUARDIANA ZERO» reaches this copy too.
pub fn corre() {
    // SAFETY: initialises COM and the Windows Runtime for this thread, once, before any notice is built.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
    let nombre: Vec<u16> = "Local\\GuardianaAvisos"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: creates or opens a named mutex from a NUL-terminated name that outlives the call. The handle stays open
    // for the life of the process: that is what marks this copy as running.
    let (mutex, ya) = unsafe {
        let m = CreateMutexW(None, true, PCWSTR(nombre.as_ptr()));
        (m, GetLastError() == ERROR_ALREADY_EXISTS)
    };
    if mutex.is_err() || ya {
        return;
    }
    let datos = carpeta_datos();
    let _ = std::fs::create_dir_all(&datos);
    let icono = datos.join("guardiana.ico");
    if std::fs::metadata(&icono).map(|m| m.len()).ok() != u64::try_from(ICONO.len()).ok() {
        let _ = std::fs::write(&icono, ICONO);
    }
    registra(&icono);
    let ruta_visto = datos.join("avisos-visto.txt");
    let visto = std::fs::read_to_string(&ruta_visto)
        .ok()
        .and_then(|t| t.trim().parse::<u64>().ok())
        .unwrap_or_else(|| ahora_ms().saturating_sub(avisos::ATRAS_MS));
    ESTADO.with(|e| {
        *e.borrow_mut() = Some(Estado {
            archivo: avisos::ruta(),
            ruta_visto,
            visto,
            cambiado: None,
        });
    });
    BANDEJA.with(|b| {
        *b.borrow_mut() = Some(Bandeja {
            datos,
            memoria: Memoria::default(),
            puesto: false,
            version4: false,
            color: None,
            linea: String::new(),
            icono: None,
        });
    });
    mira();
    let cada = u32::try_from(avisos::CADA.as_millis()).unwrap_or(4_000);
    let cada_vpn = u32::try_from(bandeja::CADA.as_millis()).unwrap_or(3_000);
    // SAFETY: registers a window class and creates one hidden window of this thread, with static class and title
    // strings; then runs this thread's message loop until the quit message.
    unsafe {
        let Ok(modulo) = GetModuleHandleW(PCWSTR::null()) else {
            return;
        };
        let instancia: HINSTANCE = modulo.into();
        let clase = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(proc_avisos),
            hInstance: instancia,
            lpszClassName: CLASE,
            ..Default::default()
        };
        RegisterClassExW(&clase);
        BARRA_NUEVA.store(
            RegisterWindowMessageW(w!("TaskbarCreated")),
            Ordering::SeqCst,
        );
        let Ok(h) = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASE,
            w!("GUARDIANA avisos"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instancia),
            None,
        ) else {
            return;
        };
        SetTimer(Some(h), RELOJ_AVISOS, cada, None);
        SetTimer(Some(h), RELOJ_VPN, cada_vpn, None);
        // the icon appears with the first reading, already in its colour
        lee_vpn(h);
        let mut m = MSG::default();
        while GetMessageW(&mut m, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&m);
            DispatchMessageW(&m);
        }
    }
}
