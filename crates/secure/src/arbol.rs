//! A hash tree over the pieces of a file (18 Oct 2026, the committee: «un árbol de hashes para
//! que una transferencia reanudada verifique cada trozo»).
//!
//! The shape is the Merkle Tree Hash of RFC 6962 (Certificate Transparency), section 2.1, with
//! SHA-256: a leaf is `SHA-256(0x00 ‖ piece)`, an inner node `SHA-256(0x01 ‖ left ‖ right)`, and
//! the tree of `n` leaves splits at the largest power of two below `n`. The two prefixes keep a
//! leaf from ever passing as a node. The same few lines run in the receiver's page with
//! WebCrypto, so both ends reach the same root without trusting each other's word.
//!
//! What it is for: a transfer that broke and resumes at piece `k` first compares the root of the
//! `k` pieces the receiver already holds with the sender's; a piece that was changed on the
//! receiver's disk, or a resume against the wrong file, shows as a different root and the
//! transfer starts that file again. The root of the whole file goes into the signed receipt.

use sha2::{Digest as _, Sha256};

/// One leaf: the hash of one piece.
#[must_use]
pub fn hoja(trozo: &[u8]) -> [u8; 32] {
    let h = Sha256::new()
        .chain_update([0u8])
        .chain_update(trozo)
        .finalize();
    let mut o = [0u8; 32];
    o.copy_from_slice(&h);
    o
}

fn nodo(izquierda: &[u8; 32], derecha: &[u8; 32]) -> [u8; 32] {
    let h = Sha256::new()
        .chain_update([1u8])
        .chain_update(izquierda)
        .chain_update(derecha)
        .finalize();
    let mut o = [0u8; 32];
    o.copy_from_slice(&h);
    o
}

/// The root of these leaves, in order. No leaves: the hash of nothing, as RFC 6962 says.
#[must_use]
pub fn raiz(hojas: &[[u8; 32]]) -> [u8; 32] {
    match hojas.len() {
        0 => {
            let h = Sha256::digest([]);
            let mut o = [0u8; 32];
            o.copy_from_slice(&h);
            o
        }
        1 => hojas[0],
        n => {
            // The largest power of two strictly below n.
            let k = 1usize << (usize::BITS - 1 - (n - 1).leading_zeros());
            nodo(&raiz(&hojas[..k]), &raiz(&hojas[k..]))
        }
    }
}

/// Hex, as the receipt and the page write it.
#[must_use]
pub fn hex(h: &[u8; 32]) -> String {
    h.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn de_hex(s: &str) -> [u8; 32] {
        let mut o = [0u8; 32];
        for (i, b) in o.iter_mut().enumerate() {
            *b = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap();
        }
        o
    }

    #[test]
    fn it_is_rfc_6962() {
        // The empty tree and the one-leaf tree of an empty piece, from RFC 6962 / RFC 9162 test
        // vectors (Certificate Transparency's own).
        assert_eq!(
            hex(&raiz(&[])),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex(&raiz(&[hoja(b"")])),
            "6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d"
        );
        // Certificate Transparency's vectors for the leaves "", 0x00, 0x10, 0x2021, 0x3031.
        let hojas = [
            hoja(b""),
            hoja(&[0x00]),
            hoja(&[0x10]),
            hoja(&[0x20, 0x21]),
            hoja(&[0x30, 0x31]),
        ];
        assert_eq!(
            raiz(&hojas[..2]),
            de_hex("fac54203e7cc696cf0dfcb42c92a1d9dbaf70ad9e621f4bd8d98662f00e3c125")
        );
        assert_eq!(
            raiz(&hojas[..3]),
            de_hex("aeb6bcfe274b70a14fb067a5e5578264db0fa9b51af5e0ba159158f329e06e77")
        );
        assert_eq!(
            raiz(&hojas[..4]),
            de_hex("d37ee418976dd95753c1c73862b9398fa2a2cf9b4ff0fdfe8b30cd95209614b7")
        );
        assert_eq!(
            raiz(&hojas),
            de_hex("4e3bbb1f7b478dcfe71fb631631519a3bca12c9aefca1612bfce4c13a86264d4")
        );
    }

    #[test]
    fn a_changed_or_missing_piece_changes_the_root() {
        let trozos: Vec<Vec<u8>> = (0u8..7).map(|i| vec![i; 100]).collect();
        let hojas: Vec<[u8; 32]> = trozos.iter().map(|t| hoja(t)).collect();
        let r = raiz(&hojas);
        let mut otro = hojas.clone();
        otro[3] = hoja(&[9; 100]);
        assert_ne!(raiz(&otro), r);
        assert_ne!(raiz(&hojas[..6]), r);
        let mut cambiadas = hojas.clone();
        cambiadas.swap(1, 2);
        assert_ne!(raiz(&cambiadas), r, "order counts");
        // A leaf never passes as a node: two leaves are not one leaf of their concatenation.
        assert_ne!(raiz(&hojas[..2]), hoja(&[hojas[0], hojas[1]].concat()));
    }
}
