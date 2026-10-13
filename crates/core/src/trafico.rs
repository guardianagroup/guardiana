//! What each program sent and received, minute by minute (1.0.21, «vigila a tu antivirus»).
//!
//! The owner, 13 Oct 2026: «registrar todo lo que sale del pc… hacia qué empresa, por país».
//! On Windows the system's own network event channel says, for every packet a program sends
//! or receives, its size, the program and the address at the other end. GUARDIANA adds those
//! up per minute and keeps **one row per (minute, program, destination)**:
//!
//! - the destination is the **name** the program had asked for that address, when GUARDIANA saw
//!   the question (its own resolver answered it), or empty when the program connected without
//!   asking (a fixed address, or its own resolver);
//! - the **country and the network** of the address, from the tables that come inside the
//!   program (decision 203);
//! - the bytes each way, the packets and the port.
//!
//! **The address itself is never written** (decision 203 holds): it lives in memory only, on the
//! live screen. And what travelled inside the packets is never read: GUARDIANA counts, it does
//! not open.
//!
//! These rows are measurements, not events: they are not in the hash chain, and they follow the
//! detail's retention (they are pruned with it).

use rusqlite::params;
use serde::Serialize;

use crate::error::Result;
use crate::ledger::Ledger;

const TRAFICO_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS trafico (
    minuto     INTEGER NOT NULL,
    programa   TEXT    NOT NULL,
    firma      TEXT    NOT NULL DEFAULT '',
    destino    TEXT    NOT NULL DEFAULT '',
    pais       TEXT    NOT NULL DEFAULT '',
    red        TEXT    NOT NULL DEFAULT '',
    asn        INTEGER NOT NULL DEFAULT 0,
    proto      TEXT    NOT NULL,
    puerto     INTEGER NOT NULL,
    enviados   INTEGER NOT NULL DEFAULT 0,
    recibidos  INTEGER NOT NULL DEFAULT 0,
    paquetes   INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (minuto, programa, firma, destino, pais, red, asn, proto, puerto)
);
CREATE INDEX IF NOT EXISTS trafico_programa ON trafico (programa, minuto);
";

/// One minute of one program talking to one destination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct Trafico {
    /// Start of the minute, Unix ms.
    pub minuto: i64,
    /// The program's file name (`MsMpEng.exe`).
    pub programa: String,
    /// Who signs it, as the system reported; empty when unknown.
    pub firma: String,
    /// The name the program asked for this address, or empty when it asked none GUARDIANA saw.
    pub destino: String,
    /// Two-letter country of the address, empty when the tables give none (anycast, local).
    pub pais: String,
    /// The network that announces the address (`Amazon.com, Inc.`), empty when unknown.
    pub red: String,
    /// Its autonomous system number, 0 when unknown.
    pub asn: u32,
    /// `tcp` or `udp`.
    pub proto: String,
    /// The port at the other end.
    pub puerto: u16,
    /// Bytes this program sent there.
    pub enviados: u64,
    /// Bytes it received from there.
    pub recibidos: u64,
    /// Packets either way.
    pub paquetes: u64,
}

/// Minute of a Unix ms timestamp.
#[must_use]
pub const fn minuto_de(ts: i64) -> i64 {
    ts - ts.rem_euclid(60_000)
}

impl Ledger {
    pub(crate) fn ensure_trafico_table(&self) -> Result<()> {
        self.conn.execute_batch(TRAFICO_SCHEMA)?;
        Ok(())
    }

    /// Add these measurements: a row for the same minute, program and destination grows.
    pub fn suma_trafico(&mut self, filas: &[Trafico]) -> Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut st = tx.prepare_cached(
                "INSERT INTO trafico (minuto, programa, firma, destino, pais, red, asn, proto, \
                                      puerto, enviados, recibidos, paquetes) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12) \
                 ON CONFLICT (minuto, programa, firma, destino, pais, red, asn, proto, puerto) \
                 DO UPDATE SET enviados = enviados + excluded.enviados, \
                               recibidos = recibidos + excluded.recibidos, \
                               paquetes = paquetes + excluded.paquetes",
            )?;
            let n = |v: u64| i64::try_from(v).unwrap_or(i64::MAX);
            for f in filas {
                st.execute(params![
                    minuto_de(f.minuto),
                    f.programa,
                    f.firma,
                    f.destino,
                    f.pais,
                    f.red,
                    i64::from(f.asn),
                    f.proto,
                    i64::from(f.puerto),
                    n(f.enviados),
                    n(f.recibidos),
                    n(f.paquetes),
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// The measurements of a window, oldest first; `programas`, when not empty, keeps only
    /// those programs (by file name, any case).
    pub fn trafico(&self, since: i64, until: i64, programas: &[&str]) -> Result<Vec<Trafico>> {
        let mut st = self.conn.prepare(
            "SELECT minuto, programa, firma, destino, pais, red, asn, proto, puerto, enviados, \
                    recibidos, paquetes \
             FROM trafico WHERE minuto >= ?1 AND minuto < ?2 ORDER BY minuto ASC",
        )?;
        let quiere: Vec<String> = programas.iter().map(|p| p.to_ascii_lowercase()).collect();
        let n = |v: i64| u64::try_from(v).unwrap_or(0);
        let filas = st.query_map(params![minuto_de(since), until], |r| {
            Ok(Trafico {
                minuto: r.get(0)?,
                programa: r.get(1)?,
                firma: r.get(2)?,
                destino: r.get(3)?,
                pais: r.get(4)?,
                red: r.get(5)?,
                asn: u32::try_from(r.get::<_, i64>(6)?).unwrap_or(0),
                proto: r.get(7)?,
                puerto: u16::try_from(r.get::<_, i64>(8)?).unwrap_or(0),
                enviados: n(r.get(9)?),
                recibidos: n(r.get(10)?),
                paquetes: n(r.get(11)?),
            })
        })?;
        let mut out = Vec::new();
        for f in filas {
            let f = f?;
            if quiere.is_empty() || quiere.contains(&f.programa.to_ascii_lowercase()) {
                out.push(f);
            }
        }
        Ok(out)
    }
}

/// The setting that says whether GUARDIANA counts what the antivirus sends and receives (1.0.22).
/// Absent means yes: watching is the default; turning it off is the person's choice.
pub const AJUSTE_VIGILAR_ANTIVIRUS: &str = "antivirus_vigilar";

impl Ledger {
    /// Whether the antivirus traffic is being watched.
    pub fn vigilando_antivirus(&self) -> bool {
        !matches!(self.setting(AJUSTE_VIGILAR_ANTIVIRUS), Ok(Some(v)) if v == "0")
    }

    /// Turn the watching on or off.
    pub fn vigila_antivirus(&self, si: bool) -> Result<()> {
        self.set_setting(AJUSTE_VIGILAR_ANTIVIRUS, if si { "1" } else { "0" })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::hash::Hash;

    fn fila(minuto: i64, programa: &str, destino: &str, env: u64) -> Trafico {
        Trafico {
            minuto,
            programa: programa.into(),
            firma: "Av s.r.o.".into(),
            destino: destino.into(),
            pais: "CZ".into(),
            red: "Av Net".into(),
            asn: 1,
            proto: "tcp".into(),
            puerto: 443,
            enviados: env,
            recibidos: 10,
            paquetes: 2,
        }
    }

    #[test]
    fn a_minute_of_a_program_and_a_destination_adds_up_and_is_pruned_with_the_detail() {
        let mut l = Ledger::open_in_memory(Hash::of(b"t")).unwrap();
        let m = 1_000 * 60_000;
        l.suma_trafico(&[fila(m + 5, "AvSvc.exe", "t.av.example", 100)])
            .unwrap();
        l.suma_trafico(&[
            fila(m + 50_000, "AvSvc.exe", "t.av.example", 50),
            fila(m + 61_000, "AvSvc.exe", "t.av.example", 7),
            fila(m + 1, "chrome.exe", "x.example", 9),
        ])
        .unwrap();
        let av = l.trafico(m, m + 3 * 60_000, &["avsvc.exe"]).unwrap();
        assert_eq!(av.len(), 2);
        assert_eq!(av[0].minuto, m);
        assert_eq!(
            (av[0].enviados, av[0].recibidos, av[0].paquetes),
            (150, 20, 4)
        );
        assert_eq!(av[1].enviados, 7);
        assert_eq!(l.trafico(m, m + 3 * 60_000, &[]).unwrap().len(), 3);
        // The free plan keeps 24 h of detail: a day and a minute later, only the last minute is left.
        let r = l
            .prune(crate::Retention::FREE, m + 60_000 + 24 * 3_600_000)
            .unwrap();
        assert_eq!(r.trafico_pruned, 2);
        assert_eq!(l.trafico(0, i64::MAX, &[]).unwrap().len(), 1);
    }

    #[test]
    fn watching_the_antivirus_is_on_until_the_person_turns_it_off() {
        let l = Ledger::open_in_memory(Hash::of(b"t")).unwrap();
        assert!(l.vigilando_antivirus());
        l.vigila_antivirus(false).unwrap();
        assert!(!l.vigilando_antivirus());
        l.vigila_antivirus(true).unwrap();
        assert!(l.vigilando_antivirus());
    }
}
