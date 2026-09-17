//! The access log (`registro.jsonl`): one JSON line per action, hash-chained like the DNS ledger
//! (`hash = sha256(anterior ‖ fields)`, each field length-prefixed with 4 big-endian bytes).
//! It is not encrypted: it holds times, actions and object ids, never names or content. Deleting
//! or editing a line breaks the chain and `check` says at which line.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::hexs;

/// File name inside the vault directory.
pub const FILE: &str = "registro.jsonl";

/// One line of the log.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    /// 1-based line number.
    pub n: u64,
    /// RFC 3339 UTC.
    pub hora: String,
    /// `crear`, `abrir`, `fallo`, `guardar`, `leer`, `contrasena`, `recuperar`, `trozos`, `capsula`.
    pub accion: String,
    /// Object id when the action is about one.
    pub objeto: Option<String>,
    /// Free text: who asked (terminal, panel) and any detail without secrets.
    pub detalle: String,
    /// Hash of the previous line (or the genesis), hex.
    pub anterior: String,
    /// Hash of this line, hex.
    pub hash: String,
}

/// The chain starts at `sha256("guardiana-boveda/1\n" ‖ vault id)`.
#[must_use]
pub fn genesis(vault_id: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"guardiana-boveda/1\n");
    h.update(vault_id.as_bytes());
    h.finalize().into()
}

fn field(h: &mut Sha256, s: &str) {
    h.update((s.len() as u32).to_be_bytes());
    h.update(s.as_bytes());
}

/// Hash of a line from its previous hash and fields, in this fixed order.
#[must_use]
pub fn entry_hash(
    prev: &[u8; 32],
    n: u64,
    hora: &str,
    accion: &str,
    objeto: Option<&str>,
    detalle: &str,
) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(prev);
    field(&mut h, &n.to_string());
    field(&mut h, hora);
    field(&mut h, accion);
    field(&mut h, objeto.unwrap_or(""));
    field(&mut h, detalle);
    h.finalize().into()
}

/// Every line of the log, in order.
pub fn read_all(dir: &Path) -> Result<Vec<Entry>> {
    let p = dir.join(FILE);
    if !p.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(p)?;
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<Entry>(l).map_err(Error::Json))
        .collect()
}

/// Append one line, chained to the last (or to the genesis of `vault_id`).
pub fn append(
    dir: &Path,
    vault_id: &str,
    hora: &str,
    accion: &str,
    objeto: Option<&str>,
    detalle: &str,
) -> Result<Entry> {
    let all = read_all(dir)?;
    let (n, prev) = match all.last() {
        Some(e) => (e.n + 1, hexs::array::<32>(&e.hash)?),
        None => (1, genesis(vault_id)),
    };
    let hash = entry_hash(&prev, n, hora, accion, objeto, detalle);
    let e = Entry {
        n,
        hora: hora.to_owned(),
        accion: accion.to_owned(),
        objeto: objeto.map(str::to_owned),
        detalle: detalle.to_owned(),
        anterior: hexs::to_hex(&prev),
        hash: hexs::to_hex(&hash),
    };
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(FILE))?;
    f.write_all(serde_json::to_string(&e)?.as_bytes())?;
    f.write_all(b"\n")?;
    f.sync_all()?;
    Ok(e)
}

/// Verify the whole chain; returns the number of lines, or the first line that does not check.
pub fn check(dir: &Path, vault_id: &str) -> Result<u64> {
    let mut prev = genesis(vault_id);
    let mut expect = 1u64;
    for e in read_all(dir)? {
        let ok = e.n == expect
            && e.anterior == hexs::to_hex(&prev)
            && e.hash
                == hexs::to_hex(&entry_hash(
                    &prev,
                    e.n,
                    &e.hora,
                    &e.accion,
                    e.objeto.as_deref(),
                    &e.detalle,
                ));
        if !ok {
            return Err(Error::ChainBroken(expect));
        }
        prev = hexs::array::<32>(&e.hash)?;
        expect += 1;
    }
    Ok(expect - 1)
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn chain_holds_and_breaks() {
        let dir = std::env::temp_dir().join(format!("gdn-log-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap_or_default();
        append(
            &dir,
            "id1",
            "2026-09-17T10:00:00Z",
            "crear",
            None,
            "terminal",
        )
        .unwrap_or_else(|e| panic!("{e}"));
        append(
            &dir,
            "id1",
            "2026-09-17T10:01:00Z",
            "guardar",
            Some("abc"),
            "terminal",
        )
        .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(check(&dir, "id1").unwrap_or_default(), 2);
        assert!(matches!(check(&dir, "otro"), Err(Error::ChainBroken(1))));
        let mut lines: Vec<String> = std::fs::read_to_string(dir.join(FILE))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect();
        lines[0] = lines[0].replace("crear", "abrir");
        std::fs::write(dir.join(FILE), lines.join("\n") + "\n").unwrap_or_default();
        assert!(matches!(check(&dir, "id1"), Err(Error::ChainBroken(1))));
        std::fs::remove_dir_all(&dir).unwrap_or_default();
    }
}
