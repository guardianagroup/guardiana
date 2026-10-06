//! The household panel keeps the promise printed on it and in the privacy policy: the detail of
//! a phone is only seen from that phone, unless its owner shares it. Until 1.0.1 the sharing
//! flag only painted a label and every listing returned the names (review of 1 Oct 2026,
//! entry 1). This drives the real HTTP panel on a loopback port with a ledger holding three
//! devices: this computer, a phone that did not share, and one that did.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::net::SocketAddr;
use std::path::PathBuf;

use guardiana_core::time::now_ms;
use guardiana_core::{Action, Hash, Ledger, MatchKind, NewEvent, NewRule, Scope, SELF_DEVICE_ID};
use guardiana_panel::{start, Config, RuntimeInfo};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const CALLADO: &str = "mac:aa-callado";
const ABIERTO: &str = "mac:bb-abierto";
const NOMBRE_CALLADO: &str = "clinica-ejemplo.example";
const NOMBRE_ABIERTO: &str = "tienda-ejemplo.example";
const NOMBRE_PROPIO: &str = "equipo-ejemplo.example";
const REGLA_DEL_CALLADO: &str = "citas-privadas.example";
const REGLA_DE_LA_CASA: &str = "juegos-ejemplo.example";

/// One plain HTTP/1.1 request over a fresh connection; the status line and the body.
async fn get(addr: SocketAddr, token: Option<&str>, path: &str) -> (u16, String) {
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
    let (head, body) = text.split_once("\r\n\r\n").expect("a complete response");
    let status: u16 = head
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .expect("a status code");
    (status, body.to_owned())
}

fn genesis() -> Hash {
    Hash::of(b"panel consent test key")
}

/// A fresh data folder with the ledger the test needs.
fn prepare(tag: &str) -> (PathBuf, PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("guardiana-panel-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("ledger.db");
    let token = dir.join("panel.token");
    let now = now_ms();
    let mut l = Ledger::open(&db, genesis()).unwrap();
    l.upsert_device(SELF_DEVICE_ID, None, Some("127.0.0.1"), now - 5000)
        .unwrap();
    l.upsert_device(CALLADO, Some("aa:aa"), Some("10.0.0.2"), now - 5000)
        .unwrap();
    l.upsert_device(ABIERTO, Some("bb:bb"), Some("10.0.0.3"), now - 5000)
        .unwrap();
    l.set_share_detail(ABIERTO, true).unwrap();
    l.append(NewEvent::observed(
        now - 4000,
        SELF_DEVICE_ID,
        "127.0.0.1",
        NOMBRE_PROPIO,
        "A",
    ))
    .unwrap();
    for i in 0..3 {
        l.append(NewEvent::observed(
            now - 3000 + i,
            CALLADO,
            "10.0.0.2",
            NOMBRE_CALLADO,
            "A",
        ))
        .unwrap();
    }
    l.append(NewEvent::observed(
        now - 2000,
        ABIERTO,
        "10.0.0.3",
        NOMBRE_ABIERTO,
        "A",
    ))
    .unwrap();
    // A rule the silent phone set for itself from its own page, and one the household set on
    // that same phone from the panel.
    let regla = |pattern: &str, created_by: &str| NewRule {
        scope: Scope::Device,
        device_id: Some(CALLADO.to_owned()),
        match_kind: MatchKind::Domain,
        pattern: pattern.to_owned(),
        action: Action::Cortar,
        created_at: now - 1000,
        created_by: created_by.to_owned(),
        expires_at: None,
        confirmed: false,
    };
    l.add_rule(regla(REGLA_DEL_CALLADO, "usuario (dispositivo)"))
        .unwrap();
    l.add_rule(regla(REGLA_DE_LA_CASA, "usuario (panel)"))
        .unwrap();
    (dir, db, token)
}

#[tokio::test]
async fn the_household_panel_never_lists_a_phone_that_did_not_share() {
    let (dir, db, token_path) = prepare("consent");
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
    .expect("the panel starts on a free loopback port");
    let addr = running.addrs[0];
    let token = running.token.clone();
    let t = Some(token.as_str());

    // The extract, asked for the silent phone by id: no rows, no name.
    let (status, body) = get(addr, t, &format!("/api/extracto?device_id={CALLADO}")).await;
    assert_eq!(status, 200, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["eventos"].as_array().unwrap().len(), 0, "{body}");
    assert!(!body.contains(NOMBRE_CALLADO), "{body}");

    // The whole extract: this computer and the sharing phone, nothing of the silent one. The
    // total is what the household can read; the rest is a number with no names (review of
    // 5 Oct 2026, privacy item 3).
    let (status, body) = get(addr, t, "/api/extracto").await;
    assert_eq!(status, 200, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["total"], 2, "{body}");
    assert_eq!(v["ocultos"], 3, "{body}");
    assert_eq!(v["eventos"].as_array().unwrap().len(), 2, "{body}");
    assert!(
        body.contains(NOMBRE_PROPIO) && body.contains(NOMBRE_ABIERTO),
        "{body}"
    );
    assert!(!body.contains(NOMBRE_CALLADO), "{body}");

    // The rules: the one the household set on the silent phone is listed (it can always undo
    // it); the one the phone set for itself is a number, not a name (review of 5 Oct 2026,
    // privacy item 5).
    let (status, body) = get(addr, t, "/api/reglas").await;
    assert_eq!(status, 200, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(body.contains(REGLA_DE_LA_CASA), "{body}");
    assert!(!body.contains(REGLA_DEL_CALLADO), "{body}");
    assert_eq!(v["ocultas"], 1, "{body}");

    // The export takes the whole history, and still not the silent phone's: CSV and JSON.
    for path in [
        format!("/api/extracto/exportar?device_id={CALLADO}"),
        format!("/api/extracto/exportar?device_id={CALLADO}&formato=json"),
        "/api/extracto/exportar".to_owned(),
        "/api/extracto/exportar?formato=json".to_owned(),
    ] {
        let (status, body) = get(addr, t, &path).await;
        assert_eq!(status, 200, "{path}: {body}");
        assert!(!body.contains(NOMBRE_CALLADO), "{path}: {body}");
    }
    let (_, body) = get(addr, t, "/api/extracto/exportar").await;
    assert!(
        body.contains(NOMBRE_PROPIO) && body.contains(NOMBRE_ABIERTO),
        "{body}"
    );

    // The radiography: the counters add up the whole house (three distinct names), the list of
    // names leaves the silent phone out.
    let (status, body) = get(addr, t, "/api/radiografia?segundos=3600").await;
    assert_eq!(status, 200, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["servicios"], 3, "{body}");
    assert_eq!(v["eventos"].as_array().unwrap().len(), 2, "{body}");
    assert!(!body.contains(NOMBRE_CALLADO), "{body}");

    // The devices page: the silent phone is there with its figures and marked as not visible,
    // and nothing read sideways from its names (companies, countries) either.
    let (status, body) = get(addr, t, "/api/dispositivos").await;
    assert_eq!(status, 200, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let callado = v
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"] == CALLADO)
        .expect("the silent phone is listed");
    assert_eq!(callado["totales"]["consultas"], 3, "{body}");
    assert_eq!(callado["detalle_visible"], false, "{body}");
    assert_eq!(callado["share_detail_with_home"], false, "{body}");
    assert_eq!(callado["lectura"]["empresas_total"], 0, "{body}");
    assert!(!body.contains(NOMBRE_CALLADO), "{body}");

    // This computer's own page (loopback is this computer): its own rows in full, with the key
    // (without it, any program or account on the computer could read them; 5 Oct 2026).
    let (status, _) = get(addr, None, "/api/mi-dispositivo").await;
    assert_eq!(status, 401);
    let (status, body) = get(addr, t, "/api/mi-dispositivo").await;
    assert_eq!(status, 200, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["es_este_computador"], true, "{body}");
    assert_eq!(v["eventos"].as_array().unwrap().len(), 1, "{body}");
    assert!(
        body.contains(NOMBRE_PROPIO) && !body.contains(NOMBRE_CALLADO),
        "{body}"
    );

    // Without the session token the household extract is not served at all.
    let (status, _) = get(addr, None, "/api/extracto").await;
    assert_eq!(status, 401);

    running.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}
