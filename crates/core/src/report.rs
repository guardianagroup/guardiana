//! The weekly household report (brief §8, HOGAR.md §4): totals per device
//! over the last seven days, combining the detail still kept in `events`
//! with the daily totals that retention already rolled up. The two never
//! overlap: pruning moves rows from one to the other.

use std::collections::BTreeMap;

use rusqlite::params;
use serde::Serialize;

use crate::error::Result;
use crate::ledger::Ledger;
use crate::time::{day_utc, DAY_MS};

/// One device's week.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct DeviceWeek {
    /// Device id.
    pub device_id: String,
    /// Name given by the user, if any.
    pub name: Option<String>,
    /// All queries.
    pub queries: i64,
    /// `rastreador`.
    pub trackers: i64,
    /// `publicidad`.
    pub ads: i64,
    /// `telemetria`.
    pub telemetry: i64,
    /// `esperado`.
    pub expected: i64,
    /// `desconocido`.
    pub unknown: i64,
    /// Verdict `cortado`.
    pub blocked: i64,
}

/// The household's week.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct WeekSummary {
    /// Window start, Unix ms (seven days before `until`).
    pub since: i64,
    /// Window end, Unix ms.
    pub until: i64,
    /// Per device, most queries first.
    pub devices: Vec<DeviceWeek>,
    /// Sum over devices.
    pub total: DeviceWeek,
}

fn add(target: &mut DeviceWeek, category: &str, verdict: &str, n: i64) {
    target.queries += n;
    match category {
        "rastreador" => target.trackers += n,
        "publicidad" => target.ads += n,
        "telemetria" => target.telemetry += n,
        "esperado" => target.expected += n,
        _ => target.unknown += n,
    }
    if verdict == "cortado" {
        target.blocked += n;
    }
}

impl Ledger {
    /// Totals for the seven days ending at `until`.
    pub fn week_summary(&self, until: i64) -> Result<WeekSummary> {
        let since = until - 7 * DAY_MS;
        let mut per: BTreeMap<String, DeviceWeek> = BTreeMap::new();

        // Detail still retained.
        {
            let mut stmt = self.conn.prepare(
                "SELECT device_id, category, verdict, COUNT(*) FROM events \
                 WHERE ts >= ?1 AND ts < ?2 GROUP BY device_id, category, verdict",
            )?;
            let rows = stmt.query_map(params![since, until], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })?;
            for row in rows {
                let (device, category, verdict, n) = row?;
                let entry = per.entry(device.clone()).or_insert_with(|| DeviceWeek {
                    device_id: device,
                    ..DeviceWeek::default()
                });
                add(entry, &category, &verdict, n);
            }
        }

        // Days already rolled up by retention.
        {
            let first_day = day_utc(since);
            let mut stmt = self.conn.prepare(
                "SELECT device_id, category, verdict, count FROM daily_totals WHERE day >= ?1",
            )?;
            let rows = stmt.query_map([first_day], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })?;
            for row in rows {
                let (device, category, verdict, n) = row?;
                let entry = per.entry(device.clone()).or_insert_with(|| DeviceWeek {
                    device_id: device,
                    ..DeviceWeek::default()
                });
                add(entry, &category, &verdict, n);
            }
        }

        let names: BTreeMap<String, Option<String>> = self
            .devices()?
            .into_iter()
            .map(|d| (d.id, d.name))
            .collect();
        let mut total = DeviceWeek {
            device_id: "total".to_owned(),
            ..DeviceWeek::default()
        };
        let mut devices: Vec<DeviceWeek> = per
            .into_values()
            .map(|mut d| {
                d.name = names.get(&d.device_id).cloned().flatten();
                total.queries += d.queries;
                total.trackers += d.trackers;
                total.ads += d.ads;
                total.telemetry += d.telemetry;
                total.expected += d.expected;
                total.unknown += d.unknown;
                total.blocked += d.blocked;
                d
            })
            .collect();
        devices.sort_by(|a, b| {
            b.queries
                .cmp(&a.queries)
                .then(a.device_id.cmp(&b.device_id))
        });
        Ok(WeekSummary {
            since,
            until,
            devices,
            total,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::hash::Hash;
    use crate::model::{Category, NewEvent, Verdict};
    use crate::retention::Retention;
    use crate::time::HOUR_MS;

    #[test]
    fn week_combines_detail_and_rolled_up_days() {
        let mut l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        let day0 = 10 * DAY_MS;
        // Three days ago: two trackers from the phone, one blocked.
        for i in 0..2 {
            let mut e = NewEvent::observed(
                day0 + 3 * DAY_MS + i,
                "mac:phone",
                "10.0.0.2",
                "t.example",
                "A",
            );
            e.category = Category::Rastreador;
            if i == 0 {
                e.verdict = Verdict::Cortado;
            }
            l.append(e).unwrap();
        }
        // Ten days ago: outside the week, must not count.
        let mut old =
            NewEvent::observed(day0 - 3 * DAY_MS, "mac:phone", "10.0.0.2", "t.example", "A");
        old.category = Category::Rastreador;
        l.append(old).unwrap();
        // Now: one expected query from this computer.
        let now = day0 + 6 * DAY_MS + 2 * HOUR_MS;
        let mut e = NewEvent::observed(now - HOUR_MS, "self", "127.0.0.1", "u.example", "A");
        e.category = Category::Esperado;
        l.append(e).unwrap();

        // Retention rolls the older rows into daily totals (free plan: 24 h detail).
        l.prune(Retention::FREE, now).unwrap();
        assert_eq!(l.event_count().unwrap(), 1);

        let w = l.week_summary(now).unwrap();
        assert_eq!(w.total.queries, 3);
        assert_eq!(w.total.trackers, 2);
        assert_eq!(w.total.blocked, 1);
        assert_eq!(w.total.expected, 1);
        assert_eq!(w.devices[0].device_id, "mac:phone");
        assert_eq!(w.devices[0].trackers, 2);
        assert_eq!(w.devices[1].device_id, "self");
    }
}
