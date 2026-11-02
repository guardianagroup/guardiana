//! «Protección» (phase A3; the owner, 1 Nov 2026, 18:55: «que vea todo lo posible»): Windows' own protection switched
//! off. Turning off Microsoft Defender's real-time protection, its tamper protection or the firewall is among the first
//! things many harmful programs do. The continuous watch already notes every change of Windows Security
//! (`guardiana_apps::vigilancia`, «Protección» in the panel); this rule turns the ones an antivirus should point at into
//! warnings: grave when the protection goes off, a plain warning when it was already off at the watch's first look.
//!
//! And Microsoft Defender's exclusions: the watch reads them (the service can) and notes each new one. Adding an
//! exclusion, so the antivirus does not look at a folder, a program or a kind of file, is what many harmful programs do
//! to hide. A new exclusion always warns, grave when it leaves out much ([`amplia`]); one already there at the first
//! look warns only when it is that wide.
//!
//! And the event logs emptied: Windows records it itself (in the Security log, when that one is emptied; in the System
//! log, for any other), and the watch reads when. Emptying the Security log, where Windows notes sign-ins and changes of
//! rights, is how an intruder wipes the trail: grave. Another log: a warning (cleaning programs do it too).

use crate::primera_vez::Aviso;

/// The name shown where a warning names a program.
const QUIEN: &str = "Seguridad de Windows";

/// Programs that run whatever they are given: an exclusion of one leaves out everything it runs.
const INTERPRETES: &[&str] = &[
    "powershell.exe",
    "pwsh.exe",
    "cmd.exe",
    "wscript.exe",
    "cscript.exe",
    "mshta.exe",
    "rundll32.exe",
    "regsvr32.exe",
    "msiexec.exe",
    "explorer.exe",
    "svchost.exe",
    "schtasks.exe",
    "certutil.exe",
    "bitsadmin.exe",
];

/// Kinds of file that run, or carry, programs.
const EJECUTABLES: &[&str] = &[
    "exe", "dll", "scr", "com", "pif", "bat", "cmd", "ps1", "psm1", "vbs", "vbe", "js", "jse",
    "wsf", "wsh", "hta", "msi", "msp", "lnk", "sys", "cpl", "ocx", "jar", "iso", "img", "vhd",
    "vhdx", "zip", "rar", "7z", "docm", "xlsm", "pptm",
];

/// Folders where what comes from the internet lands, or anyone can write (their names on disk, also as OneDrive
/// names them in Spanish and Portuguese).
const SITIOS: &[&str] = &[
    "downloads",
    "descargas",
    "transferências",
    "desktop",
    "escritorio",
    "área de trabalho",
    "appdata",
    "temp",
    "tmp",
];

/// The same places as Windows' variables.
const VARIABLES: &[&str] = &[
    "%temp%",
    "%tmp%",
    "%appdata%",
    "%localappdata%",
    "%userprofile%",
    "%public%",
];

/// What an exclusion of Microsoft Defender (`ruta:…`, `proceso:…` or `extension:…`, as the continuous watch reads
/// them) leaves out, in words.
#[must_use]
pub fn que_excluye(x: &str) -> String {
    match x.split_once(':') {
        Some(("ruta", r)) => format!("la carpeta o el archivo «{r}»"),
        Some(("proceso", p)) => format!("lo que abre el programa «{p}»"),
        Some(("extension", e)) => format!("los archivos «.{}»", e.trim_start_matches('.')),
        _ => format!("«{x}»"),
    }
}

/// Whether a lower-case path (with `\`) goes through one of [`SITIOS`] or [`VARIABLES`], or `Users\Public`.
fn en_sitio(v: &str) -> bool {
    let partes: Vec<&str> = v.split('\\').collect();
    partes.iter().any(|p| SITIOS.contains(p))
        || partes.windows(2).any(|w| w == ["users", "public"])
        || VARIABLES.iter().any(|x| v.contains(x))
}

/// Whether an exclusion leaves out much: a whole drive, a folder of the system or a person's whole folder, a place
/// where downloads land or anyone can write, a program that runs others, a kind of file that runs, or a wildcard.
#[must_use]
pub fn amplia(x: &str) -> bool {
    let Some((tipo, valor)) = x.split_once(':') else {
        return false;
    };
    let v = valor.trim().replace('/', "\\").to_lowercase();
    if v.contains('*') {
        return true;
    }
    match tipo {
        "extension" => EJECUTABLES.contains(&v.trim_start_matches('.')),
        "proceso" => {
            let nombre = v.rsplit('\\').next().unwrap_or_default();
            INTERPRETES.contains(&nombre) || en_sitio(&v)
        }
        "ruta" => {
            let partes: Vec<&str> = v
                .trim_start_matches(r"\\?\")
                .split('\\')
                .filter(|p| !p.is_empty())
                .collect();
            let unidad = partes.len() <= 1;
            let del_sistema = partes.len() == 2
                && matches!(
                    partes[1],
                    "users" | "windows" | "program files" | "program files (x86)" | "programdata"
                );
            let de_una_persona = partes.len() == 3 && partes[1] == "users";
            unidad || del_sistema || de_una_persona || en_sitio(&v)
        }
        _ => false,
    }
}

/// A change the continuous watch noted (its sentence key `frase`, the thing it is about `x`, whether it is now on
/// `activo`, what identifies it `clave`) as a warning, when it is protection going off. The second value says whether
/// it is grave.
#[must_use]
pub fn juzga(
    frase: &str,
    x: &str,
    activo: Option<bool>,
    clave: &str,
    cuando_ms: u64,
) -> Option<(Aviso, bool)> {
    let (texto, grave) = match frase {
        "vg_ev_tiempo_real_apagado" => (
            "La protección en tiempo real de Microsoft Defender se ha apagado. Si no lo hiciste tú, vuelve a encenderla \
             en Seguridad de Windows: apagarla es de lo primero que hacen muchos programas dañinos."
                .to_owned(),
            true,
        ),
        "vg_ev_sin_antivirus" => (
            "Ningún antivirus tiene la protección encendida en este equipo. Si no lo esperabas, enciende la de \
             Microsoft Defender en Seguridad de Windows."
                .to_owned(),
            true,
        ),
        "vg_ev_prot_cambio"
            if activo == Some(false) && x.to_lowercase().contains("contra alteraciones") =>
        {
            (
                "La protección contra alteraciones de Microsoft Defender se ha apagado. Si no lo hiciste tú, vuelve a \
                 encenderla en Seguridad de Windows: es la que impide que otro programa apague el antivirus."
                    .to_owned(),
                true,
            )
        }
        "vg_ev_cortafuegos_apagado" => (
            format!(
                "El cortafuegos de Windows se ha apagado ({}). Si no lo hiciste tú, vuelve a encenderlo en Seguridad \
                 de Windows.",
                if x.is_empty() { "un perfil" } else { x }
            ),
            false,
        ),
        "vg_ev_tiempo_real_ya" => (
            "La protección en tiempo real de Microsoft Defender está apagada. Enciéndela en Seguridad de Windows si no \
             usas otro antivirus."
                .to_owned(),
            false,
        ),
        "vg_ev_exclusion_nueva" => {
            let grave = amplia(x);
            (
                format!(
                    "Microsoft Defender tiene una exclusión nueva: ya no analiza {}. {}Si no la añadiste tú, quítala en \
                     Seguridad de Windows, en las exclusiones de la protección antivirus.",
                    que_excluye(x),
                    if grave {
                        "Es de las que añaden los programas dañinos para que el antivirus no los vea. "
                    } else {
                        ""
                    }
                ),
                grave,
            )
        }
        "vg_ev_exclusion_ya" if amplia(x) => (
            format!(
                "Microsoft Defender tiene una exclusión muy amplia: no analiza {}. Si no la necesitas, quítala en \
                 Seguridad de Windows, en las exclusiones de la protección antivirus.",
                que_excluye(x)
            ),
            false,
        ),
        "vg_ev_vaciado_seguridad" => (
            "Alguien ha vaciado el registro de seguridad de Windows, donde Windows anota los inicios de sesión y los \
             cambios de permisos. Vaciarlo es de lo que hace un intruso para borrar su rastro. Si no fuiste tú, \
             analiza el equipo."
                .to_owned(),
            true,
        ),
        "vg_ev_vaciado_otro" => (
            "Alguien ha vaciado un registro de eventos de Windows. Lo hacen algunos programas de limpieza, y también \
             quien quiere borrar su rastro. Si no fuiste tú ni un programa tuyo, analiza el equipo."
                .to_owned(),
            false,
        ),
        "vg_ev_sin_antivirus_ya" => (
            "Ningún antivirus tiene la protección encendida en este equipo. Enciende la de Microsoft Defender en \
             Seguridad de Windows si no usas otro."
                .to_owned(),
            false,
        ),
        _ => return None,
    };
    Some((
        Aviso {
            regla: "proteccion",
            programa: QUIEN.to_owned(),
            ruta: String::new(),
            sitio: String::new(),
            nombre: clave.to_owned(),
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
    fn apagar_la_proteccion_es_grave() {
        let (a, grave) = juzga(
            "vg_ev_tiempo_real_apagado",
            "",
            None,
            "tiempo_real|apagada|1",
            1,
        )
        .unwrap();
        assert!(grave);
        assert_eq!(a.regla, "proteccion");
        assert_eq!(a.nombre, "tiempo_real|apagada|1");
        let (_, grave) = juzga(
            "vg_ev_prot_cambio",
            "Protección contra alteraciones de Microsoft Defender",
            Some(false),
            "k",
            1,
        )
        .unwrap();
        assert!(grave);
    }

    #[test]
    fn que_exclusion_deja_fuera_mucho() {
        for x in [
            r"ruta:C:\",
            "ruta:D:",
            r"ruta:C:\Users",
            r"ruta:C:\Windows",
            r"ruta:C:\Program Files (x86)",
            r"ruta:C:\Users\ana",
            r"ruta:C:\Users\ana\Downloads",
            r"ruta:C:\Users\ana\AppData\Roaming\Actualizador",
            r"ruta:C:\Users\ana\OneDrive\Escritorio\x",
            r"ruta:C:\Temp",
            r"ruta:C:\Users\Public\x",
            r"ruta:%TEMP%\x",
            r"ruta:C:\*",
            "proceso:powershell.exe",
            r"proceso:C:\Windows\System32\WindowsPowerShell\v1.0\PowerShell.exe",
            r"proceso:C:\Users\ana\AppData\Local\x\x.exe",
            "extension:exe",
            "extension:.PS1",
            "extension:*",
        ] {
            assert!(amplia(x), "{x}");
        }
        for x in [
            r"ruta:C:\Juegos",
            r"ruta:D:\Proyectos\compilaciones",
            r"ruta:C:\ProgramData\Copias\cache",
            r"ruta:C:\Users\ana\Documents\Templates",
            r"proceso:C:\Program Files\Juego\juego.exe",
            "proceso:juego.exe",
            "extension:log",
            "extension:.vmdk",
            "otra cosa",
        ] {
            assert!(!amplia(x), "{x}");
        }
        assert_eq!(que_excluye("extension:exe"), "los archivos «.exe»");
    }

    #[test]
    fn una_exclusion_nueva_avisa_y_si_es_amplia_es_grave() {
        let (a, grave) = juzga(
            "vg_ev_exclusion_nueva",
            r"ruta:C:\Users\ana\AppData\Roaming\Actualizador",
            None,
            "exclusion|x|1",
            1,
        )
        .unwrap();
        assert!(grave);
        assert_eq!(
            (a.regla, a.nombre.as_str()),
            ("proteccion", "exclusion|x|1")
        );
        assert!(a.texto.contains("ya no analiza la carpeta o el archivo «C:\\Users\\ana\\AppData\\Roaming\\Actualizador»"), "{}", a.texto);
        let (a, grave) = juzga("vg_ev_exclusion_nueva", r"ruta:C:\Juegos", None, "k", 1).unwrap();
        assert!(!grave);
        assert!(!a.texto.contains("programas dañinos"), "{}", a.texto);
        // one already there at the first look: only when it is wide
        let (_, grave) = juzga("vg_ev_exclusion_ya", "extension:exe", None, "k", 1).unwrap();
        assert!(!grave);
        assert_eq!(
            juzga("vg_ev_exclusion_ya", r"ruta:C:\Juegos", None, "k", 1),
            None
        );
        // one removed: nothing
        assert_eq!(
            juzga("vg_ev_exclusion_quitada", "extension:exe", None, "k", 1),
            None
        );
    }

    #[test]
    fn vaciar_el_registro_de_seguridad_es_grave() {
        let (a, grave) = juzga(
            "vg_ev_vaciado_seguridad",
            "seguridad",
            None,
            "vaciado|seguridad|5",
            5,
        )
        .unwrap();
        assert!(grave);
        assert_eq!(a.nombre, "vaciado|seguridad|5");
        let (_, grave) = juzga("vg_ev_vaciado_otro", "otro", None, "k", 5).unwrap();
        assert!(!grave);
    }

    #[test]
    fn lo_demas_no_o_sin_gravedad() {
        // encenderla, o cambiar otra cosa: no
        assert_eq!(
            juzga(
                "vg_ev_prot_cambio",
                "Protección contra alteraciones de Microsoft Defender",
                Some(true),
                "k",
                1
            ),
            None
        );
        assert_eq!(
            juzga("vg_ev_prot_cambio", "Avast", Some(false), "k", 1),
            None
        );
        assert_eq!(juzga("vg_ev_arranque_nuevo", "", None, "k", 1), None);
        // el cortafuegos, o lo que ya estaba apagado al empezar: aviso, no grave
        let (a, grave) =
            juzga("vg_ev_cortafuegos_apagado", "perfil Público", None, "k", 1).unwrap();
        assert!(!grave);
        assert!(a.texto.contains("(perfil Público)"), "{}", a.texto);
        assert!(!juzga("vg_ev_tiempo_real_ya", "", None, "k", 1).unwrap().1);
    }
}
