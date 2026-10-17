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

pub mod tls;
pub mod ws;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{
    AsyncBufReadExt as _, AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _, BufReader,
};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

/// The first line a computer sends to meet another computer, before the room.
pub const SALUDO: &str = "GUARDIANA-RELE 1 ";
/// The first line a computer sends to meet its owner's phone, before the room.
pub const SALUDO_MOVIL: &str = "GUARDIANA-RELE-M 1 ";
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
    tls: Option<tokio_rustls::TlsAcceptor>,
    /// One more page allowed to open a phone's link (`GUARDIANA_RELE_ORIGEN`, tests only).
    otro_origen: Option<Arc<str>>,
}

/// Serve forever on `listener`, computers only (no certificate: no phones).
pub async fn sirve(listener: TcpListener) {
    sirve_con(listener, None).await;
}

/// Serve forever on `listener`; with a certificate, phones too.
pub async fn sirve_con(listener: TcpListener, tls: Option<Arc<rustls::ServerConfig>>) {
    let estado = Estado {
        esperando: Arc::new(Mutex::new(HashMap::new())),
        moviles: Arc::new(Mutex::new(HashMap::new())),
        tls: tls.map(tokio_rustls::TlsAcceptor::from),
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
    if let Some(sala) = sala_movil_de(&linea) {
        llega_movil(sala, Esperante::Equipo(lector, Instant::now()), &estado).await;
        return Ok(());
    }
    let Some(sala) = sala_de(&linea) else {
        return Ok(());
    };
    let otro = {
        let mut m = estado.esperando.lock().await;
        match m.remove(&sala) {
            Some((otro, _)) => otro,
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
        match (llega, m.remove(&sala)) {
            (Esperante::Equipo(e, _), Some(Esperante::Movil(t, _)))
            | (Esperante::Movil(t, _), Some(Esperante::Equipo(e, _))) => Some((e, t)),
            (llega, _) => {
                if m.len() < MAX_SALAS {
                    m.insert(sala, llega);
                }
                None
            }
        }
    };
    if let Some((equipo, movil)) = par {
        tokio::spawn(puente(equipo, movil));
    }
}

/// Bytes between a computer (length + bytes) and a phone (WebSocket messages) until either
/// goes away. Nothing is read, nothing is kept.
async fn puente(mut equipo: Conexion, movil: Conexion) {
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
