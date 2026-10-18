//! The EXIF block (a small TIFF) that cameras and phones put in a photo: only the tags that say
//! who, where, with what and when. The picture itself is never read.

use super::{Clave, Huella};

/// What one EXIF block says.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct Exif {
    pub(crate) huellas: Vec<Huella>,
    /// Which way up the photo goes (1 to 8), so a clean copy is not shown on its side.
    pub(crate) orientacion: Option<u16>,
}

#[derive(Clone, Copy)]
struct Tiff<'a> {
    d: &'a [u8],
    be: bool,
}

impl<'a> Tiff<'a> {
    fn u16(self, o: usize) -> Option<u16> {
        let b = [*self.d.get(o)?, *self.d.get(o + 1)?];
        Some(if self.be {
            u16::from_be_bytes(b)
        } else {
            u16::from_le_bytes(b)
        })
    }

    fn u32(self, o: usize) -> Option<u32> {
        let b = [
            *self.d.get(o)?,
            *self.d.get(o + 1)?,
            *self.d.get(o + 2)?,
            *self.d.get(o + 3)?,
        ];
        Some(if self.be {
            u32::from_be_bytes(b)
        } else {
            u32::from_le_bytes(b)
        })
    }

    /// The entries of the directory at `o`: (tag, type, count, where the value is).
    fn ifd(self, o: usize) -> Vec<(u16, u16, u32, usize)> {
        let mut v = Vec::new();
        let Some(n) = self.u16(o) else { return v };
        // A real directory has a few dozen entries; a crafted one is cut short.
        for i in 0..usize::from(n.min(512)) {
            let e = o + 2 + i * 12;
            let (Some(tag), Some(tipo), Some(cuenta)) =
                (self.u16(e), self.u16(e + 2), self.u32(e + 4))
            else {
                break;
            };
            let ancho = match tipo {
                1 | 2 | 6 | 7 => 1,
                3 | 8 => 2,
                4 | 9 | 11 => 4,
                5 | 10 | 12 => 8,
                _ => continue,
            };
            let total = usize::try_from(cuenta)
                .unwrap_or(usize::MAX)
                .saturating_mul(ancho);
            let valor = if total <= 4 {
                e + 8
            } else {
                match self.u32(e + 8) {
                    Some(p) => usize::try_from(p).unwrap_or(usize::MAX),
                    None => continue,
                }
            };
            v.push((tag, tipo, cuenta, valor));
        }
        v
    }

    fn texto(self, cuenta: u32, o: usize) -> Option<String> {
        let n = usize::try_from(cuenta).ok()?.min(4096);
        let b = self.d.get(o..o.checked_add(n)?)?;
        let fin = b.iter().position(|&c| c == 0).unwrap_or(b.len());
        let s = super::legible(&String::from_utf8_lossy(&b[..fin]));
        (!s.is_empty()).then_some(s)
    }

    fn racional(self, o: usize) -> Option<f64> {
        let n = self.u32(o)?;
        let d = self.u32(o + 4)?;
        (d != 0).then(|| f64::from(n) / f64::from(d))
    }
}

/// Read an EXIF block that starts at its TIFF header (`II*\0` or `MM\0*`).
pub(crate) fn lee(tiff: &[u8]) -> Exif {
    let mut out = Exif::default();
    let be = match tiff.get(..4) {
        Some(b"MM\0*") => true,
        Some(b"II*\0") => false,
        _ => return out,
    };
    let t = Tiff { d: tiff, be };
    let Some(ifd0) = t.u32(4).and_then(|p| usize::try_from(p).ok()) else {
        return out;
    };
    let mut marca = String::new();
    let mut modelo = String::new();
    let mut exif_ifd = None;
    let mut gps_ifd = None;
    for (tag, tipo, cuenta, v) in t.ifd(ifd0) {
        match (tag, tipo) {
            (0x010f, 2) => marca = t.texto(cuenta, v).unwrap_or_default(),
            (0x0110, 2) => modelo = t.texto(cuenta, v).unwrap_or_default(),
            (0x0131, 2) => anade(&mut out, Clave::Programa, t.texto(cuenta, v)),
            (0x0132, 2) => anade(&mut out, Clave::Fecha, t.texto(cuenta, v)),
            (0x013b, 2) => anade(&mut out, Clave::Autor, t.texto(cuenta, v)),
            (0x8298, 2) => anade(&mut out, Clave::Derechos, t.texto(cuenta, v)),
            (0x010e, 2) => anade(&mut out, Clave::Descripcion, t.texto(cuenta, v)),
            (0x0112, 3) => out.orientacion = t.u16(v).filter(|o| (1..=8).contains(o)),
            (0x8769, 4) => exif_ifd = t.u32(v).and_then(|p| usize::try_from(p).ok()),
            (0x8825, 4) => gps_ifd = t.u32(v).and_then(|p| usize::try_from(p).ok()),
            _ => {}
        }
    }
    let dispositivo = if modelo.to_lowercase().starts_with(&marca.to_lowercase()) {
        modelo
    } else {
        format!("{marca} {modelo}").trim().to_owned()
    };
    if !dispositivo.is_empty() {
        out.huellas
            .push(Huella::new(Clave::Dispositivo, dispositivo));
    }
    if let Some(o) = exif_ifd {
        for (tag, tipo, cuenta, v) in t.ifd(o) {
            match (tag, tipo) {
                (0x9003, 2) => anade(&mut out, Clave::Fecha, t.texto(cuenta, v)),
                (0xa430, 2) => anade(&mut out, Clave::Autor, t.texto(cuenta, v)),
                (0xa431, 2) => anade(&mut out, Clave::Serie, t.texto(cuenta, v)),
                (0xa434, 2) => anade(&mut out, Clave::Dispositivo, t.texto(cuenta, v)),
                _ => {}
            }
        }
    }
    if let Some(o) = gps_ifd {
        let mut lat = None;
        let mut lon = None;
        let mut lat_ref = b'N';
        let mut lon_ref = b'E';
        for (tag, tipo, cuenta, v) in t.ifd(o) {
            match (tag, tipo) {
                (1, 2) => lat_ref = *tiff.get(v).unwrap_or(&b'N'),
                (3, 2) => lon_ref = *tiff.get(v).unwrap_or(&b'E'),
                (2, 5) if cuenta >= 3 => lat = grados(t, v),
                (4, 5) if cuenta >= 3 => lon = grados(t, v),
                _ => {}
            }
        }
        // A GPS section with no position still says the phone was asked where it was.
        let valor = match (lat, lon) {
            (Some(la), Some(lo)) => {
                let la = if lat_ref == b'S' { -la } else { la };
                let lo = if lon_ref == b'W' { -lo } else { lo };
                format!("{la:.5}, {lo:.5}")
            }
            _ => String::new(),
        };
        out.huellas.insert(0, Huella::new(Clave::Gps, valor));
    }
    out
}

fn grados(t: Tiff<'_>, v: usize) -> Option<f64> {
    let g = t.racional(v)?;
    let m = t.racional(v + 8).unwrap_or(0.0);
    let s = t.racional(v + 16).unwrap_or(0.0);
    let x = g + m / 60.0 + s / 3600.0;
    (x.is_finite() && x <= 180.0).then_some(x)
}

fn anade(out: &mut Exif, clave: Clave, valor: Option<String>) {
    if let Some(v) = valor {
        if !out.huellas.iter().any(|h| h.clave == clave && h.valor == v) {
            out.huellas.push(Huella::new(clave, v));
        }
    }
}

/// The smallest EXIF that says only which way up the photo goes: a big-endian TIFF with one
/// directory of one entry (Orientation), and nothing else.
pub(crate) fn solo_orientacion(o: u16) -> Vec<u8> {
    let mut v = b"MM\0*\0\0\0\x08".to_vec();
    v.extend_from_slice(&1u16.to_be_bytes());
    v.extend_from_slice(&0x0112u16.to_be_bytes());
    v.extend_from_slice(&3u16.to_be_bytes());
    v.extend_from_slice(&1u32.to_be_bytes());
    v.extend_from_slice(&o.to_be_bytes());
    v.extend_from_slice(&[0, 0]);
    v.extend_from_slice(&0u32.to_be_bytes());
    v
}

/// An EXIF with an empty directory: valid, and it says nothing.
pub(crate) const VACIO: &[u8] = b"MM\0*\0\0\0\x08\0\0\0\0\0\0";

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn the_orientation_only_block_reads_back_with_nothing_else() {
        let e = lee(&solo_orientacion(6));
        assert_eq!(e.orientacion, Some(6));
        assert!(e.huellas.is_empty());
        assert_eq!(lee(VACIO), Exif::default());
    }

    #[test]
    fn a_truncated_or_crafted_block_never_panics() {
        for n in 0..solo_orientacion(6).len() {
            let _ = lee(&solo_orientacion(6)[..n]);
        }
        let mut raro = b"II*\0\x08\0\0\0\xff\xff".to_vec();
        raro.extend_from_slice(&[0xff; 40]);
        let _ = lee(&raro);
    }
}
