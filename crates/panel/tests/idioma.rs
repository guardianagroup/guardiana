//! 1.0.46 (radiografias-globo): the program's language is one setting of the service, behind the
//! session token (R10), and every launch path opens on En vivo, the globe (R11). Through the real
//! HTTP panel on a loopback port.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::net::SocketAddr;
use std::path::PathBuf;

use guardiana_core::{Hash, Ledger};
use guardiana_panel::{start, Config, RuntimeInfo};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

async fn pide(
    addr: SocketAddr,
    token: &str,
    metodo: &str,
    path: &str,
    body: &str,
) -> (u16, String) {
    let mut s = TcpStream::connect(addr).await.unwrap();
    let tk = if token.is_empty() {
        String::new()
    } else {
        format!("X-Guardiana-Token: {token}\r\n")
    };
    let req = format!(
        "{metodo} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n{tk}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes()).await.unwrap();
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).await.unwrap();
    let raw = String::from_utf8_lossy(&raw).into_owned();
    let (head, cuerpo) = raw.split_once("\r\n\r\n").unwrap();
    let status: u16 = head.split_whitespace().nth(1).unwrap().parse().unwrap();
    (status, cuerpo.to_owned())
}

fn genesis() -> Hash {
    Hash::of(b"panel language test key")
}

fn prepare(tag: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("guardiana-idioma-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    (dir.join("ledger.db"), dir.join("panel.token"))
}

#[tokio::test]
async fn one_language_for_the_whole_program_kept_by_the_service() {
    let (db, token_path) = prepare("api");
    drop(Ledger::open(&db, genesis()).unwrap());
    let running = start(Config {
        listen: vec!["127.0.0.1:0".parse().unwrap()],
        optional_listen: Vec::new(),
        db_path: db.clone(),
        genesis: genesis(),
        token_path,
        extra_hosts: Vec::new(),
        info: RuntimeInfo::default(),
    })
    .await
    .expect("the panel starts");
    let addr = running.addrs[0];
    let tk = running.token.clone();

    // Without the session token, nothing.
    let (st, _) = pide(addr, "", "GET", "/api/ajustes/idioma", "").await;
    assert_eq!(st, 401);
    let (st, _) = pide(addr, "", "PUT", "/api/ajustes/idioma", r#"{"idioma":"es"}"#).await;
    assert_eq!(st, 401);

    // English until the person chooses.
    let (st, b) = pide(addr, &tk, "GET", "/api/ajustes/idioma", "").await;
    assert_eq!(st, 200, "{b}");
    let v: serde_json::Value = serde_json::from_str(&b).unwrap();
    assert_eq!(v["idioma"], "en");
    assert_eq!(v["elegido"], false);

    // Chosen once, the same answer for every page (and for ZERO, which asks the same route).
    let (st, b) = pide(
        addr,
        &tk,
        "PUT",
        "/api/ajustes/idioma",
        r#"{"idioma":"es"}"#,
    )
    .await;
    assert_eq!(st, 200, "{b}");
    let (_, b) = pide(addr, &tk, "GET", "/api/ajustes/idioma", "").await;
    let v: serde_json::Value = serde_json::from_str(&b).unwrap();
    assert_eq!(v["idioma"], "es");
    assert_eq!(v["elegido"], true);

    // Only the three languages.
    let (st, _) = pide(
        addr,
        &tk,
        "PUT",
        "/api/ajustes/idioma",
        r#"{"idioma":"fr"}"#,
    )
    .await;
    assert_eq!(st, 400);
    let (_, b) = pide(addr, &tk, "GET", "/api/ajustes/idioma", "").await;
    assert!(b.contains("\"es\""), "{b}");

    // R11: «/» (where every launch path lands) is En vivo, the globe; the 60-second snapshot
    // keeps its own address.
    let (st, html) = pide(addr, &tk, "GET", "/", "").await;
    assert_eq!(st, 200);
    assert!(
        html.contains("data-page=\"por-dentro\""),
        "the program opens on the globe"
    );
    let (st, html) = pide(addr, &tk, "GET", "/foto", "").await;
    assert_eq!(st, 200);
    assert!(!html.contains("data-page=\"por-dentro\""));
}
