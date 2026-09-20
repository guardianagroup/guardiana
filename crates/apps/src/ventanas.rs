//! La mitad de Windows: la sesión de sucesos y la lista corta de lo que oyó.
//!
//! Windows publica cada consulta DNS de su propio resolutor en el canal
//! `Microsoft-Windows-DNS-Client`, con el nombre pedido y, en la cabecera del suceso, el proceso
//! que lo pidió. Aquí se escucha ese canal, se traduce el número de proceso a un programa y se
//! guarda en memoria unos segundos, hasta que el resolutor de Guardiana anote esa misma consulta.
//! No se guarda nada en disco desde aquí, no se lee ningún contenido y no se corta nada.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use ferrisetw::parser::Parser;
use ferrisetw::provider::Provider;
use ferrisetw::schema_locator::SchemaLocator;
use ferrisetw::trace::UserTrace;
use ferrisetw::EventRecord;
use sha2::{Digest, Sha256};

use super::{Error, Proceso, MAXIMO, VENTANA_MS};

/// `Microsoft-Windows-DNS-Client`: por aquí pasa toda consulta que resuelve Windows.
const PROVEEDOR: &str = "1c95126e-7eea-49a9-a3fe-a378b03ddb4d";
/// «DNS query is called»: lleva `QueryName` y, en la cabecera, quién lo pidió.
///
/// El brief (§12.3) pide comprobar el identificador en la máquina antes de fiarse, y por eso
/// existe `guardiana apps --escuchar`: enseña en vivo lo que llega, con su número de suceso.
const EVENTO_CONSULTA: u16 = 3006;

/// Nombre de la sesión de sucesos. Es fijo a propósito, para poder cerrarla si quedó abierta.
const SESION: &str = "Guardiana-DNS-Client";
/// La del diagnóstico, aparte, para que una cosa no pise la otra.
const SESION_DIAGNOSTICO: &str = "Guardiana-DNS-Diagnostico";

/// Las sesiones de sucesos de Windows **sobreviven al programa que las abrió**: si Guardiana se
/// cierra de golpe, la suya se queda encendida y el siguiente arranque falla con «ya existe» —y
/// se queda sin saber qué programa pide cada nombre, en silencio y para siempre, hasta reiniciar.
/// Pasó en la primera prueba de punta a punta (20 sep 2026). Así que antes de abrir la suya,
/// Guardiana cierra la que pudiera haber quedado con ese nombre, que solo puede ser suya.
fn cerrar_la_que_quedo(nombre: &str) {
    let _ = ferrisetw::trace::stop_trace_by_name(nombre);
}

/// Una consulta oída, esperando a que el resolutor anote la suya.
struct Oida {
    ts: i64,
    qname: String,
    proceso: Proceso,
}

/// Lo que se comparte entre el hilo de la sesión y quien pregunta.
#[derive(Default)]
struct Compartido {
    cola: VecDeque<Oida>,
    /// Número de proceso → (nombre, ruta). Windows reutiliza los números, así que esto es una
    /// ayuda para no abrir el mismo proceso mil veces, no una identidad: se limpia con la cola.
    rutas: HashMap<u32, (String, String)>,
    /// Ruta → huella del archivo. Calcular un SHA-256 de 10 MB por consulta sería absurdo.
    huellas: HashMap<String, String>,
}

/// La sesión de sucesos en marcha. Al soltarla, se cierra.
pub(crate) struct Sesion {
    datos: Arc<Mutex<Compartido>>,
    _traza: UserTrace,
}

impl Sesion {
    pub(crate) fn arrancar() -> Result<Self, Error> {
        let datos: Arc<Mutex<Compartido>> = Arc::default();
        let para_callback = Arc::clone(&datos);
        let proveedor = Provider::by_guid(PROVEEDOR)
            .add_callback(move |registro: &EventRecord, esquemas: &SchemaLocator| {
                anotar(&para_callback, registro, esquemas);
            })
            .build();
        cerrar_la_que_quedo(SESION);
        let traza = UserTrace::new()
            .named(SESION.to_owned())
            .enable(proveedor)
            .start_and_process()
            .map_err(|e| Error::Sistema(format!("{e:?}")))?;
        Ok(Self {
            datos,
            _traza: traza,
        })
    }

    pub(crate) fn quien_pidio(&self, qname: &str, ts_ms: i64) -> Option<Proceso> {
        let buscado = qname.trim_end_matches('.').to_ascii_lowercase();
        let Ok(mut d) = self.datos.lock() else {
            return None;
        };
        limpiar(&mut d, ts_ms);
        // El más cercano en el tiempo, y **no se consume**: una sola llamada de un programa hace
        // que Windows pregunte por el mismo nombre dos veces (A y AAAA), así que consumirlo
        // dejaba la mitad de las filas sin programa y con pinta de fallo. Dentro de dos segundos,
        // el mismo nombre pedido por el mismo equipo es la misma llamada.
        let mut mejor: Option<(usize, i64)> = None;
        for (i, o) in d.cola.iter().enumerate() {
            if o.qname != buscado {
                continue;
            }
            let distancia = (o.ts - ts_ms).abs();
            if distancia <= VENTANA_MS && mejor.is_none_or(|(_, m)| distancia < m) {
                mejor = Some((i, distancia));
            }
        }
        let (i, _) = mejor?;
        d.cola.get(i).map(|o| o.proceso.clone())
    }
}

/// Saca de la cola lo que ya no puede casar con nada.
fn limpiar(d: &mut Compartido, ahora: i64) {
    while let Some(o) = d.cola.front() {
        if (ahora - o.ts) > VENTANA_MS * 4 || d.cola.len() > MAXIMO {
            d.cola.pop_front();
        } else {
            break;
        }
    }
    if d.rutas.len() > MAXIMO {
        d.rutas.clear();
    }
    if d.huellas.len() > MAXIMO {
        d.huellas.clear();
    }
}

/// Lo que hace el callback de la sesión con cada suceso.
fn anotar(datos: &Arc<Mutex<Compartido>>, registro: &EventRecord, esquemas: &SchemaLocator) {
    if registro.event_id() != EVENTO_CONSULTA {
        return;
    }
    let Ok(esquema) = esquemas.event_schema(registro) else {
        return;
    };
    let lector = Parser::create(registro, &esquema);
    let Ok(qname) = lector.try_parse::<String>("QueryName") else {
        return;
    };
    let qname = qname.trim_end_matches('.').to_ascii_lowercase();
    if qname.is_empty() {
        return;
    }
    let pid = registro.process_id();
    let ahora = guardiana_core::time::now_ms();
    let Ok(mut d) = datos.lock() else {
        return;
    };
    limpiar(&mut d, ahora);
    let (nombre, ruta) = match d.rutas.get(&pid) {
        Some(x) => x.clone(),
        None => {
            let x = programa_de(pid);
            d.rutas.insert(pid, x.clone());
            x
        }
    };
    let sha256 = if ruta.is_empty() {
        String::new()
    } else {
        match d.huellas.get(&ruta) {
            Some(h) => h.clone(),
            None => {
                let h = huella_de(&ruta);
                d.huellas.insert(ruta.clone(), h.clone());
                h
            }
        }
    };
    d.cola.push_back(Oida {
        ts: ahora,
        qname,
        proceso: Proceso {
            pid,
            nombre,
            ruta,
            sha256,
            firmado_por: String::new(),
        },
    });
}

/// SHA-256 del archivo del programa. Si no se puede leer, se devuelve vacío y se dice así en la
/// pantalla: mejor un hueco que un número inventado.
fn huella_de(ruta: &str) -> String {
    let Ok(datos) = std::fs::read(ruta) else {
        return String::new();
    };
    let mut h = Sha256::new();
    h.update(&datos);
    h.finalize().iter().fold(String::new(), |mut s, b| {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Nombre y ruta del programa que tiene ese número de proceso.
///
/// `unsafe` está prohibido en este proyecto salvo justificación (CLAUDE.md), y esta es la
/// justificación: no hay forma segura de preguntarle esto a Windows sin añadir una dependencia
/// grande (`sysinfo` y su árbol) para una sola llamada. Son dos funciones del sistema, con el
/// permiso más pequeño que existe para esto (`PROCESS_QUERY_LIMITED_INFORMATION`, que solo deja
/// leer la ruta), el asa se cierra siempre, y el buffer se pasa con su longitud y se corta por lo
/// que Windows dice que escribió. Si algo falla, se devuelve vacío: un hueco, nunca una invención.
fn programa_de(pid: u32) -> (String, String) {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let mut buffer = [0u16; 32_768];
    let mut largo: u32 = buffer.len() as u32;
    // SAFETY: OpenProcess con el permiso mínimo; el asa se comprueba y se cierra abajo.
    let asa = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if asa.is_null() || asa == INVALID_HANDLE_VALUE {
        return (String::new(), String::new());
    }
    // SAFETY: `buffer` es nuestro y `largo` lleva su tamaño; Windows escribe como mucho eso y
    // devuelve en `largo` lo que escribió de verdad.
    let ok = unsafe { QueryFullProcessImageNameW(asa, 0, buffer.as_mut_ptr(), &raw mut largo) };
    // SAFETY: el asa la acabamos de abrir y no se usa después de cerrarla.
    unsafe {
        CloseHandle(asa);
    }
    if ok == 0 {
        return (String::new(), String::new());
    }
    let largo = (largo as usize).min(buffer.len());
    let ruta = String::from_utf16_lossy(&buffer[..largo]);
    let nombre = ruta
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or_default()
        .to_owned();
    (nombre, ruta)
}

/// Escucha el proveedor unos segundos y devuelve lo que llegó, sin filtrar por número de suceso.
///
/// Solo para comprobar en la máquina lo que la documentación promete. No guarda nada.
pub(crate) fn diagnostico(segundos: u64) -> Result<Vec<(u16, String, u32)>, Error> {
    let vistos: Arc<Mutex<Vec<(u16, String, u32)>>> = Arc::default();
    let para_callback = Arc::clone(&vistos);
    let proveedor = Provider::by_guid(PROVEEDOR)
        .add_callback(move |registro: &EventRecord, esquemas: &SchemaLocator| {
            let id = registro.event_id();
            let pid = registro.process_id();
            let nombre = esquemas
                .event_schema(registro)
                .ok()
                .and_then(|e| {
                    Parser::create(registro, &e)
                        .try_parse::<String>("QueryName")
                        .ok()
                })
                .unwrap_or_default();
            if let Ok(mut v) = para_callback.lock() {
                if v.len() < MAXIMO {
                    v.push((id, nombre, pid));
                }
            }
        })
        .build();
    cerrar_la_que_quedo(SESION_DIAGNOSTICO);
    let _traza = UserTrace::new()
        .named(SESION_DIAGNOSTICO.to_owned())
        .enable(proveedor)
        .start_and_process()
        .map_err(|e| Error::Sistema(format!("{e:?}")))?;
    std::thread::sleep(std::time::Duration::from_secs(segundos));
    let salida = vistos.lock().map(|v| v.clone()).unwrap_or_default();
    Ok(salida)
}
