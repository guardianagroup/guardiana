//! DPAPI for the Windows user who is logged in. `CRYPTPROTECT_UI_FORBIDDEN`
//! so a service cannot pop a dialog. Not `CRYPTPROTECT_LOCAL_MACHINE`: another
//! user on the same PC must not unwrap this.

use std::ptr;

use windows_sys::Win32::Foundation::{LocalFree, BOOL};
use windows_sys::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};

use super::Envolver;
use crate::Error;

/// Current-user DPAPI. The DNS service does not construct this.
pub struct Dpapi;

impl Envolver for Dpapi {
    fn envolver(&self, clave: &[u8; 32]) -> Result<Vec<u8>, Error> {
        proteger(clave, true)
    }

    fn desenvolver(&self, blob: &[u8]) -> Result<[u8; 32], Error> {
        let claro = proteger(blob, false)?;
        if claro.len() != 32 {
            return Err(Error::CannotRestore);
        }
        let mut clave = [0u8; 32];
        clave.copy_from_slice(&claro);
        Ok(clave)
    }
}

/// `cifrar` true calls `CryptProtectData`. False calls `CryptUnprotectData`.
///
/// Safety of the `unsafe` block: both functions are the Windows C API. The
/// input pointer lasts for the call and points at `datos`. Windows allocates
/// the output; this function copies it and `LocalFree`s that pointer before
/// returning. No other code keeps the pointer.
#[allow(unsafe_code)]
fn proteger(datos: &[u8], cifrar: bool) -> Result<Vec<u8>, Error> {
    let len = u32::try_from(datos.len()).map_err(|_| Error::CannotRestore)?;
    let mut entrada = CRYPT_INTEGER_BLOB {
        cbData: len,
        pbData: datos.as_ptr().cast_mut(),
    };
    let mut salida = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };
    let ok: BOOL = unsafe {
        if cifrar {
            CryptProtectData(
                &entrada,
                ptr::null(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut salida,
            )
        } else {
            CryptUnprotectData(
                &entrada,
                ptr::null_mut(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut salida,
            )
        }
    };
    if ok == 0 || salida.pbData.is_null() || salida.cbData == 0 {
        return Err(Error::CannotRestore);
    }
    let copia =
        unsafe { std::slice::from_raw_parts(salida.pbData, salida.cbData as usize).to_vec() };
    unsafe {
        LocalFree(salida.pbData.cast());
    }
    let _ = entrada;
    Ok(copia)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn dpapi_roundtrip_does_not_store_the_key_in_the_blob() {
        let clave = [7u8; 32];
        let blob = Dpapi.envolver(&clave).unwrap();
        assert!(!blob.windows(32).any(|window| window == clave));
        assert_eq!(Dpapi.desenvolver(&blob).unwrap(), clave);
    }
}
