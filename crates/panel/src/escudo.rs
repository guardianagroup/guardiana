//! «El escudo» in the panel (1.0.52, in preparation; docs/ESCUDO.md): what the antivirus' rules have seen, as the
//! service left it next to the ledger (`escudo-avisos.jsonl` and `escudo-memoria.json`, see `crates/cli/src/escudo.rs`).
//!
//! And the quarantine (ronda 88, 1 Nov 2026: «Cuarentena desde la lista de avisos: sí»): the file a warning names can be
//! put in the quarantine (`guardiana_escudo::cuarentena`: encrypted, inert, and always restorable exactly as it was),
//! only when the escudo named it, it is still there, and it is not in Windows' folder, Program Files or Microsoft
//! Defender's. GUARDIANA's web in the program may ask for that (by the warning's id, never a path); putting a file back
//! is done from the panel itself.

// The handler answers with the same error type as the rest of the API (see api.rs).
#![allow(clippy::result_large_err)]

use std::collections::HashSet;
use std::path::{Path as Ruta, PathBuf};
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use guardiana_core::time::now_ms;
use guardiana_escudo::cuarentena::Cuarentena;
use guardiana_escudo::Memoria;
use serde::Serialize;
use serde_json::{json, Value};

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

/// Where the quarantine lives: next to the ledger, in the service's own folder.
fn carpeta_cuarentena(db: &Ruta) -> PathBuf {
    db.with_file_name("cuarentena")
}

/// The warnings as the service wrote them, oldest first, each with its id: its line in the file and when it was seen
/// (`3-1793589348314`), so an id never names another warning.
fn avisos_del_archivo(db: &Ruta) -> Vec<(String, Value)> {
    std::fs::read_to_string(db.with_file_name("escudo-avisos.jsonl"))
        .map(|t| {
            t.lines()
                .enumerate()
                .filter_map(|(i, l)| {
                    let v: Value = serde_json::from_str(l).ok()?;
                    let cuando = v["cuando_ms"].as_u64().unwrap_or(0);
                    Some((format!("{i}-{cuando}"), v))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Whether a path is somewhere the quarantine never touches: Windows' folder, the installed programs' and Microsoft
/// Defender's.
fn protegida(ruta: &str) -> bool {
    let r = ruta.replace('/', "\\").to_ascii_lowercase();
    let r = r.trim_start_matches(r"\\?\");
    r.get(1..).is_none_or(|x| {
        x.starts_with(r":\windows\")
            || x.starts_with(r":\program files\")
            || x.starts_with(r":\program files (x86)\")
            || x.starts_with(r":\programdata\microsoft\")
    })
}

/// Whether the file a warning names can go to the quarantine now.
fn se_puede(ruta: &str) -> bool {
    if ruta.is_empty() || !Ruta::new(ruta).is_absolute() || protegida(ruta) {
        return false;
    }
    // The canonical path too: links, `..`, short 8.3 names and `\\?\` prefixes are resolved by Windows there.
    let Ok(canonica) = std::fs::canonicalize(ruta) else {
        return false;
    };
    canonica.is_file() && !protegida(&canonica.to_string_lossy())
}

/// The newest [`MAX`] warnings, each with its id and whether its file can go to the quarantine (`cuarentena`: `posible`,
/// `hecha` or empty). Looks at files: not for the page's thread.
fn avisos_con_cuarentena(db: &Ruta) -> Vec<Value> {
    let en_cuarentena: HashSet<String> = Cuarentena::en(carpeta_cuarentena(db))
        .map(|c| {
            c.lista()
                .into_iter()
                .map(|e| e.ruta.to_lowercase())
                .collect()
        })
        .unwrap_or_default();
    let mut avisos: Vec<Value> = avisos_del_archivo(db)
        .into_iter()
        .map(|(id, mut v)| {
            let ruta = v["ruta"].as_str().unwrap_or_default().to_owned();
            let estado = if en_cuarentena.contains(&ruta.to_lowercase()) {
                "hecha"
            } else if se_puede(&ruta) {
                "posible"
            } else {
                ""
            };
            if let Some(o) = v.as_object_mut() {
                o.insert("id".into(), Value::String(id));
                o.insert("cuarentena".into(), Value::String(estado.into()));
            }
            v
        })
        .collect();
    avisos.reverse();
    avisos.truncate(MAX);
    avisos
}

/// `GET /api/escudo`: the rules' state and their warnings, each with its id and whether its file can go to the
/// quarantine (`cuarentena`: `posible`, `hecha` or empty).
pub(crate) async fn ver(State(state): State<Arc<AppState>>, _s: Session) -> ApiResult<Escudo> {
    let memoria = std::fs::read_to_string(state.db_path.with_file_name("escudo-memoria.json"))
        .ok()
        .and_then(|j| Memoria::desde_json(&j).ok());
    let ahora = u64::try_from(now_ms()).unwrap_or(0);
    // Files are looked at (the quarantine's list, each warning's file): off the page's thread.
    let db = state.db_path.clone();
    let avisos = tokio::task::spawn_blocking(move || avisos_con_cuarentena(&db))
        .await
        .unwrap_or_default();
    Ok(Json(Escudo {
        aprendiendo: memoria.as_ref().is_none_or(|m| m.aprendiendo(ahora)),
        aprende_hasta_ms: memoria.as_ref().map_or(0, Memoria::aprende_hasta_ms),
        sitios: memoria.as_ref().map_or(0, Memoria::sitios),
        avisos,
    }))
}

/// `GET /api/escudo/cuarentena`: what is in the quarantine, the newest first, with each file's name.
pub(crate) async fn cuarentena(
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<Value> {
    let mut entradas: Vec<Value> = Cuarentena::en(carpeta_cuarentena(&state.db_path))
        .map(|c| c.lista())
        .unwrap_or_default()
        .into_iter()
        .map(|e| {
            let nombre = e
                .ruta
                .rsplit(['\\', '/'])
                .next()
                .unwrap_or_default()
                .to_owned();
            json!({
                "id": e.id,
                "nombre": nombre,
                "ruta": e.ruta,
                "bytes": e.bytes,
                "motivo": e.motivo,
                "fuente": e.fuente,
                "cuando_ms": e.cuando_ms,
            })
        })
        .collect();
    entradas.reverse();
    Ok(Json(json!({ "entradas": entradas })))
}

/// `POST /api/escudo/cuarentena/{id}`: the file the warning `id` names, to the quarantine, when it can go (see the
/// module). A program that is still open cannot be moved: it says so.
pub(crate) async fn poner(
    State(state): State<Arc<AppState>>,
    _s: Session,
    Path(id): Path<String>,
) -> ApiResult<Value> {
    let Some(aviso) = avisos_del_archivo(&state.db_path)
        .into_iter()
        .find(|(i, _)| *i == id)
        .map(|(_, v)| v)
    else {
        return Ok(Json(
            json!({ "ok": false, "motivo": "ese aviso ya no está" }),
        ));
    };
    let ruta = aviso["ruta"].as_str().unwrap_or_default().to_owned();
    if !se_puede(&ruta) {
        return Ok(Json(json!({
            "ok": false,
            "motivo": "ese archivo no se puede poner en cuarentena: ya no está, o está en la carpeta de Windows o en la de los programas instalados"
        })));
    }
    let motivo: String = aviso["texto"]
        .as_str()
        .unwrap_or_default()
        .chars()
        .take(400)
        .collect();
    let fuente = aviso["fuente"].as_str().map_or_else(
        || {
            format!(
                "escudo de GUARDIANA, regla «{}»",
                aviso["regla"].as_str().unwrap_or_default()
            )
        },
        str::to_owned,
    );
    let carpeta = carpeta_cuarentena(&state.db_path);
    let ahora = u64::try_from(now_ms()).unwrap_or(0);
    let hecho = tokio::task::spawn_blocking(move || {
        Cuarentena::en(carpeta)
            .map_err(|e| e.to_string())
            .and_then(|c| {
                c.poner(Ruta::new(&ruta), &motivo, &fuente, ahora)
                    .map_err(|e| e.to_string())
            })
    })
    .await;
    Ok(Json(match hecho {
        Ok(Ok(e)) => json!({ "ok": true, "id": e.id }),
        Ok(Err(m)) if m.contains("no se pudo quitar el original") => json!({
            "ok": false,
            "motivo": "está abierto o en uso: ciérralo y vuelve a intentarlo"
        }),
        Ok(Err(m)) => json!({ "ok": false, "motivo": m }),
        Err(_) => json!({ "ok": false, "motivo": "no se pudo terminar" }),
    }))
}

/// `POST /api/escudo/cuarentena/restaura/{id}`: the quarantine's entry `id` back where it was, exactly as it was (never
/// over a file that is there now). Only from the panel itself: GUARDIANA's web cannot ask for it.
pub(crate) async fn restaura(
    State(state): State<Arc<AppState>>,
    _s: Session,
    Path(id): Path<String>,
) -> ApiResult<Value> {
    let carpeta = carpeta_cuarentena(&state.db_path);
    let hecho = tokio::task::spawn_blocking(move || {
        Cuarentena::en(carpeta)
            .map_err(|e| e.to_string())
            .and_then(|c| c.restaurar(&id, None).map_err(|e| e.to_string()))
    })
    .await;
    Ok(Json(match hecho {
        Ok(Ok(ruta)) => json!({ "ok": true, "ruta": ruta.to_string_lossy() }),
        Ok(Err(m)) => json!({ "ok": false, "motivo": m }),
        Err(_) => json!({ "ok": false, "motivo": "no se pudo terminar" }),
    }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn lo_que_la_cuarentena_no_toca() {
        for r in [
            r"C:\Windows\System32\drivers\etc\hosts",
            r"c:\program files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\x\x.exe",
            r"C:\ProgramData\Microsoft\Windows Defender\Platform\x\MsMpEng.exe",
            r"\\?\C:\Windows\x.exe",
            "",
        ] {
            assert!(protegida(r), "{r}");
        }
        assert!(!protegida(r"C:\Users\ana\Downloads\x.exe"));
        assert!(!se_puede(""));
    }

    #[test]
    fn cada_aviso_con_su_id() {
        let dir = std::env::temp_dir().join(format!("guardiana-escudo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("guardiana.db");
        std::fs::write(
            dir.join("escudo-avisos.jsonl"),
            "{\"regla\":\"medio\",\"cuando_ms\":5}\nno es json\n{\"regla\":\"engano\",\"cuando_ms\":7}\n",
        )
        .unwrap();
        let ids: Vec<String> = avisos_del_archivo(&db)
            .into_iter()
            .map(|(i, _)| i)
            .collect();
        assert_eq!(ids, vec!["0-5".to_owned(), "2-7".to_owned()]);
        // a real file outside the protected folders can go; a missing or relative one cannot
        let archivo = dir.join("sospechoso.bin");
        std::fs::write(&archivo, b"x").unwrap();
        assert!(se_puede(&archivo.to_string_lossy()));
        assert!(!se_puede(&dir.join("no-esta.bin").to_string_lossy()));
        assert!(!se_puede("relativa.exe"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
