//! The program's language, kept by the service (1.0.46, R10 of the owner's list).
//!
//! On the owner's PC the language switch did nothing: the panel runs inside GUARDIANA ZERO's
//! WebView2, the choice lived in that page's `localStorage`, and ZERO's own screens never saw it.
//! Now the choice is one setting of the service, behind the session token like every other
//! setting: `GET /api/ajustes/idioma` says it (English until the person picks another) and
//! `PUT /api/ajustes/idioma {"idioma":"es"}` changes it for the whole program. The panel reads it
//! before it draws its first texts, and ZERO reads the same answer for its own interface.

// Handlers answer with the same error type as the rest of the API (see api.rs).
#![allow(clippy::result_large_err)]

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::api::{with_ledger, ApiResult};
use crate::auth::Session;
use crate::AppState;

/// The setting's name in the ledger.
const CLAVE: &str = "idioma_programa";
/// The languages the program speaks; the first is the one used until the person chooses.
pub(crate) const IDIOMAS: [&str; 3] = ["en", "es", "pt"];

pub(crate) fn rutas() -> Router<Arc<AppState>> {
    Router::new().route("/api/ajustes/idioma", get(ver).put(cambiar))
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub(crate) struct Idioma {
    /// `en`, `es` or `pt`.
    idioma: String,
    /// Whether the person chose it (false: the default, English).
    #[serde(default)]
    elegido: bool,
}

/// What is stored, read back as one of the three languages (anything else is the default).
pub(crate) fn de_ajuste(guardado: Option<&str>) -> Idioma {
    match guardado.map(str::trim) {
        Some(x) if IDIOMAS.contains(&x) => Idioma {
            idioma: x.to_owned(),
            elegido: true,
        },
        _ => Idioma {
            idioma: IDIOMAS[0].to_owned(),
            elegido: false,
        },
    }
}

async fn ver(State(state): State<Arc<AppState>>, _s: Session) -> ApiResult<Idioma> {
    let guardado = with_ledger(&state, |l| l.setting(CLAVE))?;
    Ok(Json(de_ajuste(guardado.as_deref())))
}

#[derive(Deserialize)]
struct Cambio {
    idioma: String,
}

async fn cambiar(
    State(state): State<Arc<AppState>>,
    _s: Session,
    Json(body): Json<Cambio>,
) -> ApiResult<Idioma> {
    let pedido = body.idioma.trim().to_ascii_lowercase();
    if !IDIOMAS.contains(&pedido.as_str()) {
        return Err((StatusCode::BAD_REQUEST, "unknown language").into_response());
    }
    with_ledger(&state, |l| l.set_setting(CLAVE, &pedido))?;
    Ok(Json(de_ajuste(Some(&pedido))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_until_the_person_chooses_and_only_the_three_languages() {
        assert_eq!(
            de_ajuste(None),
            Idioma {
                idioma: "en".into(),
                elegido: false
            }
        );
        assert_eq!(
            de_ajuste(Some("pt")),
            Idioma {
                idioma: "pt".into(),
                elegido: true
            }
        );
        assert_eq!(de_ajuste(Some("fr")).idioma, "en");
        assert!(!de_ajuste(Some("")).elegido);
    }
}
