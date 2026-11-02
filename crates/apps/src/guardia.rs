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
use guardiana_escudo::{cadenas, disfraz, ladrones, secuestro};

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
    /// Every program started since the session began: process number → (name, path, when). A program that has
    /// already ended when its events arrive (a `cmd /c` that opened a secret and left; the real test found it) is
    /// still named from here; a live one is asked to Windows first, so a reused number is never misnamed.
    iniciados: HashMap<u32, (String, String, i64)>,
    /// The volumes' device names (`\device\harddiskvolume3`) and their letters (`C:`), read when the session starts:
    /// the kernel names a started program by its device path.
    volumenes: Vec<(String, String)>,
    /// For the real test only: how many openings arrived, and the ones of a «Login Data» file, before any filter.
    #[cfg(test)]
    prueba_creados: u64,
    #[cfg(test)]
    prueba_login: Vec<String>,
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
    // a program wearing the name of one of Windows' own outside Windows' folder (escudo, «disfraz») goes on even
    // without a parent known
    let disfrazado = disfraz::sospechoso(&hijo, &imagen);
    if !cadenas::vigilado(&hijo) && !disfrazado {
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
    if padre.is_empty() && !disfrazado {
        return;
    }
    let ahora = guardiana_core::time::now_ms();
    let Ok(mut d) = datos.lock() else {
        return;
    };
    let clave = if disfrazado {
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
    let Ok(esquema) = esquemas.event_schema(registro) else {
        return;
    };
    let lector = Parser::create(registro, &esquema);
    #[cfg(test)]
    {
        let nombre = lector.try_parse::<String>("FileName");
        let opciones = lector.try_parse::<u32>("CreateOptions");
        if let Ok(mut d) = datos.lock() {
            d.prueba_creados += 1;
            if let Ok(n) = &nombre {
                if n.to_lowercase().contains("login data") && d.prueba_login.len() < 20 {
                    d.prueba_login.push(format!("{pid} {n} {opciones:?}"));
                }
            } else if d.prueba_login.len() < 3 {
                d.prueba_login
                    .push(format!("FileName ilegible: {nombre:?}"));
            }
        }
    }
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;
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
        std::thread::sleep(Duration::from_secs(4));
        let aperturas = sesion.toma();
        let lanzamientos = sesion.lanzamientos();
        let rafagas = sesion.rafagas();
        if let Ok(d) = sesion.datos.lock() {
            println!("aperturas recibidas: {}", d.prueba_creados);
            println!("de «Login Data»: {:?}", d.prueba_login);
        }
        let _ = std::fs::remove_dir_all(&carpeta);
        let _ = std::fs::remove_file(&secreto);
        println!("aperturas: {aperturas:?}");
        println!("lanzamientos: {lanzamientos:?}");
        println!("ráfagas: {rafagas:?}");
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
    }
}
