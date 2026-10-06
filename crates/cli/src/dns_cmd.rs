//! `guardiana dns`: show, apply (with explicit consent) and restore the
//! system DNS change of brief §4. In 1.0 the panel asks for consent on
//! first start; this command exists so the change can be tested on a
//! Windows or Linux machine before the panel exists (decision 20).

use std::error::Error;
use std::net::{IpAddr, Ipv4Addr};

use guardiana_core::time::{now_ms, rfc3339_utc};
use guardiana_core::{i18n, identity, paths, Ledger};
use guardiana_service::sysdns::{self, Backup, SETTING_BACKUP, SETTING_BACKUP_APARCADA};

use crate::args::Opts;

pub(crate) fn open_or_create(opts: &Opts) -> Result<Ledger, Box<dyn Error>> {
    let db = opts
        .get("db")
        .map_or_else(paths::ledger_path, std::path::PathBuf::from);
    if let Some(parent) = db.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ledger::open(&db, identity::genesis()).map_err(|e| {
        // The service runs as root and the extract is its file, so a person who
        // types `guardiana dns` without sudo gets "unable to open database file",
        // which explains nothing. If the file is there, the answer is always the
        // same and it fits in one sentence.
        if paths::unreadable_here(&db) {
            i18n::current().cli("cli.extracto.sin_permiso").into()
        } else {
            Box::new(e) as Box<dyn Error>
        }
    })
}

/// Whether the trial or the subscription is over, so the program has stood aside and nothing
/// answers on 127.0.0.1. Unknown counts as "not over", like the service does.
pub(crate) fn apartado(ledger: &Ledger) -> bool {
    let Ok(secret) =
        guardiana_panel::load_or_create_token(&paths::data_dir().join(guardiana_panel::TOKEN_FILE))
    else {
        return false;
    };
    guardiana_license::status(ledger, &secret, now_ms()).is_ok_and(|s| !s.puede_funcionar)
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
    sysdns::sole_or_secondary_key()
}

pub fn run(opts: &Opts) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    let ledger = open_or_create(opts)?;

    if opts.has("apply") {
        if stored_backup(&ledger)?.is_some() {
            println!("{}", t.cli("dns.ya_aplicado"));
            return Ok(());
        }
        // Stood aside, nothing answers on 127.0.0.1: pointing the machine there would leave it
        // without names. The panel already refused (G6, 28 Sep 2026); the terminal, and with it
        // the installer of a program reinstalled after the trial, did not until 1.0.2.
        if apartado(&ledger) {
            println!("{}", t.panel("caducado_dns"));
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
            Err(e) if e.falta_administrador() => return Err(t.cli("dns.sin_admin").into()),
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
            if e.falta_administrador() {
                // Refused before anything changed: the copy is not kept either, or the service
                // would apply it a minute later and do what the person was just told it could
                // not (review of 5 Oct 2026, second pass).
                if sysdns::guardian_still_set() == Some(false)
                    && ledger.set_setting(SETTING_BACKUP, "").is_ok()
                {
                    return Err(t.cli("dns.sin_admin").into());
                }
                println!("{}", t.cli("dns.sin_admin"));
            }
            // A change that failed half-way is rolled back; the copy is cleared only when the
            // rollback really happened. Until the review of 1 Oct 2026 it was cleared either
            // way, and a rollback that failed on one interface left the machine half pointed at
            // Guardiana with "nothing to undo" on the screen.
            if sysdns::restore(&backup).is_ok() {
                ledger.set_setting(SETTING_BACKUP, "")?;
            } else {
                println!("{}", t.cli("dns.deshacer_pendiente"));
            }
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
            // The Windows installer runs this command; the record says who asked.
            if opts.has("instalador") {
                guardiana_core::ChangeWho::Instalador
            } else {
                guardiana_core::ChangeWho::Terminal
            },
            &backup
                .original_servers()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", "),
        )?;
        // Asking is the only way to know the queries really arrive (27 Sep 2026). The installer
        // runs this command right after starting the service, so the resolver is there.
        if cfg!(target_os = "windows") {
            std::thread::sleep(std::time::Duration::from_millis(800));
            let llega = sysdns::system_reaches_guardian(std::time::Duration::from_secs(4));
            println!(
                "{}",
                t.cli(if llega {
                    "dns.camino_ok"
                } else {
                    "dns.camino_no"
                })
            );
        }
        return Ok(());
    }

    if opts.has("restore") {
        // Undoing by hand is also saying no to the copy parked when the trial ended: it is not
        // applied again when the licence comes back.
        let aparcada = ledger
            .setting(SETTING_BACKUP_APARCADA)?
            .is_some_and(|v| !v.is_empty());
        let Some(backup) = stored_backup(&ledger)? else {
            if aparcada {
                ledger.set_setting(SETTING_BACKUP_APARCADA, "")?;
                println!("{}", t.cli("dns.aparcada_borrada"));
            } else {
                println!("{}", t.cli("dns.no_hay_copia"));
            }
            return Ok(());
        };
        println!(
            "{}",
            t.cli("dns.restaurando")
                .replace("{n}", &backup.interfaces.len().to_string())
        );
        // El vigilante del servicio reaplica el cambio mientras exista la copia
        // (engine::reapply_dns_if_dropped). Si se deshace primero y se borra la
        // copia después, hay unos segundos en los que el vigilante ve el DNS
        // «caído» y lo vuelve a poner: medido en la VM el 16 sep 2026, dejaba el
        // DNS global de systemd-resolved apuntando a Guardiana después de un
        // restaurado que decía «exactamente como estaba». Se le quita la razón
        // antes de tocar nada, y si deshacer falla se devuelve la copia: perderla
        // dejaría el equipo apuntando a Guardiana sin manera de volver.
        ledger.set_setting(SETTING_BACKUP, "")?;
        if let Err(e) = sysdns::restore(&backup) {
            ledger.set_setting(SETTING_BACKUP, &serde_json::to_string(&backup)?)?;
            if e.falta_administrador() {
                return Err(t.cli("dns.sin_admin").into());
            }
            return Err(Box::new(e));
        }
        if aparcada {
            ledger.set_setting(SETTING_BACKUP_APARCADA, "")?;
        }
        ledger.record_change(
            now_ms(),
            guardiana_core::ChangeKind::DnsOff,
            if opts.has("desinstalando") {
                guardiana_core::ChangeWho::Desinstalador
            } else {
                guardiana_core::ChangeWho::Terminal
            },
            "",
        )?;
        println!("{}", t.cli("dns.restaurado"));
        return Ok(());
    }

    status(&ledger)
}
