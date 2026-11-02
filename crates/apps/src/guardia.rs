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
use guardiana_escudo::{cadenas, disfraz, engano, ladrones, secuestro};

use super::{Acceso, Apertura, DesdeMedio, Error, Lanzamiento, Rafaga};
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
/// `Microsoft-Windows-Kernel-Audit-API-Calls`: Windows' own audit of some kernel calls, among them each opening of a
/// process (its event 5: `TargetProcessId`, `DesiredAccess`, `ReturnCode`; manifest read on 1 Nov 2026).
const PROVEEDOR_API: &str = "e02a841c-75a3-4fa7-afc8-ae09cf9b7f23";
/// «OpenProcess».
const EV_ABRE_PROCESO: u16 = 5;
/// The most openings of LSASS waiting for the service.
const MAXIMO_ACCESOS: usize = 64;
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
    /// Every program started since the session began: process number → (name, path, when). A program that has
    /// already ended when its events arrive (a `cmd /c` that opened a secret and left; the real test found it) is
    /// still named from here; a live one is asked to Windows first, so a reused number is never misnamed.
    iniciados: HashMap<u32, (String, String, i64)>,
    /// The volumes' device names (`\device\harddiskvolume3`) and their letters (`C:`), read when the session starts:
    /// the kernel names a started program by its device path.
    volumenes: Vec<(String, String)>,
    /// The openings of LSASS that asked to read its memory, waiting for the service.
    accesos: VecDeque<Acceso>,
    /// The programs started from a disc, a disk image or a removable drive, waiting for the service; and program path →
    /// when, so the same one counts once a minute.
    desde_medios: VecDeque<DesdeMedio>,
    medios_vistos: HashMap<String, i64>,
    /// When the volumes were last read, and each drive letter's kind (removable or not) with when it was asked.
    volumenes_ms: i64,
    tipos_de_unidad: HashMap<String, (bool, i64)>,
    /// The same program opening LSASS again within [`REPETICION_MS`] is the same opening: program path → when.
    accesos_vistos: HashMap<String, i64>,
    /// For the real test only: the openings of a «Login Data» file, before any filter.
    #[cfg(test)]
    prueba_login: Vec<String>,
}

/// For the real tests only: what arrived and what it cost, counted without the lock, so counting weighs next to
/// nothing.
#[cfg(test)]
mod medida {
    use std::sync::atomic::AtomicU64;

    /// The openings that arrived from other programs, before any filter.
    pub(super) static CREADOS: AtomicU64 = AtomicU64::new(0);
    /// Nanoseconds spent inside the file events' callback.
    pub(super) static EN_LLAMADA_NS: AtomicU64 = AtomicU64::new(0);
    /// Of those, nanoseconds spent reading an opening's fields (its schema, its options and its file name).
    pub(super) static LEYENDO_NS: AtomicU64 = AtomicU64::new(0);
    /// The openings of a process that arrived from other programs, and those of LSASS.
    pub(super) static PROCESOS: AtomicU64 = AtomicU64::new(0);
    pub(super) static DE_LSASS: AtomicU64 = AtomicU64::new(0);

    /// Nanoseconds since `desde`.
    pub(super) fn ns(desde: std::time::Instant) -> u64 {
        u64::try_from(desde.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }
}

/// The most programs remembered from their start.
const MAX_INICIADOS: usize = 8_192;

/// The volumes' device names and their letters, as Windows maps them now (`QueryDosDeviceW` for `A:` to `Z:`).
fn volumenes() -> Vec<(String, String)> {
    use windows_sys::Win32::Storage::FileSystem::QueryDosDeviceW;
    let mut out = Vec::new();
    for letra in b'A'..=b'Z' {
        let unidad = format!("{}:", char::from(letra));
        let nombre: Vec<u16> = unidad.encode_utf16().chain(std::iter::once(0)).collect();
        let mut buf = [0u16; 512];
        // SAFETY: a NUL-terminated drive name and a buffer owned here, with its length in characters.
        #[allow(unsafe_code)]
        let n = unsafe { QueryDosDeviceW(nombre.as_ptr(), buf.as_mut_ptr(), 512) };
        if n == 0 {
            continue;
        }
        let fin = buf.iter().position(|c| *c == 0).unwrap_or(0);
        let dispositivo = String::from_utf16_lossy(&buf[..fin]).to_ascii_lowercase();
        if !dispositivo.is_empty() {
            out.push((dispositivo, unidad));
        }
    }
    out
}

/// `DRIVE_REMOVABLE`, what `GetDriveTypeW` says of a USB stick or a memory card.
const UNIDAD_EXTRAIBLE: u32 = 2;
/// How long a drive letter's kind is trusted.
const VIGENCIA_UNIDAD_MS: i64 = 10_000;

/// Where a started program ran from, by its kernel path (`\Device\CdRom0\x.exe`, `\Device\HarddiskVolume7\x.exe`):
/// a disc or a mounted disk image, a removable drive, or neither. A volume mounted after the session began is not in
/// the table of letters yet: it is read again (at most every few seconds).
fn medio_de(nt: &str, d: &mut Compartido) -> Option<guardiana_escudo::medios::Medio> {
    use guardiana_escudo::medios::Medio;
    if nt.to_ascii_lowercase().starts_with(r"\device\cdrom") {
        return Some(Medio::Imagen);
    }
    let ahora = guardiana_core::time::now_ms();
    let mut ruta = ruta_dos(nt, &d.volumenes);
    if ruta.starts_with('\\') && ahora - d.volumenes_ms > 5_000 {
        d.volumenes = volumenes();
        d.volumenes_ms = ahora;
        ruta = ruta_dos(nt, &d.volumenes);
    }
    let letra = ruta
        .get(..2)
        .filter(|l| l.ends_with(':'))?
        .to_ascii_uppercase();
    if letra == "C:" {
        return None;
    }
    let extraible = match d.tipos_de_unidad.get(&letra) {
        Some((e, cuando)) if ahora - *cuando < VIGENCIA_UNIDAD_MS => *e,
        _ => {
            use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
            let raiz: Vec<u16> = format!("{letra}\\")
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            // SAFETY: a NUL-terminated root path owned here.
            #[allow(unsafe_code)]
            let tipo = unsafe { GetDriveTypeW(raiz.as_ptr()) };
            let e = tipo == UNIDAD_EXTRAIBLE;
            d.tipos_de_unidad.insert(letra, (e, ahora));
            e
        }
    };
    extraible.then_some(Medio::Usb)
}

/// The process number of LSASS: the `lsass.exe` running from Windows' own folder (a copy elsewhere is not it); 0 when
/// not found.
fn pid_de_lsass() -> u32 {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    // SAFETY: a snapshot of the system's processes; the handle is checked and closed below.
    #[allow(unsafe_code)]
    let foto = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if foto == INVALID_HANDLE_VALUE || foto.is_null() {
        return 0;
    }
    let mut e = PROCESSENTRY32W {
        dwSize: u32::try_from(std::mem::size_of::<PROCESSENTRY32W>()).unwrap_or(0),
        ..PROCESSENTRY32W::default()
    };
    let mut pid = 0;
    // SAFETY: `e` carries its size and the snapshot is valid.
    #[allow(unsafe_code)]
    let mut hay = unsafe { Process32FirstW(foto, &raw mut e) } != 0;
    while hay {
        let largo = e
            .szExeFile
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(e.szExeFile.len());
        let nombre = String::from_utf16_lossy(e.szExeFile.get(..largo).unwrap_or(&[]));
        if nombre.eq_ignore_ascii_case("lsass.exe") {
            let (_, ruta) = programa_de(e.th32ProcessID);
            if ruta
                .to_ascii_lowercase()
                .ends_with(r"\windows\system32\lsass.exe")
            {
                pid = e.th32ProcessID;
                break;
            }
        }
        // SAFETY: as above.
        #[allow(unsafe_code)]
        let siguiente = unsafe { Process32NextW(foto, &raw mut e) } != 0;
        hay = siguiente;
    }
    // SAFETY: the snapshot's handle, closed once.
    #[allow(unsafe_code)]
    unsafe {
        CloseHandle(foto)
    };
    pid
}

/// A device path (`\Device\HarddiskVolume3\Windows\x.exe`) as a drive path (`C:\Windows\x.exe`); as it is when
/// no volume matches.
fn ruta_dos(nt: &str, volumenes: &[(String, String)]) -> String {
    let bajo = nt.to_ascii_lowercase();
    for (dispositivo, unidad) in volumenes {
        if let Some(resto) = bajo.strip_prefix(dispositivo.as_str()) {
            if resto.starts_with('\\') {
                return format!(
                    "{unidad}{}",
                    nt.get(dispositivo.len()..).unwrap_or_default()
                );
            }
        }
    }
    nt.to_owned()
}

/// The program behind a process number: Windows' word while it runs; else what its start said, when it started
/// during this session.
fn programa_en(d: &Compartido, pid: u32) -> (String, String) {
    let vivo = programa_de(pid);
    if !vivo.0.is_empty() {
        return vivo;
    }
    d.iniciados
        .get(&pid)
        .map(|(n, r, _)| (n.clone(), r.clone()))
        .unwrap_or_default()
}

/// The session running. Dropping it closes it.
pub(crate) struct Sesion {
    datos: Arc<Mutex<Compartido>>,
    _traza: UserTrace,
}

impl Sesion {
    pub(crate) fn arrancar() -> Result<Self, Error> {
        let datos: Arc<Mutex<Compartido>> = Arc::default();
        if let Ok(mut d) = datos.lock() {
            d.volumenes = volumenes();
        }
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
                #[cfg(test)]
                let desde = std::time::Instant::now();
                anotar(&para_callback, registro, esquemas, yo);
                anotar_cambio(&para_callback, registro, esquemas, yo);
                #[cfg(test)]
                medida::EN_LLAMADA_NS
                    .fetch_add(medida::ns(desde), std::sync::atomic::Ordering::Relaxed);
            })
            .build();
        let procesos = Provider::by_guid(PROVEEDOR_PROCESOS)
            .any(PALABRAS_PROCESOS)
            .add_filter(EventFilter::ByEventIds(vec![EV_INICIO]))
            .add_callback(move |registro: &EventRecord, esquemas: &SchemaLocator| {
                anotar_inicio(&para_procesos, registro, esquemas);
            })
            .build();
        let para_accesos = Arc::clone(&datos);
        let lsass = pid_de_lsass();
        let api = Provider::by_guid(PROVEEDOR_API)
            .add_filter(EventFilter::ByEventIds(vec![EV_ABRE_PROCESO]))
            .add_callback(move |registro: &EventRecord, esquemas: &SchemaLocator| {
                anotar_acceso(&para_accesos, registro, esquemas, yo, lsass);
            })
            .build();
        let _ = ferrisetw::trace::stop_trace_by_name(SESION);
        let traza = UserTrace::new()
            .named(SESION.to_owned())
            .enable(proveedor)
            .enable(procesos)
            .enable(api)
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

    /// The programs started from a disc, a disk image or a removable drive (`guardiana_escudo::medios`) since the last
    /// take.
    pub(crate) fn desde_medios(&self) -> Vec<DesdeMedio> {
        self.datos
            .lock()
            .map(|mut d| d.desde_medios.drain(..).collect())
            .unwrap_or_default()
    }

    /// The openings of LSASS that asked to read its memory (`guardiana_escudo::credenciales`) since the last take.
    pub(crate) fn accesos(&self) -> Vec<Acceso> {
        self.datos
            .lock()
            .map(|mut d| d.accesos.drain(..).collect())
            .unwrap_or_default()
    }
}

/// A process opened: only LSASS (process `lsass`, found when the session started), and only when the rights asked for
/// include reading its memory (`guardiana_escudo::credenciales::LEE_MEMORIA`). Almost every opening on the computer
/// stops at its first field.
fn anotar_acceso(
    datos: &Arc<Mutex<Compartido>>,
    registro: &EventRecord,
    esquemas: &SchemaLocator,
    yo: u32,
    lsass: u32,
) {
    if lsass == 0 || registro.event_id() != EV_ABRE_PROCESO {
        return;
    }
    let pid = registro.process_id();
    if pid == 4 || pid == 0 || pid == yo || pid == lsass {
        return;
    }
    let Ok(esquema) = esquemas.event_schema(registro) else {
        return;
    };
    let lector = Parser::create(registro, &esquema);
    let objetivo = lector.try_parse::<u32>("TargetProcessId").ok();
    #[cfg(test)]
    {
        use std::sync::atomic::Ordering;
        medida::PROCESOS.fetch_add(1, Ordering::Relaxed);
        if objetivo == Some(lsass) {
            medida::DE_LSASS.fetch_add(1, Ordering::Relaxed);
        }
    }
    if objetivo != Some(lsass) {
        return;
    }
    let acceso = lector.try_parse::<u32>("DesiredAccess").unwrap_or(0);
    if acceso & guardiana_escudo::credenciales::LEE_MEMORIA == 0 {
        return;
    }
    let concedido = lector.try_parse::<u32>("ReturnCode").ok() == Some(0);
    let ahora = guardiana_core::time::now_ms();
    let Ok(mut d) = datos.lock() else {
        return;
    };
    let (programa, ruta) = programa_en(&d, pid);
    if programa.is_empty() {
        return;
    }
    let clave = ruta.to_ascii_lowercase();
    if d.accesos_vistos
        .get(&clave)
        .is_some_and(|t| ahora - *t < REPETICION_MS)
    {
        return;
    }
    if d.accesos_vistos.len() > 4_096 {
        d.accesos_vistos.clear();
    }
    d.accesos_vistos.insert(clave, ahora);
    if d.accesos.len() < MAXIMO_ACCESOS {
        d.accesos.push_back(Acceso {
            ts: ahora,
            programa,
            ruta,
            acceso,
            concedido,
        });
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
    let (programa, ruta) = programa_en(&d, pid);
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
    // every start is remembered, so its events can name it after it has ended
    let pid_nuevo = lector.try_parse::<u32>("ProcessID").unwrap_or(0);
    if pid_nuevo != 0 {
        if let Ok(mut d) = datos.lock() {
            if d.iniciados.len() >= MAX_INICIADOS {
                d.iniciados.clear();
            }
            let ruta = ruta_dos(&imagen, &d.volumenes);
            d.iniciados.insert(
                pid_nuevo,
                (hijo.clone(), ruta, guardiana_core::time::now_ms()),
            );
        }
    }
    // a program started from a disc, a mounted disk image or a removable drive (escudo, «medio»)
    if let Ok(mut d) = datos.lock() {
        if let Some(medio) = medio_de(&imagen, &mut d) {
            let ahora = guardiana_core::time::now_ms();
            let ruta = ruta_dos(&imagen, &d.volumenes);
            let clave = ruta.to_ascii_lowercase();
            let repetido = d
                .medios_vistos
                .get(&clave)
                .is_some_and(|t| ahora - *t < REPETICION_MS);
            if !repetido && d.desde_medios.len() < MAXIMO_RAFAGAS {
                if d.medios_vistos.len() > 4_096 {
                    d.medios_vistos.clear();
                }
                d.medios_vistos.insert(clave, ahora);
                d.desde_medios.push_back(DesdeMedio {
                    ts: ahora,
                    programa: hijo.clone(),
                    ruta,
                    medio: medio.codigo(),
                });
            }
        }
    }
    // a program wearing the name of one of Windows' own outside Windows' folder (escudo, «disfraz»), or a name made
    // to look like a document («engaño»), goes on even without a parent known
    let sin_padre = disfraz::sospechoso(&hijo, &imagen) || engano::sospechoso(&hijo).is_some();
    if !cadenas::vigilado(&hijo) && !sin_padre {
        return;
    }
    let padre_pid = lector.try_parse::<u32>("ParentProcessID").unwrap_or(0);
    let (padre, padre_ruta) = if padre_pid == 0 || padre_pid == 4 {
        (String::new(), String::new())
    } else {
        datos
            .lock()
            .map(|d| programa_en(&d, padre_pid))
            .unwrap_or_default()
    };
    if padre.is_empty() && !sin_padre {
        return;
    }
    let ahora = guardiana_core::time::now_ms();
    let Ok(mut d) = datos.lock() else {
        return;
    };
    let clave = if sin_padre {
        (imagen.to_lowercase(), hijo.to_ascii_lowercase())
    } else {
        (padre_ruta.to_ascii_lowercase(), hijo.to_ascii_lowercase())
    };
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
        let imagen = ruta_dos(&imagen, &d.volumenes);
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
    #[cfg(test)]
    let leyendo = std::time::Instant::now();
    let Ok(esquema) = esquemas.event_schema(registro) else {
        return;
    };
    let lector = Parser::create(registro, &esquema);
    let opciones = lector.try_parse::<u32>("CreateOptions").unwrap_or(0);
    // a folder being opened is not a file: its name is not even read
    let nombre = ((opciones & ES_CARPETA) == 0).then(|| lector.try_parse::<String>("FileName"));
    #[cfg(test)]
    prueba(datos, pid, nombre.as_ref(), opciones, leyendo);
    let Some(Ok(archivo)) = nombre else {
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
            let (n, r) = programa_en(&d, pid);
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

/// For the real tests only: counts an opening and the time spent reading it, and keeps the openings of a «Login Data»
/// file (or a file name that could not be read) as they arrived, to show them when the test fails.
#[cfg(test)]
fn prueba<E: std::fmt::Debug>(
    datos: &Arc<Mutex<Compartido>>,
    pid: u32,
    nombre: Option<&Result<String, E>>,
    opciones: u32,
    leyendo: std::time::Instant,
) {
    use std::sync::atomic::Ordering;
    medida::LEYENDO_NS.fetch_add(medida::ns(leyendo), Ordering::Relaxed);
    medida::CREADOS.fetch_add(1, Ordering::Relaxed);
    let apunte = match nombre {
        Some(Ok(n)) if n.contains("Login Data") => format!("{pid} {n} {opciones:#x}"),
        Some(Err(e)) => format!("FileName ilegible: {e:?}"),
        _ => return,
    };
    if let Ok(mut d) = datos.lock() {
        if d.prueba_login.len() < 20 {
            d.prueba_login.push(apunte);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    fn powershell(orden: &str) {
        let estado = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", orden])
            .status()
            .unwrap();
        assert!(estado.success(), "{orden}");
    }

    /// On a real Windows, as an administrator (the escudo workflow): the escudo's own session sees, from other
    /// processes, a saved secret opened (a file where Chrome keeps its passwords), program starts with their parent,
    /// and a burst of the person's files renamed. It checks the kernel channels' field names this module reads.
    #[test]
    #[ignore = "necesita Windows como administrador"]
    fn la_guardia_ve_de_verdad() {
        let sesion = Sesion::arrancar().unwrap();
        std::thread::sleep(Duration::from_secs(3));
        // a saved secret, opened by another program (cmd's «type»)
        let local = PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap());
        let chrome = local.join(r"Google\Chrome\User Data\Default");
        std::fs::create_dir_all(&chrome).unwrap();
        let secreto = chrome.join("Login Data");
        std::fs::write(&secreto, b"prueba").unwrap();
        let estado = Command::new("cmd")
            .args(["/c", "type"])
            .arg(&secreto)
            .status()
            .unwrap();
        assert!(estado.success());
        // a burst: PowerShell renames 120 files in the person's Documents
        let perfil = PathBuf::from(std::env::var_os("USERPROFILE").unwrap());
        let carpeta = perfil.join(r"Documents\guardiana-prueba-escudo");
        std::fs::create_dir_all(&carpeta).unwrap();
        for i in 0..120 {
            std::fs::write(carpeta.join(format!("f{i}.txt")), b"x").unwrap();
        }
        powershell(&format!(
            "Get-ChildItem -LiteralPath '{}' -File | Rename-Item -NewName {{ $_.Name + '.cifrado' }}",
            carpeta.display()
        ));
        // LSASS opened by another program with the right to read its memory: only a handle, closed when PowerShell
        // ends; nothing is read (and Windows may refuse it, which is seen too)
        powershell("try { $h = (Get-Process -Name lsass).Handle } catch { }");
        std::thread::sleep(Duration::from_secs(4));
        let aperturas = sesion.toma();
        let lanzamientos = sesion.lanzamientos();
        let rafagas = sesion.rafagas();
        let accesos = sesion.accesos();
        println!(
            "aperturas recibidas: {}",
            medida::CREADOS.load(Ordering::Relaxed)
        );
        if let Ok(d) = sesion.datos.lock() {
            println!("de «Login Data»: {:?}", d.prueba_login);
        }
        let _ = std::fs::remove_dir_all(&carpeta);
        let _ = std::fs::remove_file(&secreto);
        println!("aperturas: {aperturas:?}");
        println!("lanzamientos: {lanzamientos:?}");
        println!("ráfagas: {rafagas:?}");
        println!(
            "accesos a LSASS: {accesos:?} (aperturas de procesos: {}, de LSASS: {}, LSASS es {})",
            medida::PROCESOS.load(Ordering::Relaxed),
            medida::DE_LSASS.load(Ordering::Relaxed),
            pid_de_lsass()
        );
        assert!(
            aperturas.iter().any(
                |a| a.archivo.to_lowercase().ends_with(r"\default\login data")
                    && a.programa.eq_ignore_ascii_case("cmd.exe")
            ),
            "la apertura de «Login Data» por cmd.exe"
        );
        assert!(
            lanzamientos
                .iter()
                .any(|l| l.hijo.eq_ignore_ascii_case("powershell.exe") && !l.padre.is_empty()),
            "el inicio de PowerShell con su padre"
        );
        assert!(
            rafagas
                .iter()
                .any(|r| r.programa.eq_ignore_ascii_case("powershell.exe")
                    && r.n >= secuestro::UMBRAL),
            "la ráfaga de PowerShell"
        );
        assert!(
            accesos
                .iter()
                .any(|a| a.programa.eq_ignore_ascii_case("powershell.exe")
                    && a.acceso & guardiana_escudo::credenciales::LEE_MEMORIA != 0),
            "la apertura de LSASS por PowerShell"
        );
    }

    /// An ISO image with a copy of `whoami.exe`, made with Windows' own IMAPI2, mounted, the program run from it and the
    /// image taken out again; what the PowerShell says.
    const ISO: &str = r#"
$ErrorActionPreference = 'Stop'
$dir = Join-Path $env:TEMP 'guardiana-iso'
$iso = Join-Path $env:TEMP 'guardiana-prueba.iso'
New-Item -ItemType Directory -Force -Path $dir | Out-Null
Copy-Item "$env:SystemRoot\System32\whoami.exe" (Join-Path $dir 'whoami.exe') -Force
$cp = New-Object CodeDom.Compiler.CompilerParameters
$cp.CompilerOptions = '/unsafe'
Add-Type -CompilerParameters $cp -TypeDefinition @'
public class GuardianaIso {
  public unsafe static void Escribe(string ruta, object flujo, int bloque, int bloques) {
    int leidos = 0; byte[] buf = new byte[bloque]; var p = (System.IntPtr)(&leidos);
    var salida = System.IO.File.OpenWrite(ruta); var entrada = flujo as System.Runtime.InteropServices.ComTypes.IStream;
    while (bloques-- > 0) { entrada.Read(buf, bloque, p); salida.Write(buf, 0, leidos); }
    salida.Flush(); salida.Close();
  }
}
'@
$fsi = New-Object -ComObject IMAPI2FS.MsftFileSystemImage
$fsi.FileSystemsToCreate = 3
$fsi.VolumeName = 'GUARDIANA'
$fsi.Root.AddTree($dir, $false)
$r = $fsi.CreateResultImage()
[GuardianaIso]::Escribe($iso, $r.ImageStream, $r.BlockSize, $r.TotalBlocks)
$letra = (Mount-DiskImage -ImagePath $iso -PassThru | Get-Volume).DriveLetter
Write-Output "montada en $letra"
& "$($letra):\whoami.exe" | Out-Null
Start-Sleep -Seconds 3
Dismount-DiskImage -ImagePath $iso | Out-Null
Remove-Item $iso, $dir -Recurse -Force
"#;

    /// On a real Windows, as an administrator: a program run from a mounted ISO image is seen as come from an image
    /// (escudo, «medio»).
    #[test]
    #[ignore = "necesita Windows como administrador"]
    fn la_guardia_ve_lo_que_viene_de_una_imagen() {
        let sesion = Sesion::arrancar().unwrap();
        std::thread::sleep(Duration::from_secs(3));
        let salida = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", ISO])
            .output()
            .unwrap();
        println!(
            "la imagen: {} {}",
            String::from_utf8_lossy(&salida.stdout).trim(),
            String::from_utf8_lossy(&salida.stderr).trim()
        );
        assert!(salida.status.success(), "la ISO de prueba");
        std::thread::sleep(Duration::from_secs(3));
        let medios = sesion.desde_medios();
        println!("desde medios: {medios:?}");
        assert!(
            medios
                .iter()
                .any(|m| m.programa.eq_ignore_ascii_case("whoami.exe") && m.medio == "imagen"),
            "whoami.exe abierto desde la imagen"
        );
    }

    /// This process's processor time so far (kernel and user), in milliseconds.
    fn cpu_ms() -> u64 {
        use windows_sys::Win32::Foundation::FILETIME;
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
        let cero = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let (mut c, mut s, mut k, mut u) = (cero, cero, cero, cero);
        // SAFETY: this process's own pseudo-handle and four FILETIMEs owned here.
        #[allow(unsafe_code)]
        let ok = unsafe { GetProcessTimes(GetCurrentProcess(), &mut c, &mut s, &mut k, &mut u) };
        assert_ne!(ok, 0);
        let a = |f: FILETIME| (u64::from(f.dwHighDateTime) << 32) | u64::from(f.dwLowDateTime);
        (a(k) + a(u)) / 10_000
    }

    /// On a real Windows, as an administrator: what the escudo's session costs. Another process creates and reads
    /// 20 000 files; the test says how many openings arrived, the processor time this process spent in all (Windows'
    /// delivery of the events included), and how much of it went inside the callback and into reading the fields.
    #[test]
    #[ignore = "necesita Windows como administrador"]
    fn cuanto_cuesta_la_guardia() {
        let sesion = Sesion::arrancar().unwrap();
        std::thread::sleep(Duration::from_secs(3));
        let antes_eventos = medida::CREADOS.load(Ordering::Relaxed);
        let antes_llamada = medida::EN_LLAMADA_NS.load(Ordering::Relaxed);
        let antes_leyendo = medida::LEYENDO_NS.load(Ordering::Relaxed);
        let antes = cpu_ms();
        let tmp = std::env::temp_dir().join("guardiana-carga");
        std::fs::create_dir_all(&tmp).unwrap();
        powershell(&format!(
            "$d = '{}'; 1..20000 | ForEach-Object {{ [IO.File]::WriteAllText(\"$d\\f$_.txt\", 'x') }}; \
             Get-ChildItem -LiteralPath $d -File | ForEach-Object {{ [void][IO.File]::ReadAllBytes($_.FullName) }}",
            tmp.display()
        ));
        std::thread::sleep(Duration::from_secs(5));
        let gastado = cpu_ms().saturating_sub(antes);
        let eventos = medida::CREADOS
            .load(Ordering::Relaxed)
            .saturating_sub(antes_eventos);
        let llamada_ms = medida::EN_LLAMADA_NS
            .load(Ordering::Relaxed)
            .saturating_sub(antes_llamada)
            / 1_000_000;
        let leyendo_ms = medida::LEYENDO_NS
            .load(Ordering::Relaxed)
            .saturating_sub(antes_leyendo)
            / 1_000_000;
        drop(sesion);
        let _ = std::fs::remove_dir_all(&tmp);
        let por_evento = |ms: u64| ms * 1_000_000 / eventos.max(1);
        println!(
            "coste: {eventos} aperturas, {gastado} ms de procesador en total ({} ns por apertura), {llamada_ms} ms \
             dentro de la llamada ({} ns), {leyendo_ms} ms leyendo los campos ({} ns)",
            por_evento(gastado),
            por_evento(llamada_ms),
            por_evento(leyendo_ms)
        );
        assert!(eventos > 20_000, "llegaron {eventos} aperturas");
    }
}
