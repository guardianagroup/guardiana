//! The free level and Plus, as the panel applies them (the owner's free plan, approved on
//! 18 Oct 2026).
//!
//! When the trial ends without a licence GUARDIANA does not stop: it goes on at the free level
//! (`guardiana_license::Nivel::Gratis`). The free level keeps the live globe, the DNS view of
//! every program with its country, company and category, the person's own cuts of a name or a
//! program and their undo, the last 24 hours of history and this one computer. Everything else
//! is Plus (trial, subscription or Lifetime, sold as «Fundador» until 18 Oct 2026).
//!
//! Two tools for the rest of the panel:
//!
//! - [`solo_plus`], a middleware on the whole router: the routes in [`RUTAS_PLUS`] answer
//!   `402` with `{"plan":"plus","texto":…,"enlace":…,"boton":…}` at the free level, so a page
//!   can show one short line and «Pasar a Plus» instead of an error. A new Plus feature only adds
//!   its prefix to that list.
//! - [`desde`], which moves the start of a history window to the last 24 hours at the free
//!   level. It is a display filter: nothing is deleted. The ledger keeps every row, chained, and
//!   all of it shows again with Plus (the program never deletes a piece of the person's extract
//!   to sell them the rest; see the hourly tick in the engine).
//!
//! `/api/plan` says which level is in force, for the pages.

// Handlers return a full HTTP `Response` as their error, as axum intends.
#![allow(clippy::result_large_err)]

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use guardiana_core::i18n::Texts;
use guardiana_core::time::now_ms;
use guardiana_license::Nivel;
use serde::Serialize;

use crate::api::{with_ledger, ApiResult};
use crate::auth::{Lang, Session};
use crate::AppState;

/// The routes that are Plus: (method, path prefix). `None` is any method. At the free level they
/// answer `402` with the line the page shows. Prefixes of features that live on other branches
/// of 1.0.45 (Vigilancia, Contrato vivo, Certificado) are here already, so they are gated the
/// day they are merged; a prefix that matches no route costs nothing.
pub(crate) const RUTAS_PLUS: &[(Option<Method>, &str)] = &[
    // Automatic cuts: «Protección máxima» sets one rule per category.
    (Some(Method::POST), "/api/proteccion-maxima"),
    // Home Mode: the rest of the house.
    (Some(Method::POST), "/api/hogar/activar"),
    // The weekly report.
    (None, "/api/informe"),
    // Rights: the letters and the requests sent with them.
    (Some(Method::POST), "/api/expedientes"),
    (Some(Method::GET), "/api/expedientes/"),
    (Some(Method::GET), "/api/borrado/carta"),
    (Some(Method::POST), "/api/borrado/envios"),
    // Other branches of 1.0.45.
    (None, "/api/vigilancia"),
    (None, "/api/contratos"),
    (None, "/api/certificado"),
    (None, "/api/derechos"),
];

/// Whether `method path` is one of [`RUTAS_PLUS`]. `GET /api/expedientes/{id}/carta` and
/// `/queja` are letters; `GET /api/expedientes` (the list) is not.
pub(crate) fn es_plus(method: &Method, path: &str) -> bool {
    RUTAS_PLUS.iter().any(|(m, prefijo)| {
        m.as_ref().is_none_or(|m| m == method)
            && path.starts_with(prefijo)
            && (*prefijo != "/api/expedientes/"
                || path.ends_with("/carta")
                || path.ends_with("/queja"))
    })
}

/// The level in force. A licence that cannot be read counts as Plus: an error must never take
/// features away from someone who paid.
pub(crate) fn nivel(state: &AppState) -> Nivel {
    with_ledger(state, |l| {
        Ok(guardiana_license::status(l, &state.token, now_ms()).ok())
    })
    .ok()
    .flatten()
    .map_or(Nivel::Plus, |s| s.nivel)
}

/// The start of a history window as the level allows it: `since` with Plus, at most the last
/// 24 hours at the free level.
pub(crate) fn desde_para(nivel: Nivel, since: i64, now: i64) -> i64 {
    match nivel.historial_ms() {
        Some(ms) => since.max(now - ms),
        None => since,
    }
}

/// [`desde_para`] with the level in force.
pub(crate) fn desde(state: &AppState, since: i64, now: i64) -> i64 {
    desde_para(nivel(state), since, now)
}

/// The answer of a Plus route at the free level: one line and the way to Plus, never a bare
/// error.
#[derive(Serialize)]
pub(crate) struct SoloPlus {
    /// Always `plus`: what the route needs.
    plan: &'static str,
    texto: String,
    boton: String,
    enlace: String,
}

pub(crate) fn respuesta_plus(t: &Texts, texto: &str) -> Response {
    (
        StatusCode::PAYMENT_REQUIRED,
        Json(SoloPlus {
            plan: "plus",
            texto: t.panel(texto).to_owned(),
            boton: t.panel("plan_pasar_plus").to_owned(),
            enlace: t.panel("licencia_comprar_url").to_owned(),
        }),
    )
        .into_response()
}

/// Middleware: the routes of [`RUTAS_PLUS`] answer [`respuesta_plus`] at the free level. The
/// licence is read only for those routes, never for the others.
pub(crate) async fn solo_plus(
    State(state): State<Arc<AppState>>,
    Lang(t): Lang,
    req: Request,
    next: Next,
) -> Response {
    if es_plus(req.method(), req.uri().path()) && nivel(&state) == Nivel::Gratis {
        let texto = if req.uri().path().starts_with("/api/hogar") {
            "plan_hogar_plus"
        } else if req.uri().path().starts_with("/api/proteccion-maxima") {
            "plan_categoria_plus"
        } else {
            "plan_plus_linea"
        };
        return respuesta_plus(t, texto);
    }
    next.run(req).await
}

/// What `/api/plan` says.
#[derive(Serialize)]
pub(crate) struct PlanView {
    /// `plus` or `gratis`.
    nivel: &'static str,
    /// How far back the history is shown, in hours; `null` is all of it.
    historial_horas: Option<i64>,
    /// Devices this level watches.
    dispositivos: usize,
    /// The pricing page, in the panel's language.
    enlace: String,
}

/// `/api/plan`: the level in force.
pub(crate) async fn ver(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<PlanView> {
    let n = nivel(&state);
    Ok(Json(PlanView {
        nivel: n.nombre(),
        historial_horas: n
            .historial_ms()
            .map(|ms| ms / guardiana_core::time::HOUR_MS),
        dispositivos: n.dispositivos(),
        enlace: t.panel("licencia_comprar_url").to_owned(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn las_rutas_de_plus_son_las_de_la_lista() {
        assert!(es_plus(&Method::POST, "/api/proteccion-maxima"));
        assert!(!es_plus(&Method::GET, "/api/proteccion-maxima"));
        assert!(es_plus(&Method::POST, "/api/hogar/activar"));
        assert!(
            !es_plus(&Method::POST, "/api/hogar/desactivar"),
            "switching off stays free"
        );
        assert!(es_plus(&Method::GET, "/api/informe"));
        assert!(es_plus(&Method::GET, "/api/expedientes/3/carta"));
        assert!(!es_plus(&Method::GET, "/api/expedientes"));
        assert!(es_plus(&Method::POST, "/api/expedientes"));
        assert!(es_plus(&Method::GET, "/api/vigilancia/resumen"));
        // What the free level keeps: the globe, the DNS view, manual cuts and their undo.
        assert!(!es_plus(&Method::GET, "/api/por-dentro"));
        assert!(!es_plus(&Method::POST, "/api/reglas"));
        assert!(!es_plus(&Method::POST, "/api/reglas/4/deshacer"));
        assert!(!es_plus(&Method::POST, "/api/antivirus/cortar"));
        assert!(!es_plus(&Method::GET, "/api/extracto/exportar"));
        assert!(!es_plus(&Method::GET, "/api/licencia"));
    }

    #[test]
    fn gratis_ve_las_ultimas_24_horas_y_plus_todo() {
        let ahora = 100 * guardiana_core::time::DAY_MS;
        let hace_una_semana = ahora - 7 * guardiana_core::time::DAY_MS;
        assert_eq!(
            desde_para(Nivel::Plus, hace_una_semana, ahora),
            hace_una_semana
        );
        assert_eq!(
            desde_para(Nivel::Gratis, hace_una_semana, ahora),
            ahora - guardiana_core::time::DAY_MS
        );
        // A window already inside the day is left as it is.
        assert_eq!(
            desde_para(Nivel::Gratis, ahora - 60_000, ahora),
            ahora - 60_000
        );
    }
}
