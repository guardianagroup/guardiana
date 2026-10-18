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
use guardiana_secure::cuantico;
use guardiana_secure::destruccion::{self, Motivo as MotivoDestruccion};
use guardiana_secure::monitor::{Identidad, Monitor, Nivel};
use guardiana_secure::pausa::Sesion as EstadoSesion;
use guardiana_secure::prueba::{Cadena, Prueba};
use guardiana_secure::resumen::{Archivos, Clase, Conexion, Resumen, Veredicto, Vigilancia};
use guardiana_secure::sframe;
use guardiana_secure::verificar::{Codigo, Inicio, Verified};
use guardiana_secure::{tarjeta, Conversation, Envelope, Identity};
use serde::Serialize;
use serde_json::{json, Value};

use crate::mensaje::{self, Mensaje};
use sha2::Digest as _;

const MAX_FRAME: usize = 2 * 1024 * 1024;
/// Video calls (18 Oct 2026, the committee — Claude, ChatGPT, Grok, Kimi — and the owner: «si no
/// llegamos a eso, nos quedaríamos solamente con voz»). A web page on a phone cannot reach the
/// quality of FaceTime or WhatsApp (no CallKit, no PushKit, no background on an iPhone), so video
/// is off everywhere: a call asked with video starts as voice, and a video call that arrives from
/// an older version rings as voice. The code stays, behind this switch.
pub const VIDEO: bool = false;
const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// What the network thread tells the session.
#[derive(Debug)]
pub enum Entrante {
    /// Someone connected to our listener: the socket to adopt.
    Aceptado(TcpStream),
    /// The relay joined us with the other computer: the socket to adopt (bytes pass through
    /// the relay, which cannot read them).
    AceptadoRele(TcpStream, String, String),
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
    /// Whether the other end of a direct link sat on this same network (a private, link-local
    /// or loopback address) when the link opened. Only this yes/no is kept, never the address
    /// (18 Oct 2026, the owner: never store IPs; decision 203).
    misma_red: Option<bool>,
}

/// Whether an address belongs to the same local network (private, link-local, loopback or
/// unique-local): what a direct link may say about where the other end was, and all it says.
fn es_misma_red(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v) => v.is_private() || v.is_link_local() || v.is_loopback(),
        std::net::IpAddr::V6(v) => {
            let s = v.segments()[0];
            v.is_loopback() || (s & 0xfe00) == 0xfc00 || (s & 0xffc0) == 0xfe80
        }
    }
}

impl Enlace {
    fn new(mut s: TcpStream) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        let misma_red = s.peer_addr().ok().map(|a| es_misma_red(a.ip()));
        // Small frames (voice) leave at once instead of waiting to be grouped.
        let _ = s.set_nodelay(true);
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
            misma_red,
        }
    }

    /// Whether the other end was on this network (see the field).
    fn misma_red(&self) -> Option<bool> {
        self.misma_red
    }

    fn manda(&self, v: &Value) -> Result<(), String> {
        let mut cuerpo = serde_json::to_vec(v).map_err(|e| e.to_string())?;
        if cuerpo.len() > MAX_FRAME {
            return Err("trama demasiado grande".to_owned());
        }
        // Sizes in buckets (18 Oct 2026, `relleno`): spaces after the JSON, which every reader,
        // older versions included, skips; the relay sees a few sizes instead of every length.
        guardiana_secure::relleno::rellena(&mut cuerpo, b' ', MAX_FRAME);
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
    /// Bytes written on links that dropped since it started (their counters are gone).
    antes: u64,
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
    /// Encrypted blocks the other side has not confirmed yet (16 Oct 2026, «reanudación»):
    /// if the link drops, the other side says how many it has and these go out again from
    /// there. It confirms every [`CONFIRMA_CADA`] blocks, so this stays small.
    guardados: VecDeque<(u64, Vec<u8>)>,
    guardados_bytes: u64,
    /// Blocks are kept but not sent: the link dropped and the other side has not yet said
    /// where it is.
    pausada: bool,
    /// The closing key, once every block was handed over; sent again if the other side asks
    /// after a drop.
    llave: Option<Llave>,
    /// The opening key and the announced size, to start over if the start was lost in a drop.
    inicial: Llave,
    anunciado: u64,
}

/// The receiver confirms every this many blocks (4 MB of 64 KB blocks).
const CONFIRMA_CADA: u64 = 64;
/// Unconfirmed blocks kept for a resend, at most; past this the page waits.
const GUARDADOS_MAX: u64 = 32 * 1024 * 1024;

/// How long a call rings before «no answer».
pub const TIMBRE_S: u64 = 45;

/// The longest a disappearing message or file may wait (a week), whatever the other side asks.
pub const TEMPORIZADOR_MAX: u64 = 7 * 24 * 3600;
/// The timers the screen offers, in seconds (off, 30 s, 5 min, 1 h, 1 day, 1 week).
pub const TEMPORIZADORES: [u64; 6] = [0, 30, 300, 3600, 86_400, 604_800];

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

    /// A sound the page can play in the chat (a voice note, 17 Oct 2026).
    pub fn audio(&self) -> bool {
        mensaje::mime_de(&self.nombre).starts_with("audio/")
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
        json!({ "id": self.id, "nombre": self.nombre, "bytes": self.bytes, "t": self.t, "borrado": self.borrado, "imagen": self.imagen(), "audio": self.audio(), "abrible": self.abrible() })
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
    /// Started from here.
    saliente: bool,
    /// Since when: the call (or, while calling, the ring).
    desde: u64,
    bytes_enviados: u64,
    bytes_recibidos: u64,
    /// For the bitrate on screen: (unix s, bytes at that second).
    marca: (u64, u64, u64),
}

/// A conversation whose link dropped, being picked up again through the relay.
#[derive(Debug, Clone)]
struct Reconexion {
    sala: String,
    desde: u64,
    proximo: u64,
    intentos: u32,
    buscando: bool,
}

/// How long both sides keep trying to meet again before giving the conversation up.
const RECONEXION_S: u64 = 5 * 60;

/// A stop that looks like an interference, as the screen and the saved report show it.
#[derive(Debug, Clone, Serialize)]
pub struct Incidente {
    /// Unix seconds.
    pub cuando: u64,
    /// `sas_distinto`, `clave_cambiada`, `olm_fallos`, `media_fallos`, `archivo_mal`,
    /// `rele_certificado`.
    pub senal: &'static str,
    /// The concrete fact, in Spanish for the chain (the screen translates `senal`).
    pub detalle: String,
    /// `misma_red` (direct, the other end on this network), `directo` (direct across the
    /// internet), `rele` or `ninguno`.
    pub transporte: &'static str,
    /// Through the relay: the relay's city (`Núremberg`, `Miami`), the only place named.
    pub ciudad_rele: Option<&'static str>,
    /// How many frames failed in the window, when that is the signal.
    pub fallos: u32,
    /// The contact's name as this computer knows it.
    pub contacto: String,
    /// The time zone the other side said, if it did (never an address).
    pub zona_otra: String,
    // No address of the other end, not even on a direct link and not in the saved report
    // (18 Oct 2026, the owner: «nunca guardar IPs»): only «directo por la misma red» or
    // «por relé · ciudad».
}

/// The relay's city from its name, for «por relé · ciudad» (no address involved).
fn ciudad_de_rele(rele: &str) -> Option<&'static str> {
    let host = rele.split(':').next().unwrap_or_default();
    if host.starts_with("rele-am.") {
        Some("Miami")
    } else if host == crate::rele::RELE_NOMBRE {
        Some("Núremberg")
    } else {
        None
    }
}

/// The SECURE side of a browser session.
pub struct Secure {
    datos: PathBuf,
    descargas: PathBuf,
    identidad: Identity,
    agenda: Agenda,
    /// Identities this person blocked (Ed25519, base64): their calls end at the greeting.
    bloqueados: Vec<String>,
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
    /// A wait at the relay still in its room after this session met its member another way
    /// (directly): the group's next listener takes it over, so one room never holds two waits
    /// of the same host, which the relay would join with each other (17 Oct 2026).
    espera_rele: Option<mpsc::Receiver<Entrante>>,
    /// This link goes through the blind relay.
    rele: bool,
    rele_error: Option<String>,
    /// The relay this conversation met at (a name, kept in memory only): a dropped link goes
    /// back to that same one, where the other side also goes (17 Oct 2026: with one relay
    /// down, or one side pinned to another, both sides waited at different relays and the
    /// conversation died after five minutes).
    rele_usado: Option<String>,
    /// The link dropped in the middle of an open conversation and both sides are going back
    /// to the relay to pick it up where it was (16 Oct 2026, «que sirva en cualquier router e
    /// internet del mundo»): the room both derive, when the next try is due, how many tries.
    /// The Olm session carries on; only the socket is new. A deliberate close sends «adios»
    /// first, so the other side does not wait.
    reconexion: Option<Reconexion>,
    tarjeta_otra: Option<tarjeta::Tarjeta>,
    sas_inicio: Option<Inicio>,
    sas_codigo: Option<Codigo>,
    sas_yo_ok: bool,
    sas_mac_otra: Option<(Mac, Ed25519Signature)>,
    verificado: Option<Verified>,
    conversacion: Option<Conversation>,
    /// Messages for the screen, with their state (see [`crate::mensaje`]).
    pub mensajes: Vec<Mensaje>,
    /// Ids received and not yet reported as seen (16 Oct 2026: the ✓✓ in green).
    por_ver: Vec<u64>,
    /// The other side is typing: unix seconds of its last «escribiendo».
    otro_escribe: u64,
    /// Our last «escribiendo» sent, so one goes every few seconds at most.
    escribi_en: u64,
    /// The last piece of a file coming from the phone, kept until the next one (or the end)
    /// says whether it is the last.
    movil_trozo: Option<Vec<u8>>,
    /// The person turned the read receipts off: nothing is reported as seen (they then see no
    /// green ticks either, as in the big apps).
    pub sin_visto: bool,
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
    /// A stop that looks like an interference (16 Oct 2026, the owner's first concern): what
    /// tripped, when, by which path, and what can honestly be said about the other end. Kept
    /// until the person saves it, verifies again or ends; `cerrar` leaves it alone.
    incidente: Option<Incidente>,
    /// Frames that failed to open lately (unix seconds): a burst stops the conversation, one
    /// alone is a bad network.
    fallos_olm: VecDeque<u64>,
    fallos_media: VecDeque<u64>,
    /// How many times the other side asked our recorder to start again (a new player there,
    /// on its phone or back on its computer, cannot open a stream joined halfway).
    reinicios: u64,
    /// Our call's pieces were dropped because the link was full (see `media_atascada`).
    media_saltada: bool,
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
    /// Whether the other side is talking from its phone (it says so inside Olm each time that
    /// changes), and what we last told it about ours. The screen shows the protection level
    /// from the two: both computers, the highest; a phone on either side, strong (16 Oct 2026).
    otro_movil: bool,
    movil_dicho: Option<bool>,
    /// «Voz protegida» on each side: ours (the page says it; told to the other side) and
    /// theirs (shown on the call panel). 16 Oct 2026.
    voz_protegida: bool,
    otro_voz_protegida: bool,
    /// The relay room this conversation met in (a hash; kept in memory only): the call
    /// service is asked for it.
    sala_rele: Option<String>,
    /// The other side's program can carry calls over WebRTC (17 Oct 2026).
    otro_rtc: bool,
    /// The relay's call service for this conversation, once it answered.
    turn: Option<crate::rele::Turn>,
    turn_rx: Option<mpsc::Receiver<Result<crate::rele::Turn, String>>>,
    /// When it was last asked (unix s), so a relay without the service is not asked each tick.
    turn_pedido: u64,
    /// WebRTC messages from the other side for this computer's page (SDP, ICE candidates).
    pub rtc_para_pagina: VecDeque<Value>,
    /// «Continuar en el móvil» without a gap (18 Oct 2026, [`crate::relevo`]): where the
    /// call's voice is, and the bridge's messages from the phone for this computer's page.
    relevo: crate::relevo::Relevo,
    /// The phone's bridge messages for the page (offer, ICE candidates, end).
    pub puente_para_pagina: VecDeque<Value>,
    /// How the computer's own leg of the call sounds, as the page estimated it (`excelente`,
    /// `buena`, `debil`) and when: the phone shows the worse of its leg and this one.
    calidad_pc: Option<(String, u64)>,
    /// The post-quantum lock over Olm (16 Oct 2026, [`cuantico`]): side 0 offers an ML-KEM
    /// key once the conversation opens, side 1 answers, and from then every Olm payload is
    /// sealed with the layer too. `cuantico_visto`: a sealed message already came from the
    /// other side, so a plain one now would be a downgrade.
    cuantico_oferta: Option<(cuantico::Oferta, Vec<u8>)>,
    capa: Option<cuantico::Capa>,
    cuantico_visto: bool,
    /// When the layer's current epoch began (unix s): a new ML-KEM encapsulation is due every
    /// [`cuantico::RENUEVA_CADA`] messages or [`cuantico::RENUEVA_S`] seconds (18 Oct 2026), and
    /// the shield shows how long ago it was renewed.
    capa_desde: u64,
    /// Epochs of the layer in this conversation (the first one counts).
    capa_epocas: u64,
    /// When the numbers were confirmed on both sides in this conversation (unix s).
    verificado_en: u64,
    /// Messages that opened (Olm and the layer) and that did not, and file blocks that passed
    /// their authentication and that did not: the shield's integrity line, from real events.
    abiertos_ok: u64,
    abiertos_mal: u64,
    trozos_ok: u64,
    trozos_mal: u64,
    /// When the window last handed the watch a look (unix s).
    vigia_visto: u64,
    /// Destroy the conversation on an anomaly (18 Oct 2026, the owner: «si detectamos espionaje o
    /// anomalía, destrucción del contenido automáticamente»). On by default; the person may turn
    /// it off, and then the old pause applies.
    pub auto_destruccion: bool,
    /// The last destruction, for the calm notice, until the person closes it.
    destruida: Option<Value>,
    /// Messages and files that disappear after this many seconds (0: they stay until the
    /// conversation ends). Both sides use the same value: changing it tells the other side.
    temporizador: u64,
    /// Files received with a timer: (id, unix s when they go).
    caducan: Vec<(u64, u64)>,
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
    /// The last call's report, signed, with its verdict (17 Oct 2026: «la valoración final:
    /// si ha habido espionaje, si ha habido grabación»), until the person closes it.
    pub informe: Option<Resumen>,
    /// The call's figures as the page measured them (WebRTC), refreshed every few seconds.
    medidas: Option<Conexion>,
    /// The watch over the microphone, the camera and the screen (Windows' record).
    vigia: crate::vigia::Vigia,
    /// What the other side's GUARDIANA said its own watch saw.
    vigia_otro: Option<Vigilancia>,
    /// The last verdict sent to the other side, so it is sent only when it changes.
    vigia_mandado: Option<Value>,
    /// When a stop for something seen may end (the cause gone and the countdown run).
    reanuda_en: Option<u64>,
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
        let bloqueados = std::fs::read_to_string(carpeta.join("bloqueados.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let mut s = Self {
            datos: carpeta,
            descargas: descargas.to_path_buf(),
            bloqueados,
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
            incidente: None,
            fallos_olm: VecDeque::new(),
            fallos_media: VecDeque::new(),
            reinicios: 0,
            media_saltada: false,
            envio: None,
            ultimo_envio: std::time::Instant::now(),
            ultimo_recibido: std::time::Instant::now(),
            otro_late: false,
            otro_movil: false,
            movil_dicho: None,
            voz_protegida: false,
            otro_voz_protegida: false,
            sala_rele: None,
            otro_rtc: false,
            turn: None,
            turn_rx: None,
            turn_pedido: 0,
            rtc_para_pagina: VecDeque::new(),
            relevo: crate::relevo::Relevo::default(),
            puente_para_pagina: VecDeque::new(),
            calidad_pc: None,
            cuantico_oferta: None,
            capa: None,
            cuantico_visto: false,
            capa_desde: 0,
            capa_epocas: 0,
            verificado_en: 0,
            abiertos_ok: 0,
            abiertos_mal: 0,
            trozos_ok: 0,
            trozos_mal: 0,
            vigia_visto: 0,
            auto_destruccion: true,
            destruida: None,
            temporizador: 0,
            caducan: Vec::new(),
            zona_propia: String::new(),
            zona_otra: String::new(),
            enviados: Vec::new(),
            recibidos: Vec::new(),
            llamado_en_grupo: false,
            tx: None,
            rele_codigo: None,
            espera_rele: None,
            rele: false,
            rele_error: None,
            rele_usado: None,
            reconexion: None,
            tarjeta_otra: None,
            sas_inicio: None,
            sas_codigo: None,
            sas_yo_ok: false,
            sas_mac_otra: None,
            verificado: None,
            conversacion: None,
            mensajes: Vec::new(),
            por_ver: Vec::new(),
            otro_escribe: 0,
            escribi_en: 0,
            sin_visto: false,
            movil_trozo: None,
            monitor: Monitor::new(),
            cadena: Cadena::new("sin sesion", ahora_s()),
            estado: EstadoSesion::default(),
            entrada: None,
            salida: None,
            llamada: None,
            archivos: Archivos::default(),
            resumen: None,
            informe: None,
            medidas: None,
            vigia: crate::vigia::Vigia::default(),
            vigia_otro: None,
            vigia_mandado: None,
            reanuda_en: None,
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
        crate::rele::mide_cercania();
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

    /// This computer's GUARDIANA number (1.0.42): from its identity key, the same forever.
    #[must_use]
    pub fn numero_propio(&self) -> String {
        agenda::numero(self.identidad.signing_key())
    }

    /// Block the person on the other end (their identity, not their number), and end.
    pub fn bloquear(&mut self) {
        if let Some(t) = &self.tarjeta_otra {
            let k = t.signing().to_base64();
            if !self.bloqueados.contains(&k) {
                self.bloqueados.push(k);
                let _ = std::fs::write(
                    self.datos.join("bloqueados.json"),
                    serde_json::to_string(&self.bloqueados).unwrap_or_default(),
                );
            }
        }
        self.cerrar("bloqueado por la persona");
    }

    /// Connect to the other computer. `direccion` is `host:puerto`, an invitation code or a
    /// GUARDIANA number; it is used now and kept nowhere.
    pub fn conectar(&mut self, direccion: &str) -> Result<(), String> {
        self.cerrar("nueva conexión");
        let por_numero = crate::rele::sala_de_numero(direccion);
        if let Some(n) = por_numero.as_ref().and(agenda::lee_numero(direccion)) {
            // Calling a number saved in the book: whoever answers must hold the key verified
            // for it, or the conversation stops as a possible impersonation (see `hola`).
            if let Some(c) = self
                .agenda
                .contactos()
                .iter()
                .find(|c| agenda::numero_de_base64(&c.signing).as_deref() == Some(n.as_str()))
            {
                self.nombre_contacto = c.nombre.clone();
            }
        }
        if let Some(sala) = por_numero.or_else(|| crate::rele::sala_de(direccion)) {
            // The other person waits at a relay: go to all of them and keep the one where
            // the pair completes.
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let r = crate::rele::conecta_a_cualquiera(&sala);
                let _ = tx.send(match r {
                    Ok((s, rele)) => Entrante::AceptadoRele(s, rele, sala),
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
        if !rele && self.fase == Fase::Escuchando && self.rele_codigo.is_some() {
            // Met directly while also waiting at the relay: that wait stays in its room, and
            // the next listener of the group takes it (see `espera_rele`).
            self.espera_rele = self.rx.take();
        }
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
        let conexion = self.medidas.clone().unwrap_or_else(|| Conexion {
            directa: self.monitor.transporte.directo,
            codec_audio: self.monitor.transporte.codec_audio.clone(),
            codec_video: self.monitor.transporte.codec_video.clone(),
            ..Conexion::default()
        });
        let veredicto = self.veredicto(self.inicio, ahora_s());
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
        )
        .map(|r| r.con_veredicto(&self.identidad, veredicto));
        self.archivos = Archivos::default();
        self.vigia = crate::vigia::Vigia::default();
        self.vigia_otro = None;
        self.vigia_mandado = None;
    }

    /// The verdict for the part of the conversation between `desde` and `hasta`.
    fn veredicto(&self, desde: u64, hasta: u64) -> Veredicto {
        let interferencias = self
            .cadena
            .registros
            .iter()
            .filter(|r| r.nivel == Nivel::Inusual && r.cuando >= desde && r.cuando <= hasta)
            .count() as u64;
        Veredicto {
            este: self.vigia.veredicto(desde),
            otro: self.vigia_otro.clone(),
            otro_movil: self.otro_movil,
            identidad_verificada: self
                .monitor
                .identidad
                .as_ref()
                .is_some_and(|i| i.estado == "verificado"),
            interferencias,
        }
    }

    /// The report of a call that just ended (it was answered): signed, with the verdict and the
    /// figures the page measured, for the page to show at once and to download.
    fn cierra_informe(&mut self, video: bool, desde: u64) {
        if self.monitor.identidad.is_none() {
            return;
        }
        let fin = ahora_s();
        let conexion = self.medidas.take().unwrap_or_else(|| Conexion {
            directa: self.monitor.transporte.directo,
            codec_audio: self.monitor.transporte.codec_audio.clone(),
            codec_video: self.monitor.transporte.codec_video.clone(),
            ..Conexion::default()
        });
        let veredicto = self.veredicto(desde, fin);
        self.informe = Resumen::cerrar_parte(
            &self.identidad,
            &self.version,
            if video {
                Clase::Videollamada
            } else {
                Clase::Llamada
            },
            &self.monitor,
            &self.cadena,
            desde,
            fin,
            Archivos::default(),
            conexion,
            veredicto,
        );
        self.cambio = true;
    }

    /// The person closed the call's report.
    pub fn olvida_informe(&mut self) {
        if self.informe.take().is_some() {
            self.cambio = true;
        }
    }

    /// The call's figures as the page measures them (every few seconds during a call): what the
    /// report will say about the connection. Only numbers and codec names, bounded.
    pub fn medidas(&mut self, d: &Value) {
        if self.llamada.is_none() {
            return;
        }
        let n = |k: &str, max: u64| {
            d[k].as_u64()
                .filter(|v| *v <= max)
                .and_then(|v| u32::try_from(v).ok())
        };
        let corto = |k: &str| {
            d[k].as_str().filter(|s| !s.is_empty()).map(|s| {
                s.chars()
                    .filter(|c| !c.is_control())
                    .take(40)
                    .collect::<String>()
            })
        };
        self.medidas = Some(Conexion {
            directa: Some(!d["rele"].as_bool().unwrap_or(true)),
            latencia_media_ms: n("latencia_media_ms", 60_000),
            perdida_max_pct: d["perdida_max_pct"]
                .as_f64()
                .filter(|p| (0.0..=100.0).contains(p))
                .map(|p| p as f32),
            kbps_audio_medio: n("kbps_audio_medio", 100_000),
            kbps_video_medio: n("kbps_video_medio", 100_000),
            codec_audio: corto("codec_audio"),
            codec_video: corto("codec_video"),
            resolucion: corto("resolucion"),
        });
    }

    /// What Windows recorded about the microphone, the camera and the screen (the window looks
    /// every two seconds while a conversation is open). Another program using one of them during
    /// the conversation goes into the history and the chain with its name and path, and stops
    /// the conversation while it goes on (Francisco, 15 Oct 2026: «que pare automáticamente»);
    /// it resumes on its own once that program lets go, or at once if the person says it is
    /// theirs.
    pub fn vigia(&mut self, usos: &[crate::vigia::Uso]) {
        if self.fase != Fase::Lista {
            return;
        }
        let ahora = ahora_s();
        self.vigia_visto = ahora;
        let antes = self.vigia.en_uso.clone();
        for v in self.vigia.mira(usos, self.inicio) {
            let texto = format!("«{}» {}", v.uso.programa, v.uso.recurso.verbo());
            let hecho = guardiana_secure::prueba::Hecho {
                texto: texto.clone(),
                proceso: Some(v.uso.programa.clone()),
                ruta: v.uso.ruta.clone(),
                sha256_exe: None,
                evidencia: Some(format!(
                    "Windows ConsentStore · {} · LastUsedTimeStart {} · {}",
                    match v.uso.recurso {
                        crate::vigia::Recurso::Microfono => "microphone",
                        crate::vigia::Recurso::Camara => "webcam",
                        crate::vigia::Recurso::Pantalla => "graphicsCaptureProgrammatic",
                    },
                    v.uso.desde,
                    v.uso
                        .hasta
                        .map_or_else(|| "en uso".to_owned(), |h| format!("LastUsedTimeStop {h}")),
                )),
            };
            // Something that *started* during the conversation destroys it (18 Oct 2026); a
            // program that was already using it before keeps the old pause, so a person can
            // close it and go on.
            if v.ahora && self.auto_destruccion && v.uso.desde >= self.inicio {
                self.cadena.anota(ahora, Nivel::Observada, hecho);
                let p = v.uso.programa.clone();
                self.destruir(match v.uso.recurso {
                    crate::vigia::Recurso::Microfono => MotivoDestruccion::Microfono(p),
                    crate::vigia::Recurso::Camara => MotivoDestruccion::Camara(p),
                    crate::vigia::Recurso::Pantalla => MotivoDestruccion::Captura(p),
                });
                return;
            }
            if v.ahora && self.estado.en_marcha() {
                // The stop writes its own line in the history and the chain.
                self.cadena.anota(ahora, Nivel::Observada, hecho);
                self.estado.parar(
                    guardiana_secure::pausa::Motivo::Observado {
                        texto,
                        proceso: Some(v.uso.programa.clone()),
                    },
                    ahora,
                    &mut self.monitor,
                    &mut self.cadena,
                );
                self.aviso("sec_vigia_parada", v.uso.programa.clone());
            } else {
                self.monitor.anota(ahora, Nivel::Observada, texto);
                self.cadena.anota(ahora, Nivel::Observada, hecho);
            }
            self.cambio = true;
        }
        // A stop for something seen: what still keeps it, and the way back once it is gone.
        let por_algo_visto = matches!(
            &self.estado,
            EstadoSesion::Pausada(p) if matches!(p.motivo, guardiana_secure::pausa::Motivo::Observado { .. })
        );
        {
            if por_algo_visto {
                let pendiente: Vec<String> = self
                    .vigia
                    .en_uso
                    .iter()
                    .map(|(p, r)| format!("«{p}» {}", r.ahora()))
                    .collect();
                self.estado.comprobada(pendiente);
                if self.estado.lista_para_reanudar() {
                    match self.reanuda_en {
                        None => {
                            self.reanuda_en =
                                Some(ahora + u64::from(guardiana_secure::pausa::CUENTA_ATRAS_S));
                        }
                        Some(t) if ahora >= t => {
                            self.reanuda_en = None;
                            self.estado
                                .reanudar(ahora, &mut self.monitor, &mut self.cadena);
                        }
                        Some(_) => {}
                    }
                } else {
                    self.reanuda_en = None;
                }
                self.cambio = true;
            }
        }
        if antes != self.vigia.en_uso {
            self.cambio = true;
        }
        self.manda_vigia();
    }

    /// Whether the window is kept out of screenshots and recordings (the window says, after
    /// asking Windows; `false` where it could not).
    pub fn pantalla(&mut self, protegida: bool) {
        if self.fase != Fase::Lista {
            return;
        }
        if self.vigia.pantalla_protegida != protegida {
            self.vigia.pon_pantalla(protegida);
            self.cambio = true;
            self.manda_vigia();
        }
    }

    /// Tell the other side what this computer's watch sees, when it changed: its report says
    /// it too («en el equipo de ferrari, según su GUARDIANA»).
    fn manda_vigia(&mut self) {
        if self.fase != Fase::Lista || !self.estado.en_marcha() || !self.vigia.activa {
            return;
        }
        let v = serde_json::to_value(self.vigia.veredicto(self.inicio)).unwrap_or(Value::Null);
        if self.vigia_mandado.as_ref() == Some(&v) {
            return;
        }
        self.vigia_mandado = Some(v.clone());
        self.manda_olm(json!({ "m": "vigia", "v": v }));
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

    /// Destroy this conversation for an anomaly (18 Oct 2026, the owner: «si detectamos espionaje
    /// o anomalía, destrucción del contenido automáticamente»): the other side is told with a
    /// notice signed by this computer's identity (so it does the same), the event goes into the
    /// signed chain and to disk as signed evidence (no content, no address), and then every key
    /// of the session, every message, every file it brought and anything half received goes.
    /// With [`Secure::auto_destruccion`] off nothing happens here and the old pause applies.
    ///
    /// What it cannot do, and the screen says so: erase what was already saved, copied or
    /// photographed on a device that was compromised.
    pub fn destruir(&mut self, motivo: MotivoDestruccion) {
        if !self.auto_destruccion {
            return;
        }
        self.destruye(motivo, true);
    }

    fn destruye(&mut self, motivo: MotivoDestruccion, avisar: bool) {
        let ahora = ahora_s();
        let clave = motivo.clave();
        if avisar && self.enlace.is_some() && self.fase != Fase::Sola {
            let inicio = self.inicio_destruccion();
            let sesion = self.sesion_destruccion(inicio);
            let firma = destruccion::firma(&self.identidad, &sesion, clave, ahora);
            // Plain, not inside Olm: the session may be the thing that failed. The signature is
            // what the other side trusts.
            self.manda(json!({ "t": "destruir", "motivo": clave, "cuando": ahora, "inicio": inicio, "firma": firma }));
        }
        self.cadena.anota(
            ahora,
            Nivel::Inusual,
            guardiana_secure::prueba::Hecho {
                texto: motivo.texto(),
                proceso: motivo.programa().map(str::to_owned),
                ruta: None,
                sha256_exe: None,
                evidencia: Some(format!("GUARDIANA SECURE · destrucción · {clave}")),
            },
        );
        self.guarda_destruccion(ahora, clave);
        self.monitor.anota(ahora, Nivel::Inusual, motivo.texto());
        self.destruida = Some(json!({
            "cuando": ahora,
            "motivo": clave,
            "programa": motivo.programa(),
            "contacto": self.nombre_contacto,
            "otro_lado": matches!(motivo, MotivoDestruccion::OtroLado(_)),
        }));
        self.reconexion = None;
        // The phone that carries this conversation empties its screen too (guardianagroup.com/m/).
        if let Some(m) = &self.movil {
            m.manda(crate::movil::Hacia::Estado(
                json!({ "tipo": "secure_destruida", "motivo": clave }).to_string(),
            ));
            std::thread::sleep(Duration::from_millis(60));
        }
        self.cerrar("destruida por seguridad");
        // What `cerrar` keeps for a normal end goes too: the lists of files, the summaries.
        self.enviados.clear();
        self.recibidos.clear();
        self.caducan.clear();
        self.resumen = None;
        self.informe = None;
        self.media_para_pagina.clear();
        self.rtc_para_pagina.clear();
        self.puente_para_pagina.clear();
        self.calidad_pc = None;
        self.mensajes.clear();
        self.aviso("sec_destruida", clave);
        self.novedad = true;
        self.cambio = true;
    }

    /// The signed chain of this conversation, kept as evidence of the destruction: what tripped
    /// and when, never a message, a file or an address.
    fn guarda_destruccion(&self, cuando: u64, clave: &str) {
        let carpeta = self.datos.join("destrucciones");
        if std::fs::create_dir_all(&carpeta).is_err() {
            return;
        }
        let prueba = Prueba::firmar(&self.identidad, self.cadena.clone(), &self.version, cuando);
        let _ = std::fs::write(
            carpeta.join(format!("{cuando}-{clave}.json")),
            prueba.to_json(),
        );
    }

    /// The second the destruction notice is bound to: the conversation's opening second once it
    /// is open (both sides share it), 0 before.
    fn inicio_destruccion(&self) -> u64 {
        if self.fase == Fase::Lista {
            self.inicio
        } else {
            0
        }
    }

    /// The session a notice names: both identity keys, in byte order, and `inicio`.
    fn sesion_destruccion(&self, inicio: u64) -> String {
        let mia = self.identidad.public_key().to_bytes().to_vec();
        let suya = self
            .verificado
            .map(|v| v.curve())
            .or_else(|| self.tarjeta_otra.as_ref().map(tarjeta::Tarjeta::curve))
            .map(|k| k.to_bytes().to_vec())
            .unwrap_or_default();
        if mia <= suya {
            destruccion::sesion(&mia, &suya, inicio)
        } else {
            destruccion::sesion(&suya, &mia, inicio)
        }
    }

    /// The other side destroyed the conversation: if the notice is signed by the key verified
    /// for it, about this conversation and recent, do the same here; anything else is ignored
    /// (nobody on the road can cut a conversation by forging one).
    fn destruir_recibido(&mut self, v: &Value) {
        let Some(firmante) = self
            .verificado
            .map(|x| x.signing())
            .or_else(|| self.tarjeta_otra.as_ref().map(tarjeta::Tarjeta::signing))
        else {
            return;
        };
        let (Some(cuando), Some(inicio)) = (v["cuando"].as_u64(), v["inicio"].as_u64()) else {
            return;
        };
        let motivo = v["motivo"].as_str().unwrap_or_default();
        let firma = v["firma"].as_str().unwrap_or_default();
        if ahora_s().abs_diff(cuando) > 300 || self.inicio_destruccion().abs_diff(inicio) > 5 {
            return;
        }
        let sesion = self.sesion_destruccion(inicio);
        if !destruccion::comprueba(firmante, &sesion, motivo, cuando, firma) {
            return;
        }
        self.destruye(
            MotivoDestruccion::OtroLado(MotivoDestruccion::de_clave(motivo).to_owned()),
            false,
        );
    }

    /// The last destruction, for the notice (null when none).
    #[must_use]
    pub fn destruida(&self) -> Option<&Value> {
        self.destruida.as_ref()
    }

    /// The person closed the notice.
    pub fn olvida_destruida(&mut self) {
        if self.destruida.take().is_some() {
            self.cambio = true;
        }
    }

    /// Sign a chain with this computer's identity (the delivery room's ledger uses it too).
    #[must_use]
    pub fn firma_cadena(&self, cadena: Cadena) -> String {
        Prueba::firmar(&self.identidad, cadena, &self.version, ahora_s()).to_json()
    }

    /// Messages and files that disappear after `segundos` (0: off). Told to the other side, so
    /// both screens use the same timer, as in the big apps.
    pub fn pon_temporizador(&mut self, segundos: u64, decirlo: bool) {
        let s = segundos.min(TEMPORIZADOR_MAX);
        if s == self.temporizador {
            return;
        }
        self.temporizador = s;
        if decirlo && self.fase == Fase::Lista {
            self.manda_olm(json!({ "m": "temporizador", "s": s }));
        }
        self.cambio = true;
    }

    /// The timer in use.
    #[must_use]
    pub fn temporizador(&self) -> u64 {
        self.temporizador
    }

    /// Messages whose time came leave the screen, and files whose time came leave the disk.
    fn caducidades(&mut self) {
        let ahora = ahora_s();
        if crate::mensaje::purga(&mut self.mensajes, ahora) {
            self.cambio = true;
        }
        if self.caducan.is_empty() {
            return;
        }
        let idos: Vec<u64> = self
            .caducan
            .iter()
            .filter(|(_, t)| *t <= ahora)
            .map(|(id, _)| *id)
            .collect();
        self.caducan.retain(|(_, t)| *t > ahora);
        for id in idos {
            self.borra_recibido(id);
        }
    }

    /// What the «Escudo en directo» shows (18 Oct 2026, the owner: «somos como un escudo»): every
    /// line a check this program really made, with the second it was last true. Nothing is
    /// estimated here; a line without a measure says so.
    #[must_use]
    pub fn escudo_json(&self) -> Value {
        let verificado = self.verificado.is_some();
        let olm = self.conversacion.is_some();
        let movil_aqui = self
            .movil
            .as_ref()
            .is_some_and(|m| m.enganchado && m.aprobado);
        let mut dispositivos = Vec::new();
        if self.fase != Fase::Sola {
            dispositivos.push("este_equipo");
            if self.tarjeta_otra.is_some() {
                dispositivos.push(if self.otro_movil {
                    "movil_del_otro"
                } else {
                    "equipo_del_otro"
                });
            }
            if movil_aqui {
                dispositivos.push("mi_movil");
            }
        }
        let en_uso: Vec<Value> = self
            .vigia
            .en_uso
            .iter()
            .map(|(p, r)| json!({ "programa": p, "recurso": r }))
            .collect();
        json!({
            "identidad": {
                "ok": verificado,
                "desde": (self.verificado_en > 0).then_some(self.verificado_en),
                "contacto": self.nombre_contacto,
            },
            "cifrado": {
                "olm": olm,
                "cuantico": self.capa.is_some(),
                "renovado": (self.capa_desde > 0).then_some(self.capa_desde),
                "epocas": self.capa_epocas,
                "renueva_cada": cuantico::RENUEVA_CADA,
                "renueva_s": cuantico::RENUEVA_S,
            },
            "ruta": {
                "directa": self.monitor.transporte.directo,
                "rele": self.rele,
                "rele_nombre": self.rele_usado,
                "camino": self.camino().0,
                "ciudad": self.camino().1,
            },
            "vigia": {
                // Only Windows records it; elsewhere the line says it is not measured.
                "activa": self.vigia.activa,
                "mirado": (self.vigia_visto > 0).then_some(self.vigia_visto),
                "en_uso": en_uso,
                "pantalla_protegida": self.vigia.pantalla_protegida,
            },
            "integridad": {
                "mensajes_ok": self.abiertos_ok,
                "mensajes_mal": self.abiertos_mal,
                "trozos_ok": self.trozos_ok,
                "trozos_mal": self.trozos_mal,
            },
            "dispositivos": dispositivos,
            "auto_destruccion": self.auto_destruccion,
            "temporizador": self.temporizador,
        })
    }

    /// Drop the link and forget the handshake; the identity and the book stay.
    pub fn cerrar(&mut self, por: &str) {
        if self.enlace.is_some() && self.fase != Fase::Sola {
            // Deliberate: tell the other side, so it does not wait at the relay for us.
            self.manda(json!({ "t": "adios" }));
            std::thread::sleep(Duration::from_millis(60));
        }
        self.reconexion = None;
        self.rele_usado = None;
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
        self.espera_rele = None;
        self.tarjeta_otra = None;
        self.sas_inicio = None;
        self.sas_codigo = None;
        self.sas_yo_ok = false;
        self.sas_mac_otra = None;
        self.verificado = None;
        self.conversacion = None;
        // A file half received goes too: nothing of the conversation stays on the disk.
        if let Some(r) = self.entrada.take().and_then(|e| e.ruta) {
            let _ = std::fs::remove_file(r);
        }
        self.salida = None;
        self.cuantico_oferta = None;
        self.capa = None;
        self.cuantico_visto = false;
        self.capa_desde = 0;
        self.capa_epocas = 0;
        self.verificado_en = 0;
        self.abiertos_ok = 0;
        self.abiertos_mal = 0;
        self.trozos_ok = 0;
        self.trozos_mal = 0;
        self.temporizador = 0;
        self.caducan.clear();
        self.llamada = None;
        self.entrante = None;
        self.perdida = None;
        self.llamado_en_grupo = false;
        self.otro_late = false;
        self.otro_movil = false;
        self.movil_dicho = None;
        self.otro_voz_protegida = false;
        self.sala_rele = None;
        self.otro_rtc = false;
        self.turn = None;
        self.turn_rx = None;
        self.turn_pedido = 0;
        self.rtc_para_pagina.clear();
        self.puente_para_pagina.clear();
        self.calidad_pc = None;
        // A pause belongs to the conversation that had the incident: the next one starts
        // running (17 Oct 2026 review: it stayed paused until the program restarted, and the
        // next conversation hung at «abriendo»). A changed key is caught again by `hola`.
        self.estado = EstadoSesion::default();
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
        self.borra_rastro();
        self.cambio = true;
    }

    /// When a conversation ends, nothing of it stays (17 Oct 2026, the owner: «cuando una
    /// conversación termina, tiene que quedar todo borrado, absolutamente todo… en caso de robo
    /// o pérdida»): its messages leave the screen and the files it brought leave Descargas.
    /// The signed summary of the conversation is kept until the person closes it.
    fn borra_rastro(&mut self) {
        for r in self.recibidos.drain(..) {
            if !r.borrado {
                let _ = std::fs::remove_file(&r.ruta);
            }
        }
        self.mensajes.clear();
        self.por_ver.clear();
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
        self.relevo_tic();
        self.timbres();
        self.caducidades();
        // A quiet conversation renews its post-quantum epoch on the clock too (18 Oct 2026).
        if self.fase == Fase::Lista
            && self.estado.en_marcha()
            && self.cuantico_oferta.is_none()
            && self
                .capa
                .as_ref()
                .is_some_and(|c| cuantico::toca_renovar(c.sellados(), self.capa_desde, ahora_s()))
        {
            self.cuantico_empieza();
        }
        self.sigue_envio();
        self.di_dispositivo();
        self.reconexion_tic();
        self.turn_tic();
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
            // A silent link in an open conversation is a drop like any other: look for the
            // other side again at the relay (it does the same) before giving up.
            let reconecta = self.fase == Fase::Lista && self.empieza_reconexion();
            if !reconecta {
                self.aviso("sec_enlace_cerrado", "sin señal de la otra persona");
                self.cerrar("sin señal de la otra persona");
            }
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
                Entrante::AceptadoRele(s, rele, sala) => {
                    self.rele_usado = Some(rele);
                    self.sala_rele = Some(sala);
                    self.adopta(s, true);
                }
                Entrante::ReleFallo(e) => {
                    if let Some(r) = &mut self.reconexion {
                        r.buscando = false;
                        r.proximo = ahora_s() + 3;
                        self.rele_error = Some(e);
                        self.cambio = true;
                        continue;
                    }
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
                Entrante::Conectado => {
                    if self.reconexion.take().is_some() {
                        self.fase = Fase::Lista;
                        self.ultimo_recibido = std::time::Instant::now();
                        self.monitor.anota(
                            ahora_s(),
                            Nivel::Info,
                            "enlace recuperado; la conversación sigue",
                        );
                        self.manda(json!({ "t": "latido" }));
                        self.di_dispositivo_otra_vez();
                        self.digo_lo_que_tengo();
                        self.cuantico_empieza();
                        self.cambio = true;
                    } else {
                        self.saluda();
                    }
                }
                Entrante::Trama(v) => self.trama(v),
                Entrante::Cerrado(por) => {
                    if self.fase == Fase::Lista && self.empieza_reconexion() {
                        // Not over: both sides go back to the relay.
                    } else if self.fase != Fase::Sola && self.reconexion.is_none() {
                        self.aviso("sec_enlace_cerrado", por.clone());
                        self.cerrar(&por);
                    }
                }
            }
        }
    }

    /// The link dropped with the conversation open: keep the Olm session, drop the socket and
    /// the call, and go back to the relay to a room both sides derive from the two verified
    /// keys and the moment the conversation opened. `false` when there is nothing to pick up.
    fn empieza_reconexion(&mut self) -> bool {
        let Some(v) = self.verificado else {
            return false;
        };
        if self.conversacion.is_none() {
            return false;
        }
        let (mut a, mut b) = (self.identidad.public_key().to_bytes(), v.curve().to_bytes());
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        let h = sha2::Sha256::digest(
            [
                b"GUARDIANA SECURE reconexion".as_slice(),
                &a,
                &b,
                &self.inicio.to_be_bytes(),
            ]
            .concat(),
        );
        let sala: String = h[..16].iter().map(|x| format!("{x:02x}")).collect();
        self.envio_al_caer();
        self.enlace = None;
        self.rx = None;
        self.otro_late = false;
        if self.llamada.is_some() {
            self.fin_de_llamada();
            self.aviso("sec_llamada_cortada_red", "");
        }
        // The phone stays linked to this computer: only the other side's link is being found
        // again (17 Oct 2026 review: a blip ended the hand-off and the QR had to be read again).
        self.reconexion = Some(Reconexion {
            sala,
            desde: ahora_s(),
            proximo: 0,
            intentos: 0,
            buscando: false,
        });
        self.monitor.anota(
            ahora_s(),
            Nivel::Info,
            "enlace caído; buscando a la otra persona otra vez",
        );
        self.cambio = true;
        true
    }

    /// Try again when due; give up after [`RECONEXION_S`].
    fn reconexion_tic(&mut self) {
        let Some(r) = self.reconexion.clone() else {
            return;
        };
        let ahora = ahora_s();
        if ahora.saturating_sub(r.desde) > RECONEXION_S {
            self.reconexion = None;
            self.aviso("sec_enlace_cerrado", "sin señal de la otra persona");
            self.cerrar("no se recuperó el enlace");
            return;
        }
        if r.buscando || ahora < r.proximo {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.tx = Some(tx.clone());
        let sala = r.sala.clone();
        // Both sides derive the same relay from the room, so they meet.
        let indice = usize::from(sala.bytes().next().unwrap_or(0) % 16) % crate::rele::RELES.len();
        let rele = self.rele_usado.clone().unwrap_or_else(|| {
            std::env::var("GUARDIANA_RELE")
                .unwrap_or_else(|_| crate::rele::RELES[indice].direccion())
        });
        std::thread::spawn(move || {
            let res = crate::rele::conecta(&rele, &sala);
            let _ = tx.send(match res {
                Ok(s) => Entrante::AceptadoRele(s, rele, sala),
                Err(e) => Entrante::ReleFallo(e),
            });
        });
        if let Some(r) = &mut self.reconexion {
            r.buscando = true;
            r.intentos += 1;
        }
    }

    /// After a new link, the other side hears again where we talk from.
    fn di_dispositivo_otra_vez(&mut self) {
        self.movil_dicho = None;
        self.di_dispositivo();
    }

    /// Whether the conversation is waiting to meet the other side again.
    pub fn reconectando(&self) -> bool {
        self.reconexion.is_some()
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
        // Wait at the relay the code names (the nearest measured, see `rele::mide_cercania`).
        let rele = std::env::var("GUARDIANA_RELE").unwrap_or_else(|_| {
            crate::rele::RELES[crate::rele::rele_de_codigo(&codigo)].direccion()
        });
        std::thread::spawn(move || {
            let r = crate::rele::conecta(&rele, &sala);
            let _ = tx.send(match r {
                Ok(s) => Entrante::AceptadoRele(s, rele, sala),
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
            "adios" => {
                self.reconexion = None;
                if self.fase != Fase::Sola {
                    self.aviso("sec_enlace_cerrado", "la otra persona cerró");
                    self.cerrar("la otra persona cerró");
                }
            }
            "hola" => self.hola(&v),
            "sas" => self.sas(&v),
            "sas_mac" => self.sas_mac(&v),
            "otk" => self.otk(&v),
            "olm" => self.olm(&v),
            "bloque" => self.bloque(&v),
            "media" => self.media_entrante(&v),
            "destruir" => self.destruir_recibido(&v),
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
        if self.bloqueados.contains(&t.signing().to_base64()) {
            self.cerrar("identidad bloqueada");
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
                // Known and the same keys: still the numbers, every new conversation (17 Oct
                // 2026, the owner: «la identidad hay que hacerla cada vez que se conecta por
                // seguridad»). A link that drops and comes back inside the same conversation
                // does not greet again, so it does not ask again either.
                Ok(()) => {}
                Err(_) => {
                    let n = self.nombre_contacto.clone();
                    self.estado.parar(
                        guardiana_secure::pausa::Motivo::ClaveCambiada { nombre: n.clone() },
                        ahora_s(),
                        &mut self.monitor,
                        &mut self.cadena,
                    );
                    self.incidente(
                        "clave_cambiada",
                        format!("la clave de identidad de {n} no es la verificada"),
                        0,
                    );
                    // Another device under a known name: an unexpected device (18 Oct 2026).
                    if self.auto_destruccion {
                        self.destruir(MotivoDestruccion::DispositivoNuevo);
                        return;
                    }
                }
            }
        }
        // Every new conversation (known, new or changed keys): the short code.
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
            self.destruir(MotivoDestruccion::CodigoDistinto);
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
                self.verificado_en = ahora_s();
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
                self.incidente(
                    "sas_distinto",
                    "los números de verificación no cuadraron".to_owned(),
                    0,
                );
                self.destruir(MotivoDestruccion::CodigoDistinto);
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
                    // The first envelope opens the session on the other side, and carries the
                    // moment the conversation opened: both sides derive the room they meet in
                    // again after a drop from it, so it must be the same second on both.
                    self.manda_olm(json!({ "m": "hola", "inicio": self.inicio }));
                    self.monitor
                        .anota(ahora_s(), Nivel::Info, "conversación cifrada abierta");
                    self.manda_zona();
                    self.manda_olm(json!({ "m": "rtc_si" }));
                    self.cuantico_empieza();
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
        let mut bytes = serde_json::to_vec(&carga).unwrap_or_default();
        let renovar = self
            .capa
            .as_ref()
            .is_some_and(|c| cuantico::toca_renovar(c.sellados(), self.capa_desde, ahora_s()));
        if let Some(capa) = &mut self.capa {
            // The post-quantum lock first; Olm wraps the sealed bytes as any other payload.
            match capa.sella(&bytes) {
                Ok(sellado) => {
                    bytes = serde_json::to_vec(&json!({ "pq": B64.encode(sellado) }))
                        .unwrap_or_default();
                }
                Err(e) => {
                    self.aviso("sec_err_olm", e.to_string());
                    return;
                }
            }
        }
        let Some(conv) = &mut self.conversacion else {
            return;
        };
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
        if renovar {
            self.cuantico_empieza();
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
                    self.manda_olm(json!({ "m": "rtc_si" }));
                    self.cuantico_empieza();
                    Ok(plano)
                }
                Err(e) => Err(e),
            }
        };
        match abierto {
            Ok(plano) => {
                self.abiertos_ok += 1;
                self.monitor.recibido(ahora_s());
                if let Ok(carga) = serde_json::from_slice::<Value>(&plano) {
                    if let Some(carga) = self.cuantico_abre(carga) {
                        self.carga(&carga);
                    }
                }
            }
            Err(e) => {
                self.abiertos_mal += 1;
                self.aviso("sec_err_olm", e.to_string());
                self.fallo_abriendo(false);
            }
        }
        self.cambio = true;
    }

    /// Side 0 (by the order of the keys) offers the ML-KEM key once the conversation is open,
    /// and again after every link and every [`cuantico::RENUEVA_CADA`] sealed messages (a new
    /// epoch, so a secret that leaked opens only what it sealed).
    fn cuantico_empieza(&mut self) {
        if self.mi_numero() != 0 || self.cuantico_oferta.is_some() {
            return;
        }
        let (oferta, llave) = cuantico::Oferta::nueva();
        self.cuantico_oferta = Some((oferta, llave.clone()));
        self.manda_olm(json!({ "m": "pq_llave", "llave": B64.encode(llave) }));
    }

    /// The two verified identities in the order the layer binds them to (side 0 first).
    fn identidades_ordenadas(&self) -> (Vec<u8>, Vec<u8>) {
        let mia = self.identidad.public_key().to_bytes().to_vec();
        let suya = self
            .verificado
            .map(|v| v.curve().to_bytes().to_vec())
            .unwrap_or_default();
        if self.mi_numero() == 0 {
            (mia, suya)
        } else {
            (suya, mia)
        }
    }

    /// What came inside Olm: sealed by the post-quantum layer, or plain. Plain is fine until
    /// the layer is agreed and the other side has used it; after that it is a downgrade and
    /// is dropped as an attack signal.
    fn cuantico_abre(&mut self, carga: Value) -> Option<Value> {
        let Some(sellado) = carga["pq"].as_str().and_then(|s| B64.decode(s).ok()) else {
            if self.capa.is_some() && self.cuantico_visto {
                self.incidente(
                    "cuantico_degradado",
                    "llegó un mensaje sin la capa poscuántica ya acordada".to_owned(),
                    1,
                );
                self.destruir(MotivoDestruccion::Manipulada);
                return None;
            }
            return Some(carga);
        };
        let Some(capa) = &mut self.capa else {
            return None;
        };
        match capa.abre(&sellado) {
            Ok(plano) => {
                self.cuantico_visto = true;
                serde_json::from_slice::<Value>(&plano).ok()
            }
            Err(_) => {
                self.abiertos_mal += 1;
                self.incidente(
                    "cuantico_mal",
                    "un mensaje no pasó la capa poscuántica (repetido o alterado)".to_owned(),
                    1,
                );
                self.destruir(MotivoDestruccion::Repeticion);
                None
            }
        }
    }

    /// The layer is on: shown on screen as «resistente a ordenadores cuánticos».
    #[must_use]
    pub fn cuantico(&self) -> bool {
        self.capa.is_some()
    }

    fn carga(&mut self, c: &Value) {
        match c["m"].as_str().unwrap_or_default() {
            "hola" => {
                // The opening second of the side that started (an older version sends none).
                if let Some(t) = c["inicio"].as_u64() {
                    self.inicio = t;
                }
            }
            "pq_llave" => {
                // Side 1: a secret for that key, its ciphertext back, and the layer from here
                // on (the answer itself goes plain: the other side has no layer yet).
                let Some(llave) = c["llave"].as_str().and_then(|s| B64.decode(s).ok()) else {
                    return;
                };
                if self.mi_numero() != 1 {
                    return;
                }
                match cuantico::encapsula(&llave) {
                    Ok((cifrado, secreto)) => {
                        let (id0, id1) = self.identidades_ordenadas();
                        let t = cuantico::transcript(&id0, &id1, &llave, &cifrado);
                        // The answer goes out under the epoch the other side still has.
                        self.manda_olm(
                            json!({ "m": "pq_cifrado", "cifrado": B64.encode(cifrado) }),
                        );
                        match &mut self.capa {
                            Some(capa) => capa.renueva(&secreto, &t),
                            None => self.capa = Some(cuantico::Capa::nueva(&secreto, &t, 1)),
                        }
                        self.capa_desde = ahora_s();
                        self.capa_epocas += 1;
                        self.monitor
                            .anota(ahora_s(), Nivel::Info, "capa poscuántica activa");
                        self.cambio = true;
                    }
                    Err(e) => self.aviso("sec_err_olm", e.to_string()),
                }
            }
            "pq_cifrado" => {
                let Some(cifrado) = c["cifrado"].as_str().and_then(|s| B64.decode(s).ok()) else {
                    return;
                };
                let Some((oferta, llave)) = self.cuantico_oferta.take() else {
                    return;
                };
                match oferta.abre(&cifrado) {
                    Ok(secreto) => {
                        let (id0, id1) = self.identidades_ordenadas();
                        let t = cuantico::transcript(&id0, &id1, &llave, &cifrado);
                        match &mut self.capa {
                            Some(capa) => capa.renueva(&secreto, &t),
                            None => self.capa = Some(cuantico::Capa::nueva(&secreto, &t, 0)),
                        }
                        self.capa_desde = ahora_s();
                        self.capa_epocas += 1;
                        self.monitor
                            .anota(ahora_s(), Nivel::Info, "capa poscuántica activa");
                        self.cambio = true;
                    }
                    Err(e) => self.aviso("sec_err_olm", e.to_string()),
                }
            }
            "temporizador" => {
                self.pon_temporizador(c["s"].as_u64().unwrap_or(0), false);
            }
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
                let id = mensaje::id_de(&c["id"]).unwrap_or_else(Mensaje::nuevo_id);
                if self.mensajes.iter().any(|m| m.id == id) {
                    // The same text twice (a resend after a cut): it is here, say so again.
                    self.manda_olm(json!({ "m": "entregado", "id": id.to_string() }));
                    return;
                }
                let re = mensaje::id_de(&c["re"]);
                let mut m = Mensaje::del_otro(id, texto, de, re);
                // A message that disappears: from when it arrived here, at most a week.
                if let Some(d) = c["dura"].as_u64().filter(|&d| d > 0) {
                    m.caduca = Some(m.t + d.min(TEMPORIZADOR_MAX));
                }
                self.mensajes.push(m);
                self.por_ver.push(id);
                self.otro_escribe = 0;
                // Tell the sender it is here (its ✓✓): the relay never says that.
                self.manda_olm(json!({ "m": "entregado", "id": id.to_string() }));
                // «Seen» only when someone looks: the page (or the phone) says so when the
                // message is on a screen in front (17 Oct 2026 review: green ticks went out
                // with the window minimised).
                self.novedad = true;
            }
            "entregado" => {
                if let Some(id) = mensaje::id_de(&c["id"]) {
                    if let Some(m) = self.mensajes.iter_mut().find(|m| m.id == id && m.mio) {
                        m.entregado();
                    }
                }
            }
            "visto" => {
                let ids: Vec<u64> = c["ids"]
                    .as_array()
                    .map(|a| a.iter().filter_map(mensaje::id_de).collect())
                    .unwrap_or_default();
                for m in self
                    .mensajes
                    .iter_mut()
                    .filter(|m| m.mio && ids.contains(&m.id))
                {
                    m.visto();
                }
            }
            "escribiendo" => {
                self.otro_escribe = ahora_s();
            }
            "editar" => {
                let texto: String = c["texto"]
                    .as_str()
                    .unwrap_or_default()
                    .chars()
                    .take(8000)
                    .collect();
                if let Some(id) = mensaje::id_de(&c["id"]) {
                    // Only the other side's own texts: ours are ours.
                    if let Some(m) = self.mensajes.iter_mut().find(|m| m.id == id && !m.mio) {
                        m.edita(texto);
                    }
                }
            }
            "borrar" => {
                if let Some(id) = mensaje::id_de(&c["id"]) {
                    if let Some(m) = self.mensajes.iter_mut().find(|m| m.id == id && !m.mio) {
                        m.borra();
                    }
                }
            }
            "reaccion" => {
                let emoji = c["emoji"].as_str().unwrap_or_default();
                let quien = self.nombre_contacto.clone();
                if let Some(id) = mensaje::id_de(&c["id"]) {
                    if let Some(m) = self.mensajes.iter_mut().find(|m| m.id == id) {
                        m.reacciona(&quien, emoji);
                    }
                }
            }
            "llamada" | "llamada_ok" => {
                let Some(clave) = c["clave"].as_str().and_then(|x| B64.decode(x).ok()) else {
                    return;
                };
                let Ok(clave) = <[u8; 32]>::try_from(clave.as_slice()) else {
                    return;
                };
                let video = c["video"].as_bool().unwrap_or(false) && VIDEO;
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
            "reinicia_media" => {
                self.reinicios += 1;
                self.cambio = true;
            }
            "dispositivo" => {
                self.otro_movil = c["movil"].as_bool().unwrap_or(false);
                self.cambio = true;
            }
            "voz" => {
                self.otro_voz_protegida = c["protegida"].as_bool().unwrap_or(false);
                self.cambio = true;
            }
            "rtc_si" => {
                self.otro_rtc = true;
                self.cambio = true;
            }
            "vigia" => {
                self.vigia_otro = Vigilancia::de_json(&c["v"]);
                self.cambio = true;
            }
            "rtc" => {
                // For whoever talks here: the phone when it took the call, else the page.
                let d = c["d"].clone();
                // With the bridge (18 Oct 2026) the page always holds the leg to the other side,
                // also while the phone talks through it.
                if self.movil_manda() && !self.puente_modo() {
                    if let Some(m) = &self.movil {
                        m.manda(crate::movil::Hacia::Estado(
                            json!({ "tipo": "secure_rtc", "d": d }).to_string(),
                        ));
                    }
                } else if self.rtc_para_pagina.len() < 256 {
                    self.rtc_para_pagina.push_back(d);
                }
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
                    self.anota_llamada(tm.video, false, None, "perdida");
                    self.monitor
                        .anota(ahora_s(), Nivel::Info, "llamada perdida");
                    self.aviso("sec_llamada_perdida", self.nombre_contacto.clone());
                    self.cambio = true;
                } else if let Some(l) = self.llamada.take() {
                    if l.conectada {
                        self.anota_llamada(
                            l.video,
                            l.saliente,
                            Some(ahora_s().saturating_sub(l.desde)),
                            "fin",
                        );
                        self.cierra_informe(l.video, l.desde);
                    } else {
                        self.anota_llamada(l.video, true, None, "rechazada");
                    }
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
            "archivo_tengo" => {
                let bloques = c["bloques"].as_u64().unwrap_or(0);
                let ultimo = c["ultimo"].as_str().unwrap_or_default();
                let ultimo = if ultimo.is_empty() {
                    String::new()
                } else {
                    limpia_nombre(ultimo)
                };
                let nombre = c["nombre"].as_str().unwrap_or_default();
                let nombre = if nombre.is_empty() {
                    String::new()
                } else {
                    limpia_nombre(nombre)
                };
                self.archivo_tengo(&nombre, bloques, &ultimo);
            }
            "archivo_ok" => {
                let nombre = limpia_nombre(c["nombre"].as_str().unwrap_or_default());
                if self
                    .salida
                    .as_ref()
                    .is_some_and(|s| s.nombre == nombre && s.llave.is_some())
                {
                    // Whole on the other disk: nothing left to resend.
                    self.salida = None;
                }
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
        // Only inside an open conversation (before it, anyone who reached the port could fill
        // memory with blocks), and never more than 256 waiting.
        if self.fase != Fase::Lista
            || self
                .entrada
                .as_ref()
                .is_some_and(|e| e.bloques_pendientes.len() >= 256)
        {
            return;
        }
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
        let mut ok_trozos = 0u64;
        let mut fallo: Option<&'static str> = None;
        let mut completo = false;
        let mut confirma: Option<(String, u64)> = None;
        if let Some(e) = &mut self.entrada {
            if let (Some(r), Some(f)) = (&mut e.recepcion, &mut e.archivo) {
                while let Some((n, b)) = e.bloques_pendientes.pop_front() {
                    if n < r.bloques() {
                        // Already here (sent again after a drop): let it pass.
                        continue;
                    }
                    match r.mete(n, &b) {
                        Ok(plano) => {
                            ok_trozos += 1;
                            e.recibido += plano.len() as u64;
                            if f.write_all(&plano).is_err() {
                                fallo = Some("sec_err_archivo");
                                break;
                            }
                            if r.bloques() % CONFIRMA_CADA == 0 {
                                confirma = Some((e.nombre.clone(), r.bloques()));
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
        self.trozos_ok += ok_trozos;
        if let Some(clave) = fallo {
            if let Some(e) = self.entrada.take() {
                if let Some(r) = &e.ruta {
                    let _ = std::fs::remove_file(r);
                }
            }
            self.aviso(clave, "");
            if clave == "sec_archivo_mal" {
                self.trozos_mal += 1;
                self.incidente(
                    "archivo_mal",
                    "un bloque de archivo llegó alterado".to_owned(),
                    1,
                );
                self.destruir(MotivoDestruccion::Manipulada);
            }
            return;
        }
        if let Some((nombre, bloques)) = confirma.filter(|_| !completo) {
            self.manda_olm(json!({ "m": "archivo_tengo", "nombre": nombre, "bloques": bloques }));
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
                    let mut linea = Mensaje::archivo(n.clone(), false);
                    if self.temporizador > 0 {
                        linea.caduca = Some(linea.t + self.temporizador);
                        if let Some(r) = self.recibidos.last() {
                            self.caducan.push((r.id, linea.t + self.temporizador));
                        }
                    }
                    // A picture goes on to the linked phone so it shows in its chat.
                    if let Some(r) = self
                        .recibidos
                        .iter()
                        .rev()
                        .find(|r| r.nombre == n && (r.imagen() || r.audio()))
                    {
                        linea.archivo_id = (r.id & 0xffff_ffff) as u32;
                        if let Some(m) = &self.movil {
                            if m.enganchado && m.aprobado && r.bytes <= 16 * 1024 * 1024 {
                                if let Ok(datos) = std::fs::read(&r.ruta) {
                                    m.manda(crate::movil::Hacia::Estado(json!({ "tipo": "archivo_movil", "id": linea.archivo_id, "nombre": n, "mime": if r.audio() { mensaje::mime_de(&n) } else { mime_imagen(&n) }, "bytes": datos.len() }).to_string()));
                                    for trozo in datos.chunks(65536) {
                                        m.manda(crate::movil::Hacia::Archivo(
                                            linea.archivo_id,
                                            trozo.to_vec(),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    self.mensajes.push(linea);
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
        self.medidas = None;
        self.llamada = Some(Llamada {
            video,
            mime_local: mime.chars().take(80).collect(),
            mime_remoto: None,
            emisor: sframe::Emisor::new(&clave, yo),
            receptor: None,
            conectada: !inicia,
            saliente: inicia,
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
        self.empieza_llamada(video && VIDEO, true, mime);
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
        if let Some(l) = self.llamada.take() {
            if l.conectada {
                self.anota_llamada(
                    l.video,
                    l.saliente,
                    Some(ahora_s().saturating_sub(l.desde)),
                    "fin",
                );
                self.cierra_informe(l.video, l.desde);
            } else {
                self.anota_llamada(l.video, true, None, "cancelada");
            }
            self.manda_olm(json!({ "m": "cuelga" }));
            self.monitor
                .anota(ahora_s(), Nivel::Info, "llamada terminada");
            self.fin_de_llamada();
            self.cambio = true;
        }
    }

    /// A line in the chat for the call that just ended (16 Oct 2026: «historial de llamadas»).
    fn anota_llamada(
        &mut self,
        video: bool,
        saliente: bool,
        segundos: Option<u64>,
        fin: &'static str,
    ) {
        self.mensajes.push(Mensaje::llamada(mensaje::Llamada {
            video,
            saliente,
            segundos,
            fin,
        }));
        self.cambio = true;
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

    /// Our player starts fresh (the phone took the call, or gave it back): the other side's
    /// recorder starts again so the first piece carries the stream's header.
    pub fn pide_inicio(&mut self) {
        if self.llamada.is_some() {
            self.manda_olm(json!({ "m": "reinicia_media" }));
        }
    }

    /// The person closed the «missed call» note.
    pub fn olvida_perdida(&mut self) {
        if self.perdida.take().is_some() {
            self.cambio = true;
        }
    }

    /// The person rejects the call ringing here.
    pub fn rechazar(&mut self) {
        if let Some(t) = self.entrante.take() {
            self.anota_llamada(t.video, false, None, "rechazada_aqui");
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
            if let Some(l) = self.llamada.take() {
                self.anota_llamada(l.video, true, None, "sin_respuesta");
            }
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
            if let Some((_, video)) = self.perdida {
                self.anota_llamada(video, false, None, "perdida");
            }
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
        // The phone carries the voice: what the page still records stays here.
        if !self.relevo.pagina_envia(self.movil_listo()) {
            return Ok(());
        }
        let bytes = B64.decode(b64).map_err(|e| e.to_string())?;
        self.media_bytes(bytes)
    }

    /// More than this of our call waiting in the link: the pieces are dropped (a call that
    /// waits is worse than one that skips a moment), and once it drains the recorder starts
    /// again so the other player gets a fresh header (17 Oct 2026 review: a slow uplink made
    /// the delay grow without end).
    fn media_atascada(&mut self) -> bool {
        const MEDIA_COLA_MAX: u64 = 512 * 1024;
        let cola = self.enlace.as_ref().map_or(0, |e| {
            let (a, b) = e.cuenta();
            a.saturating_sub(b)
        });
        if cola > MEDIA_COLA_MAX {
            self.media_saltada = true;
            return true;
        }
        if self.media_saltada {
            self.media_saltada = false;
            self.reinicios += 1;
            self.cambio = true;
            return true;
        }
        false
    }

    fn media_bytes(&mut self, bytes: Vec<u8>) -> Result<(), String> {
        if !self.estado.en_marcha() || self.media_atascada() {
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
        if !self.estado.en_marcha() || self.media_atascada() {
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
                // On the phone only a two-person call: a group's pieces come from several
                // people. While the voice moves there, both players get them (make before break).
                let listo = self
                    .movil
                    .as_ref()
                    .is_some_and(|m| m.enganchado && m.aprobado);
                let al_movil = de.is_empty() && self.relevo.piezas_al_movil(listo);
                if al_movil {
                    if let Some(m) = &self.movil {
                        m.manda(crate::movil::Hacia::Media(pieza.to_vec()));
                    }
                }
                if !al_movil || self.relevo.piezas_a_pagina(listo) {
                    self.media_para_pagina
                        .push_back((de.to_owned(), B64.encode(pieza)));
                    if self.media_para_pagina.len() > 400 {
                        self.media_para_pagina.pop_front();
                    }
                }
            }
            Err(_) => {
                self.monitor.anota(
                    ahora_s(),
                    Nivel::Inusual,
                    "un trozo de la llamada no se pudo abrir; descartado",
                );
                self.fallo_abriendo(true);
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
        self.texto_re(texto, None);
    }

    /// The person wrote something, maybe answering another message.
    pub fn texto_re(&mut self, texto: &str, re: Option<u64>) {
        if self.fase != Fase::Lista {
            self.aviso("sec_no_lista", "");
            return;
        }
        let t: String = texto.chars().take(8000).collect();
        let mut m = Mensaje::mio(t.clone(), re);
        let mut carga = json!({ "m": "texto", "texto": t, "id": m.id.to_string() });
        if let Some(re) = re {
            carga["re"] = json!(re.to_string());
        }
        if self.temporizador > 0 {
            m.caduca = Some(m.t + self.temporizador);
            carga["dura"] = json!(self.temporizador);
        }
        self.mensajes.push(m);
        self.manda_olm(carga);
        self.cambio = true;
    }

    /// The person changed a text of theirs.
    pub fn editar(&mut self, id: u64, texto: &str) {
        let t: String = texto.chars().take(8000).collect();
        if let Some(m) = self.mensajes.iter_mut().find(|m| m.id == id && m.mio) {
            m.edita(t.clone());
            self.manda_olm(json!({ "m": "editar", "id": id.to_string(), "texto": t }));
            self.cambio = true;
        }
    }

    /// The person deletes a message: for everyone (theirs only), or just here.
    pub fn borrar(&mut self, id: u64, para_todos: bool) {
        let Some(i) = self.mensajes.iter().position(|m| m.id == id) else {
            return;
        };
        if para_todos && self.mensajes[i].mio {
            self.mensajes[i].borra();
            self.manda_olm(json!({ "m": "borrar", "id": id.to_string() }));
        } else {
            self.mensajes.remove(i);
        }
        self.cambio = true;
    }

    /// The whole conversation goes from this screen (the other side keeps its copy).
    pub fn borrar_todo(&mut self) {
        self.mensajes.clear();
        self.por_ver.clear();
        self.cambio = true;
    }

    /// The person reacts to a message with an emoji (the same one again takes it away).
    pub fn reaccionar(&mut self, id: u64, emoji: &str) {
        let yo = self.mi_nombre();
        if let Some(m) = self.mensajes.iter_mut().find(|m| m.id == id) {
            m.reacciona(&yo, emoji);
            self.manda_olm(json!({ "m": "reaccion", "id": id.to_string(), "emoji": emoji }));
            self.cambio = true;
        }
    }

    /// The person is typing: the other side shows «escribiendo…» for a few seconds. Sent at
    /// most every three seconds.
    pub fn escribiendo(&mut self) {
        let ahora = ahora_s();
        if self.fase == Fase::Lista && ahora.saturating_sub(self.escribi_en) >= 3 {
            self.escribi_en = ahora;
            self.manda_olm(json!({ "m": "escribiendo" }));
        }
    }

    /// The other side is typing right now.
    pub fn otro_escribiendo(&self) -> bool {
        self.otro_escribe > 0 && ahora_s().saturating_sub(self.otro_escribe) < 6
    }

    /// The conversation is on screen here: whatever came since is reported as seen.
    pub fn visto(&mut self) {
        self.novedad = false;
        self.manda_vistos();
    }

    fn manda_vistos(&mut self) {
        if self.sin_visto || self.por_ver.is_empty() || self.fase != Fase::Lista {
            return;
        }
        let ids: Vec<String> = self.por_ver.drain(..).map(|i| i.to_string()).collect();
        self.manda_olm(json!({ "m": "visto", "ids": ids }));
    }

    /// Our own reactions are filed under the empty name: the screen says «tú».
    fn mi_nombre(&self) -> String {
        String::new()
    }

    /// A file starts: the page will hand the pieces over.
    pub fn archivo_empieza(&mut self, nombre: &str, bytes: u64) -> Result<(), String> {
        if self.fase != Fase::Lista {
            return Err("sec_no_lista".to_owned());
        }
        let nombre = limpia_nombre(nombre);
        let trozos = Trozos::new().map_err(|e| e.to_string())?;
        let inicial = trozos.llave_inicial();
        // The key first: the other side writes each block to disk as it comes.
        self.manda_olm(
            json!({ "m": "archivo_inicio", "nombre": nombre, "llave": inicial, "bytes": bytes }),
        );
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
            antes: 0,
            marcas: VecDeque::new(),
            aviso: std::time::Instant::now(),
        });
        self.salida = Some(Salida {
            nombre,
            trozos,
            bytes: 0,
            guardados: VecDeque::new(),
            guardados_bytes: 0,
            pausada: self.enlace.is_none(),
            llave: None,
            inicial,
            anunciado: bytes,
        });
        self.cambio = true;
        Ok(())
    }

    /// The other side says how many blocks of the file it has (every few MB, and right after
    /// a link comes back, when it also names the last file it got whole): the blocks before
    /// that are let go, and if the sending was paused by a drop, it goes on from there. The
    /// keys go again if they were lost in the drop.
    fn archivo_tengo(&mut self, nombre: &str, bloques: u64, ultimo: &str) {
        let Some(s) = &mut self.salida else {
            return;
        };
        let mut bloques = bloques;
        let mut desde_el_principio = false;
        if s.nombre != nombre {
            if !s.pausada {
                return;
            }
            if s.llave.is_some() && ultimo == s.nombre {
                // It got there whole; only the «it is here» was lost.
                let llego = self.salida.take().map(|s| s.nombre);
                if let Some(x) = self
                    .enviados
                    .iter_mut()
                    .find(|x| Some(&x.nombre) == llego.as_ref() && x.llego_en.is_none())
                {
                    x.llego_en = Some(x.empezo.elapsed().as_secs_f64());
                }
                self.envio = None;
                self.cambio = true;
                return;
            }
            // The start never got there: from the first block, key first.
            bloques = 0;
            desde_el_principio = true;
        }
        while s.guardados.front().is_some_and(|(n, _)| *n < bloques) {
            if let Some((_, c)) = s.guardados.pop_front() {
                s.guardados_bytes = s.guardados_bytes.saturating_sub(c.len() as u64);
            }
        }
        if !s.pausada {
            return;
        }
        // Everything from `bloques` on must still be here; otherwise the file is lost.
        let enviados = s.llave.as_ref().map_or(s.trozos.bloques(), |l| l.bloques);
        let primero = s.guardados.front().map_or(enviados, |(n, _)| *n);
        if primero != bloques {
            self.salida = None;
            self.envio = None;
            self.aviso("sec_archivo_cortado", "");
            self.cambio = true;
            return;
        }
        s.pausada = false;
        let otra_vez: Vec<Value> = s
            .guardados
            .iter()
            .map(|(n, c)| json!({ "t": "bloque", "n": n, "b": B64.encode(c) }))
            .collect();
        let llave = s.llave.clone();
        let nombre = s.nombre.clone();
        let inicio = desde_el_principio.then(|| (s.inicial.clone(), s.anunciado));
        if let Some((inicial, anunciado)) = inicio {
            self.manda_olm(json!({ "m": "archivo_inicio", "nombre": nombre, "llave": inicial, "bytes": anunciado }));
        }
        for t in otra_vez {
            self.manda(t);
        }
        if let Some(l) = llave {
            self.manda_olm(json!({ "m": "archivo", "nombre": nombre, "llave": l }));
            let hasta = self.enlace.as_ref().map_or(0, |e| e.cuenta().0);
            if let Some(v) = &mut self.envio {
                v.fin = Some(hasta);
            }
        }
        self.monitor.anota(
            ahora_s(),
            Nivel::Info,
            format!("archivo reanudado: {nombre}"),
        );
        self.cambio = true;
    }

    /// The link dropped mid-file: keep what is on its way, pause, and count what was written.
    fn envio_al_caer(&mut self) {
        let escritos = self.enlace.as_ref().map(|e| e.cuenta().1);
        if let Some(v) = &mut self.envio {
            if let Some(w) = escritos {
                v.antes += w.saturating_sub(v.inicio);
            }
            v.inicio = 0;
            v.fin = None;
            v.marcas.clear();
        }
        if let Some(s) = &mut self.salida {
            s.pausada = true;
        }
    }

    /// After the link came back: tell the sender where we are with the file coming in (or
    /// that none is, and which was the last one that got here whole).
    fn digo_lo_que_tengo(&mut self) {
        let (nombre, bloques) = self
            .entrada
            .as_ref()
            .and_then(|e| {
                e.recepcion
                    .as_ref()
                    .map(|r| (e.nombre.clone(), r.bloques()))
            })
            .unwrap_or_default();
        let ultimo = self
            .recibidos
            .last()
            .map(|r| r.nombre.clone())
            .unwrap_or_default();
        self.manda_olm(
            json!({ "m": "archivo_tengo", "nombre": nombre, "bloques": bloques, "ultimo": ultimo }),
        );
    }

    /// The file going out: how far, and the seconds left at the speed of the last seconds.
    fn sigue_envio(&mut self) {
        let Some((encolados, escritos)) = self.enlace.as_ref().map(Enlace::cuenta) else {
            // No link: the file waits for the reconnection (it is dropped with the
            // conversation in `cerrar`).
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
        let Some(v) = &self.envio else {
            return Value::Null;
        };
        let (encolados, escritos) = self.enlace.as_ref().map_or((0, 0), Enlace::cuenta);
        // In base64 inside JSON a file takes about 4/3 of its size, plus a little per block.
        let total = v
            .fin
            .map_or(v.bytes.saturating_mul(4) / 3 + v.bytes / 400 + 1, |f| {
                v.antes + f.saturating_sub(v.inicio)
            })
            .max(1);
        let hecho = (v.antes + escritos.saturating_sub(v.inicio)).min(total);
        let pausado = self.enlace.is_none()
            || self
                .salida
                .as_ref()
                .is_some_and(|s| s.pausada || s.guardados_bytes > GUARDADOS_MAX);
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
            "pausado": pausado,
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
        let trama = json!({ "t": "bloque", "n": n, "b": B64.encode(&c) });
        s.guardados_bytes += c.len() as u64;
        s.guardados.push_back((n, c));
        let pausada = s.pausada;
        if !pausada {
            self.manda(trama);
        }
        if ultimo {
            // The file stays in `salida` (blocks kept, key ready) until the other side says
            // it is whole, or the next file starts: a drop in between can still be mended.
            let Some(s) = &mut self.salida else {
                return Ok(());
            };
            let nombre = s.nombre.clone();
            let bytes = s.bytes;
            let trozos =
                std::mem::replace(&mut s.trozos, Trozos::new().map_err(|e| e.to_string())?);
            let llave = trozos.termina().map_err(|e| e.to_string())?;
            s.llave = Some(llave.clone());
            if !pausada {
                self.manda_olm(json!({ "m": "archivo", "nombre": nombre, "llave": llave }));
                let hasta = self.enlace.as_ref().map_or(0, |e| e.cuenta().0);
                if let Some(v) = &mut self.envio {
                    v.fin = Some(hasta);
                }
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
            self.archivos.bytes += bytes;
            self.monitor
                .anota(ahora_s(), Nivel::Info, format!("archivo enviado: {nombre}"));
            let mut linea = Mensaje::archivo(nombre.clone(), true);
            if self.temporizador > 0 {
                linea.caduca = Some(linea.t + self.temporizador);
            }
            self.mensajes.push(linea);
            self.cambio = true;
        }
        Ok(())
    }

    /// The path right now, as the card says it: `misma_red`, `directo`, `rele` or `ninguno`,
    /// with the relay's city when through it. Never an address.
    fn camino(&self) -> (&'static str, Option<&'static str>) {
        match self.monitor.transporte.directo {
            Some(true) => {
                let misma = self.enlace.as_ref().and_then(Enlace::misma_red);
                (
                    if misma == Some(true) {
                        "misma_red"
                    } else {
                        "directo"
                    },
                    None,
                )
            }
            Some(false) => ("rele", self.rele_usado.as_deref().and_then(ciudad_de_rele)),
            None => ("ninguno", None),
        }
    }

    /// Stop everything and keep the card: sends frozen, the call hung up, the monitor and the
    /// chain told. Only the first incident of a stop is kept.
    fn incidente(&mut self, senal: &'static str, detalle: String, fallos: u32) {
        if self.incidente.is_some() {
            return;
        }
        let (transporte, ciudad_rele) = self.camino();
        self.incidente = Some(Incidente {
            cuando: ahora_s(),
            senal,
            detalle: detalle.clone(),
            transporte,
            ciudad_rele,
            fallos,
            contacto: self.nombre_contacto.clone(),
            zona_otra: self.zona_otra.clone(),
        });
        if let Some(l) = self.llamada.take() {
            self.manda_olm(json!({ "m": "cuelga" }));
            self.fin_de_llamada();
            if l.conectada {
                self.anota_llamada(
                    l.video,
                    l.saliente,
                    Some(ahora_s().saturating_sub(l.desde)),
                    "fin",
                );
                self.cierra_informe(l.video, l.desde);
            }
        }
        if self.estado.en_marcha() {
            self.estado.parar(
                guardiana_secure::pausa::Motivo::Inusual {
                    texto: format!("posible interferencia: {detalle}"),
                },
                ahora_s(),
                &mut self.monitor,
                &mut self.cadena,
            );
        }
        self.cambio = true;
    }

    /// A frame that failed to open: one is a bad network; a burst (3 in 10 s or 5 in 60 s for
    /// messages, 8 in 30 s for the call) is treated as an interference.
    fn fallo_abriendo(&mut self, media: bool) {
        let ahora = ahora_s();
        let (cola, limite) = if media {
            (&mut self.fallos_media, (8u32, 30u64))
        } else {
            (&mut self.fallos_olm, (5u32, 60u64))
        };
        cola.push_back(ahora);
        while cola
            .front()
            .is_some_and(|&t| ahora.saturating_sub(t) > limite.1)
        {
            cola.pop_front();
        }
        let n = cola.len() as u32;
        let corto = !media
            && cola
                .iter()
                .filter(|&&t| ahora.saturating_sub(t) <= 10)
                .count()
                >= 3;
        if n >= limite.0 || corto {
            let (senal, detalle) = if media {
                (
                    "media_fallos",
                    format!(
                        "{n} trozos de la llamada no se pudieron abrir en {} s",
                        limite.1
                    ),
                )
            } else {
                (
                    "olm_fallos",
                    format!("{n} mensajes no se pudieron autenticar"),
                )
            };
            self.incidente(senal, detalle, n);
            self.destruir(MotivoDestruccion::Manipulada);
        }
    }

    /// The incident, for the screen (null when none).
    pub fn estado_incidente(&self) -> Value {
        self.incidente.as_ref().map_or(Value::Null, |i| {
            serde_json::to_value(i).unwrap_or(Value::Null)
        })
    }

    /// The incident report the person saves: the card, this computer's version, and the signed
    /// chain of everything seen, so the sheet proves who wrote it and when (not who attacked).
    pub fn informe_incidente(&self) -> Option<String> {
        let i = self.incidente.as_ref()?;
        let prueba = Prueba::firmar(
            &self.identidad,
            self.cadena.clone(),
            &self.version,
            ahora_s(),
        );
        Some(
            json!({
                "tipo": "guardiana-secure-incidente",
                "version": 1,
                "programa": self.version,
                "incidente": i,
                "prueba": serde_json::from_str::<Value>(&prueba.to_json()).unwrap_or(Value::Null),
            })
            .to_string(),
        )
    }

    /// The person closed the card (after saving it, verifying again or ending).
    pub fn olvida_incidente(&mut self) {
        if self.incidente.take().is_some() {
            self.cambio = true;
        }
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

    /// The wait at the relay this session left in its room when it met its member directly
    /// (see `espera_rele`), for the group's next listener.
    pub(crate) fn toma_espera_rele(&mut self) -> Option<mpsc::Receiver<Entrante>> {
        self.espera_rele.take()
    }

    /// Listen through a wait already in the relay room as well (another session's, which met
    /// its member directly): whoever the relay joins there comes to this session. For as long
    /// as the relay keeps a wait, then nothing.
    pub(crate) fn hereda_espera_rele(&mut self, espera: mpsc::Receiver<Entrante>, codigo: String) {
        let Some(tx) = self.tx.clone() else {
            return;
        };
        std::thread::spawn(move || {
            let hasta = std::time::Instant::now() + Duration::from_secs(12 * 60);
            while let Some(queda) = hasta.checked_duration_since(std::time::Instant::now()) {
                match espera.recv_timeout(queda) {
                    Ok(e @ (Entrante::AceptadoRele(..) | Entrante::ReleFallo(_))) => {
                        let _ = tx.send(e);
                        return;
                    }
                    Ok(_) => {}
                    Err(_) => return,
                }
            }
        });
        self.rele_codigo = Some(codigo);
        self.cambio = true;
    }

    // ---------- The phone (15 Oct 2026) ----------

    /// Hand the conversation to the phone: start the listener, ask the router for the UDP door
    /// and put the QR code on screen. `idioma` goes into the QR so the page opens in the
    /// person's language.
    pub fn movil_empieza(&mut self, idioma: &str) -> Result<(), String> {
        // Any moment, since 16 Oct 2026: the phone can be in from before the other side
        // arrives (it then shows the code to share, the wait, the numbers to compare).
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

    /// WebRTC (17 Oct 2026): as soon as the conversation is open and both programs can carry
    /// calls this way, ask the relay it met at for its call service (valid for hours), so it is
    /// ready before anyone calls; at most once a minute while the relay says no.
    fn turn_tic(&mut self) {
        if let Some(rx) = &self.turn_rx {
            if let Ok(r) = rx.try_recv() {
                self.turn_rx = None;
                match r {
                    Ok(t) => {
                        self.turn = Some(t);
                        self.cambio = true;
                    }
                    Err(e) => self.monitor.anota(
                        ahora_s(),
                        Nivel::Info,
                        format!("servicio de llamadas del relé no disponible: {e}"),
                    ),
                }
            }
        }
        if self.turn.is_some()
            || self.turn_rx.is_some()
            || !self.otro_rtc
            || self.fase != Fase::Lista
            || ahora_s().saturating_sub(self.turn_pedido) < 60
        {
            return;
        }
        let (Some(rele), Some(sala)) = (self.rele_usado.clone(), self.sala_rele.clone()) else {
            return;
        };
        self.turn_pedido = ahora_s();
        let (tx, rx) = mpsc::channel();
        self.turn_rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(crate::rele::turn(&rele, &sala));
        });
    }

    /// The WebRTC part of the call for the screens (or null: the call then goes the old way,
    /// recorded pieces through this program). Who makes the offer is fixed by the two keys.
    fn rtc_json(&self) -> Value {
        match (&self.turn, self.otro_rtc) {
            (Some(t), true) => json!({ "ice": t.json(), "ofrece": self.mi_numero() == 0 }),
            // Still asking the relay: the screens wait a moment before the old way.
            (None, true) if self.turn_rx.is_some() => json!({ "pendiente": true }),
            _ => Value::Null,
        }
    }

    /// A WebRTC message from whoever talks here (the page, or the phone that took the call):
    /// an offer, an answer, an ICE candidate or a restart. It goes to the other side inside
    /// Olm, which is what makes the media keys (DTLS fingerprints in the SDP) trustworthy.
    pub fn rtc(&mut self, d: &Value) {
        if self.fase != Fase::Lista || self.llamada.is_none() {
            return;
        }
        let grande = match serde_json::to_vec(d) {
            Ok(v) => v.len() > 64 * 1024,
            Err(_) => true,
        };
        if grande {
            return;
        }
        self.manda_olm(json!({ "m": "rtc", "d": d }));
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
        // An old error must not keep the «continue on the phone» card on screen for good.
        if self.movil_error.take().is_some() {
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
        // During a call the voice moves only once the phone's leg is live.
        self.relevo_empieza();
    }

    /// The page turned «Voz protegida» on or off: remember it and tell the other side.
    pub fn voz_protegida(&mut self, activa: bool) {
        self.voz_protegida = activa;
        if self.fase == Fase::Lista {
            self.manda_olm(json!({ "m": "voz", "protegida": activa }));
        }
        self.cambio = true;
    }

    /// Tell the other side whether our phone is the one talking, each time that changes.
    fn di_dispositivo(&mut self) {
        if self.fase != Fase::Lista {
            return;
        }
        let ahora = self.movil_manda();
        if self.movil_dicho == Some(ahora) {
            return;
        }
        if self.movil_dicho.is_none() && self.voz_protegida {
            self.manda_olm(json!({ "m": "voz", "protegida": true }));
        }
        self.movil_dicho = Some(ahora);
        self.manda_olm(json!({ "m": "dispositivo", "movil": ahora }));
    }

    /// Whether the other side said it is talking from its phone.
    pub fn otro_en_movil(&self) -> bool {
        self.otro_movil
    }

    /// Whether the other side said its voice leaves transformed.
    pub fn otro_voz_protegida(&self) -> bool {
        self.otro_voz_protegida
    }

    /// A phone is connected and the person accepted it.
    fn movil_listo(&self) -> bool {
        self.movil
            .as_ref()
            .is_some_and(|m| m.enganchado && m.aprobado)
    }

    /// Whether the phone is the one talking right now: during a call, only once its own leg
    /// carries the voice (make before break, [`crate::relevo`]).
    fn movil_manda(&self) -> bool {
        self.relevo
            .movil_manda(self.movil_listo(), self.llamada.is_some())
    }

    /// The call goes by WebRTC (the relay's call service, both programs able).
    fn rtc_disponible(&self) -> bool {
        self.turn.is_some() && self.otro_rtc
    }

    /// The phone's voice travels by the bridge to this computer's page (not by pieces).
    fn puente_modo(&self) -> bool {
        self.relevo.puente(self.rtc_disponible())
    }

    /// What a step of the hand-off did: the monitor, the person and the screens are told.
    fn relevo_suceso(&mut self, s: Option<crate::relevo::Suceso>) {
        use crate::relevo::Suceso;
        let Some(s) = s else { return };
        match s {
            Suceso::EnElMovil => {
                self.monitor
                    .anota(ahora_s(), Nivel::Info, "la voz de la llamada pasó al móvil");
            }
            Suceso::NoPaso => {
                self.monitor.anota(
                    ahora_s(),
                    Nivel::Info,
                    "no se pudo pasar la llamada al móvil; sigue en el equipo",
                );
                self.aviso("sec_mv_no_paso", "");
            }
            Suceso::Volvio => {
                self.monitor.anota(
                    ahora_s(),
                    Nivel::Info,
                    "la voz de la llamada volvió al equipo",
                );
            }
        }
        // The other side's player starts again with whoever records now.
        self.reinicios += 1;
        self.cambio = true;
    }

    /// Once a tick: the hand-off follows the phone and the call.
    fn relevo_tic(&mut self) {
        let antes = self.relevo.donde();
        let conectada = self.llamada.as_ref().is_some_and(|l| l.conectada);
        let s = self.relevo.tic(
            self.movil_listo(),
            self.llamada.is_some(),
            conectada,
            ahora_s(),
        );
        self.relevo_suceso(s);
        if antes != self.relevo.donde() {
            self.cambio = true;
        }
    }

    /// Start moving the call's voice to the phone (accepted, back, or asked for).
    fn relevo_empieza(&mut self) {
        if self
            .relevo
            .empieza(self.movil_listo(), self.llamada.is_some(), ahora_s())
        {
            self.monitor
                .anota(ahora_s(), Nivel::Info, "pasando la llamada al móvil");
            self.cambio = true;
        }
    }

    /// The person asked to move the call to the phone (on the phone or on this screen).
    pub fn relevo_pasar(&mut self) {
        self.relevo_empieza();
    }

    /// «Volver al PC»: during a call the voice comes back to this computer (whose leg never
    /// stopped) and the phone stays for the chat; without a call the phone hands the whole
    /// conversation back.
    pub fn relevo_volver(&mut self) {
        if self.llamada.is_none() {
            self.movil_fin();
            return;
        }
        let s = self.relevo.volver();
        self.relevo_suceso(s);
    }

    /// A bridge message from the phone (offer, ICE candidate, end) for this computer's page.
    fn puente_del_movil(&mut self, d: Value) {
        if self.llamada.is_none() || !self.puente_modo() {
            return;
        }
        let grande = match serde_json::to_vec(&d) {
            Ok(v) => v.len() > 64 * 1024,
            Err(_) => true,
        };
        if !grande && self.puente_para_pagina.len() < 256 {
            self.puente_para_pagina.push_back(d);
        }
    }

    /// A bridge message from this computer's page (answer, ICE candidate) for the phone.
    pub fn puente_de_pagina(&mut self, d: &Value) {
        let grande = match serde_json::to_vec(d) {
            Ok(v) => v.len() > 64 * 1024,
            Err(_) => true,
        };
        if grande || !self.movil_listo() {
            return;
        }
        if let Some(m) = &self.movil {
            m.manda(crate::movil::Hacia::Estado(
                json!({ "tipo": "secure_puente", "d": d }).to_string(),
            ));
        }
    }

    /// The page says its bridge to the phone carries sound: the voice is on the phone.
    pub fn puente_listo(&mut self) {
        let s = self.relevo.puente_listo(self.movil_listo());
        self.relevo_suceso(s);
    }

    /// The page's bridge broke or never came up; `sin_rtc`: the page's own leg left WebRTC.
    pub fn puente_fallo(&mut self, sin_rtc: bool) {
        let s = self.relevo.puente_fallo(sin_rtc, ahora_s());
        self.relevo_suceso(s);
    }

    /// How the computer's leg of the call sounds, as the page estimated it.
    pub fn calidad(&mut self, nivel: &str) {
        if matches!(nivel, "excelente" | "buena" | "debil") && self.llamada.is_some() {
            self.calidad_pc = Some((nivel.to_owned(), ahora_s()));
        }
    }

    /// The page's estimate, while it is fresh (6 s: it comes every 2 s).
    fn calidad_pc_json(&self) -> Value {
        match &self.calidad_pc {
            Some((n, t)) if self.llamada.is_some() && ahora_s().saturating_sub(*t) <= 6 => {
                json!(n)
            }
            _ => Value::Null,
        }
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
        // A code nobody used within a minute is replaced by a new one (18 Oct 2026).
        if self
            .movil
            .as_ref()
            .is_some_and(crate::movil::Movil::caducado)
        {
            let idioma = self.movil_idioma.clone();
            self.movil = None;
            if let Some(p) = self.movil_puerta.take() {
                std::thread::spawn(move || p.cierra());
            }
            if self.movil_empieza(&idioma).is_err() {
                self.cambio = true;
            }
            return;
        }
        let Some(m) = &mut self.movil else {
            return;
        };
        let llegado = m.recoge();
        if llegado.is_empty() {
            return;
        }
        let aprobado = m.aprobado;
        // Call pieces and file pieces change nothing on screen: redrawing (and sending the
        // whole state to the phone) for each one, about eight times a second during a call,
        // was a cause of the stutter (17 Oct 2026 review).
        let mut algo = false;
        for d in llegado {
            use crate::movil::Desde;
            if !matches!(d, Desde::Media(_) | Desde::ArchivoTrozo(_)) {
                algo = true;
            }
            match d {
                Desde::Enganchado => {
                    self.aviso("sec_mv_enganchado", "");
                    self.novedad = true;
                    self.monitor
                        .anota(ahora_s(), Nivel::Info, "un móvil se ha conectado");
                    // An accepted phone that comes back during a call takes it again, the
                    // same way: the computer keeps it until the phone's leg is live.
                    if aprobado {
                        self.relevo_empieza();
                    }
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
                Desde::Visto => self.visto(),
                Desde::Reproduce(mimes) if aprobado => self.reproduce(mimes),
                Desde::Texto(t) if aprobado => self.texto(&t),
                Desde::TextoRe(t, re) if aprobado => self.texto_re(&t, Some(re)),
                Desde::Editar(id, t) if aprobado => self.editar(id, &t),
                Desde::Borrar(id, todos) if aprobado => self.borrar(id, todos),
                Desde::Reaccion(id, e) if aprobado => self.reaccionar(id, &e),
                Desde::Escribiendo if aprobado => self.escribiendo(),
                // A file from the phone goes to the other side like one chosen here: the phone
                // sends its bytes in pieces of 64 KB, each one becoming a block on the wire.
                Desde::ArchivoEmpieza(nombre, bytes, _mime) if aprobado => {
                    self.movil_trozo = None;
                    if let Err(e) = self.archivo_empieza(&nombre, bytes) {
                        self.aviso(&e, "");
                    }
                }
                Desde::ArchivoTrozo(b) if aprobado => {
                    if let Some(antes) = self.movil_trozo.replace(b) {
                        let _ = self.archivo_trozo(&B64.encode(antes), false);
                    }
                }
                Desde::ArchivoFin if aprobado => {
                    if let Some(ultimo) = self.movil_trozo.take() {
                        let _ = self.archivo_trozo(&B64.encode(ultimo), true);
                    }
                }
                Desde::Llamar { video, mime } if aprobado => {
                    let _ = self.llamar(video, &mime);
                }
                Desde::Colgar if aprobado => self.colgar(),
                Desde::Contestar(mime) if aprobado => {
                    let _ = self.contestar(&mime);
                }
                Desde::Rechazar if aprobado => self.rechazar(),
                Desde::Sas { ok, nombre } if aprobado => self.sas_persona(ok, &nombre),
                Desde::PideInicio if aprobado => self.pide_inicio(),
                Desde::VozProtegida(a) if aprobado => self.voz_protegida(a),
                // The phone's own WebRTC leg to the other side (a page from before the bridge):
                // only once it holds the voice and the call does not go by the bridge.
                Desde::Rtc(d)
                    if aprobado
                        && self.relevo.donde() == crate::relevo::Donde::Movil
                        && !self.puente_modo() =>
                {
                    self.rtc(&d);
                }
                Desde::Puente(d) if aprobado => self.puente_del_movil(d),
                Desde::Volver if aprobado => self.relevo_volver(),
                Desde::Pasar if aprobado => self.relevo_pasar(),
                Desde::Media(b) if aprobado => {
                    // Make before break with pieces: the phone's first piece moves the voice.
                    let (vale, s) = self.relevo.pieza_del_movil(true);
                    self.relevo_suceso(s);
                    if vale {
                        let _ = self.media_bytes(b);
                    }
                }
                _ => {}
            }
        }
        if algo {
            self.cambio = true;
        }
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
            "listo": self.movil_listo(),
            "paso": self.relevo.json(self.rtc_disponible(), ahora_s()),
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
        let mut e = json!({
            "tipo": "secure_estado",
            "fase": fase,
            "puerto": self.puerto,
            "codigo": self.puerta.as_ref().map(crate::puerta::Puerta::codigo),
            "puerta_error": self.puerta_error,
            "reproduce_otro": self.reproduce_otro,
            "otro_movil": self.otro_movil,
            "otro_voz_protegida": self.otro_voz_protegida,
            "incidente": self.estado_incidente(),
            "reconectando": self.reconexion.as_ref().map(|r| json!({ "desde": r.desde, "intentos": r.intentos })),
            "cuantico": self.capa.is_some(),
            "rele_codigo": self.rele_codigo,
            "rele": self.rele,
            "rele_error": self.rele_error,
            "grupo": self.grupo,
            "huella": self.huella_propia(),
            "tarjeta": self.tarjeta(),
            "contacto": self.nombre_contacto,
            "numeros": self.numeros().map(|(a, b, c)| vec![a, b, c]),
            "llamada": self.llamada.as_ref().map(|l| json!({ "video": l.video, "desde": l.desde, "conectada": l.conectada, "mi_numero": self.mi_numero(), "mime_remoto": l.mime_remoto, "reinicio": self.reinicios, "rtc": self.rtc_json(), "calidad_pc": self.calidad_pc_json() })),
            "entrante": self.entrante.as_ref().map(|t| json!({ "video": t.video, "de": self.nombre_contacto, "desde": t.desde })),
            "perdida": self.perdida.map(|(t, video)| json!({ "t": t, "video": video, "de": self.nombre_contacto })),
            "recibidos": self.recibidos.iter().map(Recibido::json).collect::<Vec<_>>(),
            "envio": self.envio_json(),
            "enviados": self.enviados.iter().filter(|x| x.t > 0).map(|x| json!({ "nombre": x.nombre, "t": x.t, "bytes": x.bytes, "llego_en": x.llego_en })).collect::<Vec<_>>(),
            "recibiendo": self.entrada.as_ref().filter(|e| e.recepcion.is_some()).map(|e| json!({ "nombre": e.nombre, "bytes": e.esperado, "hecho": e.recibido })),
            "movil": self.movil_json(),
            "mensajes": self.mensajes.iter().rev().take(300).rev().map(Mensaje::json).collect::<Vec<_>>(),
            "escribiendo": self.otro_escribiendo(),
            "archivos": true,
            "contactos": self.agenda.contactos().iter().map(|c| json!({ "nombre": c.nombre, "verificado_en": c.verificado_en, "estado": matches!(c.estado, agenda::Estado::Verificado), "numero": agenda::numero_de_base64(&c.signing) })).collect::<Vec<_>>(),
            "mi_numero": self.numero_propio(),
            "numero_otro": self.tarjeta_otra.as_ref().map(|t| agenda::numero(t.signing())),
            "monitor": monitor,
            "vigia": {
                "este": self.vigia.veredicto(self.inicio),
                "en_uso": self.vigia.en_uso.iter().map(|(p, r)| json!({ "programa": p, "recurso": r })).collect::<Vec<_>>(),
                "otro": self.vigia_otro,
            },
            "informe": self.informe.as_ref().map(|r| json!({ "json": r.to_json(), "csv": r.to_csv(), "clase": serde_json::to_value(r.clase).unwrap_or(Value::Null), "contacto": r.contacto, "duracion_s": r.duracion_s(), "inicio": r.inicio, "fin": r.fin, "veredicto": r.veredicto, "sin_espionaje": r.veredicto.as_ref().is_some_and(Veredicto::sin_espionaje), "conexion": r.conexion, "observadas": r.observadas, "inusuales": r.inusuales, "firmante": r.firmante, "firma": r.firma, "hash_cadena": r.hash_cadena })),
            "resumen": self.resumen.as_ref().map(|r| json!({ "json": r.to_json(), "csv": r.to_csv(), "clase": serde_json::to_value(r.clase).unwrap_or(Value::Null), "contacto": r.contacto, "duracion_s": r.duracion_s(), "mensajes": r.mensajes_enviados + r.mensajes_recibidos, "archivos": r.archivos.enviados + r.archivos.recibidos, "observadas": r.observadas, "inusuales": r.inusuales, "fin": r.fin })),
        });
        e["escudo"] = self.escudo_json();
        e["video"] = json!(VIDEO);
        e["temporizador"] = json!(self.temporizador);
        e["temporizadores"] = json!(TEMPORIZADORES);
        e
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
    // At most 60 bytes, cut where a character ends (a cut letter made the name unreadable and
    // mixed two people's pieces in one player).
    let mut fin = de.len().min(60);
    while !de.is_char_boundary(fin) {
        fin -= 1;
    }
    let nombre: Vec<u8> = de.get(..fin).unwrap_or_default().as_bytes().to_vec();
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

/// The key that locks the identity and the book: sealed with DPAPI for this Windows account
/// since 18 Oct 2026 (an older plain file is sealed on the first read), owner-only elsewhere.
fn lee_o_crea_clave(ruta: &Path) -> [u8; 32] {
    let nueva = || {
        let mut k = [0u8; 32];
        crate::sesion::azar(&mut k);
        Some(k.to_vec())
    };
    crate::dpapi::lee_o_crea(ruta, nueva, |b| b.len() == 32)
        .and_then(|b| <[u8; 32]>::try_from(b.as_slice()).ok())
        .unwrap_or_else(|| {
            // No disk at all: a key for this run only (nothing can be restored later).
            let mut k = [0u8; 32];
            crate::sesion::azar(&mut k);
            k
        })
}

fn limpia_nombre(n: &str) -> String {
    let s: String = n
        .chars()
        .filter(|c| {
            !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') && !c.is_control()
        })
        .take(120)
        .collect();
    // No dot or space at the end either: Windows drops them, so «x.exe.» would be saved and
    // opened as «x.exe» after passing as a file with no extension (17 Oct 2026 review).
    let s = s
        .trim()
        .trim_start_matches('.')
        .trim_end_matches(['.', ' '])
        .to_owned();
    if s.is_empty() {
        "archivo".to_owned()
    } else if nombre_de_dispositivo(&s) {
        format!("_{s}")
    } else {
        s
    }
}

/// Windows keeps these names for devices, with any extension («CON.txt» too).
pub(crate) fn nombre_de_dispositivo(n: &str) -> bool {
    let base = n
        .split('.')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_uppercase();
    matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((base.starts_with("COM") || base.starts_with("LPT"))
            && base.len() == 4
            && base
                .as_bytes()
                .get(3)
                .is_some_and(|d| (b'1'..=b'9').contains(d)))
}

fn mime_imagen(nombre: &str) -> &'static str {
    match extension(nombre).as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "image/bmp",
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
    #[test]
    fn the_path_is_named_without_an_address() {
        use std::net::IpAddr;
        for (ip, misma) in [
            ("192.168.1.20", true),
            ("10.0.0.5", true),
            ("172.16.3.4", true),
            ("169.254.1.1", true),
            ("127.0.0.1", true),
            ("fe80::1", true),
            ("fd12::1", true),
            ("::1", true),
            ("203.0.113.7", false),
            ("8.8.8.8", false),
            ("2001:db8::1", false),
        ] {
            assert_eq!(
                super::es_misma_red(ip.parse::<IpAddr>().unwrap()),
                misma,
                "{ip}"
            );
        }
        assert_eq!(
            super::ciudad_de_rele("rele.guardianagroup.com:443"),
            Some("Núremberg")
        );
        assert_eq!(
            super::ciudad_de_rele("rele-am.guardianagroup.com:443"),
            Some("Miami")
        );
        assert_eq!(super::ciudad_de_rele("127.0.0.1:9"), None);
    }

    #[test]
    fn received_names_cannot_hide_a_program_or_a_device() {
        assert_eq!(super::limpia_nombre("x.exe."), "x.exe");
        assert_eq!(super::limpia_nombre("foto.jpg . "), "foto.jpg");
        assert_eq!(super::limpia_nombre("CON.txt"), "_CON.txt");
        assert_eq!(super::limpia_nombre("lpt1"), "_lpt1");
        assert_eq!(super::limpia_nombre("COM0.txt"), "COM0.txt");
        assert_eq!(super::limpia_nombre("console.txt"), "console.txt");
        assert_eq!(super::limpia_nombre("..."), "archivo");
        let largo = "ñ".repeat(40);
        let v = super::con_remitente(&largo, b"x");
        let n = usize::from(v[0]);
        assert!(std::str::from_utf8(&v[1..=n]).is_ok());
    }

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
        let _turno = crate::PRUEBAS_RED
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        assert_eq!(b.mensajes.last().unwrap().texto, "hola Bea");
        assert!(!b.mensajes.last().unwrap().mio);

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
        // Each side told the other it talks from its computer: the screen shows the highest
        // level. (A phone talking would say so the same way; see `di_dispositivo`.)
        assert_eq!(b.estado_json()["otro_movil"], false);
        assert_eq!(a.movil_dicho, Some(false));
        // «Voz protegida» on a's side: b hears it and shows it on the call panel.
        a.voz_protegida(true);
        gira(&mut a, &mut b, 15);
        assert_eq!(b.estado_json()["otro_voz_protegida"], true);
        a.voz_protegida(false);
        gira(&mut a, &mut b, 15);
        assert_eq!(b.estado_json()["otro_voz_protegida"], false);

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
        assert!(b.mensajes.last().unwrap().archivo.as_deref() == Some("informe.pdf"));
        assert!(b.monitor.cifrado.recibidos >= 2);

        // Three frames that fail to open within ten seconds: a stop that looks like an
        // interference, with the card and the signed report, which say «directo por la misma
        // red» and never the other end's address; the person closes it and the card goes. (With the
        // destruction on an anomaly turned off: the old pause; see the next test for it on.)
        b.auto_destruccion = false;
        assert!(b.estado_json()["incidente"].is_null());
        b.fallo_abriendo(false);
        assert!(
            b.estado_json()["incidente"].is_null(),
            "one failure is a bad network"
        );
        b.fallo_abriendo(false);
        b.fallo_abriendo(false);
        let inc = b.estado_json()["incidente"].clone();
        assert_eq!(inc["senal"], "olm_fallos");
        assert_eq!(inc["fallos"], 3);
        assert_eq!(inc["transporte"], "misma_red");
        assert!(inc.get("otro").is_none(), "no other end on the card");
        assert!(
            !inc.to_string().contains("127.0.0.1"),
            "no address on the card"
        );
        assert!(!b.estado.en_marcha(), "sends are frozen");
        let informe: Value = serde_json::from_str(&b.informe_incidente().unwrap()).unwrap();
        assert_eq!(informe["tipo"], "guardiana-secure-incidente");
        assert_eq!(informe["incidente"]["senal"], "olm_fallos");
        assert!(informe["prueba"]["firma"].is_string());
        assert!(
            !b.informe_incidente().unwrap().contains("127.0.0.1"),
            "no address in the saved report"
        );
        b.olvida_incidente();
        assert!(b.estado_json()["incidente"].is_null());

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
        let _turno = crate::PRUEBAS_RED
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        assert_eq!(b.mensajes.last().unwrap().texto, "por el relé");

        // The link drops (a router reboot, a change of network): the conversation is not
        // over. Both sides go back to the relay to a room they derive on their own, meet
        // again, and the same Olm session carries on: no new greeting, no numbers again.
        for s in [&mut a, &mut b] {
            s.enlace = None;
            s.rx = None;
            assert!(s.empieza_reconexion());
            assert!(s.reconectando());
        }
        assert_eq!(
            a.reconexion.as_ref().unwrap().sala,
            b.reconexion.as_ref().unwrap().sala
        );
        let sala = a.reconexion.as_ref().unwrap().sala.clone();
        assert!(!sala.contains("127.0.0.1"));
        gira(&mut a, &mut b, 60);
        assert!(!a.reconectando(), "{}", a.estado_json());
        assert!(!b.reconectando(), "{}", b.estado_json());
        assert_eq!(a.estado_json()["fase"], "lista");
        assert_eq!(b.estado_json()["fase"], "lista");
        b.texto("sigo aquí");
        gira(&mut a, &mut b, 15);
        assert_eq!(a.mensajes.last().unwrap().texto, "sigo aquí");
        assert!(a.monitor.identidad.is_some(), "still verified");

        // A file cut in the middle by another drop goes on from where the other side got to:
        // the blocks the drop swallowed go out again, and the file arrives whole (16 Oct
        // 2026, «reanudación»).
        let plano: Vec<u8> = (0..(guardiana_secure::archivo::BLOQUE * 5 + 77))
            .map(|i| (i % 251) as u8)
            .collect();
        let trozos: Vec<&[u8]> = plano.chunks(guardiana_secure::archivo::BLOQUE).collect();
        a.archivo_empieza("grande.bin", plano.len() as u64).unwrap();
        gira(&mut a, &mut b, 5);
        for t in &trozos[..2] {
            a.archivo_trozo(&B64.encode(t), false).unwrap();
        }
        gira(&mut a, &mut b, 10);
        assert_eq!(
            b.entrada
                .as_ref()
                .unwrap()
                .recepcion
                .as_ref()
                .unwrap()
                .bloques(),
            2
        );
        // The next block is handed over right as the link dies: nobody gets it.
        a.archivo_trozo(&B64.encode(trozos[2]), false).unwrap();
        for s in [&mut a, &mut b] {
            s.enlace = None;
            s.rx = None;
            assert!(s.empieza_reconexion());
        }
        assert!(a.envio_json()["pausado"].as_bool().unwrap());
        // The page keeps handing pieces over only once `pausado` clears; here two more come
        // while it is paused, to be sure they wait inside the program.
        for t in &trozos[3..5] {
            a.archivo_trozo(&B64.encode(t), false).unwrap();
        }
        gira(&mut a, &mut b, 60);
        assert!(!a.reconectando() && !b.reconectando());
        gira(&mut a, &mut b, 10);
        assert!(
            !a.envio_json()["pausado"].as_bool().unwrap(),
            "{}",
            a.envio_json()
        );
        a.archivo_trozo(&B64.encode(trozos[5]), true).unwrap();
        gira(&mut a, &mut b, 20);
        let r = b.recibidos.last().unwrap();
        assert_eq!(r.nombre, "grande.bin");
        assert_eq!(std::fs::read(&r.ruta).unwrap(), plano);
        assert!(a.enviados.last().unwrap().llego_en.is_some());
        assert!(a.salida.is_none() && a.envio.is_none());

        // A deliberate close says goodbye: the other side does not wait at the relay.
        a.cerrar("adiós");
        gira(&mut a, &mut b, 15);
        assert!(!b.reconectando());
        assert_eq!(b.estado_json()["fase"], "sola");
        let _ = std::fs::remove_dir_all(base);
        // The next test must not find this relay, gone with this runtime.
        std::env::remove_var("GUARDIANA_RELE");
    }

    #[test]
    fn another_program_on_the_microphone_stops_the_talk_and_the_call_report_says_so() {
        let _turno = crate::PRUEBAS_RED
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let base = std::env::temp_dir().join(format!("guardiana-vigia-{}", std::process::id()));
        let (da, db) = (base.join("a"), base.join("b"));
        std::fs::create_dir_all(&da).unwrap();
        std::fs::create_dir_all(&db).unwrap();
        let mut a = Secure::abre(&da, &da);
        let mut b = Secure::abre(&db, &db);
        let puerto = a.escuchar_efimero().unwrap();
        b.conectar(&format!("127.0.0.1:{puerto}")).unwrap();
        gira(&mut a, &mut b, 25);
        a.sas_persona(true, "Bea");
        b.sas_persona(true, "Ana");
        gira(&mut a, &mut b, 25);
        assert_eq!(b.estado_json()["fase"], "lista");
        // The old pause (destruction on an anomaly is tested on its own below).
        b.auto_destruccion = false;

        // GUARDIANA's own window and its engine never count; a recorder that is on now does.
        let ahora = ahora_s();
        let ft = |t: u64| (t + 11_644_473_600) * 10_000_000;
        let propio = crate::vigia::de_registro(
            "C:#EdgeWebView#msedgewebview2.exe",
            crate::vigia::Recurso::Microfono,
            ft(ahora),
            0,
        )
        .unwrap();
        b.vigia(std::slice::from_ref(&propio));
        assert!(b.estado.en_marcha());
        let zoom = crate::vigia::de_registro(
            "C:#Program Files#Zoom#Zoom.exe",
            crate::vigia::Recurso::Microfono,
            ft(ahora),
            0,
        )
        .unwrap();
        b.vigia(&[propio.clone(), zoom.clone()]);
        assert!(
            !b.estado.en_marcha(),
            "stopped while Zoom.exe holds the microphone"
        );
        assert!(b
            .cadena
            .registros
            .iter()
            .any(|r| r.proceso.as_deref() == Some("Zoom.exe")
                && r.evidencia
                    .as_deref()
                    .is_some_and(|e| e.contains("ConsentStore"))));
        assert_eq!(
            b.estado_json()["vigia"]["en_uso"][0]["programa"],
            "Zoom.exe"
        );
        // The person says it is theirs: the talk goes on, and is not stopped again for it.
        b.reanudar_a_mano();
        b.vigia(&[propio.clone(), zoom.clone()]);
        assert!(b.estado.en_marcha());
        b.pantalla(true);
        gira(&mut a, &mut b, 15);
        // The other side heard what b's watch saw.
        let otro = a.vigia_otro.clone().unwrap();
        assert!(otro.vigilado && otro.pantalla_protegida);
        assert_eq!(otro.microfono, vec!["Zoom.exe".to_owned()]);

        // A call, measured by the page, then hung up: both get a signed report with a verdict.
        a.llamar(false, "audio/webm;codecs=opus").unwrap();
        gira(&mut a, &mut b, 15);
        b.contestar("audio/webm;codecs=opus").unwrap();
        gira(&mut a, &mut b, 15);
        a.medidas(&json!({ "rele": true, "latencia_media_ms": 96, "perdida_max_pct": 1.1, "kbps_audio_medio": 48, "codec_audio": "opus" }));
        a.colgar();
        gira(&mut a, &mut b, 15);
        let ia = a.informe.clone().unwrap();
        assert!(ia.verificar().is_ok());
        assert_eq!(ia.clase, Clase::Llamada);
        assert_eq!(ia.conexion.latencia_media_ms, Some(96));
        let va = ia.veredicto.clone().unwrap();
        assert!(va.identidad_verificada);
        assert_eq!(
            va.otro.as_ref().unwrap().microfono,
            vec!["Zoom.exe".to_owned()]
        );
        assert!(
            !va.sin_espionaje(),
            "the other side's microphone was shared"
        );
        assert!(
            !va.este.vigilado,
            "a never got Windows' record in this test"
        );
        let ib = b.informe.clone().unwrap();
        let vb = ib.veredicto.unwrap();
        assert_eq!(vb.este.microfono, vec!["Zoom.exe".to_owned()]);
        assert!(vb.este.pantalla_protegida);
        assert_eq!(a.estado_json()["informe"]["sin_espionaje"], false);
        a.olvida_informe();
        assert!(a.estado_json()["informe"].is_null());
        let _ = std::fs::remove_dir_all(base);
    }

    /// Two computers verified and talking, through a port on this computer.
    fn par(base: &Path) -> (Secure, Secure) {
        let (da, db) = (base.join("a"), base.join("b"));
        std::fs::create_dir_all(&da).unwrap();
        std::fs::create_dir_all(&db).unwrap();
        let mut a = Secure::abre(&da, &da);
        let mut b = Secure::abre(&db, &db);
        let puerto = a.escuchar_efimero().unwrap();
        b.conectar(&format!("127.0.0.1:{puerto}")).unwrap();
        gira(&mut a, &mut b, 25);
        a.sas_persona(true, "Bea");
        b.sas_persona(true, "Ana");
        gira(&mut a, &mut b, 25);
        assert_eq!(a.estado_json()["fase"], "lista");
        assert_eq!(b.estado_json()["fase"], "lista");
        (a, b)
    }

    #[test]
    fn a_recorder_that_starts_destroys_the_talk_on_both_sides_and_leaves_signed_evidence() {
        let _turno = crate::PRUEBAS_RED
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let base = std::env::temp_dir().join(format!("guardiana-destruye-{}", std::process::id()));
        let (mut a, mut b) = par(&base);
        a.texto("confidencial");
        gira(&mut a, &mut b, 15);
        assert_eq!(b.mensajes.last().unwrap().texto, "confidencial");
        // The shield's lines come from real events.
        let e = b.estado_json()["escudo"].clone();
        assert_eq!(e["identidad"]["ok"], true);
        assert!(e["identidad"]["desde"].as_u64().is_some());
        assert_eq!(e["cifrado"]["olm"], true);
        assert!(e["integridad"]["mensajes_ok"].as_u64().unwrap() >= 1);
        assert_eq!(e["integridad"]["mensajes_mal"], 0);
        assert_eq!(e["dispositivos"], json!(["este_equipo", "equipo_del_otro"]));
        // A recorder starts on b's microphone during the talk.
        let ft = |t: u64| (t + 11_644_473_600) * 10_000_000;
        let grabador = crate::vigia::de_registro(
            "C:#Users#x#Grabador.exe",
            crate::vigia::Recurso::Microfono,
            ft(ahora_s() + 1),
            0,
        )
        .unwrap();
        b.vigia(&[grabador]);
        let d = b.estado_json();
        assert_eq!(d["fase"], "sola");
        assert!(b.mensajes.is_empty(), "nothing of the talk stays on screen");
        assert!(b
            .destruida()
            .is_some_and(|d| d["motivo"] == "microfono" && d["programa"] == "Grabador.exe"));
        // The evidence: the signed chain, on disk, with the program and no message.
        let dir = base.join("b").join("secure").join("destrucciones");
        let f = std::fs::read_dir(&dir)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let texto = std::fs::read_to_string(&f).unwrap();
        let p = Prueba::from_json(&texto).unwrap();
        assert!(p.verificar().is_ok());
        assert!(texto.contains("Grabador.exe"));
        assert!(!texto.contains("confidencial"));
        assert!(!texto.contains("127.0.0.1"));
        // The other side got the signed notice and did the same.
        gira(&mut a, &mut b, 15);
        assert_eq!(a.estado_json()["fase"], "sola");
        assert!(a.mensajes.is_empty());
        assert!(a
            .destruida()
            .is_some_and(|d| d["motivo"] == "otro_lado" && d["otro_lado"] == true));
        a.olvida_destruida();
        assert!(a.destruida().is_none());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn a_forged_or_foreign_destruction_notice_is_ignored() {
        let _turno = crate::PRUEBAS_RED
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let base = std::env::temp_dir().join(format!("guardiana-falsa-{}", std::process::id()));
        let (a, mut b) = par(&base);
        let ahora = ahora_s();
        let inicio = b.inicio_destruccion();
        let sesion = b.sesion_destruccion(inicio);
        // Signed by a stranger (the relay, say), or for another conversation: nothing happens.
        let extrano = Identity::new();
        b.trama(
            json!({ "t": "destruir", "motivo": "microfono", "cuando": ahora, "inicio": inicio,
            "firma": destruccion::firma(&extrano, &sesion, "microfono", ahora) }),
        );
        b.trama(json!({ "t": "destruir", "motivo": "microfono", "cuando": ahora, "inicio": inicio + 1000,
            "firma": destruccion::firma(&a.identidad, &b.sesion_destruccion(inicio + 1000), "microfono", ahora) }));
        b.trama(json!({ "t": "destruir", "motivo": "microfono", "cuando": ahora - 3600, "inicio": inicio,
            "firma": destruccion::firma(&a.identidad, &sesion, "microfono", ahora - 3600) }));
        assert_eq!(b.estado_json()["fase"], "lista");
        assert!(b.destruida().is_none());
        // Both sides name the session the same way; a's real notice is obeyed.
        assert_eq!(a.sesion_destruccion(a.inicio_destruccion()), sesion);
        b.trama(
            json!({ "t": "destruir", "motivo": "camara", "cuando": ahora, "inicio": inicio,
            "firma": destruccion::firma(&a.identidad, &sesion, "camara", ahora) }),
        );
        assert_eq!(b.estado_json()["fase"], "sola");
        assert!(b.destruida().is_some_and(|d| d["motivo"] == "otro_lado"));
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn numbers_that_do_not_match_or_a_burst_of_bad_frames_destroy_the_talk() {
        let _turno = crate::PRUEBAS_RED
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let base = std::env::temp_dir().join(format!("guardiana-sas-{}", std::process::id()));
        let (da, db) = (base.join("a"), base.join("b"));
        std::fs::create_dir_all(&da).unwrap();
        std::fs::create_dir_all(&db).unwrap();
        let mut a = Secure::abre(&da, &da);
        let mut b = Secure::abre(&db, &db);
        let puerto = a.escuchar_efimero().unwrap();
        b.conectar(&format!("127.0.0.1:{puerto}")).unwrap();
        gira(&mut a, &mut b, 25);
        assert!(a.numeros().is_some());
        a.sas_persona(false, "");
        assert!(a
            .destruida()
            .is_some_and(|d| d["motivo"] == "codigo_distinto"));
        gira(&mut a, &mut b, 15);
        assert_eq!(b.estado_json()["fase"], "sola");
        assert!(
            b.destruida().is_some(),
            "told with a signed notice before the link closed"
        );
        let _ = std::fs::remove_dir_all(&base);

        let (mut a, mut b) = par(&base);
        a.texto("uno");
        gira(&mut a, &mut b, 10);
        b.fallo_abriendo(false);
        b.fallo_abriendo(false);
        b.fallo_abriendo(false);
        assert!(b.destruida().is_some_and(|d| d["motivo"] == "manipulada"));
        assert!(b.mensajes.is_empty());
        gira(&mut a, &mut b, 15);
        assert!(a.destruida().is_some());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn disappearing_messages_go_from_both_screens_and_frames_are_padded() {
        let _turno = crate::PRUEBAS_RED
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let base = std::env::temp_dir().join(format!("guardiana-temporal-{}", std::process::id()));
        let (mut a, mut b) = par(&base);
        a.pon_temporizador(30, true);
        gira(&mut a, &mut b, 10);
        assert_eq!(b.temporizador(), 30, "both screens use the same timer");
        a.texto("se borra");
        gira(&mut a, &mut b, 10);
        let t = b.mensajes.last().unwrap().clone();
        assert_eq!(t.texto, "se borra");
        assert!(t.caduca.is_some_and(|c| c >= t.t + 30));
        assert!(a.mensajes.last().unwrap().caduca.is_some());
        // Time passes (the lines carry their own end): both go.
        for m in a.mensajes.iter_mut().chain(b.mensajes.iter_mut()) {
            m.caduca = m.caduca.map(|_| 1);
        }
        gira(&mut a, &mut b, 2);
        assert!(a.mensajes.iter().all(|m| m.texto != "se borra"));
        assert!(b.mensajes.iter().all(|m| m.texto != "se borra"));
        // Every frame on the wire is one of a few sizes.
        let mut v = serde_json::to_vec(&json!({ "t": "latido" })).unwrap();
        guardiana_secure::relleno::rellena(&mut v, b' ', MAX_FRAME);
        assert_eq!(v.len(), guardiana_secure::relleno::MINIMO);
        // Video stays off: a call asked with video is voice.
        a.llamar(true, "audio/webm;codecs=opus").unwrap();
        assert_eq!(a.estado_json()["llamada"]["video"], false);
        assert_eq!(a.estado_json()["video"], false);
        a.colgar();
        let _ = std::fs::remove_dir_all(base);
    }
}
