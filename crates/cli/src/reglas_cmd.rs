//! `guardiana reglas [deshacer ID | deshacer-hoy]`: list rules and undo them
//! from the terminal (brief §6). Creating rules is a panel action.

use std::error::Error;

use guardiana_core::i18n;
use guardiana_core::time::{now_ms, rfc3339_utc, DAY_MS};

use crate::args::Opts;
use crate::ledger_cmd;

pub fn run(opts: &Opts) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    let mut ledger = ledger_cmd::open(opts)?;
    let words = opts.positional();
    match (words.first().map(String::as_str), words.get(1)) {
        (Some("deshacer"), Some(id)) => {
            let ok = ledger.undo_rule(id.parse()?, now_ms())?;
            println!(
                "{}",
                t.cli(if ok {
                    "reglas.deshecha"
                } else {
                    "reglas.no_encontrada"
                })
            );
            Ok(())
        }
        (Some("deshacer-hoy"), _) => {
            let now = now_ms();
            let n = ledger.undo_rules_since(now - DAY_MS, now)?;
            println!(
                "{}",
                t.cli("reglas.deshechas_hoy").replace("{n}", &n.to_string())
            );
            Ok(())
        }
        _ => {
            let now = now_ms();
            let rules = ledger.rules()?;
            if rules.is_empty() {
                println!("{}", t.cli("reglas.ninguna"));
                return Ok(());
            }
            println!("{}", t.cli("reglas.cabecera"));
            for r in rules {
                let estado = if r.is_active(now) {
                    t.cli("reglas.activa")
                } else {
                    t.cli("reglas.deshecha_estado")
                };
                println!(
                    "  #{}  {}  {}  {}:{}  {}  {}  {}",
                    r.id,
                    estado,
                    r.scope,
                    r.match_kind,
                    r.pattern,
                    r.action,
                    r.device_id.as_deref().unwrap_or("-"),
                    rfc3339_utc(r.created_at)
                );
            }
            Ok(())
        }
    }
}
