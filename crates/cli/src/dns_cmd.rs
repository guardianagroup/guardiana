//! `guardiana dns`: show, apply (with explicit consent) and restore the
//! system DNS change of brief §4. In 1.0 the panel asks for consent on
//! first start; this command exists so the change can be tested on a
//! Windows or Linux machine before the panel exists (decision 20).

use std::error::Error;
use std::net::{IpAddr, Ipv4Addr};

use guardiana_core::time::{now_ms, rfc3339_utc};
use guardiana_core::{i18n, identity, paths, Ledger};
use guardiana_service::sysdns::{self, Backup, SETTING_BACKUP};

use crate::args::Opts;

pub(crate) fn open_or_create(opts: &Opts) -> Result<Ledger, Box<dyn Error>> {
    let db = opts
        .get("db")
        .map_or_else(paths::ledger_path, std::path::PathBuf::from);
    if let Some(parent) = db.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(Ledger::open(&db, identity::genesis())?)
}

fn stored_backup(ledger: &Ledger) -> Result<Option<Backup>, Box<dyn Error>> {
    match ledger.setting(SETTING_BACKUP)? {
        // Restore leaves the key with an empty value: no backup.
        Some(json) if !json.trim().is_empty() => Ok(Some(serde_json::from_str(&json)?)),
        _ => Ok(None),
    }
}

fn status(ledger: &Ledger) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    match sysdns::current_resolvers() {
        Some(c) => println!(
            "{}",
            t.cli("dns.estado.sistema")
                .replace(
                    "{servers}",
                    &c.servers
                        .iter()
                        .map(|s| s.ip().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
                .replace("{fuente}", &format!("{:?}", c.source))
        ),
        None => println!("{}", t.cli("dns.estado.sin_resolutor")),
    }
    if sysdns::guardian_is_primary() == Some(true) {
        println!("{}", t.cli("dns.estado.guardiana_primero"));
    }
    match stored_backup(ledger)? {
        Some(b) => println!(
            "{}",
            t.cli("dns.estado.aplicado")
                .replace("{fecha}", &rfc3339_utc(b.taken_at))
        ),
        None => println!("{}", t.cli("dns.estado.no_aplicado")),
    }
    Ok(())
}

/// Which sentence tells the truth on this machine about what happens to the
/// resolver that was there before.
pub(crate) fn sole_or_secondary_key() -> &'static str {
    if sysdns::guardian_is_sole_resolver() {
        "dns.solo_guardiana"
    } else {
        "dns.reserva_secundario"
    }
}

pub fn run(opts: &Opts) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    let ledger = open_or_create(opts)?;

    if opts.has("apply") {
        if stored_backup(&ledger)?.is_some() {
            println!("{}", t.cli("dns.ya_aplicado"));
            return Ok(());
        }
        if !opts.has("yes") {
            println!("{}", t.cli("dns.consentimiento"));
            println!("{}", t.cli(sole_or_secondary_key()));
            println!("{}", t.cli("dns.consentimiento_si"));
            return Ok(());
        }
        let backup = match sysdns::snapshot(now_ms()) {
            Ok(b) => b,
            Err(sysdns::Error::Unsupported) => {
                println!("{}", t.cli("dns.no_soportado"));
                return Ok(());
            }
            Err(e) => return Err(Box::new(e)),
        };
        println!(
            "{}",
            t.cli("dns.aplicando")
                .replace("{n}", &backup.interfaces.len().to_string())
                .replace("{metodo}", &format!("{:?}", backup.method))
        );
        // The backup is stored before anything changes, so a crash mid-way is still restorable.
        ledger.set_setting(SETTING_BACKUP, &serde_json::to_string(&backup)?)?;
        let guardian = IpAddr::V4(Ipv4Addr::LOCALHOST);
        if let Err(e) = sysdns::apply(&backup, guardian) {
            let _ = sysdns::restore(&backup);
            ledger.set_setting(SETTING_BACKUP, "")?;
            return Err(Box::new(e));
        }
        println!(
            "{}",
            t.cli("dns.aplicado").replace(
                "{originales}",
                &backup
                    .original_servers()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        );
        println!("{}", t.cli(sole_or_secondary_key()));
        ledger.record_change(
            now_ms(),
            guardiana_core::ChangeKind::DnsOn,
            "terminal",
            &backup
                .original_servers()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", "),
        )?;
        if cfg!(target_os = "windows") {
            println!("{}", t.cli("dns.limite_windows"));
        }
        return Ok(());
    }

    if opts.has("restore") {
        let Some(backup) = stored_backup(&ledger)? else {
            println!("{}", t.cli("dns.no_hay_copia"));
            return Ok(());
        };
        println!(
            "{}",
            t.cli("dns.restaurando")
                .replace("{n}", &backup.interfaces.len().to_string())
        );
        sysdns::restore(&backup)?;
        ledger.set_setting(SETTING_BACKUP, "")?;
        ledger.record_change(now_ms(), guardiana_core::ChangeKind::DnsOff, "terminal", "")?;
        println!("{}", t.cli("dns.restaurado"));
        return Ok(());
    }

    status(&ledger)
}
