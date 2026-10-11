//! «Haz que te borren» (decision 204), through the real HTTP panel: what the house asked of each
//! company, counted right and without the devices that keep their detail to themselves; the
//! letter with the article of the chosen law; and the clock, which keeps the company, the law and
//! the day, and nothing that could name the person.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::net::SocketAddr;
use std::path::PathBuf;

use guardiana_core::time::now_ms;
use guardiana_core::{Hash, Ledger, NewEvent, Verdict, SELF_DEVICE_ID};
use guardiana_panel::{start, Config, RuntimeInfo};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const CALLADO: &str = "mac:cc-callado";
const COMPARTE: &str = "mac:dd-comparte";
const NOMBRE: &str = "Zuleima Quintanilla";
const CORREO: &str = "zuleima@secreto.example";

async fn pide(
    addr: SocketAddr,
    token: &str,
    metodo: &str,
    path: &str,
    body: &str,
) -> (u16, String) {
    let mut s = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "{metodo} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nX-Guardiana-Token: {token}\r\nX-Guardiana-Lang: es\r\nX-Guardiana-Tz: 0\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
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
    Hash::of(b"panel deletion test key")
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
    l.upsert_device(COMPARTE, Some("dd:dd"), Some("10.0.0.8"), now - 60_000)
        .unwrap();
    l.rename_device(COMPARTE, "Tablet de la cocina").unwrap();
    l.set_share_detail(COMPARTE, true).unwrap();
    let mut ts = now - 50_000;
    let mut pon = |dev: &str, q: &str, veces: usize, v: Verdict| {
        for _ in 0..veces {
            ts += 10;
            let mut e = NewEvent::observed(ts, dev, "127.0.0.1", q, "A");
            e.verdict = v;
            l.append(e).unwrap();
        }
    };
    pon(SELF_DEVICE_ID, "www.spokeo.com", 4, Verdict::Observado);
    pon(SELF_DEVICE_ID, "www.spokeo.com", 1, Verdict::Cortado);
    pon(COMPARTE, "thatsthem.com", 2, Verdict::Observado);
    pon(SELF_DEVICE_ID, "static.criteo.net", 3, Verdict::Observado);
    pon(SELF_DEVICE_ID, "example.com", 9, Verdict::Observado);
    // The silent phone asked for Spokeo too, and alone for a people-search site: none of it may
    // show up, not as a count, not as a device.
    pon(CALLADO, "www.spokeo.com", 7, Verdict::Observado);
    pon(CALLADO, "radaris.com", 5, Verdict::Observado);
    (dir, db, token)
}

#[tokio::test]
async fn the_page_counts_right_writes_the_letter_and_keeps_no_name() {
    let (dir, db, token_path) = prepare("borrado");
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

    // The page itself, and the nav entry on another page.
    let (status, html) = pide(addr, &t, "GET", "/borrado", "").await;
    assert_eq!(status, 200);
    assert!(html.contains("data-page=\"borrado\""));
    for paso in [
        "borrado_paso1",
        "borrado_paso4",
        "borrado_ver_ley",
        "borrado_mailto",
    ] {
        assert!(html.contains(paso), "{paso}");
    }
    let (_, inicio) = pide(addr, &t, "GET", "/", "").await;
    assert!(inicio.contains("href=\"/borrado\""), "the nav links to it");

    // What the house saw, by company.
    let (status, body) = pide(addr, &t, "GET", "/api/borrado", "").await;
    assert_eq!(status, 200, "{body}");
    assert!(!body.contains(CALLADO), "the silent phone is never named");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["callados"], 1);
    let corredores = v["corredores"].as_array().unwrap();
    let de = |id: &str| corredores.iter().find(|c| c["id"] == id).unwrap().clone();
    let spokeo = de("spokeo");
    // 4 + 1 cut from this computer, 2 from the tablet that shares; the silent phone's 7 are out.
    assert_eq!(spokeo["evidencia"]["consultas"], 7, "{spokeo}");
    assert_eq!(spokeo["evidencia"]["cortadas"], 1, "{spokeo}");
    assert_eq!(spokeo["evidencia"]["nombres"].as_array().unwrap().len(), 2);
    let aparatos: Vec<&str> = spokeo["aparatos"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a[0].as_str().unwrap())
        .collect();
    assert_eq!(aparatos, vec![SELF_DEVICE_ID, COMPARTE]);
    assert_eq!(spokeo["aparatos"][1][1], "Tablet de la cocina");
    let frase = spokeo["frase"].as_str().unwrap();
    assert!(
        frase.contains("7 veces") && frase.contains("2 nombres"),
        "{frase}"
    );
    assert!(
        !frase.contains("tiene tus datos"),
        "evidence, never a verdict: {frase}"
    );
    assert_eq!(de("criteo")["evidencia"]["consultas"], 3);
    assert_eq!(
        de("radaris")["evidencia"]["consultas"],
        0,
        "only the silent phone asked"
    );
    assert_eq!(de("radaris")["estado"], "incautado");
    // The busiest first.
    assert_eq!(corredores[0]["id"], "spokeo");
    // Every jurisdiction comes with its texts in the panel's language.
    let jur = v["jurisdicciones"].as_array().unwrap();
    assert!(jur.len() >= 20);
    let es = jur.iter().find(|j| j["codigo"] == "es").unwrap();
    assert!(es["articulo"].as_str().unwrap().contains("artículo 17"));
    assert_eq!(
        es["corta"],
        "Plazo: 1 mes (+2 meses). Si no responden: AEPD."
    );
    assert_eq!(v["envios"].as_array().unwrap().len(), 0);

    // The letter: the article of the chosen law, in the chosen language, with the markers the
    // page fills in. A name or an e-mail in the query is not read.
    let (status, body) = pide(
        addr,
        &t,
        "GET",
        &format!(
            "/api/borrado/carta?corredor=spokeo&jurisdiccion=es&idioma=en&nombre={}&correo={CORREO}",
            NOMBRE.replace(' ', "%20")
        ),
        "",
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let c: serde_json::Value = serde_json::from_str(&body).unwrap();
    let cuerpo = c["cuerpo"].as_str().unwrap();
    assert!(cuerpo.contains("Article 17 of the GDPR"), "{cuerpo}");
    assert!(cuerpo.contains("[[NOMBRE]]") && cuerpo.contains("[[CORREO]]"));
    assert!(!cuerpo.contains("Zuleima") && !cuerpo.contains("secreto"));
    assert!(
        cuerpo.contains("www.spokeo.com"),
        "the evidence goes in: {cuerpo}"
    );
    assert!(!cuerpo.contains("radaris"), "{cuerpo}");
    assert_eq!(c["idioma"], "en");
    assert_eq!(
        c["para"], "CustomerCare@spokeo.com",
        "the EU address for Spain"
    );
    let (_, body) = pide(
        addr,
        &t,
        "GET",
        "/api/borrado/carta?corredor=spokeo&jurisdiccion=us_ca&idioma=pt&evidencia=0",
        "",
    )
    .await;
    let c: serde_json::Value = serde_json::from_str(&body).unwrap();
    let cuerpo = c["cuerpo"].as_str().unwrap();
    assert!(cuerpo.contains("1798.105"), "{cuerpo}");
    assert!(
        !cuerpo.contains("www.spokeo.com"),
        "evidence left out on request"
    );
    assert!(
        c["para"].is_null(),
        "no address is published for California"
    );
    let (status, _) = pide(
        addr,
        &t,
        "GET",
        "/api/borrado/carta?corredor=nadie&jurisdiccion=es",
        "",
    )
    .await;
    assert_eq!(status, 404);

    // The clock: a body with a name in it is refused whole.
    let (status, _) = pide(
        addr,
        &t,
        "POST",
        "/api/borrado/envios",
        &format!(r#"{{"corredor":"spokeo","jurisdiccion":"es","enviada":"2026-10-01","nombre":"{NOMBRE}","correo":"{CORREO}"}}"#),
    )
    .await;
    assert!((400..500).contains(&status), "{status}");
    let (status, _) = pide(
        addr,
        &t,
        "POST",
        "/api/borrado/envios",
        r#"{"corredor":"spokeo","jurisdiccion":"marte","enviada":"2026-10-01"}"#,
    )
    .await;
    assert_eq!(status, 400);
    let (status, _) = pide(
        addr,
        &t,
        "POST",
        "/api/borrado/envios",
        r#"{"corredor":"spokeo","jurisdiccion":"es","enviada":"2999-01-01"}"#,
    )
    .await;
    assert_eq!(status, 400, "a letter sent in the future");
    let hoy = guardiana_core::time::local_day(now_ms(), 0);
    for (corredor, jur) in [("spokeo", "es"), ("criteo", "co")] {
        let (status, body) = pide(
            addr,
            &t,
            "POST",
            "/api/borrado/envios",
            &format!(r#"{{"corredor":"{corredor}","jurisdiccion":"{jur}","enviada":"{hoy}"}}"#),
        )
        .await;
        assert_eq!(status, 200, "{body}");
    }
    let (_, body) = pide(addr, &t, "GET", "/api/borrado", "").await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let envios = v["envios"].as_array().unwrap();
    assert_eq!(envios.len(), 2);
    assert_eq!(envios[0]["vencido"], false);
    assert!(envios[0]["dias"].as_i64().unwrap() >= 28, "{}", envios[0]);
    assert!(
        envios[0]["queja"].is_null(),
        "no complaint before the deadline"
    );
    let id = envios[1]["id"].as_u64().unwrap();
    let (status, body) = pide(
        addr,
        &t,
        "POST",
        &format!("/api/borrado/envios/{id}/quitar"),
        "",
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let lista: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(lista.as_array().unwrap().len(), 1);

    running.shutdown().await;

    // What was kept: the company, the law and the day, in the settings. Nothing else, and no
    // trace of the name or the address anywhere in the database files.
    let l = Ledger::open(&db, genesis()).unwrap();
    let guardado = l.setting("borrado_envios").unwrap().unwrap();
    let v: serde_json::Value = serde_json::from_str(&guardado).unwrap();
    let uno = v.as_array().unwrap()[0].as_object().unwrap();
    let mut campos: Vec<&str> = uno.keys().map(String::as_str).collect();
    campos.sort_unstable();
    assert_eq!(campos, vec!["corredor", "enviada", "id", "jurisdiccion"]);
    drop(l);
    for entrada in std::fs::read_dir(&dir).unwrap() {
        let p = entrada.unwrap().path();
        let bytes = std::fs::read(&p).unwrap();
        let texto = String::from_utf8_lossy(&bytes);
        for prohibido in ["Zuleima", "Quintanilla", "secreto.example"] {
            assert!(
                !texto.contains(prohibido),
                "{prohibido} written to {}",
                p.display()
            );
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
