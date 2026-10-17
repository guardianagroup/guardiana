//! The watch over the microphone, the camera and the screen while a SECURE conversation is
//! open. Francisco, 17 Oct 2026: the summary at the end of a call must say «si ha habido
//! espionaje, si ha habido grabación».
//!
//! Windows keeps, per program, when it last started and stopped using the microphone, the camera
//! and programmatic screen capture (the privacy settings' «recent activity»: the registry's
//! `CapabilityAccessManager\ConsentStore`). The window reads it (`zero/navegador`, Windows only)
//! and hands the uses here; this module decides, on every system, which of them count: another
//! program, during the conversation. Nothing here guesses: a use is a fact Windows recorded,
//! with the program's name, and the summary says plainly what Windows cannot tell (a recorder
//! that copies the screen without asking it, say: that one finds the window black, see
//! [`Vigia::pantalla_protegida`]).

use serde::{Deserialize, Serialize};

/// What was used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Recurso {
    /// The microphone.
    Microfono,
    /// The camera.
    Camara,
    /// The screen, captured through Windows' own capture (Windows 11 records it).
    Pantalla,
}

impl Recurso {
    /// The words for the history and the chain («usó el micrófono»).
    #[must_use]
    pub fn verbo(self) -> &'static str {
        match self {
            Self::Microfono => "usó el micrófono",
            Self::Camara => "usó la cámara",
            Self::Pantalla => "capturó la pantalla",
        }
    }

    /// The same, while it goes on («está usando el micrófono»).
    #[must_use]
    pub fn ahora(self) -> &'static str {
        match self {
            Self::Microfono => "está usando el micrófono",
            Self::Camara => "está usando la cámara",
            Self::Pantalla => "está capturando la pantalla",
        }
    }
}

/// One use, as Windows recorded it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Uso {
    /// The program: its file name (`Zoom.exe`) or, for a Store app, its package name.
    pub programa: String,
    /// Its full path when Windows gave one (the key's name, with `#` for `\`).
    pub ruta: Option<String>,
    /// What it used.
    pub recurso: Recurso,
    /// Unix seconds when it last started using it.
    pub desde: u64,
    /// Unix seconds when it stopped; `None` while it is still using it.
    pub hasta: Option<u64>,
}

/// Windows' FILETIME (100 ns since 1601) as Unix seconds; `None` for zero (never, or «still»).
#[must_use]
pub fn de_filetime(ft: u64) -> Option<u64> {
    const DESDE_1601: u64 = 11_644_473_600;
    if ft == 0 {
        return None;
    }
    (ft / 10_000_000).checked_sub(DESDE_1601)
}

/// One ConsentStore entry as the registry gives it: the key's name, and its two times.
/// `NonPackaged` programs are named by their path with `#` for `\`; Store apps by their package
/// family name (`Microsoft.WindowsCamera_8wekyb3d8bbwe`). A start of zero means it never used it.
#[must_use]
pub fn de_registro(clave: &str, recurso: Recurso, inicio: u64, fin: u64) -> Option<Uso> {
    let desde = de_filetime(inicio)?;
    let (programa, ruta) = if clave.contains('#') {
        let ruta = clave.replace('#', "\\");
        let nombre = ruta.rsplit('\\').next().unwrap_or_default().to_owned();
        (nombre, Some(ruta))
    } else {
        // `Name_publisherhash`: the name is enough for a person to recognise it.
        let nombre = clave.split('_').next().unwrap_or(clave).to_owned();
        (nombre, None)
    };
    if programa.is_empty() {
        return None;
    }
    Some(Uso {
        programa: programa.chars().take(120).collect(),
        ruta: ruta.map(|r| r.chars().take(400).collect()),
        recurso,
        desde,
        // A stop older than the start is the previous use's: it is using it now.
        hasta: de_filetime(fin).filter(|&h| fin >= inicio && h >= desde),
    })
}

/// Whether the use is GUARDIANA's own: this window (`guardiana-zero.exe`) and the WebView2
/// engine that draws it, whose helper processes are the ones Windows sees taking the microphone
/// and the camera for the call.
#[must_use]
pub fn es_propio(u: &Uso) -> bool {
    let p = u.programa.to_ascii_lowercase();
    p == "guardiana-zero.exe" || p == "guardiana.exe" || p == "msedgewebview2.exe"
}

/// The watch's verdict for one computer: kept in the signed summary (crates/secure), so it is
/// defined there; this module fills it.
pub use guardiana_secure::resumen::Vigilancia as Veredicto;

/// What one look found that is new.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Visto {
    /// The use.
    pub uso: Uso,
    /// Whether the program is still using it now.
    pub ahora: bool,
}

/// The watch over one conversation.
#[derive(Debug, Clone, Default)]
pub struct Vigia {
    /// Whether the window handed a look at least once.
    pub activa: bool,
    /// Whether the window is kept out of screen captures right now.
    pub pantalla_protegida: bool,
    /// Whether it ever stopped being protected while the conversation was open.
    pub pantalla_expuesta: bool,
    /// Every (program, resource) seen during the conversation, with its first use.
    vistos: Vec<(String, Recurso, u64)>,
    /// What is using something right now (other programs only).
    pub en_uso: Vec<(String, Recurso)>,
}

impl Vigia {
    /// Look at the uses Windows recorded: returns those that are new for a conversation that
    /// started at `inicio` (a use still going on counts even if it began before).
    pub fn mira(&mut self, usos: &[Uso], inicio: u64) -> Vec<Visto> {
        self.activa = true;
        self.en_uso.clear();
        let mut nuevos = Vec::new();
        for u in usos {
            if es_propio(u) {
                continue;
            }
            let ahora = u.hasta.is_none();
            let cuenta = ahora || u.hasta.is_some_and(|h| h >= inicio);
            if !cuenta {
                continue;
            }
            if ahora {
                self.en_uso.push((u.programa.clone(), u.recurso));
            }
            let ya = self
                .vistos
                .iter()
                .any(|(p, r, _)| p.eq_ignore_ascii_case(&u.programa) && *r == u.recurso);
            if !ya && self.vistos.len() < 64 {
                self.vistos.push((u.programa.clone(), u.recurso, u.desde));
                nuevos.push(Visto {
                    uso: u.clone(),
                    ahora,
                });
            }
        }
        nuevos
    }

    /// The verdict so far, or for the part of the conversation from `desde` (a call).
    #[must_use]
    pub fn veredicto(&self, desde: u64) -> Veredicto {
        let de = |r: Recurso| -> Vec<String> {
            let mut v: Vec<String> = self
                .vistos
                .iter()
                .filter(|(p, x, d)| {
                    *x == r && (*d >= desde || self.en_uso.iter().any(|(q, y)| q == p && *y == r))
                })
                .map(|(p, _, _)| p.clone())
                .collect();
            v.dedup();
            v
        };
        Veredicto {
            vigilado: self.activa,
            microfono: de(Recurso::Microfono),
            camara: de(Recurso::Camara),
            pantalla: de(Recurso::Pantalla),
            pantalla_protegida: self.pantalla_protegida && !self.pantalla_expuesta,
        }
    }

    /// The window was (or could not be) kept out of screen captures.
    pub fn pon_pantalla(&mut self, protegida: bool) {
        if self.pantalla_protegida && !protegida {
            self.pantalla_expuesta = true;
        }
        self.pantalla_protegida = protegida;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn ft(unix: u64) -> u64 {
        (unix + 11_644_473_600) * 10_000_000
    }

    #[test]
    fn registry_entries_become_named_uses() {
        let u = de_registro(
            "C:#Program Files#Zoom#bin#Zoom.exe",
            Recurso::Microfono,
            ft(1_000),
            0,
        )
        .unwrap();
        assert_eq!(u.programa, "Zoom.exe");
        assert_eq!(
            u.ruta.as_deref(),
            Some("C:\\Program Files\\Zoom\\bin\\Zoom.exe")
        );
        assert_eq!((u.desde, u.hasta), (1_000, None));
        let s = de_registro(
            "Microsoft.WindowsCamera_8wekyb3d8bbwe",
            Recurso::Camara,
            ft(5),
            ft(9),
        )
        .unwrap();
        assert_eq!(
            (s.programa.as_str(), s.hasta),
            ("Microsoft.WindowsCamera", Some(9))
        );
        assert!(de_registro("C:#x.exe", Recurso::Camara, 0, 0).is_none());
        let otra_vez = de_registro("C:#y.exe", Recurso::Camara, ft(20), ft(10)).unwrap();
        assert_eq!(otra_vez.hasta, None, "started again after its last stop");
    }

    #[test]
    fn only_other_programs_during_the_conversation_count_once() {
        let mut v = Vigia::default();
        let usos = vec![
            de_registro("C:#a#msedgewebview2.exe", Recurso::Microfono, ft(150), 0).unwrap(),
            de_registro("C:#a#guardiana-zero.exe", Recurso::Camara, ft(150), 0).unwrap(),
            // Before the conversation and over: not counted.
            de_registro("C:#a#viejo.exe", Recurso::Microfono, ft(10), ft(20)).unwrap(),
            // Still going on from before: counted.
            de_registro("C:#a#obs64.exe", Recurso::Pantalla, ft(50), 0).unwrap(),
            // During it, already over: counted.
            de_registro("C:#a#Zoom.exe", Recurso::Microfono, ft(120), ft(130)).unwrap(),
        ];
        let n = v.mira(&usos, 100);
        assert_eq!(n.len(), 2);
        assert!(n.iter().any(|x| x.uso.programa == "obs64.exe" && x.ahora));
        assert!(n.iter().any(|x| x.uso.programa == "Zoom.exe" && !x.ahora));
        assert!(v.mira(&usos, 100).is_empty());
        let r = v.veredicto(100);
        assert!(r.vigilado && !r.limpio());
        assert_eq!(r.microfono, vec!["Zoom.exe".to_owned()]);
        assert_eq!(r.pantalla, vec!["obs64.exe".to_owned()]);
        // A call that started later: Zoom's use was before it, OBS is still on.
        let c = v.veredicto(140);
        assert!(c.microfono.is_empty());
        assert_eq!(c.pantalla, vec!["obs64.exe".to_owned()]);
    }

    #[test]
    fn the_screen_counts_as_protected_only_if_it_never_stopped_being() {
        let mut v = Vigia::default();
        assert!(!v.veredicto(0).pantalla_protegida);
        v.pon_pantalla(true);
        assert!(v.veredicto(0).pantalla_protegida);
        v.pon_pantalla(false);
        v.pon_pantalla(true);
        assert!(!v.veredicto(0).pantalla_protegida);
    }

    #[test]
    fn the_other_sides_verdict_is_read_with_limits() {
        let j = serde_json::json!({ "vigilado": true, "microfono": ["a\u{7}b.exe"], "pantalla_protegida": true });
        let v = Veredicto::de_json(&j).unwrap();
        assert_eq!(v.microfono, vec!["ab.exe".to_owned()]);
        assert!(v.pantalla_protegida && v.camara.is_empty());
        assert!(Veredicto::de_json(&serde_json::json!({})).is_none());
    }
}
