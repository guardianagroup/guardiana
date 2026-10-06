//! Licensing (brief §9, decisions 52 and 53).
//!
//! One program, one plan. Plus is a subscription, monthly or yearly — or, for the launch's
//! limited "Fundador" licence, a single payment that never expires:
//!
//! - a 7-day trial that starts the first time the program runs and lives in the local settings
//!   and in a mark only an administrator can remove (`ancla`); when it ends the program stops
//!   watching and gives the system DNS back, nothing is charged;
//! - a key from the gateway (Dodo Payments): one activation call when the user types the key,
//!   then one validation when the gateway's own free week ends and one per billing period,
//!   recorded in `outbound` and shown in the panel. Only an explicit "not valid" from the
//!   gateway ends a licence. A check that cannot be done is retried every hour; a subscription
//!   stays on for a grace period counted from the first failed attempt and then stops, with a
//!   notice, never breaking the DNS, and keeps retrying so a working network brings it back
//!   without retyping the key (review of 1 Oct 2026, entries 3 and 4).
//!
//! The stored licence is marked with its own secret, created once and never regenerated, and
//! both the secret and the licence are copied to the anchor, so neither a lost panel token nor a
//! lost data folder turns a paying customer into "trial over" (entries 9 and 12). Every licence
//! decision is taken with the latest clock reading ever seen, so moving the clock back does not
//! stretch anything (entry 11).

pub mod ancla;

use guardiana_core::time::{DAY_MS, HOUR_MS};
use guardiana_core::{identity, Ledger, Purpose};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Settings key: Unix ms when the Plus trial started.
pub const SETTING_TRIAL_STARTED: &str = "plus_trial_started_at";
/// Settings key: the licence key itself (needed for the periodic check).
pub const SETTING_LICENSE_KEY: &str = "license_key";
/// Settings key: the gateway's activation response, once accepted.
pub const SETTING_LICENSE_ACTIVATION: &str = "license_key_activation";
/// Settings key: local integrity mark of the stored key and activation.
pub const SETTING_LICENSE_KEY_MARK: &str = "license_key_mark";
/// Settings key: the licence's own secret (hex), the one the mark is made with. Created once,
/// never regenerated, copied to the anchor. Until 1.0.1 the panel session token played this
/// role, and losing that file lost the licence (review of 1 Oct 2026, entry 9).
pub const SETTING_LICENSE_SECRET: &str = "license_secret";
/// Settings key: Unix ms of the key activation.
pub const SETTING_LICENSE_KEY_AT: &str = "license_key_at";
/// Settings key: Unix ms of the last successful validation.
pub const SETTING_LICENSE_CHECKED_AT: &str = "license_checked_at";
/// Settings key: Unix ms of the first failed validation attempt since the last success. The
/// grace period is counted from here, never from the calendar (entry 4).
pub const SETTING_LICENSE_CHECK_FAILED_AT: &str = "license_check_failed_at";
/// Settings key: Unix ms when the gateway said the key is no longer valid.
pub const SETTING_LICENSE_ENDED_AT: &str = "license_ended_at";
/// Settings key: Unix ms of the first answer since the last success in which the gateway itself
/// turned the key down with a 4xx (not found, disabled). For a licence bought once this is what
/// a refund looks like when the gateway does not answer `valid:false` (review of 5 Oct 2026,
/// licence item 8); a check that could not be done never counts here.
pub const SETTING_LICENSE_REJECTED_AT: &str = "license_rejected_at";
/// Settings key: "1" once this version has asked the gateway again about an end that 1.0.1
/// stored, or about a 1.0.1 licence whose local mark no longer verifies (licence item 7).
pub const SETTING_LICENSE_REVISADA: &str = "license_revisada_1_0_2";
/// Settings key: days per billing period (30, 365, or 0 for a licence bought once).
pub const SETTING_LICENSE_PERIOD_DAYS: &str = "license_period_days";
/// Settings key: the latest clock reading this installation has seen, Unix ms. It only goes up,
/// and every licence decision uses the later of it and the clock (entry 11).
pub const SETTING_CLOCK_SEEN: &str = "license_clock_seen";
/// Trial length.
pub const TRIAL_DAYS: i64 = 7;
/// Days a key subscription keeps working after its first failed validation attempt.
pub const GRACE_DAYS: i64 = 7;
/// Days after activating a subscription key when it is checked for the first time. Until 5 Oct
/// 2026 monthly and yearly started at the gateway with 7 free days and no card, and the key
/// arrived on day one; if no card was added, the gateway put the subscription on hold on day 7
/// and disabled the key. Checking only once per period left Plus on for up to ~37 days without
/// paying; checking the day after those 7 days cut it then (1.0.1, at the owner's request).
/// Since 5 Oct 2026 the gateway charges at purchase (the only free week is the program's own);
/// the day-8 check stays, for subscriptions bought before and for a refund to reach the
/// machine soon. After that first check, once per billing period as before.
pub const PRIMERA_COMPROBACION_DIAS: i64 = TRIAL_DAYS + 1;
/// Billing period assumed for a monthly key.
pub const MONTH_DAYS: i64 = 30;
/// Billing period assumed for a yearly key.
pub const YEAR_DAYS: i64 = 365;
/// A licence sold once, for good: it never expires for a failed check. Stored as 0 so an older
/// installation that does not know about it falls back to the monthly rule instead of crashing.
/// It is still checked, monthly, so a refund ends it as the panel and the terms say (entry 13).
pub const DE_POR_VIDA: i64 = 0;
/// The gateway (decision 50: Dodo Payments). Its licence endpoints are public.
pub const GATEWAY_HOST: &str = "live.dodopayments.com";
/// The same gateway in test mode, where no real money moves. Used to try the
/// whole activation flow before anyone is charged.
pub const TEST_GATEWAY_HOST: &str = "test.dodopayments.com";
/// Kept for the panel text: the host of every licence connection.
pub const ACTIVATE_HOST: &str = GATEWAY_HOST;

/// Environment variable that sends licence calls to the test gateway.
pub const TEST_GATEWAY_ENV: &str = "GUARDIANA_GATEWAY_TEST";

/// The clock mark in the ledger is refreshed at most once a minute, and the one in the anchor at
/// most once an hour: `status` runs every few seconds while the panel is open, and on Windows
/// the anchor is a process (`reg`).
const RELOJ_PASO_EXTRACTO_MS: i64 = 60 * 1000;
const RELOJ_PASO_ANCLA_MS: i64 = HOUR_MS;
/// How far past real time the remembered clock may move between two of this process's writes.
const RELOJ_HOLGURA_MS: i64 = 5 * 60 * 1000;

/// The last clock reading this process remembered, and when, by the monotonic clock. A reading
/// that runs ahead of the time that really passed in this process is not remembered beyond it:
/// until 1.0.2 a date set a year ahead by mistake, for a minute, ended the trial for good
/// (review of 5 Oct 2026, licence item 6). The first write of a process has nothing to compare
/// with and is believed.
static RELOJ_PROCESO: std::sync::Mutex<Option<(std::time::Instant, i64)>> =
    std::sync::Mutex::new(None);

/// Where licence calls go. The test gateway is only reachable from a build that
/// carries the development key: a published binary carries the release key and
/// always talks to the live gateway, so no installation out there can be
/// redirected by setting a variable. Whatever this returns is what goes into the
/// `outbound` table and what the panel shows, so the record never disagrees with
/// where the connection actually went.
#[must_use]
pub fn gateway_host() -> &'static str {
    if identity::public_key_is_dev() && std::env::var_os(TEST_GATEWAY_ENV).is_some() {
        TEST_GATEWAY_HOST
    } else {
        GATEWAY_HOST
    }
}

/// Activation endpoint: one call when the user types the key.
#[must_use]
pub fn activate_url() -> String {
    format!("https://{}/licenses/activate", gateway_host())
}

/// Validation endpoint: one call per billing period.
#[must_use]
pub fn validate_url() -> String {
    format!("https://{}/licenses/validate", gateway_host())
}

/// Errors of licensing.
#[derive(Debug)]
pub enum Error {
    /// The gateway's answer is not the expected JSON shape.
    Malformed(String),
    /// The gateway answered that the key is not valid.
    KeyRejected(String),
    /// The key typed is empty: nothing was sent (review of 1 Oct 2026, entry 26).
    EmptyKey,
    /// The network call failed.
    Network(String),
    /// Plus is already active: nothing to try.
    AlreadyPlus,
    /// Storage failed.
    Ledger(guardiana_core::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(s) => write!(f, "gateway answer: {s}"),
            Self::KeyRejected(s) => write!(f, "key rejected: {s}"),
            Self::EmptyKey => f.write_str("empty key"),
            Self::Network(s) => write!(f, "network: {s}"),
            Self::AlreadyPlus => f.write_str("Plus already active"),
            Self::Ledger(e) => write!(f, "ledger: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<guardiana_core::Error> for Error {
    fn from(e: guardiana_core::Error) -> Self {
        Self::Ledger(e)
    }
}

/// State of the periodic check of a key subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Comprobacion {
    /// Last check succeeded and the next one is not due yet.
    AlDia,
    /// A check is due (the program will do it on its next housekeeping pass).
    Pendiente,
    /// The gateway could not be reached; the grace period is running.
    Fallida,
}

/// The plan in force.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "plan", rename_all = "snake_case")]
pub enum Plan {
    /// Plus trial running.
    Prueba {
        /// Trial start.
        empieza: i64,
        /// Trial end.
        termina: i64,
        /// Whole days left, between 0 and `TRIAL_DAYS`.
        dias_restantes: i64,
    },
    /// Trial used up, no subscription: the program stops watching and gives the system DNS back.
    PruebaAgotada {
        /// When it ended.
        termino: i64,
    },
    /// Plus subscription active.
    Plus {
        /// Always `clave`.
        origen: String,
        /// Activation time, Unix ms.
        desde: i64,
        /// Holder if known.
        titular: Option<String>,
        /// Days per billing period (30, 365, or 0 when it was bought once and never expires).
        periodo_dias: Option<i64>,
        /// Last successful validation, Unix ms (key only).
        comprobada: Option<i64>,
        /// When the next validation is due, Unix ms (key only).
        proxima_comprobacion: Option<i64>,
        /// Hard end, Unix ms: the end of the grace that started with the first failed check.
        /// `None` while no check has failed, and always for a licence bought once.
        caduca_ms: Option<i64>,
        /// State of the periodic check (key only).
        comprobacion: Option<Comprobacion>,
    },
    /// Plus ended: the subscription was cancelled, or the key could not be checked for the
    /// whole grace period. The program stops watching and gives the system DNS
    /// back; the ledger stays on the machine, whole, to be exported.
    PlusTerminado {
        /// When it ended.
        termino: i64,
        /// `cancelada`, `sin_comprobar`, or `rechazada` (a licence bought once whose key the
        /// gateway turned down for the whole grace).
        motivo: String,
    },
}

/// Licence status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Status {
    /// The plan in force.
    pub plan: Plan,
    /// Whether Plus features are on (subscription or trial).
    pub plus_activo: bool,
    /// Whether the program may watch at all. False once the trial or the subscription is over:
    /// Guardiana then stops watching and restores the system DNS, and the panel offers only the
    /// licence page and the export of what is already on the machine.
    pub puede_funcionar: bool,
    /// Home Mode is always allowed (decision 52); kept for the callers.
    pub hogar_permitido: bool,
    /// Milliseconds this installation has been observing.
    pub observado_ms: i64,
}

fn mark(secret: &str, payload: &str) -> String {
    let mut h = Sha256::new();
    h.update(secret.as_bytes());
    h.update(b"\n");
    h.update(payload.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn key_mark(secret: &str, key: &str, activation: &str) -> String {
    mark(secret, &format!("{key}\n{activation}"))
}

/// A fresh licence secret: 32 random bytes, hex. Made once per installation and never again.
fn nuevo_secreto() -> Result<String, Error> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| {
        Error::Ledger(guardiana_core::Error::Io(std::io::Error::other(
            e.to_string(),
        )))
    })?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Shape of the gateway's activation answer (the fields we use).
#[derive(Debug, Deserialize, Default)]
struct ActivationResponse {
    /// Activation instance id (`license_key_instance_id` for later calls).
    #[serde(default)]
    id: String,
    #[serde(default)]
    product: Option<ActivationProduct>,
    #[serde(default)]
    customer: Option<ActivationCustomer>,
}

#[derive(Debug, Deserialize, Default)]
struct ActivationProduct {
    #[serde(default)]
    product_id: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

/// The three products sold at guardianagroup.com/comprar.html, by their Dodo Payments id. The id
/// decides the period before the name does: the monthly and the yearly subscription are both
/// shown to the buyer as "GUARDIANA Plus", and a name can be edited in the gateway any day, while
/// the id of a product never changes (1.0.1).
const PRODUCTO_MENSUAL: &str = "pdt_0NnxPZGJy7PYHrGx1oBki";
const PRODUCTO_ANUAL: &str = "pdt_0NnxPZ40xI6mR2MOoaAMt";
const PRODUCTO_FUNDADOR: &str = "pdt_0NnxfRzo4H78OnLGFOS2H";

/// Billing period of an activation: by product id when it is one of ours, else by name.
fn period_days_of(product: Option<&ActivationProduct>) -> i64 {
    period_days_from_id(product.and_then(|p| p.product_id.as_deref()))
        .unwrap_or_else(|| period_days_from_product(product.and_then(|p| p.name.as_deref())))
}

fn period_days_from_id(id: Option<&str>) -> Option<i64> {
    match id? {
        PRODUCTO_MENSUAL => Some(MONTH_DAYS),
        PRODUCTO_ANUAL => Some(YEAR_DAYS),
        PRODUCTO_FUNDADOR => Some(DE_POR_VIDA),
        _ => None,
    }
}

#[derive(Debug, Deserialize, Default)]
struct ActivationCustomer {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    email: Option<String>,
}

/// Shape of the gateway's validation answer. `valid` is not defaulted on purpose: a 2xx body
/// without it (a maintenance page, a proxy's block page, a changed API) is not an answer, and
/// until 1.0.1 it read as "not valid" and cancelled the licence for good (review of 1 Oct 2026,
/// entry 3).
#[derive(Debug, Deserialize, Default)]
struct ValidationResponse {
    valid: Option<bool>,
}

/// Billing period from the product name: a one-off licence never expires, a yearly one is a year,
/// anything else is a month. The lifetime case is not a detail: a person who paid once for a
/// "Fundador" licence and then left the machine off for five weeks must never have Plus switched
/// off by a rule that exists for subscriptions.
fn period_days_from_product(name: Option<&str>) -> i64 {
    let n = name.unwrap_or("").to_lowercase();
    if n.contains("fundador")
        || n.contains("founder")
        || n.contains("vitalicia")
        || n.contains("lifetime")
    {
        DE_POR_VIDA
    } else if n.contains("anual") || n.contains("annual") || n.contains("year") || n.contains("año")
    {
        YEAR_DAYS
    } else {
        MONTH_DAYS
    }
}

fn setting_i64(ledger: &Ledger, key: &str) -> Result<Option<i64>, Error> {
    Ok(ledger.setting(key)?.and_then(|v| v.parse::<i64>().ok()))
}

fn setting_text(ledger: &Ledger, key: &str) -> Result<Option<String>, Error> {
    Ok(ledger.setting(key)?.filter(|v| !v.is_empty()))
}

/// How long this installation has been observing: since the first device
/// (this PC included) was seen. 0 before the first query.
fn observed_ms(ledger: &Ledger, now: i64) -> Result<i64, Error> {
    let first = ledger.devices()?.iter().map(|d| d.first_seen).min();
    Ok(first.map_or(0, |f| (now - f).max(0)))
}

fn finish(ledger: &Ledger, plan: Plan, now: i64) -> Result<Status, Error> {
    let plus_activo = matches!(plan, Plan::Plus { .. } | Plan::Prueba { .. });
    let observado_ms = observed_ms(ledger, now)?;
    Ok(Status {
        // There is one program and one plan: while the trial or the subscription is on it works,
        // and when neither is it stops watching and gives the system DNS back. It never keeps
        // resolving badly and it never leaves the machine without a resolver.
        puede_funcionar: plus_activo,
        plus_activo,
        hogar_permitido: true,
        observado_ms,
        plan,
    })
}

/// The clock every licence decision runs on: the later of the system clock and the latest
/// reading ever seen, kept in the ledger and in the anchor (review of 1 Oct 2026, entry 11).
/// Without it, setting the machine's date back put the trial back inside its seven days and the
/// panel showed hundreds of days left.
fn reloj(ledger: &Ledger, marcas: &ancla::Marcas, now: i64) -> Result<i64, Error> {
    let en_extracto = setting_i64(ledger, SETTING_CLOCK_SEEN)?;
    let visto = en_extracto.into_iter().chain(marcas.visto).max();
    let mut proceso = RELOJ_PROCESO
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // A floor that time itself moves: the last reading this process remembered plus the time that
    // really passed since, by the monotonic clock. With the date held back the system clock stood
    // below the remembered one, the licence clock stopped at it, and the trial never ended
    // (review of 5 Oct 2026, licence medium). Now it goes on counting while the program runs.
    let monotono = proceso.map(|(cuando, antes)| {
        antes.saturating_add(i64::try_from(cuando.elapsed().as_millis()).unwrap_or(i64::MAX))
    });
    let reloj = [Some(now), visto, monotono]
        .into_iter()
        .flatten()
        .max()
        .unwrap_or(now);
    // What may be remembered: the reading, but no further past the remembered clock than the
    // time this process has really been running since it last moved it, plus a little.
    let recordable = match (*proceso, visto) {
        (Some((cuando, antes)), Some(v)) => {
            let pasado = i64::try_from(cuando.elapsed().as_millis()).unwrap_or(i64::MAX);
            reloj.min(
                v.max(antes)
                    .saturating_add(pasado)
                    .saturating_add(RELOJ_HOLGURA_MS),
            )
        }
        _ => reloj,
    };
    let mut escrito = false;
    if en_extracto.is_none_or(|v| recordable >= v + RELOJ_PASO_EXTRACTO_MS) {
        ledger.set_setting(SETTING_CLOCK_SEEN, &recordable.to_string())?;
        escrito = true;
    }
    if marcas
        .visto
        .is_none_or(|v| recordable >= v + RELOJ_PASO_ANCLA_MS)
    {
        // Best effort: without administrator rights the ledger's own copy still counts.
        ancla::escribir_visto(recordable);
        escrito = true;
    }
    if escrito || proceso.is_none() {
        *proceso = Some((std::time::Instant::now(), recordable));
    }
    Ok(reloj)
}

/// Tests run many "processes" in one: each step that stands for a new start forgets this one.
#[cfg(test)]
pub(crate) fn olvidar_proceso() {
    *RELOJ_PROCESO
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
}

/// The stored licence, once its mark verified: what the plan is decided from.
struct Guardada {
    activation: String,
    since: i64,
    period: i64,
}

/// Billing period of a stored licence: the product id in the activation (covered by the mark)
/// wins over the period saved next to it, so a key activated with 1.0.0, which read the period
/// only from the product name, gets the right one without activating again.
fn periodo_guardado(ledger: &Ledger, activation: &str) -> Result<i64, Error> {
    let parsed: ActivationResponse = serde_json::from_str(activation).unwrap_or_default();
    let id = parsed
        .product
        .as_ref()
        .and_then(|p| p.product_id.as_deref());
    match period_days_from_id(id) {
        Some(p) => Ok(p),
        None => Ok(setting_i64(ledger, SETTING_LICENSE_PERIOD_DAYS)?.unwrap_or(MONTH_DAYS)),
    }
}

/// The licence this installation holds, if any: the ledger's copy when its mark verifies, else
/// the anchor's. `legacy_secret` is the panel session token, which 1.0.0 and 1.0.1 used to mark
/// the licence: it is tried once, and a licence it verifies is re-marked with the licence's own
/// secret, so the token can be lost or regenerated from then on (review of 1 Oct 2026, entries 9
/// and 12). Whatever verifies is copied to the anchor: key, activation number and product, and
/// nothing about the person (review of 5 Oct 2026, licence item 4).
fn licencia_guardada(
    ledger: &Ledger,
    marcas: &ancla::Marcas,
    legacy_secret: &str,
    reloj: i64,
) -> Result<Option<Guardada>, Error> {
    let own = setting_text(ledger, SETTING_LICENSE_SECRET)?;
    let anclada = marcas.licencia.as_ref();
    let key = setting_text(ledger, SETTING_LICENSE_KEY)?;
    let activation = setting_text(ledger, SETTING_LICENSE_ACTIVATION)?;
    if let (Some(key), Some(activation)) = (key, activation) {
        let stored_mark = ledger
            .setting(SETTING_LICENSE_KEY_MARK)?
            .unwrap_or_default();
        // Candidates, in order: the licence's own secret; the panel token, for a licence
        // activated by 1.0.0 or 1.0.1.
        let candidates: Vec<&str> = [own.as_deref(), Some(legacy_secret)]
            .into_iter()
            .flatten()
            .filter(|c| !c.is_empty())
            .collect();
        if let Some(found) = candidates
            .iter()
            .find(|c| stored_mark == key_mark(c, &key, &activation))
        {
            let canonical = match &own {
                Some(o) => o.clone(),
                None => nuevo_secreto()?,
            };
            if own.is_none() {
                ledger.set_setting(SETTING_LICENSE_SECRET, &canonical)?;
            }
            if *found != canonical {
                ledger.set_setting(
                    SETTING_LICENSE_KEY_MARK,
                    &key_mark(&canonical, &key, &activation),
                )?;
            }
            let since = setting_i64(ledger, SETTING_LICENSE_KEY_AT)?.unwrap_or(reloj);
            let period = periodo_guardado(ledger, &activation)?;
            // The anchor keeps what it takes to get the licence back, so losing the data folder
            // does not lose what was paid.
            let parsed: ActivationResponse = serde_json::from_str(&activation).unwrap_or_default();
            let copia = ancla::Licencia {
                clave: key,
                instancia: parsed.id,
                producto: parsed
                    .product
                    .and_then(|p| p.product_id)
                    .unwrap_or_default(),
                desde: since,
                periodo: period,
            };
            if !copia.instancia.is_empty() && anclada != Some(&copia) {
                ancla::escribir_licencia(&copia);
            }
            return Ok(Some(Guardada {
                activation,
                since,
                period,
            }));
        }
    }
    // Nothing usable in the ledger: read the licence back from the anchor, under the licence's
    // own secret (a new one if the ledger lost that too). The activation is rebuilt with what the
    // gateway needs and the period needs; the holder's name is not kept, so it is not shown until
    // the gateway is asked again. The check history starts afresh, so it is asked at the next
    // pass. An end the ledger still records stays: restoring from the anchor used to clear it,
    // and a refunded licence came back by deleting three rows (licence item 7).
    let Some(a) = anclada else {
        return Ok(None);
    };
    let secreto = secreto_propio(ledger)?;
    let activacion = serde_json::json!({
        "id": a.instancia,
        "product": { "product_id": (!a.producto.is_empty()).then_some(a.producto.as_str()) },
    })
    .to_string();
    ledger.set_setting(SETTING_LICENSE_KEY, &a.clave)?;
    ledger.set_setting(SETTING_LICENSE_ACTIVATION, &activacion)?;
    ledger.set_setting(
        SETTING_LICENSE_KEY_MARK,
        &key_mark(&secreto, &a.clave, &activacion),
    )?;
    ledger.set_setting(SETTING_LICENSE_KEY_AT, &a.desde.to_string())?;
    ledger.set_setting(SETTING_LICENSE_PERIOD_DAYS, &a.periodo.to_string())?;
    ledger.set_setting(SETTING_LICENSE_CHECKED_AT, "")?;
    ledger.set_setting(SETTING_LICENSE_CHECK_FAILED_AT, "")?;
    ledger.set_setting(SETTING_LICENSE_REJECTED_AT, "")?;
    let period = periodo_guardado(ledger, &activacion)?;
    Ok(Some(Guardada {
        activation: activacion,
        since: a.desde,
        period,
    }))
}

/// The plan a stored licence gives at `reloj`.
fn plan_de_clave(ledger: &Ledger, g: &Guardada, reloj: i64) -> Result<Plan, Error> {
    if let Some(ended) = setting_i64(ledger, SETTING_LICENSE_ENDED_AT)? {
        return Ok(Plan::PlusTerminado {
            termino: ended,
            motivo: "cancelada".to_owned(),
        });
    }
    let parsed: ActivationResponse = serde_json::from_str(&g.activation).unwrap_or_default();
    let checked = setting_i64(ledger, SETTING_LICENSE_CHECKED_AT)?.unwrap_or(g.since);
    let de_por_vida = g.period == DE_POR_VIDA;
    // A licence bought once has no billing period, but it is still asked about, monthly, like
    // the shortest subscription: a refund has to reach the machine, as the panel and the terms
    // say it does. Until 1.0.1 it was never checked (review of 1 Oct 2026, entry 13).
    let intervalo = if de_por_vida { MONTH_DAYS } else { g.period };
    let next = if checked <= g.since {
        // Never checked since it was activated: the first check is when the gateway's own
        // trial is over (see PRIMERA_COMPROBACION_DIAS), or earlier if the period is shorter.
        (checked + intervalo * DAY_MS).min(g.since + PRIMERA_COMPROBACION_DIAS * DAY_MS)
    } else {
        checked + intervalo * DAY_MS
    };
    let failed_at = setting_i64(ledger, SETTING_LICENSE_CHECK_FAILED_AT)?;
    // The grace runs from the first failed attempt, never from the calendar: a laptop that was
    // off for two weeks has not failed anything. And a licence bought once is never cut for a
    // check that could not be done -- not offline, not with the machine off for months -- as
    // sold. What does end it, besides an explicit "not valid", is the gateway itself turning the
    // key down (a 4xx) for the whole grace: until 1.0.2 a refunded Founder licence whose key the
    // gateway answered with "not found" kept working for good (review of 5 Oct 2026, item 8).
    let caduca = if de_por_vida {
        setting_i64(ledger, SETTING_LICENSE_REJECTED_AT)?
    } else {
        failed_at
    }
    .map(|f| f.saturating_add(GRACE_DAYS * DAY_MS));
    if let Some(fin) = caduca {
        if reloj >= fin {
            return Ok(Plan::PlusTerminado {
                termino: fin,
                motivo: if de_por_vida {
                    "rechazada"
                } else {
                    "sin_comprobar"
                }
                .to_owned(),
            });
        }
    }
    let comprobacion = if failed_at.is_some() {
        Comprobacion::Fallida
    } else if reloj < next {
        Comprobacion::AlDia
    } else {
        Comprobacion::Pendiente
    };
    Ok(Plan::Plus {
        origen: "clave".to_owned(),
        desde: g.since,
        titular: parsed
            .customer
            .and_then(|c| c.email.or(c.name))
            .filter(|s| !s.is_empty()),
        periodo_dias: Some(g.period),
        comprobada: Some(checked),
        proxima_comprobacion: Some(next),
        caduca_ms: caduca,
        comprobacion: Some(comprobacion),
    })
}

/// Current status from the settings. `secret` is the panel session token, which 1.0.0 and
/// 1.0.1 used to mark the stored licence; it is only tried to migrate such a licence to its own
/// secret, and may be empty.
pub fn status(ledger: &Ledger, secret: &str, now: i64) -> Result<Status, Error> {
    status_con_reloj(ledger, secret, now).map(|(s, _)| s)
}

/// `status`, plus the clock it was decided on.
fn status_con_reloj(ledger: &Ledger, secret: &str, now: i64) -> Result<(Status, i64), Error> {
    let marcas = ancla::leer_todo();
    let reloj = reloj(ledger, &marcas, now)?;
    // 1. Key licence, stored with a local mark, in the ledger or in the anchor.
    if let Some(g) = licencia_guardada(ledger, &marcas, secret, reloj)? {
        let plan = plan_de_clave(ledger, &g, reloj)?;
        return finish(ledger, plan, reloj).map(|s| (s, reloj));
    }
    // 2. Trial. The date is kept twice: in the ledger, which travels with the person's data,
    // and in a mark only an administrator can remove (`ancla`). The earlier of the two wins, so
    // deleting one does not restart the seven days and editing one does not extend them. If one
    // of the two is missing, it is written back from the other.
    let en_extracto = setting_i64(ledger, SETTING_TRIAL_STARTED)?;
    let anclado = marcas.prueba;
    let empezo = match (en_extracto, anclado) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    if let Some(started) = empezo {
        if en_extracto != Some(started) {
            ledger.set_setting(SETTING_TRIAL_STARTED, &started.to_string())?;
        }
        if anclado != Some(started) {
            // Best effort; `ancla::leer()` tells whoever asks whether the mark is really there.
            ancla::escribir(started);
        }
    }
    let status = match empezo {
        Some(started) => {
            let ends = started + TRIAL_DAYS * DAY_MS;
            if reloj < ends {
                finish(
                    ledger,
                    Plan::Prueba {
                        empieza: started,
                        termina: ends,
                        // Never a number that does not exist: between 0 and the seven days.
                        dias_restantes: (((ends - reloj) + DAY_MS - 1) / DAY_MS)
                            .clamp(0, TRIAL_DAYS),
                    },
                    reloj,
                )
            } else {
                finish(ledger, Plan::PruebaAgotada { termino: ends }, reloj)
            }
        }
        None => {
            // There is no free plan to wait in: the seven days start the first time the program
            // runs, and the first run is this call. Writing it here and not in the engine means
            // the clock is the same however Guardiana was started (service, terminal or panel).
            ledger.set_setting(SETTING_TRIAL_STARTED, &reloj.to_string())?;
            ancla::escribir(reloj);
            finish(
                ledger,
                Plan::Prueba {
                    empieza: reloj,
                    termina: reloj + TRIAL_DAYS * DAY_MS,
                    dias_restantes: TRIAL_DAYS,
                },
                reloj,
            )
        }
    }?;
    Ok((status, reloj))
}

fn post_json(url: &str, body: &serde_json::Value) -> Result<(u16, String), Error> {
    let response = ureq::post(url)
        .config()
        .timeout_global(Some(std::time::Duration::from_secs(20)))
        .http_status_as_error(false)
        .build()
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .send(serde_json::to_vec(body).unwrap_or_default().as_slice())
        .map_err(|e| Error::Network(e.to_string()))?;
    let code = response.status().as_u16();
    let text = response
        .into_body()
        .read_to_string()
        .map_err(|e| Error::Network(e.to_string()))?;
    Ok((code, text))
}

fn short(body: &str) -> String {
    let t = body.trim();
    let msg = serde_json::from_str::<serde_json::Value>(t)
        .ok()
        .and_then(|v| {
            v.get("message")
                .or_else(|| v.get("error"))
                .and_then(|m| m.as_str().map(str::to_owned))
        })
        .unwrap_or_else(|| t.to_owned());
    msg.chars().take(200).collect()
}

/// The licence's own secret, made on the first activation and kept for good.
fn secreto_propio(ledger: &Ledger) -> Result<String, Error> {
    if let Some(s) = setting_text(ledger, SETTING_LICENSE_SECRET)? {
        return Ok(s);
    }
    let s = nuevo_secreto()?;
    ledger.set_setting(SETTING_LICENSE_SECRET, &s)?;
    Ok(s)
}

/// Activate with a key: one call to the gateway, initiated by the user,
/// recorded in `outbound` with host and bytes whether it succeeds or not.
/// An empty key never leaves the machine (review of 1 Oct 2026, entry 26).
pub fn activate_with_key(
    ledger: &mut Ledger,
    key: &str,
    secret: &str,
    now: i64,
) -> Result<Status, Error> {
    let key = key.trim();
    if key.is_empty() {
        return Err(Error::EmptyKey);
    }
    // Migrate or restore whatever is stored first, so the mark below is made with the
    // licence's own secret, never with the panel token.
    let (_, reloj) = status_con_reloj(ledger, secret, now)?;
    let own = secreto_propio(ledger)?;
    if let Some(st) = revalidate_stored_key(ledger, key, &own, secret, now, reloj)? {
        return Ok(st);
    }
    let instance = format!("guardiana-{}", &mark(&own, "instance")[..8]);
    let body = serde_json::json!({ "license_key": key, "name": instance });
    let sent = serde_json::to_vec(&body).map(|v| v.len()).unwrap_or(0);
    let _ = ledger.record_outbound(now, Purpose::Licencia, gateway_host(), sent as i64, true);
    let (code, text) = post_json(&activate_url(), &body)?;
    if !(200..300).contains(&code) {
        return Err(Error::KeyRejected(short(&text)));
    }
    let parsed: ActivationResponse =
        serde_json::from_str(&text).map_err(|e| Error::Malformed(e.to_string()))?;
    if parsed.id.is_empty() {
        return Err(Error::Malformed("sin id de activación".to_owned()));
    }
    let period = period_days_of(parsed.product.as_ref());
    ledger.set_setting(SETTING_LICENSE_KEY, key)?;
    ledger.set_setting(SETTING_LICENSE_ACTIVATION, &text)?;
    ledger.set_setting(SETTING_LICENSE_KEY_MARK, &key_mark(&own, key, &text))?;
    ledger.set_setting(SETTING_LICENSE_KEY_AT, &reloj.to_string())?;
    ledger.set_setting(SETTING_LICENSE_CHECKED_AT, &reloj.to_string())?;
    ledger.set_setting(SETTING_LICENSE_PERIOD_DAYS, &period.to_string())?;
    ledger.set_setting(SETTING_LICENSE_CHECK_FAILED_AT, "")?;
    ledger.set_setting(SETTING_LICENSE_ENDED_AT, "")?;
    // A new key says nothing about the old one's rejection (review of 5 Oct 2026).
    ledger.set_setting(SETTING_LICENSE_REJECTED_AT, "")?;
    // `status` copies the new licence to the anchor.
    status(ledger, secret, now)
}

/// The same key typed again on a machine that already activated it: ask the gateway whether it
/// is valid now, with the activation already made, instead of activating a second time. This is
/// the way back after the gateway disabled the key (trial over without a card, a payment that
/// failed) and the person then paid: the key is enabled again at the gateway, and a new
/// activation could hit the key's activation limit. `None` when the key is not the stored one.
fn revalidate_stored_key(
    ledger: &mut Ledger,
    key: &str,
    own: &str,
    secret: &str,
    now: i64,
    reloj: i64,
) -> Result<Option<Status>, Error> {
    let stored = ledger.setting(SETTING_LICENSE_KEY)?.unwrap_or_default();
    let activation = ledger
        .setting(SETTING_LICENSE_ACTIVATION)?
        .unwrap_or_default();
    let marked = ledger
        .setting(SETTING_LICENSE_KEY_MARK)?
        .is_some_and(|m| m == key_mark(own, &stored, &activation));
    let instance: ActivationResponse = serde_json::from_str(&activation).unwrap_or_default();
    if stored.is_empty() || stored != key || !marked || instance.id.is_empty() {
        return Ok(None);
    }
    let body = serde_json::json!({ "license_key": key, "license_key_instance_id": instance.id });
    let sent = serde_json::to_vec(&body).map(|v| v.len()).unwrap_or(0);
    let _ = ledger.record_outbound(now, Purpose::Licencia, gateway_host(), sent as i64, true);
    let (code, text) = post_json(&validate_url(), &body)?;
    if !(200..300).contains(&code) {
        return Err(Error::KeyRejected(short(&text)));
    }
    let parsed: ValidationResponse =
        serde_json::from_str(&text).map_err(|e| Error::Malformed(e.to_string()))?;
    match parsed.valid {
        Some(true) => {}
        Some(false) => {
            return Err(Error::KeyRejected(
                "not active yet (the subscription is waiting for a payment)".to_owned(),
            ))
        }
        None => return Err(Error::Malformed("sin campo valid".to_owned())),
    }
    marcar_comprobada(ledger, reloj)?;
    ledger.set_setting(SETTING_LICENSE_CHECK_FAILED_AT, "")?;
    ledger.set_setting(SETTING_LICENSE_ENDED_AT, "")?;
    // The gateway says the key is good: a rejection it gave before is over (review of 5 Oct 2026:
    // it was kept, and a Founder licence that was refused once and then fixed ended anyway).
    ledger.set_setting(SETTING_LICENSE_REJECTED_AT, "")?;
    status(ledger, secret, now).map(Some)
}

/// A successful check, written down. Inside the gateway's own free week it does not count as the
/// first check: that one is on day 8, when the gateway knows whether a card was given. Until
/// 1.0.2 typing the key again during that week moved the next check a whole month away, and a
/// trial that ended without paying kept Plus for that month (review of 5 Oct 2026).
fn marcar_comprobada(ledger: &Ledger, reloj: i64) -> Result<(), Error> {
    let desde = setting_i64(ledger, SETTING_LICENSE_KEY_AT)?;
    let cuando = match desde {
        Some(d) if reloj < d.saturating_add(PRIMERA_COMPROBACION_DIAS * DAY_MS) => d,
        _ => reloj,
    };
    ledger.set_setting(SETTING_LICENSE_CHECKED_AT, &cuando.to_string())?;
    Ok(())
}

/// Whether a validation is to be attempted now: one is due, the last one failed, or the
/// subscription was cut for not being checked -- that one keeps being retried, so a working
/// network brings it back without retyping the key (review of 1 Oct 2026, entry 4).
fn toca_comprobar(plan: &Plan) -> bool {
    match plan {
        Plan::Plus {
            comprobacion: Some(Comprobacion::Pendiente | Comprobacion::Fallida),
            ..
        } => true,
        Plan::PlusTerminado { motivo, .. } => motivo == "sin_comprobar" || motivo == "rechazada",
        _ => false,
    }
}

/// Whether a periodic validation is due now.
pub fn check_due(ledger: &Ledger, secret: &str, now: i64) -> Result<bool, Error> {
    Ok(toca_comprobar(&status(ledger, secret, now)?.plan))
}

/// What one validation answer does to the stored licence. Only an explicit, well-formed "not
/// valid" ends it; anything else -- a network error, a non-2xx code, a 2xx body that is not the
/// expected JSON or has no `valid` -- is a failed check, retried every hour, with the grace
/// running from the first failure (review of 1 Oct 2026, entries 3 and 4). Until 1.0.1 a body
/// that could not be read was stored as "the gateway cancelled your subscription", for ever.
fn aplicar_comprobacion(
    ledger: &Ledger,
    answer: Result<(u16, String), Error>,
    reloj: i64,
) -> Result<(), Error> {
    // A 4xx is the gateway answering about the key; a 5xx, a timeout or no network is a check
    // that could not be done.
    let rechazada = matches!(&answer, Ok((code, _)) if (400..500).contains(code));
    let veredicto = match answer {
        Ok((code, text)) if (200..300).contains(&code) => {
            serde_json::from_str::<ValidationResponse>(&text)
                .ok()
                .and_then(|v| v.valid)
        }
        Ok(_) | Err(Error::Network(_)) => None,
        Err(e) => return Err(e),
    };
    match veredicto {
        Some(true) => {
            marcar_comprobada(ledger, reloj)?;
            ledger.set_setting(SETTING_LICENSE_CHECK_FAILED_AT, "")?;
            ledger.set_setting(SETTING_LICENSE_REJECTED_AT, "")?;
        }
        Some(false) => {
            ledger.set_setting(SETTING_LICENSE_ENDED_AT, &reloj.to_string())?;
        }
        None => {
            if setting_i64(ledger, SETTING_LICENSE_CHECK_FAILED_AT)?.is_none() {
                ledger.set_setting(SETTING_LICENSE_CHECK_FAILED_AT, &reloj.to_string())?;
            }
            if rechazada && setting_i64(ledger, SETTING_LICENSE_REJECTED_AT)?.is_none() {
                ledger.set_setting(SETTING_LICENSE_REJECTED_AT, &reloj.to_string())?;
            }
        }
    }
    Ok(())
}

/// One question to the gateway about what 1.0.1 left behind that this version does not take on
/// trust (review of 5 Oct 2026, licence item 7):
///
/// - an end stored by 1.0.1, which stored one for any 2xx answer it could not read (entry 3):
///   those customers were still "cancelled" after updating;
/// - a 1.0.1 licence whose mark no longer verifies because the panel token it was made with was
///   lost before updating (entry 9): those customers were "trial over".
///
/// The stored key and activation number are validated once. A valid key is marked again with the
/// licence's own secret and any stored end is cleared; an answer that says otherwise is believed
/// and not asked again; a check that could not be done is tried again next time. Only an
/// installation in one of those two states asks anything. Recorded in `outbound`, like the
/// periodic check. Returns whether the licence came back.
pub fn revisar_herencia(ledger: &mut Ledger, secret: &str, now: i64) -> Result<bool, Error> {
    if setting_text(ledger, SETTING_LICENSE_REVISADA)?.is_some() {
        return Ok(false);
    }
    let revisada = |l: &Ledger| l.set_setting(SETTING_LICENSE_REVISADA, "1");
    let (Some(key), Some(activation)) = (
        setting_text(ledger, SETTING_LICENSE_KEY)?,
        setting_text(ledger, SETTING_LICENSE_ACTIVATION)?,
    ) else {
        revisada(ledger)?;
        return Ok(false);
    };
    // `status` first: it migrates a licence whose mark the panel token still verifies.
    let (_, reloj) = status_con_reloj(ledger, secret, now)?;
    let own = setting_text(ledger, SETTING_LICENSE_SECRET)?;
    let stored_mark = ledger
        .setting(SETTING_LICENSE_KEY_MARK)?
        .unwrap_or_default();
    let marcada = [own.as_deref(), Some(secret)]
        .into_iter()
        .flatten()
        .any(|c| !c.is_empty() && stored_mark == key_mark(c, &key, &activation));
    // A mark that fails while the licence already has its own secret is not 1.0.1's: no help.
    let marca_de_antes = !marcada && own.is_none();
    let fin_de_antes = setting_i64(ledger, SETTING_LICENSE_ENDED_AT)?.is_some();
    let instance: ActivationResponse = serde_json::from_str(&activation).unwrap_or_default();
    if !(marca_de_antes || fin_de_antes) || instance.id.is_empty() {
        revisada(ledger)?;
        return Ok(false);
    }
    let body = serde_json::json!({ "license_key": key, "license_key_instance_id": instance.id });
    let sent = serde_json::to_vec(&body).map(|v| v.len()).unwrap_or(0);
    let _ = ledger.record_outbound(now, Purpose::Licencia, gateway_host(), sent as i64, false);
    aplicar_revision(
        ledger,
        &key,
        &activation,
        post_json(&validate_url(), &body),
        reloj,
    )
}

/// What the one answer of [`revisar_herencia`] does: a valid key is marked again and any stored
/// end is cleared; an answer that says otherwise is believed; no answer is asked again later.
fn aplicar_revision(
    ledger: &Ledger,
    key: &str,
    activation: &str,
    answer: Result<(u16, String), Error>,
    reloj: i64,
) -> Result<bool, Error> {
    let revisada = |l: &Ledger| l.set_setting(SETTING_LICENSE_REVISADA, "1");
    match answer {
        Ok((code, text)) if (200..300).contains(&code) => {
            match serde_json::from_str::<ValidationResponse>(&text)
                .ok()
                .and_then(|v| v.valid)
            {
                Some(true) => {
                    let own = secreto_propio(ledger)?;
                    ledger
                        .set_setting(SETTING_LICENSE_KEY_MARK, &key_mark(&own, key, activation))?;
                    marcar_comprobada(ledger, reloj)?;
                    ledger.set_setting(SETTING_LICENSE_CHECK_FAILED_AT, "")?;
                    ledger.set_setting(SETTING_LICENSE_REJECTED_AT, "")?;
                    ledger.set_setting(SETTING_LICENSE_ENDED_AT, "")?;
                    revisada(ledger)?;
                    Ok(true)
                }
                Some(false) => {
                    revisada(ledger)?;
                    Ok(false)
                }
                // Not an answer: ask again next time.
                None => Ok(false),
            }
        }
        Ok((code, _)) if (400..500).contains(&code) => {
            revisada(ledger)?;
            Ok(false)
        }
        Ok(_) | Err(Error::Network(_)) => Ok(false),
        Err(e) => Err(e),
    }
}

/// The periodic validation of a key licence (decision 52). It runs from the program's
/// housekeeping when due; the user consented to it when subscribing and the panel shows when
/// the next one happens. Recorded in `outbound` as not initiated by the user. Returns the
/// status afterwards, or `Ok(None)` when nothing was due.
pub fn check_if_due(ledger: &mut Ledger, secret: &str, now: i64) -> Result<Option<Status>, Error> {
    let (before, reloj) = status_con_reloj(ledger, secret, now)?;
    if !toca_comprobar(&before.plan) {
        return Ok(None);
    }
    let key = ledger.setting(SETTING_LICENSE_KEY)?.unwrap_or_default();
    let activation = ledger
        .setting(SETTING_LICENSE_ACTIVATION)?
        .unwrap_or_default();
    let instance: ActivationResponse = serde_json::from_str(&activation).unwrap_or_default();
    let body = serde_json::json!({
        "license_key": key,
        "license_key_instance_id": if instance.id.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(instance.id) },
    });
    let sent = serde_json::to_vec(&body).map(|v| v.len()).unwrap_or(0);
    let _ = ledger.record_outbound(now, Purpose::Licencia, gateway_host(), sent as i64, false);
    let answer = post_json(&validate_url(), &body);
    aplicar_comprobacion(ledger, answer, reloj)?;
    status(ledger, secret, now).map(Some)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use guardiana_core::Hash;

    /// Cada prueba que llama a `status` toma el cerrojo del módulo `ancla` y trabaja en su propia
    /// carpeta, vacía al empezar: así ninguna escribe la marca de verdad de esta máquina. La
    /// primera versión de esto no lo hacía y una prueba dejó un archivo con fecha 0 en la carpeta
    /// de datos del Mac del responsable, que con el código nuevo habría dado la prueba por
    /// caducada.
    fn a_solas() -> std::sync::MutexGuard<'static, ()> {
        let dir = std::env::temp_dir().join(format!(
            "guardiana-lic-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let g = ancla::a_solas_en(&dir);
        // Each test is a fresh start of the program.
        olvidar_proceso();
        g
    }

    fn ledger_observing() -> Ledger {
        let mut l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        // The PC has been observing since t=0.
        l.upsert_device("self", None, None, 0).unwrap();
        l
    }

    fn plus_con(s: &Status) -> (Comprobacion, Option<i64>, Option<i64>) {
        match &s.plan {
            Plan::Plus {
                comprobacion: Some(c),
                proxima_comprobacion,
                caduca_ms,
                ..
            } => (*c, *proxima_comprobacion, *caduca_ms),
            otro => unreachable!("esperaba Plus, llegó {otro:?}"),
        }
    }

    fn ok_200(body: &str) -> Result<(u16, String), Error> {
        Ok((200, body.to_owned()))
    }

    /// Borrar los datos no devuelve siete días nuevos. La fecha vive en dos sitios —el extracto
    /// y una marca que solo un administrador puede quitar— y manda la más antigua de las dos.
    /// Sin esto, un archivo menos y la prueba empezaba otra vez (aviso del responsable, 22 sep
    /// 2026: «eso sería malo porque lo utilizarían gratis»).
    #[test]
    fn borrar_el_extracto_no_devuelve_la_prueba() {
        let _a_solas = a_solas();
        let empezo = 1_790_000_000_000;

        // Primera vez: se anotan las dos marcas.
        let mut l = ledger_observing();
        let s = status(&l, "k", empezo).unwrap();
        assert!(matches!(s.plan, Plan::Prueba { .. }));
        assert_eq!(ancla::leer(), Some(empezo), "la marca de fuera se escribió");

        // El extracto desaparece entero (lo borra la persona) y se empieza uno nuevo.
        l = ledger_observing();
        let dia_seis = empezo + 6 * DAY_MS;
        let s = status(&l, "k", dia_seis).unwrap();
        let Plan::Prueba {
            empieza,
            dias_restantes,
            ..
        } = s.plan
        else {
            unreachable!("debería seguir en prueba: {:?}", s.plan)
        };
        assert_eq!(empieza, empezo, "sigue contando desde el primer día");
        assert_eq!(dias_restantes, 1, "queda un día, no siete");

        // Y al octavo día se acabó, aunque el extracto sea nuevo.
        let l = ledger_observing();
        let s = status(&l, "k", empezo + 8 * DAY_MS).unwrap();
        assert!(matches!(s.plan, Plan::PruebaAgotada { .. }), "{:?}", s.plan);
        assert!(!s.puede_funcionar);
    }

    /// There is one program and one plan: the first run starts the seven days by itself, and
    /// seven days later the program may not watch any more. Nobody has to press anything to
    /// start the trial, and nobody discovers on day eight that it never started.
    #[test]
    fn the_first_run_starts_the_trial_and_it_ends_seven_days_later() {
        let _a_solas = a_solas();
        let l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        let s = status(&l, "tok", 0).unwrap();
        assert!(
            matches!(
                s.plan,
                Plan::Prueba {
                    dias_restantes: 7,
                    ..
                }
            ),
            "{:?}",
            s.plan
        );
        assert!(s.plus_activo);
        assert!(s.puede_funcionar);
        assert!(s.hogar_permitido);

        // Asking again the next day does not restart it.
        let s = status(&l, "tok", DAY_MS).unwrap();
        assert!(
            matches!(
                s.plan,
                Plan::Prueba {
                    dias_restantes: 6,
                    ..
                }
            ),
            "{:?}",
            s.plan
        );

        // On the eighth day it is over and the program must stand down.
        let s = status(&l, "tok", 7 * DAY_MS + 1).unwrap();
        assert!(matches!(s.plan, Plan::PruebaAgotada { .. }), "{:?}", s.plan);
        assert!(!s.plus_activo);
        assert!(!s.puede_funcionar);
    }

    /// Atrasar el reloj del equipo no devuelve días de prueba ni inventa un contador: manda la
    /// hora más tardía que el programa haya visto, guardada en el extracto y en la marca
    /// (revisión del 1 oct 2026, hallazgo 11).
    #[test]
    fn atrasar_el_reloj_no_alarga_la_prueba() {
        let _a_solas = a_solas();
        let t = 1_790_000_000_000;
        let l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        assert!(matches!(
            status(&l, "tok", t).unwrap().plan,
            Plan::Prueba {
                dias_restantes: 7,
                ..
            }
        ));
        // Six days later (a test stands for them with a new start: within one start, the
        // remembered clock only moves as fast as real time).
        olvidar_proceso();
        let s = status(&l, "tok", t + 6 * DAY_MS).unwrap();
        assert!(matches!(
            s.plan,
            Plan::Prueba {
                dias_restantes: 1,
                ..
            }
        ));
        // The clock goes back five days: still one day left, not six.
        let s = status(&l, "tok", t + DAY_MS).unwrap();
        assert!(
            matches!(
                s.plan,
                Plan::Prueba {
                    dias_restantes: 1,
                    ..
                }
            ),
            "{:?}",
            s.plan
        );
        // A year back: the counter cannot say 372 days.
        let s = status(&l, "tok", t - 365 * DAY_MS).unwrap();
        assert!(
            matches!(
                s.plan,
                Plan::Prueba {
                    dias_restantes: 1,
                    ..
                }
            ),
            "{:?}",
            s.plan
        );
        // Over on day eight, and still over when the clock is set back to day one.
        olvidar_proceso();
        assert!(matches!(
            status(&l, "tok", t + 8 * DAY_MS).unwrap().plan,
            Plan::PruebaAgotada { .. }
        ));
        assert!(matches!(
            status(&l, "tok", t).unwrap().plan,
            Plan::PruebaAgotada { .. }
        ));
        // Even with a brand-new ledger: the anchor remembers the clock too.
        let nuevo = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        assert!(matches!(
            status(&nuevo, "tok", t).unwrap().plan,
            Plan::PruebaAgotada { .. }
        ));
    }

    /// A licence as 1.0.0 and 1.0.1 stored it: marked with the panel token, no secret of its
    /// own.
    fn store_key(l: &Ledger, token: &str, at: i64, product: &str) {
        let activation = format!(
            r#"{{"id":"lki_1","product":{{"product_id":"p","name":"{product}"}},"customer":{{"customer_id":"c","name":"Ana","email":"a@b.c"}}}}"#
        );
        l.set_setting(SETTING_LICENSE_KEY, "KEY-1").unwrap();
        l.set_setting(SETTING_LICENSE_ACTIVATION, &activation)
            .unwrap();
        l.set_setting(
            SETTING_LICENSE_KEY_MARK,
            &key_mark(token, "KEY-1", &activation),
        )
        .unwrap();
        l.set_setting(SETTING_LICENSE_KEY_AT, &at.to_string())
            .unwrap();
        l.set_setting(SETTING_LICENSE_CHECKED_AT, &at.to_string())
            .unwrap();
        l.set_setting(
            SETTING_LICENSE_PERIOD_DAYS,
            &period_days_from_product(Some(product)).to_string(),
        )
        .unwrap();
    }

    #[test]
    fn stored_key_needs_the_local_mark() {
        let _a_solas = a_solas();
        // A key whose mark verifies with nothing -- edited by hand -- does not count, and what
        // is left is the trial this installation started on its first run.
        let l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        store_key(&l, "tok", 5, "GUARDIANA Plus · mensual");
        l.set_setting(SETTING_LICENSE_KEY_MARK, "garbage").unwrap();
        assert!(matches!(
            status(&l, "tok", 10).unwrap().plan,
            Plan::Prueba { .. }
        ));
        // With its mark it counts.
        let l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        store_key(&l, "tok", 5, "GUARDIANA Plus · mensual");
        let s = status(&l, "tok", 10).unwrap();
        assert!(s.plus_activo);
        assert!(matches!(
            s.plan,
            Plan::Plus { ref origen, ref titular, periodo_dias: Some(30), comprobacion: Some(Comprobacion::AlDia), .. }
                if origen == "clave" && titular.as_deref() == Some("a@b.c")
        ));
    }

    /// Hasta la 1.0.1 la licencia pagada estaba atada a la llave de sesión del panel: perder o
    /// regenerar `panel.token` convertía a quien pagó en «prueba agotada». Ahora la licencia
    /// tiene su propio secreto, se migra la marca vieja una vez, y la llave del panel ya no
    /// pinta nada (revisión del 1 oct 2026, hallazgo 9).
    #[test]
    fn cambiar_la_llave_del_panel_no_apaga_la_licencia() {
        let _a_solas = a_solas();
        let l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        store_key(&l, "tok", 5, "GUARDIANA Plus · mensual");
        assert!(status(&l, "tok", 10).unwrap().plus_activo);
        let secreto = l.setting(SETTING_LICENSE_SECRET).unwrap().unwrap();
        assert_eq!(secreto.len(), 64, "un secreto propio, de 32 bytes");
        assert_ne!(secreto, "tok");
        // The mark is now made with that secret, not with the token.
        let activation = l.setting(SETTING_LICENSE_ACTIVATION).unwrap().unwrap();
        assert_eq!(
            l.setting(SETTING_LICENSE_KEY_MARK).unwrap().unwrap(),
            key_mark(&secreto, "KEY-1", &activation)
        );
        // A regenerated token, or none at all: still Plus, same secret.
        assert!(status(&l, "otra-llave", 20).unwrap().plus_activo);
        assert!(status(&l, "", 30).unwrap().plus_activo);
        assert_eq!(
            l.setting(SETTING_LICENSE_SECRET).unwrap().unwrap(),
            secreto,
            "nunca se regenera"
        );
    }

    /// El fin de la prueba vivía fuera de la carpeta de datos; la licencia pagada, no. Ahora la
    /// marca guarda una copia de la licencia y un extracto nuevo la recupera sin volver a
    /// activar (revisión del 1 oct 2026, hallazgo 12).
    #[test]
    fn perder_la_carpeta_de_datos_no_pierde_la_licencia() {
        let _a_solas = a_solas();
        let l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        store_key(&l, "tok", 5, "GUARDIANA Plus · anual");
        assert!(status(&l, "tok", 10).unwrap().plus_activo);
        let copia = ancla::leer_todo().licencia.unwrap();
        assert_eq!(copia.clave, "KEY-1");
        assert_eq!(copia.instancia, "lki_1");
        assert_eq!(copia.desde, 5);
        assert_eq!(copia.periodo, YEAR_DAYS);
        // Nothing about the person, and not the secret, in a place any user may read (review of
        // 5 Oct 2026, licence item 4).
        let crudo = std::fs::read_to_string(ancla::donde_licencia()).unwrap();
        for nada in ["a@b.c", "Ana", "customer", "secreto"] {
            assert!(!crudo.contains(nada), "{nada} en {crudo}");
        }
        assert!(!crudo.contains(&l.setting(SETTING_LICENSE_SECRET).unwrap().unwrap()));

        // The data folder is gone: a brand-new ledger, a different panel token.
        let nuevo = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        let s = status(&nuevo, "otra-llave", 20).unwrap();
        assert!(s.plus_activo, "{:?}", s.plan);
        assert!(matches!(
            s.plan,
            Plan::Plus {
                desde: 5,
                titular: None,
                periodo_dias: Some(YEAR_DAYS),
                comprobada: Some(5),
                ..
            }
        ));
        assert_eq!(
            nuevo.setting(SETTING_LICENSE_KEY).unwrap().as_deref(),
            Some("KEY-1"),
            "la clave vuelve al extracto"
        );
        // Marked with a secret of its own, so the next pass verifies it.
        let secreto = nuevo.setting(SETTING_LICENSE_SECRET).unwrap().unwrap();
        let activation = nuevo.setting(SETTING_LICENSE_ACTIVATION).unwrap().unwrap();
        assert_eq!(
            nuevo.setting(SETTING_LICENSE_KEY_MARK).unwrap().unwrap(),
            key_mark(&secreto, "KEY-1", &activation)
        );
        assert!(activation.contains("lki_1"), "{activation}");
        assert!(status(&nuevo, "", 30).unwrap().plus_activo);
    }

    /// Restoring from the anchor does not clear an end the ledger still records: deleting the
    /// three licence rows used to bring a refunded licence back (licence item 7).
    #[test]
    fn recuperar_del_ancla_no_borra_un_fin() {
        let _a_solas = a_solas();
        let l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        store_key(&l, "tok", 5, "GUARDIANA Plus · mensual");
        assert!(status(&l, "tok", 10).unwrap().plus_activo);
        l.set_setting(SETTING_LICENSE_ENDED_AT, "15").unwrap();
        for k in [
            SETTING_LICENSE_KEY,
            SETTING_LICENSE_ACTIVATION,
            SETTING_LICENSE_KEY_MARK,
        ] {
            l.set_setting(k, "").unwrap();
        }
        let s = status(&l, "tok", 20).unwrap();
        assert!(!s.puede_funcionar, "{:?}", s.plan);
        assert!(matches!(s.plan, Plan::PlusTerminado { termino: 15, .. }));
    }

    /// With the date held back, the trial goes on ending: the licence clock moves with the time that
    /// really passes while the program runs. Until 1.0.2 it stopped at the latest reading seen,
    /// and a clock kept a month behind kept the trial open for good (review of 5 Oct 2026).
    #[test]
    fn con_el_reloj_atrasado_la_prueba_sigue_contando() {
        let _a_solas = a_solas();
        let l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        let t0 = 1_790_000_000_000;
        let _ = status(&l, "", t0).unwrap();
        // A new start six days and 23 hours later sees that time…
        olvidar_proceso();
        let casi = t0 + 6 * DAY_MS + 23 * HOUR_MS;
        assert!(matches!(
            status(&l, "", casi).unwrap().plan,
            Plan::Prueba { .. }
        ));
        // …then the date is put back a month while the program keeps running two more hours.
        let Some(hace) =
            std::time::Instant::now().checked_sub(std::time::Duration::from_secs(2 * 3600))
        else {
            return; // a machine up for less than two hours cannot stand for this
        };
        *RELOJ_PROCESO.lock().unwrap() = Some((hace, casi));
        let s = status(&l, "", t0 - 30 * DAY_MS).unwrap();
        assert!(matches!(s.plan, Plan::PruebaAgotada { .. }), "{:?}", s.plan);
    }

    /// A successful check inside the gateway's free week is not the first check: that one stays on
    /// day 8, when the gateway knows whether a card was given (review of 5 Oct 2026).
    #[test]
    fn una_comprobacion_en_la_semana_gratis_no_mueve_el_dia_8() {
        let _a_solas = a_solas();
        let l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        let desde = 1_790_000_000_000;
        l.set_setting(SETTING_LICENSE_KEY_AT, &desde.to_string())
            .unwrap();
        let valida = || Ok((200, r#"{"valid":true}"#.to_owned()));
        aplicar_comprobacion(&l, valida(), desde + 2 * DAY_MS).unwrap();
        assert_eq!(
            setting_i64(&l, SETTING_LICENSE_CHECKED_AT).unwrap(),
            Some(desde)
        );
        aplicar_comprobacion(&l, valida(), desde + 9 * DAY_MS).unwrap();
        assert_eq!(
            setting_i64(&l, SETTING_LICENSE_CHECKED_AT).unwrap(),
            Some(desde + 9 * DAY_MS)
        );
    }

    /// A date set far ahead by mistake, while the program runs, is not remembered beyond the time
    /// that really passed: when the clock is put right the trial is still there (licence item 6).
    /// A new start believes the clock it finds, as before.
    #[test]
    fn adelantar_el_reloj_un_momento_no_acaba_la_prueba() {
        let _a_solas = a_solas();
        olvidar_proceso();
        let l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        let t0 = 1_790_000_000_000;
        assert!(matches!(
            status(&l, "", t0).unwrap().plan,
            Plan::Prueba { .. }
        ));
        // A year ahead for a moment: the decision of that moment follows the clock…
        let s = status(&l, "", t0 + 365 * DAY_MS).unwrap();
        assert!(matches!(s.plan, Plan::PruebaAgotada { .. }), "{:?}", s.plan);
        // …but what is remembered is not a year ahead.
        let visto = setting_i64(&l, SETTING_CLOCK_SEEN).unwrap().unwrap();
        assert!(visto < t0 + HOUR_MS, "{}", visto - t0);
        // Put right an hour later: still on trial, with its days.
        let s = status(&l, "", t0 + HOUR_MS).unwrap();
        assert!(
            matches!(
                s.plan,
                Plan::Prueba {
                    dias_restantes: 7,
                    ..
                }
            ),
            "{:?}",
            s.plan
        );
        // Setting the clock back is still caught: what was remembered stays the floor.
        olvidar_proceso();
        let _ = status(&l, "", t0 + 3 * DAY_MS).unwrap();
        let s = status(&l, "", t0).unwrap();
        assert!(
            matches!(
                s.plan,
                Plan::Prueba {
                    dias_restantes: 4,
                    ..
                }
            ),
            "{:?}",
            s.plan
        );
    }

    #[test]
    fn key_subscription_is_checked_per_period_with_grace() {
        let _a_solas = a_solas();
        let l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        store_key(&l, "tok", 0, "GUARDIANA Plus · anual");
        // The first check, when the gateway's 7-day trial ends, is tested on its own below; here
        // it already happened (activated a moment before the last successful check), so the
        // clock is the yearly one.
        l.set_setting(SETTING_LICENSE_KEY_AT, "-1").unwrap();
        let year = YEAR_DAYS * DAY_MS;
        // A day short of the year, not a millisecond: the licence clock also counts the time the
        // process really ran since the last reading (a monotonic floor), and on Windows the first
        // status call, which spawns `reg`, takes more than the one millisecond this test left.
        let casi = year - DAY_MS;
        let s = status(&l, "tok", casi).unwrap();
        assert!(matches!(
            s.plan,
            Plan::Plus {
                comprobacion: Some(Comprobacion::AlDia),
                periodo_dias: Some(365),
                ..
            }
        ));
        assert!(!check_due(&l, "tok", casi).unwrap());
        // Due: still Plus, no end date yet, because nothing has failed.
        let s = status(&l, "tok", year + DAY_MS).unwrap();
        assert!(s.plus_activo);
        assert_eq!(plus_con(&s), (Comprobacion::Pendiente, Some(year), None));
        assert!(check_due(&l, "tok", year + DAY_MS).unwrap());
        // A failed attempt is shown as such, Plus still on, and now the grace has an end.
        aplicar_comprobacion(&l, Err(Error::Network("down".to_owned())), year + DAY_MS).unwrap();
        let s = status(&l, "tok", year + 2 * DAY_MS).unwrap();
        assert_eq!(
            plus_con(&s),
            (
                Comprobacion::Fallida,
                Some(year),
                Some(year + DAY_MS + GRACE_DAYS * DAY_MS)
            )
        );
        // After the grace period without a check: the program steps aside, with the reason.
        let s = status(&l, "tok", year + (GRACE_DAYS + 1) * DAY_MS).unwrap();
        assert!(!s.plus_activo);
        assert!(
            matches!(s.plan, Plan::PlusTerminado { ref motivo, termino } if motivo == "sin_comprobar" && termino == year + (GRACE_DAYS + 1) * DAY_MS)
        );
        // A successful check moves the next one a year ahead.
        aplicar_comprobacion(
            &l,
            ok_200(r#"{"valid":true}"#),
            year + (GRACE_DAYS + 1) * DAY_MS,
        )
        .unwrap();
        let s = status(&l, "tok", year + (GRACE_DAYS + 1) * DAY_MS).unwrap();
        assert_eq!(
            plus_con(&s),
            (
                Comprobacion::AlDia,
                Some(2 * year + (GRACE_DAYS + 1) * DAY_MS),
                None
            )
        );
        // The gateway saying "not valid" ends Plus at once.
        aplicar_comprobacion(
            &l,
            ok_200(r#"{"valid":false}"#),
            year + (GRACE_DAYS + 2) * DAY_MS,
        )
        .unwrap();
        let s = status(&l, "tok", year + (GRACE_DAYS + 3) * DAY_MS).unwrap();
        assert!(!s.plus_activo);
        assert!(matches!(s.plan, Plan::PlusTerminado { ref motivo, .. } if motivo == "cancelada"));
        assert!(!check_due(&l, "tok", year + (GRACE_DAYS + 3) * DAY_MS).unwrap());
    }

    /// La gracia corre desde el primer intento fallido, no por calendario: un portátil apagado
    /// dos semanas no ha fallado nada. Y una vez cortada se sigue intentando, así que en cuanto
    /// la red vuelve la licencia vuelve sola, sin reescribir la clave (revisión del 1 oct 2026,
    /// hallazgo 4).
    #[test]
    fn sin_intentos_no_corre_la_gracia_y_un_intento_con_exito_la_devuelve() {
        let _a_solas = a_solas();
        let l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        store_key(&l, "tok", 0, "GUARDIANA Plus · mensual");
        // Day 16, the laptop was off since day 7: Plus, with a check pending and no end date.
        let s = status(&l, "tok", 16 * DAY_MS).unwrap();
        assert!(s.plus_activo, "{:?}", s.plan);
        assert_eq!(
            plus_con(&s),
            (
                Comprobacion::Pendiente,
                Some(PRIMERA_COMPROBACION_DIAS * DAY_MS),
                None
            )
        );
        assert!(check_due(&l, "tok", 16 * DAY_MS).unwrap());
        // The first attempt fails: the seven days start here, on day 16, not on day 8.
        aplicar_comprobacion(&l, Err(Error::Network("down".to_owned())), 16 * DAY_MS).unwrap();
        let s = status(&l, "tok", 16 * DAY_MS).unwrap();
        assert!(s.plus_activo);
        assert_eq!(plus_con(&s).2, Some((16 + GRACE_DAYS) * DAY_MS));
        // More failures do not move the end.
        aplicar_comprobacion(&l, Ok((503, String::new())), 20 * DAY_MS).unwrap();
        assert_eq!(
            plus_con(&status(&l, "tok", 20 * DAY_MS).unwrap()).2,
            Some((16 + GRACE_DAYS) * DAY_MS)
        );
        // Day 23: cut, and still retried.
        let s = status(&l, "tok", 23 * DAY_MS).unwrap();
        assert!(!s.puede_funcionar);
        assert!(
            matches!(s.plan, Plan::PlusTerminado { ref motivo, .. } if motivo == "sin_comprobar")
        );
        assert!(check_due(&l, "tok", 23 * DAY_MS).unwrap());
        // Day 24, the network is back: one good answer and Plus is on again, nothing retyped.
        aplicar_comprobacion(&l, ok_200(r#"{"valid":true}"#), 24 * DAY_MS).unwrap();
        let s = status(&l, "tok", 24 * DAY_MS).unwrap();
        assert!(s.puede_funcionar, "{:?}", s.plan);
        assert_eq!(
            plus_con(&s),
            (Comprobacion::AlDia, Some((24 + MONTH_DAYS) * DAY_MS), None)
        );
        assert!(!check_due(&l, "tok", 24 * DAY_MS).unwrap());
    }

    /// Una respuesta 2xx que no se entiende es una comprobación fallida, temporal, que se
    /// reintenta; nunca una cancelación. Solo un «no vale» explícito y bien formado termina la
    /// licencia (revisión del 1 oct 2026, hallazgo 3).
    #[test]
    fn una_respuesta_que_no_se_entiende_no_cancela_la_licencia() {
        let _a_solas = a_solas();
        let l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        store_key(&l, "tok", 0, "GUARDIANA Plus · mensual");
        let dia = 9 * DAY_MS;
        for cuerpo in [
            "<html>mantenimiento</html>",
            "",
            r#"{"data":{"valid":true}}"#,
            r#"{"valid":"no"}"#,
            r#"{"valid":null}"#,
        ] {
            aplicar_comprobacion(&l, ok_200(cuerpo), dia).unwrap();
            let s = status(&l, "tok", dia).unwrap();
            assert!(s.plus_activo, "{cuerpo:?} -> {:?}", s.plan);
            assert_eq!(plus_con(&s).0, Comprobacion::Fallida, "{cuerpo:?}");
            assert_eq!(
                l.setting(SETTING_LICENSE_ENDED_AT).unwrap().as_deref(),
                None,
                "{cuerpo:?} no es una cancelación"
            );
        }
        // A good answer clears the failure.
        aplicar_comprobacion(&l, ok_200(r#"{"valid":true}"#), dia).unwrap();
        assert_eq!(
            plus_con(&status(&l, "tok", dia).unwrap()).0,
            Comprobacion::AlDia
        );
        // Only the explicit, well-formed "not valid" ends it.
        aplicar_comprobacion(&l, ok_200(r#"{"valid":false}"#), dia).unwrap();
        let s = status(&l, "tok", dia).unwrap();
        assert!(!s.plus_activo);
        assert!(matches!(s.plan, Plan::PlusTerminado { ref motivo, .. } if motivo == "cancelada"));
    }

    /// Con un solo plan ya no hay poda: el extracto es de la persona, haya pagado o no. Antes,
    /// al cancelar, el repaso de la hora siguiente volvía a la retención gratis y quien había
    /// pagado un año perdía el detalle de ese año sin haber visto un aviso (decisión 168).
    #[test]
    fn al_pararse_el_programa_no_se_borra_nada() {
        let _a_solas = a_solas();
        let l = ledger_observing();
        // La prueba corre desde la primera consulta al estado.
        let s = status(&l, "tok", 0).unwrap();
        assert!(s.plus_activo);
        assert!(s.puede_funcionar);

        // Se agota: el programa se aparta, pero nada se borra y el extracto sigue entero.
        let fin = TRIAL_DAYS * DAY_MS;
        let s = status(&l, "tok", fin + 1).unwrap();
        assert!(!s.plus_activo);
        assert!(!s.puede_funcionar);
        assert!(matches!(s.plan, Plan::PruebaAgotada { .. }));

        // Y un año después sigue sin borrarse.
        let s = status(&l, "tok", fin + 365 * DAY_MS).unwrap();
        assert!(!s.puede_funcionar);
        assert!(matches!(s.plan, Plan::PruebaAgotada { .. }));
    }

    /// Una licencia de por vida se comprueba —al mes, como la suscripción más corta— para que
    /// una devolución la termine, como dicen el panel y los términos; pero ningún fallo de red ni
    /// un equipo apagado meses la apaga (revisión del 1 oct 2026, hallazgo 13).
    #[test]
    fn una_licencia_de_por_vida_se_comprueba_pero_no_caduca_por_no_comprobarse() {
        let _a_solas = a_solas();
        let l = Ledger::open_in_memory(guardiana_core::Hash::of(b"x")).unwrap();
        l.set_setting(SETTING_LICENSE_KEY, "GUARDIANA-FUNDADOR")
            .unwrap();
        let activacion = r#"{"id":"inst_1","product":{"name":"GUARDIANA Fundador"}}"#;
        l.set_setting(SETTING_LICENSE_ACTIVATION, activacion)
            .unwrap();
        l.set_setting(
            SETTING_LICENSE_KEY_MARK,
            &key_mark("tok", "GUARDIANA-FUNDADOR", activacion),
        )
        .unwrap();
        l.set_setting(SETTING_LICENSE_KEY_AT, "0").unwrap();
        l.set_setting(
            SETTING_LICENSE_PERIOD_DAYS,
            &period_days_from_product(Some("GUARDIANA Fundador")).to_string(),
        )
        .unwrap();
        // Day 7: nothing due yet; day 8: the first check, like any subscription.
        let s = status(&l, "tok", 7 * DAY_MS).unwrap();
        assert_eq!(
            plus_con(&s),
            (
                Comprobacion::AlDia,
                Some(PRIMERA_COMPROBACION_DIAS * DAY_MS),
                None
            )
        );
        assert!(!check_due(&l, "tok", 7 * DAY_MS).unwrap());
        assert!(check_due(&l, "tok", PRIMERA_COMPROBACION_DIAS * DAY_MS).unwrap());
        // The gateway cannot be reached for months: still Plus, no end date, still retried.
        aplicar_comprobacion(&l, Err(Error::Network("down".to_owned())), 9 * DAY_MS).unwrap();
        let s = status(&l, "tok", 100 * DAY_MS).unwrap();
        assert!(
            s.plus_activo,
            "una licencia comprada una vez no caduca sola: {:?}",
            s.plan
        );
        assert!(matches!(
            s.plan,
            Plan::Plus {
                periodo_dias: Some(DE_POR_VIDA),
                caduca_ms: None,
                comprobacion: Some(Comprobacion::Fallida),
                ..
            }
        ));
        assert!(check_due(&l, "tok", 100 * DAY_MS).unwrap());
        // A good check: the next one is a month ahead.
        aplicar_comprobacion(&l, ok_200(r#"{"valid":true}"#), 100 * DAY_MS).unwrap();
        assert_eq!(
            plus_con(&status(&l, "tok", 100 * DAY_MS).unwrap()),
            (Comprobacion::AlDia, Some((100 + MONTH_DAYS) * DAY_MS), None)
        );
        // Ten years without a single attempt: still Plus.
        let diez_anos = 3650 * DAY_MS;
        assert!(status(&l, "tok", diez_anos).unwrap().plus_activo);
        // But the gateway saying it is no longer valid (a refund) ends it, as the terms say.
        aplicar_comprobacion(&l, ok_200(r#"{"valid":false}"#), diez_anos).unwrap();
        let s2 = status(&l, "tok", diez_anos).unwrap();
        assert!(!s2.plus_activo);
        assert!(matches!(s2.plan, Plan::PlusTerminado { ref motivo, .. } if motivo == "cancelada"));
    }

    /// A Founder licence whose key the gateway itself turns down (a 4xx, which is how a refund
    /// can look) ends after the grace; a check that could not be done still never ends it, and a
    /// good answer in between starts the count again (review of 5 Oct 2026, licence item 8).
    #[test]
    fn una_licencia_de_por_vida_rechazada_por_la_pasarela_termina_tras_la_gracia() {
        let _a_solas = a_solas();
        let l = Ledger::open_in_memory(guardiana_core::Hash::of(b"x")).unwrap();
        let activacion = r#"{"id":"inst_1","product":{"product_id":"pdt_0NnxfRzo4H78OnLGFOS2H"}}"#;
        l.set_setting(SETTING_LICENSE_KEY, "FUNDADOR").unwrap();
        l.set_setting(SETTING_LICENSE_ACTIVATION, activacion)
            .unwrap();
        l.set_setting(
            SETTING_LICENSE_KEY_MARK,
            &key_mark("tok", "FUNDADOR", activacion),
        )
        .unwrap();
        l.set_setting(SETTING_LICENSE_KEY_AT, "0").unwrap();
        let no_encontrada = || Ok((404, r#"{"message":"license key not found"}"#.to_owned()));
        // The gateway down for weeks, with 5xx: never cut.
        aplicar_comprobacion(&l, Ok((503, String::new())), 9 * DAY_MS).unwrap();
        assert!(status(&l, "tok", 60 * DAY_MS).unwrap().plus_activo);
        // A 404, then a good answer: the count starts again.
        aplicar_comprobacion(&l, no_encontrada(), 60 * DAY_MS).unwrap();
        aplicar_comprobacion(&l, ok_200(r#"{"valid":true}"#), 61 * DAY_MS).unwrap();
        assert!(status(&l, "tok", 80 * DAY_MS).unwrap().plus_activo);
        // Turned down from day 90 on: still Plus during the grace, with its date said…
        aplicar_comprobacion(&l, no_encontrada(), 90 * DAY_MS).unwrap();
        aplicar_comprobacion(&l, no_encontrada(), 91 * DAY_MS).unwrap();
        let s = status(&l, "tok", 96 * DAY_MS).unwrap();
        assert!(s.plus_activo);
        assert!(matches!(s.plan, Plan::Plus { caduca_ms: Some(c), .. } if c == 97 * DAY_MS));
        // …and over once it has run, asked again every hour in case it was a mistake.
        let s = status(&l, "tok", 97 * DAY_MS).unwrap();
        assert!(!s.puede_funcionar);
        assert!(matches!(s.plan, Plan::PlusTerminado { ref motivo, .. } if motivo == "rechazada"));
        assert!(check_due(&l, "tok", 97 * DAY_MS).unwrap());
        aplicar_comprobacion(&l, ok_200(r#"{"valid":true}"#), 98 * DAY_MS).unwrap();
        assert!(status(&l, "tok", 98 * DAY_MS).unwrap().plus_activo);
    }

    /// What 1.0.1 left behind is asked about once: an end it stored for an answer it could not
    /// read, and a licence whose mark died with a lost panel token. A valid answer brings the
    /// licence back; nothing else is invented (review of 5 Oct 2026, licence item 7).
    #[test]
    fn lo_que_dejo_la_1_0_1_se_pregunta_una_vez() {
        let _a_solas = a_solas();
        // A 1.0.1 licence marked with a token that is gone: no own secret, the mark fails.
        let l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        store_key(&l, "token-perdido", 0, "GUARDIANA Plus · mensual");
        let s = status(&l, "token-nuevo", 30 * DAY_MS).unwrap();
        assert!(!matches!(s.plan, Plan::Plus { .. }), "{:?}", s.plan);
        let activation = l.setting(SETTING_LICENSE_ACTIVATION).unwrap().unwrap();
        // No answer: not settled, asked again next time.
        assert!(
            !aplicar_revision(&l, "KEY-1", &activation, Err(Error::Network("x".into())), 1)
                .unwrap()
        );
        assert!(l.setting(SETTING_LICENSE_REVISADA).unwrap().is_none());
        // Valid: marked again with its own secret, and Plus.
        assert!(aplicar_revision(
            &l,
            "KEY-1",
            &activation,
            ok_200(r#"{"valid":true}"#),
            30 * DAY_MS
        )
        .unwrap());
        assert!(status(&l, "token-nuevo", 30 * DAY_MS).unwrap().plus_activo);
        assert_eq!(
            l.setting(SETTING_LICENSE_REVISADA).unwrap().as_deref(),
            Some("1")
        );

        // An end 1.0.1 stored by mistake, on a licence that is fine.
        let m = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        store_key(&m, "tok", 0, "GUARDIANA Plus · mensual");
        m.set_setting(SETTING_LICENSE_ENDED_AT, &(20 * DAY_MS).to_string())
            .unwrap();
        assert!(!status(&m, "tok", 30 * DAY_MS).unwrap().puede_funcionar);
        let activation = m.setting(SETTING_LICENSE_ACTIVATION).unwrap().unwrap();
        assert!(aplicar_revision(
            &m,
            "KEY-1",
            &activation,
            ok_200(r#"{"valid":true}"#),
            30 * DAY_MS
        )
        .unwrap());
        assert!(status(&m, "tok", 30 * DAY_MS).unwrap().puede_funcionar);
        // Asked once: the next end stands.
        m.set_setting(SETTING_LICENSE_ENDED_AT, &(40 * DAY_MS).to_string())
            .unwrap();
        let mut m = m;
        assert!(!revisar_herencia(&mut m, "tok", 41 * DAY_MS).unwrap());
        assert!(!status(&m, "tok", 41 * DAY_MS).unwrap().puede_funcionar);

        // An end the gateway confirms stays, and is not asked again.
        let n = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        store_key(&n, "tok", 0, "GUARDIANA Plus · mensual");
        n.set_setting(SETTING_LICENSE_ENDED_AT, &(20 * DAY_MS).to_string())
            .unwrap();
        let activation = n.setting(SETTING_LICENSE_ACTIVATION).unwrap().unwrap();
        assert!(!aplicar_revision(
            &n,
            "KEY-1",
            &activation,
            ok_200(r#"{"valid":false}"#),
            30 * DAY_MS
        )
        .unwrap());
        assert!(!status(&n, "tok", 30 * DAY_MS).unwrap().puede_funcionar);
        assert_eq!(
            n.setting(SETTING_LICENSE_REVISADA).unwrap().as_deref(),
            Some("1")
        );
    }

    /// A licence that is fine is never asked about by the 1.0.1 recheck: no connection.
    #[test]
    fn una_licencia_en_regla_no_se_vuelve_a_preguntar() {
        let _a_solas = a_solas();
        let mut l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        store_key(&l, "tok", 0, "GUARDIANA Plus · mensual");
        assert!(!revisar_herencia(&mut l, "tok", 5 * DAY_MS).unwrap());
        assert!(l.outbound().unwrap().is_empty());
        assert_eq!(
            l.setting(SETTING_LICENSE_REVISADA).unwrap().as_deref(),
            Some("1")
        );
    }

    /// Una clave vacía no sale del equipo: ni conexión ni apunte en `outbound` (revisión del 1
    /// oct 2026, hallazgo 26).
    #[test]
    fn una_clave_vacia_no_sale_del_equipo() {
        let _a_solas = a_solas();
        let mut l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        for clave in ["", "   ", "\t\n"] {
            assert!(matches!(
                activate_with_key(&mut l, clave, "tok", 0),
                Err(Error::EmptyKey)
            ));
        }
        assert!(l.outbound().unwrap().is_empty(), "nada salió");
    }

    #[test]
    fn period_comes_from_the_product_name() {
        assert_eq!(
            period_days_from_product(Some("GUARDIANA Plus · mensual")),
            30
        );
        assert_eq!(
            period_days_from_product(Some("GUARDIANA Plus · anual")),
            365
        );
        assert_eq!(period_days_from_product(Some("Plus yearly")), 365);
        assert_eq!(period_days_from_product(None), 30);
    }

    #[test]
    fn period_comes_from_the_product_id_before_the_name() {
        let producto = |id: &str, name: &str| ActivationProduct {
            product_id: Some(id.to_owned()),
            name: Some(name.to_owned()),
        };
        // Monthly and yearly are both called "GUARDIANA Plus" at the checkout.
        assert_eq!(
            period_days_of(Some(&producto(PRODUCTO_MENSUAL, "GUARDIANA Plus"))),
            MONTH_DAYS
        );
        assert_eq!(
            period_days_of(Some(&producto(PRODUCTO_ANUAL, "GUARDIANA Plus"))),
            YEAR_DAYS
        );
        assert_eq!(
            period_days_of(Some(&producto(PRODUCTO_FUNDADOR, "GUARDIANA Plus"))),
            DE_POR_VIDA
        );
        // An id that is not ours falls back to the name; nothing at all is a month.
        assert_eq!(
            period_days_of(Some(&producto("pdt_otro", "Plus anual"))),
            YEAR_DAYS
        );
        assert_eq!(period_days_of(None), MONTH_DAYS);
    }

    #[test]
    fn a_key_activated_by_1_0_0_gets_the_period_of_its_product_id() {
        let _a_solas = a_solas();
        let l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        // 1.0.0 saved 30 days for a Founder licence whose name did not say "Fundador".
        let activacion = format!(
            r#"{{"id":"lki_9","product":{{"product_id":"{PRODUCTO_FUNDADOR}","name":"GUARDIANA Plus"}}}}"#
        );
        l.set_setting(SETTING_LICENSE_KEY, "KEY-9").unwrap();
        l.set_setting(SETTING_LICENSE_ACTIVATION, &activacion)
            .unwrap();
        l.set_setting(
            SETTING_LICENSE_KEY_MARK,
            &key_mark("tok", "KEY-9", &activacion),
        )
        .unwrap();
        l.set_setting(SETTING_LICENSE_KEY_AT, "0").unwrap();
        l.set_setting(SETTING_LICENSE_PERIOD_DAYS, "30").unwrap();
        let s = status(&l, "tok", 400 * DAY_MS).unwrap();
        assert!(s.plus_activo, "{:?}", s.plan);
        assert!(
            matches!(
                s.plan,
                Plan::Plus {
                    periodo_dias: Some(DE_POR_VIDA),
                    ..
                }
            ),
            "{:?}",
            s.plan
        );
    }

    #[test]
    fn a_new_subscription_is_checked_when_the_gateway_trial_ends() {
        let _a_solas = a_solas();
        let l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        store_key(&l, "tok", 0, "GUARDIANA Plus · mensual");
        // Day 7: no check due yet, and the next one is announced for day 8, not day 30.
        let s = status(&l, "tok", 7 * DAY_MS).unwrap();
        assert_eq!(
            plus_con(&s),
            (
                Comprobacion::AlDia,
                Some(PRIMERA_COMPROBACION_DIAS * DAY_MS),
                None
            )
        );
        // Day 8: the check is due.
        assert!(check_due(&l, "tok", PRIMERA_COMPROBACION_DIAS * DAY_MS + 1).unwrap());
        // Once it passed, the next one is a whole period later.
        l.set_setting(
            SETTING_LICENSE_CHECKED_AT,
            &(PRIMERA_COMPROBACION_DIAS * DAY_MS + 1).to_string(),
        )
        .unwrap();
        let s = status(&l, "tok", 9 * DAY_MS).unwrap();
        assert!(matches!(
            s.plan,
            Plan::Plus { proxima_comprobacion: Some(p), .. } if p == (PRIMERA_COMPROBACION_DIAS + MONTH_DAYS) * DAY_MS + 1
        ));
        // A Founder licence follows the same first-check rule (it used to be never checked).
        let f = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        store_key(&f, "tok", 0, "GUARDIANA Fundador");
        assert!(check_due(&f, "tok", 400 * DAY_MS).unwrap());
    }

    #[test]
    fn licence_urls_follow_the_host_actually_used() {
        // The record and the call have to agree: both are built from the same
        // host, so the ledger can never say one thing and the connection another.
        let host = gateway_host();
        assert!(activate_url().starts_with(&format!("https://{host}/")));
        assert!(validate_url().starts_with(&format!("https://{host}/")));
        assert!(activate_url().ends_with("/licenses/activate"));
        assert!(validate_url().ends_with("/licenses/validate"));
    }

    #[test]
    fn a_published_binary_can_never_be_sent_to_the_test_gateway() {
        // The switch is guarded by the development key, not only by the variable.
        if !identity::public_key_is_dev() {
            std::env::set_var(TEST_GATEWAY_ENV, "1");
            assert_eq!(gateway_host(), GATEWAY_HOST);
            std::env::remove_var(TEST_GATEWAY_ENV);
        }
    }
}
