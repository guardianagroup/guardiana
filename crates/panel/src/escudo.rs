//! «El escudo» in the panel (1.0.52, in preparation; docs/ESCUDO.md): what the antivirus' «primera vez» rule has seen,
//! as the service left it next to the ledger (`escudo-avisos.jsonl` and `escudo-memoria.json`, see
//! `crates/cli/src/escudo.rs`). Read only.

// The handler answers with the same error type as the rest of the API (see api.rs).
#![allow(clippy::result_large_err)]

use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use guardiana_core::time::now_ms;
use guardiana_escudo::Memoria;
use serde::Serialize;
use serde_json::Value;

use crate::api::ApiResult;
use crate::auth::Session;
use crate::AppState;

/// Warnings sent to the page, the newest first.
const MAX: usize = 100;

#[derive(Serialize)]
pub(crate) struct Escudo {
    /// Whether the memory is still in its first days, when it only learns and never warns.
    aprendiendo: bool,
    /// Until when it only learns (milliseconds since 1970); 0 when the service has not started it yet.
    aprende_hasta_ms: u64,
    /// How many sites this computer has asked for since the memory started.
    sitios: usize,
    /// The warnings, the newest first.
    avisos: Vec<Value>,
}

/// `GET /api/escudo`: the rule's state and its warnings.
pub(crate) async fn ver(State(state): State<Arc<AppState>>, _s: Session) -> ApiResult<Escudo> {
    let memoria = std::fs::read_to_string(state.db_path.with_file_name("escudo-memoria.json"))
        .ok()
        .and_then(|j| Memoria::desde_json(&j).ok());
    let ahora = u64::try_from(now_ms()).unwrap_or(0);
    let mut avisos: Vec<Value> =
        std::fs::read_to_string(state.db_path.with_file_name("escudo-avisos.jsonl"))
            .map(|t| {
                t.lines()
                    .filter_map(|l| serde_json::from_str(l).ok())
                    .collect()
            })
            .unwrap_or_default();
    avisos.reverse();
    avisos.truncate(MAX);
    Ok(Json(Escudo {
        aprendiendo: memoria.as_ref().is_none_or(|m| m.aprendiendo(ahora)),
        aprende_hasta_ms: memoria.as_ref().map_or(0, Memoria::aprende_hasta_ms),
        sitios: memoria.as_ref().map_or(0, Memoria::sitios),
        avisos,
    }))
}
