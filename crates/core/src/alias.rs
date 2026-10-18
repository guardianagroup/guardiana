//! The aliases a name answered through, kept by name (1.0.45).
//!
//! GUARDIANA is the resolver, so it sees the CNAME records of every answer it forwards:
//! `www.tienda.com` → `www.tienda.com.edgekey.net` → `e12.a.akamaiedge.net`. Until 1.0.44 only a
//! disguise was kept (decision 205: the alias of a tracker of another site). The owner, 18 Oct
//! 2026: «precisión máxima de todos los datos hacia dónde van». The chain is the most direct
//! evidence of whose service answers under a name, so its **sites** are kept here, one row per
//! name, and the panel attributes from them.
//!
//! What is kept: the asked name, the registrable sites of its aliases (never the aliases' own
//! hostnames under a shared service, never an address), and the day they were last seen. Like
//! `trafico`, these rows are measurements, not events: they are not in the hash chain, they follow
//! the detail's retention and «Borrar todo» removes them.

use std::collections::HashMap;

use rusqlite::params;

use crate::error::Result;
use crate::ledger::Ledger;

const ALIAS_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS alias_sites (
    qname     TEXT    PRIMARY KEY,
    sites     TEXT    NOT NULL,
    last_seen INTEGER NOT NULL
);
";

/// The most sites kept for one name.
pub const MAX_ALIAS_SITES: usize = 6;

/// A name is written again at most this often when its sites have not changed: the resolver
/// answers the same name many times a minute.
const REESCRIBIR_CADA_MS: i64 = 6 * 60 * 60 * 1_000;

/// The sites one name answered through, and when.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AliasSites {
    /// The registrable sites of the aliases, in the order of the answer.
    pub sites: Vec<String>,
    /// Last time they were seen, Unix ms.
    pub last_seen: i64,
}

/// Queries for one name answered from one network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameNetwork {
    /// The name.
    pub qname: String,
    /// The answering server's autonomous system, 0 when none was kept.
    pub asn: u32,
    /// How many queries.
    pub queries: i64,
}

fn limpio(s: &str) -> Option<String> {
    let s = s.trim().trim_end_matches('.').to_ascii_lowercase();
    let ok = !s.is_empty()
        && s.len() <= 253
        && s.contains('.')
        && s.parse::<std::net::IpAddr>().is_err()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_');
    ok.then_some(s)
}

impl Ledger {
    pub(crate) fn ensure_alias_table(&self) -> Result<()> {
        self.conn.execute_batch(ALIAS_SCHEMA)?;
        Ok(())
    }

    /// Note the alias sites `qname` answered through at `ts`. Sites that read as an address or
    /// not as a name are dropped; nothing is written for an empty list. A name already written
    /// with the same sites is rewritten only every few hours.
    pub fn note_alias_sites(&mut self, qname: &str, sites: &[String], ts: i64) -> Result<()> {
        let Some(qname) = limpio(qname) else {
            return Ok(());
        };
        let mut v: Vec<String> = Vec::new();
        for s in sites.iter().filter_map(|s| limpio(s)) {
            if !v.contains(&s) && v.len() < MAX_ALIAS_SITES {
                v.push(s);
            }
        }
        if v.is_empty() {
            return Ok(());
        }
        let texto = v.join(" ");
        self.conn.execute(
            "INSERT INTO alias_sites (qname, sites, last_seen) VALUES (?1, ?2, ?3) \
             ON CONFLICT (qname) DO UPDATE SET sites = excluded.sites, \
                                               last_seen = excluded.last_seen \
             WHERE alias_sites.sites <> excluded.sites \
                OR excluded.last_seen - alias_sites.last_seen >= ?4",
            params![qname, texto, ts, REESCRIBIR_CADA_MS],
        )?;
        Ok(())
    }

    /// The alias sites of each of `qnames` that has any.
    pub fn alias_sites_for(&self, qnames: &[&str]) -> Result<HashMap<String, AliasSites>> {
        let mut out = HashMap::new();
        let mut st = self
            .conn
            .prepare_cached("SELECT sites, last_seen FROM alias_sites WHERE qname = ?1")?;
        for q in qnames {
            let Some(q) = limpio(q) else { continue };
            if out.contains_key(&q) {
                continue;
            }
            let fila: Option<(String, i64)> = st
                .query_row([&q], |r| Ok((r.get(0)?, r.get(1)?)))
                .map(Some)
                .or_else(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => Ok(None),
                    e => Err(e),
                })?;
            if let Some((sites, last_seen)) = fila {
                out.insert(
                    q,
                    AliasSites {
                        sites: sites.split(' ').map(str::to_owned).collect(),
                        last_seen,
                    },
                );
            }
        }
        Ok(out)
    }

    /// The forwarded queries of `[since, until)` by name and by the network of the server that
    /// answered (0 when none was kept), for the rows the household may read: what the «%
    /// identificado» is counted over. Most queried first.
    pub fn destinations_by_network(&self, since: i64, until: i64) -> Result<Vec<NameNetwork>> {
        let mut st = self.conn.prepare(
            "SELECT qname, server_asn, COUNT(*) AS n FROM events \
             WHERE ts >= ?1 AND ts < ?2 AND verdict = 'observado' \
               AND (device_id = ?3 OR device_id IN \
                    (SELECT id FROM devices WHERE share_detail_with_home != 0)) \
             GROUP BY qname, server_asn \
             ORDER BY n DESC, qname ASC, server_asn ASC",
        )?;
        let rows = st.query_map(params![since, until, crate::SELF_DEVICE_ID], |r| {
            Ok(NameNetwork {
                qname: r.get(0)?,
                asn: u32::try_from(r.get::<_, i64>(1)?).unwrap_or(0),
                queries: r.get(2)?,
            })
        })?;
        rows.map(|r| r.map_err(crate::Error::from)).collect()
    }

    /// How many names have alias sites kept.
    pub fn alias_sites_count(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM alias_sites", [], |r| r.get(0))?)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::{Hash, Ledger, Retention};

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| (*x).to_owned()).collect()
    }

    #[test]
    fn sites_are_kept_by_name_without_addresses() {
        let mut l = Ledger::open_in_memory(Hash::of(b"alias")).unwrap();
        l.note_alias_sites(
            "Metricas.Tienda.com.",
            &s(&[
                "omtrdc.net",
                "93.184.216.34",
                "akamaiedge.net",
                "omtrdc.net",
                "x y",
            ]),
            1_000,
        )
        .unwrap();
        let m = l
            .alias_sites_for(&["metricas.tienda.com", "otra.com"])
            .unwrap();
        let a = &m["metricas.tienda.com"];
        assert_eq!(a.sites, vec!["omtrdc.net", "akamaiedge.net"]);
        assert_eq!(a.last_seen, 1_000);
        assert!(!m.contains_key("otra.com"));
        // Nothing to keep: nothing written.
        l.note_alias_sites("vacio.com", &s(&["1.2.3.4"]), 1_000)
            .unwrap();
        assert_eq!(l.alias_sites_count().unwrap(), 1);
    }

    #[test]
    fn the_same_sites_are_not_rewritten_every_answer_but_a_change_is() {
        let mut l = Ledger::open_in_memory(Hash::of(b"alias2")).unwrap();
        l.note_alias_sites("a.com", &s(&["edgekey.net"]), 1_000)
            .unwrap();
        l.note_alias_sites("a.com", &s(&["edgekey.net"]), 2_000)
            .unwrap();
        assert_eq!(
            l.alias_sites_for(&["a.com"]).unwrap()["a.com"].last_seen,
            1_000
        );
        l.note_alias_sites("a.com", &s(&["cloudfront.net"]), 3_000)
            .unwrap();
        let a = &l.alias_sites_for(&["a.com"]).unwrap()["a.com"];
        assert_eq!(a.sites, vec!["cloudfront.net"]);
        assert_eq!(a.last_seen, 3_000);
        let tarde = 3_000 + 7 * 3_600_000;
        l.note_alias_sites("a.com", &s(&["cloudfront.net"]), tarde)
            .unwrap();
        assert_eq!(
            l.alias_sites_for(&["a.com"]).unwrap()["a.com"].last_seen,
            tarde
        );
    }

    #[test]
    fn the_week_counts_by_name_and_network() {
        use crate::{NewEvent, ServerInfo, ServerKind, Verdict, SELF_DEVICE_ID};
        let mut l = Ledger::open_in_memory(Hash::of(b"alias4")).unwrap();
        let mut pon = |ts: i64, q: &str, asn: u32, v: Verdict| {
            let mut e = NewEvent::observed(ts, SELF_DEVICE_ID, "127.0.0.1", q, "A");
            e.server = (asn != 0).then(|| ServerInfo {
                kind: ServerKind::Publico,
                country: "US".into(),
                asn,
                base: "dbip-2026-10".into(),
            });
            e.verdict = v;
            l.append(e).unwrap();
        };
        pon(10, "a.com", 32934, Verdict::Observado);
        pon(11, "a.com", 32934, Verdict::Observado);
        pon(12, "a.com", 16509, Verdict::Observado);
        pon(13, "b.com", 0, Verdict::Observado);
        pon(14, "c.com", 0, Verdict::Cortado);
        pon(99, "d.com", 0, Verdict::Observado);
        let v = l.destinations_by_network(0, 50).unwrap();
        let t: Vec<(&str, u32, i64)> = v
            .iter()
            .map(|r| (r.qname.as_str(), r.asn, r.queries))
            .collect();
        assert_eq!(
            t,
            vec![("a.com", 32934, 2), ("a.com", 16509, 1), ("b.com", 0, 1)]
        );
    }

    #[test]
    fn wipe_and_retention_remove_them() {
        let mut l = Ledger::open_in_memory(Hash::of(b"alias3")).unwrap();
        let dia = 24 * 3_600_000;
        l.note_alias_sites("viejo.com", &s(&["edgekey.net"]), dia)
            .unwrap();
        l.note_alias_sites("nuevo.com", &s(&["edgekey.net"]), 100 * dia)
            .unwrap();
        l.prune(Retention::FREE, 100 * dia).unwrap();
        let m = l.alias_sites_for(&["viejo.com", "nuevo.com"]).unwrap();
        assert!(!m.contains_key("viejo.com"));
        assert!(m.contains_key("nuevo.com"));
        l.suma_trafico(&[crate::Trafico {
            minuto: 100 * dia,
            programa: "x.exe".into(),
            proto: "tcp".into(),
            puerto: 443,
            enviados: 1,
            ..crate::Trafico::default()
        }])
        .unwrap();
        l.wipe().unwrap();
        assert_eq!(l.alias_sites_count().unwrap(), 0);
        // «Borrar todo» takes what each program sent with it.
        let n: i64 = l
            .connection()
            .query_row("SELECT COUNT(*) FROM trafico", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }
}
