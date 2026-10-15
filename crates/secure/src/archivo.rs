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
/// The largest file this version carries: 1 TiB. Francisco, 15 Oct 2026: «el envío de archivos
/// grandes hay que incluirlo, lo máximo posible». Nothing here holds the whole file: the
/// streaming pair ([`Cifrador`], [`Descifrador`]) works one block at a time, so the limit is
/// the disk, not the memory.
pub const MAX_BYTES: u64 = 1024 * 1024 * 1024 * 1024;
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

/// Encrypt a file of any size block by block: read from `origen`, write each block's bytes
/// (16-byte length prefix, big-endian, then the block) to `destino`. Nothing is held beyond one
/// block. Returns the key for Olm once the whole file has gone through.
pub struct Cifrador {
    clave: [u8; 32],
    prefijo: [u8; 16],
    cipher: XChaCha20Poly1305,
    hash: Sha256,
    bytes: u64,
    bloques: u64,
}

impl Cifrador {
    /// Fresh key and nonce prefix for one file.
    pub fn new() -> Result<Self, Error> {
        let mut clave = [0u8; 32];
        let mut prefijo = [0u8; 16];
        getrandom::fill(&mut clave).map_err(|_| Error::CannotOpen)?;
        getrandom::fill(&mut prefijo).map_err(|_| Error::CannotOpen)?;
        Ok(Self {
            clave,
            prefijo,
            cipher: XChaCha20Poly1305::new((&clave).into()),
            hash: Sha256::new(),
            bytes: 0,
            bloques: 0,
        })
    }

    /// Stream the whole file. The block count is not known until the end, so blocks are bound
    /// to their number only; the key (sent after) carries the count and the hash, and the
    /// receiver refuses a file whose count or hash differ.
    /// The same file key again, to resume a transfer that broke: the receiver says how many
    /// blocks it already holds and the sender encrypts the rest with the very same key and
    /// nonces, so the blocks join up. The key only ever lives in the two computers.
    pub fn con(llave: &Llave) -> Result<Self, Error> {
        let clave: [u8; 32] = de_b64(&llave.clave)?
            .try_into()
            .map_err(|_| Error::CannotOpen)?;
        let prefijo: [u8; 16] = de_b64(&llave.prefijo)?
            .try_into()
            .map_err(|_| Error::CannotOpen)?;
        Ok(Self {
            clave,
            prefijo,
            cipher: XChaCha20Poly1305::new((&clave).into()),
            hash: Sha256::new(),
            bytes: 0,
            bloques: 0,
        })
    }

    /// Encrypt the whole file as a stream.
    pub fn cifra<R: std::io::Read, W: std::io::Write>(
        self,
        origen: R,
        destino: W,
    ) -> Result<Llave, Error> {
        self.cifra_desde(origen, destino, 0)
    }

    /// Encrypt from block `desde` on (the earlier ones are read and hashed, not written), so a
    /// transfer resumes where it stopped. The returned key is the same as the first time.
    pub fn cifra_desde<R: std::io::Read, W: std::io::Write>(
        mut self,
        mut origen: R,
        mut destino: W,
        desde: u64,
    ) -> Result<Llave, Error> {
        let mut buf = vec![0u8; BLOQUE];
        let mut primero = true;
        loop {
            let n = lee_lleno(&mut origen, &mut buf)?;
            if n == 0 && !primero {
                break;
            }
            primero = false;
            let trozo = &buf[..n];
            self.hash.update(trozo);
            self.bytes += n as u64;
            if self.bytes > MAX_BYTES {
                return Err(Error::CannotOpen);
            }
            let c = self
                .cipher
                .encrypt(
                    &nonce(&self.prefijo, self.bloques),
                    Payload {
                        msg: trozo,
                        aad: &aad_flujo(self.bloques),
                    },
                )
                .map_err(|_| Error::CannotOpen)?;
            if self.bloques >= desde {
                destino
                    .write_all(&(c.len() as u64).to_be_bytes())
                    .and_then(|()| destino.write_all(&c))
                    .map_err(|_| Error::CannotOpen)?;
            }
            self.bloques += 1;
            if n < BLOQUE {
                break;
            }
        }
        destino.flush().map_err(|_| Error::CannotOpen)?;
        Ok(Llave {
            clave: b64(&self.clave),
            prefijo: b64(&self.prefijo),
            bytes: self.bytes,
            bloques: self.bloques,
            sha256: hex(&self.hash.finalize()),
        })
    }
}

/// The other end of [`Cifrador`]: read the stream block by block, write the plaintext as it
/// comes, and only at the end say whether the whole file is the one that was sent. A caller
/// that writes to disk keeps the file under a temporary name until `Ok`.
pub fn descifra_flujo<R: std::io::Read, W: std::io::Write>(
    llave: &Llave,
    mut origen: R,
    mut destino: W,
) -> Result<(), Error> {
    let clave: [u8; 32] = de_b64(&llave.clave)?
        .try_into()
        .map_err(|_| Error::CannotOpen)?;
    let prefijo: [u8; 16] = de_b64(&llave.prefijo)?
        .try_into()
        .map_err(|_| Error::CannotOpen)?;
    if llave.bytes > MAX_BYTES {
        return Err(Error::CannotOpen);
    }
    let cipher = XChaCha20Poly1305::new((&clave).into());
    let mut hash = Sha256::new();
    let mut bytes = 0u64;
    let mut n = 0u64;
    let mut largo = [0u8; 8];
    while n < llave.bloques {
        origen
            .read_exact(&mut largo)
            .map_err(|_| Error::CannotOpen)?;
        let l = usize::try_from(u64::from_be_bytes(largo)).map_err(|_| Error::CannotOpen)?;
        if l > BLOQUE + 64 {
            return Err(Error::CannotOpen);
        }
        let mut c = vec![0u8; l];
        origen.read_exact(&mut c).map_err(|_| Error::CannotOpen)?;
        let trozo = cipher
            .decrypt(
                &nonce(&prefijo, n),
                Payload {
                    msg: &c,
                    aad: &aad_flujo(n),
                },
            )
            .map_err(|_| Error::CannotOpen)?;
        hash.update(&trozo);
        bytes += trozo.len() as u64;
        destino.write_all(&trozo).map_err(|_| Error::CannotOpen)?;
        n += 1;
    }
    destino.flush().map_err(|_| Error::CannotOpen)?;
    if bytes != llave.bytes || hex(&hash.finalize()) != llave.sha256 {
        return Err(Error::CannotOpen);
    }
    Ok(())
}

fn aad_flujo(n: u64) -> Vec<u8> {
    let mut a = DOMINIO.to_vec();
    a.extend_from_slice(b"|flujo|");
    a.extend_from_slice(&n.to_be_bytes());
    a
}

/// Fill `buf` as far as the reader allows; fewer bytes only at the end.
fn lee_lleno<R: std::io::Read>(r: &mut R, buf: &mut [u8]) -> Result<usize, Error> {
    let mut hecho = 0;
    while hecho < buf.len() {
        match r.read(&mut buf[hecho..]) {
            Ok(0) => break,
            Ok(k) => hecho += k,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Err(Error::CannotOpen),
        }
    }
    Ok(hecho)
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
    fn a_broken_transfer_resumes_from_the_block_the_receiver_has() {
        let plano: Vec<u8> = (0..(BLOQUE * 3 + 77)).map(|i| (i % 253) as u8).collect();
        let mut todo = Vec::new();
        let llave = Cifrador::new()
            .unwrap()
            .cifra(&plano[..], &mut todo)
            .unwrap();
        assert_eq!(llave.bloques, 4);
        // The road broke after two blocks: the receiver kept them as they came.
        let mut parcial = Vec::new();
        let mut leido = &todo[..];
        for _ in 0..2 {
            let l = u64::from_be_bytes(leido[..8].try_into().unwrap()) as usize;
            parcial.extend_from_slice(&leido[..8 + l]);
            leido = &leido[8 + l..];
        }
        // The sender goes on from block 2 with the same key.
        let mut resto = Vec::new();
        let otra = Cifrador::con(&llave)
            .unwrap()
            .cifra_desde(&plano[..], &mut resto, 2)
            .unwrap();
        assert_eq!(otra, llave);
        parcial.extend_from_slice(&resto);
        assert_eq!(parcial, todo);
        let mut salida = Vec::new();
        descifra_flujo(&llave, &parcial[..], &mut salida).unwrap();
        assert_eq!(salida, plano);
    }

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

    #[test]
    fn a_large_file_streams_through_block_by_block_and_comes_back_whole() {
        // 1 MiB + a tail, through the streaming pair, without ever holding it all in the cipher.
        let plano: Vec<u8> = (0..(BLOQUE * 16 + 777))
            .map(|i| (i * 7 % 253) as u8)
            .collect();
        let mut cifrado = Vec::new();
        let llave = Cifrador::new()
            .unwrap()
            .cifra(&plano[..], &mut cifrado)
            .unwrap();
        assert_eq!(llave.bloques, 17);
        assert_eq!(llave.bytes, plano.len() as u64);
        let mut vuelta = Vec::new();
        descifra_flujo(&llave, &cifrado[..], &mut vuelta).unwrap();
        assert_eq!(vuelta, plano);
        // A byte changed on the road: refused, even though the earlier blocks were written out.
        let mut t = cifrado.clone();
        t[BLOQUE * 5] ^= 1;
        assert!(descifra_flujo(&llave, &t[..], &mut Vec::new()).is_err());
        // The stream cut short.
        assert!(descifra_flujo(&llave, &cifrado[..cifrado.len() - 10], &mut Vec::new()).is_err());
        // Another file's key over this stream.
        let otra = Cifrador::new()
            .unwrap()
            .cifra(&b"x"[..], &mut Vec::new())
            .unwrap();
        assert!(descifra_flujo(&otra, &cifrado[..], &mut Vec::new()).is_err());
        // Empty file: one block, comes back empty.
        let l0 = Cifrador::new()
            .unwrap()
            .cifra(&b""[..], &mut Vec::new())
            .unwrap();
        assert_eq!(l0.bloques, 1);
    }
}
