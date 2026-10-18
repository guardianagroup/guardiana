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
mod firmas;
#[cfg(windows)]
mod red;
#[cfg(windows)]
mod revision;
#[cfg(windows)]
mod seguridad;
#[cfg(windows)]
mod ventanas;
mod vpn;

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
