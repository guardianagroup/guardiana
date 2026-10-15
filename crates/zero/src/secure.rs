//! GUARDIANA SECURE inside the browser: one conversation between two computers, over a plain
//! TCP link (the same network, or a port the person opened), everything inside Olm
//! (`guardiana_secure`). First version, 15 Oct 2026, asked for «hoy»: messages and files, with
//! the short-code check before anything is said, and the monitor the whole time.
//!
//! What travels on the wire is frames: four bytes of length, then JSON. Only three kinds carry
//! anything meaningful, and none of them is readable without the keys:
//! - `hola`: this computer's signed card (public keys only);
//! - `sas`, `sas_mac`: the short-code handshake (`verificar`);
//! - `otk`, `olm`: a one-time key and Olm envelopes (every message and every file's key);
//! - `bloque`: a file block, encrypted with the key that went inside Olm.
//!
//! The address the person types is used to connect and shown nowhere afterwards; nothing here
//! writes an address to disk (decision 203).

use std::collections::VecDeque;
use std::io::{Read as _, Write as _};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs as _};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use guardiana_secure::agenda::{self, Agenda};
use guardiana_secure::archivo::{Llave, Recepcion, Trozos};
use guardiana_secure::claves::{Curve25519PublicKey, Ed25519Signature, Mac};
use guardiana_secure::monitor::{Identidad, Monitor, Nivel};
use guardiana_secure::pausa::Sesion as EstadoSesion;
use guardiana_secure::prueba::Cadena;
use guardiana_secure::verificar::{Codigo, Inicio, Verified};
use guardiana_secure::{tarjeta, Conversation, Envelope, Identity};
use serde_json::{json, Value};

const MAX_FRAME: usize = 2 * 1024 * 1024;
const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// What the network thread tells the session.
#[derive(Debug)]
pub enum Entrante {
    /// Someone connected to our listener: the socket to adopt.
    Aceptado(TcpStream),
    /// The link is up (we accepted or connected).
    Conectado,
    /// A frame arrived.
    Trama(Value),
    /// The link went down, with why (never an address).
    Cerrado(String),
}

/// One link to the other computer: a writer half for us, a reader thread for what comes.
struct Enlace {
    escritor: Arc<Mutex<TcpStream>>,
}

impl Enlace {
    fn manda(&self, v: &Value) -> Result<(), String> {
        let cuerpo = serde_json::to_vec(v).map_err(|e| e.to_string())?;
        if cuerpo.len() > MAX_FRAME {
            return Err("trama demasiado grande".to_owned());
        }
        let mut w = self.escritor.lock().map_err(|_| "enlace".to_owned())?;
        w.write_all(&(cuerpo.len() as u32).to_be_bytes())
            .and_then(|()| w.write_all(&cuerpo))
            .and_then(|()| w.flush())
            .map_err(|e| e.to_string())
    }
}

fn lee_tramas(mut s: TcpStream, tx: Sender<Entrante>) {
    let _ = s.set_read_timeout(None);
    let _ = tx.send(Entrante::Conectado);
    let mut largo = [0u8; 4];
    loop {
        if s.read_exact(&mut largo).is_err() {
            let _ = tx.send(Entrante::Cerrado("cerrado".to_owned()));
            return;
        }
        let n = u32::from_be_bytes(largo) as usize;
        if n > MAX_FRAME {
            let _ = tx.send(Entrante::Cerrado("trama demasiado grande".to_owned()));
            return;
        }
        let mut cuerpo = vec![0u8; n];
        if s.read_exact(&mut cuerpo).is_err() {
            let _ = tx.send(Entrante::Cerrado("cerrado".to_owned()));
            return;
        }
        match serde_json::from_slice::<Value>(&cuerpo) {
            Ok(v) => {
                if tx.send(Entrante::Trama(v)).is_err() {
                    return;
                }
            }
            Err(_) => {
                let _ = tx.send(Entrante::Cerrado("trama ilegible".to_owned()));
                return;
            }
        }
    }
}

/// Where the conversation is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fase {
    /// Nobody on the other end.
    Sola,
    /// Listening for someone.
    Escuchando,
    /// Link up; cards exchanged or about to be.
    Saludando,
    /// Both ephemeral keys known: the numbers are on screen, waiting for the person.
    Comparando,
    /// Verified; Olm session being started.
    Abriendo,
    /// Talking.
    Lista,
}

/// A file coming in: the blocks may arrive before the key does.
struct Entrada {
    nombre: String,
    bloques_pendientes: VecDeque<(u64, Vec<u8>)>,
    recepcion: Option<Recepcion>,
    llave: Option<Llave>,
    archivo: Option<std::fs::File>,
    ruta: Option<PathBuf>,
}

/// A file going out, block by block as the page hands them over.
struct Salida {
    nombre: String,
    trozos: Trozos,
    bytes: u64,
}

/// The SECURE side of a browser session.
pub struct Secure {
    datos: PathBuf,
    descargas: PathBuf,
    identidad: Identity,
    agenda: Agenda,
    nombre_contacto: String,
    fase: Fase,
    enlace: Option<Enlace>,
    rx: Option<Receiver<Entrante>>,
    puerto: u16,
    tarjeta_otra: Option<tarjeta::Tarjeta>,
    sas_inicio: Option<Inicio>,
    sas_codigo: Option<Codigo>,
    sas_yo_ok: bool,
    sas_mac_otra: Option<(Mac, Ed25519Signature)>,
    verificado: Option<Verified>,
    conversacion: Option<Conversation>,
    /// Messages for the screen: (unix s, mine?, text).
    pub mensajes: Vec<(u64, bool, String)>,
    /// What the screen shows the whole time.
    pub monitor: Monitor,
    /// The chain of what was seen, for the signed evidence.
    pub cadena: Cadena,
    /// Running, or stopped with its reason.
    pub estado: EstadoSesion,
    entrada: Option<Entrada>,
    salida: Option<Salida>,
    inicio: u64,
    /// Lines for the page's event list, drained by the session.
    pub avisos: VecDeque<Value>,
    cambio: bool,
}

impl std::fmt::Debug for Secure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Secure")
            .field("fase", &self.fase)
            .field("contacto", &self.nombre_contacto)
            .finish()
    }
}

fn ahora_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Secure {
    /// Open or create this computer's identity and book under `datos/secure`.
    pub fn abre(datos: &Path, descargas: &Path) -> Self {
        let carpeta = datos.join("secure");
        let _ = std::fs::create_dir_all(&carpeta);
        let clave = lee_o_crea_clave(&carpeta.join("clave.bin"));
        let identidad = std::fs::read_to_string(carpeta.join("identidad.txt"))
            .ok()
            .and_then(|s| Identity::unlock(s.trim(), &clave).ok())
            .unwrap_or_else(|| {
                let i = Identity::new();
                let _ = std::fs::write(carpeta.join("identidad.txt"), i.lock(&clave));
                i
            });
        let agenda = std::fs::read_to_string(carpeta.join("agenda.json"))
            .ok()
            .and_then(|s| Agenda::from_json(&s).ok())
            .unwrap_or_default();
        let mut s = Self {
            datos: carpeta,
            descargas: descargas.to_path_buf(),
            identidad,
            agenda,
            nombre_contacto: String::new(),
            fase: Fase::Sola,
            enlace: None,
            rx: None,
            puerto: 0,
            tarjeta_otra: None,
            sas_inicio: None,
            sas_codigo: None,
            sas_yo_ok: false,
            sas_mac_otra: None,
            verificado: None,
            conversacion: None,
            mensajes: Vec::new(),
            monitor: Monitor::new(),
            cadena: Cadena::new("sin sesion", ahora_s()),
            estado: EstadoSesion::default(),
            entrada: None,
            salida: None,
            inicio: ahora_s(),
            avisos: VecDeque::new(),
            cambio: true,
        };
        s.monitor
            .anota(ahora_s(), Nivel::Info, "identidad lista".to_owned());
        s
    }

    fn guarda_agenda(&self) {
        let _ = std::fs::write(self.datos.join("agenda.json"), self.agenda.to_json());
    }

    /// This computer's card, the text for a QR.
    pub fn tarjeta(&self) -> String {
        self.identidad.tarjeta(ahora_s())
    }

    /// Short form of this computer's signing key.
    pub fn huella_propia(&self) -> String {
        agenda::huella_corta(self.identidad.signing_key())
    }

    fn aviso(&mut self, clave: &str, texto: impl Into<String>) {
        self.avisos
            .push_back(json!({ "clave": clave, "texto": texto.into() }));
        self.cambio = true;
    }

    /// Start listening on a port (0 lets the system choose). The address is never shown.
    pub fn escuchar(&mut self, puerto: u16) -> Result<u16, String> {
        self.cerrar("nueva escucha");
        let listener = TcpListener::bind(("0.0.0.0", puerto)).map_err(|e| e.to_string())?;
        let puerto = listener.local_addr().map_err(|e| e.to_string())?.port();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            if let Ok((s, _)) = listener.accept() {
                let _ = tx.send(Entrante::Aceptado(s));
            }
        });
        self.rx = Some(rx);
        self.puerto = puerto;
        self.fase = Fase::Escuchando;
        self.cambio = true;
        Ok(puerto)
    }

    /// Connect to the other computer. `direccion` is `host:puerto`; it is used now and kept
    /// nowhere.
    pub fn conectar(&mut self, direccion: &str) -> Result<(), String> {
        self.cerrar("nueva conexión");
        let addr: SocketAddr = direccion
            .trim()
            .to_socket_addrs()
            .map_err(|e| e.to_string())?
            .next()
            .ok_or_else(|| "dirección vacía".to_owned())?;
        let s =
            TcpStream::connect_timeout(&addr, Duration::from_secs(8)).map_err(|e| e.to_string())?;
        self.adopta(s);
        Ok(())
    }

    fn adopta(&mut self, s: TcpStream) {
        let _ = s.set_nodelay(true);
        let (tx, rx) = mpsc::channel();
        let lector = match s.try_clone() {
            Ok(l) => l,
            Err(e) => {
                self.aviso("sec_err_enlace", e.to_string());
                return;
            }
        };
        std::thread::spawn(move || lee_tramas(lector, tx));
        self.enlace = Some(Enlace {
            escritor: Arc::new(Mutex::new(s)),
        });
        self.rx = Some(rx);
        self.fase = Fase::Saludando;
        self.cambio = true;
    }

    /// Drop the link and forget the handshake; the identity and the book stay.
    pub fn cerrar(&mut self, por: &str) {
        self.enlace = None;
        self.rx = None;
        self.tarjeta_otra = None;
        self.sas_inicio = None;
        self.sas_codigo = None;
        self.sas_yo_ok = false;
        self.sas_mac_otra = None;
        self.verificado = None;
        self.conversacion = None;
        self.entrada = None;
        self.salida = None;
        if self.fase != Fase::Sola {
            self.monitor
                .anota(ahora_s(), Nivel::Info, format!("enlace cerrado: {por}"));
        }
        self.fase = Fase::Sola;
        self.monitor.identidad = None;
        self.cambio = true;
    }

    fn manda(&mut self, v: Value) {
        if let Some(e) = &self.enlace {
            if let Err(err) = e.manda(&v) {
                self.aviso("sec_err_enlace", err);
                self.cerrar("fallo al enviar");
            }
        }
    }

    /// Called every tick: everything the network thread left, and the handshake that follows.
    pub fn tic(&mut self) {
        let mut pendientes = Vec::new();
        if let Some(rx) = &self.rx {
            while let Ok(e) = rx.try_recv() {
                pendientes.push(e);
            }
        }
        for e in pendientes {
            match e {
                Entrante::Aceptado(s) => self.adopta(s),
                Entrante::Conectado => self.saluda(),
                Entrante::Trama(v) => self.trama(v),
                Entrante::Cerrado(por) => {
                    if self.fase != Fase::Sola {
                        self.aviso("sec_enlace_cerrado", por.clone());
                        self.cerrar(&por);
                    }
                }
            }
        }
    }

    fn saluda(&mut self) {
        self.fase = Fase::Saludando;
        let t = self.tarjeta();
        self.manda(json!({ "t": "hola", "tarjeta": t }));
        self.cambio = true;
    }

    fn trama(&mut self, v: Value) {
        let t = v["t"].as_str().unwrap_or_default().to_owned();
        match t.as_str() {
            "hola" => self.hola(&v),
            "sas" => self.sas(&v),
            "sas_mac" => self.sas_mac(&v),
            "otk" => self.otk(&v),
            "olm" => self.olm(&v),
            "bloque" => self.bloque(&v),
            _ => {}
        }
    }

    fn hola(&mut self, v: &Value) {
        let Some(texto) = v["tarjeta"].as_str() else {
            return;
        };
        let Ok(t) = tarjeta::lee(texto) else {
            self.aviso("sec_tarjeta_mala", "");
            self.cerrar("tarjeta sin firma válida");
            return;
        };
        if t.signing() == self.identidad.signing_key() {
            self.cerrar("es este mismo ordenador");
            return;
        }
        self.tarjeta_otra = Some(t);
        // Pinned already under some name? Then the keys must be those.
        if let Some(c) = self
            .agenda
            .contactos()
            .iter()
            .find(|c| c.signing == t.signing().to_base64())
            .cloned()
        {
            self.nombre_contacto = c.nombre.clone();
        }
        if !self.nombre_contacto.is_empty() {
            let n = self.nombre_contacto.clone();
            match self.agenda.comprueba(&n, t.curve(), t.signing(), ahora_s()) {
                Ok(()) => {
                    if let Some(v) = self
                        .agenda
                        .contacto(&n)
                        .and_then(agenda::Contacto::verified)
                    {
                        self.verificado = Some(v);
                        self.abre_olm();
                        return;
                    }
                }
                Err(_) => {
                    let n = self.nombre_contacto.clone();
                    self.estado.parar(
                        guardiana_secure::pausa::Motivo::ClaveCambiada { nombre: n },
                        ahora_s(),
                        &mut self.monitor,
                        &mut self.cadena,
                    );
                }
            }
        }
        // Not known (or changed): the short code.
        let inicio = Inicio::new();
        let pk = inicio.public_key().to_base64();
        self.sas_inicio = Some(inicio);
        self.manda(json!({ "t": "sas", "clave": pk }));
        self.fase = Fase::Comparando;
        self.cambio = true;
    }

    fn sas(&mut self, v: &Value) {
        let Some(k) = v["clave"]
            .as_str()
            .and_then(|s| Curve25519PublicKey::from_base64(s).ok())
        else {
            return;
        };
        if self.sas_inicio.is_none() {
            // The other side spoke first: answer with ours.
            let inicio = Inicio::new();
            let pk = inicio.public_key().to_base64();
            self.sas_inicio = Some(inicio);
            self.manda(json!({ "t": "sas", "clave": pk }));
        }
        if let Some(inicio) = self.sas_inicio.take() {
            match inicio.establish(k) {
                Ok(c) => {
                    self.sas_codigo = Some(c);
                    self.fase = Fase::Comparando;
                }
                Err(_) => self.cerrar("código corto fallido"),
            }
        }
        self.cambio = true;
    }

    /// The numbers the person compares, once both ephemeral keys are known.
    pub fn numeros(&self) -> Option<(u16, u16, u16)> {
        self.sas_codigo.as_ref().map(Codigo::decimals)
    }

    /// The person says the numbers match (or not).
    pub fn sas_persona(&mut self, iguales: bool, nombre: &str) {
        if !iguales {
            self.aviso("sec_sas_no", "");
            self.cerrar("los números no coincidían");
            return;
        }
        let Some(c) = &self.sas_codigo else { return };
        if !nombre.trim().is_empty() {
            self.nombre_contacto = nombre.trim().chars().take(60).collect();
        }
        let mac = c.mac(self.identidad.signing_key(), self.identidad.public_key());
        let firma = self
            .identidad
            .sign(&c.transcript(self.identidad.signing_key(), self.identidad.public_key()));
        self.sas_yo_ok = true;
        self.manda(json!({ "t": "sas_mac", "mac": mac.to_base64(), "firma": firma.to_base64() }));
        self.confirma_si_puedo();
    }

    fn sas_mac(&mut self, v: &Value) {
        let (Some(mac), Some(firma)) = (
            v["mac"].as_str().and_then(|s| Mac::from_base64(s).ok()),
            v["firma"]
                .as_str()
                .and_then(|s| Ed25519Signature::from_base64(s).ok()),
        ) else {
            return;
        };
        self.sas_mac_otra = Some((mac, firma));
        self.confirma_si_puedo();
    }

    fn confirma_si_puedo(&mut self) {
        if !self.sas_yo_ok {
            return;
        }
        let (Some(c), Some(t), Some((mac, firma))) =
            (&self.sas_codigo, &self.tarjeta_otra, &self.sas_mac_otra)
        else {
            return;
        };
        match c.confirm(t.curve(), t.signing(), mac, firma, true) {
            Ok(verificado) => {
                if self.nombre_contacto.is_empty() {
                    self.nombre_contacto = agenda::huella_corta(t.signing());
                }
                let n = self.nombre_contacto.clone();
                self.agenda.fijar(&n, verificado, ahora_s());
                self.guarda_agenda();
                self.verificado = Some(verificado);
                self.sas_codigo = None;
                self.sas_mac_otra = None;
                self.sas_yo_ok = false;
                if !self.estado.en_marcha() {
                    self.estado
                        .reverificada(ahora_s(), &mut self.monitor, &mut self.cadena);
                }
                self.monitor.anota(
                    ahora_s(),
                    Nivel::Info,
                    format!("verificación con {n} hecha"),
                );
                self.abre_olm();
            }
            Err(_) => {
                self.aviso("sec_sas_fallo", "");
                self.cerrar("la verificación no cuadró");
            }
        }
    }

    fn abre_olm(&mut self) {
        let Some(v) = self.verificado else { return };
        self.fase = Fase::Abriendo;
        let yo = self.identidad.public_key().to_bytes();
        let otro = v.curve().to_bytes();
        // One side asks for a one-time key and starts; the other answers. Decided by the keys.
        if yo < otro {
            self.manda(json!({ "t": "otk" }));
        }
        self.pinta_identidad();
        self.cambio = true;
    }

    fn pinta_identidad(&mut self) {
        let Some(v) = self.verificado else { return };
        let c = self.agenda.contacto(&self.nombre_contacto).cloned();
        self.monitor.identidad = Some(Identidad {
            nombre: self.nombre_contacto.clone(),
            huella: agenda::huella(self.identidad.signing_key(), v.signing()),
            propia: agenda::huella_corta(self.identidad.signing_key()),
            contacto: agenda::huella_corta(v.signing()),
            verificado_en: c.as_ref().map_or(0, |c| c.verificado_en),
            estado: match c.map(|c| c.estado) {
                Some(agenda::Estado::ClaveCambiada { .. }) => "clave_cambiada".to_owned(),
                _ => "verificado".to_owned(),
            },
        });
    }

    fn otk(&mut self, v: &Value) {
        if let Some(k) = v["clave"].as_str() {
            // The answer: start the conversation with it.
            let (Some(verificado), Ok(otk)) =
                (self.verificado, Curve25519PublicKey::from_base64(k))
            else {
                return;
            };
            match self.identidad.start(verificado.curve(), otk) {
                Ok(conv) => {
                    self.conversacion = Some(conv);
                    self.inicio = ahora_s();
                    self.cadena = Cadena::new("secure", self.inicio);
                    self.fase = Fase::Lista;
                    // The first envelope opens the session on the other side.
                    self.manda_olm(json!({ "m": "hola" }));
                    self.monitor
                        .anota(ahora_s(), Nivel::Info, "conversación cifrada abierta");
                }
                Err(e) => {
                    self.aviso("sec_err_olm", e.to_string());
                    self.cerrar("no se pudo abrir Olm");
                }
            }
        } else {
            // The request: one fresh one-time key.
            match self.identidad.fresh_one_time_key() {
                Ok(k) => self.manda(json!({ "t": "otk", "clave": k.to_base64() })),
                Err(e) => self.aviso("sec_err_olm", e.to_string()),
            }
        }
        self.cambio = true;
    }

    fn manda_olm(&mut self, carga: Value) {
        if !self.estado.en_marcha() {
            self.aviso("sec_en_pausa", "");
            return;
        }
        let trust = self.agenda.trust();
        let Some(conv) = &mut self.conversacion else {
            return;
        };
        let bytes = serde_json::to_vec(&carga).unwrap_or_default();
        match trust.send(conv, &bytes) {
            Ok(env) => {
                let (tipo, cuerpo) = env.to_wire();
                self.monitor.enviado(ahora_s());
                self.manda(json!({ "t": "olm", "tipo": tipo, "cuerpo": B64.encode(cuerpo) }));
            }
            Err(e) => {
                self.aviso("sec_err_olm", e.to_string());
            }
        }
    }

    fn olm(&mut self, v: &Value) {
        let (Some(tipo), Some(cuerpo)) = (
            v["tipo"].as_u64(),
            v["cuerpo"].as_str().and_then(|s| B64.decode(s).ok()),
        ) else {
            return;
        };
        let Ok(env) = Envelope::from_wire(tipo as usize, &cuerpo) else {
            return;
        };
        let trust = self.agenda.trust();
        let abierto = if let Some(conv) = &mut self.conversacion {
            trust.open(conv, &env)
        } else {
            let Some(verificado) = self.verificado else {
                return;
            };
            match self.identidad.receive(&trust, verificado.curve(), &env) {
                Ok((conv, plano)) => {
                    self.conversacion = Some(conv);
                    self.inicio = ahora_s();
                    self.cadena = Cadena::new("secure", self.inicio);
                    self.fase = Fase::Lista;
                    self.monitor
                        .anota(ahora_s(), Nivel::Info, "conversación cifrada abierta");
                    Ok(plano)
                }
                Err(e) => Err(e),
            }
        };
        match abierto {
            Ok(plano) => {
                self.monitor.recibido(ahora_s());
                if let Ok(carga) = serde_json::from_slice::<Value>(&plano) {
                    self.carga(&carga);
                }
            }
            Err(e) => self.aviso("sec_err_olm", e.to_string()),
        }
        self.cambio = true;
    }

    fn carga(&mut self, c: &Value) {
        match c["m"].as_str().unwrap_or_default() {
            "texto" => {
                let texto: String = c["texto"]
                    .as_str()
                    .unwrap_or_default()
                    .chars()
                    .take(8000)
                    .collect();
                self.mensajes.push((ahora_s(), false, texto));
            }
            "archivo" => {
                // The file's key: open what already arrived and whatever comes next.
                let nombre = limpia_nombre(c["nombre"].as_str().unwrap_or("archivo"));
                let Ok(llave) = serde_json::from_value::<Llave>(c["llave"].clone()) else {
                    return;
                };
                let Ok(recepcion) = Recepcion::new(&llave) else {
                    return;
                };
                let ruta = nombre_libre(&self.descargas, &nombre);
                let archivo = std::fs::File::create(&ruta).ok();
                let mut e = self.entrada.take().unwrap_or(Entrada {
                    nombre: nombre.clone(),
                    bloques_pendientes: VecDeque::new(),
                    recepcion: None,
                    llave: None,
                    archivo: None,
                    ruta: None,
                });
                e.nombre = nombre;
                e.recepcion = Some(recepcion);
                e.llave = Some(llave);
                e.archivo = archivo;
                e.ruta = Some(ruta);
                self.entrada = Some(e);
                self.vacia_bloques();
            }
            _ => {}
        }
    }

    fn bloque(&mut self, v: &Value) {
        let (Some(n), Some(b)) = (
            v["n"].as_u64(),
            v["b"].as_str().and_then(|s| B64.decode(s).ok()),
        ) else {
            return;
        };
        let e = self.entrada.get_or_insert(Entrada {
            nombre: String::new(),
            bloques_pendientes: VecDeque::new(),
            recepcion: None,
            llave: None,
            archivo: None,
            ruta: None,
        });
        e.bloques_pendientes.push_back((n, b));
        self.vacia_bloques();
    }

    fn vacia_bloques(&mut self) {
        let mut fallo: Option<&'static str> = None;
        let mut completo = false;
        if let Some(e) = &mut self.entrada {
            if let (Some(r), Some(f)) = (&mut e.recepcion, &mut e.archivo) {
                while let Some((n, b)) = e.bloques_pendientes.pop_front() {
                    match r.mete(n, &b) {
                        Ok(plano) => {
                            if f.write_all(&plano).is_err() {
                                fallo = Some("sec_err_archivo");
                                break;
                            }
                        }
                        Err(_) => {
                            fallo = Some("sec_archivo_mal");
                            break;
                        }
                    }
                }
                if fallo.is_none() {
                    if let Some(l) = &e.llave {
                        completo = r.bloques() == l.bloques;
                    }
                }
            }
        }
        if let Some(clave) = fallo {
            if let Some(e) = self.entrada.take() {
                if let Some(r) = &e.ruta {
                    let _ = std::fs::remove_file(r);
                }
            }
            self.aviso(clave, "");
            return;
        }
        if completo {
            if let Some(e) = self.entrada.take() {
                let ok = match (e.recepcion, &e.llave) {
                    (Some(r), Some(l)) => r.termina(l).is_ok(),
                    _ => false,
                };
                if ok {
                    let n = e.nombre.clone();
                    self.monitor
                        .anota(ahora_s(), Nivel::Info, format!("archivo recibido: {n}"));
                    self.mensajes.push((ahora_s(), false, format!("📎 {n}")));
                    self.aviso("sec_archivo_recibido", n);
                } else {
                    if let Some(r) = &e.ruta {
                        let _ = std::fs::remove_file(r);
                    }
                    self.aviso("sec_archivo_mal", "");
                }
                self.cambio = true;
            }
        }
    }

    /// The person wrote something.
    pub fn texto(&mut self, texto: &str) {
        if self.fase != Fase::Lista {
            self.aviso("sec_no_lista", "");
            return;
        }
        let t: String = texto.chars().take(8000).collect();
        self.mensajes.push((ahora_s(), true, t.clone()));
        self.manda_olm(json!({ "m": "texto", "texto": t }));
        self.cambio = true;
    }

    /// A file starts: the page will hand the pieces over.
    pub fn archivo_empieza(&mut self, nombre: &str) -> Result<(), String> {
        if self.fase != Fase::Lista {
            return Err("sec_no_lista".to_owned());
        }
        self.salida = Some(Salida {
            nombre: limpia_nombre(nombre),
            trozos: Trozos::new().map_err(|e| e.to_string())?,
            bytes: 0,
        });
        Ok(())
    }

    /// One piece of the file, base64 from the page; `ultimo` closes it.
    pub fn archivo_trozo(&mut self, b64: &str, ultimo: bool) -> Result<(), String> {
        let Some(s) = &mut self.salida else {
            return Err("sin archivo".to_owned());
        };
        let bytes = B64.decode(b64).map_err(|e| e.to_string())?;
        let (n, c) = s.trozos.empuja(&bytes).map_err(|e| e.to_string())?;
        s.bytes += bytes.len() as u64;
        let trama = json!({ "t": "bloque", "n": n, "b": B64.encode(c) });
        self.manda(trama);
        if ultimo {
            let Some(s) = self.salida.take() else {
                return Ok(());
            };
            let nombre = s.nombre.clone();
            let llave = s.trozos.termina().map_err(|e| e.to_string())?;
            self.manda_olm(json!({ "m": "archivo", "nombre": nombre, "llave": llave }));
            self.monitor
                .anota(ahora_s(), Nivel::Info, format!("archivo enviado: {nombre}"));
            self.mensajes
                .push((ahora_s(), true, format!("📎 {nombre}")));
            self.cambio = true;
        }
        Ok(())
    }

    /// The person says the program seen is theirs: go on (only for something observed).
    pub fn reanudar_a_mano(&mut self) {
        if self
            .estado
            .reanudar_a_mano(ahora_s(), &mut self.monitor, &mut self.cadena)
        {
            self.cambio = true;
        }
    }

    /// Whether the screen must be redrawn, and clear the flag.
    pub fn cambiado(&mut self) -> bool {
        std::mem::take(&mut self.cambio)
    }

    /// Everything the page draws.
    pub fn estado_json(&self) -> Value {
        let fase = match self.fase {
            Fase::Sola => "sola",
            Fase::Escuchando => "escuchando",
            Fase::Saludando => "saludando",
            Fase::Comparando => "comparando",
            Fase::Abriendo => "abriendo",
            Fase::Lista => "lista",
        };
        let mut monitor = serde_json::to_value(&self.monitor).unwrap_or(Value::Null);
        monitor["sesion"] = serde_json::to_value(&self.estado).unwrap_or(Value::Null);
        json!({
            "tipo": "secure_estado",
            "fase": fase,
            "puerto": self.puerto,
            "huella": self.huella_propia(),
            "tarjeta": self.tarjeta(),
            "contacto": self.nombre_contacto,
            "numeros": self.numeros().map(|(a, b, c)| vec![a, b, c]),
            "mensajes": self.mensajes.iter().rev().take(200).rev().map(|(t, mio, x)| json!({ "t": t, "mio": mio, "texto": x })).collect::<Vec<_>>(),
            "contactos": self.agenda.contactos().iter().map(|c| json!({ "nombre": c.nombre, "verificado_en": c.verificado_en, "estado": matches!(c.estado, agenda::Estado::Verificado) })).collect::<Vec<_>>(),
            "monitor": monitor,
        })
    }
}

fn lee_o_crea_clave(ruta: &Path) -> [u8; 32] {
    if let Ok(b) = std::fs::read(ruta) {
        if let Ok(k) = <[u8; 32]>::try_from(b.as_slice()) {
            return k;
        }
    }
    let mut k = [0u8; 32];
    crate::sesion::azar(&mut k);
    let _ = std::fs::write(ruta, k);
    k
}

fn limpia_nombre(n: &str) -> String {
    let s: String = n
        .chars()
        .filter(|c| {
            !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') && !c.is_control()
        })
        .take(120)
        .collect();
    let s = s.trim().trim_start_matches('.').to_owned();
    if s.is_empty() {
        "archivo".to_owned()
    } else {
        s
    }
}

fn nombre_libre(carpeta: &Path, nombre: &str) -> PathBuf {
    let primera = carpeta.join(nombre);
    if !primera.exists() {
        return primera;
    }
    let (base, ext) = match nombre.rsplit_once('.') {
        Some((b, e)) if !b.is_empty() => (b.to_owned(), format!(".{e}")),
        _ => (nombre.to_owned(), String::new()),
    };
    (2..1000)
        .map(|n| carpeta.join(format!("{base} ({n}){ext}")))
        .find(|p| !p.exists())
        .unwrap_or(primera)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn gira(a: &mut Secure, b: &mut Secure, veces: u32) {
        for _ in 0..veces {
            a.tic();
            b.tic();
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn two_computers_verify_each_other_then_a_message_and_a_file_cross_encrypted() {
        let base = std::env::temp_dir().join(format!("guardiana-secure-{}", std::process::id()));
        let (da, db) = (base.join("a"), base.join("b"));
        std::fs::create_dir_all(&da).unwrap();
        std::fs::create_dir_all(&db).unwrap();
        let mut a = Secure::abre(&da, &da);
        let mut b = Secure::abre(&db, &db);
        let puerto = a.escuchar(0).unwrap();
        b.conectar(&format!("127.0.0.1:{puerto}")).unwrap();
        gira(&mut a, &mut b, 25);
        let (na, nb) = (a.numeros(), b.numeros());
        assert!(na.is_some(), "a has numbers: {:?}", a.estado_json()["fase"]);
        assert_eq!(na, nb, "both see the same numbers");
        a.sas_persona(true, "Bea");
        b.sas_persona(true, "Ana");
        gira(&mut a, &mut b, 25);
        assert_eq!(a.estado_json()["fase"], "lista");
        assert_eq!(b.estado_json()["fase"], "lista");
        assert_eq!(a.monitor.identidad.as_ref().unwrap().nombre, "Bea");
        assert_eq!(
            a.monitor.identidad.as_ref().unwrap().huella,
            b.monitor.identidad.as_ref().unwrap().huella
        );

        a.texto("hola Bea");
        gira(&mut a, &mut b, 15);
        assert_eq!(b.mensajes.last().unwrap().2, "hola Bea");
        assert!(!b.mensajes.last().unwrap().1);

        // A file in three pieces, the last one short.
        let plano: Vec<u8> = (0..(guardiana_secure::archivo::BLOQUE * 2 + 321))
            .map(|i| (i % 211) as u8)
            .collect();
        a.archivo_empieza("informe.pdf").unwrap();
        let trozos: Vec<&[u8]> = plano.chunks(guardiana_secure::archivo::BLOQUE).collect();
        for (i, t) in trozos.iter().enumerate() {
            a.archivo_trozo(&B64.encode(t), i + 1 == trozos.len())
                .unwrap();
        }
        gira(&mut a, &mut b, 30);
        let recibido = std::fs::read(db.join("informe.pdf")).unwrap();
        assert_eq!(recibido, plano);
        assert!(b.mensajes.last().unwrap().2.contains("informe.pdf"));
        assert!(b.monitor.cifrado.recibidos >= 2);

        // Both books remember each other; the next hello skips the short code.
        assert_eq!(
            a.agenda.contacto("Bea").unwrap().estado,
            agenda::Estado::Verificado
        );
        let json = a.estado_json().to_string();
        assert!(!json.contains("127.0.0.1"), "no address on screen");
        let _ = std::fs::remove_dir_all(base);
    }
}
