//! GUARDIANA ZERO, the browser: everything that decides, tested on every system.
//!
//! The window, the tabs and the hooks into the engine live in `zero/` (Windows, WebView2) and
//! ask this crate what each request is, whether it may leave, and what to write down. Nothing
//! here opens a connection: the browser's only traffic is the pages the person asks for.
// SECURE's state for the page is one big `json!` (16 Oct 2026: past the default limit).
#![recursion_limit = "256"]

pub mod campos;
pub mod cartas;
pub mod chivatos;
pub mod comunicaciones;
pub mod cortes;
pub mod datos;
pub mod decision;
pub mod destino;
pub mod direccion;
pub mod dominio;
pub mod escudo;
pub mod favoritos;
pub mod fecha;
pub mod filtros;
pub mod grupo;
pub mod libro;
pub mod licencia;
pub mod mandato;
mod md5;
pub mod medidas;
pub mod mensaje;
pub mod movil;
pub mod paginas;
pub mod puerta;
pub mod recibo;
pub mod rele;
pub mod sala;
pub mod secure;
pub mod servidor;
pub mod sesion;
pub mod tachon;
pub mod textos;
pub mod tinta;
pub mod url_limpia;
pub mod vigia;

/// The tests that set up conversations over real sockets take turns: on a slow runner (macOS,
/// three cores) two of them at once starved the three-computer group of its greeting
/// (17 Oct 2026).
#[cfg(test)]
pub(crate) static PRUEBAS_RED: std::sync::Mutex<()> = std::sync::Mutex::new(());
