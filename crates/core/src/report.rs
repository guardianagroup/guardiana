//! The weekly household report (brief §8, HOGAR.md §4): totals per device
//! over the last seven days, combining the detail still kept in `events`
//! with the daily totals that retention already rolled up. The two never
//! overlap: pruning moves rows from one to the other.

use std::collections::BTreeMap;

use rusqlite::params;
use serde::Serialize;

use crate::error::Result;
use crate::ledger::Ledger;
use crate::model::ServerKind;
use crate::time::{day_utc, DAY_MS};
use crate::SELF_DEVICE_ID;

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

    /// Every name asked for in the seven days ending at `until`, most asked first, with how many
    /// times, over the rows the household may read (this computer and the devices that share
    /// their detail, as in [`Ledger::events`]). Names rather than rows: a week of a busy house is
    /// tens of thousands of rows and a few thousand names, and what is asked of this is who owns
    /// the names (the registered data brokers of the weekly report, decision 194).
    pub fn week_names(&self, until: i64) -> Result<Vec<NameCount>> {
        let since = until - 7 * DAY_MS;
        let mut stmt = self.conn.prepare(
            "SELECT qname, COUNT(*) AS n FROM events \
             WHERE ts >= ?1 AND ts < ?2 \
               AND (device_id = ?3 OR device_id IN \
                    (SELECT id FROM devices WHERE share_detail_with_home != 0)) \
             GROUP BY qname ORDER BY n DESC, qname ASC",
        )?;
        let rows = stmt.query_map(params![since, until, SELF_DEVICE_ID], |r| {
            Ok(NameCount {
                qname: r.get(0)?,
                queries: r.get(1)?,
            })
        })?;
        rows.map(|r| r.map_err(crate::Error::from)).collect()
    }

    /// Every (name, device) the household may read in everything the extract still keeps in
    /// detail, with how many times it was asked for, how many of those were cut, and the first
    /// and last time (decision 204, «Haz que te borren»). The page that helps a person ask a data
    /// broker to delete them shows which of the broker's names the house asked for, when, and from
    /// which device. A device whose owner did not share its detail is left out entirely, as in
    /// [`Ledger::events`]: that a phone asked for a people-search site is exactly the kind of
    /// detail its owner kept to themselves.
    pub fn name_device_spans(&self) -> Result<Vec<NameSpan>> {
        let mut stmt = self.conn.prepare(
            "SELECT qname, device_id, COUNT(*), \
                    SUM(CASE WHEN verdict = 'cortado' THEN 1 ELSE 0 END), MIN(ts), MAX(ts) \
             FROM events \
             WHERE device_id = ?1 OR device_id IN \
                   (SELECT id FROM devices WHERE share_detail_with_home != 0) \
             GROUP BY qname, device_id",
        )?;
        let rows = stmt.query_map(params![SELF_DEVICE_ID], |r| {
            Ok(NameSpan {
                qname: r.get(0)?,
                device_id: r.get(1)?,
                queries: r.get(2)?,
                cut: r.get(3)?,
                first_ts: r.get(4)?,
                last_ts: r.get(5)?,
            })
        })?;
        rows.map(|r| r.map_err(crate::Error::from)).collect()
    }

    /// The week's forwarded queries the household may read, by (name, kind and country of the
    /// server that answered), most asked first (decision 203). Cut queries are not here: they
    /// never left the house. Rows without a server (written before decision 203, or an answer
    /// with no address) are still counted, so the panel can say out of how many.
    pub fn week_destinations(&self, until: i64) -> Result<Vec<DestinationCount>> {
        let since = until - 7 * DAY_MS;
        let mut stmt = self.conn.prepare(
            "SELECT qname, server_kind, server_country, COUNT(*) AS n FROM events \
             WHERE ts >= ?1 AND ts < ?2 AND verdict = 'observado' \
               AND (device_id = ?3 OR device_id IN \
                    (SELECT id FROM devices WHERE share_detail_with_home != 0)) \
             GROUP BY qname, server_kind, server_country \
             ORDER BY n DESC, qname ASC, server_kind ASC, server_country ASC",
        )?;
        let rows = stmt.query_map(params![since, until, SELF_DEVICE_ID], |r| {
            Ok(DestinationCount {
                qname: r.get(0)?,
                kind: kind_of(r.get(1)?),
                country: r.get(2)?,
                queries: r.get(3)?,
            })
        })?;
        rows.map(|r| r.map_err(crate::Error::from)).collect()
    }

    /// Every distinct (kind, country) of server in the rows the household may read, for the
    /// extract's filter by server country.
    pub fn server_buckets(&self) -> Result<Vec<(ServerKind, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT server_kind, server_country FROM events WHERE server_kind != 0 \
               AND (device_id = ?1 OR device_id IN \
                    (SELECT id FROM devices WHERE share_detail_with_home != 0))",
        )?;
        let rows = stmt.query_map(params![SELF_DEVICE_ID], |r| {
            Ok((kind_of(r.get(0)?), r.get::<_, String>(1)?))
        })?;
        let mut out = Vec::new();
        for r in rows {
            if let (Some(k), c) = r? {
                out.push((k, c));
            }
        }
        Ok(out)
    }

    /// Every (device, name, program) asked for between `since` and `until`, with how many times
    /// and when last, over every device (the AI page reads it per device and shows each device
    /// its own; the household rule of [`Ledger::events`] is applied by the caller, which only
    /// lists devices the panel may name). Grouped in SQL so a week of a busy house is a few
    /// thousand rows and not a capped sample: the AI page said "last 7 days" while reading the
    /// 20 000 newest rows per device, which on a normal PC was half a week (review of 8 Oct 2026).
    pub fn names_grouped(&self, since: i64, until: i64) -> Result<Vec<NameGroup>> {
        let mut stmt = self.conn.prepare(
            // `process_json` is '' for rows without a program (and in databases older than the
            // column): json_extract on '' is an error, not NULL.
            "SELECT device_id, qname, \
                    CASE WHEN process_json = '' THEN NULL \
                         ELSE json_extract(process_json, '$.nombre') END AS programa, \
                    COUNT(*), MAX(ts) \
             FROM events WHERE ts >= ?1 AND ts < ?2 \
             GROUP BY device_id, qname, programa",
        )?;
        let rows = stmt.query_map(params![since, until], |r| {
            Ok(NameGroup {
                device_id: r.get(0)?,
                qname: r.get(1)?,
                process: r.get::<_, Option<String>>(2)?.filter(|p| !p.is_empty()),
                queries: u64::try_from(r.get::<_, i64>(3)?).unwrap_or(0),
                last_ts: r.get(4)?,
            })
        })?;
        rows.map(|r| r.map_err(crate::Error::from)).collect()
    }

    /// What each program asked for in a window, per (device, name, program, signer): how many
    /// times, how many of them between midnight and six in the person's own day (`offset_min`
    /// minutes east of UTC), how many were cut, the last time and the name's category.
    ///
    /// Only rows where the system said which program asked. Made for «GUARDIANA vigila a tu
    /// antivirus» (13 Oct 2026), where the signer matters: a process called like an antivirus
    /// and signed by somebody else is not that antivirus.
    pub fn program_names(
        &self,
        since: i64,
        until: i64,
        offset_min: i64,
    ) -> Result<Vec<ProgramName>> {
        let mut stmt = self.conn.prepare(
            "SELECT device_id, qname, json_extract(process_json, '$.nombre') AS programa, \
                    COALESCE(json_extract(process_json, '$.firmado_por'), '') AS firma, \
                    COUNT(*), MAX(ts), \
                    SUM(CASE WHEN ((ts + ?3) % 86400000) < 21600000 THEN 1 ELSE 0 END), \
                    SUM(CASE WHEN verdict = 'cortado' THEN 1 ELSE 0 END), \
                    MAX(category) \
             FROM events WHERE ts >= ?1 AND ts < ?2 AND process_json != '' \
             GROUP BY device_id, qname, programa, firma",
        )?;
        let n = |v: i64| u64::try_from(v).unwrap_or(0);
        let rows = stmt.query_map(params![since, until, offset_min * 60_000], |r| {
            Ok(ProgramName {
                device_id: r.get(0)?,
                qname: r.get(1)?,
                program: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                signer: r.get(3)?,
                queries: n(r.get(4)?),
                last_ts: r.get(5)?,
                night: n(r.get(6)?),
                cut: n(r.get(7)?),
                // A name keeps its category; when a list update changed it inside the window,
                // the highest of the two by name, which puts telemetry and trackers first.
                category: r
                    .get::<_, String>(8)?
                    .parse()
                    .unwrap_or(crate::Category::Desconocido),
            })
        })?;
        rows.map(|r| r.map_err(crate::Error::from))
            .filter(|r| !matches!(r, Ok(p) if p.program.is_empty()))
            .collect()
    }

    /// Destinations the house had never asked for before the seven days ending at `until`.
    ///
    /// `seen_domains` remembers when each (device, name) pair first appeared and survives
    /// pruning; the count, the device and the category come from `events`. With the free plan's
    /// 24 hours of detail there is nothing left to count after a day, which is why the panel
    /// only offers this with Plus: it is not a feature held back, it is a question that cannot
    /// be answered without the memory Plus keeps.
    ///
    /// Each row names a destination and the device that asked for it, so it is detail, not a
    /// total: a device whose owner did not share its detail with the home panel is left out, as
    /// in [`Ledger::events`] (review of 1 Oct 2026, entry 1). Its week still appears, as
    /// figures, in [`Ledger::week_summary`].
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
               AND (s.device_id = ?4 OR s.device_id IN \
                    (SELECT id FROM devices WHERE share_detail_with_home != 0)) \
             GROUP BY s.qname, s.device_id \
             ORDER BY n DESC, s.first_seen DESC \
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![since, until, limit as i64, SELF_DEVICE_ID], |r| {
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

/// A (device, name, program) group of a window: how many times and when last.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct NameGroup {
    /// Which device asked.
    pub device_id: String,
    /// The name asked for.
    pub qname: String,
    /// The program that asked, when the system said so; `None` for rows without one.
    pub process: Option<String>,
    /// Times it was asked for inside the window.
    pub queries: u64,
    /// The last time, Unix ms.
    pub last_ts: i64,
}

/// What one program asked for in a window: see [`Ledger::program_names`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProgramName {
    /// Which device asked.
    pub device_id: String,
    /// The name asked for.
    pub qname: String,
    /// The program's file name, as the system gave it.
    pub program: String,
    /// Who signs the program, as the system reported it; empty when unsigned or not checked.
    pub signer: String,
    /// Times it was asked for inside the window.
    pub queries: u64,
    /// Of those, between midnight and six in the person's day.
    pub night: u64,
    /// Of those, cut.
    pub cut: u64,
    /// The last time, Unix ms.
    pub last_ts: i64,
    /// The name's category.
    pub category: crate::Category,
}

/// A (name, device) of the whole extract: how many times, how many cut, first and last time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct NameSpan {
    /// The name asked for.
    pub qname: String,
    /// Which device asked.
    pub device_id: String,
    /// Times it was asked for.
    pub queries: i64,
    /// Of those, the ones GUARDIANA cut (they never left the house).
    pub cut: i64,
    /// The first time, Unix ms.
    pub first_ts: i64,
    /// The last time, Unix ms.
    pub last_ts: i64,
}

/// A (name, server kind and country) of the week and how many forwarded queries got it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct DestinationCount {
    /// Queried name.
    pub qname: String,
    /// What kind of server answered; `None` when the row has no server.
    pub kind: Option<ServerKind>,
    /// Its country, empty when the base gave none.
    pub country: String,
    /// Forwarded queries for that name that got such a server.
    pub queries: i64,
}

/// A stored server kind code as the kind; 0 (or anything unknown) is no server.
fn kind_of(code: i64) -> Option<ServerKind> {
    match code {
        1 => Some(ServerKind::Publico),
        2 => Some(ServerKind::Anycast),
        3 => Some(ServerKind::Local),
        _ => None,
    }
}

/// A name and how many times it was asked for in a window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct NameCount {
    /// The name asked for.
    pub qname: String,
    /// Times it was asked for inside the window.
    pub queries: i64,
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
    fn what_each_program_asked_with_its_signer_its_nights_and_its_cuts() {
        let mut l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        let dia = 20 * DAY_MS;
        let prog = |nombre: &str, firma: &str| crate::model::Proceso {
            pid: 1,
            nombre: nombre.into(),
            ruta: String::new(),
            sha256: String::new(),
            firmado_por: firma.into(),
        };
        // Two in the person's night (UTC-5: 07:00 UTC is 02:00 there), one cut; one in the day.
        for (h, cortar) in [(7, false), (8, true), (15, false)] {
            let mut e =
                NewEvent::observed(dia + h * HOUR_MS, "self", "127.0.0.1", "t.av.example", "A");
            e.category = Category::Telemetria;
            e.process = Some(prog("AvSvc.exe", "Av s.r.o."));
            if cortar {
                e.verdict = Verdict::Cortado;
            }
            l.append(e).unwrap();
        }
        // The same name from a process with the same file name and another signer: its own row.
        let mut otro =
            NewEvent::observed(dia + 15 * HOUR_MS, "self", "127.0.0.1", "t.av.example", "A");
        otro.process = Some(prog("AvSvc.exe", ""));
        l.append(otro).unwrap();
        // No program: left out.
        l.append(NewEvent::observed(
            dia + 16 * HOUR_MS,
            "self",
            "127.0.0.1",
            "x.example",
            "A",
        ))
        .unwrap();
        let mut r = l.program_names(dia, dia + DAY_MS, -300).unwrap();
        r.sort_by(|a, b| b.signer.cmp(&a.signer));
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].signer, "Av s.r.o.");
        assert_eq!((r[0].queries, r[0].night, r[0].cut), (3, 2, 1));
        assert_eq!(r[0].category, Category::Telemetria);
        assert_eq!(r[0].last_ts, dia + 15 * HOUR_MS);
        assert_eq!(r[1].signer, "");
        assert_eq!(r[1].queries, 1);
    }

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

    /// The week's destinations: forwarded only, the household's rows only, by (name, server).
    #[test]
    fn week_destinations_count_what_left_and_only_what_the_house_may_read() {
        use crate::model::ServerInfo;
        let mut l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        let now = 1_700_000_000_000i64;
        l.upsert_device("mac:callado", None, Some("10.0.0.9"), now - DAY_MS)
            .unwrap();
        let sv = |pais: &str| {
            (!pais.is_empty()).then(|| ServerInfo {
                kind: ServerKind::Publico,
                country: pais.to_owned(),
                asn: 64_500,
                base: "dbip-2026-10".to_owned(),
            })
        };
        let mut add = |device: &str, name: &str, pais: &str, verdict: Verdict, ts: i64| {
            let mut e = NewEvent::observed(ts, device, "127.0.0.1", name, "A");
            e.server = sv(pais);
            e.verdict = verdict;
            l.append(e).unwrap();
        };
        add("self", "a.example", "IE", Verdict::Observado, now - HOUR_MS);
        add(
            "self",
            "a.example",
            "IE",
            Verdict::Observado,
            now - 2 * HOUR_MS,
        );
        add(
            "self",
            "a.example",
            "US",
            Verdict::Observado,
            now - 3 * HOUR_MS,
        );
        add("self", "b.example", "", Verdict::Observado, now - HOUR_MS);
        add("self", "t.example", "", Verdict::Cortado, now - HOUR_MS);
        add(
            "mac:callado",
            "c.example",
            "FR",
            Verdict::Observado,
            now - HOUR_MS,
        );
        add(
            "self",
            "viejo.example",
            "DE",
            Verdict::Observado,
            now - 8 * DAY_MS,
        );
        let d = l.week_destinations(now).unwrap();
        let tuplas: Vec<(&str, Option<ServerKind>, &str, i64)> = d
            .iter()
            .map(|x| (x.qname.as_str(), x.kind, x.country.as_str(), x.queries))
            .collect();
        let p = Some(ServerKind::Publico);
        assert_eq!(
            tuplas,
            vec![
                ("a.example", p, "IE", 2),
                ("a.example", p, "US", 1),
                ("b.example", None, "", 1),
            ]
        );
        let mut cubos = l.server_buckets().unwrap();
        cubos.sort_by(|a, b| a.1.cmp(&b.1));
        assert_eq!(
            cubos,
            vec![
                (ServerKind::Publico, "DE".to_owned()),
                (ServerKind::Publico, "IE".to_owned()),
                (ServerKind::Publico, "US".to_owned()),
            ]
        );
    }

    /// Decision 204: per (name, device), counts, cuts and first/last time over the whole extract;
    /// a device that shares is in, one that does not is out entirely.
    #[test]
    fn name_device_spans_count_everything_kept_and_leave_silent_devices_out() {
        let mut l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        let t0 = 1_700_000_000_000i64;
        l.upsert_device("mac:callado", None, Some("10.0.0.9"), t0)
            .unwrap();
        l.upsert_device("mac:comparte", None, Some("10.0.0.8"), t0)
            .unwrap();
        l.set_share_detail("mac:comparte", true).unwrap();
        let mut add = |device: &str, name: &str, verdict: Verdict, ts: i64| {
            let mut e = NewEvent::observed(ts, device, "127.0.0.1", name, "A");
            e.verdict = verdict;
            l.append(e).unwrap();
        };
        add("self", "a.spokeo.com", Verdict::Observado, t0 + 30 * DAY_MS);
        add("self", "a.spokeo.com", Verdict::Cortado, t0 + DAY_MS);
        add("self", "a.spokeo.com", Verdict::Observado, t0 + 2 * DAY_MS);
        add("mac:comparte", "a.spokeo.com", Verdict::Observado, t0);
        add(
            "mac:callado",
            "a.spokeo.com",
            Verdict::Observado,
            t0 - DAY_MS,
        );
        add("mac:callado", "secreto.example", Verdict::Observado, t0);
        let mut s = l.name_device_spans().unwrap();
        s.sort_by(|a, b| a.device_id.cmp(&b.device_id));
        assert_eq!(s.len(), 2, "{s:?}");
        assert!(s.iter().all(|x| x.device_id != "mac:callado"));
        assert_eq!(
            (s[0].device_id.as_str(), s[0].queries, s[0].cut),
            ("mac:comparte", 1, 0)
        );
        assert_eq!(
            (s[1].device_id.as_str(), s[1].queries, s[1].cut),
            ("self", 3, 1)
        );
        assert_eq!(
            (s[1].first_ts, s[1].last_ts),
            (t0 + DAY_MS, t0 + 30 * DAY_MS)
        );
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
