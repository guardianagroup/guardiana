//! `guardiana observe`: run the engine in the foreground and print every
//! query live as name → category → device (brief §13, day 7 milestone).
//! Stops with Ctrl+C.

use std::error::Error;
use std::net::{IpAddr, SocketAddr};

use crate::args::Opts;
use crate::engine::{self, EngineConfig};

fn parse_upstream(s: &str) -> Result<SocketAddr, Box<dyn Error>> {
    if let Ok(a) = s.parse::<SocketAddr>() {
        return Ok(a);
    }
    let ip: IpAddr = s.parse()?;
    Ok(SocketAddr::new(ip, 53))
}

/// Build the engine configuration from command-line options.
pub fn config_from(opts: &Opts) -> Result<EngineConfig, Box<dyn Error>> {
    let mut cfg = EngineConfig::default_service();
    if let Some(l) = opts.get("listen") {
        cfg.listen = l.parse()?;
    }
    cfg.upstreams = opts
        .all("upstream")
        .iter()
        .map(|s| parse_upstream(s))
        .collect::<Result<_, _>>()?;
    if let Some(db) = opts.get("db") {
        cfg.db = std::path::PathBuf::from(db);
    }
    cfg.panel = !opts.has("no-panel");
    if let Some(p) = opts.get("panel-listen") {
        cfg.panel_listen = p.parse()?;
    }
    cfg.open_browser = opts.has("open");
    cfg.self_test = opts.has("self-test");
    cfg.canary = opts.has("canary");
    cfg.print_events = true;
    Ok(cfg)
}

/// Run the observer until Ctrl+C.
pub fn run(opts: &Opts) -> Result<(), Box<dyn Error>> {
    let cfg = config_from(opts)?;
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(engine::run(cfg, async {
        let _ = tokio::signal::ctrl_c().await;
    }))
}
