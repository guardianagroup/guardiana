//! «Expedientes» (1.0.17), through the real HTTP panel: the clocks of «Haz que te borren» become
//! files once; a file is opened, sent, answered and closed; the letter and the complaint say the
//! law, the dates and the authority; every page hears about the overdue ones; and nothing that
//! could name the person is ever kept.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::net::SocketAddr;
use std::path::PathBuf;

use guardiana_core::time::{local_day, now_ms};
use guardiana_core::{Hash, Ledger, NewEvent, Verdict, SELF_DEVICE_ID};
use guardiana_panel::{start, Config, RuntimeInfo};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const NOMBRE: &str = "Zuleima Quintanilla";
const CORREO: &str = "zuleima@secreto.example";
const DIA: i64 = 86_400_000;

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
    Hash::of(b"panel case files test key")
}

fn prepare(tag: &str, hace_90: &str) -> (PathBuf, PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("guardiana-panel-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("ledger.db");
    let token = dir.join("panel.token");
    let now = now_ms();
    let mut l = Ledger::open(&db, genesis()).unwrap();
    l.upsert_device(SELF_DEVICE_ID, None, Some("127.0.0.1"), now - 60_000)
        .unwrap();
    let mut ts = now - 50_000;
    for (q, veces) in [
        ("www.spokeo.com", 3),
        ("static.criteo.net", 5),
        ("www.clarity.ms", 2),
    ] {
        for _ in 0..veces {
            ts += 10;
            let mut e = NewEvent::observed(ts, SELF_DEVICE_ID, "127.0.0.1", q, "A");
            e.verdict = Verdict::Observado;
            l.append(e).unwrap();
        }
    }
    // A clock of «Haz que te borren» from before 1.0.17, sent three months ago under Spanish law:
    // overdue.
    l.set_setting(
        "borrado_envios",
        &format!(r#"[{{"id":1,"corredor":"spokeo","jurisdiccion":"es","enviada":"{hace_90}"}}]"#),
    )
    .unwrap();
    (dir, db, token)
}

fn json(body: &str) -> Value {
    serde_json::from_str(body)
        .map_err(|e| format!("{e}: {body}"))
        .unwrap()
}

#[tokio::test]
async fn files_open_send_answer_close_and_complain_without_a_name() {
    let hoy = local_day(now_ms(), 0);
    let hace_90 = local_day(now_ms() - 90 * DIA, 0);
    let (dir, db, token_path) = prepare("expedientes", &hace_90);
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

    // The page, and the nav entry on the others.
    let (status, html) = pide(addr, &t, "GET", "/expedientes", "").await;
    assert_eq!(status, 200);
    assert!(html.contains("data-page=\"expedientes\""));
    let (_, inicio) = pide(addr, &t, "GET", "/foto", "").await;
    assert!(
        inicio.contains("href=\"/expedientes\""),
        "the nav links to it"
    );

    // The old clock became a file, sent, overdue, with the authority to complain to.
    let (status, body) = pide(addr, &t, "GET", "/api/expedientes", "").await;
    assert_eq!(status, 200, "{body}");
    let v = json(&body);
    let lista = v["expedientes"].as_array().unwrap();
    assert_eq!(lista.len(), 1, "{body}");
    let viejo = &lista[0];
    assert_eq!(viejo["empresa"], "Spokeo");
    assert_eq!(viejo["corredor"], "spokeo");
    assert_eq!(viejo["tipo"], "borrado");
    assert_eq!(viejo["estado"], "enviada");
    assert_eq!(viejo["enviada"], hace_90.as_str());
    assert_eq!(viejo["vencido"], true, "{viejo}");
    assert!(viejo["dias"].as_i64().unwrap() < 0);
    assert!(viejo["autoridad"].as_str().unwrap().contains("AEPD"));
    assert!(viejo["autoridad_url"]
        .as_str()
        .unwrap()
        .starts_with("https://"));
    assert_eq!(v["vencidos"], 1);
    assert_eq!(v["hoy"], hoy.as_str());
    assert!(v["jurisdicciones"].as_array().unwrap().len() >= 20);
    assert_eq!(v["resultados"].as_array().unwrap().len(), 6);
    let vistas = v["vistas"].as_array().unwrap();
    let criteo = vistas.iter().find(|e| e["empresa"] == "Criteo").unwrap();
    assert_eq!(criteo["consultas"], 5);
    assert_eq!(criteo["corredor"], "criteo");
    assert!(v["corredores"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["corredor"] == "spokeo"));
    // Every page's banner hears about it.
    let (_, body) = pide(addr, &t, "GET", "/api/estado", "").await;
    let e = json(&body);
    assert_eq!(e["exp_vencidos"], 1, "{body}");
    // And «Haz que te borren» still shows it as its clock, from the same file.
    let (_, body) = pide(addr, &t, "GET", "/api/borrado", "").await;
    assert_eq!(json(&body)["envios"].as_array().unwrap().len(), 1);

    // Opening: a body with a name in it is refused whole; unknown places, companies and future
    // days too.
    for (malo, codigos) in [
        (
            format!(r#"{{"empresa":"Microsoft","jurisdiccion":"co","nombre":"{NOMBRE}","correo":"{CORREO}"}}"#),
            400..500,
        ),
        (r#"{"empresa":"Microsoft","jurisdiccion":"marte"}"#.to_owned(), 404..405),
        (r#"{"empresa":"Nadie que exista SL","jurisdiccion":"co"}"#.to_owned(), 404..405),
        (r#"{"jurisdiccion":"co"}"#.to_owned(), 400..401),
        (
            r#"{"empresa":"Microsoft","jurisdiccion":"co","estado":"enviada","fecha":"2999-01-01"}"#.to_owned(),
            400..401,
        ),
        (
            r#"{"empresa":"Microsoft","jurisdiccion":"co","estado":"cerrada"}"#.to_owned(),
            400..401,
        ),
    ] {
        let (status, body) = pide(addr, &t, "POST", "/api/expedientes", &malo).await;
        assert!(codigos.contains(&status), "{status} {body} for {malo}");
    }

    // An access request to Microsoft under Colombian law, prepared: no clock yet.
    let (status, body) = pide(
        addr,
        &t,
        "POST",
        "/api/expedientes",
        r#"{"empresa":"Microsoft","jurisdiccion":"co","tipo":"acceso"}"#,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let lista = json(&body);
    let nuevo = &lista[0];
    assert_eq!(nuevo["empresa"], "Microsoft");
    assert!(nuevo["corredor"].is_null(), "{nuevo}");
    assert_eq!(nuevo["tipo"], "acceso");
    assert_eq!(nuevo["estado"], "preparada");
    assert!(nuevo["vence"].is_null() && nuevo["dias"].is_null());
    let id = nuevo["id"].as_u64().unwrap();
    assert_eq!(id, 2);

    // Its letter: the access article of Colombia, the evidence, the markers the page fills in.
    let (status, body) = pide(addr, &t, "GET", &format!("/api/expedientes/{id}/carta"), "").await;
    assert_eq!(status, 200, "{body}");
    let c = json(&body);
    let cuerpo = c["cuerpo"].as_str().unwrap();
    assert!(c["asunto"].as_str().unwrap().contains("acceso"), "{body}");
    assert!(cuerpo.contains("Ley 1581 de 2012"), "{cuerpo}");
    assert!(cuerpo.contains("artículo 14"), "{cuerpo}");
    assert!(cuerpo.contains("[[NOMBRE]]") && cuerpo.contains("[[CORREO]]"));
    assert!(
        cuerpo.contains("www.clarity.ms"),
        "the evidence goes in: {cuerpo}"
    );
    let (_, body) = pide(
        addr,
        &t,
        "GET",
        &format!("/api/expedientes/{id}/carta?evidencia=0&idioma=en"),
        "",
    )
    .await;
    let c = json(&body);
    assert_eq!(c["idioma"], "en");
    assert!(!c["cuerpo"].as_str().unwrap().contains("clarity.ms"));

    // Sent today: the clock of Colombia for access, ten working days.
    let (status, body) = pide(
        addr,
        &t,
        "POST",
        &format!("/api/expedientes/{id}"),
        r#"{"estado":"enviada"}"#,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let lista = json(&body);
    let e = lista
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == id)
        .unwrap();
    assert_eq!(e["estado"], "enviada");
    assert_eq!(e["enviada"], hoy.as_str());
    let dias = e["dias"].as_i64().unwrap();
    assert!((12..=17).contains(&dias), "ten working days: {e}");
    assert_eq!(e["vencido"], false);
    assert!(e["queja"].is_null(), "no complaint before the deadline");

    // Answered, with what it said and a note; then closed. A step in the future is refused, and so
    // is anything that is not a step or a note.
    let (status, _) = pide(
        addr,
        &t,
        "POST",
        &format!("/api/expedientes/{id}"),
        r#"{"estado":"respondida","fecha":"2999-01-01"}"#,
    )
    .await;
    assert_eq!(status, 400);
    let (status, _) = pide(
        addr,
        &t,
        "POST",
        &format!("/api/expedientes/{id}"),
        &format!(r#"{{"estado":"respondida","correo":"{CORREO}"}}"#),
    )
    .await;
    assert!((400..500).contains(&status), "{status}");
    let nota = "Llegó un PDF con 3 hojas. ".repeat(40);
    let (status, body) = pide(
        addr,
        &t,
        "POST",
        &format!("/api/expedientes/{id}"),
        &serde_json::json!({"estado":"respondida","resultado":"datos_entregados","notas":nota})
            .to_string(),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let lista = json(&body);
    let e = lista
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == id)
        .unwrap();
    assert_eq!(e["estado"], "respondida");
    assert_eq!(e["resultado"], "datos_entregados");
    assert_eq!(e["respondida"], hoy.as_str());
    assert!(e["dias"].is_null(), "the clock stops once answered: {e}");
    assert_eq!(
        e["notas"].as_str().unwrap().chars().count(),
        600,
        "notes are capped"
    );
    let (status, body) = pide(
        addr,
        &t,
        "POST",
        &format!("/api/expedientes/{id}"),
        r#"{"estado":"cerrada"}"#,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let (status, _) = pide(
        addr,
        &t,
        "POST",
        "/api/expedientes/999",
        r#"{"estado":"cerrada"}"#,
    )
    .await;
    assert_eq!(status, 404);

    // The complaint for the old one: the authority, what was asked and when, that it did not
    // answer, and the markers.
    let (status, body) = pide(addr, &t, "GET", "/api/expedientes/1/queja", "").await;
    assert_eq!(status, 200, "{body}");
    let q = json(&body);
    let cuerpo = q["cuerpo"].as_str().unwrap();
    assert!(q["asunto"].as_str().unwrap().contains("Spokeo"));
    assert!(cuerpo.contains("AEPD"), "{cuerpo}");
    assert!(cuerpo.contains("artículo 17"), "{cuerpo}");
    assert!(cuerpo.contains("no he recibido respuesta"), "{cuerpo}");
    assert!(cuerpo.contains("[[NOMBRE]]") && cuerpo.contains("[[CORREO]]"));
    assert!(q["url"].as_str().unwrap().starts_with("https://"));

    // The overview now: two files, one overdue; the counts the banner reads.
    let (_, body) = pide(addr, &t, "GET", "/api/expedientes", "").await;
    let v = json(&body);
    assert_eq!(v["expedientes"].as_array().unwrap().len(), 2);
    assert_eq!(v["vencidos"], 1);
    assert_eq!(v["expedientes"][0]["estado"], "cerrada", "newest first");

    // Removing the old one also removes it from «Haz que te borren».
    let (status, body) = pide(addr, &t, "POST", "/api/expedientes/1/quitar", "").await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(json(&body).as_array().unwrap().len(), 1);
    let (_, body) = pide(addr, &t, "GET", "/api/borrado", "").await;
    assert_eq!(json(&body)["envios"].as_array().unwrap().len(), 0);
    let (_, body) = pide(addr, &t, "GET", "/api/estado", "").await;
    assert_eq!(json(&body)["exp_vencidos"], 0);

    running.shutdown().await;

    // What was kept: the migration emptied the old setting, and the file holds the company, the
    // law, the kind, the steps, the result and the note. No name or address anywhere on disk.
    let l = Ledger::open(&db, genesis()).unwrap();
    assert_eq!(l.setting("borrado_envios").unwrap().as_deref(), Some("[]"));
    let guardado = l.setting("expedientes").unwrap().unwrap();
    let v = json(&guardado);
    let uno = v.as_array().unwrap()[0].as_object().unwrap();
    let mut campos: Vec<&str> = uno.keys().map(String::as_str).collect();
    campos.sort_unstable();
    assert_eq!(
        campos,
        vec![
            "cerrada",
            "creada",
            "empresa",
            "enviada",
            "estado",
            "id",
            "jurisdiccion",
            "notas",
            "respondida",
            "resultado",
            "tipo"
        ]
    );
    drop(l);
    for entrada in std::fs::read_dir(&dir).unwrap() {
        let p = entrada.unwrap().path();
        let bytes = std::fs::read(&p).unwrap();
        let texto = String::from_utf8_lossy(&bytes);
        assert!(!texto.contains("Zuleima"), "{}", p.display());
        assert!(!texto.contains("secreto.example"), "{}", p.display());
    }
    let _ = std::fs::remove_dir_all(&dir);
}
