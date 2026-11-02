//! GUARDIANA's antivirus, «el escudo» (1.0.52, in preparation; the owner, 1 Nov 2026, 16:21: «el escáner que tenemos del
//! PC. Quiero que sea el antivirus»; 16:36: «el mejor antivirus de la Tierra… Y siempre código abierto»).
//!
//! Phase A1 (docs/ESCUDO.md): a file's SHA-256 against open lists of known malicious samples ([`huellas`]), the
//! analysis of a file or a folder ([`analisis`]) and a quarantine that can always be undone ([`cuarentena`]). Phase A3
//! starts with [`primera_vez`]: a program without a signature, run from a folder anyone can write to, calling a site this
//! computer never asked for before; and [`ladrones`]: a program that is not the browser opening its saved passwords,
//! cookies or keys, or a wallet or a messaging session; and [`cadenas`]: which program started which (a document
//! starting PowerShell, a downloaded program starting the tools that delete the backups); and [`arranque`]: a new program
//! that starts on its own with Windows, unsigned, from a folder anyone can write to or come from the internet; and
//! [`secuestro`]: one program renaming or deleting many of the person's own files in under a minute; and [`proteccion`]:
//! Windows' own protection (Defender's real-time and tamper protection, the firewall) switched off.
//! Everything happens on this computer: nothing is uploaded anywhere.
//!
//! A list is never a verdict on its own (docs/LISTS.md): whoever acts on a match also looks at who signs the file, and a
//! file signed by a trusted publisher or in Windows' own catalogue is never quarantined because of a list.

pub mod analisis;
pub mod arranque;
pub mod cadenas;
pub mod cuarentena;
pub mod huellas;
pub mod ladrones;
pub mod primera_vez;
pub mod proteccion;
pub mod secuestro;

pub use analisis::{analiza, analiza_carpeta, huella_de, Resultado, Veredicto};
pub use cuarentena::{Cuarentena, Entrada, ErrorCuarentena};
pub use huellas::{ErrorLista, Huellas};
pub use primera_vez::{zona_de, Aviso, Firma, Memoria, Mirada, Programa};
