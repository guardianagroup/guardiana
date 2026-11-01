//! GUARDIANA's antivirus, «el escudo» (1.0.52, in preparation; the owner, 1 Nov 2026, 16:21: «el escáner que tenemos del
//! PC. Quiero que sea el antivirus»; 16:36: «el mejor antivirus de la Tierra… Y siempre código abierto»).
//!
//! Phase A1 (docs/ESCUDO.md): a file's SHA-256 against open lists of known malicious samples ([`huellas`]), the
//! analysis of a file or a folder ([`analisis`]) and a quarantine that can always be undone ([`cuarentena`]).
//! Everything happens on this computer: nothing is uploaded anywhere.
//!
//! A list is never a verdict on its own (docs/LISTS.md): whoever acts on a match also looks at who signs the file, and a
//! file signed by a trusted publisher or in Windows' own catalogue is never quarantined because of a list.

pub mod analisis;
pub mod cuarentena;
pub mod huellas;

pub use analisis::{analiza, analiza_carpeta, huella_de, Resultado, Veredicto};
pub use cuarentena::{Cuarentena, Entrada, ErrorCuarentena};
pub use huellas::{ErrorLista, Huellas};
