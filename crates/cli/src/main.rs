//! `guardiana` command line (brief §2). Week 1: observe | ledger | export.
//!
//! Every sentence printed comes from `crates/panel/i18n/es.json` through
//! `guardiana_core::i18n`; nothing user-facing is written here.

mod apps_cmd;
mod args;
mod dns_cmd;
mod engine;
mod export_cmd;
mod hogar_cmd;
mod informe_cmd;
mod ledger_cmd;
mod licencia_cmd;
mod menu;
mod observe;
mod panel_cmd;
mod reglas_cmd;
mod service_cmd;
mod show;
mod trampa_cmd;
mod verify_cmd;

use std::error::Error;

use guardiana_core::i18n;

/// Piping the output (`guardiana hogar status | head`) must end quietly, not
/// with a panic: Rust ignores SIGPIPE by default and `println!` then fails.
/// Restore the default disposition so the process just exits like any CLI.
#[cfg(unix)]
#[allow(unsafe_code)]
fn reset_sigpipe() {
    // SAFETY: `signal` with SIG_DFL only changes the disposition of SIGPIPE
    // for this process, before any thread is spawned; no memory is touched.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

#[cfg(not(unix))]
fn reset_sigpipe() {}

fn main() {
    reset_sigpipe();
    let code = match run() {
        Ok(()) => 0,
        Err(e) => {
            // Un extracto pertenece a una identidad: el que se creó con otra clave no se abre.
            // El mensaje interno ("ledger was created for another public key…") es inglés
            // técnico con dos huellas de 64 caracteres, y el día que le toque a alguien tiene
            // que poder entender qué le pasa y qué hacer.
            eprintln!("{}", motivo(&e.to_string()));
            1
        }
    };
    std::process::exit(code);
}

/// The reason a run failed, in the words of whoever is reading it. The service uses the same
/// function: until 20 Sep 2026 it threw the error away and stopped with exit code 0, so a Windows
/// machine whose ledger belonged to another key showed "service: installed but stopped" and not
/// one word about why (found while rehearsing the 1.0 upgrade on the test PC).
pub(crate) fn motivo(texto: &str) -> String {
    if texto.contains("ledger was created for another public key") {
        i18n::current().cli("extracto.otra_clave").to_owned()
    } else {
        format!("guardiana: {texto}")
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let (command, opts) = args::parse(&argv);
    let t = i18n::current();
    // `guardiana observe --help` pedía ayuda y en su lugar intentaba arrancar: en una instalación
    // normal la carpeta de datos es del sistema, así que lo que veía la persona era
    // «sqlite: attempt to write a readonly database». Pedir ayuda no abre nada y no falla nunca.
    if argv.iter().any(|a| a == "--help" || a == "-h") {
        println!("{}", t.cli("uso"));
        return Ok(());
    }
    match command.as_deref() {
        Some("observe") => observe::run(&opts),
        Some("ledger") => ledger_cmd::run(&opts),
        Some("export") => export_cmd::run(&opts),
        Some("dns") => dns_cmd::run(&opts),
        Some("menu") => menu::run(&opts),
        Some("hogar") => hogar_cmd::run(&opts),
        Some("informe") => informe_cmd::run(&opts),
        Some("licencia") => licencia_cmd::run(&opts),
        Some("reglas") => reglas_cmd::run(&opts),
        Some("service" | "servicio") => service_cmd::run(&opts),
        Some("panel") => panel_cmd::run(&opts),
        Some("apps") => apps_cmd::run(&opts),
        Some("trampa" | "trampas") => trampa_cmd::run(&opts),
        Some("verify" | "verificar") => verify_cmd::run(&opts),
        Some("version" | "--version" | "-V") => {
            println!("guardiana {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some(other) if other != "help" && other != "--help" && other != "-h" => {
            eprintln!("{}", t.cli("comando_desconocido").replace("{cmd}", other));
            eprintln!("{}", t.cli("uso"));
            Err("".into())
        }
        None => {
            // Double-clicked with no arguments: the interactive test menu
            // (decision 24) instead of a window that vanishes.
            menu::run(&opts)
        }
        _ => {
            println!("{}", t.cli("uso"));
            Ok(())
        }
    }
}
