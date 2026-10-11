//! The rows behind the weekly report's measurements (decision 205): «mientras dormías», the
//! companies that could tell a device was online each day, how often the busiest trackers call,
//! and the names dressed as another. Everything here is read from the ledger as it is; nothing
//! new is stored for it. Like every listing by name, only the rows the household may read: this
//! computer and the devices whose owner shares their detail (the rule of [`Ledger::events`]).

use rusqlite::params;
use serde::Serialize;

use crate::error::Result;
use crate::ledger::Ledger;
use crate::model::Signal;
use crate::time::DAY_MS;
use crate::SELF_DEVICE_ID;

/// The household's rows: this computer, and the devices that share their detail.
const COMPARTIDOS: &str = "(device_id = ?1 OR device_id IN \
                           (SELECT id FROM devices WHERE share_detail_with_home != 0))";

/// The local day a moment falls in, as a whole number of days since 1 January 1970 in the zone
/// `offset_min` minutes east of UTC. One fixed offset for the whole window: a change of summer
/// time inside it moves those hours by one, and the report says it reads one offset.
#[must_use]
pub fn local_day_index(ts: i64, offset_min: i64) -> i64 {
    (ts + offset_min * 60_000).div_euclid(DAY_MS)
}

/// Unix ms of the local midnight that starts day `index` (see [`local_day_index`]).
#[must_use]
pub fn local_midnight(index: i64, offset_min: i64) -> i64 {
    index * DAY_MS - offset_min * 60_000
}

/// One query to a tracker, advertising or telemetry name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrackerRow {
    /// Which device asked.
    pub device_id: String,
    /// Unix ms.
    pub ts: i64,
    /// The name asked for.
    pub qname: String,
    /// `rastreador`, `publicidad` or `telemetria`.
    pub category: String,
    /// Whether GUARDIANA cut it (it never left the house).
    pub cut: bool,
    /// The interval of the `baliza` signal on this row, in minutes, when it fired (brief §5).
    pub beacon_minutes: Option<u32>,
    /// The tracker's site the name answered as, when it was disguised (decision 205), else empty.
    pub target: String,
}

/// How many forwarded queries a device made to a name on one local day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NameDay {
    /// Which device asked.
    pub device_id: String,
    /// Local day (see [`local_day_index`]).
    pub day: i64,
    /// The name asked for.
    pub qname: String,
    /// The tracker's site it answered as, when it was disguised (decision 205), else empty.
    pub target: String,
    /// Forwarded queries.
    pub queries: i64,
}

/// A name that answered as another site's tracker (decision 205), and how often.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CloakedCount {
    /// The name asked for.
    pub qname: String,
    /// The registrable site it answered as.
    pub target: String,
    /// Queries.
    pub queries: i64,
    /// Distinct devices that asked for it.
    pub devices: i64,
}

impl Ledger {
    /// Every query to a tracker, advertising or telemetry name from `since` to `until`, of the
    /// household's rows, by device and then in time order. A name that answered as another
    /// site's tracker (decision 205) is one of them too, whatever its own category says.
    pub fn tracker_rows(&self, since: i64, until: i64) -> Result<Vec<TrackerRow>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT device_id, ts, qname, category, verdict, \
                    CASE WHEN instr(signals_json, '\"baliza\"') > 0 THEN signals_json END, \
                    cname_target \
             FROM events \
             WHERE ts >= ?2 AND ts < ?3 \
               AND (category IN ('rastreador', 'publicidad', 'telemetria') OR cname_target != '') \
               AND {COMPARTIDOS} \
             ORDER BY device_id ASC, ts ASC, id ASC"
        ))?;
        let rows = stmt.query_map(params![SELF_DEVICE_ID, since, until], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, String>(6)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (device_id, ts, qname, category, verdict, senales, target) = row?;
            // Only the rows whose signals name the beacon are parsed; one that does not read is a
            // row without it, not a failed report.
            let beacon_minutes = senales.and_then(|s| {
                serde_json::from_str::<Vec<Signal>>(&s)
                    .ok()?
                    .into_iter()
                    .find_map(|x| match x {
                        Signal::Baliza { minutes } => Some(minutes),
                        _ => None,
                    })
            });
            out.push(TrackerRow {
                device_id,
                ts,
                qname,
                category,
                cut: verdict == "cortado",
                beacon_minutes,
                target,
            });
        }
        Ok(out)
    }

    /// The household's forwarded queries from `since` to `until`, by device, local day and
    /// name. Cut queries are not here: they never left the house.
    pub fn names_by_local_day(
        &self,
        since: i64,
        until: i64,
        offset_min: i64,
    ) -> Result<Vec<NameDay>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT device_id, (ts + ?4) / {DAY_MS} AS dia, qname, cname_target, COUNT(*) \
             FROM events \
             WHERE ts >= ?2 AND ts < ?3 AND verdict = 'observado' AND {COMPARTIDOS} \
             GROUP BY device_id, dia, qname, cname_target"
        ))?;
        let rows = stmt.query_map(
            params![SELF_DEVICE_ID, since, until, offset_min * 60_000],
            |r| {
                Ok(NameDay {
                    device_id: r.get(0)?,
                    day: r.get(1)?,
                    qname: r.get(2)?,
                    target: r.get(3)?,
                    queries: r.get(4)?,
                })
            },
        )?;
        rows.map(|r| r.map_err(crate::Error::from)).collect()
    }

    /// The names that answered as another site's tracker from `since` to `until` (decision 205),
    /// of the household's rows, most asked first.
    pub fn cloaked_names(&self, since: i64, until: i64) -> Result<Vec<CloakedCount>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT qname, cname_target, COUNT(*) AS n, COUNT(DISTINCT device_id) FROM events \
             WHERE ts >= ?2 AND ts < ?3 AND cname_target != '' AND {COMPARTIDOS} \
             GROUP BY qname, cname_target ORDER BY n DESC, qname ASC"
        ))?;
        let rows = stmt.query_map(params![SELF_DEVICE_ID, since, until], |r| {
            Ok(CloakedCount {
                qname: r.get(0)?,
                target: r.get(1)?,
                queries: r.get(2)?,
                devices: r.get(3)?,
            })
        })?;
        rows.map(|r| r.map_err(crate::Error::from)).collect()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::hash::Hash;
    use crate::model::{Category, NewEvent, Verdict};
    use crate::time::HOUR_MS;

    #[test]
    fn local_days_follow_the_offset() {
        // 2026-10-11 03:00 UTC is still the 10th in Bogotá (UTC-5).
        let ts = 1_791_687_600_000;
        assert_eq!(local_day_index(ts, 0) - local_day_index(ts, -300), 1);
        let d = local_day_index(ts, -300);
        let medianoche = local_midnight(d, -300);
        assert!(medianoche <= ts && ts < medianoche + DAY_MS);
        assert_eq!((ts - medianoche) / HOUR_MS, 22);
    }

    #[test]
    fn only_the_households_rows_and_only_what_was_asked() {
        let mut l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        let t0 = 1_800_000_000_000i64;
        l.upsert_device("mac:callado", None, Some("10.0.0.9"), t0)
            .unwrap();
        l.upsert_device("mac:comparte", None, Some("10.0.0.8"), t0)
            .unwrap();
        l.set_share_detail("mac:comparte", true).unwrap();
        let mut pon = |dev: &str, q: &str, cat: Category, v: Verdict, ts: i64, disfraz: &str| {
            let mut e = NewEvent::observed(ts, dev, "10.0.0.1", q, "A");
            e.category = cat;
            e.verdict = v;
            if q.starts_with('b') {
                e.signals = vec![Signal::Baliza { minutes: 5 }];
            }
            e.cname_target = (!disfraz.is_empty()).then(|| disfraz.to_owned());
            l.append(e).unwrap();
        };
        pon(
            "self",
            "a.t",
            Category::Rastreador,
            Verdict::Observado,
            t0,
            "",
        );
        pon(
            "self",
            "b.t",
            Category::Publicidad,
            Verdict::Cortado,
            t0 + 1,
            "",
        );
        pon(
            "self",
            "c.n",
            Category::Desconocido,
            Verdict::Observado,
            t0 + 2,
            "",
        );
        pon(
            "mac:comparte",
            "m.tienda.test",
            Category::Desconocido,
            Verdict::Observado,
            t0 + 3,
            "omtrdc.net",
        );
        pon(
            "mac:callado",
            "a.t",
            Category::Rastreador,
            Verdict::Observado,
            t0 + 4,
            "",
        );
        pon(
            "mac:callado",
            "m.tienda.test",
            Category::Desconocido,
            Verdict::Observado,
            t0 + 5,
            "omtrdc.net",
        );
        let t = l.tracker_rows(t0, t0 + DAY_MS).unwrap();
        // The two of this computer, and the shared phone's disguised name, which answered as a
        // tracker whatever its own category; nothing of the silent phone.
        assert_eq!(t.len(), 3, "{t:?}");
        assert!(t.iter().all(|r| r.device_id != "mac:callado"));
        assert_eq!(t[0].qname, "m.tienda.test");
        assert_eq!(t[2].beacon_minutes, Some(5));
        assert!(t[2].cut && !t[1].cut);
        let n = l.names_by_local_day(t0, t0 + DAY_MS, 0).unwrap();
        // The cut one never left; the silent phone is not here.
        let nombres: Vec<&str> = n.iter().map(|x| x.qname.as_str()).collect();
        assert_eq!(n.len(), 3, "{nombres:?}");
        assert!(!nombres.contains(&"b.t"));
        assert!(n.iter().all(|x| x.device_id != "mac:callado"));
        assert!(n
            .iter()
            .any(|x| x.qname == "m.tienda.test" && x.target == "omtrdc.net"));
        let c = l.cloaked_names(t0, t0 + DAY_MS).unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(
            (c[0].queries, c[0].devices),
            (1, 1),
            "the silent phone's row is not counted"
        );
        assert_eq!(c[0].target, "omtrdc.net");
    }
}
