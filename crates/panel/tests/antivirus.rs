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

async fn post(addr: SocketAddr, token: &str, path: &str, body: &str) -> (u16, String) {
    let mut s = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nX-Guardiana-Token: {token}\r\nX-Guardiana-Lang: es\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
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
            lugar: "azure:eastus".into(),
        }])
        .unwrap();
        // One recent minute too, so the live panel has something to show.
        l.suma_trafico(&[guardiana_core::Trafico {
            minuto: now - 2 * 60_000,
            programa: "MsMpEng.exe".into(),
            firma: "Microsoft Windows Publisher".into(),
            destino: "vortex-win.data.microsoft.com".into(),
            pais: "US".into(),
            red: "Microsoft Corporation".into(),
            asn: 8075,
            proto: "tcp".into(),
            puerto: 443,
            enviados: 44_000,
            recibidos: 3_000,
            paquetes: 40,
            lugar: String::new(),
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
    assert_eq!(td["subidos"], 3_044_000);
    assert_eq!(td["bajados"], 123_000);
    assert_eq!(td["paises"][0]["pais"], "US");
    assert_eq!(td["paises"][0]["subidos"], 3_044_000);
    assert_eq!(td["destinos"][0]["destino"], "wdcp.microsoft.com");
    assert!(v["trafico_horas"].as_i64().unwrap() >= 1);
    // The live panel shows the recent minute, newest first, only for antivirus.
    let vivo = v["vivo"].as_array().unwrap();
    assert!(!vivo.is_empty());
    assert_eq!(vivo[0]["destino"], "vortex-win.data.microsoft.com");
    assert_eq!(vivo[0]["producto"], "Microsoft Defender");

    // Export: a CSV with the header and Defender's rows, never an address.
    let (status, csv) = get(addr, &t, "/api/antivirus/export").await;
    assert_eq!(status, 200);
    assert!(csv.starts_with("minuto;fecha_hora;producto;"));
    assert!(csv.contains("Microsoft Defender;MsMpEng.exe"));
    assert!(csv.contains("wdcp.microsoft.com"));

    // The 24-hour summary on top: Defender's 3 MB to the US, one country, watching on by default.
    let r24 = &v["resumen"];
    assert_eq!(r24["subidos"], 3_044_000);
    assert_eq!(r24["bajados"], 123_000);
    assert_eq!(r24["paises"], 1);
    assert_eq!(r24["productos"][0], "Microsoft Defender");
    assert_eq!(v["vigilando"], true);
    assert_eq!(v["maxima"], false);

    // Watching can be turned off and on; it is remembered and reported.
    let (status, body) = post(addr, &t, "/api/antivirus/vigilar", r#"{"activa":false}"#).await;
    assert_eq!(status, 200, "{body}");
    let (_, body) = get(addr, &t, "/api/antivirus").await;
    let v3: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v3["vigilando"], false);
    let (status, _) = post(addr, &t, "/api/antivirus/vigilar", r#"{"activa":true}"#).await;
    assert_eq!(status, 200);
    let (_, body) = get(addr, &t, "/api/antivirus").await;
    let v3: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v3["vigilando"], true);

    // «Cortar todo lo que envía el antivirus» (until 13 Oct 2026 «Protección máxima del
    // antivirus»): letting everything through always works; cutting everything needs a product
    // with a path seen (this test ledger has none) and, off Windows, says so.
    let (status, body) = post(addr, &t, "/api/antivirus/maxima", r#"{"activa":false}"#).await;
    assert_eq!(status, 200, "{body}");
    let (status, _) = post(addr, &t, "/api/antivirus/maxima", r#"{"activa":true}"#).await;
    assert_eq!(status, 409);

    // The installed antivirus are asked of Windows by the page itself, with no button.
    // The detect endpoint answers on every system. Outside Windows there is no Security Center,
    // said plainly; on Windows it lists what Windows lists (the CI machine has Defender).
    let (status, body) = get(addr, &t, "/api/antivirus/detectar").await;
    assert_eq!(status, 200, "{body}");
    let d: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(d["detectados"].is_array(), "{body}");
    if cfg!(not(windows)) {
        assert_eq!(d["aviso"], "no_windows");
        assert!(d["detectados"].as_array().unwrap().is_empty());
    }

    // The clean cut: an unknown product is refused; off Windows (or not as the system) cutting
    // is unavailable, said so, and nothing is recorded as cut.
    let (status, _) = post(
        addr,
        &t,
        "/api/antivirus/cortar",
        r#"{"producto":"Nadie","cortar":true}"#,
    )
    .await;
    assert_eq!(status, 400);
    if cfg!(not(windows)) {
        assert_eq!(v["cortar_disponible"], false);
        let (status, body) = post(
            addr,
            &t,
            "/api/antivirus/cortar",
            r#"{"producto":"Microsoft Defender","cortar":true}"#,
        )
        .await;
        assert_eq!(status, 503, "{body}");
        assert!(body.contains("Windows"), "{body}");
        let (_, body) = get(addr, &t, "/api/antivirus").await;
        let v2: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(v2["cortados"].as_array().unwrap().is_empty());
    }

    // «GUARDIANA por dentro» (1.0.22): the same measurements, for the live globe.
    let (status, body) = get(addr, &t, "/api/por-dentro").await;
    assert_eq!(status, 200, "{body}");
    let u: serde_json::Value = serde_json::from_str(&body).unwrap();
    // Whether this system measures the bytes of each program: only Windows does, and elsewhere
    // the page says so instead of waiting for measurements that never come.
    assert_eq!(u["mide_bytes"], cfg!(windows), "{body}");
    assert_eq!(u["maxima"], false, "{body}");
    let progs = u["programas"].as_array().unwrap();
    assert!(
        progs.iter().any(|p| p["id"] == "Microsoft Defender"
            && p["tipo"] == "antivirus"
            && p["cortable"] == true),
        "{body}"
    );
    // A browser is watched too, and it is never cut whole (it would stop working).
    assert!(
        progs
            .iter()
            .any(|p| p["id"] == "Chrome" && p["tipo"] == "navegador" && p["cortable"] == false),
        "{body}"
    );
    // The antivirus comes first.
    assert_eq!(progs[0]["tipo"], "antivirus");
    let dest = u["destinos"].as_array().unwrap();
    let wdcp = dest
        .iter()
        .find(|d| d["destino"] == "wdcp.microsoft.com" && d["programa"] == "Microsoft Defender")
        .unwrap();
    assert_eq!(wdcp["enviados"], 3_000_000);
    assert_eq!(wdcp["recibidos"], 120_000);
    assert_eq!(wdcp["paquetes"], 2_100);
    assert_eq!(wdcp["pais"], "US");
    assert_eq!(wdcp["red"], "Microsoft Corporation");
    assert_eq!(wdcp["asn"], 8075);
    assert_eq!(wdcp["proto"], "tcp");
    assert_eq!(wdcp["puerto"], 443);
    assert_eq!(wdcp["vias"][0], "tcp/443");
    assert_eq!(wdcp["consultas"], 12);
    assert_eq!(wdcp["sede"], false);
    assert!(wdcp["documentado"].is_boolean(), "{wdcp}");
    // Never the server's address (decision 203).
    assert!(!body.contains("\"ip\""), "{body}");
    // A name stopped before it left has no server: it is placed at its company's seat, and says
    // so; for a browser, «documented by the maker» does not apply.
    let ads = dest
        .iter()
        .find(|d| d["destino"] == "googleads.g.doubleclick.net" && d["programa"] == "Chrome")
        .unwrap();
    assert_eq!(ads["sede"], true);
    assert_eq!(ads["paradas"], 7);
    assert_eq!(ads["enviados"], 0);
    assert!(ads["documentado"].is_null());
    // The latest minutes, row by row, for the live particles.
    assert!(u["recientes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["destino"] == "vortex-win.data.microsoft.com" && r["enviados"] == 44_000));
    assert_eq!(u["ventana_h"], 24);
    // The page, its own script and the Earth's pictures come from the panel itself (no internet).
    let (status, page) = get(addr, &t, "/por-dentro").await;
    assert_eq!(status, 200);
    assert!(page.contains("data-page=\"por-dentro\""));
    assert!(
        page.contains("/static/por-dentro.js?v="),
        "the script is asked for by its tag"
    );
    let (status, script) = get(addr, &t, "/static/por-dentro.js").await;
    assert_eq!(status, 200);
    assert!(script.contains("GuardianaUniverso"));
    for imagen in ["dia.jpg", "luces.jpg", "agua.png"] {
        let (status, _) = get(addr, &t, &format!("/static/tierra/{imagen}")).await;
        assert_eq!(status, 200, "{imagen}");
    }
    let (status, _) = get(addr, &t, "/static/tierra/otra.jpg").await;
    assert_eq!(status, 404);
    // The way in is on the start page and on «Tu antivirus».
    for pagina in ["/", "/antivirus"] {
        let (_, html) = get(addr, &t, pagina).await;
        assert!(html.contains("href=\"/por-dentro\""), "{pagina}");
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
