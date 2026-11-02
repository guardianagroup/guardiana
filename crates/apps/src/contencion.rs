//! Freezing, letting go and stopping a program (el escudo, 2 Nov 2026; the owner, 07:55: «tenemos que poder detectarlos
//! y sacarlos, eso es primordial»). Which programs may be frozen is decided by `guardiana_escudo::contencion`; this
//! module only asks Windows.
//!
//! Freezing suspends every thread of the process (`NtSuspendProcess`, the call Windows' own tools use); letting go
//! resumes them (`NtResumeProcess`); stopping ends it (`TerminateProcess`). A frozen program keeps everything it had and
//! does nothing until one of the other two. The whole family is frozen: the program, the programs it started, and the
//! ones above it that may be (a `cmd.exe` a ransomware opened, and the ransomware that opened it), never Windows' own.
//!
//! `unsafe` is used here, as the project allows with justification: these are Windows' C calls, two of them found at
//! run time in `ntdll.dll` (they are not in the import libraries). Every handle is checked and closed, every buffer is
//! owned here and its length is passed with it.
#![allow(unsafe_code)]

use std::collections::{HashMap, HashSet};
use std::ffi::c_void;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows_sys::Win32::System::Threading::{
    OpenProcess, TerminateProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SUSPEND_RESUME,
    PROCESS_TERMINATE,
};

use crate::ventanas::programa_de;

/// `ProcessCommandLineInformation`, the class of `NtQueryInformationProcess` that gives a process's command line.
const LINEA_DE_ORDENES: u32 = 60;
/// `STATUS_INFO_LENGTH_MISMATCH`.
const TAMANO_INSUFICIENTE: i32 = 0xC000_0004_u32 as i32;
/// How far up the family goes from the program that was caught.
const MAX_ANCESTROS: usize = 4;

type NtProceso = unsafe extern "system" fn(HANDLE) -> i32;
type NtConsulta = unsafe extern "system" fn(HANDLE, u32, *mut c_void, u32, *mut u32) -> i32;

/// A function of `ntdll.dll`, by its NUL-terminated name.
fn de_ntdll(nombre: &[u8]) -> Option<unsafe extern "system" fn() -> isize> {
    let modulo: Vec<u16> = "ntdll.dll\0".encode_utf16().collect();
    // SAFETY: a NUL-terminated module name; ntdll.dll is loaded in every process, so the handle needs no closing.
    let h = unsafe { GetModuleHandleW(modulo.as_ptr()) };
    if h.is_null() || nombre.last() != Some(&0) {
        return None;
    }
    // SAFETY: a module handle from GetModuleHandleW and a NUL-terminated name.
    unsafe { GetProcAddress(h, nombre.as_ptr()) }
}

/// Runs `f` on process `pid` opened with `acceso`, closing it after.
fn con_proceso<T>(pid: u32, acceso: u32, f: impl FnOnce(HANDLE) -> T) -> Result<T, String> {
    // SAFETY: no handle is inherited; the result is checked.
    let h = unsafe { OpenProcess(acceso, 0, pid) };
    if h.is_null() {
        // SAFETY: reads this thread's last error code.
        let e = unsafe { windows_sys::Win32::Foundation::GetLastError() };
        return Err(format!("no se pudo abrir el proceso {pid} ({e})"));
    }
    let r = f(h);
    // SAFETY: the handle opened above, closed once.
    unsafe { CloseHandle(h) };
    Ok(r)
}

fn nt(nombre: &[u8], pid: u32) -> Result<(), String> {
    let f = de_ntdll(nombre).ok_or_else(|| "ntdll no tiene esa función".to_owned())?;
    // SAFETY: NtSuspendProcess and NtResumeProcess take one process handle and return an NTSTATUS.
    let f: NtProceso =
        unsafe { std::mem::transmute::<unsafe extern "system" fn() -> isize, NtProceso>(f) };
    let estado = con_proceso(pid, PROCESS_SUSPEND_RESUME, |h| {
        // SAFETY: a process handle opened with PROCESS_SUSPEND_RESUME.
        unsafe { f(h) }
    })?;
    if estado < 0 {
        return Err(format!("Windows no lo permitió (0x{estado:08x})"));
    }
    Ok(())
}

/// Freezes one process: every thread suspended.
///
/// # Errors
/// When the process cannot be opened or Windows refuses.
pub fn congelar(pid: u32) -> Result<(), String> {
    nt(b"NtSuspendProcess\0", pid)
}

/// Lets a frozen process carry on.
///
/// # Errors
/// When the process cannot be opened or Windows refuses.
pub fn reanudar(pid: u32) -> Result<(), String> {
    nt(b"NtResumeProcess\0", pid)
}

/// Stops a process.
///
/// # Errors
/// When the process cannot be opened or Windows refuses.
pub fn detener(pid: u32) -> Result<(), String> {
    let ok = con_proceso(pid, PROCESS_TERMINATE, |h| {
        // SAFETY: a process handle opened with PROCESS_TERMINATE.
        unsafe { TerminateProcess(h, 1) }
    })?;
    if ok == 0 {
        return Err("Windows no lo permitió".to_owned());
    }
    Ok(())
}

/// Every running process: number → parent's number.
fn padres() -> HashMap<u32, u32> {
    let mut out = HashMap::new();
    // SAFETY: a snapshot of the system's processes; checked and closed below.
    let foto = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if foto == INVALID_HANDLE_VALUE || foto.is_null() {
        return out;
    }
    let mut e = PROCESSENTRY32W {
        dwSize: u32::try_from(std::mem::size_of::<PROCESSENTRY32W>()).unwrap_or(0),
        ..PROCESSENTRY32W::default()
    };
    // SAFETY: `e` carries its size and the snapshot is valid.
    let mut hay = unsafe { Process32FirstW(foto, &raw mut e) } != 0;
    while hay {
        out.insert(e.th32ProcessID, e.th32ParentProcessID);
        // SAFETY: as above.
        hay = unsafe { Process32NextW(foto, &raw mut e) } != 0;
    }
    // SAFETY: the snapshot's handle, closed once.
    unsafe { CloseHandle(foto) };
    out
}

/// The family of `pid` that may be frozen, the topmost first: the ones above it that may be (up to a few), and every
/// program started by any of them, as far down as it goes.
#[must_use]
pub fn familia(pid: u32) -> Vec<u32> {
    let yo = std::process::id();
    let mapa = padres();
    let puede = |p: u32| {
        if p == 0 || p == 4 || p == yo {
            return false;
        }
        let (programa, ruta) = programa_de(p);
        guardiana_escudo::contencion::se_puede_congelar(&programa, &ruta)
    };
    if !puede(pid) {
        return Vec::new();
    }
    let mut arriba = pid;
    for _ in 0..MAX_ANCESTROS {
        match mapa.get(&arriba) {
            Some(&p) if p != arriba && puede(p) => arriba = p,
            _ => break,
        }
    }
    let mut out = vec![arriba];
    let mut vistos: HashSet<u32> = out.iter().copied().collect();
    let mut i = 0;
    while let Some(&p) = out.get(i) {
        for (&hijo, &padre) in &mapa {
            if padre == p && hijo != p && !vistos.contains(&hijo) && puede(hijo) {
                vistos.insert(hijo);
                out.push(hijo);
            }
        }
        i += 1;
    }
    if !vistos.contains(&pid) {
        out.push(pid);
    }
    out
}

/// Freezes `pid` and its family ([`familia`]); the numbers frozen.
#[must_use]
pub fn congelar_familia(pid: u32) -> Vec<u32> {
    familia(pid)
        .into_iter()
        .filter(|p| congelar(*p).is_ok())
        .collect()
}

/// A running process's command line, as it was started.
#[must_use]
pub fn linea_de_ordenes(pid: u32) -> Option<String> {
    let f = de_ntdll(b"NtQueryInformationProcess\0")?;
    // SAFETY: NtQueryInformationProcess's signature (handle, class, buffer, its length, length needed) → NTSTATUS.
    let f: NtConsulta =
        unsafe { std::mem::transmute::<unsafe extern "system" fn() -> isize, NtConsulta>(f) };
    con_proceso(pid, PROCESS_QUERY_LIMITED_INFORMATION, |h| {
        // a buffer of u64, so the UNICODE_STRING at its start is aligned
        let mut buf: Vec<u64> = vec![0; 512];
        let mut necesita = 0u32;
        let mut largo = u32::try_from(buf.len() * 8).unwrap_or(0);
        // SAFETY: a process handle with PROCESS_QUERY_LIMITED_INFORMATION and a buffer owned here with its length.
        let mut estado = unsafe {
            f(
                h,
                LINEA_DE_ORDENES,
                buf.as_mut_ptr().cast(),
                largo,
                &raw mut necesita,
            )
        };
        if estado == TAMANO_INSUFICIENTE && necesita > 0 && necesita < 1 << 20 {
            buf = vec![0; (necesita as usize).div_ceil(8)];
            largo = u32::try_from(buf.len() * 8).unwrap_or(0);
            // SAFETY: as above, with the size Windows asked for.
            estado = unsafe {
                f(
                    h,
                    LINEA_DE_ORDENES,
                    buf.as_mut_ptr().cast(),
                    largo,
                    &raw mut necesita,
                )
            };
        }
        if estado < 0 {
            return None;
        }
        // UNICODE_STRING: Length (bytes, u16), MaximumLength (u16), padding, Buffer (pointer into `buf`)
        let primera = *buf.first()?;
        let bytes = usize::from(u16::try_from(primera & 0xFFFF).ok()?);
        let puntero = usize::try_from(*buf.get(1)?).ok()?;
        let base = buf.as_ptr() as usize;
        let desde = puntero.checked_sub(base)?;
        let total = buf.len() * 8;
        if desde.checked_add(bytes)? > total || desde % 2 != 0 {
            return None;
        }
        let palabras: Vec<u16> = buf
            .iter()
            .flat_map(|x| x.to_le_bytes())
            .collect::<Vec<u8>>()
            .get(desde..desde + bytes)?
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        Some(String::from_utf16_lossy(&palabras))
    })
    .ok()
    .flatten()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// On a real Windows: a child that writes a line every 100 ms stops writing while frozen, carries on when let go,
    /// and ends when stopped; its command line reads as it was started.
    #[test]
    #[ignore = "necesita Windows de verdad"]
    fn congelar_reanudar_y_detener_de_verdad() {
        let dir = std::env::temp_dir().join(format!("guardiana-congelar-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let archivo = dir.join("latido.txt");
        let orden = format!(
            "$f='{}'; while ($true) {{ [IO.File]::AppendAllText($f, 'x'); Start-Sleep -Milliseconds 100 }}",
            archivo.display()
        );
        let mut hijo = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", &orden])
            .spawn()
            .unwrap();
        let pid = hijo.id();
        let largo = || std::fs::metadata(&archivo).map(|m| m.len()).unwrap_or(0);
        std::thread::sleep(std::time::Duration::from_millis(2_500));
        assert!(largo() > 0, "el latido no empezó");
        let linea = linea_de_ordenes(pid).unwrap();
        assert!(
            linea.to_ascii_lowercase().contains("powershell") && linea.contains("AppendAllText"),
            "{linea}"
        );
        assert!(familia(pid).contains(&pid));
        congelar(pid).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(400));
        let a = largo();
        std::thread::sleep(std::time::Duration::from_millis(1_500));
        assert_eq!(largo(), a, "congelado y seguía escribiendo");
        reanudar(pid).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1_500));
        assert!(largo() > a, "reanudado y no escribía");
        detener(pid).unwrap();
        let fin = hijo.wait().unwrap();
        assert!(!fin.success());
        println!("congelar, reanudar y detener: correcto (línea: {linea})");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
