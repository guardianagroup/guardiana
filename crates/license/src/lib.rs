//! Licensing (brief §9, decisions 52 and 53).
//!
//! The free guardian needs no licence and never expires: the PC and Home
//! Mode are free. Plus is a subscription, monthly or yearly — or, for the
//! launch's limited "Fundador" licence, a single payment that never expires:
//!
//! - a 7-day trial that lives in the local settings, started by the user
//!   (from the panel or `guardiana licencia probar`) and never before 24 h of
//!   observation (decision 53); when it ends everything goes back to free,
//!   nothing is charged;
//! - a key from the gateway (Dodo Payments): one activation call when the
//!   user types the key, then one validation per billing period, recorded in
//!   `outbound` and shown in the panel; if the gateway cannot be reached the
//!   subscription stays valid for a grace period and then stops, with a
//!   notice, never breaking the DNS.
//!
//! The trial counter and the stored activation live in the local settings
//! and the interface says so: "Este contador vive en tu equipo. Reinstalar lo
//! reinicia. Confiamos en ti."

pub mod ancla;

use guardiana_core::time::DAY_MS;
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
/// Settings key: Unix ms of the key activation.
pub const SETTING_LICENSE_KEY_AT: &str = "license_key_at";
/// Settings key: Unix ms of the last successful validation.
pub const SETTING_LICENSE_CHECKED_AT: &str = "license_checked_at";
/// Settings key: Unix ms of the first failed validation attempt since the last success.
pub const SETTING_LICENSE_CHECK_FAILED_AT: &str = "license_check_failed_at";
/// Settings key: Unix ms when the gateway said the key is no longer valid.
pub const SETTING_LICENSE_ENDED_AT: &str = "license_ended_at";
/// Settings key: days per billing period (30, 365, or 0 for a licence bought once).
pub const SETTING_LICENSE_PERIOD_DAYS: &str = "license_period_days";
/// Trial length.
pub const TRIAL_DAYS: i64 = 7;
/// Days a key subscription keeps working after a validation could not be done.
pub const GRACE_DAYS: i64 = 7;
/// Billing period assumed for a monthly key.
pub const MONTH_DAYS: i64 = 30;
/// Billing period assumed for a yearly key.
pub const YEAR_DAYS: i64 = 365;
/// A licence sold once, for good: it is never checked against a clock. Stored as 0 so an older
/// installation that does not know about it falls back to the monthly rule instead of crashing.
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
        /// Whole days left, at least 0.
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
        /// When the next validation is due, Unix ms (key only); `None` for a licence bought once.
        proxima_comprobacion: Option<i64>,
        /// Hard end, Unix ms: end of the grace period; `None` when nothing ends it.
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
        /// `cancelada` or `sin_comprobar`.
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

/// Billing period from the product name: a one-off licence never expires, a yearly one is a year,
/// anything else is a month. The lifetime case is not a detail: a person who paid once for a
/// "Fundador" licence and then left the machine off for five weeks would have had Plus switched
/// off by the `sin_comprobar` rule below, which exists for subscriptions and has no business
/// touching something that was sold as permanent.
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

/// Current status from the settings. `secret` is the local integrity secret
/// (the panel token) used to mark the stored key activation.
pub fn status(ledger: &Ledger, secret: &str, now: i64) -> Result<Status, Error> {
    // 1. Key subscription, stored with a local mark.
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
            // Una licencia de por vida no caduca por no haberse podido comprobar. Se sigue
            // preguntando cuando se puede —si la pasarela dice que ya no vale, lo dice por
            // SETTING_LICENSE_ENDED_AT, que se atiende más arriba y sigue mandando—, pero un
            // equipo apagado, sin internet o detrás de un cortafuegos nunca apaga lo que se
            // vendió como permanente.
            let de_por_vida = period == DE_POR_VIDA;
            let next = if de_por_vida {
                i64::MAX
            } else {
                checked + period * DAY_MS
            };
            let hard_end = next.saturating_add(GRACE_DAYS * DAY_MS);
            if !de_por_vida && now >= hard_end {
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
                // Nada de fechas inventadas: una licencia de por vida no tiene próxima
                // comprobación ni fecha de fin, y enseñar el año 292.277.026.596 sería peor
                // que no enseñar nada.
                proxima_comprobacion: if de_por_vida { None } else { Some(next) },
                caduca_ms: if de_por_vida { None } else { Some(hard_end) },
                comprobacion: Some(comprobacion),
            };
            return finish(ledger, plan, now);
        }
    }
    // 3. Trial. The date is kept twice: in the ledger, which travels with the person's data,
    // and in a mark only an administrator can remove (`ancla`). The earlier of the two wins, so
    // deleting one does not restart the seven days and editing one does not extend them. If one
    // of the two is missing, it is written back from the other.
    let en_extracto = setting_i64(ledger, SETTING_TRIAL_STARTED)?;
    let anclado = ancla::leer();
    let empezo = match (en_extracto, anclado) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    if let Some(started) = empezo {
        if en_extracto != Some(started) {
            ledger.set_setting(SETTING_TRIAL_STARTED, &started.to_string())?;
        }
        if anclado != Some(started) {
            ancla::escribir(started);
        }
    }
    match empezo {
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
        None => {
            // There is no free plan to wait in: the seven days start the first time the program
            // runs, and the first run is this call. Writing it here and not in the engine means
            // the clock is the same however Guardiana was started (service, terminal or panel).
            ledger.set_setting(SETTING_TRIAL_STARTED, &now.to_string())?;
            ancla::escribir(now);
            finish(
                ledger,
                Plan::Prueba {
                    empieza: now,
                    termina: now + TRIAL_DAYS * DAY_MS,
                    dias_restantes: TRIAL_DAYS,
                },
                now,
            )
        }
    }
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

    /// Cada prueba que llama a `status` toma el cerrojo del módulo `ancla` y trabaja en su propia
    /// carpeta: así ninguna escribe la marca de verdad de esta máquina. La primera versión de
    /// esto no lo hacía y una prueba dejó un archivo con fecha 0 en la carpeta de datos del Mac
    /// del responsable, que con el código nuevo habría dado la prueba por caducada.
    fn a_solas() -> std::sync::MutexGuard<'static, ()> {
        let dir = std::env::temp_dir().join(format!(
            "guardiana-lic-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        ancla::a_solas_en(&dir)
    }

    fn ledger_observing() -> Ledger {
        let mut l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        // The PC has been observing since t=0.
        l.upsert_device("self", None, None, 0).unwrap();
        l
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
        let _a_solas = a_solas();
        let l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        store_key(&l, "tok", 5, "GUARDIANA Plus · mensual");
        let s = status(&l, "tok", 10).unwrap();
        assert!(s.plus_activo);
        assert!(matches!(
            s.plan,
            Plan::Plus { ref origen, ref titular, periodo_dias: Some(30), comprobacion: Some(Comprobacion::AlDia), .. }
                if origen == "clave" && titular.as_deref() == Some("a@b.c")
        ));
        // A different secret invalidates the mark: the key no longer counts, and what is left
        // is the trial this installation started on its first run.
        assert!(matches!(
            status(&l, "other", 10).unwrap().plan,
            Plan::Prueba { .. }
        ));
    }

    #[test]
    fn key_subscription_is_checked_per_period_with_grace() {
        let _a_solas = a_solas();
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

    #[test]
    fn una_licencia_de_por_vida_no_caduca_por_no_comprobarse() {
        let _a_solas = a_solas();
        // El caso que habría roto al primer Fundador: paga una vez, apaga el equipo cinco
        // semanas y al volver se encuentra el Plus apagado por la regla de las suscripciones.
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
        // Diez años después, sin una sola comprobación con éxito.
        let diez_anos = 3650 * DAY_MS;
        let s = status(&l, "tok", diez_anos).unwrap();
        assert!(
            s.plus_activo,
            "una licencia comprada una vez no caduca sola"
        );
        match s.plan {
            Plan::Plus {
                proxima_comprobacion,
                caduca_ms,
                periodo_dias,
                ..
            } => {
                assert_eq!(periodo_dias, Some(DE_POR_VIDA));
                assert_eq!(proxima_comprobacion, None, "no hay próxima comprobación");
                assert_eq!(caduca_ms, None, "no hay fecha de fin");
            }
            otro => unreachable!("esperaba Plus, llegó {otro:?}"),
        }
        // Pero si la pasarela dice que ya no vale (devolución, por ejemplo), termina igual.
        l.set_setting(SETTING_LICENSE_ENDED_AT, &(2 * DAY_MS).to_string())
            .unwrap();
        let s2 = status(&l, "tok", diez_anos).unwrap();
        assert!(!s2.plus_activo);
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
}
