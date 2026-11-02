//! «Señuelos» contra el secuestro de archivos (ransomware). The owner, 2 Nov 2026, 07:55: «ahora hay unos malware
//! muy fuertes, ransomware… tenemos que poder detectarlos y sacarlos, eso es primordial».
//!
//! GUARDIANA leaves a few hidden files in the folders a ransomware goes through first (the profile itself, Documents,
//! Desktop, Pictures and the public Documents), named to come first and last in a folder's order. Nobody has a reason
//! to change them: a program that renames, deletes or writes over one is encrypting what it finds, and the service
//! freezes it at once (`crate::contencion`), before it reaches the rest. The person then chooses: stop it and put it in
//! the quarantine, or let it carry on if it was theirs.
//!
//! What this module decides is pure (names, contents, the verdict); writing the files and freezing the program are the
//! service's (`crates/cli/src/escudo.rs`) and Windows' (`crates/apps/src/contencion.rs`). The files say what they are
//! in their first line, so anyone who finds one knows.

use std::path::{Path, PathBuf};

use crate::primera_vez::Aviso;

/// The decoys of one folder: the first and the last of it, as documents a ransomware looks for.
pub const NOMBRES: [&str; 2] = [
    "!!!!GUARDIANA-senuelo-no-tocar.docx",
    "~~~~GUARDIANA-senuelo-no-tocar.pdf",
];

/// The folders of a profile where decoys go (each one only if it exists and is a real folder of the disk, not one kept
/// by OneDrive, whose copies would travel to the cloud).
pub const CARPETAS_DEL_PERFIL: [&str; 4] = ["", "Documents", "Desktop", "Pictures"];

/// How big a decoy is: some ransomware leaves very small files alone.
pub const TAMANO: usize = 64 * 1024;

/// What a decoy says about itself, first thing in the file.
pub const CABECERA: &str = "GUARDIANA - archivo señuelo contra el secuestro de archivos (ransomware). No lo cambies ni \
lo borres: si un programa lo hace, GUARDIANA lo congela al momento y te pregunta qué hacer. Es seguro tenerlo y no ocupa \
casi nada.\r\n";

/// Whether a path is one of GUARDIANA's decoys (by its file name). Cheap: it is asked for every file opened on the
/// computer, so almost every name stops at its first four bytes, without allocating.
#[must_use]
pub fn es_senuelo(ruta: &str) -> bool {
    let nombre = ruta.rsplit(['\\', '/']).next().unwrap_or_default();
    let b = nombre.as_bytes();
    if b.len() < 21 || !(b.starts_with(b"!!!!") || b.starts_with(b"~~~~")) {
        return false;
    }
    nombre
        .get(4..21)
        .is_some_and(|x| x.eq_ignore_ascii_case("guardiana-senuelo"))
}

/// A decoy's bytes: its first line says what it is; the rest looks like any document's content (a fixed pseudo-random
/// series, the same in every decoy, so a changed one is told by its length and its time of writing).
#[must_use]
pub fn contenido() -> Vec<u8> {
    let mut out = Vec::with_capacity(TAMANO);
    out.extend_from_slice(CABECERA.as_bytes());
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    while out.len() < TAMANO {
        // xorshift64*: no randomness is needed, only bytes that do not compress to nothing
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        let v = x.wrapping_mul(0x2545_F491_4F6C_DD1D);
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.truncate(TAMANO);
    out
}

/// Where the decoys of a profile go: its folders of [`CARPETAS_DEL_PERFIL`], each with the two [`NOMBRES`].
#[must_use]
pub fn de_perfil(perfil: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for c in CARPETAS_DEL_PERFIL {
        let carpeta = if c.is_empty() {
            perfil.to_path_buf()
        } else {
            perfil.join(c)
        };
        for n in NOMBRES {
            out.push(carpeta.join(n));
        }
    }
    out
}

/// The profiles of a computer's `Users` folder whose decoys are looked after: every account's, never `Default`,
/// `Default User`, `All Users` or `defaultuser0` (Windows' templates), plus the public Documents.
#[must_use]
pub fn es_perfil_de_persona(nombre: &str) -> bool {
    let n = nombre.to_ascii_lowercase();
    !matches!(
        n.as_str(),
        "default" | "default user" | "all users" | "defaultuser0" | "public" | "desktop.ini"
    ) && !n.starts_with("defaultapppool")
}

/// What touched a decoy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toque {
    /// It was deleted.
    Borrado,
    /// It was renamed (a ransomware adds its own extension).
    Renombrado,
    /// Its content changed (written over in place).
    Escrito,
}

impl Toque {
    fn que(self) -> &'static str {
        match self {
            Self::Borrado => "ha borrado",
            Self::Renombrado => "ha renombrado",
            Self::Escrito => "ha reescrito",
        }
    }
}

/// What to do about a touched decoy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accion {
    /// Freeze the program now: nobody has a reason to change a decoy.
    Congelar,
    /// Only say it: the person's own file explorer (they deleted it by hand).
    Avisar,
}

/// The verdict on a program that touched decoy `archivo`. Windows' file explorer is the person's hand: only said, never
/// frozen. Any other program is frozen, signed or not: a decoy is not anybody's work.
#[must_use]
pub fn juzga(
    programa: &str,
    ruta: &str,
    archivo: &str,
    toque: Toque,
    cuando_ms: u64,
) -> (Aviso, Accion) {
    let explorador = programa.eq_ignore_ascii_case("explorer.exe")
        && ruta
            .to_ascii_lowercase()
            .ends_with(r"\windows\explorer.exe");
    let carpeta = archivo
        .rsplit_once(['\\', '/'])
        .map_or("", |(c, _)| c)
        .to_string();
    let (texto, accion) = if explorador {
        (
            format!(
                "Alguien {} desde el Explorador de Windows uno de los archivos señuelo de GUARDIANA (en {carpeta}). Si fuiste \
                 tú, no pasa nada: GUARDIANA lo vuelve a poner.",
                toque.que()
            ),
            Accion::Avisar,
        )
    } else {
        (
            format!(
                "«{programa}» {} uno de los archivos señuelo de GUARDIANA (en {carpeta}). Nadie tiene motivo para tocarlos: \
                 es lo que hace un programa de secuestro de archivos (ransomware) al encriptar lo que encuentra. GUARDIANA lo \
                 ha congelado. Elige: detenerlo y ponerlo en cuarentena, o dejarlo seguir si es tuyo.",
                toque.que()
            ),
            Accion::Congelar,
        )
    };
    (
        Aviso {
            regla: "senuelo",
            programa: programa.to_string(),
            ruta: ruta.to_string(),
            sitio: String::new(),
            nombre: programa.to_ascii_lowercase(),
            cuando_ms,
            fuente: None,
            texto,
        },
        accion,
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn los_nombres_y_su_orden() {
        for n in NOMBRES {
            assert!(es_senuelo(&format!(r"C:\Users\ana\Documents\{n}")));
            assert!(es_senuelo(&n.to_uppercase()));
        }
        assert!(es_senuelo(
            r"C:\Users\ana\Desktop\!!!!GUARDIANA-senuelo-no-tocar.docx.locked"
        ));
        assert!(!es_senuelo(r"C:\Users\ana\Documents\informe.docx"));
        assert!(!es_senuelo(r"C:\Users\ana\Documents\!!!!otra-cosa.docx"));
        // the first before any letter or digit, the last after them (as NTFS and the explorer order names)
        let mut v = ["0001.docx", "Zeta.pdf", "a.txt", NOMBRES[0], NOMBRES[1]];
        v.sort_by_key(|s| s.to_uppercase());
        assert_eq!(v.first(), Some(&NOMBRES[0]));
        assert_eq!(v.last(), Some(&NOMBRES[1]));
    }

    #[test]
    fn el_contenido_dice_lo_que_es() {
        let c = contenido();
        assert_eq!(c.len(), TAMANO);
        assert!(c.starts_with(CABECERA.as_bytes()));
        assert_eq!(c, contenido());
        // not a file of zeros: the rest varies
        let resto = &c[CABECERA.len()..];
        let distintos: std::collections::BTreeSet<u8> = resto.iter().copied().collect();
        assert!(distintos.len() > 200);
    }

    #[test]
    fn donde_van() {
        let v = de_perfil(Path::new(r"C:\Users\ana"));
        assert_eq!(v.len(), 8);
        assert!(v.iter().all(|p| es_senuelo(&p.to_string_lossy())));
        assert!(es_perfil_de_persona("ana"));
        for n in [
            "Default",
            "Public",
            "All Users",
            "defaultuser0",
            "Default User",
        ] {
            assert!(!es_perfil_de_persona(n), "{n}");
        }
    }

    #[test]
    fn congela_a_cualquiera_menos_al_explorador() {
        let (a, x) = juzga(
            "cifra.exe",
            r"C:\Users\ana\AppData\Local\Temp\cifra.exe",
            r"C:\Users\ana\Documents\!!!!GUARDIANA-senuelo-no-tocar.docx",
            Toque::Renombrado,
            7,
        );
        assert_eq!(x, Accion::Congelar);
        assert_eq!(a.regla, "senuelo");
        assert!(a.texto.contains("congelado") && a.texto.contains("ransomware"));
        // signed or not: a decoy is nobody's work
        let (_, x) = juzga(
            "powershell.exe",
            r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
            r"C:\Users\ana\Desktop\~~~~GUARDIANA-senuelo-no-tocar.pdf",
            Toque::Escrito,
            7,
        );
        assert_eq!(x, Accion::Congelar);
        let (a, x) = juzga(
            "explorer.exe",
            r"C:\Windows\explorer.exe",
            r"C:\Users\ana\Desktop\~~~~GUARDIANA-senuelo-no-tocar.pdf",
            Toque::Borrado,
            7,
        );
        assert_eq!(x, Accion::Avisar);
        assert!(a.texto.contains("Explorador"));
        // a program that only calls itself explorer.exe, elsewhere, is frozen
        let (_, x) = juzga(
            "explorer.exe",
            r"C:\Users\ana\AppData\Roaming\explorer.exe",
            r"C:\Users\ana\Desktop\~~~~GUARDIANA-senuelo-no-tocar.pdf",
            Toque::Borrado,
            7,
        );
        assert_eq!(x, Accion::Congelar);
    }
}
