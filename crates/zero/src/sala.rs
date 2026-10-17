//! «Sala de casa» (16 Oct 2026, asked by the owner): the computer at home shows one QR code
//! and a duration; the people of the house read it with their phones, leave, and keep
//! talking and calling each other all day on mobile data, with the computer as the house's
//! exchange and GUARDIANA's blind relay in between. Nothing is installed on the phones: the
//! same page as «pasar al móvil» (`movil`), the same sealed link (AES-256-GCM, keys from the
//! QR code and fresh numbers), the same relay that cannot read anything.
//!
//! One code, several phones: the code carries one key `K`; the computer derives one seat key
//! per seat (`SEAT_KEY_i = SHA-256("guardiana-sala-llave" ‖ i ‖ K)`) and waits in each seat's
//! room at the relay. The phone derives them the same way and tries the seats in order until
//! the computer greets it; it remembers its seat and its name, so it comes back without
//! reading the code again for as long as the room lasts.
//!
//! Honest about the level: the computer at home opens everything (it is the exchange), so the
//! room is «fuerte», not «máxima»; a phone is let in only when the person at the computer
//! says so, by the name it typed; the room ends at the hour chosen.

use std::collections::VecDeque;

use base64::Engine as _;
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};

use crate::movil::{Desde, Hacia, Movil};

/// Seats in a room (phones that can be in at once).
pub const ASIENTOS: usize = 4;
/// Durations the screen offers, in hours.
pub const HORAS: [u64; 3] = [12, 24, 48];
/// How long a call rings before «no answer».
const TIMBRE_S: u64 = 45;
/// Messages kept for the screens.
const MENSAJES_MAX: usize = 400;
/// The computer itself, when it writes.
const CASA: usize = usize::MAX;

const B64URL: base64::engine::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;

/// The key of seat `i`, from the room's key.
#[must_use]
pub fn llave_de_asiento(clave: &[u8; 32], i: usize) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"guardiana-sala-llave");
    h.update([i as u8]);
    h.update(clave);
    let mut k = [0u8; 32];
    k.copy_from_slice(&h.finalize());
    k
}

/// One seat: the phone in it, if any, and what the person said about themselves.
struct Asiento {
    movil: Movil,
    nombre: String,
    /// Connected and not yet let in: the screen asks.
    pendiente: bool,
    /// The person at the computer let this phone in (by its name).
    aprobado: bool,
    enganchado: bool,
    /// What its recorder produces, while in a call.
    mime: Option<String>,
    /// Grows when the other side's player started anew: this phone's recorder starts again.
    reinicio: u64,
}

struct Mensaje {
    t: u64,
    de: usize,
    texto: String,
}

struct Llamada {
    de: usize,
    a: Option<usize>,
    video: bool,
    desde: u64,
}

/// The room.
pub struct Sala {
    clave: [u8; 32],
    asientos: Vec<Asiento>,
    rele: String,
    idioma: String,
    creada: u64,
    horas: u64,
    mensajes: VecDeque<Mensaje>,
    llamada: Option<Llamada>,
    cambio: bool,
    error: Option<String>,
}

fn ahora_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Sala {
    /// Open a room for `horas` hours, with the relay the conversations use.
    pub fn abre(horas: u64, idioma: &str) -> Result<Self, String> {
        Self::abre_con(&crate::rele::direccion(), horas, idioma)
    }

    /// The same, with the relay at `rele` (`host:port`).
    pub fn abre_con(rele: &str, horas: u64, idioma: &str) -> Result<Self, String> {
        let horas = if HORAS.contains(&horas) { horas } else { 24 };
        let mut clave = [0u8; 32];
        crate::sesion::azar(&mut clave);
        let mut asientos = Vec::with_capacity(ASIENTOS);
        for i in 0..ASIENTOS {
            let movil = Movil::arranca_con_clave(rele, llave_de_asiento(&clave, i))?;
            asientos.push(Asiento {
                movil,
                nombre: String::new(),
                pendiente: false,
                aprobado: false,
                enganchado: false,
                mime: None,
                reinicio: 0,
            });
        }
        Ok(Self {
            clave,
            asientos,
            rele: rele.strip_suffix(":443").unwrap_or(rele).to_owned(),
            idioma: idioma.chars().take(5).collect(),
            creada: ahora_s(),
            horas,
            mensajes: VecDeque::new(),
            llamada: None,
            cambio: true,
            error: None,
        })
    }

    /// When the room closes on its own.
    #[must_use]
    pub fn caduca(&self) -> u64 {
        self.creada + self.horas * 3600
    }

    /// The room is past its hour.
    #[must_use]
    pub fn caducada(&self) -> bool {
        ahora_s() >= self.caduca()
    }

    /// What the QR code says: the page's address with the room after `#` (never sent to the
    /// web server).
    #[must_use]
    pub fn enlace(&self) -> String {
        let datos = json!({
            "v": 2,
            "sala": 1,
            "k": B64URL.encode(self.clave),
            "r": self.rele,
            "l": self.idioma,
            "h": self.horas,
            "c": self.caduca(),
        });
        format!(
            "{}#{}",
            crate::movil::PAGINA,
            B64URL.encode(datos.to_string())
        )
    }

    /// The person at the computer lets seat `i` in.
    pub fn aprueba(&mut self, i: usize) {
        if let Some(a) = self.asientos.get_mut(i) {
            a.aprobado = true;
            a.pendiente = false;
            a.movil.aprueba();
            self.cambio = true;
        }
    }

    /// The person at the computer puts seat `i` out (and its phone needs the OK again).
    pub fn expulsa(&mut self, i: usize) {
        if let Some(a) = self.asientos.get_mut(i) {
            a.aprobado = false;
            a.pendiente = a.enganchado;
            if self
                .llamada
                .as_ref()
                .is_some_and(|l| l.de == i || l.a == Some(i))
            {
                self.llamada = None;
            }
            self.cambio = true;
        }
    }

    /// The computer writes to everyone.
    pub fn texto(&mut self, texto: &str) {
        let t: String = texto.trim().chars().take(8000).collect();
        if t.is_empty() {
            return;
        }
        self.mete(CASA, t);
    }

    fn mete(&mut self, de: usize, texto: String) {
        self.mensajes.push_back(Mensaje {
            t: ahora_s(),
            de,
            texto,
        });
        while self.mensajes.len() > MENSAJES_MAX {
            self.mensajes.pop_front();
        }
        self.cambio = true;
    }

    fn nombre_de(&self, i: usize) -> String {
        if i == CASA {
            return "Casa".to_owned();
        }
        self.asientos
            .get(i)
            .map(|a| {
                if a.nombre.is_empty() {
                    format!("Móvil {}", i + 1)
                } else {
                    a.nombre.clone()
                }
            })
            .unwrap_or_default()
    }

    /// Whom seat `i` is talking to in the call, if it is in one that is connected.
    fn pareja(&self, i: usize) -> Option<usize> {
        let l = self.llamada.as_ref()?;
        let a = l.a?;
        if l.de == i {
            Some(a)
        } else if a == i {
            Some(l.de)
        } else {
            None
        }
    }

    /// Everything the phones left since the last tick.
    pub fn tic(&mut self) {
        if self
            .llamada
            .as_ref()
            .is_some_and(|l| l.a.is_none() && ahora_s().saturating_sub(l.desde) > TIMBRE_S)
        {
            self.llamada = None;
            self.cambio = true;
        }
        for i in 0..self.asientos.len() {
            let llegado = self.asientos[i].movil.recoge();
            for d in llegado {
                let aprobado = self.asientos[i].aprobado;
                match d {
                    Desde::Enganchado => {
                        let a = &mut self.asientos[i];
                        a.enganchado = true;
                        a.pendiente = !a.aprobado;
                        self.cambio = true;
                    }
                    Desde::Soltado => {
                        let a = &mut self.asientos[i];
                        a.enganchado = false;
                        a.pendiente = false;
                        self.cambio = true;
                    }
                    Desde::Error(e) => {
                        self.error = Some(e);
                        self.cambio = true;
                    }
                    Desde::Nombre(n) => {
                        if !n.is_empty() {
                            self.asientos[i].nombre = n;
                            self.cambio = true;
                        }
                    }
                    Desde::Texto(t) if aprobado => self.mete(i, t),
                    Desde::Llamar { video, mime } if aprobado => {
                        if self.llamada.is_none() {
                            self.llamada = Some(Llamada {
                                de: i,
                                a: None,
                                video,
                                desde: ahora_s(),
                            });
                        }
                        if self.pareja(i).is_some()
                            || self.llamada.as_ref().is_some_and(|l| l.de == i)
                        {
                            self.asientos[i].mime = Some(mime);
                        }
                        self.cambio = true;
                    }
                    Desde::Contestar(mime) if aprobado => {
                        if let Some(l) = &mut self.llamada {
                            if l.a.is_none() && l.de != i {
                                l.a = Some(i);
                                l.desde = ahora_s();
                                self.asientos[i].mime = Some(mime);
                                self.cambio = true;
                            }
                        }
                    }
                    Desde::Rechazar if aprobado => {
                        if self.llamada.as_ref().is_some_and(|l| l.a.is_none()) {
                            self.llamada = None;
                            self.cambio = true;
                        }
                    }
                    Desde::Colgar if aprobado => {
                        if self
                            .llamada
                            .as_ref()
                            .is_some_and(|l| l.de == i || l.a == Some(i))
                        {
                            self.llamada = None;
                            for a in &mut self.asientos {
                                a.mime = None;
                            }
                            self.cambio = true;
                        }
                    }
                    Desde::PideInicio if aprobado => {
                        if let Some(otro) = self.pareja(i) {
                            self.asientos[otro].reinicio += 1;
                            self.cambio = true;
                        }
                    }
                    Desde::Media(b) if aprobado => {
                        if let Some(otro) = self.pareja(i) {
                            self.asientos[otro].movil.manda(Hacia::Media(b));
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// Whether the screens must be redrawn, and clear the flag.
    pub fn cambiado(&mut self) -> bool {
        std::mem::take(&mut self.cambio)
    }

    /// What seat `i`'s phone draws: the shape the page already knows (a conversation that is
    /// «lista»), with the room on top.
    fn estado_de(&self, i: usize) -> Value {
        let otros: Vec<String> = (0..self.asientos.len())
            .filter(|&j| j != i && self.asientos[j].aprobado)
            .map(|j| self.nombre_de(j))
            .collect();
        let mensajes: Vec<Value> = self
            .mensajes
            .iter()
            .map(|m| {
                let mio = m.de == i;
                json!({
                    "t": m.t,
                    "mio": mio,
                    "texto": if mio { m.texto.clone() } else { format!("{}: {}", self.nombre_de(m.de), m.texto) },
                })
            })
            .collect();
        let mut llamada = Value::Null;
        let mut entrante = Value::Null;
        if let Some(l) = &self.llamada {
            match l.a {
                None if l.de == i => {
                    llamada = json!({ "video": l.video, "desde": l.desde, "conectada": false });
                }
                None => {
                    entrante = json!({ "video": l.video, "de": self.nombre_de(l.de) });
                }
                Some(_) => {
                    if let Some(otro) = self.pareja(i) {
                        llamada = json!({
                            "video": l.video,
                            "desde": l.desde,
                            "conectada": true,
                            "reinicio": self.asientos[i].reinicio,
                            "mime_remoto": self.asientos[otro].mime,
                        });
                    }
                }
            }
        }
        json!({
            "tipo": "secure_estado",
            "fase": "lista",
            "contacto": otros.join(" · "),
            "mensajes": mensajes,
            "llamada": llamada,
            "entrante": entrante,
            "monitor": { "identidad": { "estado": "sala" }, "transporte": {} },
            "sala": {
                "yo": i,
                "nombre": self.nombre_de(i),
                "miembros": (0..self.asientos.len()).filter(|&j| self.asientos[j].aprobado).map(|j| json!({ "nombre": self.nombre_de(j), "enganchado": self.asientos[j].enganchado })).collect::<Vec<_>>(),
                "caduca": self.caduca(),
            },
        })
    }

    /// After anything changed: every phone gets its view.
    pub fn sincroniza(&self) {
        for (i, a) in self.asientos.iter().enumerate() {
            if !a.enganchado {
                continue;
            }
            let e = if a.aprobado {
                self.estado_de(i)
            } else {
                json!({ "tipo": "secure_estado", "movil": { "pendiente": true }, "sala": { "pendiente": true } })
            };
            a.movil.manda(Hacia::Estado(e.to_string()));
        }
    }

    /// The computer's view.
    #[must_use]
    pub fn json(&self) -> Value {
        json!({
            "creada": self.creada,
            "horas": self.horas,
            "caduca": self.caduca(),
            "qr": guardiana_secure::tarjeta::qr_svg(&self.enlace()).ok(),
            "miembros": self.asientos.iter().enumerate().map(|(i, a)| json!({
                "i": i,
                "nombre": self.nombre_de(i),
                "con_nombre": !a.nombre.is_empty(),
                "enganchado": a.enganchado,
                "pendiente": a.pendiente,
                "aprobado": a.aprobado,
            })).collect::<Vec<_>>(),
            "mensajes": self.mensajes.iter().rev().take(200).rev().map(|m| json!({
                "t": m.t, "de": self.nombre_de(m.de), "mio": m.de == CASA, "texto": m.texto,
            })).collect::<Vec<_>>(),
            "llamada": self.llamada.as_ref().map(|l| json!({
                "de": self.nombre_de(l.de),
                "a": l.a.map(|a| self.nombre_de(a)),
                "video": l.video,
                "desde": l.desde,
            })),
            "error": self.error,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn seat_keys_differ_and_the_code_names_the_room() {
        let k = [7u8; 32];
        let a = llave_de_asiento(&k, 0);
        let b = llave_de_asiento(&k, 1);
        assert_ne!(a, b);
        assert_eq!(a, llave_de_asiento(&k, 0));
    }

    #[test]
    fn the_room_lasts_the_hours_chosen_and_the_code_carries_them() {
        let s = Sala::abre_con("127.0.0.1:1", 48, "es").unwrap();
        assert_eq!(s.caduca(), s.creada + 48 * 3600);
        assert!(!s.caducada());
        let v = crate::movil::lee_enlace(&s.enlace()).unwrap();
        assert_eq!(v["sala"], 1);
        assert_eq!(v["h"], 48);
        assert_eq!(v["k"].as_str().unwrap().len(), 43);
        let j = s.json();
        assert_eq!(j["miembros"].as_array().unwrap().len(), ASIENTOS);
        assert!(j["qr"].as_str().unwrap().contains("<svg"));
    }

    #[test]
    fn a_bad_duration_becomes_a_day() {
        let s = Sala::abre_con("127.0.0.1:1", 5, "es").unwrap();
        assert_eq!(s.horas, 24);
    }
}
