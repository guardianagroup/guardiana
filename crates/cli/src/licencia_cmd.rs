//! `guardiana licencia [clave CLAVE | comprobar]` (brief §9, decisions 52 and 53).

use std::error::Error;

use guardiana_core::time::{now_ms, rfc3339_utc};
use guardiana_core::{i18n, paths};
use guardiana_license::{self as license, Comprobacion, Plan};

use crate::args::Opts;
use crate::dns_cmd;

/// Licence errors in the user's words.
pub(crate) fn plain(t: &i18n::Texts, e: &license::Error) -> String {
    match e {
        license::Error::Malformed(_) => t.panel("licencia_err_formato").to_owned(),
        license::Error::KeyRejected(why) => t.panel("licencia_err_clave").replace("{motivo}", why),
        license::Error::Network(why) => t.panel("licencia_err_red").replace("{motivo}", why),
        license::Error::AlreadyPlus => t.panel("licencia_ya_plus").to_owned(),
        license::Error::Ledger(err) => err.to_string(),
    }
}

/// The plan in force, in one sentence.
pub(crate) fn describe(t: &i18n::Texts, s: &license::Status) -> String {
    let day = |ms: i64| rfc3339_utc(ms)[..10].to_owned();
    match &s.plan {
        Plan::Prueba {
            termina,
            dias_restantes,
            ..
        } => t
            .panel("licencia_prueba")
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
                            .replace("{limite}", &day(caduca_ms.unwrap_or(*next))),
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
        println!("{}", t.panel("licencia_prueba_texto"));
    }
    Ok(())
}
