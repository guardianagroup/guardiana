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
//!
//! The same call reads the version the file declares (1.0.45, «contrato vivo»): its product
//! version, else its file version, as Explorer's «Details» tab shows them. One PowerShell per
//! program file for both, never one per query.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Condvar, Mutex};

/// The most programs remembered.
const MAXIMO: usize = 4_096;

#[derive(Default)]
struct Estado {
    /// Program file → (signer, declared version).
    hechas: HashMap<String, (String, String)>,
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
    /// The signer once checked (`Some("")`: no valid signature), or `None` while not checked yet. Asking
    /// does not queue a check: [`Firmas::firma_y_version`] already did when the program was seen.
    pub(crate) fn comprobada(&self, ruta: &str) -> Option<String> {
        let (m, _) = &*self.estado;
        let e = m.lock().ok()?;
        e.hechas.get(ruta).map(|(firma, _)| firma.clone())
    }

    pub(crate) fn de(&self, ruta: &str) -> String {
        self.firma_y_version(ruta).0
    }

    /// Who signs the program at `ruta` and the version its file declares, if already checked;
    /// asking queues it. Both empty until the check is done.
    pub(crate) fn firma_y_version(&self, ruta: &str) -> (String, String) {
        if ruta.is_empty() {
            return (String::new(), String::new());
        }
        let (m, cv) = &*self.estado;
        let Ok(mut e) = m.lock() else {
            return (String::new(), String::new());
        };
        if let Some(f) = e.hechas.get(ruta) {
            return f.clone();
        }
        if e.pedidas.len() < MAXIMO && e.pedidas.insert(ruta.to_owned()) {
            e.cola.push_back(ruta.to_owned());
            cv.notify_one();
        }
        (String::new(), String::new())
    }
}

fn trabaja(estado: &(Mutex<Estado>, Condvar)) {
    let (m, cv) = estado;
    loop {
        let tanda = {
            let Ok(mut e) = m.lock() else { return };
            loop {
                if !e.cola.is_empty() {
                    break toma_tanda(&mut e.cola);
                }
                e = match cv.wait(e) {
                    Ok(e) => e,
                    Err(_) => return,
                };
            }
        };
        let firmas = firmantes(&tanda);
        if let Ok(mut e) = m.lock() {
            for (ruta, firma) in tanda.into_iter().zip(firmas) {
                e.hechas.insert(ruta, firma);
            }
        }
    }
}

/// The most programs one PowerShell checks (1.0.55, «ligero»; the owner, 2 Nov 2026: «se me
/// traba»). Until 1.0.54 it was one PowerShell per program file, and the programs a computer
/// already has running all show up in the first minutes after the service starts, which is when
/// the computer is busiest: now one start checks up to this many.
const POR_TANDA: usize = 16;
/// The most characters of paths in one batch: they travel in one environment variable, which
/// holds at most 32 767.
const MAXIMO_TANDA: usize = 30_000;
/// The variable the paths travel in, one per line.
const VARIABLE: &str = "GUARDIANA_FIRMAS";

/// The check, for every path in [`VARIABLE`]. The paths are data, never part of the command:
/// nothing in a file name can become one, not even the typographic quotes (‘ ’) PowerShell also
/// takes for quotes. Before each path's lines, a line `N:`; then its two tagged lines, `F:` the
/// signer and `V:` the version, so neither can pass for the other. What fails for one path (it
/// is not there, it cannot be read) leaves its lines empty and the next path is checked anyway.
const GUION: &str = "foreach ($r in $env:GUARDIANA_FIRMAS.Split([char]10)) { 'N:'; \
     try { $s = Get-AuthenticodeSignature -LiteralPath $r -ErrorAction Stop; \
     if ($s.Status -eq 'Valid') { 'F:' + $s.SignerCertificate.GetNameInfo(0, $false) } } catch { }; \
     try { $v = (Get-Item -LiteralPath $r -ErrorAction Stop).VersionInfo; \
     if ($v) { $p = $v.ProductVersion; if (-not $p) { $p = $v.FileVersion }; \
     if ($p) { 'V:' + $p } } } catch { } }";

/// Up to [`POR_TANDA`] paths from the front of the queue, while they fit in [`MAXIMO_TANDA`]; at
/// least one.
fn toma_tanda(cola: &mut VecDeque<String>) -> Vec<String> {
    let mut tanda = Vec::new();
    let mut largo = 0;
    while let Some(r) = cola.front() {
        if !tanda.is_empty() && (tanda.len() >= POR_TANDA || largo + r.len() + 1 > MAXIMO_TANDA) {
            break;
        }
        largo += r.len() + 1;
        if let Some(r) = cola.pop_front() {
            tanda.push(r);
        }
    }
    tanda
}

/// What [`GUION`] printed, path by path: the lines after each `N:`, up to the next one.
fn de_tanda(texto: &str) -> Vec<String> {
    let mut partes: Vec<String> = Vec::new();
    for linea in texto.lines() {
        if linea.trim() == "N:" {
            partes.push(String::new());
        } else if let Some(p) = partes.last_mut() {
            p.push_str(linea);
            p.push('\n');
        }
    }
    partes
}

/// For each path, in its order: the signer's common name when the signature is valid (empty
/// otherwise), and the version the file declares (empty when it declares none). One PowerShell
/// for all of them; if it stopped half-way, the ones it did not reach are checked one by one.
fn firmantes(rutas: &[String]) -> Vec<(String, String)> {
    use std::os::windows::process::CommandExt;
    // CREATE_NO_WINDOW: no console flashing when this runs from the service.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let vacio = || (String::new(), String::new());
    let Ok(salida) = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", GUION])
        .env(VARIABLE, rutas.join("\n"))
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    else {
        return rutas.iter().map(|_| vacio()).collect();
    };
    let partes = de_tanda(&String::from_utf8_lossy(&salida.stdout));
    let mut out: Vec<(String, String)> = partes
        .iter()
        .take(rutas.len())
        .map(|p| crate::lee_firma_y_version(p))
        .collect();
    if out.len() < rutas.len() {
        if rutas.len() == 1 {
            out.push(vacio());
        } else {
            for r in &rutas[out.len()..] {
                out.extend(firmantes(std::slice::from_ref(r)));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The queue is taken in batches of at most [`POR_TANDA`], in order, never empty, and a path too
    /// long for a batch goes alone.
    #[test]
    fn la_cola_sale_en_tandas() {
        let mut cola: VecDeque<String> = (0..40).map(|i| format!(r"C:\p\{i}.exe")).collect();
        let a = toma_tanda(&mut cola);
        assert_eq!(a.len(), POR_TANDA);
        assert_eq!(a[0], r"C:\p\0.exe");
        assert_eq!(cola.len(), 40 - POR_TANDA);
        let mut larga: VecDeque<String> = ["x".repeat(MAXIMO_TANDA + 5), "y".to_owned()]
            .into_iter()
            .collect();
        assert_eq!(toma_tanda(&mut larga).len(), 1);
        assert_eq!(toma_tanda(&mut larga), vec!["y".to_owned()]);
        assert!(toma_tanda(&mut larga).is_empty());
    }

    /// Each path's lines are its own, and a path with nothing to say still counts.
    #[test]
    fn lo_que_imprime_la_tanda_se_reparte_por_ruta() {
        let texto = "N:\r\nF:Microsoft Windows\r\nV:10.0.1\r\nN:\r\nN:\r\nV:2.1\r\n";
        let p = de_tanda(texto);
        assert_eq!(p.len(), 3);
        assert_eq!(
            crate::lee_firma_y_version(&p[0]),
            ("Microsoft Windows".to_owned(), "10.0.1".to_owned())
        );
        assert_eq!(
            crate::lee_firma_y_version(&p[1]),
            (String::new(), String::new())
        );
        assert_eq!(
            crate::lee_firma_y_version(&p[2]),
            (String::new(), "2.1".to_owned())
        );
        assert!(de_tanda("").is_empty());
    }

    /// On a real Windows: one PowerShell checks several programs and says for each what it said
    /// alone; a path that is not there says nothing; and a file name made to break out of quotes
    /// (with Windows' typographic quotes, which PowerShell also takes for quotes) runs nothing.
    #[test]
    fn en_windows_una_tanda_dice_lo_mismo_que_de_una_en_una() {
        let raiz = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_owned());
        let carpeta = std::env::temp_dir().join(format!("guardiana-firmas-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&carpeta);
        // Relative: the PowerShell would make it in its working folder, which is this test's.
        let marca = format!("ligero-marca-{}", std::process::id());
        let trampa = carpeta.join(format!(
            "a\u{2019}; New-Item -ItemType File -Path {marca}; \u{2018}.txt"
        ));
        let _ = std::fs::write(&trampa, b"x");
        let rutas: Vec<String> = vec![
            format!(r"{raiz}\System32\notepad.exe"),
            format!(r"{raiz}\System32\cmd.exe"),
            format!(r"{raiz}\no-esta-{}.exe", std::process::id()),
            trampa.display().to_string(),
            format!(r"{raiz}\explorer.exe"),
        ];
        let juntas = firmantes(&rutas);
        assert_eq!(juntas.len(), rutas.len());
        for (r, f) in rutas.iter().zip(&juntas) {
            let sola = firmantes(std::slice::from_ref(r));
            println!("{r}: {f:?}");
            assert_eq!(sola, vec![f.clone()], "{r}");
        }
        assert!(
            juntas.iter().any(|(f, _)| !f.is_empty()),
            "los programas de Windows están firmados: {juntas:?}"
        );
        assert_eq!(juntas[2], (String::new(), String::new()));
        let ejecutada = std::path::Path::new(&marca).exists();
        let _ = std::fs::remove_file(&marca);
        let _ = std::fs::remove_dir_all(&carpeta);
        assert!(!ejecutada, "un nombre de archivo ejecutó una orden");
    }
}
