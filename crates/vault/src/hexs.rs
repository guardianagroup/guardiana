//! Hexadecimal helpers. Every binary field of the format is stored as lowercase hex so the files
//! stay plain JSON that any language can parse without a base64 library.

use crate::error::{Error, Result};

/// Lowercase hex of `bytes`.
#[must_use]
pub fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Bytes of a hex string (either case). Fails on odd length or non-hex characters.
pub fn from_hex(s: &str) -> Result<Vec<u8>> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        return Err(Error::Format(format!("odd hex length {}", s.len())));
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    for chunk in s.as_bytes().chunks(2) {
        let pair = std::str::from_utf8(chunk).map_err(|_| Error::Format("non-utf8 hex".into()))?;
        out.push(
            u8::from_str_radix(pair, 16).map_err(|_| Error::Format(format!("bad hex {pair}")))?,
        );
    }
    Ok(out)
}

/// Fixed-size array from hex, or a format error naming the expected size.
pub fn array<const N: usize>(s: &str) -> Result<[u8; N]> {
    let v = from_hex(s)?;
    v.try_into()
        .map_err(|v: Vec<u8>| Error::Format(format!("expected {N} bytes, got {}", v.len())))
}
