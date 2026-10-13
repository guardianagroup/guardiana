//! Who signs a program, as Windows itself checks it (1.0.21).
//!
//! Until 1.0.20 `Proceso::firmado_por` was always empty, and the antivirus page said for every
//! process that Windows had not said who signs it. Windows can say it: `Get-AuthenticodeSignature`
//! checks the program's signature (embedded or in the system catalogue) the same way Explorer's
//! «Digital signatures» tab does. Only a **valid** signature counts: the name on a broken or
//! untrusted one is not given.
//!
//! The check runs once per program file, on a thread of its own, and is remembered: the first
//! query of a program goes without its signer, the next ones carry it.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Condvar, Mutex};

/// The most programs remembered.
const MAXIMO: usize = 4_096;

#[derive(Default)]
struct Estado {
    hechas: HashMap<String, String>,
    pedidas: HashSet<String>,
    cola: VecDeque<String>,
}

/// Remembers, per program file, who signs it.
#[derive(Clone, Default)]
pub(crate) struct Firmas {
    estado: Arc<(Mutex<Estado>, Condvar)>,
}

impl Firmas {
    /// Starts the thread that checks the signatures.
    pub(crate) fn arrancar() -> Self {
        let f = Self::default();
        let estado = Arc::clone(&f.estado);
        std::thread::Builder::new()
            .name("guardiana-firmas".into())
            .spawn(move || trabaja(&estado))
            .ok();
        f
    }

    /// Who signs the program at `ruta`, if already checked; asking queues it.
    pub(crate) fn de(&self, ruta: &str) -> String {
        if ruta.is_empty() {
            return String::new();
        }
        let (m, cv) = &*self.estado;
        let Ok(mut e) = m.lock() else {
            return String::new();
        };
        if let Some(f) = e.hechas.get(ruta) {
            return f.clone();
        }
        if e.pedidas.len() < MAXIMO && e.pedidas.insert(ruta.to_owned()) {
            e.cola.push_back(ruta.to_owned());
            cv.notify_one();
        }
        String::new()
    }
}

fn trabaja(estado: &(Mutex<Estado>, Condvar)) {
    let (m, cv) = estado;
    loop {
        let ruta = {
            let Ok(mut e) = m.lock() else { return };
            loop {
                if let Some(r) = e.cola.pop_front() {
                    break r;
                }
                e = match cv.wait(e) {
                    Ok(e) => e,
                    Err(_) => return,
                };
            }
        };
        let firma = firmante(&ruta);
        if let Ok(mut e) = m.lock() {
            e.hechas.insert(ruta, firma);
        }
    }
}

/// The signer's common name when the signature is valid, empty otherwise.
fn firmante(ruta: &str) -> String {
    use std::os::windows::process::CommandExt;
    // CREATE_NO_WINDOW: no console flashing when this runs from the service.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // The path goes as a literal between single quotes, with its own quotes doubled: nothing in
    // a file name can become a command.
    let literal = ruta.replace('\'', "''");
    let orden = format!(
        "$s = Get-AuthenticodeSignature -LiteralPath '{literal}'; \
         if ($s.Status -eq 'Valid') {{ $s.SignerCertificate.GetNameInfo(0, $false) }}"
    );
    let Ok(salida) = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &orden])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    else {
        return String::new();
    };
    String::from_utf8_lossy(&salida.stdout)
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or_default()
        .chars()
        .take(200)
        .collect()
}
