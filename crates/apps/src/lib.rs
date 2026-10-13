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
mod firmas;
#[cfg(windows)]
mod red;
#[cfg(windows)]
mod seguridad;
#[cfg(windows)]
mod ventanas;

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
