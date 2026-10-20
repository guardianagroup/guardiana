//! Where this computer is: its city, kept by the service (19 Oct 2026).
//!
//! The owner: «necesito la máxima precisión para detectar la ciudad donde está la persona; tiene
//! que ser la ciudad». Agreed by Claude, ChatGPT and Grok: the city comes from the system's own
//! location (measured, with its accuracy said) or from the person, and it is always confirmed by
//! the person before it is kept. Never from an IP address or a third party's database (those put
//! the owner in Bogotá or Barranquilla while he was in Cartagena), and never sent anywhere.
//!
//! Until now each browser kept its own (`localStorage`), so ZERO's window, the person's browser and
//! «Órbita» could disagree. Now it is one setting of the service, behind the session token like the
//! language (see `idioma.rs`): `GET /api/ajustes/lugar` says it (or that there is none),
//! `PUT /api/ajustes/lugar` keeps a confirmed city, `DELETE /api/ajustes/lugar` forgets it.

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
const CLAVE: &str = "lugar_equipo";
/// The longest name kept (a city, or its region).
const MAX_NOMBRE: usize = 80;

pub(crate) fn rutas() -> Router<Arc<AppState>> {
    Router::new().route("/api/ajustes/lugar", get(ver).put(cambiar).delete(olvidar))
}

/// A confirmed place: the city, its region and country, its point, and how it was found.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub(crate) struct Lugar {
    ciudad: String,
    #[serde(default)]
    region: String,
    /// Two capital letters (`CO`).
    pais: String,
    lat: f64,
    lon: f64,
    /// `sistema` (the system's location, measured) or `manual` (chosen by the person).
    fuente: String,
    /// Metres, as the system said; absent for a chosen city.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    precision: Option<f64>,
    /// Milliseconds since 1970, when the person confirmed it.
    #[serde(default)]
    cuando: i64,
}

#[derive(Serialize, Debug, PartialEq)]
pub(crate) struct Respuesta {
    /// None until the person confirms a city.
    lugar: Option<Lugar>,
}

/// A place is kept only when every part of it makes sense.
pub(crate) fn valido(l: &Lugar) -> bool {
    let nombre = |s: &str| !s.contains(['<', '>', '\n', '\r']) && s.chars().count() <= MAX_NOMBRE;
    !l.ciudad.trim().is_empty()
        && nombre(&l.ciudad)
        && nombre(&l.region)
        && l.pais.len() == 2
        && l.pais.bytes().all(|b| b.is_ascii_uppercase())
        && l.lat.is_finite()
        && l.lon.is_finite()
        && (-90.0..=90.0).contains(&l.lat)
        && (-180.0..=180.0).contains(&l.lon)
        && (l.fuente == "sistema" || l.fuente == "manual")
        && l.precision.is_none_or(|p| p.is_finite() && p >= 0.0)
}

/// What is stored, read back; anything unreadable or empty is no place at all.
pub(crate) fn de_ajuste(guardado: Option<&str>) -> Respuesta {
    let lugar = guardado
        .filter(|s| !s.trim().is_empty())
        .and_then(|s| serde_json::from_str::<Lugar>(s).ok())
        .filter(valido);
    Respuesta { lugar }
}

async fn ver(State(state): State<Arc<AppState>>, _s: Session) -> ApiResult<Respuesta> {
    let guardado = with_ledger(&state, |l| l.setting(CLAVE))?;
    Ok(Json(de_ajuste(guardado.as_deref())))
}

async fn cambiar(
    State(state): State<Arc<AppState>>,
    _s: Session,
    Json(mut body): Json<Lugar>,
) -> ApiResult<Respuesta> {
    body.ciudad = body.ciudad.trim().to_owned();
    body.region = body.region.trim().to_owned();
    // Kept to about a hundred metres: enough for a city, no more than the person needs to give.
    body.lat = (body.lat * 1000.0).round() / 1000.0;
    body.lon = (body.lon * 1000.0).round() / 1000.0;
    if !valido(&body) {
        return Err((StatusCode::BAD_REQUEST, "invalid place").into_response());
    }
    let texto = serde_json::to_string(&body)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "place").into_response())?;
    with_ledger(&state, |l| l.set_setting(CLAVE, &texto))?;
    Ok(Json(Respuesta { lugar: Some(body) }))
}

async fn olvidar(State(state): State<Arc<AppState>>, _s: Session) -> ApiResult<Respuesta> {
    with_ledger(&state, |l| l.set_setting(CLAVE, ""))?;
    Ok(Json(Respuesta { lugar: None }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cartagena() -> Lugar {
        Lugar {
            ciudad: "Cartagena".into(),
            region: "Bolívar".into(),
            pais: "CO".into(),
            lat: 10.391,
            lon: -75.479,
            fuente: "sistema".into(),
            precision: Some(40.0),
            cuando: 1_792_464_000_000,
        }
    }

    #[test]
    fn a_confirmed_city_comes_back_as_it_was_kept() {
        let texto = serde_json::to_string(&cartagena()).unwrap();
        assert_eq!(de_ajuste(Some(&texto)).lugar, Some(cartagena()));
    }

    #[test]
    fn nothing_kept_or_nothing_readable_is_no_place() {
        assert_eq!(de_ajuste(None).lugar, None);
        assert_eq!(de_ajuste(Some("")).lugar, None);
        assert_eq!(de_ajuste(Some("{roto")).lugar, None);
    }

    #[test]
    fn only_sensible_places_are_kept() {
        let mut l = cartagena();
        assert!(valido(&l));
        l.pais = "co".into();
        assert!(!valido(&l));
        let mut l = cartagena();
        l.lat = 91.0;
        assert!(!valido(&l));
        let mut l = cartagena();
        l.fuente = "ip".into();
        assert!(!valido(&l), "never a place guessed from an address");
        let mut l = cartagena();
        l.ciudad = "<script>".into();
        assert!(!valido(&l));
        let mut l = cartagena();
        l.ciudad = "  ".into();
        assert!(!valido(&l));
        let mut l = cartagena();
        l.precision = Some(f64::NAN);
        assert!(!valido(&l));
    }
}
