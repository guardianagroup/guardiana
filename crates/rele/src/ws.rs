//! The phone's side of the relay (16 Oct 2026, «necesito que funcione en iPhone y iPad»):
//! a browser cannot open a raw TCP connection, and Safari does not yet accept the pinned
//! certificate the phone's direct link uses, so the phone's page reaches the relay as a
//! WebSocket (RFC 6455) over the relay's own TLS certificate, and the relay pairs it with the
//! computer that showed the QR code.
//!
//! Only what the relay needs is here: the opening handshake, binary messages, ping, pong and
//! close. The relay never looks inside a message: everything is encrypted end to end between
//! the phone and the computer with a key that only the QR code carries.

use base64::Engine as _;
use tokio::io::{
    AsyncBufReadExt as _, AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _,
};

/// The only page allowed to open a phone's link (the web server never sees the conversation).
pub const ORIGENES: [&str; 2] = [
    "https://guardianagroup.com",
    "https://www.guardianagroup.com",
];
/// Largest message accepted (a piece of a call is ~16 KB; a text ~8 KB; with room to spare).
pub const MAX_MENSAJE: usize = 1024 * 1024 + 1024;
const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// What the opening request asked for.
#[derive(Debug, PartialEq, Eq)]
pub enum Peticion {
    /// A phone's link to room `sala` (32 hex), already answered with 101.
    Movil(String),
    /// Anything else, already answered (a short page for `/`, 404 or 403 otherwise).
    Otra,
}

/// Read the rest of an HTTP request whose first line was `primera`, and answer it.
/// `otro_origen`: one more page allowed (only for tests on this computer).
pub async fn acepta<S>(
    s: &mut S,
    primera: &str,
    otro_origen: Option<&str>,
) -> std::io::Result<Peticion>
where
    S: AsyncRead + AsyncWrite + tokio::io::AsyncBufRead + Unpin,
{
    let mut cabeceras: Vec<(String, String)> = Vec::new();
    let mut total = primera.len();
    loop {
        let mut l = String::new();
        let n = s.read_line(&mut l).await?;
        total += n;
        if n == 0 || total > 8192 || cabeceras.len() > 64 {
            return Ok(Peticion::Otra);
        }
        let l = l.trim_end_matches(['\r', '\n']);
        if l.is_empty() {
            break;
        }
        if let Some((k, v)) = l.split_once(':') {
            cabeceras.push((k.trim().to_ascii_lowercase(), v.trim().to_owned()));
        }
    }
    let cab = |k: &str| {
        cabeceras
            .iter()
            .find(|(x, _)| x == k)
            .map(|(_, v)| v.as_str())
    };
    let mut partes = primera.split_whitespace();
    let (metodo, ruta) = (partes.next().unwrap_or(""), partes.next().unwrap_or(""));
    if metodo != "GET" {
        responde(s, "405 Method Not Allowed", "").await?;
        return Ok(Peticion::Otra);
    }
    if ruta == "/" {
        responde(s, "200 OK", "GUARDIANA SECURE relay\n").await?;
        return Ok(Peticion::Otra);
    }
    let Some(sala) = sala_de_ruta(ruta) else {
        responde(s, "404 Not Found", "").await?;
        return Ok(Peticion::Otra);
    };
    let origen_ok = cab("origin").is_some_and(|o| ORIGENES.contains(&o) || otro_origen == Some(o));
    let es_ws = cab("upgrade").is_some_and(|u| u.eq_ignore_ascii_case("websocket"))
        && cab("sec-websocket-version") == Some("13");
    let (Some(clave), true, true) = (cab("sec-websocket-key"), origen_ok, es_ws) else {
        responde(s, "403 Forbidden", "").await?;
        return Ok(Peticion::Otra);
    };
    let respuesta = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
        aceptacion(clave)
    );
    s.write_all(respuesta.as_bytes()).await?;
    s.flush().await?;
    Ok(Peticion::Movil(sala))
}

async fn responde<S: AsyncWrite + Unpin>(
    s: &mut S,
    estado: &str,
    cuerpo: &str,
) -> std::io::Result<()> {
    let r = format!(
        "HTTP/1.1 {estado}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{cuerpo}",
        cuerpo.len()
    );
    s.write_all(r.as_bytes()).await?;
    s.flush().await
}

/// `/m/<32 hex>` → the room, lower case.
pub fn sala_de_ruta(ruta: &str) -> Option<String> {
    let resto = ruta.strip_prefix("/m/")?;
    if resto.len() != 32 || !resto.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(resto.to_ascii_lowercase())
}

/// `Sec-WebSocket-Accept` for a client's key (RFC 6455 §4.2.2).
pub fn aceptacion(clave: &str) -> String {
    let d = ring::digest::digest(
        &ring::digest::SHA1_FOR_LEGACY_USE_ONLY,
        format!("{clave}{GUID}").as_bytes(),
    );
    base64::engine::general_purpose::STANDARD.encode(d.as_ref())
}

/// One thing the phone sent.
#[derive(Debug, PartialEq, Eq)]
pub enum Entrante {
    /// A whole data message.
    Mensaje(Vec<u8>),
    /// A ping, to be answered with this payload.
    Ping(Vec<u8>),
    /// A pong (ignored).
    Pong,
    /// The phone closed.
    Cierre,
}

/// What goes to the phone.
#[derive(Debug)]
pub enum Saliente {
    /// A binary message.
    Mensaje(Vec<u8>),
    /// A ping (keeps mobile networks from dropping a quiet link).
    Ping,
    /// The answer to a ping.
    Pong(Vec<u8>),
    /// Close.
    Cierre,
}

/// Reads messages from the phone, putting fragments back together.
#[derive(Debug, Default)]
pub struct Lector {
    parcial: Vec<u8>,
    en_curso: bool,
}

impl Lector {
    /// The next message, ping, pong or close. An error ends the link.
    pub async fn lee<R: AsyncRead + Unpin>(&mut self, r: &mut R) -> std::io::Result<Entrante> {
        loop {
            let mut h = [0u8; 2];
            r.read_exact(&mut h).await?;
            let fin = h[0] & 0x80 != 0;
            let op = h[0] & 0x0f;
            if h[0] & 0x70 != 0 || h[1] & 0x80 == 0 {
                // Reserved bits, or a client frame without its mask: not a browser.
                return Err(mal("trama no válida"));
            }
            let len = match h[1] & 0x7f {
                126 => {
                    let mut b = [0u8; 2];
                    r.read_exact(&mut b).await?;
                    u64::from(u16::from_be_bytes(b))
                }
                127 => {
                    let mut b = [0u8; 8];
                    r.read_exact(&mut b).await?;
                    u64::from_be_bytes(b)
                }
                n => u64::from(n),
            };
            let control = op >= 8;
            if (control && (!fin || len > 125))
                || len > MAX_MENSAJE as u64
                || self.parcial.len() as u64 + len > MAX_MENSAJE as u64
            {
                return Err(mal("trama demasiado grande"));
            }
            let mut mascara = [0u8; 4];
            r.read_exact(&mut mascara).await?;
            let mut datos = vec![0u8; len as usize];
            r.read_exact(&mut datos).await?;
            for (i, b) in datos.iter_mut().enumerate() {
                *b ^= mascara[i % 4];
            }
            match op {
                0x8 => return Ok(Entrante::Cierre),
                0x9 => return Ok(Entrante::Ping(datos)),
                0xA => return Ok(Entrante::Pong),
                0x1 | 0x2 if !self.en_curso => {
                    self.parcial = datos;
                    self.en_curso = true;
                }
                0x0 if self.en_curso => self.parcial.extend_from_slice(&datos),
                _ => return Err(mal("orden de tramas no válido")),
            }
            if fin {
                self.en_curso = false;
                return Ok(Entrante::Mensaje(std::mem::take(&mut self.parcial)));
            }
        }
    }
}

/// Write one frame to the phone (server frames are not masked).
pub async fn escribe<W: AsyncWrite + Unpin>(w: &mut W, s: Saliente) -> std::io::Result<()> {
    let (op, datos) = match s {
        Saliente::Mensaje(d) => (0x2u8, d),
        Saliente::Ping => (0x9, Vec::new()),
        Saliente::Pong(d) => (0xA, d),
        Saliente::Cierre => (0x8, Vec::new()),
    };
    let mut c = Vec::with_capacity(10 + datos.len());
    c.push(0x80 | op);
    match datos.len() {
        n if n < 126 => c.push(n as u8),
        n if n <= 0xffff => {
            c.push(126);
            c.extend_from_slice(&(n as u16).to_be_bytes());
        }
        n => {
            c.push(127);
            c.extend_from_slice(&(n as u64).to_be_bytes());
        }
    }
    c.extend_from_slice(&datos);
    w.write_all(&c).await?;
    w.flush().await
}

fn mal(que: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, que.to_owned())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn the_accept_key_is_the_one_in_the_rfc() {
        // RFC 6455 §1.3.
        assert_eq!(
            aceptacion("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }

    #[test]
    fn only_a_room_path_is_a_phone() {
        assert_eq!(
            sala_de_ruta("/m/0123456789ABCDEF0123456789abcdef").as_deref(),
            Some("0123456789abcdef0123456789abcdef")
        );
        assert!(sala_de_ruta("/m/0123").is_none());
        assert!(sala_de_ruta("/x/0123456789abcdef0123456789abcdef").is_none());
        assert!(sala_de_ruta("/m/0123456789abcdef0123456789abcdeg").is_none());
    }

    /// A browser's frame: masked, as clients must.
    pub(crate) fn trama_cliente(op: u8, fin: bool, datos: &[u8]) -> Vec<u8> {
        let mascara = [0x12u8, 0x34, 0x56, 0x78];
        let mut c = vec![if fin { 0x80 | op } else { op }];
        match datos.len() {
            n if n < 126 => c.push(0x80 | n as u8),
            n if n <= 0xffff => {
                c.push(0x80 | 126);
                c.extend_from_slice(&(n as u16).to_be_bytes());
            }
            n => {
                c.push(0x80 | 127);
                c.extend_from_slice(&(n as u64).to_be_bytes());
            }
        }
        c.extend_from_slice(&mascara);
        c.extend(datos.iter().enumerate().map(|(i, b)| b ^ mascara[i % 4]));
        c
    }

    #[tokio::test]
    async fn fragments_pings_and_sizes() {
        let mut v = Vec::new();
        v.extend(trama_cliente(0x2, false, b"hola "));
        v.extend(trama_cliente(0x9, true, b"p"));
        v.extend(trama_cliente(0x0, true, b"mundo"));
        let grande = vec![7u8; 70_000];
        v.extend(trama_cliente(0x2, true, &grande));
        v.extend(trama_cliente(0x8, true, b""));
        let mut r = &v[..];
        let mut l = Lector::default();
        assert_eq!(l.lee(&mut r).await.unwrap(), Entrante::Ping(b"p".to_vec()));
        assert_eq!(
            l.lee(&mut r).await.unwrap(),
            Entrante::Mensaje(b"hola mundo".to_vec())
        );
        assert_eq!(l.lee(&mut r).await.unwrap(), Entrante::Mensaje(grande));
        assert_eq!(l.lee(&mut r).await.unwrap(), Entrante::Cierre);
        // An unmasked frame is refused.
        let mut r: &[u8] = &[0x82, 0x01, 0x00];
        assert!(Lector::default().lee(&mut r).await.is_err());
    }

    #[tokio::test]
    async fn server_frames_have_the_right_length_forms() {
        for n in [0usize, 125, 126, 65_535, 65_536] {
            let mut w = Vec::new();
            escribe(&mut w, Saliente::Mensaje(vec![1; n]))
                .await
                .unwrap();
            let cab = match n {
                n if n < 126 => 2,
                n if n <= 0xffff => 4,
                _ => 10,
            };
            assert_eq!(w.len(), cab + n, "{n}");
            assert_eq!(w[0], 0x82);
        }
    }
}
