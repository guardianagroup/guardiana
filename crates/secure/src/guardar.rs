//! Save an identity, its sessions, and the device list so they are still there
//! after a restart.
//!
//! The private state is a vodozemac pickle under a random 32-byte key. That key
//! is not written as itself: [`Envolver`] wraps it. Windows uses DPAPI. macOS
//! uses the Keychain, on this device only, not iCloud. Linux has no wrapper yet.
//! The message bytes on the wire do not change with the wrapper.
//!
//! A failed read returns [`Error::CannotRestore`]. It does not create another
//! identity that could be mistaken for the one that failed to open.
//!
//! ## Keychain limits
//!
//! The item is `AccessibleAfterFirstUnlockThisDeviceOnly` and is marked not
//! synchronizable, so it is not copied to iCloud and does not move with a
//! backup onto another Mac. Before the first unlock after boot, the read fails.
//! This crate does not sign or notarize the macOS app. A shipped app will need
//! that, and a keychain access group, or macOS can refuse the item. Until then
//! the Keychain path is for a developer or CI session whose login keychain is
//! unlocked. A copied `identidad.gsec` without that item does not open, and it
//! does not become a new device by itself.

use std::path::Path;

use vodozemac::{Curve25519PublicKey, Ed25519PublicKey};

use crate::verificar::{Trust, Verified};
use crate::{Conversation, Error, Identity};

const MAGIC: &[u8] = b"GSEC2";
const MAX_WRAP: usize = 4096;
const MAX_PICKLE: usize = 1024 * 1024;
const MAX_DEVICES: usize = 100;
const MAX_SESSIONS: usize = 100;

/// Wraps the 32-byte key that encrypts the identity and the sessions.
pub trait Envolver {
    /// Wrap `clave`. The result must not be the raw 32 bytes.
    fn envolver(&self, clave: &[u8; 32]) -> Result<Vec<u8>, Error>;
    /// Unwrap a blob from [`Envolver::envolver`].
    fn desenvolver(&self, blob: &[u8]) -> Result<[u8; 32], Error>;
    /// Drop a blob that a newer save replaced. The default keeps it.
    fn olvidar(&self, _blob: &[u8]) -> Result<(), Error> {
        Ok(())
    }
}

/// Write `identidad.gsec` in `dir`. `sessions` are ratchets already open with
/// [`Conversation`]; their plaintexts are not written.
pub fn guardar(
    dir: &Path,
    identity: &Identity,
    trust: &Trust,
    sessions: &[Conversation],
    envolver: &impl Envolver,
) -> Result<(), Error> {
    if trust.peers().len() > MAX_DEVICES
        || trust.revoked_keys().len() > MAX_DEVICES
        || sessions.len() > MAX_SESSIONS
    {
        return Err(Error::CannotRestore);
    }
    let anterior = blob_anterior(dir);
    let mut clave = [0u8; 32];
    getrandom::fill(&mut clave).map_err(|_| Error::CannotRestore)?;
    let wrapped = envolver.envolver(&clave)?;
    let pickle = identity.lock(&clave);
    let mut sealed = Vec::with_capacity(sessions.len());
    for session in sessions {
        let secret = session.seal(&clave);
        if secret.len() > MAX_PICKLE {
            let _ = envolver.olvidar(&wrapped);
            return Err(Error::CannotRestore);
        }
        sealed.push((session.peer(), secret));
    }
    if wrapped.len() > MAX_WRAP || pickle.len() > MAX_PICKLE {
        let _ = envolver.olvidar(&wrapped);
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
    push_u32(
        &mut out,
        u32::try_from(sealed.len()).map_err(|_| Error::CannotRestore)?,
    );
    for (peer, secret) in &sealed {
        out.extend_from_slice(peer.as_bytes());
        push_bytes(&mut out, secret.as_bytes());
    }
    if out.windows(32).any(|window| window == clave) {
        let _ = envolver.olvidar(&wrapped);
        return Err(Error::CannotRestore);
    }
    if let Err(error) = write_file(dir, &out) {
        let _ = envolver.olvidar(&wrapped);
        return Err(error);
    }
    if let Some(old) = anterior {
        if old != wrapped {
            let _ = envolver.olvidar(&old);
        }
    }
    Ok(())
}

/// Read what [`guardar`] wrote. A missing file, a damaged file, or a key the
/// wrapper cannot open is an error. No identity is created here.
pub fn abrir(
    dir: &Path,
    envolver: &impl Envolver,
) -> Result<(Identity, Trust, Vec<Conversation>), Error> {
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
        let curve =
            Curve25519PublicKey::from_slice(&rest[..32]).map_err(|_| Error::CannotRestore)?;
        let mut signing_bytes = [0u8; 32];
        signing_bytes.copy_from_slice(&rest[32..64]);
        let signing =
            Ed25519PublicKey::from_slice(&signing_bytes).map_err(|_| Error::CannotRestore)?;
        peers.push(Verified::from_public_keys(curve, signing));
        rest = &rest[64..];
    }
    let n_revoked = take_u32(&mut rest)?;
    if n_revoked as usize > MAX_DEVICES || rest.len() < n_revoked as usize * 32 {
        return Err(Error::CannotRestore);
    }
    let mut revoked = Vec::with_capacity(n_revoked as usize);
    for _ in 0..n_revoked {
        let mut signing_bytes = [0u8; 32];
        signing_bytes.copy_from_slice(&rest[..32]);
        revoked
            .push(Ed25519PublicKey::from_slice(&signing_bytes).map_err(|_| Error::CannotRestore)?);
        rest = &rest[32..];
    }
    let n_sessions = take_u32(&mut rest)?;
    if n_sessions as usize > MAX_SESSIONS {
        return Err(Error::CannotRestore);
    }
    let mut sessions = Vec::with_capacity(n_sessions as usize);
    for _ in 0..n_sessions {
        if rest.len() < 32 {
            return Err(Error::CannotRestore);
        }
        let peer =
            Curve25519PublicKey::from_slice(&rest[..32]).map_err(|_| Error::CannotRestore)?;
        rest = &rest[32..];
        let secret = take_bytes(&mut rest)?;
        let secret = std::str::from_utf8(secret).map_err(|_| Error::CannotRestore)?;
        sessions.push(Conversation::unseal(secret, &clave, peer)?);
    }
    if !rest.is_empty() {
        return Err(Error::CannotRestore);
    }
    Ok((identity, Trust::restore(peers, revoked), sessions))
}

fn blob_anterior(dir: &Path) -> Option<Vec<u8>> {
    let bytes = std::fs::read(dir.join("identidad.gsec")).ok()?;
    if bytes.len() < MAGIC.len() || &bytes[..MAGIC.len()] != MAGIC {
        return None;
    }
    let mut rest = &bytes[MAGIC.len()..];
    take_bytes(&mut rest).ok().map(|blob| blob.to_vec())
}

fn write_file(dir: &Path, bytes: &[u8]) -> Result<(), Error> {
    std::fs::create_dir_all(dir).map_err(|_| Error::CannotRestore)?;
    let tmp = dir.join("identidad.gsec.tmp");
    std::fs::write(&tmp, bytes).map_err(|_| Error::CannotRestore)?;
    std::fs::rename(&tmp, dir.join("identidad.gsec")).map_err(|_| Error::CannotRestore)?;
    Ok(())
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    push_u32(out, len);
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

#[cfg(target_os = "macos")]
mod llavero;

#[cfg(target_os = "macos")]
pub use llavero::Llavero;

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

/// A wrapper that returns the wrong key. It must not be treated as the device.
#[cfg(test)]
struct Ajena;

#[cfg(test)]
impl Envolver for Ajena {
    fn envolver(&self, _clave: &[u8; 32]) -> Result<Vec<u8>, Error> {
        Err(Error::CannotRestore)
    }

    fn desenvolver(&self, _blob: &[u8]) -> Result<[u8; 32], Error> {
        Ok([3u8; 32])
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::verificar::Trust;

    fn dir() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "guardiana-secure-disco-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&path);
        path
    }

    #[test]
    fn a_restart_keeps_the_identity_the_revocation_and_the_session() {
        let dir = dir();
        let alice = Identity::new();
        let mut bob = Identity::new();
        let one_time = bob.fresh_one_time_key().unwrap();
        let mut alice_side = alice.start(bob.public_key(), one_time).unwrap();
        let envelope = alice_side.send(bob.public_key(), b"hola").unwrap();
        let (bob_side, opened) = bob.receive_session(alice.public_key(), &envelope).unwrap();
        assert_eq!(opened, b"hola");

        let mut trust = Trust::new();
        let other = Identity::new();
        trust.insert(Verified::from_public_keys(
            other.public_key(),
            other.signing_key(),
        ));
        trust.revoke(other.signing_key());
        guardar(&dir, &alice, &trust, &[alice_side, bob_side], &Reverso).unwrap();
        let file = std::fs::read(dir.join("identidad.gsec")).unwrap();
        assert!(!file.windows(b"hola".len()).any(|window| window == b"hola"));

        let (opened, loaded, mut sessions) = abrir(&dir, &Reverso).unwrap();
        assert_eq!(opened.public_key(), alice.public_key());
        assert_eq!(opened.signing_key(), alice.signing_key());
        assert!(loaded.is_revoked(other.signing_key()));
        assert!(!loaded.allows(other.public_key()));
        assert_eq!(sessions.len(), 2);
        let reply = sessions[1].send(alice.public_key(), b"sigue").unwrap();
        assert_eq!(sessions[0].open(&reply).unwrap(), b"sigue");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_damaged_or_foreign_store_does_not_invent_an_identity() {
        let dir = dir();
        let identity = Identity::new();
        let kept = identity.public_key();
        guardar(&dir, &identity, &Trust::new(), &[], &Reverso).unwrap();

        assert!(abrir(&dir.join("no-esta"), &Reverso).is_err());
        let mut damaged = std::fs::read(dir.join("identidad.gsec")).unwrap();
        damaged[0] ^= 0xff;
        std::fs::write(dir.join("identidad.gsec"), &damaged).unwrap();
        assert!(abrir(&dir, &Reverso).is_err());

        std::fs::write(dir.join("identidad.gsec"), b"GSEC2").unwrap();
        assert!(abrir(&dir, &Reverso).is_err());

        guardar(&dir, &identity, &Trust::new(), &[], &Reverso).unwrap();
        assert!(abrir(&dir, &Ajena).is_err());
        assert_eq!(identity.public_key(), kept);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
