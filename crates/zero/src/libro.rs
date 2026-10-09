//! «Quién tiene mis datos»: every site that received personal data from this browser, which
//! kinds, and when. Built from what actually left (the form guard and the marked values), not
//! from guesses, so one button can ask exactly those companies to delete it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::datos::Clase;
use crate::destino::clasifica;

/// One site in the book.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entrada {
    /// The registrable site.
    pub sitio: String,
    /// The company behind it, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empresa: Option<String>,
    /// Its country.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pais: Option<String>,
    /// Registered as a data broker: its name in the registry and its deletion page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corredor: Option<(String, Option<String>)>,
    /// Kinds of data it received.
    pub clases: Vec<Clase>,
    /// First and last time, ms since the epoch.
    pub primera: i64,
    /// Last time.
    pub ultima: i64,
    /// How many times.
    pub veces: u32,
    /// When a deletion letter was prepared for it, if ever.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub carta: Option<i64>,
}

/// The book: site → entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Libro {
    /// Every site, by registrable site.
    pub entradas: BTreeMap<String, Entrada>,
}

impl Libro {
    /// Write down that `host` received `clases` at `ts`.
    pub fn anota(&mut self, host: &str, clases: &[Clase], ts: i64) {
        if clases.is_empty() {
            return;
        }
        let d = clasifica(host);
        let e = self
            .entradas
            .entry(d.sitio.clone())
            .or_insert_with(|| Entrada {
                sitio: d.sitio.clone(),
                empresa: d.empresa.or(d.ia).map(str::to_string),
                pais: d.pais.map(str::to_string),
                corredor: d
                    .corredor
                    .as_ref()
                    .map(|c| (c.nombre.to_string(), c.url_derechos.map(str::to_string))),
                clases: Vec::new(),
                primera: ts,
                ultima: ts,
                veces: 0,
                carta: None,
            });
        for c in clases {
            if !e.clases.contains(c) {
                e.clases.push(*c);
            }
        }
        e.ultima = e.ultima.max(ts);
        e.veces += 1;
    }

    /// Entries, most recent first.
    #[must_use]
    pub fn recientes(&self) -> Vec<&Entrada> {
        let mut v: Vec<&Entrada> = self.entradas.values().collect();
        v.sort_by_key(|e| std::cmp::Reverse(e.ultima));
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_book_keeps_one_line_per_site() {
        let mut l = Libro::default();
        l.anota("www.tienda.co", &[Clase::Correo], 10);
        l.anota("pagos.tienda.co", &[Clase::Correo, Clase::Telefono], 20);
        l.anota("33across.com", &[Clase::Correo], 5);
        assert_eq!(l.entradas.len(), 2);
        let t = &l.entradas["tienda.co"];
        assert_eq!(t.clases, vec![Clase::Correo, Clase::Telefono]);
        assert_eq!((t.primera, t.ultima, t.veces), (10, 20, 2));
        assert!(l.entradas["33across.com"].corredor.is_some());
        assert_eq!(l.recientes()[0].sitio, "tienda.co");
    }
}
