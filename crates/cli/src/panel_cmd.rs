//! `guardiana panel`: open the panel in the browser with the session token
//! from the token file (the launcher of brief §8).

use std::error::Error;

use guardiana_core::{i18n, paths};

use crate::args::Opts;

/// How long to wait for the service to be up before opening the browser. The installer runs
/// this right after starting (or restarting, on an update) the service, and a browser opened
/// one second too early showed "cannot connect" or nothing at all: the person thought the
/// program had stayed in the background (owner's report, 8 Oct 2026).
const ESPERA_ARRANQUE: std::time::Duration = std::time::Duration::from_secs(20);

/// Whether the panel answers on its port right now.
fn panel_responde() -> bool {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], guardiana_panel::DEFAULT_PORT));
    std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(500)).is_ok()
}

/// Waits, up to [`ESPERA_ARRANQUE`], for the token file to exist and the panel to answer.
/// Returns whether it did; the caller carries on either way, with the sentence that fits.
fn esperar_al_servicio(path: &std::path::Path) -> bool {
    let fin = std::time::Instant::now() + ESPERA_ARRANQUE;
    loop {
        if path.exists() && panel_responde() {
            return true;
        }
        if std::time::Instant::now() >= fin {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
}

pub fn run(_opts: &Opts) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    let path = paths::data_dir().join(guardiana_panel::TOKEN_FILE);
    if !panel_responde() {
        println!("{}", t.cli("panel.esperando"));
        esperar_al_servicio(&path);
    }
    let token = std::fs::read_to_string(&path).map_err(|e| {
        // On Linux and the Mac the key belongs to the service, and a person who types
        // `guardiana panel` without sudo was asked whether Guardiana was running, which it was
        // (review of 5 Oct 2026, Linux medium). The real reason, and what to type.
        let clave =
            if e.kind() == std::io::ErrorKind::PermissionDenied || paths::unreadable_here(&path) {
                // On Windows "sudo" does not exist: another sentence.
                if cfg!(windows) {
                    "panel.sin_permiso_windows"
                } else {
                    "panel.sin_permiso"
                }
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
