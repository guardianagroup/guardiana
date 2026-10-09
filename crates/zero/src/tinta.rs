//! «Tinta»: the person marks their own data (an email, a phone, a document number, a name) and
//! GUARDIANA ZERO watches every request for it, as written and in the forms ad networks pass it
//! around in: lowercased, URL-encoded, and hashed. The advertising industry shares an email as
//! its SHA-256 (Unified ID 2.0, RampID) or its MD5 and SHA-1, precisely so that it «is not the
//! email»; to whoever holds the same email it is. So a hash of a marked email leaving to a third
//! party is the email leaving, and it is cut the same way.
//!
//! The marked values stay in the browser's own data folder and never reach a page: the matching
//! happens here, outside the engine, on what the engine is about to send.

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Kinds of marked data. The kind decides how the value is normalised.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tipo {
    /// An email address.
    Correo,
    /// A phone number.
    Telefono,
    /// An identity document number (cédula, DNI, CPF, passport...).
    Documento,
    /// A full name.
    Nombre,
    /// Anything else the person wants watched (an address, an account...).
    Otro,
    /// A decoy GUARDIANA ZERO made for a mandate: nobody has it, so seeing it leave is proof.
    Senuelo,
}

/// One marked value, as the person wrote it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marcado {
    /// What kind of data it is.
    pub tipo: Tipo,
    /// The value as typed.
    pub valor: String,
}

/// One form of a marked value worth looking for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Forma {
    /// Index of the marked value it comes from.
    pub origen: usize,
    /// What it is, for the receipt: `tal_cual`, `sha256`, `md5`...
    pub como: &'static str,
    /// The bytes to look for, lowercase ASCII where the form is case-insensitive.
    pub aguja: String,
}

/// A marked value found in an outgoing request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Hallazgo {
    /// Index of the marked value.
    pub origen: usize,
    /// The form it travelled in.
    pub como: &'static str,
    /// Where: `direccion` (the address) or `cuerpo` (what is sent).
    pub donde: &'static str,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn solo_digitos(s: &str) -> String {
    s.chars().filter(char::is_ascii_digit).collect()
}

/// The email the way identity graphs normalise it before hashing: lowercase, trimmed, and for
/// Gmail without dots or `+tag` in the local part.
fn correo_normalizado(c: &str) -> Vec<String> {
    let c = c.trim().to_lowercase();
    let mut v = vec![c.clone()];
    if let Some((local, dominio)) = c.split_once('@') {
        if dominio == "gmail.com" || dominio == "googlemail.com" {
            let local = local.split('+').next().unwrap_or(local).replace('.', "");
            let g = format!("{local}@gmail.com");
            if g != c {
                v.push(g);
            }
        }
    }
    v
}

fn codifica_url(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02x}")),
        }
    }
    out
}

/// Every form of the marked values worth looking for. Values shorter than 5 characters are left
/// out: they would match by accident everywhere and cut what should not be cut.
#[must_use]
pub fn formas(marcados: &[Marcado]) -> Vec<Forma> {
    let mut out: Vec<Forma> = Vec::new();
    let mut pon = |origen: usize, como: &'static str, aguja: String| {
        if aguja.len() >= 5 && !out.iter().any(|f| f.aguja == aguja) {
            out.push(Forma {
                origen,
                como,
                aguja,
            });
        }
    };
    for (i, m) in marcados.iter().enumerate() {
        let base: Vec<String> = match m.tipo {
            // The decoy is an email too: it is caught in every form an email travels in.
            Tipo::Correo | Tipo::Senuelo => correo_normalizado(&m.valor),
            Tipo::Telefono => {
                let d = solo_digitos(&m.valor);
                // The number with its country code and without it, the way forms take it.
                let mut v = vec![d.clone()];
                if d.len() > 10 {
                    v.push(d[d.len() - 10..].to_string());
                }
                v
            }
            Tipo::Documento => {
                let d = m
                    .valor
                    .chars()
                    .filter(char::is_ascii_alphanumeric)
                    .collect::<String>();
                vec![d.to_lowercase()]
            }
            Tipo::Nombre | Tipo::Otro => vec![m.valor.trim().to_lowercase()],
        };
        for b in base {
            pon(i, "tal_cual", b.clone());
            let cod = codifica_url(&b);
            if cod != b {
                pon(i, "codificado", cod.clone());
                pon(i, "codificado", cod.replace("%20", "+"));
            }
            if matches!(m.tipo, Tipo::Correo | Tipo::Telefono | Tipo::Senuelo) {
                let sha = Sha256::digest(b.as_bytes());
                pon(i, "sha256", hex(&sha));
                pon(
                    i,
                    "sha256_base64",
                    base64::engine::general_purpose::STANDARD
                        .encode(sha)
                        .to_lowercase(),
                );
                pon(i, "md5", hex(&crate::md5::digest(b.as_bytes())));
                let sha1 =
                    ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, b.as_bytes());
                pon(i, "sha1", hex(sha1.as_ref()));
                pon(
                    i,
                    "base64",
                    base64::engine::general_purpose::STANDARD
                        .encode(b.as_bytes())
                        .trim_end_matches('=')
                        .to_lowercase(),
                );
            }
        }
    }
    out
}

/// Lowercase ASCII view of bytes for case-insensitive search (non-ASCII bytes kept).
fn minusculas(b: &[u8]) -> Vec<u8> {
    b.iter().map(u8::to_ascii_lowercase).collect()
}

fn contiene(pajar: &[u8], aguja: &[u8]) -> bool {
    !aguja.is_empty() && pajar.windows(aguja.len()).any(|w| w == aguja)
}

/// The marked values found in a request: in its address and in what it sends.
#[must_use]
pub fn busca(formas: &[Forma], direccion: &str, cuerpo: &[u8]) -> Vec<Hallazgo> {
    let dir = minusculas(direccion.as_bytes());
    let cuerpo = minusculas(cuerpo);
    let mut out: Vec<Hallazgo> = Vec::new();
    for f in formas {
        let aguja = f.aguja.as_bytes();
        for (donde, pajar) in [("direccion", &dir), ("cuerpo", &cuerpo)] {
            if contiene(pajar, aguja)
                && !out.iter().any(|h| h.origen == f.origen && h.donde == donde)
            {
                out.push(Hallazgo {
                    origen: f.origen,
                    como: f.como,
                    donde,
                });
            }
        }
    }
    out
}

/// A decoy for a mandate: looks like an email, belongs to nobody (the `.invalid` top-level
/// domain is reserved and never resolves), and is unique, so seeing it leave is proof.
#[must_use]
pub fn senuelo(semilla: &[u8]) -> String {
    let h = hex(&Sha256::digest(semilla));
    format!("tinta.{}@guardiana-zero.invalid", &h[..10])
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn m(tipo: Tipo, valor: &str) -> Marcado {
        Marcado {
            tipo,
            valor: valor.into(),
        }
    }

    #[test]
    fn an_email_is_found_as_written_encoded_and_hashed() {
        let f = formas(&[m(Tipo::Correo, "Ana.Perez+tienda@Gmail.com")]);
        // The SHA-256 of the normalised Gmail address, as an identity graph would send it.
        let sha = hex(&Sha256::digest(b"anaperez@gmail.com"));
        let h = busca(&f, &format!("https://px.ads.example/u?id={sha}"), b"");
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].como, "sha256");
        let h = busca(
            &f,
            "https://x.example/c",
            b"email=ana.perez%2btienda%40gmail.com",
        );
        assert_eq!(h[0].como, "codificado");
        assert_eq!(h[0].donde, "cuerpo");
        assert!(busca(&f, "https://x.example/?q=ana", b"nada").is_empty());
    }

    #[test]
    fn a_phone_is_found_with_and_without_its_country_code() {
        let f = formas(&[m(Tipo::Telefono, "+57 300 123 4567")]);
        assert_eq!(busca(&f, "https://t.example/?tel=3001234567", b"").len(), 1);
        assert_eq!(
            busca(&f, "https://t.example/", b"{\"p\":\"573001234567\"}").len(),
            1
        );
    }

    #[test]
    fn short_values_are_never_watched() {
        assert!(formas(&[m(Tipo::Otro, "abc")]).is_empty());
    }

    #[test]
    fn the_md5_is_the_real_md5() {
        assert_eq!(
            hex(&crate::md5::digest(b"")),
            "d41d8cd98f00b204e9800998ecf8427e"
        );
        assert_eq!(
            hex(&crate::md5::digest(
                b"The quick brown fox jumps over the lazy dog"
            )),
            "9e107d9d372bb6826bd81d3542a419d6"
        );
    }

    #[test]
    fn a_decoy_is_unique_and_never_resolves() {
        let a = senuelo(b"uno");
        assert!(a.ends_with("@guardiana-zero.invalid"));
        assert_ne!(a, senuelo(b"dos"));
        let f = formas(&[m(Tipo::Senuelo, &a)]);
        assert_eq!(
            busca(&f, &format!("https://evil.example/?d={a}"), b"").len(),
            1
        );
    }
}
