//! Aggregates for the panel (brief §8): the 60-second counters, totals per
//! device and what is stored. Read-only.

use rusqlite::params;
use serde::Serialize;

use crate::error::Result;
use crate::ledger::Ledger;

/// The live counters of the radiography page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub struct Counters {
    /// Window start, Unix ms.
    pub since: i64,
    /// Window end, Unix ms.
    pub until: i64,
    /// Distinct names queried.
    pub services: i64,
    /// Distinct names categorised as `rastreador`.
    pub trackers: i64,
    /// Distinct names categorised as `publicidad`. It is the commonest category of all, so leaving it
    /// out of the live counters made the radiography say "18 services · 0 trackers · 0 normal"
    /// for a page full of advertising (21 Sep 2026).
    pub ads: i64,
    /// Distinct names carrying `destino_nuevo`.
    pub new_destinations: i64,
    /// Distinct names categorised as `esperado`.
    pub expected: i64,
    /// Distinct names with verdict `cortado`.
    pub blocked: i64,
}

/// Totals for one device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct DeviceTotals {
    /// Device id.
    pub device_id: String,
    /// All queries.
    pub queries: i64,
    /// Per category.
    pub rastreador: i64,
    /// Per category.
    pub publicidad: i64,
    /// Per category.
    pub telemetria: i64,
    /// Per category.
    pub esperado: i64,
    /// Per category.
    pub desconocido: i64,
    /// Verdict `cortado`.
    pub cortado: i64,
}

/// How many rows each table holds: what Guardiana stores about you.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub struct TableCounts {
    /// Query events.
    pub events: i64,
    /// Devices.
    pub devices: i64,
    /// Rules, undone ones included.
    pub rules: i64,
    /// Outbound connections made by the program.
    pub outbound: i64,
    /// Distinct (device, name) pairs seen.
    pub seen_domains: i64,
}

impl Ledger {
    /// Counters over `since <= ts < until`.
    pub fn counters(&self, since: i64, until: i64) -> Result<Counters> {
        let row = self.conn.query_row(
            // Every counter counts names, like the first one: until 1.0.2 the first counted
            // names and the rest counted queries, so the radiography read «65 distinct services
            // · 101 advertising» and the shared text «885 trackers» for ten tracker names
            // (review of 28 Sep 2026, G1).
            "SELECT COUNT(DISTINCT qname), \
                    COUNT(DISTINCT CASE WHEN category = 'rastreador' THEN qname END), \
                    COUNT(DISTINCT CASE WHEN category = 'publicidad' THEN qname END), \
                    COUNT(DISTINCT CASE WHEN instr(signals_json, '\"destino_nuevo\"') > 0 \
                                        THEN qname END), \
                    COUNT(DISTINCT CASE WHEN category = 'esperado' THEN qname END), \
                    COUNT(DISTINCT CASE WHEN verdict = 'cortado' THEN qname END) \
             FROM events WHERE ts >= ?1 AND ts < ?2",
            params![since, until],
            |r| {
                Ok(Counters {
                    since,
                    until,
                    services: r.get::<_, i64>(0)?,
                    trackers: r.get::<_, Option<i64>>(1)?.unwrap_or(0),
                    ads: r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                    new_destinations: r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                    expected: r.get::<_, Option<i64>>(4)?.unwrap_or(0),
                    blocked: r.get::<_, Option<i64>>(5)?.unwrap_or(0),
                })
            },
        )?;
        Ok(row)
    }

    /// Totals per device over retained events, optionally only `ts >= since`.
    pub fn device_totals(&self, since: Option<i64>) -> Result<Vec<DeviceTotals>> {
        let mut stmt = self.conn.prepare(
            "SELECT device_id, COUNT(*), \
                    SUM(category = 'rastreador'), SUM(category = 'publicidad'), \
                    SUM(category = 'telemetria'), SUM(category = 'esperado'), \
                    SUM(category = 'desconocido'), SUM(verdict = 'cortado') \
             FROM events WHERE ts >= ?1 GROUP BY device_id ORDER BY COUNT(*) DESC",
        )?;
        let rows = stmt.query_map([since.unwrap_or(0)], |r| {
            Ok(DeviceTotals {
                device_id: r.get(0)?,
                queries: r.get(1)?,
                rastreador: r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                publicidad: r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                telemetria: r.get::<_, Option<i64>>(4)?.unwrap_or(0),
                esperado: r.get::<_, Option<i64>>(5)?.unwrap_or(0),
                desconocido: r.get::<_, Option<i64>>(6)?.unwrap_or(0),
                cortado: r.get::<_, Option<i64>>(7)?.unwrap_or(0),
            })
        })?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }

    /// [`Self::device_totals`] for one device, read through the `(device_id, ts)` index instead
    /// of the whole table. `None` when the device has no rows since `since`.
    pub fn device_totals_of(
        &self,
        device_id: &str,
        since: Option<i64>,
    ) -> Result<Option<DeviceTotals>> {
        let mut stmt = self.conn.prepare(
            "SELECT COUNT(*), \
                    SUM(category = 'rastreador'), SUM(category = 'publicidad'), \
                    SUM(category = 'telemetria'), SUM(category = 'esperado'), \
                    SUM(category = 'desconocido'), SUM(verdict = 'cortado') \
             FROM events WHERE device_id = ?1 AND ts >= ?2",
        )?;
        let t = stmt.query_row(rusqlite::params![device_id, since.unwrap_or(0)], |r| {
            Ok(DeviceTotals {
                device_id: device_id.to_owned(),
                queries: r.get(0)?,
                rastreador: r.get::<_, Option<i64>>(1)?.unwrap_or(0),
                publicidad: r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                telemetria: r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                esperado: r.get::<_, Option<i64>>(4)?.unwrap_or(0),
                desconocido: r.get::<_, Option<i64>>(5)?.unwrap_or(0),
                cortado: r.get::<_, Option<i64>>(6)?.unwrap_or(0),
            })
        })?;
        Ok((t.queries > 0).then_some(t))
    }

    /// Row counts of every table.
    pub fn table_counts(&self) -> Result<TableCounts> {
        let count = |sql: &str| -> Result<i64> { Ok(self.conn.query_row(sql, [], |r| r.get(0))?) };
        Ok(TableCounts {
            events: count("SELECT COUNT(*) FROM events")?,
            devices: count("SELECT COUNT(*) FROM devices")?,
            rules: count("SELECT COUNT(*) FROM rules")?,
            outbound: count("SELECT COUNT(*) FROM outbound")?,
            seen_domains: count("SELECT COUNT(*) FROM seen_domains")?,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::hash::Hash;
    use crate::model::{Category, NewEvent, Signal, Verdict};

    #[test]
    fn counters_and_totals() {
        let mut l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        let mut a = NewEvent::observed(10, "self", "127.0.0.1", "t.example", "A");
        a.category = Category::Rastreador;
        a.signals = vec![Signal::DestinoNuevo];
        l.append(a).unwrap();
        let mut b = NewEvent::observed(20, "self", "127.0.0.1", "u.example", "A");
        b.category = Category::Esperado;
        b.verdict = Verdict::Cortado;
        l.append(b).unwrap();
        l.append(NewEvent::observed(
            30,
            "mac:aa",
            "10.0.0.2",
            "t.example",
            "A",
        ))
        .unwrap();
        // Advertising is its own counter: it is the commonest category, and while it was missing
        // the radiography put those queries in no bucket at all.
        let mut d = NewEvent::observed(40, "self", "127.0.0.1", "ads.example", "A");
        d.category = Category::Publicidad;
        l.append(d).unwrap();

        let c = l.counters(0, 100).unwrap();
        assert_eq!(c.services, 3);
        assert_eq!(c.trackers, 1);
        assert_eq!(c.ads, 1);
        assert_eq!(c.new_destinations, 1);
        assert_eq!(c.expected, 1);
        assert_eq!(c.blocked, 1);
        // From t=25 on there are two names left: the tracker asked again and the advertising one.
        assert_eq!(l.counters(25, 100).unwrap().services, 2);

        let t = l.device_totals(None).unwrap();
        assert_eq!(t[0].device_id, "self");
        assert_eq!(t[0].queries, 3);
        assert_eq!(t[0].rastreador, 1);
        assert_eq!(t[0].publicidad, 1);
        assert_eq!(t[0].cortado, 1);
        assert_eq!(t[1].desconocido, 1);
        // One device's totals are the same numbers, read through its index.
        assert_eq!(
            l.device_totals_of("self", None).unwrap().as_ref(),
            Some(&t[0])
        );
        assert_eq!(
            l.device_totals_of("mac:aa", None).unwrap().as_ref(),
            Some(&t[1])
        );
        assert_eq!(
            l.device_totals_of("self", Some(25))
                .unwrap()
                .map(|x| x.queries),
            Some(1)
        );
        assert!(l.device_totals_of("ip:10.0.0.9", None).unwrap().is_none());

        let n = l.table_counts().unwrap();
        assert_eq!(n.events, 4);

        // A tracker asked a hundred times is one tracker, not a hundred.
        for i in 0..100 {
            let mut e = NewEvent::observed(50 + i, "self", "127.0.0.1", "t.example", "A");
            e.category = Category::Rastreador;
            l.append(e).unwrap();
        }
        assert_eq!(l.counters(0, 200).unwrap().trackers, 1);
    }
}
