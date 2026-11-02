//! «Secuestro» (phase A3; the owner, 1 Nov 2026, 18:55: «que vea todo lo posible»): one program renaming or deleting
//! many of the person's own files (documents, desktop, pictures, music, videos, downloads, OneDrive) in under a
//! minute, which is what file-encrypting extortion programs (ransomware) do.
//!
//! Windows says every rename and delete by path (its kernel file event channel); [`es_personal`] is the cheap test,
//! [`Contador`] counts per program and [`juzga`] turns a burst into a warning. A program that writes over the files in
//! place, without renaming or deleting any, is not seen by this rule.

use std::collections::HashMap;

use crate::primera_vez::{carpeta_escribible, Aviso, Firma};

/// The person's files renamed or deleted by one program within [`VENTANA_MS`] that make a burst.
pub const UMBRAL: usize = 100;
/// The window.
pub const VENTANA_MS: u64 = 60_000;

/// The folders of a profile that hold the person's own files.
const PERSONALES: &[&str] = &[
    "\\documents\\",
    "\\desktop\\",
    "\\pictures\\",
    "\\music\\",
    "\\videos\\",
    "\\downloads\\",
    "\\onedrive\\",
    "\\onedrive - ",
];

/// Interpreters: signed by Microsoft, but a script they run can do the same as any program; judged even when signed.
const INTERPRETES: &[&str] = &[
    "powershell.exe",
    "pwsh.exe",
    "wscript.exe",
    "cscript.exe",
    "mshta.exe",
    "cmd.exe",
];

/// Whether a path is one of the person's own files: in a profile's documents, desktop, pictures, music, videos,
/// downloads or OneDrive folder, and not in a program's data folder (`AppData`).
#[must_use]
pub fn es_personal(ruta: &str) -> bool {
    let r = ruta.replace('/', "\\").to_ascii_lowercase();
    let Some(i) = r.find("\\users\\") else {
        return false;
    };
    let resto = r.get(i..).unwrap_or_default();
    !resto.contains("\\appdata\\") && PERSONALES.iter().any(|p| resto.contains(p))
}

/// Counts, per program (its process number), the person's files it renamed or deleted in its current window.
#[derive(Debug, Default)]
pub struct Contador {
    /// Process number → (when its window started, how many files, whether this window already said so).
    por: HashMap<u32, (u64, usize, bool)>,
}

impl Contador {
    /// One more file for `pid` at `cuando_ms`. `true` once per window: when it reaches [`UMBRAL`].
    pub fn anota(&mut self, pid: u32, cuando_ms: u64) -> bool {
        if self.por.len() > 4_096 {
            self.por
                .retain(|_, (inicio, _, _)| cuando_ms.saturating_sub(*inicio) < VENTANA_MS);
        }
        let e = self.por.entry(pid).or_insert((cuando_ms, 0, false));
        if cuando_ms.saturating_sub(e.0) >= VENTANA_MS {
            *e = (cuando_ms, 0, false);
        }
        e.1 += 1;
        if e.1 >= UMBRAL && !e.2 {
            e.2 = true;
            return true;
        }
        false
    }
}

/// A burst as a warning: `programa` (its file name) at `ruta` renamed or deleted at least `n` of the person's files
/// in under a minute. A signed program is not judged (a sync client, a backup, a photo manager), except the
/// interpreters. The second value says whether it is grave: no signature, come from the internet (`descargado`) or
/// run from a folder anyone can write to.
#[must_use]
pub fn juzga(
    programa: &str,
    ruta: &str,
    firma: &Firma,
    descargado: bool,
    n: usize,
    cuando_ms: u64,
) -> Option<(Aviso, bool)> {
    let interprete = INTERPRETES.iter().any(|i| programa.eq_ignore_ascii_case(i));
    if matches!(firma, Firma::Firmado(_)) && !interprete {
        return None;
    }
    let lugar = carpeta_escribible(ruta);
    let grave = matches!(firma, Firma::SinFirma) || descargado || lugar.is_some();
    let texto = format!(
        "«{programa}» ha renombrado o borrado al menos {n} archivos tuyos (documentos, fotos, escritorio…) en menos de \
         un minuto. Es lo que hacen los programas de secuestro de archivos (ransomware) al encriptarlos. Si no lo \
         esperabas, desconecta el equipo de internet y de los discos externos, y cierra ese programa."
    );
    Some((
        Aviso {
            regla: "secuestro",
            programa: programa.to_string(),
            ruta: ruta.to_string(),
            sitio: String::new(),
            nombre: programa.to_ascii_lowercase(),
            cuando_ms,
            fuente: None,
            texto,
        },
        grave,
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn solo_los_archivos_de_la_persona() {
        assert!(es_personal(
            r"\Device\HarddiskVolume3\Users\ana\Documents\factura.docx"
        ));
        assert!(es_personal(r"C:\Users\ana\OneDrive - Empresa\x.xlsx"));
        assert!(es_personal(r"C:\Users\ana\Pictures\2026\foto.jpg"));
        assert!(!es_personal(
            r"C:\Users\ana\AppData\Local\Temp\Documents\x.tmp"
        ));
        assert!(!es_personal(r"C:\Windows\Temp\x.tmp"));
        assert!(!es_personal(r"C:\Program Files\App\Documents\x.dat"));
    }

    #[test]
    fn cuenta_por_programa_y_ventana() {
        let mut c = Contador::default();
        for i in 0..(UMBRAL - 1) {
            assert!(!c.anota(7, i as u64));
            assert!(!c.anota(8, i as u64));
        }
        assert!(c.anota(7, 500));
        // una vez por ventana
        assert!(!c.anota(7, 600));
        // otra ventana: empieza de cero
        assert!(!c.anota(7, VENTANA_MS + 1_000));
    }

    #[test]
    fn juzga_las_rafagas() {
        let (a, grave) = juzga(
            "x.exe",
            r"C:\Users\ana\Downloads\x.exe",
            &Firma::SinFirma,
            true,
            100,
            1,
        )
        .unwrap();
        assert!(grave);
        assert_eq!(a.regla, "secuestro");
        assert!(a.texto.contains("al menos 100"), "{}", a.texto);
        // firmado: un programa de sincronización o de copias; no
        assert_eq!(
            juzga(
                "OneDrive.exe",
                r"C:\Program Files\Microsoft OneDrive\OneDrive.exe",
                &Firma::Firmado("Microsoft".into()),
                false,
                100,
                1
            ),
            None
        );
        // un intérprete firmado sí, sin ser grave
        let (_, grave) = juzga(
            "powershell.exe",
            r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
            &Firma::Firmado("Microsoft".into()),
            false,
            100,
            1,
        )
        .unwrap();
        assert!(!grave);
    }
}
