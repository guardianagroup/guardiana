//! `guardiana hogar status | on | off` (brief §7). Turning it on records the
//! setting and adds the firewall rule; the LAN sockets open the next time
//! Guardiana starts. `status` prints ports, devices and active rules.

use std::error::Error;

use guardiana_core::i18n;
use guardiana_core::time::{now_ms, rfc3339_utc};
use guardiana_core::ChangeKind;
use guardiana_core::{identity, paths, Ledger};
use guardiana_service::home::{
    self, FirewallError, SETTING_HOME_IP, SETTING_HOME_MODE, SETTING_HOME_SINCE,
};

use crate::args::Opts;

fn open(opts: &Opts) -> Result<Ledger, Box<dyn Error>> {
    let db = opts
        .get("db")
        .map_or_else(paths::ledger_path, std::path::PathBuf::from);
    if let Some(parent) = db.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(Ledger::open(&db, identity::genesis())?)
}

pub fn status(ledger: &Ledger) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    let on = ledger.setting(SETTING_HOME_MODE)?.is_some_and(|v| v == "1");
    let ip = ledger.setting(SETTING_HOME_IP)?.unwrap_or_default();
    let since = ledger
        .setting(SETTING_HOME_SINCE)?
        .and_then(|v| v.parse::<i64>().ok())
        .map(rfc3339_utc)
        .unwrap_or_default();
    if on {
        println!(
            "{}",
            t.cli("hogar.estado.on")
                .replace("{fecha}", &since)
                .replace("{ip}", &ip)
        );
        println!(
            "{}",
            t.cli("hogar.puertos").replace(
                "{puertos}",
                &format!("{ip}:53/udp {ip}:53/tcp {ip}:7443/tcp")
            )
        );
        if let Ok(ip4) = ip.parse::<std::net::Ipv4Addr>() {
            println!(
                "{}",
                t.cli("hogar.qr").replace("{url}", &home::home_url(ip4))
            );
        }
    } else {
        println!("{}", t.cli("hogar.estado.off"));
        println!(
            "{}",
            t.cli("hogar.puertos")
                .replace("{puertos}", t.cli("hogar.puertos.ninguno"))
        );
    }
    println!(
        "{}",
        t.cli("hogar.dispositivos")
            .replace("{n}", &ledger.devices()?.len().to_string())
    );
    println!(
        "{}",
        t.cli("hogar.reglas")
            .replace("{n}", &ledger.active_rules(now_ms())?.len().to_string())
    );
    Ok(())
}

pub fn run(opts: &Opts) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    let ledger = open(opts)?;
    let word = opts
        .positional()
        .first()
        .map(String::as_str)
        .unwrap_or("status");
    match word {
        "on" | "encender" => {
            let Some(lan) = guardiana_devices::local_lan_ipv4() else {
                return Err(t.panel("hogar_sin_lan").into());
            };
            match home::firewall_allow() {
                Ok(()) => {}
                Err(FirewallError::Manual) => println!("{}", t.panel("hogar_firewall_manual")),
                Err(FirewallError::Command(_)) => println!("{}", t.panel("hogar_no_admin")),
            }
            ledger.set_setting(SETTING_HOME_MODE, "1")?;
            ledger.set_setting(SETTING_HOME_IP, &lan.to_string())?;
            ledger.set_setting(SETTING_HOME_SINCE, &now_ms().to_string())?;
            ledger.record_change(now_ms(), ChangeKind::HogarOn, "terminal", &lan.to_string())?;
            println!(
                "{}",
                t.cli("hogar.on.hecho").replace("{ip}", &lan.to_string())
            );
            if let Some(true) = home::ip_is_dynamic(lan) {
                println!(
                    "{}",
                    t.panel("hogar_ip_dinamica")
                        .replace("{ip}", &lan.to_string())
                );
            }
            if let Some((ac, dc)) = home::sleep_after_minutes() {
                println!("{}", sleep_text(t, ac, dc));
            }
            Ok(())
        }
        "off" | "apagar" => {
            if let Err(FirewallError::Command(_)) = home::firewall_remove() {
                println!("{}", t.panel("hogar_no_admin"));
            }
            ledger.set_setting(SETTING_HOME_MODE, "0")?;
            ledger.set_setting(SETTING_HOME_IP, "")?;
            // The uninstaller runs `hogar off --desinstalando`: the record says who asked.
            let who = if opts.has("desinstalando") {
                // Only when the program is leaving: the rules Windows wrote by
                // itself for this binary point at a file that is about to
                // disappear, and they were still there after uninstalling on the
                // test machine. Turning Home Mode off is not the same thing and
                // must not touch them, or Windows would ask again next time.
                let _ = home::firewall_remove_program_rules();
                "desinstalador"
            } else {
                "terminal"
            };
            ledger.record_change(now_ms(), ChangeKind::HogarOff, who, "")?;
            println!("{}", t.cli("hogar.off.hecho"));
            Ok(())
        }
        _ => status(&ledger),
    }
}

/// The sleep warning of Home Mode: a sleeping guardian leaves the phones
/// without DNS. 0 minutes means the system never suspends itself.
fn sleep_text(t: &guardiana_core::i18n::Texts, ac: u32, dc: u32) -> String {
    if ac == 0 && dc == 0 {
        return t.panel("hogar_suspension_nunca").to_owned();
    }
    let value = |n: u32| {
        if n == 0 {
            t.panel("hogar_suspension_nunca_valor").to_owned()
        } else {
            t.panel("hogar_suspension_valor")
                .replace("{n}", &n.to_string())
        }
    };
    t.panel("hogar_suspension")
        .replace("{ac}", &value(ac))
        .replace("{dc}", &value(dc))
}
