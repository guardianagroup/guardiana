//! «Sin nombre» (phase A3; the owner, 1 Nov 2026, 18:55: «que vea todo lo posible»; ronda 87, Gemini's second goal:
//! «toda conexión de un proceso debe venir de una consulta DNS»): a program that connects straight to an address on
//! the internet that no name this computer asked for pointed to. Ordinary programs ask for a name first; many harmful
//! ones talk to whoever controls them at a bare address.
//!
//! The service sees each program's connections (Windows' own network events) and the addresses its DNS answered
//! with; what reaches this rule is a connection to an address on the internet (not local, not anycast) that no answer
//! named. Here, as in the other rules, a signed program is not judged, and only a program in a folder anyone can write
//! to, or come from the internet, is warned about. The address itself is never written (decision 203): only the
//! network and the country it belongs to.

use crate::primera_vez::{carpeta_escribible, Aviso, Firma};

/// The folders where an unsigned program that talks to bare addresses is most suspicious.
fn lugar_fuerte(lugar: &str) -> bool {
    matches!(
        lugar,
        "la carpeta de Descargas" | "una carpeta temporal" | "el Escritorio"
    )
}

/// A connection without a name, as a warning: `programa` (its file name) at `ruta`, its signature, whether Windows
/// marked its file as come from the internet, and `donde`, the network and country of the address (never the address).
/// The second value says whether it is grave: no signature and in Downloads, a temporary folder or the Desktop, or come
/// from the internet.
#[must_use]
pub fn juzga(
    programa: &str,
    ruta: &str,
    firma: &Firma,
    descargado: bool,
    donde: &str,
    cuando_ms: u64,
) -> Option<(Aviso, bool)> {
    if matches!(firma, Firma::Firmado(_)) {
        return None;
    }
    let lugar = carpeta_escribible(ruta);
    if lugar.is_none() && !descargado {
        return None;
    }
    let sin_firma = matches!(firma, Firma::SinFirma);
    let grave = sin_firma && (descargado || lugar.is_some_and(lugar_fuerte));
    let quien = match (sin_firma, lugar) {
        (true, Some(l)) => format!("«{programa}», sin firma, desde {l},"),
        (true, None) => format!("«{programa}», sin firma y descargado de internet,"),
        (false, Some(l)) => format!("«{programa}», desde {l},"),
        (false, None) => format!("«{programa}», descargado de internet,"),
    };
    let donde = if donde.trim().is_empty() {
        "una dirección de internet".to_owned()
    } else {
        format!("una dirección de internet ({})", donde.trim())
    };
    let texto = format!(
        "{quien} se conecta directamente a {donde} sin haber preguntado por ningún nombre. Los programas normales \
         preguntan antes; muchos programas dañinos hablan así con quien los controla. Si no lo conoces, ciérralo."
    );
    Some((
        Aviso {
            regla: "sin_nombre",
            programa: programa.to_owned(),
            ruta: ruta.to_owned(),
            sitio: String::new(),
            nombre: ruta.to_lowercase(),
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
    fn solo_lo_que_no_esta_firmado_y_esta_donde_no_debe() {
        // firmado o instalado: no
        assert_eq!(
            juzga(
                "a.exe",
                r"C:\Users\ana\Downloads\a.exe",
                &Firma::Firmado("X".into()),
                false,
                "",
                1
            ),
            None
        );
        assert_eq!(
            juzga(
                "a.exe",
                r"C:\Program Files\A\a.exe",
                &Firma::SinFirma,
                false,
                "",
                1
            ),
            None
        );
        // sin firma, desde Descargas: grave, sin la dirección, con su red y su país
        let (a, grave) = juzga(
            "a.exe",
            r"C:\Users\ana\Downloads\a.exe",
            &Firma::SinFirma,
            false,
            "Ejemplo Hosting, NL",
            1,
        )
        .unwrap();
        assert!(grave);
        assert_eq!(a.regla, "sin_nombre");
        assert!(a.texto.contains("(Ejemplo Hosting, NL)"), "{}", a.texto);
        // desde AppData, sin comprobar la firma: aviso, no grave
        let (_, grave) = juzga(
            "b.exe",
            r"C:\Users\ana\AppData\Roaming\b.exe",
            &Firma::SinComprobar,
            false,
            "",
            1,
        )
        .unwrap();
        assert!(!grave);
    }
}
