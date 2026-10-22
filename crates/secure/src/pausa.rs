//! The automatic stop. The owner, 15 Oct 2026: «si GUARDIANA detecta un espionaje o un
//! saboteo, que pare automáticamente, aparezca en pantalla por qué hemos parado, y se reanude
//! donde se quedó cuando veamos que no hay peligro».
//!
//! A session is either running or paused for a reason. Something seen (`Observada`) pauses it
//! and lets it resume on its own once the cause is gone, or at once if the person says the
//! program is theirs (that choice is recorded). Something unusual pauses it until the cause is
//! gone, with no shortcut. A changed key pauses it until a new short-code check, never on its
//! own. Every stop and every resumption goes into the chain of evidence.

use serde::{Deserialize, Serialize};

use crate::monitor::{Monitor, Nivel};
use crate::prueba::{Cadena, Hecho};

/// Why the session is stopped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "por", rename_all = "snake_case")]
pub enum Motivo {
    /// Another program was seen doing something concrete. Resumes on its own when it stops,
    /// or now if the person says that program is theirs.
    Observado {
        /// What was seen, in the person's language.
        texto: String,
        /// The program, when known.
        proceso: Option<String>,
    },
    /// Something unusual. Resumes on its own when it clears; no shortcut.
    Inusual {
        /// What was seen.
        texto: String,
    },
    /// The contact's keys changed. Resumes only after a new short-code check.
    ClaveCambiada {
        /// The contact's name.
        nombre: String,
    },
}

/// The stop, as the screen shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pausa {
    /// Why.
    pub motivo: Motivo,
    /// Unix seconds when it stopped.
    pub desde: u64,
    /// What still keeps it stopped, in the person's language; empty means the cause is gone
    /// and the resumption countdown may start.
    pub pendiente: Vec<String>,
    /// Whether the person may resume now without the cause being gone.
    pub puede_reanudar: bool,
    /// How often the cause is checked, seconds.
    pub comprueba_cada_s: u32,
    /// Seconds of countdown before resuming once the cause is gone.
    pub cuenta_atras_s: u32,
}

/// Running or stopped.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "estado", rename_all = "snake_case")]
pub enum Sesion {
    /// Messages, files and media flow.
    #[default]
    EnMarcha,
    /// Nothing leaves this computer; the conversation is kept where it was.
    Pausada(Pausa),
}

/// How often the cause is checked while stopped.
pub const COMPRUEBA_CADA_S: u32 = 2;
/// The countdown before resuming once the cause is gone.
pub const CUENTA_ATRAS_S: u32 = 3;

impl Sesion {
    /// Stop now for `motivo`, recording it in the monitor and the chain. A session already
    /// stopped keeps its first reason; the new one is only recorded.
    pub fn parar(
        &mut self,
        motivo: Motivo,
        ahora: u64,
        monitor: &mut Monitor,
        cadena: &mut Cadena,
    ) {
        let (nivel, texto, proceso) = match &motivo {
            Motivo::Observado { texto, proceso } => {
                (Nivel::Observada, texto.clone(), proceso.clone())
            }
            Motivo::Inusual { texto } => (Nivel::Inusual, texto.clone(), None),
            Motivo::ClaveCambiada { nombre } => {
                (Nivel::Inusual, format!("clave cambiada: {nombre}"), None)
            }
        };
        monitor.anota(ahora, nivel, format!("parada automática: {texto}"));
        cadena.anota(
            ahora,
            nivel,
            Hecho {
                texto: format!("parada automática: {texto}"),
                proceso: proceso.clone(),
                ..Hecho::default()
            },
        );
        if matches!(self, Sesion::Pausada(_)) {
            return;
        }
        let puede_reanudar = matches!(motivo, Motivo::Observado { .. });
        *self = Sesion::Pausada(Pausa {
            pendiente: vec![texto],
            motivo,
            desde: ahora,
            puede_reanudar,
            comprueba_cada_s: COMPRUEBA_CADA_S,
            cuenta_atras_s: CUENTA_ATRAS_S,
        });
    }

    /// The watch looked again: `pendiente` is what still keeps the session stopped (empty when
    /// the cause is gone). A changed key ignores this: only [`Sesion::reverificada`] clears it.
    pub fn comprobada(&mut self, pendiente: Vec<String>) {
        if let Sesion::Pausada(p) = self {
            if !matches!(p.motivo, Motivo::ClaveCambiada { .. }) {
                p.pendiente = pendiente;
            }
        }
    }

    /// Whether the countdown to resume may run: stopped, and nothing pending.
    pub fn lista_para_reanudar(&self) -> bool {
        matches!(self, Sesion::Pausada(p) if p.pendiente.is_empty())
    }

    /// Resume on its own, once the cause is gone and the countdown ran. Returns `false` and
    /// changes nothing while something is still pending.
    pub fn reanudar(&mut self, ahora: u64, monitor: &mut Monitor, cadena: &mut Cadena) -> bool {
        if !self.lista_para_reanudar() {
            return false;
        }
        self.sigue(ahora, monitor, cadena, "reanudada: la causa desapareció");
        true
    }

    /// The person said the program seen is theirs. Only for [`Motivo::Observado`]; the choice
    /// is recorded with the stop it answers. Returns `false` and changes nothing otherwise.
    pub fn reanudar_a_mano(
        &mut self,
        ahora: u64,
        monitor: &mut Monitor,
        cadena: &mut Cadena,
    ) -> bool {
        let Sesion::Pausada(p) = self else {
            return false;
        };
        if !p.puede_reanudar {
            return false;
        }
        self.sigue(
            ahora,
            monitor,
            cadena,
            "reanudada por la persona: dijo que el programa es suyo",
        );
        true
    }

    /// A new short-code check pinned the contact again: the only way out of a changed key.
    pub fn reverificada(&mut self, ahora: u64, monitor: &mut Monitor, cadena: &mut Cadena) -> bool {
        let Sesion::Pausada(p) = self else {
            return false;
        };
        if !matches!(p.motivo, Motivo::ClaveCambiada { .. }) {
            return false;
        }
        self.sigue(
            ahora,
            monitor,
            cadena,
            "reanudada: identidad verificada de nuevo",
        );
        true
    }

    fn sigue(&mut self, ahora: u64, monitor: &mut Monitor, cadena: &mut Cadena, texto: &str) {
        monitor.anota(ahora, Nivel::Info, texto);
        cadena.anota(
            ahora,
            Nivel::Info,
            Hecho {
                texto: texto.to_owned(),
                ..Hecho::default()
            },
        );
        *self = Sesion::EnMarcha;
    }

    /// Whether anything may leave this computer right now.
    pub fn en_marcha(&self) -> bool {
        matches!(self, Sesion::EnMarcha)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn visto() -> Motivo {
        Motivo::Observado {
            texto: "Chrome usó el micrófono".to_owned(),
            proceso: Some("chrome.exe".to_owned()),
        }
    }

    #[test]
    fn something_seen_stops_and_resumes_when_it_is_gone_or_when_the_person_says_so() {
        let mut m = Monitor::new();
        let mut c = Cadena::new("s", 1);
        let mut s = Sesion::default();
        assert!(s.en_marcha());
        s.parar(visto(), 10, &mut m, &mut c);
        assert!(!s.en_marcha());
        assert!(!s.lista_para_reanudar());
        assert!(!s.reanudar(11, &mut m, &mut c));
        // Still there at the next check; then gone.
        s.comprobada(vec!["Chrome todavía usa el micrófono".to_owned()]);
        assert!(!s.reanudar(12, &mut m, &mut c));
        s.comprobada(Vec::new());
        assert!(s.lista_para_reanudar());
        assert!(s.reanudar(15, &mut m, &mut c));
        assert!(s.en_marcha());
        assert_eq!(c.registros.len(), 2);
        assert!(c.registros[1].texto.starts_with("reanudada"));
        assert_eq!(m.vigilancia.len(), 1);

        // By hand, because the program is the person's own.
        s.parar(visto(), 20, &mut m, &mut c);
        assert!(s.reanudar_a_mano(21, &mut m, &mut c));
        assert!(c.registros[3].texto.contains("por la persona"));
    }

    #[test]
    fn unusual_has_no_shortcut_and_a_changed_key_waits_for_the_short_code() {
        let mut m = Monitor::new();
        let mut c = Cadena::new("s", 1);
        let mut s = Sesion::default();
        s.parar(
            Motivo::Inusual {
                texto: "conexión nueva".to_owned(),
            },
            10,
            &mut m,
            &mut c,
        );
        assert!(!s.reanudar_a_mano(11, &mut m, &mut c));
        s.comprobada(Vec::new());
        assert!(s.reanudar(14, &mut m, &mut c));

        s.parar(
            Motivo::ClaveCambiada {
                nombre: "Marta".to_owned(),
            },
            20,
            &mut m,
            &mut c,
        );
        s.comprobada(Vec::new());
        assert!(!s.lista_para_reanudar());
        assert!(!s.reanudar(21, &mut m, &mut c));
        assert!(!s.reanudar_a_mano(21, &mut m, &mut c));
        // A second reason while stopped is recorded but does not replace the first.
        s.parar(visto(), 22, &mut m, &mut c);
        assert!(
            matches!(&s, Sesion::Pausada(p) if matches!(p.motivo, Motivo::ClaveCambiada { .. }))
        );
        assert!(s.reverificada(30, &mut m, &mut c));
        assert!(s.en_marcha());
        assert!(c.comprueba().is_ok());
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("en_marcha"));
    }
}
