//! «Por dónde viajan tus datos», medido (1.0.51). The owner, 31 Oct 2026, 08:53: «quiero que
//! intentes detectar… por dónde viaja, si viaja por cable, qué cable, si viaja por satélite, más o
//! menos la zona… lo más preciso posible».
//!
//! On request (and only then), the panel traces the route from this computer to the places it
//! talked to most in the last 24 hours, with Windows' own `tracert`: each hop with the country,
//! city and network the tables inside the program give its address. The addresses themselves are
//! never kept nor handed to any page; the house and the operator's private part are marked as
//! private, with no place. Which cable carried a hop across the sea is inferred by the page from
//! the map (the landing points on both shores), and said as probable, never as certain.

// The handlers answer with axum's `Response` as their error, like the rest of the panel.
#![allow(clippy::result_large_err)]

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use axum::extract::State;
use axum::response::Response;
use axum::Json;
use guardiana_core::time::{now_ms, HOUR_MS};
use serde::Serialize;

use crate::api::{with_ledger, ApiResult};
use crate::auth::Session;
use crate::AppState;

/// How many places are traced: the ones that received the most, one per company.
const DESTINOS: usize = 6;
/// At most this many hops, this long for each answer, and the whole trace stopped after this.
const SALTOS: u8 = 22;
const ESPERA_MS: u32 = 700;
const LIMITE: Duration = Duration::from_secs(70);

/// What the page gets: the routes measured last, or that they are being measured.
#[derive(Serialize, Clone, Default)]
pub(crate) struct Respuesta {
    disponible: bool,
    corriendo: bool,
    cuando: Option<i64>,
    rutas: Vec<Ruta>,
    /// Set when the way out of this computer is a satellite: by the network of the first hop on
    /// the internet (`red`) or by its delay (`latencia`, a geostationary one adds half a second).
    satelite: Option<Satelite>,
}

#[derive(Serialize, Clone)]
struct Ruta {
    /// The name the computer asked for (`www.youtube.com`).
    destino: String,
    empresa: String,
    pais: String,
    /// Whether the last hop is the place itself (many servers do not answer the trace).
    llega: bool,
    saltos: Vec<Salto>,
}

#[derive(Serialize, Clone, Default)]
struct Salto {
    n: u32,
    /// Best of the three times, ms.
    ms: Option<u32>,
    /// Something answered at this hop.
    responde: bool,
    /// The house or the operator's private part: no place is given.
    privado: bool,
    anycast: bool,
    pais: Option<String>,
    ciudad: Option<String>,
    region: Option<String>,
    lat: Option<f64>,
    lon: Option<f64>,
    red: String,
    asn: u32,
}

#[derive(Serialize, Clone)]
struct Satelite {
    operador: String,
    por: &'static str,
}

/// Networks whose name says they are satellite operators.
const SATELITE: &[&str] = &[
    "starlink",
    "space exploration",
    "spacex",
    "viasat",
    "hughes",
    "oneweb",
    "eutelsat",
    "kacific",
    "telesat",
    "inmarsat",
    "avanti",
    "skylogic",
    "echostar",
    "intelsat",
];

fn estado() -> &'static Mutex<Respuesta> {
    static E: OnceLock<Mutex<Respuesta>> = OnceLock::new();
    E.get_or_init(|| Mutex::new(Respuesta::default()))
}

fn respuesta() -> Respuesta {
    let mut r = estado().lock().map(|g| g.clone()).unwrap_or_default();
    r.disponible = cfg!(windows);
    r
}

/// `GET /api/ruta`: the routes measured last.
pub(crate) async fn ver(State(_state): State<Arc<AppState>>, _s: Session) -> ApiResult<Respuesta> {
    Ok(Json(respuesta()))
}

/// `POST /api/ruta/ahora`: measure them now, in the background, unless it is already being done.
pub(crate) async fn ahora(State(state): State<Arc<AppState>>, _s: Session) -> ApiResult<Respuesta> {
    if cfg!(windows) {
        let ocupado = estado().lock().map(|g| g.corriendo).unwrap_or(true);
        if !ocupado {
            let destinos = principales(&state)?;
            if let Ok(mut g) = estado().lock() {
                g.corriendo = true;
            }
            tokio::task::spawn_blocking(move || {
                let rutas = mide(&destinos);
                let satelite = satelite_de(&rutas);
                if let Ok(mut g) = estado().lock() {
                    g.corriendo = false;
                    g.cuando = Some(now_ms());
                    g.rutas = rutas;
                    g.satelite = satelite;
                }
            });
        }
    }
    Ok(Json(respuesta()))
}

/// The places that received the most from this computer in the last 24 hours, one per company.
fn principales(state: &AppState) -> Result<Vec<(String, String, String)>, Response> {
    let now = now_ms();
    let filas = with_ledger(state, |l| l.trafico(now - 24 * HOUR_MS, now + 1, &[]))?;
    let mut por: HashMap<String, (u64, String, String)> = HashMap::new();
    for f in filas {
        if f.destino.is_empty() {
            continue;
        }
        let e = por.entry(f.destino.clone()).or_insert_with(|| {
            (
                0,
                crate::vision::empresa_de(&f.destino, f.asn, &f.red),
                f.pais.clone(),
            )
        });
        e.0 += f.enviados + f.recibidos;
    }
    let mut v: Vec<(String, (u64, String, String))> = por.into_iter().collect();
    v.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then_with(|| a.0.cmp(&b.0)));
    let mut empresas = HashSet::new();
    Ok(v.into_iter()
        .filter(|(_, (_, e, _))| e.is_empty() || empresas.insert(e.clone()))
        .take(DESTINOS)
        .map(|(d, (_, e, p))| (d, e, p))
        .collect())
}

fn mide(destinos: &[(String, String, String)]) -> Vec<Ruta> {
    std::thread::scope(|s| {
        let hilos: Vec<_> = destinos
            .iter()
            .map(|(d, e, p)| s.spawn(move || ruta(d, e, p)))
            .collect();
        hilos
            .into_iter()
            .filter_map(|h| h.join().ok().flatten())
            .collect()
    })
}

fn ruta(destino: &str, empresa: &str, pais: &str) -> Option<Ruta> {
    let ip = (destino, 443)
        .to_socket_addrs()
        .ok()?
        .map(|a| a.ip())
        .find(IpAddr::is_ipv4)?;
    // A name GUARDIANA cut answers nothing real: there is no route to trace.
    if ip.is_unspecified() || ip.is_loopback() {
        return None;
    }
    let hops = guardiana_apps::traza(ip, SALTOS, ESPERA_MS, LIMITE).ok()?;
    let llega = hops.iter().any(|(_, x, _)| *x == Some(ip));
    Some(Ruta {
        destino: destino.to_owned(),
        empresa: empresa.to_owned(),
        pais: pais.to_owned(),
        llega,
        saltos: hops.into_iter().map(|(n, x, ms)| salto(n, x, ms)).collect(),
    })
}

fn salto(n: u32, ip: Option<IpAddr>, ms: Option<u32>) -> Salto {
    let mut s = Salto {
        n,
        ms,
        ..Salto::default()
    };
    let Some(ip) = ip else {
        return s;
    };
    s.responde = true;
    let sv = guardiana_lists::servidor_de(ip);
    if sv.local {
        s.privado = true;
        return s;
    }
    s.anycast = sv.anycast;
    if let Some(r) = sv.red {
        r.nombre.clone_into(&mut s.red);
        s.asn = r.asn;
    }
    let u = guardiana_lists::ubicacion::ubicacion_de_ip(ip);
    s.pais = u.pais.or_else(|| sv.pais.map(str::to_owned));
    s.ciudad = u.ciudad.or(u.cerca_de);
    s.region = u.region;
    s.lat = u.lat;
    s.lon = u.lon;
    s
}

fn satelite_de(rutas: &[Ruta]) -> Option<Satelite> {
    let primeros: Vec<&Salto> = rutas
        .iter()
        .filter_map(|r| r.saltos.iter().find(|s| s.responde && !s.privado))
        .collect();
    if let Some(s) = primeros.iter().find(|s| {
        let red = s.red.to_ascii_lowercase();
        SATELITE.iter().any(|x| red.contains(x))
    }) {
        return Some(Satelite {
            operador: s.red.clone(),
            por: "red",
        });
    }
    // The fastest first hop of all the routes: a geostationary satellite adds about half a second
    // even there, a cable never does.
    let minimo = primeros.iter().filter_map(|s| s.ms).min()?;
    if minimo > 450 {
        Some(Satelite {
            operador: String::new(),
            por: "latencia",
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(red: &str, ms: u32) -> Salto {
        Salto {
            n: 3,
            ms: Some(ms),
            responde: true,
            red: red.to_owned(),
            ..Salto::default()
        }
    }

    fn r(saltos: Vec<Salto>) -> Ruta {
        Ruta {
            destino: "x.example".into(),
            empresa: String::new(),
            pais: String::new(),
            llega: false,
            saltos,
        }
    }

    #[test]
    fn a_satellite_is_told_by_its_network_or_its_delay() {
        let casa = Salto {
            n: 1,
            ms: Some(1),
            responde: true,
            privado: true,
            ..Salto::default()
        };
        let cable = vec![r(vec![casa.clone(), s("Telmex Colombia", 9)])];
        assert!(satelite_de(&cable).is_none());
        let starlink = vec![r(vec![
            casa.clone(),
            s("SpaceX Services, Inc. (Starlink)", 38),
        ])];
        assert_eq!(satelite_de(&starlink).map(|x| x.por), Some("red"));
        let geo = vec![
            r(vec![casa.clone(), s("Operador", 620)]),
            r(vec![casa, s("Operador", 640)]),
        ];
        assert_eq!(satelite_de(&geo).map(|x| x.por), Some("latencia"));
    }
}
