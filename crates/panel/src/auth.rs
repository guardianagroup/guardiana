//! Token and `Host` checks (brief §8).

use std::path::Path;
use std::sync::Arc;

use axum::extract::{FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::AppState;

/// File name of the token inside the data directory.
pub const TOKEN_FILE: &str = "panel.token";

/// Read the token, or create a fresh random one readable only by the user.
pub fn load_or_create_token(path: &Path) -> std::io::Result<String> {
    if let Ok(existing) = std::fs::read_to_string(path) {
        let t = existing.trim();
        if t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit()) {
            return Ok(t.to_owned());
        }
    }
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| std::io::Error::other(e.to_string()))?;
    let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, &token)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(token)
}

/// Middleware: refuse any request whose `Host` is not one of ours (DNS rebinding).
pub(crate) async fn check_host(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(strip_port)
        .unwrap_or_default();
    if !state.allowed_hosts.iter().any(|h| h == host) {
        return (StatusCode::MISDIRECTED_REQUEST, "host not recognised").into_response();
    }
    next.run(req).await
}

pub(crate) fn strip_port(host: &str) -> &str {
    if let Some(rest) = host.strip_prefix('[') {
        return rest.split(']').next().unwrap_or("");
    }
    host.rsplit_once(':').map_or(host, |(h, _)| h)
}

/// Extractor: the request carries the session token (header or `?t=`).
pub(crate) struct Session;

impl FromRequestParts<Arc<AppState>> for Session {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let from_header = parts
            .headers
            .get("x-guardiana-token")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let from_query = parts.uri.query().and_then(|q| {
            q.split('&')
                .find_map(|kv| kv.strip_prefix("t=").map(str::to_owned))
        });
        let given = from_header.or(from_query).unwrap_or_default();
        if !given.is_empty() && constant_time_eq(given.as_bytes(), state.token.as_bytes()) {
            Ok(Self)
        } else {
            Err((StatusCode::UNAUTHORIZED, "session token required").into_response())
        }
    }
}

/// Extractor: the language of the request. The pages send `X-Guardiana-Lang` (the person's
/// choice, kept in the browser); a first visit falls back to `Accept-Language`; Spanish otherwise.
pub(crate) struct Lang(pub(crate) &'static guardiana_core::i18n::Texts);

impl<S: Send + Sync> FromRequestParts<S> for Lang {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let code = parts
            .headers
            .get("x-guardiana-lang")
            .or_else(|| parts.headers.get(axum::http::header::ACCEPT_LANGUAGE))
            .and_then(|v| v.to_str().ok())
            .unwrap_or("es");
        Ok(Self(guardiana_core::i18n::by_code(code)))
    }
}

pub(crate) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_port_is_stripped() {
        assert_eq!(strip_port("127.0.0.1:7443"), "127.0.0.1");
        assert_eq!(strip_port("localhost"), "localhost");
        assert_eq!(strip_port("[::1]:7443"), "::1");
    }

    #[test]
    fn token_file_round_trip() {
        let dir = std::env::temp_dir().join(format!("guardiana-panel-{}", std::process::id()));
        let path = dir.join(TOKEN_FILE);
        let _ = std::fs::remove_dir_all(&dir);
        let a = load_or_create_token(&path).unwrap_or_default();
        let b = load_or_create_token(&path).unwrap_or_default();
        assert_eq!(a.len(), 64);
        assert_eq!(a, b);
        assert!(constant_time_eq(a.as_bytes(), b.as_bytes()));
        assert!(!constant_time_eq(b"a", b"b"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
