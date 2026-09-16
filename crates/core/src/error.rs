//! Error type for the core crate. No third-party error crate: the set of
//! failures is small and each one is spelled out.

use std::fmt;

/// Everything that can go wrong in the core crate.
#[derive(Debug)]
pub enum Error {
    /// The SQLite layer failed.
    Sqlite(rusqlite::Error),
    /// JSON (de)serialization failed.
    Json(serde_json::Error),
    /// Writing an export failed.
    Io(std::io::Error),
    /// A text value stored in the database is not one of the allowed values.
    UnknownValue {
        /// Which enumeration was expected (e.g. `Category`).
        kind: &'static str,
        /// The offending text.
        value: String,
    },
    /// A hash column does not contain 32 bytes / 64 hex characters.
    BadHash(String),
    /// The database was created for another installer public key.
    GenesisMismatch {
        /// Hash stored in the database when it was created.
        stored: String,
        /// Hash of the public key the running binary carries.
        expected: String,
    },
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(e) => write!(f, "sqlite: {e}"),
            Self::Json(e) => write!(f, "json: {e}"),
            Self::Io(e) => write!(f, "io: {e}"),
            Self::UnknownValue { kind, value } => write!(f, "unknown {kind} value: {value:?}"),
            Self::BadHash(v) => write!(f, "malformed hash: {v:?}"),
            Self::GenesisMismatch { stored, expected } => write!(
                f,
                "ledger was created for another public key (stored {stored}, expected {expected})"
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(e) => Some(e),
            Self::Json(e) => Some(e),
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sqlite(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
