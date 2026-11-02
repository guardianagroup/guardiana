//! GUARDIANA ZERO for Windows: the window, the tabs and the hooks into the web engine
//! (Microsoft Edge WebView2, kept up to date by Windows).
//!
//! Every decision is made by `guardiana-zero` (crates/zero), which is tested on every system:
//! this program tells it what happened (a message from the bar, a navigation, a request about to
//! leave) and does exactly what it answers. Nothing here decides what is cut or counted.
#![cfg_attr(windows, windows_subsystem = "windows")]
// The window, the engine and the system are reached through Win32 and COM, and every call into
// them is `unsafe` by the bindings' own definition. This program is that thin layer and nothing
// else; the crate that decides (`guardiana-zero`) has no `unsafe` at all.
#![allow(unsafe_code)]

#[cfg(windows)]
mod app;
#[cfg(windows)]
mod avisos;
#[cfg(windows)]
mod interfaz;

fn main() {
    // `--avisos`: the notices of el escudo's grave warnings (1.0.52), with no window and nothing else of the browser.
    #[cfg(windows)]
    if std::env::args().nth(1).as_deref() == Some("--avisos") {
        avisos::corre();
        return;
    }
    // The rule lists (EasyList, EasyPrivacy, uBlock Origin, URLhaus) are compiled while the
    // window opens, so the first page does not wait for them.
    std::thread::spawn(|| {
        let _ = guardiana_zero::filtros::motor();
        let _ = guardiana_zero::filtros::cosmetica();
    });
    #[cfg(windows)]
    app::arranca();
    #[cfg(not(windows))]
    eprintln!("GUARDIANA ZERO funciona en Windows 10 y 11.");
}
