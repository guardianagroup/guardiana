//! «Vigía» (1.0.48): el estado del vigía (`guardiana_apps::vigia`) para la web de Comunicaciones
//! y para el panel. El dueño, 30 Oct 2026: durante una llamada o un chat de GUARDIANA, que el
//! ordenador detecte señales de espionaje y la comunicación se pare sola, diciendo qué ha visto.
//!
//! El vigía corre dentro del servicio (el motor y el panel son el mismo proceso): mira cada
//! [`CADA_ACTIVO`] mientras alguien lo está preguntando (una llamada en curso) y cada
//! [`CADA_REPOSO`] si no, y deja su última mirada en memoria. Defender, que cuesta un PowerShell,
//! se mira cada minuto durante una llamada y cada diez minutos si no.
//!
//! `GET /vigia` contesta con esa mirada, en JSON y sin cambiar nada. Lo pregunta la web de
//! Comunicaciones (`https://guardianagroup.com`) desde el navegador de este mismo ordenador, y por
//! eso es la única respuesta del panel que se deja leer desde otra web. La decisión, punto por
//! punto (y en `docs/THREAT_MODEL.md`, C):
//!
//! - **Sin llave para la web.** La llave del panel vive en un archivo que la web no puede leer, y
//!   dársela sería darle el panel entero. Lo que se contesta sin llave es poco y nada sirve para
//!   cambiar algo: si hay señales, cuáles (nombre del programa y su archivo) y una frase. Ni rutas,
//!   ni cuentas, ni el equipo desde el que entra un escritorio remoto.
//! - **Solo a `https://guardianagroup.com`, exactamente.** Sin llave, la petición tiene que traer
//!   esa `Origin` y nada más (ni `http://`, ni subdominios, ni `null`): otra web recibe `403`, y
//!   una petición sin `Origin` (una etiqueta `<img>` o `<script>` de otra web, que el navegador
//!   manda sin ella) recibe `401`. Así otra web no puede leer qué programas tiene la persona, ni
//!   deducirlo por si la respuesta llega o no: le llega siempre el mismo error, haya lo que haya.
//! - **Solo desde este ordenador.** Sin llave, solo desde `127.0.0.1`: en Modo Hogar el panel
//!   escucha también en la Wi-Fi, y alguien de la red podría inventarse la `Origin`.
//! - **CORS mínimo.** `Access-Control-Allow-Origin` con ese origen exacto, `Vary: Origin`, sin
//!   `Access-Control-Allow-Credentials` (no hay cookies: la web pregunta con
//!   `credentials: 'omit'`), solo `GET`, ninguna cabecera propia permitida, y
//!   `Access-Control-Allow-Private-Network` solo en la consulta previa que lo pide (Chrome).
//! - **Con llave**, el panel ve lo mismo y algo más (la ruta de cada programa y, en una sesión de
//!   escritorio remoto, la cuenta y el equipo), desde cualquier dirección que ya podía abrir el
//!   panel.
//!
//! Un programa de este mismo ordenador puede preguntar sin llave inventándose la `Origin`: verá lo
//! mismo que la web, que es lo que ya podría ver por sí mismo en su mayor parte (la lista de
//! procesos, las sesiones); y de las demás cuentas de este ordenador, solo que algún programa usa
//! el micrófono, la cámara o la pantalla, que es justo la señal que se quiere dar.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;

use axum::extract::{ConnectInfo, Request, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use guardiana_apps::vigia::{self as vg, Como, Defensa, Idioma, Lectura, Llamante, Nivel, Propio};
use guardiana_core::time::now_ms;
use serde_json::{json, Value};

use crate::auth::constant_time_eq;
use crate::AppState;

/// La web de Comunicaciones: el único origen que puede leer `/vigia` sin llave.
pub(crate) const ORIGEN_WEB: &str = "https://guardianagroup.com";

/// Cada cuánto mira durante una llamada.
pub(crate) const CADA_ACTIVO: Duration = Duration::from_secs(3);
/// Cada cuánto mira cuando nadie pregunta.
pub(crate) const CADA_REPOSO: Duration = Duration::from_secs(20);
/// Cuánto dura «alguien está preguntando» tras la última pregunta.
const ACTIVO_MS: i64 = 120_000;
/// Defender: cada minuto durante una llamada, cada diez minutos si no, y la primera vez al minuto
/// de arrancar (o en cuanto alguien pregunte): el ordenador está ocupado al arrancar, y un panel
/// abierto un momento (las pruebas) no paga un PowerShell.
const DEFENSA_ACTIVO_MS: i64 = 60_000;
const DEFENSA_REPOSO_MS: i64 = 600_000;
const PRIMERA_DEFENSA_MS: i64 = 60_000;

/// Lo que el vigía guarda en memoria.
#[derive(Default)]
pub(crate) struct Vigia {
    /// Su última mirada.
    lectura: Mutex<Option<Lectura>>,
    /// Cuándo se le preguntó por última vez (ms Unix).
    pedido: AtomicI64,
    /// Alguien preguntó tras un rato sin hacerlo: que mire ya. Despierta al bucle en el acto
    /// (1.0.55): duerme su espera de una vez, en vez de despertarse cada segundo a ver.
    despierta: Arc<tokio::sync::Notify>,
    /// Una mirada lenta está en curso.
    defensa_corriendo: AtomicBool,
}

impl Vigia {
    /// Arranca el bucle, que vive mientras viva el panel. Fuera de Windows no hay nada que mirar.
    pub(crate) fn arrancar(state: &Arc<AppState>) {
        if !vg::DISPONIBLE {
            return;
        }
        let debil: Weak<AppState> = Arc::downgrade(state);
        tokio::spawn(async move {
            let arranque = now_ms();
            tokio::time::sleep(Duration::from_secs(1)).await;
            loop {
                let Some(st) = debil.upgrade() else { break };
                let s2 = Arc::clone(&st);
                let _ = tokio::task::spawn_blocking(move || mira_rapido(&s2)).await;
                let ahora = now_ms();
                let activo = ahora - st.vigia.pedido.load(Ordering::SeqCst) < ACTIVO_MS;
                let edad = st.vigia.lectura.lock().ok().and_then(|l| {
                    l.as_ref()
                        .and_then(|x| x.defensa.as_ref())
                        .map(|d| ahora - d.cuando)
                });
                let toca = match edad {
                    None => activo || ahora - arranque >= PRIMERA_DEFENSA_MS,
                    Some(e) if activo => e >= DEFENSA_ACTIVO_MS,
                    Some(e) => e >= DEFENSA_REPOSO_MS,
                };
                if toca {
                    lanza_defensa(&st);
                }
                let despierta = Arc::clone(&st.vigia.despierta);
                drop(st);
                let espera = if activo { CADA_ACTIVO } else { CADA_REPOSO };
                // Una pregunta tras un rato despierta al vigía en el acto, y al cerrar el panel el
                // bucle termina sin retenerlo (a la vuelta siguiente). Hasta la 1.0.55 se despertaba
                // cada segundo a ver si alguien había preguntado (el dueño, 2 nov 2026: «se me traba»).
                tokio::select! {
                    () = tokio::time::sleep(espera) => {}
                    () = despierta.notified() => {}
                }
            }
        });
    }
}

/// La mirada rápida, fuera del bucle asíncrono; la mirada lenta anterior se conserva.
fn mira_rapido(st: &AppState) {
    let nueva = vg::lee_rapido(now_ms());
    if let Ok(mut l) = st.vigia.lectura.lock() {
        let defensa = l.take().and_then(|x| x.defensa);
        *l = Some(Lectura { defensa, ..nueva });
    }
}

/// La mirada lenta, fuera del bucle, si no hay otra en curso. Si falla, queda anotado cuándo y
/// por qué: no se reintenta en cada vuelta. Mientras corre su PowerShell, el panel solo se guarda
/// débil (revisión de vpn-1055b): uno que el motor rehace mientras tanto no sigue vivo por ella.
fn lanza_defensa(st: &Arc<AppState>) {
    if st.vigia.defensa_corriendo.swap(true, Ordering::SeqCst) {
        return;
    }
    let debil = Arc::downgrade(st);
    tokio::task::spawn_blocking(move || {
        let ahora = now_ms();
        let d = vg::lee_defensa(ahora).unwrap_or_else(|e| {
            let mut d = Defensa {
                cuando: ahora,
                ..Defensa::default()
            };
            d.errores.insert("powershell".to_owned(), e.to_string());
            d
        });
        let Some(st) = debil.upgrade() else {
            return;
        };
        if let Ok(mut l) = st.vigia.lectura.lock() {
            if let Some(x) = l.as_mut() {
                x.defensa = Some(d);
            }
        }
        st.vigia.defensa_corriendo.store(false, Ordering::SeqCst);
    });
}

/// La carpeta de GUARDIANA, que nunca cuenta como señal.
fn carpeta() -> Option<String> {
    static CARPETA: OnceLock<Option<String>> = OnceLock::new();
    CARPETA.get_or_init(vg::carpeta_propia).clone()
}

// ---------------------------------------------------------------------------------------------
// Quién pregunta, y qué se le contesta.

/// A quién se contesta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Acceso {
    /// Con la llave del panel: todo, con rutas.
    Panel,
    /// La web de Comunicaciones, sin llave: lo justo.
    Web,
}

/// Quién puede leer el vigía: con llave, el panel; sin llave, solo la web de Comunicaciones y
/// solo desde este ordenador. Otra web, `403`; sin `Origin` ni llave, `401`.
pub(crate) fn decide(
    con_llave: bool,
    origen: Option<&str>,
    local: bool,
) -> Result<Acceso, StatusCode> {
    if con_llave {
        return Ok(Acceso::Panel);
    }
    match origen {
        Some(o) if o == ORIGEN_WEB && local => Ok(Acceso::Web),
        Some(_) => Err(StatusCode::FORBIDDEN),
        None => Err(StatusCode::UNAUTHORIZED),
    }
}

/// Lo que importa de una petición, copiado: nada de ella se guarda durante la espera.
struct Pregunta {
    cliente: Option<SocketAddr>,
    origen: Option<String>,
    agente: String,
    idioma: Idioma,
    con_llave: bool,
}

fn cabecera<'a>(h: &'a HeaderMap, nombre: &str) -> Option<&'a str> {
    h.get(nombre).and_then(|v| v.to_str().ok())
}

fn de_consulta<'a>(req: &'a Request, clave: &str) -> Option<&'a str> {
    req.uri().query().and_then(|q| {
        q.split('&').find_map(|kv| {
            kv.split_once('=')
                .filter(|(k, _)| *k == clave)
                .map(|(_, v)| v)
        })
    })
}

fn pregunta(req: &Request, token: &str) -> Pregunta {
    let h = req.headers();
    // La llave como en el resto del panel: en la cabecera o en `?t=`.
    let dada = cabecera(h, "x-guardiana-token")
        .or_else(|| de_consulta(req, "t"))
        .unwrap_or_default();
    // El idioma: `?idioma=` (la web), la elección del panel, o lo que dice el navegador.
    let idioma = de_consulta(req, "idioma")
        .or_else(|| cabecera(h, "x-guardiana-lang"))
        .or_else(|| cabecera(h, "accept-language"))
        .map(Idioma::de)
        .unwrap_or_default();
    Pregunta {
        cliente: req
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|c| c.0),
        origen: cabecera(h, "origin").map(str::to_owned),
        agente: cabecera(h, "user-agent")
            .unwrap_or_default()
            .chars()
            .take(400)
            .collect(),
        idioma,
        con_llave: !dada.is_empty() && constant_time_eq(dada.as_bytes(), token.as_bytes()),
    }
}

fn es_local(cliente: Option<SocketAddr>) -> bool {
    cliente.is_some_and(|c| c.ip().is_loopback())
}

/// Las cabeceras CORS de la web de Comunicaciones: su origen exacto y nada más.
fn pon_cors(h: &mut HeaderMap) {
    h.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static(ORIGEN_WEB),
    );
}

/// `GET /vigia`: lo que el vigía ve ahora. Ver el principio de este archivo para quién puede.
pub(crate) async fn ver(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let p = pregunta(&req, &state.token);
    drop(req);
    let local = es_local(p.cliente);
    let acceso = match decide(p.con_llave, p.origen.as_deref(), local) {
        Ok(a) => a,
        Err(codigo) => {
            let mut res = codigo.into_response();
            res.headers_mut()
                .insert(header::VARY, HeaderValue::from_static("Origin"));
            return res;
        }
    };
    let ahora = now_ms();
    let antes = state.vigia.pedido.swap(ahora, Ordering::SeqCst);
    if ahora - antes >= ACTIVO_MS {
        state.vigia.despierta.notify_one();
    }
    // El navegador que pregunta: por la conexión (Windows dice qué programa la abrió) y, si no se
    // pudo, por cómo se presenta.
    let por_conexion = match p.cliente {
        Some(c) if local => tokio::task::spawn_blocking(move || vg::llamante_de(c))
            .await
            .ok()
            .flatten(),
        _ => None,
    };
    let llamante = por_conexion.or_else(|| {
        vg::navegador_de_agente(&p.agente).map(|n| Llamante {
            nombre: n.to_owned(),
            ruta: None,
            como: Como::Presentacion,
        })
    });
    let propio = Propio {
        carpeta: carpeta(),
        llamante,
    };
    let lectura = state.vigia.lectura.lock().ok().and_then(|l| l.clone());
    let respuesta = cuerpo(lectura.as_ref(), &propio, p.idioma, acceso == Acceso::Panel);
    let mut res = Json(respuesta).into_response();
    let h = res.headers_mut();
    h.insert(header::VARY, HeaderValue::from_static("Origin"));
    if p.origen.as_deref() == Some(ORIGEN_WEB) {
        pon_cors(h);
    }
    res
}

/// `OPTIONS /vigia`: la consulta previa que Chrome hace antes de dejar a una web pública hablar
/// con este ordenador. Solo para la web de Comunicaciones, solo `GET`, ninguna cabecera propia.
pub(crate) async fn previa(req: Request) -> Response {
    let cliente = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0);
    let h = req.headers();
    if cabecera(h, "origin") != Some(ORIGEN_WEB) || !es_local(cliente) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let red_privada = cabecera(h, "access-control-request-private-network")
        .is_some_and(|v| v.eq_ignore_ascii_case("true"));
    let mut res = StatusCode::NO_CONTENT.into_response();
    let r = res.headers_mut();
    pon_cors(r);
    r.insert(header::VARY, HeaderValue::from_static("Origin"));
    r.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET"),
    );
    r.insert(
        header::ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static("600"),
    );
    if red_privada {
        r.insert(
            HeaderName::from_static("access-control-allow-private-network"),
            HeaderValue::from_static("true"),
        );
    }
    res
}

/// La respuesta. `detallado` (con llave) añade la ruta de cada programa, la cuenta y el equipo de
/// una sesión remota y lo que falló al leer Defender.
pub(crate) fn cuerpo(
    lectura: Option<&Lectura>,
    propio: &Propio,
    idioma: Idioma,
    detallado: bool,
) -> Value {
    let mut alertas: Vec<Value> = Vec::new();
    let mut avisos: Vec<Value> = Vec::new();
    for h in lectura.map(|l| vg::analiza(l, propio)).unwrap_or_default() {
        let mut o = json!({
            "tipo": h.tipo.como_texto(),
            "programa": h.programa(idioma),
            "archivo": h.clave.clone(),
            "texto": h.texto(idioma, detallado),
        });
        if let Some(d) = h.desde {
            o["desde"] = json!(d);
        }
        if detallado {
            if let Some(r) = &h.ruta {
                o["ruta"] = json!(r);
            }
        }
        match h.nivel {
            Nivel::Alerta => alertas.push(o),
            Nivel::Aviso => avisos.push(o),
        }
    }
    let estado = match lectura {
        None if vg::DISPONIBLE => "arrancando",
        None => "no_disponible",
        Some(_) if alertas.is_empty() => "limpio",
        Some(_) => "alerta",
    };
    let defensa = lectura.and_then(|l| l.defensa.as_ref());
    let mut v = json!({
        "vigia": 1,
        "estado": estado,
        "vigilando": lectura.is_some(),
        "hay_alertas": !alertas.is_empty(),
        "alertas": alertas,
        "avisos": avisos,
        "mirado": lectura.map(|l| l.cuando),
        "defender_mirado": defensa.map(|d| d.cuando),
        "cada_s": CADA_ACTIVO.as_secs(),
        "excluido": propio.llamante.as_ref().map(|l| l.nombre.clone()),
        "idioma": idioma.codigo(),
        "limites": vg::limites(idioma),
    });
    if detallado {
        v["errores"] = json!(defensa.map(|d| d.errores.clone()).unwrap_or_default());
    }
    v
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use guardiana_apps::vigia::{Proceso, Sesion};

    #[test]
    fn sin_llave_solo_la_web_de_comunicaciones_y_solo_desde_este_ordenador() {
        assert_eq!(decide(true, None, false), Ok(Acceso::Panel));
        assert_eq!(
            decide(true, Some("https://otra.example"), true),
            Ok(Acceso::Panel)
        );
        assert_eq!(decide(false, Some(ORIGEN_WEB), true), Ok(Acceso::Web));
        // Desde la Wi-Fi (Modo Hogar), una Origin se inventa: no.
        assert_eq!(
            decide(false, Some(ORIGEN_WEB), false),
            Err(StatusCode::FORBIDDEN)
        );
        for otra in [
            "http://guardianagroup.com",
            "https://www.guardianagroup.com",
            "https://guardianagroup.com.otra.example",
            "https://guardianagroup.com:8443",
            "https://GUARDIANAGROUP.COM",
            "null",
            "",
        ] {
            assert_eq!(
                decide(false, Some(otra), true),
                Err(StatusCode::FORBIDDEN),
                "{otra}"
            );
        }
        assert_eq!(decide(false, None, true), Err(StatusCode::UNAUTHORIZED));
    }

    fn lectura() -> Lectura {
        let ft = |ms: u64| (ms + 11_644_473_600_000) * 10_000;
        let uso = |clave: &str, tipo| {
            vg::de_registro(clave, tipo, ft(1_000_000), 0, "S-1-5-21-9").unwrap()
        };
        Lectura {
            cuando: 2_000_000,
            usos: Some(vec![
                uso("C:#Users#zuleima#espia.exe", vg::Tipo::Microfono),
                uso(
                    "C:#Program Files#Google#Chrome#Application#chrome.exe",
                    vg::Tipo::Camara,
                ),
            ]),
            procesos: Some(vec![
                Proceso {
                    pid: 1,
                    nombre: "espia.exe".to_owned(),
                    sesion: None,
                },
                Proceso {
                    pid: 2,
                    nombre: "chrome.exe".to_owned(),
                    sesion: None,
                },
            ]),
            sesiones: Some(vec![Sesion {
                id: 2,
                estado: 0,
                protocolo: 2,
                puesto: "RDP-Tcp#0".to_owned(),
                usuario: "zuleima".to_owned(),
                cliente: "PORTATIL-ZX".to_owned(),
            }]),
            defensa: None,
        }
    }

    fn chrome() -> Propio {
        Propio {
            carpeta: None,
            llamante: Some(Llamante {
                nombre: "chrome.exe".to_owned(),
                ruta: Some("C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe".to_owned()),
                como: Como::Conexion,
            }),
        }
    }

    #[test]
    fn la_web_recibe_lo_justo_y_el_panel_algo_mas() {
        let l = lectura();
        let web = cuerpo(Some(&l), &chrome(), Idioma::Es, false);
        assert_eq!(web["estado"], "alerta");
        assert_eq!(web["hay_alertas"], true);
        assert_eq!(web["excluido"], "chrome.exe");
        let a = web["alertas"].as_array().unwrap();
        assert_eq!(a.len(), 2, "{web}");
        assert_eq!(a[0]["tipo"], "microfono");
        assert_eq!(a[0]["programa"], "espia");
        assert_eq!(a[0]["archivo"], "espia.exe");
        assert_eq!(a[0]["texto"], "espia está usando el micrófono.");
        assert_eq!(a[1]["tipo"], "escritorio_remoto");
        let texto = web.to_string();
        // Ni rutas, ni cuentas, ni el equipo remoto, ni lo que falló.
        for nada in [
            "\"ruta\"",
            "zuleima",
            "PORTATIL-ZX",
            "S-1-5-21",
            "\"errores\"",
        ] {
            assert!(!texto.contains(nada), "{nada}: {texto}");
        }
        let panel = cuerpo(Some(&l), &chrome(), Idioma::En, true);
        assert_eq!(panel["alertas"][0]["ruta"], "C:\\Users\\zuleima\\espia.exe");
        assert_eq!(
            panel["alertas"][1]["texto"],
            "A Remote Desktop session is open on this computer (account zuleima), from the computer PORTATIL-ZX."
        );
        assert!(panel["errores"].is_object());
        // Sin mirada todavía: no dice «limpio».
        let nada = cuerpo(None, &chrome(), Idioma::Pt, false);
        assert_eq!(nada["hay_alertas"], false);
        assert_eq!(nada["vigilando"], false);
        assert_ne!(nada["estado"], "limpio");
        assert_eq!(nada["idioma"], "pt");
    }

    #[test]
    fn solo_el_origen_exacto_va_en_cors() {
        let mut h = HeaderMap::new();
        pon_cors(&mut h);
        assert_eq!(
            h.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(),
            "https://guardianagroup.com"
        );
        assert!(h.get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS).is_none());
        assert_eq!(h.len(), 1);
    }
}
