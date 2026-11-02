//! La VPN en el panel (en preparación; docs/VPN.md, «Pendiente» 1): su estado y los botones de conectar y desconectar.
//!
//! Solo en Windows, y con el `wireguard.dll` firmado por WireGuard LLC junto al programa (lo pone el instalador desde
//! 1.0.52, con su huella comprobada): en otro sistema, o sin él, el estado dice que no está disponible y por qué. La conexión entera la hace
//! `guardiana_apps::tunel_win::conecta` (llaves nuevas, alta por HTTPS, túnel con todo dentro y el corte puesto, y el
//! primer saludo del servidor; si no llega, el túnel se cierra y el equipo queda como estaba). Tarda unos segundos, así
//! que «conectar» la empieza y la página pregunta por el estado. Al parar el servicio, el túnel se cierra con él.
//!
//! Nada de esto guarda llaves ni a qué servidor se conectó nadie: el estado vive en la memoria del servicio.
// Handlers answer axum's own `Response` on error, as every module of the panel does (see `api.rs`).
#![allow(clippy::result_large_err)]

use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::ApiResult;
use crate::auth::Session;
use crate::AppState;

/// En qué está la VPN. Fuera de Windows nunca llega a conectada ni a error: no se conecta.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
enum Fase {
    #[default]
    Apagada,
    Conectando,
    Conectada,
    Error,
}

impl Fase {
    fn como_texto(self) -> &'static str {
        match self {
            Self::Apagada => "apagada",
            Self::Conectando => "conectando",
            Self::Conectada => "conectada",
            Self::Error => "error",
        }
    }
}

/// Lo que la página enseña.
#[derive(Debug, Clone, Default)]
struct Estado {
    fase: Fase,
    servidor: String,
    desde_ms: u64,
    direccion: String,
    error: String,
}

/// La VPN del servicio: su estado y, en Windows, la conexión abierta.
#[derive(Default)]
pub(crate) struct Vpn {
    estado: Mutex<Estado>,
    #[cfg(windows)]
    conexion: Mutex<Option<guardiana_apps::tunel_win::Conexion>>,
}

/// Por qué la VPN no se puede usar aquí, si no se puede.
fn no_disponible() -> Option<&'static str> {
    #[cfg(windows)]
    {
        let dll = std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|p| p.join("wireguard.dll")));
        if !dll.is_some_and(|d| d.is_file()) {
            return Some("falta wireguard.dll junto al programa");
        }
        None
    }
    #[cfg(not(windows))]
    {
        Some("la VPN de GUARDIANA es solo para Windows")
    }
}

impl Vpn {
    fn estado(&self) -> Estado {
        self.estado.lock().map(|e| e.clone()).unwrap_or_default()
    }

    fn pon(&self, e: Estado) {
        if let Ok(mut g) = self.estado.lock() {
            *g = e;
        }
    }

    /// El estado para la página, con lo que el controlador dice del servidor cuando está conectada.
    fn foto(&self) -> Value {
        let e = self.estado();
        #[cfg(windows)]
        let par = self
            .conexion
            .lock()
            .ok()
            .and_then(|c| c.as_ref().and_then(|c| c.tunel.estado()));
        #[cfg(not(windows))]
        let par: Option<guardiana_tunel::nt::EstadoPar> = None;
        let motivo = no_disponible();
        json!({
            "disponible": motivo.is_none(),
            "motivo": motivo,
            "fase": e.fase.como_texto(),
            "servidor": e.servidor,
            "desde_ms": e.desde_ms,
            "direccion": e.direccion,
            "error": e.error,
            "enviados": par.as_ref().map(|p| p.enviados),
            "recibidos": par.as_ref().map(|p| p.recibidos),
            "ultimo_saludo_ms": par.and_then(|p| p.ultimo_saludo_ms),
        })
    }

    /// La conexión entera, fuera del hilo de la página (tarda unos segundos).
    #[cfg(windows)]
    fn haz_conexion(&self, host: &str) {
        let resultado = std::env::current_exe()
            .map_err(|e| e.to_string())
            .and_then(|exe| {
                let dll = exe
                    .parent()
                    .map(|p| p.join("wireguard.dll"))
                    .ok_or_else(|| "no se sabe dónde está el programa".to_owned())?;
                guardiana_apps::tunel_win::conecta(host, &dll, &exe).map_err(|e| e.to_string())
            });
        match resultado {
            Ok(c) => {
                let estado = Estado {
                    fase: Fase::Conectada,
                    servidor: c.servidor.clone(),
                    desde_ms: c.desde_ms,
                    direccion: c.direccion.to_string(),
                    error: String::new(),
                };
                if let Ok(mut g) = self.conexion.lock() {
                    *g = Some(c);
                }
                self.pon(estado);
            }
            Err(error) => self.pon(Estado {
                fase: Fase::Error,
                servidor: host.to_owned(),
                error,
                ..Estado::default()
            }),
        }
    }
}

/// `GET /api/vpn`: en qué está la VPN.
pub(crate) async fn ver(State(state): State<Arc<AppState>>, _s: Session) -> ApiResult<Value> {
    Ok(Json(state.vpn.foto()))
}

/// Lo que pide «conectar».
#[derive(Debug, Default, Deserialize)]
pub(crate) struct Pedido {
    /// El servidor: un nombre de guardianagroup.com. Vacío (o sin cuerpo, como pide la web de GUARDIANA a través de
    /// ZERO): el de [`SERVIDOR`].
    #[serde(default)]
    servidor: String,
}

/// El servidor al que se conecta quien no elige uno: el nombre que tendrá el primer extremo (Helsinki) cuando esté
/// dentro del sello (docs/VPN.md). Hasta entonces el nombre no resuelve y la conexión lo dice.
pub(crate) const SERVIDOR: &str = "vpn.guardianagroup.com";

/// `POST /api/vpn/conecta` `{"servidor": "…guardianagroup.com"}` (o `{}`): empieza a conectar; la página pregunta a
/// `GET /api/vpn` cómo va.
pub(crate) async fn conecta(
    State(state): State<Arc<AppState>>,
    _s: Session,
    cuerpo: axum::body::Bytes,
) -> ApiResult<Value> {
    if let Some(motivo) = no_disponible() {
        return Ok(Json(json!({ "ok": false, "motivo": motivo })));
    }
    // Sin cuerpo, o `{}`: el servidor por defecto. Otra cosa que no sea el JSON del pedido: se dice.
    let pedido = if cuerpo.iter().all(u8::is_ascii_whitespace) {
        Pedido::default()
    } else {
        match serde_json::from_slice::<Pedido>(&cuerpo) {
            Ok(p) => p,
            Err(_) => {
                return Ok(Json(
                    json!({ "ok": false, "motivo": "el pedido no se entiende" }),
                ))
            }
        }
    };
    let host = match pedido.servidor.trim() {
        "" => SERVIDOR.to_owned(),
        s => s.to_ascii_lowercase(),
    };
    if !guardiana_tunel::alta::es_servidor(&host) {
        return Ok(Json(
            json!({ "ok": false, "motivo": "ese servidor no es de GUARDIANA" }),
        ));
    }
    {
        let Ok(mut e) = state.vpn.estado.lock() else {
            return Ok(Json(
                json!({ "ok": false, "motivo": "estado no disponible" }),
            ));
        };
        if matches!(e.fase, Fase::Conectando | Fase::Conectada) {
            return Ok(Json(
                json!({ "ok": false, "motivo": "ya está conectada, o conectando" }),
            ));
        }
        *e = Estado {
            fase: Fase::Conectando,
            servidor: host.clone(),
            ..Estado::default()
        };
    }
    #[cfg(windows)]
    {
        let st = Arc::clone(&state);
        tokio::task::spawn_blocking(move || st.vpn.haz_conexion(&host));
    }
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/vpn/desconecta`: cierra el túnel (el corte con él); mientras conecta, espera a que termine.
pub(crate) async fn desconecta(
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<Value> {
    if state.vpn.estado().fase == Fase::Conectando {
        return Ok(Json(
            json!({ "ok": false, "motivo": "está conectando: espera a que termine" }),
        ));
    }
    #[cfg(windows)]
    {
        let abierta = state.vpn.conexion.lock().ok().and_then(|mut g| g.take());
        if let Some(c) = abierta {
            let _ = tokio::task::spawn_blocking(move || c.tunel.cierra()).await;
        }
    }
    state.vpn.pon(Estado::default());
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn el_estado_para_la_pagina() {
        let v = Vpn::default();
        let f = v.foto();
        assert_eq!(f["fase"], "apagada");
        assert_eq!(f["enviados"], Value::Null);
        #[cfg(not(windows))]
        {
            assert_eq!(f["disponible"], false);
            assert_eq!(f["motivo"], "la VPN de GUARDIANA es solo para Windows");
        }
        v.pon(Estado {
            fase: Fase::Error,
            servidor: "motor.guardianagroup.com".into(),
            error: "el servidor no contesta".into(),
            ..Estado::default()
        });
        let f = v.foto();
        assert_eq!(f["fase"], "error");
        assert_eq!(f["error"], "el servidor no contesta");
    }
}
