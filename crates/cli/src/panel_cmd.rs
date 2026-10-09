//! `guardiana panel`: open the panel in the browser with the session token
//! from the token file (the launcher of brief §8).

use std::error::Error;

use guardiana_core::{i18n, paths};

use crate::args::Opts;

/// How long to wait for the service to be up before opening the browser. The installer runs
/// this right after starting (or restarting, on an update) the service, and a browser opened
/// one second too early showed "cannot connect" or nothing at all: the person thought the
/// program had stayed in the background (owner's report, 8 Oct 2026).
const ESPERA_ARRANQUE: std::time::Duration = std::time::Duration::from_secs(20);

/// Whether the panel answers on its port right now.
fn panel_responde() -> bool {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], guardiana_panel::DEFAULT_PORT));
    std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(500)).is_ok()
}

/// Waits, up to [`ESPERA_ARRANQUE`], for the token file to exist (the service writes it as it
/// starts, a second or two in). Returns whether it did.
fn esperar_la_llave(path: &std::path::Path) -> bool {
    let fin = std::time::Instant::now() + ESPERA_ARRANQUE;
    while !path.exists() {
        if std::time::Instant::now() >= fin {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    true
}

/// The page the browser shows while the service comes up: the logo, one sentence, and a
/// script that asks the panel every half second and goes there with the key as soon as it
/// answers. Until 1.0.5 that wait was a black console window with a line of text, and the
/// owner asked for something with the logo, more professional (8 Oct 2026). Everything is in
/// the file: no fonts, no scripts, no network but the panel's own port.
fn pagina_de_espera(t: &'static i18n::Texts, url: &str) -> String {
    let esc = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    };
    let url_js = serde_json::to_string(url).unwrap_or_else(|_| "\"\"".to_owned());
    format!(
        r##"<!doctype html>
<html lang="{lang}"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="referrer" content="no-referrer"><title>GUARDIANA</title>
<style>
html,body{{height:100%;margin:0}}
body{{background:#070A10;color:#E8EDF5;font-family:"IBM Plex Sans","Segoe UI",system-ui,-apple-system,Roboto,sans-serif;display:flex;align-items:center;justify-content:center;text-align:center}}
main{{max-width:520px;padding:32px 24px}}
.logo{{width:112px;height:112px;margin:0 auto 22px;display:block}}
.anillo{{transform-origin:50% 50%;animation:latido 1.8s ease-in-out infinite}}
@keyframes latido{{0%,100%{{opacity:.55;transform:scale(.96)}}50%{{opacity:1;transform:scale(1)}}}}
h1{{font-family:"Unbounded","IBM Plex Sans","Segoe UI",system-ui,sans-serif;font-weight:700;letter-spacing:.18em;font-size:22px;margin:0 0 14px}}
p{{font-size:16px;line-height:1.5;color:#AEB8CB;margin:0 0 10px}}
.barra{{height:3px;width:220px;margin:22px auto 0;background:#152033;border-radius:3px;overflow:hidden}}
.barra i{{display:block;height:100%;width:40%;background:#62E6FF;border-radius:3px;animation:va 1.6s ease-in-out infinite}}
@keyframes va{{0%{{transform:translateX(-120%)}}100%{{transform:translateX(320%)}}}}
#tarda{{display:none;color:#8C97AB;font-size:14px;margin-top:18px}}
a{{color:#62E6FF}}
</style></head>
<body><main>
<svg class="logo" viewBox="0 0 200 200" aria-hidden="true"><rect x="8" y="8" width="184" height="184" rx="40" fill="#0B1220" stroke="#17324A" stroke-width="2"/><g class="anillo"><circle cx="100" cy="100" r="42" fill="none" stroke="#62E6FF" stroke-width="12"/><circle cx="100" cy="100" r="14" fill="#62E6FF"/></g></svg>
<h1>GUARDIANA</h1>
<p>{texto}</p>
<div class="barra"><i></i></div>
<p id="tarda">{tarda}</p>
<script>
(function(){{
  var url={url_js}, inicio=Date.now();
  function mirar(){{
    fetch("http://127.0.0.1:{port}/api/textos",{{mode:"no-cors",cache:"no-store"}}).then(function(){{location.replace(url);}}).catch(function(){{
      if(Date.now()-inicio>25000){{document.getElementById("tarda").style.display="block";}}
      setTimeout(mirar,600);
    }});
  }}
  mirar();
}})();
</script>
</main></body></html>
"##,
        lang = esc(i18n::code_of(t)),
        texto = esc(t.cli("panel.esperando")),
        tarda = esc(t.cli("panel.tarda")),
        port = guardiana_panel::DEFAULT_PORT,
    )
}

/// Where the wait page is written: the user's own temporary folder, readable by nobody else
/// (it carries the session key in its link).
fn ruta_de_espera() -> std::path::PathBuf {
    std::env::temp_dir().join("guardiana-arrancando.html")
}

/// On Windows, `guardiana panel` from the Start menu shortcut comes with a console window of
/// its own, and that black window was the whole "screen" while the service came up. It is
/// released at once: what the person sees is the page in the browser.
///
/// `unsafe` is forbidden in this project unless justified (CLAUDE.md), and this is the
/// justification: `FreeConsole` takes no pointers and only detaches this process from its
/// console; the process keeps running, nothing here writes to the console afterwards, and the
/// libraries the project uses have no safe wrapper for it. Its result is ignored on purpose: with
/// no console to release there is nothing to do.
#[cfg(windows)]
#[allow(unsafe_code)]
fn soltar_consola() {
    // SAFETY: see above; no pointers, no handles, no state shared with anything else.
    let _ = unsafe { windows_sys::Win32::System::Console::FreeConsole() };
}

pub fn run(_opts: &Opts) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    let path = paths::data_dir().join(guardiana_panel::TOKEN_FILE);
    let arrancando = !panel_responde();
    if arrancando {
        println!("{}", t.cli("panel.esperando"));
        esperar_la_llave(&path);
    }
    let token = std::fs::read_to_string(&path).map_err(|e| {
        // On Linux and the Mac the key belongs to the service, and a person who types
        // `guardiana panel` without sudo was asked whether Guardiana was running, which it was
        // (review of 5 Oct 2026, Linux medium). The real reason, and what to type.
        let clave =
            if e.kind() == std::io::ErrorKind::PermissionDenied || paths::unreadable_here(&path) {
                // On Windows "sudo" does not exist: another sentence.
                if cfg!(windows) {
                    "panel.sin_permiso_windows"
                } else {
                    "panel.sin_permiso"
                }
            } else {
                "panel.sin_token"
            };
        t.cli(clave).replace("{ruta}", &path.display().to_string())
    })?;
    let url = format!(
        "http://127.0.0.1:{}/?t={}",
        guardiana_panel::DEFAULT_PORT,
        token.trim()
    );
    // Through sudo, the browser would open as the administrator, or not at all: the link is
    // given to the person to open in their own.
    if std::env::var_os("SUDO_USER").is_some() {
        println!("{}", t.cli("panel.enlace_sudo").replace("{url}", &url));
        return Ok(());
    }
    if arrancando && !panel_responde() {
        // The service is still coming up: the browser opens on the wait page, which goes to
        // the panel by itself the moment it answers.
        let espera = ruta_de_espera();
        if std::fs::write(&espera, pagina_de_espera(t, &url)).is_ok() {
            println!("{}", t.cli("panel.abriendo_espera"));
            #[cfg(windows)]
            soltar_consola();
            crate::engine::open_in_browser(&espera.display().to_string());
            return Ok(());
        }
        // The page could not be written: the old wait, in the terminal.
        let fin = std::time::Instant::now() + ESPERA_ARRANQUE;
        while !panel_responde() && std::time::Instant::now() < fin {
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
    }
    println!("{}", t.cli("panel.abriendo").replace("{url}", &url));
    #[cfg(windows)]
    soltar_consola();
    crate::engine::open_in_browser(&url);
    Ok(())
}
