//! Interactive test menu shown when the program is double-clicked with no
//! arguments (decision 24). A testing aid for the first users; the panel is
//! the real interface. Every sentence comes from es.json.

use std::error::Error;
use std::io::{BufRead, Write};

use guardiana_core::i18n;

use crate::args::Opts;
use crate::{dns_cmd, ledger_cmd, observe};

/// `None` when there is no one to answer: input ended (a pipe, a service, a
/// command run over ssh). Without this the menu asked for ever and pinned a
/// core; found running `guardiana --version` on a machine with no terminal.
fn ask(prompt: &str) -> Option<String> {
    print!("{prompt}");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    match std::io::stdin().lock().read_line(&mut line) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(line.trim().to_owned()),
    }
}

fn pause() {
    let t = i18n::current();
    let _ = ask(t.cli("menu.volver"));
}

#[cfg(target_os = "windows")]
fn is_admin() -> bool {
    std::process::Command::new("net")
        .arg("session")
        .output()
        .is_ok_and(|o| o.status.success())
}

#[cfg(not(target_os = "windows"))]
fn is_admin() -> bool {
    // Unix: root has uid 0; read it from the environment-free `id -u` to avoid unsafe.
    std::process::Command::new("id")
        .arg("-u")
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "0")
}

/// Re-run this program elevated with `menu --choice N`. Windows only; on
/// other systems the user is told to use sudo.
fn relaunch_elevated(choice: &str) -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    #[cfg(target_os = "windows")]
    {
        let script = format!(
            "Start-Process -FilePath '{}' -ArgumentList 'menu','--choice','{}' -Verb RunAs",
            exe.display().to_string().replace('\'', "''"),
            choice
        );
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .status()
            .is_ok_and(|s| s.success())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (exe, choice);
        false
    }
}

fn observe_test() -> Result<(), Box<dyn Error>> {
    let mut o = Opts::default();
    o.set("listen", Some("127.0.0.1:5335"));
    o.set("open", None);
    o.set("self-test", None);
    let t = i18n::current();
    println!("{}", t.cli("menu.parar"));
    observe::run(&o)
}

fn watch_for_real() -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    let mut status = Opts::default();
    status.set("status", None);
    dns_cmd::run(&status)?;
    println!();
    println!("{}", t.cli("dns.consentimiento"));
    println!("{}", t.cli(dns_cmd::sole_or_secondary_key()));
    println!();
    if ask(t.cli("menu.confirmar_si")).as_deref() != Some("SI") {
        println!("{}", t.cli("menu.nada_cambiado"));
        return Ok(());
    }
    let mut apply = Opts::default();
    apply.set("apply", None);
    apply.set("yes", None);
    dns_cmd::run(&apply)?;
    println!();
    println!("{}", t.cli("menu.navega"));
    println!("{}", t.cli("menu.parar"));
    let mut o = Opts::default();
    o.set("open", None);
    let result = observe::run(&o);
    println!();
    let mut restore = Opts::default();
    restore.set("restore", None);
    let _ = dns_cmd::run(&restore);
    result
}

fn run_choice(choice: &str) -> Result<bool, Box<dyn Error>> {
    let t = i18n::current();
    match choice {
        "1" => observe_test()?,
        "2" => {
            let mut o = Opts::default();
            o.set("check", None);
            let _ = ledger_cmd::run(&o);
            println!();
            let mut o = Opts::default();
            o.set("last", Some("20"));
            let _ = ledger_cmd::run(&o);
        }
        "3" | "4" => {
            if !is_admin() {
                println!("{}", t.cli("menu.admin_necesario"));
                if relaunch_elevated(choice) {
                    println!("{}", t.cli("menu.admin_lanzado"));
                } else {
                    println!("{}", t.cli("menu.admin_fallo"));
                }
            } else if choice == "3" {
                watch_for_real()?;
            } else {
                let mut o = Opts::default();
                o.set("restore", None);
                dns_cmd::run(&o)?;
            }
        }
        "5" => {
            let mut o = Opts::default();
            o.set("status", None);
            dns_cmd::run(&o)?;
        }
        "6" | "7" => {
            let mut o = Opts::default();
            o.positional_push(if choice == "6" { "on" } else { "off" });
            crate::hogar_cmd::run(&o)?;
        }
        "8" => crate::hogar_cmd::run(&Opts::default())?,
        "9" | "10" => {
            if !is_admin() {
                println!("{}", t.cli("menu.admin_necesario"));
                if relaunch_elevated(choice) {
                    println!("{}", t.cli("menu.admin_lanzado"));
                } else {
                    println!("{}", t.cli("menu.admin_fallo"));
                }
            } else {
                let mut o = Opts::default();
                o.positional_push(if choice == "9" {
                    "install"
                } else {
                    "uninstall"
                });
                crate::service_cmd::run(&o)?;
            }
        }
        "11" => crate::panel_cmd::run(&Opts::default())?,
        "12" => crate::service_cmd::status()?,
        "13" => crate::verify_cmd::run(&Opts::default())?,
        "14" => crate::licencia_cmd::run(&Opts::default())?,
        "0" | "q" | "salir" => return Ok(false),
        _ => println!("{}", t.cli("menu.no_entiendo")),
    }
    Ok(true)
}

/// The menu loop. `--choice N` runs one option and exits (used when relaunched elevated).
pub fn run(opts: &Opts) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    if let Some(choice) = opts.get("choice") {
        if let Err(e) = run_choice(choice) {
            eprintln!("guardiana: {e}");
        }
        pause();
        return Ok(());
    }
    loop {
        println!();
        println!("{}", t.cli("menu.titulo"));
        println!("{}", t.cli("menu.opciones"));
        println!();
        let Some(choice) = ask(t.cli("menu.elige")) else {
            println!();
            return Ok(());
        };
        println!();
        match run_choice(&choice) {
            Ok(true) => {}
            Ok(false) => return Ok(()),
            Err(e) => eprintln!("guardiana: {e}"),
        }
        if !matches!(choice.as_str(), "1" | "3") {
            pause();
        }
    }
}

/// Where the self-test sends its queries and which names it asks.
pub(crate) const SELF_TEST_NAMES: &[&str] = &[
    "example.com",
    "www.google-analytics.com",
    "ad.doubleclick.net",
    "time.windows.com",
    "dns.google",
    "use-application-dns.net",
];
