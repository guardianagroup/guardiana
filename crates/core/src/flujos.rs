//! What a «Certificado de flujos» reads from the ledger (1.0.45): this computer's rows of a
//! period, one at a time and with only the fields a certificate may carry, and the bytes the
//! network channel measured, summed per destination name.
//!
//! The address the query came from is never read here (the owner's principle: never store or
//! hand out IPs), nor the program, nor the signals: a certificate says which companies this
//! computer asked for and what was cut, by company, country and category, and nothing finer.

use rusqlite::params;

use crate::error::Result;
use crate::hash::Hash;
use crate::ledger::{Ledger, KEY_HEAD};
use crate::model::{Category, Verdict};

/// One row of a period as a certificate reads it. No `client_ip`, no program, no signals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilaFlujo {
    /// Row id.
    pub id: i64,
    /// Unix ms of the query.
    pub ts: i64,
    /// Queried name.
    pub qname: String,
    /// Category from the lists, as stored.
    pub category: Category,
    /// What GUARDIANA did.
    pub verdict: Verdict,
    /// Two-letter country the bundled base gave for the server that answered; empty when there
    /// was none (cut, anycast, local, unknown).
    pub server_country: String,
    /// Kind of that server as stored: 0 none, 1 public, 2 anycast, 3 local (decision 203).
    pub server_kind: i64,
    /// Hash of the previous row.
    pub prev_hash: Hash,
    /// This row's hash.
    pub row_hash: Hash,
}

/// Bytes measured towards one destination name and country in a period. Not in the hash chain:
/// these are measurements, pruned with the detail (see `trafico.rs`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BytesDestino {
    /// The name the program had asked for that address, empty when it asked none GUARDIANA saw.
    pub destino: String,
    /// Two-letter country of the address, empty when the tables give none.
    pub pais: String,
    /// Bytes sent there.
    pub enviados: u64,
    /// Bytes received from there.
    pub recibidos: u64,
}

impl Ledger {
    /// Walk the rows of `device_id` with `since <= ts < until`, oldest first, handing each to
    /// `f`. Streams: a month of queries is never held in memory at once. Returns how many rows
    /// were walked.
    pub fn recorre_flujos(
        &self,
        device_id: &str,
        since: i64,
        until: i64,
        mut f: impl FnMut(FilaFlujo),
    ) -> Result<u64> {
        let mut st = self.conn.prepare(
            "SELECT id, ts, qname, category, verdict, server_country, server_kind, prev_hash, \
                    row_hash \
             FROM events WHERE device_id = ?1 AND ts >= ?2 AND ts < ?3 ORDER BY id ASC",
        )?;
        let mut rows = st.query(params![device_id, since, until])?;
        let mut n = 0u64;
        while let Some(r) = rows.next()? {
            let category: String = r.get(3)?;
            let verdict: String = r.get(4)?;
            let prev: Vec<u8> = r.get(7)?;
            let hash: Vec<u8> = r.get(8)?;
            f(FilaFlujo {
                id: r.get(0)?,
                ts: r.get(1)?,
                qname: r.get(2)?,
                category: category.parse()?,
                verdict: verdict.parse()?,
                server_country: r.get(5)?,
                server_kind: r.get(6)?,
                prev_hash: Hash::from_bytes(&prev)?,
                row_hash: Hash::from_bytes(&hash)?,
            });
            n += 1;
        }
        Ok(n)
    }

    /// The time of the oldest row still in the ledger (any device), `None` when it is empty.
    /// Rows before it were pruned by the retention: a certificate of an older period says so.
    pub fn primer_evento_ts(&self) -> Result<Option<i64>> {
        Ok(self
            .conn
            .query_row("SELECT MIN(ts) FROM events", [], |r| r.get(0))?)
    }

    /// The hash of the last row ever appended, as kept beside the rows (the chain head), when
    /// this file keeps one.
    pub fn cabeza_cadena(&self) -> Result<Option<String>> {
        self.setting(KEY_HEAD)
    }

    /// The bytes measured in `since <= minute < until`, summed per destination name and country.
    /// Never an address: the table does not hold one.
    pub fn bytes_por_destino(&self, since: i64, until: i64) -> Result<Vec<BytesDestino>> {
        let mut st = self.conn.prepare(
            "SELECT destino, pais, SUM(enviados), SUM(recibidos) FROM trafico \
             WHERE minuto >= ?1 AND minuto < ?2 GROUP BY destino, pais ORDER BY destino, pais",
        )?;
        let n = |v: i64| u64::try_from(v).unwrap_or(0);
        let filas = st.query_map(params![since, until], |r| {
            Ok(BytesDestino {
                destino: r.get(0)?,
                pais: r.get(1)?,
                enviados: n(r.get(2)?),
                recibidos: n(r.get(3)?),
            })
        })?;
        filas.map(|r| r.map_err(Into::into)).collect()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::model::NewEvent;
    use crate::trafico::Trafico;

    #[test]
    fn walks_only_the_device_and_the_window_and_never_the_address() {
        let mut l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        for (ts, dev, q) in [
            (1_000, "self", "a.example"),
            (2_000, "tel", "b.example"),
            (3_000, "self", "c.example"),
            (9_000, "self", "d.example"),
        ] {
            l.append(NewEvent::observed(ts, dev, "192.168.1.9", q, "A"))
                .unwrap();
        }
        let mut vistas = Vec::new();
        let n = l
            .recorre_flujos("self", 1_000, 9_000, |f| vistas.push(f))
            .unwrap();
        assert_eq!(n, 2);
        let nombres: Vec<&str> = vistas.iter().map(|f| f.qname.as_str()).collect();
        assert_eq!(nombres, ["a.example", "c.example"]);
        assert_eq!(l.primer_evento_ts().unwrap(), Some(1_000));
        assert!(l.cabeza_cadena().unwrap().is_some());
        // The struct has no field for the address; this only documents it.
        assert!(!format!("{vistas:?}").contains("192.168"));
    }

    #[test]
    fn sums_bytes_per_destination() {
        let mut l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        let fila = |minuto: i64, destino: &str, enviados: u64| Trafico {
            minuto,
            programa: "x.exe".into(),
            destino: destino.into(),
            pais: "US".into(),
            proto: "tcp".into(),
            puerto: 443,
            enviados,
            recibidos: 1,
            paquetes: 1,
            ..Trafico::default()
        };
        l.suma_trafico(&[
            fila(60_000, "a.example", 10),
            fila(120_000, "a.example", 5),
            fila(180_000, "", 7),
            fila(600_000, "a.example", 99),
        ])
        .unwrap();
        let b = l.bytes_por_destino(0, 300_000).unwrap();
        assert_eq!(b.len(), 2);
        let a = b.iter().find(|x| x.destino == "a.example").unwrap();
        assert_eq!((a.enviados, a.recibidos), (15, 2));
        let sin = b.iter().find(|x| x.destino.is_empty()).unwrap();
        assert_eq!(sin.enviados, 7);
    }
}
