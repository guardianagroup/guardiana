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
//! túnel se cierra y el bloqueo sigue: no sale nada de los programas fuera del túnel, la página lo dice («reconectando»,
//! con el aviso) y se vuelve a intentar, hasta que conecte o la persona la apague. Las salidas, siempre: «Desconectar»
//! (el panel, Órbita y la barra de ZERO, `POST /api/vpn/desconecta`, en cualquier fase), `guardiana vpn apaga` con el
//! servicio parado ([`quita_el_bloqueo`]), el arranque del servicio cuando ya no se quiere y la desinstalación.
//!
//! La revisión de la VPN de la 1.0.56 (5 nov 2026): cambiar de salida con el túnel caído va a la dirección que ya se sabe
//! de esa salida (lo guardado o lo medido con la VPN apagada) y, sin ninguna, se dice en vez de seguir en silencio con la
//! de antes; la reconciliación del arranque solo la hace el servicio y se repite cada minuto mientras no salga bien
//! ([`comprueba_la_vpn`]); el vigía vuelve a leer lo guardado; y el estado dice cuándo hace falta «Desconectar»
//! (`"desconectar"`), si Windows no aceptó los filtros de arranque del bloqueo (`"aviso_arranque"`) y qué se midió de
//! verdad de lo que una web verá (`"comprobado"`).
//!
//! Las salidas (5 nov 2026; el dueño, 3 nov, 09:25: «me ha conectado, pero no sé a dónde… Todas las VPN te dicen a
//! dónde… Haz una VPN de verdad y de calidad»; docs/VPN.md, «La API»): cada servidor de GUARDIANA es una salida real en
//! su país (`guardiana_tunel::salidas::SALIDAS`). `GET /api/vpn/salidas` las da con lo medido desde este equipo (si
//! están, a cuánto, y su dirección) y la más rápida; se miden solo con la VPN apagada y sin el bloqueo (puesta, la medida
//! iría por el túnel), y lo medido vale un minuto. `POST /api/vpn/conecta` `{"salida": "<id>" | "rapida"}` conecta a
//! una. `GET /api/vpn/comprueba` dice lo que una web verá: la dirección (la que el eco de otro servidor de GUARDIANA ve
//! llegar, nunca un tercero), el DNS, el IPv6, la hora del equipo frente a la de la salida y el permiso de ubicación de
//! Windows.
//!
//! De la conexión no se guarda nada más que eso: ni llaves ni a dónde se va; el estado vive en la memoria del servicio.
// Handlers answer axum's own `Response` on error, as every module of the panel does (see `api.rs`).
#![allow(clippy::result_large_err)]

use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, PoisonError};
#[cfg(windows)]
use std::time::Duration;

use axum::extract::State;
use axum::Json;
use guardiana_tunel::estricto::{self, Deseo};
use guardiana_tunel::salidas::{self, Medida, Salida, SALIDAS};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::ApiResult;
use crate::auth::{Lang, Session, Zona};
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

/// Cuánto vale lo medido de las salidas antes de medirlas otra vez (con la VPN apagada).
const MEDIDAS_VALEN_MS: u64 = 60_000;

/// Cuánto vale lo que `GET /api/vpn/comprueba` preguntó fuera (el eco de una salida, por HTTPS, y el permiso de
/// ubicación de Windows, con `reg.exe`) para el mismo estado de la VPN: unos segundos, para no repetirlo en cada llamada
/// (revisión de la 1.0.56).
const COMPROBADO_VALE_MS: u64 = 15_000;

/// Lo que se dice a quien pide cambiar a una salida cuya dirección no se sabe, con el bloqueo del modo estricto puesto y
/// sin túnel (revisión de la 1.0.56: hasta entonces se seguía en silencio con la salida de antes).
const SIN_DIRECCION: &str = "no se sabe la dirección de esa salida y, con el bloqueo del modo estricto puesto y sin \
                             túnel, no se puede buscar: espera a que la VPN vuelva a conectar, o desconecta y elígela \
                             después";

/// Lo que se sabe de las salidas desde este equipo: lo medido con la VPN apagada o, sin poder medir, solo sus
/// direcciones.
#[derive(Debug, Clone, Default)]
struct Medidas {
    /// Cuándo, en milisegundos desde 1970.
    cuando_ms: u64,
    /// La medida de cada salida, por su id, en el orden de la tabla.
    de: Vec<(&'static str, Medida)>,
    /// Si se midió de verdad (con la VPN apagada y sin el bloqueo). Si no, de cada salida solo vale su dirección: ni si
    /// está ni a cuánto se sabe (revisión de la 1.0.56: hasta entonces, con la VPN puesta, una salida cuyo nombre
    /// resolvía salía como disponible).
    medido: bool,
}

impl Medidas {
    fn de(&self, id: &str) -> Option<&Medida> {
        self.de.iter().find(|(i, _)| *i == id).map(|(_, m)| m)
    }

    /// La salida disponible más rápida, por su nombre; sin medida de verdad, ninguna.
    fn mas_rapida(&self) -> Option<&'static Salida> {
        if !self.medido {
            return None;
        }
        salidas::mas_rapida(&self.de).and_then(salidas::por_id)
    }

    /// Lo mismo sin darlo por medido: de cada salida, solo su dirección.
    fn sin_medir(self) -> Self {
        Self {
            medido: false,
            ..self
        }
    }

    /// Las direcciones que se saben de las salidas, con su nombre.
    #[cfg_attr(not(windows), allow(dead_code))]
    fn direcciones(&self) -> impl Iterator<Item = (&'static str, IpAddr)> + '_ {
        self.de
            .iter()
            .filter_map(|(id, m)| Some((salidas::por_id(id)?.nombre, m.ip?)))
    }
}

/// Lo que quedó por hacer y [`comprueba_la_vpn`] intenta otra vez cada minuto (revisión de la 1.0.56).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pendiente {
    /// La reconciliación del arranque ([`vpn_al_arrancar`]): Windows no dejó quitar lo que sobraba, o falta
    /// `wireguard.dll` para levantar la VPN.
    Arranque,
    /// La VPN apagada en lo guardado: al apagarla no se pudo escribir ni quitar el archivo.
    Apagada,
}

/// Lo que `GET /api/vpn/comprueba` preguntó fuera, y para qué estado de la VPN.
#[derive(Debug, Clone)]
struct Preguntado {
    /// Cuándo, en milisegundos desde 1970.
    cuando_ms: u64,
    /// La generación, la fase, el servidor y si se podía salir fuera del túnel cuando se preguntó: con otros, ya no vale.
    para: (u64, Fase, String, bool),
    /// La dirección que vio llegar el eco de una salida, si alguno contestó.
    ip_vista: Option<IpAddr>,
    /// El permiso de ubicación de Windows.
    ubicacion: &'static str,
}

impl Preguntado {
    /// Si vale aún a las `ahora_ms` para el estado `para`.
    fn vale(&self, para: &(u64, Fase, String, bool), ahora_ms: u64) -> bool {
        self.para == *para && ahora_ms.saturating_sub(self.cuando_ms) < COMPROBADO_VALE_MS
    }
}

/// Lo que se lee de la conexión abierta para la página, sin esperar por ella.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(not(windows), allow(dead_code))]
struct Abierta {
    /// Lo que el controlador dice del servidor.
    par: Option<guardiana_tunel::nt::EstadoPar>,
    /// La dirección del servidor: la que las webs ven mientras es la salida.
    extremo: std::net::IpAddr,
    /// El resolutor de GUARDIANA manda solo al del túnel.
    dns_tunel: bool,
    /// El corte (o el bloqueo del modo estricto) está puesto.
    corte: bool,
}

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
    /// Lo último que se midió de las salidas (con la VPN apagada).
    medidas: Mutex<Option<Medidas>>,
    /// Una medida de las salidas a la vez.
    midiendo: Mutex<()>,
    /// Por qué Windows no aceptó los filtros de arranque del bloqueo puesto, si no los aceptó: el bloqueo vale entonces
    /// desde que arranca el servicio de filtrado de Windows, no desde el arranque del equipo. Se anota, y el estado lo
    /// dice (revisión de la 1.0.56).
    sin_arranque: Mutex<Option<String>>,
    /// Lo que quedó por hacer y se repite cada minuto ([`comprueba_la_vpn`]), con la generación de entonces: si la
    /// persona conecta o desconecta mientras tanto, la generación cambia y deja de hacer falta (eso pone y quita lo suyo).
    pendiente: Mutex<Option<(u64, Pendiente)>>,
    /// Lo último que `GET /api/vpn/comprueba` preguntó fuera.
    preguntado: Mutex<Option<Preguntado>>,
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

/// Al arrancar el servicio (el modo estricto, docs/VPN.md), y solo desde él (no desde `guardiana observe`): lo que quede
/// del modo estricto en Windows y ya nadie quiera se quita (el modo estricto apagado, o la VPN apagada), y la VPN que
/// debe seguir encendida en el modo estricto vuelve, con su bloqueo puesto mientras tanto. Si no sale bien (Windows no
/// deja quitar lo que sobra, o falta `wireguard.dll` para levantar la VPN), [`comprueba_la_vpn`] lo repite cada minuto
/// hasta que salga (revisión de la 1.0.56: hasta entonces se intentaba una sola vez). Fuera de Windows, nada.
pub fn vpn_al_arrancar() {
    let vpn = global();
    let gen = vpn.generacion.load(Ordering::SeqCst);
    if !vpn.reconcilia(gen) {
        vpn.queda_pendiente(gen, Pendiente::Arranque);
    }
}

/// La mirada de cada minuto del servicio a la VPN (revisión de la 1.0.56, como `comprueba_los_cortes` con los cortes del
/// antivirus): lo que quedó por hacer se intenta otra vez (la reconciliación del arranque, o dejar la VPN apagada en lo
/// guardado), hasta que salga bien o deje de hacer falta porque la persona conectó o desconectó entretanto. Sin nada
/// pendiente no hace nada, ni pregunta a Windows.
pub fn comprueba_la_vpn() {
    let vpn = global();
    let Some((gen, que)) = vpn.pendiente_ahora() else {
        return;
    };
    let hecho = match que {
        Pendiente::Arranque => vpn.reconcilia(gen),
        Pendiente::Apagada => vpn.guarda_apagada_otra_vez(gen),
    };
    if hecho {
        vpn.sin_pendiente(gen, que);
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
    /// De esos, persistentes y de arranque, los que están en las capas de reenvío de Windows (1.0.56: el bloqueo de lo
    /// que Windows reenvía para sus máquinas virtuales o una conexión compartida), según Windows, filtro a filtro.
    pub reenvio: usize,
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
    let (persistentes, arranque, reenvio, objetos, error) =
        match guardiana_apps::corte_vpn::en_windows() {
            Ok(en) => (
                en.persistentes,
                en.arranque,
                en.reenvio,
                en.proveedor || en.subcapa,
                None,
            ),
            Err(no) => (0, 0, 0, false, Some(no.to_string())),
        };
    #[cfg(not(windows))]
    let (persistentes, arranque, reenvio, objetos, error) = (0, 0, 0, false, None);
    EstadoEstricto {
        estricto: d.estricto,
        encendida: d.encendida,
        servidor: d.servidor,
        persistentes,
        arranque,
        reenvio,
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

    #[cfg_attr(not(windows), allow(dead_code))]
    fn guarda_deseo(&self, d: &Deseo) -> Result<(), String> {
        estricto::guarda_en(&datos(), d).map_err(|e| e.to_string())
    }

    /// El estado para la página, con lo que el controlador dice del servidor cuando está conectada, y el modo estricto:
    /// si está encendido, si su bloqueo está puesto y, mientras el servidor no contesta con el bloqueo puesto, el aviso.
    /// `"desconectar"`: si hace falta «Desconectar» (la VPN encendida, el bloqueo aún en Windows, también tras un error, o
    /// la VPN aún encendida en lo guardado); `"aviso_arranque"`: si Windows no aceptó los filtros de arranque del bloqueo
    /// (revisión de la 1.0.56).
    fn foto(&self, t: &guardiana_core::i18n::Texts) -> Value {
        self.foto_con(t, &self.deseo())
    }

    /// [`Self::foto`] con lo guardado `d` (las pruebas dan el suyo, sin leer la carpeta de datos del equipo).
    fn foto_con(&self, t: &guardiana_core::i18n::Texts, d: &Deseo) -> Value {
        let e = self.estado();
        let bloqueado = self.bloqueado.load(Ordering::SeqCst);
        // sin esperar: si la conexión está ocupada en ese momento, el estado sale sin bytes
        let con_tunel = matches!(e.fase, Fase::Conectada | Fase::SinRespuesta);
        let abierta = self.abierta().filter(|_| con_tunel);
        let par = abierta.and_then(|a| a.par);
        let motivo = no_disponible();
        let no_conecta = bloqueado && matches!(e.fase, Fase::Reconectando | Fase::SinRespuesta);
        let sin_arranque = bloqueado && self.sin_arranque.lock().is_ok_and(|g| g.is_some());
        // 5 nov 2026: la salida en la que está, o a la que iba: su país, su ciudad y su dirección (la del túnel abierto, la
        // medida o la que se sabe). Disponible solo si está conectada (el servidor contesta); si no, lo medido con la VPN
        // apagada, o no se sabe (revisión de la 1.0.56: decía que sí también reconectando o sin respuesta).
        let salida = (e.fase.encendida() || e.fase == Fase::Error)
            .then(|| salidas::por_nombre(&e.servidor))
            .flatten()
            .map(|s| {
                let m = self.medidas_guardadas().and_then(|m| m.de(s.id).copied());
                let ip = abierta
                    .map(|a| a.extremo)
                    .or(m.and_then(|m| m.ip))
                    .or_else(|| d.sin_resolver(s.nombre, 443).map(|a| a.ip()));
                let disponible = if e.fase == Fase::Conectada {
                    Some(true)
                } else {
                    m.map(|m| m.disponible)
                };
                salida_json(s, ip, disponible, m.and_then(|m| m.ms))
            });
        json!({
            "disponible": motivo.is_none(),
            "motivo": motivo,
            "fase": e.fase.como_texto(),
            "servidor": e.servidor,
            "salida": salida,
            "desde": (e.desde_ms > 0).then_some(e.desde_ms),
            "saludo": par.and_then(|p| p.ultimo_saludo_ms),
            "subidos": par.map(|p| p.enviados),
            "bajados": par.map(|p| p.recibidos),
            // el alta de GUARDIANA siempre lleva su mitad de ML-KEM (`guardiana_tunel::alta`): sin ella no hay túnel
            "postcuantica": con_tunel,
            "dns_tunel": abierta.is_some_and(|a| a.dns_tunel),
            "desde_ms": e.desde_ms,
            "direccion": e.direccion,
            "error": e.error,
            "enviados": par.map(|p| p.enviados),
            "recibidos": par.map(|p| p.recibidos),
            "ultimo_saludo_ms": par.and_then(|p| p.ultimo_saludo_ms),
            "estricto": d.estricto,
            "bloqueado": bloqueado,
            "aviso": if no_conecta { t.panel("vpn_estricto_no_conecta") } else { "" },
            "estricto_titulo": t.panel("vpn_estricto"),
            "estricto_linea": t.panel("vpn_estricto_linea"),
            "desconectar": e.fase.encendida() || bloqueado || d.quiere_bloqueo(),
            "aviso_arranque": if sin_arranque { t.panel("vpn_estricto_sin_arranque") } else { "" },
        })
    }

    /// «Desconectar», en cualquier fase (la salida del modo estricto): la VPN apagada en lo guardado, el túnel cerrado y
    /// el bloqueo del modo estricto fuera de Windows. Lo guardado va primero: si lo demás falla, el próximo arranque del
    /// servicio quita lo que quede. Si lo guardado no se puede escribir, [`Self::guarda_apagada`].
    fn apaga(&self) -> Result<(), String> {
        self.apaga_de(None)
    }

    /// [`Self::apaga`]; con `de`, la generación de quien la apaga desde dentro (el bucle de reconexión, al ver la VPN
    /// apagada en lo guardado): si ya no es la vigente, otro cambió algo entretanto (apagó el modo estricto, conectó o
    /// desconectó) y no se toca nada.
    fn apaga_de(&self, de: Option<u64>) -> Result<(), String> {
        let _uno = self.cambio.lock().unwrap_or_else(PoisonError::into_inner);
        if de.is_some_and(|g| g != self.generacion.load(Ordering::SeqCst)) {
            return Ok(());
        }
        // el vigía y el bucle de reconexión se acaban, y la conexión que esté a medias ya no se guarda ni pone nada
        let gen = self.generacion.fetch_add(1, Ordering::SeqCst) + 1;
        let d = self.deseo();
        let guardado = if d.encendida {
            self.guarda_apagada(&d.apagada(), gen)
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
            self.sin_bloqueo();
        }
        self.pon(Estado::default());
        quitado.and(guardado)
    }

    /// `nuevo`, la VPN apagada, en lo guardado, con la generación `gen` de quien la apaga. Si no se puede escribir, el
    /// archivo fuera, pero solo si sin él queda justo `nuevo` (sin archivo, la VPN está apagada y el modo estricto
    /// encendido; segunda revisión de la 1.0.56, «N3»: quitarlo cuando la persona acababa de apagar el modo estricto lo
    /// volvía a encender en lo guardado, y el estado decía lo que no era); y si tampoco, se dice y se vuelve a intentar
    /// cada minuto mientras el servicio siga ([`comprueba_la_vpn`]).
    ///
    /// Lo más seguro (revisión de la 1.0.56): es aquí, al apagar, donde se hace todo para que quede escrito, y no al
    /// arrancar. Un arranque que encuentra la VPN encendida en lo guardado la vuelve a levantar con su bloqueo, porque no
    /// puede distinguir un «Desconectar» que no se pudo guardar de un bloqueo que otro programa quitó de Windows o de un
    /// cierre a mitad de la primera conexión, y en esos casos dejarla apagada sería que los programas salieran fuera del
    /// túnel sin que la persona lo pidiera. Solo si ni se escribe ni se quita el archivo, y el servicio se reinicia antes
    /// de poder hacerlo, la VPN vuelve con su bloqueo, y la respuesta de este «Desconectar» ya lo ha dicho.
    fn guarda_apagada(&self, nuevo: &Deseo, gen: u64) -> Result<(), String> {
        self.guarda_apagada_en(&datos(), nuevo, gen)
    }

    /// [`Self::guarda_apagada`] en la carpeta de datos `carpeta` (las pruebas usan otra).
    fn guarda_apagada_en(
        &self,
        carpeta: &std::path::Path,
        nuevo: &Deseo,
        gen: u64,
    ) -> Result<(), String> {
        let Err(e) = estricto::guarda_en(carpeta, nuevo) else {
            return Ok(());
        };
        if *nuevo == Deseo::default() && estricto::olvida_en(carpeta).is_ok() {
            return Ok(());
        }
        self.queda_pendiente(gen, Pendiente::Apagada);
        eprintln!("guardiana: la VPN está apagada, pero no se pudo guardar: {e}");
        // lo que queda escrito solo la vuelve a encender al arrancar con el modo estricto encendido
        let antes = if nuevo.estricto {
            ", y si GUARDIANA se reinicia antes, la VPN volverá a encenderse con su bloqueo"
        } else {
            ""
        };
        Err(format!(
            "la VPN está apagada, pero no se pudo guardar ({e}): se vuelve a intentar cada minuto{antes}"
        ))
    }

    /// [`Self::guarda_apagada`] otra vez, desde [`comprueba_la_vpn`]: si la generación `gen` sigue en vigor y lo guardado
    /// aún dice que la VPN está encendida. Dice si ya no hace falta.
    fn guarda_apagada_otra_vez(&self, gen: u64) -> bool {
        self.guarda_apagada_otra_vez_en(&datos(), gen)
    }

    /// [`Self::guarda_apagada_otra_vez`] en la carpeta de datos `carpeta` (las pruebas usan otra).
    fn guarda_apagada_otra_vez_en(&self, carpeta: &std::path::Path, gen: u64) -> bool {
        let _uno = self.cambio.lock().unwrap_or_else(PoisonError::into_inner);
        if self.generacion.load(Ordering::SeqCst) != gen {
            return true;
        }
        let d = estricto::lee_de(carpeta);
        if !d.encendida {
            return true;
        }
        let apagada = d.apagada();
        estricto::guarda_en(carpeta, &apagada).is_ok()
            || (apagada == Deseo::default() && estricto::olvida_en(carpeta).is_ok())
    }

    /// El bloqueo del modo estricto ya no está en Windows.
    fn sin_bloqueo(&self) {
        self.bloqueado.store(false, Ordering::SeqCst);
        if let Ok(mut g) = self.sin_arranque.lock() {
            *g = None;
        }
    }

    /// Lo que quedó por hacer, si aún hace falta: con otra generación (la persona conectó o desconectó), ya no.
    fn pendiente_ahora(&self) -> Option<(u64, Pendiente)> {
        let mut p = self
            .pendiente
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        match *p {
            Some((gen, _)) if gen != self.generacion.load(Ordering::SeqCst) => {
                *p = None;
                None
            }
            otro => otro,
        }
    }

    /// `que` queda por hacer, para la generación `gen`.
    fn queda_pendiente(&self, gen: u64, que: Pendiente) {
        *self
            .pendiente
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some((gen, que));
    }

    /// `que`, de la generación `gen`, ya está hecho (si no lo reemplazó otra cosa entretanto).
    fn sin_pendiente(&self, gen: u64, que: Pendiente) {
        let mut p = self
            .pendiente
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if *p == Some((gen, que)) {
            *p = None;
        }
    }

    /// La reconciliación del arranque ([`vpn_al_arrancar`]) para la generación `gen`: si mientras tanto la persona
    /// conectó o desconectó, ya no hace falta y no se toca nada. Dice si quedó hecha.
    ///
    /// Lo guardado que ahora no se puede leer no cuenta como la VPN apagada (segunda revisión de la 1.0.56, con «N2»):
    /// ni se quita ni se pone nada, y se vuelve a mirar en un minuto ([`guardado_para_reconciliar`]). Hasta entonces se
    /// leía como apagada y se quitaba un bloqueo que la persona quería.
    fn reconcilia(&'static self, gen: u64) -> bool {
        #[cfg(windows)]
        {
            let Some(d) = guardado_para_reconciliar(estricto::lee_si_se_puede(&datos())) else {
                // lo que Windows tenga, para que «Desconectar» esté a mano mientras tanto (revisión final de la 1.0.56:
                // sin esto la página decía «apagada» con el bloqueo puesto y sin internet)
                let quedan = guardiana_apps::corte_vpn::en_windows().map_or(true, |en| en.alguno());
                self.bloqueado.store(quedan, Ordering::SeqCst);
                eprintln!(
                    "guardiana: lo guardado de la VPN no se puede leer ahora; se vuelve a mirar en un minuto"
                );
                return false;
            };
            // si Windows no se deja preguntar, como si quedara algo: quitar lo que no está no cuesta nada
            let quedan = match guardiana_apps::corte_vpn::en_windows() {
                Ok(en) => en.alguno(),
                Err(_) => true,
            };
            match estricto::al_arrancar(&d, quedan) {
                estricto::AlArrancar::Nada => true,
                estricto::AlArrancar::Quita => {
                    let _uno = self.cambio.lock().unwrap_or_else(PoisonError::into_inner);
                    if self.generacion.load(Ordering::SeqCst) != gen {
                        return true;
                    }
                    let quitado = quita_de_windows();
                    if quitado.is_ok() {
                        self.sin_bloqueo();
                    }
                    let guardado = if d.encendida {
                        self.guarda_deseo(&d.apagada())
                    } else {
                        Ok(())
                    };
                    if let Err(e) = quitado.as_ref().and(guardado.as_ref()) {
                        eprintln!(
                            "guardiana: lo que quedó del modo estricto no se pudo quitar al arrancar; se intenta otra vez cada minuto: {e}"
                        );
                    }
                    quitado.is_ok() && guardado.is_ok()
                }
                estricto::AlArrancar::Vuelve => self.vuelve(d, quedan, gen),
            }
        }
        #[cfg(not(windows))]
        {
            let _ = gen;
            true
        }
    }

    /// Si lo guardado dice que la VPN está apagada: `guardiana vpn apaga` pudo apagarla, y quitar su bloqueo, desde fuera
    /// del servicio. Un archivo que ahora no se puede leer no cuenta como apagada.
    #[cfg_attr(not(windows), allow(dead_code))]
    fn apagada_desde_fuera(&self) -> bool {
        estricto::lee_si_se_puede(&datos()).is_ok_and(|d| !d.encendida)
    }

    /// La dirección del alta de `host` sin preguntar a ningún resolutor: la que se sabe en lo guardado (su servidor, el
    /// extremo de la última alta o la última conocida de cada salida, `Deseo::sin_resolver`) o la medida con la VPN
    /// apagada.
    fn direccion_conocida(&self, host: &str, d: &Deseo) -> Option<SocketAddr> {
        d.sin_resolver(host, 443).or_else(|| {
            let s = salidas::por_nombre(host)?;
            let m = self.medidas_guardadas()?;
            m.de(s.id)?.ip.map(|ip| SocketAddr::new(ip, 443))
        })
    }

    /// Si conectar con `host` necesita una dirección que no se sabe (revisión de la 1.0.56): con el bloqueo del modo
    /// estricto puesto y sin un túnel conectado, ningún nombre resuelve fuera del túnel, así que una salida solo puede ir
    /// a una dirección ya sabida. Sin ninguna, se dice a quien pide conectar en vez de seguir con la salida de antes.
    fn falta_direccion(&self, host: &str) -> bool {
        self.falta_direccion_con(host, &self.deseo())
    }

    /// [`Self::falta_direccion`] con lo guardado `d` (las pruebas dan el suyo).
    fn falta_direccion_con(&self, host: &str, d: &Deseo) -> bool {
        d.estricto
            && self.bloqueado.load(Ordering::SeqCst)
            && self.estado().fase != Fase::Conectada
            && self.direccion_conocida(host, d).is_none()
    }

    /// La conexión abierta, sin esperar por ella: si en ese momento está ocupada, nada.
    fn abierta(&self) -> Option<Abierta> {
        #[cfg(windows)]
        {
            self.conexion.try_lock().ok().and_then(|g| {
                g.as_ref().map(|c| Abierta {
                    par: c.tunel.estado(),
                    extremo: c.extremo.ip(),
                    dns_tunel: c.tunel.con_dns_del_tunel(),
                    corte: c.tunel.con_corte(),
                })
            })
        }
        #[cfg(not(windows))]
        {
            None
        }
    }

    /// Lo último que se midió de las salidas, como está.
    fn medidas_guardadas(&self) -> Option<Medidas> {
        self.medidas.lock().ok().and_then(|m| m.clone())
    }

    /// Si ahora se pueden medir las salidas: con la VPN apagada y sin el bloqueo del modo estricto (encendida, la medida
    /// iría por el túnel; con el bloqueo, no saldría).
    fn se_puede_medir(&self) -> bool {
        !self.estado().fase.encendida() && !self.bloqueado.load(Ordering::SeqCst)
    }

    /// Lo que se sabe de las salidas, al día: lo medido si vale todavía (un minuto); si no, se miden todas ahora, a la vez
    /// (cada una en su hilo). Cuando no se puede medir, lo medido antes; y si no hay nada, solo sus direcciones, sin dar
    /// ninguna por disponible ([`Self::solo_direcciones`]).
    fn medidas_al_dia(&self) -> Option<Medidas> {
        if no_disponible().is_some() {
            return None;
        }
        self.medidas_con(ahora_ms, mide_todas, |cuando| self.solo_direcciones(cuando))
    }

    /// [`Self::medidas_al_dia`] con el reloj (`ahora`), la medida (`mide`) y lo que se sabe sin medir (`sin_medir`)
    /// dados: las pruebas los cambian.
    fn medidas_con(
        &self,
        ahora: impl Fn() -> u64,
        mide: impl FnOnce(u64) -> Medidas,
        sin_medir: impl FnOnce(u64) -> Medidas,
    ) -> Option<Medidas> {
        if !self.se_puede_medir() {
            return self
                .medidas_guardadas()
                .or_else(|| Some(sin_medir(ahora())));
        }
        let _una = self.midiendo.lock().unwrap_or_else(PoisonError::into_inner);
        let cuando = ahora();
        if let Some(m) = self
            .medidas_guardadas()
            .filter(|m| cuando.saturating_sub(m.cuando_ms) < MEDIDAS_VALEN_MS)
        {
            return Some(m);
        }
        let m = mide(cuando);
        // si mientras se medía se encendió la VPN (o se puso el bloqueo), lo medido pudo ir por el túnel: ni se guarda ni
        // vale como medida; de ello, solo las direcciones
        if !self.se_puede_medir() {
            return self.medidas_guardadas().or_else(|| Some(m.sin_medir()));
        }
        if let Ok(mut g) = self.medidas.lock() {
            *g = Some(m.clone());
        }
        Some(m)
    }

    /// Lo único que se sabe de las salidas con la VPN encendida (o el bloqueo puesto) y nada medido: su dirección, la de
    /// su nombre resuelto ahora (por el túnel, si está abierto; con el bloqueo puesto y sin túnel ni se pregunta) o la que
    /// ya se sabía. Ni si está ni a cuánto, que iría por el túnel; ni se guarda.
    fn solo_direcciones(&self, cuando_ms: u64) -> Medidas {
        let resolver =
            !self.bloqueado.load(Ordering::SeqCst) || self.estado().fase == Fase::Conectada;
        let mut m = if resolver {
            solo_nombres(cuando_ms)
        } else {
            Medidas {
                cuando_ms,
                de: SALIDAS.iter().map(|s| (s.id, Medida::default())).collect(),
                medido: false,
            }
        };
        let d = self.deseo();
        for (id, medida) in &mut m.de {
            if medida.ip.is_none() {
                medida.ip = salidas::por_id(id)
                    .and_then(|s| d.sin_resolver(s.nombre, 443))
                    .map(|a| a.ip());
            }
        }
        m
    }

    /// `GET /api/vpn/salidas`: la tabla de salidas, con lo medido y la más rápida; la salida en la que está la VPN, con la
    /// dirección del túnel abierto, y disponible si está conectada.
    fn lista(&self) -> Value {
        let medidas = self.medidas_al_dia();
        let e = self.estado();
        let con_tunel = matches!(e.fase, Fase::Conectada | Fase::SinRespuesta);
        let en = e
            .fase
            .encendida()
            .then(|| salidas::por_nombre(&e.servidor))
            .flatten();
        let ip = self.abierta().filter(|_| con_tunel).map(|a| a.extremo);
        lista_json(
            medidas.as_ref(),
            en.map(|s| (s.id, ip)),
            e.fase == Fase::Conectada,
        )
    }

    /// `GET /api/vpn/comprueba`: lo que una web verá desde este equipo ahora, con `desfase_equipo` (los minutos al este de
    /// UTC del navegador que pregunta: la hora de este equipo).
    ///
    /// - La dirección, la que ve llegar el eco de un servidor de GUARDIANA (`guardiana_tunel::salidas::ip_vista`), nunca
    ///   un tercero: con el túnel abierto, el de otra salida (la propia no contesta a lo que llega por su túnel), así que
    ///   es la dirección con la que sale el tráfico del túnel; sin VPN, el de cualquiera, y es la de este equipo. En
    ///   medio (conectando, sin respuesta, con el bloqueo puesto) no se pregunta. Sin eco, sin dirección: la página no la
    ///   da por comprobada.
    /// - Su país, según la base de direcciones del programa (`guardiana_lists::ubicacion`).
    /// - El DNS y el IPv6, de lo que el túnel tiene puesto: el resolutor de GUARDIANA solo hacia el del túnel y el corte
    ///   (que no deja salir DNS ni IPv6 fuera del túnel; docs/VPN.md, «Reglas»). Se deducen, no se miden:
    ///   `"comprobado"` lo dice (revisión de la 1.0.56).
    /// - La hora: el desfase de este equipo y el de la salida ahora, con su horario de verano.
    /// - El permiso de ubicación de Windows para el equipo.
    ///
    /// Lo que se pregunta fuera (el eco y el permiso de ubicación) vale [`COMPROBADO_VALE_MS`] para el mismo estado de la
    /// VPN: dos peticiones HTTPS y un `reg.exe` en cada llamada eran demasiado (revisión de la 1.0.56).
    fn comprueba(&self, desfase_equipo: i64) -> Value {
        let e = self.estado();
        let con_tunel = e.fase == Fase::Conectada;
        let fuera = self.se_puede_medir();
        let abierta = self.abierta().filter(|_| con_tunel);
        let en = if con_tunel {
            salidas::por_nombre(&e.servidor)
        } else {
            None
        };
        let ahora = ahora_ms();
        let para = (
            self.generacion.load(Ordering::SeqCst),
            e.fase,
            e.servidor.clone(),
            fuera,
        );
        let guardado = self
            .preguntado
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .filter(|p| p.vale(&para, ahora));
        let p = guardado.unwrap_or_else(|| {
            let medidas = self.medidas_guardadas();
            let ip_vista = if no_disponible().is_none() && (con_tunel || fuera) {
                a_quien_preguntar(en, medidas.as_ref())
                    .into_iter()
                    .find_map(|s| salidas::ip_vista(s.nombre))
            } else {
                None
            };
            let p = Preguntado {
                cuando_ms: ahora,
                para,
                ip_vista,
                ubicacion: ubicacion_windows(),
            };
            if let Ok(mut g) = self.preguntado.lock() {
                *g = Some(p.clone());
            }
            p
        });
        comprueba_json(&Comprobacion {
            con_tunel,
            fuera,
            tunel: abierta.map(|a| (a.dns_tunel, a.corte)),
            tiene_ipv6: salidas::tiene_ipv6(),
            en,
            ip_vista: p.ip_vista,
            ahora_ms: i64::try_from(ahora).unwrap_or(0),
            desfase_equipo,
            ubicacion: p.ubicacion,
        })
    }
}

/// Lo que [`Vpn::comprueba`] reúne para su respuesta: lo del túnel y lo preguntado fuera.
#[derive(Debug, Clone, Copy)]
struct Comprobacion {
    /// La VPN está conectada: su túnel abierto y el servidor contestando.
    con_tunel: bool,
    /// La VPN está apagada y sin el bloqueo: lo que sale, sale fuera del túnel.
    fuera: bool,
    /// Del túnel abierto, si se pudo leer: si el resolutor de GUARDIANA manda solo al del túnel, y si el corte (o el
    /// bloqueo del modo estricto) está puesto.
    tunel: Option<(bool, bool)>,
    /// Si el equipo tiene salida a internet por IPv6 (`guardiana_tunel::salidas::tiene_ipv6`).
    tiene_ipv6: bool,
    /// La salida del túnel abierto.
    en: Option<&'static Salida>,
    /// La dirección que vio llegar el eco de una salida, si alguno contestó.
    ip_vista: Option<IpAddr>,
    /// Ahora, en milisegundos desde 1970.
    ahora_ms: i64,
    /// Los minutos al este de UTC del navegador que pregunta (la hora de este equipo).
    desfase_equipo: i64,
    /// El permiso de ubicación de Windows para el equipo.
    ubicacion: &'static str,
}

/// La respuesta de `GET /api/vpn/comprueba` con `c`. `"comprobado"` dice qué se midió de verdad (revisión de la 1.0.56):
/// la dirección, si un eco contestó; el DNS y el IPv6 salen de lo que el túnel tiene puesto, así que la página los da
/// «según la configuración».
fn comprueba_json(c: &Comprobacion) -> Value {
    let dns = match (c.con_tunel, c.tunel) {
        (true, Some((dns_tunel, corte))) if dns_tunel && corte => "tunel",
        (true, Some(_)) => "fuera",
        (false, _) if c.fuera => "fuera",
        _ => "desconocido",
    };
    // con el corte puesto, de IPv6 fuera del túnel solo sale lo del enlace, la red tenga IPv6 o no
    let ipv6 = match (c.con_tunel, c.tunel) {
        (true, Some((_, true))) => Some("bloqueado"),
        (true, Some(_)) => Some(if c.tiene_ipv6 { "fuga" } else { "sin_ipv6" }),
        (false, _) if c.fuera && !c.tiene_ipv6 => Some("sin_ipv6"),
        _ => None,
    };
    let desfase_salida = c.en.and_then(|s| salidas::desfase_min(s.zona, c.ahora_ms));
    json!({
        "ip_vista": c.ip_vista.map(|ip| ip.to_string()),
        "pais_ip": c.ip_vista.and_then(|ip| guardiana_lists::ubicacion::ubicacion_de_ip(ip).pais),
        "dns": dns,
        "ipv6": ipv6,
        "comprobado": {
            "ip": c.ip_vista.is_some(),
            "dns": false,
            "ipv6": false,
        },
        "zona": {
            "equipo": Value::Null,
            "desfase_equipo": c.desfase_equipo,
            "salida": c.en.map(|s| s.zona),
            "desfase_salida": desfase_salida,
            "coincide": desfase_salida.map(|d| i64::from(d) == c.desfase_equipo),
        },
        "idioma": Value::Null,
        "ubicacion_windows": c.ubicacion,
    })
}

/// Las salidas a cuyo eco se pregunta qué dirección ve, en el orden de la tabla: todas menos `en` (la salida del túnel
/// abierto) y menos las que la última medida vio no disponibles.
fn a_quien_preguntar(
    en: Option<&'static Salida>,
    medidas: Option<&Medidas>,
) -> Vec<&'static Salida> {
    SALIDAS
        .iter()
        .filter(|s| en.is_none_or(|x| x.id != s.id))
        .filter(|s| {
            medidas
                .filter(|m| m.medido)
                .and_then(|m| m.de(s.id))
                .is_none_or(|m| m.disponible)
        })
        .collect()
}

/// Qué hace una conexión del modo estricto que no pudo seguir (sin dirección, o sin poder guardarla) y no tiene túnel
/// abierto (segunda revisión de la 1.0.56, «N2»): quien pidió el cambio ya subió la generación, así que el bucle que
/// reconectaba con la salida de antes se acabó.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
enum TrasNoCambiar {
    /// Lo guardado aún quiere la VPN encendida en el modo estricto: el bloqueo sigue, y se vuelve a intentar con su
    /// servidor (su nombre y la dirección de su alta).
    Sigue(String, SocketAddr),
    /// Lo guardado no se puede leer ahora: lo hace la mirada de cada minuto, como la reconciliación del arranque.
    Luego,
    /// Nada que seguir: la VPN no estaba encendida en el modo estricto.
    Nada,
}

/// [`TrasNoCambiar`] con lo guardado, como se pudo leer (`lee_si_se_puede`).
#[cfg_attr(not(windows), allow(dead_code))]
fn tras_no_cambiar(guardado: std::io::Result<Deseo>) -> TrasNoCambiar {
    match guardado {
        Ok(d) if d.quiere_bloqueo() => d.alta.map_or(TrasNoCambiar::Nada, |alta| {
            TrasNoCambiar::Sigue(d.servidor, alta)
        }),
        Ok(_) => TrasNoCambiar::Nada,
        Err(_) => TrasNoCambiar::Luego,
    }
}

/// Lo guardado para la reconciliación ([`Vpn::reconcilia`]), como se pudo leer (`lee_si_se_puede`): un archivo que ahora
/// no se puede leer (otro programa lo tiene un momento) no es la VPN apagada, y se vuelve a mirar en un minuto
/// (`None`); uno que se lee pero no es texto, sí, como el texto que no se entiende ([`Deseo::de_texto`]).
#[cfg_attr(not(windows), allow(dead_code))]
fn guardado_para_reconciliar(leido: std::io::Result<Deseo>) -> Option<Deseo> {
    match leido {
        Ok(d) => Some(d),
        Err(e) if e.kind() == std::io::ErrorKind::InvalidData => Some(Deseo::default()),
        Err(_) => None,
    }
}

/// Una salida para la página, con lo que se sabe de ella: su dirección, si está (`None`: no se sabe) y a cuánto.
fn salida_json(s: &Salida, ip: Option<IpAddr>, disponible: Option<bool>, ms: Option<u32>) -> Value {
    json!({
        "id": s.id,
        "pais": s.pais,
        "ciudad": s.ciudad,
        "continente": s.continente,
        "proveedor": s.proveedor,
        "nombre": s.nombre,
        "zona": s.zona,
        "ip": ip.map(|ip| ip.to_string()),
        "disponible": disponible,
        "ms": ms,
    })
}

/// La tabla de salidas para la página, con `medidas`: sin medida de verdad (la VPN encendida y nada medido), de cada una
/// solo su dirección, y ni si está, ni a cuánto, ni la más rápida (revisión de la 1.0.56). `en`, la salida en la que está
/// la VPN (su id, y la dirección del túnel abierto), disponible si está `conectada`.
fn lista_json(
    medidas: Option<&Medidas>,
    en: Option<(&str, Option<IpAddr>)>,
    conectada: bool,
) -> Value {
    let medido = medidas.is_some_and(|m| m.medido);
    let lista: Vec<Value> = SALIDAS
        .iter()
        .map(|s| {
            let m = medidas.and_then(|m| m.de(s.id)).copied();
            let mut ip = m.and_then(|m| m.ip);
            let mut disponible = m.filter(|_| medido).map(|m| m.disponible);
            let ms = m.filter(|_| medido).and_then(|m| m.ms);
            if let Some((id, del_tunel)) = en {
                if id == s.id {
                    ip = del_tunel.or(ip);
                    if conectada {
                        disponible = Some(true);
                    }
                }
            }
            salida_json(s, ip, disponible, ms)
        })
        .collect();
    json!({
        "salidas": lista,
        "mas_rapida": medidas.and_then(Medidas::mas_rapida).map(|s| s.id),
        "medido": medidas.filter(|m| m.medido).map(|m| m.cuando_ms),
    })
}

/// Mide todas las salidas desde aquí, a la vez (cada una en su hilo): si están, a cuánto y su dirección.
fn mide_todas(cuando_ms: u64) -> Medidas {
    let hilos: Vec<_> = SALIDAS
        .iter()
        .map(|s| {
            let s = *s;
            (s.id, std::thread::spawn(move || salidas::mide(&s)))
        })
        .collect();
    Medidas {
        cuando_ms,
        de: hilos
            .into_iter()
            .map(|(id, h)| (id, h.join().unwrap_or_default()))
            .collect(),
        medido: true,
    }
}

/// Las direcciones de las salidas, con su nombre resuelto ahora (por el túnel, si está puesto), cada una en su hilo. Ni
/// si están ni a cuánto (sin medida de verdad); ni se guarda.
fn solo_nombres(cuando_ms: u64) -> Medidas {
    let hilos: Vec<_> = SALIDAS
        .iter()
        .map(|s| {
            let nombre = s.nombre;
            (
                s.id,
                std::thread::spawn(move || {
                    use std::net::ToSocketAddrs;
                    let ip = (nombre, 443)
                        .to_socket_addrs()
                        .ok()
                        .and_then(|mut v| v.find(SocketAddr::is_ipv4))
                        .map(|a| a.ip());
                    Medida {
                        disponible: false,
                        ms: None,
                        ip,
                    }
                }),
            )
        })
        .collect();
    Medidas {
        cuando_ms,
        de: hilos
            .into_iter()
            .map(|(id, h)| (id, h.join().unwrap_or_default()))
            .collect(),
        medido: false,
    }
}

/// El permiso de ubicación de Windows para todo el equipo (`HKLM\…\ConsentStore\location`, `Value`), leído con el
/// `reg.exe` del propio Windows: «permitida», «denegada» o «desconocida».
fn ubicacion_windows() -> &'static str {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // el reg.exe de Windows, por su ruta entera; CREATE_NO_WINDOW: sin consola desde el servicio
        let raiz = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_owned());
        std::process::Command::new(format!(r"{raiz}\System32\reg.exe"))
            .args([
                "query",
                r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\location",
                "/v",
                "Value",
            ])
            .creation_flags(0x0800_0000)
            .output()
            .map_or("desconocida", |o| permiso_de_reg(&String::from_utf8_lossy(&o.stdout)))
    }
    #[cfg(not(windows))]
    {
        "desconocida"
    }
}

/// `Value` en la salida de `reg query` (`    Value    REG_SZ    Allow`): «permitida», «denegada» o «desconocida».
#[cfg_attr(not(windows), allow(dead_code))]
fn permiso_de_reg(salida: &str) -> &'static str {
    let valor = salida.lines().find_map(|l| {
        let mut p = l.split_whitespace();
        if p.next()? != "Value" {
            return None;
        }
        p.nth(1)
    });
    match valor {
        Some(v) if v.eq_ignore_ascii_case("Allow") => "permitida",
        Some(v) if v.eq_ignore_ascii_case("Deny") => "denegada",
        _ => "desconocida",
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

    /// El modo estricto, de la generación `gen`, con el servidor `host`: su dirección (el nombre resuelto ahora, si hay
    /// por dónde: con el bloqueo puesto solo resuelve por un túnel que siga abierto; si no, la que ya se sabía, guardada o
    /// medida con la VPN apagada); la conexión de antes (otro servidor) se cierra con el bloqueo puesto, y empieza el
    /// bucle. Sin dirección, o sin poder guardarla encendida, se dice y nada cambia ([`Self::no_cambia`]; revisión de la
    /// 1.0.56: hasta entonces se seguía en silencio con el servidor de antes, aunque estuviera caído).
    fn conecta_estricto(&'static self, host: &str, gen: u64) {
        let d = self.deseo();
        let sin_tunel = self
            .conexion
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_none();
        let resuelto = if self.bloqueado.load(Ordering::SeqCst) && sin_tunel {
            // con el bloqueo puesto y sin túnel ningún nombre resuelve: ni se pregunta
            None
        } else {
            guardiana_tunel::conexion::resuelve(host, 443).ok()
        };
        let Some(alta) = resuelto.or_else(|| self.direccion_conocida(host, &d)) else {
            self.no_cambia(
                host,
                gen,
                format!("no se encontró la dirección de {host}: la VPN no cambió de salida"),
            );
            return;
        };
        let host = host.to_owned();
        {
            let uno = self.cambio.lock().unwrap_or_else(PoisonError::into_inner);
            if self.generacion.load(Ordering::SeqCst) != gen {
                return;
            }
            // lo medido de las salidas con la VPN apagada, también: con el bloqueo puesto, cambiar de salida va a esas
            // direcciones
            let medidas = self.medidas_guardadas().unwrap_or_default();
            let guardado = self.guarda_deseo(
                &self
                    .deseo()
                    .encendida_con(&host, alta)
                    .con_conocidas(medidas.direcciones()),
            );
            if let Err(e) = guardado {
                // sin quedar guardado, el modo estricto no se sostiene (el bucle lo leería como apagada y la apagaría sin
                // decir nada): se dice, y nada cambia
                drop(uno);
                self.no_cambia(
                    &host,
                    gen,
                    format!(
                        "no se pudo guardar la VPN encendida ({e}): la VPN no cambió de salida"
                    ),
                );
                return;
            }
            // con el bloqueo ya puesto (otro servidor, o el de antes de un cierre), va otra vez con este servidor
            if self.bloqueado.load(Ordering::SeqCst) {
                let _ = self.pon_bloqueo_dentro(self.deseo().servidor_del_bloqueo());
            }
            drop(uno);
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

    /// Conectar con `host`, de la generación `gen`, no puede seguir (no hay dirección a la que ir, o no se pudo guardar):
    /// se dice (`error`), y nada cambia, ni el bloqueo ni lo guardado (revisión de la 1.0.56). El túnel de antes, si sigue
    /// abierto, sigue con su vigía (el de antes se acabó al empezar esta conexión).
    ///
    /// Sin túnel abierto (la VPN reconectando con la salida de antes), el bucle que lo intentaba también se acabó al
    /// empezar esta conexión: si lo guardado aún quiere la VPN encendida en el modo estricto, vuelve a intentarlo con su
    /// servidor, con el bloqueo puesto como estaba; si lo guardado no se puede leer ahora, lo hace la mirada de cada
    /// minuto ([`tras_no_cambiar`]; segunda revisión de la 1.0.56, «N2»: hasta entonces el bloqueo se quedaba sin nadie
    /// que reconectara, y la página decía que la VPN no había cambiado).
    fn no_cambia(&'static self, host: &str, gen: u64, error: String) {
        let g = self.conexion.lock().unwrap_or_else(PoisonError::into_inner);
        if self.generacion.load(Ordering::SeqCst) != gen {
            return;
        }
        if let Some(c) = g.as_ref() {
            self.pon(Estado {
                error,
                ..conectada(c)
            });
            drop(g);
            std::thread::spawn(move || self.vigila(gen));
            return;
        }
        match tras_no_cambiar(estricto::lee_si_se_puede(&datos())) {
            TrasNoCambiar::Sigue(servidor, alta) => {
                self.pon(Estado {
                    fase: Fase::Reconectando,
                    servidor: servidor.clone(),
                    error,
                    ..Estado::default()
                });
                drop(g);
                self.bucle_estricto(&servidor, alta, gen);
            }
            que => {
                if que == TrasNoCambiar::Luego {
                    self.queda_pendiente(gen, Pendiente::Arranque);
                }
                self.pon(Estado {
                    fase: Fase::Error,
                    servidor: host.to_owned(),
                    error,
                    ..Estado::default()
                });
            }
        }
    }

    /// La VPN que debe seguir encendida en el modo estricto (`d`), otra vez, si `gen` sigue en vigor (al arrancar el
    /// servicio, o en la mirada de cada minuto): el bloqueo en el acto (Windows guardó el de antes, `quedan`, y este lleva
    /// el archivo del programa como está ahora, tras una actualización también) y el bucle que la levanta. Sin
    /// `wireguard.dll` no puede volver: no se pone otra vez el bloqueo (el que Windows ya tenga sigue, y «Desconectar» lo
    /// quita), el estado lo dice y se vuelve a mirar cada minuto (revisión de la 1.0.56). Dice si quedó hecho.
    fn vuelve(&'static self, d: Deseo, quedan: bool, gen: u64) -> bool {
        let Some(alta) = d.alta else {
            return true;
        };
        let uno = self.cambio.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(motivo) = no_disponible() {
            if self.generacion.load(Ordering::SeqCst) != gen {
                return true;
            }
            self.bloqueado.store(quedan, Ordering::SeqCst);
            let sigue = if quedan {
                "; el bloqueo del modo estricto sigue en Windows hasta que la apagues"
            } else {
                ""
            };
            self.pon(Estado {
                fase: Fase::Error,
                servidor: d.servidor,
                error: format!("{motivo}: la VPN no puede volver a conectar{sigue}"),
                ..Estado::default()
            });
            return false;
        }
        // la generación nueva, solo si nadie conectó ni desconectó entretanto
        if self
            .generacion
            .compare_exchange(gen, gen + 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return true;
        }
        let gen = gen + 1;
        self.bloqueado.store(quedan, Ordering::SeqCst);
        let _ = self.pon_bloqueo_dentro(d.servidor_del_bloqueo());
        self.pon(Estado {
            fase: Fase::Conectando,
            servidor: d.servidor.clone(),
            ..Estado::default()
        });
        drop(uno);
        let servidor = d.servidor;
        std::thread::spawn(move || self.bucle_estricto(&servidor, alta, gen));
        true
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
    /// servicio es el que puede ir al servidor. Los filtros de arranque van aparte: si Windows no los acepta, el bloqueo
    /// vale desde que arranca su servicio de filtrado y no desde el arranque del equipo; se anota y el estado lo dice
    /// (revisión de la 1.0.56: hasta entonces no se miraba).
    fn pon_bloqueo_dentro(&self, servidor: guardiana_tunel::corte::Servidor) -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let puesto = guardiana_apps::corte_vpn::pon_estricto(
            &guardiana_tunel::corte::estricto(servidor),
            &exe,
        )
        .map_err(|e| e.to_string())?;
        self.bloqueado.store(true, Ordering::SeqCst);
        let sin = puesto.arranque.err();
        if let Some(motivo) = &sin {
            eprintln!("guardiana: el bloqueo del modo estricto quedó sin sus filtros de arranque: {motivo}");
        }
        if let Ok(mut g) = self.sin_arranque.lock() {
            *g = sin;
        }
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
            if self.apagada_desde_fuera() {
                // `guardiana vpn apaga` la apagó desde fuera del servicio; si entretanto cambió la generación (quien apagó
                // el modo estricto lo guarda antes de subirla), ya se ocupó otro
                let _ = self.apaga_de(Some(gen));
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
    ///
    /// Lo guardado se lee en cada vistazo (revisión de la 1.0.56): si `guardiana vpn apaga` apagó la VPN y quitó su
    /// bloqueo sin el servicio, un túnel del modo estricto no sigue sin él (ni la página diciendo «bloqueado»).
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
            // Lo guardado se lee con el candado de la conexión ya tomado (segunda revisión de la 1.0.56, «N1»): quien
            // cambia de modo (`cambia_estricto`) lo guarda sin soltarlo, así que aquí nunca se mezcla el túnel nuevo con
            // lo guardado de antes. Leído antes del candado, encender el estricto con el túnel abierto podía verse como
            // «apagada desde fuera» y apagar la VPN entera.
            let apagada_fuera = self.apagada_desde_fuera();
            if apagada_fuera && c.tunel.es_estricto() {
                drop(g);
                let _ = self.apaga();
                return;
            }
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
                let Some(v) = vieja else {
                    return;
                };
                // con qué servidor sigue: el guardado; si ahora no se puede leer, el de este túnel
                let (servidor, alta) = (v.servidor.clone(), SocketAddr::new(v.extremo.ip(), 443));
                v.tunel.cierra();
                let (servidor, alta) = match estricto::lee_si_se_puede(&datos()) {
                    Ok(d) if !d.encendida => {
                        let _ = self.apaga();
                        return;
                    }
                    Ok(d) => (d.servidor, d.alta.unwrap_or(alta)),
                    Err(_) => (servidor, alta),
                };
                {
                    // con el candado de la conexión y la generación mirada dentro, como al guardarla: quien apaga sube la
                    // generación antes de tomar la conexión, así que «reconectando» nunca queda puesto tras apagar
                    // (revisión de la 1.0.56)
                    let _guardada = self.conexion.lock().unwrap_or_else(PoisonError::into_inner);
                    if self.generacion.load(Ordering::SeqCst) != gen {
                        return;
                    }
                    self.pon(Estado {
                        fase: Fase::Reconectando,
                        servidor: servidor.clone(),
                        ..Estado::default()
                    });
                }
                self.bucle_estricto(&servidor, alta, gen);
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
    /// el bloqueo puesto), apagarlo es volver a la conexión normal, como haría la VPN sin él. Lo que se guarda, siempre,
    /// y primero: si no se puede, nada cambia y se dice (segunda revisión de la 1.0.56, «N3»: hasta entonces se cambiaba
    /// y luego, si no se podía escribir, se quitaba el archivo, que vuelve a dejar el modo estricto encendido; con el
    /// túnel ya en el modo normal, el estado decía «estricto» sin serlo).
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
                let alta = SocketAddr::new(c.extremo.ip(), 443);
                let medidas = self.medidas_guardadas().unwrap_or_default();
                let nuevo = Deseo {
                    estricto: true,
                    ..d
                }
                .encendida_con(&c.servidor, alta)
                .con_extremo(&c.servidor, c.extremo)
                .con_conocidas(medidas.direcciones());
                self.pon_bloqueo_dentro(nuevo.servidor_del_bloqueo())?;
                if let Err(e) = c.tunel.a_estricto() {
                    // el túnel sigue con su corte de siempre; el bloqueo, aparte, se va
                    if quita_de_windows().is_ok() {
                        self.sin_bloqueo();
                    }
                    return Err(e.to_string());
                }
                if let Err(e) = self.guarda_deseo(&nuevo) {
                    // sin quedar guardado el modo estricto no se sostiene (el vigía y el próximo arranque lo leen):
                    // vuelve el corte de siempre
                    if c.tunel.a_normal().is_ok() && quita_de_windows().is_ok() {
                        self.sin_bloqueo();
                    }
                    return Err(e);
                }
                return Ok(true);
            }
            // sin modo estricto la VPN acaba con el servicio, como siempre: no se guarda encendida. Primero, y con el
            // candado de la conexión puesto (el vigía lee lo guardado con él): si no se puede escribir, nada cambia
            let apagado = Deseo {
                estricto: false,
                ..d.clone()
            }
            .apagada();
            self.guarda_deseo(&apagado).map_err(|e| {
                format!("no se pudo guardar ({e}): el modo estricto sigue encendido")
            })?;
            if let Err(e) = c.tunel.a_normal() {
                // el túnel sigue en el modo estricto: lo guardado, otra vez como estaba
                let _ = self.guarda_deseo(&d);
                return Err(e.to_string());
            }
            drop(g);
            let quitado = quita_de_windows();
            if quitado.is_ok() {
                self.sin_bloqueo();
            }
            return quitado.map(|()| false);
        }
        drop(g);
        if !activo && self.bloqueado.load(Ordering::SeqCst) {
            // sin túnel y con el bloqueo puesto (el servidor no contesta): sin modo estricto, el equipo vuelve a su
            // conexión normal, y la VPN queda apagada. Lo guardado, primero: si no se puede escribir, nada cambia (el
            // bloqueo sigue y se sigue intentando conectar)
            self.guarda_deseo(
                &Deseo {
                    estricto: false,
                    ..d.clone()
                }
                .apagada(),
            )
            .map_err(|e| format!("no se pudo guardar ({e}): el modo estricto sigue encendido"))?;
            self.generacion.fetch_add(1, Ordering::SeqCst);
            let quitado = quita_de_windows();
            if quitado.is_ok() {
                self.sin_bloqueo();
            }
            // el estado solo dice que la conexión del equipo sigue como estaba si el bloqueo se fue: si no, sigue puesto,
            // «desconectar» lo dice a la página y «Desconectar» lo vuelve a intentar (revisión de la 1.0.56)
            self.pon(Estado {
                fase: Fase::Error,
                servidor: d.servidor,
                error: match &quitado {
                    Ok(()) => guardiana_tunel::conexion::ErrorConexion::NoContesta.to_string(),
                    Err(e) => format!(
                        "el servidor no contesta, y el bloqueo del modo estricto no se pudo quitar ({e}): sigue sin \
                         salir nada de tus programas fuera del túnel; «Desconectar» lo vuelve a intentar"
                    ),
                },
                ..Estado::default()
            });
            quitado?;
            return Ok(false);
        }
        self.guarda_deseo(&Deseo {
            estricto: activo,
            ..d
        })?;
        Ok(activo)
    }
}

/// La hora, en milisegundos desde 1970.
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
    /// El servidor: un nombre de guardianagroup.com. Vacío (o sin cuerpo): el de [`SERVIDOR`].
    #[serde(default)]
    servidor: String,
    /// La salida, por su id (`hel`), o `rapida`: la más rápida de lo medido (5 nov 2026; la web de GUARDIANA lo pide así
    /// a través de ZERO). Si está, manda sobre `servidor`.
    #[serde(default)]
    salida: String,
}

/// El servidor de un pedido: la salida por su id; la más rápida de `medidas` (`rapida`; sin medidas, [`SERVIDOR`]); o el
/// nombre dicho; sin nada, [`SERVIDOR`].
fn servidor_del_pedido(p: &Pedido, medidas: Option<&Medidas>) -> Result<String, &'static str> {
    match (p.salida.trim(), p.servidor.trim()) {
        ("", "") => Ok(SERVIDOR.to_owned()),
        ("", s) => Ok(s.to_ascii_lowercase()),
        (salidas::RAPIDA, _) => Ok(medidas
            .and_then(Medidas::mas_rapida)
            .map_or(SERVIDOR, |s| s.nombre)
            .to_owned()),
        (id, _) => salidas::por_id(id)
            .map(|s| s.nombre.to_owned())
            .ok_or("esa salida no es de GUARDIANA"),
    }
}

/// El servidor al que se conecta quien no elige uno: el nombre que tendrá el primer extremo (Helsinki) cuando esté
/// dentro del sello (docs/VPN.md). Hasta entonces el nombre no resuelve y la conexión lo dice.
pub(crate) const SERVIDOR: &str = "vpn.guardianagroup.com";

/// `POST /api/vpn/conecta` `{"servidor": "…guardianagroup.com"}` (o `{}`): empieza a conectar; la página pregunta a
/// `GET /api/vpn` cómo va. Con la VPN ya encendida, otro servidor la cambia de servidor (en el modo estricto, con el
/// bloqueo puesto todo el rato); el mismo, nada. Con el bloqueo puesto y sin túnel, solo a una salida cuya dirección se
/// sabe: si no, `{"ok": false, "motivo": …}` y nada cambia.
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
    let host = match servidor_del_pedido(&pedido, state.vpn.medidas_guardadas().as_ref()) {
        Ok(h) => h,
        Err(motivo) => return Ok(Json(json!({ "ok": false, "motivo": motivo }))),
    };
    if !guardiana_tunel::alta::es_servidor(&host) {
        return Ok(Json(
            json!({ "ok": false, "motivo": "ese servidor no es de GUARDIANA" }),
        ));
    }
    // con el bloqueo del modo estricto puesto y sin túnel, solo a una salida cuya dirección se sabe; si no, se dice y nada
    // cambia (revisión de la 1.0.56: hasta entonces se contestaba que sí y se seguía con la salida de antes)
    if state.vpn.falta_direccion(&host) {
        return Ok(Json(json!({ "ok": false, "motivo": SIN_DIRECCION })));
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

/// `GET /api/vpn/salidas`: las salidas de GUARDIANA, con lo medido desde este equipo (si están, a cuánto y su
/// dirección) y la más rápida. Medir tarda unos segundos: fuera de los hilos de la página.
pub(crate) async fn ver_salidas(
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<Value> {
    let vpn = state.vpn;
    let r = tokio::task::spawn_blocking(move || vpn.lista()).await;
    Ok(Json(r.unwrap_or_else(|_| lista_json(None, None, false))))
}

/// `GET /api/vpn/comprueba`: lo que una web verá desde este equipo ahora (la dirección, el DNS, el IPv6, la hora y el
/// permiso de ubicación de Windows). Pregunta a los servidores de GUARDIANA: fuera de los hilos de la página.
pub(crate) async fn comprueba(
    State(state): State<Arc<AppState>>,
    Zona(desfase): Zona,
    _s: Session,
) -> ApiResult<Value> {
    let vpn = state.vpn;
    let r = tokio::task::spawn_blocking(move || vpn.comprueba(desfase)).await;
    Ok(Json(r.unwrap_or_else(|_| json!({}))))
}

/// Lo que pide el interruptor del modo estricto.
#[derive(Debug, Default, Deserialize)]
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) struct PideEstricto {
    /// Encendido o apagado (también como `estricto`, como lo pide la web de GUARDIANA). Sin él (o sin cuerpo), lo
    /// contrario de lo que está: el interruptor.
    #[serde(default, alias = "estricto")]
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
        let f = v.foto_con(es, &Deseo::default());
        assert_eq!(f["fase"], "apagada");
        assert_eq!(f["enviados"], Value::Null);
        // 5 nov 2026: sin VPN, ni salida ni cifras; los campos están (así la web sabe que el programa tiene salidas)
        assert!(f.get("salida").is_some_and(Value::is_null));
        assert_eq!(f["desde"], Value::Null);
        assert_eq!(f["subidos"], Value::Null);
        assert_eq!(f["postcuantica"], false);
        assert_eq!(f["dns_tunel"], false);
        // el modo estricto: su interruptor, con su línea, y sin aviso mientras no hay bloqueo
        assert!(f["estricto"].is_boolean());
        assert_eq!(f["bloqueado"], false);
        assert_eq!(f["aviso"], "");
        assert_eq!(f["estricto_titulo"], "Modo estricto");
        assert!(f["estricto_linea"]
            .as_str()
            .is_some_and(|l| l.len() > 40 && !l.contains("vpn_")));
        // revisión de la 1.0.56: sin VPN ni bloqueo, nada que desconectar ni aviso de los filtros de arranque
        assert_eq!(f["desconectar"], false);
        assert_eq!(f["aviso_arranque"], "");
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
        let f = v.foto_con(es, &Deseo::default());
        assert_eq!(f["fase"], "error");
        assert_eq!(f["error"], "el servidor no contesta");
        assert_eq!(f["desconectar"], false);
        // un servidor que no es una salida de la tabla: sin salida
        assert_eq!(f["salida"], Value::Null);
        // el servidor dejó de contestar con el túnel puesto (desde la 1.0.53)
        v.pon(Estado {
            fase: Fase::SinRespuesta,
            servidor: "vpn.guardianagroup.com".into(),
            direccion: "10.66.0.7".into(),
            error: "el servidor aún no contesta".into(),
            ..Estado::default()
        });
        let f = v.foto_con(es, &Deseo::default());
        assert_eq!(f["fase"], "sin_respuesta");
        assert_eq!(f["direccion"], "10.66.0.7");
        assert_eq!(f["aviso"], "");
        assert_eq!(f["desconectar"], true);
        // la salida: Helsinki, con su país y su ciudad; sin túnel abierto en esta prueba, sin dirección. Con el servidor
        // sin contestar y nada medido, si está no se sabe (revisión de la 1.0.56: decía que sí)
        assert_eq!(f["salida"]["id"], "hel");
        assert_eq!(f["salida"]["pais"], "FI");
        assert_eq!(f["salida"]["ciudad"], "Helsinki");
        assert_eq!(f["salida"]["disponible"], Value::Null);
        assert_eq!(f["salida"]["ip"], Value::Null);
        assert_eq!(f["postcuantica"], true);
        // conectada (el servidor contesta): disponible
        v.pon(Estado {
            fase: Fase::Conectada,
            servidor: "vpn.guardianagroup.com".into(),
            ..Estado::default()
        });
        assert_eq!(
            v.foto_con(es, &Deseo::default())["salida"]["disponible"],
            true
        );
        // el modo estricto con el servidor callado: el bloqueo sigue, y la página lo dice con el aviso
        v.bloqueado.store(true, Ordering::SeqCst);
        v.pon(Estado {
            fase: Fase::Reconectando,
            servidor: "vpn.guardianagroup.com".into(),
            ..Estado::default()
        });
        let f = v.foto_con(es, &Deseo::default());
        assert_eq!(f["fase"], "reconectando");
        assert_eq!(f["bloqueado"], true);
        assert_eq!(f["desconectar"], true);
        assert_eq!(
            f["aviso"],
            "La VPN no conecta: no sale nada de tus programas de Windows fuera del túnel hasta que vuelva o la apagues tú"
        );
        // en inglés, el mismo aviso en su idioma
        let f = v.foto_con(guardiana_core::i18n::en(), &Deseo::default());
        assert!(f["aviso"]
            .as_str()
            .is_some_and(|a| a.starts_with("The VPN")));
        // un error con el bloqueo aún en Windows (no se pudo quitar): «Desconectar» hace falta, y la página lo sabe
        v.pon(Estado {
            fase: Fase::Error,
            servidor: "vpn.guardianagroup.com".into(),
            error: "no se pudo quitar".into(),
            ..Estado::default()
        });
        let f = v.foto_con(es, &Deseo::default());
        assert_eq!(
            (f["fase"].as_str(), f["desconectar"].as_bool()),
            (Some("error"), Some(true))
        );
        // Windows no aceptó los filtros de arranque del bloqueo: el aviso, mientras el bloqueo esté
        *v.sin_arranque.lock().unwrap() = Some("FWP_E_…".into());
        let aviso = v.foto_con(es, &Deseo::default())["aviso_arranque"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        assert!(aviso.starts_with("Windows no aceptó"), "{aviso}");
        v.sin_bloqueo();
        let f = v.foto_con(es, &Deseo::default());
        assert_eq!(
            (f["bloqueado"].as_bool(), f["aviso_arranque"].as_str()),
            (Some(false), Some(""))
        );
        assert_eq!(f["desconectar"], false);
        // la VPN aún encendida en lo guardado (sin wireguard.dll, o tras no poder guardar al desconectar): «Desconectar»
        // sigue a la vista; y la dirección de su salida, la que se sabe
        let encendida = Deseo::default().encendida_con(
            "vpn.guardianagroup.com",
            "62.238.17.121:443".parse().unwrap(),
        );
        let f = v.foto_con(es, &encendida);
        assert_eq!(f["desconectar"], true);
        assert_eq!(f["salida"]["ip"], "62.238.17.121");
        assert_eq!(f["salida"]["disponible"], Value::Null);
    }

    /// Review of the 1.0.56 VPN: an exit's address with the block on and the tunnel down comes from what is kept or what
    /// was measured; with neither, «conectar» says so instead of carrying on with the exit of before.
    #[test]
    fn sin_tunel_y_con_el_bloqueo_solo_a_una_direccion_que_se_sabe() {
        let v = Vpn::default();
        let nbg = "vpn-nbg.guardianagroup.com";
        let hel: SocketAddr = "62.238.17.121:443".parse().unwrap();
        let d = Deseo::default().encendida_con("vpn.guardianagroup.com", hel);
        // without the block, or with a tunnel connected, names resolve: nothing is missing
        assert!(!v.falta_direccion_con(nbg, &d));
        v.bloqueado.store(true, Ordering::SeqCst);
        v.pon(Estado {
            fase: Fase::Conectada,
            servidor: "vpn.guardianagroup.com".into(),
            ..Estado::default()
        });
        assert!(!v.falta_direccion_con(nbg, &d));
        // the tunnel down: Núremberg's address is not known, Helsinki's is
        v.pon(Estado {
            fase: Fase::Reconectando,
            servidor: "vpn.guardianagroup.com".into(),
            ..Estado::default()
        });
        assert!(v.falta_direccion_con(nbg, &d));
        assert!(!v.falta_direccion_con("vpn.guardianagroup.com", &d));
        assert_eq!(
            v.direccion_conocida("vpn.guardianagroup.com", &d),
            Some(hel)
        );
        // without strict mode there is no block to go through
        let normal = Deseo {
            estricto: false,
            ..d.clone()
        };
        assert!(!v.falta_direccion_con(nbg, &normal));
        // measured with the VPN off: known
        let ip: IpAddr = "168.119.225.147".parse().unwrap();
        *v.medidas.lock().unwrap() = Some(Medidas {
            cuando_ms: 1,
            de: vec![(
                "nbg",
                Medida {
                    disponible: true,
                    ms: Some(31),
                    ip: Some(ip),
                },
            )],
            medido: true,
        });
        assert_eq!(
            v.direccion_conocida(nbg, &d),
            Some(SocketAddr::new(ip, 443))
        );
        assert!(!v.falta_direccion_con(nbg, &d));
        // or kept from an earlier exit
        let antes = d
            .clone()
            .encendida_con(nbg, SocketAddr::new(ip, 443))
            .encendida_con("vpn.guardianagroup.com", hel);
        *v.medidas.lock().unwrap() = None;
        assert_eq!(
            v.direccion_conocida(nbg, &antes),
            Some(SocketAddr::new(ip, 443))
        );
        // and what is measured goes into what is kept when connecting
        let m = Medidas {
            cuando_ms: 1,
            de: vec![(
                "mia",
                Medida {
                    disponible: true,
                    ms: None,
                    ip: Some(ip),
                },
            )],
            medido: true,
        };
        let guardado = d.con_conocidas(m.direcciones());
        assert_eq!(
            guardado.sin_resolver("vpn-mia.guardianagroup.com", 443),
            Some(SocketAddr::new(ip, 443))
        );
    }

    /// Review of the 1.0.56 VPN: what `GET /api/vpn/comprueba` says in each case, and what of it was really measured.
    #[test]
    fn lo_que_dice_la_comprobacion_en_cada_caso() {
        let hel = salidas::por_id("hel");
        let ip: IpAddr = "62.238.17.121".parse().unwrap();
        let base = Comprobacion {
            con_tunel: false,
            fuera: false,
            tunel: None,
            tiene_ipv6: false,
            en: None,
            ip_vista: None,
            // 5 Nov 2026, 12:00 UTC
            ahora_ms: 1_793_880_000_000,
            desfase_equipo: -300,
            ubicacion: "desconocida",
        };
        // «dns ipv6», con «-» cuando el IPv6 no se dice
        let dice = |c: Comprobacion| {
            let v = comprueba_json(&c);
            format!(
                "{} {}",
                v["dns"].as_str().unwrap_or_default(),
                v["ipv6"].as_str().unwrap_or("-")
            )
        };
        // connected, its resolver only to the tunel's and the cut on: by the tunnel, and IPv6 blocked
        let conectada = Comprobacion {
            con_tunel: true,
            tunel: Some((true, true)),
            en: hel,
            ip_vista: Some(ip),
            ..base
        };
        assert_eq!(dice(conectada), "tunel bloqueado");
        // connected without the cut: the DNS out, and IPv6 out if there is IPv6
        let sin_corte = Comprobacion {
            tunel: Some((true, false)),
            ..conectada
        };
        assert_eq!(dice(sin_corte), "fuera sin_ipv6");
        assert_eq!(
            dice(Comprobacion {
                tiene_ipv6: true,
                ..sin_corte
            }),
            "fuera fuga"
        );
        // connected, the tunnel busy at that moment: not known
        assert_eq!(
            dice(Comprobacion {
                tunel: None,
                ..conectada
            }),
            "desconocido -"
        );
        // no VPN and no block: out; IPv6 said only when there is none
        let fuera = Comprobacion {
            fuera: true,
            ..base
        };
        assert_eq!(dice(fuera), "fuera sin_ipv6");
        assert_eq!(
            dice(Comprobacion {
                tiene_ipv6: true,
                ..fuera
            }),
            "fuera -"
        );
        // connecting, reconnecting or with the block on: not known
        assert_eq!(dice(base), "desconocido -");
        // what was really measured: the address when an echo answered; the DNS and IPv6, never (from the settings)
        let v = comprueba_json(&conectada);
        assert_eq!(v["ip_vista"], "62.238.17.121");
        assert_eq!(
            v["comprobado"],
            json!({ "ip": true, "dns": false, "ipv6": false })
        );
        let v = comprueba_json(&base);
        assert_eq!(v["ip_vista"], Value::Null);
        assert_eq!(
            v["comprobado"],
            json!({ "ip": false, "dns": false, "ipv6": false })
        );
        // the time: the exit's zone and offset against the computer's (Helsinki in November: +120; the computer, -300)
        let v = comprueba_json(&conectada);
        assert_eq!(v["zona"]["salida"], "Europe/Helsinki");
        assert_eq!(v["zona"]["desfase_salida"], 120);
        assert_eq!(v["zona"]["desfase_equipo"], -300);
        assert_eq!(v["zona"]["coincide"], false);
        assert_eq!(v["ubicacion_windows"], "desconocida");
        assert_eq!(comprueba_json(&base)["zona"]["salida"], Value::Null);
    }

    /// Review of the 1.0.56 VPN: what `comprueba` asked outside (two HTTPS requests and a reg.exe) is reused for 15 s,
    /// and only for the same state of the VPN.
    #[test]
    fn lo_preguntado_vale_unos_segundos_y_para_el_mismo_estado() {
        let para = (
            3,
            Fase::Conectada,
            "vpn.guardianagroup.com".to_owned(),
            false,
        );
        let p = Preguntado {
            cuando_ms: 1_000,
            para: para.clone(),
            ip_vista: None,
            ubicacion: "permitida",
        };
        assert!(p.vale(&para, 1_000));
        assert!(p.vale(&para, 1_000 + COMPROBADO_VALE_MS - 1));
        assert!(!p.vale(&para, 1_000 + COMPROBADO_VALE_MS));
        // another generation, phase, server or way out: asked again
        assert!(!p.vale(&(4, para.1, para.2.clone(), false), 1_001));
        assert!(!p.vale(&(3, Fase::Apagada, para.2.clone(), false), 1_001));
        assert!(!p.vale(
            &(3, para.1, "vpn-nbg.guardianagroup.com".into(), false),
            1_001
        ));
        assert!(!p.vale(&(3, para.1, para.2.clone(), true), 1_001));
    }

    /// Review of the 1.0.56 VPN: the exits are measured only with the VPN off and no block, what was measured is worth a
    /// minute, and a measure taken while the VPN came on is neither kept nor taken as measured.
    #[test]
    fn las_salidas_se_miden_solo_fuera_del_tunel_y_valen_un_minuto() {
        use std::cell::Cell;
        let v = Vpn::default();
        let veces = Cell::new(0);
        let ip: IpAddr = "62.238.17.121".parse().unwrap();
        let medida = |cuando_ms| {
            veces.set(veces.get() + 1);
            Medidas {
                cuando_ms,
                de: vec![(
                    "hel",
                    Medida {
                        disponible: true,
                        ms: Some(48),
                        ip: Some(ip),
                    },
                )],
                medido: true,
            }
        };
        let sin_medir = |cuando_ms| Medidas {
            cuando_ms,
            de: vec![("hel", Medida::default())],
            medido: false,
        };
        // the VPN off: measured, and kept
        let m = v.medidas_con(|| 10_000, medida, sin_medir).unwrap();
        assert_eq!((veces.get(), m.medido, m.cuando_ms), (1, true, 10_000));
        assert!(v.medidas_guardadas().is_some());
        // within the minute: what was kept
        let m = v.medidas_con(|| 69_999, medida, sin_medir).unwrap();
        assert_eq!((veces.get(), m.cuando_ms), (1, 10_000));
        // a minute later: measured again
        let m = v.medidas_con(|| 70_000, medida, sin_medir).unwrap();
        assert_eq!((veces.get(), m.cuando_ms), (2, 70_000));
        // the VPN on: never measured; what was measured before
        v.pon(Estado {
            fase: Fase::Conectada,
            servidor: "vpn.guardianagroup.com".into(),
            ..Estado::default()
        });
        let m = v.medidas_con(|| 999_999, medida, sin_medir).unwrap();
        assert_eq!((veces.get(), m.cuando_ms, m.medido), (2, 70_000, true));
        // and with nothing measured, only the addresses, nothing taken as available
        *v.medidas.lock().unwrap() = None;
        let m = v.medidas_con(|| 999_999, medida, sin_medir).unwrap();
        assert_eq!((veces.get(), m.medido), (2, false));
        let tabla = lista_json(Some(&m), None, false);
        assert_eq!(tabla["salidas"][0]["disponible"], Value::Null);
        assert_eq!(
            (tabla["mas_rapida"].is_null(), tabla["medido"].is_null()),
            (true, true)
        );
        // the VPN off but strict mode's block on: not measured either
        v.pon(Estado::default());
        v.bloqueado.store(true, Ordering::SeqCst);
        let _ = v.medidas_con(|| 999_999, medida, sin_medir);
        assert_eq!(veces.get(), 2);
        v.bloqueado.store(false, Ordering::SeqCst);
        // the VPN came on while measuring: the measure is not kept, and only its addresses count
        let v = Vpn::default();
        let m = v
            .medidas_con(
                || 5_000,
                |cuando_ms| {
                    v.pon(Estado {
                        fase: Fase::Conectando,
                        servidor: "vpn.guardianagroup.com".into(),
                        ..Estado::default()
                    });
                    medida(cuando_ms)
                },
                sin_medir,
            )
            .unwrap();
        assert!(!m.medido);
        assert_eq!(m.de("hel").and_then(|h| h.ip), Some(ip));
        assert!(v.medidas_guardadas().is_none());
    }

    /// Review of the 1.0.56 VPN: what was left to do is tried again only while the generation it was left at is the one
    /// in force; a connection or a disconnection since makes it moot.
    #[test]
    fn lo_pendiente_vale_solo_en_su_generacion() {
        let v = Vpn::default();
        assert_eq!(v.pendiente_ahora(), None);
        v.queda_pendiente(0, Pendiente::Arranque);
        assert_eq!(v.pendiente_ahora(), Some((0, Pendiente::Arranque)));
        // done, but only what was there: something newer stays
        v.queda_pendiente(0, Pendiente::Apagada);
        v.sin_pendiente(0, Pendiente::Arranque);
        assert_eq!(v.pendiente_ahora(), Some((0, Pendiente::Apagada)));
        v.sin_pendiente(0, Pendiente::Apagada);
        assert_eq!(v.pendiente_ahora(), None);
        // the person connected since: nothing left
        v.queda_pendiente(0, Pendiente::Arranque);
        v.generacion.fetch_add(1, Ordering::SeqCst);
        assert_eq!(v.pendiente_ahora(), None);
        assert_eq!(*v.pendiente.lock().unwrap(), None);
    }

    /// Second review of the 1.0.56 VPN («N2»): a change of exit that cannot go on, with no tunnel open, carries on with
    /// what is kept when the VPN is meant to be on in strict mode (the loop that was reconnecting ended when the change
    /// began); when what is kept cannot be read now, the minute's look does it; otherwise there is nothing to carry on.
    #[test]
    fn un_cambio_de_salida_que_no_sigue_vuelve_a_lo_guardado() {
        let hel: SocketAddr = "62.238.17.121:443".parse().unwrap();
        let encendida = Deseo::default().encendida_con("vpn.guardianagroup.com", hel);
        assert_eq!(
            tras_no_cambiar(Ok(encendida.clone())),
            TrasNoCambiar::Sigue("vpn.guardianagroup.com".into(), hel)
        );
        // strict mode off, or the VPN off: nothing to carry on with
        let sin_estricto = Deseo {
            estricto: false,
            ..encendida.clone()
        };
        assert_eq!(tras_no_cambiar(Ok(sin_estricto)), TrasNoCambiar::Nada);
        assert_eq!(
            tras_no_cambiar(Ok(encendida.apagada())),
            TrasNoCambiar::Nada
        );
        assert_eq!(tras_no_cambiar(Ok(Deseo::default())), TrasNoCambiar::Nada);
        // what is kept cannot be read now: the minute's look
        assert_eq!(
            tras_no_cambiar(Err(std::io::Error::other("ocupado"))),
            TrasNoCambiar::Luego
        );
    }

    /// Second review of the 1.0.56 VPN (with «N2», whose minute's look is the reconciliation): what is kept that cannot be
    /// read now is not the VPN off (it would remove a block the person wants); a file read whole that is not text is,
    /// as text that cannot be understood is.
    #[test]
    fn lo_que_no_se_puede_leer_no_es_la_vpn_apagada() {
        let hel: SocketAddr = "62.238.17.121:443".parse().unwrap();
        let encendida = Deseo::default().encendida_con("vpn.guardianagroup.com", hel);
        assert_eq!(
            guardado_para_reconciliar(Ok(encendida.clone())),
            Some(encendida.clone())
        );
        assert_eq!(
            guardado_para_reconciliar(Err(std::io::Error::other("ocupado"))),
            None
        );
        let dir = std::env::temp_dir().join(format!(
            "guardiana-vpn-reconcilia-{}-{}",
            std::process::id(),
            ahora_ms()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        // no file: the VPN off
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(
            guardado_para_reconciliar(estricto::lee_si_se_puede(&dir)),
            Some(Deseo::default())
        );
        // a file that cannot be read (a folder where it goes): not now
        std::fs::create_dir_all(dir.join(estricto::ARCHIVO)).unwrap();
        assert_eq!(
            guardado_para_reconciliar(estricto::lee_si_se_puede(&dir)),
            None
        );
        // read, but not text: the VPN off, as for text that cannot be understood
        std::fs::remove_dir_all(dir.join(estricto::ARCHIVO)).unwrap();
        std::fs::write(dir.join(estricto::ARCHIVO), [0xff, 0xfe, 0x00, 0x7b]).unwrap();
        assert_eq!(
            guardado_para_reconciliar(estricto::lee_si_se_puede(&dir)),
            Some(Deseo::default())
        );
        // read: as kept
        estricto::guarda_en(&dir, &encendida).unwrap();
        assert_eq!(
            guardado_para_reconciliar(estricto::lee_si_se_puede(&dir)),
            Some(encendida)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Review of the 1.0.56 VPN: «Desconectar» that cannot write what is kept removes the file (no file is the VPN off);
    /// if that fails too, it says so and leaves it to the minute's look, which writes it once it can.
    #[test]
    fn apagar_sin_poder_guardar() {
        let dir = std::env::temp_dir().join(format!(
            "guardiana-vpn-apagada-{}-{}",
            std::process::id(),
            ahora_ms()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let encendida = Deseo::default().encendida_con(
            "vpn.guardianagroup.com",
            "62.238.17.121:443".parse().unwrap(),
        );
        let v = Vpn::default();
        // written: off
        estricto::guarda_en(&dir, &encendida).unwrap();
        assert_eq!(
            v.guarda_apagada_en(&dir, &encendida.clone().apagada(), 0),
            Ok(())
        );
        assert!(!estricto::lee_de(&dir).encendida);
        // the new file cannot be written (a folder where it goes aside): the file goes, and the VPN reads off
        estricto::guarda_en(&dir, &encendida).unwrap();
        let aparte = dir.join(estricto::ARCHIVO).with_extension("json.tmp");
        std::fs::create_dir_all(&aparte).unwrap();
        assert_eq!(
            v.guarda_apagada_en(&dir, &encendida.clone().apagada(), 0),
            Ok(())
        );
        assert!(!dir.join(estricto::ARCHIVO).exists());
        assert!(!estricto::lee_de(&dir).encendida);
        assert_eq!(v.pendiente_ahora(), None);
        // second review («N3»): when no file would not be what was asked (strict mode off: without the file it reads on),
        // the file stays as it was; it is said (a restart would not bring the block back: strict mode is off), and left
        // for the minute's look
        std::fs::remove_dir_all(&aparte).unwrap();
        let sin_estricto = Deseo {
            estricto: false,
            ..encendida.clone()
        };
        estricto::guarda_en(&dir, &sin_estricto).unwrap();
        std::fs::create_dir_all(&aparte).unwrap();
        let r = v.guarda_apagada_en(&dir, &sin_estricto.clone().apagada(), 0);
        assert!(
            r.as_ref()
                .is_err_and(|e| e.contains("cada minuto") && !e.contains("su bloqueo")),
            "{r:?}"
        );
        assert_eq!(estricto::lee_de(&dir), sin_estricto);
        assert_eq!(v.pendiente_ahora(), Some((0, Pendiente::Apagada)));
        v.sin_pendiente(0, Pendiente::Apagada);
        std::fs::remove_file(dir.join(estricto::ARCHIVO)).unwrap();
        // neither written nor removed (a folder where the file is): said, and left for the minute's look
        std::fs::remove_dir_all(&aparte).unwrap();
        std::fs::create_dir_all(dir.join(estricto::ARCHIVO)).unwrap();
        let r = v.guarda_apagada_en(&dir, &encendida.clone().apagada(), 0);
        assert!(
            r.as_ref()
                .is_err_and(|e| e.contains("cada minuto") && e.contains("su bloqueo")),
            "{r:?}"
        );
        assert_eq!(v.pendiente_ahora(), Some((0, Pendiente::Apagada)));
        // not yet: it stays; once it can, written off
        let _ = std::fs::remove_file(&aparte);
        std::fs::remove_dir_all(dir.join(estricto::ARCHIVO)).unwrap();
        estricto::guarda_en(&dir, &encendida).unwrap();
        assert!(v.guarda_apagada_otra_vez_en(&dir, 0));
        assert!(!estricto::lee_de(&dir).encendida);
        // with another generation (a new connection since) it is moot, and nothing is touched
        estricto::guarda_en(&dir, &encendida).unwrap();
        assert!(v.guarda_apagada_otra_vez_en(&dir, 7));
        assert!(estricto::lee_de(&dir).encendida);
        // nor does the minute's look remove the file when that would turn strict mode back on («N3»)
        let sin_estricto_encendida = Deseo {
            estricto: false,
            ..encendida.clone()
        };
        estricto::guarda_en(&dir, &sin_estricto_encendida).unwrap();
        std::fs::create_dir_all(&aparte).unwrap();
        assert!(!v.guarda_apagada_otra_vez_en(&dir, 0));
        assert_eq!(estricto::lee_de(&dir), sin_estricto_encendida);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 5 Nov 2026: what «conectar» connects to: an exit by its id, the fastest measured one, a name, or Helsinki.
    #[test]
    fn el_servidor_de_cada_pedido() {
        let pide = |c: &str| serde_json::from_str::<Pedido>(c).unwrap();
        let m = |disponible, ms| Medida {
            disponible,
            ms,
            ip: None,
        };
        let medidas = Medidas {
            cuando_ms: 1,
            de: vec![
                ("hel", m(true, Some(48))),
                ("nbg", m(true, Some(31))),
                ("mia", m(false, Some(5))),
            ],
            medido: true,
        };
        let servidor = |c: &str, med: Option<&Medidas>| servidor_del_pedido(&pide(c), med);
        assert_eq!(servidor("{}", None).unwrap(), SERVIDOR);
        assert_eq!(
            servidor(r#"{"salida":"nbg"}"#, None).unwrap(),
            "vpn-nbg.guardianagroup.com"
        );
        assert_eq!(
            servidor(
                r#"{"salida":"mia","servidor":"vpn.guardianagroup.com"}"#,
                None
            )
            .unwrap(),
            "vpn-mia.guardianagroup.com"
        );
        // la más rápida de las disponibles (Miami no lo está); sin medidas, la de siempre
        assert_eq!(
            servidor(r#"{"salida":"rapida"}"#, Some(&medidas)).unwrap(),
            "vpn-nbg.guardianagroup.com"
        );
        assert_eq!(servidor(r#"{"salida":"rapida"}"#, None).unwrap(), SERVIDOR);
        // sin medida de verdad (la VPN encendida, solo direcciones), ninguna es la más rápida: la de siempre
        let sin_medir = medidas.clone().sin_medir();
        assert_eq!(
            servidor(r#"{"salida":"rapida"}"#, Some(&sin_medir)).unwrap(),
            SERVIDOR
        );
        assert_eq!(
            servidor(r#"{"servidor":"VPN.guardianagroup.com"}"#, None).unwrap(),
            "vpn.guardianagroup.com"
        );
        assert!(servidor(r#"{"salida":"xyz"}"#, None).is_err());
        assert!(servidor(r#"{"salida":"../hel"}"#, None).is_err());
        // el interruptor del modo estricto, también como lo pide la web
        let e = |c: &str| serde_json::from_str::<PideEstricto>(c).unwrap().activo;
        assert_eq!(e(r#"{"activo":true}"#), Some(true));
        assert_eq!(e(r#"{"estricto":false}"#), Some(false));
        assert_eq!(e("{}"), None);
    }

    /// 5 Nov 2026: the exits for the page, with what was measured, the fastest, and the one the VPN is in.
    #[test]
    fn la_tabla_de_salidas_para_la_pagina() {
        // nada medido: cada salida sin disponible ni retraso (no se sabe), y ninguna más rápida
        let v = lista_json(None, None, false);
        let l = v["salidas"].as_array().unwrap();
        assert_eq!(l.len(), SALIDAS.len());
        assert_eq!(l[0]["id"], "hel");
        assert_eq!(l[0]["zona"], "Europe/Helsinki");
        assert_eq!(l[0]["disponible"], Value::Null);
        assert_eq!(v["mas_rapida"], Value::Null);
        assert_eq!(v["medido"], Value::Null);
        let ip: std::net::IpAddr = "62.238.17.121".parse().unwrap();
        let medidas = Medidas {
            cuando_ms: 7,
            de: vec![
                (
                    "hel",
                    Medida {
                        disponible: true,
                        ms: Some(176),
                        ip: Some(ip),
                    },
                ),
                ("nbg", Medida::default()),
                ("mia", Medida::default()),
            ],
            medido: true,
        };
        let v = lista_json(Some(&medidas), None, false);
        assert_eq!(v["salidas"][0]["ms"], 176);
        assert_eq!(v["salidas"][0]["ip"], "62.238.17.121");
        assert_eq!(v["salidas"][1]["disponible"], false);
        assert_eq!(v["mas_rapida"], "hel");
        assert_eq!(v["medido"], 7);
        // la salida en la que está la VPN, con la dirección del túnel abierto: disponible si está conectada; si no
        // (reconectando, sin respuesta), lo medido (revisión de la 1.0.56: era disponible siempre)
        let otra: std::net::IpAddr = "168.119.225.147".parse().unwrap();
        let v = lista_json(Some(&medidas), Some(("nbg", Some(otra))), true);
        assert_eq!(v["salidas"][1]["disponible"], true);
        assert_eq!(v["salidas"][1]["ip"], "168.119.225.147");
        assert_eq!(v["salidas"][1]["ms"], Value::Null);
        let v = lista_json(Some(&medidas), Some(("nbg", Some(otra))), false);
        assert_eq!(v["salidas"][1]["disponible"], false);
        assert_eq!(v["salidas"][1]["ip"], "168.119.225.147");
        // con la VPN puesta y nada medido, solo las direcciones: ni disponible, ni retraso, ni la más rápida
        let solo = medidas.clone().sin_medir();
        let v = lista_json(Some(&solo), Some(("hel", None)), false);
        assert_eq!(v["salidas"][0]["ip"], "62.238.17.121");
        assert_eq!(v["salidas"][0]["disponible"], Value::Null);
        assert_eq!(v["salidas"][0]["ms"], Value::Null);
        assert_eq!(v["salidas"][1]["disponible"], Value::Null);
        assert_eq!(
            (v["mas_rapida"].is_null(), v["medido"].is_null()),
            (true, true)
        );
        // ni se pregunta a una salida «caída» por algo que no se midió
        assert_eq!(
            nombres_de(a_quien_preguntar(None, Some(&solo))),
            "hel,nbg,mia"
        );
        // a quién se pregunta qué dirección ve: con el túnel en Helsinki, a las otras disponibles; sin VPN, a todas las
        // que no se vieron caídas
        let hel = salidas::por_id("hel");
        assert_eq!(nombres_de(a_quien_preguntar(hel, None)), "nbg,mia");
        assert_eq!(nombres_de(a_quien_preguntar(None, Some(&medidas))), "hel");
        assert_eq!(nombres_de(a_quien_preguntar(hel, Some(&medidas))), "");
    }

    /// Los ids de unas salidas, separados por comas.
    fn nombres_de(v: Vec<&Salida>) -> String {
        v.into_iter().map(|s| s.id).collect::<Vec<_>>().join(",")
    }

    /// The Windows location permission, as `reg query` writes it.
    #[test]
    fn el_permiso_de_ubicacion_de_windows() {
        let reg = |v: &str| {
            format!(
                "\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\location\r\n    Value    REG_SZ    {v}\r\n\r\n"
            )
        };
        assert_eq!(permiso_de_reg(&reg("Allow")), "permitida");
        assert_eq!(permiso_de_reg(&reg("Deny")), "denegada");
        assert_eq!(permiso_de_reg(&reg("Prompt")), "desconocida");
        assert_eq!(
            permiso_de_reg(
                "ERROR: The system was unable to find the specified registry key or value."
            ),
            "desconocida"
        );
        assert_eq!(permiso_de_reg(""), "desconocida");
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
