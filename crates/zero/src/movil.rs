//! The phone takes the conversation over (15 Oct 2026, asked for «ya»): a call, a video call
//! or a chat that is running on this computer continues on the person's phone, with nothing
//! installed on it. The phone's camera reads a QR code shown by this computer; the code opens a
//! page of guardianagroup.com that talks to this computer directly, over WebTransport (HTTP/3,
//! QUIC, TLS 1.3), on the same Wi-Fi or from the street on mobile data through the door the
//! program opened in the router (`puerta`).
//!
//! Nothing is weakened on the way:
//! - the page is only the code of the page: the web server never sees a byte of the
//!   conversation, and the QR data travels in the URL's fragment, which the browser never sends;
//! - the phone talks TLS 1.3 to a certificate made here, pinned by its hash inside the QR code
//!   (`serverCertificateHashes`): no authority, no middleman possible;
//! - the phone proves it read the QR code with a secret that is only in the QR code, and the
//!   person says «OK» on this screen the first time it connects;
//! - the leg between this computer and the other person stays as it was: Olm for everything
//!   said, the media layer (`sframe`) for every piece of a call.
//!
//! What travels between phone and computer is one bidirectional stream of frames: one byte of
//! kind (0 = JSON text, 1 = a piece of a call), four bytes of length, the bytes.
//!
//! iPhone and iPad (16 Oct 2026, «necesito que funcione en iPhone y iPad también»): Safari does
//! not accept a pinned certificate yet, so the QR code also carries a key of 32 random bytes.
//! The phone's page can then go through GUARDIANA's relay (`wss://rele.guardianagroup.com`,
//! the relay's own public certificate) to a room named by a hash of that key, where this
//! computer waits. The relay passes messages it cannot read: after a greeting of two fresh
//! random numbers, each side derives its sending key from the QR key with HKDF-SHA256 and seals
//! every frame with AES-256-GCM and a counter, so nothing can be read, changed, replayed or
//! reordered on the way. The same path serves an Android phone on mobile data when the router
//! opened no door.
//!
//! Addresses are used to connect and are written nowhere (decision 203).

use std::net::{Ipv4Addr, UdpSocket};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use base64::Engine as _;
use ring::aead;
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::sync::mpsc as tmpsc;
use wtransport::endpoint::IncomingSession;
use wtransport::tls::Sha256DigestFmt;
use wtransport::{Endpoint, Identity, ServerConfig};

/// Where the page that talks to the phone lives (static; it never sees the conversation).
pub const PAGINA: &str = "https://guardianagroup.com/m/";
/// The origin the phone's page must come from.
const ORIGEN: &str = "https://guardianagroup.com";
/// The port asked first for the phone (next to SECURE's 47300); any port if taken.
pub const PUERTO: u16 = 47302;
/// Largest frame accepted from the phone (a piece of a call is ~16 KB; a text, ~8 KB).
const MAX_TRAMA: usize = 1024 * 1024;
const B64URL: base64::engine::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;

/// What the phone's thread tells the conversation.
#[derive(Debug)]
pub enum Desde {
    /// A phone that knows the secret connected (first time: the person must say OK).
    Enganchado,
    /// The phone wrote something.
    Texto(String),
    /// The phone starts, or takes over, a call; `mime` is what its recorder produces.
    Llamar {
        /// With camera.
        video: bool,
        /// The container and codecs of the phone's pieces (what the other side must play).
        mime: String,
    },
    /// The phone hangs up.
    Colgar,
    /// The phone answers the call ringing; `mime` is what its recorder produces.
    Contestar(String),
    /// The phone rejects the call ringing.
    Rechazar,
    /// The person compared the three numbers on the phone: whether they match, and the
    /// contact's name as typed there (the phone is in the conversation from the start since
    /// 16 Oct 2026).
    Sas {
        /// They matched.
        ok: bool,
        /// The name for the book.
        nombre: String,
    },
    /// The phone's player started: the other side's recorder must start again.
    PideInicio,
    /// One recorded piece from the phone's microphone or camera.
    Media(Vec<u8>),
    /// The phone's page was brought to the front.
    Visto,
    /// The phone answers a message: the text and the id it answers (16 Oct 2026).
    TextoRe(String, u64),
    /// The phone changed a text of ours.
    Editar(u64, String),
    /// The phone deletes a message; `para_todos` tells the other side too.
    Borrar(u64, bool),
    /// The phone reacts with an emoji.
    Reaccion(u64, String),
    /// The person is typing on the phone.
    Escribiendo,
    /// A file from the phone starts: name, size, type (its bytes come as frames of type 2).
    ArchivoEmpieza(String, u64, String),
    /// One piece of that file.
    ArchivoTrozo(Vec<u8>),
    /// The file is complete.
    ArchivoFin,
    /// The person typed their name on the phone (a home room, 16 Oct 2026).
    Nombre(String),
    /// What the phone's player can open (an iPhone may not open WebM): the other side records
    /// in one of these when it can.
    Reproduce(Vec<String>),
    /// The phone went away (network change, page closed); it may come back.
    Soltado,
    /// The listener itself failed.
    Error(String),
}

/// What the conversation tells the phone.
#[derive(Debug)]
pub enum Hacia {
    /// The whole state the page draws (the same JSON the Comunicaciones tab gets).
    Estado(String),
    /// One opened piece of the call from the other side.
    Media(Vec<u8>),
    /// One piece of a file for the phone (frame type 2): the file's id and the bytes.
    Archivo(u32, Vec<u8>),
    /// The person took the conversation back to the computer, or closed it.
    Fin,
}

/// The phone side of a conversation: a QUIC listener on its own thread, and the two queues.
pub struct Movil {
    /// The UDP port the phone connects to (the same inside and outside, through the door).
    pub puerto: u16,
    /// SHA-256 of the certificate the phone pins, as the QR code carries it.
    pub huella: String,
    /// The secret only the QR code holds.
    secreto: String,
    /// The key only the QR code holds, for the path through the relay.
    clave: [u8; 32],
    /// The relay the phone may go through (host, and port when it is not 443).
    rele: String,
    /// Addresses the phone may try, LAN first; the public one is added when the door opens.
    pub direcciones: Vec<String>,
    /// A phone is connected right now.
    pub enganchado: bool,
    /// A phone connected and the person has not said OK yet.
    pub pendiente: bool,
    /// The person said OK: this secret is trusted until the hand-off ends.
    pub aprobado: bool,
    rx: Receiver<Desde>,
    tx: tmpsc::UnboundedSender<Hacia>,
}

impl std::fmt::Debug for Movil {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Movil")
            .field("puerto", &self.puerto)
            .field("enganchado", &self.enganchado)
            .finish()
    }
}

impl Movil {
    /// Bind the listener and start the thread. `idioma` goes into the QR code so the page opens
    /// in the person's language.
    pub fn arranca() -> Result<Self, String> {
        Self::arranca_con(&crate::rele::direccion())
    }

    /// The same, with the relay at `rele` (`host:port`; the tests run one on this computer).
    pub fn arranca_con(rele: &str) -> Result<Self, String> {
        let mut clave = [0u8; 32];
        crate::sesion::azar(&mut clave);
        Self::arranca_con_clave(rele, clave)
    }

    /// The same, with the key the QR code will carry chosen by the caller (a home room derives
    /// one per seat from a single code, 16 Oct 2026).
    pub fn arranca_con_clave(rele: &str, clave: [u8; 32]) -> Result<Self, String> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, PUERTO))
            .or_else(|_| UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)))
            .map_err(|e| e.to_string())?;
        let puerto = socket.local_addr().map_err(|e| e.to_string())?.port();
        let mut direcciones = Vec::new();
        if let Some(ip) = crate::puerta::ip_local() {
            direcciones.push(format!("{ip}:{puerto}"));
        }
        // The certificate the phone pins: fresh every hand-off, two weeks of validity (what
        // browsers accept for a pinned certificate), ECDSA P-256.
        let mut sans: Vec<String> = direcciones
            .iter()
            .filter_map(|d| d.rsplit_once(':').map(|(ip, _)| ip.to_owned()))
            .collect();
        sans.push("guardiana.local".to_owned());
        let identidad = Identity::self_signed_builder()
            .subject_alt_names(sans.iter().map(String::as_str))
            .from_now_utc()
            .validity_days(13)
            .build()
            .map_err(|e| e.to_string())?;
        let huella = identidad
            .certificate_chain()
            .as_slice()
            .first()
            .map(|c| c.hash().fmt(Sha256DigestFmt::DottedHex).replace(':', ""))
            .ok_or_else(|| "sin certificado".to_owned())?;
        let mut azar = [0u8; 18];
        crate::sesion::azar(&mut azar);
        let secreto = B64URL.encode(azar);
        let rele_dir = rele.to_owned();
        let rele_host = rele.strip_suffix(":443").unwrap_or(rele).to_owned();

        let config = ServerConfig::builder()
            .with_bind_socket(socket)
            .with_identity(identidad)
            .keep_alive_interval(Some(Duration::from_secs(4)))
            .max_idle_timeout(Some(Duration::from_secs(20)))
            .map_err(|_| "tiempo de espera inválido".to_owned())?
            .build();
        let (tx_desde, rx) = mpsc::channel::<Desde>();
        let (tx, rx_hacia) = tmpsc::unbounded_channel::<Hacia>();
        let secreto_hilo = secreto.clone();
        std::thread::Builder::new()
            .name("guardiana-movil".to_owned())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        let _ = tx_desde.send(Desde::Error(e.to_string()));
                        return;
                    }
                };
                rt.block_on(servidor(
                    config,
                    secreto_hilo,
                    (rele_dir, clave),
                    tx_desde,
                    rx_hacia,
                ));
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            puerto,
            huella,
            secreto,
            clave,
            rele: rele_host,
            direcciones,
            enganchado: false,
            pendiente: false,
            aprobado: false,
            rx,
            tx,
        })
    }

    /// The door in the router opened: the phone can also come from outside.
    pub fn con_puerta(&mut self, externa: Ipv4Addr) {
        let d = format!("{externa}:{}", self.puerto);
        if !self.direcciones.contains(&d) {
            self.direcciones.push(d);
        }
    }

    /// What the QR code says: the page's address with everything the phone needs after `#`,
    /// which the browser keeps to itself.
    pub fn enlace(&self, idioma: &str, nombre: &str) -> String {
        let datos = json!({
            "v": 1,
            "h": self.huella,
            "a": self.direcciones,
            "s": self.secreto,
            "k": B64URL.encode(self.clave),
            "r": self.rele,
            "l": idioma,
            "n": nombre,
        });
        format!("{PAGINA}#{}", B64URL.encode(datos.to_string()))
    }

    /// Everything the phone's thread left since the last tick.
    pub fn recoge(&mut self) -> Vec<Desde> {
        let mut v = Vec::new();
        while let Ok(d) = self.rx.try_recv() {
            match &d {
                Desde::Enganchado => {
                    self.enganchado = true;
                    if !self.aprobado {
                        self.pendiente = true;
                    }
                }
                Desde::Soltado => {
                    self.enganchado = false;
                    self.pendiente = false;
                }
                _ => {}
            }
            v.push(d);
        }
        v
    }

    /// The person said OK to the phone that connected.
    pub fn aprueba(&mut self) {
        self.aprobado = true;
        self.pendiente = false;
    }

    /// Send something to the phone (dropped silently if none is connected).
    pub fn manda(&self, h: Hacia) {
        let _ = self.tx.send(h);
    }
}

impl Drop for Movil {
    fn drop(&mut self) {
        let _ = self.tx.send(Hacia::Fin);
    }
}

/// The listener: one phone at a time; a new one with the right secret replaces the old (that is
/// how a phone that changed network comes back).
async fn servidor(
    config: ServerConfig,
    secreto: String,
    (rele, clave): (String, [u8; 32]),
    tx: Sender<Desde>,
    mut rx: tmpsc::UnboundedReceiver<Hacia>,
) {
    // The relay's path waits in its room from the start, for a phone that cannot come direct.
    let (tx_rele, mut rx_rele) = tmpsc::unbounded_channel::<EnlaceRele>();
    tokio::spawn(rele_bucle(rele, clave, tx_rele));
    let endpoint = match Endpoint::server(config) {
        Ok(e) => e,
        Err(e) => {
            let _ = tx.send(Desde::Error(e.to_string()));
            return;
        }
    };
    let (tx_con, mut rx_con) = tmpsc::unbounded_channel::<wtransport::Connection>();
    // Accept in the background: every session with the secret and the right origin is handed
    // to the loop below, which keeps only the newest.
    {
        let tx_con = tx_con.clone();
        let secreto = secreto.clone();
        tokio::spawn(async move {
            loop {
                let entrante = endpoint.accept().await;
                let tx_con = tx_con.clone();
                let secreto = secreto.clone();
                tokio::spawn(async move {
                    if let Some(c) = admite(entrante, &secreto).await {
                        let _ = tx_con.send(c);
                    }
                });
            }
        });
    }
    // Frames from the current phone, read by their own task: (generation, kind, bytes); a
    // kind of 255 means that phone is gone.
    let (tx_tramas, mut rx_tramas) = tmpsc::unbounded_channel::<(u64, u8, Vec<u8>)>();
    let mut generacion: u64 = 0;
    let mut conexion: Option<wtransport::Connection> = None;
    let mut manda: Option<Salida> = None;
    loop {
        tokio::select! {
            Some(c) = rx_con.recv() => {
                if let Some(viejo) = conexion.take() {
                    viejo.close(wtransport::VarInt::from_u32(0), b"otro movil");
                }
                manda = None;
                let espera = tokio::time::timeout(Duration::from_secs(10), c.accept_bi());
                if let Ok(Ok((m, recibe))) = espera.await {
                    generacion += 1;
                    let g = generacion;
                    let tx_tramas = tx_tramas.clone();
                    tokio::spawn(async move {
                        let mut recibe = recibe;
                        while let Some((tipo, cuerpo)) = lee(&mut recibe).await {
                            if tx_tramas.send((g, tipo, cuerpo)).is_err() {
                                return;
                            }
                        }
                        let _ = tx_tramas.send((g, 255, Vec::new()));
                    });
                    manda = Some(Salida::Directa(m));
                    conexion = Some(c);
                    let _ = tx.send(Desde::Enganchado);
                }
            }
            Some(e) = rx_rele.recv() => {
                if let Some(viejo) = conexion.take() {
                    viejo.close(wtransport::VarInt::from_u32(0), b"otro movil");
                }
                generacion += 1;
                let g = generacion;
                let tx_tramas = tx_tramas.clone();
                let mut entra = e.entra;
                tokio::spawn(async move {
                    while let Some((tipo, cuerpo)) = entra.recv().await {
                        if tx_tramas.send((g, tipo, cuerpo)).is_err() {
                            return;
                        }
                    }
                    let _ = tx_tramas.send((g, 255, Vec::new()));
                });
                manda = Some(Salida::Rele(e.sale));
                let _ = tx.send(Desde::Enganchado);
            }
            Some(h) = rx.recv() => {
                let (tipo, cuerpo) = match h {
                    Hacia::Fin => {
                        if let Some(c) = conexion.take() {
                            c.close(wtransport::VarInt::from_u32(0), b"fin");
                        }
                        return;
                    }
                    Hacia::Estado(j) => (0u8, j.into_bytes()),
                    Hacia::Media(b) => (1u8, b),
                    Hacia::Archivo(id, b) => {
                        let mut c = id.to_be_bytes().to_vec();
                        c.extend_from_slice(&b);
                        (2u8, c)
                    }
                };
                if let Some(m) = &mut manda {
                    let ok = match m {
                        Salida::Directa(s) => escribe(s, tipo, &cuerpo).await.is_ok(),
                        Salida::Rele(s) => s.send((tipo, cuerpo)).is_ok(),
                    };
                    if !ok {
                        manda = None;
                        conexion = None;
                        let _ = tx.send(Desde::Soltado);
                    }
                }
            }
            Some((g, tipo, cuerpo)) = rx_tramas.recv() => {
                if g != generacion {
                    continue;
                }
                match tipo {
                    0 => {
                        if let Ok(v) = serde_json::from_slice::<Value>(&cuerpo) {
                            if let Some(d) = traduce(&v) {
                                let _ = tx.send(d);
                            }
                        }
                    }
                    1 => {
                        let _ = tx.send(Desde::Media(cuerpo));
                    }
                    2 => {
                        let _ = tx.send(Desde::ArchivoTrozo(cuerpo));
                    }
                    _ => {
                        manda = None;
                        conexion = None;
                        let _ = tx.send(Desde::Soltado);
                    }
                }
            }
        }
    }
}

/// Where the phone's frames go: straight over WebTransport, or sealed through the relay.
enum Salida {
    Directa(wtransport::SendStream),
    Rele(tmpsc::UnboundedSender<(u8, Vec<u8>)>),
}

/// A phone that came through the relay: frames to it, and its opened frames (kind, bytes).
struct EnlaceRele {
    sale: tmpsc::UnboundedSender<(u8, Vec<u8>)>,
    entra: tmpsc::UnboundedReceiver<(u8, Vec<u8>)>,
}

/// The room at the relay: a hash of the QR key, so the relay never sees the key.
pub fn sala_de_clave(clave: &[u8; 32]) -> String {
    let mut h = Sha256::new();
    h.update(b"guardiana-movil-sala");
    h.update(clave);
    h.finalize()
        .iter()
        .take(16)
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The two sending keys of one link: phone → computer, computer → phone. Both random numbers
/// are fresh every link, so a frame recorded from one link means nothing in the next. Since
/// 16 Oct 2026 (review with ChatGPT for the home room) the link also agrees a fresh secret
/// (ECDH P-256, `secreto`), mixed in with the QR key: whoever photographed the code can ask
/// to come in, but cannot read a link, not even another phone of the same room.
pub fn claves_de_enlace(
    clave: &[u8; 32],
    secreto: &[u8],
    del_equipo: &[u8; 16],
    del_movil: &[u8; 16],
) -> Option<(aead::LessSafeKey, aead::LessSafeKey)> {
    let mut sal = [0u8; 32];
    sal[..16].copy_from_slice(del_equipo);
    sal[16..].copy_from_slice(del_movil);
    let mut ikm = clave.to_vec();
    ikm.extend_from_slice(secreto);
    let prk = ring::hkdf::Salt::new(ring::hkdf::HKDF_SHA256, &sal).extract(&ikm);
    let una = |info: &[u8]| -> Option<aead::LessSafeKey> {
        let mut k = [0u8; 32];
        prk.expand(&[info], ring::hkdf::HKDF_SHA256)
            .ok()?
            .fill(&mut k)
            .ok()?;
        Some(aead::LessSafeKey::new(
            aead::UnboundKey::new(&aead::AES_256_GCM, &k).ok()?,
        ))
    };
    Some((
        una(b"guardiana-movil-1 m>e")?,
        una(b"guardiana-movil-1 e>m")?,
    ))
}

/// The nonce of frame number `n` (each direction has its own key, so its own count).
pub fn nonce(n: u64) -> aead::Nonce {
    let mut b = [0u8; 12];
    b[4..].copy_from_slice(&n.to_be_bytes());
    aead::Nonce::assume_unique_for_key(b)
}

/// Seal `datos` as frame `n`.
pub fn sella(k: &aead::LessSafeKey, n: u64, aad: &[u8], datos: &[u8]) -> Vec<u8> {
    let mut v = datos.to_vec();
    if k.seal_in_place_append_tag(nonce(n), aead::Aad::from(aad), &mut v)
        .is_err()
    {
        v.clear();
    }
    v
}

/// Open frame `n`, or `None` if it was changed, replayed or reordered.
pub fn abre(k: &aead::LessSafeKey, n: u64, aad: &[u8], sellado: &[u8]) -> Option<Vec<u8>> {
    let mut v = sellado.to_vec();
    let largo = k
        .open_in_place(nonce(n), aead::Aad::from(aad), &mut v)
        .ok()?
        .len();
    v.truncate(largo);
    Some(v)
}

/// Wait at the relay for the phone, over and over, until the hand-off ends.
async fn rele_bucle(rele: String, clave: [u8; 32], tx: tmpsc::UnboundedSender<EnlaceRele>) {
    let sala = sala_de_clave(&clave);
    loop {
        if tx.is_closed() {
            return;
        }
        let desde = std::time::Instant::now();
        let _ = rele_una_vez(&rele, &sala, &clave, &tx).await;
        let pausa = if desde.elapsed() > Duration::from_secs(30) {
            1
        } else {
            3
        };
        tokio::time::sleep(Duration::from_secs(pausa)).await;
    }
}

async fn conecta_rele(rele: &str) -> Option<tokio::net::TcpStream> {
    // The same way in as a conversation's (inside TLS for GUARDIANA's relay, see `rele::abre`),
    // handed over as a socket of this runtime.
    let rele = rele.to_owned();
    let s = tokio::task::spawn_blocking(move || crate::rele::abre(&rele))
        .await
        .ok()?
        .ok()?;
    s.set_nonblocking(true).ok()?;
    tokio::net::TcpStream::from_std(s).ok()
}

async fn lee_rele(r: &mut (impl tokio::io::AsyncRead + Unpin)) -> Option<Vec<u8>> {
    let mut n = [0u8; 4];
    r.read_exact(&mut n).await.ok()?;
    let n = u32::from_be_bytes(n) as usize;
    if n > MAX_TRAMA + 64 {
        return None;
    }
    let mut v = vec![0u8; n];
    r.read_exact(&mut v).await.ok()?;
    Some(v)
}

async fn escribe_rele(w: &mut (impl tokio::io::AsyncWrite + Unpin), datos: &[u8]) -> Option<()> {
    let mut c = Vec::with_capacity(4 + datos.len());
    c.extend_from_slice(&(datos.len() as u32).to_be_bytes());
    c.extend_from_slice(datos);
    w.write_all(&c).await.ok()?;
    w.flush().await.ok()
}

/// One stay at the relay: wait for the phone, greet it, then carry frames until it goes.
async fn rele_una_vez(
    rele: &str,
    sala: &str,
    clave: &[u8; 32],
    tx: &tmpsc::UnboundedSender<EnlaceRele>,
) -> Option<()> {
    let mut s = conecta_rele(rele).await?;
    let _ = s.set_nodelay(true);
    s.write_all(format!("GUARDIANA-RELE-M 1 {sala}\n").as_bytes())
        .await
        .ok()?;
    // The relay keeps a room ten minutes; then this comes back and waits again.
    let mut ok = [0u8; 3];
    tokio::time::timeout(Duration::from_secs(11 * 60), s.read_exact(&mut ok))
        .await
        .ok()?
        .ok()?;
    if &ok != b"OK\n" {
        return None;
    }
    // The greeting («GM2», 16 Oct 2026): our random number and a fresh P-256 public key; the
    // phone answers with its own number, its public key, and proves it holds the QR key with
    // the keys both derive from the QR key, the agreed secret and the two numbers.
    let rng = ring::rand::SystemRandom::new();
    let privada =
        ring::agreement::EphemeralPrivateKey::generate(&ring::agreement::ECDH_P256, &rng).ok()?;
    let publica = privada.compute_public_key().ok()?;
    let mut del_equipo = [0u8; 16];
    crate::sesion::azar(&mut del_equipo);
    let mut saludo = b"GM2".to_vec();
    saludo.extend_from_slice(&del_equipo);
    saludo.extend_from_slice(publica.as_ref());
    escribe_rele(&mut s, &saludo).await?;
    let respuesta = tokio::time::timeout(Duration::from_secs(20), lee_rele(&mut s))
        .await
        .ok()??;
    if respuesta.len() < 16 + 65 {
        return None;
    }
    let mut del_movil = [0u8; 16];
    del_movil.copy_from_slice(&respuesta[..16]);
    let suya = ring::agreement::UnparsedPublicKey::new(
        &ring::agreement::ECDH_P256,
        respuesta[16..16 + 65].to_vec(),
    );
    let secreto = ring::agreement::agree_ephemeral(privada, &suya, |s| s.to_vec()).ok()?;
    let (k_me, k_em) = claves_de_enlace(clave, &secreto, &del_equipo, &del_movil)?;
    if abre(&k_me, 0, b"GM2", &respuesta[16 + 65..]).as_deref() != Some(b"hola".as_slice()) {
        return None;
    }
    let (mut r, mut w) = s.into_split();
    let (tx_sale, mut rx_sale) = tmpsc::unbounded_channel::<(u8, Vec<u8>)>();
    let (tx_entra, rx_entra) = tmpsc::unbounded_channel::<(u8, Vec<u8>)>();
    tx.send(EnlaceRele {
        sale: tx_sale,
        entra: rx_entra,
    })
    .ok()?;
    // To the phone: every frame sealed with the next number; an empty one every 25 s so no
    // network in between drops a quiet link.
    let escritor = tokio::spawn(async move {
        let mut n: u64 = 0;
        let mut latido = tokio::time::interval(Duration::from_secs(25));
        latido.tick().await;
        loop {
            let (tipo, cuerpo) = tokio::select! {
                f = rx_sale.recv() => match f { Some(f) => f, None => break },
                _ = latido.tick() => (2u8, Vec::new()),
            };
            let mut plano = Vec::with_capacity(1 + cuerpo.len());
            plano.push(tipo);
            plano.extend_from_slice(&cuerpo);
            let sellado = sella(&k_em, n, b"", &plano);
            n += 1;
            if escribe_rele(&mut w, &sellado).await.is_none() {
                break;
            }
        }
        let _ = w.shutdown().await;
    });
    // From the phone: frame 0 was the proof, so its frames count from 1.
    let mut n: u64 = 1;
    while let Some(sellado) = lee_rele(&mut r).await {
        let Some(plano) = abre(&k_me, n, b"", &sellado) else {
            break;
        };
        n += 1;
        let Some((&tipo, cuerpo)) = plano.split_first() else {
            continue;
        };
        if tx_entra.send((tipo, cuerpo.to_vec())).is_err() {
            break;
        }
    }
    escritor.abort();
    Some(())
}

async fn escribe(m: &mut wtransport::SendStream, tipo: u8, cuerpo: &[u8]) -> Result<(), ()> {
    let mut cabecera = [0u8; 5];
    cabecera[0] = tipo;
    cabecera[1..].copy_from_slice(&(cuerpo.len() as u32).to_be_bytes());
    m.write_all(&cabecera).await.map_err(|_| ())?;
    m.write_all(cuerpo).await.map_err(|_| ())
}

/// One frame from the phone, or `None` when it is gone.
async fn lee(recibe: &mut wtransport::RecvStream) -> Option<(u8, Vec<u8>)> {
    let mut cabecera = [0u8; 5];
    recibe.read_exact(&mut cabecera).await.ok()?;
    let n = u32::from_be_bytes([cabecera[1], cabecera[2], cabecera[3], cabecera[4]]) as usize;
    if n > MAX_TRAMA {
        return None;
    }
    let mut cuerpo = vec![0u8; n];
    recibe.read_exact(&mut cuerpo).await.ok()?;
    Some((cabecera[0], cuerpo))
}

/// Only a phone that read the QR code, from our own page.
async fn admite(entrante: IncomingSession, secreto: &str) -> Option<wtransport::Connection> {
    let peticion = entrante.await.ok()?;
    let ruta_ok = iguales(peticion.path().trim_start_matches('/'), secreto);
    let origen_ok = peticion.origin().is_some_and(|o| o == ORIGEN);
    if !ruta_ok || !origen_ok {
        peticion.forbidden().await;
        return None;
    }
    peticion.accept().await.ok()
}

/// Same length and same bytes, looking at every byte.
fn iguales(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn traduce(v: &Value) -> Option<Desde> {
    match v["t"].as_str()? {
        "texto" => {
            let t: String = v["texto"].as_str()?.chars().take(8000).collect();
            match crate::mensaje::id_de(&v["re"]) {
                Some(re) => Some(Desde::TextoRe(t, re)),
                None => Some(Desde::Texto(t)),
            }
        }
        "editar" => Some(Desde::Editar(
            crate::mensaje::id_de(&v["id"])?,
            v["texto"].as_str()?.chars().take(8000).collect(),
        )),
        "borrar" => Some(Desde::Borrar(
            crate::mensaje::id_de(&v["id"])?,
            v["para_todos"].as_bool().unwrap_or(false),
        )),
        "reaccion" => Some(Desde::Reaccion(
            crate::mensaje::id_de(&v["id"])?,
            v["emoji"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .take(8)
                .collect(),
        )),
        "escribiendo" => Some(Desde::Escribiendo),
        "archivo" => Some(Desde::ArchivoEmpieza(
            v["nombre"]
                .as_str()
                .unwrap_or("archivo")
                .chars()
                .take(120)
                .collect(),
            v["bytes"].as_u64().unwrap_or(0),
            v["mime"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .take(80)
                .collect(),
        )),
        "archivo_fin" => Some(Desde::ArchivoFin),
        "llamar" => Some(Desde::Llamar {
            video: v["video"].as_bool().unwrap_or(false),
            mime: v["mime"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .take(80)
                .collect(),
        }),
        "colgar" => Some(Desde::Colgar),
        "contestar" => Some(Desde::Contestar(
            v["mime"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .take(80)
                .collect(),
        )),
        "rechazar" => Some(Desde::Rechazar),
        "sas" => Some(Desde::Sas {
            ok: v["ok"].as_bool().unwrap_or(false),
            nombre: v["nombre"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .take(60)
                .collect(),
        }),
        "pide_inicio" => Some(Desde::PideInicio),
        "visto" => Some(Desde::Visto),
        "nombre" => Some(Desde::Nombre(
            v["nombre"]
                .as_str()
                .unwrap_or_default()
                .trim()
                .chars()
                .take(40)
                .collect(),
        )),
        "reproduce" => Some(Desde::Reproduce(
            v["mimes"]
                .as_array()?
                .iter()
                .filter_map(Value::as_str)
                .take(8)
                .map(|s| s.chars().take(80).collect())
                .collect(),
        )),
        _ => None,
    }
}

/// Decode what the QR code carries (used by the tests and by anyone checking the format).
pub fn lee_enlace(enlace: &str) -> Option<Value> {
    let datos = enlace.strip_prefix(PAGINA)?.strip_prefix('#')?;
    let bytes = B64URL.decode(datos).ok()?;
    serde_json::from_slice(&bytes).ok()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn the_qr_link_carries_hash_addresses_secret_and_language_in_the_fragment() {
        let m = Movil::arranca().unwrap();
        let e = m.enlace("es", "Marta");
        assert!(e.starts_with("https://guardianagroup.com/m/#"));
        let d = lee_enlace(&e).unwrap();
        assert_eq!(d["v"], 1);
        assert_eq!(d["h"].as_str().unwrap().len(), 64);
        assert_eq!(d["l"], "es");
        assert_eq!(d["n"], "Marta");
        assert_eq!(d["s"].as_str().unwrap().len(), 24);
        for a in d["a"].as_array().unwrap() {
            assert!(a.as_str().unwrap().ends_with(&format!(":{}", m.puerto)));
        }
        // Nothing before the fragment names this computer.
        assert_eq!(e.split('#').next().unwrap(), PAGINA);
    }

    #[test]
    fn the_secret_check_rejects_a_near_miss() {
        assert!(iguales("abc", "abc"));
        assert!(!iguales("abc", "abd"));
        assert!(!iguales("abc", "abcd"));
    }

    #[test]
    fn a_wrong_secret_is_refused_and_the_right_one_gets_a_session() {
        // A real QUIC client against the listener: the whole TLS 1.3 + WebTransport path, as
        // the phone does it, with the certificate pinned by hash and nothing else.
        let m = Movil::arranca().unwrap();
        let puerto = m.puerto;
        let huella = m.huella.clone();
        let secreto = m.secreto.clone();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let conecta = |ruta: String| {
            let huella = huella.clone();
            async move {
                let con_puntos = huella
                    .as_bytes()
                    .chunks(2)
                    .map(|c| std::str::from_utf8(c).unwrap())
                    .collect::<Vec<_>>()
                    .join(":");
                let digest = wtransport::tls::Sha256Digest::from_str_fmt(
                    con_puntos,
                    Sha256DigestFmt::DottedHex,
                )
                .unwrap();
                let config = wtransport::ClientConfig::builder()
                    .with_bind_default()
                    .with_server_certificate_hashes([digest])
                    .build();
                let cliente = Endpoint::client(config).unwrap();
                let url = format!("https://127.0.0.1:{puerto}/{ruta}");
                let c = cliente
                    .connect(
                        wtransport::endpoint::ConnectOptions::builder(&url)
                            .add_header("origin", ORIGEN)
                            .build(),
                    )
                    .await;
                c.map_err(|e| e.to_string())
            }
        };
        let mal = rt.block_on(conecta("no-es".to_owned()));
        assert!(mal.is_err(), "wrong secret must be refused");
        let bien = rt.block_on(conecta(secreto)).unwrap();
        // The stream stays open while the computer reads: dropping it would reset it.
        let _abierto = rt.block_on(async {
            let (mut tx, rx) = bien.open_bi().await.unwrap().await.unwrap();
            let cuerpo = br#"{"t":"texto","texto":"hola desde el movil"}"#;
            let mut cab = [0u8; 5];
            cab[1..].copy_from_slice(&(cuerpo.len() as u32).to_be_bytes());
            tx.write_all(&cab).await.unwrap();
            tx.write_all(cuerpo).await.unwrap();
            (tx, rx)
        });
        let mut m = m;
        let mut visto = Vec::new();
        for _ in 0..100 {
            // The client's endpoint lives on this runtime: give it time to drive the QUIC
            // connection (acks, keep-alives) while the computer's thread reads.
            rt.block_on(async { tokio::time::sleep(Duration::from_millis(50)).await });
            visto.extend(m.recoge());
            if visto
                .iter()
                .any(|d| matches!(d, Desde::Texto(t) if t == "hola desde el movil"))
            {
                break;
            }
        }
        assert!(
            visto.iter().any(|d| matches!(d, Desde::Enganchado)),
            "the phone is seen"
        );
        assert!(visto
            .iter()
            .any(|d| matches!(d, Desde::Texto(t) if t == "hola desde el movil")));
    }

    async fn escribe_ws(t: &mut tokio::net::TcpStream, datos: &[u8]) {
        let m = [1u8, 2, 3, 4];
        let mut c = vec![0x82u8];
        if datos.len() < 126 {
            c.push(0x80 | datos.len() as u8);
        } else {
            c.push(0x80 | 126);
            c.extend_from_slice(&(datos.len() as u16).to_be_bytes());
        }
        c.extend_from_slice(&m);
        c.extend(datos.iter().enumerate().map(|(i, b)| b ^ m[i % 4]));
        t.write_all(&c).await.unwrap();
    }

    /// The next binary message from the relay (pings skipped).
    async fn lee_ws(t: &mut tokio::net::TcpStream) -> Vec<u8> {
        loop {
            let mut h = [0u8; 2];
            t.read_exact(&mut h).await.unwrap();
            let n = match h[1] & 0x7f {
                126 => {
                    let mut b = [0u8; 2];
                    t.read_exact(&mut b).await.unwrap();
                    u16::from_be_bytes(b) as usize
                }
                127 => {
                    let mut b = [0u8; 8];
                    t.read_exact(&mut b).await.unwrap();
                    u64::from_be_bytes(b) as usize
                }
                n => n as usize,
            };
            let mut v = vec![0u8; n];
            t.read_exact(&mut v).await.unwrap();
            if h[0] & 0x0f == 0x2 {
                return v;
            }
        }
    }

    #[test]
    fn a_phone_through_the_relay_proves_the_key_and_everything_travels_sealed() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let listener = rt
            .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
            .unwrap();
        let dir = listener.local_addr().unwrap().to_string();
        rt.spawn(guardiana_rele::sirve(listener));
        let mut m = Movil::arranca_con(&dir).unwrap();
        let d = lee_enlace(&m.enlace("es", "Marta")).unwrap();
        assert_eq!(d["r"], dir.as_str());
        let clave: [u8; 32] = B64URL
            .decode(d["k"].as_str().unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        let sala = sala_de_clave(&clave);
        assert_eq!(sala.len(), 32);
        let (k_em, mut tel) = rt.block_on(async {
            tokio::time::sleep(Duration::from_millis(300)).await;
            let mut t = tokio::net::TcpStream::connect(&dir).await.unwrap();
            t.write_all(
                format!(
                    "GET /m/{sala} HTTP/1.1\r\nHost: rele\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nOrigin: https://guardianagroup.com\r\n\r\n"
                )
                .as_bytes(),
            )
            .await
            .unwrap();
            let mut r = Vec::new();
            while !r.ends_with(b"\r\n\r\n") {
                let mut b = [0u8; 1];
                t.read_exact(&mut b).await.unwrap();
                r.push(b[0]);
            }
            assert!(r.starts_with(b"HTTP/1.1 101"));
            let saludo = lee_ws(&mut t).await;
            assert_eq!(&saludo[..3], b"GM2");
            assert_eq!(saludo.len(), 3 + 16 + 65);
            let del_equipo: [u8; 16] = saludo[3..19].try_into().unwrap();
            // The phone's half of the agreement: its own P-256 key, the shared secret.
            let rng = ring::rand::SystemRandom::new();
            let privada = ring::agreement::EphemeralPrivateKey::generate(
                &ring::agreement::ECDH_P256,
                &rng,
            )
            .unwrap();
            let publica = privada.compute_public_key().unwrap();
            let del_equipo_pub = ring::agreement::UnparsedPublicKey::new(
                &ring::agreement::ECDH_P256,
                saludo[19..].to_vec(),
            );
            let secreto =
                ring::agreement::agree_ephemeral(privada, &del_equipo_pub, |s| s.to_vec())
                    .unwrap();
            let del_movil = [7u8; 16];
            let (k_me, k_em) =
                claves_de_enlace(&clave, &secreto, &del_equipo, &del_movil).unwrap();
            let mut primera = del_movil.to_vec();
            primera.extend_from_slice(publica.as_ref());
            primera.extend(sella(&k_me, 0, b"GM2", b"hola"));
            escribe_ws(&mut t, &primera).await;
            let mut plano = vec![0u8];
            plano.extend_from_slice(br#"{"t":"texto","texto":"hola por el rele"}"#);
            escribe_ws(&mut t, &sella(&k_me, 1, b"", &plano)).await;
            // A frame replayed with the same number is not accepted: it ends the link later,
            // so it is not sent here; a changed byte is refused by the key itself.
            let mut cambiado = sella(&k_me, 2, b"", &plano);
            cambiado[3] ^= 1;
            assert!(abre(&k_me, 2, b"", &cambiado).is_none());
            (k_em, t)
        });
        let mut visto = Vec::new();
        for _ in 0..100 {
            std::thread::sleep(Duration::from_millis(50));
            visto.extend(m.recoge());
            if visto
                .iter()
                .any(|d| matches!(d, Desde::Texto(t) if t == "hola por el rele"))
            {
                break;
            }
        }
        assert!(
            visto.iter().any(|d| matches!(d, Desde::Enganchado)),
            "{visto:?}"
        );
        assert!(
            visto
                .iter()
                .any(|d| matches!(d, Desde::Texto(t) if t == "hola por el rele")),
            "{visto:?}"
        );
        m.manda(Hacia::Estado(r#"{"tipo":"secure_estado"}"#.to_owned()));
        let plano = rt.block_on(async {
            let v = tokio::time::timeout(Duration::from_secs(5), lee_ws(&mut tel))
                .await
                .unwrap();
            abre(&k_em, 0, b"", &v).unwrap()
        });
        assert_eq!(plano[0], 0);
        assert_eq!(&plano[1..], br#"{"tipo":"secure_estado"}"#);
    }
}
