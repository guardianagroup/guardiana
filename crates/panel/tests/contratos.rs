//! «Contratos» (1.0.45) through the real HTTP panel: a program learnt over a week, its update
//! starting to contact a new company, the notice and the pill's «Atención», the cut of only what
//! is new, and the register that verifies.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::net::SocketAddr;

use guardiana_core::time::{now_ms, DAY_MS};
use guardiana_core::{Hash, Ledger, NewEvent, Proceso, SELF_DEVICE_ID};
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
    let req = format!(
        "{metodo} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nX-Guardiana-Token: {token}\r\nX-Guardiana-Lang: es\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
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

fn zoom(pid: u32, version: &str) -> Proceso {
    Proceso {
        pid,
        nombre: "Zoom.exe".into(),
        ruta: r"C:\Users\ana\AppData\Roaming\Zoom\bin\Zoom.exe".into(),
        sha256: String::new(),
        firmado_por: "Zoom Video Communications, Inc.".into(),
        version: version.into(),
    }
}

#[tokio::test]
async fn a_program_that_starts_contacting_a_new_company_is_told_calmly_and_can_be_cut() {
    let dir = std::env::temp_dir().join(format!("guardiana-panel-ct-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("ledger.db");
    let genesis = Hash::of(b"panel contratos test key");
    let now = now_ms();
    {
        let mut l = Ledger::open(&db, genesis).unwrap();
        // 1.0.46: GUARDIANA has been learning on this computer for eleven days (a baseline is a
        // week watched, not a week read from history).
        l.set_setting("contratos_inicio", &(now - 11 * DAY_MS).to_string())
            .unwrap();
        let mut pon = |ts: i64, q: &str, p: Proceso| {
            let mut e = NewEvent::observed(ts, SELF_DEVICE_ID, "127.0.0.1", q, "A");
            e.process = Some(p);
            l.append(e).unwrap();
        };
        pon(now - 10 * DAY_MS, "zoom.us", zoom(1, "6.1.0"));
        pon(now - 9 * DAY_MS, "api.zoom.us", zoom(2, "6.1.0"));
        pon(now - 60_000, "zoom.us", zoom(3, "6.2.0"));
        pon(
            now - 50_000,
            "api2.contrato-nuevo-de-prueba.com",
            zoom(3, "6.2.0"),
        );
    }
    let running = start(Config {
        listen: vec!["127.0.0.1:0".parse().unwrap()],
        optional_listen: Vec::new(),
        db_path: db,
        genesis,
        token_path: dir.join("panel.token"),
        extra_hosts: Vec::new(),
        info: RuntimeInfo::default(),
    })
    .await
    .expect("the panel starts");
    let addr = running.addrs[0];
    let t = running.token.clone();

    let (status, body) = pide(addr, &t, "GET", "/contratos", "").await;
    assert_eq!(status, 200);
    assert!(body.contains("data-page=\"contratos\""));
    let (status, _) = pide(addr, &t, "GET", "/static/contratos.js", "").await;
    assert_eq!(status, 200);

    let (status, body) = pide(addr, &t, "GET", "/api/contratos", "").await;
    assert_eq!(status, 200, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let c = &v["contratos"][0];
    assert_eq!(c["programa"], "Zoom");
    assert_eq!(c["estado"], "vigilando");
    assert_eq!(c["version"], "6.2.0");
    let a = &v["avisos"][0];
    assert_eq!(
        a["titulo"],
        "Zoom 6.2.0 ha cambiado: empezó a contactar con una empresa nueva"
    );
    assert_eq!(a["version_antes"], "6.1.0");
    assert_eq!(a["estado"], "abierto");
    assert!(a["nota"].as_str().unwrap().contains("no dice qué envió"));
    let id = a["id"].as_i64().unwrap();

    let (_, body) = pide(addr, &t, "GET", "/api/contratos/atencion", "").await;
    let p: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(p["atencion"], true);
    assert_eq!(
        p["texto"],
        "Atención: Zoom empezó a contactar con empresas nuevas"
    );

    // Without the key, nothing.
    let (status, _) = pide(addr, "x", "GET", "/api/contratos", "").await;
    assert_eq!(status, 401);

    let (status, body) = pide(
        addr,
        &t,
        "POST",
        &format!("/api/contratos/avisos/{id}/cortar"),
        "",
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let (_, body) = pide(addr, &t, "GET", "/api/reglas", "").await;
    assert!(body.contains("contrato-nuevo-de-prueba.com"), "{body}");
    assert!(!body.contains("\"zoom.us\""), "{body}");
    // Twice is not found: it is resolved.
    let (status, _) = pide(
        addr,
        &t,
        "POST",
        &format!("/api/contratos/avisos/{id}/cortar"),
        "",
    )
    .await;
    assert_eq!(status, 404);

    let (_, body) = pide(addr, &t, "GET", "/api/contratos/atencion", "").await;
    let p: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(p["atencion"], false);

    let (_, body) = pide(addr, &t, "GET", "/api/contratos/comprobar", "").await;
    let r: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(r["ok"], true, "{body}");
    let (_, body) = pide(addr, &t, "GET", "/api/contratos/registro", "").await;
    assert!(body.contains("Cortado lo nuevo"), "{body}");
    assert!(body.contains("Versión 6.1.0 → 6.2.0"), "{body}");

    running.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}
