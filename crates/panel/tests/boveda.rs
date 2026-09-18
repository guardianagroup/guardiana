//! The vault page, end to end over HTTP: no token → 401; create (24 words), store, list, download
//! the same bytes, lock, wrong password → 401, open, log chain intact, exit stops the server.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use guardiana_panel::boveda;
use guardiana_vault::KdfParams;

const FAST: KdfParams = KdfParams {
    memoria_kib: 8 * 1024,
    pasadas: 1,
    hilos: 1,
};

fn agent() -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .http_status_as_error(false)
            .build(),
    )
}

#[test]
fn page_round_trip() {
    let dir: PathBuf = std::env::temp_dir().join(format!("gdn-page-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let s = rt.block_on(boveda::bind_with(dir.clone(), FAST)).unwrap();
    let base = format!("http://{}", s.addr);
    let tok = s.token.clone();
    let server = rt.spawn(s.run());
    let a = agent();
    let get = |path: &str| {
        a.get(format!("{base}{path}"))
            .header("x-guardiana-token", &tok)
            .call()
            .unwrap()
    };
    let post_json = |path: &str, v: serde_json::Value| {
        a.post(format!("{base}{path}"))
            .header("x-guardiana-token", &tok)
            .send_json(v)
            .unwrap()
    };

    // no token: refused; wrong host: refused
    assert_eq!(
        a.get(format!("{base}/api/estado")).call().unwrap().status(),
        401
    );
    assert_eq!(
        a.get(format!("{base}/api/estado"))
            .header("host", "evil.example")
            .header("x-guardiana-token", &tok)
            .call()
            .unwrap()
            .status(),
        421
    );
    let mut r = get("/api/estado");
    let e: serde_json::Value = r.body_mut().read_json().unwrap();
    assert_eq!(e["existe"], false);

    // too short a password: refused with the sentence
    assert_eq!(
        post_json("/api/crear", serde_json::json!({"contrasena": "corta"})).status(),
        400
    );
    let mut r = post_json(
        "/api/crear",
        serde_json::json!({"contrasena": "una clave maestra larga"}),
    );
    assert_eq!(r.status(), 200);
    let w: serde_json::Value = r.body_mut().read_json().unwrap();
    let words: Vec<String> = w["palabras"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(words.len(), 24);

    // store, list, download
    let data: Vec<u8> = (0..70_000u32).map(|i| (i % 251) as u8).collect();
    let mut r = a
        .post(format!("{base}/api/objetos?nombre=informe%20final.pdf"))
        .header("x-guardiana-token", &tok)
        .header("content-type", "application/octet-stream")
        .send(&data[..])
        .unwrap();
    assert_eq!(
        r.status(),
        200,
        "{}",
        r.body_mut().read_to_string().unwrap()
    );
    let o: serde_json::Value = r.body_mut().read_json().unwrap();
    let id = o["id"].as_str().unwrap().to_owned();
    assert_eq!(o["nombre"], "informe final.pdf");
    let mut r = get("/api/objetos");
    let l: serde_json::Value = r.body_mut().read_json().unwrap();
    assert_eq!(l.as_array().unwrap().len(), 1);
    let mut r = get(&format!("/api/objetos/{id}"));
    assert_eq!(r.status(), 200);
    assert!(r
        .headers()
        .get("content-disposition")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("informe%20final.pdf"));
    assert_eq!(r.body_mut().read_to_vec().unwrap(), data);

    // lock, wrong password, open with the words, chain intact
    assert_eq!(
        a.post(format!("{base}/api/cerrar"))
            .header("x-guardiana-token", &tok)
            .send_empty()
            .unwrap()
            .status(),
        204
    );
    assert_eq!(get("/api/objetos").status(), 401);
    assert_eq!(
        post_json(
            "/api/abrir",
            serde_json::json!({"contrasena": "otra clave distinta"})
        )
        .status(),
        401
    );
    assert_eq!(
        post_json(
            "/api/recuperar",
            serde_json::json!({"palabras": words.join(" "), "nueva": "clave nueva y larga"})
        )
        .status(),
        204
    );
    assert_eq!(
        a.post(format!("{base}/api/cerrar"))
            .header("x-guardiana-token", &tok)
            .send_empty()
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        post_json(
            "/api/abrir",
            serde_json::json!({"contrasena": "clave nueva y larga"})
        )
        .status(),
        200
    );
    let mut r = get("/api/registro");
    let reg: serde_json::Value = r.body_mut().read_json().unwrap();
    assert_eq!(reg["cadena_ok"], true);
    let acts: Vec<&str> = reg["lineas"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["accion"].as_str().unwrap())
        .collect();
    assert_eq!(
        acts,
        [
            "crear",
            "guardar",
            "leer",
            "fallo",
            "recuperar",
            "contrasena",
            "abrir"
        ]
    );

    // the page can load without a token; the API cannot
    assert_eq!(a.get(format!("{base}/")).call().unwrap().status(), 200);
    // exit stops the server
    assert_eq!(
        a.post(format!("{base}/api/salir"))
            .header("x-guardiana-token", &tok)
            .send_empty()
            .unwrap()
            .status(),
        204
    );
    rt.block_on(server).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
