//! OS randomness, in one place.

use crate::error::{Error, Result};

/// `n` random bytes from the operating system.
pub fn bytes(n: usize) -> Result<Vec<u8>> {
    let mut v = vec![0u8; n];
    getrandom::fill(&mut v).map_err(|e| Error::Kdf(format!("randomness: {e}")))?;
    Ok(v)
}

/// A random 32-byte array (keys, salts, recovery entropy).
pub fn key() -> Result<[u8; 32]> {
    let mut k = [0u8; 32];
    getrandom::fill(&mut k).map_err(|e| Error::Kdf(format!("randomness: {e}")))?;
    Ok(k)
}

/// A random 24-byte XChaCha20 nonce.
pub fn nonce() -> Result<[u8; 24]> {
    let mut n = [0u8; 24];
    getrandom::fill(&mut n).map_err(|e| Error::Kdf(format!("randomness: {e}")))?;
    Ok(n)
}
