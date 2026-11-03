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
//! El modo estricto (ronda 91, la prioridad del dueño: «No se puede caer entre medias… Ahí tenemos que ser impecables»;
//! docs/VPN.md, «Modo estricto»), encendido salvo que la persona lo apague: el bloqueo vive en Windows aparte del
//! servicio (`guardiana_apps::corte_vpn::pon_estricto`, filtros persistentes y de arranque), así que sigue si el servicio
//! se cae, se para para una actualización o el equipo se reinicia. Lo que la persona quiere (el modo, si la VPN está
//! encendida y su servidor) se guarda en `sistema/vpn.json` (`guardiana_tunel::estricto::Deseo`), y con eso el servicio,
//! al arrancar, vuelve a levantar el túnel o quita lo que quedó ([`vpn_al_arrancar`]). Si el servidor no contesta, el
//! túnel se cierra y el bloqueo sigue: no sale nada, la página lo dice («reconectando», con el aviso) y se vuelve a
//! intentar, hasta que conecte o la persona la apague. Las salidas, siempre: «Desconectar» (el panel, Órbita y la barra
//! de ZERO, `POST /api/vpn/desconecta`, en cualquier fase), `guardiana vpn apaga` con el servicio parado
//! ([`quita_el_bloqueo`]), el arranque del servicio cuando ya no se quiere y la desinstalación.
//!
//! De la conexión no se guarda nada más que eso: ni llaves ni a dónde se va; el estado vive en la memoria del servicio.
// Handlers answer axum's own `Response` on error, as every module of the panel does (see `api.rs`).
#![allow(clippy::result_large_err)]

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, PoisonError};
#[cfg(windows)]
use std::time::Duration;

use axum::extract::State;
use axum::Json;
use guardiana_tunel::estricto::{self, Deseo};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::ApiResult;
use crate::auth::{Lang, Session};
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
    /// Modo estricto: el servidor no contesta y el túnel se cerró; el bloqueo sigue y se vuelve a intentar.
    Reconectando,
    Error,
}

impl Fase {
    fn como_texto(self) -> &'static str {
        match self {
            Self::Apagada => "apagada",
            Self::Conectando => "conectando",
            Self::Conectada => "conectada",
            Self::SinRespuesta => "sin_respuesta",
            Self::Reconectando => "reconectando",
            Self::Error => "error",
        }
    }

    /// La VPN está encendida: conectando, conectada, o sin su servidor.
    fn encendida(self) -> bool {
        matches!(
            self,
            Self::Conectando | Self::Conectada | Self::SinRespuesta | Self::Reconectando
        )
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
    /// Si el bloqueo del modo estricto está en Windows, como lo puso, lo encontró o lo quitó este proceso.
    bloqueado: AtomicBool,
    /// Un cambio a la vez del bloqueo del modo estricto o de lo guardado, con la generación mirada dentro: quien apaga
    /// sube la generación con este candado puesto, así que una conexión a medias nunca vuelve a poner el bloqueo después
    /// de que la persona apagara la VPN.
    cambio: Mutex<()>,
    #[cfg(windows)]
    conexion: Mutex<Option<guardiana_apps::tunel_win::Conexion>>,
}

/// La VPN del proceso: la misma para todas las vueltas del motor y todos los paneles que levante.
pub(crate) fn global() -> &'static Vpn {
    static VPN: LazyLock<Vpn> = LazyLock::new(Vpn::default);
    &VPN
}

/// La carpeta de datos, donde vive lo que la persona quiere de la VPN.
fn datos() -> std::path::PathBuf {
    guardiana_core::paths::data_dir()
}

/// Lo que haya del modo estricto en Windows, fuera. Fuera de Windows no hay nada que quitar.
fn quita_de_windows() -> Result<(), String> {
    #[cfg(windows)]
    {
        guardiana_apps::corte_vpn::quita_estricto()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        Ok(())
    }
}

/// Al parar el servicio para siempre: el túnel se cierra (y su corte con él) y la VPN vuelve a «apagada». Una conexión
/// que aún esté a medias se cierra sola al terminar ([`guarda_si_sigue`]). En el modo estricto, el bloqueo y lo que la
/// persona quiere se quedan: una parada (una actualización, un reinicio) no es apagar la VPN, y el servicio la levanta
/// otra vez al arrancar ([`vpn_al_arrancar`]).
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

/// Al arrancar el servicio (el modo estricto, docs/VPN.md): lo que quede del modo estricto en Windows y ya nadie quiera
/// se quita (el modo estricto apagado, o la VPN apagada), y la VPN que debe seguir encendida en el modo estricto vuelve,
/// con su bloqueo puesto mientras tanto. Fuera de Windows, nada.
pub fn vpn_al_arrancar() {
    #[cfg(windows)]
    {
        let vpn = global();
        let d = vpn.deseo();
        // si Windows no se deja preguntar, como si quedara algo: quitar lo que no está no cuesta nada
        let quedan = match guardiana_apps::corte_vpn::en_windows() {
            Ok(en) => en.alguno(),
            Err(_) => true,
        };
        match estricto::al_arrancar(&d, quedan) {
            estricto::AlArrancar::Nada => {}
            estricto::AlArrancar::Quita => {
                let _uno = vpn.cambio.lock().unwrap_or_else(PoisonError::into_inner);
                if quita_de_windows().is_ok() {
                    vpn.bloqueado.store(false, Ordering::SeqCst);
                }
                if d.encendida {
                    let _ = vpn.guarda_deseo(&d.apagada());
                }
            }
            estricto::AlArrancar::Vuelve => {
                let Some(alta) = d.alta else {
                    return;
                };
                let gen = vpn.generacion.fetch_add(1, Ordering::SeqCst) + 1;
                // el bloqueo otra vez, en el acto: Windows guardó el de antes, y este lleva el archivo del programa como
                // está ahora (tras una actualización también)
                vpn.bloqueado.store(quedan, Ordering::SeqCst);
                let _ = vpn.pon_bloqueo(gen, d.servidor_del_bloqueo());
                vpn.pon(Estado {
                    fase: Fase::Conectando,
                    servidor: d.servidor.clone(),
                    ..Estado::default()
                });
                let servidor = d.servidor;
                std::thread::spawn(move || vpn.bucle_estricto(&servidor, alta, gen));
            }
        }
    }
}

/// Lo que quitó [`quita_el_bloqueo`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BloqueoQuitado {
    /// Los filtros del modo estricto que se quitaron de Windows.
    pub filtros: usize,
    /// Si su subcapa tuvo que quedarse, porque el servicio aún tenía dentro el permiso de su túnel: no bloquea nada por
    /// sí sola, y se va con él.
    pub en_uso: bool,
}

/// Por qué [`quita_el_bloqueo`] no pudo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoSeQuito {
    /// Lo que dijo el sistema.
    pub motivo: String,
    /// Si fue por no tener permiso de administrador.
    pub sin_permiso: bool,
}

/// `guardiana vpn apaga`, la salida del modo estricto con el servicio en marcha o parado (docs/VPN.md): la VPN apagada en
/// lo que se guarda (con `olvida`, al desinstalar, el archivo fuera), y todo lo del modo estricto fuera de Windows.
/// Fuera de Windows, solo lo guardado.
///
/// # Errors
/// [`NoSeQuito`] con lo que dijo el sistema.
pub fn quita_el_bloqueo(olvida: bool) -> Result<BloqueoQuitado, NoSeQuito> {
    let carpeta = datos();
    let guardado = if olvida {
        estricto::olvida_en(&carpeta)
    } else {
        let d = estricto::lee_de(&carpeta);
        if d.encendida {
            estricto::guarda_en(&carpeta, &d.apagada())
        } else {
            Ok(())
        }
    };
    #[cfg(windows)]
    let quitado = guardiana_apps::corte_vpn::quita_estricto()
        .map(|q| BloqueoQuitado {
            filtros: q.filtros,
            en_uso: q.en_uso,
        })
        .map_err(|e| NoSeQuito {
            sin_permiso: guardiana_apps::corte_vpn::sin_permiso(&e),
            motivo: e.to_string(),
        })?;
    #[cfg(not(windows))]
    let quitado = BloqueoQuitado::default();
    guardado.map_err(|e| NoSeQuito {
        sin_permiso: e.kind() == std::io::ErrorKind::PermissionDenied,
        motivo: e.to_string(),
    })?;
    Ok(quitado)
}

/// Lo que dice `guardiana vpn estado`: el modo estricto, la VPN como está guardada y lo que Windows tiene del bloqueo.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct EstadoEstricto {
    /// El modo estricto, encendido.
    pub estricto: bool,
    /// La VPN, encendida en lo guardado.
    pub encendida: bool,
    /// Su servidor.
    pub servidor: String,
    /// Los filtros persistentes del modo estricto que Windows tiene ahora.
    pub persistentes: usize,
    /// Sus filtros de arranque.
    pub arranque: usize,
    /// Si Windows tiene su proveedor o su subcapa.
    pub objetos: bool,
    /// Por qué no se pudo preguntar a Windows, si no se pudo.
    pub error: Option<String>,
}

/// El estado del modo estricto, preguntado a lo guardado y a Windows (fuera de Windows, solo a lo guardado).
#[must_use]
pub fn estado_del_modo_estricto() -> EstadoEstricto {
    let d = estricto::lee_de(&datos());
    #[cfg(windows)]
    let (persistentes, arranque, objetos, error) = match guardiana_apps::corte_vpn::en_windows() {
        Ok(en) => (
            en.persistentes,
            en.arranque,
            en.proveedor || en.subcapa,
            None,
        ),
        Err(no) => (0, 0, false, Some(no.to_string())),
    };
    #[cfg(not(windows))]
    let (persistentes, arranque, objetos, error) = (0, 0, false, None);
    EstadoEstricto {
        estricto: d.estricto,
        encendida: d.encendida,
        servidor: d.servidor,
        persistentes,
        arranque,
        objetos,
        error,
    }
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

    /// Lo que la persona quiere de la VPN, como está guardado: se lee cada vez, porque `guardiana vpn apaga` puede
    /// cambiarlo desde fuera del servicio.
    fn deseo(&self) -> Deseo {
        estricto::lee_de(&datos())
    }

    fn guarda_deseo(&self, d: &Deseo) -> Result<(), String> {
        estricto::guarda_en(&datos(), d).map_err(|e| e.to_string())
    }

    /// El estado para la página, con lo que el controlador dice del servidor cuando está conectada, y el modo estricto:
    /// si está encendido, si su bloqueo está puesto y, mientras el servidor no contesta con el bloqueo puesto, el aviso.
    fn foto(&self, t: &guardiana_core::i18n::Texts) -> Value {
        let e = self.estado();
        let d = self.deseo();
        let bloqueado = self.bloqueado.load(Ordering::SeqCst);
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
        let no_conecta = bloqueado && matches!(e.fase, Fase::Reconectando | Fase::SinRespuesta);
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
            "estricto": d.estricto,
            "bloqueado": bloqueado,
            "aviso": if no_conecta { t.panel("vpn_estricto_no_conecta") } else { "" },
            "estricto_titulo": t.panel("vpn_estricto"),
            "estricto_linea": t.panel("vpn_estricto_linea"),
        })
    }

    /// «Desconectar», en cualquier fase (la salida del modo estricto): la VPN apagada en lo guardado, el túnel cerrado y
    /// el bloqueo del modo estricto fuera de Windows. Lo guardado va primero: si lo demás falla, el próximo arranque del
    /// servicio quita lo que quede.
    fn apaga(&self) -> Result<(), String> {
        let _uno = self.cambio.lock().unwrap_or_else(PoisonError::into_inner);
        // el vigía y el bucle de reconexión se acaban, y la conexión que esté a medias ya no se guarda ni pone nada
        self.generacion.fetch_add(1, Ordering::SeqCst);
        let d = self.deseo();
        let guardado = if d.encendida {
            self.guarda_deseo(&d.apagada())
        } else {
            Ok(())
        };
        #[cfg(windows)]
        {
            let abierta = self
                .conexion
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take();
            if let Some(c) = abierta {
                c.tunel.cierra();
            }
        }
        let quitado = quita_de_windows();
        if quitado.is_ok() {
            self.bloqueado.store(false, Ordering::SeqCst);
        }
        self.pon(Estado::default());
        quitado.and(guardado)
    }
}

#[cfg(windows)]
impl Vpn {
    /// La conexión entera, fuera del hilo de la página (tarda unos segundos), de la generación `gen`, con el modo que
    /// la persona tenga: el estricto o el de siempre.
    fn haz_conexion(&'static self, host: &str, gen: u64) {
        if self.deseo().estricto {
            self.conecta_estricto(host, gen);
            return;
        }
        // otra conexión de antes (otro servidor): se cierra primero
        let vieja = {
            let mut g = self.conexion.lock().unwrap_or_else(PoisonError::into_inner);
            if self.generacion.load(Ordering::SeqCst) != gen {
                return;
            }
            g.take()
        };
        if let Some(v) = vieja {
            v.tunel.cierra();
        }
        if self.haz_conexion_normal(host, gen) {
            // el vigía, en un hilo suyo: no retiene el cierre del programa
            std::thread::spawn(move || self.vigila(gen));
        }
    }

    /// La conexión de siempre, de la generación `gen`. Dice si quedó abierta: no, si mientras conectaba se cerró la VPN
    /// o empezó otra; entonces se cierra en el acto ([`guarda_si_sigue`]).
    fn haz_conexion_normal(&self, host: &str, gen: u64) -> bool {
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

    /// El modo estricto, de la generación `gen`, con el servidor `host`: su dirección (el nombre resuelto ahora; con el
    /// bloqueo puesto solo resuelve por un túnel que siga abierto, y si no, la que ya se sabía); la conexión de antes
    /// (otro servidor) se cierra con el bloqueo puesto, y empieza el bucle.
    fn conecta_estricto(&'static self, host: &str, gen: u64) {
        let d = self.deseo();
        let alta = guardiana_tunel::conexion::resuelve(host, 443)
            .ok()
            .or_else(|| d.sin_resolver(host, 443));
        let (host, alta) = match alta {
            Some(a) => (host.to_owned(), a),
            // ni resuelve ni se sabía (otro servidor, con el bloqueo puesto y sin túnel): se sigue con el de antes
            None if d.quiere_bloqueo() && self.bloqueado.load(Ordering::SeqCst) => match d.alta {
                Some(a) => (d.servidor.clone(), a),
                None => return,
            },
            None => {
                let _guardada = self.conexion.lock().unwrap_or_else(PoisonError::into_inner);
                if self.generacion.load(Ordering::SeqCst) == gen {
                    self.pon(Estado {
                        fase: Fase::Error,
                        servidor: host.to_owned(),
                        error: guardiana_tunel::conexion::ErrorConexion::Nombre(host.to_owned())
                            .to_string(),
                        ..Estado::default()
                    });
                }
                return;
            }
        };
        {
            let _uno = self.cambio.lock().unwrap_or_else(PoisonError::into_inner);
            if self.generacion.load(Ordering::SeqCst) != gen {
                return;
            }
            let _ = self.guarda_deseo(&self.deseo().encendida_con(&host, alta));
            // con el bloqueo ya puesto (otro servidor, o el de antes de un cierre), va otra vez con este servidor
            if self.bloqueado.load(Ordering::SeqCst) {
                let _ = self.pon_bloqueo_dentro(self.deseo().servidor_del_bloqueo());
            }
        }
        // la conexión de antes, si la había: se cierra; el bloqueo sigue
        let vieja = {
            let mut g = self.conexion.lock().unwrap_or_else(PoisonError::into_inner);
            if self.generacion.load(Ordering::SeqCst) != gen {
                return;
            }
            g.take()
        };
        if let Some(v) = vieja {
            v.tunel.cierra();
        }
        self.bucle_estricto(&host, alta, gen);
    }

    /// El bloqueo del modo estricto para `servidor`, si `gen` sigue siendo la generación en vigor.
    fn pon_bloqueo(
        &self,
        gen: u64,
        servidor: guardiana_tunel::corte::Servidor,
    ) -> Result<(), String> {
        let _uno = self.cambio.lock().unwrap_or_else(PoisonError::into_inner);
        if self.generacion.load(Ordering::SeqCst) != gen {
            return Err("la VPN se apagó mientras conectaba".to_owned());
        }
        self.pon_bloqueo_dentro(servidor)
    }

    /// El bloqueo del modo estricto para `servidor`, con el candado de los cambios ya puesto: el programa de este
    /// servicio es el que puede ir al servidor.
    fn pon_bloqueo_dentro(&self, servidor: guardiana_tunel::corte::Servidor) -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        guardiana_apps::corte_vpn::pon_estricto(&guardiana_tunel::corte::estricto(servidor), &exe)
            .map_err(|e| e.to_string())?;
        self.bloqueado.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Lo que dio un alta (el extremo del túnel) se guarda, y el bloqueo va con él, si `gen` sigue en vigor y la VPN
    /// sigue encendida en lo guardado (`guardiana vpn apaga` la apaga desde fuera del servicio).
    fn bloquea_con(
        &self,
        gen: u64,
        punto: &str,
        extremo: std::net::SocketAddr,
    ) -> Result<(), String> {
        let _uno = self.cambio.lock().unwrap_or_else(PoisonError::into_inner);
        let d = self.deseo();
        if self.generacion.load(Ordering::SeqCst) != gen || !d.encendida {
            return Err("la VPN se apagó mientras conectaba".to_owned());
        }
        let d = d.con_extremo(punto, extremo);
        let _ = self.guarda_deseo(&d);
        self.pon_bloqueo_dentro(d.servidor_del_bloqueo())
    }

    /// El bucle del modo estricto, de la generación `gen`, con el servidor `host` en `alta`: intento tras intento hasta
    /// que conecta, se apaga la VPN o empieza otra. Cada vez que el servidor no contesta, el túnel se cierra y el
    /// bloqueo sigue: no sale nada, la página dice «reconectando» con el aviso, y se espera un poco más
    /// (`guardiana_tunel::estricto::espera_tras`).
    fn bucle_estricto(&'static self, host: &str, alta: std::net::SocketAddr, gen: u64) {
        let mut fallos = 0u32;
        loop {
            if self.generacion.load(Ordering::SeqCst) != gen {
                return;
            }
            if !self.deseo().encendida {
                // `guardiana vpn apaga` la apagó desde fuera del servicio
                let _ = self.apaga();
                return;
            }
            let resultado = std::env::current_exe()
                .map_err(|e| guardiana_tunel::conexion::ErrorConexion::Tunel(e.to_string()))
                .and_then(|exe| {
                    let dll = exe
                        .parent()
                        .map(|p| p.join("wireguard.dll"))
                        .unwrap_or_default();
                    guardiana_apps::tunel_win::intento_estricto(
                        host,
                        alta,
                        |nombre, puerto| self.deseo().sin_resolver(nombre, puerto),
                        &dll,
                        &exe,
                        |a, extremo| self.bloquea_con(gen, &a.host, extremo),
                    )
                });
            match resultado {
                Ok(c) => {
                    match guarda_si_sigue(&self.generacion, gen, &self.conexion, c, |c| {
                        self.pon(conectada(c));
                    }) {
                        None => {
                            std::thread::spawn(move || self.vigila(gen));
                        }
                        // la VPN se apagó, o empezó otra, mientras conectaba
                        Some(tarde) => tarde.tunel.cierra(),
                    }
                    return;
                }
                Err(e) => {
                    if self.generacion.load(Ordering::SeqCst) != gen {
                        return;
                    }
                    // el primer alta no contestó y el bloqueo aún no estaba: va ahora, con lo que se sabe del servidor;
                    // desde aquí no sale nada
                    if !self.bloqueado.load(Ordering::SeqCst) {
                        if let Err(no) = self.pon_bloqueo(gen, self.deseo().servidor_del_bloqueo())
                        {
                            if self.generacion.load(Ordering::SeqCst) != gen {
                                return;
                            }
                            // Windows no deja poner el bloqueo: el modo estricto no puede sostenerse; se dice, y la VPN
                            // queda apagada
                            let _ = self.apaga();
                            self.pon(Estado {
                                fase: Fase::Error,
                                servidor: host.to_owned(),
                                error: no,
                                ..Estado::default()
                            });
                            return;
                        }
                    }
                    fallos = fallos.saturating_add(1);
                    {
                        let _guardada =
                            self.conexion.lock().unwrap_or_else(PoisonError::into_inner);
                        if self.generacion.load(Ordering::SeqCst) != gen {
                            return;
                        }
                        self.pon(Estado {
                            fase: Fase::Reconectando,
                            servidor: host.to_owned(),
                            // «el servidor no contesta» ya lo dice el aviso; su texto de siempre diría que el equipo
                            // quedó como estaba, y en el modo estricto no es así
                            error: if e == guardiana_tunel::conexion::ErrorConexion::NoContesta {
                                String::new()
                            } else {
                                e.to_string()
                            },
                            ..Estado::default()
                        });
                    }
                    if !self.espera(gen, estricto::espera_tras(fallos)) {
                        return;
                    }
                }
            }
        }
    }

    /// Espera `cuanto`; `false` si mientras tanto se apagó la VPN o empezó otra.
    fn espera(&self, gen: u64, cuanto: Duration) -> bool {
        let hasta = std::time::Instant::now() + cuanto;
        while std::time::Instant::now() < hasta {
            if self.generacion.load(Ordering::SeqCst) != gen {
                return false;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        self.generacion.load(Ordering::SeqCst) == gen
    }

    /// El vigía de la conexión de la generación `gen`, en su propio hilo, hasta que se desconecte (o empiece otra): si el
    /// servidor deja de contestar, «sin respuesta»; si vuelve, «conectada». En el modo estricto, si deja de contestar, el
    /// túnel se rehace con un alta nueva, con el bloqueo puesto mientras tanto. Tras un reposo del equipo, el servidor
    /// tiene su margen entero otra vez antes de darlo por caído.
    fn vigila(&'static self, gen: u64) {
        use guardiana_tunel::conexion;
        let cada = u64::try_from(VISTAZO.as_millis()).unwrap_or(u64::MAX);
        let mut antes = ahora_ms();
        let mut despierto = 0u64;
        loop {
            std::thread::sleep(VISTAZO);
            if self.generacion.load(Ordering::SeqCst) != gen {
                return;
            }
            let ahora = ahora_ms();
            despierto = estricto::desde_tras_vistazo(despierto, antes, ahora, cada);
            antes = ahora;
            let Ok(g) = self.conexion.lock() else {
                return;
            };
            // desconectada: se acabó
            let Some(c) = g.as_ref() else {
                return;
            };
            let saludo = c.tunel.estado().and_then(|e| e.ultimo_saludo_ms);
            let callado = conexion::caida(ahora, c.desde_ms.max(despierto), saludo, None);
            if callado && c.tunel.es_estricto() {
                drop(g);
                let vieja = {
                    let mut g = self.conexion.lock().unwrap_or_else(PoisonError::into_inner);
                    if self.generacion.load(Ordering::SeqCst) != gen {
                        return;
                    }
                    g.take()
                };
                if let Some(v) = vieja {
                    v.tunel.cierra();
                }
                let d = self.deseo();
                if let (true, Some(alta)) = (d.encendida, d.alta) {
                    self.pon(Estado {
                        fase: Fase::Reconectando,
                        servidor: d.servidor.clone(),
                        ..Estado::default()
                    });
                    self.bucle_estricto(&d.servidor, alta, gen);
                } else {
                    let _ = self.apaga();
                }
                return;
            }
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

    /// El modo estricto encendido o apagado (`activo`). Con un túnel abierto, cambia en el acto y sin un momento sin
    /// corte: al encenderlo, primero el bloqueo y el permiso del túnel en él, y luego se va el corte de siempre; al
    /// apagarlo, primero el corte de siempre y luego se va el bloqueo. Con el servidor sin contestar (el túnel cerrado,
    /// el bloqueo puesto), apagarlo es volver a la conexión normal, como haría la VPN sin él. Lo que se guarda, siempre.
    fn cambia_estricto(&self, activo: bool) -> Result<bool, String> {
        let _uno = self.cambio.lock().unwrap_or_else(PoisonError::into_inner);
        let d = self.deseo();
        if d.estricto == activo {
            return Ok(activo);
        }
        if self.estado().fase == Fase::Conectando {
            return Err("está conectando: espera a que termine".to_owned());
        }
        let mut g = self.conexion.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(c) = g.as_mut() {
            if activo {
                let alta = std::net::SocketAddr::new(c.extremo.ip(), 443);
                let nuevo = Deseo {
                    estricto: true,
                    ..d
                }
                .encendida_con(&c.servidor, alta)
                .con_extremo(&c.servidor, c.extremo);
                self.pon_bloqueo_dentro(nuevo.servidor_del_bloqueo())?;
                if let Err(e) = c.tunel.a_estricto() {
                    // el túnel sigue con su corte de siempre; el bloqueo, aparte, se va
                    if quita_de_windows().is_ok() {
                        self.bloqueado.store(false, Ordering::SeqCst);
                    }
                    return Err(e.to_string());
                }
                self.guarda_deseo(&nuevo)?;
                return Ok(true);
            }
            c.tunel.a_normal().map_err(|e| e.to_string())?;
            drop(g);
            let quitado = quita_de_windows();
            if quitado.is_ok() {
                self.bloqueado.store(false, Ordering::SeqCst);
            }
            // sin modo estricto la VPN acaba con el servicio, como siempre: no se guarda encendida
            self.guarda_deseo(
                &Deseo {
                    estricto: false,
                    ..d
                }
                .apagada(),
            )?;
            return quitado.map(|()| false);
        }
        drop(g);
        if !activo && self.bloqueado.load(Ordering::SeqCst) {
            // sin túnel y con el bloqueo puesto (el servidor no contesta): sin modo estricto, el equipo vuelve a su
            // conexión normal, y la VPN queda apagada
            self.generacion.fetch_add(1, Ordering::SeqCst);
            let quitado = quita_de_windows();
            if quitado.is_ok() {
                self.bloqueado.store(false, Ordering::SeqCst);
            }
            self.guarda_deseo(
                &Deseo {
                    estricto: false,
                    ..d.clone()
                }
                .apagada(),
            )?;
            self.pon(Estado {
                fase: Fase::Error,
                servidor: d.servidor,
                error: guardiana_tunel::conexion::ErrorConexion::NoContesta.to_string(),
                ..Estado::default()
            });
            return quitado.map(|()| false);
        }
        self.guarda_deseo(&Deseo {
            estricto: activo,
            ..d
        })?;
        Ok(activo)
    }
}

/// La hora, en milisegundos desde 1970.
#[cfg(windows)]
fn ahora_ms() -> u64 {
    u64::try_from(guardiana_core::time::now_ms()).unwrap_or(0)
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

/// `GET /api/vpn`: en qué está la VPN, y el modo estricto.
pub(crate) async fn ver(
    State(state): State<Arc<AppState>>,
    Lang(t): Lang,
    _s: Session,
) -> ApiResult<Value> {
    Ok(Json(state.vpn.foto(t)))
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
/// `GET /api/vpn` cómo va. Con la VPN ya encendida, otro servidor la cambia de servidor (en el modo estricto, con el
/// bloqueo puesto todo el rato); el mismo, nada.
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
        if e.fase.encendida() && (e.fase == Fase::Conectando || e.servidor == host) {
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
        tokio::task::spawn_blocking(move || vpn.haz_conexion(&host, gen));
    }
    #[cfg(not(windows))]
    let _ = gen;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/vpn/desconecta`: la VPN apagada, en cualquier fase (también conectando o reconectando): el túnel se
/// cierra y el bloqueo del modo estricto sale de Windows. Es la salida para volver a la conexión normal.
pub(crate) async fn desconecta(
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<Value> {
    // fuera de los hilos de la página
    let vpn = state.vpn;
    let r = tokio::task::spawn_blocking(move || vpn.apaga()).await;
    Ok(Json(match r {
        Ok(Ok(())) => json!({ "ok": true }),
        Ok(Err(motivo)) => json!({ "ok": false, "motivo": motivo }),
        Err(_) => json!({ "ok": false, "motivo": "estado no disponible" }),
    }))
}

/// Lo que pide el interruptor del modo estricto.
#[derive(Debug, Default, Deserialize)]
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) struct PideEstricto {
    /// Encendido o apagado. Sin él (o sin cuerpo, como pide la web de GUARDIANA a través de ZERO), lo contrario de lo que
    /// está: el interruptor.
    #[serde(default)]
    activo: Option<bool>,
}

/// `POST /api/vpn/estricto` `{"activo": true|false}` (o `{}`: lo contrario de lo que está): el modo estricto. La
/// respuesta dice cómo quedó (`"estricto"`).
pub(crate) async fn modo_estricto(
    State(state): State<Arc<AppState>>,
    _s: Session,
    cuerpo: axum::body::Bytes,
) -> ApiResult<Value> {
    let pedido = if cuerpo.iter().all(u8::is_ascii_whitespace) {
        PideEstricto::default()
    } else {
        match serde_json::from_slice::<PideEstricto>(&cuerpo) {
            Ok(p) => p,
            Err(_) => {
                return Ok(Json(
                    json!({ "ok": false, "motivo": "el pedido no se entiende" }),
                ))
            }
        }
    };
    #[cfg(not(windows))]
    {
        let _ = (state, pedido);
        Ok(Json(json!({ "ok": false, "motivo": no_disponible() })))
    }
    #[cfg(windows)]
    {
        let vpn = state.vpn;
        let r = tokio::task::spawn_blocking(move || {
            let activo = pedido.activo.unwrap_or(!vpn.deseo().estricto);
            vpn.cambia_estricto(activo)
        })
        .await;
        Ok(Json(match r {
            Ok(Ok(activo)) => json!({ "ok": true, "estricto": activo }),
            Ok(Err(motivo)) => {
                json!({ "ok": false, "motivo": motivo, "estricto": vpn.deseo().estricto })
            }
            Err(_) => json!({ "ok": false, "motivo": "estado no disponible" }),
        }))
    }
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

    /// The strict provider names GUARDIANA's service: Windows loads its persistent filters at start only while that
    /// service exists and starts with the computer.
    #[test]
    fn el_proveedor_del_modo_estricto_nombra_el_servicio() {
        assert_eq!(estricto::SERVICIO, guardiana_service::daemon::SERVICE_NAME);
    }

    #[test]
    fn el_estado_para_la_pagina() {
        let es = guardiana_core::i18n::es();
        let v = Vpn::default();
        let f = v.foto(es);
        assert_eq!(f["fase"], "apagada");
        assert_eq!(f["enviados"], Value::Null);
        // el modo estricto: su interruptor, con su línea, y sin aviso mientras no hay bloqueo
        assert!(f["estricto"].is_boolean());
        assert_eq!(f["bloqueado"], false);
        assert_eq!(f["aviso"], "");
        assert_eq!(f["estricto_titulo"], "Modo estricto");
        assert!(f["estricto_linea"]
            .as_str()
            .is_some_and(|l| l.len() > 40 && !l.contains("vpn_")));
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
        let f = v.foto(es);
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
        let f = v.foto(es);
        assert_eq!(f["fase"], "sin_respuesta");
        assert_eq!(f["direccion"], "10.66.0.7");
        assert_eq!(f["aviso"], "");
        // el modo estricto con el servidor callado: el bloqueo sigue, y la página lo dice con el aviso
        v.bloqueado.store(true, Ordering::SeqCst);
        v.pon(Estado {
            fase: Fase::Reconectando,
            servidor: "vpn.guardianagroup.com".into(),
            ..Estado::default()
        });
        let f = v.foto(es);
        assert_eq!(f["fase"], "reconectando");
        assert_eq!(f["bloqueado"], true);
        assert_eq!(
            f["aviso"],
            "La VPN no conecta: no sale nada hasta que vuelva o la apagues tú"
        );
        // en inglés, el mismo aviso en su idioma
        let f = v.foto(guardiana_core::i18n::en());
        assert!(f["aviso"]
            .as_str()
            .is_some_and(|a| a.starts_with("The VPN")));
    }

    /// The VPN is on in every phase where «Desconectar» shows, «reconectando» among them (strict mode).
    #[test]
    fn las_fases_de_la_vpn_encendida() {
        for (f, encendida) in [
            (Fase::Apagada, false),
            (Fase::Conectando, true),
            (Fase::Conectada, true),
            (Fase::SinRespuesta, true),
            (Fase::Reconectando, true),
            (Fase::Error, false),
        ] {
            assert_eq!(f.encendida(), encendida, "{f:?}");
        }
        assert_eq!(Fase::Reconectando.como_texto(), "reconectando");
    }
}
