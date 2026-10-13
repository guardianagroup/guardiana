//! Cutting what a program sends out, cleanly and always reversibly (1.0.22, the owner, 13 Oct
//! 2026: «quiero que la información que sale del antivirus se pueda cortar… el corte limpio»).
//!
//! GUARDIANA already cuts names at the DNS layer. A program that connects straight to an address,
//! without asking for a name, slips past that. So, when the person asks, GUARDIANA blocks the
//! program's outgoing connections at the network layer, with the Windows Filtering Platform (WFP),
//! the same mechanism a firewall uses. **No kernel driver of our own**: these are declarative
//! filters a service running as the system adds from user mode (checked with ChatGPT against
//! Microsoft's docs, 13 Oct 2026).
//!
//! What it is, and what it is not (said honestly on the screen):
//! - It blocks, cleanly. The program's connection fails; GUARDIANA does not pretend it succeeded,
//!   does not read anything, does not break TLS. Blocking is not deception.
//! - It is by the program's file (its path). All instances of that file are covered; moving the
//!   file needs the rule renewed.
//! - It is always reversible: every filter carries GUARDIANA's own provider and sublayer, so they
//!   can be found and removed, and nothing else is touched.
//!
//! On anything but Windows this is not available and every call says so.

use std::fmt;

/// Why a cut could not be made or undone.
#[derive(Debug)]
pub enum ErrorCorte {
    /// Only Windows has this.
    NoSoportado,
    /// Windows refused; the text is the call that failed and its code.
    Sistema(String),
}

impl fmt::Display for ErrorCorte {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSoportado => write!(
                f,
                "cortar lo que sale de un programa solo está en Windows (cortafuegos del sistema)"
            ),
            Self::Sistema(e) => write!(f, "Windows no dejó cambiar el cortafuegos: {e}"),
        }
    }
}

impl std::error::Error for ErrorCorte {}

/// What a program is blocked from reaching, when it is blocked.
#[derive(Debug, Clone, Default)]
pub struct Destino {
    /// Only this remote IP (v4 or v6) as text; empty for «everywhere».
    pub ip: String,
    /// Only this remote port; 0 for «any port».
    pub puerto: u16,
}

#[cfg(windows)]
mod ventanas;

/// Blocks and unblocks the outgoing traffic of programs, by file path.
pub struct Cortafuegos {
    #[cfg(windows)]
    interno: ventanas::Motor,
}

impl Cortafuegos {
    /// Opens the filter engine and makes sure GUARDIANA's own provider and sublayer exist.
    ///
    /// # Errors
    /// [`ErrorCorte::NoSoportado`] outside Windows; [`ErrorCorte::Sistema`] when Windows refuses
    /// (which means: not running as the system, or the filter platform is unavailable).
    pub fn abrir() -> Result<Self, ErrorCorte> {
        #[cfg(windows)]
        {
            Ok(Self {
                interno: ventanas::Motor::abrir()?,
            })
        }
        #[cfg(not(windows))]
        {
            Err(ErrorCorte::NoSoportado)
        }
    }

    /// Blocks the program at `ruta` from reaching `destino` (everywhere when `destino` is empty).
    /// Returns the ids of the filters added (one for IPv4, one for IPv6), to undo later.
    ///
    /// # Errors
    /// [`ErrorCorte::Sistema`] when Windows refuses the filter.
    pub fn cortar(&self, ruta: &str, destino: &Destino) -> Result<Vec<u64>, ErrorCorte> {
        #[cfg(windows)]
        {
            self.interno.cortar(ruta, destino)
        }
        #[cfg(not(windows))]
        {
            let _ = (ruta, destino);
            Err(ErrorCorte::NoSoportado)
        }
    }

    /// Removes the filters with these ids. Ones already gone are ignored.
    ///
    /// # Errors
    /// [`ErrorCorte::Sistema`] when Windows refuses a delete for a reason other than «not found».
    pub fn quitar(&self, ids: &[u64]) -> Result<(), ErrorCorte> {
        #[cfg(windows)]
        {
            self.interno.quitar(ids)
        }
        #[cfg(not(windows))]
        {
            let _ = ids;
            Err(ErrorCorte::NoSoportado)
        }
    }

    /// Removes every filter GUARDIANA added (its whole sublayer), to clean up on uninstall or
    /// when the person turns cutting off entirely.
    ///
    /// # Errors
    /// [`ErrorCorte::Sistema`] when Windows refuses.
    pub fn quitar_todo(&self) -> Result<u64, ErrorCorte> {
        #[cfg(windows)]
        {
            self.interno.quitar_todo()
        }
        #[cfg(not(windows))]
        {
            Err(ErrorCorte::NoSoportado)
        }
    }

    /// How many of GUARDIANA's filters are in place right now.
    ///
    /// # Errors
    /// [`ErrorCorte::Sistema`] when Windows refuses the enumeration.
    pub fn cuantos(&self) -> Result<usize, ErrorCorte> {
        #[cfg(windows)]
        {
            self.interno.cuantos()
        }
        #[cfg(not(windows))]
        {
            Err(ErrorCorte::NoSoportado)
        }
    }
}

#[cfg(test)]
#[cfg(not(windows))]
#[allow(clippy::expect_used)]
mod pruebas {
    use super::{Cortafuegos, ErrorCorte};

    #[test]
    fn fuera_de_windows_lo_dice_y_no_rompe_nada() {
        let e = Cortafuegos::abrir()
            .err()
            .expect("debería no estar disponible");
        assert!(matches!(e, ErrorCorte::NoSoportado));
        assert!(e.to_string().contains("Windows"));
    }
}
