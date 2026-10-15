//! Carry one envelope from the side that encrypted it to the side that can open it.
//!
//! The relay binds to loopback and keeps the bytes of exactly one envelope.
//! A second deposit is rejected and the first envelope stays. The relay never
//! builds an [`crate::Identity`]. A call, a video call, and a server outside
//! this machine are not this module.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use vodozemac::Curve25519PublicKey;

use crate::{Envelope, Error};

/// The largest envelope this relay will hold. A first message is far smaller.
const MAX_FRAME: usize = 1024 * 1024;
const WAIT: Duration = Duration::from_secs(5);

/// The relay could not carry the envelope. The text does not say what was inside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarryError {
    /// The socket failed, the frame was not the agreed shape, or the ciphertext did not decode.
    Failed,
    /// This relay already holds its one envelope. The new one was not stored.
    Rejected,
}

impl std::fmt::Display for CarryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            CarryError::Failed => "the envelope could not be carried",
            CarryError::Rejected => "the relay already holds an envelope",
        })
    }
}

impl std::error::Error for CarryError {}

impl From<Error> for CarryError {
    fn from(_: Error) -> Self {
        CarryError::Failed
    }
}

/// One running loopback relay. Dropping it stops the thread.
pub struct Relay {
    addr: SocketAddr,
    seen: Arc<Mutex<Vec<u8>>>,
    join: Option<JoinHandle<()>>,
}

impl Relay {
    /// Bind `127.0.0.1` on a free port and serve one deposit and one collection.
    pub fn listen() -> Result<Self, CarryError> {
        let listener =
            TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|_| CarryError::Failed)?;
        let addr = listener.local_addr().map_err(|_| CarryError::Failed)?;
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_thread = Arc::clone(&seen);
        let join = std::thread::spawn(move || serve(listener, seen_thread));
        Ok(Self {
            addr,
            seen,
            join: Some(join),
        })
    }

    /// Where to [`deposit`] and [`collect`].
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Every byte the relay held, including the sender's public key. Not the plaintext.
    pub fn seen(&self) -> Vec<u8> {
        match self.seen.lock() {
            Ok(bytes) => bytes.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        if let Ok(mut socket) = TcpStream::connect(self.addr) {
            let _ = socket.write_all(&[0]);
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// Give the relay one envelope from `from`. The relay answers `1` if it stored
/// this envelope, or `0` if it already holds one. A rejection leaves the first
/// envelope where it was.
pub fn deposit(
    addr: SocketAddr,
    from: Curve25519PublicKey,
    envelope: &Envelope,
) -> Result<(), CarryError> {
    let mut socket = connect(addr)?;
    socket.write_all(&[1]).map_err(|_| CarryError::Failed)?;
    write_frame(&mut socket, &encode(from, envelope))?;
    let mut answer = [0u8; 1];
    socket
        .read_exact(&mut answer)
        .map_err(|_| CarryError::Failed)?;
    match answer[0] {
        1 => Ok(()),
        0 => Err(CarryError::Rejected),
        _ => Err(CarryError::Failed),
    }
}

/// Take the one envelope the relay is holding.
pub fn collect(addr: SocketAddr) -> Result<(Curve25519PublicKey, Envelope), CarryError> {
    let mut socket = connect(addr)?;
    socket.write_all(&[2]).map_err(|_| CarryError::Failed)?;
    decode(&read_frame(&mut socket)?)
}

fn connect(addr: SocketAddr) -> Result<TcpStream, CarryError> {
    let socket = TcpStream::connect(addr).map_err(|_| CarryError::Failed)?;
    let _ = socket.set_read_timeout(Some(WAIT));
    let _ = socket.set_write_timeout(Some(WAIT));
    Ok(socket)
}

fn serve(listener: TcpListener, seen: Arc<Mutex<Vec<u8>>>) {
    let mut stored: Option<Vec<u8>> = None;
    let mut waiting: Option<TcpStream> = None;
    let mut deposited = false;
    let mut collected = false;
    while !deposited || !collected {
        let Ok((mut socket, _)) = listener.accept() else {
            break;
        };
        let _ = socket.set_read_timeout(Some(WAIT));
        let _ = socket.set_write_timeout(Some(WAIT));
        let mut command = [0u8; 1];
        if socket.read_exact(&mut command).is_err() {
            break;
        }
        match command[0] {
            0 => break,
            1 => match read_frame(&mut socket) {
                Ok(frame) => {
                    if deposited {
                        let _ = socket.write_all(&[0]);
                    } else {
                        match seen.lock() {
                            Ok(mut bytes) => bytes.extend_from_slice(&frame),
                            Err(poisoned) => poisoned.into_inner().extend_from_slice(&frame),
                        }
                        if let Some(mut parked) = waiting.take() {
                            if write_frame(&mut parked, &frame).is_ok() {
                                collected = true;
                            }
                        } else {
                            stored = Some(frame);
                        }
                        deposited = true;
                        let _ = socket.write_all(&[1]);
                    }
                }
                Err(_) => break,
            },
            2 => {
                if let Some(frame) = stored.take() {
                    if write_frame(&mut socket, &frame).is_ok() {
                        collected = true;
                    }
                } else {
                    waiting = Some(socket);
                }
            }
            _ => {}
        }
    }
}

fn encode(from: Curve25519PublicKey, envelope: &Envelope) -> Vec<u8> {
    let (kind, bytes) = envelope.to_wire();
    let mut out = Vec::with_capacity(36 + bytes.len());
    out.extend_from_slice(from.as_bytes());
    let kind = u32::try_from(kind).unwrap_or(u32::MAX);
    out.extend_from_slice(&kind.to_be_bytes());
    out.extend_from_slice(&bytes);
    out
}

fn decode(frame: &[u8]) -> Result<(Curve25519PublicKey, Envelope), CarryError> {
    if frame.len() < 36 {
        return Err(CarryError::Failed);
    }
    let from = Curve25519PublicKey::from_slice(&frame[..32]).map_err(|_| CarryError::Failed)?;
    let kind_bytes: [u8; 4] = frame[32..36].try_into().map_err(|_| CarryError::Failed)?;
    let envelope = Envelope::from_wire(u32::from_be_bytes(kind_bytes) as usize, &frame[36..])?;
    Ok((from, envelope))
}

fn write_frame(socket: &mut TcpStream, frame: &[u8]) -> Result<(), CarryError> {
    if frame.is_empty() || frame.len() > MAX_FRAME {
        return Err(CarryError::Failed);
    }
    let len = u32::try_from(frame.len()).map_err(|_| CarryError::Failed)?;
    socket
        .write_all(&len.to_be_bytes())
        .map_err(|_| CarryError::Failed)?;
    socket.write_all(frame).map_err(|_| CarryError::Failed)?;
    Ok(())
}

fn read_frame(socket: &mut TcpStream) -> Result<Vec<u8>, CarryError> {
    let mut len_buf = [0u8; 4];
    socket
        .read_exact(&mut len_buf)
        .map_err(|_| CarryError::Failed)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len == 0 || len > MAX_FRAME {
        return Err(CarryError::Failed);
    }
    let mut buf = vec![0u8; len];
    socket
        .read_exact(&mut buf)
        .map_err(|_| CarryError::Failed)?;
    Ok(buf)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::Identity;

    #[test]
    fn the_relay_carries_the_envelope_and_not_the_text() {
        let relay = Relay::listen().unwrap();
        let alice = Identity::new();
        let mut bob = Identity::new();
        let one_time = bob.fresh_one_time_key().unwrap();
        let mut alice_side = alice.start(bob.public_key(), one_time).unwrap();
        let envelope = alice_side.send(bob.public_key(), b"hola").unwrap();

        deposit(relay.addr(), alice.public_key(), &envelope).unwrap();
        let (from, carried) = collect(relay.addr()).unwrap();
        let seen = relay.seen();

        assert!(!seen.is_empty());
        assert!(!seen.windows(b"hola".len()).any(|window| window == b"hola"));
        let (_, opened) = bob.receive(from, &carried).unwrap();
        assert_eq!(opened, b"hola");
    }

    #[test]
    fn a_second_deposit_is_rejected_and_the_first_envelope_stays() {
        let relay = Relay::listen().unwrap();
        let alice = Identity::new();
        let mut bob = Identity::new();
        let one_time = bob.fresh_one_time_key().unwrap();
        let mut alice_side = alice.start(bob.public_key(), one_time).unwrap();
        let first = alice_side.send(bob.public_key(), b"hola").unwrap();
        let second = alice_side.send(bob.public_key(), b"otro").unwrap();

        deposit(relay.addr(), alice.public_key(), &first).unwrap();
        let held = relay.seen().len();
        assert_eq!(
            deposit(relay.addr(), alice.public_key(), &second).unwrap_err(),
            CarryError::Rejected
        );
        assert_eq!(relay.seen().len(), held);

        let (from, carried) = collect(relay.addr()).unwrap();
        let (_, opened) = bob.receive(from, &carried).unwrap();
        assert_eq!(opened, b"hola");
    }
}
