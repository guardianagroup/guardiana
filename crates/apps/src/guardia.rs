//! El escudo's watch over saved secrets (1.0.52, in preparation; `guardiana_escudo::ladrones`): one more session on
//! Windows' kernel file event channel (`Microsoft-Windows-Kernel-File`), only for the openings, on while the service
//! runs, and filtered here to the files that hold a saved secret: the browsers' passwords, cookies and keys, wallets and
//! messaging sessions. For an opening by a program that is not the app's own installed program, it notes which program
//! and which file. Nothing else is kept and no file is read.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use ferrisetw::parser::Parser;
use ferrisetw::provider::{EventFilter, Provider};
use ferrisetw::schema_locator::SchemaLocator;
use ferrisetw::trace::UserTrace;
use ferrisetw::EventRecord;
use guardiana_escudo::ladrones;

use super::{Apertura, Error};
use crate::ventanas::programa_de;

/// `Microsoft-Windows-Kernel-File`.
const PROVEEDOR: &str = "edd08927-9cc4-4e65-b970-c2560fb5c289";
/// Its create keyword (0x80): the openings, nothing else.
const PALABRAS: u64 = 0x80;
const EV_CREATE: u16 = 12;
/// `FILE_DIRECTORY_FILE` in the create options: a folder being opened is not a file.
const ES_CARPETA: u32 = 0x1;
const SESION: &str = "Guardiana-Escudo";
/// The most openings waiting for the service.
const MAXIMO: usize = 1_024;
/// The same program opening the same file again within this time is the same opening.
const REPETICION_MS: i64 = 60_000;
/// How long the program behind a process number is trusted.
const VIGENCIA_MS: i64 = 5_000;

#[derive(Default)]
struct Compartido {
    cola: VecDeque<Apertura>,
    vistas: HashMap<(String, String), i64>,
    rutas: HashMap<u32, (String, String, i64)>,
}

/// The session running. Dropping it closes it.
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
            .add_filter(EventFilter::ByEventIds(vec![EV_CREATE]))
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

    /// The openings since the last take.
    pub(crate) fn toma(&self) -> Vec<Apertura> {
        self.datos
            .lock()
            .map(|mut d| d.cola.drain(..).collect())
            .unwrap_or_default()
    }
}

/// Where installed programs live: an app's own program found there is the app (its signature is checked later anyway).
fn instalado(ruta: &str) -> bool {
    let r = ruta.to_ascii_lowercase();
    r.contains(r":\program files\")
        || r.contains(r":\program files (x86)\")
        || r.contains(r":\windows\")
        || r.contains(r"\appdata\local\programs\")
        || (r.contains(r"\appdata\local\") && r.contains(r"\application\"))
}

fn anotar(
    datos: &Arc<Mutex<Compartido>>,
    registro: &EventRecord,
    esquemas: &SchemaLocator,
    yo: u32,
) {
    if registro.event_id() != EV_CREATE {
        return;
    }
    let pid = registro.process_id();
    if pid == 4 || pid == 0 || pid == yo {
        return;
    }
    let Ok(esquema) = esquemas.event_schema(registro) else {
        return;
    };
    let lector = Parser::create(registro, &esquema);
    if lector.try_parse::<u32>("CreateOptions").unwrap_or(0) & ES_CARPETA != 0 {
        return;
    }
    let Ok(archivo) = lector.try_parse::<String>("FileName") else {
        return;
    };
    // the cheap test first: almost every opening on the computer stops here
    let Some(h) = ladrones::que_es(&archivo) else {
        return;
    };
    let ahora = guardiana_core::time::now_ms();
    let Ok(mut d) = datos.lock() else {
        return;
    };
    let conocido = d
        .rutas
        .get(&pid)
        .filter(|(_, _, visto)| ahora - *visto < VIGENCIA_MS)
        .map(|(n, r, _)| (n.clone(), r.clone()));
    let (nombre, ruta) = match conocido {
        Some(x) => x,
        None => {
            if d.rutas.len() > 4_096 {
                d.rutas.clear();
            }
            let (n, r) = programa_de(pid);
            d.rutas.insert(pid, (n.clone(), r.clone(), ahora));
            (n, r)
        }
    };
    if nombre.is_empty() || (ladrones::es_de_casa(&nombre, &h) && instalado(&ruta)) {
        return;
    }
    let clave = (ruta.clone(), archivo.to_ascii_lowercase());
    if d.vistas
        .get(&clave)
        .is_some_and(|t| ahora - *t < REPETICION_MS)
    {
        return;
    }
    if d.vistas.len() > 4_096 {
        d.vistas.clear();
    }
    d.vistas.insert(clave, ahora);
    if d.cola.len() < MAXIMO {
        d.cola.push_back(Apertura {
            ts: ahora,
            programa: nombre,
            ruta,
            archivo,
        });
    }
}
