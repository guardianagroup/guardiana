//! Windows half of «which of your files a program opened» (1.0.45, «Visión profunda»).
//!
//! Windows publishes, on its kernel file event channel (`Microsoft-Windows-Kernel-File`), one
//! event when a program opens a file, with the file's path, and one per read, with its size.
//! Here only three events are asked for (open, read, close) and only the openings of files inside
//! the person's own folders (Documents, Desktop, Downloads, Pictures, also under OneDrive) are
//! kept, by **name**: the folder and everything above it stay here, the contents are never read
//! (the channel does not even carry them). It runs only while the person has the file watch on.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ferrisetw::parser::{Parser, Pointer};
use ferrisetw::provider::{EventFilter, Provider};
use ferrisetw::schema_locator::SchemaLocator;
use ferrisetw::trace::UserTrace;
use ferrisetw::EventRecord;
use guardiana_core::vision::Lectura;

use super::{carpeta_de_usuario, Error};
use crate::ventanas::programa_de;

/// `Microsoft-Windows-Kernel-File`.
const PROVEEDOR: &str = "edd08927-9cc4-4e65-b970-c2560fb5c289";
/// Its keywords: file I/O (0x20, which carries the close), create (0x80) and read (0x100).
const PALABRAS: u64 = 0x20 | 0x80 | 0x100;
/// Create, close and read: nothing else crosses from the kernel (filtered there, by number).
const EV_CREATE: u16 = 12;
const EV_CLOSE: u16 = 14;
const EV_READ: u16 = 15;
/// `FILE_DIRECTORY_FILE` in the create options: a folder being opened is not a file.
const ES_CARPETA: u32 = 0x1;
const SESION: &str = "Guardiana-Archivos";
/// The most files open at once that are followed for their reads.
const MAXIMO_ABIERTOS: usize = 4_096;
/// The most openings waiting for the next take.
const MAXIMO_PENDIENTES: usize = 4_096;
const VIGENCIA_MS: i64 = 5_000;

struct Abierto {
    programa: String,
    archivo: String,
    /// Bytes read since the last take.
    leido: u64,
    ultimo: i64,
}

#[derive(Default)]
struct Compartido {
    /// File object (the kernel's pointer, only an id here) → what was opened.
    abiertos: HashMap<usize, Abierto>,
    pendientes: Vec<Lectura>,
    rutas: HashMap<u32, (String, i64)>,
}

/// The file session running. Dropping it closes it.
pub(crate) struct Sesion {
    datos: Arc<Mutex<Compartido>>,
    _traza: UserTrace,
}

impl Sesion {
    pub(crate) fn arrancar() -> Result<Self, Error> {
        let datos: Arc<Mutex<Compartido>> = Arc::default();
        let para_callback = Arc::clone(&datos);
        let yo = std::process::id();
        let proveedor = Provider::by_guid(PROVEEDOR)
            .any(PALABRAS)
            .add_filter(EventFilter::ByEventIds(vec![EV_CREATE, EV_CLOSE, EV_READ]))
            .add_callback(move |registro: &EventRecord, esquemas: &SchemaLocator| {
                anotar(&para_callback, registro, esquemas, yo);
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
            _traza: traza,
        })
    }

    /// The openings since the last take, and the bytes read from files still open.
    pub(crate) fn toma(&self) -> Vec<Lectura> {
        let Ok(mut d) = self.datos.lock() else {
            return Vec::new();
        };
        let mut out = std::mem::take(&mut d.pendientes);
        for a in d.abiertos.values_mut() {
            if a.leido > 0 {
                out.push(Lectura {
                    ts: a.ultimo,
                    programa: a.programa.clone(),
                    archivo: a.archivo.clone(),
                    bytes: a.leido,
                });
                a.leido = 0;
            }
        }
        out
    }
}

fn programa(d: &mut Compartido, pid: u32, ahora: i64) -> String {
    if let Some((n, visto)) = d.rutas.get(&pid) {
        if ahora - *visto < VIGENCIA_MS {
            return n.clone();
        }
    }
    if d.rutas.len() > 4_096 {
        d.rutas.clear();
    }
    let (n, _) = programa_de(pid);
    d.rutas.insert(pid, (n.clone(), ahora));
    n
}

fn anotar(
    datos: &Arc<Mutex<Compartido>>,
    registro: &EventRecord,
    esquemas: &SchemaLocator,
    yo: u32,
) {
    let id = registro.event_id();
    let pid = registro.process_id();
    // The system's own process does paging reads for everybody; GUARDIANA's are its own.
    if pid == 4 || pid == 0 || pid == yo {
        return;
    }
    // A read or a close of a file nobody is following: out before parsing anything, because
    // these are most of what arrives.
    if id != EV_CREATE {
        let vacio = datos.lock().map(|d| d.abiertos.is_empty()).unwrap_or(true);
        if vacio {
            return;
        }
    }
    let Ok(esquema) = esquemas.event_schema(registro) else {
        return;
    };
    let lector = Parser::create(registro, &esquema);
    let Ok(objeto) = lector.try_parse::<Pointer>("FileObject") else {
        return;
    };
    let objeto = *objeto;
    let ahora = guardiana_core::time::now_ms();
    match id {
        EV_CREATE => {
            let opciones = lector.try_parse::<u32>("CreateOptions").unwrap_or(0);
            if opciones & ES_CARPETA != 0 {
                return;
            }
            let Ok(ruta) = lector.try_parse::<String>("FileName") else {
                return;
            };
            let Some((_, archivo)) = carpeta_de_usuario(&ruta) else {
                return;
            };
            let Ok(mut d) = datos.lock() else {
                return;
            };
            let nombre = programa(&mut d, pid, ahora);
            if nombre.is_empty() {
                return;
            }
            if d.pendientes.len() < MAXIMO_PENDIENTES {
                d.pendientes.push(Lectura {
                    ts: ahora,
                    programa: nombre.clone(),
                    archivo: archivo.clone(),
                    bytes: 0,
                });
            }
            if d.abiertos.len() < MAXIMO_ABIERTOS {
                d.abiertos.insert(
                    objeto,
                    Abierto {
                        programa: nombre,
                        archivo,
                        leido: 0,
                        ultimo: ahora,
                    },
                );
            }
        }
        EV_READ => {
            let tamano = lector.try_parse::<u32>("IOSize").unwrap_or(0);
            if let Ok(mut d) = datos.lock() {
                if let Some(a) = d.abiertos.get_mut(&objeto) {
                    a.leido = a.leido.saturating_add(u64::from(tamano));
                    a.ultimo = ahora;
                }
            }
        }
        EV_CLOSE => {
            if let Ok(mut d) = datos.lock() {
                if let Some(a) = d.abiertos.remove(&objeto) {
                    if a.leido > 0 && d.pendientes.len() < MAXIMO_PENDIENTES {
                        d.pendientes.push(Lectura {
                            ts: a.ultimo,
                            programa: a.programa,
                            archivo: a.archivo,
                            bytes: a.leido,
                        });
                    }
                }
            }
        }
        _ => {}
    }
}
