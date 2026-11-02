//! «Protección» (phase A3; the owner, 1 Nov 2026, 18:55: «que vea todo lo posible»): Windows' own protection switched
//! off. Turning off Microsoft Defender's real-time protection, its tamper protection or the firewall is among the first
//! things many harmful programs do. The continuous watch already notes every change of Windows Security
//! (`guardiana_apps::vigilancia`, «Protección» in the panel); this rule turns the ones an antivirus should point at into
//! warnings: grave when the protection goes off, a plain warning when it was already off at the watch's first look.

use crate::primera_vez::Aviso;

/// The name shown where a warning names a program.
const QUIEN: &str = "Seguridad de Windows";

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
