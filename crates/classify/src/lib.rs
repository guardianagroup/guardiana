//! Category by name plus the five signals (brief §5): `baliza`,
//! `destino_nuevo`, `fuera_de_horas`, `evasion_dns`, `volumen`.
//!
//! No signal is a verdict. The classifier keeps a small in-memory history
//! per device (recent timestamps per name, hourly counts, hours profile)
//! and returns, for each query, a category and the signals that fired. The
//! caller writes the ledger; this crate stores nothing itself except the
//! hours profile it hands back for persistence.

mod signals;

use std::collections::HashMap;
use std::sync::Arc;

use guardiana_core::{Category, Signal};
use guardiana_lists::{Catalog, ExpectedKind};

pub use signals::{DeviceState, HoursProfile, Thresholds};

/// One query to classify.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Input<'a> {
    /// Device the query came from.
    pub device_id: &'a str,
    /// Queried name, lowercase, no trailing dot.
    pub name: &'a str,
    /// Unix time in milliseconds.
    pub ts: i64,
    /// Whether the ledger had never seen this name for this device.
    pub first_time: bool,
}

/// What the classifier concluded about one query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classified {
    /// Category from the lists, `Desconocido` if none.
    pub category: Category,
    /// Which list decided the category, empty if none.
    pub list_source: String,
    /// The list entry that matched, if any.
    pub matched: Option<String>,
    /// For `esperado`, which kind (decides the §6 confirmation text).
    pub expected: Option<ExpectedKind>,
    /// Signals observed on this query, in a fixed order.
    pub signals: Vec<Signal>,
}

/// The classifier: a catalog plus per-device history.
pub struct Classifier {
    catalog: Arc<Catalog>,
    thresholds: Thresholds,
    devices: HashMap<String, DeviceState>,
}

impl Classifier {
    /// A classifier over `catalog` with the brief's thresholds.
    #[must_use]
    pub fn new(catalog: Arc<Catalog>) -> Self {
        Self::with_thresholds(catalog, Thresholds::default())
    }

    /// A classifier with custom thresholds (tests, tuning).
    #[must_use]
    pub fn with_thresholds(catalog: Arc<Catalog>, thresholds: Thresholds) -> Self {
        Self {
            catalog,
            thresholds,
            devices: HashMap::new(),
        }
    }

    /// The catalog in use.
    #[must_use]
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// Restore a device's hours profile from `devices.hours_profile_json`.
    pub fn load_profile(&mut self, device_id: &str, json: &str) -> Result<(), serde_json::Error> {
        let profile: HoursProfile = serde_json::from_str(json)?;
        self.state(device_id).profile = profile;
        Ok(())
    }

    /// Hours profiles changed since the last call, as JSON, for persistence.
    pub fn take_dirty_profiles(&mut self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for (id, state) in &mut self.devices {
            if state.profile_dirty {
                state.profile_dirty = false;
                if let Ok(json) = serde_json::to_string(&state.profile) {
                    out.push((id.clone(), json));
                }
            }
        }
        out
    }

    fn state(&mut self, device_id: &str) -> &mut DeviceState {
        self.devices.entry(device_id.to_owned()).or_default()
    }

    /// Classify one query and update the device's history.
    pub fn classify(&mut self, input: &Input<'_>) -> Classified {
        let hit = self.catalog.lookup(input.name);
        let evasion = self.catalog.is_evasion_resolver(input.name);
        let thresholds = self.thresholds;
        let state = self.state(input.device_id);

        let mut signals = Vec::new();
        if let Some(minutes) = state.beacon(input.name, input.ts, &thresholds) {
            signals.push(Signal::Baliza { minutes });
        }
        if input.first_time {
            signals.push(Signal::DestinoNuevo);
        }
        if state.off_hours(input.ts, &thresholds) {
            signals.push(Signal::FueraDeHoras);
        }
        if evasion {
            signals.push(Signal::EvasionDns);
        }
        if state.volume(input.ts, &thresholds) {
            signals.push(Signal::Volumen);
        }

        match hit {
            Some(m) => Classified {
                category: m.category,
                list_source: m.source.to_owned(),
                matched: Some(m.matched),
                expected: m.expected,
                signals,
            },
            None => Classified {
                category: Category::Desconocido,
                list_source: String::new(),
                matched: None,
                expected: None,
                signals,
            },
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use guardiana_core::time::{DAY_MS, HOUR_MS};
    use guardiana_core::SignalKind;

    const MIN: i64 = 60_000;

    fn classifier() -> Classifier {
        let mut catalog = Catalog::new();
        catalog.add_guardiana("@hora\npool.ntp.org\n", Category::Esperado, "own");
        catalog.add_source(&guardiana_lists::SOURCES[0], "||tracker.example^\n");
        catalog.add_evasion("dns.google\n");
        Classifier::new(Arc::new(catalog))
    }

    fn kinds(c: &Classified) -> Vec<SignalKind> {
        c.signals.iter().map(Signal::kind).collect()
    }

    #[test]
    fn category_and_new_destination() {
        let mut c = classifier();
        let out = c.classify(&Input {
            device_id: "self",
            name: "a.tracker.example",
            ts: 0,
            first_time: true,
        });
        assert_eq!(out.category, Category::Rastreador);
        assert_eq!(out.list_source, "easyprivacy");
        assert_eq!(out.matched.as_deref(), Some("tracker.example"));
        assert_eq!(kinds(&out), vec![SignalKind::DestinoNuevo]);

        let out = c.classify(&Input {
            device_id: "self",
            name: "0.pool.ntp.org",
            ts: 1,
            first_time: false,
        });
        assert_eq!(out.category, Category::Esperado);
        assert_eq!(out.expected, Some(ExpectedKind::Hora));
        assert!(out.signals.is_empty());
    }

    #[test]
    fn beacon_needs_regular_intervals_over_thirty_minutes() {
        let mut c = classifier();
        let mut last = None;
        for i in 0..8 {
            last = Some(c.classify(&Input {
                device_id: "self",
                name: "beat.example",
                ts: i * 5 * MIN,
                first_time: i == 0,
            }));
        }
        // 8 samples, 35 minutes span, every 5 minutes.
        assert_eq!(last.unwrap().signals, vec![Signal::Baliza { minutes: 5 }]);

        // Irregular: never a beacon.
        let mut c = classifier();
        let mut last = None;
        for (i, t) in [0, 3, 9, 10, 25, 26, 40, 41].iter().enumerate() {
            last = Some(c.classify(&Input {
                device_id: "self",
                name: "random.example",
                ts: t * MIN,
                first_time: i == 0,
            }));
        }
        assert!(last.unwrap().signals.is_empty());

        // Regular but only 15 minutes so far: not yet.
        let mut c = classifier();
        let mut last = None;
        for i in 0..4 {
            last = Some(c.classify(&Input {
                device_id: "self",
                name: "young.example",
                ts: i * 5 * MIN,
                first_time: i == 0,
            }));
        }
        assert!(last.unwrap().signals.is_empty());
    }

    #[test]
    fn off_hours_only_after_three_days_of_profile() {
        let mut c = classifier();
        // Three days of activity between 09:00 and 17:00 UTC.
        for day in 0..3 {
            for hour in 9..17 {
                c.classify(&Input {
                    device_id: "phone",
                    name: "work.example",
                    ts: day * DAY_MS + hour * HOUR_MS,
                    first_time: false,
                });
            }
        }
        // Day 4 at 03:00 (more than three days after the first query):
        // never seen this hour → off hours.
        let out = c.classify(&Input {
            device_id: "phone",
            name: "night.example",
            ts: 4 * DAY_MS + 3 * HOUR_MS,
            first_time: false,
        });
        assert_eq!(kinds(&out), vec![SignalKind::FueraDeHoras]);
        // Same day at 10:00: usual hour → nothing.
        let out = c.classify(&Input {
            device_id: "phone",
            name: "work.example",
            ts: 4 * DAY_MS + 10 * HOUR_MS,
            first_time: false,
        });
        assert!(out.signals.is_empty());
        // Another device with one day of history: profile not learned yet.
        let out = c.classify(&Input {
            device_id: "tv",
            name: "x.example",
            ts: 3 * HOUR_MS,
            first_time: false,
        });
        assert!(out.signals.is_empty());
        let dirty = c.take_dirty_profiles();
        assert!(dirty.iter().any(|(id, _)| id == "phone"));
        assert!(c.take_dirty_profiles().is_empty());
    }

    #[test]
    fn profile_round_trips_through_json() {
        let mut c = classifier();
        for day in 0..3 {
            c.classify(&Input {
                device_id: "phone",
                name: "a.example",
                ts: day * DAY_MS + 12 * HOUR_MS,
                first_time: false,
            });
        }
        let (id, json) = c.take_dirty_profiles().pop().unwrap();
        let mut fresh = classifier();
        fresh.load_profile(&id, &json).unwrap();
        let out = fresh.classify(&Input {
            device_id: "phone",
            name: "b.example",
            ts: 4 * DAY_MS + 2 * HOUR_MS,
            first_time: false,
        });
        assert_eq!(kinds(&out), vec![SignalKind::FueraDeHoras]);
    }

    #[test]
    fn evasion_resolver_is_flagged() {
        let mut c = classifier();
        let out = c.classify(&Input {
            device_id: "self",
            name: "dns.google",
            ts: 0,
            first_time: false,
        });
        assert_eq!(kinds(&out), vec![SignalKind::EvasionDns]);
    }

    #[test]
    fn volume_fires_above_five_times_the_median_and_the_floor() {
        let mut c = classifier();
        // Three quiet hours of 10 queries each.
        for hour in 0..3 {
            for i in 0..10 {
                c.classify(&Input {
                    device_id: "self",
                    name: "q.example",
                    ts: hour * HOUR_MS + i * 1000,
                    first_time: false,
                });
            }
        }
        // Hour 3: 5 × median = 50, floor 50 → the 51st query fires.
        let mut fired_at = None;
        for i in 0..60 {
            let out = c.classify(&Input {
                device_id: "self",
                name: "q.example",
                ts: 3 * HOUR_MS + i * 1000,
                first_time: false,
            });
            if kinds(&out).contains(&SignalKind::Volumen) {
                fired_at = fired_at.or(Some(i + 1));
            }
        }
        assert_eq!(fired_at, Some(51));
    }
}
