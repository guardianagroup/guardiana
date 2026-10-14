//! What a program does inside the computer, as Windows counts it (1.0.24): processor time, memory
//! and input/output, per program file name, summed over its processes.
//!
//! The owner, 13 Oct 2026: «hay que ver todo el tiempo por dónde se mueve el antivirus y de qué
//! forma… pero tampoco hay que inventar nada». These are the counters Windows keeps for every
//! process and shows in its own Task Manager; nothing here reads a file, a window or a packet.
//!
//! The input/output counters are the process's own (`GetProcessIoCounters`): every read and write
//! it asked for, to files and to other devices alike. They are not «your documents read»: the page
//! says «leído y escrito por el programa», never more.

use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows_sys::Win32::System::Threading::{
    GetProcessIoCounters, GetProcessTimes, OpenProcess, IO_COUNTERS,
    PROCESS_QUERY_LIMITED_INFORMATION,
};

use super::Actividad;

fn ft(f: &FILETIME) -> u64 {
    (u64::from(f.dwHighDateTime) << 32) | u64::from(f.dwLowDateTime)
}

/// The counters of one process, when Windows lets this service ask (protected processes allow
/// the limited query this uses).
fn de_proceso(pid: u32) -> Option<(u64, u64, u64, u64)> {
    // SAFETY: OpenProcess con el permiso mínimo; el asa se comprueba y se cierra abajo.
    let asa = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if asa.is_null() {
        return None;
    }
    let mut c = FILETIME::default();
    let mut s = FILETIME::default();
    let mut k = FILETIME::default();
    let mut u = FILETIME::default();
    // SAFETY: las cuatro estructuras viven en esta función y el asa es válida.
    let t_ok = unsafe { GetProcessTimes(asa, &raw mut c, &raw mut s, &raw mut k, &raw mut u) } != 0;
    let mut io = IO_COUNTERS::default();
    // SAFETY: estructura local, asa válida.
    let io_ok = unsafe { GetProcessIoCounters(asa, &raw mut io) } != 0;
    let mut m = PROCESS_MEMORY_COUNTERS {
        cb: u32::try_from(std::mem::size_of::<PROCESS_MEMORY_COUNTERS>()).unwrap_or(0),
        ..PROCESS_MEMORY_COUNTERS::default()
    };
    // SAFETY: estructura local con su tamaño en cb, asa válida.
    let m_ok = unsafe { K32GetProcessMemoryInfo(asa, &raw mut m, m.cb) } != 0;
    // SAFETY: el asa la abrió esta función y no se usa después.
    unsafe { CloseHandle(asa) };
    if !t_ok && !io_ok && !m_ok {
        return None;
    }
    // CPU time in 100 ns units → milliseconds.
    let cpu_ms = if t_ok { (ft(&k) + ft(&u)) / 10_000 } else { 0 };
    Some((
        cpu_ms,
        if io_ok { io.ReadTransferCount } else { 0 },
        if io_ok { io.WriteTransferCount } else { 0 },
        if m_ok {
            u64::try_from(m.WorkingSetSize).unwrap_or(0)
        } else {
            0
        },
    ))
}

pub(crate) fn actividad(nombres: &[String]) -> Vec<Actividad> {
    let quiere: Vec<String> = nombres.iter().map(|n| n.to_ascii_lowercase()).collect();
    let mut out: Vec<Actividad> = Vec::new();
    // SAFETY: instantánea de procesos del sistema; el asa se comprueba y se cierra abajo.
    let foto = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if foto == INVALID_HANDLE_VALUE || foto.is_null() {
        return out;
    }
    let mut e = PROCESSENTRY32W {
        dwSize: u32::try_from(std::mem::size_of::<PROCESSENTRY32W>()).unwrap_or(0),
        ..PROCESSENTRY32W::default()
    };
    // SAFETY: e tiene su tamaño en dwSize y la instantánea es válida.
    let mut hay = unsafe { Process32FirstW(foto, &raw mut e) } != 0;
    while hay {
        let largo = e
            .szExeFile
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(e.szExeFile.len());
        let nombre = String::from_utf16_lossy(e.szExeFile.get(..largo).unwrap_or(&[]));
        if quiere.contains(&nombre.to_ascii_lowercase()) {
            if let Some((cpu, lee, escribe, mem)) = de_proceso(e.th32ProcessID) {
                let i = out
                    .iter()
                    .position(|a| a.nombre.eq_ignore_ascii_case(&nombre));
                let a = match i {
                    Some(i) => &mut out[i],
                    None => {
                        out.push(Actividad {
                            nombre: nombre.clone(),
                            ..Actividad::default()
                        });
                        let n = out.len() - 1;
                        &mut out[n]
                    }
                };
                a.procesos += 1;
                a.cpu_ms += cpu;
                a.leido += lee;
                a.escrito += escribe;
                a.memoria += mem;
            }
        }
        // SAFETY: igual que arriba.
        hay = unsafe { Process32NextW(foto, &raw mut e) } != 0;
    }
    // SAFETY: la instantánea la abrió esta función.
    unsafe { CloseHandle(foto) };
    out
}
