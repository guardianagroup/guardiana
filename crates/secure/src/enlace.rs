//! One direct connection between two computers.
//!
//! This is not [`crate::puesto`]. There is one TCP connection, a size cap and
//! a timeout. An unspecified address (`0.0.0.0`, `::`) is refused, so this
//! does not listen on every interface by accident. The short-code numbers are
//! not decided on this socket: each side shows [`crate::verificar::Pendiente::numeros`]
//! and calls [`crate::verificar::Pendiente::la_persona_dice`] from its own user.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

use vodozemac::Curve25519PublicKey;

use crate::Error;

const MAX_FRAME: usize = 1024 * 1024;
const WAIT: Duration = Duration::from_secs(15);

/// A listener that will accept one connection, then stop listening.
pub struct Escucha {
    listener: TcpListener,
}

impl Escucha {
    /// Bind `addr`. Unspecified addresses are refused before the socket opens.
    pub fn bind(addr: SocketAddr) -> Result<Self, Error> {
        concreta(addr)?;
        let listener = TcpListener::bind(addr).map_err(|_| Error::CannotStart)?;
        Ok(Self { listener })
    }

    /// The address that was actually bound, including a port of `0`.
    pub fn addr(&self) -> Result<SocketAddr, Error> {
        self.listener.local_addr().map_err(|_| Error::CannotStart)
    }

    /// Accept one connection and drop the listener, so a second client cannot queue on it.
    pub fn uno(self) -> Result<Canal, Error> {
        let (socket, _) = self.listener.accept().map_err(|_| Error::CannotStart)?;
        drop(self.listener);
        Canal::new(socket)
    }
}

/// The one connection.
pub struct Canal {
    socket: TcpStream,
}

impl Canal {
    /// Connect to a specific address. Unspecified addresses are refused.
    pub fn conectar(addr: SocketAddr) -> Result<Self, Error> {
        concreta(addr)?;
        let socket = TcpStream::connect(addr).map_err(|_| Error::CannotStart)?;
        Self::new(socket)
    }

    fn new(socket: TcpStream) -> Result<Self, Error> {
        socket
            .set_read_timeout(Some(WAIT))
            .map_err(|_| Error::CannotStart)?;
        socket
            .set_write_timeout(Some(WAIT))
            .map_err(|_| Error::CannotStart)?;
        Ok(Self { socket })
    }

    /// Exchange ephemeral SAS keys. `primero` writes first (the side that connected).
    pub fn claves(
        &mut self,
        mio: Curve25519PublicKey,
        primero: bool,
    ) -> Result<Curve25519PublicKey, Error> {
        let mine = *mio.as_bytes();
        let bytes = if primero {
            self.socket
                .write_all(&mine)
                .map_err(|_| Error::CannotStart)?;
            read_exact_n(&mut self.socket, 32)?
        } else {
            let theirs = read_exact_n(&mut self.socket, 32)?;
            self.socket
                .write_all(&mine)
                .map_err(|_| Error::CannotStart)?;
            theirs
        };
        Curve25519PublicKey::from_slice(&bytes).map_err(|_| Error::CannotStart)
    }

    /// Exchange the 160-byte proofs. `primero` writes first.
    pub fn pruebas(&mut self, mio: &[u8], primero: bool) -> Result<Vec<u8>, Error> {
        if mio.len() != 160 {
            return Err(Error::CannotStart);
        }
        if primero {
            self.socket.write_all(mio).map_err(|_| Error::CannotStart)?;
            read_exact_n(&mut self.socket, 160)
        } else {
            let theirs = read_exact_n(&mut self.socket, 160)?;
            self.socket.write_all(mio).map_err(|_| Error::CannotStart)?;
            Ok(theirs)
        }
    }

    /// Send one frame. The body is not inspected.
    pub fn enviar(&mut self, frame: &[u8]) -> Result<(), Error> {
        write_frame(&mut self.socket, frame)
    }

    /// Read one frame, at most [`MAX_FRAME`] bytes.
    pub fn recibir(&mut self) -> Result<Vec<u8>, Error> {
        read_frame(&mut self.socket)
    }
}

fn concreta(addr: SocketAddr) -> Result<(), Error> {
    if addr.ip().is_unspecified() {
        Err(Error::CannotStart)
    } else {
        Ok(())
    }
}

fn read_exact_n(socket: &mut TcpStream, len: usize) -> Result<Vec<u8>, Error> {
    let mut buf = vec![0u8; len];
    socket
        .read_exact(&mut buf)
        .map_err(|_| Error::CannotStart)?;
    Ok(buf)
}

fn write_frame(socket: &mut TcpStream, frame: &[u8]) -> Result<(), Error> {
    if frame.is_empty() || frame.len() > MAX_FRAME {
        return Err(Error::CannotStart);
    }
    let len = u32::try_from(frame.len()).map_err(|_| Error::CannotStart)?;
    socket
        .write_all(&len.to_be_bytes())
        .map_err(|_| Error::CannotStart)?;
    socket.write_all(frame).map_err(|_| Error::CannotStart)?;
    Ok(())
}

fn read_frame(socket: &mut TcpStream) -> Result<Vec<u8>, Error> {
    let mut len_buf = [0u8; 4];
    socket
        .read_exact(&mut len_buf)
        .map_err(|_| Error::CannotStart)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len == 0 || len > MAX_FRAME {
        return Err(Error::CannotStart);
    }
    read_exact_n(socket, len)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn an_unspecified_address_is_refused() {
        let v4 = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 9));
        let v6 = SocketAddr::from((Ipv6Addr::UNSPECIFIED, 9));
        assert!(Escucha::bind(v4).is_err());
        assert!(Escucha::bind(v6).is_err());
        assert!(Canal::conectar(v4).is_err());
    }
}
