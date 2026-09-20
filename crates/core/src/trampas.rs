//! Archivos trampa: un cebo que no necesita vigilar ningún archivo.
//!
//! La idea que pidió el responsable el 20 de septiembre de 2026 es saber **quién pica el anzuelo**:
//! dejar un archivo que parezca valioso y enterarse de quién lo abre. Vigilar el sistema de
//! archivos está fuera del programa y lo va a seguir estando (brief §1: «qué archivos abre o lee
//! un agente» es otro producto y una promesa que hoy no se puede cumplir), y además la web dice en
//! tres idiomas que Guardiana no mira lo que lee nadie.
//!
//! Hay una forma de hacerlo sin romper nada de eso, y es la que está aquí: **el cebo no es el
//! archivo, es el nombre que lleva dentro**. Cada trampa escribe un archivo con una dirección
//! única —`<id>.trampa.guardianagroup.com`— y nada más. Guardiana no mira el archivo: sigue
//! mirando solo nombres, como siempre. Si algún día alguien pregunta por ese nombre, es que ese
//! archivo se leyó y algo intentó ir a buscarlo; la consulta aparece en el extracto con su hora y,
//! en Windows, con el programa que la hizo.
//!
//! Lo que esto **no** atrapa, dicho antes de que lo pregunte nadie: un programa que lea el archivo
//! y se lo mande a su servidor sin preguntar por esa dirección no aparece. Es una trampa para
//! quien sigue enlaces, no un detector de robos.

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::ledger::Ledger;

/// Setting where the traps live, as JSON.
const CLAVE: &str = "trampas_json";

/// El dominio de las trampas. Es nuestro, así que nadie más puede recibir la pregunta, y la
/// pregunta no necesita salir del equipo: Guardiana la ve antes que nadie.
pub const DOMINIO: &str = "trampa.guardianagroup.com";

/// Un archivo trampa puesto por la persona.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trampa {
    /// Identificador corto y único; es lo que lleva el nombre dentro del archivo.
    pub id: String,
    /// Dónde se dejó el archivo.
    pub archivo: String,
    /// Cuándo se creó (ms).
    pub creada: i64,
}

impl Trampa {
    /// El nombre que va dentro del archivo y que delata su lectura.
    #[must_use]
    pub fn nombre(&self) -> String {
        format!("{}.{DOMINIO}", self.id)
    }
}

/// El identificador de la trampa si este nombre es una de ellas.
#[must_use]
pub fn id_de(qname: &str) -> Option<String> {
    let limpio = qname.trim_end_matches('.').to_ascii_lowercase();
    let resto = limpio.strip_suffix(DOMINIO)?.strip_suffix('.')?;
    // Solo el nivel de al lado: `a.b.trampa...` no es la trampa `b`.
    (!resto.is_empty() && !resto.contains('.')).then(|| resto.to_owned())
}

/// Las trampas puestas hasta ahora.
///
/// # Errors
/// Si el extracto no se puede leer.
pub fn listar(l: &Ledger) -> Result<Vec<Trampa>> {
    let Some(json) = l.setting(CLAVE)? else {
        return Ok(Vec::new());
    };
    Ok(serde_json::from_str(&json).unwrap_or_default())
}

/// Guarda la lista entera.
///
/// # Errors
/// Si el extracto no se puede escribir.
pub fn guardar(l: &Ledger, trampas: &[Trampa]) -> Result<()> {
    l.set_setting(CLAVE, &serde_json::to_string(trampas)?)
}

/// Lo que se escribe dentro del archivo.
///
/// Parece lo que tiene que parecer para que un programa curioso siga el enlace, y **la última
/// línea dice lo que es**. Ese equilibrio es a propósito: la persona que se encuentre este archivo
/// dentro de seis meses tiene que poder entender qué es sin llamar a nadie, y un agente que lea
/// una línea que dice «esto es una trampa» y aun así vaya a buscar la dirección ha contado algo
/// todavía más claro que si no la hubiera leído.
#[must_use]
pub fn contenido(t: &Trampa) -> String {
    format!(
        "# Copia de acceso · no borrar\n\
         # backup access · do not delete\n\
         \n\
         endpoint = https://{nombre}/v1\n\
         token    = gd_{id}\n\
         \n\
         # Este archivo es una TRAMPA de GUARDIANA, puesta el {fecha}.\n\
         # No hay ninguna credencial de verdad aquí dentro. Si algún programa lee este archivo y\n\
         # pregunta por esa dirección, quedará anotado en el extracto con su hora y, en Windows,\n\
         # con el nombre del programa. Se quita con: guardiana trampa quitar {id}\n",
        nombre = t.nombre(),
        id = t.id,
        fecha = crate::time::rfc3339_utc(t.creada),
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn reconoce_su_nombre_y_solo_el_suyo() {
        let t = Trampa {
            id: "a1b2c3d4".to_owned(),
            archivo: "/tmp/x.txt".to_owned(),
            creada: 0,
        };
        assert_eq!(t.nombre(), format!("a1b2c3d4.{DOMINIO}"));
        assert_eq!(id_de(&t.nombre()).as_deref(), Some("a1b2c3d4"));
        assert_eq!(
            id_de(&t.nombre().to_uppercase()).as_deref(),
            Some("a1b2c3d4")
        );
        assert_eq!(
            id_de(&format!("{}.", t.nombre())).as_deref(),
            Some("a1b2c3d4")
        );
        // Ni el dominio a secas, ni un nivel de más, ni cualquier otro nombre.
        assert_eq!(id_de(DOMINIO), None);
        assert_eq!(id_de(&format!("otra.cosa.{DOMINIO}")), None);
        assert_eq!(id_de("guardianagroup.com"), None);
        assert_eq!(id_de("trampa.guardianagroup.com.evil.example"), None);
    }

    #[test]
    fn el_contenido_lleva_el_nombre_y_dice_lo_que_es() {
        let t = Trampa {
            id: "deadbeef".to_owned(),
            archivo: "/tmp/x.txt".to_owned(),
            creada: 1_700_000_000_000,
        };
        let c = contenido(&t);
        assert!(c.contains(&t.nombre()));
        assert!(c.contains("TRAMPA de GUARDIANA"));
        assert!(c.contains("guardiana trampa quitar deadbeef"));
    }
}
