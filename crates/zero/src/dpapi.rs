//! Local secrets at rest under Windows' DPAPI, user scope (18 Oct 2026, the committee: «secretos
//! locales protegidos con DPAPI donde no lo estén ya»).
//!
//! The 32-byte key that locks SECURE's identity and its book, and the key that signs the
//! delivery receipts, used to lie in the person's folder as plain bytes: anyone who copied the
//! folder (a backup, another account, a stolen disk) had them. On Windows they are now sealed
//! with `CryptProtectData` for the person's own account: only that Windows account, on that
//! computer, can open them again, and nothing here holds a second copy. On Linux and macOS
//! there is no DPAPI; the file stays as before, readable by its owner only (0600), and this
//! module says so instead of pretending.
//!
//! A file written before keeps working: it is read as it was and written back sealed.

/// What a sealed file starts with, so an older plain file is told apart.
const MARCA: &[u8] = b"GDPAPI1\n";

/// Seal `datos` for this Windows account (on other systems: the bytes as they are).
#[must_use]
pub fn protege(datos: &[u8]) -> Vec<u8> {
    match sistema::protege(datos) {
        Some(sellado) => {
            let mut v = MARCA.to_vec();
            v.extend_from_slice(&sellado);
            v
        }
        None => datos.to_vec(),
    }
}

/// Open what [`protege`] wrote; a plain (older) file comes back as it is, with `true` meaning
/// «write it again sealed». `None`: sealed, but not openable by this account.
#[must_use]
pub fn abre(guardado: &[u8]) -> Option<(Vec<u8>, bool)> {
    match guardado.strip_prefix(MARCA) {
        Some(sellado) => sistema::abre(sellado).map(|d| (d, false)),
        None => Some((guardado.to_vec(), sistema::DISPONIBLE)),
    }
}

/// Whether this system seals at all (Windows).
#[must_use]
pub fn disponible() -> bool {
    sistema::DISPONIBLE
}

/// Read a secret file (sealing it if it was plain), or create it with `nuevo` and write it
/// sealed. On other systems the file is made readable by its owner only.
pub fn lee_o_crea(
    ruta: &std::path::Path,
    nuevo: impl FnOnce() -> Option<Vec<u8>>,
    valido: impl Fn(&[u8]) -> bool,
) -> Option<Vec<u8>> {
    if let Ok(b) = std::fs::read(ruta) {
        if let Some((d, reescribir)) = abre(&b) {
            if valido(&d) {
                if reescribir {
                    escribe(ruta, &d);
                }
                return Some(d);
            }
        }
    }
    let d = nuevo()?;
    escribe(ruta, &d);
    Some(d)
}

/// Write a secret sealed (or, without DPAPI, readable by its owner only).
pub fn escribe(ruta: &std::path::Path, datos: &[u8]) {
    let _ = std::fs::write(ruta, protege(datos));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = std::fs::set_permissions(ruta, std::fs::Permissions::from_mode(0o600));
    }
}

#[cfg(windows)]
mod sistema {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    pub const DISPONIBLE: bool = true;

    fn entrada(datos: &[u8]) -> Option<CRYPT_INTEGER_BLOB> {
        Some(CRYPT_INTEGER_BLOB {
            cbData: u32::try_from(datos.len()).ok()?,
            pbData: datos.as_ptr().cast_mut(),
        })
    }

    /// Copy the blob Windows allocated and give it back to Windows.
    // SAFETY: `blob` was filled by a successful CryptProtectData/CryptUnprotectData, so
    // `pbData` points at `cbData` bytes allocated with LocalAlloc, which LocalFree releases.
    #[allow(unsafe_code)]
    fn copia(blob: &CRYPT_INTEGER_BLOB) -> Vec<u8> {
        if blob.pbData.is_null() {
            return Vec::new();
        }
        let v = unsafe { std::slice::from_raw_parts(blob.pbData, blob.cbData as usize) }.to_vec();
        unsafe {
            LocalFree(blob.pbData.cast());
        }
        v
    }

    // SAFETY: the input blob points into `datos`, alive for the call and never written by
    // Windows; no description, entropy, reserved pointer or prompt is passed; the output blob is
    // ours to free (see `copia`).
    #[allow(unsafe_code)]
    pub fn protege(datos: &[u8]) -> Option<Vec<u8>> {
        let e = entrada(datos)?;
        let mut salida = CRYPT_INTEGER_BLOB::default();
        let ok = unsafe {
            CryptProtectData(
                &e,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut salida,
            )
        };
        (ok != 0).then(|| copia(&salida))
    }

    // SAFETY: as in `protege`; the description pointer is null, so Windows allocates none.
    #[allow(unsafe_code)]
    pub fn abre(sellado: &[u8]) -> Option<Vec<u8>> {
        let e = entrada(sellado)?;
        let mut salida = CRYPT_INTEGER_BLOB::default();
        let ok = unsafe {
            CryptUnprotectData(
                &e,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut salida,
            )
        };
        (ok != 0).then(|| copia(&salida))
    }
}

#[cfg(not(windows))]
mod sistema {
    pub const DISPONIBLE: bool = false;

    pub fn protege(_datos: &[u8]) -> Option<Vec<u8>> {
        None
    }

    pub fn abre(_sellado: &[u8]) -> Option<Vec<u8>> {
        None
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_comes_back_and_an_old_plain_file_is_sealed_where_it_can_be() {
        let d = std::env::temp_dir().join(format!("guardiana-dpapi-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&d);
        let ruta = d.join("clave.bin");
        let _ = std::fs::remove_file(&ruta);
        // An older plain key.
        std::fs::write(&ruta, [7u8; 32]).unwrap();
        let k = lee_o_crea(&ruta, || None, |b| b.len() == 32).unwrap();
        assert_eq!(k, vec![7u8; 32]);
        let en_disco = std::fs::read(&ruta).unwrap();
        if disponible() {
            assert!(en_disco.starts_with(MARCA), "sealed on Windows");
            assert!(!en_disco.windows(32).any(|w| w == [7u8; 32]));
        } else {
            assert_eq!(en_disco, vec![7u8; 32]);
        }
        // Read again: the same key, whatever the form on disk.
        assert_eq!(lee_o_crea(&ruta, || None, |b| b.len() == 32).unwrap(), k);
        // A new one when there is none (or it is not valid).
        std::fs::write(&ruta, b"corto").unwrap();
        let nueva = lee_o_crea(&ruta, || Some(vec![1u8; 32]), |b| b.len() == 32).unwrap();
        assert_eq!(nueva, vec![1u8; 32]);
        assert_eq!(abre(&protege(b"hola")).unwrap().0, b"hola");
        let _ = std::fs::remove_dir_all(&d);
    }
}
