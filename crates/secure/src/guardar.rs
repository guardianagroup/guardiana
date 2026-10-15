//! Save an identity and the device list so they are still there after a restart.
//!
//! The identity pickle is vodozemac's, under a random 32-byte key. That key is
//! not written as itself: [`Envolver`] wraps it. On Windows the wrapper is DPAPI
//! for the current user. The Olm session ratchet is not stored. After a restart
//! the same identity can open a new session. This file is not a conversation
//! transcript and it does not go in the DNS service.

use std::path::Path;

use vodozemac::Ed25519PublicKey;

use crate::verificar::{Trust, Verified};
use crate::{Error, Identity};

const MAGIC: &[u8] = b"GSEC1";
const MAX_WRAP: usize = 4096;
const MAX_PICKLE: usize = 1024 * 1024;
const MAX_DEVICES: usize = 100;

/// Wraps the 32-byte key that encrypts the identity pickle.
pub trait Envolver {
    /// Wrap `clave`. The result must not be the raw 32 bytes.
    fn envolver(&self, clave: &[u8; 32]) -> Result<Vec<u8>, Error>;
    /// Unwrap a blob from [`Envolver::envolver`].
    fn desenvolver(&self, blob: &[u8]) -> Result<[u8; 32], Error>;
}

/// Write `identidad.gsec` in `dir`.
pub fn guardar(
    dir: &Path,
    identity: &Identity,
    trust: &Trust,
    envolver: &impl Envolver,
) -> Result<(), Error> {
    let mut clave = [0u8; 32];
    getrandom::fill(&mut clave).map_err(|_| Error::CannotRestore)?;
    let wrapped = envolver.envolver(&clave)?;
    let pickle = identity.lock(&clave);
    if wrapped.len() > MAX_WRAP || pickle.len() > MAX_PICKLE {
        return Err(Error::CannotRestore);
    }
    if trust.peers().len() > MAX_DEVICES || trust.revoked_keys().len() > MAX_DEVICES {
        return Err(Error::CannotRestore);
    }
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    push_bytes(&mut out, &wrapped);
    push_bytes(&mut out, pickle.as_bytes());
    push_u32(
        &mut out,
        u32::try_from(trust.peers().len()).map_err(|_| Error::CannotRestore)?,
    );
    for peer in trust.peers() {
        out.extend_from_slice(peer.curve().as_bytes());
        out.extend_from_slice(peer.signing().as_bytes());
    }
    push_u32(
        &mut out,
        u32::try_from(trust.revoked_keys().len()).map_err(|_| Error::CannotRestore)?,
    );
    for key in trust.revoked_keys() {
        out.extend_from_slice(key.as_bytes());
    }
    if out.windows(32).any(|window| window == clave) {
        return Err(Error::CannotRestore);
    }
    std::fs::create_dir_all(dir).map_err(|_| Error::CannotRestore)?;
    std::fs::write(dir.join("identidad.gsec"), out).map_err(|_| Error::CannotRestore)?;
    Ok(())
}

/// Read what [`guardar`] wrote, with the same wrapper.
pub fn abrir(dir: &Path, envolver: &impl Envolver) -> Result<(Identity, Trust), Error> {
    let bytes = std::fs::read(dir.join("identidad.gsec")).map_err(|_| Error::CannotRestore)?;
    if bytes.len() < MAGIC.len() || &bytes[..MAGIC.len()] != MAGIC {
        return Err(Error::CannotRestore);
    }
    let mut rest = &bytes[MAGIC.len()..];
    let wrapped = take_bytes(&mut rest)?;
    let pickle = take_bytes(&mut rest)?;
    let clave = envolver.desenvolver(wrapped)?;
    let pickle = std::str::from_utf8(pickle).map_err(|_| Error::CannotRestore)?;
    let identity = Identity::unlock(pickle, &clave)?;
    let n_peers = take_u32(&mut rest)?;
    if n_peers as usize > MAX_DEVICES {
        return Err(Error::CannotRestore);
    }
    let mut peers = Vec::with_capacity(n_peers as usize);
    for _ in 0..n_peers {
        if rest.len() < 64 {
            return Err(Error::CannotRestore);
        }
        let curve = vodozemac::Curve25519PublicKey::from_slice(&rest[..32])
            .map_err(|_| Error::CannotRestore)?;
        let mut signing_bytes = [0u8; 32];
        signing_bytes.copy_from_slice(&rest[32..64]);
        let signing =
            Ed25519PublicKey::from_slice(&signing_bytes).map_err(|_| Error::CannotRestore)?;
        peers.push(Verified::from_public_keys(curve, signing));
        rest = &rest[64..];
    }
    let n_revoked = take_u32(&mut rest)?;
    if n_revoked as usize > MAX_DEVICES || rest.len() != n_revoked as usize * 32 {
        return Err(Error::CannotRestore);
    }
    let mut revoked = Vec::with_capacity(n_revoked as usize);
    for chunk in rest.chunks(32) {
        let mut signing_bytes = [0u8; 32];
        signing_bytes.copy_from_slice(chunk);
        revoked
            .push(Ed25519PublicKey::from_slice(&signing_bytes).map_err(|_| Error::CannotRestore)?);
    }
    Ok((identity, Trust::restore(peers, revoked)))
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    push_u32(out, u32::try_from(bytes.len()).unwrap_or(u32::MAX));
    out.extend_from_slice(bytes);
}

fn take_u32(rest: &mut &[u8]) -> Result<u32, Error> {
    if rest.len() < 4 {
        return Err(Error::CannotRestore);
    }
    let mut buf = [0u8; 4];
    buf.copy_from_slice(&rest[..4]);
    *rest = &rest[4..];
    Ok(u32::from_le_bytes(buf))
}

fn take_bytes<'a>(rest: &mut &'a [u8]) -> Result<&'a [u8], Error> {
    let len = take_u32(rest)? as usize;
    if len > MAX_PICKLE || rest.len() < len {
        return Err(Error::CannotRestore);
    }
    let (head, tail) = rest.split_at(len);
    *rest = tail;
    Ok(head)
}

#[cfg(windows)]
mod dpapi;

#[cfg(windows)]
pub use dpapi::Dpapi;

/// Not a protector. Tests use it to show the file does not contain the raw key.
#[cfg(test)]
struct Reverso;

#[cfg(test)]
impl Envolver for Reverso {
    fn envolver(&self, clave: &[u8; 32]) -> Result<Vec<u8>, Error> {
        Ok(clave.iter().rev().copied().collect())
    }

    fn desenvolver(&self, blob: &[u8]) -> Result<[u8; 32], Error> {
        if blob.len() != 32 {
            return Err(Error::CannotRestore);
        }
        let mut clave = [0u8; 32];
        for (slot, byte) in clave.iter_mut().zip(blob.iter().rev()) {
            *slot = *byte;
        }
        Ok(clave)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::verificar::Trust;

    #[test]
    fn a_restart_keeps_the_identity_and_the_revocation() {
        let dir =
            std::env::temp_dir().join(format!("guardiana-secure-disco-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let identity = Identity::new();
        let mut trust = Trust::new();
        let other = Identity::new();
        let verified = Verified::from_public_keys(other.public_key(), other.signing_key());
        trust.insert(verified);
        trust.revoke(other.signing_key());
        guardar(&dir, &identity, &trust, &Reverso).unwrap();
        let file = std::fs::read(dir.join("identidad.gsec")).unwrap();
        assert!(!file
            .windows(32)
            .any(|window| window == other.public_key().as_bytes()));
        let (opened, loaded) = abrir(&dir, &Reverso).unwrap();
        assert_eq!(opened.public_key(), identity.public_key());
        assert_eq!(opened.signing_key(), identity.signing_key());
        assert!(loaded.is_revoked(other.signing_key()));
        assert!(!loaded.allows(other.public_key()));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
