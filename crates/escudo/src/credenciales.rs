//! «Credenciales» (phase A3; the owner, 1 Nov 2026, 18:55: «que vea todo lo posible, todo, absolutamente todo»): a
//! program that opens the memory of LSASS, the Windows process that keeps the credentials of the accounts signed in,
//! with the right to read it. Reading that memory is how passwords and sign-in tokens are stolen from a computer.
//!
//! The service sees every opening of a process with Windows' own kernel audit channel
//! (`Microsoft-Windows-Kernel-Audit-API-Calls`, the same session as [`crate::cadenas`]) and keeps those of LSASS that
//! ask to read its memory. Windows' own programs (in Windows' folder) and those installed in Program Files (security
//! and backup programs) are not judged; a program that runs others (PowerShell, the console, `rundll32`) or a tool
//! that copies a process's memory is, wherever it is, because that is how it is done with Windows' own programs.
//! Grave, also when Windows refused (LSA protection): the attempt is the sign.
//!
//! A program with a valid signature is not judged either (1.0.58, 10 Nov 2026). Many ordinary programs installed in the
//! person's own folder list the processes of the computer asking every one of them for the usual rights, reading
//! memory among them (`PROCESS_QUERY_INFORMATION | PROCESS_VM_READ`), and LSASS is one more in that list: Microsoft
//! OneDrive does it every hour from `AppData\Local\Microsoft\OneDrive`. Until the 1.0.57 that was a grave warning
//! every hour, with the button that puts the program in the quarantine, on any computer with OneDrive (seen on the
//! owner's: 62 warnings in three days, all of them OneDrive). The rights asked for do not tell the two apart (the
//! tools that steal credentials ask for the same ones), so what does is who signs the file: the warning stays for a
//! program with no valid signature, or whose signature Windows has not been able to check, and for the tools above,
//! signed or not.

use crate::primera_vez::{Aviso, Firma};

/// `PROCESS_VM_READ`: the right to read another process's memory.
pub const LEE_MEMORIA: u32 = 0x0010;

/// Programs that run others, and tools that copy a process's memory: judged even when they are Windows' own.
const HERRAMIENTAS: &[&str] = &[
    "rundll32.exe",
    "powershell.exe",
    "pwsh.exe",
    "powershell_ise.exe",
    "cmd.exe",
    "wscript.exe",
    "cscript.exe",
    "mshta.exe",
    "regsvr32.exe",
    "wmic.exe",
    "procdump.exe",
    "procdump64.exe",
    "procdump64a.exe",
    "sqldumper.exe",
    "createdump.exe",
    "dotnet-dump.exe",
];

/// Whether `ruta` (a program's full path) is inside Windows' folder or Program Files, where only an administrator
/// installs.
fn instalado(ruta: &str) -> bool {
    let r = ruta.replace('/', "\\").to_ascii_lowercase();
    let r = r.trim_start_matches(r"\\?\");
    r.get(1..).is_some_and(|x| {
        x.starts_with(r":\windows\")
            || x.starts_with(r":\program files\")
            || x.starts_with(r":\program files (x86)\")
            || x.starts_with(r":\programdata\microsoft\windows defender\")
    })
}

/// Whether `programa` (its file name) is one of the tools judged wherever they are and whoever signs them.
#[must_use]
pub fn es_herramienta(programa: &str) -> bool {
    HERRAMIENTAS.contains(&programa.to_ascii_lowercase().as_str())
}

/// Whether the signature has to be known before judging: a program that is not one of the tools, with a known path
/// outside Windows' folder and Program Files, asking to read the memory. For the rest the answer does not depend on it.
#[must_use]
pub fn depende_de_la_firma(programa: &str, ruta: &str, acceso: u32) -> bool {
    acceso & LEE_MEMORIA != 0 && !es_herramienta(programa) && !ruta.is_empty() && !instalado(ruta)
}

/// `programa` (its file name) at `ruta`, signed as `firma` says, opened LSASS asking for `acceso` (the rights, as
/// Windows' `DesiredAccess`); `concedido` says whether Windows allowed it. A warning when the rights include reading
/// its memory and the program is one to judge (see the module). Always grave.
#[must_use]
pub fn juzga(
    programa: &str,
    ruta: &str,
    firma: &Firma,
    acceso: u32,
    concedido: bool,
    cuando_ms: u64,
) -> Option<(Aviso, bool)> {
    if acceso & LEE_MEMORIA == 0 {
        return None;
    }
    let herramienta = es_herramienta(programa);
    // a program with no known path is judged by its name only
    if !herramienta && (ruta.is_empty() || instalado(ruta)) {
        return None;
    }
    // a program Windows found validly signed, listing the processes as so many do: not a sign (see the module)
    if !herramienta && matches!(firma, Firma::Firmado(_)) {
        return None;
    }
    let resultado = if concedido {
        "Windows se lo permitió"
    } else {
        "Windows no se lo permitió (la protección de LSA lo impide), pero el intento ya es una señal"
    };
    let texto = format!(
        "«{programa}» ha abierto la memoria de LSASS, el proceso de Windows que guarda las credenciales de las cuentas \
         con sesión abierta, para leerla; {resultado}. Así se roban las contraseñas de un equipo. Si no lo conoces, \
         analízalo y cambia tus contraseñas desde otro dispositivo."
    );
    Some((
        Aviso {
            regla: "credenciales",
            programa: programa.to_owned(),
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
    fn leer_la_memoria_de_lsass_es_grave() {
        // un programa de una carpeta donde cualquiera escribe
        let (a, grave) = juzga(
            "x.exe",
            r"C:\Users\ana\AppData\Local\Temp\x.exe",
            &Firma::SinFirma,
            0x1010,
            true,
            1,
        )
        .unwrap();
        assert!(grave);
        assert_eq!(a.regla, "credenciales");
        assert!(a.texto.contains("Windows se lo permitió"), "{}", a.texto);
        // un programa de Windows que ejecuta otros, aunque Windows lo negara
        let microsoft = Firma::Firmado("Microsoft Windows".into());
        let (a, _) = juzga(
            "rundll32.exe",
            r"C:\Windows\System32\rundll32.exe",
            &microsoft,
            0x1F_FFFF,
            false,
            1,
        )
        .unwrap();
        assert!(a.texto.contains("no se lo permitió"), "{}", a.texto);
        assert!(juzga("PowerShell.exe", "", &microsoft, 0x10, true, 1).is_some());
        // una herramienta que copia la memoria de un proceso, firmada y en la carpeta de la persona
        assert!(juzga(
            "procdump64.exe",
            r"C:\Users\ana\Downloads\procdump64.exe",
            &Firma::Firmado("Microsoft Corporation".into()),
            0x1F_FFFF,
            true,
            1
        )
        .is_some());
        // sin firma comprobada todavía, tras la espera: se avisa igual
        assert!(juzga(
            "x.exe",
            r"C:\Users\ana\Downloads\x.exe",
            &Firma::SinComprobar,
            0x1410,
            true,
            1
        )
        .is_some());
    }

    #[test]
    fn un_programa_firmado_que_lista_los_procesos_no_avisa() {
        // OneDrive, instalado en la carpeta de la persona: cada hora pide a LSASS lo mismo que a los demás procesos
        // (lo que vio el dueño el 10 nov 2026: 62 avisos graves en tres días, todos de OneDrive)
        let onedrive = r"C:\Users\ana\AppData\Local\Microsoft\OneDrive\OneDrive.exe";
        let microsoft = Firma::Firmado("Microsoft Corporation".into());
        for acceso in [0x0410, 0x1410, 0x1010] {
            for concedido in [true, false] {
                assert_eq!(
                    juzga("OneDrive.exe", onedrive, &microsoft, acceso, concedido, 1),
                    None,
                    "{acceso:#x} {concedido}"
                );
            }
        }
        // el mismo archivo sin firma válida (otro programa con su nombre, o uno cambiado): se avisa
        assert!(juzga("OneDrive.exe", onedrive, &Firma::SinFirma, 0x1410, false, 1).is_some());
        // de qué depende la espera de la firma
        assert!(depende_de_la_firma("OneDrive.exe", onedrive, 0x1410));
        assert!(!depende_de_la_firma("OneDrive.exe", onedrive, 0x1000));
        assert!(!depende_de_la_firma("powershell.exe", onedrive, 0x1410));
        assert!(!depende_de_la_firma("x.exe", "", 0x1410));
        assert!(!depende_de_la_firma(
            "av.exe",
            r"C:\Program Files\Antivirus\av.exe",
            0x1410
        ));
    }

    #[test]
    fn lo_de_windows_lo_instalado_y_lo_que_no_lee_no() {
        // sin el derecho de leer la memoria
        assert_eq!(
            juzga(
                "x.exe",
                r"C:\Users\ana\x.exe",
                &Firma::SinFirma,
                0x1000,
                true,
                1
            ),
            None
        );
        // Windows y Program Files
        assert_eq!(
            juzga(
                "taskmgr.exe",
                r"C:\Windows\System32\Taskmgr.exe",
                &Firma::SinComprobar,
                0x1410,
                true,
                1
            ),
            None
        );
        assert_eq!(
            juzga(
                "MsMpEng.exe",
                r"C:\ProgramData\Microsoft\Windows Defender\Platform\4.18.1\MsMpEng.exe",
                &Firma::SinComprobar,
                0x1F_FFFF,
                true,
                1
            ),
            None
        );
        assert_eq!(
            juzga(
                "av.exe",
                r"C:\Program Files\Antivirus\av.exe",
                &Firma::SinFirma,
                0x1410,
                true,
                1
            ),
            None
        );
        // un programa sin ruta conocida que no es una herramienta: no se sabe dónde está
        assert_eq!(juzga("x.exe", "", &Firma::SinFirma, 0x10, true, 1), None);
    }
}
