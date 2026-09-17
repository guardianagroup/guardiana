//! The vault directory and its master key.
//!
//! ```text
//! <boveda>/
//!   boveda.json        header: format, id, creation time, Argon2id parameters, the master key
//!                      wrapped twice (under the password, under the 24 recovery words)
//!   objetos/<id>.gdn   one file per stored object (see `object`)
//!   registro.jsonl     the hash-chained access log (see `log`)
//! ```
//! The master key never touches the disk in clear. Losing both the password and the 24 words
//! loses the content: there is no other way in, for anyone (concept §05.2).

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::error::{Error, Result};
use crate::kdf::{self, KdfParams};
use crate::object::{self, Meta, Wrapped};
use crate::{hexs, log, rng, shamir, time, words};

/// Format label written in the header and used in associated data.
pub const FORMAT: &str = "guardiana-boveda/1";
/// Header file name.
pub const HEADER: &str = "boveda.json";
/// Objects directory name.
pub const OBJECTS: &str = "objetos";
/// Pieces the recovery words are split into, and how many rebuild them.
pub const PIECES: u8 = 5;
/// Pieces needed.
pub const NEEDED: u8 = 3;

/// One way to open the master key.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeySlot {
    /// `contrasena` (master password) or `palabras` (24 recovery words).
    pub tipo: String,
    /// Argon2id salt, 32 bytes hex.
    pub sal: String,
    /// The master key wrapped under the derived key.
    #[serde(flatten)]
    pub llave: Wrapped,
}

/// The clear header of a vault.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Header {
    /// Always `guardiana-boveda/1`.
    pub formato: String,
    /// 16 hex characters identifying this vault (also seeds the log chain).
    pub id: String,
    /// RFC 3339 UTC.
    pub creada: String,
    /// Argon2id parameters shared by both slots.
    pub kdf: KdfParams,
    /// The two slots, password first.
    pub llaves: Vec<KeySlot>,
}

/// An open vault: directory, header and the master key in memory (wiped on drop).
pub struct Vault {
    /// Where it lives.
    pub dir: PathBuf,
    /// Its header.
    pub header: Header,
    master: Zeroizing<[u8; 32]>,
}

/// Result of creating a vault: the vault and the 24 words, shown once.
pub struct Created {
    /// The open vault.
    pub vault: Vault,
    /// The recovery words. Show them once; never store them.
    pub words: Vec<&'static str>,
}

/// One listed object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// Object id (file name without `.gdn`).
    pub id: String,
    /// Its metadata.
    pub meta: Meta,
}

fn aad_slot(id: &str, tipo: &str) -> String {
    format!("{FORMAT} boveda {id} llave {tipo}")
}

fn read_header(dir: &Path) -> Result<Header> {
    let p = dir.join(HEADER);
    if !p.exists() {
        return Err(Error::NotFound(dir.to_owned()));
    }
    let h: Header = serde_json::from_slice(&fs::read(p)?)?;
    if h.formato != FORMAT {
        return Err(Error::Format(format!("unknown format {}", h.formato)));
    }
    Ok(h)
}

fn slot_wrap(
    master: &[u8; 32],
    id: &str,
    tipo: &str,
    secret: &[u8],
    params: KdfParams,
) -> Result<KeySlot> {
    let salt = rng::key()?;
    let kek = kdf::derive(secret, &salt, params)?;
    Ok(KeySlot {
        tipo: tipo.to_owned(),
        sal: hexs::to_hex(&salt),
        llave: object::wrap_key(&kek, master, &aad_slot(id, tipo))?,
    })
}

fn slot_open(h: &Header, tipo: &str, secret: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
    let slot = h
        .llaves
        .iter()
        .find(|s| s.tipo == tipo)
        .ok_or_else(|| Error::Format(format!("no {tipo} slot")))?;
    let salt = hexs::from_hex(&slot.sal)?;
    let kek = kdf::derive(secret, &salt, h.kdf)?;
    object::unwrap_key(&kek, &slot.llave, &aad_slot(&h.id, tipo))
}

impl Vault {
    /// Create a new vault at `dir` (must not exist yet) protected by `password`. Also generates the
    /// 24 recovery words and writes the first log line.
    pub fn create(dir: &Path, password: &[u8], params: KdfParams, quien: &str) -> Result<Created> {
        if dir.join(HEADER).exists() {
            return Err(Error::Exists(dir.to_owned()));
        }
        fs::create_dir_all(dir.join(OBJECTS))?;
        let master = Zeroizing::new(rng::key()?);
        let id = hexs::to_hex(&rng::bytes(8)?);
        let entropy = Zeroizing::new(rng::key()?);
        let w = words::to_words(&entropy);
        let header = Header {
            formato: FORMAT.to_owned(),
            id: id.clone(),
            creada: time::now_rfc3339(),
            kdf: params,
            llaves: vec![
                slot_wrap(&master, &id, "contrasena", password, params)?,
                slot_wrap(&master, &id, "palabras", entropy.as_slice(), params)?,
            ],
        };
        fs::write(dir.join(HEADER), serde_json::to_vec_pretty(&header)?)?;
        log::append(dir, &id, &time::now_rfc3339(), "crear", None, quien)?;
        Ok(Created {
            vault: Self {
                dir: dir.to_owned(),
                header,
                master,
            },
            words: w,
        })
    }

    /// Open with the master password. A wrong password is logged as `fallo` (no secret in it).
    pub fn open(dir: &Path, password: &[u8], quien: &str) -> Result<Self> {
        let header = read_header(dir)?;
        match slot_open(&header, "contrasena", password) {
            Ok(master) => {
                log::append(dir, &header.id, &time::now_rfc3339(), "abrir", None, quien)?;
                Ok(Self {
                    dir: dir.to_owned(),
                    header,
                    master,
                })
            }
            Err(e) => {
                log::append(dir, &header.id, &time::now_rfc3339(), "fallo", None, quien)?;
                Err(e)
            }
        }
    }

    /// Open with the 24 recovery words (the password was lost).
    pub fn open_with_words(dir: &Path, w: &[&str], quien: &str) -> Result<Self> {
        let header = read_header(dir)?;
        let entropy = words::from_words(w)?;
        match slot_open(&header, "palabras", entropy.as_slice()) {
            Ok(master) => {
                log::append(
                    dir,
                    &header.id,
                    &time::now_rfc3339(),
                    "recuperar",
                    None,
                    quien,
                )?;
                Ok(Self {
                    dir: dir.to_owned(),
                    header,
                    master,
                })
            }
            Err(e) => {
                log::append(
                    dir,
                    &header.id,
                    &time::now_rfc3339(),
                    "fallo",
                    None,
                    &format!("{quien} (palabras)"),
                )?;
                Err(e)
            }
        }
    }

    /// Replace the password slot with a new password (after recovery, or by choice).
    pub fn set_password(&mut self, new: &[u8], quien: &str) -> Result<()> {
        let slot = slot_wrap(
            &self.master,
            &self.header.id,
            "contrasena",
            new,
            self.header.kdf,
        )?;
        if let Some(s) = self
            .header
            .llaves
            .iter_mut()
            .find(|s| s.tipo == "contrasena")
        {
            *s = slot;
        } else {
            self.header.llaves.insert(0, slot);
        }
        let tmp = self.dir.join("boveda.json.nuevo");
        fs::write(&tmp, serde_json::to_vec_pretty(&self.header)?)?;
        fs::rename(&tmp, self.dir.join(HEADER))?;
        log::append(
            &self.dir,
            &self.header.id,
            &time::now_rfc3339(),
            "contrasena",
            None,
            quien,
        )?;
        Ok(())
    }

    /// Store `src` (`size` bytes) under `nombre`. Returns the new object.
    pub fn add(&self, nombre: &str, src: &mut impl Read, size: u64, quien: &str) -> Result<Item> {
        let id = hexs::to_hex(&rng::bytes(8)?);
        let path = self.dir.join(OBJECTS).join(format!("{id}.gdn"));
        let meta = object::write(
            &path,
            &self.master,
            &id,
            nombre,
            &time::now_rfc3339(),
            src,
            size,
        )?;
        log::append(
            &self.dir,
            &self.header.id,
            &time::now_rfc3339(),
            "guardar",
            Some(&id),
            &format!("{quien} · {} bytes", size),
        )?;
        Ok(Item { id, meta })
    }

    /// Store a file from disk by path (its file name becomes the object name).
    pub fn add_file(&self, path: &Path, quien: &str) -> Result<Item> {
        let nombre = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| Error::Format("file name".into()))?;
        let size = fs::metadata(path)?.len();
        let mut f = fs::File::open(path)?;
        self.add(nombre, &mut f, size, quien)
    }

    /// Every object, newest first.
    pub fn list(&self) -> Result<Vec<Item>> {
        let mut out = Vec::new();
        let dir = self.dir.join(OBJECTS);
        if dir.exists() {
            for entry in fs::read_dir(dir)? {
                let p = entry?.path();
                if p.extension().and_then(|e| e.to_str()) != Some("gdn") {
                    continue;
                }
                let (h, _) = object::read_header(&p)?;
                let (_, meta) = object::open_meta(&h, &self.master)?;
                out.push(Item { id: h.id, meta });
            }
        }
        out.sort_by(|a, b| b.meta.guardado.cmp(&a.meta.guardado).then(a.id.cmp(&b.id)));
        Ok(out)
    }

    /// Find one object by id or by exact name.
    pub fn find(&self, id_or_name: &str) -> Result<Item> {
        self.list()?
            .into_iter()
            .find(|i| i.id == id_or_name || i.meta.nombre == id_or_name)
            .ok_or_else(|| Error::NoObject(id_or_name.to_owned()))
    }

    /// Decrypt one object into `out`; logged as `leer`.
    pub fn extract(&self, id_or_name: &str, out: &mut impl Write, quien: &str) -> Result<Meta> {
        let item = self.find(id_or_name)?;
        let path = self.dir.join(OBJECTS).join(format!("{}.gdn", item.id));
        let meta = object::extract(&path, &self.master, out)?;
        log::append(
            &self.dir,
            &self.header.id,
            &time::now_rfc3339(),
            "leer",
            Some(&item.id),
            quien,
        )?;
        Ok(meta)
    }

    /// Copy the whole vault (still encrypted) into `dest/guardiana-boveda-<id>-<date>/` — the
    /// capsule for an external disk. Returns the folder written; logged as `capsula`.
    pub fn capsule(&self, dest: &Path, quien: &str) -> Result<PathBuf> {
        let stamp = time::now_rfc3339();
        let folder = dest.join(format!(
            "guardiana-boveda-{}-{}",
            self.header.id,
            &stamp[..10]
        ));
        fs::create_dir_all(folder.join(OBJECTS))?;
        log::append(
            &self.dir,
            &self.header.id,
            &stamp,
            "capsula",
            None,
            &format!("{quien} · {}", folder.display()),
        )?;
        fs::copy(self.dir.join(HEADER), folder.join(HEADER))?;
        fs::copy(self.dir.join(log::FILE), folder.join(log::FILE))?;
        for entry in fs::read_dir(self.dir.join(OBJECTS))? {
            let p = entry?.path();
            if let Some(name) = p.file_name() {
                fs::copy(&p, folder.join(OBJECTS).join(name))?;
            }
        }
        Ok(folder)
    }

    /// Split the 24 words into 5 pieces of which any 3 rebuild them; logged as `trozos`.
    pub fn pieces(&self, w: &[&str], quien: &str) -> Result<Vec<String>> {
        let entropy = words::from_words(w)?;
        // The words must be this vault's: check them against the slot before splitting.
        slot_open(&self.header, "palabras", entropy.as_slice())?;
        let pieces = shamir::split(entropy.as_slice(), PIECES, NEEDED)?;
        log::append(
            &self.dir,
            &self.header.id,
            &time::now_rfc3339(),
            "trozos",
            None,
            quien,
        )?;
        Ok(pieces
            .iter()
            .map(|p| shamir::to_text(p, NEEDED, PIECES))
            .collect())
    }

    /// The log, and whether its chain holds.
    pub fn log(dir: &Path) -> Result<(Vec<log::Entry>, std::result::Result<u64, u64>)> {
        let h = read_header(dir)?;
        let entries = log::read_all(dir)?;
        let check = match log::check(dir, &h.id) {
            Ok(n) => Ok(n),
            Err(Error::ChainBroken(n)) => Err(n),
            Err(e) => return Err(e),
        };
        Ok((entries, check))
    }

    /// The header alone (no key needed).
    pub fn peek(dir: &Path) -> Result<Header> {
        read_header(dir)
    }
}

/// Rebuild the 24 words from any 3 of the 5 pieces (pure: needs no vault).
pub fn words_from_pieces(texts: &[&str]) -> Result<Vec<&'static str>> {
    let pieces = texts
        .iter()
        .map(|t| shamir::from_text(t))
        .collect::<Result<Vec<_>>>()?;
    if pieces.len() < usize::from(NEEDED) {
        return Err(Error::BadShare(format!(
            "{} pieces, need {NEEDED}",
            pieces.len()
        )));
    }
    let entropy = Zeroizing::new(shamir::combine(&pieces[..usize::from(NEEDED)])?);
    let arr: [u8; 32] = entropy
        .as_slice()
        .try_into()
        .map_err(|_| Error::BadShare("length".into()))?;
    Ok(words::to_words(&arr))
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("gdn-vault-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    const FAST: KdfParams = KdfParams {
        memoria_kib: 8 * 1024,
        pasadas: 1,
        hilos: 1,
    };

    #[test]
    fn create_store_list_extract_recover() {
        let dir = tmp("a");
        let created =
            Vault::create(&dir, b"clave maestra", FAST, "prueba").unwrap_or_else(|e| panic!("{e}"));
        let w = created.words.clone();
        let v = created.vault;
        let item = v
            .add("dni.pdf", &mut (&b"contenido secreto"[..]), 17, "prueba")
            .unwrap_or_else(|e| panic!("{e}"));
        let l = v.list().unwrap_or_default();
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].meta.nombre, "dni.pdf");
        let mut out = Vec::new();
        v.extract(&item.id, &mut out, "prueba")
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(out, b"contenido secreto");
        drop(v);
        assert!(matches!(
            Vault::open(&dir, b"otra", "prueba"),
            Err(Error::WrongKey)
        ));
        let v2 = Vault::open(&dir, b"clave maestra", "prueba").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(v2.list().unwrap_or_default().len(), 1);
        drop(v2);
        // lost the password: the words open it and set a new one
        let mut v3 = Vault::open_with_words(&dir, &w, "prueba").unwrap_or_else(|e| panic!("{e}"));
        v3.set_password(b"nueva", "prueba")
            .unwrap_or_else(|e| panic!("{e}"));
        let mut out = Vec::new();
        v3.extract("dni.pdf", &mut out, "prueba")
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(out, b"contenido secreto");
        drop(v3);
        let v4 = Vault::open(&dir, b"nueva", "prueba").unwrap_or_else(|e| panic!("{e}"));
        // pieces round trip
        let pieces = v4.pieces(&w, "prueba").unwrap_or_default();
        assert_eq!(pieces.len(), 5);
        let sel: Vec<&str> = vec![&pieces[4], &pieces[1], &pieces[2]];
        assert_eq!(words_from_pieces(&sel).unwrap_or_default(), w);
        // capsule
        let cap = v4
            .capsule(&tmp("cap"), "prueba")
            .unwrap_or_else(|e| panic!("{e}"));
        let v5 = Vault::open(&cap, b"nueva", "prueba").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(v5.list().unwrap_or_default()[0].meta.nombre, "dni.pdf");
        // the log holds and names every action
        let (entries, check) = Vault::log(&dir).unwrap_or_else(|e| panic!("{e}"));
        assert!(check.is_ok());
        let acts: Vec<&str> = entries.iter().map(|e| e.accion.as_str()).collect();
        assert_eq!(
            acts,
            [
                "crear",
                "guardar",
                "leer",
                "fallo",
                "abrir",
                "recuperar",
                "contrasena",
                "leer",
                "abrir",
                "trozos",
                "capsula"
            ]
        );
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(cap.parent().unwrap_or(&cap));
    }
}
