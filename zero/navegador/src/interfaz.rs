//! The browser's own pages (bar, side panel, new tab), embedded in the program and written to
//! its folder on start, where the engine serves them as `https://zero.guardiana/…`. Nothing is
//! fetched from the network to draw the browser itself.

use std::fs;
use std::io;
use std::path::Path;

include!(concat!(env!("OUT_DIR"), "/interfaz.rs"));

/// Write the pages into `carpeta` (only the ones that changed).
pub fn prepara(carpeta: &Path) -> io::Result<()> {
    for (nombre, datos) in ARCHIVOS {
        let ruta = carpeta.join(nombre);
        if fs::read(&ruta).ok().as_deref() == Some(*datos) {
            continue;
        }
        if let Some(dir) = ruta.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(&ruta, datos)?;
    }
    Ok(())
}

/// The window icon: the PNG images inside the program's `.ico`.
pub const ICONO: &[u8] = include_bytes!("../recursos/guardiana-zero.ico");
