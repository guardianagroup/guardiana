//! «Disfraz» (phase A3; the owner, 1 Nov 2026, 18:55: «que vea todo lo posible»): a program that takes the name of one
//! of Windows' own (`svchost.exe`, `lsass.exe`, `csrss.exe`…) but runs from anywhere else than Windows' folder. Windows'
//! own programs live only there; a copy elsewhere is a disguise, one of the oldest ways of hiding in the list of
//! processes.
//!
//! The service sees every program start (Windows' kernel process events, the same watch as [`crate::cadenas`]);
//! [`sospechoso`] is the cheap test, run for each, and [`juzga`] turns a start into a grave warning.

use crate::primera_vez::Aviso;

/// Windows' own programs, which live only in Windows' folder.
const DE_WINDOWS: &[&str] = &[
    "svchost.exe",
    "lsass.exe",
    "csrss.exe",
    "smss.exe",
    "wininit.exe",
    "winlogon.exe",
    "services.exe",
    "spoolsv.exe",
    "lsm.exe",
    "taskhost.exe",
    "taskhostw.exe",
    "dllhost.exe",
    "conhost.exe",
    "explorer.exe",
    "rundll32.exe",
    "regsvr32.exe",
    "dwm.exe",
    "sihost.exe",
    "ctfmon.exe",
    "searchindexer.exe",
    "wuauclt.exe",
    "audiodg.exe",
    "fontdrvhost.exe",
    "runtimebroker.exe",
];

/// The path after its volume: `C:\x` → `\x`; `\Device\HarddiskVolume3\x` → `\x`; `\\?\C:\x` → `\x`. Lower case.
fn sin_volumen(ruta: &str) -> String {
    let r = ruta.replace('/', "\\").to_lowercase();
    let r = r.strip_prefix(r"\\?\").unwrap_or(&r).to_owned();
    if let Some(resto) = r.strip_prefix(r"\device\") {
        // \device\harddiskvolume3\windows\… → \windows\…
        return resto
            .find('\\')
            .map_or_else(String::new, |i| resto[i..].to_owned());
    }
    match r.get(1..3) {
        Some(":\\") => r[2..].to_owned(),
        _ => r,
    }
}

/// Whether a started program, by its file name and path, wears the name of one of Windows' own programs outside
/// Windows' folder. A path that cannot be read says nothing.
#[must_use]
pub fn sospechoso(nombre: &str, ruta: &str) -> bool {
    let n = nombre.to_ascii_lowercase();
    if ruta.trim().is_empty() || !DE_WINDOWS.contains(&n.as_str()) {
        return false;
    }
    !sin_volumen(ruta).starts_with(r"\windows\")
}

/// A disguised start as a grave warning.
#[must_use]
pub fn juzga(nombre: &str, ruta: &str, cuando_ms: u64) -> Option<(Aviso, bool)> {
    if !sospechoso(nombre, ruta) {
        return None;
    }
    let texto = format!(
        "Un programa se llama «{nombre}», como uno de Windows, pero no está en la carpeta de Windows, el único sitio \
         donde está el de verdad. Es una forma clásica de esconderse entre los procesos del sistema. Si no lo \
         conoces, analízalo y no lo dejes en marcha."
    );
    Some((
        Aviso {
            regla: "disfraz",
            programa: nombre.to_owned(),
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
    fn los_de_windows_en_su_sitio() {
        assert!(!sospechoso(
            "svchost.exe",
            r"C:\Windows\System32\svchost.exe"
        ));
        assert!(!sospechoso(
            "SVCHOST.EXE",
            r"\Device\HarddiskVolume3\Windows\System32\svchost.exe"
        ));
        assert!(!sospechoso("explorer.exe", r"C:\Windows\explorer.exe"));
        assert!(!sospechoso(
            "rundll32.exe",
            r"\\?\C:\Windows\SysWOW64\rundll32.exe"
        ));
        // un programa cualquiera, o sin ruta: nada
        assert!(!sospechoso(
            "chrome.exe",
            r"C:\Users\ana\AppData\Local\Temp\chrome.exe"
        ));
        assert!(!sospechoso("svchost.exe", ""));
    }

    #[test]
    fn el_disfraz() {
        assert!(sospechoso(
            "svchost.exe",
            r"C:\Users\ana\AppData\Roaming\svchost.exe"
        ));
        assert!(sospechoso(
            "lsass.exe",
            r"\Device\HarddiskVolume3\ProgramData\lsass.exe"
        ));
        // una carpeta que se llama Windows dentro de otra no es la de Windows
        assert!(sospechoso(
            "csrss.exe",
            r"C:\Users\ana\Windows\System32\csrss.exe"
        ));
        let (a, grave) = juzga(
            "svchost.exe",
            r"C:\Users\ana\AppData\Roaming\svchost.exe",
            1,
        )
        .unwrap();
        assert!(grave);
        assert_eq!(a.regla, "disfraz");
        assert_eq!(
            juzga("svchost.exe", r"C:\Windows\System32\svchost.exe", 1),
            None
        );
    }
}
