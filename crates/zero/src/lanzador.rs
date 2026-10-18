//! How ZERO opens GUARDIANA's panel by itself (1.0.44): `--panel` from the Start menu, the
//! desktop and the end of the install, and `guardiana://activar?clave=…` from the licence email.
//!
//! Until 1.0.43 the shortcuts ran `guardiana.exe panel`, a console program: a black window for
//! three seconds, then ZERO empty, then a white wait page written to the temporary folder (the
//! owner's video of 1.0.43). Now ZERO opens at once on a dark wait page of its own, waits here
//! for the service in a thread of its own, reads the panel's key the same way the CLI does and
//! opens the panel in that same tab.
//!
//! The `guardiana:` scheme is registered on Windows only, by the MSI. On macOS and Linux nothing
//! registers it yet: the panel there is opened by `guardiana panel` in the person's own browser.

use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::textos::Textos;

/// The built-in wait page, served from the browser's own folder. Its name contains
/// `guardiana-arrancando.html` on purpose: the session already takes any address with it for
/// GUARDIANA's tab (its title, its place among the product tabs, its colour).
pub const ESPERA: &str = "https://zero.guardiana/guardiana-arrancando.html";

/// The same page saying the service never answered.
pub const ESPERA_SIN_SERVICIO: &str =
    "https://zero.guardiana/guardiana-arrancando.html?fallo=servicio";

/// The same page saying the panel's key could not be read.
pub const ESPERA_SIN_LLAVE: &str = "https://zero.guardiana/guardiana-arrancando.html?fallo=llave";

/// The wait page's file in the browser's own folder.
pub const ARCHIVO_ESPERA: &str = "guardiana-arrancando.html";

/// The panel's port on this computer (`guardiana_panel::DEFAULT_PORT`).
pub const PUERTO_PANEL: u16 = 7443;

/// The file the service writes the panel's session key to (`guardiana_panel::TOKEN_FILE`),
/// in the system's data folder (`%ProgramData%\Guardiana` on Windows).
pub const LLAVE_PANEL: &str = "panel.token";

/// How long ZERO waits for the service: a first start after the install, on a slow computer,
/// can take most of a minute.
pub const ESPERA_MAXIMA: Duration = Duration::from_secs(90);

/// The shortest and longest licence key a link may carry. The licence crate accepts any key
/// that is not empty once trimmed (`guardiana_license::activate_with_key`) and the gateway's keys
/// are letters, digits and dashes; a link is held to that, and to what the panel's field takes.
const CLAVE_MIN: usize = 8;
const CLAVE_MAX: usize = 100;

/// What the start of the program asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pedido {
    /// `--abrir-guardiana <url>`: an address the CLI already has (the panel with its key).
    AbreGuardiana(String),
    /// `--panel`: GUARDIANA's panel, waiting for the service if it is still coming up.
    Panel,
    /// `--url guardiana://activar?clave=…`: the panel's licence page with this key filled in.
    Activar(String),
}

impl Pedido {
    /// The key to fill in, if any.
    #[must_use]
    pub fn clave(&self) -> Option<&str> {
        match self {
            Self::Activar(c) => Some(c.as_str()),
            _ => None,
        }
    }
}

/// What the program was started with (without the program's own name). Anything else, or a
/// `guardiana:` link that is not exactly an activation with a valid key, is no request at all:
/// ZERO opens as it always does.
#[must_use]
pub fn pedido_de_argumentos(args: &[String]) -> Option<Pedido> {
    match args {
        [a, url] if a == "--abrir-guardiana" => Some(Pedido::AbreGuardiana(url.clone())),
        [a] if a == "--panel" => Some(Pedido::Panel),
        [a, url] if a == "--url" => clave_de_enlace(url).map(Pedido::Activar),
        _ => None,
    }
}

/// Whether a licence key has only what a key can have: ASCII letters, digits, `-` and `_`,
/// between [`CLAVE_MIN`] and [`CLAVE_MAX`] characters.
#[must_use]
pub fn es_clave_valida(clave: &str) -> bool {
    (CLAVE_MIN..=CLAVE_MAX).contains(&clave.len())
        && clave
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// The key of `guardiana://activar?clave=<key>`. The scheme and the host are compared without
/// case (Windows and browsers may change it) and must be exactly those; one slash before the
/// query is allowed (some mail programs add it), nothing else: no other parameter, no fragment,
/// no user, no port, no path.
#[must_use]
pub fn clave_de_enlace(url: &str) -> Option<String> {
    const ESQUEMA: &str = "guardiana://activar";
    let url = url.trim();
    if url.len() > ESQUEMA.len() + 16 + CLAVE_MAX || !url.is_char_boundary(ESQUEMA.len()) {
        return None;
    }
    let (cabeza, resto) = url.split_at(ESQUEMA.len());
    if !cabeza.eq_ignore_ascii_case(ESQUEMA) {
        return None;
    }
    let resto = resto.strip_prefix('/').unwrap_or(resto);
    let clave = resto.strip_prefix("?clave=")?;
    es_clave_valida(clave).then(|| clave.to_owned())
}

/// Where the service keeps the panel's key: the same file `guardiana panel` reads.
#[must_use]
pub fn ruta_llave() -> PathBuf {
    guardiana_core::paths::system_data_dir().join(LLAVE_PANEL)
}

/// The panel's key from its file, trimmed, only when it is whole: 64 hexadecimal characters,
/// the panel's own test (the service's write is not atomic).
#[must_use]
pub fn lee_llave(ruta: &Path) -> Option<String> {
    let t = std::fs::read_to_string(ruta).ok()?;
    let t = t.trim();
    (t.len() == 64 && t.bytes().all(|b| b.is_ascii_hexdigit())).then(|| t.to_owned())
}

/// The panel's address with its key, or its licence page with `clave` in the fragment: the
/// fragment never leaves the browser, so the key reaches no server and no log; the page reads
/// it and takes it out of the address at once.
#[must_use]
pub fn enlace(llave: &str, clave: Option<&str>) -> String {
    match clave {
        Some(c) => format!("http://127.0.0.1:{PUERTO_PANEL}/licencia?t={llave}#clave={c}"),
        None => format!("http://127.0.0.1:{PUERTO_PANEL}/?t={llave}"),
    }
}

/// Whether the panel answers on its port right now.
#[must_use]
pub fn panel_responde() -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], PUERTO_PANEL));
    TcpStream::connect_timeout(&addr, Duration::from_millis(400)).is_ok()
}

/// Why the panel could not be opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FalloPanel {
    /// The service never answered.
    Servicio,
    /// The service answered but its key could not be read.
    Llave,
}

impl FalloPanel {
    /// The wait page that says so.
    #[must_use]
    pub const fn pagina(self) -> &'static str {
        match self {
            Self::Servicio => ESPERA_SIN_SERVICIO,
            Self::Llave => ESPERA_SIN_LLAVE,
        }
    }
}

/// The panel's key as soon as the panel answers and its key is whole, if that happens before
/// `hasta`. Blocks: it runs on a thread of its own, never on the window's.
///
/// # Errors
///
/// [`FalloPanel::Servicio`] when the panel never answered, [`FalloPanel::Llave`] when it
/// answered but the key could not be read.
pub fn espera_llave(ruta: &Path, hasta: Duration) -> Result<String, FalloPanel> {
    espera_llave_con(ruta, hasta, panel_responde)
}

fn espera_llave_con(
    ruta: &Path,
    hasta: Duration,
    responde: impl Fn() -> bool,
) -> Result<String, FalloPanel> {
    let fin = Instant::now() + hasta;
    loop {
        let arriba = responde();
        if arriba {
            if let Some(llave) = lee_llave(ruta) {
                return Ok(llave);
            }
        }
        if Instant::now() >= fin {
            return Err(if arriba {
                FalloPanel::Llave
            } else {
                FalloPanel::Servicio
            });
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

/// The address to open right now, without waiting, when the panel already answers.
#[must_use]
pub fn enlace_si_listo(clave: Option<&str>) -> Option<String> {
    if !panel_responde() {
        return None;
    }
    lee_llave(&ruta_llave()).map(|llave| enlace(&llave, clave))
}

fn escapa(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// A text as a JavaScript string inside a `<script>`: JSON, with `<` written so that no text can
/// close the script.
fn cadena_js(s: &str) -> String {
    serde_json::to_string(s)
        .unwrap_or_else(|_| "\"\"".to_owned())
        .replace('<', "\\u003c")
}

/// The wait page: the logo, one sentence and a thin moving line, in the program's dark colours,
/// with nothing from outside (no fonts, no images, no files to fetch). With `?fallo=servicio` or
/// `?fallo=llave` it says instead what happened and what to do.
#[must_use]
pub fn pagina_espera(t: &Textos) -> String {
    format!(
        r##"<!doctype html>
<html lang="{lang}"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="referrer" content="no-referrer"><meta name="color-scheme" content="dark"><title>GUARDIANA</title>
<style>
html,body{{height:100%;margin:0;background:#06080C}}
body{{color:#E8ECF1;font-family:"IBM Plex Sans","Segoe UI",system-ui,sans-serif;display:flex;align-items:center;justify-content:center;text-align:center}}
main{{max-width:520px;padding:32px 24px}}
.logo{{width:104px;height:104px;margin:0 auto 22px;display:block}}
h1{{font-weight:700;letter-spacing:.18em;font-size:20px;margin:0 0 14px}}
p{{font-size:16px;line-height:1.5;color:#A9B3BE;margin:0}}
.barra{{height:2px;width:160px;margin:24px auto 0;background:#1E2631;border-radius:2px;overflow:hidden}}
.barra i{{display:block;height:100%;width:35%;background:#9CC3E4;border-radius:2px;animation:va 2.4s ease-in-out infinite}}
@keyframes va{{0%{{transform:translateX(-120%)}}100%{{transform:translateX(340%)}}}}
.fallo .barra{{display:none}}
.fallo p{{color:#E8ECF1}}
</style></head>
<body><main>
<svg class="logo" viewBox="0 0 100 100" aria-hidden="true"><rect width="100" height="100" rx="22" fill="#0C1118"/><g transform="translate(9 7) scale(.82)"><defs><mask id="g"><rect width="100" height="100" fill="#fff"/><path d="M63 58.29 V55.07 Q63 54 58.71 54 H44.15 Q37 54 37 61.15 V72.85 Q37 80 44.15 80 H58.71 Q63 80 63 75.71 V67 H51" fill="none" stroke="#000" stroke-width="7.5"/></mask></defs><path d="M31 44 V31 A19 19 0 0 1 69 31 V44" fill="none" stroke="#E8ECF1" stroke-width="9"/><rect x="17" y="42" width="66" height="50" rx="13" fill="#9CC3E4" mask="url(#g)"/></g></svg>
<h1>GUARDIANA</h1>
<p id="texto" role="status" aria-live="polite">{espera}</p>
<div class="barra"><i></i></div>
<script>
(function(){{
  var f={{servicio:{servicio},llave:{llave}}};
  var q=new URLSearchParams(location.search).get("fallo");
  if(q&&Object.prototype.hasOwnProperty.call(f,q)){{
    document.getElementById("texto").textContent=f[q];
    document.body.className="fallo";
  }}
}})();
</script>
</main></body></html>
"##,
        lang = t.idioma().codigo(),
        espera = escapa(&t.t("lanzador_esperando")),
        servicio = cadena_js(&t.t("lanzador_sin_servicio")),
        llave = cadena_js(&t.t("lanzador_sin_llave")),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::textos::Idioma;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn the_start_arguments_are_read_exactly() {
        assert_eq!(
            pedido_de_argumentos(&args(&["--panel"])),
            Some(Pedido::Panel)
        );
        assert_eq!(
            pedido_de_argumentos(&args(&["--abrir-guardiana", "x"])),
            Some(Pedido::AbreGuardiana("x".into()))
        );
        assert_eq!(
            pedido_de_argumentos(&args(&[
                "--url",
                "guardiana://activar?clave=ABCD-1234-EFGH"
            ])),
            Some(Pedido::Activar("ABCD-1234-EFGH".into()))
        );
        assert_eq!(pedido_de_argumentos(&args(&[])), None);
        assert_eq!(pedido_de_argumentos(&args(&["--panel", "x"])), None);
        assert_eq!(pedido_de_argumentos(&args(&["--url"])), None);
        assert_eq!(
            pedido_de_argumentos(&args(&["--url", "https://example.com/?clave=ABCD-1234"])),
            None
        );
    }

    #[test]
    fn only_an_exact_activation_link_gives_a_key() {
        let k = "3F2B9C1A-7D4E-4A11-9B0C-55E6D7F8A9B0";
        assert_eq!(
            clave_de_enlace(&format!("guardiana://activar?clave={k}")).as_deref(),
            Some(k)
        );
        assert_eq!(
            clave_de_enlace(&format!("GUARDIANA://Activar/?clave={k}")).as_deref(),
            Some(k)
        );
        for malo in [
            format!("guardiana://activar?clave={k}&otra=1"),
            format!("guardiana://activar?clave={k}#x"),
            format!("guardiana://activarx?clave={k}"),
            format!("guardiana://activar.evil.com?clave={k}"),
            format!("guardiana://user@activar?clave={k}"),
            format!("guardiana://activar:80?clave={k}"),
            format!("guardiana://activar/otra?clave={k}"),
            format!("guardiana:activar?clave={k}"),
            format!("guardianas://activar?clave={k}"),
            format!("guardiana://activar?key={k}"),
            "guardiana://activar?clave=".to_owned(),
            "guardiana://activar?clave=corta".to_owned(),
            "guardiana://activar?clave=con%20espacio".to_owned(),
            "guardiana://activar?clave=<script>alert(1)</script>".to_owned(),
            "guardiana://activar?clave=ÁÉÍÓÚáéíóú".to_owned(),
            format!("guardiana://activar?clave={}", "A".repeat(CLAVE_MAX + 1)),
            "guardiana://a".to_owned(),
            "guardiana://activ".to_owned(),
            "guardiana://acti\u{e1}r?clave=ABCDEFGH".to_owned(),
        ] {
            assert_eq!(clave_de_enlace(&malo), None, "{malo}");
        }
        assert!(es_clave_valida(&"a".repeat(CLAVE_MAX)));
        assert!(!es_clave_valida("abc.defgh"));
    }

    #[test]
    fn the_key_file_is_read_trimmed_and_only_when_whole() {
        let d =
            std::env::temp_dir().join(format!("guardiana-zero-lanzador-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&d);
        let r = d.join(LLAVE_PANEL);
        let llave = "ab".repeat(32);
        let _ = std::fs::write(&r, format!("{llave}\r\n"));
        assert_eq!(lee_llave(&r).as_deref(), Some(llave.as_str()));
        let _ = std::fs::write(&r, &llave[..40]);
        assert_eq!(lee_llave(&r), None);
        let _ = std::fs::write(&r, "z".repeat(64));
        assert_eq!(lee_llave(&r), None);
        assert_eq!(lee_llave(&d.join("no-existe")), None);

        // The wait: the key once the panel answers; the right failure otherwise.
        let _ = std::fs::write(&r, &llave);
        assert_eq!(
            espera_llave_con(&r, Duration::from_millis(10), || true).as_deref(),
            Ok(llave.as_str())
        );
        assert_eq!(
            espera_llave_con(&r, Duration::from_millis(10), || false),
            Err(FalloPanel::Servicio)
        );
        let _ = std::fs::write(&r, "");
        assert_eq!(
            espera_llave_con(&r, Duration::from_millis(10), || true),
            Err(FalloPanel::Llave)
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_panel_address_carries_the_key_only_in_the_fragment() {
        let llave = "c".repeat(64);
        assert_eq!(
            enlace(&llave, None),
            format!("http://127.0.0.1:7443/?t={llave}")
        );
        let e = enlace(&llave, Some("ABCD-1234-EFGH"));
        assert_eq!(
            e,
            format!("http://127.0.0.1:7443/licencia?t={llave}#clave=ABCD-1234-EFGH")
        );
        assert!(!e.split('#').next().unwrap_or_default().contains("ABCD"));
    }

    #[test]
    fn the_wait_page_is_dark_self_contained_and_in_the_three_languages() {
        for idioma in [Idioma::Es, Idioma::En, Idioma::Pt] {
            let t = Textos::de(idioma);
            for k in [
                "lanzador_esperando",
                "lanzador_sin_servicio",
                "lanzador_sin_llave",
            ] {
                assert_ne!(t.t(k), k);
            }
            let h = pagina_espera(&t);
            assert!(h.contains("background:#06080C"));
            assert!(h.contains(&escapa(&t.t("lanzador_esperando"))));
            assert!(h.contains(&format!("lang=\"{}\"", idioma.codigo())));
            // Nothing from outside: no address at all, no file to fetch.
            assert!(!h.contains("http://") && !h.contains("https://"));
            assert!(!h.contains("src="));
        }
        assert!(ESPERA_SIN_SERVICIO.starts_with(ESPERA));
        assert!(ESPERA_SIN_LLAVE.starts_with(ESPERA));
        assert!(ESPERA.ends_with(ARCHIVO_ESPERA));
    }
}
