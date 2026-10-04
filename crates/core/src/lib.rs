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
pub mod trampas;

pub use changes::{Change, ChangeKind, ChangeWho};
pub use error::{Error, Result};
pub use export::{write_csv, write_csv_for_spreadsheet, write_json};
pub use gaps::{Gap, GAP_THRESHOLD_MS, KEY_HEARTBEAT};
pub use hash::{chain_hash, Hash, HashInput};
pub use ledger::{ChainFault, CheckReport, EventFilter, Ledger};
pub use model::{
    Action, Category, DecidedBy, Device, Event, MatchKind, NewEvent, NewRule, Outbound, Proceso,
    Purpose, Rule, Scope, Signal, SignalKind, Verdict,
};
pub use report::{DeviceWeek, WeekSummary};
pub use retention::{DailyTotal, PruneReport, Retention};
pub use stats::{Counters, DeviceTotals, TableCounts};

/// Identifier of the computer running Guardiana in the `devices` table (brief §3).
pub const SELF_DEVICE_ID: &str = "self";

/// Names under this suffix are answered by Guardiana itself (`127.0.0.1` / `::1`) and by
/// nobody else in the world. Asking one through the system's own resolver is the only honest
/// way to know whether this machine's queries really reach Guardiana: reading the DNS settings
/// only says what Windows was told, not what it does. On 27 Sep 2026 the settings said
/// "127.0.0.1 first" and not one query arrived, because the Wi-Fi also handed out IPv6
/// resolvers and Windows asked those. Nothing asked under this suffix is written down.
pub const SELF_CHECK_SUFFIX: &str = "prueba.guardiana.hogar";
