//! HEIC/HEIF (and AVIF, the same container): the `meta` box lists the items, and the EXIF and the
//! XMP are items of their own whose bytes `iloc` places in the file. Only those bytes are read,
//! and the clean copy overwrites them in place with an empty EXIF and an empty XMP of the same
//! size, so every offset in the file stays right and the picture is not touched. Which way up the
//! photo goes lives in `irot`, not in EXIF, and stays.

use super::{exif, Clave, Huella, Informe};

struct Caja {
    tipo: [u8; 4],
    /// Where its content starts and ends in the file.
    ini: usize,
    fin: usize,
}

fn cajas(d: &[u8], desde: usize, hasta: usize) -> Vec<Caja> {
    let mut v = Vec::new();
    let mut p = desde;
    while p + 8 <= hasta {
        let Some(t) = d.get(p..p + 8) else { break };
        let mut len = u64::from(u32::from_be_bytes([t[0], t[1], t[2], t[3]]));
        let tipo = [t[4], t[5], t[6], t[7]];
        let mut ini = p + 8;
        if len == 1 {
            let Some(l) = d.get(p + 8..p + 16) else { break };
            len = u64::from_be_bytes([l[0], l[1], l[2], l[3], l[4], l[5], l[6], l[7]]);
            ini = p + 16;
        } else if len == 0 {
            len = u64::try_from(hasta - p).unwrap_or(0);
        }
        let Some(fin) = usize::try_from(len).ok().and_then(|l| p.checked_add(l)) else {
            break;
        };
        if fin > hasta || fin < ini {
            break;
        }
        v.push(Caja { tipo, ini, fin });
        p = fin;
    }
    v
}

fn be(d: &[u8], o: usize, n: usize) -> Option<u64> {
    let b = d.get(o..o.checked_add(n)?)?;
    Some(b.iter().fold(0u64, |a, &x| (a << 8) | u64::from(x)))
}

/// The EXIF and XMP items: (is EXIF, where its bytes are in the file). `None` when the file is
/// not one or keeps them in a way this does not follow (then it is not offered as cleanable).
fn items(d: &[u8]) -> Option<Vec<(bool, usize, usize)>> {
    let top = cajas(d, 0, d.len());
    let ftyp = top.first().filter(|c| &c.tipo == b"ftyp")?;
    let marca = d.get(ftyp.ini..ftyp.ini + 4)?;
    if !matches!(
        marca,
        b"heic" | b"heix" | b"hevc" | b"heim" | b"heis" | b"mif1" | b"msf1" | b"avif"
    ) {
        return None;
    }
    let meta = top.iter().find(|c| &c.tipo == b"meta")?;
    // `meta` is a full box: version and flags first.
    let hijos = cajas(d, meta.ini + 4, meta.fin);
    let mut tipos: Vec<(u32, bool)> = Vec::new();
    if let Some(iinf) = hijos.iter().find(|c| &c.tipo == b"iinf") {
        let v = *d.get(iinf.ini)?;
        let desde = iinf.ini + 4 + if v == 0 { 2 } else { 4 };
        for infe in cajas(d, desde, iinf.fin) {
            if &infe.tipo != b"infe" {
                continue;
            }
            let ver = *d.get(infe.ini)?;
            if ver < 2 {
                continue;
            }
            let (id, p) = if ver == 2 {
                (u32::try_from(be(d, infe.ini + 4, 2)?).ok()?, infe.ini + 6)
            } else {
                (u32::try_from(be(d, infe.ini + 4, 4)?).ok()?, infe.ini + 8)
            };
            // item_protection_index, then the item type.
            let tipo = d.get(p + 2..p + 6)?;
            if tipo == b"Exif" {
                tipos.push((id, true));
            } else if tipo == b"mime" {
                // name\0 then content_type\0
                let resto = d.get(p + 6..infe.fin)?;
                let nombre = resto.iter().position(|&b| b == 0)?;
                let ct = resto.get(nombre + 1..)?;
                if ct.starts_with(b"application/rdf+xml") {
                    tipos.push((id, false));
                }
            }
        }
    }
    let mut out = Vec::new();
    if tipos.is_empty() {
        return Some(out);
    }
    let iloc = hijos.iter().find(|c| &c.tipo == b"iloc")?;
    let ver = *d.get(iloc.ini)?;
    let p0 = iloc.ini + 4;
    let a = *d.get(p0)?;
    let b = *d.get(p0 + 1)?;
    let (t_off, t_len, t_base) = (
        usize::from(a >> 4),
        usize::from(a & 15),
        usize::from(b >> 4),
    );
    let t_idx = if ver == 1 || ver == 2 {
        usize::from(b & 15)
    } else {
        0
    };
    let (n, mut p) = if ver < 2 {
        (be(d, p0 + 2, 2)?, p0 + 4)
    } else {
        (be(d, p0 + 2, 4)?, p0 + 6)
    };
    let ancho_id = if ver < 2 { 2 } else { 4 };
    for _ in 0..n.min(10_000) {
        let id = u32::try_from(be(d, p, ancho_id)?).ok()?;
        p += ancho_id;
        let mut metodo = 0;
        if ver == 1 || ver == 2 {
            metodo = be(d, p, 2)? & 15;
            p += 2;
        }
        p += 2; // data_reference_index
        let base = be(d, p, t_base)?;
        p += t_base;
        let extensiones = be(d, p, 2)?;
        p += 2;
        let mut partes = Vec::new();
        for _ in 0..extensiones.min(1_000) {
            p += t_idx;
            let off = be(d, p, t_off)?;
            p += t_off;
            let len = be(d, p, t_len)?;
            p += t_len;
            partes.push((off, len));
        }
        if let Some(&(_, es_exif)) = tipos.iter().find(|(i, _)| *i == id) {
            // Only bytes placed directly in the file, in one piece: anything else is not
            // offered as cleanable rather than cleaned halfway.
            if metodo != 0 || partes.len() != 1 {
                return None;
            }
            let (off, len) = partes[0];
            let ini = usize::try_from(base.checked_add(off)?).ok()?;
            let fin = ini.checked_add(usize::try_from(len).ok()?)?;
            if fin > d.len() || len == 0 {
                return None;
            }
            out.push((es_exif, ini, fin));
        }
    }
    Some(out)
}

pub(crate) fn mira(d: &[u8]) -> Option<Informe> {
    let top = cajas(d, 0, d.len());
    if top.first().is_none_or(|c| &c.tipo != b"ftyp") {
        return None;
    }
    let mut inf = Informe::nuevo("heic");
    match items(d) {
        Some(lista) => {
            inf.limpiable = true;
            for (es_exif, ini, fin) in lista {
                let b = d.get(ini..fin).unwrap_or_default();
                if es_exif {
                    // Four bytes say where the TIFF header starts after them.
                    let salto = be(b, 0, 4).and_then(|x| usize::try_from(x).ok());
                    let tiff = salto
                        .and_then(|s| b.get(4usize.checked_add(s)?..))
                        .unwrap_or_default();
                    inf.anade(exif::lee(tiff).huellas);
                } else {
                    inf.anade(super::xmp_huellas(b));
                }
            }
        }
        None => {
            // A HEIF this does not follow: say so instead of saying it is clean.
            items_ilegibles(&mut inf, d)?;
        }
    }
    Some(inf)
}

fn items_ilegibles(inf: &mut Informe, d: &[u8]) -> Option<()> {
    let top = cajas(d, 0, d.len());
    let ftyp = top.first()?;
    let marca = d.get(ftyp.ini..ftyp.ini + 4)?;
    if !matches!(
        marca,
        b"heic" | b"heix" | b"hevc" | b"heim" | b"heis" | b"mif1" | b"msf1" | b"avif"
    ) {
        return None;
    }
    inf.limpiable = false;
    inf.nota = Some("heic_no_seguido");
    inf.huellas
        .push(Huella::new(Clave::Propiedades, String::new()));
    Some(())
}

/// An empty XMP packet; the rest of the item is padding (spaces, which XMP allows).
const XMP_VACIO: &[u8] = b"<?xpacket begin=\"\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?><x:xmpmeta xmlns:x=\"adobe:ns:meta/\"/><?xpacket end=\"w\"?>";

pub(crate) fn limpia(d: &[u8]) -> Option<Vec<u8>> {
    let lista = items(d)?;
    let mut out = d.to_vec();
    for (es_exif, ini, fin) in lista {
        let zona = out.get_mut(ini..fin)?;
        if es_exif {
            zona.fill(0);
            // Offset 0 to the TIFF header, then a TIFF that says nothing.
            if zona.len() >= 4 + exif::VACIO.len() {
                zona[4..4 + exif::VACIO.len()].copy_from_slice(exif::VACIO);
            }
        } else if zona.len() >= XMP_VACIO.len() {
            zona.fill(b' ');
            zona[..XMP_VACIO.len()].copy_from_slice(XMP_VACIO);
        } else {
            zona.fill(b' ');
        }
    }
    Some(out)
}
