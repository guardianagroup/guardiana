//! A company's profile in the panel (1.0.16, the same «ficha» as GUARDIANA ZERO), through the
//! real HTTP panel: what the house asked of Criteo and of nobody else, its checked facts, the
//! letter to it under the law of a place, the domain ending of a name nobody owns, and nothing
//! about a name that is neither a company nor a site. A phone that did not share its detail
//! stays out of the counts.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::net::SocketAddr;
use std::path::PathBuf;

use guardiana_core::time::now_ms;
use guardiana_core::{Hash, Ledger, NewEvent, Verdict, SELF_DEVICE_ID};
use guardiana_panel::{start, Config, RuntimeInfo};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const CALLADO: &str = "mac:cc-callado";

async fn get(addr: SocketAddr, token: &str, path: &str) -> (u16, String) {
    let mut s = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nX-Guardiana-Token: {token}\r\nX-Guardiana-Lang: es\r\n\r\n"
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
    Hash::of(b"panel company profile test key")
}

fn prepare() -> (PathBuf, PathBuf, PathBuf, String) {
    let dir = std::env::temp_dir().join(format!("guardiana-panel-empresa-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("ledger.db");
    let token = dir.join("panel.token");
    let now = now_ms();
    let criteo = guardiana_lists::company_record("Criteo").unwrap().domains[0];
    let mut l = Ledger::open(&db, genesis()).unwrap();
    l.upsert_device(SELF_DEVICE_ID, None, Some("127.0.0.1"), now - 60_000)
        .unwrap();
    l.upsert_device(CALLADO, Some("cc:cc"), Some("10.0.0.7"), now - 60_000)
        .unwrap();
    let mut ts = now - 50_000;
    let mut pon = |dev: &str, q: &str, veces: usize, v: Verdict| {
        for _ in 0..veces {
            ts += 10;
            let mut e = NewEvent::observed(ts, dev, "127.0.0.1", q, "A");
            e.verdict = v;
            l.append(e).unwrap();
        }
    };
    pon(
        SELF_DEVICE_ID,
        &format!("static.{criteo}"),
        4,
        Verdict::Cortado,
    );
    pon(SELF_DEVICE_ID, criteo, 2, Verdict::Observado);
    pon(SELF_DEVICE_ID, "graph.facebook.com", 9, Verdict::Observado);
    pon(
        SELF_DEVICE_ID,
        "medidor.ejemplo-sin-duenio.cz",
        3,
        Verdict::Observado,
    );
    // The silent phone asks Criteo too, and must not be counted.
    pon(CALLADO, criteo, 5, Verdict::Observado);
    (dir, db, token, criteo.to_owned())
}

#[tokio::test]
async fn a_company_profile_shows_its_facts_what_the_house_asked_and_its_letter() {
    let (dir, db, token_path, criteo) = prepare();
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
    let t = running.token.clone();

    // The page itself.
    let (status, body) = get(addr, &t, "/empresa?e=Criteo").await;
    assert_eq!(status, 200);
    assert!(body.contains("data-page=\"empresa\""));

    // Criteo: its seat, its checked profile, and the house's six queries (four cut).
    let (status, body) = get(addr, &t, "/api/empresa?e=Criteo").await;
    assert_eq!(status, 200, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["empresa"], "Criteo");
    assert_eq!(v["conocida"], true);
    assert_eq!(v["pais"], "Francia");
    assert!(v["ficha"]["sanciones"].is_array(), "{body}");
    assert_eq!(v["casa"]["consultas"], 6, "{body}");
    assert_eq!(v["casa"]["cortadas"], 4);
    assert_eq!(v["casa"]["n_nombres"], 2);
    assert_eq!(v["casa"]["callados"], 1);
    assert!(!body.contains("graph.facebook.com"), "only Criteo's names");
    assert!(body.contains(&criteo));

    // Its letter, under Spain's law, with the house's evidence in it.
    let (status, body) = get(
        addr,
        &t,
        "/api/borrado/carta?empresa=Criteo&jurisdiccion=es&idioma=es&evidencia=1",
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let c: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(c["asunto"].as_str().unwrap().contains("Criteo"), "{body}");
    assert!(c["cuerpo"].as_str().unwrap().contains(&criteo), "{body}");
    assert!(c["cuerpo"].as_str().unwrap().contains("[[NOMBRE]]"));

    // A name nobody owns: a site, with the country of its ending said as such.
    let (status, body) = get(addr, &t, "/api/empresa?e=medidor.ejemplo-sin-duenio.cz").await;
    assert_eq!(status, 200, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["conocida"], false);
    assert_eq!(v["pais_por_dominio"], true);
    assert_eq!(v["pais_codigo"], "CZ");
    assert_eq!(v["casa"]["consultas"], 3);

    // The X-ray's row for it says the same, and Facebook's row does not.
    let (status, body) = get(addr, &t, "/api/extracto").await;
    assert_eq!(status, 200, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let eventos = v["eventos"].as_array().unwrap();
    let cz = eventos
        .iter()
        .find(|e| e["qname"] == "medidor.ejemplo-sin-duenio.cz")
        .unwrap();
    assert!(
        cz["pais_dominio"].as_str().is_some_and(|p| !p.is_empty()),
        "{cz}"
    );
    let fb = eventos
        .iter()
        .find(|e| e["qname"] == "graph.facebook.com")
        .unwrap();
    assert!(fb.get("pais_dominio").is_none(), "{fb}");

    // Neither a company nor a site: nothing to say.
    let (status, _) = get(addr, &t, "/api/empresa?e=Nadie%20en%20particular").await;
    assert_eq!(status, 404);
    // And without the session, nothing at all.
    let (status, _) = get(addr, "otra", "/api/empresa?e=Criteo").await;
    assert_eq!(status, 401);

    running.shutdown().await;
    let _ = std::fs::remove_dir_all(dir);
}
