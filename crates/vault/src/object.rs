//! One stored object: a file encrypted with its own key, that key wrapped with the master key.
//!
//! File layout (`objetos/<id>.gdn`):
//! ```text
//! "GDNBOV1\n"            8 bytes, magic
//! u32 big-endian         length L of the JSON header
//! L bytes                ObjectHeader as JSON (hex fields)
//! then, per chunk i in 0..trozos:
//!   24 bytes             nonce
//!   min(trozo, rest)+16  XChaCha20-Poly1305 ciphertext + tag of that chunk of the plaintext
//! ```
//! Associated data binds every piece to the object id and, for chunks, to their index and count,
//! so a chunk cannot be moved, dropped or taken from another object without the tag failing.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::error::{Error, Result};
use crate::{hexs, rng};

/// First bytes of every object file.
pub const MAGIC: &[u8; 8] = b"GDNBOV1\n";
/// Plaintext bytes per chunk (1 MiB).
pub const CHUNK: usize = 1 << 20;
/// Poly1305 tag length.
pub const TAG: usize = 16;
/// The format label used in every associated-data string.
pub const FORMAT: &str = "guardiana-boveda/1";

/// A 32-byte key sealed under another key: nonce + ciphertext(48 bytes), hex.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Wrapped {
    /// 24-byte nonce, hex.
    pub nonce: String,
    /// Ciphertext with tag, hex.
    pub envuelta: String,
}

/// Small sealed JSON (the object's metadata), hex.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Sealed {
    /// 24-byte nonce, hex.
    pub nonce: String,
    /// Ciphertext with tag, hex.
    pub cifrado: String,
}

/// What the object is, kept encrypted: only the id is visible on disk.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Meta {
    /// Original file name.
    pub nombre: String,
    /// When it was stored (RFC 3339 UTC).
    pub guardado: String,
    /// SHA-256 of the plaintext, hex, so the owner can check an extracted copy.
    pub sha256: String,
    /// Plaintext size in bytes.
    pub tamano: u64,
}

/// The clear part of an object file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectHeader {
    /// 16 hex characters, also the file name.
    pub id: String,
    /// The object key, wrapped with the master key.
    pub llave: Wrapped,
    /// The metadata, sealed with the object key.
    pub meta: Sealed,
    /// Plaintext bytes per chunk.
    pub trozo: usize,
    /// Number of chunks.
    pub trozos: u32,
    /// Plaintext size in bytes.
    pub tamano: u64,
}

fn cipher(key: &[u8; 32]) -> XChaCha20Poly1305 {
    XChaCha20Poly1305::new(key.into())
}

/// Seal `plain` under `key` with fresh nonce and `aad`; returns (nonce, ciphertext).
pub fn seal(key: &[u8; 32], plain: &[u8], aad: &str) -> Result<([u8; 24], Vec<u8>)> {
    let n = rng::nonce()?;
    let ct = cipher(key)
        .encrypt(
            XNonce::from_slice(&n),
            Payload {
                msg: plain,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| Error::Tampered("encrypt".into()))?;
    Ok((n, ct))
}

/// Open `ct` under `key`, `nonce` and `aad`; fails if anything was altered.
pub fn open(key: &[u8; 32], nonce: &[u8; 24], ct: &[u8], aad: &str) -> Result<Zeroizing<Vec<u8>>> {
    cipher(key)
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ct,
                aad: aad.as_bytes(),
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| Error::Tampered(aad.to_owned()))
}

/// Wrap a 32-byte key under another key.
pub fn wrap_key(kek: &[u8; 32], key: &[u8; 32], aad: &str) -> Result<Wrapped> {
    let (n, ct) = seal(kek, key, aad)?;
    Ok(Wrapped {
        nonce: hexs::to_hex(&n),
        envuelta: hexs::to_hex(&ct),
    })
}

/// Unwrap a key; `WrongKey` when the tag does not check (wrong password, altered file).
pub fn unwrap_key(kek: &[u8; 32], w: &Wrapped, aad: &str) -> Result<Zeroizing<[u8; 32]>> {
    let n: [u8; 24] = hexs::array(&w.nonce)?;
    let ct = hexs::from_hex(&w.envuelta)?;
    let plain = open(kek, &n, &ct, aad).map_err(|_| Error::WrongKey)?;
    let arr: [u8; 32] = plain.as_slice().try_into().map_err(|_| Error::WrongKey)?;
    Ok(Zeroizing::new(arr))
}

fn aad_key(id: &str) -> String {
    format!("{FORMAT} objeto {id} llave")
}
fn aad_meta(id: &str) -> String {
    format!("{FORMAT} objeto {id} meta")
}
fn aad_chunk(id: &str, i: u32, n: u32) -> String {
    format!("{FORMAT} objeto {id} trozo {i}/{n}")
}

/// Encrypt `src` (of `size` bytes) into the object file at `path`, under a fresh object key
/// wrapped with `master`. Returns the metadata written (with the plaintext hash).
pub fn write(
    path: &Path,
    master: &[u8; 32],
    id: &str,
    nombre: &str,
    guardado: &str,
    src: &mut impl Read,
    size: u64,
) -> Result<Meta> {
    let key = Zeroizing::new(rng::key()?);
    let trozos = u32::try_from(size.div_ceil(CHUNK as u64).max(1))
        .map_err(|_| Error::Format("file too large".into()))?;
    let mut hasher = Sha256::new();
    let mut chunks: Vec<Vec<u8>> = Vec::with_capacity(trozos as usize);
    let mut buf = Zeroizing::new(vec![0u8; CHUNK]);
    let mut total = 0u64;
    for i in 0..trozos {
        let want = usize::try_from((size - total).min(CHUNK as u64))
            .map_err(|_| Error::Format("size".into()))?;
        src.read_exact(&mut buf[..want])?;
        hasher.update(&buf[..want]);
        total += want as u64;
        let (n, ct) = seal(&key, &buf[..want], &aad_chunk(id, i, trozos))?;
        let mut piece = n.to_vec();
        piece.extend(ct);
        chunks.push(piece);
    }
    let sha = hexs::to_hex(&hasher.finalize());
    let meta = Meta {
        nombre: nombre.to_owned(),
        guardado: guardado.to_owned(),
        sha256: sha,
        tamano: size,
    };
    let (mn, mct) = seal(&key, serde_json::to_vec(&meta)?.as_slice(), &aad_meta(id))?;
    let header = ObjectHeader {
        id: id.to_owned(),
        llave: wrap_key(master, &key, &aad_key(id))?,
        meta: Sealed {
            nonce: hexs::to_hex(&mn),
            cifrado: hexs::to_hex(&mct),
        },
        trozo: CHUNK,
        trozos,
        tamano: size,
    };
    let hj = serde_json::to_vec(&header)?;
    let hl = u32::try_from(hj.len()).map_err(|_| Error::Format("header too large".into()))?;
    let mut f = File::create(path)?;
    f.write_all(MAGIC)?;
    f.write_all(&hl.to_be_bytes())?;
    f.write_all(&hj)?;
    for c in &chunks {
        f.write_all(c)?;
    }
    f.sync_all()?;
    Ok(meta)
}

/// Read the clear header of an object file; also returns the offset where the chunks start.
pub fn read_header(path: &Path) -> Result<(ObjectHeader, u64)> {
    let mut f = File::open(path)?;
    let mut magic = [0u8; 8];
    f.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(Error::Format(format!(
            "{} is not a vault object",
            path.display()
        )));
    }
    let mut lb = [0u8; 4];
    f.read_exact(&mut lb)?;
    let l = u32::from_be_bytes(lb) as usize;
    if l > 1 << 20 {
        return Err(Error::Format("header too large".into()));
    }
    let mut hj = vec![0u8; l];
    f.read_exact(&mut hj)?;
    let h: ObjectHeader = serde_json::from_slice(&hj)?;
    Ok((h, 12 + l as u64))
}

/// Unwrap the object key with the master key and open the metadata.
pub fn open_meta(h: &ObjectHeader, master: &[u8; 32]) -> Result<(Zeroizing<[u8; 32]>, Meta)> {
    let key = unwrap_key(master, &h.llave, &aad_key(&h.id))?;
    let n: [u8; 24] = hexs::array(&h.meta.nonce)?;
    let ct = hexs::from_hex(&h.meta.cifrado)?;
    let plain = open(&key, &n, &ct, &aad_meta(&h.id))?;
    let meta: Meta = serde_json::from_slice(&plain)?;
    Ok((key, meta))
}

/// Decrypt the object at `path` into `out`, verifying every chunk and the final hash.
pub fn extract(path: &Path, master: &[u8; 32], out: &mut impl Write) -> Result<Meta> {
    let (h, offset) = read_header(path)?;
    let (key, meta) = open_meta(&h, master)?;
    let mut f = File::open(path)?;
    std::io::Seek::seek(&mut f, std::io::SeekFrom::Start(offset))?;
    let mut hasher = Sha256::new();
    let mut rest = h.tamano;
    let mut buf = Vec::with_capacity(h.trozo + TAG + 24);
    for i in 0..h.trozos {
        let want =
            usize::try_from(rest.min(h.trozo as u64)).map_err(|_| Error::Format("size".into()))?;
        buf.clear();
        buf.resize(24 + want + TAG, 0);
        f.read_exact(&mut buf)?;
        let n: [u8; 24] = buf[..24]
            .try_into()
            .map_err(|_| Error::Format("nonce".into()))?;
        let plain = open(&key, &n, &buf[24..], &aad_chunk(&h.id, i, h.trozos))?;
        hasher.update(&plain);
        out.write_all(&plain)?;
        rest -= want as u64;
    }
    if hexs::to_hex(&hasher.finalize()) != meta.sha256 {
        return Err(Error::Tampered("content hash".into()));
    }
    Ok(meta)
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_two_chunks_and_tamper() {
        let dir = std::env::temp_dir().join(format!("gdn-obj-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap_or_default();
        let path = dir.join("abc.gdn");
        let master = [3u8; 32];
        let data: Vec<u8> = (0..(CHUNK + 1000)).map(|i| (i % 251) as u8).collect();
        let meta = write(
            &path,
            &master,
            "abc",
            "informe.pdf",
            "2026-09-17T00:00:00Z",
            &mut data.as_slice(),
            data.len() as u64,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(meta.tamano, data.len() as u64);
        let mut out = Vec::new();
        let m2 = extract(&path, &master, &mut out).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(out, data);
        assert_eq!(m2.nombre, "informe.pdf");
        // wrong master key
        assert!(matches!(
            extract(&path, &[4u8; 32], &mut Vec::new()),
            Err(Error::WrongKey)
        ));
        // flip one byte of the last chunk
        let mut bytes = std::fs::read(&path).unwrap_or_default();
        let last = bytes.len() - 5;
        bytes[last] ^= 1;
        std::fs::write(&path, &bytes).unwrap_or_default();
        assert!(matches!(
            extract(&path, &master, &mut Vec::new()),
            Err(Error::Tampered(_))
        ));
        std::fs::remove_dir_all(&dir).unwrap_or_default();
    }

    #[test]
    fn empty_file_is_one_empty_chunk() {
        let dir = std::env::temp_dir().join(format!("gdn-obj0-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap_or_default();
        let path = dir.join("e.gdn");
        write(
            &path,
            &[1u8; 32],
            "e",
            "vacio.txt",
            "2026-01-01T00:00:00Z",
            &mut (&[][..]),
            0,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let mut out = Vec::new();
        extract(&path, &[1u8; 32], &mut out).unwrap_or_else(|e| panic!("{e}"));
        assert!(out.is_empty());
        std::fs::remove_dir_all(&dir).unwrap_or_default();
    }
}
