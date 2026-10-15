//! Files between two computers: encrypted in blocks, each block authenticated and numbered,
//! with one fresh key per file. The key travels inside an Olm message
//! ([`crate::verificar::Trust::send`]), so only the verified contact can open the file; the
//! blocks themselves can go by any road (the relay, a USB stick) and nobody on the road reads
//! or alters one. A block out of order, repeated, dropped or from another file does not open.
//!
//! XChaCha20-Poly1305 (the same family vodozemac uses), 24-byte nonces made of the file's own
//! random nonce prefix and the block number, so no nonce repeats within a key.

use chacha20poly1305::aead::{Aead as _, KeyInit as _, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::Error;

/// Bytes of plaintext per block.
pub const BLOQUE: usize = 64 * 1024;
/// The largest file this version carries: 4 GiB.
pub const MAX_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const DOMINIO: &[u8] = b"GUARDIANA_SECURE_ARCHIVO_V1";

/// What the sender tells the receiver inside Olm, before any block: enough to open the blocks
/// and to know when the file is whole. No file name travels here; the person names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Llave {
    /// The file's own key, base64. Never reused.
    pub clave: String,
    /// The nonce prefix, base64 (16 bytes).
    pub prefijo: String,
    /// Plaintext size, bytes.
    pub bytes: u64,
    /// How many blocks to expect.
    pub bloques: u64,
    /// SHA-256 of the whole plaintext, hex: the receiver checks the file is the one sent.
    pub sha256: String,
}

/// One encrypted block as it travels: its number and its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bloque {
    /// Position, from 0.
    pub n: u64,
    /// Ciphertext with its tag.
    pub datos: Vec<u8>,
}

fn nonce(prefijo: &[u8; 16], n: u64) -> XNonce {
    let mut b = [0u8; 24];
    b[..16].copy_from_slice(prefijo);
    b[16..].copy_from_slice(&n.to_be_bytes());
    XNonce::from(b)
}

fn aad(n: u64, bloques: u64) -> Vec<u8> {
    let mut a = DOMINIO.to_vec();
    a.extend_from_slice(&n.to_be_bytes());
    a.extend_from_slice(&bloques.to_be_bytes());
    a
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Encrypt a whole file held in memory. Returns the key (to send inside Olm) and the blocks
/// (to send by any road). `Err` only when the file is larger than [`MAX_BYTES`] or no
/// randomness was available.
pub fn cifrar(plano: &[u8]) -> Result<(Llave, Vec<Bloque>), Error> {
    if plano.len() as u64 > MAX_BYTES {
        return Err(Error::CannotOpen);
    }
    let mut clave = [0u8; 32];
    let mut prefijo = [0u8; 16];
    getrandom::fill(&mut clave).map_err(|_| Error::CannotOpen)?;
    getrandom::fill(&mut prefijo).map_err(|_| Error::CannotOpen)?;
    let bloques = plano.chunks(BLOQUE).count().max(1) as u64;
    let cipher = XChaCha20Poly1305::new((&clave).into());
    let mut salida = Vec::with_capacity(bloques as usize);
    let trozos: Vec<&[u8]> = if plano.is_empty() {
        vec![&[][..]]
    } else {
        plano.chunks(BLOQUE).collect()
    };
    for (i, trozo) in trozos.into_iter().enumerate() {
        let n = i as u64;
        let datos = cipher
            .encrypt(
                &nonce(&prefijo, n),
                Payload {
                    msg: trozo,
                    aad: &aad(n, bloques),
                },
            )
            .map_err(|_| Error::CannotOpen)?;
        salida.push(Bloque { n, datos });
    }
    let llave = Llave {
        clave: b64(&clave),
        prefijo: b64(&prefijo),
        bytes: plano.len() as u64,
        bloques,
        sha256: hex(&Sha256::digest(plano)),
    };
    Ok((llave, salida))
}

/// Put a file back together from its key and its blocks, in any order. A missing, repeated,
/// altered or foreign block, or a size or hash that does not match, is [`Error::CannotOpen`]
/// and nothing is returned.
pub fn descifrar(llave: &Llave, bloques: &[Bloque]) -> Result<Vec<u8>, Error> {
    let clave: [u8; 32] = de_b64(&llave.clave)?
        .try_into()
        .map_err(|_| Error::CannotOpen)?;
    let prefijo: [u8; 16] = de_b64(&llave.prefijo)?
        .try_into()
        .map_err(|_| Error::CannotOpen)?;
    if llave.bytes > MAX_BYTES || bloques.len() as u64 != llave.bloques {
        return Err(Error::CannotOpen);
    }
    let mut ordenados: Vec<&Bloque> = bloques.iter().collect();
    ordenados.sort_by_key(|b| b.n);
    if ordenados.iter().enumerate().any(|(i, b)| b.n != i as u64) {
        return Err(Error::CannotOpen);
    }
    let cipher = XChaCha20Poly1305::new((&clave).into());
    let mut plano = Vec::with_capacity(usize::try_from(llave.bytes).unwrap_or(0));
    for b in ordenados {
        let trozo = cipher
            .decrypt(
                &nonce(&prefijo, b.n),
                Payload {
                    msg: &b.datos,
                    aad: &aad(b.n, llave.bloques),
                },
            )
            .map_err(|_| Error::CannotOpen)?;
        plano.extend_from_slice(&trozo);
    }
    if plano.len() as u64 != llave.bytes || hex(&Sha256::digest(&plano)) != llave.sha256 {
        return Err(Error::CannotOpen);
    }
    Ok(plano)
}

fn b64(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn de_b64(s: &str) -> Result<Vec<u8>, Error> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|_| Error::CannotOpen)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_file_comes_back_whole_and_any_change_on_the_road_is_refused() {
        let plano: Vec<u8> = (0..(BLOQUE * 2 + 1234)).map(|i| (i % 251) as u8).collect();
        let (llave, bloques) = cifrar(&plano).unwrap();
        assert_eq!(llave.bloques, 3);
        assert_eq!(llave.bytes, plano.len() as u64);
        assert!(bloques
            .iter()
            .all(|b| !b.datos.windows(16).any(|w| w == &plano[..16])));

        // Any order.
        let mut al_reves = bloques.clone();
        al_reves.reverse();
        assert_eq!(descifrar(&llave, &al_reves).unwrap(), plano);

        // A changed byte.
        let mut t = bloques.clone();
        t[1].datos[10] ^= 1;
        assert_eq!(descifrar(&llave, &t).unwrap_err(), Error::CannotOpen);
        // A dropped block, a repeated block, a renumbered block.
        assert!(descifrar(&llave, &bloques[..2]).is_err());
        let mut t = bloques.clone();
        t[2] = t[1].clone();
        assert!(descifrar(&llave, &t).is_err());
        let mut t = bloques.clone();
        t[0].n = 1;
        t[1].n = 0;
        assert!(descifrar(&llave, &t).is_err());
        // A block from another file under this key.
        let (_, otros) = cifrar(&plano).unwrap();
        let mut t = bloques.clone();
        t[0] = otros[0].clone();
        assert!(descifrar(&llave, &t).is_err());
        // The key of another file.
        let (otra, _) = cifrar(b"x").unwrap();
        assert!(descifrar(&otra, &bloques).is_err());

        // Empty files travel too, as one block.
        let (l0, b0) = cifrar(b"").unwrap();
        assert_eq!(b0.len(), 1);
        assert_eq!(descifrar(&l0, &b0).unwrap(), b"");

        // The key is a small JSON for Olm; it names no file.
        let json = serde_json::to_string(&llave).unwrap();
        assert!(json.contains("\"sha256\"") && !json.contains("nombre"));
    }
}
