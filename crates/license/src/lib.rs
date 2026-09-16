//! Licensing (brief §9, decisions 52 and 53).
//!
//! The free guardian needs no licence and never expires: the PC and Home
//! Mode are free. Plus is a subscription, monthly or yearly:
//!
//! - a 7-day trial that lives in the local settings, started by the user
//!   (from the panel or `guardiana licencia probar`) and never before 24 h of
//!   observation (decision 53); when it ends everything goes back to free,
//!   nothing is charged;
//! - a key from the gateway (Dodo Payments): one activation call when the
//!   user types the key, then one validation per billing period, recorded in
//!   `outbound` and shown in the panel; if the gateway cannot be reached the
//!   subscription stays valid for a grace period and then stops, with a
//!   notice, never breaking the DNS;
//! - or a licence file signed with our minisign key, issued per period, for
//!   whoever does not want even that connection.
//!
//! The trial counter and the stored activation live in the local settings
//! and the interface says so: "Este contador vive en tu equipo. Reinstalar lo
//! reinicia. Confiamos en ti."

use guardiana_core::time::{DAY_MS, HOUR_MS};
use guardiana_core::{identity, Ledger, Purpose};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Settings key: Unix ms when the Plus trial started.
pub const SETTING_TRIAL_STARTED: &str = "plus_trial_started_at";
/// Settings key: the signed licence file text, once accepted.
pub const SETTING_LICENSE_FILE: &str = "license_file";
/// Settings key: the licence key itself (needed for the periodic check).
pub const SETTING_LICENSE_KEY: &str = "license_key";
/// Settings key: the gateway's activation response, once accepted.
pub const SETTING_LICENSE_ACTIVATION: &str = "license_key_activation";
/// Settings key: local integrity mark of the stored key and activation.
pub const SETTING_LICENSE_KEY_MARK: &str = "license_key_mark";
/// Settings key: Unix ms of the key activation.
pub const SETTING_LICENSE_KEY_AT: &str = "license_key_at";
/// Settings key: Unix ms of the last successful validation.
pub const SETTING_LICENSE_CHECKED_AT: &str = "license_checked_at";
/// Settings key: Unix ms of the first failed validation attempt since the last success.
pub const SETTING_LICENSE_CHECK_FAILED_AT: &str = "license_check_failed_at";
/// Settings key: Unix ms when the gateway said the key is no longer valid.
pub const SETTING_LICENSE_ENDED_AT: &str = "license_ended_at";
/// Settings key: days per billing period (30 or 365).
pub const SETTING_LICENSE_PERIOD_DAYS: &str = "license_period_days";
/// Trial length.
pub const TRIAL_DAYS: i64 = 7;
/// Days a key subscription keeps working after a validation could not be done.
pub const GRACE_DAYS: i64 = 7;
/// Billing period assumed for a monthly key.
pub const MONTH_DAYS: i64 = 30;
/// Billing period assumed for a yearly key.
pub const YEAR_DAYS: i64 = 365;
/// Observation required before the trial can start (decision 53).
pub const MIN_OBSERVATION_MS: i64 = 24 * HOUR_MS;
/// The gateway (decision 50: Dodo Payments). Its licence endpoints are public.
pub const GATEWAY_HOST: &str = "live.dodopayments.com";
/// The same gateway in test mode, where no real money moves. Used to try the
/// whole activation flow before anyone is charged.
pub const TEST_GATEWAY_HOST: &str = "test.dodopayments.com";
/// Kept for the panel text: the host of every licence connection.
pub const ACTIVATE_HOST: &str = GATEWAY_HOST;

/// Environment variable that sends licence calls to the test gateway.
pub const TEST_GATEWAY_ENV: &str = "GUARDIANA_GATEWAY_TEST";

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
    /// The binary carries the development key: no licence file can be verified.
    DevKey,
    /// The licence file is not the expected JSON shape.
    Malformed(String),
    /// The signature does not verify against our public key.
    BadSignature,
    /// The licence has expired.
    Expired,
    /// The gateway answered that the key is not valid.
    KeyRejected(String),
    /// The network call failed.
    Network(String),
    /// The trial cannot start yet: less than 24 h observed.
    TrialTooEarly {
        /// Hours observed so far.
        horas: i64,
    },
    /// The trial was already used.
    TrialUsed,
    /// Plus is already active: nothing to try.
    AlreadyPlus,
    /// Storage failed.
    Ledger(guardiana_core::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DevKey => f.write_str("development key: licence files cannot be verified"),
            Self::Malformed(s) => write!(f, "licence file: {s}"),
            Self::BadSignature => f.write_str("licence signature does not verify"),
            Self::Expired => f.write_str("licence expired"),
            Self::KeyRejected(s) => write!(f, "key rejected: {s}"),
            Self::Network(s) => write!(f, "network: {s}"),
            Self::TrialTooEarly { horas } => write!(f, "trial too early: {horas} h observed"),
            Self::TrialUsed => f.write_str("trial already used"),
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

/// The signed part of a licence file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LicenseInfo {
    /// Licence id.
    pub id: String,
    /// Plan name (`plus`).
    pub plan: String,
    /// Holder, as written by us (name or e-mail), optional.
    #[serde(default)]
    pub titular: Option<String>,
    /// Issue date, RFC 3339.
    pub emitida: String,
    /// Expiry, Unix ms; `None` means it does not expire.
    #[serde(default)]
    pub caduca_ms: Option<i64>,
}

/// A licence file: the signed object plus its minisign signature text.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LicenseFile {
    /// The licence.
    pub licencia: serde_json::Value,
    /// Minisign signature (the two/four lines of a `.minisig`), `\n` separated.
    pub firma: String,
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
    /// Free: the complete guardian, Home Mode included. No Plus.
    Gratis,
    /// Plus trial running.
    Prueba {
        /// Trial start.
        empieza: i64,
        /// Trial end.
        termina: i64,
        /// Whole days left, at least 0.
        dias_restantes: i64,
    },
    /// Trial used up, no subscription: back to free.
    PruebaAgotada {
        /// When it ended.
        termino: i64,
    },
    /// Plus subscription active.
    Plus {
        /// `clave` or `archivo`.
        origen: String,
        /// Activation time, Unix ms.
        desde: i64,
        /// Holder if known.
        titular: Option<String>,
        /// Days per billing period for a key (30 or 365); `None` for a file.
        periodo_dias: Option<i64>,
        /// Last successful validation, Unix ms (key only).
        comprobada: Option<i64>,
        /// When the next validation is due, Unix ms (key only).
        proxima_comprobacion: Option<i64>,
        /// Hard end, Unix ms: file expiry, or end of grace for a key.
        caduca_ms: Option<i64>,
        /// State of the periodic check (key only).
        comprobacion: Option<Comprobacion>,
    },
    /// Plus ended: the subscription was cancelled, the file expired, or the
    /// key could not be checked for the whole grace period. Back to free.
    PlusTerminado {
        /// When it ended.
        termino: i64,
        /// `cancelada`, `caducada` or `sin_comprobar`.
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
    /// Home Mode is always allowed (decision 52); kept for the callers.
    pub hogar_permitido: bool,
    /// Milliseconds this installation has been observing.
    pub observado_ms: i64,
    /// Whether the user may start the trial now.
    pub puede_probar: bool,
}

/// Canonical bytes that are signed: the `licencia` object as compact JSON
/// with keys in sorted order (serde_json without `preserve_order` sorts them).
#[must_use]
pub fn canonical_bytes(licencia: &serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(licencia).unwrap_or_default()
}

/// Verify a minisign signature text over `bytes` with a public key text.
pub fn verify_signature(
    public_key_text: &str,
    signature_text: &str,
    bytes: &[u8],
) -> Result<(), Error> {
    let pk = minisign_verify::PublicKey::decode(public_key_text)
        .or_else(|_| minisign_verify::PublicKey::from_base64(public_key_text.trim()))
        .map_err(|_| Error::BadSignature)?;
    let sig =
        minisign_verify::Signature::decode(signature_text).map_err(|_| Error::BadSignature)?;
    pk.verify(bytes, &sig, true)
        .map_err(|_| Error::BadSignature)
}

/// Parse and verify a licence file against the embedded public key.
pub fn verify_license_file(text: &str, now: i64) -> Result<LicenseInfo, Error> {
    if identity::public_key_is_dev() {
        return Err(Error::DevKey);
    }
    let file: LicenseFile =
        serde_json::from_str(text).map_err(|e| Error::Malformed(e.to_string()))?;
    verify_signature(
        identity::PUBLIC_KEY_TEXT,
        &file.firma,
        &canonical_bytes(&file.licencia),
    )?;
    let info: LicenseInfo =
        serde_json::from_value(file.licencia).map_err(|e| Error::Malformed(e.to_string()))?;
    if info.caduca_ms.is_some_and(|c| c <= now) {
        return Err(Error::Expired);
    }
    Ok(info)
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
    name: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ActivationCustomer {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    email: Option<String>,
}

/// Shape of the gateway's validation answer.
#[derive(Debug, Deserialize, Default)]
struct ValidationResponse {
    #[serde(default)]
    valid: bool,
}

/// Billing period from the product name: yearly if it says so, monthly otherwise.
fn period_days_from_product(name: Option<&str>) -> i64 {
    let n = name.unwrap_or("").to_lowercase();
    if n.contains("anual") || n.contains("annual") || n.contains("year") || n.contains("año") {
        YEAR_DAYS
    } else {
        MONTH_DAYS
    }
}

fn setting_i64(ledger: &Ledger, key: &str) -> Result<Option<i64>, Error> {
    Ok(ledger.setting(key)?.and_then(|v| v.parse::<i64>().ok()))
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
    let trial_used = ledger.setting(SETTING_TRIAL_STARTED)?.is_some();
    Ok(Status {
        puede_probar: !plus_activo && !trial_used && observado_ms >= MIN_OBSERVATION_MS,
        plus_activo,
        hogar_permitido: true,
        observado_ms,
        plan,
    })
}

/// Current status from the settings. `secret` is the local integrity secret
/// (the panel token) used to mark the stored key activation.
pub fn status(ledger: &Ledger, secret: &str, now: i64) -> Result<Status, Error> {
    // 1. Signed file.
    if let Some(text) = ledger
        .setting(SETTING_LICENSE_FILE)?
        .filter(|t| !t.is_empty())
    {
        match verify_license_file(&text, now) {
            Ok(info) => {
                let plan = Plan::Plus {
                    origen: "archivo".to_owned(),
                    desde: setting_i64(ledger, SETTING_LICENSE_KEY_AT)?.unwrap_or(now),
                    titular: info.titular,
                    periodo_dias: None,
                    comprobada: None,
                    proxima_comprobacion: None,
                    caduca_ms: info.caduca_ms,
                    comprobacion: None,
                };
                return finish(ledger, plan, now);
            }
            Err(Error::Expired) => {
                // An expired file no longer counts, but a key may still be valid below.
                if ledger.setting(SETTING_LICENSE_KEY)?.is_none() {
                    let file: Option<LicenseFile> = serde_json::from_str(&text).ok();
                    let ended = file
                        .and_then(|f| serde_json::from_value::<LicenseInfo>(f.licencia).ok())
                        .and_then(|i| i.caduca_ms)
                        .unwrap_or(now);
                    return finish(
                        ledger,
                        Plan::PlusTerminado {
                            termino: ended,
                            motivo: "caducada".to_owned(),
                        },
                        now,
                    );
                }
            }
            Err(_) => {}
        }
    }
    // 2. Key subscription, stored with a local mark.
    if let (Some(key), Some(activation)) = (
        ledger
            .setting(SETTING_LICENSE_KEY)?
            .filter(|t| !t.is_empty()),
        ledger
            .setting(SETTING_LICENSE_ACTIVATION)?
            .filter(|t| !t.is_empty()),
    ) {
        let stored_mark = ledger
            .setting(SETTING_LICENSE_KEY_MARK)?
            .unwrap_or_default();
        if stored_mark == key_mark(secret, &key, &activation) {
            if let Some(ended) = setting_i64(ledger, SETTING_LICENSE_ENDED_AT)? {
                return finish(
                    ledger,
                    Plan::PlusTerminado {
                        termino: ended,
                        motivo: "cancelada".to_owned(),
                    },
                    now,
                );
            }
            let parsed: ActivationResponse = serde_json::from_str(&activation).unwrap_or_default();
            let since = setting_i64(ledger, SETTING_LICENSE_KEY_AT)?.unwrap_or(now);
            let period = setting_i64(ledger, SETTING_LICENSE_PERIOD_DAYS)?.unwrap_or(MONTH_DAYS);
            let checked = setting_i64(ledger, SETTING_LICENSE_CHECKED_AT)?.unwrap_or(since);
            let next = checked + period * DAY_MS;
            let hard_end = next + GRACE_DAYS * DAY_MS;
            if now >= hard_end {
                return finish(
                    ledger,
                    Plan::PlusTerminado {
                        termino: hard_end,
                        motivo: "sin_comprobar".to_owned(),
                    },
                    now,
                );
            }
            let failed = setting_i64(ledger, SETTING_LICENSE_CHECK_FAILED_AT)?.is_some();
            let comprobacion = if now < next {
                Comprobacion::AlDia
            } else if failed {
                Comprobacion::Fallida
            } else {
                Comprobacion::Pendiente
            };
            let plan = Plan::Plus {
                origen: "clave".to_owned(),
                desde: since,
                titular: parsed
                    .customer
                    .and_then(|c| c.email.or(c.name))
                    .filter(|s| !s.is_empty()),
                periodo_dias: Some(period),
                comprobada: Some(checked),
                proxima_comprobacion: Some(next),
                caduca_ms: Some(hard_end),
                comprobacion: Some(comprobacion),
            };
            return finish(ledger, plan, now);
        }
    }
    // 3. Trial.
    match setting_i64(ledger, SETTING_TRIAL_STARTED)? {
        Some(started) => {
            let ends = started + TRIAL_DAYS * DAY_MS;
            if now < ends {
                finish(
                    ledger,
                    Plan::Prueba {
                        empieza: started,
                        termina: ends,
                        dias_restantes: ((ends - now) + DAY_MS - 1) / DAY_MS,
                    },
                    now,
                )
            } else {
                finish(ledger, Plan::PruebaAgotada { termino: ends }, now)
            }
        }
        None => finish(ledger, Plan::Gratis, now),
    }
}

/// Start the 7-day Plus trial: only once, only without Plus, and only after
/// 24 h of observation (decision 53). Returns the new status.
pub fn start_trial(ledger: &Ledger, secret: &str, now: i64) -> Result<Status, Error> {
    let current = status(ledger, secret, now)?;
    if current.plus_activo {
        return Err(Error::AlreadyPlus);
    }
    if ledger.setting(SETTING_TRIAL_STARTED)?.is_some() {
        return Err(Error::TrialUsed);
    }
    if current.observado_ms < MIN_OBSERVATION_MS {
        return Err(Error::TrialTooEarly {
            horas: current.observado_ms / HOUR_MS,
        });
    }
    ledger.set_setting(SETTING_TRIAL_STARTED, &now.to_string())?;
    status(ledger, secret, now)
}

/// Accept a signed licence file.
pub fn activate_with_file(
    ledger: &Ledger,
    text: &str,
    secret: &str,
    now: i64,
) -> Result<Status, Error> {
    verify_license_file(text, now)?;
    ledger.set_setting(SETTING_LICENSE_FILE, text)?;
    ledger.set_setting(SETTING_LICENSE_KEY_AT, &now.to_string())?;
    status(ledger, secret, now)
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

/// Activate with a key: one call to the gateway, initiated by the user,
/// recorded in `outbound` with host and bytes whether it succeeds or not.
pub fn activate_with_key(
    ledger: &mut Ledger,
    key: &str,
    secret: &str,
    now: i64,
) -> Result<Status, Error> {
    let key = key.trim();
    let instance = format!("guardiana-{}", &mark(secret, "instance")[..8]);
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
    let period = period_days_from_product(parsed.product.as_ref().and_then(|p| p.name.as_deref()));
    ledger.set_setting(SETTING_LICENSE_KEY, key)?;
    ledger.set_setting(SETTING_LICENSE_ACTIVATION, &text)?;
    ledger.set_setting(SETTING_LICENSE_KEY_MARK, &key_mark(secret, key, &text))?;
    ledger.set_setting(SETTING_LICENSE_KEY_AT, &now.to_string())?;
    ledger.set_setting(SETTING_LICENSE_CHECKED_AT, &now.to_string())?;
    ledger.set_setting(SETTING_LICENSE_PERIOD_DAYS, &period.to_string())?;
    ledger.set_setting(SETTING_LICENSE_CHECK_FAILED_AT, "")?;
    ledger.set_setting(SETTING_LICENSE_ENDED_AT, "")?;
    status(ledger, secret, now)
}

/// Whether a periodic validation is due now.
pub fn check_due(ledger: &Ledger, secret: &str, now: i64) -> Result<bool, Error> {
    Ok(matches!(
        status(ledger, secret, now)?.plan,
        Plan::Plus {
            comprobacion: Some(Comprobacion::Pendiente | Comprobacion::Fallida),
            ..
        }
    ))
}

/// The once-per-period validation of a key subscription (decision 52). It
/// runs from the program's housekeeping when due; the user consented to it
/// when subscribing and the panel shows when the next one happens. Recorded
/// in `outbound` as not initiated by the user. Returns the status afterwards,
/// or `Ok(None)` when nothing was due.
pub fn check_if_due(ledger: &mut Ledger, secret: &str, now: i64) -> Result<Option<Status>, Error> {
    if !check_due(ledger, secret, now)? {
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
    match post_json(&validate_url(), &body) {
        Ok((code, text)) if (200..300).contains(&code) => {
            let parsed: ValidationResponse = serde_json::from_str(&text).unwrap_or_default();
            if parsed.valid {
                ledger.set_setting(SETTING_LICENSE_CHECKED_AT, &now.to_string())?;
                ledger.set_setting(SETTING_LICENSE_CHECK_FAILED_AT, "")?;
            } else {
                ledger.set_setting(SETTING_LICENSE_ENDED_AT, &now.to_string())?;
            }
        }
        Ok(_) | Err(Error::Network(_)) => {
            if setting_i64(ledger, SETTING_LICENSE_CHECK_FAILED_AT)?.is_none() {
                ledger.set_setting(SETTING_LICENSE_CHECK_FAILED_AT, &now.to_string())?;
            }
        }
        Err(e) => return Err(e),
    }
    status(ledger, secret, now).map(Some)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use guardiana_core::Hash;

    // Vector from the minisign-verify crate: key, signature over the bytes "test".
    const PK: &str = "RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";
    const SIG: &str = "untrusted comment: signature from minisign secret key\nRWQf6LRCGA9i59SLOFxz6NxvASXDJeRtuZykwQepbDEGt87ig1BNpWaVWuNrm73YiIiJbq71Wi+dP9eKL8OC351vwIasSSbXxwA=\ntrusted comment: timestamp:1555779966\tfile:test\nQtKMXWyYcwdpZAlPF7tE2ENJkRd1ujvKjlj1m9RtHTBnZPa5WKU5uWRs5GoP5M/VqE81QFuMKI5k/SfNQUaOAA==";

    fn ledger_observing() -> Ledger {
        let mut l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        // The PC has been observing since t=0.
        l.upsert_device("self", None, None, 0).unwrap();
        l
    }

    #[test]
    fn signature_verification_uses_the_embedded_key_format() {
        assert!(verify_signature(PK, SIG, b"test").is_ok());
        assert!(matches!(
            verify_signature(PK, SIG, b"Test"),
            Err(Error::BadSignature)
        ));
        let file_form = format!("untrusted comment: minisign public key\n{PK}");
        assert!(verify_signature(&file_form, SIG, b"test").is_ok());
    }

    #[test]
    fn canonical_bytes_sort_keys() {
        let v: serde_json::Value = serde_json::from_str(r#"{"plan":"plus","id":"L-1"}"#).unwrap();
        assert_eq!(canonical_bytes(&v), br#"{"id":"L-1","plan":"plus"}"#);
    }

    #[test]
    fn free_plan_allows_home_mode_and_no_plus() {
        let l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        let s = status(&l, "tok", 0).unwrap();
        assert_eq!(s.plan, Plan::Gratis);
        assert!(s.hogar_permitido);
        assert!(!s.plus_activo);
        assert!(!s.puede_probar);
    }

    #[test]
    fn trial_needs_24_hours_then_runs_seven_days_once() {
        let l = ledger_observing();
        assert!(matches!(
            start_trial(&l, "tok", 23 * HOUR_MS),
            Err(Error::TrialTooEarly { horas: 23 })
        ));
        assert!(!status(&l, "tok", 23 * HOUR_MS).unwrap().puede_probar);
        let t0 = 24 * HOUR_MS;
        assert!(status(&l, "tok", t0).unwrap().puede_probar);
        let s = start_trial(&l, "tok", t0).unwrap();
        assert!(s.plus_activo);
        assert!(matches!(
            s.plan,
            Plan::Prueba {
                dias_restantes: 7,
                ..
            }
        ));
        let s = status(&l, "tok", t0 + 6 * DAY_MS + 1).unwrap();
        assert!(matches!(
            s.plan,
            Plan::Prueba {
                dias_restantes: 1,
                ..
            }
        ));
        let s = status(&l, "tok", t0 + 7 * DAY_MS).unwrap();
        assert!(matches!(s.plan, Plan::PruebaAgotada { .. }));
        assert!(!s.plus_activo);
        assert!(s.hogar_permitido);
        assert!(!s.puede_probar);
        assert!(matches!(
            start_trial(&l, "tok", t0 + 8 * DAY_MS),
            Err(Error::TrialUsed)
        ));
    }

    fn store_key(l: &Ledger, secret: &str, at: i64, product: &str) {
        let activation = format!(
            r#"{{"id":"lki_1","product":{{"product_id":"p","name":"{product}"}},"customer":{{"customer_id":"c","name":"Ana","email":"a@b.c"}}}}"#
        );
        l.set_setting(SETTING_LICENSE_KEY, "KEY-1").unwrap();
        l.set_setting(SETTING_LICENSE_ACTIVATION, &activation)
            .unwrap();
        l.set_setting(
            SETTING_LICENSE_KEY_MARK,
            &key_mark(secret, "KEY-1", &activation),
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
        let l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        store_key(&l, "tok", 5, "GUARDIANA Plus · mensual");
        let s = status(&l, "tok", 10).unwrap();
        assert!(s.plus_activo);
        assert!(matches!(
            s.plan,
            Plan::Plus { ref origen, ref titular, periodo_dias: Some(30), comprobacion: Some(Comprobacion::AlDia), .. }
                if origen == "clave" && titular.as_deref() == Some("a@b.c")
        ));
        // A different secret invalidates the mark.
        assert_eq!(status(&l, "other", 10).unwrap().plan, Plan::Gratis);
    }

    #[test]
    fn key_subscription_is_checked_per_period_with_grace() {
        let l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        store_key(&l, "tok", 0, "GUARDIANA Plus · anual");
        let year = YEAR_DAYS * DAY_MS;
        let s = status(&l, "tok", year - 1).unwrap();
        assert!(matches!(
            s.plan,
            Plan::Plus {
                comprobacion: Some(Comprobacion::AlDia),
                periodo_dias: Some(365),
                ..
            }
        ));
        assert!(!check_due(&l, "tok", year - 1).unwrap());
        // Due: still Plus during the grace period.
        let s = status(&l, "tok", year + DAY_MS).unwrap();
        assert!(s.plus_activo);
        assert!(matches!(
            s.plan,
            Plan::Plus {
                comprobacion: Some(Comprobacion::Pendiente),
                ..
            }
        ));
        assert!(check_due(&l, "tok", year + DAY_MS).unwrap());
        // A failed attempt is shown as such, Plus still on.
        l.set_setting(
            SETTING_LICENSE_CHECK_FAILED_AT,
            &(year + DAY_MS).to_string(),
        )
        .unwrap();
        assert!(matches!(
            status(&l, "tok", year + 2 * DAY_MS).unwrap().plan,
            Plan::Plus {
                comprobacion: Some(Comprobacion::Fallida),
                ..
            }
        ));
        // After the grace period without a check: back to free, with the reason.
        let s = status(&l, "tok", year + GRACE_DAYS * DAY_MS).unwrap();
        assert!(!s.plus_activo);
        assert!(
            matches!(s.plan, Plan::PlusTerminado { ref motivo, .. } if motivo == "sin_comprobar")
        );
        // A successful check moves the next one a year ahead.
        l.set_setting(SETTING_LICENSE_CHECKED_AT, &(year + DAY_MS).to_string())
            .unwrap();
        l.set_setting(SETTING_LICENSE_CHECK_FAILED_AT, "").unwrap();
        assert!(matches!(
            status(&l, "tok", year + GRACE_DAYS * DAY_MS).unwrap().plan,
            Plan::Plus {
                comprobacion: Some(Comprobacion::AlDia),
                ..
            }
        ));
        // The gateway saying "not valid" ends Plus at once.
        l.set_setting(SETTING_LICENSE_ENDED_AT, &(year + 2 * DAY_MS).to_string())
            .unwrap();
        let s = status(&l, "tok", year + 3 * DAY_MS).unwrap();
        assert!(!s.plus_activo);
        assert!(matches!(s.plan, Plan::PlusTerminado { ref motivo, .. } if motivo == "cancelada"));
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

    #[test]
    fn dev_key_refuses_licence_files() {
        assert!(matches!(verify_license_file("{}", 0), Err(Error::DevKey)));
    }
}
