//! Data model of the ledger (brief §3). Text values are stored in Spanish
//! exactly as the brief names them, so exports read the same as the brief.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::hash::Hash;

/// Declare an enum whose variants map one-to-one to fixed text values.
macro_rules! str_enum {
    ($(#[$m:meta])* $name:ident { $($(#[$vm:meta])* $variant:ident => $s:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum $name {
            $( $(#[$vm])* #[serde(rename = $s)] $variant, )+
        }

        impl $name {
            /// Every variant, in declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// The stable text stored in the database and shown in exports.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $s),+ }
            }
        }

        impl FromStr for $name {
            type Err = Error;
            fn from_str(s: &str) -> Result<Self, Error> {
                match s {
                    $($s => Ok(Self::$variant),)+
                    _ => Err(Error::UnknownValue { kind: stringify!($name), value: s.to_owned() }),
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

str_enum! {
    /// Category of a queried name (brief §5). `Esperado` carries the same
    /// weight as `Rastreador` in the interface: system updates are not a leak.
    Category {
        /// Known tracker.
        Rastreador => "rastreador",
        /// Advertising.
        Publicidad => "publicidad",
        /// Telemetry.
        Telemetria => "telemetria",
        /// Expected traffic: system updates, resolvers, time, messaging, calls.
        Esperado => "esperado",
        /// Not in any list.
        Desconocido => "desconocido",
    }
}

str_enum! {
    /// What Guardiana did with a query (brief §3, decision 8).
    Verdict {
        /// Forwarded to the upstream resolver and answered.
        Observado => "observado",
        /// Answered by Guardiana itself: cache, canary or checker name.
        Respondido => "respondido",
        /// Blocked by a rule.
        Cortado => "cortado",
    }
}

str_enum! {
    /// Who decided the verdict (brief §3, decision 9).
    DecidedBy {
        /// An explicit one-off decision or confirmation by the user.
        Usuario => "usuario",
        /// A persistent rule the user created.
        ReglaUsuario => "regla_usuario",
        /// Nobody: plain observation.
        Nadie => "nadie",
        /// The name fell outside the scope the user declared for that device, and the
        /// device is set to cut what is outside it (decision 154). Kept apart from
        /// `ReglaUsuario` because the person did not name this destination: they named
        /// what the agent was allowed, and this was not in it.
        AlcanceDeclarado => "alcance_declarado",
    }
}

str_enum! {
    /// Name of a signal, for filters and for the ledger text (brief §5).
    SignalKind {
        /// Same name at regular intervals for more than 30 minutes.
        Baliza => "baliza",
        /// First time this device talks to this name.
        DestinoNuevo => "destino_nuevo",
        /// Outside the device's learned hours profile.
        FueraDeHoras => "fuera_de_horas",
        /// Known encrypted-DNS resolver name.
        EvasionDns => "evasion_dns",
        /// More than 5× the device's median queries in the last hour.
        Volumen => "volumen",
    }
}

/// One of the five signals (brief §5), with the detail its sentence needs.
/// None of them is a verdict. Stored in `signals_json` as, e.g.,
/// `["destino_nuevo", {"baliza": {"minutes": 5}}]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Signal {
    /// Same name at regular intervals for more than 30 minutes.
    Baliza {
        /// The interval, in whole minutes, for "cada N minutos".
        minutes: u32,
    },
    /// First time this device talks to this name.
    DestinoNuevo,
    /// Outside the device's learned hours profile.
    FueraDeHoras,
    /// Known encrypted-DNS resolver name.
    EvasionDns,
    /// More than 5× the device's median queries in the last hour.
    Volumen,
}

impl Signal {
    /// Which signal this is, without its detail.
    #[must_use]
    pub const fn kind(&self) -> SignalKind {
        match self {
            Self::Baliza { .. } => SignalKind::Baliza,
            Self::DestinoNuevo => SignalKind::DestinoNuevo,
            Self::FueraDeHoras => SignalKind::FueraDeHoras,
            Self::EvasionDns => SignalKind::EvasionDns,
            Self::Volumen => SignalKind::Volumen,
        }
    }

    /// The signal's name, as stored.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        self.kind().as_str()
    }
}

str_enum! {
    /// Scope of a rule (brief §3).
    Scope {
        /// One device.
        Device => "device",
        /// The whole home.
        Home => "home",
    }
}

str_enum! {
    /// How a rule matches a name (brief §3).
    MatchKind {
        /// Exact name.
        Domain => "domain",
        /// The name or any subdomain of it.
        Suffix => "suffix",
        /// Every name of a category.
        Category => "category",
    }
}

str_enum! {
    /// What a rule does (brief §3).
    Action {
        /// Block: answer NXDOMAIN (or 0.0.0.0).
        Cortar => "cortar",
        /// Allow, overriding a broader block.
        Permitir => "permitir",
    }
}

str_enum! {
    /// Why the program itself sent something out (brief §3, table `outbound`).
    Purpose {
        /// License activation.
        Licencia => "licencia",
        /// Version check.
        Version => "version",
        /// List update.
        Listas => "listas",
    }
}

/// One stored query. Answers are never stored (brief §4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// Row id, increasing in insertion order.
    pub id: i64,
    /// Unix time in milliseconds.
    pub ts: i64,
    /// Device the query came from (`self` for this computer).
    pub device_id: String,
    /// IP the query came from.
    pub client_ip: String,
    /// Queried name, lowercase, without trailing dot.
    pub qname: String,
    /// Query type (`A`, `AAAA`, `HTTPS`, ...).
    pub qtype: String,
    /// Category from the lists.
    pub category: Category,
    /// Which list produced the category, empty if none.
    pub list_source: String,
    /// Signals observed on this query.
    pub signals: Vec<Signal>,
    /// What Guardiana did.
    pub verdict: Verdict,
    /// Who decided.
    pub decided_by: DecidedBy,
    /// Rule that caused the verdict, if any.
    pub rule_id: Option<i64>,
    /// Hash of the previous row (or the chain anchor).
    pub prev_hash: Hash,
    /// `sha256(prev_hash || fields)`.
    pub row_hash: Hash,
}

/// A query about to be appended. The ledger fills in id and hashes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEvent {
    /// Unix time in milliseconds.
    pub ts: i64,
    /// Device the query came from.
    pub device_id: String,
    /// IP the query came from.
    pub client_ip: String,
    /// Queried name.
    pub qname: String,
    /// Query type.
    pub qtype: String,
    /// Category from the lists.
    pub category: Category,
    /// Which list produced the category, empty if none.
    pub list_source: String,
    /// Signals observed on this query.
    pub signals: Vec<Signal>,
    /// What Guardiana did.
    pub verdict: Verdict,
    /// Who decided.
    pub decided_by: DecidedBy,
    /// Rule that caused the verdict, if any.
    pub rule_id: Option<i64>,
}

impl NewEvent {
    /// A plain observation of `qname` from `device_id`, with no list hit,
    /// no signals and no rule. Callers set what they know afterwards.
    #[must_use]
    pub fn observed(ts: i64, device_id: &str, client_ip: &str, qname: &str, qtype: &str) -> Self {
        Self {
            ts,
            device_id: device_id.to_owned(),
            client_ip: client_ip.to_owned(),
            qname: qname.to_owned(),
            qtype: qtype.to_owned(),
            category: Category::Desconocido,
            list_source: String::new(),
            signals: Vec::new(),
            verdict: Verdict::Observado,
            decided_by: DecidedBy::Nadie,
            rule_id: None,
        }
    }
}

/// A device seen on the network (brief §3 and §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    /// Stable identifier (`self`, or derived from the MAC by the devices crate).
    pub id: String,
    /// MAC address if known.
    pub mac: Option<String>,
    /// Last IP seen for this device.
    pub last_ip: Option<String>,
    /// Name given by the user, `None` until named ("Dispositivo nuevo en tu Wi‑Fi").
    pub name: Option<String>,
    /// First time seen, Unix ms.
    pub first_seen: i64,
    /// Last time seen, Unix ms.
    pub last_seen: i64,
    /// Whether the owner shares this device's detail with the home panel. Off by default.
    pub share_detail_with_home: bool,
    /// Learned hours profile, JSON, owned by the classify crate.
    pub hours_profile_json: Option<String>,
}

/// A user rule (brief §3 and §6). Never created by the program on its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    /// Row id.
    pub id: i64,
    /// Device or home.
    pub scope: Scope,
    /// Device the rule applies to when `scope` is `Device`.
    pub device_id: Option<String>,
    /// How `pattern` matches.
    pub match_kind: MatchKind,
    /// Name, suffix or category text.
    pub pattern: String,
    /// Block or allow.
    pub action: Action,
    /// Creation time, Unix ms.
    pub created_at: i64,
    /// Who created it (`usuario` from which device/panel).
    pub created_by: String,
    /// Expiry, Unix ms, if any.
    pub expires_at: Option<i64>,
    /// When it was undone, Unix ms. Undoing never deletes.
    pub undone_at: Option<i64>,
    /// The user confirmed the warning for cutting expected traffic (brief §6).
    #[serde(default)]
    pub confirmed: bool,
}

impl Rule {
    /// Whether the rule is in force at `now`.
    #[must_use]
    pub fn is_active(&self, now: i64) -> bool {
        self.undone_at.is_none() && self.expires_at.is_none_or(|e| e > now)
    }
}

/// A rule about to be stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRule {
    /// Device or home.
    pub scope: Scope,
    /// Device the rule applies to when `scope` is `Device`.
    pub device_id: Option<String>,
    /// How `pattern` matches.
    pub match_kind: MatchKind,
    /// Name, suffix or category text.
    pub pattern: String,
    /// Block or allow.
    pub action: Action,
    /// Creation time, Unix ms.
    pub created_at: i64,
    /// Who created it.
    pub created_by: String,
    /// Expiry, Unix ms, if any.
    pub expires_at: Option<i64>,
    /// The user confirmed the warning for cutting expected traffic.
    pub confirmed: bool,
}

/// Something the program itself sent out (brief §3). Shown in `/sabe-de-ti`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outbound {
    /// Row id.
    pub id: i64,
    /// Unix ms.
    pub ts: i64,
    /// Why.
    pub purpose: Purpose,
    /// Host contacted.
    pub host: String,
    /// Bytes sent.
    pub bytes: i64,
    /// Whether the user started it (always true in 1.0; stored anyway).
    pub initiated_by_user: bool,
}
