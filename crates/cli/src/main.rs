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
            // An empty error means the command already said why (`apps` on Linux printed a
            // bare «guardiana: » line after its own explanation).
            let texto = e.to_string();
            if !texto.is_empty() {
                eprintln!("{}", motivo(&texto));
            }
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

#[cfg(test)]
mod tests {
    /// Every literal text key the command line asks for, and every `cli` key the panel's API
    /// asks for, exists in the three languages. `Texts::cli` and `Texts::panel` fall back to the
    /// key itself, so a missing text prints `ledger.fallo.recortado` to the person and nothing
    /// fails: the privacy branch of the 1 Oct review shipped exactly that (review of 5 Oct 2026).
    #[test]
    fn every_text_key_exists_in_every_language() {
        let sources = [
            include_str!("apps_cmd.rs"),
            include_str!("dns_cmd.rs"),
            include_str!("engine.rs"),
            include_str!("export_cmd.rs"),
            include_str!("hogar_cmd.rs"),
            include_str!("informe_cmd.rs"),
            include_str!("ledger_cmd.rs"),
            include_str!("licencia_cmd.rs"),
            include_str!("main.rs"),
            include_str!("menu.rs"),
            include_str!("observe.rs"),
            include_str!("panel_cmd.rs"),
            include_str!("reglas_cmd.rs"),
            include_str!("service_cmd.rs"),
            include_str!("show.rs"),
            include_str!("trampa_cmd.rs"),
            include_str!("verify_cmd.rs"),
            include_str!("../../panel/src/api.rs"),
            include_str!("../../verify/src/lib.rs"),
            include_str!("../../service/src/sysdns/mod.rs"),
        ];
        let keys = |section: &str| -> Vec<String> {
            let open = format!("{section}(\"");
            let mut out: Vec<String> = sources
                .iter()
                .flat_map(|s| s.split(open.as_str()).skip(1))
                .filter_map(|rest| rest.split('"').next())
                // Keys only: a literal with spaces or braces is a sentence, not an identifier.
                .filter(|k| !k.is_empty() && !k.contains([' ', '{', '}']))
                .map(str::to_owned)
                .collect();
            out.sort_unstable();
            out.dedup();
            out
        };
        let mut cli = keys("cli");
        let panel = keys("panel");
        // And the keys chosen first and printed after (`t.cli(if x { "a.b" } else { "a.c" })`),
        // which the scan above cannot see: every bare literal shaped like a key whose first part
        // is one of the cli sections. `verify.dns.solo_guardiana_ajustes` was printed raw that
        // way in all three languages (review of 5 Oct 2026).
        let es: serde_json::Value =
            serde_json::from_str(guardiana_core::i18n::ES_JSON).unwrap_or_default();
        let secciones: Vec<String> = es["cli"]
            .as_object()
            .map(|o| {
                o.keys()
                    .filter_map(|k| k.split_once('.').map(|(a, _)| a.to_owned()))
                    .collect()
            })
            .unwrap_or_default();
        for s in &sources {
            for lit in s.split('"').skip(1).step_by(2) {
                // File names and domains are not keys.
                let archivo = [".rs", ".json", ".jsonl", ".md", ".txt", ".google", ".com"]
                    .iter()
                    .any(|e| lit.ends_with(e));
                let forma = lit.contains('.')
                    && !lit.ends_with('.')
                    && !archivo
                    && lit.chars().all(|c| {
                        c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.'
                    });
                if forma
                    && lit
                        .split_once('.')
                        .is_some_and(|(a, _)| secciones.iter().any(|x| x == a))
                {
                    cli.push(lit.to_owned());
                }
            }
        }
        cli.sort_unstable();
        cli.dedup();
        assert!(
            cli.len() > 100,
            "the key scan found only {} cli keys",
            cli.len()
        );
        for (lang, json) in [
            ("es", guardiana_core::i18n::ES_JSON),
            ("en", guardiana_core::i18n::EN_JSON),
            ("pt", guardiana_core::i18n::PT_JSON),
        ] {
            let all: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
            assert!(all.is_object(), "{lang}.json does not parse");
            for (section, wanted) in [("cli", &cli), ("panel", &panel)] {
                let missing: Vec<&String> = wanted
                    .iter()
                    .filter(|k| all[section].get(k.as_str()).is_none())
                    .collect();
                assert!(
                    missing.is_empty(),
                    "{lang}.json {section} lacks {missing:?}"
                );
            }
        }
    }
}
