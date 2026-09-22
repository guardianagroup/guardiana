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

        // Days already rolled up by retention. Bounded above as well as below so the same
        // function can be asked for the week BEFORE this one without dragging in this one's
        // totals; `until`'s own day is never rolled up (retention keeps 24 h of detail), so it
        // comes from `events` above and is not lost by the upper bound.
        {
            let first_day = day_utc(since);
            let last_day = day_utc(until);
            let mut stmt = self.conn.prepare(
                "SELECT device_id, category, verdict, count FROM daily_totals \
                 WHERE day >= ?1 AND day < ?2",
            )?;
            let rows = stmt.query_map(params![first_day, last_day], |r| {
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

    /// Destinations the house had never asked for before the seven days ending at `until`.
    ///
    /// `seen_domains` remembers when each (device, name) pair first appeared and survives
    /// pruning; the count, the device and the category come from `events`. With the free plan's
    /// 24 hours of detail there is nothing left to count after a day, which is why the panel
    /// only offers this with Plus: it is not a feature held back, it is a question that cannot
    /// be answered without the memory Plus keeps.
    pub fn new_destinations(&self, until: i64, limit: usize) -> Result<Vec<NewDestination>> {
        let since = until - 7 * DAY_MS;
        let names: BTreeMap<String, Option<String>> = self
            .devices()?
            .into_iter()
            .map(|d| (d.id, d.name))
            .collect();
        let mut stmt = self.conn.prepare(
            "SELECT s.qname, s.device_id, s.first_seen, COUNT(e.id) AS n, \
                    (SELECT e2.category FROM events e2 \
                      WHERE e2.qname = s.qname AND e2.device_id = s.device_id \
                      ORDER BY e2.id DESC LIMIT 1) AS cat \
             FROM seen_domains s \
             JOIN events e ON e.qname = s.qname AND e.device_id = s.device_id \
                          AND e.ts >= ?1 AND e.ts < ?2 \
             WHERE s.first_seen >= ?1 AND s.first_seen < ?2 \
             GROUP BY s.qname, s.device_id \
             ORDER BY n DESC, s.first_seen DESC \
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![since, until, limit as i64], |r| {
            Ok(NewDestination {
                qname: r.get(0)?,
                device_id: r.get(1)?,
                name: None,
                first_seen: r.get(2)?,
                queries: r.get(3)?,
                category: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            let mut d = row?;
            d.name = names.get(&d.device_id).cloned().flatten();
            out.push(d);
        }
        Ok(out)
    }
}

/// A destination asked for this week and never before.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct NewDestination {
    /// The name asked for.
    pub qname: String,
    /// Which device asked.
    pub device_id: String,
    /// Name the user gave that device, if any.
    pub name: Option<String>,
    /// Category of its most recent query.
    pub category: String,
    /// Times it was asked for inside the window.
    pub queries: i64,
    /// When it first appeared, Unix ms.
    pub first_seen: i64,
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

    /// What Plus answers and the free plan cannot: which destinations are new this week.
    #[test]
    fn new_destinations_are_the_ones_never_asked_for_before() {
        let mut l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        let day0 = 1_700_000_000_000i64;
        let now = day0 + 30 * DAY_MS;
        // An old acquaintance: first seen a month ago, still busy this week.
        l.first_time("self", "viejo.example", day0).unwrap();
        let mut viejo = NewEvent::observed(day0, "self", "127.0.0.1", "viejo.example", "A");
        viejo.category = Category::Rastreador;
        l.append(viejo).unwrap();
        for i in 0..5 {
            let mut e = NewEvent::observed(
                now - (i + 1) * HOUR_MS,
                "self",
                "127.0.0.1",
                "viejo.example",
                "A",
            );
            e.category = Category::Rastreador;
            l.append(e).unwrap();
        }
        // New this week, asked for three times.
        l.first_time("self", "nuevo.example", now - 2 * DAY_MS)
            .unwrap();
        for i in 0..3 {
            let mut e = NewEvent::observed(
                now - 2 * DAY_MS + i * HOUR_MS,
                "self",
                "127.0.0.1",
                "nuevo.example",
                "A",
            );
            e.category = Category::Publicidad;
            l.append(e).unwrap();
        }
        let n = l.new_destinations(now, 10).unwrap();
        assert_eq!(n.len(), 1, "{n:?}");
        assert_eq!(n[0].qname, "nuevo.example");
        assert_eq!(n[0].queries, 3);
        assert_eq!(n[0].category, "publicidad");
        assert_eq!(n[0].device_id, "self");
    }

    /// The week before this one, asked of the same function: it must not drag in this week's
    /// rolled-up totals, or the comparison Plus offers would always say "the same".
    ///
    /// Pruned with the Plus policy on purpose. With the free one the answer is zero whatever the
    /// query does, because a week-old total has already been deleted: that is why the comparison
    /// is a Plus answer and not a switch we could flip.
    #[test]
    fn the_previous_week_does_not_include_this_one() {
        let mut l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        let day0 = 1_700_000_000_000i64;
        let now = day0 + 20 * DAY_MS;
        // Two queries last week, one this week.
        for ts in [now - 9 * DAY_MS, now - 8 * DAY_MS, now - 2 * DAY_MS] {
            let mut e = NewEvent::observed(ts, "self", "127.0.0.1", "a.example", "A");
            e.category = Category::Rastreador;
            l.append(e).unwrap();
        }
        l.prune(Retention::UNLIMITED, now).unwrap();
        assert_eq!(l.week_summary(now).unwrap().total.queries, 1);
        assert_eq!(l.week_summary(now - 7 * DAY_MS).unwrap().total.queries, 2);
        // And with the free policy the older week is simply gone.
        l.prune(Retention::FREE, now).unwrap();
        assert_eq!(l.week_summary(now - 7 * DAY_MS).unwrap().total.queries, 0);
    }
}
