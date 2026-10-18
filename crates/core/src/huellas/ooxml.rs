//! Word, Excel and PowerPoint files (`.docx`, `.xlsx`, `.pptx`): a ZIP whose `docProps/core.xml`
//! says who wrote it, who touched it last and when, and `docProps/app.xml` the company, the
//! manager and the template. Only those parts are inflated; the document's own text is never
//! opened.
//!
//! The clean copy keeps every other part byte for byte (still compressed as it was) and puts in
//! those three an empty set of properties. The times of every entry in the ZIP, which say when
//! each part was saved, are set to the ZIP's own zero (1 January 1980).

use super::{inflate, Clave, Huella, Informe};

const CORE: &str = "docProps/core.xml";
const APP: &str = "docProps/app.xml";
const CUSTOM: &str = "docProps/custom.xml";

const CORE_VACIO: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" xmlns:dcmitype=\"http://purl.org/dc/dcmitype/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"/>";
const APP_VACIO: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\" xmlns:vt=\"http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes\"/>";
const CUSTOM_VACIO: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/custom-properties\" xmlns:vt=\"http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes\"/>";

fn u16a(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*d.get(o)?, *d.get(o + 1)?]))
}

fn u32a(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

fn us(v: u32) -> Option<usize> {
    usize::try_from(v).ok()
}

/// One entry of the central directory.
struct Entrada {
    nombre: String,
    /// The central directory record, whole.
    central: Vec<u8>,
    metodo: u16,
    flags: u16,
    comprimido: usize,
    local: usize,
}

fn directorio(d: &[u8]) -> Option<Vec<Entrada>> {
    // The end record is in the last 64 KB + 22 bytes.
    let desde = d.len().saturating_sub(65_557);
    let fin = (desde..d.len().saturating_sub(21))
        .rev()
        .find(|&i| d.get(i..i + 4) == Some(b"PK\x05\x06"))?;
    let total = u16a(d, fin + 10)?;
    let mut p = us(u32a(d, fin + 16)?)?;
    let mut v = Vec::new();
    for _ in 0..total {
        if d.get(p..p + 4)? != b"PK\x01\x02" {
            return None;
        }
        let flags = u16a(d, p + 8)?;
        let metodo = u16a(d, p + 10)?;
        let comprimido = u32a(d, p + 20)?;
        let n = usize::from(u16a(d, p + 28)?);
        let e = usize::from(u16a(d, p + 30)?);
        let c = usize::from(u16a(d, p + 32)?);
        let local = u32a(d, p + 42)?;
        // ZIP64 (parts over 4 GB) is not followed: an Office file never needs it.
        if comprimido == u32::MAX || local == u32::MAX {
            return None;
        }
        let largo = 46 + n + e + c;
        v.push(Entrada {
            nombre: String::from_utf8_lossy(d.get(p + 46..p + 46 + n)?).into_owned(),
            central: d.get(p..p + largo)?.to_vec(),
            metodo,
            flags,
            comprimido: us(comprimido)?,
            local: us(local)?,
        });
        p += largo;
    }
    Some(v)
}

/// Where an entry's data starts.
fn inicio_datos(d: &[u8], e: &Entrada) -> Option<usize> {
    if d.get(e.local..e.local + 4)? != b"PK\x03\x04" {
        return None;
    }
    let n = usize::from(u16a(d, e.local + 26)?);
    let x = usize::from(u16a(d, e.local + 28)?);
    Some(e.local + 30 + n + x)
}

fn contenido(d: &[u8], e: &Entrada) -> Option<Vec<u8>> {
    let ini = inicio_datos(d, e)?;
    let datos = d.get(ini..ini.checked_add(e.comprimido)?)?;
    match e.metodo {
        0 => Some(datos.to_vec()),
        8 => inflate::inflate(datos),
        _ => None,
    }
}

fn es_office(entradas: &[Entrada]) -> bool {
    entradas.iter().any(|e| e.nombre == "[Content_Types].xml")
        && entradas
            .iter()
            .any(|e| e.nombre == CORE || e.nombre == APP || e.nombre.starts_with("word/"))
}

/// The text of every `<tag …>value</tag>` in an XML part.
fn etiquetas(xml: &str, tag: &str) -> Vec<String> {
    let mut v = Vec::new();
    let abre = format!("<{tag}");
    let cierra = format!("</{tag}>");
    let mut p = 0;
    while let Some(i) = xml.get(p..).and_then(|s| s.find(&abre)).map(|i| i + p) {
        let tras = i + abre.len();
        // `<dc:creator>` or `<dc:creator xsi:…>`, not `<dc:creatorX>`.
        if !matches!(xml.as_bytes().get(tras), Some(b'>' | b' ' | b'/')) {
            p = tras;
            continue;
        }
        let Some(gt) = xml.get(tras..).and_then(|s| s.find('>')).map(|g| g + tras) else {
            break;
        };
        if xml.as_bytes().get(gt.wrapping_sub(1)) == Some(&b'/') {
            p = gt;
            continue;
        }
        let Some(f) = xml.get(gt..).and_then(|s| s.find(&cierra)).map(|f| f + gt) else {
            break;
        };
        let t = super::legible(&super::desescapa(xml.get(gt + 1..f).unwrap_or_default()));
        if !t.is_empty() {
            v.push(t);
        }
        p = f;
    }
    v
}

pub(crate) fn mira(d: &[u8]) -> Option<Informe> {
    if !d.starts_with(b"PK\x03\x04") {
        return None;
    }
    let entradas = directorio(d)?;
    if !es_office(&entradas) {
        return None;
    }
    let mut inf = Informe::nuevo("office");
    let mut legible = true;
    for e in &entradas {
        let lista: &[(&str, Clave)] = match e.nombre.as_str() {
            CORE => &[
                ("dc:creator", Clave::Autor),
                ("cp:lastModifiedBy", Clave::UltimoAutor),
                ("dcterms:created", Clave::Fecha),
                ("dcterms:modified", Clave::Fecha),
                ("cp:lastPrinted", Clave::Fecha),
                ("dc:title", Clave::Titulo),
                ("dc:subject", Clave::Descripcion),
                ("dc:description", Clave::Descripcion),
                ("cp:keywords", Clave::Descripcion),
            ],
            APP => &[
                ("Company", Clave::Empresa),
                ("Manager", Clave::Responsable),
                ("Template", Clave::Plantilla),
                ("Application", Clave::Programa),
            ],
            CUSTOM => {
                inf.anade(vec![Huella::new(Clave::Propiedades, String::new())]);
                &[]
            }
            _ => continue,
        };
        let Some(xml) = contenido(d, e) else {
            // A part that could not be read: it is still replaced in the clean copy, but what it
            // said is not known, so it is listed as properties not read.
            legible = false;
            continue;
        };
        let xml = String::from_utf8_lossy(&xml);
        for (tag, clave) in lista {
            for t in etiquetas(&xml, tag) {
                // «Normal.dotm» is every Word's template; a path is not.
                if *clave == Clave::Plantilla && !t.contains(['\\', '/']) {
                    continue;
                }
                inf.anade(vec![Huella::new(*clave, t)]);
            }
        }
    }
    if !legible {
        inf.anade(vec![Huella::new(Clave::Propiedades, String::new())]);
    }
    inf.limpiable = true;
    Some(inf)
}

fn local_nuevo(nombre: &[u8], flags: u16, datos: &[u8]) -> Option<Vec<u8>> {
    let mut v = b"PK\x03\x04".to_vec();
    v.extend_from_slice(&20u16.to_le_bytes());
    v.extend_from_slice(&(flags & 0x0800).to_le_bytes());
    v.extend_from_slice(&0u16.to_le_bytes()); // stored
    v.extend_from_slice(&0u16.to_le_bytes()); // time
    v.extend_from_slice(&0x0021u16.to_le_bytes()); // 1 Jan 1980
    v.extend_from_slice(&inflate::crc32(datos).to_le_bytes());
    let n = u32::try_from(datos.len()).ok()?;
    v.extend_from_slice(&n.to_le_bytes());
    v.extend_from_slice(&n.to_le_bytes());
    v.extend_from_slice(&u16::try_from(nombre.len()).ok()?.to_le_bytes());
    v.extend_from_slice(&0u16.to_le_bytes());
    v.extend_from_slice(nombre);
    v.extend_from_slice(datos);
    Some(v)
}

pub(crate) fn limpia(d: &[u8]) -> Option<Vec<u8>> {
    let entradas = directorio(d)?;
    if !es_office(&entradas) {
        return None;
    }
    let mut out = Vec::with_capacity(d.len());
    let mut central = Vec::new();
    for e in &entradas {
        let offset = u32::try_from(out.len()).ok()?;
        let mut c = e.central.clone();
        let nuevo = match e.nombre.as_str() {
            CORE => Some(CORE_VACIO),
            APP => Some(APP_VACIO),
            CUSTOM => Some(CUSTOM_VACIO),
            _ => None,
        };
        if let Some(xml) = nuevo {
            let nombre = c.get(46..46 + e.nombre.len())?.to_vec();
            let local = local_nuevo(&nombre, e.flags, xml.as_bytes())?;
            // The central record says the same as the new local header.
            c.get_mut(6..8)?.copy_from_slice(&20u16.to_le_bytes());
            c.get_mut(8..10)?
                .copy_from_slice(&(e.flags & 0x0800).to_le_bytes());
            c.get_mut(10..12)?.copy_from_slice(&0u16.to_le_bytes());
            c.get_mut(16..28)?.copy_from_slice(local.get(14..26)?);
            // No extra field in the central record either (it may hold times).
            let n = e.nombre.len();
            let x = usize::from(u16a(&c, 30)?);
            c.drain(46 + n..46 + n + x);
            c.get_mut(30..32)?.copy_from_slice(&0u16.to_le_bytes());
            out.extend(local);
        } else {
            let ini = inicio_datos(d, e)?;
            let mut fin = ini.checked_add(e.comprimido)?;
            // A data descriptor after the data (flag bit 3): 12 bytes, or 16 with its signature.
            if e.flags & 0x0008 != 0 {
                fin += if d.get(fin..fin + 4) == Some(b"PK\x07\x08") {
                    16
                } else {
                    12
                };
            }
            let mut trozo = d.get(e.local..fin)?.to_vec();
            trozo.get_mut(10..14)?.copy_from_slice(&[0, 0, 0x21, 0]);
            out.extend(trozo);
        }
        c.get_mut(12..16)?.copy_from_slice(&[0, 0, 0x21, 0]);
        c.get_mut(42..46)?.copy_from_slice(&offset.to_le_bytes());
        central.extend(c);
    }
    let inicio_central = u32::try_from(out.len()).ok()?;
    let largo_central = u32::try_from(central.len()).ok()?;
    let n = u16::try_from(entradas.len()).ok()?;
    out.extend(central);
    out.extend_from_slice(b"PK\x05\x06\0\0\0\0");
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&largo_central.to_le_bytes());
    out.extend_from_slice(&inicio_central.to_le_bytes());
    // No ZIP comment: it is free text and could say anything.
    out.extend_from_slice(&0u16.to_le_bytes());
    Some(out)
}
