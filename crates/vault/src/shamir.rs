//! Shamir secret sharing over GF(256) (polynomial x⁸+x⁴+x³+x+1, the AES field): the 32 bytes of
//! recovery entropy split into 5 pieces of which any 3 rebuild it (concept §02, "catástrofe o
//! guerra"). Each byte of the secret is the constant term of its own random degree-(k−1)
//! polynomial; piece `x` holds the polynomial values at that `x`. Two pieces reveal nothing.

use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::{hexs, rng};

fn gf_mul(mut a: u8, mut b: u8) -> u8 {
    let mut p = 0u8;
    for _ in 0..8 {
        if b & 1 != 0 {
            p ^= a;
        }
        let carry = a & 0x80;
        a <<= 1;
        if carry != 0 {
            a ^= 0x1b;
        }
        b >>= 1;
    }
    p
}

fn gf_pow(mut a: u8, mut e: u8) -> u8 {
    let mut r = 1u8;
    while e > 0 {
        if e & 1 != 0 {
            r = gf_mul(r, a);
        }
        a = gf_mul(a, a);
        e >>= 1;
    }
    r
}

/// Multiplicative inverse in GF(256): a^254.
fn gf_inv(a: u8) -> u8 {
    gf_pow(a, 254)
}

/// Split `secret` into `n` pieces, any `k` of which rebuild it. Piece `i` is `[x, y₁…yₘ]` with
/// `x = i+1`.
pub fn split(secret: &[u8], n: u8, k: u8) -> Result<Vec<Vec<u8>>> {
    if k < 2 || n < k {
        return Err(Error::BadShare(format!("need 2 ≤ k ≤ n, got k={k} n={n}")));
    }
    let coeffs = rng::bytes(secret.len() * usize::from(k - 1))?;
    let mut pieces: Vec<Vec<u8>> = (1..=n).map(|x| vec![x]).collect();
    for (bi, &s) in secret.iter().enumerate() {
        for piece in &mut pieces {
            let x = piece[0];
            let mut y = s;
            let mut xp = x;
            for j in 0..usize::from(k - 1) {
                y ^= gf_mul(coeffs[bi * usize::from(k - 1) + j], xp);
                xp = gf_mul(xp, x);
            }
            piece.push(y);
        }
    }
    Ok(pieces)
}

/// Rebuild the secret from any `k` distinct pieces (Lagrange interpolation at x = 0).
pub fn combine(pieces: &[Vec<u8>]) -> Result<Vec<u8>> {
    if pieces.len() < 2 {
        return Err(Error::BadShare("fewer than 2 pieces".into()));
    }
    let len = pieces[0].len();
    if len < 2 || pieces.iter().any(|p| p.len() != len) {
        return Err(Error::BadShare("pieces of different length".into()));
    }
    let xs: Vec<u8> = pieces.iter().map(|p| p[0]).collect();
    for (i, &x) in xs.iter().enumerate() {
        if x == 0 || xs[..i].contains(&x) {
            return Err(Error::BadShare("repeated or invalid piece index".into()));
        }
    }
    let mut out = Vec::with_capacity(len - 1);
    for b in 1..len {
        let mut acc = 0u8;
        for (i, pi) in pieces.iter().enumerate() {
            let mut num = 1u8;
            let mut den = 1u8;
            for (j, &xj) in xs.iter().enumerate() {
                if i != j {
                    num = gf_mul(num, xj);
                    den = gf_mul(den, xs[i] ^ xj);
                }
            }
            acc ^= gf_mul(pi[b], gf_mul(num, gf_inv(den)));
        }
        out.push(acc);
    }
    Ok(out)
}

/// Text form of a piece: `GDN-3DE5-<x>-<hex>-<check>` (check = first 2 bytes of SHA-256 of x‖data).
#[must_use]
pub fn to_text(piece: &[u8], k: u8, n: u8) -> String {
    let check = &Sha256::digest(piece)[..2];
    format!(
        "GDN-{k}DE{n}-{}-{}-{}",
        piece[0],
        hexs::to_hex(&piece[1..]),
        hexs::to_hex(check)
    )
}

/// Parse the text form back into a piece, checking its checksum.
pub fn from_text(s: &str) -> Result<Vec<u8>> {
    let parts: Vec<&str> = s.trim().split('-').collect();
    if parts.len() != 5 || parts[0] != "GDN" {
        return Err(Error::BadShare(
            "expected GDN-3DE5-<n>-<hex>-<check>".into(),
        ));
    }
    let x: u8 = parts[2]
        .parse()
        .map_err(|_| Error::BadShare("bad piece number".into()))?;
    let mut piece = vec![x];
    piece.extend(hexs::from_hex(parts[3]).map_err(|_| Error::BadShare("bad hex".into()))?);
    let check = hexs::from_hex(parts[4]).map_err(|_| Error::BadShare("bad check".into()))?;
    if check != Sha256::digest(&piece)[..2] {
        return Err(Error::BadShare("a character is wrong (checksum)".into()));
    }
    Ok(piece)
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn field_basics() {
        assert_eq!(gf_mul(0x53, 0xca), 0x01); // known AES inverse pair
        assert_eq!(gf_inv(0x53), 0xca);
        for a in 1..=255u8 {
            assert_eq!(gf_mul(a, gf_inv(a)), 1);
        }
    }

    #[test]
    fn any_three_of_five_rebuild_two_do_not() {
        let secret: Vec<u8> = (0..32).map(|i| i * 7 + 3).collect();
        let pieces = split(&secret, 5, 3).unwrap_or_default();
        assert_eq!(pieces.len(), 5);
        for combo in [[0, 1, 2], [0, 2, 4], [1, 3, 4], [2, 3, 4]] {
            let sel: Vec<Vec<u8>> = combo.iter().map(|&i| pieces[i].clone()).collect();
            assert_eq!(combine(&sel).unwrap_or_default(), secret);
        }
        let two = vec![pieces[0].clone(), pieces[1].clone()];
        assert_ne!(combine(&two).unwrap_or_default(), secret);
        let four: Vec<Vec<u8>> = pieces[..4].to_vec();
        assert_eq!(combine(&four).unwrap_or_default(), secret);
    }

    #[test]
    fn text_round_trip_and_checksum() {
        let pieces = split(&[9u8; 32], 5, 3).unwrap_or_default();
        let t = to_text(&pieces[2], 3, 5);
        assert!(t.starts_with("GDN-3DE5-3-"));
        assert_eq!(from_text(&t).unwrap_or_default(), pieces[2]);
        let mut broken = t.clone();
        broken.replace_range(11..12, if &t[11..12] == "a" { "b" } else { "a" });
        assert!(from_text(&broken).is_err());
    }
}
