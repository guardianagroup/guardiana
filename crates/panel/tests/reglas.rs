//! Rules and Guard mode's declared scope take names the way people write them, and a rule on a
//! name the machine needs asks first, also for the whole home (G4 of the review of 28 Sep 2026).
//! Drives the real HTTP panel on a loopback port.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::net::SocketAddr;
use std::path::PathBuf;

use guardiana_core::time::{now_ms, DAY_MS};
use guardiana_core::{Hash, Ledger, SELF_DEVICE_ID};
use guardiana_panel::{start, Config, RuntimeInfo};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

async fn post(addr: SocketAddr, token: &str, path: &str, body: &str) -> (u16, String) {
    let mut s = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nX-Guardiana-Token: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes()).await.unwrap();
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).await.unwrap();
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (head, body) = text.split_once("\r\n\r\n").expect("a complete response");
    let status: u16 = head
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .expect("a status code");
    (status, body.to_owned())
}

fn genesis() -> Hash {
    Hash::of(b"panel rules test key")
}

/// A machine observed for two days, so wide rules and Guard mode are allowed.
fn prepare(tag: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("guardiana-reglas-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("ledger.db");
    let mut l = Ledger::open(&db, genesis()).unwrap();
    l.upsert_device(
        SELF_DEVICE_ID,
        None,
        Some("127.0.0.1"),
        now_ms() - 2 * DAY_MS,
    )
    .unwrap();
    (db, dir.join("panel.token"))
}

#[tokio::test]
async fn rules_and_scopes_take_names_as_people_write_them() {
    let (db, token_path) = prepare("nombres");
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
    let token = running.token.clone();
    let regla = |pattern: &str, kind: &str, confirmed: bool| {
        format!(
            r#"{{"scope":"home","match_kind":"{kind}","pattern":"{pattern}","action":"cortar","confirmed":{confirmed}}}"#
        )
    };

    // A whole address: the rule is for the host a query asks.
    let (st, body) = post(
        addr,
        &token,
        "/api/reglas",
        &regla("https://www.tiktok.com/@alguien?x=1", "domain", false),
    )
    .await;
    assert_eq!(st, 200, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["creada"]["pattern"], "www.tiktok.com", "{body}");

    // A wildcard: everything under the name.
    let (st, body) = post(
        addr,
        &token,
        "/api/reglas",
        &regla("*.ejemplo-juegos.com", "domain", false),
    )
    .await;
    assert_eq!(st, 200, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["creada"]["pattern"], "ejemplo-juegos.com", "{body}");
    assert_eq!(v["creada"]["match_kind"], "suffix", "{body}");

    // Something that is no name is refused, not stored to match nothing.
    let (st, _) = post(
        addr,
        &token,
        "/api/reglas",
        &regla("esto no es un nombre", "domain", false),
    )
    .await;
    assert_eq!(st, 400);

    // A name the machine needs asks first, also for the whole home…
    let (st, body) = post(
        addr,
        &token,
        "/api/reglas",
        &regla("windowsupdate.com", "suffix", false),
    )
    .await;
    assert_eq!(st, 200, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(v["creada"].is_null(), "{body}");
    assert_eq!(v["necesita"], "confirmar", "{body}");
    // …and with the "yes" it is created confirmed, so the resolver does cut it.
    let (st, body) = post(
        addr,
        &token,
        "/api/reglas",
        &regla("windowsupdate.com", "suffix", true),
    )
    .await;
    assert_eq!(st, 200, "{body}");
    let l = Ledger::open(&db, genesis()).unwrap();
    let guardadas = l.rules().unwrap();
    let wu = guardadas
        .iter()
        .find(|r| r.pattern == "windowsupdate.com")
        .expect("created");
    assert!(wu.confirmed);

    // The declared scope: an address becomes its host; a line that is no name is said.
    let alcance =
        |lineas: &str| format!(r#"{{"device_id":"{SELF_DEVICE_ID}","patrones":"{lineas}"}}"#);
    let (st, body) = post(
        addr,
        &token,
        "/api/ia/alcance",
        &alcance("https://github.com/guardianagroup\\n*.openai.com"),
    )
    .await;
    assert_eq!(st, 200, "{body}");
    let guardado = l
        .setting(&format!("alcance:{SELF_DEVICE_ID}"))
        .unwrap()
        .unwrap_or_default();
    assert_eq!(guardado, "github.com\nopenai.com");
    let (st, body) = post(
        addr,
        &token,
        "/api/ia/alcance",
        &alcance("github.com\\nmi agente"),
    )
    .await;
    assert_eq!(st, 400);
    assert!(body.contains("mi agente"), "{body}");
    // The bad save changed nothing.
    let guardado = l
        .setting(&format!("alcance:{SELF_DEVICE_ID}"))
        .unwrap()
        .unwrap_or_default();
    assert_eq!(guardado, "github.com\nopenai.com");
}

async fn get(addr: SocketAddr, token: Option<&str>, path: &str) -> u16 {
    let mut s = TcpStream::connect(addr).await.unwrap();
    let mut req = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n");
    if let Some(t) = token {
        req.push_str(&format!("X-Guardiana-Token: {t}\r\n"));
    }
    req.push_str("\r\n");
    s.write_all(req.as_bytes()).await.unwrap();
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).await.unwrap();
    let text = String::from_utf8_lossy(&raw).into_owned();
    text.split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .expect("a status code")
}

/// From this computer, "my device" is the computer: its history and its rules are not readable
/// or undoable without the key, as the rest of the panel (review of 5 Oct 2026). A phone on the
/// Wi-Fi still reads its own page by its address alone (the consent test covers that side).
#[tokio::test]
async fn this_computer_needs_the_key_for_its_own_page() {
    let (db, token_path) = prepare("propio");
    let running = start(Config {
        listen: vec!["127.0.0.1:0".parse().unwrap()],
        optional_listen: Vec::new(),
        db_path: db,
        genesis: genesis(),
        token_path,
        extra_hosts: Vec::new(),
        info: RuntimeInfo::default(),
    })
    .await
    .expect("the panel starts");
    let addr = running.addrs[0];
    let token = running.token.clone();
    for path in [
        "/api/mi-dispositivo",
        "/api/mi-dispositivo/reglas",
        "/api/me",
    ] {
        assert_eq!(get(addr, None, path).await, 401, "{path} without the key");
        assert_eq!(
            get(addr, Some(&token), path).await,
            200,
            "{path} with the key"
        );
    }
    let (st, _) = post(addr, "", "/api/mi-dispositivo/reglas/1/deshacer", "").await;
    assert_eq!(st, 401);
}
