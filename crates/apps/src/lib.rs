//! Which program asked for each name (brief §12.3), on Windows and read-only.
//!
//! GUARDIANA has always been able to say *which device* asked for a name. On this computer it can
//! also say *which program*, because Windows itself publishes that on its DNS client's event
//! channel (ETW): one event per query, with the name and the id of the process that asked. This
//! crate listens to that channel, keeps the last few seconds in memory, and answers one question:
//! «this name, at this moment — who asked for it?».
//!
//! What it does **not** do, and will not: it does not read what a program sends or receives, does
//! not look at its files, does not judge it, and does not cut anything. Per-application blocking
//! is 2.0 and stays out (brief §1). The phones and the TV in Home Mode keep being seen by name
//! only, and every screen says so.
//!
//! On Linux and macOS there is no equivalent channel, so `Observador::arrancar` says plainly that
//! it is not available and everything else keeps working exactly as before.

use std::fmt;

pub use guardiana_core::Proceso;

/// Why the watcher could not start.
#[derive(Debug)]
pub enum Error {
    /// This system has no channel that says which program asked (everything but Windows).
    NoSoportado,
    /// Windows refused the trace session; the text is what it said.
    Sistema(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSoportado => write!(
                f,
                "este sistema no dice qué programa pidió cada nombre: solo Windows lo publica"
            ),
            Self::Sistema(e) => write!(f, "Windows no dejó abrir la sesión de sucesos: {e}"),
        }
    }
}

impl std::error::Error for Error {}

/// How long a seen query stays available to be matched, in milliseconds.
///
/// Guardiana's resolver and Windows' event channel see the same query a few milliseconds apart,
/// but under load the gap grows. Two seconds is long enough for that and short enough that the
/// list stays tiny and nothing lingers in memory.
pub const VENTANA_MS: i64 = 2_000;

/// The most queries kept in memory at once. A burst of 241 names in 20 seconds is a real
/// measurement from this project, so the bound is generous and still bounded.
pub const MAXIMO: usize = 2_048;

pub mod cortafuegos;
pub use cortafuegos::{Cortafuegos, Destino, ErrorCorte};
#[cfg(windows)]
mod actividad;
#[cfg(windows)]
mod archivos;
#[cfg(windows)]
mod defender;
#[cfg(windows)]
mod firmas;
// El escudo (1.0.52): who opens the saved secrets (browser passwords, cookies, keys, wallets, sessions).
#[cfg(windows)]
mod guardia;
pub mod instalador;
// «Quién vende tus datos dentro de tu PC» (1.0.46): the system's own advertising settings.
pub mod privacidad;
#[cfg(windows)]
mod red;
#[cfg(windows)]
mod revision;
#[cfg(windows)]
mod seguridad;
#[cfg(windows)]
mod ventanas;
// «Vigía» (1.0.48): señales de espionaje durante una llamada o un chat, cada pocos segundos.
pub mod vigia;
pub mod vigilancia;
mod vpn;

// The VPN on Windows (in preparation): WireGuardNT through its signed wireguard.dll, and the cut-off switch.
#[cfg(windows)]
pub mod corte_vpn;
#[cfg(windows)]
pub mod red_vpn;
#[cfg(windows)]
pub mod tunel_win;
#[cfg(windows)]
pub mod wireguard_nt;

/// Listens to what the system says about who asked for each name.
pub struct Observador {
    #[cfg(windows)]
    interno: ventanas::Sesion,
}

impl Observador {
    /// Starts listening. On anything but Windows this returns [`Error::NoSoportado`], and the
    /// caller simply records events without a program, as it always did.
    ///
    /// # Errors
    /// [`Error::NoSoportado`] outside Windows; [`Error::Sistema`] when the trace cannot start
    /// (which on Windows means: not running with enough privileges).
    pub fn arrancar() -> Result<Self, Error> {
        #[cfg(windows)]
        {
            Ok(Self {
                interno: ventanas::Sesion::arrancar()?,
            })
        }
        #[cfg(not(windows))]
        {
            Err(Error::NoSoportado)
        }
    }

    /// Who asked for `qname` around `ts_ms`, if the system said so within the window.
    ///
    /// The match is consumed: two queries for the same name are two entries, and neither is
    /// reused for the other.
    #[must_use]
    pub fn quien_pidio(&self, qname: &str, ts_ms: i64) -> Option<Proceso> {
        #[cfg(windows)]
        {
            self.interno.quien_pidio(qname, ts_ms)
        }
        #[cfg(not(windows))]
        {
            let _ = (qname, ts_ms);
            None
        }
    }

    /// Who signs the program at `ruta`, once the system has checked it: `Some("")` when it has no
    /// valid signature, `None` while it has not been checked yet (and always outside Windows).
    #[must_use]
    pub fn firma_comprobada(&self, ruta: &str) -> Option<String> {
        #[cfg(windows)]
        {
            self.interno.firma_comprobada(ruta)
        }
        #[cfg(not(windows))]
        {
            let _ = ruta;
            None
        }
    }

    /// The same, and if the program was never checked, asks for the check now (the answer comes in a later call).
    #[must_use]
    pub fn firma(&self, ruta: &str) -> Option<String> {
        #[cfg(windows)]
        {
            self.interno.firma(ruta)
        }
        #[cfg(not(windows))]
        {
            let _ = ruta;
            None
        }
    }
}

/// What one program sent to and received from one address since the last take (1.0.21).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Medida {
    /// The program, with its path, its fingerprint and who signs it when known. `pid` is 0:
    /// the totals are per program, not per process.
    pub proceso: Proceso,
    /// The address at the other end. Kept in memory only: the service writes its country and
    /// network, never the address (decision 203).
    pub ip: std::net::IpAddr,
    /// The port at the other end.
    pub puerto: u16,
    /// `tcp` or `udp`.
    pub proto: &'static str,
    /// Bytes sent there.
    pub enviados: u64,
    /// Bytes received from there.
    pub recibidos: u64,
    /// Packets either way.
    pub paquetes: u64,
    /// Bytes sent again there (TCP retransmissions), apart from `enviados` (1.0.45).
    pub reenviados: u64,
    /// Connections opened there or accepted from there (1.0.45).
    pub conexiones: u64,
}

/// Listens to what the system says about every packet each program sends or receives, and adds
/// it up per program and address (1.0.21, «vigila a tu antivirus»). Counts, never contents.
pub struct Red {
    #[cfg(windows)]
    interno: red::Sesion,
}

impl Red {
    /// Starts listening.
    ///
    /// # Errors
    /// [`Error::NoSoportado`] outside Windows; [`Error::Sistema`] when the trace cannot start.
    pub fn arrancar() -> Result<Self, Error> {
        #[cfg(windows)]
        {
            Ok(Self {
                interno: red::Sesion::arrancar()?,
            })
        }
        #[cfg(not(windows))]
        {
            Err(Error::NoSoportado)
        }
    }

    /// The totals since the last call.
    #[must_use]
    pub fn toma(&self) -> Vec<Medida> {
        #[cfg(windows)]
        {
            self.interno.toma()
        }
        #[cfg(not(windows))]
        {
            Vec::new()
        }
    }
}

/// The network channel, raw, for a few seconds: event number, process id, size and the two
/// ends as read. `guardiana apps --red`, to check on the machine.
///
/// # Errors
/// [`Error::NoSoportado`] outside Windows; [`Error::Sistema`] if the session cannot open.
pub fn diagnostico_red(segundos: u64) -> Result<Vec<(u16, u32, u32, String)>, Error> {
    #[cfg(windows)]
    {
        red::diagnostico(segundos)
    }
    #[cfg(not(windows))]
    {
        let _ = segundos;
        Err(Error::NoSoportado)
    }
}

/// One opening of a file that holds a saved secret (a browser's passwords, cookies or key, a wallet, a messaging
/// session) by a program that is not that app's own installed program (el escudo, 1.0.52).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Apertura {
    /// When (milliseconds since 1970).
    pub ts: i64,
    /// The program's file name.
    pub programa: String,
    /// The program's full path.
    pub ruta: String,
    /// The file it opened, as Windows names it.
    pub archivo: String,
}

/// A program start the escudo watches (a command interpreter, a script host, one of Windows' own tools attackers use, or
/// the backups' tools), with the program that started it (el escudo, 1.0.52).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lanzamiento {
    /// When (milliseconds since 1970).
    pub ts: i64,
    /// The parent's file name.
    pub padre: String,
    /// The parent's full path.
    pub padre_ruta: String,
    /// The started program's file name.
    pub hijo: String,
    /// The started program's path (with its drive letter when the volume is mounted on one).
    pub imagen: String,
}

/// One program that renamed or deleted many of the person's own files in under a minute (el escudo, 1.0.52;
/// `guardiana_escudo::secuestro`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rafaga {
    /// When it reached the threshold (milliseconds since 1970).
    pub ts: i64,
    /// The program's file name.
    pub programa: String,
    /// Its full path.
    pub ruta: String,
    /// How many files, at least.
    pub n: usize,
}

/// A program that opened LSASS, the process that keeps the signed-in accounts' credentials, asking to read its memory
/// (el escudo, 1.0.52; `guardiana_escudo::credenciales`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Acceso {
    /// When (milliseconds since 1970).
    pub ts: i64,
    /// The program's file name.
    pub programa: String,
    /// Its full path.
    pub ruta: String,
    /// The rights it asked for (Windows' `DesiredAccess`).
    pub acceso: u32,
    /// Whether Windows allowed it.
    pub concedido: bool,
}

/// A program started from a disc or a mounted disk image, or from a removable drive (el escudo, 1.0.52;
/// `guardiana_escudo::medios`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesdeMedio {
    /// When (milliseconds since 1970).
    pub ts: i64,
    /// The program's file name.
    pub programa: String,
    /// Its full path (with its drive letter when the volume has one).
    pub ruta: String,
    /// `imagen` (a disc or a mounted ISO or IMG) or `usb` (a removable drive).
    pub medio: &'static str,
}

/// El escudo's watch over saved secrets, over program starts and over the person's files: on while the service runs,
/// names only.
pub struct Guardia {
    #[cfg(windows)]
    interno: guardia::Sesion,
}

impl Guardia {
    /// Starts watching.
    ///
    /// # Errors
    /// [`Error::NoSoportado`] outside Windows; [`Error::Sistema`] when the trace cannot start.
    pub fn arrancar() -> Result<Self, Error> {
        #[cfg(windows)]
        {
            Ok(Self {
                interno: guardia::Sesion::arrancar()?,
            })
        }
        #[cfg(not(windows))]
        {
            Err(Error::NoSoportado)
        }
    }

    /// The openings since the last call.
    #[must_use]
    pub fn toma(&self) -> Vec<Apertura> {
        #[cfg(windows)]
        {
            self.interno.toma()
        }
        #[cfg(not(windows))]
        {
            Vec::new()
        }
    }

    /// The bursts of the person's files renamed or deleted since the last call.
    #[must_use]
    pub fn rafagas(&self) -> Vec<Rafaga> {
        #[cfg(windows)]
        {
            self.interno.rafagas()
        }
        #[cfg(not(windows))]
        {
            Vec::new()
        }
    }

    /// The watched program starts since the last call.
    #[must_use]
    pub fn lanzamientos(&self) -> Vec<Lanzamiento> {
        #[cfg(windows)]
        {
            self.interno.lanzamientos()
        }
        #[cfg(not(windows))]
        {
            Vec::new()
        }
    }

    /// The programs started from a disc, a mounted disk image or a removable drive since the last call.
    #[must_use]
    pub fn desde_medios(&self) -> Vec<DesdeMedio> {
        #[cfg(windows)]
        {
            self.interno.desde_medios()
        }
        #[cfg(not(windows))]
        {
            Vec::new()
        }
    }

    /// The openings of LSASS that asked to read its memory since the last call.
    #[must_use]
    pub fn accesos(&self) -> Vec<Acceso> {
        #[cfg(windows)]
        {
            self.interno.accesos()
        }
        #[cfg(not(windows))]
        {
            Vec::new()
        }
    }
}

/// Listens to which files in the person's own folders each program opens and reads (1.0.45,
/// «Visión profunda»). Names only, never contents; only while the person has it on.
pub struct Archivos {
    #[cfg(windows)]
    interno: archivos::Sesion,
}

impl Archivos {
    /// Starts listening.
    ///
    /// # Errors
    /// [`Error::NoSoportado`] outside Windows; [`Error::Sistema`] when the trace cannot start.
    pub fn arrancar() -> Result<Self, Error> {
        #[cfg(windows)]
        {
            Ok(Self {
                interno: archivos::Sesion::arrancar()?,
            })
        }
        #[cfg(not(windows))]
        {
            Err(Error::NoSoportado)
        }
    }

    /// The openings and reads since the last call.
    #[must_use]
    pub fn toma(&self) -> Vec<guardiana_core::vision::Lectura> {
        #[cfg(windows)]
        {
            self.interno.toma()
        }
        #[cfg(not(windows))]
        {
            Vec::new()
        }
    }
}

/// The person's folders that count as «their files», as Windows names them on disk, and in the
/// languages OneDrive may name them when it moved them (it does use the localised name).
const CARPETAS: &[(&str, &[&str])] = &[
    ("documentos", &["documents", "documentos"]),
    (
        "escritorio",
        &[
            "desktop",
            "escritorio",
            "área de trabalho",
            "area de trabalho",
        ],
    ),
    ("descargas", &["downloads", "descargas", "transferências"]),
    ("imagenes", &["pictures", "imágenes", "imagenes", "imagens"]),
];

/// Whether a path is a file inside one of the person's folders, and which: the folder's key
/// (`documentos`, `escritorio`, `descargas`, `imagenes`) and the file's own name, without its
/// folders. Works on the kernel's form (`\Device\HarddiskVolume3\Users\ana\Documents\x.pdf`) and
/// on the usual one (`C:\Users\…`). Everything else — programs, `AppData`, the system — is `None`,
/// and so are the files Windows itself keeps there (`desktop.ini`, thumbnails, shortcuts, Office's
/// `~$` locks, temporary files). A file's alternate stream (`x.pdf:Zone.Identifier`) is the file.
#[must_use]
pub fn carpeta_de_usuario(ruta: &str) -> Option<(&'static str, String)> {
    let partes: Vec<&str> = ruta.split(['\\', '/']).filter(|p| !p.is_empty()).collect();
    let i = partes
        .iter()
        .position(|p| p.eq_ignore_ascii_case("users"))?;
    // users \ <person> \ [OneDrive…] \ <folder> \ … \ <file>
    let mut j = i + 2;
    let siguiente = partes.get(j)?.to_lowercase();
    if siguiente == "onedrive" || siguiente.starts_with("onedrive - ") {
        j += 1;
    }
    let carpeta = partes.get(j)?.to_lowercase();
    let clave = CARPETAS
        .iter()
        .find(|(_, nombres)| nombres.contains(&carpeta.as_str()))
        .map(|(k, _)| *k)?;
    if partes.len() <= j + 1 {
        return None;
    }
    let ultimo = partes.last()?;
    let archivo = ultimo.split(':').next().unwrap_or(ultimo).trim();
    let bajo = archivo.to_lowercase();
    if archivo.is_empty()
        || bajo == "desktop.ini"
        || bajo == "thumbs.db"
        || bajo.starts_with("~$")
        || bajo.ends_with(".lnk")
        || bajo.ends_with(".tmp")
    {
        return None;
    }
    Some((clave, archivo.to_owned()))
}

/// What Microsoft Defender is set to send to its cloud, as Windows says (1.0.45). Each value is
/// `None` when it could not be read (not Windows, Defender not there, or the value never set).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
pub struct DefenderNube {
    /// `SubmitSamplesConsent`: 0 always ask, 1 send safe samples automatically, 2 never send,
    /// 3 send all samples automatically (Microsoft's `Set-MpPreference` documentation).
    pub muestras: Option<u32>,
    /// The value above comes from a policy (an organisation set it), not the person.
    pub muestras_directiva: bool,
    /// `SpynetReporting` (cloud protection, «MAPS»): 0 off, 1 basic, 2 advanced.
    pub maps: Option<u32>,
    /// The value above comes from a policy.
    pub maps_directiva: bool,
}

/// Reads Defender's cloud settings. Off Windows, nothing is known.
#[must_use]
pub fn defender_nube() -> DefenderNube {
    #[cfg(windows)]
    {
        defender::leer()
    }
    #[cfg(not(windows))]
    {
        DefenderNube::default()
    }
}

/// The number `reg.exe query … /v <nombre>` printed for a `REG_DWORD`, or `None`.
///
/// The line reads `    SubmitSamplesConsent    REG_DWORD    0x1`.
#[must_use]
pub fn lee_dword_de_reg(salida: &str, nombre: &str) -> Option<u32> {
    salida.lines().find_map(|l| {
        let mut p = l.split_whitespace();
        let (n, tipo, valor) = (p.next()?, p.next()?, p.next()?);
        if !n.eq_ignore_ascii_case(nombre) || tipo != "REG_DWORD" {
            return None;
        }
        let v = valor.trim_start_matches("0x").trim_start_matches("0X");
        u32::from_str_radix(v, 16).ok()
    })
}

/// An antivirus as Windows Security Center lists it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AntivirusInstalado {
    /// Its name as the product registered it.
    pub nombre: String,
    /// The program Windows says it is (`pathToSignedProductExe`), may be empty.
    pub ruta: String,
    /// The raw `productState` the product reported.
    pub estado: u32,
    /// Whether real-time protection is on, as far as `productState` says.
    pub activo: bool,
    /// Whether its signatures are up to date, as far as `productState` says.
    pub al_dia: bool,
}

/// What Windows Security Center says is installed (1.0.21). Read-only, asked on demand.
///
/// # Errors
/// [`Error::NoSoportado`] outside Windows; [`Error::Sistema`] when Windows did not answer.
pub use vpn::Vpn;

/// The VPNs this computer's traffic is leaving through right now, as Windows lists them
/// (15 Oct 2026). Elsewhere, nothing is known.
///
/// # Errors
/// [`Error::NoSoportado`] off Windows; [`Error::Sistema`] if Windows did not answer.
pub fn vpn_activas() -> Result<Vec<Vpn>, Error> {
    #[cfg(windows)]
    {
        vpn::activas()
    }
    #[cfg(not(windows))]
    {
        Err(Error::NoSoportado)
    }
}

pub fn antivirus_instalados() -> Result<Vec<AntivirusInstalado>, Error> {
    #[cfg(windows)]
    {
        seguridad::instalados()
    }
    #[cfg(not(windows))]
    {
        Err(Error::NoSoportado)
    }
}

/// Reads a `productState`: (real-time protection on, signatures up to date).
///
/// The value is not documented by Microsoft; this is the reading every tool uses, and the page
/// says it is Windows' word, not ours: the second byte says on (`0x10`) or snoozed/off, and the
/// third, whether the signatures are up to date (`0x00`) or not (`0x10`).
#[must_use]
pub const fn lee_estado(estado: u32) -> (bool, bool) {
    let proteccion = (estado >> 8) & 0xFF;
    let firmas = estado & 0xFF;
    (proteccion & 0x10 != 0, firmas == 0)
}

/// Reads what the signature-and-version check printed: a line `F:<signer>` when the signature is
/// valid and a line `V:<version>` when the file declares one (1.0.45). Anything else is ignored,
/// the signer is cut at 200 characters and the version is kept only when it looks like one
/// (digits, letters, dots, dashes, plus signs, brackets and spaces, at most 40, with a digit), so
/// a strange file cannot write a sentence into the panel through its version field.
#[must_use]
pub fn lee_firma_y_version(salida: &str) -> (String, String) {
    let mut firma = String::new();
    let mut version = String::new();
    for linea in salida.lines().map(str::trim) {
        if let Some(f) = linea.strip_prefix("F:") {
            if firma.is_empty() {
                firma = f.trim().chars().take(200).collect();
            }
        } else if let Some(v) = linea.strip_prefix("V:") {
            let v = v.trim();
            let parece = !v.is_empty()
                && v.chars().count() <= 40
                && v.chars().any(|c| c.is_ascii_digit())
                && v.chars().all(|c| {
                    c.is_ascii_alphanumeric()
                        || matches!(c, '.' | '-' | '+' | ' ' | ',' | '(' | ')')
                });
            if version.is_empty() && parece {
                // «6, 2, 1, 0» is how some old files write it: the same number, said with dots.
                version = v.replace(", ", ".").replace(',', ".");
            }
        }
    }
    (firma, version)
}

#[cfg(test)]
mod firma_y_version {
    #[test]
    fn the_signer_and_the_version_are_read_from_their_own_lines() {
        let (f, v) =
            super::lee_firma_y_version("F:Zoom Video Communications, Inc.\r\nV:6.2.1 (45678)\r\n");
        assert_eq!(f, "Zoom Video Communications, Inc.");
        assert_eq!(v, "6.2.1 (45678)");
    }

    #[test]
    fn a_version_that_does_not_look_like_one_is_left_out() {
        let (f, v) = super::lee_firma_y_version("V:<b>hola</b>\n");
        assert!(f.is_empty());
        assert!(v.is_empty());
        let (_, v) = super::lee_firma_y_version("V:6, 2, 1, 0\n");
        assert_eq!(v, "6.2.1.0");
        let (f, v) = super::lee_firma_y_version("Microsoft Windows\n");
        assert!(
            f.is_empty() && v.is_empty(),
            "untagged lines are not trusted"
        );
    }
}

/// Lo que el canal de sucesos está contando, tal cual, durante unos segundos.
///
/// Es la comprobación que pide el brief (§12.3): antes de fiarse de un número de suceso hay que
/// verlo en la máquina. Devuelve, por cada suceso del proveedor: su número, el nombre pedido si
/// se pudo leer, y el proceso que lo pidió.
///
/// # Errors
/// [`Error::NoSoportado`] fuera de Windows; [`Error::Sistema`] si la sesión no se puede abrir.
pub fn diagnostico(segundos: u64) -> Result<Vec<(u16, String, u32)>, Error> {
    #[cfg(windows)]
    {
        ventanas::diagnostico(segundos)
    }
    #[cfg(not(windows))]
    {
        let _ = segundos;
        Err(Error::NoSoportado)
    }
}

#[cfg(test)]
#[cfg(not(windows))]
#[allow(clippy::expect_used)]
mod pruebas {
    use super::{Error, Observador};

    #[test]
    fn el_estado_de_un_antivirus_se_lee_como_lo_lee_todo_el_mundo() {
        // Defender on and up to date (397568 = 0x061100), off (393472 = 0x060100), outdated
        // (397584 = 0x061110).
        assert_eq!(super::lee_estado(397_568), (true, true));
        assert_eq!(super::lee_estado(393_472), (false, true));
        assert_eq!(super::lee_estado(397_584), (true, false));
    }

    #[test]
    fn fuera_de_windows_lo_dice_y_no_rompe_nada() {
        let e = Observador::arrancar()
            .err()
            .expect("debería no estar disponible");
        assert!(matches!(e, Error::NoSoportado));
        assert!(e.to_string().contains("solo Windows"));
    }
}

/// What one program did inside the computer, summed over its processes, as Windows counts it
/// (1.0.24). Counters since each process started: the panel turns two takes into rates.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Actividad {
    /// The program's file name as Windows lists it (`MsMpEng.exe`).
    pub nombre: String,
    /// How many of its processes are running now.
    pub procesos: u32,
    /// Processor time used, kernel and user, in milliseconds.
    pub cpu_ms: u64,
    /// Bytes it read (files and other devices: everything the process asked for).
    pub leido: u64,
    /// Bytes it wrote, likewise.
    pub escrito: u64,
    /// Memory it holds in RAM now (working set), in bytes.
    pub memoria: u64,
}

/// The counters of the programs with these file names that are running now. Windows only;
/// elsewhere, nothing (and the page says this system does not measure it).
#[must_use]
pub fn actividad(nombres: &[String]) -> Vec<Actividad> {
    #[cfg(windows)]
    {
        actividad::actividad(nombres)
    }
    #[cfg(not(windows))]
    {
        let _ = nombres;
        Vec::new()
    }
}

/// «Revisión del ordenador» (1.0.24): everything Windows says about this computer's protection,
/// access, startup, internet way out and privacy, as one JSON (see `revision.ps1`). Read-only.
/// Takes tens of seconds; call it off the async runtime. Windows only.
///
/// # Errors
/// [`Error::NoSoportado`] off Windows; [`Error::Sistema`] if PowerShell could not run, took
/// longer than `limite` or answered something that is not JSON.
pub fn revision_del_equipo(limite: std::time::Duration) -> Result<serde_json::Value, Error> {
    revision_del_equipo_en_vivo(limite, Box::new(|_, _| {}))
}

/// Who hears each live step of the review: `(kind, text)`.
pub type AvisoRevision = Box<dyn Fn(&str, &str) + Send>;

/// The same review, telling `avisa` each step as it happens: `seccion` (a section starts),
/// `hecho` (a section ended, with its milliseconds), `firma` (the file whose signature is being
/// read), `dns` (the name being asked) and `cuenta` (how many items a list had). 1.0.31: the
/// page shows them live, so nobody thinks the button does nothing.
///
/// # Errors
/// As [`revision_del_equipo`].
pub fn revision_del_equipo_en_vivo(
    limite: std::time::Duration,
    avisa: AvisoRevision,
) -> Result<serde_json::Value, Error> {
    #[cfg(windows)]
    {
        revision::revisar(limite, avisa)
    }
    #[cfg(not(windows))]
    {
        let _ = (limite, avisa);
        Err(Error::NoSoportado)
    }
}

/// The review's script, so its size and its read-only promise can be checked everywhere.
pub const REVISION_PS1: &str = include_str!("revision.ps1");

/// One hop of a route: its number, the address that answered (none when nothing did) and the
/// best of its times, in milliseconds.
pub type Salto = (u32, Option<std::net::IpAddr>, Option<u32>);

/// 1.0.51 (owner, 31 Oct 2026, 08:53: «por dónde viaja… si viaja por cable, qué cable, si viaja
/// por satélite… lo más preciso posible»): the route to one address, hop by hop, with Windows' own
/// `tracert` (no administrator rights, nothing installed, nothing written). At most `saltos` hops
/// and `espera_ms` per answer; the whole trace is stopped after `limite` and what came is kept.
///
/// # Errors
/// [`Error::NoSoportado`] outside Windows; [`Error::Sistema`] when `tracert` cannot be run.
pub fn traza(
    ip: std::net::IpAddr,
    saltos: u8,
    espera_ms: u32,
    limite: std::time::Duration,
) -> Result<Vec<Salto>, Error> {
    #[cfg(windows)]
    {
        use std::io::Read as _;
        use std::os::windows::process::CommandExt;
        use std::process::{Command, Stdio};
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let raiz = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_owned());
        let mut orden = Command::new(format!("{raiz}\\System32\\tracert.exe"));
        orden
            .arg("-d")
            .arg("-h")
            .arg(saltos.to_string())
            .arg("-w")
            .arg(espera_ms.to_string())
            .arg(if ip.is_ipv4() { "-4" } else { "-6" })
            .arg(ip.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW);
        let mut hijo = orden.spawn().map_err(|e| Error::Sistema(e.to_string()))?;
        let mut salida = hijo.stdout.take();
        let lector = std::thread::spawn(move || {
            let mut v = Vec::new();
            if let Some(s) = salida.as_mut() {
                let _ = s.read_to_end(&mut v);
            }
            v
        });
        let inicio = std::time::Instant::now();
        loop {
            match hijo.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if inicio.elapsed() > limite => {
                    let _ = hijo.kill();
                    let _ = hijo.wait();
                    break;
                }
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(150)),
                Err(e) => return Err(Error::Sistema(e.to_string())),
            }
        }
        let v = lector.join().unwrap_or_default();
        Ok(lee_tracert(&String::from_utf8_lossy(&v)))
    }
    #[cfg(not(windows))]
    {
        let _ = (ip, saltos, espera_ms, limite);
        Err(Error::NoSoportado)
    }
}

/// The hops `tracert -d` printed, whatever the language of Windows: a line is a hop when it
/// starts with its number; its times are the numbers before «ms» (`<1` counts as 1) and its
/// address is the last word when it is one.
#[must_use]
pub fn lee_tracert(texto: &str) -> Vec<Salto> {
    let mut out = Vec::new();
    for linea in texto.lines() {
        let p: Vec<&str> = linea.split_whitespace().collect();
        let Some(n) = p.first().and_then(|x| x.parse::<u32>().ok()) else {
            continue;
        };
        if p.len() < 2 {
            continue;
        }
        let mut mejor: Option<u32> = None;
        for (i, t) in p.iter().enumerate().skip(1) {
            let antes_de_ms = p.get(i + 1).is_some_and(|x| x.eq_ignore_ascii_case("ms"));
            let v = match t.strip_prefix('<') {
                Some(r) => r.parse::<u32>().ok(),
                None if antes_de_ms => t.parse::<u32>().ok(),
                None => None,
            };
            if let Some(v) = v {
                let v = v.max(1);
                mejor = Some(mejor.map_or(v, |m| m.min(v)));
            }
        }
        let ip = p.last().and_then(|x| {
            x.trim_matches(|c| c == '[' || c == ']')
                .parse::<std::net::IpAddr>()
                .ok()
        });
        out.push((n, ip, mejor));
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tracert {
    #[test]
    fn hops_are_read_in_any_language() {
        let es = "\r\nTraza a la dirección 142.250.78.4 sobre caminos de 20 saltos como máximo:\r\n\r\n  1    <1 ms    <1 ms    <1 ms  192.168.1.1\r\n  2     *        *        *     Tiempo de espera agotado para esta solicitud.\r\n  3    12 ms    11 ms    13 ms  200.75.51.132\r\n 10    41 ms    40 ms    44 ms  142.250.78.4\r\n\r\nTraza completa.\r\n";
        let h = super::lee_tracert(es);
        assert_eq!(h.len(), 4);
        assert_eq!(h[0], (1, Some("192.168.1.1".parse().unwrap()), Some(1)));
        assert_eq!(h[1], (2, None, None));
        assert_eq!(h[2], (3, Some("200.75.51.132".parse().unwrap()), Some(11)));
        assert_eq!(h[3].0, 10);
        let en = "  5    25 ms    24 ms    26 ms  [2001:4860:0:1::1]\n";
        assert_eq!(
            super::lee_tracert(en)[0].1,
            Some("2001:4860:0:1::1".parse().unwrap())
        );
    }
}

#[cfg(test)]
mod vision {
    use super::{carpeta_de_usuario, lee_dword_de_reg};

    #[test]
    fn only_files_in_the_persons_folders_count_and_only_by_name() {
        assert_eq!(
            carpeta_de_usuario(r"\Device\HarddiskVolume3\Users\Ana\Documents\Banco\informe.pdf"),
            Some(("documentos", "informe.pdf".to_owned()))
        );
        assert_eq!(
            carpeta_de_usuario(r"C:\Users\ana\Desktop\foto.jpg"),
            Some(("escritorio", "foto.jpg".to_owned()))
        );
        assert_eq!(
            carpeta_de_usuario(r"C:\Users\ana\OneDrive - Acme\Imágenes\a.png"),
            Some(("imagenes", "a.png".to_owned()))
        );
        assert_eq!(
            carpeta_de_usuario(r"C:\Users\ana\OneDrive\Documentos\x.docx:Zone.Identifier"),
            Some(("documentos", "x.docx".to_owned()))
        );
        assert_eq!(
            carpeta_de_usuario(r"C:\Users\ana\Downloads\setup.exe").map(|x| x.0),
            Some("descargas")
        );
        for fuera in [
            r"C:\Users\ana\AppData\Roaming\x.db",
            r"C:\Windows\System32\kernel32.dll",
            r"C:\Users\ana\Documents",
            r"C:\Users\ana\Documents\desktop.ini",
            r"C:\Users\ana\Desktop\Chrome.lnk",
            r"C:\Users\ana\Documents\~$carta.docx",
            r"C:\Program Files\Users\Documents",
            "",
        ] {
            assert_eq!(carpeta_de_usuario(fuera), None, "{fuera}");
        }
    }

    #[test]
    fn a_dword_is_read_from_what_reg_prints() {
        let s = "\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Windows Defender\\Spynet\r\n    \
                 SubmitSamplesConsent    REG_DWORD    0x3\r\n\r\n";
        assert_eq!(lee_dword_de_reg(s, "SubmitSamplesConsent"), Some(3));
        assert_eq!(lee_dword_de_reg(s, "SpynetReporting"), None);
        assert_eq!(lee_dword_de_reg("", "SubmitSamplesConsent"), None);
    }
}

#[cfg(test)]
mod revision_script {
    /// Nothing in the script can change the computer: no setter verbs, no deletions, no
    /// downloads. ASCII only, so standard input carries it the same in any console code page.
    #[test]
    fn the_review_script_only_reads() {
        let s = super::REVISION_PS1;
        assert!(s.is_ascii());
        let l = s.to_ascii_lowercase();
        for prohibido in [
            "set-",
            "remove-",
            "new-item",
            "new-itemproperty",
            "stop-",
            "start-",
            "restart-",
            "disable-",
            "enable-",
            "invoke-webrequest",
            "invoke-restmethod",
            "downloadstring",
            "out-file",
            "set-content",
            "add-content",
            "reg add",
            "reg delete",
            "invoke-expression",
            "iex ",
            "-encodedcommand",
        ] {
            assert!(!l.contains(prohibido), "{prohibido}");
        }
    }
}
