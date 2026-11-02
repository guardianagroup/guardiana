//! «Engaño» (phase A3; the owner, 1 Nov 2026, 18:55: «que vea todo lo posible, todo, absolutamente todo»): a program
//! whose name is made to look like something else. `factura.pdf.exe` shows as `factura.pdf` when Windows hides known
//! extensions; a name with Unicode's right-to-left override (U+202E) before `fdp.exe` is shown backwards from there, so
//! `factura`, that character and `fdp.exe` read as a name ending in `.pdf`. Both are among the oldest tricks to make a
//! person open a program believing it is a document, a photo or a song.
//!
//! The service sees every program start (Windows' kernel process events, the same watch as [`crate::cadenas`]);
//! [`sospechoso`] is the cheap test on the file name, run for each, and [`juzga`] turns a start into a grave warning.

use crate::primera_vez::Aviso;

/// Extensions of what a person opens believing it is not a program.
const DOCUMENTOS: &[&str] = &[
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "rtf", "txt", "csv", "jpg",
    "jpeg", "png", "gif", "bmp", "heic", "webp", "mp3", "wav", "mp4", "avi", "mov", "mkv", "zip",
    "rar", "7z", "html", "htm", "eml", "xml", "json",
];

/// Extensions of what runs.
const EJECUTABLES: &[&str] = &[
    "exe", "scr", "com", "pif", "bat", "cmd", "vbs", "vbe", "js", "jse", "wsf", "wsh", "hta",
    "msi", "cpl", "ps1",
];

/// Unicode's controls that change the direction of text, and with it the order a name is shown in.
const VUELTAS: &[char] = &[
    '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}', '\u{2066}', '\u{2067}', '\u{2068}',
    '\u{2069}',
];

/// Why a program's file name `nombre` deceives, or `None`.
#[must_use]
pub fn sospechoso(nombre: &str) -> Option<&'static str> {
    if nombre.contains(VUELTAS) {
        return Some(
            "su nombre lleva un carácter invisible que da la vuelta al texto, para que la extensión real no se vea",
        );
    }
    let bajo = nombre.to_lowercase();
    let mut partes = bajo.rsplit('.');
    let ultima = partes.next()?.trim();
    let penultima = partes.next()?.trim();
    // a name with only one dot has no second extension
    partes.next()?;
    if EJECUTABLES.contains(&ultima) && DOCUMENTOS.contains(&penultima) {
        return Some(
            "su nombre termina como un documento seguido de la extensión de un programa, y con las extensiones \
             ocultas Windows lo enseña como un documento",
        );
    }
    None
}

/// A deceiving program start as a grave warning: `nombre` its file name, `ruta` its path.
#[must_use]
pub fn juzga(nombre: &str, ruta: &str, cuando_ms: u64) -> Option<(Aviso, bool)> {
    let porque = sospechoso(nombre)?;
    // the name as it really is, with the controls shown and not obeyed
    let visible: String = nombre
        .chars()
        .map(|c| if VUELTAS.contains(&c) { '⟲' } else { c })
        .collect();
    let texto = format!(
        "Se ha abierto «{visible}», que es un programa aunque no lo parezca: {porque}. Es un engaño clásico para que se \
         abra un programa creyendo que es un documento, una foto o una canción. Si no lo esperabas, ciérralo y no lo \
         vuelvas a abrir."
    );
    Some((
        Aviso {
            regla: "engano",
            programa: visible,
            ruta: ruta.to_owned(),
            sitio: String::new(),
            nombre: ruta.to_lowercase(),
            cuando_ms,
            fuente: None,
            texto,
        },
        true,
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn nombres_que_enganan() {
        assert!(sospechoso("factura.pdf.exe").is_some());
        assert!(sospechoso("Factura.PDF.EXE").is_some());
        assert!(sospechoso("fotos de la boda.jpg.scr").is_some());
        assert!(sospechoso("contrato.docx          .exe").is_some());
        assert!(sospechoso("factura\u{202E}fdp.exe").is_some());
        let (a, grave) = juzga(
            "factura\u{202E}fdp.exe",
            r"C:\Users\ana\Downloads\factura\u{202E}fdp.exe",
            1,
        )
        .unwrap();
        assert!(grave);
        assert_eq!(a.regla, "engano");
        assert!(
            !a.texto.contains('\u{202E}'),
            "el aviso no obedece la vuelta"
        );
        assert!(a.texto.contains("factura⟲fdp.exe"), "{}", a.texto);
    }

    #[test]
    fn nombres_normales() {
        for n in [
            "chrome.exe",
            "setup.x64.exe",
            "python3.12.exe",
            "Microsoft.Photos.exe",
            "informe.pdf",
            "copia.tar.gz",
            "jquery.min.js",
            ".exe",
            "exe",
            "",
        ] {
            assert_eq!(sospechoso(n), None, "{n}");
        }
        assert_eq!(juzga("chrome.exe", r"C:\x\chrome.exe", 1), None);
    }
}
