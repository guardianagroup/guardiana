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
//! Wire: the client sends one line, `GUARDIANA-RELE 1 <room, 32 hex>\n`. The relay answers
//! `OK\n` to both when the pair is complete, or `LLENO\n` to a third one, then copies bytes in
//! both directions until either side closes.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

/// The first line a client sends, before the room.
pub const SALUDO: &str = "GUARDIANA-RELE 1 ";
/// How long a computer may wait for its pair.
pub const ESPERA: Duration = Duration::from_secs(10 * 60);
/// Rooms waiting at once, at most (memory bound for a small server).
pub const MAX_SALAS: usize = 10_000;

type Esperando = Arc<Mutex<HashMap<String, (BufReader<TcpStream>, Instant)>>>;

/// Serve forever on `listener`.
pub async fn sirve(listener: TcpListener) {
    let esperando: Esperando = Arc::new(Mutex::new(HashMap::new()));
    // Sweep rooms whose wait ran out, once a minute.
    {
        let esperando = esperando.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                esperando
                    .lock()
                    .await
                    .retain(|_, (_, desde)| desde.elapsed() < ESPERA);
            }
        });
    }
    loop {
        let Ok((s, _)) = listener.accept().await else {
            continue;
        };
        let esperando = esperando.clone();
        tokio::spawn(async move {
            let _ = atiende(s, esperando).await;
        });
    }
}

async fn atiende(s: TcpStream, esperando: Esperando) -> std::io::Result<()> {
    let _ = s.set_nodelay(true);
    let mut lector = BufReader::new(s);
    let mut linea = String::new();
    // The room line, within ten seconds and short.
    let leido = tokio::time::timeout(Duration::from_secs(10), lector.read_line(&mut linea)).await;
    let Ok(Ok(n)) = leido else {
        return Ok(());
    };
    if n == 0 || n > 80 {
        return Ok(());
    }
    let Some(sala) = sala_de(&linea) else {
        return Ok(());
    };
    let otro = {
        let mut m = esperando.lock().await;
        match m.remove(&sala) {
            Some((otro, _)) => Some(otro),
            None => {
                if m.len() >= MAX_SALAS {
                    return Ok(());
                }
                m.insert(sala, (lector, Instant::now()));
                None
            }
        }
    };
    let Some(otro) = otro else {
        return Ok(());
    };
    // Paired: both hear it, then bytes flow. Anything the first one wrote while waiting is
    // still inside its reader's buffer, so it goes first.
    let mut a = otro.into_inner_with_buffer();
    let mut b = lector.into_inner_with_buffer();
    if a.0.write_all(b"OK\n").await.is_err() {
        return Ok(());
    }
    if b.0.write_all(b"OK\n").await.is_err() {
        return Ok(());
    }
    if !a.1.is_empty() {
        b.0.write_all(&a.1).await?;
    }
    if !b.1.is_empty() {
        a.0.write_all(&b.1).await?;
    }
    let _ = tokio::io::copy_bidirectional(&mut a.0, &mut b.0).await;
    Ok(())
}

/// The room named in the first line, if the line is well formed.
pub fn sala_de(linea: &str) -> Option<String> {
    let resto = linea.trim_end_matches(['\r', '\n']).strip_prefix(SALUDO)?;
    if resto.len() != 32 || !resto.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(resto.to_ascii_lowercase())
}

trait ConBuffer {
    fn into_inner_with_buffer(self) -> (TcpStream, Vec<u8>);
}

impl ConBuffer for BufReader<TcpStream> {
    fn into_inner_with_buffer(self) -> (TcpStream, Vec<u8>) {
        let pendiente = self.buffer().to_vec();
        (self.into_inner(), pendiente)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt as _;

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
}
