//! `/api/plan/llamadas` (comunicaciones-1.0.46): `guardiana comunicaciones` runs as the person
//! and cannot read the service's folder on macOS, so it asks the panel whether the relays are
//! part of the plan. One bit, no key, only to this computer.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::net::SocketAddr;
use std::path::PathBuf;

use guardiana_core::{Hash, Ledger};
use guardiana_panel::{start, Config, RuntimeInfo};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

async fn pide(addr: SocketAddr, path: &str) -> (u16, String) {
    let mut s = TcpStream::connect(addr).await.unwrap();
    let req = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
    s.write_all(req.as_bytes()).await.unwrap();
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).await.unwrap();
    let texto = String::from_utf8_lossy(&raw).into_owned();
    let status: u16 = texto.split_whitespace().nth(1).unwrap().parse().unwrap();
    let cuerpo = texto
        .split("\r\n\r\n")
        .nth(1)
        .unwrap_or_default()
        .to_owned();
    (status, cuerpo)
}

fn genesis() -> Hash {
    Hash::of(b"panel plan llamadas test key")
}

#[tokio::test]
async fn the_relays_bit_is_answered_without_the_key_and_nothing_else_is() {
    let dir: PathBuf =
        std::env::temp_dir().join(format!("guardiana-plan-llamadas-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("ledger.db");
    drop(Ledger::open(&db, genesis()).unwrap());
    let running = start(Config {
        listen: vec!["127.0.0.1:0".parse().unwrap()],
        optional_listen: Vec::new(),
        db_path: db.clone(),
        genesis: genesis(),
        token_path: dir.join("panel.token"),
        extra_hosts: Vec::new(),
        info: RuntimeInfo::default(),
    })
    .await
    .expect("the panel starts");
    let addr = running.addrs[0];

    let (st, cuerpo) = pide(addr, "/api/plan/llamadas").await;
    assert_eq!(st, 200, "{cuerpo}");
    // Only the bit: no level name, no history, no link, no key.
    assert!(
        cuerpo.contains("\"reles\":true") || cuerpo.contains("\"reles\":false"),
        "{cuerpo}"
    );
    assert!(!cuerpo.contains("nivel"), "{cuerpo}");
    // The full plan still needs the key.
    let (st, _) = pide(addr, "/api/plan").await;
    assert_eq!(st, 401);
    let _ = std::fs::remove_dir_all(&dir);
}
