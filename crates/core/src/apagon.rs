//! «Modo apagón» (customer request, 18 Oct 2026): one switch that cuts every name this computer
//! asks for, except a short list of what the system needs to stay healthy and what the person
//! approves one by one. Clearly reversible: the same switch turns it off, and nothing else
//! changes (rules, scopes and cuts the person had stay as they were).
//!
//! What it never cuts, and why:
//!
//! - Windows Update and Microsoft's security definitions: a computer cut off from them stops
//!   receiving fixes, which is the opposite of protecting it.
//! - The time: certificates and logins fail with the clock wrong.
//! - Certificate checks (revocation lists, OCSP): without them updates and signed programs fail.
//! - Windows' connectivity check: without it Windows says «sin internet» and some updates wait.
//! - GUARDIANA itself (its site and its licence check) and the names GUARDIANA answers alone.
//! - The local network (`.local`, `.arpa`, `localhost`, `.lan`): those never leave the house.
//!
//! The panel cannot be locked out by it: it is served on `127.0.0.1`, which no name resolves.
//! Only this computer's own queries are cut; a phone in Home Mode is not this switch's business.
//!
//! The switch is a setting; each time it is turned on or off an event goes into the chained
//! ledger ([`evento_cambio`]), and every name it cuts is written there with
//! `decided_by = modo_apagon`, so the extract says what was cut by it and since when.

use serde::{Deserialize, Serialize};

use crate::model::{Category, DecidedBy, NewEvent, Verdict};
use crate::rules::normalizar_nombre;

/// Settings key: the switch and the person's approvals, as JSON ([`Apagon`]).
pub const AJUSTE_APAGON: &str = "modo_apagon";

/// The name the on/off events carry in the ledger. Under `guardiana.hogar`, which nobody but
/// GUARDIANA answers, so it can never be mistaken for a name a program asked for.
pub const NOMBRE_EVENTO: &str = "modo-apagon.guardiana.hogar";

/// The most names a person may approve. A list longer than this is no longer an apagón.
pub const MAX_APROBADOS: usize = 200;

/// Why a name on the essential list is there (a text key for the panel).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Para {
    /// Windows Update and Microsoft's security definitions.
    Actualizaciones,
    /// The clock.
    Hora,
    /// Certificate revocation checks.
    Certificados,
    /// Windows' connectivity check.
    Conectividad,
    /// GUARDIANA itself.
    Guardiana,
    /// The local network.
    RedLocal,
}

/// One name the switch never cuts, with everything under it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Esencial {
    /// The domain.
    pub dominio: &'static str,
    /// Why.
    pub para: Para,
}

const fn e(dominio: &'static str, para: Para) -> Esencial {
    Esencial { dominio, para }
}

/// What the switch never cuts. Each one covers itself and every name under it. Kept short on
/// purpose: every line is a door left open, and it is shown whole on the panel.
pub const ESENCIALES: &[Esencial] = &[
    e("windowsupdate.com", Para::Actualizaciones),
    e("update.microsoft.com", Para::Actualizaciones),
    e("windowsupdate.microsoft.com", Para::Actualizaciones),
    e("delivery.mp.microsoft.com", Para::Actualizaciones),
    e("prod.do.dsp.mp.microsoft.com", Para::Actualizaciones),
    e("wustat.windows.com", Para::Actualizaciones),
    e("ntservicepack.microsoft.com", Para::Actualizaciones),
    e("definitionupdates.microsoft.com", Para::Actualizaciones),
    e("time.windows.com", Para::Hora),
    e("pool.ntp.org", Para::Hora),
    e("time.nist.gov", Para::Hora),
    e("crl.microsoft.com", Para::Certificados),
    e("oneocsp.microsoft.com", Para::Certificados),
    e("ocsp.msocsp.com", Para::Certificados),
    e("ocsp.digicert.com", Para::Certificados),
    e("crl3.digicert.com", Para::Certificados),
    e("crl4.digicert.com", Para::Certificados),
    e("msftconnecttest.com", Para::Conectividad),
    e("msftncsi.com", Para::Conectividad),
    e("guardianagroup.com", Para::Guardiana),
    // The licence check (crates/license, `GATEWAY_HOST`).
    e("live.dodopayments.com", Para::Guardiana),
    e("guardiana.hogar", Para::Guardiana),
    e("local", Para::RedLocal),
    e("arpa", Para::RedLocal),
    e("lan", Para::RedLocal),
    e("localhost", Para::RedLocal),
];

/// `name` is `dominio` or under it.
fn bajo(name: &str, dominio: &str) -> bool {
    name == dominio || name.strip_suffix(dominio).is_some_and(|r| r.ends_with('.'))
}

/// The essential entry that lets `name` through, if one does.
#[must_use]
pub fn esencial(name: &str) -> Option<&'static Esencial> {
    let n = name.trim_end_matches('.').to_ascii_lowercase();
    ESENCIALES.iter().find(|e| bajo(&n, e.dominio))
}

/// The switch and what the person approved.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Apagon {
    /// On.
    pub activo: bool,
    /// Since when, Unix ms, while on.
    #[serde(default)]
    pub desde: Option<i64>,
    /// Names the person let through, each with everything under it. Kept when the switch goes
    /// off, so the next apagón starts with them.
    #[serde(default)]
    pub aprobados: Vec<String>,
}

impl Apagon {
    /// Read from its setting; anything unreadable is «off».
    #[must_use]
    pub fn de_ajuste(valor: Option<&str>) -> Self {
        valor
            .and_then(|v| serde_json::from_str(v).ok())
            .unwrap_or_default()
    }

    /// As its setting is written.
    #[must_use]
    pub fn a_ajuste(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Whether this computer's query for `name` is cut by the switch.
    #[must_use]
    pub fn corta(&self, name: &str) -> bool {
        if !self.activo {
            return false;
        }
        let n = name.trim_end_matches('.').to_ascii_lowercase();
        if n.is_empty() || esencial(&n).is_some() {
            return false;
        }
        !self.aprobados.iter().any(|a| bajo(&n, a))
    }

    /// Let `raw` through (as people write it: `https://www.x.com/y` is `www.x.com`). Returns the
    /// name kept, or `None` when it is not a name or the list is full.
    pub fn aprueba(&mut self, raw: &str) -> Option<String> {
        let (n, _) = normalizar_nombre(raw)?;
        if !self.aprobados.contains(&n) {
            if self.aprobados.len() >= MAX_APROBADOS {
                return None;
            }
            self.aprobados.push(n.clone());
            self.aprobados.sort();
        }
        Some(n)
    }

    /// Take `name` off the approved list. Whether it was there.
    pub fn quita(&mut self, name: &str) -> bool {
        let n = name.trim().trim_end_matches('.').to_ascii_lowercase();
        let antes = self.aprobados.len();
        self.aprobados.retain(|a| *a != n);
        self.aprobados.len() != antes
    }

    /// Turn it on or off at `ahora`. Whether anything changed.
    pub fn cambia(&mut self, activo: bool, ahora: i64) -> bool {
        if self.activo == activo {
            return false;
        }
        self.activo = activo;
        self.desde = activo.then_some(ahora);
        true
    }
}

/// The ledger event that says the switch went on or off, from this computer, by the person.
#[must_use]
pub fn evento_cambio(activo: bool, ts: i64) -> NewEvent {
    let mut e = NewEvent::observed(
        ts,
        crate::SELF_DEVICE_ID,
        "127.0.0.1",
        NOMBRE_EVENTO,
        "GUARDIANA",
    );
    e.category = Category::Esperado;
    e.verdict = Verdict::Respondido;
    e.decided_by = DecidedBy::Usuario;
    e.list_source = if activo {
        "modo_apagon:encendido"
    } else {
        "modo_apagon:apagado"
    }
    .to_owned();
    e
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn encendido() -> Apagon {
        let mut a = Apagon::default();
        assert!(a.cambia(true, 1_000));
        a
    }

    #[test]
    fn off_cuts_nothing() {
        let a = Apagon::default();
        assert!(!a.corta("tracker.example.com"));
    }

    #[test]
    fn on_cuts_everything_outside_the_essentials_and_the_approvals() {
        let a = encendido();
        assert!(a.corta("tracker.example.com"));
        assert!(a.corta("www.google.com"));
        assert!(
            a.corta("telemetry.microsoft.com"),
            "Microsoft is not all essential"
        );
        assert!(
            a.corta("evilwindowsupdate.com"),
            "a suffix is a whole label"
        );
        assert!(a.corta("windowsupdate.com.evil.net"));
        assert!(a.corta("wpad"), "a bare name goes to the upstream too");
    }

    #[test]
    fn the_system_essentials_and_guardiana_always_pass() {
        let a = encendido();
        for n in [
            "fe2.update.microsoft.com",
            "download.windowsupdate.com",
            "ctldl.windowsupdate.com.",
            "TIME.WINDOWS.COM",
            "0.pool.ntp.org",
            "www.msftconnecttest.com",
            "oneocsp.microsoft.com",
            "guardianagroup.com",
            "www.guardianagroup.com",
            "live.dodopayments.com",
            "prueba.guardiana.hogar",
            "comprobar.guardiana.hogar",
            "impresora.local",
            "1.0.168.192.in-addr.arpa",
            "localhost",
        ] {
            assert!(!a.corta(n), "{n} must pass");
            assert!(esencial(n).is_some(), "{n}");
        }
    }

    #[test]
    fn the_checker_and_self_check_names_are_essential() {
        // The panel's own names (crates/panel CHECKER_HOST and SELF_CHECK_SUFFIX) must never be
        // cut by the switch, or the panel would say GUARDIANA is not filtering.
        assert!(esencial(crate::SELF_CHECK_SUFFIX).is_some());
        assert!(esencial("comprobar.guardiana.hogar").is_some());
        assert!(esencial(NOMBRE_EVENTO).is_some());
    }

    #[test]
    fn approving_one_by_one_lets_that_name_and_what_hangs_from_it_through() {
        let mut a = encendido();
        assert_eq!(
            a.aprueba("https://www.Banco.com/login").as_deref(),
            Some("www.banco.com")
        );
        assert!(!a.corta("www.banco.com"));
        assert!(!a.corta("api.www.banco.com"));
        assert!(a.corta("banco.com"), "only what was approved");
        assert_eq!(a.aprueba("*.correo.org").as_deref(), Some("correo.org"));
        assert!(!a.corta("imap.correo.org"));
        assert!(a.aprueba("no es un nombre").is_none());
        assert!(a.quita("www.banco.com"));
        assert!(a.corta("www.banco.com"));
        assert!(!a.quita("www.banco.com"));
    }

    #[test]
    fn the_approved_list_has_a_ceiling() {
        let mut a = encendido();
        for i in 0..MAX_APROBADOS {
            assert!(a.aprueba(&format!("n{i}.example.com")).is_some());
        }
        assert!(a.aprueba("uno-mas.example.com").is_none());
        // Approving one already there is not a new one.
        assert!(a.aprueba("n0.example.com").is_some());
    }

    #[test]
    fn it_is_reversible_and_keeps_the_approvals() {
        let mut a = encendido();
        a.aprueba("www.banco.com");
        assert_eq!(a.desde, Some(1_000));
        assert!(!a.cambia(true, 2_000), "already on");
        assert!(a.cambia(false, 3_000));
        assert_eq!(a.desde, None);
        assert!(!a.corta("tracker.example.com"));
        assert_eq!(a.aprobados, vec!["www.banco.com".to_owned()]);
        let leido = Apagon::de_ajuste(Some(&a.a_ajuste()));
        assert_eq!(leido, a);
    }

    #[test]
    fn an_unreadable_setting_is_off_never_on() {
        assert!(!Apagon::de_ajuste(Some("{roto")).activo);
        assert!(!Apagon::de_ajuste(None).activo);
        assert!(!Apagon::de_ajuste(Some("{\"activo\":\"si\"}")).activo);
    }

    #[test]
    fn the_on_and_off_events_say_what_happened() {
        let on = evento_cambio(true, 5);
        assert_eq!(on.qname, NOMBRE_EVENTO);
        assert_eq!(on.list_source, "modo_apagon:encendido");
        assert_eq!(on.decided_by, DecidedBy::Usuario);
        assert_eq!(evento_cambio(false, 6).list_source, "modo_apagon:apagado");
    }
}
