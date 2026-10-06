//! `guardiana panel`: open the panel in the browser with the session token
//! from the token file (the launcher of brief §8).

use std::error::Error;

use guardiana_core::{i18n, paths};

use crate::args::Opts;

pub fn run(_opts: &Opts) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    let path = paths::data_dir().join(guardiana_panel::TOKEN_FILE);
    let token = std::fs::read_to_string(&path).map_err(|e| {
        // On Linux and the Mac the key belongs to the service, and a person who types
        // `guardiana panel` without sudo was asked whether Guardiana was running, which it was
        // (review of 5 Oct 2026, Linux medium). The real reason, and what to type.
        let clave =
            if e.kind() == std::io::ErrorKind::PermissionDenied || paths::unreadable_here(&path) {
                "panel.sin_permiso"
            } else {
                "panel.sin_token"
            };
        t.cli(clave).replace("{ruta}", &path.display().to_string())
    })?;
    let url = format!("http://127.0.0.1:7443/?t={}", token.trim());
    // Through sudo, the browser would open as the administrator, or not at all: the link is
    // given to the person to open in their own.
    if std::env::var_os("SUDO_USER").is_some() {
        println!("{}", t.cli("panel.enlace_sudo").replace("{url}", &url));
        return Ok(());
    }
    println!("{}", t.cli("panel.abriendo").replace("{url}", &url));
    crate::engine::open_in_browser(&url);
    Ok(())
}
