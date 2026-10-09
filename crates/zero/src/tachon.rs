//! «IA con tachón»: what you write to an AI leaves with your personal data replaced by marks
//! (`[CORREO_1]`, `[NOMBRE_1]`...) and the answer is shown back with them put in again, outside
//! the page, where the AI's website cannot read it. Duck.ai hides who you are; this hides what
//! you said about yourself. The original values never leave the browser's own process.

use serde::{Deserialize, Serialize};

use crate::datos::{detecta, Clase};
use crate::tinta::Marcado;

/// The word a mark carries for each kind, in the person's language: `CORREO`, `EMAIL`, `E-MAIL`.
pub trait Etiquetas {
    /// The uppercase word for a kind.
    fn etiqueta(&self, clase: Clase) -> String;
}

/// One replaced value: the mark the AI saw and the value it stands for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sustitucion {
    /// The mark as written in the text sent, `[CORREO_1]`.
    pub marca: String,
    /// The original value.
    pub valor: String,
    /// What it is.
    pub clase: Clase,
}

/// The text with the data replaced, and the table to undo it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Tachado {
    /// What the AI receives.
    pub texto: String,
    /// What was replaced, in order of appearance.
    pub sustituciones: Vec<Sustitucion>,
}

/// Replace the personal data of `texto`. The same value gets the same mark every time it
/// appears (and in later messages of the conversation, when `previas` are passed), so the AI
/// can still reason about «Persona 1» across the conversation.
#[must_use]
pub fn tacha(
    texto: &str,
    marcados: &[Marcado],
    previas: &[Sustitucion],
    etiquetas: &dyn Etiquetas,
) -> Tachado {
    let mut sust: Vec<Sustitucion> = previas.to_vec();
    let mut out = String::with_capacity(texto.len());
    let mut desde = 0;
    for trozo in detecta(texto, marcados) {
        let valor = &texto[trozo.inicio..trozo.fin];
        let clave = valor.trim().to_lowercase();
        let marca = if let Some(s) = sust.iter().find(|s| s.valor.trim().to_lowercase() == clave) {
            s.marca.clone()
        } else {
            let palabra = etiquetas.etiqueta(trozo.clase);
            let n = sust.iter().filter(|s| s.clase == trozo.clase).count() + 1;
            let marca = format!("[{palabra}_{n}]");
            sust.push(Sustitucion {
                marca: marca.clone(),
                valor: valor.to_string(),
                clase: trozo.clase,
            });
            marca
        };
        out.push_str(&texto[desde..trozo.inicio]);
        out.push_str(&marca);
        desde = trozo.fin;
    }
    out.push_str(&texto[desde..]);
    Tachado {
        texto: out,
        sustituciones: sust,
    }
}

/// Put the original values back into an answer that uses the marks. Only for the person's own
/// eyes: the shell shows it in its panel, never inside the AI's page.
#[must_use]
pub fn restaura(respuesta: &str, sustituciones: &[Sustitucion]) -> String {
    let mut out = respuesta.to_string();
    // Longest marks first, so `[CORREO_10]` is not eaten by `[CORREO_1]`.
    let mut orden: Vec<&Sustitucion> = sustituciones.iter().collect();
    orden.sort_by_key(|s| std::cmp::Reverse(s.marca.len()));
    for s in orden {
        out = out.replace(&s.marca, &s.valor);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tinta::Tipo;

    struct Es;
    impl Etiquetas for Es {
        fn etiqueta(&self, clase: Clase) -> String {
            match clase {
                Clase::Correo => "CORREO",
                Clase::Telefono => "TELÉFONO",
                Clase::Nombre => "NOMBRE",
                _ => "DATO",
            }
            .into()
        }
    }

    #[test]
    fn data_leaves_marked_and_comes_back_in_place() {
        let marcados = [Marcado {
            tipo: Tipo::Nombre,
            valor: "Francisco Salvatierra".into(),
        }];
        let t = tacha(
            "Soy Francisco Salvatierra, mi correo es fran@ejemplo.com. Escribe a fran@ejemplo.com.",
            &marcados,
            &[],
            &Es,
        );
        assert_eq!(
            t.texto,
            "Soy [NOMBRE_1], mi correo es [CORREO_1]. Escribe a [CORREO_1]."
        );
        assert_eq!(t.sustituciones.len(), 2);
        let r = restaura(
            "Hola [NOMBRE_1], te escribo a [CORREO_1].",
            &t.sustituciones,
        );
        assert_eq!(
            r,
            "Hola Francisco Salvatierra, te escribo a fran@ejemplo.com."
        );
    }

    #[test]
    fn a_conversation_keeps_its_marks() {
        let primera = tacha("Llama al +57 300 123 4567", &[], &[], &Es);
        let segunda = tacha(
            "¿Y si el +57 300 123 4567 no contesta? Prueba +57 310 765 4321",
            &[],
            &primera.sustituciones,
            &Es,
        );
        assert_eq!(
            segunda.texto,
            "¿Y si el [TELÉFONO_1] no contesta? Prueba [TELÉFONO_2]"
        );
    }

    #[test]
    fn text_without_data_goes_as_it_is() {
        let t = tacha("¿Qué tiempo hace en Cartagena?", &[], &[], &Es);
        assert_eq!(t.texto, "¿Qué tiempo hace en Cartagena?");
        assert!(t.sustituciones.is_empty());
    }
}
