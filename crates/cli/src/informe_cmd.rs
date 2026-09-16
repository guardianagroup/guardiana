//! `guardiana informe`: the weekly household report in the terminal, with
//! the WhatsApp-ready sentence (brief §8). Real figures, never rounded up.

use std::error::Error;

use guardiana_core::i18n;
use guardiana_core::time::{now_ms, rfc3339_utc};
use guardiana_core::SELF_DEVICE_ID;

use crate::args::Opts;
use crate::ledger_cmd;

pub fn run(opts: &Opts) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    let ledger = ledger_cmd::open(opts)?;
    let w = ledger.week_summary(now_ms())?;
    println!(
        "{}",
        t.cli("informe.cabecera")
            .replace("{desde}", &rfc3339_utc(w.since)[..10])
            .replace("{hasta}", &rfc3339_utc(w.until)[..10])
    );
    for d in &w.devices {
        let name = d.name.clone().unwrap_or_else(|| {
            if d.device_id == SELF_DEVICE_ID {
                t.cli("dispositivo.self").to_owned()
            } else {
                d.device_id.clone()
            }
        });
        println!(
            "  {}",
            t.cli("informe.fila")
                .replace("{nombre}", &name)
                .replace("{consultas}", &d.queries.to_string())
                .replace("{rastreadores}", &d.trackers.to_string())
                .replace("{publicidad}", &d.ads.to_string())
                .replace("{esperados}", &d.expected.to_string())
                .replace("{cortados}", &d.blocked.to_string())
        );
    }
    println!(
        "{}",
        t.cli("informe.total")
            .replace("{consultas}", &w.total.queries.to_string())
            .replace("{rastreadores}", &w.total.trackers.to_string())
            .replace("{publicidad}", &w.total.ads.to_string())
            .replace("{cortados}", &w.total.blocked.to_string())
    );
    println!();
    println!("{}", t.cli("informe.whatsapp"));
    println!(
        "{}",
        t.panel("informe_texto")
            .replace("{dispositivos}", &w.devices.len().to_string())
            .replace("{consultas}", &w.total.queries.to_string())
            .replace("{rastreadores}", &w.total.trackers.to_string())
            .replace("{publicidad}", &w.total.ads.to_string())
            .replace("{cortados}", &w.total.blocked.to_string())
    );
    Ok(())
}
