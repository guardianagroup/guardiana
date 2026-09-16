//! CSV and JSON export of events (brief §3). Hand-written CSV: the format is
//! tiny and one fewer dependency to audit.

use std::io::Write;

use crate::error::Result;
use crate::model::Event;
use crate::time::rfc3339_utc;

/// Column order of the CSV export.
pub const CSV_HEADER: &str = "id,ts,ts_utc,device_id,client_ip,qname,qtype,category,list_source,\
                              signals,verdict,decided_by,rule_id,prev_hash,row_hash";

/// Write events as a JSON array, one object per event, hashes as hex.
pub fn write_json<W: Write>(events: &[Event], mut w: W) -> Result<()> {
    serde_json::to_writer_pretty(&mut w, events)?;
    w.write_all(b"\n")?;
    Ok(())
}

/// Write events as CSV with a header row.
pub fn write_csv<W: Write>(events: &[Event], mut w: W) -> Result<()> {
    writeln!(w, "{CSV_HEADER}")?;
    for e in events {
        let signals = e
            .signals
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let rule = e.rule_id.map(|r| r.to_string()).unwrap_or_default();
        let fields = [
            e.id.to_string(),
            e.ts.to_string(),
            rfc3339_utc(e.ts),
            e.device_id.clone(),
            e.client_ip.clone(),
            e.qname.clone(),
            e.qtype.clone(),
            e.category.to_string(),
            e.list_source.clone(),
            signals,
            e.verdict.to_string(),
            e.decided_by.to_string(),
            rule,
            e.prev_hash.to_hex(),
            e.row_hash.to_hex(),
        ];
        let line = fields
            .iter()
            .map(|f| quote(f))
            .collect::<Vec<_>>()
            .join(",");
        writeln!(w, "{line}")?;
    }
    Ok(())
}

/// RFC 4180 quoting: wrap in quotes when needed, double inner quotes.
fn quote(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_owned()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::hash::Hash;
    use crate::ledger::Ledger;
    use crate::model::{Event, NewEvent, Signal};

    #[test]
    fn csv_quotes_only_when_needed() {
        assert_eq!(quote("plain"), "plain");
        assert_eq!(quote("a,b"), "\"a,b\"");
        assert_eq!(quote("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn exports_round_trip_through_json() {
        let mut l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        let mut e = NewEvent::observed(1, "self", "127.0.0.1", "a.example", "A");
        e.signals = vec![Signal::Volumen];
        l.append(e).unwrap();
        let events = l.events(&Default::default()).unwrap();

        let mut json = Vec::new();
        write_json(&events, &mut json).unwrap();
        let back: Vec<Event> = serde_json::from_slice(&json).unwrap();
        assert_eq!(back, events);

        let mut csv = Vec::new();
        write_csv(&events, &mut csv).unwrap();
        let text = String::from_utf8(csv).unwrap();
        let mut lines = text.lines();
        assert_eq!(lines.next().unwrap(), CSV_HEADER);
        let row = lines.next().unwrap();
        assert!(row.starts_with("1,1,1970-01-01T00:00:00.001Z,self,127.0.0.1,a.example,A,"));
        assert!(row.contains(",volumen,observado,nadie,,"));
        assert!(lines.next().is_none());
    }
}
