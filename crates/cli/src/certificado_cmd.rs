//! `guardiana certificado` (1.0.45): the «Certificado de flujos» of a month or any period, made
//! here from the ledger and written as three files beside each other: the sealed `.json`, its
//! `.json.sha256` (for `sha256sum -c`) and the `.html` to read and print. And
//! `guardiana verify --certificado ARCHIVO`, which rebuilds one from the ledger and compares.

use std::error::Error;
use std::path::PathBuf;

use guardiana_core::i18n;
use guardiana_core::time::{local_offset_min, now_ms};
use guardiana_verify::certificado::{self, Estado, Opciones};
use guardiana_verify::certificado_html;

use crate::args::Opts;
use crate::ledger_cmd;

/// The period the options ask for: `--mes AAAA-MM`, or `--desde AAAA-MM-DD --hasta AAAA-MM-DD`,
/// or the last whole month.
fn periodo(opts: &Opts, zona: i64, ahora: i64) -> Option<(i64, i64)> {
    match (opts.get("desde"), opts.get("hasta"), opts.get("mes")) {
        (Some(d), Some(h), _) => certificado::periodo_dias(d, h, zona),
        (None, None, Some(m)) => certificado::periodo_mes(m, zona),
        (None, None, None) => {
            certificado::periodo_mes(&certificado::mes_anterior(ahora, zona), zona)
        }
        _ => None,
    }
}

pub fn run(opts: &Opts) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    let ledger = ledger_cmd::open(opts)?;
    let zona = local_offset_min();
    let ahora = now_ms();
    let (desde, hasta) =
        periodo(opts, zona, ahora).ok_or_else(|| t.cli("certificado.periodo_malo").to_owned())?;
    let op = Opciones {
        desde,
        hasta,
        zona_min: zona,
        dominios: opts.has("dominios"),
        solo_empresa: opts.get("empresa").map(str::to_owned),
    };
    let c = certificado::generar(&ledger, &op, ahora)?;
    let json = certificado::a_json(&c);
    let sello = certificado::sello(json.as_bytes());
    let dir = opts
        .get("out")
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    let nombre_json = certificado::nombre_archivo(&c, "json");
    let nombre_html = certificado::nombre_archivo(&c, "html");
    std::fs::write(dir.join(&nombre_json), &json)?;
    std::fs::write(
        dir.join(format!("{nombre_json}.sha256")),
        certificado::linea_sha256(&sello, &nombre_json),
    )?;
    std::fs::write(
        dir.join(&nombre_html),
        certificado_html::html(t, &c, &sello),
    )?;
    println!(
        "{}",
        t.cli("certificado.hecho")
            .replace("{ruta}", &dir.join(&nombre_json).display().to_string())
            .replace("{sello}", &sello)
            .replace("{n}", &c.contenido.totales.empresas.to_string())
    );
    Ok(())
}

/// `guardiana verify --certificado ARCHIVO`.
pub fn verificar(opts: &Opts, archivo: &str) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    let ledger = ledger_cmd::open(opts)?;
    let bytes = std::fs::read(archivo)?;
    let r = certificado::comprobar(&bytes, &ledger);
    if opts.has("json") {
        println!("{}", serde_json::to_string_pretty(&r)?);
    } else {
        let clave = match r.estado {
            Estado::Coincide => "certificado.verificar.coincide",
            Estado::MismasFilasOtraClasificacion => "certificado.verificar.otra_clasificacion",
            Estado::FilasRecortadas => "certificado.verificar.recortadas",
            Estado::NoCoincide => "certificado.verificar.no_coincide",
            Estado::FormatoDesconocido => "certificado.verificar.formato",
            Estado::Ilegible => "certificado.verificar.ilegible",
        };
        println!(
            "{}",
            t.cli(clave)
                .replace("{version}", &r.programa_certificado)
                .replace("{ahora}", &r.programa_ahora)
        );
        println!(
            "{}",
            t.cli("certificado.verificar.sello")
                .replace("{sello}", &r.sello)
        );
        if !r.cadena_ok {
            println!("{}", t.cli("certificado.verificar.cadena_mal"));
        }
    }
    if r.estado == Estado::Coincide {
        Ok(())
    } else {
        Err("".into())
    }
}
