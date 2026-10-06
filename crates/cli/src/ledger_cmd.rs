//! `guardiana ledger`: show, check (`--check`), wipe (`--wipe`) and the
//! program's own outbound connections (`--self`), brief §3.

use std::error::Error;
use std::io::{BufRead, Write};

use guardiana_core::i18n;
use guardiana_core::time::rfc3339_utc;
use guardiana_core::{identity, paths, ChainFault, EventFilter, Ledger};

use crate::args::Opts;
use crate::show;

pub(crate) fn open(opts: &Opts) -> Result<Ledger, Box<dyn Error>> {
    let t = i18n::current();
    let db = opts
        .get("db")
        .map_or_else(paths::ledger_path, std::path::PathBuf::from);
    if !db.exists() {
        return Err(t
            .cli("ledger.sin_extracto")
            .replace("{ruta}", &db.display().to_string())
            .into());
    }
    Ok(Ledger::open(&db, identity::genesis())?)
}

pub fn run(opts: &Opts) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    let mut ledger = open(opts)?;

    if opts.has("check") {
        let r = ledger.check()?;
        match r.first_fault {
            None if r.checked == 0 && r.pruned == 0 => println!("{}", t.cli("ledger.vacio")),
            None if r.anchor_is_genesis => {
                println!("{}", t.cli_n("ledger.ok", r.checked as i64));
            }
            None => println!(
                "{}",
                t.cli("ledger.ok_recortado")
                    .replace("{n}", &r.checked.to_string())
                    .replace("{p}", &r.pruned.to_string())
            ),
            Some((id, fault)) => {
                let reason = match fault {
                    ChainFault::BrokenLink { .. } => t.cli("ledger.fallo.enlace"),
                    ChainFault::AlteredRow { .. } => t.cli("ledger.fallo.alterado"),
                    ChainFault::Unreadable(_) => t.cli("ledger.fallo.ilegible"),
                    // The newest rows are gone, or nothing says where the chain ended (review of
                    // 1 Oct 2026, entry 17, and of 5 Oct 2026).
                    ChainFault::Truncated { .. } => t.cli("ledger.fallo.recortado"),
                    ChainFault::HeadMissing => t.cli("ledger.fallo.sin_cabeza"),
                };
                println!(
                    "{}",
                    t.cli("ledger.fallo")
                        .replace("{id}", &id.to_string())
                        .replace("{motivo}", reason)
                );
                return Err("".into());
            }
        }
        return Ok(());
    }

    if opts.has("wipe") {
        print!("{}", t.cli("ledger.wipe.confirmar"));
        std::io::stdout().flush()?;
        let mut line = String::new();
        std::io::stdin().lock().read_line(&mut line)?;
        // The word the question asks for in the language it was asked (BORRAR, DELETE, APAGAR);
        // until 1.0.1 only BORRAR worked, whatever the question said.
        let dicha = line.trim().to_uppercase();
        if dicha == t.cli("ledger.wipe.palabra") || dicha == "BORRAR" {
            ledger.wipe()?;
            println!("{}", t.cli("ledger.wipe.hecho"));
        } else {
            println!("{}", t.cli("ledger.wipe.cancelado"));
        }
        return Ok(());
    }

    if opts.has("self") {
        let rows = ledger.outbound()?;
        if rows.is_empty() {
            println!("{}", t.cli("ledger.self.vacio"));
        } else {
            println!("{}", t.cli("ledger.self.cabecera"));
            for o in rows {
                println!(
                    "{}  {:<9}  {:<40}  {} bytes",
                    rfc3339_utc(o.ts),
                    o.purpose,
                    o.host,
                    o.bytes
                );
            }
        }
        return Ok(());
    }

    let last: u32 = opts.get("last").and_then(|s| s.parse().ok()).unwrap_or(50);
    let events = ledger.events(&EventFilter::default())?;
    // The last `last` of the rows this listing may show. Counting every stored row here, as
    // before, skipped past the end whenever a device that does not share had rows, and the
    // command printed nothing (review of 5 Oct 2026, privacy item 3).
    let skip = events
        .len()
        .saturating_sub(usize::try_from(last).unwrap_or(usize::MAX));
    let names: std::collections::HashMap<String, Option<String>> = ledger
        .devices()?
        .into_iter()
        .map(|d| (d.id, d.name))
        .collect();
    if events.is_empty() {
        println!("{}", t.cli("ledger.vacio"));
        return Ok(());
    }
    println!("{}", t.cli("ledger.cabecera"));
    for e in events.iter().skip(skip) {
        let name = names.get(&e.device_id).and_then(|n| n.as_deref());
        println!("{}", show::line(t, e, name));
    }
    Ok(())
}
