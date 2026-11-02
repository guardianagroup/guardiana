//! La mitad de Windows del vigía: leer, nada más.
//!
//! `unsafe` está prohibido en este proyecto salvo justificación, y esta es la justificación: son
//! funciones del propio Windows, de solo lectura, sin dependencias nuevas (`windows-sys` ya
//! estaba), y un PowerShell cada pocos segundos costaría más que todo lo demás. Cada lista que
//! Windows entrega se lee dentro de lo que dice que entregó y se libera con su función; cada asa
//! se cierra; cada búfer va con su tamaño. Si algo falla, se lee como «no se sabe», nunca como
//! «no hay».
//!
//! El registro se lee con las mismas funciones que la vigilancia constante
//! (`vigilancia::win`): mismas claves, mismas cuentas.

#![allow(unsafe_code)]

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_INSUFFICIENT_BUFFER, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, TCP_TABLE_OWNER_PID_CONNECTIONS,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Registry::{HKEY, HKEY_LOCAL_MACHINE, HKEY_USERS};
use windows_sys::Win32::System::RemoteDesktop::{
    ProcessIdToSessionId, WTSClientName, WTSClientProtocolType, WTSDomainName,
    WTSEnumerateSessionsW, WTSFreeMemory, WTSGetActiveConsoleSessionId,
    WTSQuerySessionInformationW, WTSUserName, WTS_CURRENT_SERVER_HANDLE, WTS_INFO_CLASS,
    WTS_SESSION_INFOW,
};

use super::{de_registro, es_vigilado, Proceso, Sesion, Uso, CAPACIDADES};
use crate::vigilancia::win::{abre, cuentas};
use crate::Error;

/// `AF_INET`: solo IPv4, que es por donde escucha el panel (`127.0.0.1`).
const AF_INET: u32 = 2;

/// Los usos del micrófono, la cámara y la captura de pantalla que Windows anotó, de cada cuenta
/// con sesión abierta (su registro está cargado bajo `HKEY_USERS`) y del equipo.
pub(super) fn usos() -> Vec<Uso> {
    let mut v = Vec::new();
    let mut raices: Vec<(HKEY, String, String)> = cuentas()
        .into_iter()
        .map(|sid| (HKEY_USERS, format!("{sid}\\Software"), sid))
        .collect();
    raices.push((
        HKEY_LOCAL_MACHINE,
        "SOFTWARE".to_owned(),
        "equipo".to_owned(),
    ));
    for (raiz, prefijo, cuenta) in &raices {
        for (cap, tipo) in CAPACIDADES {
            let base = format!(
                "{prefijo}\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\{cap}"
            );
            let Some(k) = abre(*raiz, &base) else {
                continue;
            };
            for sub in k.subclaves() {
                if sub.eq_ignore_ascii_case("NonPackaged") {
                    let np = format!("{base}\\NonPackaged");
                    let Some(knp) = abre(*raiz, &np) else {
                        continue;
                    };
                    for app in knp.subclaves() {
                        if let Some(ka) = abre(*raiz, &format!("{np}\\{app}")) {
                            let (i, f) =
                                (ka.qword("LastUsedTimeStart"), ka.qword("LastUsedTimeStop"));
                            v.extend(de_registro(&app, tipo, i, f, cuenta));
                        }
                    }
                } else if let Some(ka) = abre(*raiz, &format!("{base}\\{sub}")) {
                    let (i, f) = (ka.qword("LastUsedTimeStart"), ka.qword("LastUsedTimeStop"));
                    v.extend(de_registro(&sub, tipo, i, f, cuenta));
                }
            }
        }
    }
    v
}

/// Los procesos en marcha. La sesión se pide solo para los que el vigía mira por nombre (control
/// remoto, grabadores): es lo que dice si corren solo como servicio.
pub(super) fn procesos() -> Option<Vec<Proceso>> {
    // SAFETY: una foto de los procesos del sistema; el asa se comprueba y se cierra abajo.
    let foto = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if foto == INVALID_HANDLE_VALUE || foto.is_null() {
        return None;
    }
    let mut out = Vec::new();
    let mut e = PROCESSENTRY32W {
        dwSize: u32::try_from(std::mem::size_of::<PROCESSENTRY32W>()).unwrap_or(0),
        ..PROCESSENTRY32W::default()
    };
    // SAFETY: `e` lleva su tamaño en dwSize y la foto es válida.
    let mut hay = unsafe { Process32FirstW(foto, &raw mut e) } != 0;
    while hay && out.len() < 50_000 {
        let largo = e
            .szExeFile
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(e.szExeFile.len());
        let nombre = String::from_utf16_lossy(e.szExeFile.get(..largo).unwrap_or(&[]));
        let sesion = if es_vigilado(&nombre) {
            let mut s: u32 = 0;
            // SAFETY: `s` es nuestro; Windows escribe en él un número de sesión.
            let ok = unsafe { ProcessIdToSessionId(e.th32ProcessID, &raw mut s) } != 0;
            ok.then_some(s)
        } else {
            None
        };
        out.push(Proceso {
            pid: e.th32ProcessID,
            nombre,
            sesion,
        });
        // SAFETY: como arriba.
        hay = unsafe { Process32NextW(foto, &raw mut e) } != 0;
    }
    // SAFETY: la foto se abrió aquí y no se usa después.
    unsafe { CloseHandle(foto) };
    Some(out)
}

/// Un texto UTF-16 acabado en cero que entregó Windows, leído como mucho hasta `max` caracteres.
fn de_ancho(p: *const u16, max: usize) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut n = 0;
    // SAFETY: Windows entrega el texto acabado en cero; no se lee más allá del cero ni de `max`.
    while n < max && unsafe { *p.add(n) } != 0 {
        n += 1;
    }
    // SAFETY: los `n` caracteres de antes del cero, que acabamos de recorrer.
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(p, n) })
}

/// Un dato de una sesión, como bytes; `None` si Windows no lo dio.
fn dato_de_sesion(sesion: u32, clase: WTS_INFO_CLASS) -> Option<Vec<u8>> {
    let mut buf: windows_sys::core::PWSTR = std::ptr::null_mut();
    let mut bytes: u32 = 0;
    // SAFETY: `buf` y `bytes` son nuestros; Windows reserva el búfer y dice su tamaño.
    let ok = unsafe {
        WTSQuerySessionInformationW(
            WTS_CURRENT_SERVER_HANDLE,
            sesion,
            clase,
            &raw mut buf,
            &raw mut bytes,
        )
    };
    if ok == 0 || buf.is_null() {
        return None;
    }
    let largo = usize::try_from(bytes).unwrap_or(0).min(4_096);
    // SAFETY: el búfer tiene `bytes` bytes según Windows; se copian como mucho esos.
    let v = unsafe { std::slice::from_raw_parts(buf.cast::<u8>(), largo) }.to_vec();
    // SAFETY: el búfer lo reservó WTSQuerySessionInformationW y se libera una vez, aquí.
    unsafe { WTSFreeMemory(buf.cast()) };
    Some(v)
}

/// Bytes UTF-16 (little endian) acabados en cero, como texto.
fn texto_de(bytes: &[u8]) -> String {
    let unidades: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&c| c != 0)
        .take(256)
        .collect();
    String::from_utf16_lossy(&unidades)
}

/// La sesión de la consola (la pantalla y el teclado del propio equipo) y su cuenta,
/// `sesión:dominio\\cuenta`; vacío cuando no hay nadie en ella. `None` si Windows no lo dijo.
pub(super) fn cuenta_de_consola() -> Option<String> {
    // SAFETY: sin argumentos; Windows dice qué sesión tiene la consola (0xFFFFFFFF: ninguna).
    let sesion = unsafe { WTSGetActiveConsoleSessionId() };
    if sesion == u32::MAX {
        return Some(String::new());
    }
    let usuario = texto_de(&dato_de_sesion(sesion, WTSUserName)?);
    if usuario.is_empty() {
        return Some(String::new());
    }
    let dominio = dato_de_sesion(sesion, WTSDomainName)
        .map(|b| texto_de(&b))
        .unwrap_or_default();
    Some(format!("{sesion}:{dominio}\\{usuario}"))
}

/// Las sesiones de Windows: su estado, su protocolo, su cuenta y desde dónde se conectan.
pub(super) fn sesiones() -> Option<Vec<Sesion>> {
    let mut lista: *mut WTS_SESSION_INFOW = std::ptr::null_mut();
    let mut n: u32 = 0;
    // SAFETY: `lista` y `n` son nuestros; Windows reserva la lista y dice cuántas hay.
    let ok = unsafe {
        WTSEnumerateSessionsW(WTS_CURRENT_SERVER_HANDLE, 0, 1, &raw mut lista, &raw mut n)
    };
    if ok == 0 || lista.is_null() {
        return None;
    }
    let cuantas = usize::try_from(n).unwrap_or(0).min(512);
    // SAFETY: Windows entregó `n` filas en `lista`; se leen como mucho esas, antes de liberarla.
    let filas = unsafe { std::slice::from_raw_parts(lista, cuantas) }.to_vec();
    let mut out = Vec::with_capacity(filas.len());
    for f in &filas {
        let protocolo = dato_de_sesion(f.SessionId, WTSClientProtocolType)
            .and_then(|b| b.get(..2).map(|x| u16::from_le_bytes([x[0], x[1]])))
            .unwrap_or(0);
        out.push(Sesion {
            id: f.SessionId,
            estado: f.State,
            protocolo,
            puesto: de_ancho(f.pWinStationName, 64),
            usuario: dato_de_sesion(f.SessionId, WTSUserName)
                .map(|b| texto_de(&b))
                .unwrap_or_default(),
            cliente: dato_de_sesion(f.SessionId, WTSClientName)
                .map(|b| texto_de(&b))
                .unwrap_or_default(),
        });
    }
    // SAFETY: la lista la reservó WTSEnumerateSessionsW y se libera una vez; los nombres de los
    // puestos ya se copiaron arriba.
    unsafe { WTSFreeMemory(lista.cast()) };
    Some(out)
}

/// El proceso dueño de la conexión TCP que sale de `ip:puerto` (la otra punta de una petición
/// que llegó al panel): el navegador que pregunta.
pub(super) fn duenno(ip: [u8; 4], puerto: u16) -> Option<u32> {
    let mut tam: u32 = 0;
    // SAFETY: sin búfer, Windows solo dice en `tam` cuánto necesita.
    unsafe {
        GetExtendedTcpTable(
            std::ptr::null_mut(),
            &raw mut tam,
            0,
            AF_INET,
            TCP_TABLE_OWNER_PID_CONNECTIONS,
            0,
        )
    };
    // La tabla puede crecer entre una llamada y otra: tres intentos, con margen.
    for _ in 0..3 {
        let palabras = usize::try_from(tam).unwrap_or(0) / 4 + 1 + 6 * 64;
        let mut buf = vec![0u32; palabras];
        let mut largo = u32::try_from(palabras * 4).unwrap_or(u32::MAX);
        // SAFETY: `buf` es nuestro, alineado a 4 bytes, y `largo` dice su tamaño en bytes.
        let r = unsafe {
            GetExtendedTcpTable(
                buf.as_mut_ptr().cast(),
                &raw mut largo,
                0,
                AF_INET,
                TCP_TABLE_OWNER_PID_CONNECTIONS,
                0,
            )
        };
        if r == 0 {
            return super::pid_de_tabla(&buf, ip, puerto);
        }
        if r != ERROR_INSUFFICIENT_BUFFER {
            return None;
        }
        tam = largo;
    }
    None
}

/// La mirada lenta: el guion `defensa.ps1` en un PowerShell de solo lectura, con prioridad baja y
/// sin ventana, que se para si tarda más de un minuto. Devuelve lo que imprimió.
pub(super) fn defensa() -> Result<String, Error> {
    use std::io::{Read as _, Write as _};
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
    const LIMITE: std::time::Duration = std::time::Duration::from_secs(60);
    let raiz = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_owned());
    // Un GUARDIANA de 32 bits en un Windows de 64 recibiría el PowerShell de 32: «Sysnative».
    let carpeta = if std::env::var_os("PROCESSOR_ARCHITEW6432").is_some() {
        "Sysnative"
    } else {
        "System32"
    };
    let exe = format!("{raiz}\\{carpeta}\\WindowsPowerShell\\v1.0\\powershell.exe");
    let mut hijo = Command::new(exe)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "& ([scriptblock]::Create([Console]::In.ReadToEnd()))",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS)
        .spawn()
        .map_err(|e| Error::Sistema(e.to_string()))?;
    // Trabajo de fondo: también en modo de eficiencia y con la memoria en prioridad baja (1.0.55).
    let _ = crate::vigilancia::fondo::de_fondo(&hijo);
    if let Some(mut entrada) = hijo.stdin.take() {
        entrada
            .write_all(super::SCRIPT_DEFENSA.as_bytes())
            .map_err(|e| Error::Sistema(e.to_string()))?;
    }
    let mut salida = hijo.stdout.take();
    let lector = std::thread::spawn(move || {
        let mut v = Vec::new();
        if let Some(s) = salida.as_mut() {
            let _ = s.read_to_end(&mut v);
        }
        v
    });
    let inicio = std::time::Instant::now();
    loop {
        match hijo.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if inicio.elapsed() > LIMITE => {
                let _ = hijo.kill();
                let _ = hijo.wait();
                return Err(Error::Sistema(format!(
                    "la comprobación de Defender tardó más de {} s y se paró",
                    LIMITE.as_secs()
                )));
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(200)),
            Err(e) => return Err(Error::Sistema(e.to_string())),
        }
    }
    let bytes = lector.join().unwrap_or_default();
    Ok(String::from_utf8_lossy(&bytes)
        .trim_start_matches('\u{feff}')
        .to_owned())
}
