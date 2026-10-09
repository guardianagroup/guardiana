//! Days without a calendar crate: the local date of a moment, for the shield's daily and monthly
//! totals. The offset comes from the browser's own pages (`Date.getTimezoneOffset`), so the day
//! changes at the person's midnight, not at UTC's.

/// `YYYY-MM-DD` of `ms` (milliseconds since the epoch) at `zona_min` minutes east of UTC.
#[must_use]
pub fn dia(ms: i64, zona_min: i32) -> String {
    let local = ms + i64::from(zona_min) * 60_000;
    let (a, m, d) = civil(local.div_euclid(86_400_000));
    format!("{a:04}-{m:02}-{d:02}")
}

/// `YYYY-MM-DD HH:MM:SS`, local time.
#[must_use]
pub fn hora_local(ms: i64, zona_min: i32) -> String {
    let local = ms + i64::from(zona_min) * 60_000;
    let s = local.div_euclid(1000).rem_euclid(86_400);
    format!(
        "{} {:02}:{:02}:{:02}",
        dia(ms, zona_min),
        s / 3600,
        (s / 60) % 60,
        s % 60
    )
}

/// The day `dias` days before the day of `ms`.
#[must_use]
pub fn dia_antes(ms: i64, zona_min: i32, dias: i64) -> String {
    dia(ms - dias * 86_400_000, zona_min)
}

/// The first day of the month of a `YYYY-MM-DD` day.
#[must_use]
pub fn primero_del_mes(dia: &str) -> String {
    match dia.get(..7) {
        Some(mes) => format!("{mes}-01"),
        None => dia.to_string(),
    }
}

/// Year, month and day of a count of days since 1970-01-01 (H. Hinnant's algorithm).
fn civil(dias: i64) -> (i64, i64, i64) {
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let a = yoe + era * 400 + i64::from(m <= 2);
    (a, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_fall_at_local_midnight() {
        assert_eq!(dia(0, 0), "1970-01-01");
        // 2026-10-09T13:39:31Z
        let t = 1_791_553_171_000;
        assert_eq!(dia(t, 0), "2026-10-09");
        // Bogotá (UTC−5) at 02:00 UTC on the 10th is still the 9th.
        assert_eq!(dia(1_791_597_600_000, -300), "2026-10-09");
        assert_eq!(dia(1_791_597_600_000, 120), "2026-10-10");
        assert_eq!(dia(951_782_400_000, 0), "2000-02-29");
        assert_eq!(primero_del_mes("2026-10-09"), "2026-10-01");
        assert_eq!(hora_local(t, -300), "2026-10-09 08:39:31");
        assert_eq!(dia_antes(t, 0, 9), "2026-09-30");
    }
}
