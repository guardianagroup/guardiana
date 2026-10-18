//! «Vigilancia» (1.0.45): what the constant, read-only watch over this computer noticed, in a
//! chain of its own. The owner, 18 Oct 2026: «la experiencia del cliente debe ser sentir el
//! monitoreo constante… para saber que todo está bien y nada está vigilando o extrayendo sus
//! datos».
//!
//! These are not DNS queries, so they do not go into `events` (whose rows every report reads as
//! queries); they get their own table, chained the same way: each row carries the hash of the one
//! before it, the first one hangs from the ledger's genesis (the installer's key), and the hash of
//! the last row written is kept beside the rows, so a deleted, altered or truncated history is
//! reported by [`Ledger::comprueba_vigilancia`].
//!
//! What a row holds: when, which watch (`camara_microfono`, `arranque`, `privacidad`,
//! `proteccion`), a key that identifies the fact (so the same fact is not written twice), how much
//! it matters (`aviso` or `info`), the sentence in Spanish for the chain, and what the screen needs
//! to say it in the person's language. Never a content, never an address.
//!
//! `row_hash = sha256("guardiana-vigilancia/1" || prev_hash || fields)`, every field as a 4-byte
//! big-endian length and its UTF-8 bytes, in this order: `ts` (decimal), `tipo`, `clave`,
//! `importancia`, `texto`, `datos` (the JSON exactly as stored). The prefix keeps a row of this
//! chain from ever passing for a row of `events`.

use rusqlite::params;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::error::Result;
use crate::hash::Hash;
use crate::ledger::{Ledger, KEY_GENESIS};

/// Settings key: `row_hash` of the last row written, as hex.
pub(crate) const KEY_V_HEAD: &str = "vigilancia_head";
/// Settings key: the hash the first surviving row hangs from (the genesis until rows are pruned).
pub(crate) const KEY_V_ANCHOR: &str = "vigilancia_anchor";
/// Settings key prefix for what the watch remembers between looks (its baseline). Wiped with the
/// rest by «Borrar todo».
pub const PREFIJO_AJUSTES: &str = "vigilancia_";

/// The most rows kept. Older ones are pruned from the start, and the anchor moves with them, so
/// what remains still verifies. At one row per real change this is years of a normal computer.
pub const MAXIMO_FILAS: i64 = 20_000;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS vigilancia (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    ts          INTEGER NOT NULL,
    tipo        TEXT    NOT NULL,
    clave       TEXT    NOT NULL,
    importancia TEXT    NOT NULL,
    texto       TEXT    NOT NULL,
    datos       TEXT    NOT NULL DEFAULT '{}',
    prev_hash   BLOB    NOT NULL,
    row_hash    BLOB    NOT NULL
);
CREATE INDEX IF NOT EXISTS vigilancia_ts ON vigilancia (ts);
";

/// One fact the watch noticed, about to be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NuevoSuceso {
    /// Unix ms when it was noticed.
    pub ts: i64,
    /// Which watch: `camara_microfono`, `arranque`, `privacidad`, `proteccion`.
    pub tipo: String,
    /// What identifies the fact, so it is written once (`run|HKU\S-1-5-21-…|OneDrive`).
    pub clave: String,
    /// `aviso` (worth a calm notice) or `info` (history only).
    pub importancia: String,
    /// The sentence in Spanish, for the chain and for anyone who reads the file.
    pub texto: String,
    /// What the screen needs to say it in the person's language: `{"frase": key, "datos": {…}}`.
    pub datos: serde_json::Value,
}

/// One fact as written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Suceso {
    /// Row id, increasing.
    pub id: i64,
    /// Unix ms.
    pub ts: i64,
    /// Which watch.
    pub tipo: String,
    /// What identifies the fact.
    pub clave: String,
    /// `aviso` or `info`.
    pub importancia: String,
    /// The sentence in Spanish.
    pub texto: String,
    /// What the screen needs.
    pub datos: serde_json::Value,
    /// This row's hash, hex.
    pub hash: String,
}

/// The hash of one row, as the module's documentation describes it.
#[must_use]
pub fn hash_de(
    prev: &Hash,
    ts: i64,
    tipo: &str,
    clave: &str,
    importancia: &str,
    texto: &str,
    datos: &str,
) -> Hash {
    let mut h = Sha256::new();
    h.update(b"guardiana-vigilancia/1");
    h.update(prev.0);
    let ts = ts.to_string();
    for campo in [ts.as_str(), tipo, clave, importancia, texto, datos] {
        let largo = u32::try_from(campo.len()).unwrap_or(u32::MAX);
        h.update(largo.to_be_bytes());
        h.update(campo.as_bytes());
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&h.finalize());
    Hash(out)
}

/// Cuts a text to `n` characters, on a character boundary.
fn corta(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

impl Ledger {
    pub(crate) fn ensure_vigilancia_table(&self) -> Result<()> {
        self.conn.execute_batch(SCHEMA)?;
        Ok(())
    }

    fn vigilancia_hash(&self, clave: &str) -> Result<Hash> {
        let v = match self.setting(clave)? {
            Some(v) => v,
            None => self.setting(KEY_GENESIS)?.unwrap_or_default(),
        };
        Hash::from_hex(&v)
    }

    /// Writes one fact at the end of the watch's chain.
    ///
    /// # Errors
    /// SQLite failures, or a stored head that is not a hash.
    pub fn anota_vigilancia(&self, n: &NuevoSuceso) -> Result<Suceso> {
        let tipo = corta(&n.tipo, 40);
        let clave = corta(&n.clave, 600);
        let importancia = corta(&n.importancia, 16);
        let texto = corta(&n.texto, 600);
        let datos = serde_json::to_string(&n.datos)?;
        let tx = self.conn.unchecked_transaction()?;
        let prev = self.vigilancia_hash(KEY_V_HEAD)?;
        let row = hash_de(&prev, n.ts, &tipo, &clave, &importancia, &texto, &datos);
        tx.execute(
            "INSERT INTO vigilancia (ts, tipo, clave, importancia, texto, datos, prev_hash, row_hash) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                n.ts,
                tipo,
                clave,
                importancia,
                texto,
                datos,
                prev.0.as_slice(),
                row.0.as_slice()
            ],
        )?;
        let id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO settings(key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![KEY_V_HEAD, row.to_hex()],
        )?;
        // Prune from the start beyond the maximum; the anchor becomes the last pruned row's hash.
        let total: i64 = tx.query_row("SELECT COUNT(*) FROM vigilancia", [], |r| r.get(0))?;
        if total > MAXIMO_FILAS {
            let sobra = total - MAXIMO_FILAS;
            let ultimo: (i64, Vec<u8>) = tx.query_row(
                "SELECT id, row_hash FROM vigilancia ORDER BY id LIMIT 1 OFFSET ?1",
                [sobra - 1],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            tx.execute("DELETE FROM vigilancia WHERE id <= ?1", [ultimo.0])?;
            tx.execute(
                "INSERT INTO settings(key, value) VALUES (?1, ?2) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![KEY_V_ANCHOR, Hash::from_bytes(&ultimo.1)?.to_hex()],
            )?;
        }
        tx.commit()?;
        Ok(Suceso {
            id,
            ts: n.ts,
            tipo,
            clave,
            importancia,
            texto,
            datos: n.datos.clone(),
            hash: row.to_hex(),
        })
    }

    /// The latest facts, newest first, from `desde` (Unix ms) when given.
    ///
    /// # Errors
    /// SQLite failures.
    pub fn vigilancia(&self, limite: usize, desde: Option<i64>) -> Result<Vec<Suceso>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, ts, tipo, clave, importancia, texto, datos, row_hash FROM vigilancia \
             WHERE ts >= ?1 ORDER BY id DESC LIMIT ?2",
        )?;
        let limite = i64::try_from(limite).unwrap_or(i64::MAX);
        let filas = stmt.query_map(params![desde.unwrap_or(i64::MIN), limite], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, Vec<u8>>(7)?,
            ))
        })?;
        let mut out = Vec::new();
        for f in filas {
            let (id, ts, tipo, clave, importancia, texto, datos, hash) = f?;
            out.push(Suceso {
                id,
                ts,
                tipo,
                clave,
                importancia,
                texto,
                datos: serde_json::from_str(&datos).unwrap_or(serde_json::Value::Null),
                hash: Hash::from_bytes(&hash)
                    .map(Hash::to_hex)
                    .unwrap_or_default(),
            });
        }
        Ok(out)
    }

    /// Walks the watch's chain from its anchor to its recorded head. `None` when it verifies;
    /// otherwise the id of the first row that does not (0 when the rows are fine but the end
    /// recorded is not the last row: rows were removed from the end).
    ///
    /// # Errors
    /// SQLite failures.
    pub fn comprueba_vigilancia(&self) -> Result<Option<i64>> {
        let mut prev = self.vigilancia_hash(KEY_V_ANCHOR)?;
        let mut stmt = self.conn.prepare(
            "SELECT id, ts, tipo, clave, importancia, texto, datos, prev_hash, row_hash \
             FROM vigilancia ORDER BY id",
        )?;
        let mut filas = stmt.query([])?;
        while let Some(r) = filas.next()? {
            let id: i64 = r.get(0)?;
            let guardado_prev = Hash::from_bytes(&r.get::<_, Vec<u8>>(7)?)?;
            let guardado = Hash::from_bytes(&r.get::<_, Vec<u8>>(8)?)?;
            let calculado = hash_de(
                &prev,
                r.get(1)?,
                &r.get::<_, String>(2)?,
                &r.get::<_, String>(3)?,
                &r.get::<_, String>(4)?,
                &r.get::<_, String>(5)?,
                &r.get::<_, String>(6)?,
            );
            if guardado_prev != prev || guardado != calculado {
                return Ok(Some(id));
            }
            prev = guardado;
        }
        let cabeza = self.vigilancia_hash(KEY_V_HEAD)?;
        Ok((cabeza != prev).then_some(0))
    }

    /// What the watch remembers under `nombre` (its baseline), as JSON text.
    ///
    /// # Errors
    /// SQLite failures.
    pub fn ajuste_vigilancia(&self, nombre: &str) -> Result<Option<String>> {
        self.setting(&format!("{PREFIJO_AJUSTES}{nombre}"))
    }

    /// Keeps what the watch remembers under `nombre`.
    ///
    /// # Errors
    /// SQLite failures.
    pub fn pon_ajuste_vigilancia(&self, nombre: &str, valor: &str) -> Result<()> {
        self.set_setting(&format!("{PREFIJO_AJUSTES}{nombre}"), valor)
    }

    /// Everything of the watch, gone: its rows, its chain's ends and its baseline. Part of
    /// [`Ledger::wipe`].
    pub(crate) fn borra_vigilancia(tx: &rusqlite::Transaction<'_>) -> Result<()> {
        tx.execute_batch(
            "DELETE FROM vigilancia; DELETE FROM sqlite_sequence WHERE name = 'vigilancia';",
        )?;
        tx.execute(
            "DELETE FROM settings WHERE key LIKE ?1",
            [format!("{PREFIJO_AJUSTES}%")],
        )?;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn suceso(ts: i64, clave: &str) -> NuevoSuceso {
        NuevoSuceso {
            ts,
            tipo: "arranque".into(),
            clave: clave.into(),
            importancia: "aviso".into(),
            texto: format!("Nuevo programa que arranca solo: {clave}"),
            datos: serde_json::json!({ "frase": "vg_ev_arranque_nuevo", "datos": { "x": clave } }),
        }
    }

    #[test]
    fn the_chain_verifies_and_reads_newest_first() {
        let l = Ledger::open_in_memory(Hash::of(b"v")).unwrap();
        assert_eq!(l.comprueba_vigilancia().unwrap(), None);
        l.anota_vigilancia(&suceso(10, "a")).unwrap();
        let b = l.anota_vigilancia(&suceso(20, "b")).unwrap();
        assert_eq!(l.comprueba_vigilancia().unwrap(), None);
        let v = l.vigilancia(10, None).unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].clave, "b");
        assert_eq!(v[0].hash, b.hash);
        assert_eq!(v[0].datos["frase"], "vg_ev_arranque_nuevo");
        assert_eq!(l.vigilancia(10, Some(15)).unwrap().len(), 1);
    }

    #[test]
    fn an_altered_or_removed_row_is_reported() {
        let l = Ledger::open_in_memory(Hash::of(b"v")).unwrap();
        for (i, c) in ["a", "b", "c"].iter().enumerate() {
            l.anota_vigilancia(&suceso(i64::try_from(i).unwrap(), c))
                .unwrap();
        }
        l.connection()
            .execute("UPDATE vigilancia SET texto = 'nada' WHERE id = 2", [])
            .unwrap();
        assert_eq!(l.comprueba_vigilancia().unwrap(), Some(2));
        let m = Ledger::open_in_memory(Hash::of(b"v")).unwrap();
        for (i, c) in ["a", "b", "c"].iter().enumerate() {
            m.anota_vigilancia(&suceso(i64::try_from(i).unwrap(), c))
                .unwrap();
        }
        m.connection()
            .execute("DELETE FROM vigilancia WHERE id = 3", [])
            .unwrap();
        assert_eq!(
            m.comprueba_vigilancia().unwrap(),
            Some(0),
            "the newest row gone"
        );
    }

    #[test]
    fn a_row_of_this_chain_never_hashes_like_another_field_split() {
        let g = Hash::of(b"g");
        assert_ne!(
            hash_de(&g, 1, "ab", "c", "aviso", "t", "{}"),
            hash_de(&g, 1, "a", "bc", "aviso", "t", "{}")
        );
    }

    #[test]
    fn wiping_the_ledger_takes_the_watch_and_its_baseline() {
        let mut l = Ledger::open_in_memory(Hash::of(b"v")).unwrap();
        l.anota_vigilancia(&suceso(1, "a")).unwrap();
        l.pon_ajuste_vigilancia("base", "{\"x\":1}").unwrap();
        l.wipe().unwrap();
        assert!(l.vigilancia(10, None).unwrap().is_empty());
        assert_eq!(l.ajuste_vigilancia("base").unwrap(), None);
        assert_eq!(l.comprueba_vigilancia().unwrap(), None);
        l.anota_vigilancia(&suceso(2, "b")).unwrap();
        assert_eq!(l.comprueba_vigilancia().unwrap(), None);
    }
}
