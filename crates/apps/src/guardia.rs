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
use guardiana_escudo::{cadenas, ladrones, secuestro};

use super::{Apertura, Error, Lanzamiento, Rafaga};
use crate::ventanas::programa_de;

/// `Microsoft-Windows-Kernel-File`.
const PROVEEDOR: &str = "edd08927-9cc4-4e65-b970-c2560fb5c289";
/// Its create keyword (0x80): the openings; and its delete-by-path (0x400) and rename-by-path (0x800) keywords.
const PALABRAS: u64 = 0x80 | 0x400 | 0x800;
const EV_CREATE: u16 = 12;
/// «DeletePath» and «RenamePath»: a file deleted or renamed, with its path.
const EV_BORRA: u16 = 26;
const EV_RENOMBRA: u16 = 27;
/// The most bursts waiting for the service.
const MAXIMO_RAFAGAS: usize = 64;
/// `FILE_DIRECTORY_FILE` in the create options: a folder being opened is not a file.
const ES_CARPETA: u32 = 0x1;
const SESION: &str = "Guardiana-Escudo";
/// `Microsoft-Windows-Kernel-Process`: every program start, with its parent.
const PROVEEDOR_PROCESOS: &str = "22fb2cd6-0e7b-422b-a0c7-2fad1fd0e716";
/// Its process keyword (`WINEVENT_KEYWORD_PROCESS`).
const PALABRAS_PROCESOS: u64 = 0x10;
/// «ProcessStart».
const EV_INICIO: u16 = 1;
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
    lanzamientos: VecDeque<Lanzamiento>,
    lanzados: HashMap<(String, String), i64>,
    contador: secuestro::Contador,
    rafagas: VecDeque<Rafaga>,
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
        let para_procesos = Arc::clone(&datos);
        let proveedor = Provider::by_guid(PROVEEDOR)
            .any(PALABRAS)
            .add_filter(EventFilter::ByEventIds(vec![
                EV_CREATE,
                EV_BORRA,
                EV_RENOMBRA,
            ]))
            .add_callback(move |registro: &EventRecord, esquemas: &SchemaLocator| {
                anotar(&para_callback, registro, esquemas, yo);
                anotar_cambio(&para_callback, registro, esquemas, yo);
            })
            .build();
        let procesos = Provider::by_guid(PROVEEDOR_PROCESOS)
            .any(PALABRAS_PROCESOS)
            .add_filter(EventFilter::ByEventIds(vec![EV_INICIO]))
            .add_callback(move |registro: &EventRecord, esquemas: &SchemaLocator| {
                anotar_inicio(&para_procesos, registro, esquemas);
            })
            .build();
        let _ = ferrisetw::trace::stop_trace_by_name(SESION);
        let traza = UserTrace::new()
            .named(SESION.to_owned())
            .enable(proveedor)
            .enable(procesos)
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

    /// The bursts of the person's files renamed or deleted (`guardiana_escudo::secuestro`) since the last take.
    pub(crate) fn rafagas(&self) -> Vec<Rafaga> {
        self.datos
            .lock()
            .map(|mut d| d.rafagas.drain(..).collect())
            .unwrap_or_default()
    }

    /// The program starts watched (`guardiana_escudo::cadenas::vigilado`) since the last take, with their parent.
    pub(crate) fn lanzamientos(&self) -> Vec<Lanzamiento> {
        self.datos
            .lock()
            .map(|mut d| d.lanzamientos.drain(..).collect())
            .unwrap_or_default()
    }
}

/// A file deleted or renamed: only the person's own ([`secuestro::es_personal`]), counted per program; a program that
/// reaches the threshold within the window becomes a burst for the service.
fn anotar_cambio(
    datos: &Arc<Mutex<Compartido>>,
    registro: &EventRecord,
    esquemas: &SchemaLocator,
    yo: u32,
) {
    let id = registro.event_id();
    if id != EV_BORRA && id != EV_RENOMBRA {
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
    let Ok(archivo) = lector.try_parse::<String>("FilePath") else {
        return;
    };
    if !secuestro::es_personal(&archivo) {
        return;
    }
    let ahora = guardiana_core::time::now_ms();
    let Ok(mut d) = datos.lock() else {
        return;
    };
    if !d.contador.anota(pid, u64::try_from(ahora).unwrap_or(0)) {
        return;
    }
    let (programa, ruta) = programa_de(pid);
    if programa.is_empty() || d.rafagas.len() >= MAXIMO_RAFAGAS {
        return;
    }
    d.rafagas.push_back(Rafaga {
        ts: ahora,
        programa,
        ruta,
        n: secuestro::UMBRAL,
    });
}

/// A program start: only those [`cadenas::vigilado`] cares about, with the program that started it.
fn anotar_inicio(datos: &Arc<Mutex<Compartido>>, registro: &EventRecord, esquemas: &SchemaLocator) {
    if registro.event_id() != EV_INICIO {
        return;
    }
    let Ok(esquema) = esquemas.event_schema(registro) else {
        return;
    };
    let lector = Parser::create(registro, &esquema);
    let Ok(imagen) = lector.try_parse::<String>("ImageName") else {
        return;
    };
    let hijo = imagen
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or_default()
        .to_owned();
    if !cadenas::vigilado(&hijo) {
        return;
    }
    let padre_pid = lector.try_parse::<u32>("ParentProcessID").unwrap_or(0);
    if padre_pid == 0 || padre_pid == 4 {
        return;
    }
    let (padre, padre_ruta) = programa_de(padre_pid);
    if padre.is_empty() {
        return;
    }
    let ahora = guardiana_core::time::now_ms();
    let Ok(mut d) = datos.lock() else {
        return;
    };
    let clave = (padre_ruta.to_ascii_lowercase(), hijo.to_ascii_lowercase());
    if d.lanzados
        .get(&clave)
        .is_some_and(|t| ahora - *t < REPETICION_MS)
    {
        return;
    }
    if d.lanzados.len() > 4_096 {
        d.lanzados.clear();
    }
    d.lanzados.insert(clave, ahora);
    if d.lanzamientos.len() < MAXIMO {
        d.lanzamientos.push_back(Lanzamiento {
            ts: ahora,
            padre,
            padre_ruta,
            hijo,
            imagen,
        });
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
