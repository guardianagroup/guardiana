//! The VPN's adapter on Windows (in preparation; docs/VPN.md): WireGuardNT, WireGuard's own Windows driver, reached
//! only through the public interface of its signed `wireguard.dll` (`wireguard.h`), which ships next to the program
//! exactly as WireGuard LLC publishes it (its licence allows that and nothing more: see docs/VPN.md).
//!
//! The configuration it takes is written by `guardiana_tunel::nt`, tested on every system; this module only loads the
//! library, creates and closes the adapter, hands it the configuration and brings it up or down. The adapter
//! disappears when the last handle to it closes (and with this process).
//!
//! `unsafe` is used here, as the project allows with justification: the library is a C one, loaded at run time, and
//! there is no safe wrapper. Every pointer comes from the library itself or from buffers owned here for the call, every
//! result is checked, and the handle and the library are closed on drop.
#![allow(unsafe_code)]

use std::ffi::c_void;
use std::path::Path;
use std::sync::Arc;

use windows_sys::core::GUID;
use windows_sys::Win32::Foundation::{FreeLibrary, GetLastError, ERROR_MORE_DATA};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryExW};

type FnCrea = unsafe extern "system" fn(*const u16, *const u16, *const GUID) -> *mut c_void;
type FnCierra = unsafe extern "system" fn(*mut c_void);
type FnLuid = unsafe extern "system" fn(*mut c_void, *mut u64);
type FnConfigura = unsafe extern "system" fn(*mut c_void, *const u8, u32) -> i32;
type FnLee = unsafe extern "system" fn(*mut c_void, *mut u8, *mut u32) -> i32;
type FnEstado = unsafe extern "system" fn(*mut c_void, i32) -> i32;
type FnVersion = unsafe extern "system" fn() -> u32;

/// `WIREGUARD_ADAPTER_STATE_UP`.
const ARRIBA: i32 = 1;
/// `WIREGUARD_ADAPTER_STATE_DOWN`.
const ABAJO: i32 = 0;

/// Why the adapter could not be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorNt {
    /// The library could not be loaded, or lacks a function of its public interface.
    Biblioteca(String),
    /// A call of the library failed; the text is the call and Windows' error code.
    Llamada(String),
}

impl std::fmt::Display for ErrorNt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Biblioteca(e) => write!(f, "no se pudo cargar wireguard.dll: {e}"),
            Self::Llamada(e) => write!(f, "WireGuardNT no aceptó la orden: {e}"),
        }
    }
}

/// The SHA-256 of the `wireguard.dll` (amd64) of WireGuardNT 1.1 as WireGuard LLC publishes it, seen on 1 Nov 2026
/// (docs/VPN.md; `.github/workflows/vpn.yml` and the installer check the same one, with its Authenticode signature).
/// The library is loaded only when the file next to the program is exactly that one: another build, or a file someone
/// put in its place, is refused before anything runs.
pub const HUELLA_DLL: &str = "b1b85e072c45d81358be29d94c599dc76652f912be8c0f0a41e2d5d89a6461d3";

/// The file's SHA-256, in lowercase hex.
///
/// # Errors
/// [`ErrorNt::Biblioteca`] when it cannot be read.
pub fn huella_de(ruta: &Path) -> Result<String, ErrorNt> {
    use sha2::{Digest, Sha256};
    let bytes =
        std::fs::read(ruta).map_err(|e| ErrorNt::Biblioteca(format!("{}: {e}", ruta.display())))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

/// Refuses any `wireguard.dll` but the one of [`HUELLA_DLL`].
///
/// # Errors
/// [`ErrorNt::Biblioteca`] when the file cannot be read or is another one.
pub fn comprueba_huella(ruta: &Path) -> Result<(), ErrorNt> {
    let h = huella_de(ruta)?;
    if h == HUELLA_DLL {
        Ok(())
    } else {
        Err(ErrorNt::Biblioteca(format!(
            "{} no es la biblioteca de WireGuardNT 1.1 que GUARDIANA conoce (SHA-256 {h}); no se carga",
            ruta.display()
        )))
    }
}

fn ancho(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn ultimo_error(llamada: &str) -> ErrorNt {
    // SAFETY: reads this thread's last error code; no arguments.
    let codigo = unsafe { GetLastError() };
    ErrorNt::Llamada(format!("{llamada} = {codigo}"))
}

/// `wireguard.dll`, loaded, with the functions of its public interface this program uses.
pub struct Biblioteca {
    modulo: *mut c_void,
    crea: FnCrea,
    cierra: FnCierra,
    luid: FnLuid,
    configura: FnConfigura,
    lee: FnLee,
    estado: FnEstado,
    version: FnVersion,
}

// The library's functions may be called from any thread (they are the driver's control calls).
unsafe impl Send for Biblioteca {}
unsafe impl Sync for Biblioteca {}

/// One function of the library, by its name, as the function type it is declared with in `wireguard.h`.
///
/// # Safety
/// `T` must be the function pointer type of that name's declaration in `wireguard.h`.
unsafe fn funcion<T: Copy>(modulo: *mut c_void, nombre: &str) -> Result<T, ErrorNt> {
    let c: Vec<u8> = nombre.bytes().chain(std::iter::once(0)).collect();
    // SAFETY: a module handle from LoadLibraryExW and a NUL-terminated name owned for the call.
    let f = unsafe { GetProcAddress(modulo, c.as_ptr()) }
        .ok_or_else(|| ErrorNt::Biblioteca(format!("falta {nombre}")))?;
    debug_assert_eq!(std::mem::size_of::<T>(), std::mem::size_of_val(&f));
    // SAFETY: the caller names the declared type; both are plain function pointers of the same size.
    Ok(unsafe { std::mem::transmute_copy::<unsafe extern "system" fn() -> isize, T>(&f) })
}

/// The library's functions, found in the loaded module.
///
/// # Safety
/// `modulo` must be `wireguard.dll`, loaded.
unsafe fn resuelve(modulo: *mut c_void) -> Result<Biblioteca, ErrorNt> {
    // SAFETY: each type below is the declaration of that name in wireguard.h.
    unsafe {
        Ok(Biblioteca {
            modulo,
            crea: funcion(modulo, "WireGuardCreateAdapter")?,
            cierra: funcion(modulo, "WireGuardCloseAdapter")?,
            luid: funcion(modulo, "WireGuardGetAdapterLUID")?,
            configura: funcion(modulo, "WireGuardSetConfiguration")?,
            lee: funcion(modulo, "WireGuardGetConfiguration")?,
            estado: funcion(modulo, "WireGuardSetAdapterState")?,
            version: funcion(modulo, "WireGuardGetRunningDriverVersion")?,
        })
    }
}

impl Biblioteca {
    /// Loads the library from its full path (never from the search path) and finds its functions.
    ///
    /// # Errors
    /// [`ErrorNt::Biblioteca`] when the file cannot be loaded or a function is missing.
    pub fn carga(ruta: &Path) -> Result<Self, ErrorNt> {
        comprueba_huella(ruta)?;
        let r = ancho(&ruta.to_string_lossy());
        // SAFETY: a NUL-terminated full path owned for the call; no file handle; default flags (a full path is
        // loaded from exactly there).
        let modulo = unsafe { LoadLibraryExW(r.as_ptr(), std::ptr::null_mut(), 0) };
        if modulo.is_null() {
            // SAFETY: reads this thread's last error code.
            let codigo = unsafe { GetLastError() };
            return Err(ErrorNt::Biblioteca(format!(
                "{} ({codigo})",
                ruta.display()
            )));
        }
        // SAFETY: the module just loaded from the path given as wireguard.dll.
        let cargada = unsafe { resuelve(modulo) };
        if cargada.is_err() {
            // SAFETY: the module loaded above, not used after this.
            unsafe { FreeLibrary(modulo) };
        }
        cargada
    }

    /// The version of the driver that is running (0 when none is loaded yet), as `major << 16 | minor`.
    #[must_use]
    pub fn version_del_controlador(&self) -> u32 {
        // SAFETY: a function of the library, with no arguments.
        unsafe { (self.version)() }
    }
}

impl Drop for Biblioteca {
    fn drop(&mut self) {
        // SAFETY: the module this value loaded; every adapter holds the library, so none outlives it.
        unsafe { FreeLibrary(self.modulo) };
    }
}

/// One WireGuardNT adapter, created by this program. Closed (and gone) when dropped; it keeps the library loaded.
pub struct Adaptador {
    biblioteca: Arc<Biblioteca>,
    h: *mut c_void,
}

// The handle is the driver's; its control calls may come from any thread.
unsafe impl Send for Adaptador {}
unsafe impl Sync for Adaptador {}

impl Adaptador {
    /// Creates the adapter `nombre`, of the tunnel type `tipo` (the group Windows lists it under).
    ///
    /// # Errors
    /// [`ErrorNt::Llamada`] when the driver refused (it needs the system's account or an administrator's).
    pub fn crea(biblioteca: &Arc<Biblioteca>, nombre: &str, tipo: &str) -> Result<Self, ErrorNt> {
        let n = ancho(nombre);
        let t = ancho(tipo);
        // SAFETY: two NUL-terminated names owned for the call; no requested GUID (Windows picks one).
        let h = unsafe { (biblioteca.crea)(n.as_ptr(), t.as_ptr(), std::ptr::null()) };
        if h.is_null() {
            return Err(ultimo_error("WireGuardCreateAdapter"));
        }
        Ok(Self {
            biblioteca: Arc::clone(biblioteca),
            h,
        })
    }

    /// The adapter's LUID: what Windows' routing, addresses and firewall call it by.
    #[must_use]
    pub fn luid(&self) -> u64 {
        let mut l = 0u64;
        // SAFETY: an adapter handle of this library; NET_LUID is 64 bits, written into a u64 owned here.
        unsafe { (self.biblioteca.luid)(self.h, &mut l) };
        l
    }

    /// Hands the adapter its whole configuration (`guardiana_tunel::nt::configuracion`).
    ///
    /// # Errors
    /// [`ErrorNt::Llamada`] when the driver refused it.
    pub fn configura(&self, bytes: &[u8]) -> Result<(), ErrorNt> {
        let largo = u32::try_from(bytes.len())
            .map_err(|_| ErrorNt::Llamada("configuración demasiado grande".into()))?;
        // SAFETY: an adapter handle of this library and a buffer owned by the caller for the whole call, of the given
        // length, laid out as wireguard.h's structures.
        if unsafe { (self.biblioteca.configura)(self.h, bytes.as_ptr(), largo) } == 0 {
            return Err(ultimo_error("WireGuardSetConfiguration"));
        }
        Ok(())
    }

    /// The adapter's configuration as the driver has it now, with the peers' counters and last handshake
    /// (`guardiana_tunel::nt::estado_del_par` reads them). Wiped when dropped (it may carry the private key).
    ///
    /// # Errors
    /// [`ErrorNt::Llamada`] when the driver refused.
    pub fn lee(&self) -> Result<zeroize::Zeroizing<Vec<u8>>, ErrorNt> {
        let mut largo: u32 = 4_096;
        for _ in 0..4 {
            let mut b = zeroize::Zeroizing::new(vec![0u8; largo as usize]);
            let mut pedido = largo;
            // SAFETY: an adapter handle of this library, a buffer owned here of `pedido` bytes, and its length in a
            // u32 owned here that the driver overwrites with what it needs when the buffer is short.
            if unsafe { (self.biblioteca.lee)(self.h, b.as_mut_ptr(), &mut pedido) } != 0 {
                b.truncate(pedido as usize);
                return Ok(b);
            }
            // SAFETY: reads this thread's last error code.
            let codigo = unsafe { GetLastError() };
            if codigo != ERROR_MORE_DATA || pedido <= largo {
                return Err(ErrorNt::Llamada(format!(
                    "WireGuardGetConfiguration = {codigo}"
                )));
            }
            largo = pedido;
        }
        Err(ErrorNt::Llamada(
            "WireGuardGetConfiguration: el tamaño no se asienta".into(),
        ))
    }

    /// Brings the adapter up (`true`) or down.
    ///
    /// # Errors
    /// [`ErrorNt::Llamada`] when the driver refused.
    pub fn pon(&self, arriba: bool) -> Result<(), ErrorNt> {
        let estado = if arriba { ARRIBA } else { ABAJO };
        // SAFETY: an adapter handle of this library and one of the two states wireguard.h defines.
        if unsafe { (self.biblioteca.estado)(self.h, estado) } == 0 {
            return Err(ultimo_error("WireGuardSetAdapterState"));
        }
        Ok(())
    }
}

impl Drop for Adaptador {
    fn drop(&mut self) {
        // SAFETY: the handle this value created, closed once.
        unsafe { (self.biblioteca.cierra)(self.h) };
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Another file where `wireguard.dll` should be is refused before anything loads it, and says why.
    #[test]
    fn otra_biblioteca_no_se_carga() {
        let dir = std::env::temp_dir().join(format!("guardiana-wg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let falsa = dir.join("wireguard.dll");
        std::fs::write(&falsa, b"MZ no soy WireGuardNT").unwrap();
        let e = comprueba_huella(&falsa).unwrap_err().to_string();
        assert!(
            e.contains("no es la biblioteca de WireGuardNT 1.1") && e.contains("SHA-256"),
            "{e}"
        );
        assert!(Biblioteca::carga(&falsa).is_err());
        assert!(comprueba_huella(&dir.join("no-existe.dll")).is_err());
        assert_eq!(HUELLA_DLL.len(), 64);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// On a real Windows, as an administrator, with the signed library at `GUARDIANA_WG_DLL` (the VPN workflow
    /// downloads it and checks its signature): an adapter is created, configured with this crate's bytes, brought
    /// up and down, and closed.
    #[test]
    #[ignore = "necesita Windows como administrador y wireguard.dll en GUARDIANA_WG_DLL"]
    fn un_adaptador_de_verdad() {
        let ruta = std::env::var_os("GUARDIANA_WG_DLL").unwrap();
        let b = Arc::new(Biblioteca::carga(Path::new(&ruta)).unwrap());
        let a = Adaptador::crea(&b, "GUARDIANA prueba", "GUARDIANA").unwrap();
        assert_ne!(a.luid(), 0);
        let llaves = guardiana_tunel::llaves::Llaves::nuevas().unwrap();
        let par = guardiana_tunel::nt::Par {
            publica: [7; 32],
            psk: None,
            extremo: Some("192.0.2.1:51820".parse().unwrap()),
            keepalive: 25,
            permitidas: vec![("10.66.0.1".parse().unwrap(), 32)],
        };
        let bytes = guardiana_tunel::nt::configuracion(&llaves.secreta_bytes(), None, &[par]);
        assert_eq!(bytes.len(), 80 + 136 + 24);
        a.configura(&bytes).unwrap();
        // read back: the server's key as given, and no handshake (nobody answers at a documentation address)
        let leido = a.lee().unwrap();
        let par = guardiana_tunel::nt::estado_del_par(&leido).unwrap();
        assert_eq!(par.publica, [7; 32]);
        assert_eq!(par.ultimo_saludo_ms, None);
        a.pon(true).unwrap();
        assert_ne!(b.version_del_controlador(), 0);
        a.pon(false).unwrap();
    }
}
