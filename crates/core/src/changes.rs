//! Changes Guardiana makes to the machine on someone's order (Home Mode on or
//! off, the system DNS pointed here or restored), with who asked for them. The
//! panel promises "it is recorded in the ledger"; this is that record. They are
//! not query events, so they live in their own small table, like gaps.

use rusqlite::params;
use serde::Serialize;

use crate::error::Result;
use crate::ledger::Ledger;

/// What changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    /// Home Mode turned on (ports open on the LAN).
    HogarOn,
    /// Home Mode turned off.
    HogarOff,
    /// The system DNS now points at Guardiana.
    DnsOn,
    /// The system DNS restored as it was.
    DnsOff,
}

impl ChangeKind {
    /// Stored text.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HogarOn => "hogar_on",
            Self::HogarOff => "hogar_off",
            Self::DnsOn => "dns_on",
            Self::DnsOff => "dns_off",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "hogar_on" => Self::HogarOn,
            "hogar_off" => Self::HogarOff,
            "dns_on" => Self::DnsOn,
            "dns_off" => Self::DnsOff,
            _ => return None,
        })
    }
}

/// Who asked: `panel`, `terminal`, `desinstalador` (the uninstaller) or `servicio` (the watchdog).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Change {
    /// Row id.
    pub id: i64,
    /// Unix ms.
    pub ts: i64,
    /// What changed.
    pub kind: ChangeKind,
    /// Who asked for it.
    pub who: String,
    /// Free detail (an IP, the previous servers), may be empty.
    pub detail: String,
}

const CHANGES_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS changes (
    id     INTEGER PRIMARY KEY AUTOINCREMENT,
    ts     INTEGER NOT NULL,
    kind   TEXT NOT NULL,
    who    TEXT NOT NULL,
    detail TEXT NOT NULL DEFAULT ''
);
";

impl Ledger {
    pub(crate) fn ensure_changes_table(&self) -> Result<()> {
        self.conn.execute_batch(CHANGES_SCHEMA)?;
        Ok(())
    }

    /// Record a change with who asked for it.
    pub fn record_change(&self, ts: i64, kind: ChangeKind, who: &str, detail: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO changes (ts, kind, who, detail) VALUES (?1, ?2, ?3, ?4)",
            params![ts, kind.as_str(), who, detail],
        )?;
        Ok(())
    }

    /// The latest changes, newest first.
    pub fn changes(&self, limit: usize) -> Result<Vec<Change>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, ts, kind, who, detail FROM changes ORDER BY ts DESC, id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, ts, kind, who, detail) = row?;
            if let Some(kind) = ChangeKind::parse(&kind) {
                out.push(Change {
                    id,
                    ts,
                    kind,
                    who,
                    detail,
                });
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Hash;

    #[test]
    fn records_and_lists_newest_first() {
        let l = Ledger::open_in_memory(Hash::of(b"g")).unwrap_or_else(|_| unreachable!());
        l.record_change(10, ChangeKind::HogarOn, "panel", "192.168.1.39")
            .unwrap_or_else(|_| unreachable!());
        l.record_change(20, ChangeKind::HogarOff, "desinstalador", "")
            .unwrap_or_else(|_| unreachable!());
        let v = l.changes(10).unwrap_or_else(|_| unreachable!());
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].kind, ChangeKind::HogarOff);
        assert_eq!(v[0].who, "desinstalador");
        assert_eq!(v[1].detail, "192.168.1.39");
    }
}
