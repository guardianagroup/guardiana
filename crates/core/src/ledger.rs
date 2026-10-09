//! The SQLite ledger: one database per installation, `events` chained by hash.
//!
//! Chain anchor. Detail rows are pruned after the retention period (brief
//! §3), always as a prefix by id. The `row_hash` of the last pruned row is
//! kept in `settings` as the *anchor*, so the surviving rows still form a
//! verifiable chain that starts at the anchor. Before anything is pruned the
//! anchor is the *genesis*: the hash of the installer's public key.
//!
//! Chain head. The `row_hash` of the last row appended is kept in `settings`
//! too, written in the same transaction as the row. Without it, removing the
//! newest rows left a chain that verified perfectly: every surviving row still
//! linked to the one before it, and nothing recorded where the chain was
//! supposed to end (review of 1 Oct 2026, entry 17). The head is the one piece
//! of state outside the rows; it needs no signature to catch a plain deletion,
//! and it does not pretend to stop whoever rewrites the whole file.

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::error::{Error, Result};
use crate::hash::{chain_hash, Hash, HashInput};
use crate::model::SignalKind;
use crate::model::{
    Action, Category, Device, Event, MatchKind, NewEvent, NewRule, Outbound, Purpose, Rule, Scope,
    Signal, Verdict,
};
use crate::SELF_DEVICE_ID;

/// Settings key: hash of the public key this database was created for.
pub(crate) const KEY_GENESIS: &str = "chain_genesis";
/// Settings key: current chain anchor (genesis until something is pruned).
pub(crate) const KEY_ANCHOR: &str = "chain_anchor";
/// Settings key: how many events were pruned so far (informational).
pub(crate) const KEY_PRUNED: &str = "chain_pruned_events";
/// Settings key: `row_hash` of the last event appended, as hex. The chain's recorded end.
pub(crate) const KEY_HEAD: &str = "chain_head";
/// Settings key: events appended since the genesis (or the last wipe), pruned ones included.
/// With the head's hash it says how many rows are missing at the end. Ledgers written before
/// 1.0.2 have neither key and gain both on their next append.
pub(crate) const KEY_HEAD_COUNT: &str = "chain_head_count";
/// Settings key: id of the first row appended after the head went missing in a file that kept
/// one. Where `check()` says the chain broke when [`SCHEMA_HEAD_LOST`] is set.
pub(crate) const KEY_HEAD_LOST_AT: &str = "chain_head_lost_at";

/// `PRAGMA user_version` of a file that keeps a chain head (1.0.2 on). Ledgers written before
/// have 0 and are the only ones where a missing head is normal. The mark lives in the file
/// header, not in `settings`, so a `DELETE FROM settings` cannot make a 1.0.2 file pass for an
/// old one (review of 5 Oct 2026, privacy item 6).
pub(crate) const SCHEMA_HEAD: i64 = 1;
/// `PRAGMA user_version` once a file that kept a head was found without it. The next append has
/// to write somewhere, but the loss stays on record until the person wipes the ledger: without
/// this mark the resolver's next query would quietly heal the deletion.
pub(crate) const SCHEMA_HEAD_LOST: i64 = 2;

/// SQL condition: the rows the household may read by name. This computer's own rows, and the
/// rows of the devices whose owner turned "share my detail with the home panel" on. The first
/// placeholder is bound to [`SELF_DEVICE_ID`].
///
/// The panel and the published privacy policy promise that the detail of a phone is only seen
/// from that phone unless its owner shares it; until 1.0.1 the flag only painted a label and
/// every listing returned the names anyway (review of 1 Oct 2026, entry 1). A device with no row
/// in `devices` has not consented either, so it is not listed.
const SHARED_WITH_HOME: &str =
    "(device_id = ? OR device_id IN (SELECT id FROM devices WHERE share_detail_with_home != 0))";

const SCHEMA: &str = "
PRAGMA journal_mode = WAL;
-- NORMAL, not the default FULL: with the write-ahead log the file is never left damaged by a
-- crash or a power cut either way; FULL only adds a disk flush to every commit, and every query
-- is a commit. Measured on 5 Oct 2026: 500 queries at once lost 40 % to the waiting. What a
-- power cut can take with NORMAL is the last commits, events and chain head together, so the
-- chain still verifies, and the gap shows as time not watched.
PRAGMA synchronous = NORMAL;
PRAGMA busy_timeout = 5000;
PRAGMA foreign_keys = ON;
-- Zero deleted content when it costs no extra writes. «Borrar todo» does not rely on this: it
-- rebuilds the file with VACUUM and cuts the write-ahead log, which is what leaves no name behind
-- (review of 1 Oct 2026, entry 16). FULL (ON) rewrote every freed page during the wipe, holding
-- the write lock long enough on a big ledger for the resolver's next append to wait on it
-- (review of 5 Oct 2026, privacy item 4). Per connection, hence set at every open.
PRAGMA secure_delete = FAST;
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS events (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    ts           INTEGER NOT NULL,
    device_id    TEXT    NOT NULL,
    client_ip    TEXT    NOT NULL,
    qname        TEXT    NOT NULL,
    qtype        TEXT    NOT NULL,
    category     TEXT    NOT NULL,
    list_source  TEXT    NOT NULL DEFAULT '',
    signals_json TEXT    NOT NULL DEFAULT '[]',
    verdict      TEXT    NOT NULL,
    decided_by   TEXT    NOT NULL,
    rule_id      INTEGER,
    prev_hash    BLOB    NOT NULL,
    row_hash     BLOB    NOT NULL,
    -- El programa que pidió el nombre, cuando el sistema puede decirlo (Windows, este equipo).
    -- Vacío en todo lo demás, que es la mayoría: se guarda el nombre del programa, no lo que hace.
    process_json TEXT    NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS events_ts ON events(ts);
CREATE INDEX IF NOT EXISTS events_device_ts ON events(device_id, ts);
CREATE INDEX IF NOT EXISTS events_device_name ON events(device_id, qname, id);
CREATE TABLE IF NOT EXISTS devices (
    id                     TEXT PRIMARY KEY,
    mac                    TEXT,
    last_ip                TEXT,
    name                   TEXT,
    first_seen             INTEGER NOT NULL,
    last_seen              INTEGER NOT NULL,
    share_detail_with_home INTEGER NOT NULL DEFAULT 0,
    hours_profile_json     TEXT
);
CREATE TABLE IF NOT EXISTS rules (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    scope      TEXT    NOT NULL,
    device_id  TEXT,
    match_kind TEXT    NOT NULL,
    pattern    TEXT    NOT NULL,
    action     TEXT    NOT NULL,
    created_at INTEGER NOT NULL,
    created_by TEXT    NOT NULL,
    expires_at INTEGER,
    undone_at  INTEGER
);
CREATE TABLE IF NOT EXISTS outbound (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    ts                INTEGER NOT NULL,
    purpose           TEXT    NOT NULL,
    host              TEXT    NOT NULL,
    bytes             INTEGER NOT NULL,
    initiated_by_user INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS seen_domains (
    device_id  TEXT    NOT NULL,
    qname      TEXT    NOT NULL,
    first_seen INTEGER NOT NULL,
    PRIMARY KEY (device_id, qname)
);
CREATE TABLE IF NOT EXISTS daily_totals (
    day       TEXT    NOT NULL,
    device_id TEXT    NOT NULL,
    category  TEXT    NOT NULL,
    verdict   TEXT    NOT NULL,
    count     INTEGER NOT NULL,
    PRIMARY KEY (day, device_id, category, verdict)
);
";

const EVENT_COLUMNS: &str = "id, ts, device_id, client_ip, qname, qtype, category, list_source, \
                             signals_json, verdict, decided_by, rule_id, prev_hash, row_hash, \
                             process_json";

/// Handle to the ledger database.
pub struct Ledger {
    pub(crate) conn: Connection,
}

/// Why a row failed verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainFault {
    /// `prev_hash` does not match the previous row's `row_hash` (or the anchor).
    BrokenLink {
        /// What the row claims.
        found: Hash,
        /// What it should be.
        expected: Hash,
    },
    /// `row_hash` does not match the row's own fields: a field was altered.
    AlteredRow {
        /// What the row stores.
        stored: Hash,
        /// Recomputed from the fields.
        recomputed: Hash,
    },
    /// A column holds a value outside the model.
    Unreadable(String),
    /// The chain ends before its recorded end: the newest rows were removed. Reported with the
    /// id that would follow the last surviving row (review of 1 Oct 2026, entry 17).
    Truncated {
        /// Hash of the last row ever appended, which no surviving row carries.
        expected_head: Hash,
        /// How many rows are missing at the end, when the count was kept.
        missing: Option<u64>,
    },
    /// The record of where the chain ends is gone from a file that kept one, so nobody can tell
    /// whether the newest rows were removed. Reported at the first row written after the loss,
    /// or after the last row when nothing was written since (review of 5 Oct 2026).
    HeadMissing,
}

/// Result of `guardiana ledger --check`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckReport {
    /// Rows verified before stopping.
    pub checked: u64,
    /// Rows pruned earlier (their hashes are summarized by the anchor).
    pub pruned: u64,
    /// True when the anchor is still the genesis: nothing was ever pruned.
    pub anchor_is_genesis: bool,
    /// The first bad row, if any: its id and what is wrong with it.
    pub first_fault: Option<(i64, ChainFault)>,
}

impl CheckReport {
    /// True when every surviving row verifies.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.first_fault.is_none()
    }
}

/// Filters for listing or exporting events. Every field is optional.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventFilter {
    /// Only this device.
    pub device_id: Option<String>,
    /// Only this category.
    pub category: Option<Category>,
    /// Only events carrying this signal.
    pub signal: Option<SignalKind>,
    /// Only this exact queried name.
    ///
    /// A trap asks one question of the ledger -- "did anybody ever ask for my name?" -- and
    /// before this field it had to be answered by pulling the last few thousand rows into memory
    /// and looking through them, which missed a bite older than that window.
    pub qname: Option<String>,
    /// Only this verdict.
    pub verdict: Option<Verdict>,
    /// Only `ts >= since`.
    pub since: Option<i64>,
    /// Only `ts < until`.
    pub until: Option<i64>,
    /// At most this many rows: the newest ones, returned oldest first.
    pub limit: Option<u32>,
}

impl Ledger {
    /// Open (or create) the ledger at `path`. `genesis` is the hash of the
    /// installer's public key; a database created for another key is refused.
    pub fn open(path: &Path, genesis: Hash) -> Result<Self> {
        Self::init(Connection::open(path)?, genesis)
    }

    /// An in-memory ledger, for tests and dry runs.
    pub fn open_in_memory(genesis: Hash) -> Result<Self> {
        Self::init(Connection::open_in_memory()?, genesis)
    }

    fn init(conn: Connection, genesis: Hash) -> Result<Self> {
        conn.execute_batch(SCHEMA)?;
        let ledger = Self { conn };
        ledger.ensure_gaps_table()?;
        ledger.ensure_changes_table()?;
        ledger.migrate_rules_confirmed()?;
        ledger.migrate_events_process()?;
        match ledger.setting(KEY_GENESIS)? {
            None => {
                ledger.set_setting(KEY_GENESIS, &genesis.to_hex())?;
                ledger.set_setting(KEY_ANCHOR, &genesis.to_hex())?;
                ledger.set_setting(KEY_PRUNED, "0")?;
                // A new ledger starts with its head at the anchor: nothing written, nothing missing.
                ledger.set_setting(KEY_HEAD, &genesis.to_hex())?;
                ledger.set_setting(KEY_HEAD_COUNT, "0")?;
                set_schema(&ledger.conn, SCHEMA_HEAD)?;
            }
            Some(stored) if stored == genesis.to_hex() => {}
            Some(stored) => {
                return Err(Error::GenesisMismatch {
                    stored,
                    expected: genesis.to_hex(),
                })
            }
        }
        Ok(ledger)
    }

    /// Raw connection, for crates that extend the schema (devices, classify).
    #[must_use]
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    // ----- settings -------------------------------------------------------

    /// Read a setting.
    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        setting_in(&self.conn, key)
    }

    /// Write a setting.
    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings(key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Current chain anchor.
    pub fn anchor(&self) -> Result<Hash> {
        let hex = self.setting(KEY_ANCHOR)?.unwrap_or_default();
        Hash::from_hex(&hex)
    }

    // ----- events ---------------------------------------------------------

    /// Append one event, computing `prev_hash` and `row_hash` atomically, and move the chain
    /// head to the new row in the same transaction.
    pub fn append(&mut self, new: NewEvent) -> Result<Event> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let last: Option<Vec<u8>> = tx
            .query_row(
                "SELECT row_hash FROM events ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?;
        let anchor = Hash::from_hex(&setting_in(&tx, KEY_ANCHOR)?.unwrap_or_default())?;
        // Where the table says the chain ends.
        let tail = match last {
            Some(b) => Hash::from_bytes(&b)?,
            None => anchor,
        };
        // Where the last append said it ends.
        let head = setting_in(&tx, KEY_HEAD)?
            .map(|h| Hash::from_hex(&h))
            .transpose()?;
        let head_count: Option<u64> = setting_in(&tx, KEY_HEAD_COUNT)?.and_then(|c| c.parse().ok());
        let schema = schema_in(&tx)?;
        let (prev_hash, count) = match (head, head_count) {
            (Some(h), Some(n)) if h == tail => (tail, n + 1),
            // The recorded end is not in the table any more: the newest rows were removed
            // since the last append. Chain the new row from the recorded end, not from what
            // survived, so the gap stays visible to `check()` as a broken link instead of
            // being healed by the next query the resolver writes (review of 1 Oct 2026,
            // entry 17).
            (Some(h), Some(n)) if !in_chain(&tx, &h, &anchor)? => (h, n + 1),
            // No head yet (a ledger written before 1.0.2), or rows added after the head by a
            // program that did not keep it (an older version): adopt the table's end and
            // count what is there.
            _ => {
                let stored: i64 = tx.query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))?;
                (tail, pruned_count_in(&tx)? + stored.unsigned_abs() + 1)
            }
        };
        let signals_json = serde_json::to_string(&new.signals)?;
        let process_json = match &new.process {
            Some(p) => serde_json::to_string(p)?,
            None => String::new(),
        };
        let row_hash = chain_hash(
            &prev_hash,
            &HashInput {
                ts: new.ts,
                device_id: &new.device_id,
                client_ip: &new.client_ip,
                qname: &new.qname,
                qtype: &new.qtype,
                category: new.category.as_str(),
                list_source: &new.list_source,
                signals_json: &signals_json,
                verdict: new.verdict.as_str(),
                decided_by: new.decided_by.as_str(),
                rule_id: new.rule_id,
                process_json: &process_json,
            },
        );
        tx.execute(
            "INSERT INTO events (ts, device_id, client_ip, qname, qtype, category, list_source, \
             signals_json, verdict, decided_by, rule_id, prev_hash, row_hash, process_json) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                new.ts,
                new.device_id,
                new.client_ip,
                new.qname,
                new.qtype,
                new.category.as_str(),
                new.list_source,
                signals_json,
                new.verdict.as_str(),
                new.decided_by.as_str(),
                new.rule_id,
                prev_hash.0.as_slice(),
                row_hash.0.as_slice(),
                process_json,
            ],
        )?;
        let id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO settings(key, value) VALUES (?1, ?2), (?3, ?4) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![
                KEY_HEAD,
                row_hash.to_hex(),
                KEY_HEAD_COUNT,
                count.to_string()
            ],
        )?;
        if schema == 0 {
            // A ledger from before 1.0.2 has just gained its head: from now on it keeps one.
            set_schema(&tx, SCHEMA_HEAD)?;
        } else if schema == SCHEMA_HEAD && head.is_none() {
            // This file kept a head and someone removed it. Write the row (the resolver has to
            // keep going) but leave the loss on record, at this row, until a wipe.
            set_schema(&tx, SCHEMA_HEAD_LOST)?;
            tx.execute(
                "INSERT INTO settings(key, value) VALUES (?1, ?2) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![KEY_HEAD_LOST_AT, id.to_string()],
            )?;
        }
        tx.commit()?;
        Ok(Event {
            id,
            ts: new.ts,
            device_id: new.device_id,
            client_ip: new.client_ip,
            qname: new.qname,
            qtype: new.qtype,
            category: new.category,
            list_source: new.list_source,
            signals: new.signals,
            verdict: new.verdict,
            decided_by: new.decided_by,
            rule_id: new.rule_id,
            process: new.process,
            prev_hash,
            row_hash,
        })
    }

    /// Number of events currently stored.
    pub fn event_count(&self) -> Result<u64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM events", [], |r| r.get::<_, i64>(0))?
            .unsigned_abs())
    }

    /// Number of stored events the household may read by name: the same rows [`Ledger::events`]
    /// can return. `event_count()` minus this is what devices that do not share are keeping to
    /// themselves; the panel says how many, never which (review of 5 Oct 2026, privacy item 3).
    pub fn shared_event_count(&self) -> Result<u64> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT COUNT(*) FROM events WHERE {SHARED_WITH_HOME}"),
                [SELF_DEVICE_ID],
                |r| r.get::<_, i64>(0),
            )?
            .unsigned_abs())
    }

    /// Whether the household may read this device's rows by name: this computer always, any
    /// other device only when its owner turned sharing on. A device the ledger has never seen
    /// has not consented.
    pub fn shares_detail(&self, device_id: &str) -> Result<bool> {
        if device_id == SELF_DEVICE_ID {
            return Ok(true);
        }
        Ok(self
            .conn
            .query_row(
                "SELECT share_detail_with_home FROM devices WHERE id = ?1",
                [device_id],
                |r| r.get::<_, i64>(0),
            )
            .optional()?
            .is_some_and(|s| s != 0))
    }

    /// List events matching `filter`, oldest first, as the household reads them: this
    /// computer's own rows and the rows of the devices whose owner turned "share my detail with
    /// the home panel" on. A device that did not share is still counted (`device_totals`,
    /// `week_summary`, `counters`) but is never listed by name here, whatever `filter.device_id`
    /// asks for. The command line and the exports go through this same door: the privacy policy
    /// makes no exception for them (review of 1 Oct 2026, entry 1).
    pub fn events(&self, filter: &EventFilter) -> Result<Vec<Event>> {
        self.events_for(None, filter)
    }

    /// The rows of one device for that device itself, whatever its sharing flag: the page a
    /// phone opens from its own screen, which the panel identifies by the caller's IP. Only that
    /// device's rows come back; `filter.device_id` is ignored.
    pub fn own_events(&self, device_id: &str, filter: &EventFilter) -> Result<Vec<Event>> {
        self.events_for(Some(device_id), filter)
    }

    fn events_for(&self, own: Option<&str>, filter: &EventFilter) -> Result<Vec<Event>> {
        let mut sql = format!("SELECT {EVENT_COLUMNS} FROM events WHERE 1=1");
        let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        match own {
            Some(device) => {
                sql.push_str(" AND device_id = ?");
                args.push(Box::new(device.to_owned()));
            }
            None => {
                sql.push_str(" AND ");
                sql.push_str(SHARED_WITH_HOME);
                args.push(Box::new(SELF_DEVICE_ID));
                if let Some(d) = &filter.device_id {
                    sql.push_str(" AND device_id = ?");
                    args.push(Box::new(d.clone()));
                }
            }
        }
        if let Some(c) = filter.category {
            sql.push_str(" AND category = ?");
            args.push(Box::new(c.as_str()));
        }
        if let Some(v) = filter.verdict {
            sql.push_str(" AND verdict = ?");
            args.push(Box::new(v.as_str()));
        }
        if let Some(n) = &filter.qname {
            sql.push_str(" AND qname = ?");
            args.push(Box::new(n.clone()));
        }
        if let Some(s) = filter.signal {
            sql.push_str(" AND instr(signals_json, ?) > 0");
            args.push(Box::new(format!("\"{}\"", s.as_str())));
        }
        if let Some(t) = filter.since {
            sql.push_str(" AND ts >= ?");
            args.push(Box::new(t));
        }
        if let Some(t) = filter.until {
            sql.push_str(" AND ts < ?");
            args.push(Box::new(t));
        }
        // With a limit, the newest rows are wanted; they are still returned oldest first.
        match filter.limit {
            Some(n) => {
                sql.push_str(" ORDER BY id DESC LIMIT ?");
                args.push(Box::new(i64::from(n)));
            }
            None => sql.push_str(" ORDER BY id ASC"),
        }
        let mut stmt = self.conn.prepare(&sql)?;
        let params = rusqlite::params_from_iter(args.iter().map(|a| a.as_ref()));
        let rows = stmt.query_map(params, event_from_row)?;
        let mut events: Vec<Event> = rows
            .map(|r| r.map_err(Error::from))
            .collect::<Result<_>>()?;
        if filter.limit.is_some() {
            events.reverse();
        }
        Ok(events)
    }

    /// Walk every surviving row from the anchor and verify both links, then check that the
    /// chain ends where the last append said it would (the head).
    pub fn check(&self) -> Result<CheckReport> {
        // One snapshot for the settings and the rows: a row appended between reading the head
        // and walking the table would otherwise look like one that is missing.
        let tx = self.conn.unchecked_transaction()?;
        let genesis = setting_in(&tx, KEY_GENESIS)?.unwrap_or_default();
        let anchor = Hash::from_hex(&setting_in(&tx, KEY_ANCHOR)?.unwrap_or_default())?;
        let pruned = pruned_count_in(&tx)?;
        let head = setting_in(&tx, KEY_HEAD)?
            .map(|h| Hash::from_hex(&h))
            .transpose()?;
        let head_count: Option<u64> = setting_in(&tx, KEY_HEAD_COUNT)?.and_then(|c| c.parse().ok());
        let mut report = CheckReport {
            checked: 0,
            pruned,
            anchor_is_genesis: anchor.to_hex() == genesis,
            first_fault: None,
        };
        let mut stmt = tx.prepare(&format!(
            "SELECT {EVENT_COLUMNS} FROM events ORDER BY id ASC"
        ))?;
        let mut rows = stmt.query([])?;
        let mut expected = anchor;
        // Whether the recorded end was met on the way: at the anchor (everything up to it was
        // pruned, or nothing was ever written) or as a surviving row. Rows appended after the
        // head by a program that does not keep it (an older version) are not a truncation.
        let mut head_seen = head == Some(anchor);
        let mut last_id = 0i64;
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let ev = match raw_from_row(row) {
                Ok(r) => r,
                Err(e) => {
                    report.first_fault = Some((id, ChainFault::Unreadable(e.to_string())));
                    return Ok(report);
                }
            };
            if ev.prev_hash != expected {
                report.first_fault = Some((
                    id,
                    ChainFault::BrokenLink {
                        found: ev.prev_hash,
                        expected,
                    },
                ));
                return Ok(report);
            }
            let recomputed = chain_hash(&ev.prev_hash, &ev.input());
            if recomputed != ev.row_hash {
                report.first_fault = Some((
                    id,
                    ChainFault::AlteredRow {
                        stored: ev.row_hash,
                        recomputed,
                    },
                ));
                return Ok(report);
            }
            expected = ev.row_hash;
            if Some(ev.row_hash) == head {
                head_seen = true;
            }
            last_id = id;
            report.checked += 1;
        }
        // A file that kept a head and lost it cannot say where it ended. Before the schema mark,
        // deleting the head's row in `settings` made `check()` take a 1.0.2 file for a 1.0.1 one
        // and pass it (review of 5 Oct 2026, privacy item 6).
        let schema = schema_in(&tx)?;
        if schema == SCHEMA_HEAD_LOST {
            let at = setting_in(&tx, KEY_HEAD_LOST_AT)?
                .and_then(|s| s.parse().ok())
                .unwrap_or(last_id + 1);
            report.first_fault = Some((at, ChainFault::HeadMissing));
            return Ok(report);
        }
        if schema >= SCHEMA_HEAD && head.is_none() {
            report.first_fault = Some((last_id + 1, ChainFault::HeadMissing));
            return Ok(report);
        }
        if let Some(expected_head) = head {
            if !head_seen {
                report.first_fault = Some((
                    last_id + 1,
                    ChainFault::Truncated {
                        expected_head,
                        missing: head_count
                            .map(|n| n.saturating_sub(pruned + report.checked))
                            .filter(|m| *m > 0),
                    },
                ));
            }
        }
        Ok(report)
    }

    /// Delete everything and reset the anchor and the head to the genesis. Irreversible;
    /// callers must confirm with the user first (brief §3).
    pub fn wipe(&mut self) -> Result<()> {
        let genesis = self.setting(KEY_GENESIS)?.unwrap_or_default();
        let tx = self.conn.transaction()?;
        tx.execute_batch(
            "DELETE FROM events; DELETE FROM devices; DELETE FROM rules; DELETE FROM outbound; \
             DELETE FROM seen_domains; DELETE FROM daily_totals; DELETE FROM gaps; DELETE FROM changes; \
             DELETE FROM sqlite_sequence WHERE name IN ('events', 'rules', 'outbound', 'gaps', 'changes');",
        )?;
        tx.execute(
            "UPDATE settings SET value = ?1 WHERE key = ?2",
            params![genesis, KEY_ANCHOR],
        )?;
        tx.execute(
            "UPDATE settings SET value = '0' WHERE key = ?1",
            [KEY_PRUNED],
        )?;
        tx.execute(
            "INSERT INTO settings(key, value) VALUES (?1, ?2), (?3, '0') \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![KEY_HEAD, genesis, KEY_HEAD_COUNT],
        )?;
        // A fresh start keeps a head and has lost nothing.
        tx.execute("DELETE FROM settings WHERE key = ?1", [KEY_HEAD_LOST_AT])?;
        // The rules are gone, and the resolver has to hear it: it reloads them only when this
        // number moves. Until 1.0.2 it did not, and the names cut before «Borrar todo» went on
        // being cut until the next restart (review of 5 Oct 2026, serious 13). Guard mode's
        // declared scopes are rules of a device too, and the devices are gone: they go as well.
        tx.execute(
            "INSERT INTO settings(key, value) VALUES ('rules_version', '1') \
             ON CONFLICT(key) DO UPDATE SET value = CAST(value AS INTEGER) + 1",
            [],
        )?;
        tx.execute(
            "DELETE FROM settings WHERE key LIKE 'alcance:%' OR key LIKE 'alcance_modo:%'",
            [],
        )?;
        set_schema(&tx, SCHEMA_HEAD)?;
        tx.commit()?;
        // Gone from the tables is not gone from the file: a DELETE only marks pages free, and
        // «Borrar todo» left every name readable with a plain text search of ledger.db (review of
        // 1 Oct 2026, entry 16). The checkpoint folds the write-ahead log into the file and cuts
        // the log to zero bytes; VACUUM rebuilds the file from what is left, which is nothing of
        // the person's; the second checkpoint empties the log VACUUM itself wrote through. None
        // of this can run inside the transaction above.
        self.conn.execute_batch(
            "PRAGMA wal_checkpoint(TRUNCATE); VACUUM; PRAGMA wal_checkpoint(TRUNCATE);",
        )?;
        Ok(())
    }

    // ----- seen domains (decision 10) -------------------------------------

    /// Record that `device_id` queried `qname`. Returns `true` the first time.
    pub fn first_time(&mut self, device_id: &str, qname: &str, ts: i64) -> Result<bool> {
        let inserted = self.conn.execute(
            "INSERT OR IGNORE INTO seen_domains (device_id, qname, first_seen) VALUES (?1, ?2, ?3)",
            params![device_id, qname, ts],
        )?;
        Ok(inserted == 1)
    }

    // ----- devices --------------------------------------------------------

    /// Create or refresh a device. Existing name and sharing flag are kept.
    pub fn upsert_device(
        &mut self,
        id: &str,
        mac: Option<&str>,
        ip: Option<&str>,
        now: i64,
    ) -> Result<Device> {
        self.conn.execute(
            "INSERT INTO devices (id, mac, last_ip, first_seen, last_seen) \
             VALUES (?1, ?2, ?3, ?4, ?4) \
             ON CONFLICT(id) DO UPDATE SET \
               mac = COALESCE(excluded.mac, devices.mac), \
               last_ip = COALESCE(excluded.last_ip, devices.last_ip), \
               last_seen = excluded.last_seen",
            params![id, mac, ip, now],
        )?;
        self.device(id)?.ok_or_else(|| Error::UnknownValue {
            kind: "device",
            value: id.to_owned(),
        })
    }

    /// One device by id.
    pub fn device(&self, id: &str) -> Result<Option<Device>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, mac, last_ip, name, first_seen, last_seen, share_detail_with_home, \
                 hours_profile_json FROM devices WHERE id = ?1",
                [id],
                device_from_row,
            )
            .optional()?)
    }

    /// All devices, most recently seen first.
    pub fn devices(&self) -> Result<Vec<Device>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, mac, last_ip, name, first_seen, last_seen, share_detail_with_home, \
             hours_profile_json FROM devices ORDER BY last_seen DESC",
        )?;
        let rows = stmt.query_map([], device_from_row)?;
        rows.map(|r| r.map_err(Error::from)).collect()
    }

    /// Give a device a name.
    pub fn rename_device(&mut self, id: &str, name: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE devices SET name = ?2 WHERE id = ?1",
            params![id, name],
        )?;
        Ok(())
    }

    /// Set whether a device shares its detail with the home panel.
    pub fn set_share_detail(&mut self, id: &str, share: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE devices SET share_detail_with_home = ?2 WHERE id = ?1",
            params![id, i32::from(share)],
        )?;
        Ok(())
    }

    /// Store the learned hours profile for a device.
    pub fn set_hours_profile(&mut self, id: &str, profile_json: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE devices SET hours_profile_json = ?2 WHERE id = ?1",
            params![id, profile_json],
        )?;
        Ok(())
    }

    // ----- rules ----------------------------------------------------------

    /// Older databases lack `events.process_json`; add it once. Rows written before it keep an
    /// empty value, which is exactly what the hash treats as "there was no program here", so the
    /// chain of an old ledger still verifies after the upgrade.
    fn migrate_events_process(&self) -> Result<()> {
        let has: bool = self
            .conn
            .prepare("PRAGMA table_info(events)")?
            .query_map([], |r| r.get::<_, String>(1))?
            .filter_map(std::result::Result::ok)
            .any(|c| c == "process_json");
        if !has {
            self.conn.execute_batch(
                "ALTER TABLE events ADD COLUMN process_json TEXT NOT NULL DEFAULT ''",
            )?;
        }
        Ok(())
    }

    /// Older databases lack `rules.confirmed`; add it once.
    fn migrate_rules_confirmed(&self) -> Result<()> {
        let has: bool = self
            .conn
            .prepare("PRAGMA table_info(rules)")?
            .query_map([], |r| r.get::<_, String>(1))?
            .filter_map(std::result::Result::ok)
            .any(|c| c == "confirmed");
        if !has {
            self.conn.execute_batch(
                "ALTER TABLE rules ADD COLUMN confirmed INTEGER NOT NULL DEFAULT 0",
            )?;
        }
        Ok(())
    }

    /// A counter bumped on every rule change, so a running engine can refresh cheaply.
    pub fn rules_version(&self) -> Result<u64> {
        Ok(self
            .setting("rules_version")?
            .and_then(|v| v.parse().ok())
            .unwrap_or(0))
    }

    fn bump_rules_version(&self) -> Result<()> {
        let v = self.rules_version()? + 1;
        self.set_setting("rules_version", &v.to_string())
    }

    /// Store a rule the user created.
    pub fn add_rule(&mut self, new: NewRule) -> Result<Rule> {
        self.conn.execute(
            "INSERT INTO rules (scope, device_id, match_kind, pattern, action, created_at, \
             created_by, expires_at, confirmed) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                new.scope.as_str(),
                new.device_id,
                new.match_kind.as_str(),
                new.pattern,
                new.action.as_str(),
                new.created_at,
                new.created_by,
                new.expires_at,
                i32::from(new.confirmed),
            ],
        )?;
        let id = self.conn.last_insert_rowid();
        self.bump_rules_version()?;
        Ok(Rule {
            id,
            scope: new.scope,
            device_id: new.device_id,
            match_kind: new.match_kind,
            pattern: new.pattern,
            action: new.action,
            created_at: new.created_at,
            created_by: new.created_by,
            expires_at: new.expires_at,
            undone_at: None,
            confirmed: new.confirmed,
        })
    }

    /// Undo one rule. The row stays; only `undone_at` is set.
    pub fn undo_rule(&mut self, id: i64, now: i64) -> Result<bool> {
        let n = self.conn.execute(
            "UPDATE rules SET undone_at = ?2 WHERE id = ?1 AND undone_at IS NULL",
            params![id, now],
        )?;
        self.bump_rules_version()?;
        Ok(n == 1)
    }

    /// Undo every rule created at or after `since` ("deshacer todo lo de hoy").
    pub fn undo_rules_since(&mut self, since: i64, now: i64) -> Result<usize> {
        let n = self.conn.execute(
            "UPDATE rules SET undone_at = ?2 WHERE created_at >= ?1 AND undone_at IS NULL",
            params![since, now],
        )?;
        self.bump_rules_version()?;
        Ok(n)
    }

    /// All rules, newest first, undone ones included (they are history).
    pub fn rules(&self) -> Result<Vec<Rule>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, scope, device_id, match_kind, pattern, action, created_at, created_by, \
             expires_at, undone_at, confirmed FROM rules ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([], rule_from_row)?;
        rows.map(|r| r.map_err(Error::from)).collect()
    }

    /// Rules in force at `now`.
    pub fn active_rules(&self, now: i64) -> Result<Vec<Rule>> {
        Ok(self
            .rules()?
            .into_iter()
            .filter(|r| r.is_active(now))
            .collect())
    }

    // ----- outbound -------------------------------------------------------

    /// Record something the program itself sent out.
    pub fn record_outbound(
        &mut self,
        ts: i64,
        purpose: Purpose,
        host: &str,
        bytes: i64,
        initiated_by_user: bool,
    ) -> Result<Outbound> {
        self.conn.execute(
            "INSERT INTO outbound (ts, purpose, host, bytes, initiated_by_user) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                ts,
                purpose.as_str(),
                host,
                bytes,
                i32::from(initiated_by_user)
            ],
        )?;
        Ok(Outbound {
            id: self.conn.last_insert_rowid(),
            ts,
            purpose,
            host: host.to_owned(),
            bytes,
            initiated_by_user,
        })
    }

    /// Everything the program ever sent out, newest first.
    pub fn outbound(&self) -> Result<Vec<Outbound>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, ts, purpose, host, bytes, initiated_by_user FROM outbound ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            let purpose: String = r.get(2)?;
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                purpose,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, i32>(5)?,
            ))
        })?;
        rows.map(|r| {
            let (id, ts, purpose, host, bytes, by_user) = r?;
            Ok(Outbound {
                id,
                ts,
                purpose: purpose.parse()?,
                host,
                bytes,
                initiated_by_user: by_user != 0,
            })
        })
        .collect()
    }
}

/// Read a setting through any connection or transaction.
fn setting_in(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()?)
}

fn pruned_count_in(conn: &Connection) -> Result<u64> {
    Ok(setting_in(conn, KEY_PRUNED)?
        .and_then(|s| s.parse().ok())
        .unwrap_or(0))
}

/// Whether `hash` is somewhere in the chain the table holds: the anchor, or the `row_hash` of a
/// surviving row. Asked only when the head and the table disagree, so the scan over `row_hash`
/// (no index) runs once per disagreement, never per query.
fn in_chain(conn: &Connection, hash: &Hash, anchor: &Hash) -> Result<bool> {
    if hash == anchor {
        return Ok(true);
    }
    Ok(conn
        .query_row(
            "SELECT 1 FROM events WHERE row_hash = ?1 LIMIT 1",
            [hash.0.as_slice()],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// The file's chain-head schema mark (`PRAGMA user_version`): 0 before 1.0.2,
/// [`SCHEMA_HEAD`] when it keeps a head, [`SCHEMA_HEAD_LOST`] once the head went missing.
fn schema_in(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}

/// Set the chain-head schema mark. It is written to the file header inside the caller's
/// transaction, so it commits or rolls back with the rows.
fn set_schema(conn: &Connection, v: i64) -> Result<()> {
    // PRAGMA takes no bound parameters; `v` is one of the two constants above.
    conn.execute_batch(&format!("PRAGMA user_version = {v}"))?;
    Ok(())
}

/// A row read back with its stored text columns, before parsing into enums.
struct RawEvent {
    ts: i64,
    device_id: String,
    client_ip: String,
    qname: String,
    qtype: String,
    category: String,
    list_source: String,
    signals_json: String,
    verdict: String,
    decided_by: String,
    rule_id: Option<i64>,
    prev_hash: Hash,
    row_hash: Hash,
    process_json: String,
}

impl RawEvent {
    fn input(&self) -> HashInput<'_> {
        HashInput {
            ts: self.ts,
            device_id: &self.device_id,
            client_ip: &self.client_ip,
            qname: &self.qname,
            qtype: &self.qtype,
            category: &self.category,
            list_source: &self.list_source,
            signals_json: &self.signals_json,
            verdict: &self.verdict,
            decided_by: &self.decided_by,
            rule_id: self.rule_id,
            process_json: &self.process_json,
        }
    }
}

fn raw_from_row(row: &Row<'_>) -> Result<RawEvent> {
    let prev: Vec<u8> = row.get(12)?;
    let cur: Vec<u8> = row.get(13)?;
    Ok(RawEvent {
        ts: row.get(1)?,
        device_id: row.get(2)?,
        client_ip: row.get(3)?,
        qname: row.get(4)?,
        qtype: row.get(5)?,
        category: row.get(6)?,
        list_source: row.get(7)?,
        signals_json: row.get(8)?,
        verdict: row.get(9)?,
        decided_by: row.get(10)?,
        rule_id: row.get(11)?,
        prev_hash: Hash::from_bytes(&prev)?,
        row_hash: Hash::from_bytes(&cur)?,
        process_json: row.get(14).unwrap_or_default(),
    })
}

fn to_sql_err(e: Error) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
}

fn event_from_row(row: &Row<'_>) -> rusqlite::Result<Event> {
    let id: i64 = row.get(0)?;
    let raw = raw_from_row(row).map_err(to_sql_err)?;
    let signals: Vec<Signal> =
        serde_json::from_str(&raw.signals_json).map_err(|e| to_sql_err(Error::Json(e)))?;
    Ok(Event {
        id,
        ts: raw.ts,
        device_id: raw.device_id,
        client_ip: raw.client_ip,
        qname: raw.qname,
        qtype: raw.qtype,
        category: raw.category.parse().map_err(to_sql_err)?,
        list_source: raw.list_source,
        signals,
        verdict: raw.verdict.parse().map_err(to_sql_err)?,
        decided_by: raw.decided_by.parse().map_err(to_sql_err)?,
        rule_id: raw.rule_id,
        process: if raw.process_json.is_empty() {
            None
        } else {
            serde_json::from_str(&raw.process_json).map_err(|e| to_sql_err(Error::Json(e)))?
        },
        prev_hash: raw.prev_hash,
        row_hash: raw.row_hash,
    })
}

fn device_from_row(row: &Row<'_>) -> rusqlite::Result<Device> {
    Ok(Device {
        id: row.get(0)?,
        mac: row.get(1)?,
        last_ip: row.get(2)?,
        name: row.get(3)?,
        first_seen: row.get(4)?,
        last_seen: row.get(5)?,
        share_detail_with_home: row.get::<_, i32>(6)? != 0,
        hours_profile_json: row.get(7)?,
    })
}

fn rule_from_row(row: &Row<'_>) -> rusqlite::Result<Rule> {
    let scope: String = row.get(1)?;
    let kind: String = row.get(3)?;
    let action: String = row.get(5)?;
    Ok(Rule {
        id: row.get(0)?,
        scope: scope.parse::<Scope>().map_err(to_sql_err)?,
        device_id: row.get(2)?,
        match_kind: kind.parse::<MatchKind>().map_err(to_sql_err)?,
        pattern: row.get(4)?,
        action: action.parse::<Action>().map_err(to_sql_err)?,
        created_at: row.get(6)?,
        created_by: row.get(7)?,
        expires_at: row.get(8)?,
        undone_at: row.get(9)?,
        confirmed: row.get::<_, i32>(10)? != 0,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::model::{DecidedBy, Verdict};

    fn genesis() -> Hash {
        Hash::of(b"dev public key")
    }

    fn ev(ts: i64, name: &str) -> NewEvent {
        NewEvent::observed(ts, "self", "127.0.0.1", name, "A")
    }

    #[test]
    fn append_links_rows_and_check_passes() {
        let mut l = Ledger::open_in_memory(genesis()).unwrap();
        let a = l.append(ev(1, "a.example")).unwrap();
        let b = l.append(ev(2, "b.example")).unwrap();
        assert_eq!(a.prev_hash, genesis());
        assert_eq!(b.prev_hash, a.row_hash);
        let r = l.check().unwrap();
        assert!(r.is_ok());
        assert_eq!(r.checked, 2);
        assert!(r.anchor_is_genesis);
    }

    #[test]
    fn altered_field_is_detected_at_the_right_row() {
        let mut l = Ledger::open_in_memory(genesis()).unwrap();
        l.append(ev(1, "a.example")).unwrap();
        let b = l.append(ev(2, "b.example")).unwrap();
        l.append(ev(3, "c.example")).unwrap();
        l.conn
            .execute(
                "UPDATE events SET qname = 'x.example' WHERE id = ?1",
                [b.id],
            )
            .unwrap();
        let r = l.check().unwrap();
        let (id, fault) = r.first_fault.expect("must fail");
        assert_eq!(id, b.id);
        assert!(matches!(fault, ChainFault::AlteredRow { .. }));
        assert_eq!(r.checked, 1);
    }

    #[test]
    fn deleted_row_breaks_the_link() {
        let mut l = Ledger::open_in_memory(genesis()).unwrap();
        l.append(ev(1, "a.example")).unwrap();
        let b = l.append(ev(2, "b.example")).unwrap();
        let c = l.append(ev(3, "c.example")).unwrap();
        l.conn
            .execute("DELETE FROM events WHERE id = ?1", [b.id])
            .unwrap();
        let r = l.check().unwrap();
        let (id, fault) = r.first_fault.expect("must fail");
        assert_eq!(id, c.id);
        assert!(matches!(fault, ChainFault::BrokenLink { .. }));
    }

    #[test]
    fn another_public_key_is_refused() {
        let dir = std::env::temp_dir().join(format!("guardiana-core-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ledger.db");
        let _ = std::fs::remove_file(&path);
        {
            let mut l = Ledger::open(&path, genesis()).unwrap();
            l.append(ev(1, "a.example")).unwrap();
        }
        assert!(matches!(
            Ledger::open(&path, Hash::of(b"other key")),
            Err(Error::GenesisMismatch { .. })
        ));
        let l = Ledger::open(&path, genesis()).unwrap();
        assert_eq!(l.event_count().unwrap(), 1);
        assert!(l.check().unwrap().is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn filters_and_signals_round_trip() {
        let mut l = Ledger::open_in_memory(genesis()).unwrap();
        let mut tracked = ev(1, "t.example");
        tracked.category = Category::Rastreador;
        tracked.signals = vec![Signal::Baliza { minutes: 5 }, Signal::DestinoNuevo];
        tracked.verdict = Verdict::Cortado;
        tracked.decided_by = DecidedBy::ReglaUsuario;
        tracked.rule_id = Some(7);
        l.append(tracked.clone()).unwrap();
        let mut phone = ev(2, "p.example");
        phone.device_id = "mac:aa".into();
        l.append(phone).unwrap();
        // A phone is listed to the household only once its owner shares its detail.
        l.upsert_device("mac:aa", None, Some("10.0.0.2"), 2)
            .unwrap();
        l.set_share_detail("mac:aa", true).unwrap();

        let all = l.events(&EventFilter::default()).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].signals, tracked.signals);
        assert_eq!(all[0].rule_id, Some(7));

        let by_signal = l
            .events(&EventFilter {
                signal: Some(SignalKind::Baliza),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(by_signal.len(), 1);
        let by_device = l
            .events(&EventFilter {
                device_id: Some("mac:aa".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(by_device[0].qname, "p.example");
        let none = l
            .events(&EventFilter {
                category: Some(Category::Publicidad),
                ..Default::default()
            })
            .unwrap();
        assert!(none.is_empty());
    }

    #[test]
    fn el_filtro_por_nombre_encuentra_una_picada_vieja() {
        // La pregunta de una trampa: «¿alguien preguntó alguna vez por mi nombre?». Antes se
        // respondía trayendo las últimas filas y mirándolas, así que una picada enterrada bajo
        // miles de consultas posteriores no se veía. Aquí la trampa es la fila 1 de 3.000.
        let mut l = Ledger::open_in_memory(genesis()).unwrap();
        l.append(ev(1, "abcd1234.trampa.guardianagroup.com"))
            .unwrap();
        for i in 2..3_000 {
            l.append(ev(i, "relleno.example")).unwrap();
        }
        let suyas = l
            .events(&EventFilter {
                qname: Some("abcd1234.trampa.guardianagroup.com".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(suyas.len(), 1, "la picada tiene que aparecer");
        assert_eq!(suyas[0].ts, 1);
        let ninguna = l
            .events(&EventFilter {
                qname: Some("otra.trampa.guardianagroup.com".into()),
                ..Default::default()
            })
            .unwrap();
        assert!(ninguna.is_empty(), "un nombre que nadie pidió no da filas");
    }

    #[test]
    fn first_time_is_true_once_per_device() {
        let mut l = Ledger::open_in_memory(genesis()).unwrap();
        assert!(l.first_time("self", "a.example", 1).unwrap());
        assert!(!l.first_time("self", "a.example", 2).unwrap());
        assert!(l.first_time("mac:aa", "a.example", 3).unwrap());
    }

    #[test]
    fn devices_keep_name_and_sharing_on_refresh() {
        let mut l = Ledger::open_in_memory(genesis()).unwrap();
        let d = l
            .upsert_device("mac:aa", Some("aa:bb"), Some("10.0.0.2"), 1)
            .unwrap();
        assert_eq!(d.name, None);
        assert!(!d.share_detail_with_home);
        l.rename_device("mac:aa", "Móvil de Ana").unwrap();
        l.set_share_detail("mac:aa", true).unwrap();
        let d = l
            .upsert_device("mac:aa", None, Some("10.0.0.9"), 5)
            .unwrap();
        assert_eq!(d.name.as_deref(), Some("Móvil de Ana"));
        assert!(d.share_detail_with_home);
        assert_eq!(d.mac.as_deref(), Some("aa:bb"));
        assert_eq!(d.last_ip.as_deref(), Some("10.0.0.9"));
        assert_eq!(d.first_seen, 1);
        assert_eq!(d.last_seen, 5);
    }

    #[test]
    fn rules_undo_keeps_history() {
        let mut l = Ledger::open_in_memory(genesis()).unwrap();
        let r = l
            .add_rule(NewRule {
                scope: Scope::Home,
                device_id: None,
                match_kind: MatchKind::Suffix,
                pattern: "ads.example".into(),
                action: Action::Cortar,
                created_at: 100,
                created_by: "usuario".into(),
                expires_at: None,
                confirmed: false,
            })
            .unwrap();
        assert_eq!(l.active_rules(150).unwrap().len(), 1);
        assert_eq!(l.rules_version().unwrap(), 1);
        assert!(l.undo_rule(r.id, 200).unwrap());
        assert!(!l.undo_rule(r.id, 201).unwrap());
        assert!(l.active_rules(250).unwrap().is_empty());
        assert_eq!(l.rules().unwrap()[0].undone_at, Some(200));
    }

    #[test]
    fn outbound_is_recorded() {
        let mut l = Ledger::open_in_memory(genesis()).unwrap();
        l.record_outbound(1, Purpose::Listas, "lists.example", 1234, true)
            .unwrap();
        let o = l.outbound().unwrap();
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].purpose, Purpose::Listas);
        assert!(o[0].initiated_by_user);
    }

    /// Serious 13 of 5 Oct 2026: after «Borrar todo» the resolver kept cutting what the rules
    /// said, because the number it watches to reload them did not move.
    #[test]
    fn wipe_tells_the_resolver_the_rules_and_scopes_are_gone() {
        let mut l = Ledger::open_in_memory(genesis()).unwrap();
        l.add_rule(NewRule {
            scope: Scope::Home,
            device_id: None,
            match_kind: MatchKind::Suffix,
            pattern: "ads.example".into(),
            action: Action::Cortar,
            created_at: 100,
            created_by: "usuario".into(),
            expires_at: None,
            confirmed: false,
        })
        .unwrap();
        l.set_setting("alcance:self", "github.com").unwrap();
        l.set_setting("alcance_modo:self", "cortar").unwrap();
        l.set_setting("home_mode", "1").unwrap();
        let antes = l.rules_version().unwrap();
        l.wipe().unwrap();
        assert!(l.rules_version().unwrap() > antes);
        assert!(l.rules().unwrap().is_empty());
        assert_eq!(l.setting("alcance:self").unwrap(), None);
        assert_eq!(l.setting("alcance_modo:self").unwrap(), None);
        // What is not the person's history or their rules stays.
        assert_eq!(l.setting("home_mode").unwrap().as_deref(), Some("1"));
        // And on a ledger that never had a rule, the number starts.
        let mut nuevo = Ledger::open_in_memory(genesis()).unwrap();
        nuevo.wipe().unwrap();
        assert_eq!(nuevo.rules_version().unwrap(), 1);
    }

    #[test]
    fn wipe_resets_to_genesis() {
        let mut l = Ledger::open_in_memory(genesis()).unwrap();
        l.append(ev(1, "a.example")).unwrap();
        l.wipe().unwrap();
        assert_eq!(l.event_count().unwrap(), 0);
        assert!(l.check().unwrap().is_ok(), "an empty ledger verifies");
        let a = l.append(ev(2, "b.example")).unwrap();
        assert_eq!(a.id, 1);
        assert_eq!(a.prev_hash, genesis());
        assert!(l.check().unwrap().is_ok());
        assert_eq!(l.setting(KEY_HEAD_COUNT).unwrap().as_deref(), Some("1"));
    }

    fn temp_db(tag: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("guardiana-core-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ledger.db");
        (dir, path)
    }

    fn file_holds(path: &Path, needle: &[u8]) -> bool {
        match std::fs::read(path) {
            Ok(bytes) => bytes.windows(needle.len()).any(|w| w == needle),
            Err(_) => false,
        }
    }

    /// «Borrar todo» promised a file with nothing of the person's in it and left every name
    /// readable with a plain text search (review of 1 Oct 2026, entry 16).
    #[test]
    fn wipe_leaves_no_name_in_the_file_nor_in_the_log() {
        let (dir, path) = temp_db("wipe");
        let wal = dir.join("ledger.db-wal");
        let name = b"clinica-ejemplo";
        {
            let mut l = Ledger::open(&path, genesis()).unwrap();
            for i in 0..400 {
                l.append(ev(i, "clinica-ejemplo.example")).unwrap();
            }
            l.upsert_device("mac:aa", Some("aa:bb"), Some("10.0.0.2"), 1)
                .unwrap();
            l.rename_device("mac:aa", "Movil de Ana").unwrap();
            l.first_time("mac:aa", "clinica-ejemplo.example", 1)
                .unwrap();
            assert!(
                file_holds(&path, name) || file_holds(&wal, name),
                "before the wipe the name is on disk, or the test proves nothing"
            );
            l.wipe().unwrap();
            assert_eq!(l.event_count().unwrap(), 0);
            for f in [&path, &wal] {
                assert!(!file_holds(f, name), "{} still holds the name", f.display());
                assert!(
                    !file_holds(f, b"Movil de Ana"),
                    "{} still holds the device name",
                    f.display()
                );
            }
            // The ledger still works after being rebuilt.
            l.append(ev(1, "otra.example")).unwrap();
            assert!(l.check().unwrap().is_ok());
        }
        // Closing the connection writes nothing old back.
        for f in [&path, &wal] {
            assert!(
                !file_holds(f, name),
                "{} holds the name after close",
                f.display()
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ----- the recorded end of the chain (review of 1 Oct 2026, entry 17) -------------------

    fn delete_row(l: &Ledger, id: i64) {
        l.conn
            .execute("DELETE FROM events WHERE id = ?1", [id])
            .unwrap();
    }

    #[test]
    fn deleting_the_newest_rows_is_reported_as_truncation() {
        let mut l = Ledger::open_in_memory(genesis()).unwrap();
        l.append(ev(1, "a.example")).unwrap();
        let b = l.append(ev(2, "b.example")).unwrap();
        let c = l.append(ev(3, "c.example")).unwrap();
        assert!(l.check().unwrap().is_ok());

        delete_row(&l, c.id);
        let r = l.check().unwrap();
        assert_eq!(r.checked, 2, "the surviving rows still link");
        let (id, fault) = r.first_fault.expect("the missing end must be reported");
        assert_eq!(id, c.id, "reported at the first row that is missing");
        assert_eq!(
            fault,
            ChainFault::Truncated {
                expected_head: c.row_hash,
                missing: Some(1)
            }
        );

        delete_row(&l, b.id);
        let r = l.check().unwrap();
        let (id, fault) = r.first_fault.expect("still missing");
        assert_eq!(id, b.id);
        assert!(matches!(
            fault,
            ChainFault::Truncated {
                missing: Some(2),
                ..
            }
        ));
    }

    #[test]
    fn deleting_every_row_is_reported_as_truncation() {
        let mut l = Ledger::open_in_memory(genesis()).unwrap();
        l.append(ev(1, "a.example")).unwrap();
        l.append(ev(2, "b.example")).unwrap();
        l.conn.execute("DELETE FROM events", []).unwrap();
        let r = l.check().unwrap();
        assert_eq!(r.checked, 0);
        let (id, fault) = r.first_fault.expect("must fail");
        assert_eq!(id, 1);
        assert!(matches!(
            fault,
            ChainFault::Truncated {
                missing: Some(2),
                ..
            }
        ));
    }

    /// Pruning moves the anchor forward, never the head: a tail removed before the prune is
    /// still missing after it.
    #[test]
    fn truncation_is_still_seen_after_pruning() {
        use crate::retention::Retention;
        use crate::time::DAY_MS;
        let mut l = Ledger::open_in_memory(genesis()).unwrap();
        l.append(ev(1, "a.example")).unwrap();
        l.append(ev(2, "b.example")).unwrap();
        let c = l.append(ev(3, "c.example")).unwrap();
        delete_row(&l, c.id);
        let pruned = l.prune(Retention::FREE, 2 * DAY_MS).unwrap();
        assert_eq!(pruned.events_pruned, 2);
        let r = l.check().unwrap();
        assert_eq!(r.pruned, 2);
        assert!(matches!(
            r.first_fault,
            Some((
                1,
                ChainFault::Truncated {
                    missing: Some(1),
                    ..
                }
            ))
        ));
        // And an intact ledger pruned to nothing verifies: the head is the anchor.
        let mut l = Ledger::open_in_memory(genesis()).unwrap();
        l.append(ev(1, "a.example")).unwrap();
        l.prune(Retention::FREE, 2 * DAY_MS).unwrap();
        assert_eq!(l.event_count().unwrap(), 0);
        assert!(l.check().unwrap().is_ok());
    }

    /// Once the resolver writes again, the gap must not heal: the new row links to the recorded
    /// end, so the missing rows show as a broken link for as long as the chain is kept.
    #[test]
    fn appending_after_a_truncation_keeps_the_gap_visible() {
        let mut l = Ledger::open_in_memory(genesis()).unwrap();
        l.append(ev(1, "a.example")).unwrap();
        let b = l.append(ev(2, "b.example")).unwrap();
        let c = l.append(ev(3, "c.example")).unwrap();
        delete_row(&l, c.id);
        let d = l.append(ev(4, "d.example")).unwrap();
        assert_eq!(d.prev_hash, c.row_hash, "chained from the recorded end");
        let r = l.check().unwrap();
        assert_eq!(r.checked, 2);
        assert_eq!(
            r.first_fault,
            Some((
                d.id,
                ChainFault::BrokenLink {
                    found: c.row_hash,
                    expected: b.row_hash
                }
            ))
        );
        assert_eq!(l.setting(KEY_HEAD_COUNT).unwrap().as_deref(), Some("4"));
        // The head now sits on the new row: nothing else is missing.
        let e = l.append(ev(5, "e.example")).unwrap();
        assert_eq!(e.prev_hash, d.row_hash);
    }

    /// A ledger written before 1.0.2 has no head. It verifies as it always did, cannot tell a
    /// missing tail yet, and gains the head on its next append.
    #[test]
    fn a_ledger_without_a_head_verifies_as_before_and_gains_one() {
        let mut l = Ledger::open_in_memory(genesis()).unwrap();
        l.append(ev(1, "a.example")).unwrap();
        let b = l.append(ev(2, "b.example")).unwrap();
        let c = l.append(ev(3, "c.example")).unwrap();
        l.conn
            .execute(
                "DELETE FROM settings WHERE key IN (?1, ?2)",
                [KEY_HEAD, KEY_HEAD_COUNT],
            )
            .unwrap();
        // What 1.0.1 left behind: no head and no schema mark in the file header.
        set_schema(&l.conn, 0).unwrap();
        assert!(l.check().unwrap().is_ok());
        delete_row(&l, c.id);
        let r = l.check().unwrap();
        assert!(
            r.is_ok(),
            "without a head there is nothing to compare: {r:?}"
        );
        assert_eq!(r.checked, 2);

        let d = l.append(ev(4, "d.example")).unwrap();
        assert_eq!(d.prev_hash, b.row_hash, "adopts the table's end");
        assert_eq!(
            l.setting(KEY_HEAD).unwrap(),
            Some(d.row_hash.to_hex()),
            "the head is written on the first append"
        );
        assert_eq!(
            l.setting(KEY_HEAD_COUNT).unwrap().as_deref(),
            Some("3"),
            "two rows were there, one was added"
        );
        assert!(l.check().unwrap().is_ok());
        assert_eq!(
            schema_in(&l.conn).unwrap(),
            SCHEMA_HEAD,
            "and keeps one from now on"
        );
        delete_row(&l, d.id);
        assert!(matches!(
            l.check().unwrap().first_fault,
            Some((
                _,
                ChainFault::Truncated {
                    missing: Some(1),
                    ..
                }
            ))
        ));
    }

    /// Removing the head from a file that keeps one does not turn it into an old ledger: the
    /// check says the end is unknown, and the resolver's next append does not heal it. A wipe
    /// starts clean (review of 5 Oct 2026, privacy item 6).
    #[test]
    fn removing_the_head_from_a_new_ledger_is_a_fault_that_stays() {
        let mut l = Ledger::open_in_memory(genesis()).unwrap();
        assert_eq!(schema_in(&l.conn).unwrap(), SCHEMA_HEAD);
        l.append(ev(1, "a.example")).unwrap();
        let b = l.append(ev(2, "b.example")).unwrap();
        let c = l.append(ev(3, "c.example")).unwrap();
        delete_row(&l, c.id);
        l.conn
            .execute(
                "DELETE FROM settings WHERE key IN (?1, ?2)",
                [KEY_HEAD, KEY_HEAD_COUNT],
            )
            .unwrap();
        assert_eq!(
            l.check().unwrap().first_fault,
            Some((b.id + 1, ChainFault::HeadMissing))
        );
        let d = l.append(ev(4, "d.example")).unwrap();
        let e = l.append(ev(5, "e.example")).unwrap();
        let r = l.check().unwrap();
        assert_eq!(
            r.first_fault,
            Some((d.id, ChainFault::HeadMissing)),
            "the loss stays at the first row written after it, two appends later"
        );
        assert_eq!(schema_in(&l.conn).unwrap(), SCHEMA_HEAD_LOST);
        // Deleting the marker row does not clear it either: the mark is in the file header.
        l.conn
            .execute("DELETE FROM settings WHERE key = ?1", [KEY_HEAD_LOST_AT])
            .unwrap();
        assert_eq!(
            l.check().unwrap().first_fault,
            Some((e.id + 1, ChainFault::HeadMissing))
        );
        l.wipe().unwrap();
        assert!(l.check().unwrap().is_ok());
        assert_eq!(schema_in(&l.conn).unwrap(), SCHEMA_HEAD);
        l.append(ev(6, "f.example")).unwrap();
        assert!(l.check().unwrap().is_ok());
    }

    /// The schema mark is written in the same transaction as the row: a file opened, written and
    /// reopened keeps it.
    #[test]
    fn the_schema_mark_survives_reopening_the_file() {
        let dir = std::env::temp_dir().join(format!("guardiana-schema-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ledger.db");
        {
            let mut l = Ledger::open(&path, genesis()).unwrap();
            l.append(ev(1, "a.example")).unwrap();
        }
        let l = Ledger::open(&path, genesis()).unwrap();
        assert_eq!(schema_in(&l.conn).unwrap(), SCHEMA_HEAD);
        assert!(l.check().unwrap().is_ok());
        drop(l);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Rows appended by a program that does not keep the head (an older version run on the
    /// same file) are not a truncation: the head is behind, not ahead. The next append catches
    /// the head up and recounts.
    #[test]
    fn rows_appended_by_an_older_program_are_not_a_truncation() {
        let mut l = Ledger::open_in_memory(genesis()).unwrap();
        l.append(ev(1, "a.example")).unwrap();
        let b = l.append(ev(2, "b.example")).unwrap();
        let c = l.append(ev(3, "c.example")).unwrap();
        // Pretend the third row was written by 1.0.1: the head still points at the second.
        l.set_setting(KEY_HEAD, &b.row_hash.to_hex()).unwrap();
        l.set_setting(KEY_HEAD_COUNT, "2").unwrap();
        assert!(l.check().unwrap().is_ok());
        let d = l.append(ev(4, "d.example")).unwrap();
        assert_eq!(d.prev_hash, c.row_hash, "links to what is really last");
        assert_eq!(l.setting(KEY_HEAD_COUNT).unwrap().as_deref(), Some("4"));
        assert!(l.check().unwrap().is_ok());
    }

    /// A head that is missing or unreadable in a file created by 1.0.2 is not invented: the
    /// append treats it like an old ledger and writes a fresh one.
    #[test]
    fn an_unreadable_head_count_is_recounted() {
        let mut l = Ledger::open_in_memory(genesis()).unwrap();
        l.append(ev(1, "a.example")).unwrap();
        l.set_setting(KEY_HEAD_COUNT, "not a number").unwrap();
        l.append(ev(2, "b.example")).unwrap();
        assert_eq!(l.setting(KEY_HEAD_COUNT).unwrap().as_deref(), Some("2"));
        assert!(l.check().unwrap().is_ok());
    }

    // ----- who may read whose rows (review of 1 Oct 2026, entry 1) --------------------------

    fn phone(ts: i64, device: &str, name: &str) -> NewEvent {
        NewEvent::observed(ts, device, "10.0.0.2", name, "A")
    }

    /// The household panel sees this computer's rows and the rows of the devices that share;
    /// a phone that did not share is counted, never listed. The phone itself sees everything.
    #[test]
    fn a_device_that_did_not_share_is_counted_but_not_listed_for_the_household() {
        let mut l = Ledger::open_in_memory(genesis()).unwrap();
        l.upsert_device("mac:callado", Some("aa:01"), Some("10.0.0.2"), 1)
            .unwrap();
        l.upsert_device("mac:abierto", Some("aa:02"), Some("10.0.0.3"), 1)
            .unwrap();
        l.set_share_detail("mac:abierto", true).unwrap();
        l.append(ev(1, "equipo.example")).unwrap();
        l.append(phone(2, "mac:callado", "clinica.example"))
            .unwrap();
        l.append(phone(3, "mac:callado", "citas.example")).unwrap();
        l.append(phone(4, "mac:abierto", "tienda.example")).unwrap();
        // Never registered as a device: nobody consented for it either.
        l.append(phone(5, "mac:fantasma", "fantasma.example"))
            .unwrap();

        let names = |evs: Vec<Event>| -> Vec<String> { evs.into_iter().map(|e| e.qname).collect() };
        assert_eq!(
            names(l.events(&EventFilter::default()).unwrap()),
            ["equipo.example", "tienda.example"]
        );
        // Asking for the silent phone by id gives nothing, with or without a limit.
        for limit in [None, Some(10)] {
            let f = EventFilter {
                device_id: Some("mac:callado".into()),
                limit,
                ..Default::default()
            };
            assert!(l.events(&f).unwrap().is_empty(), "limit {limit:?}");
        }
        assert!(l
            .events(&EventFilter {
                device_id: Some("mac:fantasma".into()),
                ..Default::default()
            })
            .unwrap()
            .is_empty());
        // Nor by the name it asked for.
        assert!(l
            .events(&EventFilter {
                qname: Some("clinica.example".into()),
                ..Default::default()
            })
            .unwrap()
            .is_empty());

        // The phone's own page sees its own rows in full, and only its own.
        assert_eq!(
            names(
                l.own_events("mac:callado", &EventFilter::default())
                    .unwrap()
            ),
            ["clinica.example", "citas.example"]
        );
        let other = EventFilter {
            device_id: Some(SELF_DEVICE_ID.into()),
            ..Default::default()
        };
        assert_eq!(
            names(l.own_events("mac:callado", &other).unwrap()),
            ["clinica.example", "citas.example"],
            "filter.device_id cannot widen an own-device query"
        );

        // Totals keep counting every device.
        let totals = l.device_totals(None).unwrap();
        let callado = totals
            .iter()
            .find(|t| t.device_id == "mac:callado")
            .unwrap();
        assert_eq!(callado.queries, 2);
        assert_eq!(l.counters(0, 100).unwrap().services, 5);
        // The household's count is what it can list; the rest is said as a number.
        assert_eq!(l.event_count().unwrap(), 5);
        assert_eq!(l.shared_event_count().unwrap(), 2);
        assert!(l.shares_detail(SELF_DEVICE_ID).unwrap());
        assert!(l.shares_detail("mac:abierto").unwrap());
        assert!(!l.shares_detail("mac:callado").unwrap());
        assert!(!l.shares_detail("mac:fantasma").unwrap());

        // Sharing opens the door; closing it shuts it again.
        l.set_share_detail("mac:callado", true).unwrap();
        assert_eq!(l.events(&EventFilter::default()).unwrap().len(), 4);
        assert_eq!(l.shared_event_count().unwrap(), 4);
        l.set_share_detail("mac:callado", false).unwrap();
        assert_eq!(l.events(&EventFilter::default()).unwrap().len(), 2);
    }

    /// The weekly report's new destinations carry a name and a device: the same rule applies.
    #[test]
    fn new_destinations_skip_a_device_that_did_not_share() {
        use crate::time::DAY_MS;
        let mut l = Ledger::open_in_memory(genesis()).unwrap();
        let now = 30 * DAY_MS;
        l.upsert_device("mac:callado", Some("aa:01"), Some("10.0.0.2"), now - DAY_MS)
            .unwrap();
        l.first_time("mac:callado", "clinica.example", now - DAY_MS)
            .unwrap();
        l.append(phone(now - DAY_MS, "mac:callado", "clinica.example"))
            .unwrap();
        l.first_time(SELF_DEVICE_ID, "nuevo.example", now - DAY_MS)
            .unwrap();
        l.append(ev(now - DAY_MS, "nuevo.example")).unwrap();

        let shown: Vec<String> = l
            .new_destinations(now, 10)
            .unwrap()
            .into_iter()
            .map(|n| n.qname)
            .collect();
        assert_eq!(shown, ["nuevo.example"]);
        // The week's figures still count the phone.
        let week = l.week_summary(now).unwrap();
        assert!(week
            .devices
            .iter()
            .any(|d| d.device_id == "mac:callado" && d.queries == 1));

        l.set_share_detail("mac:callado", true).unwrap();
        assert_eq!(l.new_destinations(now, 10).unwrap().len(), 2);
    }
}
