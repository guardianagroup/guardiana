//! Where the start of the trial is anchored, besides the ledger.
//!
//! The seven days live in the ledger's `settings`, which lives with the person's own data.
//! Uninstalling does not touch that — measured on 22 September 2026: uninstall plus reinstall
//! keeps the same end date — but deleting the data folder starts the seven days again, and on
//! Windows the person at the console can delete it without being an administrator.
//!
//! So the date is written a second time, in a place only an administrator can remove: the
//! registry on Windows, a root-owned file on Linux and macOS. The earlier of the two wins, so the
//! trial can be neither restarted nor extended by touching one of them.
//!
//! **Nothing here is hidden.** The panel says where the mark is and what it holds — a date and
//! nothing else — and `guardiana verify` prints it. A program that asks to be checked cannot
//! leave marks it does not talk about.
//!
//! With `GUARDIANA_DATA` set (test instances and development) the mark lives inside that folder
//! instead, so trying the program out never touches the machine's own.

use std::path::PathBuf;

/// Name of the value in the Windows registry.
#[cfg(windows)]
const NOMBRE: &str = "prueba";

#[cfg(windows)]
const CLAVE: &str = r"HKLM\SOFTWARE\Guardiana";

/// Where the mark lives, in words, for the panel and for `verify`.
#[must_use]
pub fn donde() -> String {
    if let Some(p) = ruta_de_pruebas() {
        return p.display().to_string();
    }
    #[cfg(windows)]
    {
        format!(r"{CLAVE}\{NOMBRE}")
    }
    #[cfg(not(windows))]
    {
        ruta_sistema().display().to_string()
    }
}

/// In test or development mode (`GUARDIANA_DATA`), the mark goes with the data.
fn ruta_de_pruebas() -> Option<PathBuf> {
    std::env::var_os("GUARDIANA_DATA").map(|d| PathBuf::from(d).join("prueba-empezada"))
}

/// Fuera de la carpeta de datos, y donde solo manda root: en macOS los datos viven justamente
/// en `/Library/Application Support/Guardiana`, así que poner ahí la marca no serviría de nada
/// —se borraría con ellos— y además la escribiría cualquier usuario. `/etc` existe en los dos
/// sistemas y solo lo toca un administrador.
#[cfg(not(windows))]
fn ruta_sistema() -> PathBuf {
    PathBuf::from("/etc/guardiana/prueba-empezada")
}

fn leer_archivo(p: &std::path::Path) -> Option<i64> {
    std::fs::read_to_string(p).ok()?.trim().parse().ok()
}

fn escribir_archivo(p: &std::path::Path, ms: i64) {
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(p, ms.to_string());
}

/// When the trial started, according to the mark. `None` if there is none or it cannot be read.
#[must_use]
pub fn leer() -> Option<i64> {
    if let Some(p) = ruta_de_pruebas() {
        return leer_archivo(&p);
    }
    #[cfg(windows)]
    {
        // `reg` instead of a crate: one more dependency for four lines is not worth it, and the
        // service runs as SYSTEM, which can read and write HKLM.
        let salida = std::process::Command::new("reg")
            .args(["query", CLAVE, "/v", NOMBRE])
            .output()
            .ok()?;
        if !salida.status.success() {
            return None;
        }
        let texto = String::from_utf8_lossy(&salida.stdout);
        texto
            .lines()
            .find(|l| l.contains(NOMBRE))?
            .split_whitespace()
            .next_back()?
            .parse()
            .ok()
    }
    #[cfg(not(windows))]
    {
        leer_archivo(&ruta_sistema())
    }
}

/// Write the mark. Best effort: without administrator rights it simply does not happen, and the
/// trial still works with what the ledger says.
pub fn escribir(ms: i64) {
    if let Some(p) = ruta_de_pruebas() {
        escribir_archivo(&p, ms);
        return;
    }
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("reg")
            .args([
                "add",
                CLAVE,
                "/v",
                NOMBRE,
                "/t",
                "REG_SZ",
                "/d",
                &ms.to_string(),
                "/f",
            ])
            .output();
    }
    #[cfg(not(windows))]
    {
        escribir_archivo(&ruta_sistema(), ms);
    }
}

/// Las pruebas del crate corren en paralelo dentro del mismo proceso y `GUARDIANA_DATA` es de
/// todo el proceso: sin cerrojo, la prueba que escribe la marca se la cambia a las demás, y la
/// que no lo sepa acabará escribiendo en la carpeta de datos de verdad de esta máquina.
#[cfg(test)]
pub(crate) static CERROJO: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Toma el cerrojo y deja `GUARDIANA_DATA` apuntando a una carpeta propia de la prueba.
#[cfg(test)]
pub(crate) fn a_solas_en(dir: &std::path::Path) -> std::sync::MutexGuard<'static, ()> {
    let g = CERROJO
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    std::fs::create_dir_all(dir).ok();
    std::env::set_var("GUARDIANA_DATA", dir);
    g
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// With `GUARDIANA_DATA` the mark goes into that folder and nowhere else: probar el programa
    /// no deja nada en la máquina de quien lo prueba.
    #[test]
    fn con_carpeta_de_pruebas_la_marca_va_dentro() {
        let dir = std::env::temp_dir().join(format!("guardiana-ancla-{}", std::process::id()));
        let _a_solas = a_solas_en(&dir);
        assert!(leer().is_none(), "empieza sin marca");
        escribir(1_790_000_000_000);
        assert_eq!(leer(), Some(1_790_000_000_000));
        assert!(donde().contains("prueba-empezada"));
        std::env::remove_var("GUARDIANA_DATA");
        std::fs::remove_dir_all(&dir).ok();
    }
}
