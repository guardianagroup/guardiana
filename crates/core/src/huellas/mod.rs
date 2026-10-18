//! «Compartir sin huellas» (customer request, 18 Oct 2026): before a photo or a document leaves
//! the computer, say what its metadata tells about the person, and make a copy without it.
//!
//! What is looked at, and nothing else: the metadata headers of the file.
//!
//! - JPEG, PNG, HEIC/HEIF: EXIF (GPS position, phone or camera, serial number, owner, dates,
//!   program), XMP, IPTC, comments, PNG's text chunks.
//! - PDF: the information dictionary (author, program, dates, company…) and XMP.
//! - Word, Excel, PowerPoint: `docProps` (author, last author, company, manager, template, dates).
//!
//! The picture is never decoded and the document's text is never opened: the clean copy keeps
//! those bytes exactly as they were. Everything happens here, on this computer; nothing is sent.
//! The format is told by the file's first bytes, never by its name.
//!
//! What it does not see is said, not hidden: a PDF that keeps its structure compressed may carry
//! an author this cannot read (`nota = "pdf_comprimido"`), and a HEIF laid out in a way this does
//! not follow is reported and not offered as cleanable. An Office document's comments and
//! tracked changes carry their authors' names in the text itself, which is not opened.
//! This lives in `guardiana-core` because the panel (the service) and ZERO both use it, and the
//! service must not depend on `guardiana-secure`; it brings no dependency of its own.

mod exif;
mod heic;
mod inflate;
mod jpeg;
mod ooxml;
mod pdf;
mod png;

use serde::Serialize;

/// The largest file looked at. Photos and documents are well under it; the panel and ZERO say
/// so instead of looking at a bigger one.
pub const TAMANO_MAXIMO: usize = 64 * 1024 * 1024;

/// What a piece of metadata says about the person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Clave {
    /// Where the photo was taken.
    Gps,
    /// The phone, camera or lens.
    Dispositivo,
    /// A serial number.
    Serie,
    /// Who made it (author, owner, artist).
    Autor,
    /// Who saved it last.
    UltimoAutor,
    /// The company.
    Empresa,
    /// The manager.
    Responsable,
    /// A date and time.
    Fecha,
    /// The program, with its version.
    Programa,
    /// A title.
    Titulo,
    /// A description, keywords, a comment.
    Descripcion,
    /// A copyright line (usually a name).
    Derechos,
    /// A template on a path of the computer (which often says the account's name).
    Plantilla,
    /// A block of properties whose detail is not shown (IPTC, XMP, custom properties).
    Propiedades,
}

/// One thing the file says.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Huella {
    /// What it is.
    pub clave: Clave,
    /// What it says, as written in the file (empty when only its presence is known).
    pub valor: String,
}

impl Huella {
    fn new(clave: Clave, valor: String) -> Self {
        Self { clave, valor }
    }
}

/// What a file's metadata says.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Informe {
    /// `jpeg`, `png`, `heic`, `pdf`, `office` or `otro` (not a format looked at).
    pub formato: &'static str,
    /// What it says, in the file's order; empty when it says nothing about the person.
    pub huellas: Vec<Huella>,
    /// Whether a clean copy can be made.
    pub limpiable: bool,
    /// A limit of what was looked at, as a text key (`pdf_comprimido`, `heic_no_seguido`).
    pub nota: Option<&'static str>,
}

impl Informe {
    fn nuevo(formato: &'static str) -> Self {
        Self {
            formato,
            huellas: Vec::new(),
            limpiable: false,
            nota: None,
        }
    }

    fn anade(&mut self, hs: Vec<Huella>) {
        for h in hs {
            if !self.huellas.contains(&h) {
                self.huellas.push(h);
            }
        }
    }

    /// Whether the file says anything about the person (or may, where it could not be read).
    #[must_use]
    pub fn delata(&self) -> bool {
        !self.huellas.is_empty() || self.nota.is_some()
    }
}

/// Why a clean copy was not made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fallo {
    /// Not a format this cleans.
    NoSoportado,
    /// Larger than [`TAMANO_MAXIMO`].
    Grande,
    /// The file is damaged in its structure; nothing was made rather than a broken copy.
    Danado,
}

/// What the metadata of `datos` says. Never fails: a file it cannot read is `otro`.
#[must_use]
pub fn mirar(datos: &[u8]) -> Informe {
    if datos.len() > TAMANO_MAXIMO {
        return Informe::nuevo("otro");
    }
    jpeg::mira(datos)
        .or_else(|| png::mira(datos))
        .or_else(|| heic::mira(datos))
        .or_else(|| pdf::mira(datos))
        .or_else(|| ooxml::mira(datos))
        .unwrap_or_else(|| Informe::nuevo("otro"))
}

/// A copy of `datos` without the metadata [`mirar`] reports. The picture's bytes and the
/// document's content are the same as in the original.
pub fn limpiar(datos: &[u8]) -> Result<Vec<u8>, Fallo> {
    if datos.len() > TAMANO_MAXIMO {
        return Err(Fallo::Grande);
    }
    let inf = mirar(datos);
    if !inf.limpiable {
        return Err(Fallo::NoSoportado);
    }
    let r = match inf.formato {
        "jpeg" => jpeg::limpia(datos),
        "png" => png::limpia(datos),
        "heic" => heic::limpia(datos),
        "pdf" => pdf::limpia(datos),
        "office" => ooxml::limpia(datos),
        _ => return Err(Fallo::NoSoportado),
    };
    r.ok_or(Fallo::Danado)
}

/// The clean copy's name: `foto.jpg` → `foto-sin-huellas.jpg`.
#[must_use]
pub fn nombre_limpio(nombre: &str) -> String {
    let nombre = nombre.rsplit(['/', '\\']).next().unwrap_or(nombre);
    match nombre.rsplit_once('.') {
        Some((base, ext)) if !base.is_empty() => format!("{base}-sin-huellas.{ext}"),
        _ => format!("{nombre}-sin-huellas"),
    }
}

/// A value as it can be shown: no control characters, no padding, at most 160 characters.
fn legible(s: &str) -> String {
    let t: String = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    t.chars().take(160).collect()
}

fn desescapa(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// What an XMP packet says: its creator, the program, the dates and, when any of its other
/// properties is filled in, that it has more.
fn xmp_huellas(b: &[u8]) -> Vec<Huella> {
    let x = String::from_utf8_lossy(b);
    let mut v = Vec::new();
    let mut pon = |clave: Clave, valor: String| {
        let h = Huella::new(clave, valor);
        if !valor_vacio(&h) && !v.contains(&h) {
            v.push(h);
        }
    };
    for (nombre, clave) in [
        ("xmp:CreatorTool", Clave::Programa),
        ("pdf:Producer", Clave::Programa),
        ("xmp:CreateDate", Clave::Fecha),
        ("xmp:ModifyDate", Clave::Fecha),
        ("photoshop:DateCreated", Clave::Fecha),
        ("exif:GPSLatitude", Clave::Gps),
        ("tiff:Model", Clave::Dispositivo),
        ("exifEX:BodySerialNumber", Clave::Serie),
        ("aux:SerialNumber", Clave::Serie),
        ("pdf:Author", Clave::Autor),
    ] {
        // As an attribute (`xmp:CreatorTool="…"`) or as an element (`<xmp:CreatorTool>…<`).
        for patron in [format!("{nombre}=\""), format!("<{nombre}>")] {
            let mut p = 0;
            while let Some(i) = x.get(p..).and_then(|s| s.find(&patron)).map(|i| i + p) {
                let ini = i + patron.len();
                let fin_c = if patron.ends_with('"') { '"' } else { '<' };
                let Some(f) = x.get(ini..).and_then(|s| s.find(fin_c)).map(|f| f + ini) else {
                    break;
                };
                pon(
                    clave,
                    legible(&desescapa(x.get(ini..f).unwrap_or_default())),
                );
                p = f;
            }
        }
    }
    // dc:creator is a list: <dc:creator><rdf:Seq><rdf:li>Ana</rdf:li>…
    if let Some(i) = x.find("<dc:creator") {
        if let Some(f) = x
            .get(i..)
            .and_then(|s| s.find("</dc:creator>"))
            .map(|f| f + i)
        {
            let dentro = x.get(i..f).unwrap_or_default();
            let mut p = 0;
            while let Some(a) = dentro
                .get(p..)
                .and_then(|s| s.find("<rdf:li"))
                .map(|a| a + p)
            {
                let Some(gt) = dentro.get(a..).and_then(|s| s.find('>')).map(|g| g + a) else {
                    break;
                };
                let Some(c) = dentro.get(gt..).and_then(|s| s.find('<')).map(|c| c + gt) else {
                    break;
                };
                pon(
                    Clave::Autor,
                    legible(&desescapa(dentro.get(gt + 1..c).unwrap_or_default())),
                );
                p = c;
            }
        }
    }
    v
}

fn valor_vacio(h: &Huella) -> bool {
    h.valor.is_empty() && h.clave != Clave::Propiedades && h.clave != Clave::Gps
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
