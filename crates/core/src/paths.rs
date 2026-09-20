//! Where Guardiana keeps its data on each platform (brief §3: one database
//! per installation). `GUARDIANA_DATA` overrides everything, for tests and
//! for running from a checkout.

use std::path::{Path, PathBuf};

/// Environment variable that overrides the data directory.
pub const DATA_ENV: &str = "GUARDIANA_DATA";
/// File name of the ledger database inside the data directory.
pub const LEDGER_FILE: &str = "ledger.db";
/// File where a service that could not start leaves the reason, next to the ledger.
pub const MOTIVO_FILE: &str = "ultimo-error.txt";

/// The data directory for this platform (not created here).
#[must_use]
pub fn data_dir() -> PathBuf {
    if let Some(p) = std::env::var_os(DATA_ENV) {
        return PathBuf::from(p);
    }
    #[cfg(target_os = "windows")]
    {
        let base = std::env::var_os("ProgramData")
            .map_or_else(|| PathBuf::from(r"C:\ProgramData"), PathBuf::from);
        base.join("Guardiana")
    }
    #[cfg(target_os = "linux")]
    {
        PathBuf::from("/var/lib/guardiana")
    }
    #[cfg(target_os = "macos")]
    {
        // System-wide, like ProgramData on Windows and /var/lib on Linux: the daemon runs as
        // root under launchd and the ledger belongs to the machine, not to one user. Pointing
        // this at the user's home made `guardiana verify` read an empty folder and then say,
        // confidently and wrongly, that there was no ledger and that Home Mode was off.
        PathBuf::from("/Library/Application Support/Guardiana")
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("."), PathBuf::from);
        home.join("Library/Application Support/Guardiana")
    }
}

/// Full path of the ledger database.
#[must_use]
pub fn ledger_path() -> PathBuf {
    data_dir().join(LEDGER_FILE)
}

/// Restrict the data directory to the system and the user (brief §8: the
/// panel token is "readable only by the user"; the extract is the user's).
/// Unix: mode 0700. Windows: the folder under ProgramData inherits read
/// access for every local account, so inheritance is dropped, SYSTEM and
/// Administrators keep full control and the user at the console gets modify
/// rights, so `guardiana panel` and `guardiana verify` work without
/// elevation. Best effort: the caller decides what to do with an error.
pub fn harden_data_dir(dir: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
    }
    #[cfg(windows)]
    {
        use std::process::Command;
        let dir_s = dir.to_string_lossy().into_owned();
        let out = Command::new("icacls")
            .args([
                dir_s.as_str(),
                "/inheritance:r",
                "/grant:r",
                "*S-1-5-18:(OI)(CI)F",
                "*S-1-5-32-544:(OI)(CI)F",
            ])
            .output()?;
        if !out.status.success() {
            return Err(std::io::Error::other(
                String::from_utf8_lossy(&out.stderr).into_owned(),
            ));
        }
        let user = Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "(Get-CimInstance Win32_ComputerSystem).UserName",
            ])
            .output()?;
        let user = String::from_utf8_lossy(&user.stdout).trim().to_owned();
        // Nobody at the console yet (service start at boot): the grant happens
        // on a later pass. Names with quotes or newlines are not passed on.
        if !user.is_empty()
            && user
                .chars()
                .all(|c| c.is_alphanumeric() || "\\ ._-".contains(c))
        {
            let _ = Command::new("icacls")
                .args([dir_s.as_str(), "/grant", &format!("{user}:(OI)(CI)M")])
                .output();
        }
        Ok(())
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = dir;
        Ok(())
    }
}

/// Whether the extract is there but this process cannot read it.
///
/// On Linux the service runs as root and `/var/lib/guardiana` is root-only, so a
/// person who types `guardiana verify` without sudo cannot open the ledger. Asking
/// `Path::exists()` is no good: the directory is not even traversable, so the
/// answer comes back "it does not exist" and the program then says something false
/// and confident — that Guardiana has not changed the DNS. What the file system is
/// really saying is EACCES, and that is a different sentence for the person.
pub fn unreadable_here(path: &std::path::Path) -> bool {
    matches!(
        std::fs::File::open(path).err().map(|e| e.kind()),
        Some(std::io::ErrorKind::PermissionDenied)
    )
}

/// Full path of the file where a failed service start leaves its reason.
#[must_use]
pub fn motivo_path() -> PathBuf {
    data_dir().join(MOTIVO_FILE)
}

/// Writes why the service could not run, with the time, next to the ledger.
///
/// A Windows service has nowhere to print. Until 20 September 2026 the error was dropped and the
/// service stopped with exit code 0: on the test machine, whose ledger belonged to the previous
/// signing key, Windows said "installed but stopped" and neither the event log nor `verify` nor
/// the panel said a word about why. Best effort: if the file cannot be written the service still
/// stops with a code that is not "everything went fine".
pub fn guardar_motivo(texto: &str, ahora_ms: i64) {
    guardar_motivo_en(&data_dir(), texto, ahora_ms);
}

/// The last reason a service start failed: when (ms) and the text, if it is there.
#[must_use]
pub fn leer_motivo() -> Option<(i64, String)> {
    leer_motivo_en(&data_dir())
}

/// Same, against a given directory: what the two above use, and what the test can check without
/// touching the environment of the whole process.
pub fn guardar_motivo_en(dir: &Path, texto: &str, ahora_ms: i64) {
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(dir.join(MOTIVO_FILE), format!("{ahora_ms}\n{texto}\n"));
}

/// The reason written in `dir`, if any.
#[must_use]
pub fn leer_motivo_en(dir: &Path) -> Option<(i64, String)> {
    let texto = std::fs::read_to_string(dir.join(MOTIVO_FILE)).ok()?;
    let (cuando, resto) = texto.split_once('\n')?;
    let resto = resto.trim_end();
    if resto.is_empty() {
        return None;
    }
    Some((cuando.trim().parse().ok()?, resto.to_owned()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod pruebas_motivo {
    use super::*;

    #[test]
    fn el_motivo_se_escribe_y_se_lee() {
        let dir = std::env::temp_dir().join(format!("guardiana-motivo-{}", std::process::id()));
        guardar_motivo_en(&dir, "el extracto es de otra clave", 1_700_000_000_000);
        let (cuando, texto) = leer_motivo_en(&dir).expect("debería haber motivo");
        assert_eq!(cuando, 1_700_000_000_000);
        assert_eq!(texto, "el extracto es de otra clave");
        std::fs::remove_dir_all(&dir).ok();
        assert!(leer_motivo_en(&dir).is_none());
    }
}
