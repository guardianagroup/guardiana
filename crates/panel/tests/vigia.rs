//! «Vigía» (1.0.48) por el panel HTTP de verdad: quién puede leer `/vigia` y con qué cabeceras.
//!
//! En Windows y con `GUARDIANA_VIGIA_REAL=1` (lo pone la CI), además, una señal de verdad: un
//! programa que corre y que Windows tiene anotado usando el micrófono y capturando la pantalla
//! (la entrada del registro se escribe aquí: el ordenador de la CI no tiene micrófono), y un
//! programa de control remoto en marcha. El vigía los tiene que nombrar, y no nombrar al propio
//! programa que pregunta.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::net::SocketAddr;
use std::path::PathBuf;

use guardiana_core::{Hash, Ledger};
use guardiana_panel::{start, Config, Running, RuntimeInfo};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const WEB: &str = "https://guardianagroup.com";

/// (código, cabecera de la respuesta en minúsculas, cuerpo).
async fn pide(
    addr: SocketAddr,
    metodo: &str,
    ruta: &str,
    cabeceras: &[(&str, &str)],
) -> (u16, String, String) {
    let mut s = TcpStream::connect(addr).await.unwrap();
    let mut req = format!("{metodo} {ruta} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n");
    for (k, v) in cabeceras {
        req.push_str(k);
        req.push_str(": ");
        req.push_str(v);
        req.push_str("\r\n");
    }
    req.push_str("\r\n");
    s.write_all(req.as_bytes()).await.unwrap();
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).await.unwrap();
    let texto = String::from_utf8_lossy(&raw).into_owned();
    let (cabeza, cuerpo) = texto.split_once("\r\n\r\n").expect("una respuesta entera");
    let codigo: u16 = cabeza
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .expect("un código");
    (codigo, cabeza.to_lowercase(), cuerpo.to_owned())
}

async fn panel(nombre: &str) -> (Running, PathBuf) {
    let dir: PathBuf =
        std::env::temp_dir().join(format!("guardiana-vigia-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("ledger.db");
    let genesis = Hash::of(b"panel vigia test key");
    drop(Ledger::open(&db, genesis).unwrap());
    let running = start(Config {
        listen: vec!["127.0.0.1:0".parse().unwrap()],
        optional_listen: Vec::new(),
        db_path: db,
        genesis,
        token_path: dir.join("panel.token"),
        extra_hosts: Vec::new(),
        info: RuntimeInfo::default(),
    })
    .await
    .expect("el panel arranca");
    (running, dir)
}

#[tokio::test]
async fn el_vigia_solo_se_deja_leer_por_el_panel_y_por_la_web_de_comunicaciones() {
    let (running, dir) = panel("acceso").await;
    let addr = running.addrs[0];
    let token = running.token.clone();

    // Sin llave ni Origin (una etiqueta <img> o <script> de otra web, un programa): 401.
    let (c, cab, cuerpo) = pide(addr, "GET", "/vigia", &[]).await;
    assert_eq!(c, 401, "{cuerpo}");
    assert!(!cab.contains("access-control-allow-origin"), "{cab}");

    // Otra web, o la nuestra mal escrita: 403, sin CORS y sin nada que leer.
    for otra in [
        "https://otra.example",
        "http://guardianagroup.com",
        "https://www.guardianagroup.com",
        "https://guardianagroup.com.otra.example",
        "null",
    ] {
        let (c, cab, cuerpo) = pide(addr, "GET", "/vigia", &[("Origin", otra)]).await;
        assert_eq!(c, 403, "{otra}");
        assert!(
            !cab.contains("access-control-allow-origin"),
            "{otra}: {cab}"
        );
        assert!(!cuerpo.contains("alertas"), "{otra}: {cuerpo}");
    }

    // La web de Comunicaciones, desde este ordenador: 200 y CORS a su origen exacto, sin
    // credenciales, sin rutas y sin lo que falló.
    let (c, cab, cuerpo) = pide(addr, "GET", "/vigia?idioma=en", &[("Origin", WEB)]).await;
    assert_eq!(c, 200, "{cuerpo}");
    assert!(
        cab.contains("\r\naccess-control-allow-origin: https://guardianagroup.com\r\n")
            || cab.ends_with("\r\naccess-control-allow-origin: https://guardianagroup.com"),
        "{cab}"
    );
    assert!(!cab.contains("access-control-allow-credentials"), "{cab}");
    assert!(cab.contains("vary: origin"), "{cab}");
    assert!(cab.contains("cache-control: no-store"), "{cab}");
    assert!(cab.contains("content-type: application/json"), "{cab}");
    let v: serde_json::Value = serde_json::from_str(&cuerpo).unwrap();
    assert_eq!(v["vigia"], 1);
    assert_eq!(v["idioma"], "en");
    assert!(v["alertas"].is_array() && v["avisos"].is_array(), "{v}");
    assert!(v.get("errores").is_none(), "{v}");
    assert!(!cuerpo.contains("\"ruta\""), "{cuerpo}");
    assert!(v["limites"].as_str().is_some_and(|l| !l.is_empty()));
    if !cfg!(windows) {
        assert_eq!(v["estado"], "no_disponible");
        assert_eq!(v["hay_alertas"], false);
    }

    // La consulta previa de Chrome antes de dejar a una web pública hablar con este ordenador.
    let (c, cab, _) = pide(
        addr,
        "OPTIONS",
        "/vigia",
        &[
            ("Origin", WEB),
            ("Access-Control-Request-Method", "GET"),
            ("Access-Control-Request-Private-Network", "true"),
        ],
    )
    .await;
    assert_eq!(c, 204, "{cab}");
    assert!(
        cab.contains("access-control-allow-origin: https://guardianagroup.com"),
        "{cab}"
    );
    assert!(cab.contains("access-control-allow-methods: get"), "{cab}");
    assert!(
        cab.contains("access-control-allow-private-network: true"),
        "{cab}"
    );
    assert!(!cab.contains("access-control-allow-headers"), "{cab}");
    assert!(!cab.contains("access-control-allow-credentials"), "{cab}");
    let (c, cab, _) = pide(
        addr,
        "OPTIONS",
        "/vigia",
        &[
            ("Origin", "https://otra.example"),
            ("Access-Control-Request-Method", "GET"),
        ],
    )
    .await;
    assert_eq!(c, 403, "{cab}");
    assert!(!cab.contains("access-control-allow-origin"), "{cab}");

    // Con la llave, el panel: 200, sin CORS, y con lo que falló al leer Defender.
    let (c, cab, cuerpo) = pide(
        addr,
        "GET",
        "/vigia",
        &[
            ("X-Guardiana-Token", token.as_str()),
            ("X-Guardiana-Lang", "pt"),
        ],
    )
    .await;
    assert_eq!(c, 200, "{cuerpo}");
    assert!(!cab.contains("access-control-allow-origin"), "{cab}");
    let v: serde_json::Value = serde_json::from_str(&cuerpo).unwrap();
    assert_eq!(v["idioma"], "pt");
    assert!(v["errores"].is_object(), "{v}");
    // Una llave equivocada es como ninguna.
    let (c, _, _) = pide(addr, "GET", "/vigia", &[("X-Guardiana-Token", "0000")]).await;
    assert_eq!(c, 401);

    running.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(windows)]
mod real {
    //! Lo que el ordenador de la CI no tiene (un micrófono, alguien espiando), puesto a mano: dos
    //! copias de `PING.EXE` que corren con nombres elegidos y las entradas del registro que
    //! Windows escribe cuando un programa usa el micrófono o captura la pantalla. Todo se quita
    //! al terminar.

    use std::path::PathBuf;
    use std::process::{Child, Command, Stdio};

    const BASE: &str =
        r"HKCU\Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore";
    pub(super) const ESPIA: &str = "espia-guardiana-prueba.exe";
    pub(super) const REMOTO: &str = "AnyDesk.exe";

    pub(super) struct Simulacion {
        hijos: Vec<Child>,
        claves: Vec<String>,
        dir: PathBuf,
    }

    fn reg(args: &[&str]) -> bool {
        Command::new("reg.exe")
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }

    fn filetime_ahora() -> u64 {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        (u64::try_from(ms).unwrap() + 11_644_473_600_000) * 10_000
    }

    impl Simulacion {
        pub(super) fn monta() -> Self {
            let dir =
                std::env::temp_dir().join(format!("guardiana-vigia-real-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let raiz = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_owned());
            let ping = PathBuf::from(raiz).join("System32").join("PING.EXE");
            let mut yo = Self {
                hijos: Vec::new(),
                claves: Vec::new(),
                dir: dir.clone(),
            };
            for nombre in [ESPIA, REMOTO] {
                let exe = dir.join(nombre);
                std::fs::copy(&ping, &exe).unwrap();
                yo.hijos.push(
                    Command::new(&exe)
                        .args(["-n", "400", "127.0.0.1"])
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn()
                        .unwrap(),
                );
            }
            let ahora = format!("0x{:x}", filetime_ahora());
            let espia = dir.join(ESPIA).to_string_lossy().replace('\\', "#");
            // El propio programa de la prueba, que es quien pregunta (y, aquí, también GUARDIANA):
            // aunque Windows lo tenga anotado con el micrófono, no puede salir.
            let propio = std::env::current_exe()
                .unwrap()
                .to_string_lossy()
                .replace('\\', "#");
            for (cap, quien) in [
                ("microphone", espia.as_str()),
                ("graphicsCaptureProgrammatic", espia.as_str()),
                ("microphone", propio.as_str()),
            ] {
                let clave = format!(r"{BASE}\{cap}\NonPackaged\{quien}");
                assert!(reg(&[
                    "add",
                    clave.as_str(),
                    "/v",
                    "LastUsedTimeStart",
                    "/t",
                    "REG_QWORD",
                    "/d",
                    ahora.as_str(),
                    "/f"
                ]));
                assert!(reg(&[
                    "add",
                    clave.as_str(),
                    "/v",
                    "LastUsedTimeStop",
                    "/t",
                    "REG_QWORD",
                    "/d",
                    "0",
                    "/f"
                ]));
                yo.claves.push(clave);
            }
            yo
        }
    }

    impl Drop for Simulacion {
        fn drop(&mut self) {
            for h in &mut self.hijos {
                let _ = h.kill();
                let _ = h.wait();
            }
            for c in &self.claves {
                let _ = reg(&["delete", c.as_str(), "/f"]);
            }
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
}

#[cfg(windows)]
#[tokio::test]
async fn en_windows_una_senal_de_verdad_llega_a_la_web_sin_el_que_pregunta() {
    if std::env::var("GUARDIANA_VIGIA_REAL").as_deref() != Ok("1") {
        eprintln!("GUARDIANA_VIGIA_REAL no es 1: la prueba que escribe en el registro no corre");
        return;
    }
    let sim = real::Simulacion::monta();
    let (running, dir) = panel("real").await;
    let addr = running.addrs[0];
    let token = running.token.clone();
    let hay = |v: &serde_json::Value, tipo: &str, archivo: &str| {
        v["alertas"].as_array().is_some_and(|a| {
            a.iter().any(|x| {
                x["tipo"] == tipo
                    && x["archivo"]
                        .as_str()
                        .is_some_and(|f| f.eq_ignore_ascii_case(archivo))
            })
        })
    };
    let listo = |v: &serde_json::Value| {
        hay(v, "microfono", real::ESPIA)
            && hay(v, "pantalla", real::ESPIA)
            && hay(v, "control_remoto", real::REMOTO)
            && v["defender_mirado"].is_number()
    };
    let fin = std::time::Instant::now() + std::time::Duration::from_secs(90);
    let mut v = serde_json::Value::Null;
    while std::time::Instant::now() < fin {
        let (c, _, cuerpo) = pide(
            addr,
            "GET",
            "/vigia",
            &[
                ("X-Guardiana-Token", token.as_str()),
                ("X-Guardiana-Lang", "es"),
            ],
        )
        .await;
        assert_eq!(c, 200, "{cuerpo}");
        v = serde_json::from_str(&cuerpo).unwrap();
        if listo(&v) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    // Lo que el vigía dice de este ordenador de la CI (Defender, escritorio remoto), para leerlo
    // en el registro de la ejecución.
    eprintln!("vigía en la CI: {v:#}");
    assert!(hay(&v, "microfono", real::ESPIA), "{v:#}");
    assert!(hay(&v, "pantalla", real::ESPIA), "{v:#}");
    assert!(hay(&v, "control_remoto", real::REMOTO), "{v:#}");
    assert_eq!(v["hay_alertas"], true);
    assert_eq!(v["estado"], "alerta");
    let propio = std::env::current_exe()
        .unwrap()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let nombrado = v["alertas"].as_array().unwrap().iter().any(|x| {
        x["archivo"]
            .as_str()
            .is_some_and(|f| f.eq_ignore_ascii_case(&propio))
    });
    assert!(!nombrado, "quien pregunta no es una señal: {v:#}");
    // Con la llave va la ruta; la web no la ve.
    let espia = v["alertas"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["tipo"] == "microfono")
        .unwrap();
    assert!(
        espia["ruta"]
            .as_str()
            .is_some_and(|r| r.contains(real::ESPIA)),
        "{espia}"
    );
    assert_eq!(
        espia["texto"],
        "espia-guardiana-prueba está usando el micrófono."
    );
    let (c, cab, cuerpo) = pide(addr, "GET", "/vigia", &[("Origin", WEB)]).await;
    assert_eq!(c, 200, "{cuerpo}");
    assert!(cab.contains("access-control-allow-origin: https://guardianagroup.com"));
    let w: serde_json::Value = serde_json::from_str(&cuerpo).unwrap();
    assert!(hay(&w, "microfono", real::ESPIA), "{w:#}");
    assert!(!cuerpo.contains("\"ruta\""), "{cuerpo}");

    running.shutdown().await;
    drop(sim);
    let _ = std::fs::remove_dir_all(&dir);
}
