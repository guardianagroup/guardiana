//! Errors of the vault crate. No third-party error crate: each failure is spelled out.

use std::fmt;
use std::path::PathBuf;

/// Everything that can go wrong in the vault.
#[derive(Debug)]
pub enum Error {
    /// Reading or writing a file failed.
    Io(std::io::Error),
    /// A JSON header or log line could not be parsed or written.
    Json(serde_json::Error),
    /// The header, an object or the log is not in the documented format.
    Format(String),
    /// The password or the recovery words did not open the wrapped key.
    WrongKey,
    /// An object failed authentication: altered, truncated or wrong key.
    Tampered(String),
    /// The recovery words are not 24 valid words with a good checksum.
    BadWords(String),
    /// A recovery piece is malformed, or fewer than needed were given.
    BadShare(String),
    /// Key derivation refused its parameters.
    Kdf(String),
    /// The access-log chain breaks at this line number.
    ChainBroken(u64),
    /// A vault already exists at this path.
    Exists(PathBuf),
    /// No vault at this path.
    NotFound(PathBuf),
    /// No object with this id or name.
    NoObject(String),
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io: {e}"),
            Self::Json(e) => write!(f, "json: {e}"),
            Self::Format(s) => write!(f, "format: {s}"),
            Self::WrongKey => write!(f, "wrong password or recovery words"),
            Self::Tampered(s) => write!(f, "object altered or truncated: {s}"),
            Self::BadWords(s) => write!(f, "recovery words: {s}"),
            Self::BadShare(s) => write!(f, "recovery piece: {s}"),
            Self::Kdf(s) => write!(f, "key derivation: {s}"),
            Self::ChainBroken(n) => write!(f, "access log chain broken at line {n}"),
            Self::Exists(p) => write!(f, "a vault already exists at {}", p.display()),
            Self::NotFound(p) => write!(f, "no vault at {}", p.display()),
            Self::NoObject(s) => write!(f, "no object {s}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}
