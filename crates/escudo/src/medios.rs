//! «Medio» (phase A3; the owner, 1 Nov 2026, 18:55: «que vea todo lo posible, todo, absolutamente todo»): a program
//! started from a disc or a mounted disk image (an ISO or IMG file opened with a double click), or from a removable USB
//! drive. Opening what comes inside a disk image received by mail or downloaded is a common way in, and some harmful
//! programs copy themselves to USB sticks to be opened from them on another computer.
//!
//! The service sees every program start (Windows' kernel process events, the same watch as [`crate::cadenas`]) and
//! where it ran from. An installer from a disc image is usually signed: a signed program is not judged; one without a
//! signature is grave, and one whose signature could not be checked is a plain warning.

use crate::primera_vez::{Aviso, Firma};

/// Where a program ran from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Medio {
    /// A disc, or a disk image mounted as one (ISO, IMG).
    Imagen,
    /// A removable drive (a USB stick, a memory card).
    Usb,
}

impl Medio {
    /// Its code, as the service passes it.
    #[must_use]
    pub fn codigo(self) -> &'static str {
        match self {
            Self::Imagen => "imagen",
            Self::Usb => "usb",
        }
    }

    /// From its code.
    #[must_use]
    pub fn de_codigo(c: &str) -> Option<Self> {
        match c {
            "imagen" => Some(Self::Imagen),
            "usb" => Some(Self::Usb),
            _ => None,
        }
    }
}

/// `programa` (its file name) at `ruta`, started from `medio`, with its signature `firma`, as a warning.
#[must_use]
pub fn juzga(
    programa: &str,
    ruta: &str,
    firma: &Firma,
    medio: Medio,
    cuando_ms: u64,
) -> Option<(Aviso, bool)> {
    let grave = match firma {
        Firma::Firmado(_) => return None,
        Firma::SinFirma => true,
        Firma::SinComprobar => false,
    };
    let quien = if grave {
        format!("«{programa}», sin firma,")
    } else {
        format!("«{programa}»")
    };
    let texto = match medio {
        Medio::Imagen => format!(
            "{quien} se ha abierto desde un disco o una imagen montada (un archivo ISO o IMG). Abrir lo que viene dentro \
             de una imagen recibida por correo o descargada es una forma frecuente de entrar en un equipo. Si no lo \
             esperabas, ciérralo, expulsa la imagen y no lo vuelvas a abrir."
        ),
        Medio::Usb => format!(
            "{quien} se ha abierto desde una memoria USB o una tarjeta. Algunos programas dañinos se copian a las \
             memorias para abrirse desde ellas en otro equipo. Si no lo esperabas, ciérralo y analiza la memoria."
        ),
    };
    Some((
        Aviso {
            regla: "medio",
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
    fn sin_firma_desde_una_imagen_o_una_memoria() {
        let (a, grave) = juzga(
            "Factura.exe",
            r"E:\Factura.exe",
            &Firma::SinFirma,
            Medio::Imagen,
            1,
        )
        .unwrap();
        assert!(grave);
        assert_eq!(a.regla, "medio");
        assert!(a.texto.contains("imagen montada"), "{}", a.texto);
        let (a, grave) = juzga("x.exe", r"F:\x.exe", &Firma::SinComprobar, Medio::Usb, 1).unwrap();
        assert!(!grave);
        assert!(a.texto.contains("memoria USB"), "{}", a.texto);
        // un instalador firmado: nada
        assert_eq!(
            juzga(
                "setup.exe",
                r"E:\setup.exe",
                &Firma::Firmado("Microsoft Corporation".into()),
                Medio::Imagen,
                1
            ),
            None
        );
        assert_eq!(Medio::de_codigo(Medio::Usb.codigo()), Some(Medio::Usb));
        assert_eq!(Medio::de_codigo("otro"), None);
    }
}
