//! `guardiana-rele [dirección]`: the blind relay, listening on `0.0.0.0:443` unless told
//! otherwise. It prints one line at start and nothing else: no addresses, ever. With a
//! certificate in `/etc/guardiana-rele/tls` (or `GUARDIANA_RELE_TLS`), phones can reach it too.

use std::path::PathBuf;

use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let direccion = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:443".to_owned());
    let carpeta = std::env::var_os("GUARDIANA_RELE_TLS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(guardiana_rele::tls::CARPETA));
    let tls = match guardiana_rele::tls::desde(&carpeta) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("certificado no válido ({e}); sigo solo para equipos");
            None
        }
    };
    match TcpListener::bind(&direccion).await {
        Ok(l) => {
            println!(
                "guardiana-rele {} escuchando en {direccion}{}",
                env!("CARGO_PKG_VERSION"),
                if tls.is_some() {
                    " (equipos y móviles)"
                } else {
                    " (solo equipos)"
                }
            );
            guardiana_rele::sirve_con(l, tls).await;
        }
        Err(e) => {
            eprintln!("no se pudo escuchar en {direccion}: {e}");
            std::process::exit(1);
        }
    }
}
