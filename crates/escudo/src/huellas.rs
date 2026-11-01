//! Fingerprints of known malicious samples (SHA-256), as a compact sorted list.
//!
//! A list is used only when its licence allows it for GUARDIANA (docs/ESCUDO.md). Not every «open» list does: the
//! terms abuse.ch applies since November 2025 ask for-profit users of MalwareBazaar, URLhaus and ThreatFox for a
//! subscription (only Feodo Tracker, SSL Blacklist and YARAify are CC0), so those are not shipped unless the owner
//! takes those terms on. Lists travel inside each version of the program, or, if the owner so decides, from
//! GUARDIANA's servers.
//!
//! Binary form (`.huellas`): the eight bytes `GDNHUEL1`, the number of fingerprints (u64, little endian), the
//! fingerprints sorted and without repeats (32 bytes each), and the SHA-256 of everything before it. A file that does not
//! add up is refused whole: half a list is never used.

use sha2::{Digest as _, Sha256};

/// The first eight bytes of a fingerprint list.
pub const MAGIA: &[u8; 8] = b"GDNHUEL1";

/// A list of fingerprints, with where it comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Huellas {
    /// Where the list comes from, as each warning shows it (for example «signature-base, Florian Roth (DRL 1.1)»).
    pub fuente: String,
    lista: Vec<[u8; 32]>,
}

/// Why a fingerprint list was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorLista {
    /// It is not a fingerprint list, or it is cut short.
    Formato,
    /// The fingerprints are not sorted and unique.
    Orden,
    /// The final SHA-256 does not match: the file was altered or is broken.
    Integridad,
}

impl std::fmt::Display for ErrorLista {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Formato => "no es una lista de huellas o está cortada",
            Self::Orden => "las huellas no están ordenadas",
            Self::Integridad => "la lista no cuadra con su propia huella",
        })
    }
}

impl std::error::Error for ErrorLista {}

impl Huellas {
    /// An empty list.
    #[must_use]
    pub fn vacia(fuente: &str) -> Self {
        Self {
            fuente: fuente.to_string(),
            lista: Vec::new(),
        }
    }

    /// From text: one SHA-256 (64 hexadecimal digits) at the start of each line, as MalwareBazaar exports them. Empty
    /// lines, lines starting with `#` and anything that is not a fingerprint are skipped; repeats count once.
    #[must_use]
    pub fn desde_texto(fuente: &str, texto: &str) -> Self {
        let mut lista: Vec<[u8; 32]> = texto
            .lines()
            .filter_map(|linea| {
                let linea = linea.trim();
                if linea.is_empty() || linea.starts_with('#') {
                    return None;
                }
                let campo = linea
                    .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
                    .next()
                    .unwrap_or("")
                    .trim_matches('"');
                de_hex32(campo)
            })
            .collect();
        lista.sort_unstable();
        lista.dedup();
        Self {
            fuente: fuente.to_string(),
            lista,
        }
    }

    /// The binary form (see the module notes).
    #[must_use]
    pub fn a_bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(8 + 8 + self.lista.len() * 32 + 32);
        b.extend_from_slice(MAGIA);
        b.extend_from_slice(&(self.lista.len() as u64).to_le_bytes());
        for h in &self.lista {
            b.extend_from_slice(h);
        }
        let resumen = Sha256::digest(&b);
        b.extend_from_slice(&resumen);
        b
    }

    /// Reads the binary form. Anything that does not add up refuses the whole list.
    pub fn desde_bytes(fuente: &str, b: &[u8]) -> Result<Self, ErrorLista> {
        if b.len() < 8 + 8 + 32 || &b[..8] != MAGIA {
            return Err(ErrorLista::Formato);
        }
        let mut n = [0u8; 8];
        n.copy_from_slice(&b[8..16]);
        let n = usize::try_from(u64::from_le_bytes(n)).map_err(|_| ErrorLista::Formato)?;
        let cuerpo = n.checked_mul(32).ok_or(ErrorLista::Formato)?;
        if b.len() != 16 + cuerpo + 32 {
            return Err(ErrorLista::Formato);
        }
        let (datos, resumen) = b.split_at(16 + cuerpo);
        if Sha256::digest(datos).as_slice() != resumen {
            return Err(ErrorLista::Integridad);
        }
        let mut lista = Vec::with_capacity(n);
        for trozo in datos[16..].chunks_exact(32) {
            let mut h = [0u8; 32];
            h.copy_from_slice(trozo);
            if lista.last().is_some_and(|u: &[u8; 32]| *u >= h) {
                return Err(ErrorLista::Orden);
            }
            lista.push(h);
        }
        Ok(Self {
            fuente: fuente.to_string(),
            lista,
        })
    }

    /// Whether this exact fingerprint is on the list.
    #[must_use]
    pub fn contiene(&self, huella: &[u8; 32]) -> bool {
        self.lista.binary_search(huella).is_ok()
    }

    /// How many fingerprints the list has.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lista.len()
    }

    /// Whether the list has none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lista.is_empty()
    }

    /// Adds another list's fingerprints (for example, a newer one over the one that came with the program); repeats
    /// count once and this list keeps its source's name.
    pub fn une(&mut self, otra: &Huellas) {
        self.lista.extend_from_slice(&otra.lista);
        self.lista.sort_unstable();
        self.lista.dedup();
    }
}

/// 64 hexadecimal digits (either case) as 32 bytes; anything else is `None`.
#[must_use]
pub fn de_hex32(s: &str) -> Option<[u8; 32]> {
    let s = s.as_bytes();
    if s.len() != 64 {
        return None;
    }
    let mut h = [0u8; 32];
    for (i, par) in s.chunks_exact(2).enumerate() {
        let alto = valor_hex(par[0])?;
        let bajo = valor_hex(par[1])?;
        h[i] = (alto << 4) | bajo;
    }
    Some(h)
}

fn valor_hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Bytes as lowercase hexadecimal.
#[must_use]
pub fn a_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const A: &str = "0000000000000000000000000000000000000000000000000000000000000001";
    const B: &str = "ABCDEF0000000000000000000000000000000000000000000000000000000002";

    #[test]
    fn texto_como_lo_exporta_malwarebazaar() {
        let t = format!("#####\n# MalwareBazaar recent malware samples (SHA256 hash)\n#####\n{B}\n\n{A}\n{A}\nno es una huella\n\"{A}\",x\n");
        let l = Huellas::desde_texto("prueba", &t);
        assert_eq!(l.len(), 2);
        assert!(l.contiene(&de_hex32(A).unwrap_or([9; 32])));
        assert!(l.contiene(&de_hex32(&B.to_lowercase()).unwrap_or([9; 32])));
        assert!(!l.contiene(&[7u8; 32]));
    }

    #[test]
    fn ida_y_vuelta_en_binario() {
        let l = Huellas::desde_texto("prueba", &format!("{A}\n{B}\n"));
        let b = l.a_bytes();
        assert_eq!(b.len(), 16 + 64 + 32);
        assert_eq!(Huellas::desde_bytes("prueba", &b), Ok(l));
    }

    #[test]
    fn una_lista_tocada_no_vale_entera() {
        let l = Huellas::desde_texto("prueba", &format!("{A}\n{B}\n"));
        let mut b = l.a_bytes();
        b[20] ^= 1;
        assert_eq!(
            Huellas::desde_bytes("prueba", &b),
            Err(ErrorLista::Integridad)
        );
        let b = l.a_bytes();
        assert_eq!(
            Huellas::desde_bytes("prueba", &b[..b.len() - 1]),
            Err(ErrorLista::Formato)
        );
        assert_eq!(
            Huellas::desde_bytes("prueba", b"GDNHUEL0"),
            Err(ErrorLista::Formato)
        );
    }

    #[test]
    fn desordenada_no_vale_aunque_cuadre_su_huella() {
        let mut b = Vec::new();
        b.extend_from_slice(MAGIA);
        b.extend_from_slice(&2u64.to_le_bytes());
        b.extend_from_slice(&[2u8; 32]);
        b.extend_from_slice(&[1u8; 32]);
        let r = Sha256::digest(&b);
        b.extend_from_slice(&r);
        assert_eq!(Huellas::desde_bytes("prueba", &b), Err(ErrorLista::Orden));
    }

    #[test]
    fn unir_listas_sin_repetir() {
        let mut l = Huellas::desde_texto("una", &format!("{A}\n"));
        l.une(&Huellas::desde_texto("otra", &format!("{A}\n{B}\n")));
        assert_eq!(l.len(), 2);
        assert_eq!(l.fuente, "una");
    }

    #[test]
    fn hexadecimal() {
        assert_eq!(de_hex32("zz"), None);
        assert_eq!(de_hex32(&"g".repeat(64)), None);
        assert_eq!(a_hex(&[0, 255, 16]), "00ff10");
    }
}
