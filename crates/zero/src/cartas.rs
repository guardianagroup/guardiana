//! The letter that asks a company to delete your data, ready to copy or open in your mail: the
//! law that gives the right (Colombia's Ley 1581, the EU's GDPR, California's CCPA, Brazil's
//! LGPD), the site, the kinds of data it received according to the book, and your name and email
//! if you marked them. The person sends it; GUARDIANA ZERO never does.

use serde::{Deserialize, Serialize};

use crate::libro::Entrada;
use crate::textos::{Idioma, Textos};

/// The laws a letter can rely on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Ley {
    /// Colombia: Ley 1581 de 2012, art. 8 (e) and art. 15.
    Co,
    /// European Union and Spain: GDPR art. 17 and art. 12(3).
    Ue,
    /// California: CCPA §1798.105 and §1798.130.
    Ca,
    /// Brazil: LGPD art. 18.
    Br,
    /// Anywhere else: the request without a law named.
    Otro,
}

impl Ley {
    const fn clave(self) -> &'static str {
        match self {
            Self::Co => "co",
            Self::Ue => "ue",
            Self::Ca => "ca",
            Self::Br => "br",
            Self::Otro => "otro",
        }
    }
}

/// A letter: subject and body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Carta {
    /// Subject line.
    pub asunto: String,
    /// Body.
    pub cuerpo: String,
}

/// The letter for `entrada` under `ley`. It goes in the language the company has to answer in:
/// Spanish under Colombian law, Portuguese under Brazil's, English under California's, and the
/// person's own language under the GDPR or no law named. `nombre` and `correo` are the person's,
/// when marked; otherwise the letter leaves a blank for them.
#[must_use]
pub fn carta(
    ui: &Textos,
    ley: Ley,
    entrada: &Entrada,
    nombre: &str,
    correo: &str,
    hoy: &str,
) -> Carta {
    let t = match ley {
        Ley::Co => Textos::de(Idioma::Es),
        Ley::Br => Textos::de(Idioma::Pt),
        Ley::Ca => Textos::de(Idioma::En),
        Ley::Ue | Ley::Otro => *ui,
    };
    let datos = entrada
        .clases
        .iter()
        .map(|c| t.t(c.clave()))
        .collect::<Vec<_>>()
        .join(", ");
    let quien = entrada
        .empresa
        .clone()
        .unwrap_or_else(|| entrada.sitio.clone());
    let vacio = t.t("carta_hueco");
    let rellena = |plantilla: &str| {
        plantilla
            .replace("{empresa}", &quien)
            .replace("{sitio}", &entrada.sitio)
            .replace("{datos}", &datos)
            .replace(
                "{nombre}",
                if nombre.trim().is_empty() {
                    &vacio
                } else {
                    nombre
                },
            )
            .replace(
                "{correo}",
                if correo.trim().is_empty() {
                    &vacio
                } else {
                    correo
                },
            )
            .replace("{fecha}", hoy)
    };
    let mut cuerpo = rellena(&t.t(&format!("carta_{}_cuerpo", ley.clave())));
    if let Some((nombre_registro, Some(url))) = &entrada.corredor {
        cuerpo.push_str("\n\n");
        cuerpo.push_str(
            &t.t("carta_corredor")
                .replace("{registro}", nombre_registro)
                .replace("{url}", url),
        );
    }
    Carta {
        asunto: rellena(&t.t(&format!("carta_{}_asunto", ley.clave()))),
        cuerpo,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libro::Libro;

    #[test]
    fn a_letter_names_the_law_the_site_and_the_data() {
        let mut l = Libro::default();
        l.anota(
            "www.tienda.co",
            &[crate::datos::Clase::Correo, crate::datos::Clase::Telefono],
            1,
        );
        let e = &l.entradas["tienda.co"];
        let es = Textos::de(Idioma::Es);
        let c = carta(
            &es,
            Ley::Co,
            e,
            "Ana Pérez",
            "ana@correo.co",
            "9 de octubre de 2026",
        );
        assert!(c.asunto.contains("tienda.co"));
        assert!(c.cuerpo.contains("Ley 1581 de 2012"));
        assert!(c.cuerpo.contains("correo, teléfono"));
        assert!(c.cuerpo.contains("Ana Pérez"));
        // California's letter goes in English whatever the screen's language.
        let ca = carta(&es, Ley::Ca, e, "", "", "October 9, 2026");
        assert!(ca.cuerpo.contains("1798.105"));
        assert!(ca.cuerpo.contains("email, phone"));
        assert!(ca.cuerpo.contains("[write it here]"));
    }
}
