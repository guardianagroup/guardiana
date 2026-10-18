//! JPEG: the segments before the picture. EXIF and XMP live in APP1, Photoshop's IPTC in APP13,
//! comments in COM, makers' own notes in the other APPn. The picture's own segments (tables,
//! frame, scans) are copied byte for byte and never decoded.

use super::{exif, Clave, Huella, Informe};

/// Segments: (marker, the segment's bytes including marker and length).
fn segmentos(d: &[u8]) -> Option<Vec<(u8, &[u8])>> {
    if d.get(..2)? != [0xff, 0xd8] {
        return None;
    }
    let mut v = Vec::new();
    let mut p = 2;
    loop {
        // Fill bytes (0xFF repeated) may come before a marker.
        while d.get(p)? == &0xff && d.get(p + 1)? == &0xff {
            p += 1;
        }
        if *d.get(p)? != 0xff {
            return None;
        }
        let m = *d.get(p + 1)?;
        if m == 0xd9 {
            return Some(v);
        }
        if (0xd0..=0xd7).contains(&m) || m == 0x01 {
            v.push((m, d.get(p..p + 2)?));
            p += 2;
            continue;
        }
        let len = usize::from(u16::from_be_bytes([*d.get(p + 2)?, *d.get(p + 3)?]));
        if len < 2 {
            return None;
        }
        let mut fin = p + 2 + len;
        if m == 0xda {
            // The scan: compressed picture data up to the next marker that is not a stuffed
            // byte (FF 00) or a restart (FF D0..D7).
            let mut q = fin;
            loop {
                let b = *d.get(q)?;
                if b == 0xff {
                    let n = *d.get(q + 1)?;
                    if n != 0 && !(0xd0..=0xd7).contains(&n) && n != 0xff {
                        break;
                    }
                }
                q += 1;
            }
            fin = q;
        }
        v.push((m, d.get(p..fin)?));
        p = fin;
    }
}

fn cuerpo(s: &[u8]) -> &[u8] {
    s.get(4..).unwrap_or_default()
}

/// What stays in a clean copy: the picture's own segments, the JFIF header, the colour profile
/// (ICC, APP2) and Adobe's colour flag (APP14), without which some photos change colour.
fn se_queda(m: u8, s: &[u8]) -> bool {
    match m {
        0xe0 => cuerpo(s).starts_with(b"JFIF\0") || cuerpo(s).starts_with(b"JFXX\0"),
        0xe2 => cuerpo(s).starts_with(b"ICC_PROFILE\0"),
        0xee => cuerpo(s).starts_with(b"Adobe"),
        0xe1 | 0xe3..=0xed | 0xef | 0xfe => false,
        _ => true,
    }
}

pub(crate) fn mira(d: &[u8]) -> Option<Informe> {
    let segs = segmentos(d)?;
    let mut inf = Informe::nuevo("jpeg");
    for (m, s) in &segs {
        let c = cuerpo(s);
        match m {
            0xe1 if c.starts_with(b"Exif\0\0") => {
                let e = exif::lee(c.get(6..).unwrap_or_default());
                inf.huellas.extend(e.huellas);
            }
            0xe1 if c.starts_with(b"http://ns.adobe.com/xap/1.0/") => {
                inf.anade(super::xmp_huellas(c));
            }
            0xed => inf.anade(vec![Huella::new(Clave::Propiedades, "IPTC".to_owned())]),
            0xfe => {
                let t = super::legible(&String::from_utf8_lossy(c));
                if !t.is_empty() {
                    inf.anade(vec![Huella::new(Clave::Descripcion, t)]);
                }
            }
            _ => {}
        }
    }
    inf.limpiable = true;
    Some(inf)
}

pub(crate) fn limpia(d: &[u8]) -> Option<Vec<u8>> {
    let segs = segmentos(d)?;
    let orientacion = segs
        .iter()
        .find(|(m, s)| *m == 0xe1 && cuerpo(s).starts_with(b"Exif\0\0"))
        .and_then(|(_, s)| exif::lee(cuerpo(s).get(6..).unwrap_or_default()).orientacion)
        .filter(|o| *o != 1);
    let mut out = vec![0xff, 0xd8];
    let mut puesta = orientacion.is_none();
    for (m, s) in &segs {
        // The orientation goes right after the JFIF header (or first, when there is none).
        if !puesta && *m != 0xe0 {
            let mut app1 = b"Exif\0\0".to_vec();
            app1.extend(exif::solo_orientacion(orientacion.unwrap_or(1)));
            out.extend_from_slice(&[0xff, 0xe1]);
            out.extend_from_slice(&u16::try_from(app1.len() + 2).ok()?.to_be_bytes());
            out.extend(app1);
            puesta = true;
        }
        if se_queda(*m, s) {
            out.extend_from_slice(s);
        }
    }
    // Whatever came after the end of the picture (a second picture of a «portrait» photo, a
    // maker's trailer) is left out: it can carry its own EXIF.
    out.extend_from_slice(&[0xff, 0xd9]);
    Some(out)
}
