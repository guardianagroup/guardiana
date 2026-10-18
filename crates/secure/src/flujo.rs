//! Large files in pieces for the delivery room (18 Oct 2026): every piece sealed on its own with
//! AES-256-GCM in the STREAM construction (Hoang, Reyhanitabar, Rogaway, Vizár: «Online
//! Authenticated-Encryption and its Nonce-Reuse Misuse-Resistance», CRYPTO 2015), so a file of
//! any size is encrypted and checked one piece at a time, and nobody on the road can drop,
//! repeat, reorder or cut a piece without the receiver seeing it.
//!
//! The nonce of piece `n` is the file's random 7-byte prefix, `n` as 4 bytes big-endian, and one
//! byte that is 1 only for the last piece: exactly the layout of RustCrypto's
//! `aead::stream::StreamBE32`. A piece moved to another place opens under another nonce and
//! fails; a file cut after any piece fails because no earlier piece was sealed as the last one.
//! The additional data binds every piece to its delivery and its file, so a piece of one file
//! never opens inside another.
//!
//! Why AES-256-GCM here and XChaCha20-Poly1305 in a conversation's files ([`crate::archivo`]):
//! the receiver of a delivery is any browser, without an account and without GUARDIANA, and the
//! only authenticated cipher every browser carries (WebCrypto) is AES-GCM. The cipher is
//! `ring`'s, the same audited code the TLS of this program uses; nothing here is a new primitive.
//! A 256-bit key also keeps about 128 bits of strength against a quantum computer (Grover); the
//! key reaches the receiver only through the link and the sealed channel, never through a
//! public-key exchange of its own.
//!
//! Each piece also becomes a leaf of the file's hash tree ([`crate::arbol`]), whose root goes
//! into the signed receipt, together with the SHA-256 of the whole file.

use ring::aead::{self, Aad, LessSafeKey, Nonce, UnboundKey};
use sha2::{Digest as _, Sha256};

use crate::arbol;
use crate::Error;

/// Plaintext bytes per piece: 1 MiB (a browser holds a few of them at once without trouble).
pub const TROZO: usize = 1 << 20;
/// Bytes of the per-file nonce prefix.
pub const PREFIJO: usize = 7;
const DOMINIO: &[u8] = b"GUARDIANA_ENTREGA_STREAM_V1";
/// The most pieces one file may have (the counter is 32 bits).
pub const MAX_TROZOS: u64 = u32::MAX as u64;

/// How many pieces a file of `bytes` has: at least one, so an empty file still has its last
/// piece.
#[must_use]
pub fn trozos(bytes: u64) -> u64 {
    bytes.div_ceil(TROZO as u64).max(1)
}

/// One file's key and nonce prefix. Overwritten with zeros when dropped.
pub struct Clave {
    bytes: [u8; 32],
    prefijo: [u8; PREFIJO],
}

impl Drop for Clave {
    fn drop(&mut self) {
        use zeroize::Zeroize as _;
        self.bytes.zeroize();
        self.prefijo.zeroize();
    }
}

impl std::fmt::Debug for Clave {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Clave(…)")
    }
}

impl Clave {
    /// A fresh key and prefix from the system's randomness.
    pub fn nueva() -> Result<Self, Error> {
        let mut bytes = [0u8; 32];
        let mut prefijo = [0u8; PREFIJO];
        getrandom::fill(&mut bytes).map_err(|_| Error::CannotStart)?;
        getrandom::fill(&mut prefijo).map_err(|_| Error::CannotStart)?;
        Ok(Self { bytes, prefijo })
    }

    /// A key kept before (the delivery's own saved state).
    #[must_use]
    pub fn de(bytes: [u8; 32], prefijo: [u8; PREFIJO]) -> Self {
        Self { bytes, prefijo }
    }

    /// The key, to hand to the receiver inside the sealed channel and to keep at rest wrapped.
    #[must_use]
    pub fn bytes(&self) -> &[u8; 32] {
        &self.bytes
    }

    /// The nonce prefix.
    #[must_use]
    pub fn prefijo(&self) -> &[u8; PREFIJO] {
        &self.prefijo
    }
}

/// The sealing of one file: its key and its additional data.
pub struct Flujo {
    clave: LessSafeKey,
    prefijo: [u8; PREFIJO],
    aad: Vec<u8>,
}

impl std::fmt::Debug for Flujo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Flujo(…)")
    }
}

fn nonce(prefijo: &[u8; PREFIJO], n: u32, ultimo: bool) -> Nonce {
    let mut b = [0u8; 12];
    b[..PREFIJO].copy_from_slice(prefijo);
    b[PREFIJO..PREFIJO + 4].copy_from_slice(&n.to_be_bytes());
    b[11] = u8::from(ultimo);
    Nonce::assume_unique_for_key(b)
}

/// The additional data of every piece of file `archivo` in delivery `entrega` (the page builds
/// the same bytes: the domain, the delivery id as text, a zero byte, the file index as 4 bytes
/// big-endian).
#[must_use]
pub fn contexto(entrega: &str, archivo: u32) -> Vec<u8> {
    let mut v = DOMINIO.to_vec();
    v.extend_from_slice(entrega.as_bytes());
    v.push(0);
    v.extend_from_slice(&archivo.to_be_bytes());
    v
}

impl Flujo {
    /// The sealing of one file under `clave`, bound to `contexto` ([`contexto`]).
    pub fn new(clave: &Clave, contexto: Vec<u8>) -> Result<Self, Error> {
        let k =
            UnboundKey::new(&aead::AES_256_GCM, &clave.bytes).map_err(|_| Error::CannotStart)?;
        Ok(Self {
            clave: LessSafeKey::new(k),
            prefijo: clave.prefijo,
            aad: contexto,
        })
    }

    /// Seal piece `n`; `ultimo` only for the file's last piece.
    pub fn sella(&self, n: u32, ultimo: bool, plano: &[u8]) -> Result<Vec<u8>, Error> {
        let mut v = plano.to_vec();
        self.clave
            .seal_in_place_append_tag(
                nonce(&self.prefijo, n, ultimo),
                Aad::from(&self.aad),
                &mut v,
            )
            .map_err(|_| Error::CannotOpen)?;
        Ok(v)
    }

    /// Open piece `n`, said to be the last or not. A piece from another place, another file,
    /// another delivery, or changed by one bit, is [`Error::CannotOpen`].
    pub fn abre(&self, n: u32, ultimo: bool, sellado: &[u8]) -> Result<Vec<u8>, Error> {
        let mut v = sellado.to_vec();
        let largo = self
            .clave
            .open_in_place(
                nonce(&self.prefijo, n, ultimo),
                Aad::from(&self.aad),
                &mut v,
            )
            .map_err(|_| Error::CannotOpen)?
            .len();
        v.truncate(largo);
        Ok(v)
    }
}

/// What is known of a file once all of it went through: what the receipt says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Huella {
    /// Plaintext bytes.
    pub bytes: u64,
    /// Pieces.
    pub trozos: u64,
    /// SHA-256 of the whole file, hex.
    pub sha256: String,
    /// The root of its hash tree, hex.
    pub raiz: String,
    /// Every leaf, in order (kept by the sender to check a resume).
    pub hojas: Vec<[u8; 32]>,
}

/// A file going in, as the page hands its bytes over in pieces of any size: sealed pieces of
/// [`TROZO`] come out as soon as it is certain they are not the last.
pub struct Empaquetador {
    flujo: Flujo,
    pendiente: Vec<u8>,
    n: u32,
    hojas: Vec<[u8; 32]>,
    hash: Sha256,
    bytes: u64,
}

impl std::fmt::Debug for Empaquetador {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Empaquetador")
            .field("n", &self.n)
            .field("bytes", &self.bytes)
            .finish()
    }
}

impl Empaquetador {
    /// A new file under `clave`, bound to `contexto`.
    pub fn new(clave: &Clave, contexto: Vec<u8>) -> Result<Self, Error> {
        Ok(Self {
            flujo: Flujo::new(clave, contexto)?,
            pendiente: Vec::with_capacity(TROZO + 65536),
            n: 0,
            hojas: Vec::new(),
            hash: Sha256::new(),
            bytes: 0,
        })
    }

    fn sella_uno(&mut self, plano: &[u8], ultimo: bool) -> Result<Vec<u8>, Error> {
        if u64::from(self.n) >= MAX_TROZOS {
            return Err(Error::CannotOpen);
        }
        self.hojas.push(arbol::hoja(plano));
        let s = self.flujo.sella(self.n, ultimo, plano)?;
        self.n += 1;
        Ok(s)
    }

    /// More bytes of the file; the sealed pieces that are complete and not the last, in order.
    pub fn empuja(&mut self, datos: &[u8]) -> Result<Vec<Vec<u8>>, Error> {
        self.hash.update(datos);
        self.bytes += datos.len() as u64;
        self.pendiente.extend_from_slice(datos);
        let mut salida = Vec::new();
        // Strictly more than one piece waiting: the first one cannot be the last.
        while self.pendiente.len() > TROZO {
            let resto = self.pendiente.split_off(TROZO);
            let trozo = std::mem::replace(&mut self.pendiente, resto);
            salida.push(self.sella_uno(&trozo, false)?);
        }
        Ok(salida)
    }

    /// The end of the file: its last sealed piece, and what the receipt will say.
    pub fn termina(mut self) -> Result<(Vec<u8>, Huella), Error> {
        let trozo = std::mem::take(&mut self.pendiente);
        let ultimo = self.sella_uno(&trozo, true)?;
        let sha = self.hash.finalize();
        let huella = Huella {
            bytes: self.bytes,
            trozos: u64::from(self.n),
            sha256: sha.iter().map(|b| format!("{b:02x}")).collect(),
            raiz: arbol::hex(&arbol::raiz(&self.hojas)),
            hojas: self.hojas,
        };
        Ok((ultimo, huella))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn empaqueta(datos: &[u8], trozos_de: usize) -> (Clave, Vec<Vec<u8>>, Huella) {
        let c = Clave::nueva().unwrap();
        let mut e = Empaquetador::new(&c, contexto("e1", 0)).unwrap();
        let mut sellados = Vec::new();
        for parte in datos.chunks(trozos_de.max(1)) {
            sellados.extend(e.empuja(parte).unwrap());
        }
        let (ultimo, h) = e.termina().unwrap();
        sellados.push(ultimo);
        (c, sellados, h)
    }

    fn abre_todo(c: &Clave, ctx: Vec<u8>, sellados: &[Vec<u8>]) -> Result<Vec<u8>, Error> {
        let f = Flujo::new(c, ctx)?;
        let mut o = Vec::new();
        for (i, s) in sellados.iter().enumerate() {
            o.extend(f.abre(i as u32, i + 1 == sellados.len(), s)?);
        }
        Ok(o)
    }

    #[test]
    fn a_file_goes_through_in_pieces_and_comes_back_whole() {
        let datos: Vec<u8> = (0..(TROZO * 2 + 12_345)).map(|i| (i % 251) as u8).collect();
        let (c, sellados, h) = empaqueta(&datos, 65_536);
        assert_eq!(sellados.len(), 3);
        assert_eq!(h.trozos, 3);
        assert_eq!(trozos(h.bytes), 3);
        assert_eq!(h.bytes, datos.len() as u64);
        let sha: String = Sha256::digest(&datos)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(h.sha256, sha);
        let hojas: Vec<[u8; 32]> = datos.chunks(TROZO).map(arbol::hoja).collect();
        assert_eq!(h.raiz, arbol::hex(&arbol::raiz(&hojas)));
        assert_eq!(abre_todo(&c, contexto("e1", 0), &sellados).unwrap(), datos);
    }

    #[test]
    fn an_exact_multiple_and_an_empty_file_end_with_a_last_piece() {
        let (c, s, h) = empaqueta(&vec![7u8; TROZO], 1000);
        assert_eq!((s.len(), h.trozos), (1, 1));
        assert_eq!(abre_todo(&c, contexto("e1", 0), &s).unwrap().len(), TROZO);
        let (c, s, h) = empaqueta(&[], 1);
        assert_eq!((s.len(), h.trozos, h.bytes), (1, 1, 0));
        assert!(abre_todo(&c, contexto("e1", 0), &s).unwrap().is_empty());
    }

    #[test]
    fn cut_reordered_changed_or_foreign_pieces_do_not_open() {
        let datos = vec![3u8; TROZO * 3 + 10];
        let (c, s, _) = empaqueta(&datos, TROZO);
        // Cut after the second piece: the second was not sealed as the last one.
        assert!(abre_todo(&c, contexto("e1", 0), &s[..2]).is_err());
        // Two pieces swapped.
        let mut cambiado = s.clone();
        cambiado.swap(0, 1);
        assert!(abre_todo(&c, contexto("e1", 0), &cambiado).is_err());
        // One bit.
        let mut tocado = s.clone();
        tocado[1][5] ^= 1;
        assert!(abre_todo(&c, contexto("e1", 0), &tocado).is_err());
        // Another file of the same delivery, another delivery, another key.
        assert!(abre_todo(&c, contexto("e1", 1), &s).is_err());
        assert!(abre_todo(&c, contexto("e2", 0), &s).is_err());
        assert!(abre_todo(&Clave::nueva().unwrap(), contexto("e1", 0), &s).is_err());
        // Nothing of the plaintext shows in the pieces.
        assert!(!s[0].windows(64).any(|w| w == [3u8; 64]));
    }
}
