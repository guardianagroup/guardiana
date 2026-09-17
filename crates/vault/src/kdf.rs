//! Argon2id: from a master password (or the 24 recovery words) to a 32-byte key, slowly and with a
//! lot of memory so guessing passwords offline is expensive (concept §04 step 01).

use argon2::{Algorithm, Argon2, Params, Version};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::error::{Error, Result};

/// Argon2id cost parameters, stored in the vault header so the reader uses the same ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    /// Memory in KiB (64 MiB by default).
    pub memoria_kib: u32,
    /// Passes over the memory.
    pub pasadas: u32,
    /// Parallel lanes.
    pub hilos: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        Self {
            memoria_kib: 64 * 1024,
            pasadas: 3,
            hilos: 1,
        }
    }
}

/// Derive a 32-byte key from `secret` and `salt` with Argon2id v1.3 and these parameters.
pub fn derive(secret: &[u8], salt: &[u8], p: KdfParams) -> Result<Zeroizing<[u8; 32]>> {
    let params = Params::new(p.memoria_kib, p.pasadas, p.hilos, Some(32))
        .map_err(|e| Error::Kdf(e.to_string()))?;
    let a = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = Zeroizing::new([0u8; 32]);
    a.hash_password_into(secret, salt, out.as_mut())
        .map_err(|e| Error::Kdf(e.to_string()))?;
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn same_input_same_key_and_salt_matters() {
        let p = KdfParams {
            memoria_kib: 8 * 1024,
            pasadas: 1,
            hilos: 1,
        };
        let a = derive(b"clave", b"sal-de-16-bytes!", p).unwrap_or_default();
        let b = derive(b"clave", b"sal-de-16-bytes!", p).unwrap_or_default();
        let c = derive(b"clave", b"otra-sal-16-byte", p).unwrap_or_default();
        assert_eq!(*a, *b);
        assert_ne!(*a, *c);
    }
}
