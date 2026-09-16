//! Retention (brief §3): free plan keeps detail for 24 h and daily totals for
//! 7 days; the Home plan is configurable, unlimited by default. Pruning
//! always removes a prefix of the chain and moves the anchor forward, so
//! `ledger --check` keeps working on what remains.

use rusqlite::params;

use crate::error::Result;
use crate::hash::Hash;
use crate::ledger::{Ledger, KEY_ANCHOR, KEY_PRUNED};
use crate::time::{day_utc, DAY_MS, HOUR_MS};

/// How long to keep things, in milliseconds. `None` means forever.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Retention {
    /// Per-query detail (`events`).
    pub detail_ms: Option<i64>,
    /// Daily totals per device and category.
    pub totals_ms: Option<i64>,
}

impl Retention {
    /// Free plan: 24 h of detail, 7 days of totals.
    pub const FREE: Self = Self {
        detail_ms: Some(24 * HOUR_MS),
        totals_ms: Some(7 * DAY_MS),
    };

    /// Home plan default: nothing is pruned.
    pub const UNLIMITED: Self = Self {
        detail_ms: None,
        totals_ms: None,
    };
}

/// What one pruning pass did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PruneReport {
    /// Events rolled into `daily_totals` and deleted.
    pub events_pruned: u64,
    /// Rows of `daily_totals` deleted.
    pub totals_pruned: u64,
}

/// One day's totals for a device, as kept after detail is pruned.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DailyTotal {
    /// `YYYY-MM-DD` in UTC.
    pub day: String,
    /// Device.
    pub device_id: String,
    /// Category text.
    pub category: String,
    /// Verdict text.
    pub verdict: String,
    /// Number of queries.
    pub count: i64,
}

impl Ledger {
    /// Apply `policy` as of `now`. Meant to run hourly from the service.
    pub fn prune(&mut self, policy: Retention, now: i64) -> Result<PruneReport> {
        let mut report = PruneReport::default();
        let tx = self.conn.transaction()?;

        if let Some(keep) = policy.detail_ms {
            let cutoff = now - keep;
            // Prune by id prefix so the chain stays contiguous, even if the
            // clock once went backwards and a newer id carries an older ts.
            let boundary: Option<i64> = tx
                .query_row("SELECT MAX(id) FROM events WHERE ts < ?1", [cutoff], |r| {
                    r.get(0)
                })
                .optional_flat()?;
            if let Some(last_id) = boundary {
                tx.execute(
                    "INSERT INTO daily_totals (day, device_id, category, verdict, count) \
                     SELECT strftime('%Y-%m-%d', ts / 1000, 'unixepoch'), device_id, category, \
                            verdict, COUNT(*) \
                     FROM events WHERE id <= ?1 \
                     GROUP BY 1, 2, 3, 4 \
                     ON CONFLICT(day, device_id, category, verdict) \
                     DO UPDATE SET count = count + excluded.count",
                    [last_id],
                )?;
                let last_hash: Vec<u8> = tx.query_row(
                    "SELECT row_hash FROM events WHERE id = ?1",
                    [last_id],
                    |r| r.get(0),
                )?;
                let anchor = Hash::from_bytes(&last_hash)?;
                let deleted = tx.execute("DELETE FROM events WHERE id <= ?1", [last_id])?;
                report.events_pruned = deleted as u64;
                tx.execute(
                    "UPDATE settings SET value = ?1 WHERE key = ?2",
                    params![anchor.to_hex(), KEY_ANCHOR],
                )?;
                tx.execute(
                    "UPDATE settings SET value = CAST(CAST(value AS INTEGER) + ?1 AS TEXT) \
                     WHERE key = ?2",
                    params![deleted as i64, KEY_PRUNED],
                )?;
            }
        }

        if let Some(keep) = policy.totals_ms {
            let cutoff_day = day_utc(now - keep);
            let deleted = tx.execute("DELETE FROM daily_totals WHERE day < ?1", [cutoff_day])?;
            report.totals_pruned = deleted as u64;
        }

        tx.commit()?;
        Ok(report)
    }

    /// Daily totals kept after pruning, oldest day first.
    pub fn daily_totals(&self) -> Result<Vec<DailyTotal>> {
        let mut stmt = self.conn.prepare(
            "SELECT day, device_id, category, verdict, count FROM daily_totals \
             ORDER BY day ASC, device_id ASC, category ASC, verdict ASC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(DailyTotal {
                day: r.get(0)?,
                device_id: r.get(1)?,
                category: r.get(2)?,
                verdict: r.get(3)?,
                count: r.get(4)?,
            })
        })?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }
}

/// `MAX(id)` over an empty table yields one row holding NULL; fold that into `None`.
trait OptionalFlat<T> {
    fn optional_flat(self) -> rusqlite::Result<Option<T>>;
}

impl<T> OptionalFlat<T> for rusqlite::Result<Option<T>> {
    fn optional_flat(self) -> rusqlite::Result<Option<T>> {
        match self {
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            other => other,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::model::{Category, NewEvent};

    fn ev(ts: i64, device: &str, cat: Category) -> NewEvent {
        let mut e = NewEvent::observed(ts, device, "10.0.0.2", "x.example", "A");
        e.category = cat;
        e
    }

    #[test]
    fn prune_rolls_up_and_moves_anchor_but_chain_still_checks() {
        let mut l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        let day0 = 0;
        l.append(ev(day0 + 1, "self", Category::Rastreador))
            .unwrap();
        l.append(ev(day0 + 2, "self", Category::Rastreador))
            .unwrap();
        let kept_from = l
            .append(ev(day0 + 30 * HOUR_MS, "self", Category::Esperado))
            .unwrap();
        l.append(ev(day0 + 31 * HOUR_MS, "mac:aa", Category::Desconocido))
            .unwrap();

        let now = day0 + 40 * HOUR_MS; // detail cutoff = 16 h → first two rows go
        let r = l.prune(Retention::FREE, now).unwrap();
        assert_eq!(r.events_pruned, 2);
        assert_eq!(l.event_count().unwrap(), 2);

        let check = l.check().unwrap();
        assert!(check.is_ok(), "{check:?}");
        assert_eq!(check.checked, 2);
        assert_eq!(check.pruned, 2);
        assert!(!check.anchor_is_genesis);
        // The surviving chain starts at the last pruned row's hash.
        let first = &l.events(&Default::default()).unwrap()[0];
        assert_eq!(first.id, kept_from.id);
        assert_eq!(first.prev_hash, l.anchor().unwrap());

        let totals = l.daily_totals().unwrap();
        assert_eq!(totals.len(), 1);
        assert_eq!(totals[0].day, "1970-01-01");
        assert_eq!(totals[0].category, "rastreador");
        assert_eq!(totals[0].count, 2);

        // A second pass with nothing old is a no-op.
        let r2 = l.prune(Retention::FREE, now).unwrap();
        assert_eq!(r2.events_pruned, 0);
        assert!(l.check().unwrap().is_ok());

        // Nine days later: the two remaining rows roll up into day 2, and
        // every total older than 7 days (day 1 and day 2, three rows) expires.
        let r3 = l.prune(Retention::FREE, day0 + 9 * DAY_MS).unwrap();
        assert_eq!(r3.events_pruned, 2);
        assert_eq!(r3.totals_pruned, 3);
        assert!(l.daily_totals().unwrap().is_empty());
        assert_eq!(l.event_count().unwrap(), 0);
        assert!(l.check().unwrap().is_ok());
    }

    #[test]
    fn unlimited_never_prunes() {
        let mut l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        l.append(ev(1, "self", Category::Publicidad)).unwrap();
        let r = l.prune(Retention::UNLIMITED, 100 * DAY_MS).unwrap();
        assert_eq!(r, PruneReport::default());
        assert_eq!(l.event_count().unwrap(), 1);
    }

    #[test]
    fn appending_after_prune_continues_from_anchor() {
        let mut l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        l.append(ev(1, "self", Category::Publicidad)).unwrap();
        l.prune(Retention::FREE, 3 * DAY_MS).unwrap();
        assert_eq!(l.event_count().unwrap(), 0);
        let e = l
            .append(ev(3 * DAY_MS, "self", Category::Publicidad))
            .unwrap();
        assert_eq!(e.prev_hash, l.anchor().unwrap());
        assert!(l.check().unwrap().is_ok());
    }
}
