//! The panel's listeners (second review of 1.0.55): a required address that cannot be bound fails the start with
//! nothing left listening; an optional one (in Home Mode, the LAN's panel and its checker page) is reported and
//! skipped; and a shutdown right after a start ends, and frees the ports. A start that failed on its address says
//! which one to try again, and it is tried only once it can be bound (review of vpn-1055b).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::net::{SocketAddr, TcpListener as Ocupa, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

use guardiana_core::{Hash, Ledger};
use guardiana_panel::{se_puede_abrir, start, Config, Error, RuntimeInfo};

fn genesis() -> Hash {
    Hash::of(b"panel listeners test key")
}

fn prepara(tag: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("guardiana-escucha-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("ledger.db");
    drop(Ledger::open(&db, genesis()).unwrap());
    (db, dir.join("panel.token"))
}

/// A loopback address nobody holds right now.
fn libre() -> SocketAddr {
    Ocupa::bind("127.0.0.1:0").unwrap().local_addr().unwrap()
}

fn config(
    (db, token): &(PathBuf, PathBuf),
    listen: Vec<SocketAddr>,
    optional_listen: Vec<SocketAddr>,
) -> Config {
    Config {
        listen,
        optional_listen,
        db_path: db.clone(),
        genesis: genesis(),
        token_path: token.clone(),
        extra_hosts: Vec::new(),
        info: RuntimeInfo::default(),
    }
}

/// Whether something answers at `a`.
fn contesta(a: SocketAddr) -> bool {
    TcpStream::connect_timeout(&a, Duration::from_secs(2)).is_ok()
}

/// What the second review saw: the second required address could not be bound, the start failed, and the loopback
/// panel it had already started kept serving, with nobody to stop it, so every later pass found the panel's port
/// taken. Now nothing is left listening, and the next start works.
#[tokio::test]
async fn un_arranque_que_falla_no_deja_nada_escuchando() {
    let rutas = prepara("falla");
    let ocupada = Ocupa::bind("127.0.0.1:0").unwrap();
    let tomada = ocupada.local_addr().unwrap();
    let propia = libre();
    let fallo = start(config(&rutas, vec![propia, tomada], Vec::new()))
        .await
        .err();
    assert!(
        matches!(fallo, Some(Error::Bind(a, _)) if a == tomada),
        "{fallo:?}"
    );
    // the address it had bound: nobody answers there, and it can be bound again
    assert!(!contesta(propia));
    drop(Ocupa::bind(propia).expect("the panel's own port is free again"));
    // and once the other one is free too, a new start takes both
    drop(ocupada);
    let p = start(config(&rutas, vec![propia, tomada], Vec::new()))
        .await
        .expect("it starts once both are free");
    assert_eq!(p.addrs, vec![propia, tomada]);
    assert!(contesta(propia) && contesta(tomada));
    tokio::time::timeout(Duration::from_secs(30), p.shutdown())
        .await
        .expect("the shutdown ends");
}

/// An optional address that cannot be bound is said and skipped: the panel starts on the rest, which serves. A
/// shutdown right after it ends, and frees the port.
#[tokio::test]
async fn una_direccion_opcional_tomada_se_salta() {
    let rutas = prepara("opcional");
    let ocupada = Ocupa::bind("127.0.0.1:0").unwrap();
    let tomada = ocupada.local_addr().unwrap();
    let propia = libre();
    let p = start(config(&rutas, vec![propia], vec![tomada]))
        .await
        .expect("it starts without the optional address");
    assert_eq!(p.addrs, vec![propia]);
    assert_eq!(p.failed_optional.len(), 1);
    assert_eq!(p.failed_optional[0].0, tomada);
    assert!(contesta(propia));
    tokio::time::timeout(Duration::from_secs(30), p.shutdown())
        .await
        .expect("the shutdown ends");
    drop(Ocupa::bind(propia).expect("the panel's port is free after the shutdown"));
    drop(ocupada);
}

/// A shutdown asked at once, before any of the panel's tasks has run, still ends (it waited for ever until 1.0.55).
#[tokio::test]
async fn un_cierre_justo_tras_el_arranque_termina() {
    let rutas = prepara("cierre");
    let propia = libre();
    let p = start(config(&rutas, vec![propia], Vec::new()))
        .await
        .expect("it starts");
    tokio::time::timeout(Duration::from_secs(30), p.shutdown())
        .await
        .expect("the shutdown ends");
    drop(Ocupa::bind(propia).expect("the panel's port is free after the shutdown"));
}

/// With its loopback address taken there is no panel at all, and with the VPN connected no way to disconnect it
/// (review of vpn-1055b). The failed start says which address to try again; while that stays taken it cannot be
/// opened, time after time, so the engine's 15-second check rebuilds nothing; once it is free it can (and asking
/// leaves nothing listening), and the start that follows takes it. A start that failed for another reason (here its
/// token file cannot be written) is not one a free port fixes: there is nothing to try again.
#[tokio::test]
async fn sin_panel_su_direccion_se_reintenta_solo_cuando_se_puede_abrir() {
    let rutas = prepara("reintento");
    let ocupada = Ocupa::bind("127.0.0.1:0").unwrap();
    let tomada = ocupada.local_addr().unwrap();
    let fallo = start(config(&rutas, vec![tomada], Vec::new()))
        .await
        .err()
        .expect("no panel while its address is taken");
    assert_eq!(fallo.por_reintentar(), Some(tomada), "{fallo:?}");
    for _ in 0..3 {
        assert!(!se_puede_abrir(tomada), "still taken: nothing to rebuild");
    }
    drop(ocupada);
    assert!(se_puede_abrir(tomada), "free again");
    assert!(!contesta(tomada), "asking leaves nothing listening");
    let p = start(config(&rutas, vec![tomada], Vec::new()))
        .await
        .expect("it starts once its address is free");
    assert_eq!(p.addrs, vec![tomada]);
    assert!(contesta(tomada));
    tokio::time::timeout(Duration::from_secs(30), p.shutdown())
        .await
        .expect("the shutdown ends");
    // the token file inside the ledger's file, which no folder can be
    let (db, _) = &rutas;
    let sin_llave = Config {
        token_path: db.join("panel.token"),
        ..config(&rutas, vec![libre()], Vec::new())
    };
    let fallo = start(sin_llave)
        .await
        .err()
        .expect("no token file, no panel");
    assert!(matches!(fallo, Error::Token(_)), "{fallo:?}");
    assert_eq!(fallo.por_reintentar(), None);
}
