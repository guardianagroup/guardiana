//! PNG: the text chunks (`tEXt`, `zTXt`, `iTXt`), EXIF (`eXIf`) and the last-modified time
//! (`tIME`). The picture's chunks are copied byte for byte and never decoded.

use super::{exif, inflate, Clave, Huella, Informe};

const FIRMA: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Chunks: (type, the whole chunk with length, type, data and CRC).
fn trozos(d: &[u8]) -> Option<Vec<([u8; 4], &[u8])>> {
    if d.get(..8)? != FIRMA {
        return None;
    }
    let mut v = Vec::new();
    let mut p = 8;
    while p < d.len() {
        let len = usize::try_from(u32::from_be_bytes(d.get(p..p + 4)?.try_into().ok()?)).ok()?;
        let tipo: [u8; 4] = d.get(p + 4..p + 8)?.try_into().ok()?;
        let fin = p.checked_add(12)?.checked_add(len)?;
        v.push((tipo, d.get(p..fin)?));
        p = fin;
        if &tipo == b"IEND" {
            break;
        }
    }
    Some(v)
}

fn datos(t: &[u8]) -> &[u8] {
    t.get(8..t.len().saturating_sub(4)).unwrap_or_default()
}

fn clave_de(k: &str) -> Clave {
    match k.to_ascii_lowercase().as_str() {
        "author" | "artist" => Clave::Autor,
        "copyright" => Clave::Derechos,
        "software" | "source" => Clave::Programa,
        "creation time" | "date" => Clave::Fecha,
        "title" => Clave::Titulo,
        _ => Clave::Descripcion,
    }
}

pub(crate) fn mira(d: &[u8]) -> Option<Informe> {
    let mut inf = Informe::nuevo("png");
    for (tipo, t) in trozos(d)? {
        let c = datos(t);
        match &tipo {
            b"eXIf" => inf.anade(exif::lee(c).huellas),
            b"tIME" => inf.anade(vec![Huella::new(Clave::Fecha, String::new())]),
            b"tEXt" | b"zTXt" | b"iTXt" => {
                let Some(cero) = c.iter().position(|&b| b == 0) else {
                    continue;
                };
                let k = String::from_utf8_lossy(&c[..cero]).into_owned();
                let resto = c.get(cero + 1..).unwrap_or_default();
                let texto = match &tipo {
                    b"tEXt" => Some(resto.to_vec()),
                    b"zTXt" => inflate::inflate_zlib(resto.get(1..).unwrap_or_default()),
                    _ => itxt(resto),
                };
                if k == "XML:com.adobe.xmp" {
                    inf.anade(super::xmp_huellas(&texto.unwrap_or_default()));
                    continue;
                }
                let v = super::legible(&String::from_utf8_lossy(&texto.unwrap_or_default()));
                inf.anade(vec![Huella::new(clave_de(&k), v)]);
            }
            _ => {}
        }
    }
    inf.limpiable = true;
    Some(inf)
}

/// The text of an `iTXt`: flag, method, language, translated keyword, text.
fn itxt(r: &[u8]) -> Option<Vec<u8>> {
    let comprimido = *r.first()? == 1;
    let r = r.get(2..)?;
    let lang = r.iter().position(|&b| b == 0)?;
    let r = r.get(lang + 1..)?;
    let trad = r.iter().position(|&b| b == 0)?;
    let t = r.get(trad + 1..)?;
    if comprimido {
        inflate::inflate_zlib(t)
    } else {
        Some(t.to_vec())
    }
}

pub(crate) fn limpia(d: &[u8]) -> Option<Vec<u8>> {
    let mut out = FIRMA.to_vec();
    for (tipo, t) in trozos(d)? {
        if !matches!(&tipo, b"tEXt" | b"zTXt" | b"iTXt" | b"eXIf" | b"tIME") {
            out.extend_from_slice(t);
        }
    }
    Some(out)
}
