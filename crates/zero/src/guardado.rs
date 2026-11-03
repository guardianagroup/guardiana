//! The saves of the browser's beat on a thread of their own (1.0.55, «más rápido»).
//!
//! Every 15 seconds the session saves its day's figures (`diario.json`), the auctions' (`subastas.json`) and the book
//! of who received data (`libro.json`), each one whole. The bytes are made on the window's thread, from the state at
//! that moment; writing them goes to a thread of its own, in the order they were asked for, so a slow disk (or an
//! antivirus reading each new file) never holds a page's requests. [`Escritor::espera`] waits until every save asked
//! for so far is on disk: closing the browser and «Borrar todo» use it. In the tests, and if that thread cannot start,
//! the saves are written right away, as before.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

/// Write a whole file or nothing: a crash never leaves half a file (it is written beside, then renamed).
///
/// # Errors
///
/// What the system said when the folder, the file or the rename failed.
pub fn escribe(ruta: &Path, contenido: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = ruta.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = ruta.with_extension("tmp");
    std::fs::write(&tmp, contenido)?;
    std::fs::rename(&tmp, ruta)
}

enum Trabajo {
    Escribe(PathBuf, Vec<u8>),
    Espera(mpsc::Sender<()>),
}

/// Where the session's beat writes its saves.
#[derive(Debug)]
pub struct Escritor {
    hilo: Option<mpsc::Sender<Trabajo>>,
}

impl Escritor {
    /// Saves written by a thread of their own, in order.
    #[must_use]
    pub fn en_hilo() -> Self {
        let (tx, rx) = mpsc::channel::<Trabajo>();
        let lanzado = std::thread::Builder::new()
            .name("zero-guardado".into())
            .spawn(move || {
                while let Ok(t) = rx.recv() {
                    match t {
                        // Losing one save is better than stopping: the next beat writes it again.
                        Trabajo::Escribe(ruta, bytes) => {
                            let _ = escribe(&ruta, &bytes);
                        }
                        Trabajo::Espera(listo) => {
                            let _ = listo.send(());
                        }
                    }
                }
            });
        Self {
            hilo: lanzado.is_ok().then_some(tx),
        }
    }

    /// Saves written right away, on the caller's thread.
    #[must_use]
    pub const fn aqui() -> Self {
        Self { hilo: None }
    }

    /// Save `bytes` as `ruta`, whole or not at all ([`escribe`]).
    pub fn escribe(&self, ruta: PathBuf, bytes: Vec<u8>) {
        match &self.hilo {
            Some(tx) => {
                // The thread is gone (it never should be): written here instead.
                if let Err(mpsc::SendError(Trabajo::Escribe(ruta, bytes))) =
                    tx.send(Trabajo::Escribe(ruta, bytes))
                {
                    let _ = escribe(&ruta, &bytes);
                }
            }
            None => {
                let _ = escribe(&ruta, &bytes);
            }
        }
    }

    /// Wait until every save asked for so far is on disk (at most ten seconds).
    pub fn espera(&self) {
        let Some(tx) = &self.hilo else {
            return;
        };
        let (listo, hecho) = mpsc::channel();
        if tx.send(Trabajo::Espera(listo)).is_ok() {
            let _ = hecho.recv_timeout(Duration::from_secs(10));
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn the_thread_writes_in_order_and_waiting_means_written() {
        let d = std::env::temp_dir().join(format!("zero-guardado-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let e = Escritor::en_hilo();
        assert!(e.hilo.is_some());
        for i in 0..50 {
            e.escribe(d.join("diario.json"), format!("{{\"n\":{i}}}").into_bytes());
        }
        e.escribe(d.join("otra").join("libro.json"), b"{}".to_vec());
        e.espera();
        assert_eq!(
            std::fs::read_to_string(d.join("diario.json")).unwrap(),
            "{\"n\":49}"
        );
        assert_eq!(
            std::fs::read_to_string(d.join("otra").join("libro.json")).unwrap(),
            "{}"
        );
        assert!(!d.join("diario.tmp").exists());
        // Right away, the same.
        let aqui = Escritor::aqui();
        aqui.escribe(d.join("subastas.json"), b"[]".to_vec());
        aqui.espera();
        assert_eq!(
            std::fs::read_to_string(d.join("subastas.json")).unwrap(),
            "[]"
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}
