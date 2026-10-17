//! GUARDIANA SECURE's blind relay (15 Oct 2026, after the first real test: the two offices in
//! Cartagena and Prague let nothing in, so no door could be opened on either side).
//!
//! It does one thing: two computers that could not reach each other both connect *out* to it,
//! each saying the same room name, and from then on it passes bytes from one to the other. It
//! cannot read them: everything was encrypted end to end before leaving the computer (Olm for
//! what is said, the media layer for calls; the short-code check happens between the two
//! computers, so the relay cannot sit in the middle unnoticed either). The room name is a hash
//! of the invitation code; the relay never sees the code. It keeps nothing: no file, no log of
//! who came, no addresses (decision 203 applies to the relay too). Memory holds only the
//! connections waiting for their pair, for ten minutes at most.
//!
//! Wire: the client sends one line, `GUARDIANA-RELE 1 <room, 32 hex>\n`, plain or, since
//! 16 Oct 2026, inside TLS with the relay's certificate (some networks pass port 443 only when it
//! looks like a web page). The relay answers
//! `OK\n` to both when the pair is complete, or `LLENO\n` to a third one, then copies bytes in
//! both directions until either side closes.
//!
//! The phone (16 Oct 2026, «necesito que funcione en iPhone y iPad también»): a computer that
//! hands a conversation to a phone sends `GUARDIANA-RELE-M 1 <room>\n` and waits; the phone's
//! page opens `wss://rele.guardianagroup.com/m/<room>` (the relay's own certificate, see
//! `tls`; plain `ws://` is only for tests). When both are in, the computer hears `OK\n` and
//! from then on each message travels as four bytes of length and the bytes on the computer's
//! side, and as one binary WebSocket message on the phone's side. The relay cannot read them:
//! they are sealed with a key that only the QR code carries.
//!
//! Calls over WebRTC (17 Oct 2026): a computer or phone that is about to place a call in a room
//! sends `GUARDIANA-TURN 1 <room, 32 hex>\n` (plain or inside TLS, like the other lines) on a
//! new connection. The relay answers one line and closes: `OK <usuario> <clave>\n` when this
//! server runs a TURN service and the room is known right now (someone waits in it, or its pair
//! is bridged), `NO\n` otherwise. The pair follows coturn's TURN REST API (`use-auth-secret`):
//! `usuario` is `<unix expiry>:g` twelve hours ahead and `clave` is the standard base64 of
//! HMAC-SHA1(secret, usuario), so the TURN server checks it with the shared secret alone and
//! keeps no list of users. Both sides then relay through TURN only, so neither ever learns the
//! other's address; the media is still encrypted end to end (DTLS-SRTP) and the TURN server
//! cannot read it either. The relay remembers which rooms are bridged only as a count per room
//! while the bridge lasts; nothing about who asked is kept.

pub mod tls;
pub mod ws;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use tokio::io::{
    AsyncBufReadExt as _, AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _, BufReader,
};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

/// The first line a computer sends to meet another computer, before the room.
pub const SALUDO: &str = "GUARDIANA-RELE 1 ";
/// The first line a computer sends to meet its owner's phone, before the room.
pub const SALUDO_MOVIL: &str = "GUARDIANA-RELE-M 1 ";
/// The first line that asks for TURN credentials for a call in a room, before the room.
pub const SALUDO_TURN: &str = "GUARDIANA-TURN 1 ";
/// How long TURN credentials stay valid.
pub const VIGENCIA_TURN: Duration = Duration::from_secs(12 * 60 * 60);
/// Where the server keeps the secret it shares with its TURN service (`turn.sh`).
pub const SECRETO_TURN: &str = "/etc/guardiana-rele/turn.secret";
/// Shortest secret accepted; anything shorter leaves TURN off.
pub const MIN_SECRETO_TURN: usize = 16;
/// How long a computer may wait for its pair.
pub const ESPERA: Duration = Duration::from_secs(10 * 60);
/// How long a phone may wait for its computer (it is usually already there).
pub const ESPERA_MOVIL: Duration = Duration::from_secs(60);
/// Rooms waiting at once, at most (memory bound for a small server).
pub const MAX_SALAS: usize = 10_000;

/// A connection, plain or inside TLS.
pub trait Flujo: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Flujo for T {}
type Conexion = BufReader<Box<dyn Flujo>>;

/// Who waits in a phone's room.
enum Esperante {
    Equipo(Conexion, Instant),
    Movil(Conexion, Instant),
}

#[derive(Clone)]
struct Estado {
    esperando: Arc<Mutex<HashMap<String, (Conexion, Instant)>>>,
    moviles: Arc<Mutex<HashMap<String, Esperante>>>,
    /// Rooms bridged right now, with how many bridges each has. A plain (not async) mutex: it
    /// is never held across an await, and the guard that leaves a room does so in `Drop`.
    activas: Arc<std::sync::Mutex<HashMap<String, usize>>>,
    tls: Option<tokio_rustls::TlsAcceptor>,
    /// The secret shared with this server's TURN service; `None`: no TURN here.
    turn: Option<Arc<[u8]>>,
    /// One more page allowed to open a phone's link (`GUARDIANA_RELE_ORIGEN`, tests only).
    otro_origen: Option<Arc<str>>,
}

/// While alive, its room counts as bridged; dropping it (the bridge ended, or failed) takes the
/// count back down and forgets the room at zero.
struct EnPuente {
    activas: Arc<std::sync::Mutex<HashMap<String, usize>>>,
    sala: String,
}

impl EnPuente {
    /// Count one more bridge in `sala`; `None` when the table is full (the bridge still runs,
    /// the room just cannot ask for TURN).
    fn entra(activas: &Arc<std::sync::Mutex<HashMap<String, usize>>>, sala: &str) -> Option<Self> {
        let mut m = bloquea(activas);
        if !m.contains_key(sala) && m.len() >= MAX_SALAS {
            return None;
        }
        *m.entry(sala.to_owned()).or_insert(0) += 1;
        Some(EnPuente {
            activas: Arc::clone(activas),
            sala: sala.to_owned(),
        })
    }
}

impl Drop for EnPuente {
    fn drop(&mut self) {
        let mut m = bloquea(&self.activas);
        let vacia = match m.get_mut(&self.sala) {
            Some(n) => {
                *n = n.saturating_sub(1);
                *n == 0
            }
            None => false,
        };
        if vacia {
            m.remove(&self.sala);
        }
    }
}

/// A plain mutex's guard, even if a thread panicked while holding it (the table stays usable).
fn bloquea<T>(m: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Serve forever on `listener`, computers only (no certificate: no phones).
pub async fn sirve(listener: TcpListener) {
    sirve_con(listener, None).await;
}

/// Serve forever on `listener`; with a certificate, phones too. No TURN.
pub async fn sirve_con(listener: TcpListener, tls: Option<Arc<rustls::ServerConfig>>) {
    sirve_completo(listener, tls, None).await;
}

/// Serve forever on `listener`; with a certificate, phones too; with the TURN secret (see
/// `secreto_turn`), credentials for calls in known rooms.
pub async fn sirve_completo(
    listener: TcpListener,
    tls: Option<Arc<rustls::ServerConfig>>,
    turn: Option<Vec<u8>>,
) {
    let estado = Estado {
        esperando: Arc::new(Mutex::new(HashMap::new())),
        moviles: Arc::new(Mutex::new(HashMap::new())),
        activas: Arc::new(std::sync::Mutex::new(HashMap::new())),
        tls: tls.map(tokio_rustls::TlsAcceptor::from),
        turn: turn.map(Arc::from),
        otro_origen: std::env::var("GUARDIANA_RELE_ORIGEN").ok().map(Arc::from),
    };
    // Sweep rooms whose wait ran out, once a minute.
    {
        let estado = estado.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                estado
                    .esperando
                    .lock()
                    .await
                    .retain(|_, (_, desde)| desde.elapsed() < ESPERA);
                estado.moviles.lock().await.retain(|_, e| match e {
                    Esperante::Equipo(_, desde) => desde.elapsed() < ESPERA,
                    Esperante::Movil(_, desde) => desde.elapsed() < ESPERA_MOVIL,
                });
            }
        });
    }
    loop {
        let Ok((s, _)) = listener.accept().await else {
            continue;
        };
        let estado = estado.clone();
        tokio::spawn(async move {
            let _ = atiende(s, estado).await;
        });
    }
}

async fn atiende(s: TcpStream, estado: Estado) -> std::io::Result<()> {
    let _ = s.set_nodelay(true);
    // TLS starts with a handshake record (22); everything else is a line of text.
    let mut primero = [0u8; 1];
    let Ok(Ok(1)) = tokio::time::timeout(Duration::from_secs(10), s.peek(&mut primero)).await
    else {
        return Ok(());
    };
    let flujo: Box<dyn Flujo> = if primero[0] == 22 {
        let Some(acc) = &estado.tls else {
            return Ok(());
        };
        let Ok(Ok(t)) = tokio::time::timeout(Duration::from_secs(10), acc.accept(s)).await else {
            return Ok(());
        };
        Box::new(t)
    } else {
        Box::new(s)
    };
    atiende_flujo(BufReader::new(flujo), estado).await
}

async fn atiende_flujo(mut lector: Conexion, estado: Estado) -> std::io::Result<()> {
    let mut linea = String::new();
    // The first line, within ten seconds and short.
    let leido = tokio::time::timeout(
        Duration::from_secs(10),
        (&mut lector).take(2048).read_line(&mut linea),
    )
    .await;
    let Ok(Ok(n)) = leido else {
        return Ok(());
    };
    if n == 0 {
        return Ok(());
    }
    if linea.starts_with("GET ") {
        let peticion = tokio::time::timeout(
            Duration::from_secs(10),
            ws::acepta(&mut lector, &linea, estado.otro_origen.as_deref()),
        )
        .await;
        if let Ok(Ok(ws::Peticion::Movil(sala))) = peticion {
            llega_movil(sala, Esperante::Movil(lector, Instant::now()), &estado).await;
        }
        return Ok(());
    }
    if n > 80 {
        return Ok(());
    }
    if let Some(sala) = sala_turn_de(&linea) {
        let respuesta = respuesta_turn(&sala, &estado).await;
        lector.write_all(respuesta.as_bytes()).await?;
        lector.flush().await?;
        let _ = lector.shutdown().await;
        return Ok(());
    }
    if let Some(sala) = sala_movil_de(&linea) {
        llega_movil(sala, Esperante::Equipo(lector, Instant::now()), &estado).await;
        return Ok(());
    }
    let Some(sala) = sala_de(&linea) else {
        return Ok(());
    };
    // The room counts as bridged from the moment it leaves the waiting table (inside the same
    // lock, so a TURN request never finds it in neither) until the copy ends, however it ends:
    // the guard is dropped on every return below.
    let (otro, _activa) = {
        let mut m = estado.esperando.lock().await;
        match m.remove(&sala) {
            Some((otro, _)) => (otro, EnPuente::entra(&estado.activas, &sala)),
            None => {
                if m.len() >= MAX_SALAS {
                    return Ok(());
                }
                m.insert(sala, (lector, Instant::now()));
                return Ok(());
            }
        }
    };
    // Paired: both hear it, then bytes flow. Anything the first one wrote while waiting is
    // still inside its reader's buffer, so it goes first.
    let (mut a, pa) = con_buffer(otro);
    let (mut b, pb) = con_buffer(lector);
    a.write_all(b"OK\n").await?;
    b.write_all(b"OK\n").await?;
    if !pa.is_empty() {
        b.write_all(&pa).await?;
    }
    if !pb.is_empty() {
        a.write_all(&pb).await?;
    }
    let _ = tokio::io::copy_bidirectional(&mut a, &mut b).await;
    Ok(())
}

/// A computer or a phone arrives in a phone's room: pair them, or wait (the newest of the same
/// kind replaces the older: that is how a phone that changed network comes back).
async fn llega_movil(sala: String, llega: Esperante, estado: &Estado) {
    let par = {
        let mut m = estado.moviles.lock().await;
        let previo = m.remove(&sala);
        match (llega, previo) {
            (Esperante::Equipo(e, _), Some(Esperante::Movil(t, _)))
            | (Esperante::Movil(t, _), Some(Esperante::Equipo(e, _))) => {
                // Counted as bridged inside the same lock, so a TURN request never finds the
                // room in neither table.
                Some((EnPuente::entra(&estado.activas, &sala), e, t))
            }
            (llega, _) => {
                if m.len() < MAX_SALAS {
                    m.insert(sala, llega);
                }
                None
            }
        }
    };
    if let Some((activa, equipo, movil)) = par {
        tokio::spawn(puente(activa, equipo, movil));
    }
}

/// Bytes between a computer (length + bytes) and a phone (WebSocket messages) until either
/// goes away. Nothing is read, nothing is kept; the room counts as bridged while `_activa`
/// lives, that is, until this returns.
async fn puente(_activa: Option<EnPuente>, mut equipo: Conexion, movil: Conexion) {
    if equipo.write_all(b"OK\n").await.is_err() || equipo.flush().await.is_err() {
        return;
    }
    let (mut eq_r, mut eq_w) = tokio::io::split(equipo);
    let (mut mv_r, mut mv_w) = tokio::io::split(movil);
    let (tx, mut rx) = tokio::sync::mpsc::channel::<ws::Saliente>(64);
    // To the phone: messages from the computer, pongs, and a ping every 20 s.
    let mut escritor = tokio::spawn(async move {
        let mut latido = tokio::time::interval(Duration::from_secs(20));
        latido.tick().await;
        loop {
            let s = tokio::select! {
                s = rx.recv() => match s { Some(s) => s, None => break },
                _ = latido.tick() => ws::Saliente::Ping,
            };
            let fin = matches!(s, ws::Saliente::Cierre);
            if ws::escribe(&mut mv_w, s).await.is_err() || fin {
                break;
            }
        }
    });
    // From the computer: length, bytes.
    let tx_eq = tx.clone();
    let mut desde_equipo = tokio::spawn(async move {
        loop {
            let mut n = [0u8; 4];
            if eq_r.read_exact(&mut n).await.is_err() {
                break;
            }
            let n = u32::from_be_bytes(n) as usize;
            if n > ws::MAX_MENSAJE {
                break;
            }
            let mut v = vec![0u8; n];
            if eq_r.read_exact(&mut v).await.is_err() {
                break;
            }
            if tx_eq.send(ws::Saliente::Mensaje(v)).await.is_err() {
                break;
            }
        }
        let _ = tx_eq.send(ws::Saliente::Cierre).await;
    });
    // From the phone: messages, pings.
    let mut desde_movil = tokio::spawn(async move {
        let mut l = ws::Lector::default();
        loop {
            match l.lee(&mut mv_r).await {
                Ok(ws::Entrante::Mensaje(v)) => {
                    let mut c = Vec::with_capacity(4 + v.len());
                    c.extend_from_slice(&(v.len() as u32).to_be_bytes());
                    c.extend_from_slice(&v);
                    if eq_w.write_all(&c).await.is_err() || eq_w.flush().await.is_err() {
                        break;
                    }
                }
                Ok(ws::Entrante::Ping(p)) => {
                    if tx.send(ws::Saliente::Pong(p)).await.is_err() {
                        break;
                    }
                }
                Ok(ws::Entrante::Pong) => {}
                Ok(ws::Entrante::Cierre) | Err(_) => break,
            }
        }
        let _ = tx.send(ws::Saliente::Cierre).await;
    });
    let escritor_acabo = tokio::select! {
        _ = &mut escritor => true,
        _ = &mut desde_equipo => false,
        _ = &mut desde_movil => false,
    };
    // Both readers stop now (each holds half of a connection); the writer gets a moment to
    // send its close frame, then both sides are dropped.
    desde_equipo.abort();
    desde_movil.abort();
    if !escritor_acabo {
        let _ = tokio::time::timeout(Duration::from_secs(2), &mut escritor).await;
    }
    escritor.abort();
}

/// The room named in a computer's first line, if the line is well formed.
pub fn sala_de(linea: &str) -> Option<String> {
    sala_tras(linea, SALUDO)
}

/// The room named in a computer's first line for its phone, if well formed.
pub fn sala_movil_de(linea: &str) -> Option<String> {
    sala_tras(linea, SALUDO_MOVIL)
}

/// The room named in a request for TURN credentials, if well formed.
pub fn sala_turn_de(linea: &str) -> Option<String> {
    sala_tras(linea, SALUDO_TURN)
}

/// coturn's TURN REST API (`use-auth-secret`): the user name is the expiry in Unix seconds,
/// `VIGENCIA_TURN` after `ahora_unix`, followed by `:g`; the password is the standard base64
/// of HMAC-SHA1(`secreto`, user name). Returns (user name, password).
pub fn credenciales_turn(secreto: &[u8], ahora_unix: u64) -> (String, String) {
    let usuario = format!("{}:g", ahora_unix.saturating_add(VIGENCIA_TURN.as_secs()));
    let clave = ring::hmac::Key::new(ring::hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, secreto);
    let firma = ring::hmac::sign(&clave, usuario.as_bytes());
    let clave = base64::engine::general_purpose::STANDARD.encode(firma.as_ref());
    (usuario, clave)
}

/// The TURN secret in `ruta`, trimmed; `None` when the file is missing, unreadable or shorter
/// than `MIN_SECRETO_TURN` bytes (TURN then stays off).
pub fn secreto_turn(ruta: &Path) -> Option<Vec<u8>> {
    let texto = std::fs::read_to_string(ruta).ok()?;
    let texto = texto.trim();
    (texto.len() >= MIN_SECRETO_TURN).then(|| texto.as_bytes().to_vec())
}

/// The one line that answers a TURN request for `sala`.
async fn respuesta_turn(sala: &str, estado: &Estado) -> String {
    let Some(secreto) = &estado.turn else {
        return "NO\n".to_owned();
    };
    let esperando = estado.esperando.lock().await.contains_key(sala);
    let movil = estado.moviles.lock().await.contains_key(sala);
    if !(esperando || movil || esta_activa(&estado.activas, sala)) {
        return "NO\n".to_owned();
    }
    let ahora = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (usuario, clave) = credenciales_turn(secreto, ahora);
    format!("OK {usuario} {clave}\n")
}

/// Whether `sala` is bridged right now (a plain function: the plain mutex's guard must never
/// live across an await).
fn esta_activa(activas: &std::sync::Mutex<HashMap<String, usize>>, sala: &str) -> bool {
    bloquea(activas).contains_key(sala)
}

fn sala_tras(linea: &str, saludo: &str) -> Option<String> {
    let resto = linea.trim_end_matches(['\r', '\n']).strip_prefix(saludo)?;
    if resto.len() != 32 || !resto.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(resto.to_ascii_lowercase())
}

fn con_buffer(c: Conexion) -> (Box<dyn Flujo>, Vec<u8>) {
    let pendiente = c.buffer().to_vec();
    (c.into_inner(), pendiente)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn the_room_line_is_strict() {
        assert!(sala_de("GUARDIANA-RELE 1 0123456789abcdef0123456789abcdef\n").is_some());
        assert!(sala_de("GUARDIANA-RELE 1 0123456789ABCDEF0123456789abcdef\r\n").is_some());
        assert!(sala_de("GUARDIANA-RELE 1 0123\n").is_none());
        assert!(sala_de("HOLA 0123456789abcdef0123456789abcdef\n").is_none());
        assert!(sala_de("GUARDIANA-RELE 1 0123456789abcdef0123456789abcdeg\n").is_none());
    }

    #[tokio::test]
    async fn two_computers_meet_in_a_room_and_a_third_is_refused() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(sirve(listener));
        let sala = "0123456789abcdef0123456789abcdef";
        let mut a = TcpStream::connect(addr).await.unwrap();
        a.write_all(format!("{SALUDO}{sala}\n").as_bytes())
            .await
            .unwrap();
        // Written before the pair exists: must still arrive, after OK.
        a.write_all(b"primero").await.unwrap();
        let mut b = TcpStream::connect(addr).await.unwrap();
        b.write_all(format!("{SALUDO}{sala}\n").as_bytes())
            .await
            .unwrap();
        let mut ok = [0u8; 3];
        a.read_exact(&mut ok).await.unwrap();
        assert_eq!(&ok, b"OK\n");
        b.read_exact(&mut ok).await.unwrap();
        assert_eq!(&ok, b"OK\n");
        let mut buf = [0u8; 7];
        b.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"primero");
        b.write_all(b"segundo").await.unwrap();
        a.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"segundo");
        // The room is gone once paired: a third connection just waits for a new pair.
        let mut c = TcpStream::connect(addr).await.unwrap();
        c.write_all(format!("{SALUDO}{sala}\n").as_bytes())
            .await
            .unwrap();
        let r = tokio::time::timeout(Duration::from_millis(300), c.read_exact(&mut ok)).await;
        assert!(r.is_err(), "nobody answers a lone connection");
    }

    fn trama_movil(op: u8, datos: &[u8]) -> Vec<u8> {
        let m = [9u8, 8, 7, 6];
        let mut c = vec![0x80 | op, 0x80 | datos.len() as u8];
        c.extend_from_slice(&m);
        c.extend(datos.iter().enumerate().map(|(i, b)| b ^ m[i % 4]));
        c
    }

    async fn abre_movil(
        addr: std::net::SocketAddr,
        sala: &str,
        origen: &str,
    ) -> (TcpStream, String) {
        let mut t = TcpStream::connect(addr).await.unwrap();
        t.write_all(
            format!(
                "GET /m/{sala} HTTP/1.1\r\nHost: rele\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nOrigin: {origen}\r\n\r\n"
            )
            .as_bytes(),
        )
        .await
        .unwrap();
        let mut r = Vec::new();
        while !r.ends_with(b"\r\n\r\n") {
            let mut b = [0u8; 1];
            if t.read_exact(&mut b).await.is_err() {
                break;
            }
            r.push(b[0]);
        }
        (t, String::from_utf8_lossy(&r).into_owned())
    }

    #[tokio::test]
    async fn a_computer_and_its_phone_meet_and_messages_cross_both_ways() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(sirve(listener));
        let sala = "00112233445566778899aabbccddeeff";
        let mut pc = TcpStream::connect(addr).await.unwrap();
        pc.write_all(format!("{SALUDO_MOVIL}{sala}\n").as_bytes())
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        // Another page cannot open a phone's link.
        let (_, r) = abre_movil(addr, sala, "https://otro.example").await;
        assert!(r.starts_with("HTTP/1.1 403"), "{r}");
        let (mut tel, r) = abre_movil(addr, sala, "https://guardianagroup.com").await;
        assert!(r.starts_with("HTTP/1.1 101"), "{r}");
        assert!(
            r.contains("Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo="),
            "{r}"
        );
        let mut ok = [0u8; 3];
        pc.read_exact(&mut ok).await.unwrap();
        assert_eq!(&ok, b"OK\n");
        // Computer → phone: length + bytes become one binary message.
        pc.write_all(&[0, 0, 0, 4]).await.unwrap();
        pc.write_all(b"hola").await.unwrap();
        let mut m = [0u8; 6];
        tel.read_exact(&mut m).await.unwrap();
        assert_eq!(&m, &[0x82, 4, b'h', b'o', b'l', b'a']);
        // Phone → computer.
        tel.write_all(&trama_movil(0x2, b"adios")).await.unwrap();
        let mut m = [0u8; 9];
        pc.read_exact(&mut m).await.unwrap();
        assert_eq!(&m, &[0, 0, 0, 5, b'a', b'd', b'i', b'o', b's']);
        // A ping gets its pong.
        tel.write_all(&trama_movil(0x9, b"p")).await.unwrap();
        let mut m = [0u8; 3];
        tel.read_exact(&mut m).await.unwrap();
        assert_eq!(&m, &[0x8A, 1, b'p']);
        // The phone leaves: the computer's link ends too.
        tel.write_all(&trama_movil(0x8, b"")).await.unwrap();
        let mut resto = Vec::new();
        let r = tokio::time::timeout(Duration::from_secs(3), pc.read_to_end(&mut resto)).await;
        assert!(r.is_ok(), "the computer hears the end");
    }

    const SECRETO: &[u8] = b"0123456789abcdef0123456789abcdef";

    #[test]
    fn turn_credentials_follow_the_turn_rest_api() {
        // Expected value computed independently:
        // printf '1700043200:g' | openssl dgst -sha1 -hmac <secret> -binary | base64
        let (usuario, clave) = credenciales_turn(SECRETO, 1_700_000_000);
        assert_eq!(usuario, "1700043200:g");
        assert_eq!(clave, "5mzQMo+m93fgpkhFJdfLGN+Hevc=");
        assert!(sala_turn_de("GUARDIANA-TURN 1 0123456789abcdef0123456789abcdef\n").is_some());
        assert!(sala_turn_de("GUARDIANA-TURN 1 0123\n").is_none());
        assert!(sala_turn_de("GUARDIANA-RELE 1 0123456789abcdef0123456789abcdef\n").is_none());
    }

    #[test]
    fn a_short_or_missing_secret_leaves_turn_off() {
        let dir = std::env::temp_dir().join(format!("guardiana-rele-turn-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("turn.secret");
        assert!(secreto_turn(&ruta).is_none());
        std::fs::write(&ruta, "corto\n").unwrap();
        assert!(secreto_turn(&ruta).is_none());
        std::fs::write(&ruta, "  0123456789abcdef0123456789abcdef\n").unwrap();
        assert_eq!(secreto_turn(&ruta).as_deref(), Some(SECRETO));
        let _ = std::fs::remove_dir_all(&dir);
    }

    async fn pide_turn(addr: std::net::SocketAddr, sala: &str) -> String {
        let mut t = TcpStream::connect(addr).await.unwrap();
        t.write_all(format!("{SALUDO_TURN}{sala}\n").as_bytes())
            .await
            .unwrap();
        let mut r = String::new();
        tokio::time::timeout(Duration::from_secs(3), t.read_to_string(&mut r))
            .await
            .unwrap()
            .unwrap();
        r
    }

    fn ahora() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// The answer must be `OK <usuario> <clave>\n` with a fresh twelve-hour expiry.
    fn comprueba_ok(r: &str) {
        let partes: Vec<&str> = r.strip_suffix('\n').unwrap().split(' ').collect();
        assert_eq!(partes.len(), 3, "{r}");
        assert_eq!(partes[0], "OK", "{r}");
        let caduca: u64 = partes[1].strip_suffix(":g").unwrap().parse().unwrap();
        let esperado = ahora() + VIGENCIA_TURN.as_secs();
        assert!(caduca + 10 >= esperado && caduca <= esperado + 10, "{r}");
        let (usuario, clave) = credenciales_turn(SECRETO, caduca - VIGENCIA_TURN.as_secs());
        assert_eq!(partes[1], usuario);
        assert_eq!(partes[2], clave);
    }

    async fn relevo_con_turn(turn: Option<&[u8]>) -> std::net::SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(sirve_completo(listener, None, turn.map(<[u8]>::to_vec)));
        addr
    }

    #[tokio::test]
    async fn turn_is_only_for_known_rooms() {
        let addr = relevo_con_turn(Some(SECRETO)).await;
        let sala = "aaaaaaaaaaaaaaaabbbbbbbbbbbbbbbb";
        assert_eq!(pide_turn(addr, sala).await, "NO\n");
        let mut a = TcpStream::connect(addr).await.unwrap();
        a.write_all(format!("{SALUDO}{sala}\n").as_bytes())
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        comprueba_ok(&pide_turn(addr, sala).await);
        // Another room is still unknown.
        assert_eq!(
            pide_turn(addr, "ccccccccccccccccdddddddddddddddd").await,
            "NO\n"
        );
    }

    #[tokio::test]
    async fn without_a_secret_there_is_no_turn() {
        let addr = relevo_con_turn(None).await;
        let sala = "aaaaaaaaaaaaaaaabbbbbbbbbbbbbbbb";
        let mut a = TcpStream::connect(addr).await.unwrap();
        a.write_all(format!("{SALUDO}{sala}\n").as_bytes())
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(pide_turn(addr, sala).await, "NO\n");
    }

    #[tokio::test]
    async fn a_bridged_room_stays_known_until_the_bridge_ends() {
        let addr = relevo_con_turn(Some(SECRETO)).await;
        let sala = "0123456789abcdef0123456789abcdef";
        let mut a = TcpStream::connect(addr).await.unwrap();
        a.write_all(format!("{SALUDO}{sala}\n").as_bytes())
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut b = TcpStream::connect(addr).await.unwrap();
        b.write_all(format!("{SALUDO}{sala}\n").as_bytes())
            .await
            .unwrap();
        let mut ok = [0u8; 3];
        a.read_exact(&mut ok).await.unwrap();
        b.read_exact(&mut ok).await.unwrap();
        // No longer waiting, but bridged: still known.
        comprueba_ok(&pide_turn(addr, sala).await);
        drop(a);
        drop(b);
        let mut olvidada = false;
        for _ in 0..40 {
            if pide_turn(addr, sala).await == "NO\n" {
                olvidada = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(olvidada, "the room is forgotten once the bridge ends");
    }

    #[tokio::test]
    async fn a_phone_bridge_counts_as_known_too() {
        let addr = relevo_con_turn(Some(SECRETO)).await;
        let sala = "00112233445566778899aabbccddeeff";
        let mut pc = TcpStream::connect(addr).await.unwrap();
        pc.write_all(format!("{SALUDO_MOVIL}{sala}\n").as_bytes())
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        // The computer waits for its phone: known.
        comprueba_ok(&pide_turn(addr, sala).await);
        let (mut tel, r) = abre_movil(addr, sala, "https://guardianagroup.com").await;
        assert!(r.starts_with("HTTP/1.1 101"), "{r}");
        let mut ok = [0u8; 3];
        pc.read_exact(&mut ok).await.unwrap();
        assert_eq!(&ok, b"OK\n");
        // Bridged: still known.
        comprueba_ok(&pide_turn(addr, sala).await);
        tel.write_all(&trama_movil(0x8, b"")).await.unwrap();
        let mut olvidada = false;
        for _ in 0..60 {
            if pide_turn(addr, sala).await == "NO\n" {
                olvidada = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(olvidada, "the room is forgotten once the bridge ends");
    }

    #[tokio::test]
    async fn the_relay_answers_its_own_address_in_a_browser() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(sirve(listener));
        let mut t = TcpStream::connect(addr).await.unwrap();
        t.write_all(b"GET / HTTP/1.1\r\nHost: rele\r\n\r\n")
            .await
            .unwrap();
        let mut r = Vec::new();
        let _ = tokio::time::timeout(Duration::from_secs(3), t.read_to_end(&mut r)).await;
        let r = String::from_utf8_lossy(&r);
        assert!(
            r.starts_with("HTTP/1.1 200") && r.ends_with("GUARDIANA SECURE relay\n"),
            "{r}"
        );
    }
}
