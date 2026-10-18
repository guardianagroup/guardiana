//! Fixtures in `crates/core/tests/huellas/`, made with Python (Pillow for the pictures, the
//! standard `zipfile` for the Word file, qpdf to check the PDF and to make its compressed twin).
//! The HEIC is the container only, with placeholder bytes where the picture would be: what is
//! tested is that those bytes come out untouched.

use super::*;

const JPG: &[u8] = include_bytes!("../../tests/huellas/foto-gps.jpg");
const PNG: &[u8] = include_bytes!("../../tests/huellas/foto.png");
const HEIC: &[u8] = include_bytes!("../../tests/huellas/foto.heic");
const PDF: &[u8] = include_bytes!("../../tests/huellas/documento.pdf");
const PDF_COMPRIMIDO: &[u8] = include_bytes!("../../tests/huellas/comprimido.pdf");
const DOCX: &[u8] = include_bytes!("../../tests/huellas/informe.docx");

fn tiene(inf: &Informe, clave: Clave, valor: &str) -> bool {
    inf.huellas
        .iter()
        .any(|h| h.clave == clave && h.valor == valor)
}

fn busca(d: &[u8], aguja: &[u8]) -> bool {
    d.windows(aguja.len()).any(|w| w == aguja)
}

#[test]
fn a_phone_photo_says_where_with_what_and_who() {
    let inf = mirar(JPG);
    assert_eq!(inf.formato, "jpeg");
    assert!(inf.limpiable);
    // 10°23'45.12" N, 75°33'11.88" W: Cartagena de Indias.
    assert!(tiene(&inf, Clave::Gps, "10.39587, -75.55330"), "{inf:?}");
    assert!(
        tiene(&inf, Clave::Dispositivo, "Apple iPhone 15 Pro"),
        "{inf:?}"
    );
    assert!(tiene(&inf, Clave::Serie, "F2LXK0123456"));
    assert!(tiene(&inf, Clave::Autor, "Ana Perez"));
    assert!(tiene(&inf, Clave::Fecha, "2026:10:12 18:41:07"));
    assert!(tiene(&inf, Clave::Programa, "Lightroom 7.1"));
    assert!(tiene(&inf, Clave::Descripcion, "Hecho por Ana en casa"));
}

#[test]
fn the_clean_photo_says_nothing_keeps_the_picture_and_which_way_up() {
    let limpio = limpiar(JPG).unwrap();
    let inf = mirar(&limpio);
    assert_eq!(inf.formato, "jpeg");
    assert!(inf.huellas.is_empty(), "{inf:?}");
    assert!(!inf.delata());
    for fuera in [&b"Ana"[..], b"iPhone", b"F2LXK", b"Lightroom", b"TRAILER"] {
        assert!(!busca(&limpio, fuera), "{}", String::from_utf8_lossy(fuera));
    }
    // The orientation (6, turned right) stays, alone.
    assert!(busca(&limpio, &exif::solo_orientacion(6)));
    // From the first table to the end of the picture, the same bytes.
    let desde = |d: &[u8]| d.windows(2).position(|w| w == [0xff, 0xdb]).unwrap();
    let fin = JPG.len() - b"TRAILER-DE-FABRICANTE".len();
    assert_eq!(&limpio[desde(&limpio)..], &JPG[desde(JPG)..fin]);
    // Cleaning twice changes nothing more.
    assert_eq!(limpiar(&limpio).unwrap(), limpio);
}

#[test]
fn a_png_with_text_and_exif_is_cleaned_chunk_by_chunk() {
    let inf = mirar(PNG);
    assert_eq!(inf.formato, "png");
    assert!(tiene(&inf, Clave::Autor, "Ana Perez"), "{inf:?}");
    assert!(tiene(&inf, Clave::Programa, "GIMP 2.10"));
    assert!(tiene(&inf, Clave::Gps, "10.39587, -75.55330"));
    let limpio = limpiar(PNG).unwrap();
    assert!(mirar(&limpio).huellas.is_empty());
    for t in [&b"tEXt"[..], b"zTXt", b"eXIf", b"tIME"] {
        assert!(!busca(&limpio, t));
    }
    assert!(busca(&limpio, b"IDAT") && busca(&limpio, b"IEND"));
}

#[test]
fn a_heic_is_cleaned_in_place_and_its_picture_is_untouched() {
    let inf = mirar(HEIC);
    assert_eq!(inf.formato, "heic");
    assert!(inf.limpiable);
    assert!(tiene(&inf, Clave::Gps, "10.39587, -75.55330"), "{inf:?}");
    assert!(tiene(&inf, Clave::Programa, "iOS 17.4.1"));
    let limpio = limpiar(HEIC).unwrap();
    assert_eq!(
        limpio.len(),
        HEIC.len(),
        "same size: every offset stays right"
    );
    assert!(mirar(&limpio).huellas.is_empty(), "{:?}", mirar(&limpio));
    let foto = b"PICTURE-BYTES-NEVER-READ".repeat(4);
    assert!(busca(&limpio, &foto));
    assert!(!busca(&limpio, b"iPhone") && !busca(&limpio, b"iOS 17"));
}

#[test]
fn a_pdf_says_its_author_company_and_program_and_the_copy_blanks_them_in_place() {
    let inf = mirar(PDF);
    assert_eq!(inf.formato, "pdf");
    assert!(tiene(&inf, Clave::Autor, "Ana Pérez"), "{inf:?}");
    assert!(tiene(&inf, Clave::Empresa, "Ejemplo S.A."));
    assert!(tiene(&inf, Clave::Programa, "macOS Quartz (15.1)"));
    assert!(tiene(&inf, Clave::Programa, "Microsoft Word"));
    assert!(tiene(&inf, Clave::Autor, "Ana Perez"));
    assert_eq!(inf.nota, None);
    let limpio = limpiar(PDF).unwrap();
    assert_eq!(
        limpio.len(),
        PDF.len(),
        "same length: the xref offsets stay right"
    );
    let otra = mirar(&limpio);
    assert!(otra.huellas.is_empty(), "{otra:?}");
    assert!(!busca(&limpio, b"Ejemplo") && !busca(&limpio, b"Quartz"));
    assert!(busca(&limpio, b"/MediaBox [0 0 200 200]"));
}

#[test]
fn a_pdf_with_compressed_structure_is_not_called_clean() {
    let inf = mirar(PDF_COMPRIMIDO);
    assert_eq!(inf.formato, "pdf");
    assert_eq!(inf.nota, Some("pdf_comprimido"));
    assert!(inf.delata(), "what could not be read is said, not hidden");
}

#[test]
fn a_word_file_says_who_wrote_it_and_where_and_the_copy_keeps_the_text() {
    let inf = mirar(DOCX);
    assert_eq!(inf.formato, "office");
    assert!(tiene(&inf, Clave::Autor, "Ana Perez"), "{inf:?}");
    assert!(tiene(&inf, Clave::UltimoAutor, "Luis G&M"));
    assert!(tiene(&inf, Clave::Empresa, "Ejemplo S.A."));
    assert!(tiene(&inf, Clave::Responsable, "Marta"));
    assert!(tiene(&inf, Clave::Fecha, "2026-10-12T18:41:00Z"));
    assert!(inf
        .huellas
        .iter()
        .any(|h| h.clave == Clave::Plantilla && h.valor.contains("ana.perez")));
    let limpio = limpiar(DOCX).unwrap();
    let otra = mirar(&limpio);
    assert_eq!(otra.formato, "office");
    assert!(otra.huellas.is_empty(), "{otra:?}");
    assert!(!busca(&limpio, b"ana.perez"), "nor in the ZIP comment");
    // The document part is copied still compressed, byte for byte.
    let partes = ooxml_partes(DOCX);
    let partes_limpias = ooxml_partes(&limpio);
    assert_eq!(
        partes["word/document.xml"],
        partes_limpias["word/document.xml"]
    );
}

/// Each part's compressed bytes, by name (read through the same directory the cleaner uses).
fn ooxml_partes(d: &[u8]) -> std::collections::HashMap<String, Vec<u8>> {
    let mut m = std::collections::HashMap::new();
    let mut p = 0;
    while let Some(i) = d[p..].windows(4).position(|w| w == b"PK\x03\x04") {
        let o = p + i;
        let c = u32::from_le_bytes(d[o + 18..o + 22].try_into().unwrap()) as usize;
        let n = u16::from_le_bytes([d[o + 26], d[o + 27]]) as usize;
        let x = u16::from_le_bytes([d[o + 28], d[o + 29]]) as usize;
        let nombre = String::from_utf8_lossy(&d[o + 30..o + 30 + n]).into_owned();
        m.insert(nombre, d[o + 30 + n + x..o + 30 + n + x + c].to_vec());
        p = o + 4;
    }
    m
}

#[test]
fn other_files_are_other_and_are_not_cleaned() {
    let inf = mirar(b"hola, esto es texto");
    assert_eq!(inf.formato, "otro");
    assert!(!inf.limpiable);
    assert_eq!(limpiar(b"hola"), Err(Fallo::NoSoportado));
    // A ZIP that is not an Office file is not one.
    assert_eq!(mirar(b"PK\x03\x04 nada").formato, "otro");
}

#[test]
fn every_truncation_of_every_fixture_is_read_without_a_panic() {
    for f in [JPG, PNG, HEIC, PDF, DOCX] {
        for n in (0..f.len()).step_by(7) {
            let _ = mirar(&f[..n]);
            let _ = limpiar(&f[..n]);
        }
    }
}

#[test]
fn the_clean_name_keeps_the_extension() {
    assert_eq!(nombre_limpio("foto.jpg"), "foto-sin-huellas.jpg");
    assert_eq!(
        nombre_limpio("C:\\Users\\x\\informe.v2.docx"),
        "informe.v2-sin-huellas.docx"
    );
    assert_eq!(nombre_limpio("LEEME"), "LEEME-sin-huellas");
    assert_eq!(nombre_limpio(".bashrc"), ".bashrc-sin-huellas");
}
