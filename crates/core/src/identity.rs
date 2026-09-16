//! The program's identity (brief §10): the minisign public key embedded at
//! build time. Its hash is the genesis of every ledger (brief §3).
//!
//! Until `build/keys.sh` has run on the release machine the file holds a
//! development placeholder, and everything that shows the key says so.

use crate::hash::Hash;

/// Contents of `build/pubkey/minisign.pub` at build time.
pub const PUBLIC_KEY_TEXT: &str = include_str!("../../../build/pubkey/minisign.pub");

/// Marker present only in the development placeholder.
const DEV_MARKER: &str = "DEV-NOT-A-KEY";

/// True when the binary carries the development placeholder instead of the release key.
#[must_use]
pub fn public_key_is_dev() -> bool {
    PUBLIC_KEY_TEXT.contains(DEV_MARKER)
}

/// The base64 key line (second line of the minisign file), trimmed.
#[must_use]
pub fn public_key_line() -> &'static str {
    PUBLIC_KEY_TEXT.lines().nth(1).unwrap_or("").trim()
}

/// `prev_hash` of the first ledger event: SHA-256 of the public key file.
#[must_use]
pub fn genesis() -> Hash {
    Hash::of(PUBLIC_KEY_TEXT.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genesis_is_stable_for_the_embedded_key() {
        assert_eq!(genesis(), Hash::of(PUBLIC_KEY_TEXT.as_bytes()));
        assert!(!public_key_line().is_empty());
    }
}
