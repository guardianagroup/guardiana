//! One-line rendering of events for the terminal: name → category → device.

use guardiana_core::i18n::Texts;
use guardiana_core::time::rfc3339_utc;
use guardiana_core::{Event, SELF_DEVICE_ID};

/// `HH:MM:SS` in UTC.
pub fn clock(ts: i64) -> String {
    rfc3339_utc(ts).get(11..19).unwrap_or("").to_owned()
}

/// Label for a device: "este computador", its name, or its id.
pub fn device_label<'a>(t: &'a Texts, id: &'a str, name: Option<&'a str>) -> &'a str {
    if id == SELF_DEVICE_ID {
        t.cli("dispositivo.self")
    } else {
        name.unwrap_or(id)
    }
}

/// Render one event as a single line.
pub fn line(t: &Texts, e: &Event, device_name: Option<&str>) -> String {
    let mut s = format!(
        "{}  {:<40}  →  {:<11}  →  {}  ·  {}",
        clock(e.ts),
        e.qname,
        t.category(e.category),
        device_label(t, &e.device_id, device_name),
        t.verdict(e.verdict),
    );
    for sig in &e.signals {
        s.push_str("\n           · ");
        s.push_str(&t.signal(sig));
    }
    s
}
