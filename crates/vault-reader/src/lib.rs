//! The independent reader of a GUARDIANA vault (DECISIONES #103: "formato abierto y lector
//! independiente publicado desde el primer día"). It shares no code with the product: everything
//! it needs is the format described in `docs/BOVEDA.md`, Argon2id, XChaCha20-Poly1305 and SHA-256.
//! If GUARDIANA disappeared tomorrow, this file and that document would still open every vault.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Format label of the vaults this reader understands.
pub const FORMAT: &str = "guardiana-boveda/1";

/// What can go wrong, in the reader's own small vocabulary.
#[derive(Debug)]
pub enum Error {
    /// Reading a file failed.
    Io(std::io::Error),
    /// The files are not in the documented format.
    Format(String),
    /// The password or the words did not open the master key.
    WrongKey,
    /// An object failed authentication.
    Tampered(String),
    /// The 24 words are not valid.
    BadWords(String),
    /// No object with that id or name.
    NoObject(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io: {e}"),
            Self::Format(s) => write!(f, "format: {s}"),
            Self::WrongKey => write!(f, "wrong key"),
            Self::Tampered(s) => write!(f, "tampered: {s}"),
            Self::BadWords(s) => write!(f, "words: {s}"),
            Self::NoObject(s) => write!(f, "no object {s}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Format(e.to_string())
    }
}

/// Result alias.
pub type Result<T> = std::result::Result<T, Error>;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Result<Vec<u8>> {
    if s.len() % 2 != 0 {
        return Err(Error::Format("odd hex".into()));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| Error::Format("bad hex".into())))
        .collect()
}

fn field<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    v.get(k)
        .and_then(Value::as_str)
        .ok_or_else(|| Error::Format(format!("missing {k}")))
}

fn num(v: &Value, k: &str) -> Result<u64> {
    v.get(k)
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::Format(format!("missing {k}")))
}

fn open_aead(key: &[u8; 32], nonce: &[u8], ct: &[u8], aad: &str) -> Result<Vec<u8>> {
    let n: [u8; 24] = nonce
        .try_into()
        .map_err(|_| Error::Format("nonce".into()))?;
    XChaCha20Poly1305::new(key.into())
        .decrypt(
            XNonce::from_slice(&n),
            Payload {
                msg: ct,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| Error::Tampered(aad.to_owned()))
}

fn derive(secret: &[u8], salt: &[u8], kdf: &Value) -> Result<[u8; 32]> {
    let m = u32::try_from(num(kdf, "memoria_kib")?).map_err(|_| Error::Format("kdf".into()))?;
    let t = u32::try_from(num(kdf, "pasadas")?).map_err(|_| Error::Format("kdf".into()))?;
    let p = u32::try_from(num(kdf, "hilos")?).map_err(|_| Error::Format("kdf".into()))?;
    let params =
        argon2::Params::new(m, t, p, Some(32)).map_err(|e| Error::Format(e.to_string()))?;
    let a = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    let mut out = [0u8; 32];
    a.hash_password_into(secret, salt, &mut out)
        .map_err(|e| Error::Format(e.to_string()))?;
    Ok(out)
}

/// The vault header, parsed just enough.
pub struct Header {
    /// Vault id.
    pub id: String,
    raw: Value,
}

/// Read and check `boveda.json`.
pub fn header(dir: &Path) -> Result<Header> {
    let raw: Value = serde_json::from_slice(&std::fs::read(dir.join("boveda.json"))?)?;
    if field(&raw, "formato")? != FORMAT {
        return Err(Error::Format(format!(
            "formato {}",
            field(&raw, "formato")?
        )));
    }
    Ok(Header {
        id: field(&raw, "id")?.to_owned(),
        raw,
    })
}

/// The master key from the password (`tipo` = `contrasena`) or the 24 words' entropy (`palabras`).
pub fn master_key(h: &Header, tipo: &str, secret: &[u8]) -> Result<[u8; 32]> {
    let slots = h
        .raw
        .get("llaves")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::Format("llaves".into()))?;
    let slot = slots
        .iter()
        .find(|s| s.get("tipo").and_then(Value::as_str) == Some(tipo))
        .ok_or_else(|| Error::Format(format!("slot {tipo}")))?;
    let kdf = h
        .raw
        .get("kdf")
        .ok_or_else(|| Error::Format("kdf".into()))?;
    let kek = derive(secret, &unhex(field(slot, "sal")?)?, kdf)?;
    let plain = open_aead(
        &kek,
        &unhex(field(slot, "nonce")?)?,
        &unhex(field(slot, "envuelta")?)?,
        &format!("{FORMAT} boveda {} llave {tipo}", h.id),
    )
    .map_err(|_| Error::WrongKey)?;
    plain.try_into().map_err(|_| Error::WrongKey)
}

/// One object as listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// Object id.
    pub id: String,
    /// Original name.
    pub nombre: String,
    /// Stored at (RFC 3339 UTC).
    pub guardado: String,
    /// Plaintext size.
    pub tamano: u64,
    /// SHA-256 of the plaintext, hex.
    pub sha256: String,
}

fn object_header(path: &Path) -> Result<(Value, u64)> {
    let mut f = File::open(path)?;
    let mut magic = [0u8; 8];
    f.read_exact(&mut magic)?;
    if &magic != b"GDNBOV1\n" {
        return Err(Error::Format(format!("{} no es un objeto", path.display())));
    }
    let mut lb = [0u8; 4];
    f.read_exact(&mut lb)?;
    let l = u32::from_be_bytes(lb) as usize;
    let mut hj = vec![0u8; l];
    f.read_exact(&mut hj)?;
    Ok((serde_json::from_slice(&hj)?, 12 + l as u64))
}

fn object_key_and_meta(h: &Value, master: &[u8; 32]) -> Result<([u8; 32], Item)> {
    let id = field(h, "id")?;
    let llave = h
        .get("llave")
        .ok_or_else(|| Error::Format("llave".into()))?;
    let key: [u8; 32] = open_aead(
        master,
        &unhex(field(llave, "nonce")?)?,
        &unhex(field(llave, "envuelta")?)?,
        &format!("{FORMAT} objeto {id} llave"),
    )
    .map_err(|_| Error::WrongKey)?
    .try_into()
    .map_err(|_| Error::WrongKey)?;
    let meta = h.get("meta").ok_or_else(|| Error::Format("meta".into()))?;
    let plain = open_aead(
        &key,
        &unhex(field(meta, "nonce")?)?,
        &unhex(field(meta, "cifrado")?)?,
        &format!("{FORMAT} objeto {id} meta"),
    )?;
    let m: Value = serde_json::from_slice(&plain)?;
    Ok((
        key,
        Item {
            id: id.to_owned(),
            nombre: field(&m, "nombre")?.to_owned(),
            guardado: field(&m, "guardado")?.to_owned(),
            tamano: num(&m, "tamano")?,
            sha256: field(&m, "sha256")?.to_owned(),
        },
    ))
}

/// Every object, newest first.
pub fn list(dir: &Path, master: &[u8; 32]) -> Result<Vec<Item>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir.join("objetos"))? {
        let p = entry?.path();
        if p.extension().and_then(|e| e.to_str()) == Some("gdn") {
            let (h, _) = object_header(&p)?;
            out.push(object_key_and_meta(&h, master)?.1);
        }
    }
    out.sort_by(|a, b| b.guardado.cmp(&a.guardado).then(a.id.cmp(&b.id)));
    Ok(out)
}

/// Decrypt one object (by id or name) into `out`, checking every chunk and the final hash.
pub fn extract(dir: &Path, master: &[u8; 32], which: &str, out: &mut impl Write) -> Result<Item> {
    let item = list(dir, master)?
        .into_iter()
        .find(|i| i.id == which || i.nombre == which)
        .ok_or_else(|| Error::NoObject(which.to_owned()))?;
    let path = dir.join("objetos").join(format!("{}.gdn", item.id));
    let (h, offset) = object_header(&path)?;
    let (key, _) = object_key_and_meta(&h, master)?;
    let trozo = num(&h, "trozo")?;
    let trozos = num(&h, "trozos")?;
    let mut rest = num(&h, "tamano")?;
    let mut f = File::open(&path)?;
    f.seek(SeekFrom::Start(offset))?;
    let mut hasher = Sha256::new();
    for i in 0..trozos {
        let want = usize::try_from(rest.min(trozo)).map_err(|_| Error::Format("size".into()))?;
        let mut buf = vec![0u8; 24 + want + 16];
        f.read_exact(&mut buf)?;
        let plain = open_aead(
            &key,
            &buf[..24],
            &buf[24..],
            &format!("{FORMAT} objeto {} trozo {i}/{trozos}", item.id),
        )?;
        hasher.update(&plain);
        out.write_all(&plain)?;
        rest -= want as u64;
    }
    if hex(&hasher.finalize()) != item.sha256 {
        return Err(Error::Tampered("content hash".into()));
    }
    Ok(item)
}

/// One log line, as printed.
pub struct LogLine {
    /// Line number.
    pub n: u64,
    /// Time.
    pub hora: String,
    /// Action.
    pub accion: String,
    /// Object id, if any.
    pub objeto: String,
    /// Detail.
    pub detalle: String,
}

/// The log lines and, if the chain holds, how many; otherwise the first broken line number.
pub fn log(dir: &Path, vault_id: &str) -> Result<(Vec<LogLine>, std::result::Result<u64, u64>)> {
    let text = std::fs::read_to_string(dir.join("registro.jsonl")).unwrap_or_default();
    let mut prev: [u8; 32] = {
        let mut h = Sha256::new();
        h.update(b"guardiana-boveda/1\n");
        h.update(vault_id.as_bytes());
        h.finalize().into()
    };
    let mut lines = Vec::new();
    let mut check = Ok(0u64);
    for (i, l) in text.lines().filter(|l| !l.trim().is_empty()).enumerate() {
        let v: Value = serde_json::from_str(l)?;
        let n = num(&v, "n")?;
        let objeto = v
            .get("objeto")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let (hora, accion, detalle) = (
            field(&v, "hora")?.to_owned(),
            field(&v, "accion")?.to_owned(),
            field(&v, "detalle")?.to_owned(),
        );
        if check.is_ok() {
            let mut h = Sha256::new();
            h.update(prev);
            for s in [n.to_string().as_str(), &hora, &accion, &objeto, &detalle] {
                h.update((s.len() as u32).to_be_bytes());
                h.update(s.as_bytes());
            }
            let mine: [u8; 32] = h.finalize().into();
            if n != i as u64 + 1
                || field(&v, "anterior")? != hex(&prev)
                || field(&v, "hash")? != hex(&mine)
            {
                check = Err(i as u64 + 1);
            } else {
                prev = mine;
                check = Ok(n);
            }
        }
        lines.push(LogLine {
            n,
            hora,
            accion,
            objeto,
            detalle,
        });
    }
    Ok((lines, check))
}

/// 24 BIP-39 English words → the 32 bytes of entropy (checksum verified).
pub fn entropy_from_words(words: &[&str]) -> Result<[u8; 32]> {
    let list: Vec<&str> = include_str!("english.txt")
        .lines()
        .map(str::trim)
        .filter(|w| !w.is_empty())
        .collect();
    if words.len() != 24 {
        return Err(Error::BadWords(format!(
            "{} palabras, hacen falta 24",
            words.len()
        )));
    }
    let mut bits = Vec::with_capacity(264);
    for w in words {
        let w = w.trim().to_ascii_lowercase();
        let idx = list
            .binary_search(&w.as_str())
            .map_err(|_| Error::BadWords(format!("palabra desconocida «{w}»")))?;
        for i in (0..11).rev() {
            bits.push(((idx >> i) & 1) as u8);
        }
    }
    let mut e = [0u8; 32];
    for (i, c) in bits[..256].chunks(8).enumerate() {
        e[i] = c.iter().fold(0u8, |a, b| (a << 1) | b);
    }
    let check = bits[256..].iter().fold(0u8, |a, b| (a << 1) | b);
    if Sha256::digest(e)[0] != check {
        return Err(Error::BadWords(
            "una palabra está mal o fuera de orden".into(),
        ));
    }
    Ok(e)
}
