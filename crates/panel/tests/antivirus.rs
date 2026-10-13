//! «GUARDIANA vigila a tu antivirus» through the real HTTP panel: an antivirus's own processes,
//! what its maker documents and what it does not, who signed them, and every product's official
//! record; a program that is no antivirus, and a phone that does not share, stay out.
//!
//! With `GUARDIANA_VISTA_SEG=<seconds>` the panel stays up that long after the checks and prints
//! its address and key, to look at the page in a browser.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::net::SocketAddr;

use guardiana_core::time::now_ms;
use guardiana_core::{Category, Hash, Ledger, NewEvent, Proceso, Verdict, SELF_DEVICE_ID};
use guardiana_panel::{start, Config, RuntimeInfo};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

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
    Hash::of(b"panel antivirus test key")
}

fn proceso(nombre: &str, firma: &str) -> Proceso {
    Proceso {
        pid: 4242,
        nombre: nombre.into(),
        ruta: String::new(),
        sha256: String::new(),
        firmado_por: firma.into(),
    }
}

#[tokio::test]
async fn the_antivirus_page_shows_what_each_antivirus_asked_and_every_official_record() {
    let dir = std::env::temp_dir().join(format!("guardiana-panel-av-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("ledger.db");
    let now = now_ms();
    {
        let mut l = Ledger::open(&db, genesis()).unwrap();
        l.upsert_device(SELF_DEVICE_ID, None, Some("127.0.0.1"), now - 60_000)
            .unwrap();
        l.upsert_device(
            "mac:telefono",
            Some("aa:bb"),
            Some("10.0.0.9"),
            now - 60_000,
        )
        .unwrap();
        let mut ts = now - 3 * 3_600_000;
        let mut pon = |dev: &str, q: &str, p: Option<Proceso>, veces: usize, c: Category| {
            for _ in 0..veces {
                ts += 1_000;
                let mut e = NewEvent::observed(ts, dev, "127.0.0.1", q, "A");
                e.category = c;
                e.process = p.clone();
                if c == Category::Publicidad {
                    e.verdict = Verdict::Cortado;
                }
                l.append(e).unwrap();
            }
        };
        let ms = || Some(proceso("MsMpEng.exe", "Microsoft Windows Publisher"));
        pon(
            SELF_DEVICE_ID,
            "wdcp.microsoft.com",
            ms(),
            12,
            Category::Esperado,
        );
        pon(
            SELF_DEVICE_ID,
            "definitionupdates.microsoft.com",
            ms(),
            3,
            Category::Esperado,
        );
        pon(
            SELF_DEVICE_ID,
            "vortex-win.data.microsoft.com",
            ms(),
            2,
            Category::Telemetria,
        );
        let mb = || Some(proceso("mbamservice.exe", "Malwarebytes Inc."));
        pon(
            SELF_DEVICE_ID,
            "telemetry.malwarebytes.com",
            mb(),
            5,
            Category::Desconocido,
        );
        pon(
            SELF_DEVICE_ID,
            "iris.mwbsys.com",
            mb(),
            2,
            Category::Desconocido,
        );
        pon(
            SELF_DEVICE_ID,
            "googleads.g.doubleclick.net",
            mb(),
            1,
            Category::Publicidad,
        );
        // A browser asking the same advert name is no antivirus.
        pon(
            SELF_DEVICE_ID,
            "googleads.g.doubleclick.net",
            Some(proceso("chrome.exe", "Google LLC")),
            7,
            Category::Publicidad,
        );
        // The phone does not share its detail.
        pon(
            "mac:telefono",
            "wdcp.microsoft.com",
            ms(),
            4,
            Category::Esperado,
        );
        // What Defender's process sent and received, measured (1.0.21): 3 MB up to the US.
        l.suma_trafico(&[guardiana_core::Trafico {
            minuto: now - 30 * 60_000,
            programa: "MsMpEng.exe".into(),
            firma: "Microsoft Windows Publisher".into(),
            destino: "wdcp.microsoft.com".into(),
            pais: "US".into(),
            red: "Microsoft Corporation".into(),
            asn: 8075,
            proto: "tcp".into(),
            puerto: 443,
            enviados: 3_000_000,
            recibidos: 120_000,
            paquetes: 2_100,
        }])
        .unwrap();
    }
    let running = start(Config {
        listen: vec!["127.0.0.1:0".parse().unwrap()],
        optional_listen: Vec::new(),
        db_path: db,
        genesis: genesis(),
        token_path: dir.join("panel.token"),
        extra_hosts: Vec::new(),
        info: RuntimeInfo::default(),
    })
    .await
    .expect("the panel starts");
    let addr = running.addrs[0];
    let t = running.token.clone();

    let (status, body) = get(addr, &t, "/antivirus").await;
    assert_eq!(status, 200);
    assert!(body.contains("data-page=\"antivirus\""));

    let (status, body) = get(addr, &t, "/api/antivirus").await;
    assert_eq!(status, 200, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let vistos = v["vistos"].as_array().unwrap();
    assert_eq!(vistos.len(), 2, "{body}");
    let defender = vistos
        .iter()
        .find(|x| x["producto"] == "Microsoft Defender")
        .unwrap();
    assert_eq!(
        defender["consultas"], 17,
        "the phone is not counted: {body}"
    );
    assert_eq!(defender["no_documentados"], 0);
    assert_eq!(
        defender["programas"][0]["firma"],
        "Microsoft Windows Publisher"
    );
    let mb = vistos
        .iter()
        .find(|x| x["producto"] == "Malwarebytes")
        .unwrap();
    assert_eq!(mb["no_documentados"], 1);
    assert_eq!(mb["cortadas"], 1);
    // What the maker does not document goes first, with whose name it is.
    assert_eq!(mb["destinos"][0]["qname"], "googleads.g.doubleclick.net");
    assert!(mb["destinos"][0]["uso"].is_null());
    assert_eq!(mb["destinos"][0]["empresa"], "Google");
    assert!(!body.contains("chrome.exe"));
    // Every product's record, with official links only.
    let catalogo = v["catalogo"].as_array().unwrap();
    assert!(catalogo.len() >= 13);
    let avast = catalogo.iter().find(|x| x["producto"] == "Avast").unwrap();
    assert!(avast["sanciones"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["fuente"]
            .as_str()
            .unwrap()
            .starts_with("https://www.ftc.gov/")));
    assert_eq!(v["ocultos"], 1);

    // The traffic: Defender sent 3 MB to the US, and it is grouped by country and destination.
    let trafico = v["trafico"].as_array().unwrap();
    let td = trafico
        .iter()
        .find(|x| x["producto"] == "Microsoft Defender")
        .unwrap();
    assert_eq!(td["subidos"], 3_000_000);
    assert_eq!(td["bajados"], 120_000);
    assert_eq!(td["paises"][0]["pais"], "US");
    assert_eq!(td["paises"][0]["subidos"], 3_000_000);
    assert_eq!(td["destinos"][0]["destino"], "wdcp.microsoft.com");
    assert!(v["trafico_horas"].as_i64().unwrap() >= 1);

    // The detect button answers on every system. Outside Windows there is no Security Center,
    // said plainly; on Windows it lists what Windows lists (the CI machine has Defender).
    let (status, body) = get(addr, &t, "/api/antivirus/detectar").await;
    assert_eq!(status, 200, "{body}");
    let d: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(d["detectados"].is_array(), "{body}");
    if cfg!(not(windows)) {
        assert_eq!(d["aviso"], "no_windows");
        assert!(d["detectados"].as_array().unwrap().is_empty());
    }

    if let Some(seg) = std::env::var("GUARDIANA_VISTA_SEG")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
    {
        println!("VISTA http://{addr}/antivirus?t={t}");
        tokio::time::sleep(std::time::Duration::from_secs(seg)).await;
    }
    let _ = std::fs::remove_dir_all(&dir);
}
