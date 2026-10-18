//! Windows half of «what each program sends and receives» (1.0.21).
//!
//! Windows publishes, on its kernel network event channel (`Microsoft-Windows-Kernel-Network`),
//! one event per TCP or UDP packet a program sends or receives: the process id, the size and the
//! two ends. Here that channel is heard and added up in memory, per program and address at the
//! other end, until the service takes the totals (every few seconds) and writes them per minute.
//! Nothing inside the packets is read: the channel does not even carry it.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};

use ferrisetw::parser::Parser;
use ferrisetw::provider::Provider;
use ferrisetw::schema_locator::SchemaLocator;
use ferrisetw::trace::UserTrace;
use ferrisetw::EventRecord;

use super::{Error, Medida, Proceso};
use crate::firmas::Firmas;
use crate::ventanas::{huella_de, programa_de};

/// `Microsoft-Windows-Kernel-Network`.
const PROVEEDOR: &str = "7dd42a49-5329-4832-8dfd-43d979153a88";
/// Its keywords for IPv4 (0x10) and IPv6 (0x20) traffic.
const PALABRAS: u64 = 0x30;
/// Fixed name, to close it if a crash left it open (as with the DNS session).
const SESION: &str = "Guardiana-Red";
const SESION_DIAGNOSTICO: &str = "Guardiana-Red-Diagnostico";

/// What an event of the channel counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Que {
    /// Data sent.
    Envia,
    /// Data received.
    Recibe,
    /// Data sent again (a TCP retransmission). 1.0.45: counted apart, never as more data sent.
    Reenvia,
    /// A connection opened or accepted. 1.0.45.
    Conecta,
}

/// What each event number is: (protocol, what it counts).
///
/// The numbers are the manifest's (`Microsoft-Windows-Kernel-Network`, the same as the classic
/// kernel logger's TcpIp/UdpIp types): 10/26 send, 11/27 receive, 12/28 connect, 14/30
/// retransmit, 15/31 accept, IPv4/IPv6; UDP 42/58 send, 43/59 receive. A retransmission is its
/// own event, so its bytes go apart and are not added to what event 10 counted;
/// `guardiana apps --red` shows the raw numbers, to check this on a real machine.
fn que_es(id: u16) -> Option<(&'static str, Que)> {
    match id {
        10 | 26 => Some(("tcp", Que::Envia)),
        11 | 27 => Some(("tcp", Que::Recibe)),
        12 | 28 | 15 | 31 => Some(("tcp", Que::Conecta)),
        14 | 30 => Some(("tcp", Que::Reenvia)),
        42 | 58 => Some(("udp", Que::Envia)),
        43 | 59 => Some(("udp", Que::Recibe)),
        _ => None,
    }
}

/// How long what is known about a process id is trusted before asking Windows again.
const VIGENCIA_MS: i64 = 5_000;
/// The most (program, address) pairs kept between two takes.
const MAXIMO_PARES: usize = 50_000;

type Clave = (String, String, IpAddr, u16, &'static str);

/// What one (program, address) pair added up between two takes.
#[derive(Default, Clone, Copy)]
struct Cuenta {
    enviados: u64,
    recibidos: u64,
    paquetes: u64,
    reenviados: u64,
    conexiones: u64,
}

#[derive(Default)]
struct Compartido {
    /// (program name, path, address, port, protocol) → its counts.
    suma: HashMap<Clave, Cuenta>,
    /// Process id → (name, path, when looked at).
    rutas: HashMap<u32, (String, String, i64)>,
    huellas: HashMap<String, String>,
}

/// The network session running. Dropping it closes it.
pub(crate) struct Sesion {
    datos: Arc<Mutex<Compartido>>,
    firmas: Firmas,
    _traza: UserTrace,
}

impl Sesion {
    pub(crate) fn arrancar() -> Result<Self, Error> {
        let datos: Arc<Mutex<Compartido>> = Arc::default();
        let para_callback = Arc::clone(&datos);
        let proveedor = Provider::by_guid(PROVEEDOR)
            .any(PALABRAS)
            .add_callback(move |registro: &EventRecord, esquemas: &SchemaLocator| {
                anotar(&para_callback, registro, esquemas);
            })
            .build();
        let _ = ferrisetw::trace::stop_trace_by_name(SESION);
        let traza = UserTrace::new()
            .named(SESION.to_owned())
            .enable(proveedor)
            .start_and_process()
            .map_err(|e| Error::Sistema(format!("{e:?}")))?;
        Ok(Self {
            datos,
            firmas: Firmas::arrancar(),
            _traza: traza,
        })
    }

    /// The totals since the last take, and starts again from zero.
    pub(crate) fn toma(&self) -> Vec<Medida> {
        let (suma, huellas) = {
            let Ok(mut d) = self.datos.lock() else {
                return Vec::new();
            };
            if d.huellas.len() > 4_096 {
                d.huellas.clear();
            }
            (std::mem::take(&mut d.suma), d.huellas.clone())
        };
        suma.into_iter()
            .map(|((nombre, ruta, ip, puerto, proto), c)| Medida {
                proceso: Proceso {
                    pid: 0,
                    firmado_por: self.firmas.de(&ruta),
                    sha256: huellas.get(&ruta).cloned().unwrap_or_default(),
                    nombre,
                    ruta,
                },
                ip,
                puerto,
                proto,
                enviados: c.enviados,
                recibidos: c.recibidos,
                paquetes: c.paquetes,
                reenviados: c.reenviados,
                conexiones: c.conexiones,
            })
            .collect()
    }
}

fn anotar(datos: &Arc<Mutex<Compartido>>, registro: &EventRecord, esquemas: &SchemaLocator) {
    let Some((proto, que)) = que_es(registro.event_id()) else {
        return;
    };
    let Ok(esquema) = esquemas.event_schema(registro) else {
        return;
    };
    let lector = Parser::create(registro, &esquema);
    let (Ok(pid), Ok(ip), Ok(puerto)) = (
        lector.try_parse::<u32>("PID"),
        lector.try_parse::<IpAddr>("daddr"),
        lector.try_parse::<u16>("dport"),
    ) else {
        return;
    };
    // A connection counts without a size; data without one is not counted at all.
    let tamano = match (lector.try_parse::<u32>("size"), que) {
        (Ok(t), _) => t,
        (Err(_), Que::Conecta) => 0,
        (Err(_), _) => return,
    };
    // The port travels in network order.
    let puerto = u16::from_be(puerto);
    if ip.is_loopback() || ip.is_unspecified() {
        return;
    }
    let ahora = guardiana_core::time::now_ms();
    let Ok(mut d) = datos.lock() else {
        return;
    };
    let (nombre, ruta) = match d.rutas.get(&pid) {
        Some((n, r, visto)) if ahora - *visto < VIGENCIA_MS => (n.clone(), r.clone()),
        _ => {
            if d.rutas.len() > 4_096 {
                d.rutas.clear();
            }
            let (n, r) = if pid == 4 {
                ("System".to_owned(), String::new())
            } else {
                programa_de(pid)
            };
            d.rutas.insert(pid, (n.clone(), r.clone(), ahora));
            if !r.is_empty() && !d.huellas.contains_key(&r) {
                let h = huella_de(&r);
                d.huellas.insert(r.clone(), h);
            }
            (n, r)
        }
    };
    if nombre.is_empty() {
        return;
    }
    let lleno = d.suma.len() >= MAXIMO_PARES;
    let clave = (nombre, ruta, ip, puerto, proto);
    if lleno && !d.suma.contains_key(&clave) {
        return;
    }
    let e = d.suma.entry(clave).or_default();
    let t = u64::from(tamano);
    match que {
        Que::Envia => {
            e.enviados += t;
            e.paquetes += 1;
        }
        Que::Recibe => {
            e.recibidos += t;
            e.paquetes += 1;
        }
        Que::Reenvia => e.reenviados += t,
        Que::Conecta => e.conexiones += 1,
    }
}

/// One raw network event seen by the diagnostic: event number, process id, size, and the two
/// ends as text.
type FilaDiag = (u16, u32, u32, String);

/// Hears the channel for a few seconds and returns what came, raw: event number, process id,
/// size, the address and port read as the far end. To check on the machine what the
/// documentation promises (`guardiana apps --red`).
pub(crate) fn diagnostico(segundos: u64) -> Result<Vec<FilaDiag>, Error> {
    let vistos: Arc<Mutex<Vec<FilaDiag>>> = Arc::default();
    let para_callback = Arc::clone(&vistos);
    let proveedor = Provider::by_guid(PROVEEDOR)
        .any(PALABRAS)
        .add_callback(move |registro: &EventRecord, esquemas: &SchemaLocator| {
            let id = registro.event_id();
            let (pid, tamano, otro) = esquemas
                .event_schema(registro)
                .ok()
                .map(|e| {
                    let p = Parser::create(registro, &e);
                    (
                        p.try_parse::<u32>("PID").unwrap_or_default(),
                        p.try_parse::<u32>("size").unwrap_or_default(),
                        format!(
                            "{}:{} <- {}:{}",
                            p.try_parse::<IpAddr>("daddr")
                                .map(|i| i.to_string())
                                .unwrap_or_default(),
                            p.try_parse::<u16>("dport").map(u16::from_be).unwrap_or(0),
                            p.try_parse::<IpAddr>("saddr")
                                .map(|i| i.to_string())
                                .unwrap_or_default(),
                            p.try_parse::<u16>("sport").map(u16::from_be).unwrap_or(0),
                        ),
                    )
                })
                .unwrap_or_default();
            if let Ok(mut v) = para_callback.lock() {
                if v.len() < 2_048 {
                    v.push((id, pid, tamano, otro));
                }
            }
        })
        .build();
    let _ = ferrisetw::trace::stop_trace_by_name(SESION_DIAGNOSTICO);
    let _traza = UserTrace::new()
        .named(SESION_DIAGNOSTICO.to_owned())
        .enable(proveedor)
        .start_and_process()
        .map_err(|e| Error::Sistema(format!("{e:?}")))?;
    std::thread::sleep(std::time::Duration::from_secs(segundos));
    Ok(vistos.lock().map(|v| v.clone()).unwrap_or_default())
}
