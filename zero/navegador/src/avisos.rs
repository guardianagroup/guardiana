//! Windows' notices of el escudo's grave warnings (1.0.52; the owner, 1 Nov 2026, 19:29: «Me parece bien la propuesta,
//! sí, buena idea»): `GUARDIANA ZERO.exe --avisos`, started when the person signs in (the installer's Run entry) and by
//! every start of ZERO, with no window and one per person. What to show is decided by `guardiana_zero::avisos`, tested
//! on every system; this only shows it, under GUARDIANA's name and mark.

use std::cell::RefCell;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use guardiana_zero::avisos::{self, AUMID};
use windows::core::{w, HSTRING, PCWSTR};
use windows::Data::Xml::Dom::XmlDocument;
use windows::Win32::Foundation::{
    GetLastError, ERROR_ALREADY_EXISTS, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM,
};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, PostQuitMessage,
    RegisterClassExW, SetTimer, TranslateMessage, MSG, WINDOW_EX_STYLE, WM_CLOSE, WM_ENDSESSION,
    WM_QUERYENDSESSION, WM_TIMER, WNDCLASSEXW, WS_OVERLAPPED,
};
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};

/// GUARDIANA's mark, for the notices' header.
const ICONO: &[u8] = include_bytes!("../../../build/wix/guardiana.ico");
/// `CREATE_NO_WINDOW`: the registry tool runs without a console flashing.
const SIN_VENTANA: u32 = 0x0800_0000;
/// The hidden window's class.
const CLASE: PCWSTR = w!("GuardianaAvisos");

fn ahora_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(0))
        .unwrap_or(0)
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

/// The hidden window's messages: a look at each tick; closing (the installer, before it replaces this program) or the
/// end of the session ends the loop.
extern "system" fn proc_avisos(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_TIMER => {
            mira();
            LRESULT(0)
        }
        WM_QUERYENDSESSION => LRESULT(1),
        WM_CLOSE | WM_ENDSESSION => {
            // SAFETY: posts the quit message to this thread's own queue.
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        // SAFETY: the default handling of a message of this thread's own window, with its own arguments.
        _ => unsafe { DefWindowProcW(h, msg, wp, lp) },
    }
}

/// The notices' loop, until the person signs out or the installer closes it. A hidden window carries it, so the
/// installer's «close GUARDIANA ZERO» reaches this copy too.
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
    mira();
    let cada = u32::try_from(avisos::CADA.as_millis()).unwrap_or(4_000);
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
        SetTimer(Some(h), 1, cada, None);
        let mut m = MSG::default();
        while GetMessageW(&mut m, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&m);
            DispatchMessageW(&m);
        }
    }
}
