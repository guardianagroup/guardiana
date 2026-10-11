//! «A dónde viajan tus datos» (decision 203), through the real HTTP panel: the server of each
//! row, the extract filtered by the server's country, the spreadsheet CSV with its new columns
//! and the weekly report's countries and groups, all worked out from a ledger written the way
//! the resolver writes it. A phone that did not share its detail stays out of all of it.

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
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nX-Guardiana-Token: {token}\r\n\r\n"
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
    // Chunked bodies (the CSV): good enough to search in, sizes and all.
    (status, body.to_owned())
}

fn genesis() -> Hash {
    Hash::of(b"panel destinations test key")
}

fn prepare(tag: &str) -> (PathBuf, PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("guardiana-panel-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("ledger.db");
    let token = dir.join("panel.token");
    let now = now_ms();
    let mut l = Ledger::open(&db, genesis()).unwrap();
    l.upsert_device(SELF_DEVICE_ID, None, Some("127.0.0.1"), now - 60_000)
        .unwrap();
    l.upsert_device(CALLADO, Some("cc:cc"), Some("10.0.0.7"), now - 60_000)
        .unwrap();
    let mut ts = now - 50_000;
    let mut pon = |dev: &str, q: &str, ip: &str, veces: usize, v: Verdict| {
        for _ in 0..veces {
            ts += 10;
            let mut e = NewEvent::observed(ts, dev, "127.0.0.1", q, "A");
            // As the engine does when the answer arrives: the address is looked up and dropped.
            e.server =
                (!ip.is_empty()).then(|| guardiana_lists::server_info_de(ip.parse().unwrap()));
            e.verdict = v;
            l.append(e).unwrap();
        }
    };
    pon(
        SELF_DEVICE_ID,
        "graph.facebook.com",
        "157.240.1.35",
        6,
        Verdict::Observado,
    );
    pon(
        SELF_DEVICE_ID,
        "www.youtube.com",
        "142.250.1.1",
        4,
        Verdict::Observado,
    );
    pon(
        SELF_DEVICE_ID,
        "app.slack.com",
        "13.107.1.1",
        2,
        Verdict::Observado,
    );
    pon(
        SELF_DEVICE_ID,
        "one.one.one.one",
        "1.1.1.1",
        2,
        Verdict::Observado,
    );
    pon(
        SELF_DEVICE_ID,
        "router.lan",
        "192.168.1.1",
        1,
        Verdict::Observado,
    );
    pon(SELF_DEVICE_ID, "ads.example", "", 3, Verdict::Cortado);
    // The silent phone: its queries go to Ireland too, and must not be counted by name.
    pon(
        CALLADO,
        "clinica.example",
        "157.240.9.9",
        5,
        Verdict::Observado,
    );
    (dir, db, token)
}

#[tokio::test]
async fn the_panel_says_where_each_query_went_and_sums_the_week() {
    let (dir, db, token_path) = prepare("destinos");
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
    let muestra = guardiana_lists::fuente_red().muestra;

    // Every row with an address carries its server; the cut ones carry none.
    let (status, body) = get(addr, &t, "/api/extracto").await;
    assert_eq!(status, 200, "{body}");
    for ip in ["1.1.1.1", "157.240.1.35", "192.168.1.1"] {
        assert!(!body.contains(ip), "no address in the extract: {ip}");
    }
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let eventos = v["eventos"].as_array().unwrap();
    let cf = eventos
        .iter()
        .find(|e| e["qname"] == "one.one.one.one")
        .unwrap();
    assert_eq!(cf["servidor"]["anycast"], true, "{cf}");
    assert!(cf["servidor"]["pais"].is_null(), "anycast has no country");
    assert_eq!(cf["servidor"]["asn"], 13335);
    let lan = eventos.iter().find(|e| e["qname"] == "router.lan").unwrap();
    assert_eq!(lan["servidor"]["local"], true);
    let cortada = eventos
        .iter()
        .find(|e| e["qname"] == "ads.example")
        .unwrap();
    assert!(cortada.get("servidor").is_none(), "{cortada}");
    let slack = eventos
        .iter()
        .find(|e| e["qname"] == "app.slack.com")
        .unwrap();
    assert_eq!(slack["grupo"], "Salesforce");
    assert_eq!(v["fuente_red"]["fuente"], "DB-IP Lite");
    assert_eq!(v["fuente_red"]["muestra"], muestra);
    if muestra {
        let fb = eventos
            .iter()
            .find(|e| e["qname"] == "graph.facebook.com")
            .unwrap();
        assert_eq!(
            fb["servidor"]["frase"],
            "servido desde la red de Meta (AS32934) · el servidor que respondió está en Irlanda"
        );
    }
    // The filter lists what the household's rows hold, and nothing of the silent phone.
    let paises: Vec<&str> = v["paises_servidor"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p[0].as_str().unwrap())
        .collect();
    assert!(
        paises.contains(&"anycast") && paises.contains(&"local"),
        "{paises:?}"
    );
    assert!(!body.contains("clinica.example"));

    // Filtered by the server's country: only what pointed there.
    let (status, body) = get(addr, &t, "/api/extracto?pais=anycast").await;
    assert_eq!(status, 200, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let nombres: Vec<&str> = v["eventos"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["qname"].as_str().unwrap())
        .collect();
    assert_eq!(nombres, vec!["one.one.one.one", "one.one.one.one"]);
    if muestra {
        let (_, body) = get(addr, &t, "/api/extracto?pais=IE").await;
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["eventos"].as_array().unwrap().len(), 6, "{body}");
        assert!(
            !body.contains("clinica.example"),
            "the silent phone stays out"
        );
    }
    // A country nobody wrote properly is refused, not silently empty.
    let (status, _) = get(addr, &t, "/api/extracto?pais=irlanda").await;
    assert_eq!(status, 400);

    // The spreadsheet CSV: the server columns the hash covers, then the network's name and the
    // group with their source in the header. No address anywhere.
    let (status, csv) = get(addr, &t, "/api/extracto/exportar").await;
    assert_eq!(status, 200);
    assert!(
        csv.contains(";servidor_tipo;servidor_pais;servidor_asn;servidor_base;servidor_red ["),
        "{csv}"
    );
    assert!(csv.contains(";grupo [empresas.txt]"), "{csv}");
    let base = guardiana_lists::base_de_red();
    assert!(
        csv.contains(&format!(";anycast;;13335;{base};Cloudflare;")),
        "{csv}"
    );
    for ip in ["1.1.1.1", "157.240.1.35", "142.250.1.1", "192.168.1.1"] {
        assert!(!csv.contains(ip), "no address in the CSV: {ip}");
    }
    assert!(csv.contains("app.slack.com"), "{csv}");
    assert!(csv.contains(";Salesforce"), "{csv}");
    if muestra {
        assert!(csv.contains("MUESTRA"), "the sample says so in the header");
    }
    let (status, csv) = get(addr, &t, "/api/extracto/exportar?pais=anycast").await;
    assert_eq!(status, 200);
    assert!(!csv.contains("graph.facebook.com"), "{csv}");

    // The weekly report: the household's week, by country and by group, and the sentence to
    // share. The silent phone's five queries are not in it.
    let (status, body) = get(addr, &t, "/api/informe").await;
    assert_eq!(status, 200, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let d = &v["destinos"];
    // 6 + 4 + 2 + 2 + 1 forwarded; the 3 cut ones never left.
    assert_eq!(d["consultas"], 15, "{d}");
    assert_eq!(d["con_direccion"], 15, "{d}");
    let grupos: Vec<(&str, i64)> = d["grupos"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| {
            (
                g["nombre"].as_str().unwrap(),
                g["consultas"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(grupos[0], ("Meta", 6), "{grupos:?}");
    assert!(grupos.contains(&("Alphabet", 4)), "{grupos:?}");
    assert!(grupos.contains(&("Salesforce", 2)), "{grupos:?}");
    assert_eq!(d["top_n"], 3);
    // (6 + 4 + 2) of 15 → 80 %.
    assert_eq!(d["top_pct"], 80);
    let texto = v["texto_whatsapp"].as_str().unwrap();
    assert!(
        texto.contains("El 80 % de las consultas que salieron de casa fue a nombres de 3 grupos"),
        "{texto}"
    );
    // The sample is not something to send anybody: no countries in the shared text with it.
    assert_eq!(texto.contains("países"), !muestra, "{texto}");
    // And the ledger file itself holds no answer address.
    let mut datos = std::fs::read(dir.join("ledger.db")).unwrap();
    datos.extend(std::fs::read(dir.join("ledger.db-wal")).unwrap_or_default());
    for ip in [&b"157.240.1.35"[..], b"142.250.1.1", b"1.1.1.1"] {
        assert!(
            !datos.windows(ip.len()).any(|w| w == ip),
            "{}",
            String::from_utf8_lossy(ip)
        );
    }
    if muestra {
        // Ireland 6 and the United States 4 + 2 tie at 40 %: equal counts go by name.
        let paises = d["paises"].as_array().unwrap();
        assert_eq!(paises[0]["nombre"], "Estados Unidos");
        assert_eq!(paises[1]["clave"], "IE");
        assert_eq!(paises[1]["pct"], 40);
        assert_eq!(d["n_paises"], 2);
    }

    // The lists page names the two IP tables, with the sample said in words.
    let (status, body) = get(addr, &t, "/api/estado").await;
    assert_eq!(status, 200, "{body}");
    assert!(
        body.contains("\"dbip-pais\"") && body.contains("\"dbip-red\""),
        "{body}"
    );

    running.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}
