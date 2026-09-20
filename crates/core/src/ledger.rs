//! The SQLite ledger: one database per installation, `events` chained by hash.
//!
//! Chain anchor. Detail rows are pruned after the retention period (brief
//! §3), always as a prefix by id. The `row_hash` of the last pruned row is
//! kept in `settings` as the *anchor*, so the surviving rows still form a
//! verifiable chain that starts at the anchor. Before anything is pruned the
//! anchor is the *genesis*: the hash of the installer's public key.

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::error::{Error, Result};
use crate::hash::{chain_hash, Hash, HashInput};
use crate::model::SignalKind;
use crate::model::{
    Action, Category, Device, Event, MatchKind, NewEvent, NewRule, Outbound, Purpose, Rule, Scope,
    Signal, Verdict,
};

/// Settings key: hash of the public key this database was created for.
pub(crate) const KEY_GENESIS: &str = "chain_genesis";
/// Settings key: current chain anchor (genesis until something is pruned).
pub(crate) const KEY_ANCHOR: &str = "chain_anchor";
/// Settings key: how many events were pruned so far (informational).
pub(crate) const KEY_PRUNED: &str = "chain_pruned_events";

const SCHEMA: &str = "
PRAGMA journal_mode = WAL;
PRAGMA busy_timeout = 5000;
PRAGMA foreign_keys = ON;
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
        Ok(self
            .conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()?)
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

    pub(crate) fn pruned_count(&self) -> Result<u64> {
        Ok(self
            .setting(KEY_PRUNED)?
            .and_then(|s| s.parse().ok())
            .unwrap_or(0))
    }

    // ----- events ---------------------------------------------------------

    /// Append one event, computing `prev_hash` and `row_hash` atomically.
    pub fn append(&mut self, new: NewEvent) -> Result<Event> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let prev: Option<Vec<u8>> = tx
            .query_row(
                "SELECT row_hash FROM events ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?;
        let prev_hash = match prev {
            Some(b) => Hash::from_bytes(&b)?,
            None => {
                let hex: String = tx.query_row(
                    "SELECT value FROM settings WHERE key = ?1",
                    [KEY_ANCHOR],
                    |r| r.get(0),
                )?;
                Hash::from_hex(&hex)?
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

    /// List events matching `filter`, oldest first.
    pub fn events(&self, filter: &EventFilter) -> Result<Vec<Event>> {
        let mut sql = format!("SELECT {EVENT_COLUMNS} FROM events WHERE 1=1");
        let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(d) = &filter.device_id {
            sql.push_str(" AND device_id = ?");
            args.push(Box::new(d.clone()));
        }
        if let Some(c) = filter.category {
            sql.push_str(" AND category = ?");
            args.push(Box::new(c.as_str()));
        }
        if let Some(v) = filter.verdict {
            sql.push_str(" AND verdict = ?");
            args.push(Box::new(v.as_str()));
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

    /// Walk every surviving row from the anchor and verify both links.
    pub fn check(&self) -> Result<CheckReport> {
        let genesis = self.setting(KEY_GENESIS)?.unwrap_or_default();
        let anchor = self.anchor()?;
        let pruned = self.pruned_count()?;
        let mut report = CheckReport {
            checked: 0,
            pruned,
            anchor_is_genesis: anchor.to_hex() == genesis,
            first_fault: None,
        };
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {EVENT_COLUMNS} FROM events ORDER BY id ASC"
        ))?;
        let mut rows = stmt.query([])?;
        let mut expected = anchor;
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
            report.checked += 1;
        }
        Ok(report)
    }

    /// Delete everything and reset the anchor to the genesis. Irreversible;
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
        tx.commit()?;
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

    #[test]
    fn wipe_resets_to_genesis() {
        let mut l = Ledger::open_in_memory(genesis()).unwrap();
        l.append(ev(1, "a.example")).unwrap();
        l.wipe().unwrap();
        assert_eq!(l.event_count().unwrap(), 0);
        let a = l.append(ev(2, "b.example")).unwrap();
        assert_eq!(a.id, 1);
        assert_eq!(a.prev_hash, genesis());
        assert!(l.check().unwrap().is_ok());
    }
}
