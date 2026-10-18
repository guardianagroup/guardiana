//! `guardiana verify [--json]` (brief §10).

use std::error::Error;

use crate::args::Opts;

pub fn run(opts: &Opts) -> Result<(), Box<dyn Error>> {
    // A «Certificado de flujos» checked against this ledger (1.0.45).
    if let Some(archivo) = opts.get("certificado") {
        return crate::certificado_cmd::verificar(opts, archivo);
    }
    let report = guardiana_verify::run();
    if opts.has("json") {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("{}", guardiana_verify::render(&report));
    }
    Ok(())
}
