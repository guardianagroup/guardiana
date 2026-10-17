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
    pub mensajes: Vec<crate::mensaje::Mensaje>,
    /// Pieces of the call for the page: (who, base64).
    pub media_para_pagina: VecDeque<(String, String)>,
    /// The call the host keeps going across members: with video?
    llamada: Option<bool>,
    /// Since when (unix s).
    llamada_desde: u64,
    /// The kind of recording this computer's page makes (MediaRecorder), for every link.
    mime: String,
    /// This computer's time zone, as the page says it.
    zona_propia: String,
    /// The live file for the globe: written, and when.
    directo_escrito: bool,
    directo_ultimo: u64,
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
    /// The one link that is talking, in a two-way talk. The host keeps a listener open for more
    /// members, which does not make it a group: its own lines must still share the link's id
    /// (ticks, edits, deleting for everyone) (17 Oct 2026 review).
    fn unica(&self) -> Option<usize> {
        if self.sesiones.len() == 1 {
            return Some(0);
        }
        let mut hablando = self.sesiones.iter().enumerate().filter(|(_, s)| s.lista());
        match (hablando.next(), hablando.next()) {
            (Some((i, _)), None) => Some(i),
            _ => None,
        }
    }

    /// Whether a call is going on in any link (the window then runs its clock faster, so the
    /// call's pieces do not wait for the next tick).
    pub fn en_llamada(&self) -> bool {
        self.llamada.is_some() || self.sesiones.iter().any(|s| s.llamada_activa().is_some())
    }

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
            llamada_desde: 0,
            mime: String::new(),
            zona_propia: String::new(),
            directo_escrito: false,
            directo_ultimo: 0,
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

    /// Invite on a port the system picks (the tests).
    pub fn escuchar_efimero(&mut self) -> Result<u16, String> {
        self.cierra_todo("nueva invitación");
        self.anfitrion = true;
        self.primera().escuchar_efimero()
    }

    /// Join someone's group with their code.
    pub fn conectar(&mut self, direccion: &str) -> Result<(), String> {
        self.cierra_todo("nueva conexión");
        self.anfitrion = false;
        self.primera().conectar(direccion)
    }

    fn cierra_todo(&mut self, por: &str) {
        // A new conversation starts empty (17 Oct 2026 review: the last contact's messages
        // showed in the next one's chat).
        for s in &mut self.sesiones {
            s.cerrar(por);
            s.borrar_todo();
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
        self.en_directo();
    }

    /// The page said this computer's time zone.
    pub fn pon_zona(&mut self, zona: &str) {
        self.zona_propia = zona.chars().take(64).collect();
        for s in &mut self.sesiones {
            s.pon_zona(zona);
        }
    }

    /// For the globe of this computer only (16 Oct 2026): a small file next to GUARDIANA's
    /// ledger says with whom this computer is talking and where each end is (by time zone,
    /// never by address), so «GUARDIANA por dentro» draws the line between the two. Written
    /// while a conversation is open, removed when it ends; it never leaves this computer.
    fn en_directo(&mut self) {
        let ahora = ahora_s();
        let abierta = self.sesiones.iter().any(Secure::lista);
        let ruta = guardiana_core::paths::data_dir().join("secure-en-directo.json");
        if !abierta {
            if self.directo_escrito {
                let _ = std::fs::remove_file(&ruta);
                self.directo_escrito = false;
            }
            return;
        }
        if ahora.saturating_sub(self.directo_ultimo) < 2 && !self.cambio {
            return;
        }
        self.directo_ultimo = ahora;
        let abiertas: Vec<Value> = self
            .sesiones
            .iter()
            .filter(|s| s.lista())
            .map(|s| {
                json!({
                    "contacto": s.contacto(),
                    "zona": s.zona_otra(),
                    "llamada": s.llamada_activa().map(|(video, desde)| json!({ "video": video, "desde": desde, "conectada": s.conectada() })),
                    "rele": s.por_rele(),
                })
            })
            .collect();
        let v = json!({
            "version": 1,
            "zona": self.zona_propia,
            "conversaciones": abiertas,
            "actualizado": ahora,
        });
        if std::fs::write(&ruta, v.to_string()).is_ok() {
            self.directo_escrito = true;
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
            let _ = self.sesiones[i].escuchar_grupo(Some(puerto), codigo_rele, false);
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
                // The host's list starts with the host; before it arrives, the host alone.
                if s.grupo().is_empty() {
                    v.push(s.contacto().to_owned());
                } else {
                    v.extend(s.grupo().iter().cloned());
                }
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
        let unica = self.unica();
        let yo = self.nombre_o_anfitrion();
        let mut pasar_texto: Vec<(usize, String, String)> = Vec::new();
        let mut pasar_media: Vec<(usize, String, Vec<u8>)> = Vec::new();
        for i in 0..n {
            let de_enlace = self.sesiones[i].contacto().to_owned();
            let desde = self.vistos[i];
            let nuevos: Vec<crate::mensaje::Mensaje> = self.sesiones[i]
                .mensajes
                .iter()
                .skip(desde)
                .cloned()
                .collect();
            self.vistos[i] = self.sesiones[i].mensajes.len();
            for mut m in nuevos {
                // Our own texts are pushed into the group's list when sent, once; our call and
                // file lines come from the link when only one is talking (in a group each link
                // would bring its own copy).
                if m.mio && !(unica == Some(i) && (m.llamada.is_some() || m.archivo.is_some())) {
                    continue;
                }
                let quien = if m.mio {
                    yo.clone()
                } else if m.de.is_empty() {
                    de_enlace.clone()
                } else {
                    m.de.clone()
                };
                m.de = quien.clone();
                let texto = m.texto.clone();
                let es_texto = m.llamada.is_none() && m.archivo.is_none();
                self.mensajes.push(m);
                if anfitrion && es_texto {
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
        // A member who joined while a call is on is called once; someone who hung up or
        // rejected it is not called again (16 Oct 2026: the host kept calling a member who had
        // hung up, over and over, and the screen shook).
        if let Some(video) = self.llamada {
            if anfitrion {
                for s in &mut self.sesiones {
                    if s.lista() && s.llamada_activa().is_none() && !s.llamado_en_grupo {
                        s.llamado_en_grupo = true;
                        let _ = s.llamar(video, &self.mime);
                    }
                }
            }
            // Nobody left in the call or ringing: it is over.
            if ahora_s().saturating_sub(self.llamada_desde) > 2
                && self
                    .sesiones
                    .iter()
                    .all(|s| s.llamada_activa().is_none() && s.suena().is_none())
            {
                self.llamada = None;
                for s in &mut self.sesiones {
                    s.llamado_en_grupo = false;
                }
                self.cambio = true;
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
            let mut m = crate::mensaje::Mensaje::mio(t, None);
            m.de = self.nombre_o_anfitrion();
            // In a two-way talk the link's own line is the one on screen (it carries the ticks).
            if let Some(i) = self.unica() {
                if let Some(ultimo) = self.sesiones[i].mensajes.last() {
                    m.id = ultimo.id;
                }
            }
            self.mensajes.push(m);
            for (i, s) in self.sesiones.iter().enumerate() {
                self.vistos[i] = s.mensajes.len();
            }
        }
        self.cambio = true;
    }

    /// The person answers a message (two-way talk: the link carries the quote).
    pub fn texto_re(&mut self, texto: &str, re: Option<u64>) {
        let Some(i) = self.unica() else {
            self.texto(texto);
            return;
        };
        let de = self.nombre_o_anfitrion();
        let s = &mut self.sesiones[i];
        if s.lista() {
            s.texto_re(texto, re);
            let ultimo = s.mensajes.last().cloned();
            let n = s.mensajes.len();
            if let Some(mut m) = ultimo {
                m.de = de;
                self.mensajes.push(m);
            }
            self.vistos[i] = n;
            self.cambio = true;
        }
    }

    /// Edit, delete, react, typing: to every link that has the message.
    pub fn editar(&mut self, id: u64, texto: &str) {
        for s in &mut self.sesiones {
            s.editar(id, texto);
        }
        if let Some(m) = self.mensajes.iter_mut().find(|m| m.id == id && m.mio) {
            m.edita(texto.chars().take(8000).collect());
        }
        self.cambio = true;
    }

    /// Delete one message, for everyone (ours) or here.
    pub fn borrar(&mut self, id: u64, para_todos: bool) {
        for s in &mut self.sesiones {
            s.borrar(id, para_todos);
        }
        if let Some(i) = self.mensajes.iter().position(|m| m.id == id) {
            if para_todos && self.mensajes[i].mio {
                self.mensajes[i].borra();
            } else {
                self.mensajes.remove(i);
            }
        }
        // «Delete for me» shortened the links' lists: count from where they are now, or the
        // next message would be skipped.
        for (i, s) in self.sesiones.iter().enumerate() {
            if let Some(v) = self.vistos.get_mut(i) {
                *v = (*v).min(s.mensajes.len());
            }
        }
        self.cambio = true;
    }

    /// The whole conversation goes from this screen.
    pub fn borrar_todo(&mut self) {
        for s in &mut self.sesiones {
            s.borrar_todo();
        }
        self.mensajes.clear();
        self.vistos.fill(0);
        self.cambio = true;
    }

    /// React with an emoji.
    pub fn reaccionar(&mut self, id: u64, emoji: &str) {
        for s in &mut self.sesiones {
            s.reaccionar(id, emoji);
        }
        if let Some(m) = self.mensajes.iter_mut().find(|m| m.id == id) {
            m.reacciona("", emoji);
        }
        self.cambio = true;
    }

    /// The person is typing.
    pub fn escribiendo(&mut self) {
        for s in &mut self.sesiones {
            s.escribiendo();
        }
    }

    /// Read receipts on or off.
    pub fn sin_visto(&mut self, sin: bool) {
        for s in &mut self.sesiones {
            s.sin_visto = sin;
        }
    }

    /// Start a call with everyone (the host) or with the host (a member).
    pub fn llamar(&mut self, video: bool, mime: &str) -> Result<(), String> {
        let mut r = Err("sec_no_lista".to_owned());
        if !mime.is_empty() {
            self.mime = mime.chars().take(80).collect();
        }
        for s in &mut self.sesiones {
            if s.lista() {
                s.llamado_en_grupo = true;
                r = s.llamar(video, &self.mime).and(Ok(()));
            }
        }
        if r.is_ok() {
            self.llamada = Some(video);
            self.llamada_desde = ahora_s();
        }
        self.cambio = true;
        r
    }

    /// The page records in another format now (it tells every link).
    pub fn cambia_mime(&mut self, mime: &str) {
        if mime.is_empty() {
            return;
        }
        self.mime = mime.chars().take(80).collect();
        for s in &mut self.sesiones {
            s.cambia_mime(mime);
        }
    }

    /// «Pasar al móvil»: only a two-person conversation (a group's call has several streams).
    pub fn movil_empieza(&mut self, idioma: &str) -> Result<(), String> {
        // Two people: one link talking. The host also keeps a session listening for the next
        // member; that one does not count (16 Oct 2026: the host was told «only two people»
        // with only two people in the conversation).
        let hablando: Vec<usize> = (0..self.sesiones.len())
            .filter(|&i| self.sesiones[i].lista())
            .collect();
        if self.nombres().len() > 2 || hablando.len() > 1 {
            return Err("sec_mv_solo_dos".to_owned());
        }
        // Nobody talking yet (16 Oct 2026, «conectar el móvil antes que el PC»): the phone
        // joins the first link, the one waiting for or looking for the other side.
        let i = hablando.first().copied().unwrap_or(0);
        match self.sesiones.get_mut(i) {
            Some(s) => s.movil_empieza(idioma),
            None => Err("sec_mv_solo_dos".to_owned()),
        }
    }

    /// A WebRTC message from the page: to the one link talking (a group call keeps the old
    /// way, so there is none to send to then).
    pub fn rtc(&mut self, d: &Value) {
        if let Some(i) = self.unica() {
            if let Some(s) = self.sesiones.get_mut(i) {
                s.rtc(d);
            }
        }
    }

    /// WebRTC messages from the other side for the page.
    pub fn rtc_para_pagina(&mut self) -> Vec<Value> {
        let mut v = Vec::new();
        for s in &mut self.sesiones {
            v.extend(s.rtc_para_pagina.drain(..));
        }
        v
    }

    /// Our new player needs the other side's pieces from a fresh start.
    pub fn pide_inicio(&mut self) {
        for s in &mut self.sesiones {
            s.pide_inicio();
        }
    }

    /// The person said «Sí, es mi móvil».
    pub fn movil_aprueba(&mut self) {
        for s in &mut self.sesiones {
            s.movil_aprueba();
        }
    }

    /// Back to the computer.
    pub fn movil_fin(&mut self) {
        for s in &mut self.sesiones {
            s.movil_fin();
        }
        self.cambio = true;
    }

    /// The phone gets the state after every change.
    pub fn movil_sincroniza(&self) {
        for s in &self.sesiones {
            s.movil_sincroniza();
        }
    }

    /// Hang up with everyone.
    pub fn colgar(&mut self) {
        for s in &mut self.sesiones {
            s.colgar();
            s.llamado_en_grupo = false;
        }
        self.llamada = None;
        self.cambio = true;
    }

    /// Answer the call ringing here (the host who answers a member brings the rest in).
    pub fn contestar(&mut self, mime: &str) -> Result<(), String> {
        if !mime.is_empty() {
            self.mime = mime.chars().take(80).collect();
        }
        let mut r = Ok(());
        let mut video = None;
        for s in &mut self.sesiones {
            if let Some(v) = s.suena() {
                video = Some(v);
                s.llamado_en_grupo = true;
                r = s.contestar(mime);
            }
        }
        if let (Some(v), true) = (video, self.anfitrion && self.sesiones.len() > 1) {
            self.llamada = Some(v);
            self.llamada_desde = ahora_s();
        }
        self.cambio = true;
        r
    }

    /// Close the «missed call» note.
    pub fn olvida_perdida(&mut self) {
        for s in &mut self.sesiones {
            s.olvida_perdida();
        }
        self.cambio = true;
    }

    /// Reject the call ringing here.
    pub fn rechazar(&mut self) {
        for s in &mut self.sesiones {
            s.rechazar();
        }
        self.cambio = true;
    }

    /// A file that arrived on any link.
    pub fn recibido(&self, id: u64) -> Option<crate::secure::Recibido> {
        self.sesiones.iter().find_map(|s| s.recibido(id).cloned())
    }

    /// Delete a file that arrived.
    pub fn borra_recibido(&mut self, id: u64) -> bool {
        let r = self.sesiones.iter_mut().any(|s| s.borra_recibido(id));
        self.cambio |= r;
        r
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
    pub fn archivo_empieza(&mut self, nombre: &str, bytes: u64) -> Result<(), String> {
        for s in &mut self.sesiones {
            if s.lista() {
                return s.archivo_empieza(nombre, bytes);
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

    /// The incident report of whichever link stopped, signed (none when nothing stopped).
    pub fn informe_incidente(&self) -> Option<String> {
        self.sesiones.iter().find_map(Secure::informe_incidente)
    }

    /// «Voz protegida» here, told to every member.
    pub fn voz_protegida(&mut self, activa: bool) {
        for s in &mut self.sesiones {
            s.voz_protegida(activa);
        }
        self.cambio = true;
    }

    /// The person closed the incident card.
    pub fn olvida_incidente(&mut self) {
        for s in &mut self.sesiones {
            s.olvida_incidente();
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
            s.visto();
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
                            .skip(1)
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
        // The link's own list carries the live state (ticks, edits, reactions) in a two-way
        // talk; in a group the copy here is shown, refreshed by id from the links.
        e["mensajes"] = json!(match self.sesiones.as_slice() {
            [s] => s
                .mensajes
                .iter()
                .rev()
                .take(300)
                .rev()
                .map(crate::mensaje::Mensaje::json)
                .collect::<Vec<_>>(),
            _ => self
                .mensajes
                .iter()
                .rev()
                .take(300)
                .rev()
                .map(|m| {
                    self.sesiones
                        .iter()
                        .find_map(|s| s.mensajes.iter().find(|x| x.id == m.id))
                        .map_or_else(
                            || m.json(),
                            |x| {
                                let mut j = x.json();
                                j["de"] = json!(m.de);
                                j
                            },
                        )
                })
                .collect::<Vec<_>>(),
        });
        e["escribiendo"] = json!(self.sesiones.iter().any(Secure::otro_escribiendo));
        e["grupo"] = json!({
            "anfitrion": anfitrion,
            "yo": self.nombre_o_anfitrion(),
            "nombres": nombres,
            "miembros": miembros,
            "max": MAX_MIEMBROS,
            "llamada": self.llamada,
        });
        e["contacto"] = json!(self.sesiones[0].contacto());
        // A phone on any member's side: the level shown is the group's lowest.
        e["otro_movil"] = json!(self.sesiones.iter().any(Secure::otro_en_movil));
        e["otro_voz_protegida"] = json!(self.sesiones.iter().any(Secure::otro_voz_protegida));
        // The post-quantum lock: shown only when every open link in the group has it.
        e["cuantico"] = json!(self.sesiones.iter().all(Secure::cuantico));
        // WebRTC only in a two-way talk: a group call goes the old way, through the host.
        if self.unica().is_none() {
            if let Some(l) = e["llamada"].as_object_mut() {
                l.remove("rtc");
            }
        }
        if let Some(video) = self.llamada {
            if e["llamada"].is_null() {
                e["llamada"] = json!({ "video": video, "desde": self.llamada_desde, "conectada": self.sesiones.iter().any(Secure::conectada) });
            }
        }
        if e["incidente"].is_null() {
            if let Some(i) = self
                .sesiones
                .iter()
                .map(Secure::estado_incidente)
                .find(|i| !i.is_null())
            {
                e["incidente"] = i;
            }
        }
        if e["perdida"].is_null() {
            if let Some(p) = self
                .sesiones
                .iter()
                .map(|s| s.estado_perdida())
                .find(|p| !p.is_null())
            {
                e["perdida"] = p;
            }
        }
        // A call ringing on any link, and the files that came on any link.
        if e["entrante"].is_null() {
            if let Some(s) = self.sesiones.iter().find(|s| s.suena().is_some()) {
                e["entrante"] = json!({ "video": s.suena(), "de": s.contacto() });
            }
        }
        let mut recibidos: Vec<Value> = self
            .sesiones
            .iter()
            .flat_map(|s| s.recibidos.iter().map(crate::secure::Recibido::json))
            .collect();
        recibidos.sort_by_key(|r| r["t"].as_u64().unwrap_or(0));
        e["recibidos"] = json!(recibidos);
        if e["envio"].is_null() {
            if let Some(v) = self
                .sesiones
                .iter()
                .map(Secure::envio_json)
                .find(|v| !v.is_null())
            {
                e["envio"] = v;
            }
        }
        let enviados: Vec<Value> = self
            .sesiones
            .iter()
            .flat_map(|s| {
                s.enviados.iter().filter(|x| x.t > 0).map(|x| {
                    json!({ "nombre": x.nombre, "t": x.t, "bytes": x.bytes, "llego_en": x.llego_en })
                })
            })
            .collect();
        e["enviados"] = json!(enviados);
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
        let puerto = a.escuchar_efimero().unwrap();
        // The phone may be in from the start (16 Oct 2026): nobody is talking yet, so the
        // hand-off goes to the link that waits for the other side, not «only two people».
        assert_eq!(a.movil_empieza("es"), Ok(()));
        assert!(a.estado_json()["movil"].is_object());
        b.conectar(&format!("127.0.0.1:{puerto}")).unwrap();
        gira(&mut [&mut a, &mut b], 25);
        assert!(a.numeros().is_some() && a.numeros() == b.numeros());
        a.movil_fin();
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
        assert_eq!(ultimo.texto, "hola a todos");
        assert_eq!(ultimo.de, "Bea");
        assert_eq!(a.mensajes.last().unwrap().de, "Bea");
        // Ana speaks: both read it.
        a.texto("bienvenidos");
        gira(&mut [&mut a, &mut b, &mut c], 20);
        assert_eq!(b.mensajes.last().unwrap().texto, "bienvenidos");
        assert_eq!(c.mensajes.last().unwrap().texto, "bienvenidos");

        // A call: a piece from Cris reaches Bea under Cris's name, through Ana.
        a.llamar(false, "").unwrap();
        gira(&mut [&mut a, &mut b, &mut c], 20);
        // It rings for both, until each answers.
        assert_eq!(b.sesiones[0].suena(), Some(false));
        assert_eq!(c.sesiones[0].suena(), Some(false));
        assert_eq!(b.estado_json()["entrante"]["video"], false);
        assert!(
            !a.sesiones.iter().any(Secure::conectada),
            "calling, not talking yet"
        );
        b.contestar("").unwrap();
        c.contestar("").unwrap();
        gira(&mut [&mut a, &mut b, &mut c], 20);
        assert!(b.sesiones[0].llamada_activa().is_some());
        assert!(c.sesiones[0].llamada_activa().is_some());
        assert!(a
            .sesiones
            .iter()
            .filter(|s| s.lista())
            .all(Secure::conectada));
        c.media_saliente(&B64.encode(b"pieza-de-cris")).unwrap();
        gira(&mut [&mut a, &mut b, &mut c], 20);
        let (de, b64) = b.media_para_pagina.back().cloned().unwrap();
        assert_eq!(de, "Cris");
        assert_eq!(B64.decode(b64).unwrap(), b"pieza-de-cris");
        let (de_a, _) = a.media_para_pagina.back().cloned().unwrap();
        assert_eq!(de_a, "Cris");

        // Cris hangs up: the host does not call Cris again (16 Oct 2026: it did, in a loop).
        c.colgar();
        gira(&mut [&mut a, &mut b, &mut c], 30);
        assert!(c.sesiones[0].llamada_activa().is_none());
        assert!(
            c.sesiones[0].suena().is_none(),
            "no new ring for whoever hung up"
        );
        assert!(b.sesiones[0].llamada_activa().is_some(), "the others go on");
        // Bea too: nobody left, the call is over for the host.
        b.colgar();
        std::thread::sleep(Duration::from_millis(2200));
        gira(&mut [&mut a, &mut b, &mut c], 30);
        assert!(
            a.estado_json()["llamada"].is_null(),
            "{}",
            a.estado_json()["llamada"]
        );
        let _ = std::fs::remove_dir_all(base);
    }
}
