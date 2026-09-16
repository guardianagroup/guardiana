//! Events, hash-chained SQLite ledger, CSV/JSON export and rules (brief §3).
//!
//! Everything in this crate happens on the user's machine. It never opens a
//! network connection. The ledger is one SQLite file per installation whose
//! `events` rows form a hash chain: each row carries the hash of the previous
//! row, so a deleted or altered row breaks the chain and
//! [`Ledger::check`] reports exactly where.
//!
//! Only DNS *queries* are ever stored: name, type, device, time, category,
//! signals and verdict. Answers are never stored (brief §4).

mod changes;
mod error;
mod export;
mod gaps;
mod hash;
pub mod i18n;
pub mod identity;
mod ledger;
mod model;
pub mod paths;
mod report;
mod retention;
pub mod rules;
mod stats;
pub mod time;

pub use changes::{Change, ChangeKind};
pub use error::{Error, Result};
pub use export::{write_csv, write_json};
pub use gaps::{Gap, GAP_THRESHOLD_MS, KEY_HEARTBEAT};
pub use hash::{chain_hash, Hash, HashInput};
pub use ledger::{ChainFault, CheckReport, EventFilter, Ledger};
pub use model::{
    Action, Category, DecidedBy, Device, Event, MatchKind, NewEvent, NewRule, Outbound, Purpose,
    Rule, Scope, Signal, SignalKind, Verdict,
};
pub use report::{DeviceWeek, WeekSummary};
pub use retention::{DailyTotal, PruneReport, Retention};
pub use stats::{Counters, DeviceTotals, TableCounts};

/// Identifier of the computer running Guardiana in the `devices` table (brief §3).
pub const SELF_DEVICE_ID: &str = "self";
