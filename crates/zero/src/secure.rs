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
use std::time::Duration;

use base64::Engine as _;
use guardiana_secure::agenda::{self, Agenda};
use guardiana_secure::archivo::{Llave, Recepcion, Trozos};
use guardiana_secure::claves::{Curve25519PublicKey, Ed25519Signature, Mac};
use guardiana_secure::monitor::{Identidad, Monitor, Nivel};
use guardiana_secure::pausa::Sesion as EstadoSesion;
use guardiana_secure::prueba::{Cadena, Prueba};
use guardiana_secure::resumen::{Archivos, Clase, Conexion, Resumen};
use guardiana_secure::sframe;
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
    /// The relay joined us with the other computer: the socket to adopt (bytes pass through
    /// the relay, which cannot read them).
    AceptadoRele(TcpStream),
    /// The relay could not be reached, or nobody came.
    ReleFallo(String),
    /// The router answered about the door.
    Puerta(Result<crate::puerta::Puerta, crate::puerta::SinPuerta>),
    /// The link is up (we accepted or connected).
    Conectado,
    /// A frame arrived.
    Trama(Value),
    /// The link went down, with why (never an address).
    Cerrado(String),
}

/// One link to the other computer: a writer thread for us (so a slow link never stalls the
/// window), a reader thread for what comes.
struct Enlace {
    cola: Sender<Vec<u8>>,
    /// Bytes handed to the writer, and bytes it already wrote to the network: their
    /// difference is what is still on its way out (a file's «time left»).
    encolados: std::sync::Arc<std::sync::atomic::AtomicU64>,
    escritos: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl Enlace {
    fn new(mut s: TcpStream) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        let (cola, rx) = mpsc::channel::<Vec<u8>>();
        let escritos = std::sync::Arc::new(AtomicU64::new(0));
        let hilo = escritos.clone();
        std::thread::spawn(move || {
            while let Ok(cuerpo) = rx.recv() {
                if s.write_all(&(cuerpo.len() as u32).to_be_bytes())
                    .and_then(|()| s.write_all(&cuerpo))
                    .and_then(|()| s.flush())
                    .is_err()
                {
                    return;
                }
                hilo.fetch_add(cuerpo.len() as u64 + 4, Ordering::Relaxed);
            }
        });
        Self {
            cola,
            encolados: std::sync::Arc::new(AtomicU64::new(0)),
            escritos,
        }
    }

    fn manda(&self, v: &Value) -> Result<(), String> {
        let cuerpo = serde_json::to_vec(v).map_err(|e| e.to_string())?;
        if cuerpo.len() > MAX_FRAME {
            return Err("trama demasiado grande".to_owned());
        }
        let n = cuerpo.len() as u64 + 4;
        self.cola
            .send(cuerpo)
            .map_err(|_| "enlace cerrado".to_owned())?;
        self.encolados
            .fetch_add(n, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    /// (handed over, written).
    fn cuenta(&self) -> (u64, u64) {
        use std::sync::atomic::Ordering;
        (
            self.encolados.load(Ordering::Relaxed),
            self.escritos.load(Ordering::Relaxed),
        )
    }
}

/// A file sent in this conversation: when it started and, once the other side said it is
/// whole on its disk, how long it took to get there.
#[derive(Debug, Clone)]
pub struct Enviado {
    /// Its name.
    pub nombre: String,
    /// When its line went into the chat (unix s).
    pub t: u64,
    /// Size.
    pub bytes: u64,
    empezo: std::time::Instant,
    /// Seconds from the first byte to «it is here», when the other side confirmed it.
    pub llego_en: Option<f64>,
}

/// A file on its way out, followed until its last byte left this computer.
struct Envio {
    nombre: String,
    /// Its size, as the page said.
    bytes: u64,
    /// The link's «handed over» count when it started, and when its last piece was handed.
    inicio: u64,
    fin: Option<u64>,
    /// (when, written) a few times a second, for the speed.
    marcas: VecDeque<(std::time::Instant, u64)>,
    aviso: std::time::Instant,
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
    /// Connecting through the relay with the other person's code: waiting for the pair.
    Buscando,
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
    /// Size the sender announced (0: unknown), and what arrived so far.
    esperado: u64,
    recibido: u64,
}

/// A file going out, block by block as the page hands them over.
struct Salida {
    nombre: String,
    trozos: Trozos,
    bytes: u64,
}

/// How long a call rings before «no answer».
pub const TIMBRE_S: u64 = 45;

/// A call coming in, ringing until the person answers or rejects it.
struct Timbre {
    clave: [u8; 32],
    video: bool,
    mime: Option<String>,
    desde: u64,
}

/// A file that arrived, kept on screen with what can be done with it.
#[derive(Debug, Clone)]
pub struct Recibido {
    /// Unique in this run of the program.
    pub id: u64,
    /// The name the other side gave it.
    pub nombre: String,
    /// Where it was saved (Descargas).
    pub ruta: PathBuf,
    /// Size.
    pub bytes: u64,
    /// When it arrived (unix s): the same second as its line in the chat.
    pub t: u64,
    /// The person deleted it.
    pub borrado: bool,
}

impl Recibido {
    /// A picture the page can show small.
    pub fn imagen(&self) -> bool {
        matches!(
            extension(&self.nombre).as_str(),
            "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp"
        )
    }

    /// Safe to open with a double click: never a program or a script.
    pub fn abrible(&self) -> bool {
        !matches!(
            extension(&self.nombre).as_str(),
            "exe"
                | "msi"
                | "bat"
                | "cmd"
                | "com"
                | "scr"
                | "pif"
                | "ps1"
                | "psm1"
                | "vbs"
                | "vbe"
                | "js"
                | "jse"
                | "wsf"
                | "wsh"
                | "hta"
                | "lnk"
                | "reg"
                | "jar"
                | "dll"
                | "cpl"
                | "msc"
                | "msix"
                | "appx"
                | "url"
                | "iso"
                | "img"
                | "vhd"
                | "vhdx"
                | "chm"
                | "sh"
                | "app"
                | "dmg"
                | "pkg"
                | "docm"
                | "xlsm"
                | "pptm"
        ) && !self.borrado
    }

    /// What the page draws.
    pub fn json(&self) -> Value {
        json!({ "id": self.id, "nombre": self.nombre, "bytes": self.bytes, "t": self.t, "borrado": self.borrado, "imagen": self.imagen(), "abrible": self.abrible() })
    }
}

fn extension(nombre: &str) -> String {
    nombre
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default()
}

static SIGUIENTE_RECIBIDO: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Windows marks what came from outside («Mark of the Web»): SmartScreen and Office's
/// protected view then treat the file with the same care as a download.
fn marca_de_fuera(ruta: &Path) {
    #[cfg(windows)]
    {
        let mut ads = ruta.as_os_str().to_owned();
        ads.push(":Zone.Identifier");
        let _ = std::fs::write(ads, b"[ZoneTransfer]\r\nZoneId=3\r\n");
    }
    #[cfg(not(windows))]
    let _ = ruta;
}

/// A voice or video call over the same link: the page records (Opus, VP9) in short pieces, each
/// piece is encrypted here with the media layer ([`sframe`]) under a key that went inside Olm,
/// and the other side plays them as they come. Direct between the two computers, no media server.
struct Llamada {
    video: bool,
    /// What our recorder produces (container and codecs), told to the other side so its player
    /// opens the right way; it changes when the phone takes the call over.
    mime_local: String,
    /// What the other side's pieces are, to open our player with it.
    mime_remoto: Option<String>,
    emisor: sframe::Emisor,
    receptor: Option<sframe::Receptor>,
    /// The other side answered (until then this side is «calling…» and sends nothing).
    conectada: bool,
    /// Since when: the call (or, while calling, the ring).
    desde: u64,
    bytes_enviados: u64,
    bytes_recibidos: u64,
    /// For the bitrate on screen: (unix s, bytes at that second).
    marca: (u64, u64, u64),
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
    puerta: Option<crate::puerta::Puerta>,
    puerta_error: Option<String>,
    /// The phone, while the person has handed the conversation over to it.
    movil: Option<crate::movil::Movil>,
    movil_puerta: Option<crate::puerta::Puerta>,
    movil_puerta_rx: Option<Receiver<Result<crate::puerta::Puerta, crate::puerta::SinPuerta>>>,
    movil_puerta_error: Option<String>,
    movil_error: Option<String>,
    movil_idioma: String,
    /// What the other side's player can open (its phone, an iPhone, tells it): our recorder
    /// picks from this list when it is not empty.
    reproduce_otro: Vec<String>,
    /// What we last told the other side about our own player.
    reproduce_dicho: Vec<String>,
    /// To hand the network threads a way back while listening.
    tx: Option<Sender<Entrante>>,
    /// The relay code on screen while waiting through the relay (no door could be opened).
    rele_codigo: Option<String>,
    /// This link goes through the blind relay.
    rele: bool,
    rele_error: Option<String>,
    tarjeta_otra: Option<tarjeta::Tarjeta>,
    sas_inicio: Option<Inicio>,
    sas_codigo: Option<Codigo>,
    sas_yo_ok: bool,
    sas_mac_otra: Option<(Mac, Ed25519Signature)>,
    verificado: Option<Verified>,
    conversacion: Option<Conversation>,
    /// Messages for the screen: (unix s, mine?, text, who — empty for the other end of this link).
    pub mensajes: Vec<(u64, bool, String, String)>,
    /// What the screen shows the whole time.
    pub monitor: Monitor,
    /// The chain of what was seen, for the signed evidence.
    pub cadena: Cadena,
    /// Running, or stopped with its reason.
    pub estado: EstadoSesion,
    entrada: Option<Entrada>,
    salida: Option<Salida>,
    llamada: Option<Llamada>,
    /// A call ringing here.
    entrante: Option<Timbre>,
    /// A call nobody answered here: (when, with video), until the person calls back or
    /// closes the note.
    perdida: Option<(u64, bool)>,
    /// A file on its way out.
    envio: Option<Envio>,
    /// When something last went out on the link: a quiet link gets a small frame every
    /// 20 s, so no router or office firewall drops it for being idle (16 Oct 2026: a chat via
    /// the relay was cut after five minutes without messages).
    ultimo_envio: std::time::Instant,
    /// When something last came from the other side, and whether it sends heartbeats (a
    /// version that does): then a minute of silence means the link is gone, and both screens
    /// say so (16 Oct 2026: one side showed the conversation closed, the other still open).
    ultimo_recibido: std::time::Instant,
    otro_late: bool,
    /// This computer's time zone (the page tells it) and the other side's, exchanged inside
    /// Olm once the conversation is open (16 Oct 2026, «que salga en nuestro mapa, el de él y
    /// el mío»): the globe places each computer by its time zone, never by an address.
    pub zona_propia: String,
    zona_otra: String,
    /// Files sent, with how long each took to arrive.
    pub enviados: Vec<Enviado>,
    /// Files that arrived in this conversation.
    pub recibidos: Vec<Recibido>,
    /// In a group: the host already called this member for the current call (so someone who
    /// hangs up is not called again and again).
    pub llamado_en_grupo: bool,
    archivos: Archivos,
    /// The last summary, signed, for the page to show and download.
    pub resumen: Option<Resumen>,
    version: String,
    /// Media pieces for the page, drained by the session every tick: (who, base64). `who` is
    /// empty for the other end of this link; in a group the host relays with the original name.
    pub media_para_pagina: VecDeque<(String, String)>,
    /// Something happened while the tab was not in front: the tab's name carries a mark.
    pub novedad: bool,
    /// The group's members, as the host sends them (empty in a two-way talk).
    grupo: Vec<String>,
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
            puerta: None,
            puerta_error: None,
            movil: None,
            movil_puerta: None,
            movil_puerta_rx: None,
            movil_puerta_error: None,
            movil_error: None,
            movil_idioma: "es".to_owned(),
            reproduce_otro: Vec::new(),
            reproduce_dicho: Vec::new(),
            entrante: None,
            perdida: None,
            envio: None,
            ultimo_envio: std::time::Instant::now(),
            ultimo_recibido: std::time::Instant::now(),
            otro_late: false,
            zona_propia: String::new(),
            zona_otra: String::new(),
            enviados: Vec::new(),
            recibidos: Vec::new(),
            llamado_en_grupo: false,
            tx: None,
            rele_codigo: None,
            rele: false,
            rele_error: None,
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
            llamada: None,
            archivos: Archivos::default(),
            resumen: None,
            version: String::new(),
            media_para_pagina: VecDeque::new(),
            novedad: false,
            grupo: Vec::new(),
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

    /// Start listening on a port (0: a new random one). The address is never shown.
    pub fn escuchar(&mut self, puerto: u16) -> Result<u16, String> {
        self.escuchar_grupo(Some(puerto), None, true)
    }

    /// Listen on a port the system picks (the tests, so two of them never meet on 47300).
    pub fn escuchar_efimero(&mut self) -> Result<u16, String> {
        self.escuchar_grupo(None, None, true)
    }

    /// Listen for one more member of a group: the same port and the same relay code as the
    /// first session, and the door left as it is (the first session opened it and keeps it).
    pub fn escuchar_grupo(
        &mut self,
        puerto: Option<u16>,
        codigo_rele: Option<String>,
        abre_puerta: bool,
    ) -> Result<u16, String> {
        self.cerrar("nueva escucha");
        // A new port for every invitation (the owner, 16 Oct 2026: «el código debe ser aleatorio
        // cada vez»): the door code carries the port, so a fresh one gives a fresh code, and an
        // old code shared somewhere stops leading here. Any free port otherwise; `None` asks the
        // system for one.
        let pedido = match puerto {
            Some(0) => puerto_al_azar(),
            Some(p) => p,
            None => 0,
        };
        let listener = TcpListener::bind(("0.0.0.0", pedido))
            .or_else(|_| TcpListener::bind(("0.0.0.0", 0)))
            .map_err(|e| e.to_string())?;
        let puerto = listener.local_addr().map_err(|e| e.to_string())?.port();
        let (tx, rx) = mpsc::channel();
        let tx2 = tx.clone();
        let tx_rele = tx.clone();
        std::thread::spawn(move || {
            if let Ok((s, _)) = listener.accept() {
                let _ = tx.send(Entrante::Aceptado(s));
            }
        });
        if abre_puerta {
            // The door in the router, asked for in its own thread: a few seconds, never blocking.
            std::thread::spawn(move || {
                let r = crate::puerta::abre(puerto, 3 * 3600);
                let _ = tx2.send(Entrante::Puerta(r));
            });
        }
        self.rx = Some(rx);
        self.tx = Some(tx_rele);
        self.puerto = puerto;
        self.puerta = None;
        self.puerta_error = None;
        self.rele_codigo = None;
        self.rele_error = None;
        self.fase = Fase::Escuchando;
        self.cambio = true;
        if let Some(c) = codigo_rele {
            self.espera_en_rele_con(c);
        }
        Ok(puerto)
    }

    /// Connect to the other computer. `direccion` is `host:puerto`; it is used now and kept
    /// nowhere.
    pub fn conectar(&mut self, direccion: &str) -> Result<(), String> {
        self.cerrar("nueva conexión");
        if let Some(sala) = crate::rele::sala_de(direccion) {
            // The other person waits at the relay: go there and wait for the pair.
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let r = crate::rele::conecta(&crate::rele::direccion(), &sala);
                let _ = tx.send(match r {
                    Ok(s) => Entrante::AceptadoRele(s),
                    Err(e) => Entrante::ReleFallo(e),
                });
            });
            self.rx = Some(rx);
            self.fase = Fase::Buscando;
            self.cambio = true;
            return Ok(());
        }
        let direccion = match crate::puerta::lee_codigo(direccion) {
            Some((ip, puerto)) => format!("{ip}:{puerto}"),
            None => direccion.trim().to_owned(),
        };
        let addr: SocketAddr = direccion
            .trim()
            .to_socket_addrs()
            .map_err(|e| e.to_string())?
            .next()
            .ok_or_else(|| "dirección vacía".to_owned())?;
        let s =
            TcpStream::connect_timeout(&addr, Duration::from_secs(8)).map_err(|e| e.to_string())?;
        self.adopta(s, false);
        Ok(())
    }

    /// A link someone else accepted or opened (the group layer): take it as ours.
    pub fn adopta_enlace(&mut self, s: TcpStream, rele: bool) {
        self.cerrar("nuevo enlace");
        self.adopta(s, rele);
    }

    fn adopta(&mut self, s: TcpStream, rele: bool) {
        let _ = s.set_nodelay(true);
        self.ultimo_recibido = std::time::Instant::now();
        self.rele = rele;
        self.monitor.transporte.directo = Some(!rele);
        if rele {
            self.monitor.anota(
                ahora_s(),
                Nivel::Info,
                "enlace por el relé ciego (solo ve cifrado)",
            );
        }
        let (tx, rx) = mpsc::channel();
        let lector = match s.try_clone() {
            Ok(l) => l,
            Err(e) => {
                self.aviso("sec_err_enlace", e.to_string());
                return;
            }
        };
        std::thread::spawn(move || lee_tramas(lector, tx));
        self.enlace = Some(Enlace::new(s));
        self.rx = Some(rx);
        self.fase = Fase::Saludando;
        self.cambio = true;
    }

    /// The program's version, for the signed sheets.
    pub fn pon_version(&mut self, v: &str) {
        self.version = v.to_owned();
    }

    /// The signed summary of what just happened, when there was a verified conversation.
    fn cierra_resumen(&mut self) {
        if self.monitor.identidad.is_none() || self.fase != Fase::Lista {
            return;
        }
        let clase = match self.llamada.as_ref().map(|l| l.video) {
            Some(true) => Clase::Videollamada,
            Some(false) => Clase::Llamada,
            None if self.archivos.enviados + self.archivos.recibidos > 0 => Clase::Envio,
            None => Clase::Chat,
        };
        let conexion = Conexion {
            directa: self.monitor.transporte.directo,
            codec_audio: self.monitor.transporte.codec_audio.clone(),
            codec_video: self.monitor.transporte.codec_video.clone(),
            ..Conexion::default()
        };
        self.resumen = Resumen::cerrar(
            &self.identidad,
            &self.version,
            clase,
            &self.monitor,
            &self.cadena,
            self.inicio,
            ahora_s(),
            self.archivos.clone(),
            conexion,
        );
        self.archivos = Archivos::default();
    }

    /// The chain of what was seen, signed, as JSON for a file.
    pub fn prueba_json(&self) -> String {
        Prueba::firmar(
            &self.identidad,
            self.cadena.clone(),
            &self.version,
            ahora_s(),
        )
        .to_json()
    }

    /// Drop the link and forget the handshake; the identity and the book stay.
    pub fn cerrar(&mut self, por: &str) {
        self.cierra_resumen();
        self.movil_fin();
        if let Some(p) = self.puerta.take() {
            std::thread::spawn(move || p.cierra());
        }
        self.puerta_error = None;
        self.tx = None;
        self.rele_codigo = None;
        self.rele = false;
        self.rele_error = None;
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
        self.llamada = None;
        self.entrante = None;
        self.perdida = None;
        self.llamado_en_grupo = false;
        self.otro_late = false;
        self.zona_otra.clear();
        self.reproduce_otro.clear();
        self.reproduce_dicho.clear();
        self.monitor.transporte = Default::default();
        if self.fase != Fase::Sola {
            self.monitor
                .anota(ahora_s(), Nivel::Info, format!("enlace cerrado: {por}"));
        }
        self.fase = Fase::Sola;
        self.monitor.identidad = None;
        self.cambio = true;
    }

    fn manda(&mut self, v: Value) {
        self.ultimo_envio = std::time::Instant::now();
        if let Some(e) = &self.enlace {
            if let Err(err) = e.manda(&v) {
                self.aviso("sec_err_enlace", err);
                self.cerrar("fallo al enviar");
            }
        }
    }

    /// Called every tick: everything the network thread left, and the handshake that follows.
    pub fn tic(&mut self) {
        self.movil_tic();
        self.timbres();
        self.sigue_envio();
        if self.enlace.is_some()
            && self.fase != Fase::Sola
            && self.ultimo_envio.elapsed() >= Duration::from_secs(20)
        {
            // Older versions ignore a frame they do not know.
            self.manda(json!({ "t": "latido" }));
        }
        if self.otro_late
            && self.fase != Fase::Sola
            && self.ultimo_recibido.elapsed() >= Duration::from_secs(65)
        {
            self.aviso("sec_enlace_cerrado", "sin señal de la otra persona");
            self.cerrar("sin señal de la otra persona");
        }
        let mut pendientes = Vec::new();
        if let Some(rx) = &self.rx {
            while let Ok(e) = rx.try_recv() {
                pendientes.push(e);
            }
        }
        for e in pendientes {
            match e {
                Entrante::Aceptado(s) => self.adopta(s, false),
                Entrante::AceptadoRele(s) => self.adopta(s, true),
                Entrante::ReleFallo(e) => {
                    self.rele_error = Some(e.clone());
                    if self.fase == Fase::Buscando {
                        self.aviso("sec_rele_fallo", e);
                        self.cerrar("relé sin pareja");
                    } else {
                        self.rele_codigo = None;
                    }
                    self.cambio = true;
                }
                Entrante::Puerta(r) => {
                    match r {
                        Ok(p) => {
                            self.puerta = Some(p);
                            self.puerta_error = None;
                        }
                        Err(e) => {
                            self.puerta = None;
                            self.puerta_error = Some(e.to_string());
                            self.espera_en_rele();
                        }
                    }
                    self.cambio = true;
                }
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

    /// No door in the router: wait at the blind relay with a fresh code (while the listener
    /// stays open for the same network).
    fn espera_en_rele(&mut self) {
        self.espera_en_rele_con(crate::rele::codigo_nuevo());
    }

    fn espera_en_rele_con(&mut self, codigo: String) {
        if self.fase != Fase::Escuchando || self.rele_codigo.is_some() {
            return;
        }
        let Some(tx) = self.tx.clone() else { return };
        let Some(sala) = crate::rele::sala_de(&codigo) else {
            return;
        };
        std::thread::spawn(move || {
            let r = crate::rele::conecta(&crate::rele::direccion(), &sala);
            let _ = tx.send(match r {
                Ok(s) => Entrante::AceptadoRele(s),
                Err(e) => Entrante::ReleFallo(e),
            });
        });
        self.rele_codigo = Some(codigo);
        self.cambio = true;
    }

    fn saluda(&mut self) {
        self.fase = Fase::Saludando;
        let t = self.tarjeta();
        self.manda(json!({ "t": "hola", "tarjeta": t }));
        self.cambio = true;
    }

    fn trama(&mut self, v: Value) {
        self.ultimo_recibido = std::time::Instant::now();
        let t = v["t"].as_str().unwrap_or_default().to_owned();
        match t.as_str() {
            "latido" => self.otro_late = true,
            "hola" => self.hola(&v),
            "sas" => self.sas(&v),
            "sas_mac" => self.sas_mac(&v),
            "otk" => self.otk(&v),
            "olm" => self.olm(&v),
            "bloque" => self.bloque(&v),
            "media" => self.media_entrante(&v),
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
                    self.novedad = true;
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
                    self.manda_zona();
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
                    self.manda_zona();
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
            "grupo" => {
                self.grupo = c["nombres"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .map(|n| n.chars().take(60).collect())
                            .take(16)
                            .collect()
                    })
                    .unwrap_or_default();
                self.cambio = true;
            }
            "texto" => {
                let texto: String = c["texto"]
                    .as_str()
                    .unwrap_or_default()
                    .chars()
                    .take(8000)
                    .collect();
                let de: String = c["de"]
                    .as_str()
                    .unwrap_or_default()
                    .chars()
                    .take(60)
                    .collect();
                self.mensajes.push((ahora_s(), false, texto, de));
            }
            "llamada" | "llamada_ok" => {
                let Some(clave) = c["clave"].as_str().and_then(|x| B64.decode(x).ok()) else {
                    return;
                };
                let Ok(clave) = <[u8; 32]>::try_from(clave.as_slice()) else {
                    return;
                };
                let video = c["video"].as_bool().unwrap_or(false);
                let mime_remoto = c["mime"].as_str().map(|m| m.chars().take(80).collect());
                let otro = self.numero_del_otro();
                if c["m"] == "llamada" && self.llamada.is_none() {
                    // It rings here: the person answers or rejects it (16 Oct 2026: before,
                    // a call started by itself on the other side).
                    self.entrante = Some(Timbre {
                        clave,
                        video,
                        mime: mime_remoto,
                        desde: ahora_s(),
                    });
                    self.novedad = true;
                    self.aviso("sec_llamada_entrante", self.nombre_contacto.clone());
                } else if let Some(l) = &mut self.llamada {
                    // The answer (or both called at the same moment: each call is the other's
                    // answer).
                    l.receptor = Some(sframe::Receptor::new(&clave, otro));
                    l.mime_remoto = mime_remoto;
                    if !l.conectada {
                        l.conectada = true;
                        l.desde = ahora_s();
                    }
                }
                self.cambio = true;
            }
            "reproduce" => {
                self.reproduce_otro = lista_de_formatos(&c["mimes"]);
                self.cambio = true;
            }
            "zona" => {
                self.zona_otra = zona_limpia(c["zona"].as_str().unwrap_or_default());
                self.cambio = true;
            }
            "mime" => {
                if let Some(l) = &mut self.llamada {
                    l.mime_remoto = c["mime"].as_str().map(|m| m.chars().take(80).collect());
                    self.cambio = true;
                }
            }
            "cuelga" => {
                if let Some(tm) = self.entrante.take() {
                    self.perdida = Some((tm.desde, tm.video));
                    self.monitor
                        .anota(ahora_s(), Nivel::Info, "llamada perdida");
                    self.aviso("sec_llamada_perdida", self.nombre_contacto.clone());
                    self.cambio = true;
                } else if let Some(l) = self.llamada.take() {
                    self.monitor.anota(
                        ahora_s(),
                        Nivel::Info,
                        if l.conectada {
                            "llamada terminada por la otra persona"
                        } else {
                            "llamada rechazada por la otra persona"
                        },
                    );
                    self.fin_de_llamada();
                    self.aviso(
                        if l.conectada {
                            "sec_llamada_colgada"
                        } else {
                            "sec_llamada_rechazada"
                        },
                        "",
                    );
                    self.cambio = true;
                }
            }
            "archivo" => {
                // The file's key: open what already arrived and whatever comes next. If its
                // first key came before the blocks (archivo_inicio), the file is already being
                // written: this one only closes it.
                let nombre = limpia_nombre(c["nombre"].as_str().unwrap_or("archivo"));
                let Ok(llave) = serde_json::from_value::<Llave>(c["llave"].clone()) else {
                    return;
                };
                if let Some(e) = self.entrada.as_mut().filter(|e| e.recepcion.is_some()) {
                    e.esperado = llave.bytes;
                    e.llave = Some(llave);
                    self.vacia_bloques();
                    return;
                }
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
                    esperado: 0,
                    recibido: 0,
                });
                e.nombre = nombre;
                e.esperado = llave.bytes;
                e.recepcion = Some(recepcion);
                e.llave = Some(llave);
                e.archivo = archivo;
                e.ruta = Some(ruta);
                self.entrada = Some(e);
                self.vacia_bloques();
            }
            "archivo_ok" => {
                let nombre = limpia_nombre(c["nombre"].as_str().unwrap_or_default());
                if let Some(x) = self
                    .enviados
                    .iter_mut()
                    .find(|x| x.nombre == nombre && x.llego_en.is_none())
                {
                    x.llego_en = Some(x.empezo.elapsed().as_secs_f64());
                    self.cambio = true;
                }
            }
            "archivo_inicio" => {
                // A file starts (16 Oct 2026): its key comes first, so every block goes to
                // disk as it arrives and the screen can show how much is left.
                let nombre = limpia_nombre(c["nombre"].as_str().unwrap_or("archivo"));
                let Ok(inicial) = serde_json::from_value::<Llave>(c["llave"].clone()) else {
                    return;
                };
                let Ok(recepcion) = Recepcion::new(&inicial) else {
                    return;
                };
                let ruta = nombre_libre(&self.descargas, &nombre);
                let archivo = std::fs::File::create(&ruta).ok();
                self.entrada = Some(Entrada {
                    nombre,
                    bloques_pendientes: VecDeque::new(),
                    recepcion: Some(recepcion),
                    llave: None,
                    archivo,
                    ruta: Some(ruta),
                    esperado: c["bytes"].as_u64().unwrap_or(0),
                    recibido: 0,
                });
                self.cambio = true;
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
            esperado: 0,
            recibido: 0,
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
                            e.recibido += plano.len() as u64;
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
                    if let Some(ruta) = &e.ruta {
                        marca_de_fuera(ruta);
                        self.recibidos.push(Recibido {
                            id: SIGUIENTE_RECIBIDO
                                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                            nombre: n.clone(),
                            ruta: ruta.clone(),
                            bytes: e.llave.as_ref().map_or(0, |l| l.bytes),
                            t: ahora_s(),
                            borrado: false,
                        });
                    }
                    // Tell the sender it is here, whole: its screen shows how long it took.
                    self.manda_olm(json!({ "m": "archivo_ok", "nombre": n }));
                    self.archivos.recibidos += 1;
                    self.archivos.bytes += e.llave.as_ref().map_or(0, |l| l.bytes);
                    self.monitor
                        .anota(ahora_s(), Nivel::Info, format!("archivo recibido: {n}"));
                    self.mensajes
                        .push((ahora_s(), false, format!("📎 {n}"), String::new()));
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

    /// 0 or 1: who is who in the call, decided by the keys so both sides agree.
    fn mi_numero(&self) -> u8 {
        match self.verificado {
            Some(v) if self.identidad.public_key().to_bytes() < v.curve().to_bytes() => 0,
            Some(_) => 1,
            None => 0,
        }
    }

    fn numero_del_otro(&self) -> u8 {
        1 - self.mi_numero()
    }

    fn empieza_llamada(&mut self, video: bool, inicia: bool, mime: &str) {
        let mut clave = [0u8; 32];
        crate::sesion::azar(&mut clave);
        let yo = self.mi_numero();
        self.llamada = Some(Llamada {
            video,
            mime_local: mime.chars().take(80).collect(),
            mime_remoto: None,
            emisor: sframe::Emisor::new(&clave, yo),
            receptor: None,
            conectada: !inicia,
            desde: ahora_s(),
            bytes_enviados: 0,
            bytes_recibidos: 0,
            marca: (ahora_s(), 0, 0),
        });
        self.monitor.transporte.directo = Some(!self.rele);
        self.monitor.transporte.dtls_verificado = None;
        self.monitor.transporte.codec_audio = Some("Opus".to_owned());
        self.monitor.transporte.codec_video = video.then(|| "VP9".to_owned());
        self.monitor.cifrado.sframe = Some(true);
        self.monitor.anota(
            ahora_s(),
            Nivel::Info,
            if video {
                "videollamada abierta"
            } else {
                "llamada de voz abierta"
            },
        );
        let mime_local = self.llamada.as_ref().map(|l| l.mime_local.clone());
        self.manda_olm(json!({
            "m": if inicia { "llamada" } else { "llamada_ok" },
            "clave": B64.encode(clave),
            "video": video,
            "mime": mime_local
        }));
        self.cambio = true;
    }

    /// The person starts a call (voice, or voice and video).
    pub fn llamar(&mut self, video: bool, mime: &str) -> Result<(), String> {
        if self.fase != Fase::Lista {
            return Err("sec_no_lista".to_owned());
        }
        self.perdida = None;
        if self.llamada.is_some() {
            self.cambia_mime(mime);
            return Ok(());
        }
        self.empieza_llamada(video, true, mime);
        Ok(())
    }

    /// Our pieces changed shape (the phone took the call over, or gave it back): tell the other
    /// side so its player reopens.
    pub fn cambia_mime(&mut self, mime: &str) {
        let mime: String = mime.chars().take(80).collect();
        if mime.is_empty() {
            return;
        }
        if let Some(l) = &mut self.llamada {
            if l.mime_local != mime {
                l.mime_local = mime.clone();
                self.manda_olm(json!({ "m": "mime", "mime": mime }));
            }
        }
    }

    /// The person hangs up.
    pub fn colgar(&mut self) {
        if self.llamada.take().is_some() {
            self.manda_olm(json!({ "m": "cuelga" }));
            self.monitor
                .anota(ahora_s(), Nivel::Info, "llamada terminada");
            self.fin_de_llamada();
            self.cambio = true;
        }
    }

    /// The call's figures go; how the two computers are linked stays on screen (16 Oct 2026:
    /// after hanging up it said «Sin conexión todavía» with the conversation still open).
    fn fin_de_llamada(&mut self) {
        let directo = self.monitor.transporte.directo;
        self.monitor.transporte = Default::default();
        self.monitor.transporte.directo = directo;
        self.monitor.cifrado.sframe = None;
    }

    /// The person answers the call ringing here, recording in `mime`.
    pub fn contestar(&mut self, mime: &str) -> Result<(), String> {
        let Some(t) = self.entrante.take() else {
            return Ok(());
        };
        if self.fase != Fase::Lista {
            return Err("sec_no_lista".to_owned());
        }
        self.empieza_llamada(t.video, false, mime);
        let otro = self.numero_del_otro();
        if let Some(l) = &mut self.llamada {
            l.receptor = Some(sframe::Receptor::new(&t.clave, otro));
            l.mime_remoto = t.mime;
        }
        self.cambio = true;
        Ok(())
    }

    /// The missed call note, for the screen (null when none).
    pub fn estado_perdida(&self) -> Value {
        self.perdida.map_or(
            Value::Null,
            |(t, video)| json!({ "t": t, "video": video, "de": self.nombre_contacto }),
        )
    }

    /// The person closed the «missed call» note.
    pub fn olvida_perdida(&mut self) {
        if self.perdida.take().is_some() {
            self.cambio = true;
        }
    }

    /// The person rejects the call ringing here.
    pub fn rechazar(&mut self) {
        if self.entrante.take().is_some() {
            self.manda_olm(json!({ "m": "cuelga" }));
            self.monitor
                .anota(ahora_s(), Nivel::Info, "llamada rechazada");
            self.cambio = true;
        }
    }

    /// A call is ringing here: with video?
    pub fn suena(&self) -> Option<bool> {
        self.entrante.as_ref().map(|t| t.video)
    }

    /// The call is up on both sides.
    pub fn conectada(&self) -> bool {
        self.llamada.as_ref().is_some_and(|l| l.conectada)
    }

    /// Rings that nobody answered, either way.
    fn timbres(&mut self) {
        let ahora = ahora_s();
        if self
            .llamada
            .as_ref()
            .is_some_and(|l| !l.conectada && ahora.saturating_sub(l.desde) > TIMBRE_S)
        {
            self.llamada = None;
            self.manda_olm(json!({ "m": "cuelga" }));
            self.monitor
                .anota(ahora_s(), Nivel::Info, "llamada sin respuesta");
            self.fin_de_llamada();
            self.aviso("sec_llamada_sin_respuesta", self.nombre_contacto.clone());
            self.cambio = true;
        }
        if self
            .entrante
            .as_ref()
            .is_some_and(|t| ahora.saturating_sub(t.desde) > TIMBRE_S + 5)
        {
            self.perdida = self.entrante.take().map(|t| (t.desde, t.video));
            self.aviso("sec_llamada_perdida", self.nombre_contacto.clone());
            self.cambio = true;
        }
    }

    /// A file that arrived, by its id.
    pub fn recibido(&self, id: u64) -> Option<&Recibido> {
        self.recibidos.iter().find(|r| r.id == id)
    }

    /// The person deletes a file that arrived (from Descargas too).
    pub fn borra_recibido(&mut self, id: u64) -> bool {
        let Some(r) = self.recibidos.iter_mut().find(|r| r.id == id && !r.borrado) else {
            return false;
        };
        let ok = std::fs::remove_file(&r.ruta).is_ok() || !r.ruta.exists();
        if ok {
            r.borrado = true;
            self.cambio = true;
        }
        ok
    }

    /// One recorded piece from the page: encrypt it under the media layer and send it.
    pub fn media_saliente(&mut self, b64: &str) -> Result<(), String> {
        let bytes = B64.decode(b64).map_err(|e| e.to_string())?;
        self.media_bytes(bytes)
    }

    fn media_bytes(&mut self, bytes: Vec<u8>) -> Result<(), String> {
        if !self.estado.en_marcha() {
            return Ok(());
        }
        let Some(l) = &mut self.llamada else {
            return Ok(());
        };
        let c = l
            .emisor
            .cifra(&con_remitente("", &bytes))
            .map_err(|e| e.to_string())?;
        l.bytes_enviados += bytes.len() as u64;
        let trama = json!({ "t": "media", "b": B64.encode(c) });
        self.manda(trama);
        self.tasa();
        Ok(())
    }

    /// A piece that someone else in the group recorded, passed on by this computer (the host):
    /// it travels under this link's media key with the original name inside the cipher.
    pub fn media_de(&mut self, de: &str, bytes: &[u8]) -> Result<(), String> {
        if !self.estado.en_marcha() {
            return Ok(());
        }
        let Some(l) = &mut self.llamada else {
            return Ok(());
        };
        let c = l
            .emisor
            .cifra(&con_remitente(de, bytes))
            .map_err(|e| e.to_string())?;
        l.bytes_enviados += bytes.len() as u64;
        let trama = json!({ "t": "media", "b": B64.encode(c) });
        self.manda(trama);
        Ok(())
    }

    /// Something someone else in the group said, passed on by this computer (the host).
    pub fn texto_de(&mut self, de: &str, texto: &str) {
        if self.fase != Fase::Lista {
            return;
        }
        let t: String = texto.chars().take(8000).collect();
        let d: String = de.chars().take(60).collect();
        self.manda_olm(json!({ "m": "texto", "texto": t, "de": d }));
    }

    /// Who is in the group right now, for the other end's screen (sent by the host).
    pub fn grupo_lista(&mut self, nombres: &[String]) {
        if self.fase != Fase::Lista {
            return;
        }
        self.manda_olm(json!({ "m": "grupo", "nombres": nombres }));
    }

    /// The group's members as the host told them (empty when this is a two-way talk).
    pub fn grupo(&self) -> &[String] {
        &self.grupo
    }

    fn media_entrante(&mut self, v: &Value) {
        if !self.estado.en_marcha() {
            return;
        }
        let Some(b) = v["b"].as_str().and_then(|s| B64.decode(s).ok()) else {
            return;
        };
        let Some(l) = &mut self.llamada else { return };
        let Some(r) = &mut l.receptor else { return };
        match r.abre(&b) {
            Ok(plano) => {
                let (de, pieza) = sin_remitente(&plano);
                l.bytes_recibidos += pieza.len() as u64;
                match &self.movil {
                    // On the phone only a two-person call: a group's pieces come from several people.
                    Some(m) if m.enganchado && m.aprobado && de.is_empty() => {
                        m.manda(crate::movil::Hacia::Media(pieza.to_vec()));
                    }
                    _ => {
                        self.media_para_pagina
                            .push_back((de.to_owned(), B64.encode(pieza)));
                        if self.media_para_pagina.len() > 400 {
                            self.media_para_pagina.pop_front();
                        }
                    }
                }
            }
            Err(_) => {
                self.monitor.anota(
                    ahora_s(),
                    Nivel::Inusual,
                    "un trozo de la llamada no se pudo abrir; descartado",
                );
            }
        }
        self.tasa();
    }

    /// Bitrates for the screen, once a second.
    fn tasa(&mut self) {
        let ahora = ahora_s();
        let Some(l) = &mut self.llamada else { return };
        if ahora > l.marca.0 {
            let dt = ahora - l.marca.0;
            let sube = (l.bytes_enviados - l.marca.1) * 8 / 1000 / dt;
            let baja = (l.bytes_recibidos - l.marca.2) * 8 / 1000 / dt;
            l.marca = (ahora, l.bytes_enviados, l.bytes_recibidos);
            let total = u32::try_from(sube + baja).unwrap_or(u32::MAX);
            if l.video {
                self.monitor.transporte.kbps_video = Some(total);
            } else {
                self.monitor.transporte.kbps_audio = Some(total);
            }
            self.cambio = true;
        }
    }

    /// Whether a call is on, and with video.
    pub fn llamada_activa(&self) -> Option<(bool, u64)> {
        self.llamada.as_ref().map(|l| (l.video, l.desde))
    }

    /// The person wrote something.
    pub fn texto(&mut self, texto: &str) {
        if self.fase != Fase::Lista {
            self.aviso("sec_no_lista", "");
            return;
        }
        let t: String = texto.chars().take(8000).collect();
        self.mensajes
            .push((ahora_s(), true, t.clone(), String::new()));
        self.manda_olm(json!({ "m": "texto", "texto": t }));
        self.cambio = true;
    }

    /// A file starts: the page will hand the pieces over.
    pub fn archivo_empieza(&mut self, nombre: &str, bytes: u64) -> Result<(), String> {
        if self.fase != Fase::Lista {
            return Err("sec_no_lista".to_owned());
        }
        let nombre = limpia_nombre(nombre);
        let trozos = Trozos::new().map_err(|e| e.to_string())?;
        // The key first: the other side writes each block to disk as it comes.
        self.manda_olm(json!({ "m": "archivo_inicio", "nombre": nombre, "llave": trozos.llave_inicial(), "bytes": bytes }));
        let inicio = self.enlace.as_ref().map_or(0, |e| e.cuenta().0);
        self.enviados.push(Enviado {
            nombre: nombre.clone(),
            t: 0,
            bytes,
            empezo: std::time::Instant::now(),
            llego_en: None,
        });
        self.envio = Some(Envio {
            nombre: nombre.clone(),
            bytes,
            inicio,
            fin: None,
            marcas: VecDeque::new(),
            aviso: std::time::Instant::now(),
        });
        self.salida = Some(Salida {
            nombre,
            trozos,
            bytes: 0,
        });
        self.cambio = true;
        Ok(())
    }

    /// The file going out: how far, and the seconds left at the speed of the last seconds.
    fn sigue_envio(&mut self) {
        let Some((encolados, escritos)) = self.enlace.as_ref().map(Enlace::cuenta) else {
            self.envio = None;
            return;
        };
        let Some(v) = &mut self.envio else {
            return;
        };
        let ahora = std::time::Instant::now();
        if v.marcas
            .back()
            .is_none_or(|(t, _)| ahora.duration_since(*t) >= Duration::from_millis(250))
        {
            v.marcas.push_back((ahora, escritos));
            while v.marcas.len() > 24 {
                v.marcas.pop_front();
            }
        }
        if v.fin.is_some_and(|f| escritos >= f) {
            self.envio = None;
            self.cambio = true;
            return;
        }
        let _ = encolados;
        if ahora.duration_since(v.aviso) >= Duration::from_millis(500) {
            v.aviso = ahora;
            self.cambio = true;
        }
    }

    /// The file going out, for the screen (null when none).
    pub fn envio_json(&self) -> Value {
        let (Some(v), Some(e)) = (&self.envio, &self.enlace) else {
            return Value::Null;
        };
        let (encolados, escritos) = e.cuenta();
        // In base64 inside JSON a file takes about 4/3 of its size, plus a little per block.
        let total = v
            .fin
            .map_or(v.bytes.saturating_mul(4) / 3 + v.bytes / 400 + 1, |f| {
                f.saturating_sub(v.inicio)
            })
            .max(1);
        let hecho = escritos.saturating_sub(v.inicio).min(total);
        let velocidad = match (v.marcas.front(), v.marcas.back()) {
            (Some((t0, b0)), Some((t1, b1))) if t1 > t0 => {
                (b1.saturating_sub(*b0)) as f64 / t1.duration_since(*t0).as_secs_f64()
            }
            _ => 0.0,
        };
        let restante =
            (velocidad > 1.0).then(|| ((total - hecho) as f64 / velocidad).ceil() as u64);
        json!({
            "nombre": v.nombre,
            "bytes": v.bytes,
            "pct": (hecho * 100 / total).min(if v.fin.is_some() { 100 } else { 99 }),
            "restante_s": restante,
            "kbps": (velocidad * 8.0 / 1000.0) as u64,
            "cola": encolados.saturating_sub(escritos),
        })
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
            let hasta = self.enlace.as_ref().map_or(0, |e| e.cuenta().0);
            if let Some(v) = &mut self.envio {
                v.fin = Some(hasta);
            }
            if let Some(x) = self
                .enviados
                .iter_mut()
                .rev()
                .find(|x| x.nombre == nombre && x.t == 0)
            {
                x.t = ahora_s();
            }
            self.archivos.enviados += 1;
            self.archivos.bytes += s.bytes;
            self.monitor
                .anota(ahora_s(), Nivel::Info, format!("archivo enviado: {nombre}"));
            self.mensajes
                .push((ahora_s(), true, format!("📎 {nombre}"), String::new()));
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

    /// Talking (verified and the cipher open).
    pub fn lista(&self) -> bool {
        self.fase == Fase::Lista
    }

    /// Nobody on the other end and nothing in progress.
    pub fn sola(&self) -> bool {
        self.fase == Fase::Sola
    }

    /// The other end's name once verified (or named).
    pub fn contacto(&self) -> &str {
        &self.nombre_contacto
    }

    /// The listening port, the relay code and the door code, for a group to reuse.
    pub fn escucha(&self) -> (u16, Option<String>, Option<String>) {
        (
            self.puerto,
            self.rele_codigo.clone(),
            self.puerta.as_ref().map(crate::puerta::Puerta::codigo),
        )
    }

    // ---------- The phone (15 Oct 2026) ----------

    /// Hand the conversation to the phone: start the listener, ask the router for the UDP door
    /// and put the QR code on screen. `idioma` goes into the QR so the page opens in the
    /// person's language.
    pub fn movil_empieza(&mut self, idioma: &str) -> Result<(), String> {
        if self.fase != Fase::Lista {
            return Err("sec_no_lista".to_owned());
        }
        if self.movil.is_some() {
            return Ok(());
        }
        let m = crate::movil::Movil::arranca()?;
        let puerto = m.puerto;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(crate::puerta::abre_udp(puerto, 3 * 3600));
        });
        self.movil_idioma = idioma.chars().take(5).collect();
        self.movil = Some(m);
        self.movil_puerta = None;
        self.movil_puerta_rx = Some(rx);
        self.movil_puerta_error = None;
        self.movil_error = None;
        self.monitor.anota(
            ahora_s(),
            Nivel::Info,
            "paso al móvil preparado (código QR)",
        );
        self.cambio = true;
        Ok(())
    }

    /// The page said where this computer is (its time zone); the other side hears it once the
    /// conversation is open. Nothing else about the place travels.
    pub fn pon_zona(&mut self, zona: &str) {
        let z = zona_limpia(zona);
        if z == self.zona_propia {
            return;
        }
        self.zona_propia = z;
        self.manda_zona();
    }

    fn manda_zona(&mut self) {
        if self.fase == Fase::Lista && !self.zona_propia.is_empty() {
            let z = self.zona_propia.clone();
            self.manda_olm(json!({ "m": "zona", "zona": z }));
        }
    }

    /// This link goes through the relay.
    pub fn por_rele(&self) -> bool {
        self.rele
    }

    /// The other side's time zone, once it said it.
    pub fn zona_otra(&self) -> &str {
        &self.zona_otra
    }

    /// Tell the other side what our player can open (empty: the computer's own, anything).
    pub fn reproduce(&mut self, mimes: Vec<String>) {
        if mimes == self.reproduce_dicho || self.fase != Fase::Lista {
            return;
        }
        self.manda_olm(json!({ "m": "reproduce", "mimes": mimes }));
        self.reproduce_dicho = mimes;
    }

    /// The person takes the conversation back to the computer (or it ended).
    pub fn movil_fin(&mut self) {
        self.reproduce(Vec::new());
        if self.movil.take().is_some() {
            self.monitor
                .anota(ahora_s(), Nivel::Info, "paso al móvil terminado");
            self.cambio = true;
        }
        self.movil_puerta_rx = None;
        self.movil_puerta_error = None;
        if let Some(p) = self.movil_puerta.take() {
            std::thread::spawn(move || p.cierra());
        }
    }

    /// The person said «OK» to the phone that connected.
    pub fn movil_aprueba(&mut self) {
        if let Some(m) = &mut self.movil {
            m.aprueba();
            self.monitor
                .anota(ahora_s(), Nivel::Info, "móvil aceptado por la persona");
            self.cambio = true;
        }
    }

    /// Whether the phone is the one talking right now.
    fn movil_manda(&self) -> bool {
        self.movil
            .as_ref()
            .is_some_and(|m| m.enganchado && m.aprobado)
    }

    fn movil_tic(&mut self) {
        if let Some(rx) = &self.movil_puerta_rx {
            if let Ok(r) = rx.try_recv() {
                match r {
                    Ok(p) => {
                        if let Some(m) = &mut self.movil {
                            m.con_puerta(p.externa);
                        }
                        self.movil_puerta = Some(p);
                    }
                    Err(e) => self.movil_puerta_error = Some(e.to_string()),
                }
                self.movil_puerta_rx = None;
                self.cambio = true;
            }
        }
        let Some(m) = &mut self.movil else {
            return;
        };
        let llegado = m.recoge();
        if llegado.is_empty() {
            return;
        }
        let aprobado = m.aprobado;
        for d in llegado {
            use crate::movil::Desde;
            match d {
                Desde::Enganchado => {
                    self.aviso("sec_mv_enganchado", "");
                    self.novedad = true;
                    self.monitor
                        .anota(ahora_s(), Nivel::Info, "un móvil se ha conectado");
                }
                Desde::Soltado => {
                    self.aviso("sec_mv_soltado", "");
                    self.monitor
                        .anota(ahora_s(), Nivel::Info, "el móvil se ha desconectado");
                }
                Desde::Error(e) => {
                    self.movil_error = Some(e);
                    self.movil = None;
                    self.cambio = true;
                    return;
                }
                Desde::Visto => self.novedad = false,
                Desde::Reproduce(mimes) if aprobado => self.reproduce(mimes),
                Desde::Texto(t) if aprobado => self.texto(&t),
                Desde::Llamar { video, mime } if aprobado => {
                    let _ = self.llamar(video, &mime);
                }
                Desde::Colgar if aprobado => self.colgar(),
                Desde::Contestar(mime) if aprobado => {
                    let _ = self.contestar(&mime);
                }
                Desde::Rechazar if aprobado => self.rechazar(),
                Desde::Media(b) if aprobado => {
                    let _ = self.media_bytes(b);
                }
                _ => {}
            }
        }
        self.cambio = true;
    }

    fn movil_json(&self) -> Value {
        let Some(m) = &self.movil else {
            return match &self.movil_error {
                Some(e) => json!({ "error": e }),
                None => Value::Null,
            };
        };
        let enlace = m.enlace(&self.movil_idioma, &self.nombre_contacto);
        json!({
            "enganchado": m.enganchado,
            "pendiente": m.pendiente,
            "aprobado": m.aprobado,
            "habla": self.movil_manda(),
            "qr": tarjeta::qr_svg(&enlace).ok(),
            "fuera": self.movil_puerta.is_some(),
            "puerta_error": self.movil_puerta_error,
            "error": self.movil_error,
        })
    }

    /// After anything changed: the phone gets the same state the tab draws (minus the QR).
    pub fn movil_sincroniza(&self) {
        let Some(m) = &self.movil else { return };
        if !m.enganchado {
            return;
        }
        if !m.aprobado {
            // Nothing of the conversation leaves until the person says OK on this screen.
            m.manda(crate::movil::Hacia::Estado(
                json!({ "tipo": "secure_estado", "movil": { "pendiente": true } }).to_string(),
            ));
            return;
        }
        let mut e = self.estado_json();
        if let Some(mv) = e.get_mut("movil") {
            if let Some(o) = mv.as_object_mut() {
                o.remove("qr");
            }
        }
        m.manda(crate::movil::Hacia::Estado(e.to_string()));
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
            Fase::Buscando => "buscando",
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
            "codigo": self.puerta.as_ref().map(crate::puerta::Puerta::codigo),
            "puerta_error": self.puerta_error,
            "reproduce_otro": self.reproduce_otro,
            "rele_codigo": self.rele_codigo,
            "rele": self.rele,
            "rele_error": self.rele_error,
            "grupo": self.grupo,
            "huella": self.huella_propia(),
            "tarjeta": self.tarjeta(),
            "contacto": self.nombre_contacto,
            "numeros": self.numeros().map(|(a, b, c)| vec![a, b, c]),
            "llamada": self.llamada.as_ref().map(|l| json!({ "video": l.video, "desde": l.desde, "conectada": l.conectada, "mi_numero": self.mi_numero(), "mime_remoto": l.mime_remoto })),
            "entrante": self.entrante.as_ref().map(|t| json!({ "video": t.video, "de": self.nombre_contacto, "desde": t.desde })),
            "perdida": self.perdida.map(|(t, video)| json!({ "t": t, "video": video, "de": self.nombre_contacto })),
            "recibidos": self.recibidos.iter().map(Recibido::json).collect::<Vec<_>>(),
            "envio": self.envio_json(),
            "enviados": self.enviados.iter().filter(|x| x.t > 0).map(|x| json!({ "nombre": x.nombre, "t": x.t, "bytes": x.bytes, "llego_en": x.llego_en })).collect::<Vec<_>>(),
            "recibiendo": self.entrada.as_ref().filter(|e| e.recepcion.is_some()).map(|e| json!({ "nombre": e.nombre, "bytes": e.esperado, "hecho": e.recibido })),
            "movil": self.movil_json(),
            "mensajes": self.mensajes.iter().rev().take(200).rev().map(|(t, mio, x, de)| json!({ "t": t, "mio": mio, "texto": x, "de": de })).collect::<Vec<_>>(),
            "contactos": self.agenda.contactos().iter().map(|c| json!({ "nombre": c.nombre, "verificado_en": c.verificado_en, "estado": matches!(c.estado, agenda::Estado::Verificado) })).collect::<Vec<_>>(),
            "monitor": monitor,
            "resumen": self.resumen.as_ref().map(|r| json!({ "json": r.to_json(), "csv": r.to_csv(), "clase": serde_json::to_value(r.clase).unwrap_or(Value::Null), "contacto": r.contacto, "duracion_s": r.duracion_s(), "mensajes": r.mensajes_enviados + r.mensajes_recibidos, "archivos": r.archivos.enviados + r.archivos.recibidos, "observadas": r.observadas, "inusuales": r.inusuales, "fin": r.fin })),
        })
    }
}

/// An IANA time zone name as the page gives it (`America/Bogota`): letters, digits, `/`, `_`,
/// `-` and `+`, at most 64 characters; anything else is left out.
fn zona_limpia(z: &str) -> String {
    let ok = z
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-' | '+'));
    if ok && z.len() <= 64 && z.contains('/') {
        z.to_owned()
    } else {
        String::new()
    }
}

/// Up to eight container/codec strings from a JSON array, each at most 80 characters.
fn lista_de_formatos(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .take(8)
                .map(|s| s.chars().take(80).collect())
                .collect()
        })
        .unwrap_or_default()
}

/// One byte of name length, the name, the piece: the sender travels inside the cipher.
fn con_remitente(de: &str, pieza: &[u8]) -> Vec<u8> {
    let nombre: Vec<u8> = de.bytes().take(60).collect();
    let mut v = Vec::with_capacity(1 + nombre.len() + pieza.len());
    v.push(nombre.len() as u8);
    v.extend_from_slice(&nombre);
    v.extend_from_slice(pieza);
    v
}

fn sin_remitente(plano: &[u8]) -> (&str, &[u8]) {
    let n = usize::from(plano.first().copied().unwrap_or(0));
    if plano.len() < 1 + n {
        return ("", &[]);
    }
    (
        std::str::from_utf8(&plano[1..1 + n]).unwrap_or(""),
        &plano[1 + n..],
    )
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

/// A port for a new invitation, at random in 20000-60999 (above the well-known and registered
/// ranges most routers reserve, below the ephemeral range the system hands out).
fn puerto_al_azar() -> u16 {
    let mut b = [0u8; 2];
    crate::sesion::azar(&mut b);
    20000 + u16::from_be_bytes(b) % 41000
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
    fn every_invitation_gets_its_own_port_and_so_its_own_code() {
        let puertos: std::collections::HashSet<u16> =
            (0..8).map(|_| super::puerto_al_azar()).collect();
        assert!(puertos.len() > 1, "{puertos:?}");
        assert!(
            puertos.iter().all(|p| (20000..61000).contains(p)),
            "{puertos:?}"
        );
        let ip = std::net::Ipv4Addr::new(203, 0, 113, 7);
        let codigos: std::collections::HashSet<String> = puertos
            .iter()
            .map(|&p| crate::puerta::codigo(ip, p))
            .collect();
        assert_eq!(codigos.len(), puertos.len());
    }

    #[test]
    fn two_computers_verify_each_other_then_a_message_and_a_file_cross_encrypted() {
        let base = std::env::temp_dir().join(format!("guardiana-secure-{}", std::process::id()));
        let (da, db) = (base.join("a"), base.join("b"));
        std::fs::create_dir_all(&da).unwrap();
        std::fs::create_dir_all(&db).unwrap();
        let mut a = Secure::abre(&da, &da);
        let mut b = Secure::abre(&db, &db);
        let puerto = a.escuchar_efimero().unwrap();
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

        // An iPhone takes over on a's side and can only open MP4: b hears it, and hears when it
        // goes back.
        a.reproduce(vec!["video/mp4;codecs=avc1.42E01E,mp4a.40.2".to_owned()]);
        gira(&mut a, &mut b, 15);
        assert_eq!(
            b.estado_json()["reproduce_otro"][0],
            "video/mp4;codecs=avc1.42E01E,mp4a.40.2"
        );
        a.movil_fin();
        gira(&mut a, &mut b, 15);
        assert_eq!(b.estado_json()["reproduce_otro"], json!([]));

        // A file in three pieces, the last one short.
        let plano: Vec<u8> = (0..(guardiana_secure::archivo::BLOQUE * 2 + 321))
            .map(|i| (i % 211) as u8)
            .collect();
        a.archivo_empieza("informe.pdf", plano.len() as u64)
            .unwrap();
        let trozos: Vec<&[u8]> = plano.chunks(guardiana_secure::archivo::BLOQUE).collect();
        for (i, t) in trozos.iter().enumerate() {
            a.archivo_trozo(&B64.encode(t), i + 1 == trozos.len())
                .unwrap();
        }
        gira(&mut a, &mut b, 30);
        let recibido = std::fs::read(db.join("informe.pdf")).unwrap();
        assert_eq!(recibido, plano);
        // The receiver said it is whole: the sender knows how long it took, and both screens
        // show the file (a card with its actions on the receiving side).
        assert!(a.enviados[0].llego_en.is_some(), "{:?}", a.enviados);
        assert_eq!(b.recibidos.len(), 1);
        assert_eq!(b.estado_json()["recibidos"][0]["nombre"], "informe.pdf");
        assert!(b.estado_json()["recibidos"][0]["abrible"]
            .as_bool()
            .unwrap());
        assert!(a.estado_json()["envio"].is_null(), "the send is over");
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

    #[test]
    fn two_computers_without_a_door_meet_through_the_blind_relay() {
        // A relay on this computer; the waiting side gets no door (there is no router here),
        // so it shows a GR code; the other side pastes it and the relay joins them.
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let listener = rt
            .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
            .unwrap();
        let direccion = listener.local_addr().unwrap().to_string();
        rt.spawn(guardiana_rele::sirve(listener));
        // The whole test process shares the variable; both sides read it.
        std::env::set_var("GUARDIANA_RELE", &direccion);

        let base = std::env::temp_dir().join(format!("guardiana-rele-{}", std::process::id()));
        let (da, db) = (base.join("a"), base.join("b"));
        std::fs::create_dir_all(&da).unwrap();
        std::fs::create_dir_all(&db).unwrap();
        let mut a = Secure::abre(&da, &da);
        let mut b = Secure::abre(&db, &db);
        a.escuchar_efimero().unwrap();
        // Up to a few seconds for the router search to give up, then the relay code appears.
        let mut codigo = None;
        for _ in 0..600 {
            a.tic();
            if let Some(c) = a.estado_json()["rele_codigo"].as_str() {
                codigo = Some(c.to_owned());
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let codigo = codigo.unwrap();
        assert!(codigo.starts_with("GR-"));
        b.conectar(&codigo).unwrap();
        assert_eq!(b.estado_json()["fase"], "buscando");
        gira(&mut a, &mut b, 40);
        assert_eq!(a.estado_json()["fase"], "comparando", "{}", a.estado_json());
        assert_eq!(a.numeros(), b.numeros());
        a.sas_persona(true, "Bea");
        b.sas_persona(true, "Ana");
        gira(&mut a, &mut b, 25);
        assert_eq!(a.estado_json()["fase"], "lista");
        assert_eq!(a.estado_json()["rele"], true);
        assert_eq!(a.monitor.transporte.directo, Some(false));
        a.texto("por el relé");
        gira(&mut a, &mut b, 15);
        assert_eq!(b.mensajes.last().unwrap().2, "por el relé");
        let _ = std::fs::remove_dir_all(base);
    }
}
