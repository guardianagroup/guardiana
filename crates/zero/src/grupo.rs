//! Group conversations (Francisco, 15 Oct 2026: «que se puedan comunicar más personas para que
//! hagan una llamada grupal»): the one who invites is the host, and every other member talks to
//! the host over its own two-way link ([`Secure`]), verified with the host by the short code as
//! always. The host passes on what each one says, records or sends, to the rest, with the
//! original name. Each link keeps its own end-to-end cipher; the host is a member, so it reads
//! what members say — as any member does — and nobody else does, relay included.
//!
//! What a member knows about the others comes from the host («verificado por el anfitrión»):
//! the screen says so, instead of pretending every pair checked numbers.
//!
//! A two-way talk is a group of two; nothing changes for it.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde_json::{json, Value};

use crate::secure::Secure;

/// Members at most in this version (the host's upload is the limit: every piece of a call goes
/// to everyone else through it).
pub const MAX_MIEMBROS: usize = 6;
const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// One screen's worth of a group: the host's sessions, or a member's one session to the host.
pub struct Grupo {
    datos: PathBuf,
    descargas: PathBuf,
    version: String,
    /// The name others see for this computer (what the person typed; the host sends it).
    pub nombre_propio: String,
    /// The host's links, one per member; a member has exactly one, to the host.
    pub sesiones: Vec<Secure>,
    anfitrion: bool,
    /// How many messages of each session the group already took (to pass new ones on).
    vistos: Vec<usize>,
    /// The merged chat for the screen: (unix s, mine?, text, who).
    pub mensajes: Vec<(u64, bool, String, String)>,
    /// Pieces of the call for the page: (who, base64).
    pub media_para_pagina: VecDeque<(String, String)>,
    /// The call the host keeps going across members: with video?
    llamada: Option<bool>,
    cambio: bool,
}

impl std::fmt::Debug for Grupo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Grupo")
            .field("sesiones", &self.sesiones.len())
            .field("anfitrion", &self.anfitrion)
            .finish()
    }
}

impl Grupo {
    /// A group with one idle session (the identity and the book live in `datos/secure`).
    pub fn abre(datos: &Path, descargas: &Path) -> Self {
        Self {
            datos: datos.to_path_buf(),
            descargas: descargas.to_path_buf(),
            version: String::new(),
            nombre_propio: String::new(),
            sesiones: vec![Secure::abre(datos, descargas)],
            anfitrion: false,
            vistos: vec![0],
            mensajes: Vec::new(),
            media_para_pagina: VecDeque::new(),
            llamada: None,
            cambio: false,
        }
    }

    /// The program's version, for the signed sheets of every session.
    pub fn pon_version(&mut self, v: &str) {
        self.version = v.to_owned();
        for s in &mut self.sesiones {
            s.pon_version(v);
        }
    }

    fn nueva_sesion(&mut self) -> usize {
        let mut s = Secure::abre(&self.datos, &self.descargas);
        s.pon_version(&self.version);
        self.sesiones.push(s);
        self.vistos.push(0);
        self.sesiones.len() - 1
    }

    /// The first session: the one the page's connection card talks to.
    fn primera(&mut self) -> &mut Secure {
        if self.sesiones.is_empty() {
            self.nueva_sesion();
        }
        &mut self.sesiones[0]
    }

    /// Invite: listen as the host. More members come in through the same code.
    pub fn escuchar(&mut self, puerto: u16) -> Result<u16, String> {
        self.cierra_todo("nueva invitación");
        self.anfitrion = true;
        self.primera().escuchar(puerto)
    }

    /// Join someone's group with their code.
    pub fn conectar(&mut self, direccion: &str) -> Result<(), String> {
        self.cierra_todo("nueva conexión");
        self.anfitrion = false;
        self.primera().conectar(direccion)
    }

    fn cierra_todo(&mut self, por: &str) {
        for s in &mut self.sesiones {
            s.cerrar(por);
        }
        self.sesiones.truncate(1);
        self.vistos.truncate(1);
        self.vistos[0] = 0;
        self.mensajes.clear();
        self.llamada = None;
        self.cambio = true;
    }

    /// Close the conversation (every link).
    pub fn cerrar(&mut self, por: &str) {
        self.cierra_todo(por);
        self.anfitrion = false;
    }

    /// The session whose numbers are on screen (the newest one still comparing).
    fn comparando(&mut self) -> Option<&mut Secure> {
        self.sesiones
            .iter_mut()
            .rev()
            .find(|s| s.numeros().is_some())
    }

    /// The numbers to compare, for the member being verified right now.
    pub fn numeros(&self) -> Option<(u16, u16, u16)> {
        self.sesiones.iter().rev().find_map(Secure::numeros)
    }

    /// The person says the numbers match (or not).
    pub fn sas_persona(&mut self, iguales: bool, nombre: &str) {
        if let Some(s) = self.comparando() {
            s.sas_persona(iguales, nombre);
        }
        self.cambio = true;
    }

    /// Every tick: each session, then what the host passes on, then the next listener.
    pub fn tic(&mut self) {
        for s in &mut self.sesiones {
            s.tic();
        }
        self.reparte();
        if self.anfitrion {
            self.siguiente_escucha();
        }
        if self.sesiones.iter_mut().any(Secure::cambiado) {
            self.cambio = true;
        }
    }

    /// The host: once the newest session is talking (or gone), leave another one listening so
    /// the next member can come in with the same code.
    fn siguiente_escucha(&mut self) {
        let n = self.sesiones.len();
        if n >= MAX_MIEMBROS {
            return;
        }
        let (puerto, codigo_rele, _) = self.sesiones[0].escucha();
        if self.sesiones.last().is_some_and(Secure::lista) {
            let i = self.nueva_sesion();
            let _ = self.sesiones[i].escuchar_grupo(puerto, codigo_rele, false);
            self.cambio = true;
            self.manda_lista();
        }
        // A session whose member left: drop it (never the first, which holds the door and
        // the codes on screen), and tell the rest.
        let idos: Vec<usize> = self
            .sesiones
            .iter()
            .enumerate()
            .skip(1)
            .filter(|(_, s)| s.sola())
            .map(|(i, _)| i)
            .collect();
        if !idos.is_empty() {
            for i in idos.into_iter().rev() {
                self.sesiones.remove(i);
                self.vistos.remove(i);
            }
            self.manda_lista();
            self.cambio = true;
        }
    }

    /// Who is in: the host first, then every verified member, in order of arrival.
    pub fn nombres(&self) -> Vec<String> {
        let mut v = Vec::new();
        if self.anfitrion {
            v.push(self.nombre_o_anfitrion());
            v.extend(
                self.sesiones
                    .iter()
                    .filter(|s| s.lista())
                    .map(|s| s.contacto().to_owned()),
            );
        } else if let Some(s) = self.sesiones.first() {
            if s.lista() {
                v.push(s.contacto().to_owned());
                v.extend(s.grupo().iter().cloned());
            }
        }
        v
    }

    fn nombre_o_anfitrion(&self) -> String {
        if self.nombre_propio.trim().is_empty() {
            "Anfitrión".to_owned()
        } else {
            self.nombre_propio.trim().chars().take(60).collect()
        }
    }

    fn manda_lista(&mut self) {
        if !self.anfitrion {
            return;
        }
        // Each member gets everyone but themselves, with the host first.
        let todos: Vec<String> = self
            .sesiones
            .iter()
            .filter(|s| s.lista())
            .map(|s| s.contacto().to_owned())
            .collect();
        let yo = self.nombre_o_anfitrion();
        for s in &mut self.sesiones {
            if !s.lista() {
                continue;
            }
            let mut lista = vec![yo.clone()];
            lista.extend(todos.iter().filter(|n| *n != s.contacto()).cloned());
            s.grupo_lista(&lista);
        }
    }

    /// What arrived on each link since the last tick: on screen, and (the host) on to the rest.
    fn reparte(&mut self) {
        let n = self.sesiones.len();
        let anfitrion = self.anfitrion;
        let mut pasar_texto: Vec<(usize, String, String)> = Vec::new();
        let mut pasar_media: Vec<(usize, String, Vec<u8>)> = Vec::new();
        for i in 0..n {
            let de_enlace = self.sesiones[i].contacto().to_owned();
            let desde = self.vistos[i];
            let nuevos: Vec<(u64, bool, String, String)> = self.sesiones[i]
                .mensajes
                .iter()
                .skip(desde)
                .cloned()
                .collect();
            self.vistos[i] = self.sesiones[i].mensajes.len();
            for (t, mio, texto, de) in nuevos {
                if mio {
                    // Our own texts are pushed into the group's list when sent, once.
                    continue;
                }
                let quien = if de.is_empty() { de_enlace.clone() } else { de };
                self.mensajes.push((t, false, texto.clone(), quien.clone()));
                if anfitrion {
                    pasar_texto.push((i, quien, texto));
                }
                self.cambio = true;
            }
            let piezas: Vec<(String, String)> =
                self.sesiones[i].media_para_pagina.drain(..).collect();
            for (de, b64) in piezas {
                let quien = if de.is_empty() { de_enlace.clone() } else { de };
                if anfitrion {
                    if let Ok(bytes) = B64.decode(&b64) {
                        pasar_media.push((i, quien.clone(), bytes));
                    }
                }
                self.media_para_pagina.push_back((quien, b64));
            }
        }
        if self.media_para_pagina.len() > 600 {
            let sobra = self.media_para_pagina.len() - 600;
            self.media_para_pagina.drain(..sobra);
        }
        for (origen, quien, texto) in pasar_texto {
            for (j, s) in self.sesiones.iter_mut().enumerate() {
                if j != origen && s.lista() {
                    s.texto_de(&quien, &texto);
                }
            }
        }
        for (origen, quien, bytes) in pasar_media {
            for (j, s) in self.sesiones.iter_mut().enumerate() {
                if j != origen && s.lista() {
                    let _ = s.media_de(&quien, &bytes);
                }
            }
        }
        // A member joined while a call is on: bring them in.
        if let Some(video) = self.llamada {
            if anfitrion {
                for s in &mut self.sesiones {
                    if s.lista() && s.llamada_activa().is_none() {
                        let _ = s.llamar(video);
                    }
                }
            }
        }
    }

    /// The person wrote something: to everyone.
    pub fn texto(&mut self, texto: &str) {
        let t: String = texto.chars().take(8000).collect();
        let mut enviado = false;
        for s in &mut self.sesiones {
            if s.lista() {
                s.texto(&t);
                enviado = true;
            }
        }
        if enviado {
            self.mensajes
                .push((ahora_s(), true, t, self.nombre_o_anfitrion()));
            for (i, s) in self.sesiones.iter().enumerate() {
                self.vistos[i] = s.mensajes.len();
            }
        }
        self.cambio = true;
    }

    /// Start a call with everyone (the host) or with the host (a member).
    pub fn llamar(&mut self, video: bool) -> Result<(), String> {
        let mut r = Err("sec_no_lista".to_owned());
        for s in &mut self.sesiones {
            if s.lista() {
                r = s.llamar(video).and(Ok(()));
            }
        }
        if r.is_ok() {
            self.llamada = Some(video);
        }
        self.cambio = true;
        r
    }

    /// Hang up with everyone.
    pub fn colgar(&mut self) {
        for s in &mut self.sesiones {
            s.colgar();
        }
        self.llamada = None;
        self.cambio = true;
    }

    /// One recorded piece from the page: to every link.
    pub fn media_saliente(&mut self, b64: &str) -> Result<(), String> {
        let mut r = Ok(());
        for s in &mut self.sesiones {
            if s.llamada_activa().is_some() {
                r = r.and(s.media_saliente(b64));
            }
        }
        r
    }

    /// A file: to the first talking link (in a group, the host; the host passes files on later).
    pub fn archivo_empieza(&mut self, nombre: &str) -> Result<(), String> {
        for s in &mut self.sesiones {
            if s.lista() {
                return s.archivo_empieza(nombre);
            }
        }
        Err("sec_no_lista".to_owned())
    }

    /// One piece of the file.
    pub fn archivo_trozo(&mut self, b64: &str, ultimo: bool) -> Result<(), String> {
        for s in &mut self.sesiones {
            if s.lista() {
                return s.archivo_trozo(b64, ultimo);
            }
        }
        Err("sec_no_lista".to_owned())
    }

    /// The person says what was seen is theirs: every link goes on.
    pub fn reanudar_a_mano(&mut self) {
        for s in &mut self.sesiones {
            s.reanudar_a_mano();
        }
        self.cambio = true;
    }

    /// The chain of what was seen on the first link, signed.
    pub fn prueba_json(&self) -> String {
        self.sesiones
            .first()
            .map(Secure::prueba_json)
            .unwrap_or_default()
    }

    /// Whether the screen must be redrawn, and clear the flag.
    pub fn cambiado(&mut self) -> bool {
        std::mem::take(&mut self.cambio)
    }

    /// Something happened while the tab was not in front.
    pub fn novedad(&self) -> bool {
        self.sesiones.iter().any(|s| s.novedad)
    }

    /// The tab was brought to the front.
    pub fn visto(&mut self) {
        for s in &mut self.sesiones {
            s.novedad = false;
        }
    }

    /// Lines for the page's event list, from every link.
    pub fn avisos(&mut self) -> Vec<Value> {
        let mut v = Vec::new();
        for s in &mut self.sesiones {
            while let Some(a) = s.avisos.pop_front() {
                v.push(a);
            }
        }
        v
    }

    /// Everything the page draws: the first link's state, with the group's chat and members.
    pub fn estado_json(&self) -> Value {
        let anfitrion = self.anfitrion;
        let nombres = self.nombres();
        let miembros: Vec<Value> = if anfitrion {
            self.sesiones
                .iter()
                .filter(|s| s.lista())
                .map(|s| json!({ "nombre": s.contacto(), "verificado_por": "yo" }))
                .collect()
        } else {
            self.sesiones
                .first()
                .filter(|s| s.lista())
                .map(|s| {
                    let mut v = vec![json!({ "nombre": s.contacto(), "verificado_por": "yo" })];
                    v.extend(
                        s.grupo()
                            .iter()
                            .map(|n| json!({ "nombre": n, "verificado_por": s.contacto() })),
                    );
                    v
                })
                .unwrap_or_default()
        };
        // The first link carries the codes, the door and the monitor; a newer link being set
        // up (a member comparing numbers with the host) is shown on top of the chat.
        let mut e = self.sesiones[0].estado_json();
        if let Some(nueva) = self
            .sesiones
            .iter()
            .skip(1)
            .rev()
            .find(|s| s.numeros().is_some())
        {
            e["comparando"] = json!(nueva.numeros().map(|(a, b, c)| vec![a, b, c]));
            e["contacto_nuevo"] = json!(nueva.contacto());
        }
        let (_, codigo_rele, codigo_puerta) = self.sesiones[0].escucha();
        if anfitrion && self.sesiones.iter().any(Secure::lista) {
            e["codigo_grupo"] = json!(codigo_puerta.or(codigo_rele));
        }
        e["mensajes"] = json!(self
            .mensajes
            .iter()
            .rev()
            .take(300)
            .rev()
            .map(|(t, mio, x, de)| json!({ "t": t, "mio": mio, "texto": x, "de": de }))
            .collect::<Vec<_>>());
        e["grupo"] = json!({
            "anfitrion": anfitrion,
            "yo": self.nombre_o_anfitrion(),
            "nombres": nombres,
            "miembros": miembros,
            "max": MAX_MIEMBROS,
            "llamada": self.llamada,
        });
        e["contacto"] = json!(self.sesiones[0].contacto());
        if let Some(video) = self.llamada {
            if e["llamada"].is_null() {
                e["llamada"] = json!({ "video": video, "desde": 0 });
            }
        }
        e
    }
}

fn ahora_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn gira(g: &mut [&mut Grupo], veces: u32) {
        for _ in 0..veces {
            for x in g.iter_mut() {
                x.tic();
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn three_computers_meet_through_the_host_and_everyone_reads_everyone() {
        let base = std::env::temp_dir().join(format!("guardiana-grupo-{}", std::process::id()));
        let d = |n: &str| {
            let p = base.join(n);
            std::fs::create_dir_all(&p).unwrap();
            p
        };
        let (da, db, dc) = (d("a"), d("b"), d("c"));
        let mut a = Grupo::abre(&da, &da);
        a.nombre_propio = "Ana".to_owned();
        let mut b = Grupo::abre(&db, &db);
        let mut c = Grupo::abre(&dc, &dc);
        let puerto = a.escuchar(0).unwrap();
        b.conectar(&format!("127.0.0.1:{puerto}")).unwrap();
        gira(&mut [&mut a, &mut b], 25);
        assert!(a.numeros().is_some() && a.numeros() == b.numeros());
        a.sas_persona(true, "Bea");
        b.sas_persona(true, "Ana");
        gira(&mut [&mut a, &mut b], 25);
        assert!(a.sesiones[0].lista() && b.sesiones[0].lista());
        // The host left a second listener on the same port for the next member.
        gira(&mut [&mut a, &mut b], 5);
        assert_eq!(a.sesiones.len(), 2, "a second session listens");
        c.conectar(&format!("127.0.0.1:{puerto}")).unwrap();
        gira(&mut [&mut a, &mut b, &mut c], 25);
        assert!(
            a.numeros().is_some() && a.numeros() == c.numeros(),
            "{:?}",
            a.estado_json()["fase"]
        );
        a.sas_persona(true, "Cris");
        c.sas_persona(true, "Ana");
        gira(&mut [&mut a, &mut b, &mut c], 30);
        assert!(c.sesiones[0].lista());
        assert_eq!(a.nombres(), vec!["Ana", "Bea", "Cris"]);
        assert_eq!(b.nombres(), vec!["Ana", "Cris"]);
        assert_eq!(c.nombres(), vec!["Ana", "Bea"]);

        // Bea speaks: Ana (the host) and Cris read it, with Bea's name.
        b.texto("hola a todos");
        gira(&mut [&mut a, &mut b, &mut c], 20);
        let ultimo = c.mensajes.last().unwrap();
        assert_eq!(ultimo.2, "hola a todos");
        assert_eq!(ultimo.3, "Bea");
        assert_eq!(a.mensajes.last().unwrap().3, "Bea");
        // Ana speaks: both read it.
        a.texto("bienvenidos");
        gira(&mut [&mut a, &mut b, &mut c], 20);
        assert_eq!(b.mensajes.last().unwrap().2, "bienvenidos");
        assert_eq!(c.mensajes.last().unwrap().2, "bienvenidos");

        // A call: a piece from Cris reaches Bea under Cris's name, through Ana.
        a.llamar(false).unwrap();
        gira(&mut [&mut a, &mut b, &mut c], 20);
        assert!(b.sesiones[0].llamada_activa().is_some());
        assert!(c.sesiones[0].llamada_activa().is_some());
        c.media_saliente(&B64.encode(b"pieza-de-cris")).unwrap();
        gira(&mut [&mut a, &mut b, &mut c], 20);
        let (de, b64) = b.media_para_pagina.back().cloned().unwrap();
        assert_eq!(de, "Cris");
        assert_eq!(B64.decode(b64).unwrap(), b"pieza-de-cris");
        let (de_a, _) = a.media_para_pagina.back().cloned().unwrap();
        assert_eq!(de_a, "Cris");
        let _ = std::fs::remove_dir_all(base);
    }
}
