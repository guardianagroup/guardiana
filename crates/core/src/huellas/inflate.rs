//! A small DEFLATE decoder (RFC 1951), in the manner of zlib's own `puff.c`: only for the few
//! kilobytes of metadata a document keeps compressed (an Office file's `docProps/core.xml`, a
//! PNG's compressed text). Never for an image or a document's text: those are not read.
//!
//! The output is capped: a part that inflates beyond [`MAXIMO`] is refused, so a crafted file
//! cannot make the program fill its memory.

/// The most a metadata part may inflate to.
pub(crate) const MAXIMO: usize = 4 * 1024 * 1024;

struct Bits<'a> {
    datos: &'a [u8],
    pos: usize,
    buf: u32,
    cuenta: u32,
}

impl<'a> Bits<'a> {
    fn new(datos: &'a [u8]) -> Self {
        Self {
            datos,
            pos: 0,
            buf: 0,
            cuenta: 0,
        }
    }

    fn bits(&mut self, n: u32) -> Option<u32> {
        while self.cuenta < n {
            let b = *self.datos.get(self.pos)?;
            self.pos += 1;
            self.buf |= u32::from(b) << self.cuenta;
            self.cuenta += 8;
        }
        let v = self.buf & ((1u32 << n) - 1);
        self.buf >>= n;
        self.cuenta -= n;
        Some(v)
    }

    fn alinea(&mut self) {
        self.buf = 0;
        self.cuenta = 0;
    }
}

/// Canonical Huffman table: how many codes of each length, and the symbols in code order.
struct Huffman {
    cuenta: [u16; 16],
    simbolos: Vec<u16>,
}

impl Huffman {
    fn new(longitudes: &[u8]) -> Option<Self> {
        let mut cuenta = [0u16; 16];
        for &l in longitudes {
            *cuenta.get_mut(usize::from(l))? += 1;
        }
        cuenta[0] = 0;
        let mut desde = [0u16; 16];
        for i in 1..16 {
            desde[i] = desde[i - 1] + cuenta[i - 1];
        }
        let mut simbolos = vec![0u16; longitudes.len()];
        for (s, &l) in longitudes.iter().enumerate() {
            if l != 0 {
                let d = desde.get_mut(usize::from(l))?;
                *simbolos.get_mut(usize::from(*d))? = u16::try_from(s).ok()?;
                *d += 1;
            }
        }
        Some(Self { cuenta, simbolos })
    }

    fn decodifica(&self, b: &mut Bits<'_>) -> Option<u16> {
        let mut codigo: i32 = 0;
        let mut primero: i32 = 0;
        let mut indice: i32 = 0;
        for len in 1..16 {
            codigo |= i32::try_from(b.bits(1)?).ok()?;
            let n = i32::from(self.cuenta[len]);
            if codigo - n < primero {
                let i = usize::try_from(indice + (codigo - primero)).ok()?;
                return self.simbolos.get(i).copied();
            }
            indice += n;
            primero += n;
            primero <<= 1;
            codigo <<= 1;
        }
        None
    }
}

const BASE_LARGO: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const EXTRA_LARGO: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const BASE_DIST: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const EXTRA_DIST: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

fn bloque(b: &mut Bits<'_>, sal: &mut Vec<u8>, lit: &Huffman, dist: &Huffman) -> Option<()> {
    loop {
        let s = lit.decodifica(b)?;
        match s {
            0..=255 => {
                sal.push(u8::try_from(s).ok()?);
                if sal.len() > MAXIMO {
                    return None;
                }
            }
            256 => return Some(()),
            _ => {
                let i = usize::from(s - 257);
                let largo = usize::from(*BASE_LARGO.get(i)?)
                    + usize::try_from(b.bits(u32::from(*EXTRA_LARGO.get(i)?))?).ok()?;
                let d = usize::from(dist.decodifica(b)?);
                let distancia = usize::from(*BASE_DIST.get(d)?)
                    + usize::try_from(b.bits(u32::from(*EXTRA_DIST.get(d)?))?).ok()?;
                if distancia > sal.len() || sal.len() + largo > MAXIMO {
                    return None;
                }
                let desde = sal.len() - distancia;
                for k in 0..largo {
                    let c = *sal.get(desde + k)?;
                    sal.push(c);
                }
            }
        }
    }
}

fn fijas() -> Option<(Huffman, Huffman)> {
    let mut l = [0u8; 288];
    for (i, x) in l.iter_mut().enumerate() {
        *x = match i {
            0..=143 => 8,
            144..=255 => 9,
            256..=279 => 7,
            _ => 8,
        };
    }
    Some((Huffman::new(&l)?, Huffman::new(&[5u8; 30])?))
}

fn dinamicas(b: &mut Bits<'_>) -> Option<(Huffman, Huffman)> {
    const ORDEN: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
    let nlen = usize::try_from(b.bits(5)?).ok()? + 257;
    let ndist = usize::try_from(b.bits(5)?).ok()? + 1;
    let ncode = usize::try_from(b.bits(4)?).ok()? + 4;
    if nlen > 286 || ndist > 30 {
        return None;
    }
    let mut l = [0u8; 320];
    for &o in ORDEN.iter().take(ncode) {
        l[o] = u8::try_from(b.bits(3)?).ok()?;
    }
    let codigos = Huffman::new(&l[..19])?;
    let mut l = [0u8; 320];
    let mut i = 0;
    while i < nlen + ndist {
        let s = codigos.decodifica(b)?;
        if s < 16 {
            l[i] = u8::try_from(s).ok()?;
            i += 1;
            continue;
        }
        let (valor, veces) = match s {
            16 => {
                if i == 0 {
                    return None;
                }
                (l[i - 1], 3 + b.bits(2)?)
            }
            17 => (0, 3 + b.bits(3)?),
            _ => (0, 11 + b.bits(7)?),
        };
        for _ in 0..veces {
            *l.get_mut(i)? = valor;
            i += 1;
        }
        if i > nlen + ndist {
            return None;
        }
    }
    if l[256] == 0 {
        return None;
    }
    Some((
        Huffman::new(&l[..nlen])?,
        Huffman::new(&l[nlen..nlen + ndist])?,
    ))
}

/// Inflate raw DEFLATE data. `None` when it is broken or larger than [`MAXIMO`].
pub(crate) fn inflate(datos: &[u8]) -> Option<Vec<u8>> {
    let mut b = Bits::new(datos);
    let mut sal = Vec::new();
    loop {
        let ultimo = b.bits(1)? == 1;
        match b.bits(2)? {
            0 => {
                b.alinea();
                let p = b.pos;
                let len = usize::from(u16::from_le_bytes([*datos.get(p)?, *datos.get(p + 1)?]));
                let nlen = u16::from_le_bytes([*datos.get(p + 2)?, *datos.get(p + 3)?]);
                if usize::from(!nlen) != len || sal.len() + len > MAXIMO {
                    return None;
                }
                sal.extend_from_slice(datos.get(p + 4..p + 4 + len)?);
                b.pos = p + 4 + len;
            }
            1 => {
                let (lit, dist) = fijas()?;
                bloque(&mut b, &mut sal, &lit, &dist)?;
            }
            2 => {
                let (lit, dist) = dinamicas(&mut b)?;
                bloque(&mut b, &mut sal, &lit, &dist)?;
            }
            _ => return None,
        }
        if ultimo {
            return Some(sal);
        }
    }
}

/// Inflate zlib-wrapped data (RFC 1950), as PNG keeps its compressed text. The checksum at the
/// end is not checked: a wrong one only means the text shown may be wrong, and nothing is
/// decided on it.
pub(crate) fn inflate_zlib(datos: &[u8]) -> Option<Vec<u8>> {
    let cmf = *datos.first()?;
    let flg = *datos.get(1)?;
    if cmf & 0x0f != 8 || (u16::from(cmf) * 256 + u16::from(flg)) % 31 != 0 || flg & 0x20 != 0 {
        return None;
    }
    inflate(datos.get(2..)?)
}

/// CRC-32 (IEEE), as ZIP and PNG use it.
pub(crate) fn crc32(datos: &[u8]) -> u32 {
    let mut c: u32 = 0xffff_ffff;
    for &b in datos {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ 0xedb8_8320
            } else {
                c >> 1
            };
        }
    }
    !c
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn crc_of_the_standard_check_string() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn inflates_stored_fixed_and_dynamic_blocks() {
        // Made with Python's zlib: level 0 (stored) and a short string (fixed codes) raw
        // (wbits=-15), and six kilobytes of text at level 9 (dynamic codes) with its wrapper.
        let guardado = [0x01, 0x05, 0x00, 0xfa, 0xff, b'h', b'o', b'l', b'a', b'!'];
        assert_eq!(inflate(&guardado).unwrap(), b"hola!");
        let fijo = [0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x07, 0x00];
        assert_eq!(inflate(&fijo).unwrap(), b"hello");
        let zlib = include_bytes!("../../tests/huellas/texto.zlib");
        let texto = include_bytes!("../../tests/huellas/texto.txt");
        assert_eq!(inflate_zlib(zlib).unwrap(), texto.to_vec());
    }

    #[test]
    fn broken_data_is_refused_not_a_panic() {
        assert!(inflate(&[]).is_none());
        assert!(inflate(&[0xff; 64]).is_none());
        assert!(inflate(&[0x01, 0x05, 0x00, 0x00, 0x00]).is_none());
    }
}
