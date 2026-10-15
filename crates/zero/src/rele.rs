//! The computer's side of the blind relay (`guardiana-rele`): used only when no door could be
//! opened in the router (offices, hotels, operators with shared addresses). Both computers
//! connect out to the relay with the same room; the relay joins them and passes bytes it
//! cannot read. The invitation code for this path is `GR-XXXX-XXXX-XX`; the room is a hash of
//! it, so the relay never learns the code either.
//!
//! The relay's name is fixed here and can be changed with `GUARDIANA_RELE` (the tests run one
//! on this computer).

use std::io::{Read as _, Write as _};
use std::net::{TcpStream, ToSocketAddrs as _};
use std::time::Duration;

use sha2::{Digest, Sha256};

/// Where the relay lives (443 so it passes any office firewall).
pub const RELE: &str = "rele.guardianagroup.com:443";
/// The same relay by address, for a computer whose DNS cannot resolve the name (15 Oct 2026:
/// the first server, Nuremberg). Tried only after the name.
pub const RELE_IP: &str = "5.75.173.240:443";
/// The letters of a code: no 0/O/1/I, so it reads well aloud.
const ALFABETO: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// The relay's address: the environment's, or the fixed one.
pub fn direccion() -> String {
    std::env::var("GUARDIANA_RELE").unwrap_or_else(|_| RELE.to_owned())
}

/// A fresh invitation code for the relay path: 50 random bits and a check letter.
pub fn codigo_nuevo() -> String {
    let mut azar = [0u8; 8];
    crate::sesion::azar(&mut azar);
    let bits = u64::from_be_bytes(azar);
    let mut letras = Vec::with_capacity(11);
    for i in (0..10).rev() {
        letras.push(ALFABETO[((bits >> (i * 5)) & 31) as usize]);
    }
    let suma: u32 = letras.iter().map(|&b| u32::from(b)).sum();
    letras.push(ALFABETO[(suma % 32) as usize]);
    let s: String = letras.iter().map(|&b| b as char).collect();
    format!("GR-{}-{}-{}", &s[..4], &s[4..8], &s[8..])
}

/// The room a code names, or `None` if the text is not a relay code. Spaces, dashes and case do
/// not matter.
pub fn sala_de(texto: &str) -> Option<String> {
    let limpio: String = texto
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    let cuerpo = limpio.strip_prefix("GR")?;
    if cuerpo.len() != 11 {
        return None;
    }
    let b = cuerpo.as_bytes();
    if !b.iter().all(|x| ALFABETO.contains(x)) {
        return None;
    }
    let suma: u32 = b[..10].iter().map(|&x| u32::from(x)).sum();
    if ALFABETO[(suma % 32) as usize] != b[10] {
        return None;
    }
    let h = Sha256::digest(format!("GUARDIANA SECURE relé {cuerpo}").as_bytes());
    Some(h[..16].iter().map(|x| format!("{x:02x}")).collect())
}

fn abre_hacia(rele: &str) -> Result<TcpStream, String> {
    let addr = rele
        .to_socket_addrs()
        .map_err(|e| e.to_string())?
        .next()
        .ok_or_else(|| "relé sin dirección".to_owned())?;
    TcpStream::connect_timeout(&addr, Duration::from_secs(10)).map_err(|e| e.to_string())
}

/// Connect to the relay, name the room and wait (up to ten minutes) for the other computer.
/// Returns the stream once the relay says the pair is complete.
pub fn conecta(rele: &str, sala: &str) -> Result<TcpStream, String> {
    let mut s = match abre_hacia(rele) {
        Ok(s) => s,
        Err(e) if rele == RELE => abre_hacia(RELE_IP).map_err(|e2| format!("{e}; {e2}"))?,
        Err(e) => return Err(e),
    };
    let _ = s.set_nodelay(true);
    s.write_all(format!("GUARDIANA-RELE 1 {sala}\n").as_bytes())
        .map_err(|e| e.to_string())?;
    s.set_read_timeout(Some(Duration::from_secs(11 * 60)))
        .map_err(|e| e.to_string())?;
    // The answer, byte by byte up to the newline, so nothing that follows it is swallowed.
    let mut linea = Vec::new();
    loop {
        let mut b = [0u8; 1];
        s.read_exact(&mut b).map_err(|e| e.to_string())?;
        if b[0] == b'\n' {
            break;
        }
        linea.push(b[0]);
        if linea.len() > 8 {
            return Err("respuesta del relé ilegible".to_owned());
        }
    }
    if linea != b"OK" {
        return Err("el relé no emparejó".to_owned());
    }
    let _ = s.set_read_timeout(None);
    Ok(s)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_relay_code_names_a_room_and_a_typo_does_not() {
        let c = codigo_nuevo();
        assert!(c.starts_with("GR-"));
        assert_eq!(c.len(), 16);
        let sala = sala_de(&c).unwrap();
        assert_eq!(sala.len(), 32);
        assert_eq!(sala_de(&c.to_lowercase().replace('-', " ")).unwrap(), sala);
        let mut mal = c.clone().into_bytes();
        mal[3] = if mal[3] == b'A' { b'B' } else { b'A' };
        assert!(sala_de(std::str::from_utf8(&mal).unwrap()).is_none());
        assert!(sala_de("GS-ABCD-EFGH-JK").is_none());
        assert_ne!(sala_de(&codigo_nuevo()), Some(sala));
    }
}
