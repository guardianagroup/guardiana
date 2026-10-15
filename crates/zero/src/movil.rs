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
//! Addresses are used to connect and are written nowhere (decision 203).

use std::net::{Ipv4Addr, UdpSocket};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use base64::Engine as _;
use serde_json::{json, Value};
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
    /// One recorded piece from the phone's microphone or camera.
    Media(Vec<u8>),
    /// The phone's page was brought to the front.
    Visto,
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
                rt.block_on(servidor(config, secreto_hilo, tx_desde, rx_hacia));
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            puerto,
            huella,
            secreto,
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
    tx: Sender<Desde>,
    mut rx: tmpsc::UnboundedReceiver<Hacia>,
) {
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
    let mut manda: Option<wtransport::SendStream> = None;
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
                    manda = Some(m);
                    conexion = Some(c);
                    let _ = tx.send(Desde::Enganchado);
                }
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
                };
                if let Some(m) = &mut manda {
                    if escribe(m, tipo, &cuerpo).await.is_err() {
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
        "texto" => Some(Desde::Texto(
            v["texto"].as_str()?.chars().take(8000).collect(),
        )),
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
        "visto" => Some(Desde::Visto),
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
            rt.block_on(tokio::time::sleep(Duration::from_millis(50)));
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
}
