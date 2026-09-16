//! Where Guardiana keeps its data on each platform (brief §3: one database
//! per installation). `GUARDIANA_DATA` overrides everything, for tests and
//! for running from a checkout.

use std::path::{Path, PathBuf};

/// Environment variable that overrides the data directory.
pub const DATA_ENV: &str = "GUARDIANA_DATA";
/// File name of the ledger database inside the data directory.
pub const LEDGER_FILE: &str = "ledger.db";

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
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        // macOS: development only (brief §1: macOS is out of 1.0).
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
