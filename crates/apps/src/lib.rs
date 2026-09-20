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
    fn fuera_de_windows_lo_dice_y_no_rompe_nada() {
        let e = Observador::arrancar()
            .err()
            .expect("debería no estar disponible");
        assert!(matches!(e, Error::NoSoportado));
        assert!(e.to_string().contains("solo Windows"));
    }
}
