//! «Cadenas» (phase A3; the owner, 1 Nov 2026, 18:55: «que vea todo lo posible»): which program started which. Many
//! attacks are visible in that alone, before any file is known as malicious:
//!
//! - an Office program (a document with macros) starting a command interpreter or a script host;
//! - a PDF reader or a mail program starting a command interpreter, a script host or one of Windows' own tools;
//! - an archive manager starting a script host or a command interpreter (a script opened from inside a compressed file);
//! - a browser starting a command interpreter or a script host;
//! - a script host starting another interpreter or Windows' own tools to load code;
//! - a program downloaded from the internet, or run from a folder anyone can write to, starting the tools that delete
//!   the backups and the restore points (what ransomware does just before encrypting).
//!
//! Windows says every program start (its kernel process event channel); [`vigilado`] is the cheap test for the
//! programs that matter here, and [`juzga`] turns a start into a warning.

use crate::primera_vez::{carpeta_escribible, Aviso};

const OFFICE: &[(&str, &str)] = &[
    ("winword.exe", "Word"),
    ("excel.exe", "Excel"),
    ("powerpnt.exe", "PowerPoint"),
    ("outlook.exe", "Outlook"),
    ("msaccess.exe", "Access"),
    ("mspub.exe", "Publisher"),
    ("onenote.exe", "OneNote"),
    ("visio.exe", "Visio"),
];

/// PDF readers and mail programs: they open what arrives attached.
const LECTORES: &[(&str, &str)] = &[
    ("acrord32.exe", "Adobe Acrobat Reader"),
    ("acrobat.exe", "Adobe Acrobat"),
    ("foxitpdfreader.exe", "Foxit PDF Reader"),
    ("foxitreader.exe", "Foxit Reader"),
    ("sumatrapdf.exe", "SumatraPDF"),
    ("thunderbird.exe", "Thunderbird"),
];

/// Archive managers: a script opened from inside a compressed file starts from them.
const COMPRIMIDOS: &[(&str, &str)] = &[
    ("winrar.exe", "WinRAR"),
    ("7zfm.exe", "7-Zip"),
    ("7zg.exe", "7-Zip"),
    ("bandizip.exe", "Bandizip"),
    ("peazip.exe", "PeaZip"),
];

const NAVEGADORES: &[(&str, &str)] = &[
    ("chrome.exe", "Chrome"),
    ("msedge.exe", "Edge"),
    ("firefox.exe", "Firefox"),
    ("brave.exe", "Brave"),
    ("opera.exe", "Opera"),
    ("vivaldi.exe", "Vivaldi"),
    ("iexplore.exe", "Internet Explorer"),
];

/// Script hosts and command interpreters.
const INTERPRETES: &[(&str, &str)] = &[
    ("powershell.exe", "PowerShell"),
    ("pwsh.exe", "PowerShell"),
    ("cmd.exe", "la consola de órdenes"),
    ("wscript.exe", "Windows Script Host"),
    ("cscript.exe", "Windows Script Host"),
    ("mshta.exe", "MSHTA (aplicaciones HTML)"),
];

/// Windows' own tools that attackers use to load or fetch code («living off the land»; the LOLBAS project).
const HERRAMIENTAS: &[(&str, &str)] = &[
    ("rundll32.exe", "rundll32"),
    ("regsvr32.exe", "regsvr32"),
    ("certutil.exe", "certutil"),
    ("bitsadmin.exe", "bitsadmin"),
    ("msbuild.exe", "MSBuild"),
    ("installutil.exe", "InstallUtil"),
    ("wmic.exe", "WMIC"),
    ("schtasks.exe", "el programador de tareas"),
];

/// The tools that delete backups and restore points.
const COPIAS: &[(&str, &str)] = &[
    (
        "vssadmin.exe",
        "vssadmin (las copias de seguridad de Windows)",
    ),
    (
        "wbadmin.exe",
        "wbadmin (las copias de seguridad de Windows)",
    ),
    (
        "bcdedit.exe",
        "bcdedit (el arranque y la recuperación de Windows)",
    ),
];

fn nombre_en(nombre: &str, tabla: &[(&'static str, &'static str)]) -> Option<&'static str> {
    let n = nombre.to_ascii_lowercase();
    tabla
        .iter()
        .find(|(e, _)| *e == n)
        .map(|&(_, visible)| visible)
}

/// Whether a started program is one of those this rule looks at: the cheap test, run for every program start.
#[must_use]
pub fn vigilado(hijo: &str) -> bool {
    nombre_en(hijo, INTERPRETES).is_some()
        || nombre_en(hijo, HERRAMIENTAS).is_some()
        || nombre_en(hijo, COPIAS).is_some()
}

/// A program start, as a warning when it is one of the chains in the module notes. `padre_descargado` says whether
/// Windows marked the parent's file as come from the internet. The second value says whether it is grave (the
/// backups' tools, or a document starting an interpreter).
#[must_use]
pub fn juzga(
    padre: &str,
    padre_ruta: &str,
    padre_descargado: bool,
    hijo: &str,
    cuando_ms: u64,
) -> Option<(Aviso, bool)> {
    let aviso = |regla: &'static str, texto: String| Aviso {
        regla,
        programa: padre.to_string(),
        ruta: padre_ruta.to_string(),
        sitio: String::new(),
        nombre: hijo.to_ascii_lowercase(),
        cuando_ms,
        fuente: None,
        texto,
    };
    if let Some(copias) = nombre_en(hijo, COPIAS) {
        let lugar = if padre_descargado {
            Some("descargado de internet".to_string())
        } else {
            carpeta_escribible(padre_ruta).map(|c| format!("desde {c}"))
        };
        let lugar = lugar?;
        let texto = format!(
            "«{padre}», {lugar}, ha abierto {copias}. Es lo que hacen los programas de secuestro de archivos (ransomware) \
             justo antes de encriptar: borrar las copias para que no puedas recuperarlos."
        );
        return Some((aviso("copias", texto), true));
    }
    let lanzado = nombre_en(hijo, INTERPRETES).or_else(|| nombre_en(hijo, HERRAMIENTAS))?;
    if let Some(office) = nombre_en(padre, OFFICE) {
        let texto = format!(
            "{office} ha abierto {lanzado}. Un documento no necesita hacerlo: así empiezan muchos ataques con documentos \
             (macros). Si no lo esperabas, no habilites el contenido y cierra el documento."
        );
        return Some((aviso("documento", texto), true));
    }
    if let Some(lector) = nombre_en(padre, LECTORES) {
        let texto = format!(
            "{lector} ha abierto {lanzado}. Un lector de PDF o de correo no necesita hacerlo: así empiezan muchos ataques \
             con archivos adjuntos. Si no lo esperabas, ciérralo y no vuelvas a abrir ese archivo."
        );
        return Some((aviso("lector", texto), true));
    }
    if let Some(comprimido) = nombre_en(padre, COMPRIMIDOS) {
        if nombre_en(hijo, INTERPRETES).is_some() {
            let texto = format!(
                "{comprimido} ha abierto {lanzado} desde un archivo comprimido. Es una forma habitual de colar un guion \
                 dañino. Si no lo esperabas, ciérralo y borra ese archivo comprimido."
            );
            return Some((aviso("comprimido", texto), false));
        }
        return None;
    }
    if let Some(navegador) = nombre_en(padre, NAVEGADORES) {
        if nombre_en(hijo, INTERPRETES).is_some() {
            let texto = format!(
                "{navegador} ha abierto {lanzado}. Un navegador casi nunca lo hace; algunas webs engañan para que se ejecute \
                 algo. Si no lo esperabas, ciérralo."
            );
            return Some((aviso("navegador", texto), false));
        }
        return None;
    }
    let script = matches!(
        padre.to_ascii_lowercase().as_str(),
        "wscript.exe" | "cscript.exe" | "mshta.exe"
    );
    if script && !hijo.eq_ignore_ascii_case(padre) {
        let texto = format!(
            "Un guion ({}) ha abierto {lanzado}: es una cadena típica para descargar y ejecutar código.",
            nombre_en(padre, INTERPRETES).unwrap_or(padre)
        );
        return Some((aviso("guion", texto), false));
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn solo_mira_los_programas_que_importan() {
        assert!(vigilado("PowerShell.exe"));
        assert!(vigilado("vssadmin.exe"));
        assert!(vigilado("rundll32.exe"));
        assert!(!vigilado("notepad.exe"));
        assert!(!vigilado("chrome.exe"));
    }

    #[test]
    fn un_documento_que_abre_powershell() {
        let (a, grave) = juzga(
            "WINWORD.EXE",
            r"C:\Program Files\Microsoft Office\root\Office16\WINWORD.EXE",
            false,
            "powershell.exe",
            1,
        )
        .unwrap();
        assert!(grave);
        assert_eq!(a.regla, "documento");
        assert!(
            a.texto.starts_with("Word ha abierto PowerShell"),
            "{}",
            a.texto
        );
    }

    #[test]
    fn el_navegador_que_abre_la_consola() {
        let (a, grave) = juzga(
            "chrome.exe",
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            false,
            "cmd.exe",
            1,
        )
        .unwrap();
        assert!(!grave);
        assert_eq!(a.regla, "navegador");
        // las herramientas, desde el navegador, no (las usa al instalar o actualizar)
        assert_eq!(
            juzga("chrome.exe", r"C:\x\chrome.exe", false, "rundll32.exe", 1),
            None
        );
    }

    #[test]
    fn borrar_las_copias_desde_un_programa_descargado() {
        let (a, grave) = juzga("factura.exe", r"D:\factura.exe", true, "vssadmin.exe", 1).unwrap();
        assert!(grave);
        assert_eq!(a.regla, "copias");
        let (a, _) = juzga(
            "x.exe",
            r"C:\Users\ana\Downloads\x.exe",
            false,
            "bcdedit.exe",
            1,
        )
        .unwrap();
        assert!(
            a.texto.contains("desde la carpeta de Descargas"),
            "{}",
            a.texto
        );
        // un programa instalado que usa vssadmin (una copia de seguridad de verdad): no
        assert_eq!(
            juzga(
                "backup.exe",
                r"C:\Program Files\Backup\backup.exe",
                false,
                "vssadmin.exe",
                1
            ),
            None
        );
    }

    #[test]
    fn un_pdf_o_un_comprimido_que_abre_un_interprete() {
        let (a, grave) = juzga(
            "AcroRd32.exe",
            r"C:\Program Files\Adobe\AcroRd32.exe",
            false,
            "cmd.exe",
            1,
        )
        .unwrap();
        assert!(grave);
        assert_eq!(a.regla, "lector");
        assert!(
            a.texto
                .starts_with("Adobe Acrobat Reader ha abierto la consola"),
            "{}",
            a.texto
        );
        let (a, grave) = juzga(
            "WinRAR.exe",
            r"C:\Program Files\WinRAR\WinRAR.exe",
            false,
            "wscript.exe",
            1,
        )
        .unwrap();
        assert!(!grave);
        assert_eq!(a.regla, "comprimido");
        // un instalador de herramientas desde el comprimido, no
        assert_eq!(
            juzga(
                "7zFM.exe",
                r"C:\Program Files\7-Zip\7zFM.exe",
                false,
                "msbuild.exe",
                1
            ),
            None
        );
    }

    #[test]
    fn un_guion_que_abre_otro_interprete() {
        let (a, _) = juzga(
            "wscript.exe",
            r"C:\Windows\System32\wscript.exe",
            false,
            "powershell.exe",
            1,
        )
        .unwrap();
        assert_eq!(a.regla, "guion");
        assert_eq!(
            juzga(
                "explorer.exe",
                r"C:\Windows\explorer.exe",
                false,
                "powershell.exe",
                1
            ),
            None
        );
    }
}
