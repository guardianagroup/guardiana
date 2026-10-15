//! The macOS Keychain, for the 32-byte key only.
//!
//! The item is created in the data-protection keychain, marked not
//! synchronizable, and `AccessibleAfterFirstUnlockThisDeviceOnly`. It is not
//! an iCloud password and it does not travel to another Mac. This file does
//! not sign or notarize the application. A distributed build still needs that,
//! plus a keychain access group, or macOS can refuse the item. A missing item
//! is an error: this module never invents a replacement key.

use security_framework::access_control::{ProtectionMode, SecAccessControl};
use security_framework::passwords::{
    delete_generic_password_options, generic_password, set_generic_password_options,
    PasswordOptions,
};

use super::Envolver;
use crate::Error;

const SERVICE: &str = "guardiana-secure";
const PREFIX: &str = "gsec-";

/// Keychain wrapper. One item per save, named by the blob, not by the key.
pub struct Llavero;

impl Envolver for Llavero {
    fn envolver(&self, clave: &[u8; 32]) -> Result<Vec<u8>, Error> {
        let mut id = [0u8; 16];
        getrandom::fill(&mut id).map_err(|_| Error::CannotRestore)?;
        let account = format!("{PREFIX}{}", hex(&id));
        let options = opciones_nuevas(&account)?;
        set_generic_password_options(clave, options).map_err(|_| Error::CannotRestore)?;
        Ok(account.into_bytes())
    }

    fn desenvolver(&self, blob: &[u8]) -> Result<[u8; 32], Error> {
        let account = cuenta(blob)?;
        let options = opciones_lectura(&account);
        let bytes = generic_password(options).map_err(|_| Error::CannotRestore)?;
        if bytes.len() != 32 {
            return Err(Error::CannotRestore);
        }
        let mut clave = [0u8; 32];
        clave.copy_from_slice(&bytes);
        Ok(clave)
    }

    fn olvidar(&self, blob: &[u8]) -> Result<(), Error> {
        let account = cuenta(blob)?;
        delete_generic_password_options(opciones_lectura(&account))
            .map_err(|_| Error::CannotRestore)
    }
}

fn opciones_lectura(account: &str) -> PasswordOptions {
    let mut options = PasswordOptions::new_generic_password(SERVICE, account);
    options.set_access_synchronized(Some(false));
    options.use_protected_keychain();
    options
}

fn opciones_nuevas(account: &str) -> Result<PasswordOptions, Error> {
    let mut options = opciones_lectura(account);
    let access = SecAccessControl::create_with_protection(
        Some(ProtectionMode::AccessibleAfterFirstUnlockThisDeviceOnly),
        0,
    )
    .map_err(|_| Error::CannotRestore)?;
    options.set_access_control(access);
    options.set_label("GUARDIANA SECURE");
    Ok(options)
}

fn cuenta(blob: &[u8]) -> Result<String, Error> {
    let account = std::str::from_utf8(blob).map_err(|_| Error::CannotRestore)?;
    let rest = account.strip_prefix(PREFIX).ok_or(Error::CannotRestore)?;
    if rest.len() != 32 || !rest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(Error::CannotRestore);
    }
    Ok(account.to_string())
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::guardar::{abrir, guardar};
    use crate::verificar::Trust;
    use crate::Identity;

    #[test]
    fn the_keychain_keeps_the_key_and_a_missing_item_is_not_a_new_one() {
        let clave = [11u8; 32];
        let blob = Llavero.envolver(&clave).unwrap();
        assert!(!blob.windows(32).any(|window| window == clave));
        assert_eq!(Llavero.desenvolver(&blob).unwrap(), clave);
        let missing = b"gsec-0123456789abcdef0123456789abcdef";
        assert!(Llavero.desenvolver(missing).is_err());
        Llavero.olvidar(&blob).unwrap();
        assert!(Llavero.desenvolver(&blob).is_err());
    }

    #[test]
    fn a_restart_on_this_mac_keeps_the_identity_and_the_revocation() {
        let dir =
            std::env::temp_dir().join(format!("guardiana-secure-llavero-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let identity = Identity::new();
        let other = Identity::new();
        let mut trust = Trust::new();
        trust.insert(crate::verificar::Verified::from_public_keys(
            other.public_key(),
            other.signing_key(),
        ));
        trust.revoke(other.signing_key());
        guardar(&dir, &identity, &trust, &[], &Llavero).unwrap();
        let (opened, loaded, sessions) = abrir(&dir, &Llavero).unwrap();
        assert_eq!(opened.public_key(), identity.public_key());
        assert_eq!(opened.signing_key(), identity.signing_key());
        assert!(loaded.is_revoked(other.signing_key()));
        assert!(!loaded.allows(other.public_key()));
        assert!(sessions.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
