//! «Continuar en el móvil» without a gap (18 Oct 2026, the committee's spec: make before
//! break). While a call is on, the voice moves from this computer to the person's phone only
//! once the phone's own leg is carrying sound; until then the computer's leg stays exactly as
//! it was. If the phone's leg does not come up in time, nothing on the computer changed and the
//! phone is told «No se pudo pasar — vuelve a escanear». «Volver al PC» goes back the same way.
//!
//! Two ways the phone's voice can travel, chosen by what the call itself uses:
//! - the bridge (`puente`, when the call goes by WebRTC through the relay's call service): the
//!   phone opens a WebRTC leg to this computer's page, relay only like every call here (no
//!   address is ever seen, 18 Oct 2026, the owner's rule), and the page swaps the track it sends
//!   to the other person for the phone's once that leg is live. The other person's connection
//!   is never renegotiated, so they hear no cut;
//! - the recorded pieces (no call service): the computer keeps sending its own pieces and plays
//!   the other side's on both screens until the phone's first piece arrives.
//!
//! This module is only the decision; `secure.rs` routes the messages and the pieces.

use serde_json::{json, Value};

/// How long the phone's leg has to come up before the hand-off is given up (counted from the
/// moment the call is connected: a call still ringing has no voice to move).
pub const PASO_MAX_S: u64 = 12;
/// How long «No se pudo pasar — vuelve a escanear» stays on the screens.
pub const FALLO_VISIBLE_S: u64 = 20;

/// Where the voice of the call is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Donde {
    /// On this computer.
    Pc,
    /// Moving to the phone since that second (the computer's leg is still the live one).
    Pasando(u64),
    /// On the phone.
    Movil,
}

/// What happened in a step, for the monitor and the screens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suceso {
    /// The phone's leg carries the voice now.
    EnElMovil,
    /// The phone's leg did not come up in time: the computer kept the call.
    NoPaso,
    /// The voice is back on this computer (asked, or the phone went away).
    Volvio,
}

/// The hand-off of one conversation.
#[derive(Debug, Clone)]
pub struct Relevo {
    donde: Donde,
    /// The page said its bridge to the phone carries sound.
    puente_vivo: bool,
    /// The bridge cannot be used for this call (the page's own leg fell back to pieces).
    puente_roto: bool,
    /// When the last hand-off failed, 0 if none.
    fallo: u64,
    /// There was a call at the last step.
    habia_llamada: bool,
}

impl Default for Relevo {
    fn default() -> Self {
        Self {
            donde: Donde::Pc,
            puente_vivo: false,
            puente_roto: false,
            fallo: 0,
            habia_llamada: false,
        }
    }
}

impl Relevo {
    /// Where the voice is.
    #[must_use]
    pub fn donde(&self) -> Donde {
        self.donde
    }

    /// Whether the phone is the one talking (`listo`: a phone connected and accepted). Without
    /// a call the phone that holds the conversation talks at once (there is no voice to move).
    #[must_use]
    pub fn movil_manda(&self, listo: bool, hay_llamada: bool) -> bool {
        listo && (!hay_llamada || self.donde == Donde::Movil)
    }

    /// Whether the bridge is the way for this call (`rtc`: the call goes by WebRTC).
    #[must_use]
    pub fn puente(&self, rtc: bool) -> bool {
        rtc && !self.puente_roto
    }

    /// The other side's recorded pieces go to the phone (moving or moved there).
    #[must_use]
    pub fn piezas_al_movil(&self, listo: bool) -> bool {
        listo && self.donde != Donde::Pc
    }

    /// The other side's recorded pieces go to the page (until the phone carries the voice).
    #[must_use]
    pub fn piezas_a_pagina(&self, listo: bool) -> bool {
        !(listo && self.donde == Donde::Movil)
    }

    /// A piece recorded on the page may leave (the computer's leg is the live one).
    #[must_use]
    pub fn pagina_envia(&self, listo: bool) -> bool {
        !(listo && self.donde == Donde::Movil)
    }

    /// A piece recorded on the phone arrived: it may leave once the hand-off started, and the
    /// first one completes it (the phone's leg is carrying sound).
    pub fn pieza_del_movil(&mut self, listo: bool) -> (bool, Option<Suceso>) {
        match self.donde {
            Donde::Pc => (false, None),
            _ if !listo => (false, None),
            Donde::Pasando(_) => {
                self.donde = Donde::Movil;
                (true, Some(Suceso::EnElMovil))
            }
            Donde::Movil => (true, None),
        }
    }

    /// Start moving the voice to the phone (the phone was accepted, came back, or the person
    /// asked for it). Only during a call, from the computer.
    pub fn empieza(&mut self, listo: bool, hay_llamada: bool, ahora: u64) -> bool {
        if !listo || !hay_llamada || self.donde != Donde::Pc {
            return false;
        }
        self.donde = Donde::Pasando(ahora);
        self.puente_vivo = false;
        self.fallo = 0;
        true
    }

    /// The page's bridge to the phone carries sound: the voice is on the phone.
    pub fn puente_listo(&mut self, listo: bool) -> Option<Suceso> {
        if !listo || self.donde == Donde::Pc {
            return None;
        }
        self.puente_vivo = true;
        if self.donde == Donde::Movil {
            return None;
        }
        self.donde = Donde::Movil;
        Some(Suceso::EnElMovil)
    }

    /// The page's bridge broke or never came up (`sin_rtc`: the page's own leg left WebRTC, so
    /// the bridge cannot serve this call any more). The voice stays on, or goes back to, the
    /// computer.
    pub fn puente_fallo(&mut self, sin_rtc: bool, ahora: u64) -> Option<Suceso> {
        self.puente_vivo = false;
        if sin_rtc {
            self.puente_roto = true;
        }
        match self.donde {
            Donde::Pc => None,
            Donde::Pasando(_) => {
                self.donde = Donde::Pc;
                self.fallo = ahora;
                Some(Suceso::NoPaso)
            }
            Donde::Movil => {
                self.donde = Donde::Pc;
                self.fallo = ahora;
                Some(Suceso::Volvio)
            }
        }
    }

    /// «Volver al PC»: the voice comes back to the computer, whose leg never stopped.
    pub fn volver(&mut self) -> Option<Suceso> {
        self.puente_vivo = false;
        if self.donde == Donde::Pc {
            return None;
        }
        self.donde = Donde::Pc;
        Some(Suceso::Volvio)
    }

    /// Once a tick: `listo` (a phone connected and accepted), whether there is a call and
    /// whether it is connected.
    pub fn tic(
        &mut self,
        listo: bool,
        hay_llamada: bool,
        conectada: bool,
        ahora: u64,
    ) -> Option<Suceso> {
        let habia = std::mem::replace(&mut self.habia_llamada, hay_llamada);
        if !listo {
            self.puente_vivo = false;
            let era = self.donde;
            self.donde = Donde::Pc;
            return (hay_llamada && era == Donde::Movil).then_some(Suceso::Volvio);
        }
        if !hay_llamada {
            // The phone holds the conversation; the next call starts from it.
            self.donde = Donde::Movil;
            self.puente_vivo = false;
            self.puente_roto = false;
            return None;
        }
        if !habia && self.donde == Donde::Movil {
            // A call starts while the phone holds the conversation: the phone's leg comes up
            // while it rings, and the computer's leg covers until it does.
            self.donde = Donde::Pasando(ahora);
            self.puente_vivo = false;
            return None;
        }
        if let Donde::Pasando(desde) = self.donde {
            if !conectada {
                // Ringing: the clock starts when there is a voice to move.
                self.donde = Donde::Pasando(ahora);
            } else if ahora.saturating_sub(desde) >= PASO_MAX_S {
                self.donde = Donde::Pc;
                self.fallo = ahora;
                return Some(Suceso::NoPaso);
            }
        }
        None
    }

    /// What the screens draw (the page and the phone).
    #[must_use]
    pub fn json(&self, rtc: bool, ahora: u64) -> Value {
        json!({
            "relevo": match self.donde {
                Donde::Pc => "pc",
                Donde::Pasando(_) => "pasando",
                Donde::Movil => "movil",
            },
            "puente": self.puente(rtc),
            "puente_vivo": self.puente_vivo,
            "fallo_paso": self.fallo > 0 && ahora.saturating_sub(self.fallo) < FALLO_VISIBLE_S,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_a_call_the_phone_holds_the_conversation_at_once() {
        let mut r = Relevo::default();
        assert!(!r.movil_manda(false, false));
        assert_eq!(r.tic(true, false, false, 10), None);
        assert_eq!(r.donde(), Donde::Movil);
        assert!(r.movil_manda(true, false));
    }

    #[test]
    fn a_call_on_the_computer_moves_only_once_the_bridge_is_live() {
        let mut r = Relevo::default();
        r.tic(false, true, true, 0);
        assert!(r.empieza(true, true, 100));
        // Moving: the computer still talks, its pieces still leave, the other side's go to both.
        assert!(!r.movil_manda(true, true));
        assert!(r.pagina_envia(true));
        assert!(r.piezas_a_pagina(true) && r.piezas_al_movil(true));
        assert_eq!(r.tic(true, true, true, 105), None);
        assert_eq!(r.puente_listo(true), Some(Suceso::EnElMovil));
        assert_eq!(r.donde(), Donde::Movil);
        assert!(r.movil_manda(true, true));
        assert!(!r.pagina_envia(true));
        assert_eq!(r.json(true, 106)["relevo"], "movil");
        assert_eq!(r.json(true, 106)["puente_vivo"], true);
        // «Volver al PC»: back at once, the computer's leg never stopped.
        assert_eq!(r.volver(), Some(Suceso::Volvio));
        assert!(!r.movil_manda(true, true));
        assert!(r.pagina_envia(true));
        // And it stays there while the call goes on (no automatic second hand-off).
        assert_eq!(r.tic(true, true, true, 200), None);
        assert_eq!(r.donde(), Donde::Pc);
    }

    #[test]
    fn a_hand_off_that_does_not_come_up_leaves_the_computer_untouched() {
        let mut r = Relevo::default();
        r.tic(true, true, true, 0);
        assert_eq!(
            r.donde(),
            Donde::Pc,
            "a phone already there does not take a running call"
        );
        assert!(r.empieza(true, true, 100));
        assert_eq!(r.tic(true, true, true, 100 + PASO_MAX_S - 1), None);
        assert_eq!(
            r.tic(true, true, true, 100 + PASO_MAX_S),
            Some(Suceso::NoPaso)
        );
        assert_eq!(r.donde(), Donde::Pc);
        assert!(r.pagina_envia(true));
        assert_eq!(r.json(true, 115)["fallo_paso"], true);
        assert_eq!(r.json(true, 115 + FALLO_VISIBLE_S)["fallo_paso"], false);
        // A bridge that answers too late changes nothing.
        assert_eq!(r.puente_listo(true), None);
        assert_eq!(r.donde(), Donde::Pc);
    }

    #[test]
    fn the_bridge_failing_while_moving_or_moved_brings_the_voice_back() {
        let mut r = Relevo::default();
        r.tic(true, true, true, 0);
        r.empieza(true, true, 10);
        assert_eq!(r.puente_fallo(false, 11), Some(Suceso::NoPaso));
        assert_eq!(r.donde(), Donde::Pc);
        r.empieza(true, true, 20);
        r.puente_listo(true);
        assert_eq!(r.puente_fallo(true, 30), Some(Suceso::Volvio));
        assert_eq!(r.donde(), Donde::Pc);
        assert!(!r.puente(true), "the page left WebRTC: pieces from now on");
        // The next call starts afresh.
        r.tic(true, false, false, 40);
        assert!(r.puente(true));
    }

    #[test]
    fn a_call_that_starts_on_the_phone_waits_for_its_leg_while_it_rings() {
        let mut r = Relevo::default();
        r.tic(true, false, false, 0);
        assert_eq!(r.tic(true, true, false, 1), None);
        assert!(matches!(r.donde(), Donde::Pasando(_)));
        // Ringing for longer than the hand-off time is no failure.
        assert_eq!(r.tic(true, true, false, 1 + 3 * PASO_MAX_S), None);
        // Pieces: the phone's first piece completes it.
        assert_eq!(r.pieza_del_movil(true), (true, Some(Suceso::EnElMovil)));
        assert_eq!(r.pieza_del_movil(true), (true, None));
        assert!(r.movil_manda(true, true));
        assert!(!r.piezas_a_pagina(true));
    }

    #[test]
    fn the_phone_going_away_gives_the_call_back_to_the_computer() {
        let mut r = Relevo::default();
        r.tic(true, true, true, 0);
        r.empieza(true, true, 1);
        r.puente_listo(true);
        assert_eq!(r.tic(false, true, true, 2), Some(Suceso::Volvio));
        assert_eq!(r.donde(), Donde::Pc);
        assert_eq!(
            r.pieza_del_movil(true),
            (false, None),
            "a late piece does not leave"
        );
        // Nothing to move without a phone or without a call.
        assert!(!r.empieza(false, true, 3));
        assert!(!r.empieza(true, false, 3));
    }
}
