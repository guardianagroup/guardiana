//! Looking at a file (phase A1): its SHA-256, read in pieces so that a big file does not fill the memory, against the
//! fingerprint lists. And the same over a folder, without following links.

use std::fs::{self, File};
use std::io::Read as _;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest as _, Sha256};

use crate::huellas::{a_hex, Huellas};

/// What the analysis found about one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Veredicto {
    /// No list has this file. That does not make it harmless: only that it is not a known sample.
    SinCoincidencias {
        /// The file's SHA-256, in hexadecimal.
        sha256: String,
    },
    /// A list of known malicious samples has exactly this file.
    Conocido {
        /// The file's SHA-256, in hexadecimal.
        sha256: String,
        /// The list that has it, as the warning shows it.
        fuente: String,
    },
    /// The file could not be read (in use, without permission, gone…).
    NoLeido {
        /// What the system said.
        motivo: String,
    },
}

/// One file's result in a folder analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Resultado {
    /// The file.
    pub ruta: PathBuf,
    /// Its size in bytes, when it could be read.
    pub bytes: u64,
    /// What was found.
    pub veredicto: Veredicto,
}

/// The SHA-256 of a file, read in 1 MiB pieces.
pub fn huella_de(ruta: &Path) -> std::io::Result<[u8; 32]> {
    let mut f = File::open(ruta)?;
    let mut h = Sha256::new();
    let mut trozo = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut trozo)?;
        if n == 0 {
            break;
        }
        h.update(&trozo[..n]);
    }
    let mut salida = [0u8; 32];
    salida.copy_from_slice(&h.finalize());
    Ok(salida)
}

/// Analyses one file against the lists: the first list that has it is the one the warning cites.
#[must_use]
pub fn analiza(ruta: &Path, listas: &[Huellas]) -> Veredicto {
    match huella_de(ruta) {
        Err(e) => Veredicto::NoLeido {
            motivo: e.to_string(),
        },
        Ok(h) => {
            let sha256 = a_hex(&h);
            match listas.iter().find(|l| l.contiene(&h)) {
                Some(l) => Veredicto::Conocido {
                    sha256,
                    fuente: l.fuente.clone(),
                },
                None => Veredicto::SinCoincidencias { sha256 },
            }
        }
    }
}

/// Analyses the files of a folder and its subfolders, down to `profundidad` levels and at most `maximo` files. Links
/// (and Windows junctions) are not followed, so the analysis never leaves the folder it was given.
#[must_use]
pub fn analiza_carpeta(
    carpeta: &Path,
    listas: &[Huellas],
    profundidad: usize,
    maximo: usize,
) -> Vec<Resultado> {
    let mut salida = Vec::new();
    let mut pendientes: Vec<(PathBuf, usize)> = vec![(carpeta.to_path_buf(), 0)];
    while let Some((dir, nivel)) = pendientes.pop() {
        let Ok(entradas) = fs::read_dir(&dir) else {
            continue;
        };
        let mut nombres: Vec<PathBuf> = entradas.filter_map(|e| e.ok().map(|e| e.path())).collect();
        nombres.sort();
        for ruta in nombres {
            if salida.len() >= maximo {
                return salida;
            }
            let Ok(meta) = fs::symlink_metadata(&ruta) else {
                continue;
            };
            let tipo = meta.file_type();
            if tipo.is_symlink() {
                continue;
            }
            if tipo.is_dir() {
                if nivel < profundidad {
                    pendientes.push((ruta, nivel + 1));
                }
                continue;
            }
            if !tipo.is_file() {
                continue;
            }
            let veredicto = analiza(&ruta, listas);
            salida.push(Resultado {
                ruta,
                bytes: meta.len(),
                veredicto,
            });
        }
    }
    salida
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn carpeta_de_prueba(nombre: &str) -> PathBuf {
        let mut r = [0u8; 8];
        let _ = getrandom::fill(&mut r);
        let d = std::env::temp_dir().join(format!("guardiana-escudo-{nombre}-{}", a_hex(&r)));
        let _ = fs::create_dir_all(&d);
        d
    }

    #[test]
    fn un_archivo_conocido_y_otro_no() {
        let d = carpeta_de_prueba("analiza");
        let malo = d.join("factura.exe");
        let bueno = d.join("notas.txt");
        let _ = fs::write(&malo, b"contenido de prueba que hace de muestra conocida");
        let _ = fs::write(&bueno, b"otra cosa");
        let h = huella_de(&malo).unwrap_or([0; 32]);
        let lista = Huellas::desde_texto("MalwareBazaar (prueba)", &a_hex(&h));
        assert_eq!(
            analiza(&malo, std::slice::from_ref(&lista)),
            Veredicto::Conocido {
                sha256: a_hex(&h),
                fuente: "MalwareBazaar (prueba)".into()
            }
        );
        assert!(matches!(
            analiza(&bueno, std::slice::from_ref(&lista)),
            Veredicto::SinCoincidencias { .. }
        ));
        assert!(matches!(
            analiza(&d.join("no-existe"), &[lista]),
            Veredicto::NoLeido { .. }
        ));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn la_huella_de_un_archivo_grande_es_la_de_siempre() {
        let d = carpeta_de_prueba("grande");
        let f = d.join("grande.bin");
        let datos: Vec<u8> = (0..(3 << 20) + 17).map(|i| (i % 251) as u8).collect();
        let _ = fs::write(&f, &datos);
        let esperada = Sha256::digest(&datos);
        assert_eq!(
            huella_de(&f).ok().map(|h| a_hex(&h)),
            Some(a_hex(&esperada))
        );
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn una_carpeta_con_subcarpetas_y_tope() {
        let d = carpeta_de_prueba("carpeta");
        let _ = fs::create_dir_all(d.join("a/b/c"));
        for (i, r) in ["uno", "a/dos", "a/b/tres", "a/b/c/cuatro"]
            .iter()
            .enumerate()
        {
            let _ = fs::write(d.join(r), format!("archivo {i}"));
        }
        let listas = [Huellas::vacia("vacía")];
        assert_eq!(analiza_carpeta(&d, &listas, 10, 100).len(), 4);
        assert_eq!(analiza_carpeta(&d, &listas, 1, 100).len(), 2);
        assert_eq!(analiza_carpeta(&d, &listas, 10, 3).len(), 3);
        let _ = fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn no_sigue_enlaces() {
        let d = carpeta_de_prueba("enlaces");
        let fuera = carpeta_de_prueba("fuera");
        let _ = fs::write(fuera.join("secreto"), b"fuera de la carpeta");
        let _ = std::os::unix::fs::symlink(&fuera, d.join("enlace"));
        let _ = fs::write(d.join("dentro"), b"dentro");
        let r = analiza_carpeta(&d, &[Huellas::vacia("vacía")], 10, 100);
        assert_eq!(r.len(), 1);
        let _ = fs::remove_dir_all(&d);
        let _ = fs::remove_dir_all(&fuera);
    }
}
