//! `guardiana panel`: open the panel in the browser with the session token
//! from the token file (the launcher of brief §8).

use std::error::Error;

use guardiana_core::{i18n, paths};

use crate::args::Opts;

pub fn run(_opts: &Opts) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    let path = paths::data_dir().join(guardiana_panel::TOKEN_FILE);
    let token = std::fs::read_to_string(&path).map_err(|_| {
        t.cli("panel.sin_token")
            .replace("{ruta}", &path.display().to_string())
    })?;
    let url = format!("http://127.0.0.1:7443/?t={}", token.trim());
    println!("{}", t.cli("panel.abriendo").replace("{url}", &url));
    crate::engine::open_in_browser(&url);
    Ok(())
}
