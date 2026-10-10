//! `guardiana dns`: show, apply (with explicit consent) and restore the
//! system DNS change of brief §4. In 1.0 the panel asks for consent on
//! first start; this command exists so the change can be tested on a
//! Windows or Linux machine before the panel exists (decision 20).

use std::error::Error;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

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

/// `--restore` when the extract cannot be opened: the copy from its plain file, put back, and the
/// file removed once it really was. `None` when there is no such copy (the caller then reports
/// why the extract would not open).
fn restore_from_file(opts: &Opts) -> Option<Result<(), Box<dyn Error>>> {
    let t = i18n::current();
    let db = opts
        .get("db")
        .map_or_else(paths::ledger_path, std::path::PathBuf::from);
    let json = guardiana_core::copia_junto_al_extracto(&db, SETTING_BACKUP)?;
    let backup: Backup = serde_json::from_str(&json).ok()?;
    let _cambio = sysdns::un_cambio_a_la_vez();
    println!(
        "{}",
        t.cli("dns.restaurando")
            .replace("{n}", &backup.interfaces.len().to_string())
    );
    Some(match sysdns::restore(&backup) {
        Ok(()) => {
            for (_, archivo) in guardiana_core::COPIAS_EN_ARCHIVO {
                if let Some(dir) = db.parent() {
                    let _ = std::fs::remove_file(dir.join(archivo));
                }
            }
            println!("{}", t.cli("dns.restaurado"));
            Ok(())
        }
        Err(e) if e.falta_administrador() => Err(t.cli("dns.sin_admin").into()),
        Err(e) => Err(Box::new(e)),
    })
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

/// Where the machine is pointed, and where the guardian is asked before that happens.
const GUARDIAN: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 53);

/// How long `--apply` waits for the guardian to answer on 127.0.0.1:53. The Windows installer
/// runs this command right after starting the service, which may still be opening its port.
const ESPERA_GUARDIAN: Duration = Duration::from_secs(10);

/// What `dns --apply --yes` does at each step, from what it saw. Kept apart from the system
/// calls so the decision is tested without touching the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Paso {
    /// Nothing that is Guardiana answers on 127.0.0.1:53: nothing is touched, no copy taken.
    NoTocar,
    /// The guardian answers: take the copy and point the machine at it.
    Cambiar,
    /// After the change a query made by the system reached the guardian: keep it.
    Mantener,
    /// After the change no query made by the system reached it: everything goes back and the
    /// copy is dropped, on every system.
    Deshacer,
}

/// Before anything changes. Pointing the machine at 127.0.0.1 with nobody answering there is a
/// machine without names (21 Sep 2026, on the responsible's own Mac).
pub(crate) fn antes_de_cambiar(guardian_contesta: bool) -> Paso {
    if guardian_contesta {
        Paso::Cambiar
    } else {
        Paso::NoTocar
    }
}

/// After the change. Until this check ran everywhere, only Windows asked, and even there a "no"
/// only printed a warning and kept the change.
pub(crate) fn despues_de_cambiar(sistema_llega: bool) -> Paso {
    if sistema_llega {
        Paso::Mantener
    } else {
        Paso::Deshacer
    }
}

/// Whether Guardiana itself answers on 127.0.0.1:53, asked directly (not through the system's
/// resolver, which is what is about to change), for up to `espera`.
fn guardian_listening(espera: Duration) -> bool {
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return false;
    };
    rt.block_on(async {
        let hasta = Instant::now() + espera;
        loop {
            if guardiana_dns::probe::guardian_answers(GUARDIAN, Duration::from_secs(1)).await {
                return true;
            }
            if Instant::now() >= hasta {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    })
}

/// Whether a name asked through the system's resolver now arrives at Guardiana. Asked twice:
/// a network stack may take a moment to use servers it was just given.
fn system_reaches_guardian() -> bool {
    for pausa in [800, 2000] {
        std::thread::sleep(Duration::from_millis(pausa));
        if sysdns::system_reaches_guardian(Duration::from_secs(4)) {
            return true;
        }
    }
    false
}

pub fn run(opts: &Opts) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    let ledger = match open_or_create(opts) {
        Ok(l) => l,
        // An extract that will not open must not keep the machine without its DNS: undoing
        // (by hand, or the uninstaller) works from the plain copy beside it (review of
        // 10 Oct 2026, system serious 1).
        Err(e) if opts.has("restore") => return restore_from_file(opts).ok_or(e)?,
        Err(e) => return Err(e),
    };

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
        if antes_de_cambiar(guardian_listening(ESPERA_GUARDIAN)) == Paso::NoTocar {
            return Err(t.cli("dns.guardiana_no_contesta").into());
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
        let copia = serde_json::to_string(&backup)?;
        ledger.set_setting(SETTING_BACKUP, &copia)?;
        if let Err(e) = sysdns::apply(&backup, GUARDIAN.ip()) {
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
        // Asking is the only way to know the queries really arrive (27 Sep 2026): the settings
        // can say "127.0.0.1 first" while they go elsewhere.
        if despues_de_cambiar(system_reaches_guardian()) == Paso::Deshacer {
            // The service's watchdog points the machine at Guardiana again while the copy
            // exists, so the copy goes first, and comes back if undoing fails (as `--restore`).
            ledger.set_setting(SETTING_BACKUP, "")?;
            if let Err(e) = sysdns::restore(&backup) {
                ledger.set_setting(SETTING_BACKUP, &copia)?;
                println!("{}", t.cli("dns.deshacer_pendiente"));
                return Err(Box::new(e));
            }
            return Err(t.cli("dns.camino_no_deshecho").into());
        }
        if cfg!(target_os = "windows") {
            println!("{}", t.cli("dns.camino_ok"));
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Nothing answering on 127.0.0.1:53 means nothing is touched; a guardian that answers lets
    /// the change go ahead.
    #[test]
    fn without_a_guardian_answering_nothing_changes() {
        assert_eq!(antes_de_cambiar(false), Paso::NoTocar);
        assert_eq!(antes_de_cambiar(true), Paso::Cambiar);
    }

    /// A change the system's own queries do not follow is undone, whatever the system.
    #[test]
    fn a_change_that_does_not_reach_the_guardian_is_undone() {
        assert_eq!(despues_de_cambiar(false), Paso::Deshacer);
        assert_eq!(despues_de_cambiar(true), Paso::Mantener);
    }

    /// The address asked before the change is the one the machine is pointed at.
    #[test]
    fn the_guardian_asked_is_the_one_pointed_at() {
        assert_eq!(GUARDIAN.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert_eq!(GUARDIAN.port(), 53);
    }
}
