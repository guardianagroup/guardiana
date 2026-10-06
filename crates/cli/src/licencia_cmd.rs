//! `guardiana licencia [clave CLAVE | comprobar]` (brief §9, decisions 52 and 53).

use std::error::Error;

use guardiana_core::time::now_ms;
use guardiana_core::{i18n, paths};
use guardiana_license::{self as license, Comprobacion, Plan};

use crate::args::Opts;
use crate::dns_cmd;

/// Licence errors in the user's words.
pub(crate) fn plain(t: &i18n::Texts, e: &license::Error) -> String {
    match e {
        license::Error::Malformed(_) => t.panel("licencia_err_formato").to_owned(),
        license::Error::KeyRejected(why) => t.panel("licencia_err_clave").replace("{motivo}", why),
        // An empty key is refused before anything leaves the machine (review of 1 Oct 2026,
        // entry 26), and the gateway is not blamed for a key it never saw.
        license::Error::EmptyKey => t.panel("licencia_err_clave_vacia").to_owned(),
        license::Error::Network(why) => t.panel("licencia_err_red").replace("{motivo}", why),
        license::Error::AlreadyPlus => t.panel("licencia_ya_plus").to_owned(),
        license::Error::Ledger(err) => err.to_string(),
    }
}

/// The plan in force, in one sentence.
pub(crate) fn describe(t: &i18n::Texts, s: &license::Status) -> String {
    // This machine's own day, not UTC's (review of 5 Oct 2026, licence medium).
    let zona = guardiana_core::time::local_offset_min();
    let day = |ms: i64| guardiana_core::time::local_day(ms, zona);
    match &s.plan {
        Plan::Prueba {
            termina,
            dias_restantes,
            ..
        } => t
            .panel(if *dias_restantes == 1 {
                "licencia_prueba_uno"
            } else {
                "licencia_prueba"
            })
            .replace("{d}", &dias_restantes.to_string())
            .replace("{fecha}", &day(*termina)),
        Plan::PruebaAgotada { termino } => t
            .panel("licencia_prueba_agotada")
            .replace("{fecha}", &day(*termino)),
        Plan::Plus {
            desde,
            titular,
            periodo_dias,
            proxima_comprobacion,
            caduca_ms,
            comprobacion,
            ..
        } => {
            let mut text = t
                .panel("licencia_plus")
                .replace("{origen}", t.panel("licencia_origen_clave"))
                .replace("{fecha}", &day(*desde))
                .replace("{titular}", titular.as_deref().unwrap_or("-"));
            match (periodo_dias, proxima_comprobacion, comprobacion) {
                // A licence bought once is checked, monthly, so a refund ends it, but it is
                // never cut for a check that could not be done: the subscription sentences, with
                // their deadline, would be false for it (review of 1 Oct 2026, entry 13). The
                // panel has the same branch.
                (Some(p), _, _) if *p == license::DE_POR_VIDA => {
                    text.push(' ');
                    text.push_str(t.panel("licencia_de_por_vida"));
                    // The gateway turned the key down: from when it stops, said with its date.
                    if let Some(c) = caduca_ms {
                        text.push(' ');
                        text.push_str(&t.panel("licencia_caduca").replace("{fecha}", &day(*c)));
                    }
                }
                (Some(p), Some(next), Some(c)) => {
                    let key = match c {
                        Comprobacion::AlDia => "licencia_comprobacion_al_dia",
                        Comprobacion::Pendiente => "licencia_comprobacion_pendiente",
                        Comprobacion::Fallida => "licencia_comprobacion_fallida",
                    };
                    text.push(' ');
                    text.push_str(
                        &t.panel(key)
                            .replace(
                                "{periodo}",
                                if *p >= license::YEAR_DAYS {
                                    t.panel("licencia_periodo_anual")
                                } else {
                                    t.panel("licencia_periodo_mensual")
                                },
                            )
                            .replace("{fecha}", &day(*next))
                            // The deadline exists only once a check has failed: before that it
                            // was the date the check fell due, already past (licence item 2).
                            .replace("{limite}", &caduca_ms.map(day).unwrap_or_default()),
                    );
                }
                _ => {
                    if let Some(c) = caduca_ms {
                        text.push(' ');
                        text.push_str(&t.panel("licencia_caduca").replace("{fecha}", &day(*c)));
                    }
                }
            }
            text
        }
        Plan::PlusTerminado { termino, motivo } => t
            .panel(match motivo.as_str() {
                "cancelada" => "licencia_plus_cancelada",
                "rechazada" => "licencia_plus_rechazada",
                _ => "licencia_plus_sin_comprobar",
            })
            .replace("{fecha}", &day(*termino)),
    }
}

pub fn run(opts: &Opts) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    let mut ledger = dns_cmd::open_or_create(opts)?;
    let secret = guardiana_panel::load_or_create_token(
        &paths::data_dir().join(guardiana_panel::TOKEN_FILE),
    )?;
    let words = opts.positional();
    let attempt = match (words.first().map(String::as_str), words.get(1)) {
        (Some("clave"), Some(key)) => {
            license::activate_with_key(&mut ledger, key, &secret, now_ms())
        }
        (Some("comprobar"), _) => match license::check_if_due(&mut ledger, &secret, now_ms()) {
            Ok(Some(s)) => Ok(s),
            Ok(None) => {
                println!("{}", t.panel("licencia_comprobacion_no_toca"));
                license::status(&ledger, &secret, now_ms())
            }
            Err(e) => Err(e),
        },
        _ => license::status(&ledger, &secret, now_ms()),
    };
    let status = match attempt {
        Ok(s) => s,
        Err(e) => return Err(plain(t, &e).into()),
    };
    println!("{}", describe(t, &status));
    if matches!(status.plan, Plan::Prueba { .. }) {
        // The mark is written as best effort: without administrator rights it is not there, and
        // saying it is would be a sentence the program does not keep (review of 1 Oct 2026,
        // entry 24). So it is read back before being named.
        let key = if license::ancla::leer().is_some() {
            "licencia_prueba_texto"
        } else {
            "licencia_prueba_sin_marca"
        };
        println!(
            "{}",
            t.panel(key).replace("{donde}", &license::ancla::donde())
        );
    }
    Ok(())
}
