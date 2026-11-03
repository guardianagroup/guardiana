//! `guardiana-tunel-servidor`: the anonymous sign-up of GUARDIANA's VPN on one server (docs/VPN.md). It listens on the
//! machine's own address only, behind the web server, and runs with the one privilege it needs (to change WireGuard's
//! peers).
//!
//! Usage: `guardiana-tunel-servidor --punto motor.guardianagroup.com:51820 [--interfaz wg0] [--escucha 127.0.0.1:8793]
//! [--dns 10.66.0.1]`
//!
//! `POST /vpn/alta` is the sign-up; `GET /vpn/alta`, the echo: the address a website sees from whoever asks (from inside
//! the tunnel, this server's own public one), with nothing kept (3 Nov 2026; docs/VPN.md, «Las salidas»).

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{DefaultBodyLimit, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use guardiana_tunel::alta::Pide;
use guardiana_tunel_servidor::{ErrorAlta, Estado, WgReal};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let valor = |nombre: &str| {
        args.windows(2)
            .find(|w| w[0] == nombre)
            .map(|w| w[1].clone())
    };
    let Some(punto) = valor("--punto") else {
        eprintln!("guardiana-tunel-servidor: falta --punto host:puerto");
        std::process::exit(2);
    };
    let interfaz = valor("--interfaz").unwrap_or_else(|| "wg0".to_string());
    let escucha: SocketAddr = valor("--escucha")
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], 8793)));
    let dns: Ipv4Addr = valor("--dns")
        .and_then(|v| v.parse().ok())
        .unwrap_or(Ipv4Addr::new(10, 66, 0, 1));
    let wg = WgReal { interfaz };
    let servidor = match wg.publica() {
        Ok(k) => k,
        Err(e) => {
            eprintln!("guardiana-tunel-servidor: no se pudo leer la llave de WireGuard: {e}");
            std::process::exit(1);
        }
    };
    let estado = Arc::new(Estado::nuevo(servidor, &punto, dns, Box::new(wg)));
    // the computers already connected (this service started again: an update, a crash) keep their tunnel and their address
    if let Err(e) = estado.adopta(ahora_s()) {
        eprintln!("guardiana-tunel-servidor: no se pudieron leer los pares de WireGuard: {e}");
        std::process::exit(1);
    }
    let limpieza = Arc::clone(&estado);
    tokio::spawn(async move {
        let mut cada = tokio::time::interval(Duration::from_secs(600));
        loop {
            cada.tick().await;
            let e = Arc::clone(&limpieza);
            let _ = tokio::task::spawn_blocking(move || e.limpia(ahora_s())).await;
        }
    });
    // This server's public address, the one its traffic out leaves with: its endpoint's name resolved here, once (the
    // echo of `GET /vpn/alta` says it to whoever asks from inside the tunnel).
    let publica = std::net::ToSocketAddrs::to_socket_addrs(&punto)
        .ok()
        .and_then(|mut v| v.find(SocketAddr::is_ipv4))
        .map(|a| a.ip());
    let app = Router::new()
        .route(
            "/vpn/alta",
            post(alta).get(move |cabeceras: HeaderMap| async move {
                let vista = guardiana_tunel_servidor::ip_vista(origen(&cabeceras), publica);
                (
                    [(header::CONTENT_TYPE, "application/json")],
                    guardiana_tunel_servidor::eco(vista),
                )
            }),
        )
        .layer(DefaultBodyLimit::max(8 * 1024))
        .with_state(estado);
    let escuchador = match tokio::net::TcpListener::bind(escucha).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("guardiana-tunel-servidor: no se pudo escuchar en {escucha}: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = axum::serve(escuchador, app).await {
        eprintln!("guardiana-tunel-servidor: {e}");
        std::process::exit(1);
    }
}

/// `POST /vpn/alta`.
async fn alta(State(e): State<Arc<Estado>>, cabeceras: HeaderMap, Json(p): Json<Pide>) -> Response {
    let origen = origen(&cabeceras);
    match tokio::task::spawn_blocking(move || e.alta(&p, origen, ahora_s())).await {
        Ok(Ok(r)) => Json(r).into_response(),
        Ok(Err(ErrorAlta::Formato(_))) => StatusCode::BAD_REQUEST.into_response(),
        Ok(Err(ErrorAlta::Ocupado)) => StatusCode::TOO_MANY_REQUESTS.into_response(),
        Ok(Err(ErrorAlta::Wg(_))) | Err(_) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

/// Who asked, as the web server in front saw it: the last address in `X-Forwarded-For`. Caddy writes that header itself
/// and does not pass on one a client sent; this service only listens on the machine's own address, behind it.
fn origen(cabeceras: &HeaderMap) -> Option<IpAddr> {
    cabeceras
        .get("x-forwarded-for")?
        .to_str()
        .ok()?
        .rsplit(',')
        .next()?
        .trim()
        .parse()
        .ok()
}

fn ahora_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
