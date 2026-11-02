//! La VPN en el panel (en preparación; docs/VPN.md, «Pendiente» 1): su estado y los botones de conectar y desconectar.
//!
//! Solo en Windows, y con el `wireguard.dll` firmado por WireGuard LLC junto al programa (lo pone el instalador desde
//! 1.0.52, con su huella comprobada): en otro sistema, o sin él, el estado dice que no está disponible y por qué. La conexión entera la hace
//! `guardiana_apps::tunel_win::conecta` (llaves nuevas, alta por HTTPS, túnel con todo dentro y el corte puesto, y el
//! primer saludo del servidor; si no llega, el túnel se cierra y el equipo queda como estaba). Tarda unos segundos, así
//! que «conectar» la empieza y la página pregunta por el estado. Al parar el servicio, el túnel se cierra con él.
//!
//! Mientras está abierta, cada [`VISTAZO`] se mira el último saludo del servidor (1.0.53): si deja de contestar
//! (`guardiana_tunel::conexion::caida`), «sin respuesta», y la página lo dice con el botón de desconectar; si vuelve a
//! contestar, otra vez «conectada». Nada sale fuera del túnel mientras tanto. El alta nueva automática
//! (`guardiana_apps::tunel_win::renueva`) no se usa todavía: la segunda revisión del 2 nov 2026 vio que su petición
//! entraría en el túnel caído (la ruta por defecto es la del túnel); irá en una versión posterior, con la ruta resuelta y
//! probada.
//!
//! La VPN vive lo que vive el proceso, no lo que dura cada vuelta del motor (1.0.54): el motor rehace el panel cuando
//! cambia la red, y con la VPN puesta la dirección del equipo pasa a ser la del túnel; hasta la 1.0.54 eso tiraba el
//! panel con la conexión dentro, el túnel quedaba huérfano y la página decía «apagada». Al parar el servicio para
//! siempre, el motor la cierra ([`cierra_la_vpn`]).
//!
//! Una conexión que termina de abrirse después de un cierre, o después de que empezara otra, se cierra en el acto y no
//! se guarda (1.0.55, tercera revisión del 2 nov 2026): conectar tarda unos segundos sin el candado puesto, y hasta la
//! 1.0.55 una que acababa tras [`cierra_la_vpn`] se guardaba y seguía abierta, con su corte, después del cierre. Lo
//! decide [`guarda_si_sigue`] con la generación.
//!
//! Nada de esto guarda llaves ni a qué servidor se conectó nadie: el estado vive en la memoria del servicio.
// Handlers answer axum's own `Response` on error, as every module of the panel does (see `api.rs`).
#![allow(clippy::result_large_err)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, PoisonError};
#[cfg(windows)]
use std::time::Duration;

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
    /// El servidor dejó de contestar; el túnel y su corte siguen puestos.
    SinRespuesta,
    Error,
}

impl Fase {
    fn como_texto(self) -> &'static str {
        match self {
            Self::Apagada => "apagada",
            Self::Conectando => "conectando",
            Self::Conectada => "conectada",
            Self::SinRespuesta => "sin_respuesta",
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

/// Cada cuánto se mira, con la VPN abierta, si el servidor sigue contestando.
#[cfg(windows)]
const VISTAZO: Duration = Duration::from_secs(10);

/// La VPN del servicio: su estado y, en Windows, la conexión abierta.
#[derive(Default)]
pub(crate) struct Vpn {
    estado: Mutex<Estado>,
    /// Cuántas conexiones se han empezado o cerrado: el vigía de una conexión se acaba en cuanto cambia, y una conexión
    /// que acaba de abrirse solo se guarda si no ha cambiado desde que empezó ([`guarda_si_sigue`]).
    generacion: AtomicU64,
    #[cfg(windows)]
    conexion: Mutex<Option<guardiana_apps::tunel_win::Conexion>>,
}

/// La VPN del proceso: la misma para todas las vueltas del motor y todos los paneles que levante.
pub(crate) fn global() -> &'static Vpn {
    static VPN: LazyLock<Vpn> = LazyLock::new(Vpn::default);
    &VPN
}

/// Al parar el servicio para siempre: el túnel se cierra (y su corte con él) y la VPN vuelve a «apagada». Una conexión
/// que aún esté a medias se cierra sola al terminar ([`guarda_si_sigue`]).
pub fn cierra_la_vpn() {
    let vpn = global();
    // el vigía de la conexión abierta se acaba, y la que esté conectando ya no se guardará: la generación sube antes de
    // tomar la guardada
    vpn.generacion.fetch_add(1, Ordering::SeqCst);
    #[cfg(windows)]
    {
        // también con el candado envenenado: una conexión que no se cierra aquí seguiría abierta tras el cierre
        let abierta = vpn
            .conexion
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(c) = abierta {
            c.tunel.cierra();
        }
    }
    vpn.pon(Estado::default());
}

/// Lo último de una conexión que acaba de abrirse (1.0.55): se guarda en `sitio` solo si `gen`, la generación con la
/// que empezó, sigue siendo la de `generacion`, es decir, si nadie cerró la VPN ni empezó otra mientras conectaba; y
/// entonces `al_guardar` corre con el candado puesto (el estado que ve la página). Si no, se devuelve, para cerrarla
/// en el acto.
///
/// Se mira con el candado de `sitio` puesto, y quien cierra sube la generación antes de tomar lo guardado: o la ve
/// guardada y la cierra él, o esta ve la generación nueva y no se guarda. Ninguna queda abierta sin que nadie la cierre.
#[cfg_attr(not(windows), allow(dead_code))]
fn guarda_si_sigue<C>(
    generacion: &AtomicU64,
    gen: u64,
    sitio: &Mutex<Option<C>>,
    nueva: C,
    al_guardar: impl FnOnce(&C),
) -> Option<C> {
    let mut guardada = sitio.lock().unwrap_or_else(PoisonError::into_inner);
    if generacion.load(Ordering::SeqCst) != gen {
        return Some(nueva);
    }
    al_guardar(&nueva);
    *guardada = Some(nueva);
    None
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
        // sin esperar: si la conexión está ocupada en ese momento, el estado sale sin bytes
        #[cfg(windows)]
        let par = self
            .conexion
            .try_lock()
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

    /// La conexión entera, fuera del hilo de la página (tarda unos segundos), de la generación `gen`. Dice si quedó
    /// abierta: no, si mientras conectaba se cerró la VPN o empezó otra; entonces se cierra en el acto
    /// ([`guarda_si_sigue`]).
    #[cfg(windows)]
    fn haz_conexion(&self, host: &str, gen: u64) -> bool {
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
            Ok(c) => match guarda_si_sigue(&self.generacion, gen, &self.conexion, c, |c| {
                self.pon(conectada(c));
            }) {
                None => true,
                Some(tarde) => {
                    // la VPN se cerró, o empezó otra, mientras conectaba: esta no sobrevive a ese cierre
                    tarde.tunel.cierra();
                    false
                }
            },
            Err(error) => {
                // con el candado de la conexión, como al guardarla: tras un cierre, la página dice lo que dejó quien
                // cerró
                let _guardada = self.conexion.lock().unwrap_or_else(PoisonError::into_inner);
                if self.generacion.load(Ordering::SeqCst) == gen {
                    self.pon(Estado {
                        fase: Fase::Error,
                        servidor: host.to_owned(),
                        error,
                        ..Estado::default()
                    });
                }
                false
            }
        }
    }
}

impl Vpn {
    /// El vigía de la conexión de la generación `gen`, en su propio hilo, hasta que se desconecte (o empiece otra): si el
    /// servidor deja de contestar, «sin respuesta»; si vuelve, «conectada». No toca el túnel ni su corte.
    #[cfg(windows)]
    fn vigila(&self, gen: u64) {
        use guardiana_tunel::conexion;
        loop {
            std::thread::sleep(VISTAZO);
            if self.generacion.load(Ordering::SeqCst) != gen {
                return;
            }
            let ahora = u64::try_from(guardiana_core::time::now_ms()).unwrap_or(0);
            let Ok(g) = self.conexion.lock() else {
                return;
            };
            // desconectada: se acabó
            let Some(c) = g.as_ref() else {
                return;
            };
            let saludo = c.tunel.estado().and_then(|e| e.ultimo_saludo_ms);
            let callado = conexion::caida(ahora, c.desde_ms, saludo, None);
            let fase = self.estado().fase;
            if callado && fase == Fase::Conectada {
                self.pon(Estado {
                    fase: Fase::SinRespuesta,
                    ..conectada(c)
                });
            } else if !callado && fase == Fase::SinRespuesta {
                self.pon(conectada(c));
            }
        }
    }
}

/// El estado de una conexión abierta.
#[cfg(windows)]
fn conectada(c: &guardiana_apps::tunel_win::Conexion) -> Estado {
    Estado {
        fase: Fase::Conectada,
        servidor: c.servidor.clone(),
        desde_ms: c.desde_ms,
        direccion: c.direccion.to_string(),
        error: String::new(),
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
        if matches!(
            e.fase,
            Fase::Conectando | Fase::Conectada | Fase::SinRespuesta
        ) {
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
    let gen = state.vpn.generacion.fetch_add(1, Ordering::SeqCst) + 1;
    #[cfg(windows)]
    {
        // la VPN del proceso, no el panel: el motor puede rehacer el panel mientras tanto
        let vpn = state.vpn;
        tokio::task::spawn_blocking(move || {
            if vpn.haz_conexion(&host, gen) {
                // el vigía, en un hilo suyo: no retiene el cierre del programa
                std::thread::spawn(move || vpn.vigila(gen));
            }
        });
    }
    #[cfg(not(windows))]
    let _ = gen;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/vpn/desconecta`: cierra el túnel (el corte con él); mientras conecta, espera a que termine. Sin respuesta
/// del servidor, también: es la salida para volver a la conexión normal.
pub(crate) async fn desconecta(
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<Value> {
    if state.vpn.estado().fase == Fase::Conectando {
        return Ok(Json(
            json!({ "ok": false, "motivo": "está conectando: espera a que termine" }),
        ));
    }
    // el vigía de esta conexión se acaba
    state.vpn.generacion.fetch_add(1, Ordering::SeqCst);
    #[cfg(windows)]
    {
        // fuera de los hilos de la página
        let vpn = state.vpn;
        let _ = tokio::task::spawn_blocking(move || {
            let abierta = vpn
                .conexion
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take();
            if let Some(c) = abierta {
                c.tunel.cierra();
            }
            vpn.pon(Estado::default());
        })
        .await;
    }
    #[cfg(not(windows))]
    state.vpn.pon(Estado::default());
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn la_vpn_es_la_misma_en_cada_vuelta_del_motor() {
        // dos paneles seguidos (el motor rehace el suyo cuando cambia la red) ven la misma VPN
        assert!(std::ptr::eq(global(), global()));
    }

    /// 1.0.55: una conexión que termina de abrirse después de un cierre (o de que empezara otra) no se guarda: vuelve
    /// para cerrarla en el acto. La de la generación en vigor sí, con su estado puesto bajo el mismo candado.
    #[test]
    fn una_conexion_que_acaba_tras_un_cierre_no_se_guarda() {
        let generacion = AtomicU64::new(0);
        let sitio: Mutex<Option<&str>> = Mutex::new(None);
        // empieza, y nadie cierra mientras conecta: se guarda
        let gen = generacion.fetch_add(1, Ordering::SeqCst) + 1;
        let mut puesta = false;
        assert_eq!(
            guarda_si_sigue(&generacion, gen, &sitio, "primera", |_| puesta = true),
            None
        );
        assert!(puesta);
        assert_eq!(*sitio.lock().unwrap(), Some("primera"));
        // el cierre: sube la generación y después toma la guardada
        generacion.fetch_add(1, Ordering::SeqCst);
        assert_eq!(sitio.lock().unwrap().take(), Some("primera"));
        // una que empezó antes del cierre y acaba después: vuelve, para cerrarla, y nada se guarda ni se pone
        let mut puesta = false;
        assert_eq!(
            guarda_si_sigue(&generacion, gen, &sitio, "tarde", |_| puesta = true),
            Some("tarde")
        );
        assert!(!puesta);
        assert_eq!(*sitio.lock().unwrap(), None);
        // y la de una conexión nueva, de la generación en vigor, sí
        let otra = generacion.fetch_add(1, Ordering::SeqCst) + 1;
        assert_eq!(
            guarda_si_sigue(&generacion, otra, &sitio, "nueva", |_| {}),
            None
        );
        assert_eq!(*sitio.lock().unwrap(), Some("nueva"));
    }

    /// Lo que el corte de la VPN deja entrar al servicio (`guardiana_tunel::corte::HOGAR`, 1.0.55) es lo que sirve el
    /// Modo Hogar a la casa: su DNS, su panel y su página de comprobación.
    #[test]
    fn el_corte_deja_entrar_lo_que_sirve_el_modo_hogar() {
        use guardiana_service::home;
        use guardiana_tunel::corte::{HOGAR, TCP, UDP};
        let casa = std::net::Ipv4Addr::new(192, 168, 1, 10);
        let (dns, panel) = home::lan_listen_addrs(casa);
        let comprobar = home::lan_checker_addr(casa);
        assert_eq!(
            HOGAR,
            [
                (UDP, dns.port()),
                (TCP, dns.port()),
                (TCP, panel.port()),
                (TCP, comprobar.port())
            ]
        );
    }

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
        // el servidor dejó de contestar con el túnel puesto (desde la 1.0.53)
        v.pon(Estado {
            fase: Fase::SinRespuesta,
            servidor: "vpn.guardianagroup.com".into(),
            direccion: "10.66.0.7".into(),
            error: "el servidor aún no contesta".into(),
            ..Estado::default()
        });
        let f = v.foto();
        assert_eq!(f["fase"], "sin_respuesta");
        assert_eq!(f["direccion"], "10.66.0.7");
    }
}
