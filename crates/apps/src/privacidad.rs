//! «Quién vende tus datos dentro de tu PC» (1.0.46): three Windows settings that hand the
//! person's use to advertising, read and switched in the documented place, and the one Mac
//! setting that does the same, read only.
//!
//! - `publicidad`: the advertising ID (`AdvertisingInfo\Enabled`, per account): apps use it to
//!   show ads «relevant to you» across apps.
//! - `experiencias`: tailored experiences (`Privacy\TailoredExperiencesWithDiagnosticDataEnabled`,
//!   per account): Microsoft uses the diagnostic data for tips, ads and recommendations.
//! - `diagnostico`: optional diagnostic data (`DataCollection\AllowTelemetry`, the computer): 3
//!   sends the optional data, 1 only the required data.
//!
//! The service runs as the system account, so «the account» is every person's account of this
//! computer (`HKEY_USERS\S-1-5-21-…`): the switch changes it for all of them, and the page says
//! so. A value an organisation set by policy is never touched; it is said. Written with
//! `reg.exe add`, the same documented values Windows' own Settings app writes.
//!
//! On a Mac, «Personalized Ads» (`com.apple.AdLib`, `allowApplePersonalizedAdvertising`) is read
//! for each account; changing it is left to System Settings, which the page explains.

use super::Error;

/// One setting, as this computer has it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AjustePrivacidad {
    /// `publicidad`, `experiencias`, `diagnostico` (Windows) or `anuncios_apple` (Mac).
    pub clave: &'static str,
    /// On (it hands data over) for at least one account; `None` when the system does not say.
    pub activo: Option<bool>,
    /// An organisation fixed it by policy: GUARDIANA does not change it.
    pub por_politica: bool,
    /// GUARDIANA can switch it from here.
    pub cambiable: bool,
    /// Accounts of this computer it was read for (per-account settings), or 0.
    pub cuentas: u32,
}

/// The settings of this system. Empty where there are none to show.
#[must_use]
pub fn ajustes() -> Vec<AjustePrivacidad> {
    #[cfg(windows)]
    {
        windows::ajustes()
    }
    #[cfg(target_os = "macos")]
    {
        mac::ajustes()
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Vec::new()
    }
}

/// Switch one setting on or off (Windows). A setting fixed by policy is refused.
///
/// # Errors
/// [`Error::NoSoportado`] off Windows or for an unknown key; [`Error::Sistema`] with what
/// `reg.exe` said when it refused.
pub fn cambia(clave: &str, activo: bool) -> Result<(), Error> {
    #[cfg(windows)]
    {
        windows::cambia(clave, activo)
    }
    #[cfg(not(windows))]
    {
        let _ = (clave, activo);
        Err(Error::NoSoportado)
    }
}

/// The accounts of `reg.exe query HKU`: the people's own (`S-1-5-21-…`), not their classes.
#[must_use]
pub fn cuentas_de_hku(salida: &str) -> Vec<String> {
    salida
        .lines()
        .filter_map(|l| l.trim().strip_prefix("HKEY_USERS\\"))
        .filter(|s| {
            s.starts_with("S-1-5-21-")
                && !s.ends_with("_Classes")
                && s.chars()
                    .all(|c| c.is_ascii_digit() || c == '-' || c == 'S')
        })
        .map(str::to_owned)
        .collect()
}

/// How a setting reads from its per-account values: on when any account has it on, off when
/// every account that says has it off, unknown when none says.
#[must_use]
pub fn activo_de_cuentas(valores: &[Option<u32>]) -> Option<bool> {
    if valores.iter().any(|v| v.is_some_and(|n| n != 0)) {
        Some(true)
    } else if valores.iter().any(Option::is_some) {
        Some(false)
    } else {
        None
    }
}

#[cfg(windows)]
mod windows {
    use std::os::windows::process::CommandExt;

    use super::{activo_de_cuentas, cuentas_de_hku, AjustePrivacidad, Error};
    use crate::lee_dword_de_reg;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const PUBLICIDAD: (&str, &str) = (
        r"Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo",
        "Enabled",
    );
    const PUBLICIDAD_POLITICA: (&str, &str) = (
        r"HKLM\SOFTWARE\Policies\Microsoft\Windows\AdvertisingInfo",
        "DisabledByGroupPolicy",
    );
    const EXPERIENCIAS: (&str, &str) = (
        r"Software\Microsoft\Windows\CurrentVersion\Privacy",
        "TailoredExperiencesWithDiagnosticDataEnabled",
    );
    const EXPERIENCIAS_POLITICA: (&str, &str) = (
        r"HKLM\SOFTWARE\Policies\Microsoft\Windows\CloudContent",
        "DisableTailoredExperiencesWithDiagnosticData",
    );
    const DIAGNOSTICO: (&str, &str) = (
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\DataCollection",
        "AllowTelemetry",
    );
    const DIAGNOSTICO_POLITICA: (&str, &str) = (
        r"HKLM\SOFTWARE\Policies\Microsoft\Windows\DataCollection",
        "AllowTelemetry",
    );

    fn reg(args: &[&str]) -> Option<std::process::Output> {
        let raiz = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_owned());
        std::process::Command::new(format!("{raiz}\\System32\\reg.exe"))
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()
    }

    fn valor(clave: &str, nombre: &str) -> Option<u32> {
        let s = reg(&["query", clave, "/v", nombre])?;
        if !s.status.success() {
            return None;
        }
        lee_dword_de_reg(&String::from_utf8_lossy(&s.stdout), nombre)
    }

    fn cuentas() -> Vec<String> {
        reg(&["query", "HKU"])
            .filter(|s| s.status.success())
            .map(|s| cuentas_de_hku(&String::from_utf8_lossy(&s.stdout)))
            .unwrap_or_default()
    }

    fn por_cuenta(
        clave: &'static str,
        (ruta, nombre): (&str, &str),
        (pruta, pnombre): (&str, &str),
        cuentas: &[String],
    ) -> AjustePrivacidad {
        let politica = valor(pruta, pnombre);
        let valores: Vec<Option<u32>> = cuentas
            .iter()
            .map(|sid| valor(&format!(r"HKU\{sid}\{ruta}"), nombre))
            .collect();
        AjustePrivacidad {
            clave,
            // A policy that turns it off wins over the accounts.
            activo: if politica == Some(1) {
                Some(false)
            } else {
                activo_de_cuentas(&valores)
            },
            por_politica: politica.is_some(),
            cambiable: politica.is_none() && !cuentas.is_empty(),
            cuentas: u32::try_from(cuentas.len()).unwrap_or(u32::MAX),
        }
    }

    pub(super) fn ajustes() -> Vec<AjustePrivacidad> {
        let cs = cuentas();
        let politica = valor(DIAGNOSTICO_POLITICA.0, DIAGNOSTICO_POLITICA.1);
        let propio = valor(DIAGNOSTICO.0, DIAGNOSTICO.1);
        vec![
            por_cuenta("publicidad", PUBLICIDAD, PUBLICIDAD_POLITICA, &cs),
            por_cuenta("experiencias", EXPERIENCIAS, EXPERIENCIAS_POLITICA, &cs),
            AjustePrivacidad {
                clave: "diagnostico",
                // 3 (and the older 2) send the optional data; 0 and 1 only what is required.
                activo: politica.or(propio).map(|v| v >= 2),
                por_politica: politica.is_some(),
                cambiable: politica.is_none(),
                cuentas: 0,
            },
        ]
    }

    fn escribe(clave: &str, nombre: &str, dato: u32) -> Result<(), Error> {
        let d = dato.to_string();
        let s = reg(&[
            "add",
            clave,
            "/v",
            nombre,
            "/t",
            "REG_DWORD",
            "/d",
            &d,
            "/f",
        ])
        .ok_or_else(|| Error::Sistema("reg.exe no se pudo abrir".to_owned()))?;
        if s.status.success() {
            Ok(())
        } else {
            Err(Error::Sistema(
                String::from_utf8_lossy(&s.stderr).trim().to_owned(),
            ))
        }
    }

    pub(super) fn cambia(clave: &str, activo: bool) -> Result<(), Error> {
        let (ruta, nombre, politica) = match clave {
            "publicidad" => (PUBLICIDAD.0, PUBLICIDAD.1, PUBLICIDAD_POLITICA),
            "experiencias" => (EXPERIENCIAS.0, EXPERIENCIAS.1, EXPERIENCIAS_POLITICA),
            "diagnostico" => {
                if valor(DIAGNOSTICO_POLITICA.0, DIAGNOSTICO_POLITICA.1).is_some() {
                    return Err(Error::Sistema("politica".to_owned()));
                }
                return escribe(DIAGNOSTICO.0, DIAGNOSTICO.1, if activo { 3 } else { 1 });
            }
            _ => return Err(Error::NoSoportado),
        };
        if valor(politica.0, politica.1).is_some() {
            return Err(Error::Sistema("politica".to_owned()));
        }
        let cs = cuentas();
        if cs.is_empty() {
            return Err(Error::Sistema("sin cuentas".to_owned()));
        }
        for sid in cs {
            escribe(&format!(r"HKU\{sid}\{ruta}"), nombre, u32::from(activo))?;
        }
        Ok(())
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use super::{activo_de_cuentas, AjustePrivacidad};

    pub(super) fn ajustes() -> Vec<AjustePrivacidad> {
        let mut valores = Vec::new();
        if let Ok(rd) = std::fs::read_dir("/Users") {
            for e in rd.flatten() {
                let nombre = e.file_name().to_string_lossy().into_owned();
                if nombre.starts_with('.') || nombre == "Shared" || nombre == "Guest" {
                    continue;
                }
                let pref = e.path().join("Library/Preferences/com.apple.AdLib");
                let leido = std::process::Command::new("/usr/bin/defaults")
                    .arg("read")
                    .arg(&pref)
                    .arg("allowApplePersonalizedAdvertising")
                    .output()
                    .ok()
                    .filter(|s| s.status.success())
                    .and_then(|s| {
                        String::from_utf8_lossy(&s.stdout)
                            .trim()
                            .parse::<u32>()
                            .ok()
                    });
                valores.push(leido);
            }
        }
        vec![AjustePrivacidad {
            clave: "anuncios_apple",
            activo: activo_de_cuentas(&valores),
            por_politica: false,
            cambiable: false,
            cuentas: u32::try_from(valores.len()).unwrap_or(u32::MAX),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::{activo_de_cuentas, cuentas_de_hku};

    #[test]
    fn only_peoples_accounts_are_taken_from_hku() {
        let salida = "\r\nHKEY_USERS\\.DEFAULT\r\nHKEY_USERS\\S-1-5-19\r\nHKEY_USERS\\S-1-5-21-111-222-333-1001\r\nHKEY_USERS\\S-1-5-21-111-222-333-1001_Classes\r\nHKEY_USERS\\S-1-5-18\r\n";
        assert_eq!(
            cuentas_de_hku(salida),
            vec!["S-1-5-21-111-222-333-1001".to_owned()]
        );
    }

    #[test]
    fn a_setting_is_on_when_any_account_has_it_on() {
        assert_eq!(activo_de_cuentas(&[Some(0), Some(1)]), Some(true));
        assert_eq!(activo_de_cuentas(&[Some(0), None]), Some(false));
        assert_eq!(activo_de_cuentas(&[None, None]), None);
        assert_eq!(activo_de_cuentas(&[]), None);
    }
}
