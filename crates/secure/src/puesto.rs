//! A loopback post office for opaque frames.
//!
//! Each name holds one frame. A second put is refused before the body is read.
//! The post office has no identity and does not look inside a frame. It is not
//! the DNS service and it is not a server between two countries.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::carry::CarryError;

const MAX_FRAME: usize = 1024 * 1024;
const MAX_NAME: usize = 16;
const WAIT: Duration = Duration::from_secs(15);

struct Slot {
    frame: Option<Vec<u8>>,
    waiter: Option<TcpStream>,
}

/// One running loopback post office. Dropping it stops the thread.
pub struct Puesto {
    addr: SocketAddr,
    seen: Arc<Mutex<Vec<u8>>>,
    join: Option<JoinHandle<()>>,
}

impl Puesto {
    /// Bind `127.0.0.1` on a free port.
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

    /// Where to [`put`] and [`get`].
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Every byte that was stored. Not parsed.
    pub fn seen(&self) -> Vec<u8> {
        match self.seen.lock() {
            Ok(bytes) => bytes.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

impl Drop for Puesto {
    fn drop(&mut self) {
        if let Ok(mut socket) = TcpStream::connect(self.addr) {
            let _ = socket.write_all(&[0]);
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// Store one frame under `name`. A name that already has a frame is refused
/// and the body is not sent.
pub fn put(addr: SocketAddr, name: &str, frame: &[u8]) -> Result<(), CarryError> {
    let mut socket = connect(addr)?;
    socket.write_all(&[1]).map_err(|_| CarryError::Failed)?;
    write_name(&mut socket, name)?;
    let mut answer = [0u8; 1];
    socket
        .read_exact(&mut answer)
        .map_err(|_| CarryError::Failed)?;
    if answer[0] == 0 {
        return Err(CarryError::Rejected);
    }
    if answer[0] != 1 {
        return Err(CarryError::Failed);
    }
    write_frame(&mut socket, frame)?;
    socket
        .read_exact(&mut answer)
        .map_err(|_| CarryError::Failed)?;
    match answer[0] {
        1 => Ok(()),
        0 => Err(CarryError::Rejected),
        _ => Err(CarryError::Failed),
    }
}

/// Read the frame stored under `name`. Waits until a [`put`] stores it.
pub fn get(addr: SocketAddr, name: &str) -> Result<Vec<u8>, CarryError> {
    let mut socket = connect(addr)?;
    socket.write_all(&[2]).map_err(|_| CarryError::Failed)?;
    write_name(&mut socket, name)?;
    let mut answer = [0u8; 1];
    socket
        .read_exact(&mut answer)
        .map_err(|_| CarryError::Failed)?;
    if answer[0] != 1 {
        return Err(CarryError::Failed);
    }
    read_frame(&mut socket)
}

fn connect(addr: SocketAddr) -> Result<TcpStream, CarryError> {
    let socket = TcpStream::connect(addr).map_err(|_| CarryError::Failed)?;
    let _ = socket.set_read_timeout(Some(WAIT));
    let _ = socket.set_write_timeout(Some(WAIT));
    Ok(socket)
}

fn serve(listener: TcpListener, seen: Arc<Mutex<Vec<u8>>>) {
    let mut slots: HashMap<String, Slot> = HashMap::new();
    loop {
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
            1 => {
                let Ok(name) = read_name(&mut socket) else {
                    break;
                };
                let taken = slots
                    .get(&name)
                    .and_then(|slot| slot.frame.as_ref())
                    .is_some();
                if taken {
                    let _ = socket.write_all(&[0]);
                    continue;
                }
                if socket.write_all(&[1]).is_err() {
                    break;
                }
                let Ok(frame) = read_frame(&mut socket) else {
                    break;
                };
                match seen.lock() {
                    Ok(mut bytes) => bytes.extend_from_slice(&frame),
                    Err(poisoned) => poisoned.into_inner().extend_from_slice(&frame),
                }
                let slot = slots.entry(name).or_insert(Slot {
                    frame: None,
                    waiter: None,
                });
                if let Some(mut waiter) = slot.waiter.take() {
                    let _ = deliver(&mut waiter, &frame);
                }
                slot.frame = Some(frame);
                let _ = socket.write_all(&[1]);
            }
            2 => {
                let Ok(name) = read_name(&mut socket) else {
                    break;
                };
                if let Some(frame) = slots.get(&name).and_then(|slot| slot.frame.clone()) {
                    let _ = deliver(&mut socket, &frame);
                } else {
                    slots
                        .entry(name)
                        .or_insert(Slot {
                            frame: None,
                            waiter: None,
                        })
                        .waiter = Some(socket);
                }
            }
            _ => {}
        }
    }
}

fn deliver(socket: &mut TcpStream, frame: &[u8]) -> Result<(), CarryError> {
    socket.write_all(&[1]).map_err(|_| CarryError::Failed)?;
    write_frame(socket, frame)
}

fn write_name(socket: &mut TcpStream, name: &str) -> Result<(), CarryError> {
    if name.is_empty()
        || name.len() > MAX_NAME
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(CarryError::Failed);
    }
    let len = u8::try_from(name.len()).map_err(|_| CarryError::Failed)?;
    socket.write_all(&[len]).map_err(|_| CarryError::Failed)?;
    socket
        .write_all(name.as_bytes())
        .map_err(|_| CarryError::Failed)?;
    Ok(())
}

fn read_name(socket: &mut TcpStream) -> Result<String, CarryError> {
    let mut len_buf = [0u8; 1];
    socket
        .read_exact(&mut len_buf)
        .map_err(|_| CarryError::Failed)?;
    let len = usize::from(len_buf[0]);
    if len == 0 || len > MAX_NAME {
        return Err(CarryError::Failed);
    }
    let mut buf = vec![0u8; len];
    socket
        .read_exact(&mut buf)
        .map_err(|_| CarryError::Failed)?;
    let name = std::str::from_utf8(&buf).map_err(|_| CarryError::Failed)?;
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(CarryError::Failed);
    }
    Ok(name.to_string())
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

    #[test]
    fn a_second_put_of_the_same_name_is_refused() {
        let puesto = Puesto::listen().unwrap();
        put(puesto.addr(), "uno", b"primero").unwrap();
        let held = puesto.seen().len();
        assert_eq!(
            put(puesto.addr(), "uno", b"segundo").unwrap_err(),
            CarryError::Rejected
        );
        assert_eq!(puesto.seen().len(), held);
        assert_eq!(get(puesto.addr(), "uno").unwrap(), b"primero");
    }
}
