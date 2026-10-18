//! «Quién vende tus datos» and «Audita a GUARDIANA» (1.0.46) through the real HTTP panel on a
//! loopback port: a program that calls a documented data seller is named with its company, the
//! audit answers at every level (the free one too) with every connection GUARDIANA wrote down,
//! every table it keeps and no internet address, and its seal can be recomputed by anyone.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::net::SocketAddr;
use std::path::PathBuf;

use guardiana_core::time::now_ms;
use guardiana_core::{Hash, Ledger, NewEvent, Proceso, Purpose, SELF_DEVICE_ID};
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
    Hash::of(b"quien vende audita test key")
}

fn prepare(tag: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("guardiana-qva-{tag}-{}", std::process::id()));
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

fn evento(ts: i64, q: &str, prog: Option<&str>) -> NewEvent {
    let mut e = NewEvent::observed(ts, SELF_DEVICE_ID, "127.0.0.1", q, "A");
    e.process = prog.map(|p| Proceso {
        pid: 1,
        nombre: p.to_owned(),
        ruta: String::new(),
        sha256: String::new(),
        firmado_por: String::new(),
        version: String::new(),
    });
    e
}

#[tokio::test]
async fn a_program_calling_a_data_seller_is_named_with_its_company_and_case() {
    let (db, token_path) = prepare("vende");
    {
        let mut l = Ledger::open(&db, genesis()).unwrap();
        let ahora = now_ms();
        for (q, p) in [
            ("api.kochava.com", Some("juego.exe")),
            ("api.kochava.com", Some("juego.exe")),
            ("www.example.org", Some("juego.exe")),
            ("chatgpt.com", Some("chrome.exe")),
        ] {
            l.append(evento(ahora - 1000, q, p)).unwrap();
        }
    }
    let (running, addr) = arranca(&db, token_path).await;
    let tk = running.token.clone();
    let j = "application/json";
    let (st, b) = pide(addr, &tk, "GET", "/api/quien-vende", j, b"").await;
    assert_eq!(st, 200, "{}", String::from_utf8_lossy(&b));
    let v = json(&b);
    let p = &v["programas"][0];
    assert_eq!(p["programa"], "juego.exe");
    assert_eq!(p["consultas"], 2);
    assert_eq!(p["empresas"][0]["empresa"], "Kochava");
    assert_eq!(p["empresas"][0]["caso"], "ftc-kochava-2022");
    assert_eq!(
        p["empresas"][0]["dominios"][0]["dominio"],
        "api.kochava.com"
    );
    assert!(p["empresas"][0]["dominios"][0]["corte"].is_null());
    assert_eq!(v["resumen"]["programas"], 1);
    assert_eq!(v["ia"][0]["servicio"], "OpenAI (ChatGPT)");
    assert_eq!(v["ia"][0]["guia"], "chatgpt");
    // Every case shown has a date and a source.
    for c in v["casos"].as_array().unwrap() {
        assert!(c["url"].as_str().unwrap().starts_with("https://"));
        assert_eq!(c["fecha"].as_str().unwrap().len(), 10);
    }
    // A setting the page does not know is refused, and nothing is sent anywhere.
    let (st, _) = pide(
        addr,
        &tk,
        "POST",
        "/api/quien-vende/ajuste",
        j,
        br#"{"clave":"otra","activo":false}"#,
    )
    .await;
    assert_eq!(st, 400);
    let (st, b) = pide(addr, &tk, "GET", "/quien-vende", "text/html", b"").await;
    assert_eq!(st, 200);
    assert!(String::from_utf8_lossy(&b).contains("data-page=\"quienVende\""));
    let (st, _) = pide(addr, "otra", "GET", "/api/quien-vende", j, b"").await;
    assert_ne!(st, 200, "without the session, nothing");
    running.shutdown().await;
}

#[tokio::test]
async fn the_audit_lists_every_connection_every_table_no_ip_and_a_seal_anyone_can_recompute() {
    let (db, token_path) = prepare("audita");
    {
        let mut l = Ledger::open(&db, genesis()).unwrap();
        let ahora = now_ms();
        l.append(evento(ahora - 2000, "www.example.org", Some("chrome.exe")))
            .unwrap();
        l.record_outbound(
            ahora - 1000,
            Purpose::Licencia,
            "live.dodopayments.com",
            312,
            true,
        )
        .unwrap();
        l.record_outbound(
            ahora - 500,
            Purpose::Version,
            "raw.githubusercontent.com",
            90,
            false,
        )
        .unwrap();
    }
    let (running, addr) = arranca(&db, token_path).await;
    let tk = running.token.clone();
    let j = "application/json";
    let (st, b) = pide(addr, &tk, "GET", "/api/audita", j, b"").await;
    assert_eq!(
        st,
        200,
        "the audit works at every level: {}",
        String::from_utf8_lossy(&b)
    );
    let v = json(&b);
    let i = &v["informe"];
    assert_eq!(i["formato"], "guardiana-auditoria/1");
    let c = &i["conexiones"];
    assert_eq!(c["total"], 2);
    assert_eq!(c["lista"][0]["host"], "raw.githubusercontent.com");
    assert_eq!(c["lista"][0]["pedida"], false);
    assert_eq!(c["lista"][1]["motivo"], "licencia");
    let tablas: Vec<&str> = i["disco"]["base"]["tablas"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["nombre"].as_str().unwrap())
        .collect();
    assert!(
        tablas.contains(&"events") && tablas.contains(&"outbound"),
        "{tablas:?}"
    );
    assert_eq!(i["disco"]["base"]["direcciones_publicas"], 0);
    assert!(i["disco"]["archivos"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a[0] == "ledger.db"));
    assert_eq!(i["cadena"]["comprobada"], true);
    // The seal: the SHA-256 of the report with its own field empty.
    let sello = v["sello"].as_str().unwrap();
    assert_eq!(sello.len(), 64);
    let texto = v["json"].as_str().unwrap();
    let vacio = texto.replace(&format!("\"sello\": \"{sello}\""), "\"sello\": \"\"");
    assert_eq!(
        guardiana_verify::certificado::sello(vacio.as_bytes()),
        sello
    );
    assert!(v["linea_sha256"].as_str().unwrap().starts_with(sello));
    let html = v["html"].as_str().unwrap();
    assert!(html.contains(sello) && !html.contains("<script"));
    let (st, _) = pide(addr, &tk, "GET", "/audita", "text/html", b"").await;
    assert_eq!(st, 200);
    running.shutdown().await;
}
