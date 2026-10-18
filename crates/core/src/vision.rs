//! «Visión profunda» (1.0.45): what each program does on the network, seen from outside, without
//! opening anything.
//!
//! The owner, 18 Oct 2026: «precisión máxima de todos los datos hacia dónde van, vienen desde
//! cualquier país del mundo» and «la experiencia del cliente debe ser sentir el monitoreo
//! constante… para saber que todo está bien y nada está vigilando o extrayendo sus datos».
//!
//! Three things live here, all pure and all local:
//!
//! - [`Detector`]: the «posible transferencia» correlation. A program opened or read files from
//!   the person's own folders (names only, from Windows' file event channel, never content) and,
//!   within a short window, sent a sustained burst to one company outside the house. That is
//!   said as **possible**, with the figures, never as «it stole» or «it uploaded your files»:
//!   GUARDIANA sees that both things happened close together, not what travelled.
//! - `vision_transferencias`: those warnings, with the file names, kept **24 hours** on this
//!   computer and never in a shared export. Turning the file watch off deletes the names at once.
//! - `vision_resumen`: one row per hour, program and company with the bytes each way, the
//!   retransmitted bytes, the connections, the bytes to addresses no name was seen for, and how
//!   many possible transfers there were. These rows are **hash-chained** like the events
//!   (`row_hash = sha256(prev_hash || fields)`), so the aggregate record cannot be edited after
//!   the fact without the check saying where. No address and no file name ever enters them.
//!
//! Addresses are never stored here either (decision 203): the caller turns them into a company
//! and a country before anything reaches this module.

use std::collections::{HashMap, VecDeque};

use rusqlite::{params, OptionalExtension};
use serde::Serialize;

use crate::error::Result;
use crate::hash::Hash;
use crate::ledger::{Ledger, KEY_GENESIS};
use crate::time::{DAY_MS, HOUR_MS};

/// How long after the first file a burst still counts as «close together».
pub const VENTANA_MS: i64 = 2 * 60_000;
/// The network totals arrive every few seconds, measured at the end of each interval: a send
/// measured up to this long after the window closed may still belong to it.
pub const HOLGURA_MS: i64 = 10_000;
/// The smallest burst worth a warning: 4 MiB sent to one company inside the window. Below
/// that, a program's ordinary chatter (telemetry, a licence check) would ring all day.
pub const UMBRAL_BYTES: u64 = 4 * 1024 * 1024;
/// A burst is an upload when it sent at least this many times what it received. A video call or
/// a download is not «sending files away», and must not ring.
pub const PROPORCION_SUBIDA: u64 = 2;
/// File names kept per warning; the count says how many there were in all.
pub const MAX_ARCHIVOS: usize = 20;
/// The warnings with file names are kept this long, then deleted (names are sensitive).
pub const RETENCION_TRANSFERENCIAS_MS: i64 = DAY_MS;
/// The hourly chained summary is kept this long.
pub const RETENCION_RESUMEN_MS: i64 = 90 * DAY_MS;
/// At most this many (program, company) rows per hour in the summary; the rest add up into one
/// row whose company is [`OTRAS`].
pub const MAX_FILAS_HORA: usize = 60;
/// The company of the row that gathers everything past [`MAX_FILAS_HORA`].
pub const OTRAS: &str = "@otras";

/// Setting: whether the person turned on the file-name watch for «posible transferencia». Absent
/// means **off**: brief §1 keeps «qué archivos abre un programa» out unless the person asks for
/// it, and the page asks.
pub const AJUSTE_ARCHIVOS: &str = "vision_archivos";
/// Setting: the start of the first hour not yet closed into the summary.
const AJUSTE_HASTA: &str = "vision_hasta";
/// Setting: the hash the first surviving summary row chains from (moves when old rows go).
const AJUSTE_ANCLA: &str = "vision_ancla";

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS vision_transferencias (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    ts          INTEGER NOT NULL,
    desde       INTEGER NOT NULL,
    programa    TEXT    NOT NULL,
    empresa     TEXT    NOT NULL,
    pais        TEXT    NOT NULL DEFAULT '',
    enviados    INTEGER NOT NULL,
    recibidos   INTEGER NOT NULL DEFAULT 0,
    n_archivos  INTEGER NOT NULL,
    archivos    TEXT    NOT NULL DEFAULT '[]',
    leidos      INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS vision_transferencias_ts ON vision_transferencias (ts);
CREATE TABLE IF NOT EXISTS vision_resumen (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    hora            INTEGER NOT NULL,
    programa        TEXT    NOT NULL,
    empresa         TEXT    NOT NULL,
    pais            TEXT    NOT NULL,
    enviados        INTEGER NOT NULL,
    recibidos       INTEGER NOT NULL,
    reenviados      INTEGER NOT NULL,
    conexiones      INTEGER NOT NULL,
    sin_dns         INTEGER NOT NULL,
    transferencias  INTEGER NOT NULL,
    prev_hash       BLOB    NOT NULL,
    row_hash        BLOB    NOT NULL
);
CREATE INDEX IF NOT EXISTS vision_resumen_hora ON vision_resumen (hora);
";

/// One file a program opened in the person's folders, as Windows' file channel said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lectura {
    /// Unix ms.
    pub ts: i64,
    /// The program's file name (`MsMpEng.exe`).
    pub programa: String,
    /// The file's own name, without its folder (`informe.pdf`).
    pub archivo: String,
    /// Bytes the program read from it, as far as the channel said; 0 when only the opening was
    /// seen.
    pub bytes: u64,
}

/// What one program sent to one company in one take of the network totals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envio {
    /// Unix ms, the end of the interval it was measured in.
    pub ts: i64,
    /// The program's file name.
    pub programa: String,
    /// The company, as the attribution layer named it; empty when nobody knows.
    pub empresa: String,
    /// Two-letter country, empty when unknown.
    pub pais: String,
    /// Bytes sent.
    pub enviados: u64,
    /// Bytes received.
    pub recibidos: u64,
}

/// A «posible transferencia»: files opened, then a burst out to one company, close together.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PosibleTransferencia {
    /// Row id, 0 before it is written.
    pub id: i64,
    /// When the burst reached the threshold (Unix ms).
    pub ts: i64,
    /// When the first of the files was opened.
    pub desde: i64,
    /// The program.
    pub programa: String,
    /// The company it sent to; empty when unknown.
    pub empresa: String,
    /// Its country, empty when unknown.
    pub pais: String,
    /// Bytes sent to that company inside the window.
    pub enviados: u64,
    /// Bytes received from it inside the window.
    pub recibidos: u64,
    /// How many different files it opened before.
    pub n_archivos: usize,
    /// The first [`MAX_ARCHIVOS`] of their names. Local only, 24 hours.
    pub archivos: Vec<String>,
    /// Bytes it read from them, as far as Windows said (0: only the openings were seen).
    pub leidos: u64,
}

/// Bounds of the detector's memory.
const MAX_PROGRAMAS: usize = 512;
const MAX_LECTURAS: usize = 1_024;
const MAX_ENVIOS: usize = 256;

/// The «posible transferencia» correlation, in memory. Fed with file openings and network takes,
/// asked every few seconds.
#[derive(Debug, Default)]
pub struct Detector {
    /// Program (lowercase) → its recent file openings, oldest first.
    lecturas: HashMap<String, VecDeque<Lectura>>,
    /// (program lowercase, company) → its recent sends, oldest first.
    envios: HashMap<(String, String), VecDeque<Envio>>,
    /// (program lowercase, company) → when it was last warned: nothing before that is used again.
    avisado: HashMap<(String, String), i64>,
}

impl Detector {
    /// A new, empty detector.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A file a program opened.
    pub fn lectura(&mut self, l: Lectura) {
        if self.lecturas.len() >= MAX_PROGRAMAS && !self.lecturas.contains_key(&clave(&l.programa))
        {
            return;
        }
        let v = self.lecturas.entry(clave(&l.programa)).or_default();
        // The same file opened again: one file, its bytes add up, its first time stays.
        if let Some(previa) = v.iter_mut().rev().find(|p| p.archivo == l.archivo) {
            if l.ts - previa.ts <= VENTANA_MS {
                previa.bytes = previa.bytes.saturating_add(l.bytes);
                return;
            }
        }
        if v.len() >= MAX_LECTURAS {
            v.pop_front();
        }
        v.push_back(l);
    }

    /// What a program sent to a company in one take. Addresses on the local network and sends
    /// of nothing are not given here by the caller.
    pub fn envio(&mut self, e: Envio) {
        if e.enviados == 0 && e.recibidos == 0 {
            return;
        }
        let k = (clave(&e.programa), e.empresa.clone());
        if self.envios.len() >= MAX_PROGRAMAS * 4 && !self.envios.contains_key(&k) {
            return;
        }
        let v = self.envios.entry(k).or_default();
        if v.len() >= MAX_ENVIOS {
            v.pop_front();
        }
        v.push_back(e);
    }

    /// The possible transfers that completed by `ahora`, each said once. Forgets what is too old
    /// to matter any more.
    pub fn revisa(&mut self, ahora: i64) -> Vec<PosibleTransferencia> {
        let viejo = ahora - 2 * VENTANA_MS - HOLGURA_MS;
        for v in self.lecturas.values_mut() {
            while v.front().is_some_and(|l| l.ts < viejo) {
                v.pop_front();
            }
        }
        self.lecturas.retain(|_, v| !v.is_empty());
        for v in self.envios.values_mut() {
            while v.front().is_some_and(|e| e.ts < viejo) {
                v.pop_front();
            }
        }
        self.envios.retain(|_, v| !v.is_empty());
        self.avisado.retain(|_, t| *t >= viejo);

        let mut out = Vec::new();
        let mut claves: Vec<(String, String)> = self.envios.keys().cloned().collect();
        claves.sort();
        for k in claves {
            let Some(lecturas) = self.lecturas.get(&k.0) else {
                continue;
            };
            let desde_aviso = self.avisado.get(&k).copied().unwrap_or(i64::MIN);
            let Some(envios) = self.envios.get(&k) else {
                continue;
            };
            if let Some(p) = correlaciona(lecturas, envios, desde_aviso) {
                self.avisado.insert(k, p.ts);
                out.push(p);
            }
        }
        out
    }
}

fn clave(programa: &str) -> String {
    programa.to_ascii_lowercase()
}

/// The core rule, on one program and one company: starting at each file opening not yet used,
/// the sends measured after it and within the window; the first start whose sends reach the
/// threshold, as uploads, is the warning.
fn correlaciona(
    lecturas: &VecDeque<Lectura>,
    envios: &VecDeque<Envio>,
    desde_aviso: i64,
) -> Option<PosibleTransferencia> {
    let candidatas: Vec<&Lectura> = lecturas.iter().filter(|l| l.ts > desde_aviso).collect();
    for inicio in &candidatas {
        let fin = inicio.ts + VENTANA_MS + HOLGURA_MS;
        let dentro: Vec<&Envio> = envios
            .iter()
            .filter(|e| e.ts > inicio.ts && e.ts <= fin && e.ts > desde_aviso)
            .collect();
        let enviados: u64 = dentro.iter().map(|e| e.enviados).sum();
        let recibidos: u64 = dentro.iter().map(|e| e.recibidos).sum();
        if enviados < UMBRAL_BYTES || enviados < recibidos.saturating_mul(PROPORCION_SUBIDA) {
            continue;
        }
        // The files are the ones opened from this start until the burst completed.
        let Some(ultimo) = dentro.iter().map(|e| e.ts).max() else {
            continue;
        };
        let primero = dentro.first().copied()?;
        let mut nombres: Vec<String> = Vec::new();
        let mut leidos = 0u64;
        for l in candidatas
            .iter()
            .filter(|l| l.ts >= inicio.ts && l.ts <= ultimo)
        {
            leidos = leidos.saturating_add(l.bytes);
            if !nombres.contains(&l.archivo) {
                nombres.push(l.archivo.clone());
            }
        }
        let n_archivos = nombres.len();
        nombres.truncate(MAX_ARCHIVOS);
        return Some(PosibleTransferencia {
            id: 0,
            ts: ultimo,
            desde: inicio.ts,
            programa: primero.programa.clone(),
            empresa: primero.empresa.clone(),
            pais: dentro
                .iter()
                .map(|e| e.pais.as_str())
                .find(|p| !p.is_empty())
                .unwrap_or("")
                .to_owned(),
            enviados,
            recibidos,
            n_archivos,
            archivos: nombres,
            leidos,
        });
    }
    None
}

/// One hour of one program talking to one company, as the chained summary keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct FilaResumen {
    /// Start of the hour, Unix ms.
    pub hora: i64,
    /// The program's file name.
    pub programa: String,
    /// The company (`@local`, `@otras` and empty are allowed, and said as such by the page).
    pub empresa: String,
    /// Two-letter country, empty when unknown or several.
    pub pais: String,
    /// Bytes sent.
    pub enviados: u64,
    /// Bytes received.
    pub recibidos: u64,
    /// Bytes Windows said were sent again (TCP retransmissions), apart from `enviados`.
    pub reenviados: u64,
    /// Connections opened or accepted.
    pub conexiones: u64,
    /// Of `enviados + recibidos`, the bytes to addresses no name was seen for (a fixed address, the
    /// program's own resolver, or QUIC to an address learned elsewhere).
    pub sin_dns: u64,
    /// Possible transfers in that hour.
    pub transferencias: u64,
}

impl FilaResumen {
    /// The bytes that enter the chain, in a fixed order, each as a 4-byte big-endian length and
    /// its UTF-8 text: anyone can recompute a row from this description.
    fn campos(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for c in [
            self.hora.to_string(),
            self.programa.clone(),
            self.empresa.clone(),
            self.pais.clone(),
            self.enviados.to_string(),
            self.recibidos.to_string(),
            self.reenviados.to_string(),
            self.conexiones.to_string(),
            self.sin_dns.to_string(),
            self.transferencias.to_string(),
        ] {
            let b = c.as_bytes();
            out.extend_from_slice(&u32::try_from(b.len()).unwrap_or(u32::MAX).to_be_bytes());
            out.extend_from_slice(b);
        }
        out
    }

    fn hash(&self, prev: &Hash) -> Hash {
        let mut b = prev.0.to_vec();
        b.extend_from_slice(&self.campos());
        Hash::of(&b)
    }
}

/// What the check of the chained summary found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct CadenaVision {
    /// Rows checked.
    pub filas: u64,
    /// Every row recomputes and links to the one before.
    pub ok: bool,
    /// The id of the first row that does not, when one does not.
    pub rota_en: Option<i64>,
    /// The hash of the last row (the head), hex; the anchor when there are none.
    pub cabeza: String,
}

const MS_HORA: i64 = HOUR_MS;

impl Ledger {
    pub(crate) fn ensure_vision_tables(&self) -> Result<()> {
        self.conn.execute_batch(SCHEMA)?;
        Ok(())
    }

    /// Whether the person turned the file-name watch on.
    pub fn vision_archivos(&self) -> bool {
        matches!(self.setting(AJUSTE_ARCHIVOS), Ok(Some(v)) if v == "1")
    }

    /// Turn the file-name watch on or off. Off deletes every file name already kept, at once.
    pub fn pon_vision_archivos(&self, si: bool) -> Result<()> {
        self.set_setting(AJUSTE_ARCHIVOS, if si { "1" } else { "0" })?;
        if !si {
            self.conn
                .execute("UPDATE vision_transferencias SET archivos = '[]'", [])?;
        }
        Ok(())
    }

    /// Write the possible transfers just found; returns them with their ids.
    pub fn anota_transferencias(
        &self,
        nuevas: &[PosibleTransferencia],
    ) -> Result<Vec<PosibleTransferencia>> {
        let n = |v: u64| i64::try_from(v).unwrap_or(i64::MAX);
        let mut out = Vec::with_capacity(nuevas.len());
        for p in nuevas {
            let archivos = serde_json::to_string(&p.archivos)?;
            self.conn.execute(
                "INSERT INTO vision_transferencias (ts, desde, programa, empresa, pais, enviados, \
                 recibidos, n_archivos, archivos, leidos) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    p.ts,
                    p.desde,
                    p.programa,
                    p.empresa,
                    p.pais,
                    n(p.enviados),
                    n(p.recibidos),
                    i64::try_from(p.n_archivos).unwrap_or(i64::MAX),
                    archivos,
                    n(p.leidos),
                ],
            )?;
            let mut q = p.clone();
            q.id = self.conn.last_insert_rowid();
            out.push(q);
        }
        Ok(out)
    }

    /// The possible transfers since `since`, newest first.
    pub fn transferencias(&self, since: i64) -> Result<Vec<PosibleTransferencia>> {
        let mut st = self.conn.prepare(
            "SELECT id, ts, desde, programa, empresa, pais, enviados, recibidos, n_archivos, \
                    archivos, leidos \
             FROM vision_transferencias WHERE ts >= ?1 ORDER BY ts DESC, id DESC LIMIT 500",
        )?;
        let u = |v: i64| u64::try_from(v).unwrap_or(0);
        let filas = st.query_map([since], |r| {
            let archivos: String = r.get(9)?;
            Ok(PosibleTransferencia {
                id: r.get(0)?,
                ts: r.get(1)?,
                desde: r.get(2)?,
                programa: r.get(3)?,
                empresa: r.get(4)?,
                pais: r.get(5)?,
                enviados: u(r.get(6)?),
                recibidos: u(r.get(7)?),
                n_archivos: usize::try_from(r.get::<_, i64>(8)?).unwrap_or(0),
                archivos: serde_json::from_str(&archivos).unwrap_or_default(),
                leidos: u(r.get(10)?),
            })
        })?;
        Ok(filas.collect::<std::result::Result<_, _>>()?)
    }

    /// The start of the first hour the summary has not closed yet, if any was ever closed.
    pub fn vision_hasta(&self) -> Option<i64> {
        self.setting(AJUSTE_HASTA)
            .ok()
            .flatten()
            .and_then(|v| v.parse().ok())
    }

    fn ancla_vision(&self) -> Result<Hash> {
        let hex = match self.setting(AJUSTE_ANCLA)? {
            Some(h) => h,
            None => self.setting(KEY_GENESIS)?.unwrap_or_default(),
        };
        Hash::from_hex(&hex)
    }

    /// Close hours into the chained summary: append `filas` (all of hours before `hasta`) and
    /// move the mark to `hasta`, in one transaction, so an hour is never written twice nor lost.
    pub fn cierra_horas_vision(&mut self, filas: &[FilaResumen], hasta: i64) -> Result<()> {
        let mut prev = {
            let ultima: Option<Vec<u8>> = self
                .conn
                .query_row(
                    "SELECT row_hash FROM vision_resumen ORDER BY id DESC LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .optional()?;
            match ultima {
                Some(b) => Hash::from_bytes(&b)?,
                None => self.ancla_vision()?,
            }
        };
        let n = |v: u64| i64::try_from(v).unwrap_or(i64::MAX);
        let tx = self.conn.transaction()?;
        {
            let mut st = tx.prepare_cached(
                "INSERT INTO vision_resumen (hora, programa, empresa, pais, enviados, recibidos, \
                 reenviados, conexiones, sin_dns, transferencias, prev_hash, row_hash) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            )?;
            for f in filas {
                let h = f.hash(&prev);
                st.execute(params![
                    f.hora,
                    f.programa,
                    f.empresa,
                    f.pais,
                    n(f.enviados),
                    n(f.recibidos),
                    n(f.reenviados),
                    n(f.conexiones),
                    n(f.sin_dns),
                    n(f.transferencias),
                    prev.0.to_vec(),
                    h.0.to_vec(),
                ])?;
                prev = h;
            }
        }
        tx.execute(
            "INSERT INTO settings(key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![AJUSTE_HASTA, hasta.to_string()],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// The summary rows of a window, oldest first.
    pub fn resumen_vision(&self, since: i64, until: i64) -> Result<Vec<FilaResumen>> {
        let mut st = self.conn.prepare(
            "SELECT hora, programa, empresa, pais, enviados, recibidos, reenviados, conexiones, \
                    sin_dns, transferencias \
             FROM vision_resumen WHERE hora >= ?1 AND hora < ?2 ORDER BY id ASC",
        )?;
        let u = |v: i64| u64::try_from(v).unwrap_or(0);
        let filas = st.query_map(params![since, until], |r| {
            Ok(FilaResumen {
                hora: r.get(0)?,
                programa: r.get(1)?,
                empresa: r.get(2)?,
                pais: r.get(3)?,
                enviados: u(r.get(4)?),
                recibidos: u(r.get(5)?),
                reenviados: u(r.get(6)?),
                conexiones: u(r.get(7)?),
                sin_dns: u(r.get(8)?),
                transferencias: u(r.get(9)?),
            })
        })?;
        Ok(filas.collect::<std::result::Result<_, _>>()?)
    }

    /// Recompute every summary row and its link to the one before.
    pub fn comprueba_vision(&self) -> Result<CadenaVision> {
        let mut st = self.conn.prepare(
            "SELECT id, hora, programa, empresa, pais, enviados, recibidos, reenviados, \
                    conexiones, sin_dns, transferencias, prev_hash, row_hash \
             FROM vision_resumen ORDER BY id ASC",
        )?;
        let u = |v: i64| u64::try_from(v).unwrap_or(0);
        let filas = st.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                FilaResumen {
                    hora: r.get(1)?,
                    programa: r.get(2)?,
                    empresa: r.get(3)?,
                    pais: r.get(4)?,
                    enviados: u(r.get(5)?),
                    recibidos: u(r.get(6)?),
                    reenviados: u(r.get(7)?),
                    conexiones: u(r.get(8)?),
                    sin_dns: u(r.get(9)?),
                    transferencias: u(r.get(10)?),
                },
                r.get::<_, Vec<u8>>(11)?,
                r.get::<_, Vec<u8>>(12)?,
            ))
        })?;
        let mut esperado = self.ancla_vision()?;
        let mut out = CadenaVision {
            ok: true,
            ..CadenaVision::default()
        };
        for f in filas {
            let (id, fila, prev, row) = f?;
            out.filas += 1;
            let prev = Hash::from_bytes(&prev)?;
            let row = Hash::from_bytes(&row)?;
            if out.ok && (prev != esperado || fila.hash(&prev) != row) {
                out.ok = false;
                out.rota_en = Some(id);
            }
            esperado = row;
        }
        out.cabeza = esperado.to_hex();
        Ok(out)
    }

    /// Delete the warnings older than a day and the summary older than its retention, moving the
    /// summary's anchor to the last row removed so the check keeps working on what remains.
    pub fn poda_vision(&mut self, now: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM vision_transferencias WHERE ts < ?1",
            [now - RETENCION_TRANSFERENCIAS_MS],
        )?;
        let corte = now - RETENCION_RESUMEN_MS;
        let ultima: Option<(i64, Vec<u8>)> = self
            .conn
            .query_row(
                "SELECT id, row_hash FROM vision_resumen WHERE hora < ?1 ORDER BY id DESC LIMIT 1",
                [corte],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((id, hash)) = ultima {
            let hex = Hash::from_bytes(&hash)?.to_hex();
            let tx = self.conn.transaction()?;
            tx.execute("DELETE FROM vision_resumen WHERE id <= ?1", [id])?;
            tx.execute(
                "INSERT INTO settings(key, value) VALUES (?1, ?2) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![AJUSTE_ANCLA, hex],
            )?;
            tx.commit()?;
        }
        Ok(())
    }

    /// Everything «Visión profunda» kept, gone (with «Borrar todo»).
    pub(crate) fn borra_vision_en(tx: &rusqlite::Transaction<'_>) -> Result<()> {
        tx.execute_batch(
            "DELETE FROM vision_transferencias; DELETE FROM vision_resumen; \
             DELETE FROM settings WHERE key IN ('vision_hasta', 'vision_ancla');",
        )?;
        Ok(())
    }
}

/// Start of the hour a Unix ms falls in.
#[must_use]
pub const fn hora_de(ts: i64) -> i64 {
    ts - ts.rem_euclid(MS_HORA)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const MB: u64 = 1024 * 1024;

    fn lee(ts: i64, programa: &str, archivo: &str) -> Lectura {
        Lectura {
            ts,
            programa: programa.into(),
            archivo: archivo.into(),
            bytes: 0,
        }
    }

    fn manda(ts: i64, programa: &str, empresa: &str, env: u64, rec: u64) -> Envio {
        Envio {
            ts,
            programa: programa.into(),
            empresa: empresa.into(),
            pais: "US".into(),
            enviados: env,
            recibidos: rec,
        }
    }

    #[test]
    fn files_then_a_sustained_upload_to_one_company_is_one_possible_transfer() {
        let mut d = Detector::new();
        let t = 1_000_000;
        d.lectura(lee(t, "Sync.exe", "informe.pdf"));
        d.lectura(lee(t + 1_000, "sync.exe", "fotos.zip"));
        d.lectura(Lectura {
            bytes: 10,
            ..lee(t + 2_000, "Sync.exe", "informe.pdf")
        });
        for i in 1..=6 {
            d.envio(manda(t + i * 10_000, "Sync.exe", "Acme", 7 * MB, 100_000));
        }
        let p = d.revisa(t + 70_000);
        assert_eq!(p.len(), 1);
        let p = &p[0];
        assert_eq!(p.programa, "Sync.exe");
        assert_eq!(p.empresa, "Acme");
        assert_eq!(p.pais, "US");
        assert_eq!(p.n_archivos, 2, "the same file opened twice is one file");
        assert_eq!(p.archivos, vec!["informe.pdf", "fotos.zip"]);
        assert_eq!(p.leidos, 10);
        assert_eq!(p.enviados, 42 * MB);
        assert_eq!(p.desde, t);
        // Said once: asked again, nothing new.
        assert!(d.revisa(t + 80_000).is_empty());
    }

    #[test]
    fn a_download_a_small_send_or_another_program_never_rings() {
        let mut d = Detector::new();
        let t = 5_000_000;
        d.lectura(lee(t, "a.exe", "x.docx"));
        // A download: much more received than sent.
        d.envio(manda(t + 10_000, "a.exe", "Acme", 5 * MB, 50 * MB));
        // Another program's upload.
        d.envio(manda(t + 10_000, "b.exe", "Acme", 50 * MB, 0));
        assert!(d.revisa(t + 20_000).is_empty());
        // A small send.
        let mut d = Detector::new();
        d.lectura(lee(t, "a.exe", "x.docx"));
        d.envio(manda(t + 10_000, "a.exe", "Acme", UMBRAL_BYTES - 1, 0));
        assert!(d.revisa(t + 20_000).is_empty());
    }

    #[test]
    fn a_send_before_the_files_or_after_the_window_does_not_count() {
        let mut d = Detector::new();
        let t = 9_000_000;
        d.envio(manda(t - 5_000, "a.exe", "Acme", 50 * MB, 0));
        d.lectura(lee(t, "a.exe", "x.docx"));
        d.envio(manda(
            t + VENTANA_MS + HOLGURA_MS + 1,
            "a.exe",
            "Acme",
            50 * MB,
            0,
        ));
        assert!(d.revisa(t + VENTANA_MS + HOLGURA_MS + 2).is_empty());
    }

    #[test]
    fn the_burst_is_per_company_not_the_sum_of_several() {
        let mut d = Detector::new();
        let t = 2_000_000;
        d.lectura(lee(t, "a.exe", "x.docx"));
        d.envio(manda(t + 10_000, "a.exe", "Acme", 3 * MB, 0));
        d.envio(manda(t + 10_000, "a.exe", "Otra", 3 * MB, 0));
        assert!(d.revisa(t + 20_000).is_empty());
    }

    #[test]
    fn old_things_are_forgotten_and_memory_stays_bounded() {
        let mut d = Detector::new();
        for i in 0..5_000 {
            d.lectura(lee(i, "a.exe", &format!("f{i}.txt")));
        }
        assert!(d.lecturas["a.exe"].len() <= MAX_LECTURAS);
        d.revisa(10 * VENTANA_MS);
        assert!(d.lecturas.is_empty());
    }

    #[test]
    fn many_files_keep_the_first_names_and_the_whole_count() {
        let mut d = Detector::new();
        let t = 3_000_000;
        for i in 0..30 {
            d.lectura(lee(t + i, "a.exe", &format!("f{i}.txt")));
        }
        d.envio(manda(t + 10_000, "a.exe", "Acme", 9 * MB, 0));
        let p = d.revisa(t + 10_000);
        assert_eq!(p[0].n_archivos, 30);
        assert_eq!(p[0].archivos.len(), MAX_ARCHIVOS);
    }

    fn fila(hora: i64, programa: &str, empresa: &str, env: u64) -> FilaResumen {
        FilaResumen {
            hora,
            programa: programa.into(),
            empresa: empresa.into(),
            pais: "US".into(),
            enviados: env,
            recibidos: 1,
            ..FilaResumen::default()
        }
    }

    #[test]
    fn the_hourly_summary_is_chained_and_an_edit_is_found() {
        let mut l = Ledger::open_in_memory(Hash::of(b"t")).unwrap();
        let h = 100 * HOUR_MS;
        l.cierra_horas_vision(
            &[fila(h, "a.exe", "Acme", 5), fila(h, "b.exe", "Otra", 7)],
            h + HOUR_MS,
        )
        .unwrap();
        l.cierra_horas_vision(&[fila(h + HOUR_MS, "a.exe", "Acme", 9)], h + 2 * HOUR_MS)
            .unwrap();
        assert_eq!(l.vision_hasta(), Some(h + 2 * HOUR_MS));
        let c = l.comprueba_vision().unwrap();
        assert!(c.ok);
        assert_eq!(c.filas, 3);
        assert_eq!(l.resumen_vision(h, h + HOUR_MS).unwrap().len(), 2);
        l.connection()
            .execute("UPDATE vision_resumen SET enviados = 1 WHERE id = 2", [])
            .unwrap();
        let c = l.comprueba_vision().unwrap();
        assert!(!c.ok);
        assert_eq!(c.rota_en, Some(2));
    }

    #[test]
    fn pruning_the_summary_keeps_the_rest_checkable() {
        let mut l = Ledger::open_in_memory(Hash::of(b"t")).unwrap();
        let h = 1_000 * HOUR_MS;
        l.cierra_horas_vision(&[fila(h, "a.exe", "Acme", 5)], h + HOUR_MS)
            .unwrap();
        l.cierra_horas_vision(
            &[fila(h + 2 * DAY_MS, "a.exe", "Acme", 5)],
            h + 2 * DAY_MS + HOUR_MS,
        )
        .unwrap();
        l.poda_vision(h + RETENCION_RESUMEN_MS + DAY_MS).unwrap();
        let c = l.comprueba_vision().unwrap();
        assert_eq!(c.filas, 1);
        assert!(c.ok);
    }

    #[test]
    fn file_names_last_a_day_and_go_at_once_when_the_watch_is_turned_off() {
        let mut l = Ledger::open_in_memory(Hash::of(b"t")).unwrap();
        assert!(!l.vision_archivos(), "off until the person turns it on");
        l.pon_vision_archivos(true).unwrap();
        assert!(l.vision_archivos());
        let p = PosibleTransferencia {
            id: 0,
            ts: 10 * DAY_MS,
            desde: 10 * DAY_MS - 1,
            programa: "a.exe".into(),
            empresa: "Acme".into(),
            pais: "US".into(),
            enviados: 9,
            recibidos: 0,
            n_archivos: 1,
            archivos: vec!["informe.pdf".into()],
            leidos: 0,
        };
        let escritas = l.anota_transferencias(&[p]).unwrap();
        assert!(escritas[0].id > 0);
        assert_eq!(
            l.transferencias(0).unwrap()[0].archivos,
            vec!["informe.pdf"]
        );
        l.pon_vision_archivos(false).unwrap();
        let t = l.transferencias(0).unwrap();
        assert_eq!(t.len(), 1, "the warning stays, its names do not");
        assert!(t[0].archivos.is_empty());
        l.poda_vision(11 * DAY_MS + 1).unwrap();
        assert!(l.transferencias(0).unwrap().is_empty());
    }
}
