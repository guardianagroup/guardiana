//! The quarantine (phase A1): a file found to be a known malicious sample is moved out of the way, encrypted so that
//! nothing can run or open it by accident and other antivirus programs do not keep finding it, and it can always be put
//! back exactly as it was.
//!
//! Each entry is two files in the quarantine folder: `<id>.bin`, the content encrypted with XChaCha20-Poly1305 under a
//! key of its own, and `<id>.json`, what the file was (where it was, its size, its SHA-256, why and on which list's word,
//! and when) together with that key. The key is not a secret: it is only there so that the copy is inert. On Windows the
//! folder belongs to the system account and nobody else can read it (the service creates it so).

use std::fs;
use std::path::{Path, PathBuf};

use chacha20poly1305::aead::{Aead as _, KeyInit as _, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::huellas::a_hex;

/// The largest file the quarantine takes (256 MiB); a larger one is only reported.
pub const MAX_BYTES: u64 = 256 * 1024 * 1024;

/// What the quarantine keeps about a file, as the panel shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entrada {
    /// The entry's own name (32 hexadecimal digits).
    pub id: String,
    /// Where the file was.
    pub ruta: String,
    /// Its size in bytes.
    pub bytes: u64,
    /// Its SHA-256, in hexadecimal.
    pub sha256: String,
    /// Why it was put here, in plain words.
    pub motivo: String,
    /// Whose word it was (the list or the rule).
    pub fuente: String,
    /// When (milliseconds since 1970).
    pub cuando_ms: u64,
}

/// What is written next to the encrypted copy: the entry and the copy's key.
#[derive(Serialize, Deserialize)]
struct Registro {
    #[serde(flatten)]
    entrada: Entrada,
    clave: String,
    nonce: String,
}

/// Why the quarantine could not do what it was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorCuarentena {
    /// The file is larger than the quarantine takes.
    Grande,
    /// The file could not be read (or it is a link, which is never followed).
    Lectura(String),
    /// Something could not be written or removed; nothing was left half done.
    Escritura(String),
    /// There is no such entry.
    NoExiste,
    /// The stored copy does not decrypt or does not match its SHA-256: it is not put back.
    Alterada,
    /// Putting it back would overwrite a file that is there now.
    YaExiste(PathBuf),
}

impl std::fmt::Display for ErrorCuarentena {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Grande => f.write_str("el archivo es demasiado grande para la cuarentena"),
            Self::Lectura(m) => write!(f, "no se pudo leer el archivo: {m}"),
            Self::Escritura(m) => write!(f, "no se pudo escribir: {m}"),
            Self::NoExiste => f.write_str("no hay nada así en la cuarentena"),
            Self::Alterada => f.write_str("la copia de la cuarentena no está como se guardó"),
            Self::YaExiste(r) => write!(f, "ya hay un archivo en {}", r.display()),
        }
    }
}

impl std::error::Error for ErrorCuarentena {}

/// The quarantine folder.
#[derive(Debug, Clone)]
pub struct Cuarentena {
    carpeta: PathBuf,
    tope: u64,
}

impl Cuarentena {
    /// The quarantine in this folder (created if missing), taking files up to [`MAX_BYTES`].
    pub fn en(carpeta: impl Into<PathBuf>) -> std::io::Result<Self> {
        Self::con_tope(carpeta, MAX_BYTES)
    }

    /// The same, with another size limit.
    pub fn con_tope(carpeta: impl Into<PathBuf>, tope: u64) -> std::io::Result<Self> {
        let carpeta = carpeta.into();
        fs::create_dir_all(&carpeta)?;
        Ok(Self { carpeta, tope })
    }

    /// Moves a file into the quarantine. The original is removed only once the encrypted copy and its record are safely
    /// written; if it cannot be removed, the copy is undone and the error says so.
    pub fn poner(
        &self,
        ruta: &Path,
        motivo: &str,
        fuente: &str,
        ahora_ms: u64,
    ) -> Result<Entrada, ErrorCuarentena> {
        let meta =
            fs::symlink_metadata(ruta).map_err(|e| ErrorCuarentena::Lectura(e.to_string()))?;
        if !meta.file_type().is_file() {
            return Err(ErrorCuarentena::Lectura("no es un archivo normal".into()));
        }
        if meta.len() > self.tope {
            return Err(ErrorCuarentena::Grande);
        }
        let contenido = fs::read(ruta).map_err(|e| ErrorCuarentena::Lectura(e.to_string()))?;
        let mut clave = [0u8; 32];
        let mut nonce = [0u8; 24];
        let mut id = [0u8; 16];
        for b in [&mut clave[..], &mut nonce[..], &mut id[..]] {
            getrandom::fill(b).map_err(|e| ErrorCuarentena::Escritura(e.to_string()))?;
        }
        let id = a_hex(&id);
        let cifrado = XChaCha20Poly1305::new((&clave).into())
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &contenido,
                    aad: id.as_bytes(),
                },
            )
            .map_err(|_| ErrorCuarentena::Escritura("no se pudo encriptar la copia".into()))?;
        let entrada = Entrada {
            id: id.clone(),
            ruta: ruta.to_string_lossy().into_owned(),
            bytes: contenido.len() as u64,
            sha256: a_hex(&Sha256::digest(&contenido)),
            motivo: motivo.to_string(),
            fuente: fuente.to_string(),
            cuando_ms: ahora_ms,
        };
        let registro = Registro {
            entrada: entrada.clone(),
            clave: a_hex(&clave),
            nonce: a_hex(&nonce),
        };
        let json = serde_json::to_vec_pretty(&registro)
            .map_err(|e| ErrorCuarentena::Escritura(e.to_string()))?;
        let (bin, reg) = (
            self.carpeta.join(format!("{id}.bin")),
            self.carpeta.join(format!("{id}.json")),
        );
        escribe_entero(&bin, &cifrado)?;
        if let Err(e) = escribe_entero(&reg, &json) {
            let _ = fs::remove_file(&bin);
            return Err(e);
        }
        if let Err(e) = fs::remove_file(ruta) {
            let _ = fs::remove_file(&bin);
            let _ = fs::remove_file(&reg);
            return Err(ErrorCuarentena::Escritura(format!(
                "no se pudo quitar el original: {e}"
            )));
        }
        Ok(entrada)
    }

    /// Puts a file back where it was (or in `destino`), exactly as it was: the copy must decrypt and match its SHA-256,
    /// and nothing that is there now is overwritten. The entry leaves the quarantine.
    pub fn restaurar(&self, id: &str, destino: Option<&Path>) -> Result<PathBuf, ErrorCuarentena> {
        let registro = self.lee(id)?;
        let cifrado = fs::read(self.carpeta.join(format!("{id}.bin")))
            .map_err(|_| ErrorCuarentena::NoExiste)?;
        let clave: [u8; 32] =
            crate::huellas::de_hex32(&registro.clave).ok_or(ErrorCuarentena::Alterada)?;
        let nonce = de_hex(&registro.nonce)
            .filter(|n| n.len() == 24)
            .ok_or(ErrorCuarentena::Alterada)?;
        let contenido = XChaCha20Poly1305::new((&clave).into())
            .decrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &cifrado,
                    aad: id.as_bytes(),
                },
            )
            .map_err(|_| ErrorCuarentena::Alterada)?;
        if a_hex(&Sha256::digest(&contenido)) != registro.entrada.sha256 {
            return Err(ErrorCuarentena::Alterada);
        }
        let destino =
            destino.map_or_else(|| PathBuf::from(&registro.entrada.ruta), Path::to_path_buf);
        if fs::symlink_metadata(&destino).is_ok() {
            return Err(ErrorCuarentena::YaExiste(destino));
        }
        if let Some(padre) = destino.parent() {
            fs::create_dir_all(padre).map_err(|e| ErrorCuarentena::Escritura(e.to_string()))?;
        }
        escribe_entero(&destino, &contenido)?;
        let _ = fs::remove_file(self.carpeta.join(format!("{id}.bin")));
        let _ = fs::remove_file(self.carpeta.join(format!("{id}.json")));
        Ok(destino)
    }

    /// What is in the quarantine, oldest first. An unreadable record is skipped.
    #[must_use]
    pub fn lista(&self) -> Vec<Entrada> {
        let Ok(dir) = fs::read_dir(&self.carpeta) else {
            return Vec::new();
        };
        let mut v: Vec<Entrada> = dir
            .filter_map(Result::ok)
            .filter_map(|e| {
                let nombre = e.file_name().to_string_lossy().into_owned();
                let id = nombre.strip_suffix(".json")?;
                self.lee(id).ok().map(|r| r.entrada)
            })
            .collect();
        v.sort_by(|a, b| a.cuando_ms.cmp(&b.cuando_ms).then_with(|| a.id.cmp(&b.id)));
        v
    }

    fn lee(&self, id: &str) -> Result<Registro, ErrorCuarentena> {
        // only the quarantine's own names: never a path that leaves the folder
        if id.len() != 32
            || !id
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            return Err(ErrorCuarentena::NoExiste);
        }
        let json = fs::read(self.carpeta.join(format!("{id}.json")))
            .map_err(|_| ErrorCuarentena::NoExiste)?;
        let r: Registro = serde_json::from_slice(&json).map_err(|_| ErrorCuarentena::Alterada)?;
        if r.entrada.id != id {
            return Err(ErrorCuarentena::Alterada);
        }
        Ok(r)
    }
}

/// Writes a whole file through a temporary name in the same folder, so that it is either all there or not at all.
fn escribe_entero(ruta: &Path, datos: &[u8]) -> Result<(), ErrorCuarentena> {
    let tmp = ruta.with_extension("tmp-guardiana");
    fs::write(&tmp, datos).map_err(|e| ErrorCuarentena::Escritura(e.to_string()))?;
    fs::rename(&tmp, ruta).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        ErrorCuarentena::Escritura(e.to_string())
    })
}

fn de_hex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    s.as_bytes()
        .chunks_exact(2)
        .map(|p| {
            let t = std::str::from_utf8(p).ok()?;
            u8::from_str_radix(t, 16).ok()
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn carpeta_de_prueba(nombre: &str) -> PathBuf {
        let mut r = [0u8; 8];
        let _ = getrandom::fill(&mut r);
        let d = std::env::temp_dir().join(format!("guardiana-cuarentena-{nombre}-{}", a_hex(&r)));
        let _ = fs::create_dir_all(&d);
        d
    }

    #[test]
    fn poner_y_devolver_exactamente_igual() {
        let d = carpeta_de_prueba("ida-vuelta");
        let q = Cuarentena::en(d.join("cuarentena")).unwrap();
        let f = d.join("Descargas").join("factura.exe");
        let _ = fs::create_dir_all(f.parent().unwrap_or(&d));
        let datos: Vec<u8> = (0..70_000u32).map(|i| (i * 7 % 256) as u8).collect();
        let _ = fs::write(&f, &datos);
        let e = q
            .poner(&f, "muestra conocida", "MalwareBazaar (prueba)", 1_000)
            .unwrap();
        assert!(!f.exists(), "el original sigue ahí");
        assert_eq!(e.bytes, 70_000);
        assert_eq!(q.lista(), vec![e.clone()]);
        // la copia guardada no es el archivo: está encriptada
        let guardado =
            fs::read(d.join("cuarentena").join(format!("{}.bin", e.id))).unwrap_or_default();
        assert!(!guardado.windows(64).any(|w| w == &datos[..64]));
        assert_eq!(q.restaurar(&e.id, None), Ok(f.clone()));
        assert_eq!(fs::read(&f).ok(), Some(datos));
        assert!(q.lista().is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn una_copia_tocada_no_se_devuelve() {
        let d = carpeta_de_prueba("tocada");
        let q = Cuarentena::en(d.join("q")).unwrap();
        let f = d.join("a.exe");
        let _ = fs::write(&f, b"muestra");
        let e = q.poner(&f, "m", "f", 1).unwrap();
        let bin = d.join("q").join(format!("{}.bin", e.id));
        let mut b = fs::read(&bin).unwrap_or_default();
        if let Some(x) = b.first_mut() {
            *x ^= 1;
        }
        let _ = fs::write(&bin, &b);
        assert_eq!(q.restaurar(&e.id, None), Err(ErrorCuarentena::Alterada));
        assert!(!f.exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn no_pisa_lo_que_hay_ni_sale_de_su_carpeta() {
        let d = carpeta_de_prueba("pisa");
        let q = Cuarentena::en(d.join("q")).unwrap();
        let f = d.join("a.exe");
        let _ = fs::write(&f, b"muestra");
        let e = q.poner(&f, "m", "f", 1).unwrap();
        let _ = fs::write(&f, b"otro archivo con el mismo nombre");
        assert_eq!(
            q.restaurar(&e.id, None),
            Err(ErrorCuarentena::YaExiste(f.clone()))
        );
        assert_eq!(
            q.restaurar("../../etc/passwd", None),
            Err(ErrorCuarentena::NoExiste)
        );
        let otro = d.join("restaurado.exe");
        assert_eq!(q.restaurar(&e.id, Some(&otro)), Ok(otro.clone()));
        assert_eq!(fs::read(&otro).ok(), Some(b"muestra".to_vec()));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn lo_grande_y_lo_que_no_es_un_archivo_no_entran() {
        let d = carpeta_de_prueba("tope");
        let q = Cuarentena::con_tope(d.join("q"), 10).unwrap();
        let f = d.join("grande.bin");
        let _ = fs::write(&f, [0u8; 11]);
        assert_eq!(q.poner(&f, "m", "f", 1), Err(ErrorCuarentena::Grande));
        assert!(f.exists());
        assert!(matches!(
            q.poner(&d, "m", "f", 1),
            Err(ErrorCuarentena::Lectura(_))
        ));
        let _ = fs::remove_dir_all(&d);
    }
}
