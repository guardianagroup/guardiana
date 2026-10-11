//! CSV and JSON export of events (brief §3). Hand-written CSV: the format is
//! tiny and one fewer dependency to audit.

use std::io::Write;

use crate::error::Result;
use crate::model::Event;
use crate::time::rfc3339_utc;

/// Column order of the CSV export.
pub const CSV_HEADER: &str = "id,ts,ts_utc,device_id,client_ip,qname,qtype,category,list_source,\
                              signals,verdict,decided_by,rule_id,prev_hash,row_hash,\
                              programa,programa_ruta,programa_sha256,\
                              servidor_tipo,servidor_pais,servidor_asn,servidor_base,\
                              cname_destino";

/// Write events as a JSON array, one object per event, hashes as hex.
pub fn write_json<W: Write>(events: &[Event], mut w: W) -> Result<()> {
    serde_json::to_writer_pretty(&mut w, events)?;
    w.write_all(b"\n")?;
    Ok(())
}

/// Write events as CSV with a header row (RFC 4180: commas, no byte-order mark).
pub fn write_csv<W: Write>(events: &[Event], w: W) -> Result<()> {
    write_csv_with(events, ',', false, w)
}

/// CSV the way a spreadsheet opens it with a double click (the panel's «Export CSV»): UTF-8 with
/// a byte-order mark, so Excel does not garble accents, and the separator of the person's
/// spreadsheet. Spanish- and Portuguese-speaking countries write decimals with a comma, so their
/// Excel splits columns on `;` and showed a comma-separated file as one long column (found
/// exporting the published 1.0.0, 28 Sep 2026). The command line keeps plain RFC 4180.
pub fn write_csv_for_spreadsheet<W: Write>(events: &[Event], sep: char, w: W) -> Result<()> {
    write_csv_with(events, sep, true, w)
}

/// The spreadsheet CSV with more columns after the ledger's own: what the panel reads from the
/// stored fields with the data bundled in the program (the server's country and network, the
/// company's group; decision 203). They are not in the ledger and not in the hash, so they go
/// last, after every column needed to recompute it, with their header from `extra_header`.
pub fn write_csv_for_spreadsheet_with<W: Write>(
    events: &[Event],
    sep: char,
    extra_header: &[&str],
    extra: &dyn Fn(&Event) -> Vec<String>,
    w: W,
) -> Result<()> {
    write_csv_inner(events, sep, true, extra_header, extra, w)
}

fn write_csv_with<W: Write>(events: &[Event], sep: char, bom: bool, w: W) -> Result<()> {
    write_csv_inner(events, sep, bom, &[], &|_| Vec::new(), w)
}

fn write_csv_inner<W: Write>(
    events: &[Event],
    sep: char,
    bom: bool,
    extra_header: &[&str],
    extra: &dyn Fn(&Event) -> Vec<String>,
    mut w: W,
) -> Result<()> {
    if bom {
        w.write_all("\u{feff}".as_bytes())?;
    }
    let mut header = CSV_HEADER.replace(',', &sep.to_string());
    for h in extra_header {
        header.push(sep);
        header.push_str(&quote(h, sep, bom));
    }
    writeln!(w, "{header}")?;
    for e in events {
        let signals = e
            .signals
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let rule = e.rule_id.map(|r| r.to_string()).unwrap_or_default();
        let mut fields = vec![
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
            // El programa que lo pidió, cuando el sistema lo dijo (Windows, este equipo). Vacío en
            // todo lo demás, que es lo normal: un hueco, nunca una invención.
            e.process
                .as_ref()
                .map(|p| p.nombre.clone())
                .unwrap_or_default(),
            e.process
                .as_ref()
                .map(|p| p.ruta.clone())
                .unwrap_or_default(),
            e.process
                .as_ref()
                .map(|p| p.sha256.clone())
                .unwrap_or_default(),
            // El servidor que respondió según la base (decisión 203): tipo, país, red y qué base
            // lo dijo. Entra en el hash cuando está, así que va en el CSV para poder recalcularlo.
            // Nunca la dirección: no se guarda.
            e.server
                .as_ref()
                .map(|s| s.kind.as_str().to_owned())
                .unwrap_or_default(),
            e.server
                .as_ref()
                .map(|s| s.country.clone())
                .unwrap_or_default(),
            e.server
                .as_ref()
                .filter(|s| s.asn != 0)
                .map(|s| s.asn.to_string())
                .unwrap_or_default(),
            e.server
                .as_ref()
                .map(|s| s.base.clone())
                .unwrap_or_default(),
            // Un nombre disfrazado (decisión 205): el sitio del rastreador que respondía detrás
            // del alias. Entra en el hash cuando está; solo un nombre.
            e.cname_target.clone().unwrap_or_default(),
        ];
        fields.extend(extra(e));
        let line = fields
            .iter()
            .map(|f| quote(f, sep, bom))
            .collect::<Vec<_>>()
            .join(&sep.to_string());
        writeln!(w, "{line}")?;
    }
    Ok(())
}

/// RFC 4180 quoting: wrap in quotes when needed, double inner quotes.
fn quote(field: &str, sep: char, para_hoja: bool) -> String {
    // A name is typed by whoever asked it, on any device of the home, and a spreadsheet opened
    // with a double click evaluates a cell that starts with `=`, `+`, `-` or `@` as a formula,
    // quoted or not. A leading apostrophe makes it text again, and the name stays readable
    // (review of 8 Oct 2026). Only in the file made for a spreadsheet: the command line's
    // RFC 4180 CSV carries the value as written down, so the hashes can be recomputed from it
    // (review of 9 Oct 2026).
    let formula = para_hoja && field.starts_with(['=', '+', '-', '@', '\t', '\r']);
    if formula {
        format!("\"'{}\"", field.replace('"', "\"\""))
    } else if field.contains([sep, '"', '\n', '\r']) {
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

    #[test]
    fn a_name_that_looks_like_a_formula_is_quoted_as_text() {
        assert_eq!(quote("=HYPERLINK(1).x", ';', true), "\"'=HYPERLINK(1).x\"");
        assert_eq!(quote("-5.example", ';', true), "\"'-5.example\"");
        assert_eq!(quote("www.example", ';', true), "www.example");
        // The command line's CSV keeps the value as written down.
        assert_eq!(quote("-5.example", ',', false), "-5.example");
    }
    use crate::ledger::Ledger;
    use crate::model::{Event, NewEvent, Signal};

    #[test]
    fn csv_quotes_only_when_needed() {
        assert_eq!(quote("plain", ',', false), "plain");
        assert_eq!(quote("a,b", ',', false), "\"a,b\"");
        assert_eq!(quote("a,b", ';', false), "a,b");
        assert_eq!(quote("a;b", ';', false), "\"a;b\"");
        assert_eq!(quote("say \"hi\"", ',', false), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn spreadsheet_csv_has_a_bom_and_the_asked_separator() {
        let mut l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        l.append(NewEvent::observed(1, "self", "127.0.0.1", "a.example", "A"))
            .unwrap();
        let events = l.events(&Default::default()).unwrap();
        let mut csv = Vec::new();
        write_csv_for_spreadsheet(&events, ';', &mut csv).unwrap();
        assert!(csv.starts_with(&[0xEF, 0xBB, 0xBF]));
        let text = String::from_utf8(csv[3..].to_vec()).unwrap();
        let mut lines = text.lines();
        assert_eq!(lines.next().unwrap(), CSV_HEADER.replace(',', ";"));
        assert!(lines
            .next()
            .unwrap()
            .starts_with("1;1;1970-01-01T00:00:00.001Z;self;127.0.0.1;a.example;A;"));
        // The command-line CSV stays plain RFC 4180: no mark, commas.
        let mut plano = Vec::new();
        write_csv(&events, &mut plano).unwrap();
        assert!(plano.starts_with(b"id,ts,"));
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

    #[test]
    fn the_server_is_exported_and_extra_columns_go_last() {
        use crate::model::{ServerInfo, ServerKind};
        let mut l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        let mut e = NewEvent::observed(1, "self", "127.0.0.1", "a.example", "A");
        e.server = Some(ServerInfo {
            kind: ServerKind::Publico,
            country: "IE".to_owned(),
            asn: 32_934,
            base: "dbip-2026-10".to_owned(),
        });
        e.cname_target = Some("omtrdc.net".to_owned());
        l.append(e).unwrap();
        let events = l.events(&Default::default()).unwrap();
        let mut plano = Vec::new();
        write_csv(&events, &mut plano).unwrap();
        let text = String::from_utf8(plano).unwrap();
        assert!(
            text.lines()
                .nth(1)
                .unwrap()
                .ends_with(",publico,IE,32934,dbip-2026-10,omtrdc.net"),
            "{text}"
        );
        let mut hoja = Vec::new();
        write_csv_for_spreadsheet_with(
            &events,
            ';',
            &["red", "grupo"],
            &|ev| {
                vec![
                    format!("red {}", ev.server.as_ref().unwrap().asn),
                    "G".to_owned(),
                ]
            },
            &mut hoja,
        )
        .unwrap();
        let text = String::from_utf8(hoja[3..].to_vec()).unwrap();
        let mut lines = text.lines();
        assert!(lines.next().unwrap().ends_with(
            ";servidor_tipo;servidor_pais;servidor_asn;servidor_base;cname_destino;red;grupo"
        ));
        assert!(lines
            .next()
            .unwrap()
            .ends_with(";publico;IE;32934;dbip-2026-10;omtrdc.net;red 32934;G"));
        // JSON carries it too, and reads back.
        let mut json = Vec::new();
        write_json(&events, &mut json).unwrap();
        let back: Vec<Event> = serde_json::from_slice(&json).unwrap();
        assert_eq!(back[0].server, events[0].server);
    }
}
