//! The relay's own certificate, for the phones (16 Oct 2026).
//!
//! A phone's browser only opens `wss://` to a name whose certificate a public authority signed.
//! The server keeps one from Let's Encrypt for `rele.guardianagroup.com` (certbot, renewed by
//! itself; see `servidor-tls.sh`), copied where the relay can read it. Computers do not need it:
//! they speak the plain relay line, and their conversation is encrypted end to end anyway.

use std::path::Path;
use std::sync::Arc;

use rustls::pki_types::pem::PemObject as _;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};

/// Where the server keeps the certificate (`fullchain.pem`, `privkey.pem`).
pub const CARPETA: &str = "/etc/guardiana-rele/tls";

/// The TLS setup from `carpeta`, `None` when there is no certificate there yet (the relay then
/// serves computers only, as before).
pub fn desde(carpeta: &Path) -> Result<Option<Arc<rustls::ServerConfig>>, String> {
    let (cadena, clave) = (carpeta.join("fullchain.pem"), carpeta.join("privkey.pem"));
    if !cadena.exists() || !clave.exists() {
        return Ok(None);
    }
    let certs = CertificateDer::pem_file_iter(&cadena)
        .map_err(|e| format!("{}: {e}", cadena.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{}: {e}", cadena.display()))?;
    let clave =
        PrivateKeyDer::from_pem_file(&clave).map_err(|e| format!("{}: {e}", clave.display()))?;
    config(certs, clave).map(Some)
}

/// TLS 1.2 and 1.3 with ring, HTTP/1.1 only (what a WebSocket needs).
pub fn config(
    certs: Vec<CertificateDer<'static>>,
    clave: PrivateKeyDer<'static>,
) -> Result<Arc<rustls::ServerConfig>, String> {
    let mut c = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|e| e.to_string())?
    .with_no_client_auth()
    .with_single_cert(certs, clave)
    .map_err(|e| e.to_string())?;
    c.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(Arc::new(c))
}
