//! Minimal UTC time helpers. Timestamps are Unix milliseconds (`i64`).
//! No calendar crate: the two conversions needed fit in a few lines and
//! are tested against known dates.

use std::time::{SystemTime, UNIX_EPOCH};

/// Milliseconds in one hour.
pub const HOUR_MS: i64 = 60 * 60 * 1000;
/// Milliseconds in one day.
pub const DAY_MS: i64 = 24 * HOUR_MS;

/// Current Unix time in milliseconds. Returns 0 if the clock is before 1970.
#[must_use]
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_millis()).ok())
        .unwrap_or(0)
}

/// Civil date for a day count since 1970-01-01 (Howard Hinnant's algorithm).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    // `d` is 1..=31 and `m` is 1..=12 by construction.
    (
        y,
        u32::try_from(m).unwrap_or(1),
        u32::try_from(d).unwrap_or(1),
    )
}

/// `YYYY-MM-DD` in UTC for a timestamp in milliseconds.
#[must_use]
pub fn day_utc(ms: i64) -> String {
    let (y, m, d) = civil_from_days(ms.div_euclid(DAY_MS));
    format!("{y:04}-{m:02}-{d:02}")
}

/// RFC 3339 form with milliseconds in UTC, e.g. `2026-09-08T15:00:00.000Z`.
#[must_use]
pub fn rfc3339_utc(ms: i64) -> String {
    let day = day_utc(ms);
    let rem = ms.rem_euclid(DAY_MS);
    let h = rem / HOUR_MS;
    let mi = (rem / 60_000) % 60;
    let s = (rem / 1000) % 60;
    let milli = rem % 1000;
    format!("{day}T{h:02}:{mi:02}:{s:02}.{milli:03}Z")
}

/// Start of the UTC day that contains `ms`.
#[must_use]
pub fn start_of_day_utc(ms: i64) -> i64 {
    ms.div_euclid(DAY_MS) * DAY_MS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_dates() {
        assert_eq!(day_utc(0), "1970-01-01");
        assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00.000Z");
        // 2026-09-08T15:00:00Z
        assert_eq!(rfc3339_utc(1_788_879_600_000), "2026-09-08T15:00:00.000Z");
        // Leap day.
        assert_eq!(day_utc(1_709_164_800_000), "2024-02-29");
        assert_eq!(start_of_day_utc(1_788_879_600_000), 1_788_825_600_000);
    }
}
