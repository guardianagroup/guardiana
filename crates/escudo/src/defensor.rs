//! «Segunda opinión» (phase A3; the owner, 1 Nov 2026, 18:55: «lo mejor que puedas, en detección»): when a rule gives a
//! grave warning about a program's file, Microsoft Defender's own engine, which every Windows 10 and 11 carries with
//! signatures kept up to date by Windows, is asked about that one file (`MpCmdRun.exe -Scan -ScanType 3 -File …
//! -DisableRemediation`: it only says, it touches nothing). A known threat becomes a grave warning of its own, with
//! the name Defender gives it. Nothing is uploaded by this: it is the local scan.
//!
//! This module reads Defender's answer; the service runs it.

use crate::primera_vez::Aviso;

/// The threats Defender lists in its answer (`Threat : Trojan:Win32/…` lines), in order, without repeats.
#[must_use]
pub fn amenazas(salida: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for linea in salida.lines() {
        let Some((clave, valor)) = linea.split_once(':') else {
            continue;
        };
        if !clave.trim().eq_ignore_ascii_case("threat") {
            continue;
        }
        let v: String = valor.trim().chars().take(120).collect();
        if !v.is_empty() && !out.contains(&v) {
            out.push(v);
        }
    }
    out
}

/// Defender's answer about `programa` (its file name) at `ruta` as a grave warning, when it names a threat.
#[must_use]
pub fn juzga(
    programa: &str,
    ruta: &str,
    amenazas: &[String],
    cuando_ms: u64,
) -> Option<(Aviso, bool)> {
    let primera = amenazas.first()?;
    let mas = if amenazas.len() > 1 {
        format!(" (y {} más)", amenazas.len() - 1)
    } else {
        String::new()
    };
    let texto = format!(
        "Microsoft Defender reconoce «{programa}» como {primera}{mas}: es un programa dañino conocido. No lo abras; \
         puedes ponerlo en cuarentena desde Seguridad de Windows."
    );
    Some((
        Aviso {
            regla: "defensor",
            programa: programa.to_owned(),
            ruta: ruta.to_owned(),
            sitio: String::new(),
            nombre: ruta.to_lowercase(),
            cuando_ms,
            fuente: Some("Microsoft Defender".to_owned()),
            texto,
        },
        true,
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const RESPUESTA: &str =
        "Scan starting...\r\nScan finished.\r\nScanning C:\\x\\a.exe found 1 threats.\r\n\r\n\
<===========================LIST OF DETECTED THREATS==========================>\r\n\
----------------------------- Threat information ------------------------------\r\n\
Threat                  : Trojan:Win32/Ejemplo.A!ml\r\n\
Resources               : 1 total\r\n\
    file                : C:\\x\\a.exe\r\n\
-------------------------------------------------------------------------------\r\n";

    #[test]
    fn lee_la_respuesta_de_defender() {
        assert_eq!(
            amenazas(RESPUESTA),
            vec!["Trojan:Win32/Ejemplo.A!ml".to_owned()]
        );
        assert!(amenazas(
            "Scan starting...\nScan finished.\nScanning C:\\x\\b.exe found no threats.\n"
        )
        .is_empty());
        let (a, grave) = juzga("a.exe", r"C:\x\a.exe", &amenazas(RESPUESTA), 1).unwrap();
        assert!(grave);
        assert_eq!(a.regla, "defensor");
        assert!(a.texto.contains("Trojan:Win32/Ejemplo.A!ml"), "{}", a.texto);
        assert_eq!(juzga("b.exe", r"C:\x\b.exe", &[], 1), None);
    }
}
