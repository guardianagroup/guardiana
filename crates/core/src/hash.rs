//! The hash chain of the ledger (brief §3):
//! `row_hash = sha256(prev_hash || fields)`.
//!
//! Fields are encoded unambiguously: each one as a 4-byte big-endian length
//! followed by its UTF-8 bytes, in the fixed order documented on
//! [`HashInput`]. Anyone can recompute a row with only this description,
//! which is what `guardiana ledger --check` and `docs/VERIFY.md` rely on.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

/// A SHA-256 digest. Shown and exported as 64 lowercase hex characters.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hash(pub [u8; 32]);

impl Hash {
    /// SHA-256 of arbitrary bytes.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        let digest = Sha256::digest(bytes);
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        Self(out)
    }

    /// Lowercase hexadecimal form.
    #[must_use]
    pub fn to_hex(self) -> String {
        let mut s = String::with_capacity(64);
        for b in self.0 {
            s.push_str(&format!("{b:02x}"));
        }
        s
    }

    /// Parse the hexadecimal form produced by [`Hash::to_hex`].
    pub fn from_hex(s: &str) -> Result<Self> {
        if s.len() != 64 {
            return Err(Error::BadHash(s.to_owned()));
        }
        let mut out = [0u8; 32];
        for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
            let pair = std::str::from_utf8(chunk).map_err(|_| Error::BadHash(s.to_owned()))?;
            out[i] = u8::from_str_radix(pair, 16).map_err(|_| Error::BadHash(s.to_owned()))?;
        }
        Ok(Self(out))
    }

    /// Build from the raw 32 bytes stored in a BLOB column.
    pub fn from_bytes(b: &[u8]) -> Result<Self> {
        let arr: [u8; 32] = b
            .try_into()
            .map_err(|_| Error::BadHash(format!("{} bytes", b.len())))?;
        Ok(Self(arr))
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hash({})", self.to_hex())
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl Serialize for Hash {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Hash {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::from_hex(&s).map_err(serde::de::Error::custom)
    }
}

/// The fields that enter a row hash, in this exact order.
///
/// `ts` and `rule_id` are hashed as decimal text (`rule_id` empty when
/// absent); `signals_json` is hashed exactly as stored, byte for byte.
#[derive(Debug, Clone, Copy)]
pub struct HashInput<'a> {
    /// Unix time in milliseconds.
    pub ts: i64,
    /// Device identifier.
    pub device_id: &'a str,
    /// IP the query came from.
    pub client_ip: &'a str,
    /// Queried name.
    pub qname: &'a str,
    /// Query type (`A`, `AAAA`, ...).
    pub qtype: &'a str,
    /// Category text (`rastreador`, ...).
    pub category: &'a str,
    /// Which list produced the category, or empty.
    pub list_source: &'a str,
    /// JSON array of signal names, as stored.
    pub signals_json: &'a str,
    /// Verdict text (`observado`, ...).
    pub verdict: &'a str,
    /// Who decided (`nadie`, ...).
    pub decided_by: &'a str,
    /// Rule that caused the verdict, if any.
    pub rule_id: Option<i64>,
}

/// Compute `sha256(prev_hash || fields)` for one ledger row.
#[must_use]
pub fn chain_hash(prev: &Hash, input: &HashInput<'_>) -> Hash {
    let mut h = Sha256::new();
    h.update(prev.0);
    let ts = input.ts.to_string();
    let rule = input.rule_id.map(|r| r.to_string()).unwrap_or_default();
    for field in [
        ts.as_str(),
        input.device_id,
        input.client_ip,
        input.qname,
        input.qtype,
        input.category,
        input.list_source,
        input.signals_json,
        input.verdict,
        input.decided_by,
        rule.as_str(),
    ] {
        let len = u32::try_from(field.len()).unwrap_or(u32::MAX);
        h.update(len.to_be_bytes());
        h.update(field.as_bytes());
    }
    let digest = h.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    Hash(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> HashInput<'static> {
        HashInput {
            ts: 1_700_000_000_000,
            device_id: "self",
            client_ip: "127.0.0.1",
            qname: "example.com",
            qtype: "A",
            category: "desconocido",
            list_source: "",
            signals_json: "[]",
            verdict: "observado",
            decided_by: "nadie",
            rule_id: None,
        }
    }

    #[test]
    fn hex_round_trip() {
        let h = Hash::of(b"guardiana");
        assert_eq!(Hash::from_hex(&h.to_hex()).ok(), Some(h));
        assert!(Hash::from_hex("zz").is_err());
    }

    #[test]
    fn chain_is_deterministic_and_order_sensitive() {
        let genesis = Hash::of(b"key");
        let a = chain_hash(&genesis, &sample());
        let b = chain_hash(&genesis, &sample());
        assert_eq!(a, b);
        let mut swapped = sample();
        swapped.device_id = "127.0.0.1";
        swapped.client_ip = "self";
        assert_ne!(a, chain_hash(&genesis, &swapped));
        assert_ne!(a, chain_hash(&Hash::of(b"other"), &sample()));
    }

    #[test]
    fn length_prefix_prevents_field_boundary_ambiguity() {
        // "ab" + "c" must not hash like "a" + "bc".
        let genesis = Hash::of(b"key");
        let mut x = sample();
        x.qname = "ab";
        x.qtype = "c";
        let mut y = sample();
        y.qname = "a";
        y.qtype = "bc";
        assert_ne!(chain_hash(&genesis, &x), chain_hash(&genesis, &y));
    }
}
