//! The anchor's registry key read with Windows' own functions instead of a `reg query` (1.0.55, «ligero»; the owner,
//! 2 Nov 2026, 16:53: «se me traba»).
//!
//! `license::status` reads the anchor each time it is asked, and on Windows that was one `reg.exe` started each time:
//! by the resolver every thirty seconds, while the query that asked waited for it, and twice a minute by the service.
//! What is built here is the text `reg query` prints for the key's values, one line per value, in the order the
//! registry keeps them, so the same reading (`de_salida_reg`) takes the same values from it: a value of another type
//! than the program writes reads as `reg` prints it, and so does a name in other letters.
//!
//! `unsafe` is denied in this project unless justified, and this is the justification: the registry is a C API with no
//! safe wrapper, and the other way to read it is a process. Read only; every buffer goes with its size and is cut to
//! what Windows says it wrote; the key is closed when it goes out of scope.

#![allow(unsafe_code)]

use windows_sys::Win32::Foundation::{ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS};
#[cfg(test)]
pub(crate) use windows_sys::Win32::System::Registry::HKEY_CURRENT_USER;
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegEnumValueW, RegOpenKeyExW, KEY_READ, REG_BINARY, REG_DWORD,
    REG_DWORD_BIG_ENDIAN, REG_EXPAND_SZ, REG_FULL_RESOURCE_DESCRIPTOR, REG_LINK, REG_MULTI_SZ,
    REG_NONE, REG_QWORD, REG_RESOURCE_LIST, REG_RESOURCE_REQUIREMENTS_LIST, REG_SZ,
};
pub(crate) use windows_sys::Win32::System::Registry::{HKEY, HKEY_LOCAL_MACHINE};

/// What reading the key gave.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Lectura {
    /// Its values, as `reg query` prints them.
    Texto(String),
    /// The key cannot be opened here (it is not there, or this account may not read it): `reg query` fails then too.
    SinClave,
    /// A value could not be read whole: the caller asks `reg` itself.
    Fallo(u32),
}

struct Clave(HKEY);

impl Drop for Clave {
    fn drop(&mut self) {
        // SAFETY: the handle was opened in `como_reg_query` and is closed once, here.
        unsafe {
            RegCloseKey(self.0);
        }
    }
}

/// The type's name as `reg` writes it.
fn nombre_de_tipo(tipo: u32) -> &'static str {
    match tipo {
        REG_NONE => "REG_NONE",
        REG_SZ => "REG_SZ",
        REG_EXPAND_SZ => "REG_EXPAND_SZ",
        REG_BINARY => "REG_BINARY",
        REG_DWORD => "REG_DWORD",
        REG_DWORD_BIG_ENDIAN => "REG_DWORD_BIG_ENDIAN",
        REG_LINK => "REG_LINK",
        REG_MULTI_SZ => "REG_MULTI_SZ",
        REG_RESOURCE_LIST => "REG_RESOURCE_LIST",
        REG_FULL_RESOURCE_DESCRIPTOR => "REG_FULL_RESOURCE_DESCRIPTOR",
        REG_RESOURCE_REQUIREMENTS_LIST => "REG_RESOURCE_REQUIREMENTS_LIST",
        REG_QWORD => "REG_QWORD",
        _ => "REG_UNKNOWN",
    }
}

fn unidades(datos: &[u8]) -> Vec<u16> {
    datos
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect()
}

/// The data as `reg` writes it: a text up to its end, a list of texts joined by `\0`, a number in hexadecimal
/// (`0x1f`), anything else its bytes in hexadecimal capitals.
pub(crate) fn dato_como_reg(tipo: u32, datos: &[u8]) -> String {
    match tipo {
        REG_SZ | REG_EXPAND_SZ => {
            let u = unidades(datos);
            let fin = u.iter().position(|&c| c == 0).unwrap_or(u.len());
            String::from_utf16_lossy(&u[..fin])
        }
        REG_MULTI_SZ => unidades(datos)
            .split(|&c| c == 0)
            .filter(|p| !p.is_empty())
            .map(String::from_utf16_lossy)
            .collect::<Vec<_>>()
            .join("\\0"),
        REG_DWORD if datos.len() == 4 => {
            let v = u32::from_le_bytes([datos[0], datos[1], datos[2], datos[3]]);
            format!("0x{v:x}")
        }
        REG_QWORD if datos.len() == 8 => {
            let mut b = [0u8; 8];
            b.copy_from_slice(datos);
            format!("0x{:x}", u64::from_le_bytes(b))
        }
        _ => datos.iter().map(|b| format!("{b:02X}")).collect(),
    }
}

/// What `reg query <raiz>\<ruta>` prints for the key's values: `    name    REG_TYPE    data`, one line each.
pub(crate) fn como_reg_query(raiz: HKEY, ruta: &str) -> Lectura {
    let r: Vec<u16> = ruta.encode_utf16().chain(std::iter::once(0)).collect();
    let mut h: HKEY = std::ptr::null_mut();
    // SAFETY: `r` is a NUL-terminated wide string that outlives the call; `h` is ours. The registry view is the
    // process's own, as for the `reg` it would start.
    let e = unsafe { RegOpenKeyExW(raiz, r.as_ptr(), 0, KEY_READ, &raw mut h) };
    if e != ERROR_SUCCESS || h.is_null() {
        return Lectura::SinClave;
    }
    let clave = Clave(h);
    let mut texto = String::new();
    let mut datos: Vec<u8> = vec![0; 4_096];
    let mut i: u32 = 0;
    let mut otra_vez = 0;
    while i < 16_384 {
        // A value's name is at most 16 383 characters.
        let mut nombre = vec![0u16; 16_384];
        let mut largo_nombre = u32::try_from(nombre.len()).unwrap_or(0);
        let mut tipo: u32 = 0;
        let mut largo_datos = u32::try_from(datos.len()).unwrap_or(0);
        // SAFETY: both buffers are ours and their sizes go with them (characters for the name, bytes for the data);
        // Windows writes at most that and says how much it wrote.
        let e = unsafe {
            RegEnumValueW(
                clave.0,
                i,
                nombre.as_mut_ptr(),
                &raw mut largo_nombre,
                std::ptr::null(),
                &raw mut tipo,
                datos.as_mut_ptr(),
                &raw mut largo_datos,
            )
        };
        if e == ERROR_NO_MORE_ITEMS {
            break;
        }
        if e == ERROR_MORE_DATA
            && largo_datos as usize > datos.len()
            && largo_datos <= 1 << 24
            && otra_vez < 8
        {
            // The data did not fit: the same value again with room for it.
            datos.resize(largo_datos as usize, 0);
            otra_vez += 1;
            continue;
        }
        if e != ERROR_SUCCESS {
            return Lectura::Fallo(e);
        }
        let n = (largo_nombre as usize).min(nombre.len());
        let d = (largo_datos as usize).min(datos.len());
        let mut nombre = String::from_utf16_lossy(&nombre[..n]);
        if nombre.is_empty() {
            // The key's default value has no name; `reg` gives it one (in the system's language: never one of ours).
            nombre = "(Default)".to_owned();
        }
        texto.push_str(&format!(
            "    {nombre}    {}    {}\n",
            nombre_de_tipo(tipo),
            dato_como_reg(tipo, &datos[..d])
        ));
        otra_vez = 0;
        i += 1;
    }
    Lectura::Texto(texto)
}
