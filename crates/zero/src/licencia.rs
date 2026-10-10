//! The subscription. GUARDIANA ZERO comes with the GUARDIANA subscription and has seven free days
//! of its own (decided by the owner on 9 Oct 2026): the same key opens both programs, and trying
//! GUARDIANA first does not spend the browser's week, nor the other way round.
//!
//! The rules are GUARDIANA's own (`guardiana-license`): the seven days start the first time the
//! browser runs and are kept twice, a clock that only moves forward, one activation call when the
//! person types the key, a check eight days later and then once per billing period, seven days of
//! grace when the gateway cannot be reached, and an end only when the gateway itself says the key
//! is no longer valid. Every one of those connections is written down and shown in the panel.
//!
//! What changes here is where the marks live. The browser runs as the person, without
//! administrator rights, so its two copies of the trial date go into two folders of the person's
//! own: the browser's data folder and a second one outside it (the shell picks both). Deleting
//! both starts the week again; the panel says where they are, as GUARDIANA's does.
//!
//! When neither the trial nor a subscription is on, the browser keeps opening pages, but it does
//! nothing of its own: it cuts nothing, cleans no address, does not watch marked data and makes
//! no mandates. It never locks the person out of their tabs (the owner's choice, same day).

use std::path::{Path, PathBuf};

use guardiana_core::{identity, Ledger, Purpose};
use guardiana_license as lic;

/// The public ledger, where every published version is written before its download exists.
pub const REGISTRO_PUBLICO: &str =
    "https://raw.githubusercontent.com/guardianagroup/guardiana/main/ledger.jsonl";
const REGISTRO_HOST: &str = "raw.githubusercontent.com";

/// The last GUARDIANA ZERO version in the public ledger's text (lines `"version":"zero-<v>"`).
#[must_use]
pub fn ultima_zero(texto: &str) -> Option<String> {
    texto
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l.trim()).ok())
        .filter_map(|v| {
            v.get("version")
                .and_then(serde_json::Value::as_str)
                .and_then(|s| s.strip_prefix("zero-"))
                .map(str::to_string)
        })
        .max_by(|a, b| compara_versiones(a, b))
}

/// `1.0.10` after `1.0.9`: compared number by number.
#[must_use]
pub fn compara_versiones(a: &str, b: &str) -> std::cmp::Ordering {
    let partes = |s: &str| -> Vec<u64> { s.split('.').map(|x| x.parse().unwrap_or(0)).collect() };
    partes(a).cmp(&partes(b))
}

/// The licence's own small ledger, inside the browser's data folder.
pub const ARCHIVO: &str = "licencia.db";

/// Where the subscription stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Estado {
    /// Not read yet, or unreadable (a locked file): the browser works, and the next reading says.
    Desconocido,
    /// The seven free days are running.
    Prueba {
        /// When they end, Unix ms.
        termina: i64,
        /// Whole days left, 0 to 7.
        dias: i64,
    },
    /// A subscription key is active.
    Suscrita {
        /// Activation, Unix ms.
        desde: i64,
        /// Days per billing period (30, 365, 0 for a licence paid once), if known.
        periodo: Option<i64>,
        /// Next check, Unix ms.
        proxima: Option<i64>,
        /// End of the grace that a failed check started, Unix ms.
        caduca: Option<i64>,
        /// The last check could not be done.
        fallida: bool,
    },
    /// The seven days are over and there is no subscription.
    PruebaTerminada {
        /// When they ended.
        desde: i64,
    },
    /// The subscription ended (cancelled, refused, or not checked for the whole grace).
    SuscripcionTerminada {
        /// When.
        desde: i64,
        /// `cancelada`, `sin_comprobar` or `rechazada`.
        motivo: String,
    },
}

impl Estado {
    /// Whether the browser does its own work: cutting, cleaning, the form guard, mandates.
    #[must_use]
    pub const fn protege(&self) -> bool {
        !matches!(
            self,
            Self::PruebaTerminada { .. } | Self::SuscripcionTerminada { .. }
        )
    }

    /// From GUARDIANA's plan.
    #[must_use]
    pub fn de_plan(p: &lic::Plan) -> Self {
        match p {
            lic::Plan::Prueba {
                termina,
                dias_restantes,
                ..
            } => Self::Prueba {
                termina: *termina,
                dias: *dias_restantes,
            },
            lic::Plan::PruebaAgotada { termino } => Self::PruebaTerminada { desde: *termino },
            lic::Plan::Plus {
                desde,
                periodo_dias,
                proxima_comprobacion,
                caduca_ms,
                comprobacion,
                ..
            } => Self::Suscrita {
                desde: *desde,
                periodo: *periodo_dias,
                proxima: *proxima_comprobacion,
                caduca: *caduca_ms,
                fallida: *comprobacion == Some(lic::Comprobacion::Fallida),
            },
            lic::Plan::PlusTerminado { termino, motivo } => Self::SuscripcionTerminada {
                desde: *termino,
                motivo: motivo.clone(),
            },
        }
    }

    /// Its name in the messages to the panel and the bar.
    #[must_use]
    pub const fn nombre(&self) -> &'static str {
        match self {
            Self::Desconocido => "desconocido",
            Self::Prueba { .. } => "prueba",
            Self::Suscrita { .. } => "suscrita",
            Self::PruebaTerminada { .. } => "prueba_terminada",
            Self::SuscripcionTerminada { .. } => "suscripcion_terminada",
        }
    }
}

/// Why a key was not activated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fallo {
    /// Nothing typed: nothing was sent.
    Vacia,
    /// The gateway does not know the key, or turned it down.
    Rechazada(String),
    /// The gateway knows the key but it is not active (no first payment, or cancelled).
    Inactiva,
    /// The key has used every activation it allows.
    Limite,
    /// No answer from the gateway.
    Red(String),
    /// An answer that is not the gateway's.
    Formato,
    /// The browser's own file could not be written.
    Disco,
}

impl Fallo {
    fn de(e: lic::Error) -> Self {
        match e {
            lic::Error::EmptyKey => Self::Vacia,
            lic::Error::KeyRejected(s) => Self::Rechazada(s),
            lic::Error::KeyInactive => Self::Inactiva,
            lic::Error::ActivationLimit => Self::Limite,
            lic::Error::Network(s) => Self::Red(s),
            lic::Error::Malformed(_) => Self::Formato,
            // Not a failure: the key is already active here, and the status says so.
            lic::Error::AlreadyPlus => Self::Formato,
            lic::Error::Ledger(_) => Self::Disco,
        }
    }

    /// The text key that explains it, and its `{motivo}` if any.
    #[must_use]
    pub fn clave(&self) -> (&'static str, &str) {
        match self {
            Self::Vacia => ("licencia_err_vacia", ""),
            Self::Rechazada(m) => ("licencia_err_rechazada", m),
            Self::Inactiva => ("licencia_err_inactiva", ""),
            Self::Limite => ("licencia_err_limite", ""),
            Self::Red(m) => ("licencia_err_red", m),
            Self::Formato => ("licencia_err_formato", ""),
            Self::Disco => ("licencia_err_disco", ""),
        }
    }
}

/// One connection the licence made, for the panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conexion {
    /// When, Unix ms.
    pub ms: i64,
    /// To whom.
    pub host: String,
    /// Asked for by the person (an activation, a version check) or due (a periodic check).
    pub pedida: bool,
    /// For the licence, or to see whether there is a newer version.
    pub version: bool,
}

/// Where the licence lives, and the calls that read or change it. Cheap to clone: the shell keeps
/// one for its worker thread, the session another.
#[derive(Debug, Clone)]
pub struct Lugar {
    db: PathBuf,
    ancla: PathBuf,
}

impl Lugar {
    /// The licence of the browser whose data folder is `datos`, with its second mark in `ancla`
    /// (a folder outside `datos`). Sets the process's mark folder: one per process.
    #[must_use]
    pub fn nuevo(datos: &Path, ancla: PathBuf) -> Self {
        lic::ancla::usar_carpeta(Some(ancla.clone()));
        Self {
            db: datos.join(ARCHIVO),
            ancla,
        }
    }

    fn libro(&self) -> Result<Ledger, Fallo> {
        if let Some(d) = self.db.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::create_dir_all(&self.ancla);
        Ledger::open(&self.db, identity::genesis()).map_err(|_| Fallo::Disco)
    }

    /// Where things stand now. No connection: only the two marks and the clock. The first call
    /// ever starts the seven days. `None` when the file cannot be read right now.
    #[must_use]
    pub fn estado(&self, ahora: i64) -> Option<Estado> {
        let l = self.libro().ok()?;
        lic::status(&l, "", ahora)
            .ok()
            .map(|s| Estado::de_plan(&s.plan))
    }

    /// Activate a key: one connection to the gateway, asked for by the person.
    ///
    /// # Errors
    /// What the gateway answered, or that it could not be asked.
    pub fn activar(&self, clave: &str, ahora: i64) -> Result<Estado, Fallo> {
        let mut l = self.libro()?;
        lic::activate_with_key(&mut l, clave, "", ahora)
            .map(|s| Estado::de_plan(&s.plan))
            .map_err(Fallo::de)
    }

    /// The periodic check, when it is due and at most once an hour. `None` when nothing was due
    /// (or the file could not be opened).
    #[must_use]
    pub fn comprobar(&self, ahora: i64) -> Option<Estado> {
        let mut l = self.libro().ok()?;
        lic::check_if_due_hourly(&mut l, "", ahora)
            .ok()
            .flatten()
            .map(|s| Estado::de_plan(&s.plan))
    }

    /// The newest GUARDIANA ZERO in the public ledger, read once when the person asks. The
    /// connection is written down like the licence's, as asked for by the person.
    pub fn ultima_version(&self, ahora: i64) -> Result<String, String> {
        let r = lic::get_texto(REGISTRO_PUBLICO);
        if let Ok(mut l) = self.libro() {
            let bytes = r
                .as_ref()
                .map_or(0, |(_, t)| i64::try_from(t.len()).unwrap_or(0));
            let _ = l.record_outbound(ahora, Purpose::Version, REGISTRO_HOST, bytes, true);
        }
        let (code, texto) = r.map_err(|e| e.to_string())?;
        if code != 200 {
            return Err(format!("HTTP {code}"));
        }
        ultima_zero(&texto).ok_or_else(|| "el registro no tiene ninguna versión".to_string())
    }

    /// The licence's connections, newest first (at most 20).
    #[must_use]
    pub fn conexiones(&self) -> Vec<Conexion> {
        let Ok(l) = self.libro() else {
            return Vec::new();
        };
        l.outbound()
            .unwrap_or_default()
            .into_iter()
            .filter(|o| matches!(o.purpose, Purpose::Licencia | Purpose::Version))
            .take(20)
            .map(|o| Conexion {
                ms: o.ts,
                version: o.purpose == Purpose::Version,
                host: o.host,
                pedida: o.initiated_by_user,
            })
            .collect()
    }

    /// The two places the trial date is kept, in words.
    #[must_use]
    pub fn donde(&self) -> (String, String) {
        (self.db.display().to_string(), lic::ancla::donde())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {

    #[test]
    fn the_newest_zero_is_found_in_the_public_ledger() {
        let texto = concat!(
            r#"{"version":"1.0.9","files":[]}"#,
            "\n",
            r#"{"version":"zero-1.0.0","files":[]}"#,
            "\n",
            "una línea rota\n",
            r#"{"version":"zero-1.0.10","files":[]}"#,
            "\n",
            r#"{"version":"zero-1.0.9","files":[]}"#,
            "\n",
        );
        assert_eq!(ultima_zero(texto).as_deref(), Some("1.0.10"));
        assert_eq!(ultima_zero(r#"{"version":"1.0.9"}"#), None);
        assert!(compara_versiones("1.0.10", "1.0.9").is_gt());
        assert!(compara_versiones("1.0.1", "1.0.1").is_eq());
        assert!(compara_versiones("0.9.0", "1.0.0").is_lt());
    }

    use super::*;

    const DIA: i64 = 86_400_000;

    /// The browser's week, from the first run to the end, with GUARDIANA's rules and the marks in
    /// two folders of the person's own. The only test in this crate that touches the process's
    /// mark folder.
    #[test]
    fn siete_dias_propios_y_despues_sin_proteccion() {
        let base = std::env::temp_dir().join(format!("zero-licencia-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let datos = base.join("datos");
        let ancla = base.join("marca");
        let lugar = Lugar::nuevo(&datos, ancla.clone());
        let t0 = 1_791_000_000_000;

        let e = lugar.estado(t0).unwrap();
        assert_eq!(
            e,
            Estado::Prueba {
                termina: t0 + 7 * DIA,
                dias: 7
            }
        );
        assert!(e.protege());
        assert!(
            ancla.join("prueba-empezada").exists(),
            "la segunda marca, fuera de los datos"
        );
        let (d, a) = lugar.donde();
        assert!(d.ends_with(ARCHIVO) && a.starts_with(&ancla.display().to_string()));

        let e = lugar.estado(t0 + 5 * DIA + 1).unwrap();
        assert_eq!(
            e,
            Estado::Prueba {
                termina: t0 + 7 * DIA,
                dias: 2
            }
        );

        // Deleting the data folder does not give the week back: the other mark keeps the date.
        std::fs::remove_dir_all(&datos).unwrap();
        let e = lugar.estado(t0 + 7 * DIA + 1).unwrap();
        assert_eq!(
            e,
            Estado::PruebaTerminada {
                desde: t0 + 7 * DIA
            }
        );
        assert!(!e.protege());

        // An empty key is refused before anything leaves.
        assert_eq!(lugar.activar("  ", t0 + 8 * DIA), Err(Fallo::Vacia));
        assert!(lugar.conexiones().is_empty());

        // Nothing is due without a key: no connection.
        assert_eq!(lugar.comprobar(t0 + 8 * DIA), None);
        assert!(lugar.conexiones().is_empty());

        lic::ancla::usar_carpeta(None);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn los_planes_de_guardiana_se_leen_igual() {
        let p = lic::Plan::PlusTerminado {
            termino: 5,
            motivo: "cancelada".into(),
        };
        let e = Estado::de_plan(&p);
        assert_eq!(e.nombre(), "suscripcion_terminada");
        assert!(!e.protege());
        let p = lic::Plan::Plus {
            origen: "clave".into(),
            desde: 1,
            titular: None,
            periodo_dias: Some(30),
            comprobada: Some(1),
            proxima_comprobacion: Some(2),
            caduca_ms: Some(3),
            comprobacion: Some(lic::Comprobacion::Fallida),
        };
        let e = Estado::de_plan(&p);
        assert!(e.protege());
        assert!(matches!(
            e,
            Estado::Suscrita {
                fallida: true,
                caduca: Some(3),
                ..
            }
        ));
        assert!(
            Estado::Desconocido.protege(),
            "sin saber, el navegador funciona"
        );
        assert_eq!(Fallo::Limite.clave().0, "licencia_err_limite");
    }
}
