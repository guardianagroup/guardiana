//! PDF: the document information dictionary (`/Author`, `/Creator`, `/Producer`, the dates…)
//! and the XMP packet. The pages' content is never read: it is almost always compressed, and
//! nothing here inflates it.
//!
//! The clean copy blanks each value **in place**, with spaces of the same length, so every
//! offset in the file's cross-reference table stays right and the file opens as before. A PDF
//! may also keep its dictionary inside a compressed object stream, where none of this can see:
//! the report then says that it may carry data that was not looked at, instead of saying clean.

use super::{Clave, Huella, Informe};

const CLAVES: &[(&[u8], Clave)] = &[
    (b"/Author", Clave::Autor),
    (b"/Creator", Clave::Programa),
    (b"/Producer", Clave::Programa),
    (b"/CreationDate", Clave::Fecha),
    (b"/ModDate", Clave::Fecha),
    (b"/Title", Clave::Titulo),
    (b"/Subject", Clave::Descripcion),
    (b"/Keywords", Clave::Descripcion),
    (b"/Company", Clave::Empresa),
    (b"/Manager", Clave::Responsable),
    (b"/SourceModified", Clave::Fecha),
];

fn busca(d: &[u8], aguja: &[u8], desde: usize) -> Option<usize> {
    if aguja.is_empty() || desde >= d.len() {
        return None;
    }
    d.get(desde..)?
        .windows(aguja.len())
        .position(|w| w == aguja)
        .map(|p| p + desde)
}

/// A value right after a key: `(` literal `)` or `<` hex `>`. Returns where its inside is.
fn valor(d: &[u8], mut p: usize) -> Option<(bool, usize, usize)> {
    while d.get(p).is_some_and(|b| b.is_ascii_whitespace()) {
        p += 1;
    }
    match d.get(p)? {
        b'(' => {
            let ini = p + 1;
            let mut q = ini;
            let mut nivel = 1;
            while nivel > 0 {
                match d.get(q)? {
                    b'\\' => q += 1,
                    b'(' => nivel += 1,
                    b')' => nivel -= 1,
                    _ => {}
                }
                q += 1;
            }
            Some((true, ini, q - 1))
        }
        b'<' if d.get(p + 1) != Some(&b'<') => {
            let ini = p + 1;
            let fin = ini + d.get(ini..)?.iter().position(|&b| b == b'>')?;
            Some((false, ini, fin))
        }
        _ => None,
    }
}

fn texto(d: &[u8], literal: bool, ini: usize, fin: usize) -> String {
    let b = d.get(ini..fin).unwrap_or_default();
    let bytes: Vec<u8> = if literal {
        let mut v = Vec::new();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'\\' && i + 1 < b.len() {
                i += 1;
                v.push(match b[i] {
                    b'n' => b'\n',
                    b'r' => b'\r',
                    b't' => b'\t',
                    c => c,
                });
            } else {
                v.push(b[i]);
            }
            i += 1;
        }
        v
    } else {
        let h: Vec<u8> = b.iter().copied().filter(u8::is_ascii_hexdigit).collect();
        h.chunks(2)
            .filter_map(|c| {
                let s = std::str::from_utf8(c).ok()?;
                u8::from_str_radix(&format!("{s:0<2}"), 16).ok()
            })
            .collect()
    };
    let s = if bytes.starts_with(&[0xfe, 0xff]) {
        let u: Vec<u16> = bytes[2..]
            .chunks(2)
            .map(|c| u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)]))
            .collect();
        String::from_utf16_lossy(&u)
    } else {
        bytes.iter().map(|&c| char::from(c)).collect()
    };
    super::legible(&s)
}

/// Every key's value in the file: (key, literal?, inside start, inside end).
fn valores(d: &[u8]) -> Vec<(Clave, bool, usize, usize)> {
    let mut v = Vec::new();
    for (k, clave) in CLAVES {
        let mut p = 0;
        while let Some(i) = busca(d, k, p) {
            p = i + k.len();
            // `/Author` and not `/AuthorX`: the next byte ends the name.
            if d.get(p).is_some_and(|b| b.is_ascii_alphanumeric()) {
                continue;
            }
            if let Some((lit, ini, fin)) = valor(d, p) {
                v.push((*clave, lit, ini, fin));
                p = fin;
            }
        }
    }
    v
}

/// Every XMP packet: where `<x:xmpmeta` starts and `</x:xmpmeta>` ends.
fn paquetes(d: &[u8]) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    let mut p = 0;
    while let Some(i) = busca(d, b"<x:xmpmeta", p) {
        let Some(f) = busca(d, b"</x:xmpmeta>", i) else {
            break;
        };
        v.push((i, f + b"</x:xmpmeta>".len()));
        p = f + 1;
    }
    v
}

fn comprimido(d: &[u8]) -> bool {
    busca(d, b"/ObjStm", 0).is_some()
}

pub(crate) fn mira(d: &[u8]) -> Option<Informe> {
    if !d.starts_with(b"%PDF-") {
        return None;
    }
    let mut inf = Informe::nuevo("pdf");
    for (clave, lit, ini, fin) in valores(d) {
        let t = texto(d, lit, ini, fin);
        if !t.is_empty() {
            inf.anade(vec![Huella::new(clave, t)]);
        }
    }
    for (ini, fin) in paquetes(d) {
        inf.anade(super::xmp_huellas(d.get(ini..fin).unwrap_or_default()));
    }
    if comprimido(d) {
        inf.nota = Some("pdf_comprimido");
    }
    inf.limpiable = true;
    Some(inf)
}

pub(crate) fn limpia(d: &[u8]) -> Option<Vec<u8>> {
    if !d.starts_with(b"%PDF-") {
        return None;
    }
    let mut out = d.to_vec();
    for (_, lit, ini, fin) in valores(d) {
        let zona = out.get_mut(ini..fin)?;
        if lit {
            zona.fill(b' ');
        } else {
            // Hex digits become «20», a space, two by two; what is not a digit stays.
            let mut par = 0;
            for b in zona.iter_mut() {
                if b.is_ascii_hexdigit() {
                    *b = if par % 2 == 0 { b'2' } else { b'0' };
                    par += 1;
                }
            }
        }
    }
    let vacio: &[u8] = b"<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"/>";
    for (ini, fin) in paquetes(d) {
        let zona = out.get_mut(ini..fin)?;
        zona.fill(b' ');
        if zona.len() >= vacio.len() {
            zona[..vacio.len()].copy_from_slice(vacio);
        }
    }
    Some(out)
}
