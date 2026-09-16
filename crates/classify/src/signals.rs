//! Per-device history behind the signals (brief §5).

use std::collections::{HashMap, VecDeque};

use guardiana_core::time::{DAY_MS, HOUR_MS};
use serde::{Deserialize, Serialize};

/// Tunable limits. Defaults are the brief's numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thresholds {
    /// Beacon: minimum span between first and last regular query (30 min).
    pub beacon_min_span_ms: i64,
    /// Beacon: allowed deviation from the median interval, in percent (20).
    pub beacon_tolerance_percent: i64,
    /// Beacon: minimum samples before judging regularity.
    pub beacon_min_samples: usize,
    /// Off hours: days of history before the profile is trusted (3).
    pub profile_learn_days: i64,
    /// Volume: multiple of the median hourly count (5).
    pub volume_factor: u32,
    /// Volume: never fire below this many queries in the hour (decision 19).
    pub volume_floor: u32,
    /// Volume: completed hours kept for the median (24).
    pub volume_history_hours: usize,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            beacon_min_span_ms: 30 * 60 * 1000,
            beacon_tolerance_percent: 20,
            beacon_min_samples: 4,
            profile_learn_days: 3,
            volume_factor: 5,
            volume_floor: 50,
            volume_history_hours: 24,
        }
    }
}

/// Which hours (UTC) a device is usually active. Persisted per device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HoursProfile {
    /// Queries seen per UTC hour of day.
    pub counts: [u32; 24],
    /// First query ever, Unix ms; the profile is trusted `profile_learn_days` after it.
    pub first_ts: Option<i64>,
    /// Total queries counted.
    pub total: u64,
}

impl HoursProfile {
    fn hour_of(ts: i64) -> usize {
        usize::try_from(ts.rem_euclid(DAY_MS) / HOUR_MS).unwrap_or(0)
    }

    /// Whether enough days were observed to trust the profile.
    #[must_use]
    pub fn learned(&self, now: i64, learn_days: i64) -> bool {
        self.first_ts
            .is_some_and(|first| now - first >= learn_days * DAY_MS)
    }

    /// True when the device had never been seen at this hour. Checked before recording.
    fn is_unusual(&self, ts: i64) -> bool {
        self.counts[Self::hour_of(ts)] == 0
    }

    fn record(&mut self, ts: i64) {
        if self.first_ts.is_none() {
            self.first_ts = Some(ts);
        }
        let h = Self::hour_of(ts);
        self.counts[h] = self.counts[h].saturating_add(1);
        self.total += 1;
    }
}

/// Timestamps of recent queries to one name.
const BEACON_WINDOW: usize = 64;
/// Names tracked per device before old ones are dropped.
const MAX_NAMES_PER_DEVICE: usize = 5000;

/// In-memory history of one device.
#[derive(Debug, Clone, Default)]
pub struct DeviceState {
    recent: HashMap<String, VecDeque<i64>>,
    hour_index: Option<i64>,
    hour_count: u32,
    completed_hours: VecDeque<u32>,
    /// Hours profile, persisted by the caller.
    pub profile: HoursProfile,
    pub(crate) profile_dirty: bool,
}

impl DeviceState {
    /// Record a query to `name` and return the beacon period in minutes if
    /// the recent history is regular for long enough.
    pub(crate) fn beacon(&mut self, name: &str, ts: i64, t: &Thresholds) -> Option<u32> {
        if self.recent.len() >= MAX_NAMES_PER_DEVICE && !self.recent.contains_key(name) {
            let cutoff = ts - 2 * HOUR_MS;
            self.recent
                .retain(|_, times| times.back().is_some_and(|&last| last >= cutoff));
        }
        let times = self.recent.entry(name.to_owned()).or_default();
        times.push_back(ts);
        while times.len() > BEACON_WINDOW {
            times.pop_front();
        }
        let all: Vec<i64> = times.iter().copied().collect();
        for window in [all.len(), 16, 8, t.beacon_min_samples] {
            if window > all.len() || window < t.beacon_min_samples {
                continue;
            }
            if let Some(period) = regular_period(&all[all.len() - window..], t) {
                return Some(period);
            }
        }
        None
    }

    /// Record the query's hour in the profile; true when it is unusual and the profile is learned.
    pub(crate) fn off_hours(&mut self, ts: i64, t: &Thresholds) -> bool {
        let unusual = self.profile.learned(ts, t.profile_learn_days) && self.profile.is_unusual(ts);
        self.profile.record(ts);
        self.profile_dirty = true;
        unusual
    }

    /// Count the query in the current hour; true when the hour is far above the median.
    pub(crate) fn volume(&mut self, ts: i64, t: &Thresholds) -> bool {
        let index = ts.div_euclid(HOUR_MS);
        match self.hour_index {
            Some(current) if current == index => {}
            Some(current) => {
                self.completed_hours.push_back(self.hour_count);
                // Hours with no queries at all count as zero.
                let gap = (index - current - 1).clamp(0, 24);
                for _ in 0..gap {
                    self.completed_hours.push_back(0);
                }
                while self.completed_hours.len() > t.volume_history_hours {
                    self.completed_hours.pop_front();
                }
                self.hour_index = Some(index);
                self.hour_count = 0;
            }
            None => self.hour_index = Some(index),
        }
        self.hour_count = self.hour_count.saturating_add(1);
        if self.completed_hours.len() < 3 {
            return false;
        }
        let mut sorted: Vec<u32> = self.completed_hours.iter().copied().collect();
        sorted.sort_unstable();
        let median = sorted[sorted.len() / 2];
        let limit = median.saturating_mul(t.volume_factor).max(t.volume_floor);
        self.hour_count > limit
    }
}

/// Median interval in whole minutes when every interval is within tolerance
/// of the median and the samples span at least the minimum.
fn regular_period(times: &[i64], t: &Thresholds) -> Option<u32> {
    if times.len() < t.beacon_min_samples {
        return None;
    }
    let span = times[times.len() - 1] - times[0];
    if span < t.beacon_min_span_ms {
        return None;
    }
    let mut intervals: Vec<i64> = times.windows(2).map(|w| w[1] - w[0]).collect();
    intervals.sort_unstable();
    let median = intervals[intervals.len() / 2];
    if median <= 0 {
        return None;
    }
    let tolerance = median * t.beacon_tolerance_percent / 100;
    let regular = intervals.iter().all(|&i| (i - median).abs() <= tolerance);
    if !regular {
        return None;
    }
    let minutes = ((median + 30_000) / 60_000).max(1);
    u32::try_from(minutes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regular_period_respects_tolerance() {
        let t = Thresholds::default();
        let m = 60_000;
        assert_eq!(regular_period(&[0, 10 * m, 20 * m, 30 * m], &t), Some(10));
        // 18 and 22 minutes are within 20 % of 20.
        assert_eq!(regular_period(&[0, 18 * m, 38 * m, 60 * m], &t), Some(20));
        // 25 minutes is outside 20 % of 20.
        assert_eq!(regular_period(&[0, 20 * m, 45 * m, 65 * m], &t), None);
        // Too short a span.
        assert_eq!(regular_period(&[0, 5 * m, 10 * m, 15 * m], &t), None);
    }

    #[test]
    fn profile_learns_after_three_days() {
        let mut p = HoursProfile::default();
        assert!(!p.learned(0, 3));
        p.record(0);
        assert!(!p.learned(2 * DAY_MS, 3));
        assert!(p.learned(3 * DAY_MS, 3));
        assert!(p.is_unusual(5 * HOUR_MS));
        assert!(!p.is_unusual(0));
    }
}
