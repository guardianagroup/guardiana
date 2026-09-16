//! Heartbeat and gaps (brief §4): while Guardiana runs it updates a
//! heartbeat every few seconds; when it starts and finds an old heartbeat,
//! it records "Guardiana no estaba vigilando entre X e Y". Gaps are not
//! query events, so they live in their own small table (decision 27).

use rusqlite::params;
use serde::Serialize;

use crate::error::Result;
use crate::ledger::Ledger;

/// Settings key: Unix ms of the last heartbeat.
pub const KEY_HEARTBEAT: &str = "last_alive";
/// A heartbeat older than this at start means the service was down.
pub const GAP_THRESHOLD_MS: i64 = 90_000;

/// A period during which Guardiana was not watching.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Gap {
    /// Row id.
    pub id: i64,
    /// Last heartbeat before the gap, Unix ms.
    pub from_ts: i64,
    /// When Guardiana came back, Unix ms.
    pub to_ts: i64,
}

const GAPS_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS gaps (
    id      INTEGER PRIMARY KEY AUTOINCREMENT,
    from_ts INTEGER NOT NULL,
    to_ts   INTEGER NOT NULL
);
";

impl Ledger {
    pub(crate) fn ensure_gaps_table(&self) -> Result<()> {
        self.conn.execute_batch(GAPS_SCHEMA)?;
        Ok(())
    }

    /// Record the heartbeat.
    pub fn heartbeat(&self, now: i64) -> Result<()> {
        self.set_setting(KEY_HEARTBEAT, &now.to_string())
    }

    /// At start: if the previous heartbeat is old, record the gap and return it.
    pub fn record_gap_since_last_heartbeat(&mut self, now: i64) -> Result<Option<Gap>> {
        let last = self
            .setting(KEY_HEARTBEAT)?
            .and_then(|v| v.parse::<i64>().ok());
        let gap = match last {
            Some(last) if now - last > GAP_THRESHOLD_MS => {
                self.conn.execute(
                    "INSERT INTO gaps (from_ts, to_ts) VALUES (?1, ?2)",
                    params![last, now],
                )?;
                Some(Gap {
                    id: self.conn.last_insert_rowid(),
                    from_ts: last,
                    to_ts: now,
                })
            }
            _ => None,
        };
        self.heartbeat(now)?;
        Ok(gap)
    }

    /// Gaps, newest first.
    pub fn gaps(&self) -> Result<Vec<Gap>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, from_ts, to_ts FROM gaps ORDER BY id DESC")?;
        let rows = stmt.query_map([], |r| {
            Ok(Gap {
                id: r.get(0)?,
                from_ts: r.get(1)?,
                to_ts: r.get(2)?,
            })
        })?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::hash::Hash;

    #[test]
    fn gap_is_recorded_only_when_heartbeat_is_old() {
        let mut l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        assert_eq!(l.record_gap_since_last_heartbeat(1_000).unwrap(), None);
        l.heartbeat(2_000).unwrap();
        assert_eq!(l.record_gap_since_last_heartbeat(3_000).unwrap(), None);
        let g = l.record_gap_since_last_heartbeat(500_000).unwrap().unwrap();
        assert_eq!(g.from_ts, 3_000);
        assert_eq!(g.to_ts, 500_000);
        assert_eq!(l.gaps().unwrap().len(), 1);
    }
}
