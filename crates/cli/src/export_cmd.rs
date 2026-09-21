//! `guardiana export`: CSV or JSON of the events, with the filters of the
//! `/extracto` page (brief §3, §8).

use std::error::Error;
use std::io::Write;

use guardiana_core::{i18n, write_csv, write_json, EventFilter};

use crate::args::Opts;
use crate::ledger_cmd;

pub fn run(opts: &Opts) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    let ledger = ledger_cmd::open(opts)?;
    let filter = EventFilter {
        qname: opts
            .get("nombre")
            .or_else(|| opts.get("name"))
            .map(String::from),
        device_id: opts.get("device").map(String::from),
        category: opts.get("category").map(str::parse).transpose()?,
        signal: opts.get("signal").map(str::parse).transpose()?,
        verdict: opts.get("verdict").map(str::parse).transpose()?,
        since: opts.get("since").and_then(|s| s.parse().ok()),
        until: opts.get("until").and_then(|s| s.parse().ok()),
        limit: opts.get("limit").and_then(|s| s.parse().ok()),
    };
    let events = ledger.events(&filter)?;
    let json = match (opts.has("json"), opts.has("csv")) {
        (true, false) => true,
        (false, _) => false,
        (true, true) => return Err(t.cli("export.formato").into()),
    };
    let mut buf = Vec::new();
    if json {
        write_json(&events, &mut buf)?;
    } else {
        write_csv(&events, &mut buf)?;
    }
    match opts.get("out") {
        Some(path) => {
            std::fs::write(path, &buf)?;
            eprintln!(
                "{}",
                t.cli("export.hecho")
                    .replace("{n}", &events.len().to_string())
                    .replace("{ruta}", path)
            );
        }
        None => std::io::stdout().write_all(&buf)?,
    }
    Ok(())
}
