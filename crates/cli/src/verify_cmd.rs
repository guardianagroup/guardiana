//! `guardiana verify [--json]` (brief §10).

use std::error::Error;

use crate::args::Opts;

pub fn run(opts: &Opts) -> Result<(), Box<dyn Error>> {
    let report = guardiana_verify::run();
    if opts.has("json") {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("{}", guardiana_verify::render(&report));
    }
    Ok(())
}
