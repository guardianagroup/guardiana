//! The measurements of decision 205 through the real HTTP panel: a name dressed as another site's
//! tracker, written the way the engine writes it and shown in its row and in the weekly report;
//! the nights, the companies per day and how often a tracker calls. The extract it reads mixes
//! rows written before decision 205 (the column did not exist) with rows written after, and the
//! chain verifies over all of them. A phone that did not share its detail stays out of all of it,
//! and no answer address is anywhere in the file.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::net::SocketAddr;
use std::path::PathBuf;

use guardiana_core::time::{now_ms, DAY_MS, HOUR_MS};
use guardiana_core::{
    local_day_index, local_midnight, Category, Hash, Ledger, NewEvent, Signal, Verdict,
    SELF_DEVICE_ID,
};
use guardiana_panel::{start, Config, RuntimeInfo};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const CALLADO: &str = "mac:cc-callado";
const COMPARTE: &str = "mac:dd-comparte";
/// The address the disguised name's answer pointed to: looked up and dropped, never stored.
const IP_RESPUESTA: &str = "66.235.152.17";
const DISFRAZADO: &str = "metricas.tienda.example";
const CADENA: [&str; 2] = ["tienda.com.ssl.d1.sc.omtrdc.net", "e1.a.akamaiedge.net"];

async fn get(addr: SocketAddr, token: &str, path: &str) -> (u16, String) {
    let mut s = TcpStream::connect(addr).await.unwrap();
    // Zone 0: the test's days and nights are UTC's, whatever machine runs it.
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nX-Guardiana-Token: {token}\r\nX-Guardiana-Tz: 0\r\n\r\n"
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
    Hash::of(b"panel measurements test key")
}

/// One query, as the engine writes it: category from the lists, the server's country and
/// network looked up and the address dropped, and the disguise worked out from the alias chain.
fn consulta(dev: &str, ts: i64, q: &str, cadena: &[&str]) -> NewEvent {
    static CATALOGO: std::sync::OnceLock<guardiana_lists::Catalog> = std::sync::OnceLock::new();
    let catalogo = CATALOGO.get_or_init(guardiana_lists::Catalog::bundled);
    let mut e = NewEvent::observed(ts, dev, "10.0.0.2", q, "A");
    e.category = catalogo.category(q);
    e.server = Some(guardiana_lists::server_info_de(
        IP_RESPUESTA.parse().unwrap(),
    ));
    let alias: Vec<String> = cadena.iter().map(|s| (*s).to_owned()).collect();
    e.cname_target = catalogo.cloaked_target(q, &alias);
    e
}

fn prepare(tag: &str) -> (PathBuf, PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("guardiana-panel-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("ledger.db");
    let token = dir.join("panel.token");
    let now = now_ms();
    let ayer = local_midnight(local_day_index(now, 0) - 1, 0);
    // A 1.0.14 extract first: rows of before, and no column for the disguise yet.
    {
        let mut l = Ledger::open(&db, genesis()).unwrap();
        l.upsert_device(SELF_DEVICE_ID, None, Some("127.0.0.1"), ayer - DAY_MS)
            .unwrap();
        l.upsert_device(CALLADO, Some("cc:cc"), Some("10.0.0.7"), ayer - DAY_MS)
            .unwrap();
        l.upsert_device(COMPARTE, Some("dd:dd"), Some("10.0.0.8"), ayer - DAY_MS)
            .unwrap();
        l.set_share_detail(COMPARTE, true).unwrap();
        l.rename_device(COMPARTE, "Teléfono de Ana").unwrap();
        let mut viejo = NewEvent::observed(
            ayer - HOUR_MS,
            SELF_DEVICE_ID,
            "127.0.0.1",
            "www.youtube.com",
            "A",
        );
        viejo.server = Some(guardiana_lists::server_info_de(
            "142.250.1.1".parse().unwrap(),
        ));
        l.append(viejo).unwrap();
        l.append(NewEvent::observed(
            ayer - HOUR_MS + 1,
            SELF_DEVICE_ID,
            "127.0.0.1",
            "a.example",
            "A",
        ))
        .unwrap();
        l.connection()
            .execute_batch("ALTER TABLE events DROP COLUMN cname_target;")
            .unwrap();
    }
    // Then this version opens it, and writes on.
    let mut l = Ledger::open(&db, genesis()).unwrap();
    // Yesterday from 01:00 to 02:00, a tracker every five minutes, asked as A and AAAA; the
    // beacon signal fires from the fifth contact on, as the classifier would.
    for i in 0..13 {
        let ts = ayer + HOUR_MS + i * 5 * 60_000;
        for (j, qtype) in ["A", "AAAA"].iter().enumerate() {
            let mut e = consulta(
                SELF_DEVICE_ID,
                ts + j as i64,
                "b.scorecardresearch.com",
                &[],
            );
            e.qtype = (*qtype).to_owned();
            e.category = Category::Rastreador;
            if i >= 4 && j == 0 {
                e.signals = vec![Signal::Baliza { minutes: 5 }];
            }
            l.append(e).unwrap();
        }
    }
    // Yesterday morning, Meta at 07:10; the shared phone at 06:30, disguised.
    let mut fb = consulta(
        SELF_DEVICE_ID,
        ayer + 7 * HOUR_MS + 10 * 60_000,
        "graph.facebook.com",
        &[],
    );
    fb.category = Category::Rastreador;
    l.append(fb).unwrap();
    let d = consulta(
        COMPARTE,
        ayer + 6 * HOUR_MS + 30 * 60_000,
        DISFRAZADO,
        &CADENA,
    );
    assert_eq!(
        d.cname_target.as_deref(),
        Some("omtrdc.net"),
        "the engine's own logic"
    );
    l.append(d).unwrap();
    // A cut one at night: counted as asked, never as having left.
    let mut cortada = consulta(
        SELF_DEVICE_ID,
        ayer + 3 * HOUR_MS,
        "ads.doubleclick.net",
        &[],
    );
    cortada.verdict = Verdict::Cortado;
    cortada.server = None;
    l.append(cortada).unwrap();
    // Yesterday at 12:30, neither night nor morning: the disguised name again, from this computer.
    l.append(consulta(
        SELF_DEVICE_ID,
        ayer + 12 * HOUR_MS + 30 * 60_000,
        DISFRAZADO,
        &CADENA,
    ))
    .unwrap();
    // Today, a little before now (and never before today's midnight): Alphabet.
    let hoy = (now - 120_000).max(local_midnight(local_day_index(now, 0), 0));
    l.append(consulta(SELF_DEVICE_ID, hoy, "www.youtube.com", &[]))
        .unwrap();
    // The silent phone does all of it too, and none of it may show.
    for k in 0..5 {
        let mut e = consulta(
            CALLADO,
            ayer + 2 * HOUR_MS + k,
            "clinica-tracker.example",
            &[],
        );
        e.category = Category::Rastreador;
        l.append(e).unwrap();
    }
    l.append(consulta(
        CALLADO,
        ayer + 2 * HOUR_MS + 10,
        "secreto.tienda-callada.example",
        &CADENA,
    ))
    .unwrap();
    (dir, db, token)
}

#[tokio::test]
async fn the_report_measures_nights_companies_cadence_and_disguises() {
    let (dir, db, token_path) = prepare("medidas");
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
    let t = running.token.clone();

    // The chain verifies over the old rows and the new ones together.
    let (status, body) = get(addr, &t, "/api/extracto/comprobar").await;
    assert_eq!(status, 200, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["ok"], true, "{body}");
    assert_eq!(v["checked"], 2 + 26 + 1 + 1 + 1 + 1 + 1 + 6, "{body}");

    // The disguised name says who answers, in its row; nothing else does.
    let (status, body) = get(addr, &t, "/api/extracto").await;
    assert_eq!(status, 200, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let eventos = v["eventos"].as_array().unwrap();
    let fila = eventos.iter().find(|e| e["qname"] == DISFRAZADO).unwrap();
    assert_eq!(
        fila["disfraz"]["frase"],
        "se presenta como metricas.tienda.example, pero responde Adobe (omtrdc.net)"
    );
    assert_eq!(fila["disfraz"]["sitio"], "omtrdc.net");
    assert_eq!(fila["disfraz"]["empresa"], "Adobe");
    assert!(eventos
        .iter()
        .filter(|e| e["qname"] != DISFRAZADO)
        .all(|e| e.get("disfraz").is_none()));
    assert!(
        !body.contains("tienda-callada"),
        "the silent phone stays out"
    );
    assert!(!body.contains(IP_RESPUESTA));

    // The weekly report.
    let (status, body) = get(addr, &t, "/api/informe").await;
    assert_eq!(status, 200, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let m = &v["medidas"];
    assert_eq!(m["zona_min"], 0);
    assert_eq!(m["dias"].as_array().unwrap().len(), 7);

    // Mientras dormías: this computer's 26 queries of the beacon and the cut one, on one night.
    let noche = m["noche"].as_array().unwrap();
    let pc = noche.iter().find(|n| n["id"] == SELF_DEVICE_ID).unwrap();
    assert_eq!(pc["consultas"], 27, "{pc}");
    assert_eq!(pc["cortadas"], 1, "{pc}");
    assert_eq!(pc["noches"], 1, "{pc}");
    // Yesterday's morning: Meta at 07:10 (minutes after midnight).
    assert_eq!(pc["mananas"][5], 7 * 60 + 10, "{pc}");
    // The shared phone: no night, a morning at 06:30.
    let ana = noche.iter().find(|n| n["id"] == COMPARTE).unwrap();
    assert_eq!(ana["name"], "Teléfono de Ana");
    assert_eq!(ana["consultas"], 0);
    assert_eq!(ana["mananas"][5], 6 * 60 + 30, "{ana}");
    assert_eq!(m["noche_total"], 27);

    // Companies per day: today this computer asked Alphabet (YouTube); yesterday Meta, and
    // Adobe behind the disguised name, were forwarded; the cut DoubleClick was not.
    let empresas = m["empresas"].as_array().unwrap();
    let pc = empresas.iter().find(|e| e["id"] == SELF_DEVICE_ID).unwrap();
    let hoy: Vec<&str> = pc["hoy"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g.as_str().unwrap())
        .collect();
    assert!(hoy.contains(&"Alphabet"), "{pc}");
    assert!(m["empresas_hoy"]
        .as_array()
        .unwrap()
        .iter()
        .any(|g| g == "Alphabet"));
    let ayer = pc["por_dia"][5].as_i64().unwrap();
    assert!(ayer >= 2, "Meta and Adobe at least: {pc}");
    let ana = empresas.iter().find(|e| e["id"] == COMPARTE).unwrap();
    assert_eq!(ana["por_dia"][5], 1, "Adobe, behind the disguise: {ana}");

    // How often: the beacon's tracker, 13 contacts, every five minutes, at night.
    let cad = m["cadencias"].as_array().unwrap();
    let b = cad
        .iter()
        .find(|c| c["nombre"] == "b.scorecardresearch.com")
        .unwrap();
    assert_eq!(b["contactos"], 13, "{b}");
    assert_eq!(b["mediana_ms"], 5 * 60_000, "{b}");
    assert_eq!(b["baliza"], true);
    assert_eq!(b["frase"], "llama cada ~5 min, también de noche");

    // The disguised names: one, asked twice, by two devices that share; never the silent one.
    let dis = m["disfrazados"].as_array().unwrap();
    assert_eq!(dis.len(), 1, "{dis:?}");
    assert_eq!(dis[0]["nombre"], DISFRAZADO);
    assert_eq!(dis[0]["consultas"], 2);
    assert_eq!(dis[0]["aparatos"], 2);
    assert_eq!(m["disfrazados_consultas"], 2);

    // The silent phone is nowhere in the measurements (its week is counted as figures in the
    // table above them, as it always was, and never by name).
    let medidas = m.to_string();
    assert!(!medidas.contains(CALLADO), "{medidas}");
    assert!(!body.contains("clinica-tracker"), "{body}");
    assert!(!body.contains("tienda-callada"), "{body}");
    // A disguised name in the cadence names the company behind it.
    let d = cad.iter().find(|c| c["nombre"] == DISFRAZADO).unwrap();
    assert_eq!(d["empresa"], "Adobe", "{d}");

    // The text to share: two short sentences more, without a device or a site named.
    let texto = v["texto_whatsapp"].as_str().unwrap();
    assert!(
        texto.contains("Entre las 0:00 y las 6:00 de estos 7 días, los aparatos hicieron 27 consultas a rastreadores, publicidad o telemetría."),
        "{texto}"
    );
    assert!(
        texto.contains("1 nombre se presentaba como parte de una web y respondía como rastreo de otra empresa."),
        "{texto}"
    );
    assert!(!texto.contains(DISFRAZADO) && !texto.contains("omtrdc"));

    // The spreadsheet carries the disguise, which the hash covers.
    let (status, csv) = get(addr, &t, "/api/extracto/exportar").await;
    assert_eq!(status, 200);
    assert!(csv.contains(";cname_destino;"), "{csv}");
    assert!(csv.contains(";omtrdc.net;"), "{csv}");

    running.shutdown().await;

    // The file holds the site's name and never the answer's address.
    let mut datos = std::fs::read(&db).unwrap();
    datos.extend(std::fs::read(dir.join("ledger.db-wal")).unwrap_or_default());
    assert!(datos.windows(10).any(|w| w == b"omtrdc.net"));
    for ip in [IP_RESPUESTA.as_bytes(), b"142.250.1.1"] {
        assert!(
            !datos.windows(ip.len()).any(|w| w == ip),
            "{}",
            String::from_utf8_lossy(ip)
        );
    }
    // Nor any other name of the chain than the site kept.
    for alias in CADENA {
        assert!(
            !datos.windows(alias.len()).any(|w| w == alias.as_bytes()),
            "{alias}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
