//! El escudo en marcha (1.0.52, in preparation; docs/ESCUDO.md): the antivirus' behaviour rules. «Primera vez»
//! (`guardiana_escudo::primera_vez`), fed with this computer's DNS events once the observer has said which program
//! asked; and «ladrones de datos» (`guardiana_escudo::ladrones`), fed with the openings of saved secrets that the
//! watch in `guardiana_apps::Guardia` sees; and «cadenas» (`guardiana_escudo::cadenas`), fed with the program starts
//! that the same watch sees. Its memory and its warnings live next to the ledger, in two small files: `escudo-memoria.json` (since when
//! this computer asks for each site) and `escudo-avisos.jsonl` (the newest warnings, one per line, for the panel).

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use guardiana_apps::{Apertura, Contenido, DesdeMedio, Guardia, Observador, Rafaga};
use guardiana_core::vigilancia::Suceso;
use guardiana_core::{Ledger, NewEvent};
use guardiana_escudo::arranque::Archivo;
use guardiana_escudo::{Aviso, Firma, Huellas, Memoria, Mirada, Programa};

/// Warnings kept, the newest.
const MAX_AVISOS: usize = 500;
/// Queries waiting for Windows to check their program's signature, at most.
const MAX_PENDIENTES: usize = 1_000;
/// How long a query waits for that check.
const ESPERA_FIRMA_MS: u64 = 60_000;
/// How often the memory is written down when it changed.
const GUARDAR_CADA_MS: u64 = 5 * 60_000;
/// The same program touching the same kind of secret of the same app warns once a day.
const REPETIR_LADRON_MS: u64 = 24 * 60 * 60_000;
/// The same program starting the same program warns once an hour.
const REPETIR_CADENA_MS: u64 = 60 * 60_000;
/// The same program renaming or deleting many of the person's files warns once an hour.
const REPETIR_SECUESTRO_MS: u64 = 60 * 60_000;
/// A file is asked to Microsoft Defender at most once a day.
const REPETIR_DEFENSOR_MS: u64 = 24 * 60 * 60_000;
/// How often the decoys against ransomware are looked at (their size and time of writing), and put back when missing.
const MIRAR_SENUELOS_MS: u64 = 2_000;
const PLANTAR_SENUELOS_MS: u64 = 60 * 60_000;
/// How often the programs the person let carry on are read again.
const LEER_PERMITIDOS_MS: u64 = 30_000;

/// Microsoft Defender's second opinion (`guardiana_escudo::defensor`): a thread of its own runs its scan of one file at
/// a time (a few seconds each), so the escudo's loop never waits for it. Only on Windows.
#[cfg_attr(not(windows), allow(dead_code))]
struct Defensor {
    pide: std::sync::mpsc::Sender<(String, String)>,
    respuestas: std::sync::mpsc::Receiver<(String, String, Vec<String>)>,
}

impl Defensor {
    /// The thread, on Windows; nothing elsewhere.
    fn lanza() -> Option<Self> {
        #[cfg(windows)]
        {
            let (pide, peticiones) = std::sync::mpsc::channel::<(String, String)>();
            let (envia, respuestas) = std::sync::mpsc::channel();
            let hilo = std::thread::Builder::new()
                .name("escudo-defender".into())
                .spawn(move || {
                    for (programa, ruta) in peticiones {
                        let amenazas = pregunta_a_defender(&ruta);
                        if envia.send((programa, ruta, amenazas)).is_err() {
                            break;
                        }
                    }
                });
            hilo.ok().map(|_| Self { pide, respuestas })
        }
        #[cfg(not(windows))]
        {
            None
        }
    }
}

/// Microsoft Defender's scan of one file, without remediation (it only says), and the threats it names.
#[cfg(windows)]
fn pregunta_a_defender(ruta: &str) -> Vec<String> {
    let mp = std::env::var_os("ProgramFiles")
        .map_or_else(|| PathBuf::from(r"C:\Program Files"), PathBuf::from)
        .join(r"Windows Defender\MpCmdRun.exe");
    let Ok(salida) = std::process::Command::new(mp)
        .args([
            "-Scan",
            "-ScanType",
            "3",
            "-File",
            ruta,
            "-DisableRemediation",
        ])
        .output()
    else {
        return Vec::new();
    };
    guardiana_escudo::defensor::amenazas(&String::from_utf8_lossy(&salida.stdout))
}

/// The most connections without a name waiting for the escudo.
const MAX_SIN_NOMBRE: usize = 256;
/// The same program warns once a day for its connections without a name.
const REPETIR_SIN_NOMBRE_MS: u64 = 24 * 60 * 60_000;
/// How long a name this computer asked for counts for its address (the traffic's own: six hours).
const VIGENCIA_NOMBRE_MS: i64 = 6 * 60 * 60_000;
/// GUARDIANA counts as this computer's DNS when it answered a name within this time; otherwise the rule is quiet
/// (with another DNS, every connection would look nameless).
const DNS_RECIENTE_MS: i64 = 10 * 60_000;

/// A program seen connecting straight to an address on the internet that no name pointed to: the traffic counter
/// finds it, the escudo judges it (`guardiana_escudo::sin_nombre`). The address is not kept: only its network and
/// country.
pub(crate) struct SinNombre {
    programa: String,
    ruta: String,
    donde: String,
    ts: u64,
}

/// From one take of the traffic: each connection of a program not known to be signed to an address on the internet
/// (not local, not anycast) that no name answered in the last hours pointed to. Only while GUARDIANA is this computer's
/// DNS; at most one waiting per program.
pub(crate) fn anota_sin_nombre(
    medidas: &[guardiana_apps::Medida],
    nombres: &Mutex<HashMap<IpAddr, (String, i64)>>,
    cola: &Mutex<VecDeque<SinNombre>>,
    ahora: i64,
) {
    if medidas.is_empty() {
        return;
    }
    let Ok(nombres) = nombres.lock() else {
        return;
    };
    if !nombres.values().any(|(_, t)| ahora - t < DNS_RECIENTE_MS) {
        return;
    }
    let mut nuevos = Vec::new();
    for m in medidas {
        let ruta = &m.proceso.ruta;
        if ruta.is_empty() || !m.proceso.firmado_por.is_empty() {
            continue;
        }
        if nombres
            .get(&m.ip)
            .is_some_and(|(_, t)| ahora - t < VIGENCIA_NOMBRE_MS)
        {
            continue;
        }
        let s = guardiana_lists::servidor_de(m.ip);
        if s.local || s.anycast {
            continue;
        }
        let donde = match (s.red, s.pais) {
            (Some(r), Some(p)) => format!("{}, {p}", r.nombre),
            (Some(r), None) => r.nombre.to_string(),
            (None, Some(p)) => p.to_owned(),
            (None, None) => String::new(),
        };
        nuevos.push(SinNombre {
            programa: m.proceso.nombre.clone(),
            ruta: ruta.clone(),
            donde,
            ts: u64::try_from(ahora).unwrap_or(0),
        });
    }
    drop(nombres);
    if nuevos.is_empty() {
        return;
    }
    if let Ok(mut c) = cola.lock() {
        for n in nuevos {
            if c.len() >= MAX_SIN_NOMBRE {
                break;
            }
            if !c.iter().any(|x| x.ruta == n.ruta) {
                c.push_back(n);
            }
        }
    }
}

/// How often the hosts file is read.
const HOSTS_CADA_MS: u64 = 60_000;
/// How often the continuous watch's history is read for new autostarts.
const ARRANQUE_CADA_MS: u64 = 30_000;
/// On the first read, the autostarts the watch noted in the last day are looked at too.
const ARRANQUE_ATRAS_MS: i64 = 24 * 60 * 60_000;

/// A new autostart's program file waiting for Windows to check its signature.
struct PendienteArranque {
    titulo: String,
    tipo: String,
    clave: String,
    archivo: Archivo,
    cuando_ms: u64,
    visto_ms: u64,
}

/// A query whose program's signature was not checked yet.
struct Pendiente {
    nombre: String,
    sitio: String,
    cuando_ms: u64,
    programa: String,
    ruta: String,
    sha256: String,
}

/// The rule's state while the service runs.
pub(crate) struct EscudoVivo {
    memoria: Memoria,
    listas: Vec<Huellas>,
    pendientes: Vec<Pendiente>,
    ruta_memoria: PathBuf,
    ruta_avisos: PathBuf,
    guardada_ms: u64,
    cambios: bool,
    /// Program file → whether Windows marked it as come from the internet (read once per file).
    marcas: HashMap<String, bool>,
    /// The watch over saved secrets (Windows only; `None` when it could not start).
    guardia: Option<Guardia>,
    /// Openings of saved secrets whose program's signature was not checked yet.
    aperturas: Vec<Apertura>,
    /// (program, app, kind of secret) → when it last warned.
    ladrones_vistos: HashMap<(String, String, String), u64>,
    /// (parent's path, started program) → when it last warned.
    cadenas_vistas: HashMap<(String, String), u64>,
    /// The watch's history is read up to this row (its id) and this time (its `ts`).
    arranque_id: i64,
    arranque_ts: i64,
    /// When the history was last read.
    arranque_leido_ms: u64,
    /// New autostarts whose program's signature was not checked yet.
    arranques: Vec<PendienteArranque>,
    /// Bursts of the person's files renamed or deleted whose program's signature was not checked yet.
    rafagas: Vec<Rafaga>,
    /// Program path → when its last burst warned.
    secuestros_vistos: HashMap<String, u64>,
    /// Programs started from a disc, a disk image or a removable drive whose signature was not checked yet.
    medios_pend: Vec<DesdeMedio>,
    /// Connections without a name whose program's signature was not checked yet, and program path → when it was
    /// last judged.
    sin_nombre_pend: Vec<SinNombre>,
    sin_nombre_vistos: HashMap<String, u64>,
    /// Microsoft Defender's second opinion on the files of grave warnings, and file → when it was last asked.
    defensor: Option<Defensor>,
    defensor_vistos: RefCell<HashMap<String, u64>>,
    /// The hosts file as last read (name → address), and where it is kept between restarts.
    hosts: Option<BTreeMap<String, String>>,
    ruta_hosts: PathBuf,
    hosts_leido_ms: u64,
    /// The decoys against ransomware (`guardiana_escudo::senuelos`): each one's path and its time of writing as left
    /// here; when they were last looked at and last planted.
    senuelos: Vec<(PathBuf, std::time::SystemTime)>,
    senuelos_mirados_ms: u64,
    senuelos_plantados_ms: u64,
    /// The programs the person let carry on after they were frozen (their paths, in lower case), as the panel writes
    /// them (`escudo-permitidos.jsonl`), and when they were last read.
    permitidos: std::collections::HashSet<String>,
    ruta_permitidos: PathBuf,
    permitidos_leidos_ms: u64,
}

impl EscudoVivo {
    /// The memory kept next to the ledger at `db`, or a new one that starts learning now.
    pub(crate) fn abrir(db: &Path, ahora_ms: u64) -> Self {
        let ruta_memoria = db.with_file_name("escudo-memoria.json");
        let ruta_hosts = db.with_file_name("escudo-hosts.json");
        let hosts = std::fs::read_to_string(&ruta_hosts)
            .ok()
            .and_then(|j| serde_json::from_str::<BTreeMap<String, String>>(&j).ok());
        let memoria = std::fs::read_to_string(&ruta_memoria)
            .ok()
            .and_then(|j| Memoria::desde_json(&j).ok())
            .unwrap_or_else(|| Memoria::nueva(ahora_ms));
        Self {
            memoria,
            listas: Vec::new(),
            pendientes: Vec::new(),
            ruta_memoria,
            ruta_avisos: db.with_file_name("escudo-avisos.jsonl"),
            guardada_ms: ahora_ms,
            cambios: false,
            marcas: HashMap::new(),
            guardia: match Guardia::arrancar() {
                Ok(g) => Some(g),
                Err(guardiana_apps::Error::NoSoportado) => None,
                Err(e) => {
                    eprintln!("guardiana: escudo: {e}");
                    None
                }
            },
            aperturas: Vec::new(),
            ladrones_vistos: HashMap::new(),
            cadenas_vistas: HashMap::new(),
            arranque_id: 0,
            arranque_ts: i64::try_from(ahora_ms).unwrap_or(0) - ARRANQUE_ATRAS_MS,
            arranque_leido_ms: 0,
            arranques: Vec::new(),
            rafagas: Vec::new(),
            secuestros_vistos: HashMap::new(),
            medios_pend: Vec::new(),
            hosts,
            ruta_hosts,
            hosts_leido_ms: 0,
            sin_nombre_pend: Vec::new(),
            sin_nombre_vistos: HashMap::new(),
            defensor: Defensor::lanza(),
            defensor_vistos: RefCell::new(HashMap::new()),
            senuelos: Vec::new(),
            senuelos_mirados_ms: 0,
            senuelos_plantados_ms: 0,
            permitidos: std::collections::HashSet::new(),
            ruta_permitidos: db.with_file_name("escudo-permitidos.jsonl"),
            permitidos_leidos_ms: 0,
        }
    }

    /// Looks at one of this computer's queries, with the program the observer gave it.
    pub(crate) fn mira(&mut self, e: &NewEvent, obs: &Observador) {
        let sitio = guardiana_lists::dominio::sitio(&e.qname);
        let cuando = u64::try_from(e.ts).unwrap_or(0);
        let antes = self.memoria.sitios();
        let mirada = match e.process.as_ref() {
            None => self
                .memoria
                .mira(&e.qname, &sitio, cuando, None, &self.listas),
            Some(p) => {
                let descargado = self.descargado(&p.ruta);
                let programa = Programa {
                    nombre: &p.nombre,
                    ruta: &p.ruta,
                    sha256: &p.sha256,
                    firma: firma_de(obs, &p.ruta, &p.firmado_por),
                    descargado,
                };
                let m = self
                    .memoria
                    .mira(&e.qname, &sitio, cuando, Some(&programa), &self.listas);
                if m == Mirada::Pendiente && self.pendientes.len() < MAX_PENDIENTES {
                    self.pendientes.push(Pendiente {
                        nombre: e.qname.clone(),
                        sitio: sitio.clone(),
                        cuando_ms: cuando,
                        programa: p.nombre.clone(),
                        ruta: p.ruta.clone(),
                        sha256: p.sha256.clone(),
                    });
                }
                m
            }
        };
        if self.memoria.sitios() != antes {
            self.cambios = true;
        }
        if let Mirada::Aviso(a) = mirada {
            self.anota(&a, a.regla == "muestra_conocida");
        }
    }

    /// Looks again at the queries that were waiting for a signature check, and at the openings of saved secrets.
    pub(crate) fn repasa(&mut self, obs: &Observador, ahora_ms: u64) {
        self.respuestas_de_defender(ahora_ms);
        self.lee_permitidos(ahora_ms);
        self.contenidos(ahora_ms);
        self.mira_senuelos(ahora_ms);
        self.secuestros(obs, ahora_ms);
        self.mira_hosts(ahora_ms);
        self.ladrones(obs, ahora_ms);
        self.cadenas();
        self.credenciales();
        self.medios(obs, ahora_ms);
        self.arranques_pendientes(obs, ahora_ms);
        if self.pendientes.is_empty() {
            return;
        }
        for p in std::mem::take(&mut self.pendientes) {
            match obs.firma_comprobada(&p.ruta) {
                Some(f) => {
                    let descargado = self.descargado(&p.ruta);
                    let programa = Programa {
                        nombre: &p.programa,
                        ruta: &p.ruta,
                        sha256: &p.sha256,
                        firma: if f.is_empty() {
                            Firma::SinFirma
                        } else {
                            Firma::Firmado(f)
                        },
                        descargado,
                    };
                    if let Some(a) =
                        self.memoria
                            .revisa(&p.nombre, &p.sitio, p.cuando_ms, &programa)
                    {
                        self.anota(&a, a.regla == "muestra_conocida");
                    }
                }
                None if ahora_ms.saturating_sub(p.cuando_ms) < ESPERA_FIRMA_MS => {
                    self.pendientes.push(p)
                }
                None => {}
            }
        }
    }

    /// Whether Windows marked the program's file as come from the internet: its `Zone.Identifier` stream says zone 3
    /// (the internet) or 4 (untrusted sites). Read once per file; the address it came from is never read.
    fn descargado(&mut self, ruta: &str) -> bool {
        if let Some(m) = self.marcas.get(ruta) {
            return *m;
        }
        if self.marcas.len() >= 4_096 {
            self.marcas.clear();
        }
        let m = marca_de_internet(ruta);
        self.marcas.insert(ruta.to_owned(), m);
        m
    }

    /// The openings of saved secrets (browser passwords, cookies and keys, wallets, sessions) by programs that are not
    /// the app's own: each one, once Windows has checked the program's signature, through the rule
    /// (`guardiana_escudo::ladrones::juzga`). Another antivirus product reading files is its job, not a theft.
    fn ladrones(&mut self, obs: &Observador, ahora_ms: u64) {
        let nuevas = self.guardia.as_ref().map(Guardia::toma).unwrap_or_default();
        if nuevas.is_empty() && self.aperturas.is_empty() {
            return;
        }
        let mut todas = std::mem::take(&mut self.aperturas);
        todas.extend(nuevas);
        for a in todas {
            let cuando = u64::try_from(a.ts).unwrap_or(0);
            let firma = match obs.firma(&a.ruta) {
                Some(f) if f.is_empty() => Firma::SinFirma,
                Some(f) => Firma::Firmado(f),
                None if ahora_ms.saturating_sub(cuando) < ESPERA_FIRMA_MS => {
                    if self.aperturas.len() < MAX_PENDIENTES {
                        self.aperturas.push(a);
                    }
                    continue;
                }
                None => Firma::SinComprobar,
            };
            if matches!(firma, Firma::Firmado(_))
                && guardiana_lists::antivirus::antivirus_de_programa(&a.programa).is_some()
            {
                continue;
            }
            let Some((aviso, grave)) =
                guardiana_escudo::ladrones::juzga(&a.programa, &a.ruta, &firma, &a.archivo, cuando)
            else {
                continue;
            };
            let clave = (
                a.ruta.to_ascii_lowercase(),
                aviso.sitio.clone(),
                aviso.nombre.clone(),
            );
            if self
                .ladrones_vistos
                .get(&clave)
                .is_some_and(|t| cuando.saturating_sub(*t) < REPETIR_LADRON_MS)
            {
                continue;
            }
            if self.ladrones_vistos.len() > 4_096 {
                self.ladrones_vistos.clear();
            }
            self.ladrones_vistos.insert(clave, cuando);
            self.anota(&aviso, grave);
        }
    }

    /// The program starts the watch saw, each with the program that started it, through the rule
    /// (`guardiana_escudo::cadenas::juzga`), with whether Windows marked the parent's file as come from the internet.
    fn cadenas(&mut self) {
        let nuevos = self
            .guardia
            .as_ref()
            .map(Guardia::lanzamientos)
            .unwrap_or_default();
        for l in nuevos {
            let cuando = u64::try_from(l.ts).unwrap_or(0);
            // a program wearing the name of one of Windows' own outside Windows' folder: once an hour per file
            if let Some((aviso, grave)) =
                guardiana_escudo::disfraz::juzga(&l.hijo, &l.imagen, cuando)
            {
                let clave = (aviso.nombre.clone(), "disfraz".to_owned());
                if !self
                    .cadenas_vistas
                    .get(&clave)
                    .is_some_and(|t| cuando.saturating_sub(*t) < REPETIR_CADENA_MS)
                {
                    if self.cadenas_vistas.len() > 4_096 {
                        self.cadenas_vistas.clear();
                    }
                    self.cadenas_vistas.insert(clave, cuando);
                    self.anota(&aviso, grave);
                }
            }
            // a name made to look like a document: once an hour per file
            if let Some((aviso, grave)) =
                guardiana_escudo::engano::juzga(&l.hijo, &l.imagen, cuando)
            {
                let clave = (aviso.nombre.clone(), "engano".to_owned());
                if !self
                    .cadenas_vistas
                    .get(&clave)
                    .is_some_and(|t| cuando.saturating_sub(*t) < REPETIR_CADENA_MS)
                {
                    if self.cadenas_vistas.len() > 4_096 {
                        self.cadenas_vistas.clear();
                    }
                    self.cadenas_vistas.insert(clave, cuando);
                    self.anota(&aviso, grave);
                }
            }
            if l.padre.is_empty() {
                continue;
            }
            let descargado = self.descargado(&l.padre_ruta);
            let Some((aviso, grave)) = guardiana_escudo::cadenas::juzga(
                &l.padre,
                &l.padre_ruta,
                descargado,
                &l.hijo,
                cuando,
            ) else {
                continue;
            };
            let clave = (l.padre_ruta.to_ascii_lowercase(), aviso.nombre.clone());
            if self
                .cadenas_vistas
                .get(&clave)
                .is_some_and(|t| cuando.saturating_sub(*t) < REPETIR_CADENA_MS)
            {
                continue;
            }
            if self.cadenas_vistas.len() > 4_096 {
                self.cadenas_vistas.clear();
            }
            self.cadenas_vistas.insert(clave, cuando);
            self.anota(&aviso, grave);
        }
    }

    /// The programs started from a disc, a mounted disk image or a removable drive, once Windows has checked each one's
    /// signature (or after [`ESPERA_FIRMA_MS`]), through the rule (`guardiana_escudo::medios`). The same file warns once
    /// an hour.
    fn medios(&mut self, obs: &Observador, ahora_ms: u64) {
        let nuevos = self
            .guardia
            .as_ref()
            .map(Guardia::desde_medios)
            .unwrap_or_default();
        if nuevos.is_empty() && self.medios_pend.is_empty() {
            return;
        }
        let mut todos = std::mem::take(&mut self.medios_pend);
        todos.extend(nuevos);
        for m in todos {
            let Some(medio) = guardiana_escudo::medios::Medio::de_codigo(m.medio) else {
                continue;
            };
            let cuando = u64::try_from(m.ts).unwrap_or(0);
            let firma = match obs.firma(&m.ruta) {
                Some(f) if f.is_empty() => Firma::SinFirma,
                Some(f) => Firma::Firmado(f),
                None if ahora_ms.saturating_sub(cuando) < ESPERA_FIRMA_MS => {
                    if self.medios_pend.len() < MAX_PENDIENTES {
                        self.medios_pend.push(m);
                    }
                    continue;
                }
                None => Firma::SinComprobar,
            };
            let Some((aviso, grave)) =
                guardiana_escudo::medios::juzga(&m.programa, &m.ruta, &firma, medio, cuando)
            else {
                continue;
            };
            let clave = (aviso.nombre.clone(), "medio".to_owned());
            if self
                .cadenas_vistas
                .get(&clave)
                .is_some_and(|t| cuando.saturating_sub(*t) < REPETIR_CADENA_MS)
            {
                continue;
            }
            if self.cadenas_vistas.len() > 4_096 {
                self.cadenas_vistas.clear();
            }
            self.cadenas_vistas.insert(clave, cuando);
            self.anota(&aviso, grave);
        }
    }

    /// The openings of LSASS that asked to read its memory, through the rule (`guardiana_escudo::credenciales`). The
    /// same program warns once an hour.
    fn credenciales(&mut self) {
        let nuevos = self
            .guardia
            .as_ref()
            .map(Guardia::accesos)
            .unwrap_or_default();
        for a in nuevos {
            let cuando = u64::try_from(a.ts).unwrap_or(0);
            let Some((aviso, grave)) = guardiana_escudo::credenciales::juzga(
                &a.programa,
                &a.ruta,
                a.acceso,
                a.concedido,
                cuando,
            ) else {
                continue;
            };
            let clave = (aviso.nombre.clone(), "credenciales".to_owned());
            if self
                .cadenas_vistas
                .get(&clave)
                .is_some_and(|t| cuando.saturating_sub(*t) < REPETIR_CADENA_MS)
            {
                continue;
            }
            if self.cadenas_vistas.len() > 4_096 {
                self.cadenas_vistas.clear();
            }
            self.cadenas_vistas.insert(clave, cuando);
            self.anota(&aviso, grave);
        }
    }

    /// The bursts of the person's files renamed or deleted by one program, once Windows has checked its signature,
    /// through the rule (`guardiana_escudo::secuestro::juzga`). The same program warns once an hour.
    fn secuestros(&mut self, obs: &Observador, ahora_ms: u64) {
        let nuevas = self
            .guardia
            .as_ref()
            .map(Guardia::rafagas)
            .unwrap_or_default();
        if nuevas.is_empty() && self.rafagas.is_empty() {
            return;
        }
        let mut todas = std::mem::take(&mut self.rafagas);
        todas.extend(nuevas);
        for r in todas {
            let cuando = u64::try_from(r.ts).unwrap_or(0);
            let firma = match obs.firma(&r.ruta) {
                Some(f) if f.is_empty() => Firma::SinFirma,
                Some(f) => Firma::Firmado(f),
                None if ahora_ms.saturating_sub(cuando) < ESPERA_FIRMA_MS => {
                    if self.rafagas.len() < MAX_PENDIENTES {
                        self.rafagas.push(r);
                    }
                    continue;
                }
                None => Firma::SinComprobar,
            };
            let descargado = self.descargado(&r.ruta);
            let Some((aviso, grave)) = guardiana_escudo::secuestro::juzga(
                &r.programa,
                &r.ruta,
                &firma,
                descargado,
                r.n,
                cuando,
            ) else {
                continue;
            };
            let clave = r.ruta.to_ascii_lowercase();
            if self
                .secuestros_vistos
                .get(&clave)
                .is_some_and(|t| cuando.saturating_sub(*t) < REPETIR_SECUESTRO_MS)
            {
                continue;
            }
            if self.secuestros_vistos.len() > 4_096 {
                self.secuestros_vistos.clear();
            }
            self.secuestros_vistos.insert(clave.clone(), cuando);
            // grave (no signature, downloaded, or in a folder anyone writes to): frozen with its family, unless the
            // person let it carry on before (2 Nov 2026)
            let congelados = if grave && r.pid != 0 && !self.permitidos.contains(&clave) {
                guardiana_apps::congela_familia(r.pid)
            } else {
                Vec::new()
            };
            self.anota_congelado(&aviso, grave, r.pid, cuando, &congelados, "");
        }
    }

    /// The programs the watch froze the moment it caught them (a decoy deleted or renamed, the backups' deletion):
    /// each one a warning, with what was frozen. A program the person let carry on before is let go again at once.
    fn contenidos(&mut self, ahora_ms: u64) {
        let nuevos: Vec<Contenido> = self
            .guardia
            .as_ref()
            .map(Guardia::contenidos)
            .unwrap_or_default();
        for c in nuevos {
            let cuando = u64::try_from(c.ts).unwrap_or(ahora_ms);
            if self.permitidos.contains(&c.ruta.to_ascii_lowercase()) {
                for pid in &c.congelados {
                    let _ = guardiana_apps::reanuda(*pid);
                }
                continue;
            }
            let (aviso, grave) = match c.regla {
                "copias_borradas" => (
                    guardiana_escudo::contencion::aviso_copias(
                        &c.programa,
                        &c.ruta,
                        &c.padre,
                        cuando,
                    ),
                    true,
                ),
                _ => {
                    let toque = match c.toque {
                        "borrado" => guardiana_escudo::senuelos::Toque::Borrado,
                        "renombrado" => guardiana_escudo::senuelos::Toque::Renombrado,
                        _ => guardiana_escudo::senuelos::Toque::Escrito,
                    };
                    let (a, accion) = guardiana_escudo::senuelos::juzga(
                        &c.programa,
                        &c.ruta,
                        &c.archivo,
                        toque,
                        cuando,
                    );
                    (a, accion == guardiana_escudo::senuelos::Accion::Congelar)
                }
            };
            self.anota_congelado(&aviso, grave, c.pid, cuando, &c.congelados, &c.archivo);
        }
    }

    /// The paths of the programs the person let carry on (the panel writes one per line: `{"ruta": …}`).
    fn lee_permitidos(&mut self, ahora_ms: u64) {
        if ahora_ms.saturating_sub(self.permitidos_leidos_ms) < LEER_PERMITIDOS_MS {
            return;
        }
        self.permitidos_leidos_ms = ahora_ms;
        self.permitidos = std::fs::read_to_string(&self.ruta_permitidos)
            .map(|t| {
                t.lines()
                    .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                    .filter_map(|v| v["ruta"].as_str().map(str::to_ascii_lowercase))
                    .collect()
            })
            .unwrap_or_default();
    }

    /// The decoys against ransomware: planted when missing (at start and every hour), and looked at every two seconds. One
    /// written over in place (its size or its time of writing changed) has as author its last opener, which is frozen
    /// with its family; the decoy is then written again.
    fn mira_senuelos(&mut self, ahora_ms: u64) {
        if ahora_ms.saturating_sub(self.senuelos_plantados_ms) >= PLANTAR_SENUELOS_MS {
            self.senuelos_plantados_ms = ahora_ms;
            self.senuelos = planta_senuelos();
        }
        if ahora_ms.saturating_sub(self.senuelos_mirados_ms) < MIRAR_SENUELOS_MS {
            return;
        }
        self.senuelos_mirados_ms = ahora_ms;
        let mut tocados = Vec::new();
        for (ruta, cuando) in &self.senuelos {
            let cambiado = match std::fs::metadata(ruta) {
                Ok(m) => {
                    m.len() != guardiana_escudo::senuelos::TAMANO as u64
                        || m.modified().ok() != Some(*cuando)
                }
                // deleted: the watch saw who did it (a warning already); it is planted again within the hour
                Err(_) => false,
            };
            if cambiado {
                tocados.push(ruta.clone());
            }
        }
        for ruta in tocados {
            let archivo = ruta.to_string_lossy().into_owned();
            let autor = self
                .guardia
                .as_ref()
                .and_then(|g| g.abridor_de_senuelo(&archivo));
            if let Some((pid, programa, ruta_prog, _)) = autor {
                let permitido = self.permitidos.contains(&ruta_prog.to_ascii_lowercase());
                let explorador = programa.eq_ignore_ascii_case("explorer.exe");
                let congelados = if permitido || explorador {
                    Vec::new()
                } else {
                    guardiana_apps::congela_familia(pid)
                };
                if !permitido {
                    let (aviso, accion) = guardiana_escudo::senuelos::juzga(
                        &programa,
                        &ruta_prog,
                        &archivo,
                        guardiana_escudo::senuelos::Toque::Escrito,
                        ahora_ms,
                    );
                    let grave = accion == guardiana_escudo::senuelos::Accion::Congelar;
                    self.anota_congelado(&aviso, grave, pid, ahora_ms, &congelados, &archivo);
                }
            }
            // written again, so the next change is seen too
            if let Some(nuevo) = escribe_senuelo(&ruta) {
                if let Some(x) = self.senuelos.iter_mut().find(|(r, _)| *r == ruta) {
                    x.1 = nuevo;
                }
            }
        }
    }

    /// A warning that may come with programs frozen: then it carries their numbers and the case's id
    /// (`c-<when>-<pid>`), which the panel uses to stop them or let them carry on.
    fn anota_congelado(
        &self,
        a: &Aviso,
        grave: bool,
        pid: u32,
        cuando_ms: u64,
        congelados: &[u32],
        archivo: &str,
    ) {
        let mut extra = serde_json::Map::new();
        if !congelados.is_empty() {
            extra.insert("congelados".to_owned(), serde_json::json!(congelados));
            extra.insert(
                "incidente".to_owned(),
                serde_json::Value::String(format!("c-{cuando_ms}-{pid}")),
            );
        }
        if !archivo.is_empty() {
            extra.insert(
                "archivo".to_owned(),
                serde_json::Value::String(archivo.chars().take(400).collect()),
            );
        }
        self.anota_con(a, grave, extra);
    }

    /// The connections without a name the traffic counter found, once Windows has checked each program's signature,
    /// through the rule (`guardiana_escudo::sin_nombre`). A program is judged once a day, warned or not.
    pub(crate) fn sin_nombre(
        &mut self,
        cola: &Mutex<VecDeque<SinNombre>>,
        obs: &Observador,
        ahora_ms: u64,
    ) {
        let nuevos: Vec<SinNombre> = cola
            .lock()
            .map(|mut c| c.drain(..).collect())
            .unwrap_or_default();
        if nuevos.is_empty() && self.sin_nombre_pend.is_empty() {
            return;
        }
        let mut todos = std::mem::take(&mut self.sin_nombre_pend);
        todos.extend(nuevos);
        for s in todos {
            let clave = s.ruta.to_lowercase();
            if self
                .sin_nombre_vistos
                .get(&clave)
                .is_some_and(|t| s.ts.saturating_sub(*t) < REPETIR_SIN_NOMBRE_MS)
            {
                continue;
            }
            let firma = match obs.firma(&s.ruta) {
                Some(f) if f.is_empty() => Firma::SinFirma,
                Some(f) => Firma::Firmado(f),
                None if ahora_ms.saturating_sub(s.ts) < ESPERA_FIRMA_MS => {
                    if self.sin_nombre_pend.len() < MAX_PENDIENTES
                        && !self.sin_nombre_pend.iter().any(|x| x.ruta == s.ruta)
                    {
                        self.sin_nombre_pend.push(s);
                    }
                    continue;
                }
                None => Firma::SinComprobar,
            };
            if self.sin_nombre_vistos.len() > 4_096 {
                self.sin_nombre_vistos.clear();
            }
            self.sin_nombre_vistos.insert(clave, s.ts);
            let descargado = self.descargado(&s.ruta);
            if let Some((aviso, grave)) = guardiana_escudo::sin_nombre::juzga(
                &s.programa,
                &s.ruta,
                &firma,
                descargado,
                &s.donde,
                s.ts,
            ) {
                self.anota(&aviso, grave);
            }
        }
    }

    /// The hosts file, once a minute (`guardiana_escudo::hosts`): a name now sent to an address on the internet is a
    /// grave warning. The first reading, and each change after it, is kept next to the ledger, so a change made while
    /// the service was stopped is seen too.
    fn mira_hosts(&mut self, ahora_ms: u64) {
        if ahora_ms.saturating_sub(self.hosts_leido_ms) < HOSTS_CADA_MS {
            return;
        }
        self.hosts_leido_ms = ahora_ms;
        let Ok(texto) = std::fs::read_to_string(ruta_hosts()) else {
            return;
        };
        let ahora = guardiana_escudo::hosts::entradas(&texto);
        if self.hosts.as_ref() == Some(&ahora) {
            return;
        }
        if let Some(antes) = &self.hosts {
            if let Some(aviso) = guardiana_escudo::hosts::cambios(antes, &ahora, ahora_ms) {
                self.anota(&aviso, true);
            }
        }
        if let Ok(j) = serde_json::to_string(&ahora) {
            let _ = escribe(&self.ruta_hosts, j.as_bytes());
        }
        self.hosts = Some(ahora);
    }

    /// Whether it is time to read the continuous watch's history again.
    pub(crate) fn toca_vigilancia(&self, ahora_ms: u64) -> bool {
        ahora_ms.saturating_sub(self.arranque_leido_ms) >= ARRANQUE_CADA_MS
    }

    /// What the continuous watch noted since the last read: its new autostarts (not Windows' own), each program file in
    /// what they run waiting for its signature check (`guardiana_escudo::arranque`), and a new WMI subscription at once;
    /// and the changes of Windows Security (`guardiana_escudo::proteccion`).
    pub(crate) fn lee_vigilancia(&mut self, l: &Ledger, obs: &Observador, ahora_ms: u64) {
        self.arranque_leido_ms = ahora_ms;
        let Ok(filas) = l.vigilancia(500, Some(self.arranque_ts)) else {
            return;
        };
        // the newest come first: oldest first here, so the row read last is the newest
        for s in filas.iter().rev() {
            if s.id <= self.arranque_id {
                continue;
            }
            self.arranque_id = s.id;
            self.arranque_ts = s.ts;
            if s.tipo == "proteccion" {
                self.proteccion(s);
                continue;
            }
            if s.tipo != "arranque" || s.datos["frase"] != "vg_ev_arranque_nuevo" {
                continue;
            }
            let d = &s.datos["datos"];
            if d["sistema"] == "true" {
                continue;
            }
            let texto = |k: &str| d[k].as_str().unwrap_or_default().to_owned();
            // a WMI subscription, Winlogon, a program's «debugger» and AppInit_DLLs are judged as they are: what they
            // run is often Windows' own program
            let tipo = texto("tipo");
            if matches!(tipo.as_str(), "wmi" | "winlogon" | "ifeo" | "appinit") {
                let cuando = u64::try_from(s.ts).unwrap_or(0);
                let juicio = if tipo == "wmi" {
                    Some(guardiana_escudo::arranque::juzga_wmi(
                        &texto("x"),
                        &texto("ruta"),
                        &s.clave,
                        cuando,
                    ))
                } else {
                    guardiana_escudo::arranque::juzga_registro(
                        &tipo,
                        &texto("x"),
                        &texto("ruta"),
                        &s.clave,
                        cuando,
                    )
                };
                if let Some((aviso, grave)) = juicio {
                    if !self.ya_avisado(aviso.regla, &aviso.nombre) {
                        self.anota(&aviso, grave);
                    }
                }
                continue;
            }
            for archivo in guardiana_escudo::arranque::archivos_en(&texto("ruta")) {
                if archivo.exacta {
                    // asks for the check now; the answer comes in a later tick
                    let _ = obs.firma(&archivo.ruta);
                }
                if self.arranques.len() < MAX_PENDIENTES {
                    self.arranques.push(PendienteArranque {
                        titulo: texto("x"),
                        tipo: texto("tipo"),
                        clave: s.clave.clone(),
                        archivo,
                        cuando_ms: u64::try_from(s.ts).unwrap_or(0),
                        visto_ms: ahora_ms,
                    });
                }
            }
        }
    }

    /// The new autostarts waiting for a signature check, through the rule once Windows has answered (or after
    /// [`ESPERA_FIRMA_MS`]). A file that is not there, or whose exact path is not known, goes without a signature
    /// check. The same autostart warns once, also across restarts (the panel's file is the record).
    fn arranques_pendientes(&mut self, obs: &Observador, ahora_ms: u64) {
        if self.arranques.is_empty() {
            return;
        }
        for p in std::mem::take(&mut self.arranques) {
            let existe = p.archivo.exacta && Path::new(&p.archivo.ruta).is_file();
            let firma = if existe {
                match obs.firma(&p.archivo.ruta) {
                    Some(f) if f.is_empty() => Firma::SinFirma,
                    Some(f) => Firma::Firmado(f),
                    None if ahora_ms.saturating_sub(p.visto_ms) < ESPERA_FIRMA_MS => {
                        self.arranques.push(p);
                        continue;
                    }
                    None => Firma::SinComprobar,
                }
            } else {
                Firma::SinComprobar
            };
            let descargado = existe && self.descargado(&p.archivo.ruta);
            let Some((aviso, grave)) = guardiana_escudo::arranque::juzga(
                &p.titulo,
                &p.tipo,
                &p.clave,
                &p.archivo,
                &firma,
                descargado,
                p.cuando_ms,
            ) else {
                continue;
            };
            if self.ya_avisado(aviso.regla, &aviso.nombre) {
                continue;
            }
            self.anota(&aviso, grave);
        }
    }

    /// A change of Windows Security the continuous watch noted, through the rule (`guardiana_escudo::proteccion`); the
    /// same change warns once, also across restarts.
    fn proteccion(&self, s: &Suceso) {
        let d = &s.datos["datos"];
        let activo = match d["activo"].as_str() {
            Some("true") => Some(true),
            Some("false") => Some(false),
            _ => None,
        };
        let Some((aviso, grave)) = guardiana_escudo::proteccion::juzga(
            s.datos["frase"].as_str().unwrap_or_default(),
            d["x"].as_str().unwrap_or_default(),
            activo,
            &s.clave,
            u64::try_from(s.ts).unwrap_or(0),
        ) else {
            return;
        };
        if self.ya_avisado(aviso.regla, &aviso.nombre) {
            return;
        }
        self.anota(&aviso, grave);
    }

    /// Whether the panel's file already holds a warning of this rule about this thing.
    fn ya_avisado(&self, regla: &str, nombre: &str) -> bool {
        std::fs::read_to_string(&self.ruta_avisos).is_ok_and(|t| {
            t.lines()
                .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                .any(|v| v["regla"] == regla && v["nombre"] == nombre)
        })
    }

    /// Writes the memory down when it changed and enough time has passed.
    pub(crate) fn guarda_si_toca(&mut self, ahora_ms: u64) {
        if !self.cambios || ahora_ms.saturating_sub(self.guardada_ms) < GUARDAR_CADA_MS {
            return;
        }
        if let Ok(j) = self.memoria.a_json() {
            if escribe(&self.ruta_memoria, j.as_bytes()).is_ok() {
                self.cambios = false;
                self.guardada_ms = ahora_ms;
            }
        }
    }

    /// Microsoft Defender's answers: a file it names as a known threat is a grave warning of its own.
    fn respuestas_de_defender(&mut self, ahora_ms: u64) {
        let Some(d) = &self.defensor else {
            return;
        };
        let respuestas: Vec<_> = d.respuestas.try_iter().collect();
        for (programa, ruta, amenazas) in respuestas {
            if let Some((aviso, grave)) =
                guardiana_escudo::defensor::juzga(&programa, &ruta, &amenazas, ahora_ms)
            {
                self.anota(&aviso, grave);
            }
        }
    }

    /// Asks Microsoft Defender about the file of a grave warning: a program file outside Windows' folder that is
    /// there, once a day.
    fn pide_segunda_opinion(&self, a: &Aviso) {
        let Some(d) = &self.defensor else {
            return;
        };
        let ruta = a.ruta.trim();
        let bajo = ruta.to_lowercase();
        if a.regla == "defensor"
            || ruta.is_empty()
            || bajo.contains(":\\windows\\")
            || !Path::new(ruta).is_file()
        {
            return;
        }
        let ahora = u64::try_from(guardiana_core::time::now_ms()).unwrap_or(0);
        let Ok(mut vistos) = self.defensor_vistos.try_borrow_mut() else {
            return;
        };
        if vistos
            .get(&bajo)
            .is_some_and(|t| ahora.saturating_sub(*t) < REPETIR_DEFENSOR_MS)
        {
            return;
        }
        if vistos.len() > 4_096 {
            vistos.clear();
        }
        vistos.insert(bajo, ahora);
        let _ = d.pide.send((a.programa.clone(), ruta.to_owned()));
    }

    /// Adds a warning to the panel's file, with when it was written (`anotado_ms`), keeping the newest [`MAX_AVISOS`];
    /// a grave one carries `"grave": true` (a
    /// known sample, a document starting an interpreter, the backups' tools, a program in disguise).
    fn anota(&self, a: &Aviso, grave: bool) {
        self.anota_con(a, grave, serde_json::Map::new());
    }

    fn anota_con(&self, a: &Aviso, grave: bool, extra: serde_json::Map<String, serde_json::Value>) {
        let Ok(mut valor) = serde_json::to_value(a) else {
            return;
        };
        if let Some(o) = valor.as_object_mut() {
            o.extend(extra);
            if grave {
                o.insert("grave".to_owned(), serde_json::Value::Bool(true));
            }
            // when it was written: what Windows' notices go by (a warning can be written a minute after the fact,
            // once the program's signature is checked)
            o.insert(
                "anotado_ms".to_owned(),
                serde_json::Value::from(guardiana_core::time::now_ms()),
            );
        }
        let Ok(linea) = serde_json::to_string(&valor) else {
            return;
        };
        let mut lineas: Vec<String> = std::fs::read_to_string(&self.ruta_avisos)
            .map(|t| {
                t.lines()
                    .filter(|l| !l.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        lineas.push(linea);
        let sobra = lineas.len().saturating_sub(MAX_AVISOS);
        let mut texto = lineas.split_off(sobra).join("\n");
        texto.push('\n');
        if let Err(e) = escribe(&self.ruta_avisos, texto.as_bytes()) {
            eprintln!("guardiana: escudo: {e}");
        }
        if grave {
            self.pide_segunda_opinion(a);
        }
    }
}

/// Windows' hosts file (`/etc/hosts` elsewhere).
fn ruta_hosts() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var_os("SystemRoot")
            .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from)
            .join(r"System32\drivers\etc\hosts")
    }
    #[cfg(not(windows))]
    {
        PathBuf::from("/etc/hosts")
    }
}

#[cfg(windows)]
fn marca_de_internet(ruta: &str) -> bool {
    !ruta.is_empty()
        && std::fs::read_to_string(format!("{ruta}:Zone.Identifier"))
            .ok()
            .and_then(|t| guardiana_escudo::zona_de(&t))
            .is_some_and(|z| z >= 3)
}

#[cfg(not(windows))]
fn marca_de_internet(_ruta: &str) -> bool {
    false
}

/// What is known of a program's signature: Windows' check if it was done, else what the observer already said.
fn firma_de(obs: &Observador, ruta: &str, firmado_por: &str) -> Firma {
    match obs.firma_comprobada(ruta) {
        Some(f) if !f.is_empty() => Firma::Firmado(f),
        Some(_) => Firma::SinFirma,
        None if !firmado_por.is_empty() => Firma::Firmado(firmado_por.to_owned()),
        None => Firma::SinComprobar,
    }
}

/// Writes a whole file through a temporary name, so that it is either all there or not at all.
fn escribe(ruta: &Path, datos: &[u8]) -> std::io::Result<()> {
    let tmp = ruta.with_extension("tmp");
    std::fs::write(&tmp, datos)?;
    std::fs::rename(&tmp, ruta)
}

/// Plants the decoys against ransomware where they are missing or changed (Windows only): in each account's profile and
/// its Documents, Desktop and Pictures (only folders of the disk itself: one kept by OneDrive or moved elsewhere is
/// left alone), and in the public Documents. Each one hidden and marked as a system file, so the explorer does not show
/// it. The paths and their time of writing.
fn planta_senuelos() -> Vec<(PathBuf, std::time::SystemTime)> {
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut out = Vec::new();
    #[cfg(windows)]
    {
        use guardiana_escudo::senuelos;
        let raiz = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_owned());
        let usuarios = PathBuf::from(format!("{raiz}\\Users"));
        let mut rutas: Vec<PathBuf> = Vec::new();
        if let Ok(perfiles) = std::fs::read_dir(&usuarios) {
            for p in perfiles.flatten() {
                let nombre = p.file_name().to_string_lossy().into_owned();
                if p.path().is_dir() && senuelos::es_perfil_de_persona(&nombre) {
                    rutas.extend(senuelos::de_perfil(&p.path()));
                }
            }
        }
        for n in senuelos::NOMBRES {
            rutas.push(usuarios.join("Public").join("Documents").join(n));
        }
        for ruta in rutas {
            let Some(carpeta) = ruta.parent() else {
                continue;
            };
            if !carpeta_propia(carpeta) {
                continue;
            }
            let bien = std::fs::metadata(&ruta)
                .ok()
                .filter(|m| m.len() == senuelos::TAMANO as u64)
                .and_then(|m| m.modified().ok());
            match bien.or_else(|| escribe_senuelo(&ruta)) {
                Some(cuando) => out.push((ruta, cuando)),
                None => continue,
            }
        }
    }
    out
}

/// Whether a folder is a real folder of the disk, not a link to elsewhere, nor kept by OneDrive.
#[cfg(windows)]
fn carpeta_propia(carpeta: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    /// `FILE_ATTRIBUTE_REPARSE_POINT`: a link, a junction or a folder kept by a cloud service.
    const ENLACE: u32 = 0x400;
    let Ok(m) = std::fs::symlink_metadata(carpeta) else {
        return false;
    };
    m.is_dir()
        && m.file_attributes() & ENLACE == 0
        && !carpeta
            .to_string_lossy()
            .to_ascii_lowercase()
            .contains("onedrive")
}

/// Writes one decoy (hidden, a system file) and gives its time of writing.
fn escribe_senuelo(ruta: &Path) -> Option<std::time::SystemTime> {
    #[cfg(windows)]
    {
        use std::io::Write;
        use std::os::windows::fs::OpenOptionsExt;
        /// `FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM`.
        const OCULTO: u32 = 0x2 | 0x4;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .attributes(OCULTO)
            .open(ruta)
            .ok()?;
        f.write_all(&guardiana_escudo::senuelos::contenido()).ok()?;
        f.sync_all().ok()?;
        drop(f);
        std::fs::metadata(ruta).ok()?.modified().ok()
    }
    #[cfg(not(windows))]
    {
        let _ = ruta;
        None
    }
}
