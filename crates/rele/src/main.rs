//! `guardiana-rele [dirección]`: the blind relay, listening on `0.0.0.0:443` unless told
//! otherwise. It prints one line at start and nothing else: no addresses, ever.

use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let direccion = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:443".to_owned());
    match TcpListener::bind(&direccion).await {
        Ok(l) => {
            println!(
                "guardiana-rele {} escuchando en {direccion}",
                env!("CARGO_PKG_VERSION")
            );
            guardiana_rele::sirve(l).await;
        }
        Err(e) => {
            eprintln!("no se pudo escuchar en {direccion}: {e}");
            std::process::exit(1);
        }
    }
}
