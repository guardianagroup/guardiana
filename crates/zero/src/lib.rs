//! GUARDIANA ZERO, the browser: everything that decides, tested on every system.
//!
//! The window, the tabs and the hooks into the engine live in `zero/` (Windows, WebView2) and
//! ask this crate what each request is, whether it may leave, and what to write down. Nothing
//! here opens a connection: the browser's only traffic is the pages the person asks for.

pub mod cartas;
pub mod cortes;
pub mod datos;
pub mod decision;
pub mod destino;
pub mod direccion;
pub mod dominio;
pub mod escudo;
pub mod favoritos;
pub mod fecha;
pub mod libro;
pub mod licencia;
pub mod mandato;
mod md5;
pub mod paginas;
pub mod recibo;
pub mod sesion;
pub mod tachon;
pub mod textos;
pub mod tinta;
pub mod url_limpia;
