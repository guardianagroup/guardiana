//! The 24 recovery words: 256 bits of entropy plus an 8-bit checksum, encoded with the BIP-39
//! English word list (2048 words, `english.txt`, from the public BIP repository). Same scheme
//! hardware wallets use, so any BIP-39 tool can check the words; the vault never uses them for
//! anything but wrapping its own master key.

use std::sync::OnceLock;

use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::error::{Error, Result};

const LIST: &str = include_str!("english.txt");

/// The 2048 words, in list order (the index of a word is its 11-bit value).
#[must_use]
pub fn list() -> &'static [&'static str] {
    static WORDS: OnceLock<Vec<&'static str>> = OnceLock::new();
    WORDS.get_or_init(|| {
        LIST.lines()
            .map(str::trim)
            .filter(|w| !w.is_empty())
            .collect()
    })
}

/// 32 bytes of entropy → 24 words.
#[must_use]
pub fn to_words(entropy: &[u8; 32]) -> Vec<&'static str> {
    let check = Sha256::digest(entropy)[0];
    let mut bits = Vec::with_capacity(264);
    for b in entropy.iter().chain(std::iter::once(&check)) {
        for i in (0..8).rev() {
            bits.push((b >> i) & 1);
        }
    }
    let words = list();
    bits.chunks(11)
        .map(|c| c.iter().fold(0usize, |acc, b| (acc << 1) | usize::from(*b)))
        .map(|i| words[i])
        .collect()
}

/// 24 words → the 32 bytes of entropy, or why they are not valid.
pub fn from_words(words: &[&str]) -> Result<Zeroizing<[u8; 32]>> {
    if words.len() != 24 {
        return Err(Error::BadWords(format!("{} words, need 24", words.len())));
    }
    let all = list();
    let mut bits = Vec::with_capacity(264);
    for w in words {
        let w = w.trim().to_ascii_lowercase();
        let idx = all
            .binary_search(&w.as_str())
            .map_err(|_| Error::BadWords(format!("unknown word «{w}»")))?;
        for i in (0..11).rev() {
            bits.push(((idx >> i) & 1) as u8);
        }
    }
    let mut entropy = Zeroizing::new([0u8; 32]);
    for (i, chunk) in bits[..256].chunks(8).enumerate() {
        entropy[i] = chunk.iter().fold(0u8, |acc, b| (acc << 1) | b);
    }
    let check = bits[256..].iter().fold(0u8, |acc, b| (acc << 1) | b);
    if Sha256::digest(*entropy)[0] != check {
        return Err(Error::BadWords(
            "a word is wrong or out of order (checksum)".into(),
        ));
    }
    Ok(entropy)
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn list_is_complete_and_sorted() {
        let l = list();
        assert_eq!(l.len(), 2048);
        assert_eq!(l[0], "abandon");
        assert_eq!(l[2047], "zoo");
        assert!(l.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn round_trip_and_checksum() {
        let e = [7u8; 32];
        let w = to_words(&e);
        assert_eq!(w.len(), 24);
        let back = from_words(&w).unwrap_or_default();
        assert_eq!(*back, e);
        let mut bad = w.clone();
        bad[3] = "zoo";
        assert!(from_words(&bad).is_err());
        assert!(from_words(&w[..23]).is_err());
    }

    #[test]
    fn bip39_vector_all_zero() {
        // Test vector from the BIP-39 reference: 32 zero bytes.
        let w = to_words(&[0u8; 32]);
        let expected = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";
        assert_eq!(w.join(" "), expected);
    }
}
