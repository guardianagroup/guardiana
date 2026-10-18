//! The customer tools of 1.0.45 (18 Oct 2026) through the real HTTP panel on a loopback port:
//! «Modo apagón» is reversible and each switch lands in the chained ledger, «Limpiar archivo»
//! answers with a clean copy, «Qué se rompe si corto» counts programs from the ledger.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::net::SocketAddr;
use std::path::PathBuf;

use guardiana_core::apagon::{Apagon, AJUSTE_APAGON, NOMBRE_EVENTO};
use guardiana_core::time::now_ms;
use guardiana_core::{EventFilter, Hash, Ledger, NewEvent, Proceso, SELF_DEVICE_ID};
use guardiana_panel::{start, Config, RuntimeInfo};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

async fn pide(
    addr: SocketAddr,
    token: &str,
    metodo: &str,
    path: &str,
    tipo: &str,
    body: &[u8],
) -> (u16, Vec<u8>) {
    let mut s = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "{metodo} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nX-Guardiana-Token: {token}\r\nContent-Type: {tipo}\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    s.write_all(req.as_bytes()).await.unwrap();
    s.write_all(body).await.unwrap();
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).await.unwrap();
    let fin = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8_lossy(&raw[..fin]).into_owned();
    let status: u16 = head.split_whitespace().nth(1).unwrap().parse().unwrap();
    let mut cuerpo = raw[fin + 4..].to_vec();
    if head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        cuerpo = deschunk(&cuerpo);
    }
    (status, cuerpo)
}

fn deschunk(mut d: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let fin = d.windows(2).position(|w| w == b"\r\n").unwrap();
        let n = usize::from_str_radix(std::str::from_utf8(&d[..fin]).unwrap().trim(), 16).unwrap();
        if n == 0 {
            return out;
        }
        out.extend_from_slice(&d[fin + 2..fin + 2 + n]);
        d = &d[fin + 4 + n..];
    }
}

fn json(b: &[u8]) -> serde_json::Value {
    serde_json::from_slice(b).expect("a JSON answer")
}

fn genesis() -> Hash {
    Hash::of(b"panel tools test key")
}

fn prepare(tag: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("guardiana-herr-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    (dir.join("ledger.db"), dir.join("panel.token"))
}

async fn arranca(db: &std::path::Path, token: PathBuf) -> (guardiana_panel::Running, SocketAddr) {
    let running = start(Config {
        listen: vec!["127.0.0.1:0".parse().unwrap()],
        optional_listen: Vec::new(),
        db_path: db.to_path_buf(),
        genesis: genesis(),
        token_path: token,
        extra_hosts: Vec::new(),
        info: RuntimeInfo::default(),
    })
    .await
    .expect("the panel starts");
    let addr = running.addrs[0];
    (running, addr)
}

#[tokio::test]
async fn the_blackout_goes_on_and_off_and_each_switch_is_in_the_chain() {
    let (db, token_path) = prepare("apagon");
    drop(Ledger::open(&db, genesis()).unwrap());
    let (running, addr) = arranca(&db, token_path).await;
    let tk = running.token.clone();
    let j = "application/json";

    let (st, b) = pide(addr, &tk, "GET", "/api/apagon", j, b"").await;
    assert_eq!(st, 200);
    let v = json(&b);
    assert_eq!(v["activo"], false);
    assert!(v["esenciales"].as_array().unwrap().len() > 10);

    let (st, b) = pide(addr, &tk, "POST", "/api/apagon", j, br#"{"activo":true}"#).await;
    assert_eq!(st, 200);
    assert_eq!(json(&b)["activo"], true);
    let (st, b) = pide(
        addr,
        &tk,
        "POST",
        "/api/apagon/aprobar",
        j,
        br#"{"nombre":"https://www.banco.com/x"}"#,
    )
    .await;
    assert_eq!(st, 200);
    assert_eq!(json(&b)["aprobados"][0], "www.banco.com");
    let (st, _) = pide(
        addr,
        &tk,
        "POST",
        "/api/apagon/aprobar",
        j,
        br#"{"nombre":"nada"}"#,
    )
    .await;
    assert_eq!(st, 400);
    // Turning it on twice writes it once.
    pide(addr, &tk, "POST", "/api/apagon", j, br#"{"activo":true}"#).await;
    let (_, b) = pide(addr, &tk, "POST", "/api/apagon", j, br#"{"activo":false}"#).await;
    assert_eq!(json(&b)["activo"], false);
    assert_eq!(json(&b)["aprobados"][0], "www.banco.com", "approvals stay");
    // Without the session, nothing.
    let (st, _) = pide(
        addr,
        "otra",
        "POST",
        "/api/apagon",
        j,
        br#"{"activo":true}"#,
    )
    .await;
    assert_eq!(st, 401);
    running.shutdown().await;

    let l = Ledger::open(&db, genesis()).unwrap();
    let a = Apagon::de_ajuste(l.setting(AJUSTE_APAGON).unwrap().as_deref());
    assert!(!a.activo);
    let ev = l
        .events(&EventFilter {
            qname: Some(NOMBRE_EVENTO.to_owned()),
            ..EventFilter::default()
        })
        .unwrap();
    let fuentes: Vec<&str> = ev.iter().map(|e| e.list_source.as_str()).collect();
    assert_eq!(fuentes, ["modo_apagon:encendido", "modo_apagon:apagado"]);
    assert!(l.check().unwrap().is_ok(), "the chain still verifies");
}

#[tokio::test]
async fn a_photo_is_looked_at_and_cleaned_through_the_panel() {
    let (db, token_path) = prepare("huellas");
    drop(Ledger::open(&db, genesis()).unwrap());
    let (running, addr) = arranca(&db, token_path).await;
    let tk = running.token.clone();
    let foto = include_bytes!("../../core/tests/huellas/foto-gps.jpg");
    let o = "application/octet-stream";
    let (st, b) = pide(addr, &tk, "POST", "/api/huellas/mirar", o, foto).await;
    assert_eq!(st, 200);
    let v = json(&b);
    assert_eq!(v["formato"], "jpeg");
    assert_eq!(v["huellas"][0]["clave"], "gps");
    let (st, limpio) = pide(addr, &tk, "POST", "/api/huellas/limpiar", o, foto).await;
    assert_eq!(st, 200);
    assert!(guardiana_core::huellas::mirar(&limpio).huellas.is_empty());
    let (st, _) = pide(addr, &tk, "POST", "/api/huellas/limpiar", o, b"texto").await;
    assert_eq!(st, 422);
    running.shutdown().await;
}

#[tokio::test]
async fn what_breaks_names_the_programs_that_used_the_domain() {
    let (db, token_path) = prepare("rompe");
    {
        let mut l = Ledger::open(&db, genesis()).unwrap();
        let ahora = now_ms();
        for (q, prog) in [
            ("teams.microsoft.com", Some("ms-teams.exe")),
            ("teams.microsoft.com", Some("ms-teams.exe")),
            ("graph.microsoft.com", Some("olk.exe")),
            ("graph.microsoft.com", None),
            ("www.example.org", Some("chrome.exe")),
        ] {
            let mut e = NewEvent::observed(ahora - 1000, SELF_DEVICE_ID, "127.0.0.1", q, "A");
            e.process = prog.map(|p| Proceso {
                pid: 1,
                nombre: p.to_owned(),
                ruta: String::new(),
                sha256: String::new(),
                firmado_por: String::new(),
                version: String::new(),
            });
            l.append(e).unwrap();
        }
    }
    let (running, addr) = arranca(&db, token_path).await;
    let tk = running.token.clone();
    let (st, b) = pide(
        addr,
        &tk,
        "GET",
        "/api/que-se-rompe?dominio=microsoft.com",
        "application/json",
        b"",
    )
    .await;
    assert_eq!(st, 200);
    let v = json(&b);
    assert_eq!(v["objetivo"], "microsoft.com");
    assert_eq!(v["programas"][0]["programa"], "ms-teams.exe");
    assert_eq!(v["programas"][0]["consultas"], 2);
    assert_eq!(v["programas"][1]["programa"], "olk.exe");
    assert_eq!(v["programas"].as_array().unwrap().len(), 2);
    assert_eq!(v["sin_programa"], 1);
    assert_eq!(v["nombres"], 2);
    let (st, _) = pide(
        addr,
        &tk,
        "GET",
        "/api/que-se-rompe",
        "application/json",
        b"",
    )
    .await;
    assert_eq!(st, 400);
    running.shutdown().await;
}
