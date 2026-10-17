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
use std::path::PathBuf;

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

/// A file name a phone sent, safe to write under Descargas.
fn nombre_limpio(n: &str) -> String {
    let n: String = n
        .chars()
        .filter(|c| {
            !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') && !c.is_control()
        })
        .take(120)
        .collect();
    // No dot or space at the end either (Windows drops them), and no device names.
    let n = n
        .trim()
        .trim_start_matches('.')
        .trim_end_matches(['.', ' '])
        .to_owned();
    if n.is_empty() {
        "archivo".to_owned()
    } else if crate::secure::nombre_de_dispositivo(&n) {
        format!("_{n}")
    } else {
        n
    }
}

/// Hand a file to one phone: the announcement, then the bytes in pieces of 64 KB.
fn reparte_archivo(movil: &Movil, a: &Archivo, datos: &[u8]) {
    movil.manda(Hacia::Estado(
        json!({ "tipo": "archivo_movil", "id": a.id, "nombre": a.nombre, "mime": a.mime, "bytes": a.bytes }).to_string(),
    ));
    for trozo in datos.chunks(65536) {
        movil.manda(Hacia::Archivo(a.id, trozo.to_vec()));
    }
}

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
    /// Unix seconds of its last «escribiendo».
    escribe_en: u64,
    /// A file on its way from this phone: name, size, type, bytes so far.
    archivo: Option<(String, u64, String, Vec<u8>)>,
}

/// A file that lives in the room: saved on the computer and handed to the phones.
#[derive(Clone)]
struct Archivo {
    /// Its id on the phone link (frames of type 2).
    id: u32,
    nombre: String,
    mime: String,
    ruta: PathBuf,
    bytes: u64,
}

struct Mensaje {
    /// Shared with the phones (see [`crate::mensaje`]).
    id: u64,
    t: u64,
    de: usize,
    texto: String,
    /// The message it answers.
    re: Option<u64>,
    /// Seats (and `CASA`) that had it on screen.
    vistos: Vec<usize>,
    /// Seats that deleted it for themselves.
    ocultos: Vec<usize>,
    editado: bool,
    borrado: bool,
    /// (seat, emoji).
    reacciones: Vec<(usize, String)>,
    archivo: Option<Archivo>,
    llamada: Option<crate::mensaje::Llamada>,
}

impl Mensaje {
    fn nuevo(de: usize, texto: String, re: Option<u64>) -> Self {
        Self {
            id: crate::mensaje::Mensaje::nuevo_id(),
            t: ahora_s(),
            de,
            texto,
            re,
            vistos: vec![de],
            ocultos: Vec::new(),
            editado: false,
            borrado: false,
            reacciones: Vec::new(),
            archivo: None,
            llamada: None,
        }
    }
}

/// The biggest file a phone may put in the room (the whole thing sits in memory a moment).
const ARCHIVO_MAX: u64 = 64 * 1024 * 1024;

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
    /// Where the room's files are saved (Descargas); none while unset (tests).
    descargas: Option<PathBuf>,
    /// The last file id handed to the phones.
    ultimo_archivo: u32,
}

impl std::fmt::Debug for Sala {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sala")
            .field("horas", &self.horas)
            .field("asientos", &self.asientos.len())
            .finish()
    }
}

fn ahora_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Sala {
    /// Whether two phones of the house are on a call through this computer.
    pub fn en_llamada(&self) -> bool {
        self.llamada.is_some()
    }

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
                escribe_en: 0,
                archivo: None,
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
            descargas: None,
            ultimo_archivo: 0,
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
        self.mete_re(de, texto, None);
    }

    fn mete_re(&mut self, de: usize, texto: String, re: Option<u64>) {
        self.guarda(Mensaje::nuevo(de, texto, re));
    }

    fn guarda(&mut self, m: Mensaje) {
        self.mensajes.push_back(m);
        while self.mensajes.len() > MENSAJES_MAX {
            self.mensajes.pop_front();
        }
        self.cambio = true;
    }

    /// Where the room keeps its files.
    pub fn con_descargas(mut self, carpeta: &std::path::Path) -> Self {
        self.descargas = Some(carpeta.to_path_buf());
        self
    }

    /// The computer answers a message.
    pub fn texto_re(&mut self, texto: &str, re: Option<u64>) {
        let t: String = texto.trim().chars().take(8000).collect();
        if !t.is_empty() {
            self.mete_re(CASA, t, re);
        }
    }

    /// Someone (`quien`: a seat or `CASA`) had the room on screen: everything is seen by them.
    fn visto_por(&mut self, quien: usize) {
        let mut cambio = false;
        for m in &mut self.mensajes {
            if !m.vistos.contains(&quien) {
                m.vistos.push(quien);
                cambio = true;
            }
        }
        if cambio {
            self.cambio = true;
        }
    }

    /// The computer's screen is in front.
    pub fn visto(&mut self) {
        self.visto_por(CASA);
    }

    fn edita(&mut self, quien: usize, id: u64, texto: String) {
        if let Some(m) = self
            .mensajes
            .iter_mut()
            .find(|m| m.id == id && m.de == quien)
        {
            if !m.borrado && m.archivo.is_none() && m.llamada.is_none() {
                m.texto = texto.chars().take(8000).collect();
                m.editado = true;
                self.cambio = true;
            }
        }
    }

    fn borra(&mut self, quien: usize, id: u64, para_todos: bool) {
        if let Some(m) = self.mensajes.iter_mut().find(|m| m.id == id) {
            if para_todos && m.de == quien {
                m.texto.clear();
                m.borrado = true;
                m.reacciones.clear();
                if let Some(a) = m.archivo.take() {
                    let _ = std::fs::remove_file(a.ruta);
                }
            } else if !m.ocultos.contains(&quien) {
                m.ocultos.push(quien);
            }
            self.cambio = true;
        }
    }

    fn reacciona(&mut self, quien: usize, id: u64, emoji: &str) {
        let emoji: String = emoji.chars().take(8).collect();
        if let Some(m) = self.mensajes.iter_mut().find(|m| m.id == id) {
            if let Some(i) = m.reacciones.iter().position(|(q, _)| *q == quien) {
                if m.reacciones[i].1 == emoji {
                    m.reacciones.remove(i);
                } else {
                    m.reacciones[i].1 = emoji;
                }
            } else if !emoji.is_empty() {
                m.reacciones.push((quien, emoji));
            }
            self.cambio = true;
        }
    }

    /// The computer edits, deletes, reacts.
    pub fn editar(&mut self, id: u64, texto: &str) {
        self.edita(CASA, id, texto.to_owned());
    }

    /// Delete a message from the computer: for everyone (its own) or for itself.
    pub fn borrar(&mut self, id: u64, para_todos: bool) {
        self.borra(CASA, id, para_todos);
    }

    /// React from the computer.
    pub fn reaccionar(&mut self, id: u64, emoji: &str) {
        self.reacciona(CASA, id, emoji);
    }

    /// Everything goes from the computer's screen (the phones keep theirs).
    pub fn borrar_todo(&mut self) {
        for m in &mut self.mensajes {
            if !m.ocultos.contains(&CASA) {
                m.ocultos.push(CASA);
            }
        }
        self.cambio = true;
    }

    /// A file a phone finished sending: saved on the computer, a line in the room, and the
    /// bytes handed to the other phones (a picture shows in their chat).
    fn archivo_listo(&mut self, de: usize, nombre: String, mime: String, datos: Vec<u8>) {
        let Some(carpeta) = self.descargas.clone() else {
            return;
        };
        let nombre = nombre_limpio(&nombre);
        let _ = std::fs::create_dir_all(&carpeta);
        let ruta = {
            let primera = carpeta.join(&nombre);
            if primera.exists() {
                let (base, ext) = match nombre.rsplit_once('.') {
                    Some((b, e)) if !b.is_empty() => (b.to_owned(), format!(".{e}")),
                    _ => (nombre.clone(), String::new()),
                };
                (2..1000)
                    .map(|n| carpeta.join(format!("{base} ({n}){ext}")))
                    .find(|p| !p.exists())
                    .unwrap_or(primera)
            } else {
                primera
            }
        };
        if std::fs::write(&ruta, &datos).is_err() {
            return;
        }
        self.ultimo_archivo += 1;
        let archivo = Archivo {
            id: self.ultimo_archivo,
            nombre: nombre.clone(),
            mime: mime.clone(),
            ruta,
            bytes: datos.len() as u64,
        };
        let mut m = Mensaje::nuevo(de, String::new(), None);
        m.archivo = Some(archivo.clone());
        self.guarda(m);
        // Pictures go to the other phones at once; other files on demand (1.0.41).
        if mime.starts_with("image/") {
            for (j, a) in self.asientos.iter().enumerate() {
                if j != de && a.aprobado && a.enganchado {
                    reparte_archivo(&a.movil, &archivo, &datos);
                }
            }
        }
    }

    /// The call that just ended, as a line in the room.
    fn anota_llamada(&mut self, de: usize, video: bool, segundos: Option<u64>, fin: &'static str) {
        let mut m = Mensaje::nuevo(de, String::new(), None);
        m.llamada = Some(crate::mensaje::Llamada {
            video,
            saliente: true,
            segundos,
            fin,
        });
        self.guarda(m);
    }

    /// The computer's view of the room's messages (the last 300 it did not delete).
    fn mensajes_casa(&self) -> Vec<Value> {
        let mut v: Vec<Value> = self
            .mensajes
            .iter()
            .rev()
            .filter(|m| !m.ocultos.contains(&CASA))
            .take(300)
            .map(|m| self.mensaje_json(m, CASA))
            .collect();
        v.reverse();
        v
    }

    /// One message as seat `yo` (or the computer) sees it.
    fn mensaje_json(&self, m: &Mensaje, yo: usize) -> Value {
        let mio = m.de == yo;
        let visto_por_otro = m.vistos.iter().any(|&v| v != m.de);
        let mut v = json!({
            "id": m.id.to_string(),
            "t": m.t,
            "mio": mio,
            "de": if mio { String::new() } else { self.nombre_de(m.de) },
            "texto": m.texto,
            "estado": if !mio || visto_por_otro { "visto" } else { "entregado" },
        });
        if m.editado {
            v["editado"] = json!(true);
        }
        if m.borrado {
            v["borrado"] = json!(true);
        }
        if let Some(re) = m.re {
            v["re"] = json!(re.to_string());
        }
        if !m.reacciones.is_empty() {
            v["reacciones"] = json!(m
                .reacciones
                .iter()
                .map(|(q, e)| json!({ "de": if *q == yo { String::new() } else { self.nombre_de(*q) }, "emoji": e }))
                .collect::<Vec<_>>());
        }
        if let Some(a) = &m.archivo {
            v["archivo"] =
                json!({ "id": a.id, "nombre": a.nombre, "mime": a.mime, "bytes": a.bytes });
        }
        if let Some(l) = &m.llamada {
            v["llamada"] =
                json!({ "video": l.video, "saliente": mio, "segundos": l.segundos, "fin": l.fin });
        }
        v
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
            if let Some(l) = self.llamada.take() {
                self.anota_llamada(l.de, l.video, None, "sin_respuesta");
            }
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
                        // A phone that leaves ends its call (17 Oct 2026 review: the call
                        // stayed open and nobody in the house could call again).
                        if self
                            .llamada
                            .as_ref()
                            .is_some_and(|l| l.de == i || l.a == Some(i))
                        {
                            if let Some(l) = self.llamada.take() {
                                if l.a.is_some() {
                                    self.anota_llamada(
                                        l.de,
                                        l.video,
                                        Some(ahora_s().saturating_sub(l.desde)),
                                        "fin",
                                    );
                                } else {
                                    self.anota_llamada(l.de, l.video, None, "cancelada");
                                }
                            }
                            for a in &mut self.asientos {
                                a.mime = None;
                            }
                        }
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
                    Desde::TextoRe(t, re) if aprobado => self.mete_re(i, t, Some(re)),
                    Desde::Visto if aprobado => self.visto_por(i),
                    Desde::Editar(id, t) if aprobado => self.edita(i, id, t),
                    Desde::Borrar(id, todos) if aprobado => self.borra(i, id, todos),
                    Desde::Reaccion(id, e) if aprobado => self.reacciona(i, id, &e),
                    Desde::Escribiendo if aprobado => {
                        self.asientos[i].escribe_en = ahora_s();
                        self.cambio = true;
                    }
                    Desde::ArchivoEmpieza(nombre, bytes, mime) if aprobado => {
                        if bytes <= ARCHIVO_MAX {
                            self.asientos[i].archivo = Some((nombre, bytes, mime, Vec::new()));
                        }
                    }
                    Desde::ArchivoTrozo(b) if aprobado => {
                        if let Some(a) = &mut self.asientos[i].archivo {
                            if (a.3.len() + b.len()) as u64 <= a.1.max(1) {
                                a.3.extend_from_slice(&b);
                            } else {
                                self.asientos[i].archivo = None;
                            }
                        }
                    }
                    Desde::ArchivoFin if aprobado => {
                        if let Some((nombre, _, mime, datos)) = self.asientos[i].archivo.take() {
                            self.archivo_listo(i, nombre, mime, datos);
                        }
                    }
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
                            if let Some(l) = self.llamada.take() {
                                self.anota_llamada(l.de, l.video, None, "rechazada");
                            }
                            self.cambio = true;
                        }
                    }
                    Desde::Colgar if aprobado => {
                        if self
                            .llamada
                            .as_ref()
                            .is_some_and(|l| l.de == i || l.a == Some(i))
                        {
                            if let Some(l) = self.llamada.take() {
                                if l.a.is_some() {
                                    self.anota_llamada(
                                        l.de,
                                        l.video,
                                        Some(ahora_s().saturating_sub(l.desde)),
                                        "fin",
                                    );
                                } else {
                                    self.anota_llamada(l.de, l.video, None, "cancelada");
                                }
                            }
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
            .filter(|m| !m.ocultos.contains(&i))
            .map(|m| self.mensaje_json(m, i))
            .collect();
        let escribiendo = self
            .asientos
            .iter()
            .enumerate()
            .any(|(j, a)| j != i && a.aprobado && ahora_s().saturating_sub(a.escribe_en) < 6);
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
            "escribiendo": escribiendo,
            "archivos": true,
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
            "mensajes": self.mensajes_casa(),
            "escribiendo": self.asientos.iter().any(|a| a.aprobado && ahora_s().saturating_sub(a.escribe_en) < 6),
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
