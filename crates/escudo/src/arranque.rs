//! «Arranque» (phase A3; the owner, 1 Nov 2026, 18:55: «que vea todo lo posible»): a program that has just been added
//! to what starts on its own with Windows (a Run key, the Startup folder, a scheduled task, a service), when the
//! program it runs has no signature and lives in a folder any program can write to, or came from the internet.
//!
//! The continuous watch already notes every new one (`guardiana_apps::vigilancia`, «Lo que arranca solo» in the
//! panel); this rule picks the ones an antivirus should point at. A signed program is not judged here, and one whose
//! signature could not be checked counts only in the Downloads folder, a temporary folder or the Desktop.
//!
//! A new WMI event subscription that runs a command or a script ([`juzga_wmi`]) is judged apart: what it runs is often
//! Windows' own program (PowerShell), so its signature says nothing; the subscription itself is the sign.

use crate::primera_vez::{carpeta_escribible, Aviso, Firma};

/// Program and script file extensions looked for in what an autostart runs.
const EXTENSIONES: &[&str] = &[
    ".exe", ".dll", ".scr", ".com", ".bat", ".cmd", ".ps1", ".vbs", ".vbe", ".js", ".jse", ".wsf",
    ".hta", ".cpl", ".msi", ".jar",
];

/// Folders Windows names by a variable that is the same for every account.
const DEL_EQUIPO: &[(&str, &str)] = &[
    ("%systemroot%", r"C:\Windows"),
    ("%windir%", r"C:\Windows"),
    ("%programfiles(x86)%", r"C:\Program Files (x86)"),
    ("%programfiles%", r"C:\Program Files"),
    ("%programw6432%", r"C:\Program Files"),
    ("%programdata%", r"C:\ProgramData"),
    ("%allusersprofile%", r"C:\ProgramData"),
    ("%public%", r"C:\Users\Public"),
    ("%systemdrive%", "C:"),
];

/// Folders named by a variable that depends on the account: where they are is known, not their exact path (the `?`).
const DE_LA_CUENTA: &[(&str, &str)] = &[
    ("%localappdata%", r"C:\Users\?\AppData\Local"),
    ("%appdata%", r"C:\Users\?\AppData\Roaming"),
    ("%temp%", r"C:\Users\?\AppData\Local\Temp"),
    ("%tmp%", r"C:\Users\?\AppData\Local\Temp"),
    ("%userprofile%", r"C:\Users\?"),
];

/// A program file found in what an autostart runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Archivo {
    /// Its path, with the folders Windows names by a variable written out.
    pub ruta: String,
    /// Whether the path is exact (no account's folder left unknown): only then can the file itself be looked at.
    pub exacta: bool,
}

/// `texto` with every `variable` (lower case, `%…%`) replaced by `valor`, whatever its case in `texto`.
fn reemplaza(texto: &str, variable: &str, valor: &str) -> String {
    let bajo = texto.to_ascii_lowercase();
    let mut out = String::with_capacity(texto.len());
    let mut desde = 0;
    while let Some(i) = bajo.get(desde..).and_then(|r| r.find(variable)) {
        let i = desde + i;
        out.push_str(texto.get(desde..i).unwrap_or_default());
        out.push_str(valor);
        desde = i + variable.len();
    }
    out.push_str(texto.get(desde..).unwrap_or_default());
    out
}

/// Whether a path ends in one of the program extensions.
fn es_programa(ruta: &str) -> bool {
    let r = ruta.to_ascii_lowercase();
    EXTENSIONES.iter().any(|e| r.ends_with(e))
}

/// Where an unquoted program path found at byte `inicio` of `bajo` ends: right after its first program extension
/// that is followed by the end, a space, a quote, a comma or a semicolon. It never runs past a quote.
fn fin_de_programa(bajo: &str, inicio: usize) -> Option<usize> {
    let resto = bajo.get(inicio..)?;
    for (j, c) in resto.char_indices() {
        if c == '"' {
            return None;
        }
        let Some(cola) = resto.get(j..) else {
            continue;
        };
        for e in EXTENSIONES {
            if !cola.starts_with(e) {
                continue;
            }
            let despues = cola[e.len()..].chars().next();
            if despues.is_none_or(|c| c.is_whitespace() || matches!(c, '"' | '\'' | ',' | ';')) {
                return Some(inicio + j + e.len());
            }
        }
    }
    None
}

/// The program files in what an autostart runs, in order: `"C:\x\y.exe" -a`, `C:\Program Files\x\y.exe /b`,
/// `rundll32.exe C:\x\y.dll,Entrada`, `%APPDATA%\x\y.exe`, `\SystemRoot\System32\x.exe`. A bare name (`svchost.exe`)
/// is not a path and is left out: Windows finds it in its own folders.
#[must_use]
pub fn archivos_en(orden: &str) -> Vec<Archivo> {
    let orden: String = orden.chars().take(2_048).collect();
    let mut texto = orden.trim().replace('/', "\\");
    if let Some(r) = texto.strip_prefix(r"\??\") {
        texto = r.to_owned();
    }
    let bajo = texto.to_ascii_lowercase();
    if bajo.starts_with(r"\systemroot\") {
        texto = format!(r"C:\Windows\{}", texto.get(12..).unwrap_or_default());
    } else if bajo.starts_with(r"system32\") {
        texto = format!(r"C:\Windows\{texto}");
    }
    for (variable, valor) in DEL_EQUIPO.iter().chain(DE_LA_CUENTA) {
        texto = reemplaza(&texto, variable, valor);
    }
    let bajo = texto.to_ascii_lowercase();
    let b = bajo.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 2 < b.len() {
        let antes_libre = i == 0 || !b[i - 1].is_ascii_alphanumeric();
        let unidad =
            b[i].is_ascii_alphabetic() && b[i + 1] == b':' && b[i + 2] == b'\\' && antes_libre;
        let red = b[i] == b'\\' && b[i + 1] == b'\\' && (i == 0 || matches!(b[i - 1], b' ' | b'"'));
        if !(unidad || red) {
            i += 1;
            continue;
        }
        // between quotes: the whole quoted text is the path
        let fin = if i > 0 && b[i - 1] == b'"' {
            let fin = bajo
                .get(i..)
                .and_then(|r| r.find('"'))
                .map_or(b.len(), |k| i + k);
            Some(fin).filter(|f| es_programa(texto.get(i..*f).unwrap_or_default()))
        } else {
            fin_de_programa(&bajo, i)
        };
        match fin {
            Some(fin) => {
                let ruta = texto.get(i..fin).unwrap_or_default().to_owned();
                out.push(Archivo {
                    exacta: !ruta.contains('?'),
                    ruta,
                });
                i = fin;
            }
            None => i += 1,
        }
    }
    out
}

/// The folders where a program that starts on its own almost never lives.
fn lugar_fuerte(lugar: &str) -> bool {
    matches!(
        lugar,
        "la carpeta de Descargas" | "una carpeta temporal" | "el Escritorio"
    )
}

/// What kind of autostart, in plain words.
fn que_es(tipo: &str) -> &'static str {
    match tipo {
        "run" => "una clave Run del registro",
        "runonce" => "una clave RunOnce del registro",
        "carpeta" => "la carpeta Inicio",
        "tarea" => "una tarea programada",
        "servicio" => "un servicio",
        _ => "el arranque",
    }
}

/// A new autostart as a warning, when the program it runs has no signature and lives in a folder any program can
/// write to, or came from the internet (`descargado`: Windows' internet mark on the file). `titulo` is the name the
/// watch gives it, `tipo` its kind (`run`, `runonce`, `carpeta`, `tarea`, `servicio`) and `clave` what identifies it
/// in the watch's history. The second value says whether it is grave: downloaded, in Downloads, a temporary folder or
/// the Desktop, or a service.
#[must_use]
pub fn juzga(
    titulo: &str,
    tipo: &str,
    clave: &str,
    archivo: &Archivo,
    firma: &Firma,
    descargado: bool,
    cuando_ms: u64,
) -> Option<(Aviso, bool)> {
    let lugar = carpeta_escribible(&archivo.ruta);
    let fuerte = lugar.is_some_and(lugar_fuerte);
    match firma {
        Firma::Firmado(_) => return None,
        Firma::SinComprobar if !fuerte => return None,
        _ => {}
    }
    let donde = match (lugar, descargado) {
        (Some(l), true) => format!("está en {l} y llegó de internet"),
        (Some(l), false) => format!("está en {l}"),
        (None, true) => "llegó de internet".to_owned(),
        (None, false) => return None,
    };
    let programa = archivo
        .ruta
        .rsplit('\\')
        .next()
        .unwrap_or_default()
        .to_owned();
    let sin_firma = if matches!(firma, Firma::SinFirma) {
        ", sin firma,"
    } else {
        ","
    };
    let texto = format!(
        "«{titulo}» se ha añadido a lo que arranca solo con Windows ({}), y su programa, {programa}{sin_firma} {donde}. \
         Así se quedan en el equipo muchos programas dañinos; si no lo instalaste tú, revísalo.",
        que_es(tipo)
    );
    let grave = descargado || fuerte || tipo == "servicio";
    Some((
        Aviso {
            regla: "arranque",
            programa,
            ruta: archivo.ruta.clone(),
            sitio: String::new(),
            nombre: clave.to_owned(),
            cuando_ms,
            fuente: None,
            texto,
        },
        grave,
    ))
}

/// A new WMI event subscription that runs a command or a script (`tipo` `wmi` in the continuous watch), called `titulo`,
/// running `orden`: a way to start on its own that no Run key, folder or task shows, used by intrusions that leave no
/// file. Rare at home, so always grave. `clave` identifies it (the watch's key).
#[must_use]
pub fn juzga_wmi(titulo: &str, orden: &str, clave: &str, cuando_ms: u64) -> (Aviso, bool) {
    let corta: String = orden.chars().take(160).collect();
    let mas = if orden.chars().count() > 160 {
        "…"
    } else {
        ""
    };
    let texto = format!(
        "Hay una suscripción de WMI nueva, «{titulo}», que ejecuta «{corta}{mas}» cuando ocurre algo en el equipo. Es \
         una forma de arrancar solo que no aparece en el Inicio ni en las tareas programadas, y la usan intrusiones que \
         no dejan archivos. Si no la reconoces, analiza el equipo."
    );
    (
        Aviso {
            regla: "arranque",
            programa: titulo.to_owned(),
            ruta: orden.to_owned(),
            sitio: String::new(),
            nombre: clave.to_owned(),
            cuando_ms,
            fuente: None,
            texto,
        },
        true,
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn una_suscripcion_de_wmi_nueva_es_grave() {
        let largo = format!(
            "powershell.exe -NoProfile -WindowStyle Hidden -Command {}",
            "x".repeat(300)
        );
        let (a, grave) = juzga_wmi(
            "Actualizador",
            &largo,
            "wmi|commandlineeventconsumer|actualizador",
            7,
        );
        assert!(grave);
        assert_eq!(a.regla, "arranque");
        assert_eq!(a.nombre, "wmi|commandlineeventconsumer|actualizador");
        assert!(a.texto.contains("«Actualizador»"), "{}", a.texto);
        assert!(a.texto.contains("…»"), "a long command is cut: {}", a.texto);
        assert!(a.texto.chars().count() < 600, "{}", a.texto);
    }

    fn rutas(orden: &str) -> Vec<String> {
        archivos_en(orden).into_iter().map(|a| a.ruta).collect()
    }

    #[test]
    fn encuentra_el_programa_en_la_orden() {
        assert_eq!(
            rutas(r#""C:\Program Files\App\app.exe" --oculto"#),
            vec![r"C:\Program Files\App\app.exe"]
        );
        assert_eq!(
            rutas(r"C:\Program Files\App\app.exe /b"),
            vec![r"C:\Program Files\App\app.exe"]
        );
        assert_eq!(
            rutas(r"rundll32.exe C:\Users\ana\AppData\Roaming\x\y.dll,Entrada"),
            vec![r"C:\Users\ana\AppData\Roaming\x\y.dll"]
        );
        assert_eq!(
            rutas(r"\SystemRoot\System32\drivers\x.exe"),
            vec![r"C:\Windows\System32\drivers\x.exe"]
        );
        assert_eq!(
            rutas(r"%ProgramFiles%\App\app.exe"),
            vec![r"C:\Program Files\App\app.exe"]
        );
        assert!(rutas("svchost.exe -k red").is_empty());
        assert!(rutas("").is_empty());
        // la carpeta de una cuenta: se sabe dónde está, no su ruta exacta
        let a = archivos_en(r"%APPDATA%\Actualiza\upd.exe");
        assert_eq!(a.len(), 1);
        assert!(!a[0].exacta);
        assert_eq!(
            carpeta_escribible(&a[0].ruta),
            Some("la carpeta de datos de tu usuario (AppData)")
        );
        // una carpeta que se llama como un programa no corta la ruta
        assert_eq!(rutas(r"C:\x.exe.d\y.exe"), vec![r"C:\x.exe.d\y.exe"]);
        // entre comillas, con espacios, y una segunda ruta detrás
        assert_eq!(
            rutas(r#""C:\Program Files\App" "C:\Users\ana\Desktop\a b.exe" x"#),
            vec![r"C:\Users\ana\Desktop\a b.exe"]
        );
        // los nombres de lugar que la regla trata como fuertes existen tal cual
        assert!(lugar_fuerte(
            carpeta_escribible(r"C:\Users\ana\Downloads\a.exe").unwrap()
        ));
        assert!(lugar_fuerte(
            carpeta_escribible(r"C:\Users\ana\AppData\Local\Temp\a.exe").unwrap()
        ));
        assert!(lugar_fuerte(
            carpeta_escribible(r"C:\Users\ana\Desktop\a.exe").unwrap()
        ));
    }

    #[test]
    fn juzga_solo_lo_que_debe() {
        let tmp = &archivos_en(r"C:\Users\ana\AppData\Local\Temp\upd.exe")[0];
        let (a, grave) = juzga(
            "Actualiza",
            "tarea",
            "tarea|\\|actualiza",
            tmp,
            &Firma::SinFirma,
            false,
            1,
        )
        .unwrap();
        assert!(grave);
        assert_eq!(a.regla, "arranque");
        assert_eq!(a.programa, "upd.exe");
        assert!(a.texto.contains("una tarea programada"), "{}", a.texto);
        assert!(a.texto.contains("sin firma"), "{}", a.texto);
        // firmado: no es cosa de esta regla
        assert_eq!(
            juzga(
                "A",
                "run",
                "k",
                tmp,
                &Firma::Firmado("Empresa".into()),
                false,
                1
            ),
            None
        );
        // instalado en Archivos de programa, sin firma: tampoco
        let pf = &archivos_en(r"C:\Program Files\App\app.exe")[0];
        assert_eq!(juzga("A", "run", "k", pf, &Firma::SinFirma, false, 1), None);
        // sin comprobar: solo en Descargas, carpetas temporales o el Escritorio
        let ad = &archivos_en(r"%LOCALAPPDATA%\App\app.exe")[0];
        assert_eq!(
            juzga("A", "run", "k", ad, &Firma::SinComprobar, false, 1),
            None
        );
        let (_, grave) = juzga("A", "run", "k", ad, &Firma::SinFirma, false, 1).unwrap();
        assert!(!grave);
        let dl = &archivos_en(r"C:\Users\ana\Downloads\a.exe")[0];
        assert!(juzga("A", "run", "k", dl, &Firma::SinComprobar, false, 1).is_some());
        // un servicio sin firma desde ProgramData: grave
        let pd = &archivos_en(r#""C:\ProgramData\Svc\svc.exe" -s"#)[0];
        let (_, grave) = juzga(
            "Svc",
            "servicio",
            "servicio|svc",
            pd,
            &Firma::SinFirma,
            false,
            1,
        )
        .unwrap();
        assert!(grave);
    }
}
