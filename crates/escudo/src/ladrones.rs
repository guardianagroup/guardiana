//! «Ladrones de datos» (phase A3; the owner, 1 Nov 2026, 18:55: «que vea todo lo posible, todo, absolutamente todo»).
//!
//! The commonest malware on personal computers today steals what is already saved: the browser's passwords and
//! cookies (with the cookies, open sessions are taken over without a password), the key that unlocks both, the
//! autofill and cards, crypto wallets, and the sessions of Discord, Telegram or Steam. To do it, it has to open those
//! files, and nothing but the browser (or the app itself) has any reason to. This module knows where those files live
//! ([`que_es`]) and who may open them ([`Hallazgo::duenos`]); [`juzga`] turns an opening by anybody else into a warning,
//! louder when the program has no signature.
//!
//! Only names are looked at: which program opened which kind of file. Nothing is read from those files.

use crate::primera_vez::{Aviso, Firma};

/// What kind of saved secret a file holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tesoro {
    /// Saved passwords.
    Contrasenas,
    /// Cookies: the open sessions.
    Cookies,
    /// The key the browser locks passwords and cookies with.
    Llave,
    /// Autofill and saved cards.
    Tarjetas,
    /// A crypto wallet.
    Cartera,
    /// A messaging or games session.
    Sesion,
}

impl Tesoro {
    /// Its name in the warnings' code (`contrasenas`, `cookies`…).
    #[must_use]
    pub fn codigo(self) -> &'static str {
        match self {
            Self::Contrasenas => "contrasenas",
            Self::Cookies => "cookies",
            Self::Llave => "llave",
            Self::Tarjetas => "tarjetas",
            Self::Cartera => "cartera",
            Self::Sesion => "sesion",
        }
    }
}

/// A file that holds a saved secret, and whose it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hallazgo {
    /// What it holds.
    pub tesoro: Tesoro,
    /// Whose it is, as people know it («Chrome», «Discord»…).
    pub de: &'static str,
    /// The programs (file names, lower case) that may open it: the app itself.
    pub duenos: &'static [&'static str],
}

/// The Chromium family: where each one keeps its profile, and its own program.
const CHROMIUM: &[(&str, &str, &[&str])] = &[
    (
        r"\appdata\local\google\chrome\user data\",
        "Chrome",
        &["chrome.exe"],
    ),
    (
        r"\appdata\local\google\chrome beta\user data\",
        "Chrome",
        &["chrome.exe"],
    ),
    (
        r"\appdata\local\google\chrome sxs\user data\",
        "Chrome",
        &["chrome.exe"],
    ),
    (
        r"\appdata\local\microsoft\edge\user data\",
        "Edge",
        &["msedge.exe"],
    ),
    (
        r"\appdata\local\bravesoftware\brave-browser\user data\",
        "Brave",
        &["brave.exe"],
    ),
    (
        r"\appdata\local\vivaldi\user data\",
        "Vivaldi",
        &["vivaldi.exe"],
    ),
    (
        r"\appdata\local\chromium\user data\",
        "Chromium",
        &["chrome.exe"],
    ),
    (
        r"\appdata\local\yandex\yandexbrowser\user data\",
        "Yandex",
        &["browser.exe"],
    ),
    (
        r"\appdata\roaming\opera software\opera stable\",
        "Opera",
        &["opera.exe"],
    ),
    (
        r"\appdata\roaming\opera software\opera gx stable\",
        "Opera GX",
        &["opera.exe"],
    ),
];

/// Wallet extensions people use in Chromium browsers (their public extension identifiers).
const CARTERAS_EXTENSION: &[(&str, &str)] = &[
    ("nkbihfbeogaeaoehlefnkodbefgpgknn", "MetaMask"),
    ("bfnaelmomeimhlpmgjnjophhpkkoljpa", "Phantom"),
    ("hnfanknocfeofbddgcijnmhnfnkdnaad", "Coinbase Wallet"),
    ("fhbohimaelbohpjbbldcngcnapndodjp", "Binance Wallet"),
    ("ibnejdfjmmkpcnlpebklmnkoeoihofec", "TronLink"),
];

/// Apps that keep a wallet or a session in a folder of their own.
const APLICACIONES: &[(&str, Tesoro, &str, &[&str])] = &[
    (
        r"\appdata\roaming\exodus\exodus.wallet\",
        Tesoro::Cartera,
        "Exodus",
        &["exodus.exe"],
    ),
    (
        r"\appdata\roaming\electrum\wallets\",
        Tesoro::Cartera,
        "Electrum",
        &[],
    ),
    (
        r"\appdata\roaming\ethereum\keystore\",
        Tesoro::Cartera,
        "Ethereum",
        &["geth.exe"],
    ),
    (
        r"\appdata\roaming\atomic\local storage\leveldb\",
        Tesoro::Cartera,
        "Atomic Wallet",
        &["atomic wallet.exe"],
    ),
    (
        r"\appdata\roaming\discord\local storage\leveldb\",
        Tesoro::Sesion,
        "Discord",
        &["discord.exe"],
    ),
    (
        r"\appdata\roaming\telegram desktop\tdata\",
        Tesoro::Sesion,
        "Telegram",
        &["telegram.exe"],
    ),
    (
        r"\steam\config\loginusers.vdf",
        Tesoro::Sesion,
        "Steam",
        &["steam.exe", "steamwebhelper.exe", "steamservice.exe"],
    ),
];

/// Programs of the system and of Microsoft Defender that read every file as part of their job.
const SISTEMA: &[&str] = &[
    "msmpeng.exe",
    "mpdefendercoreservice.exe",
    "mssense.exe",
    "nissrv.exe",
    "searchindexer.exe",
    "searchprotocolhost.exe",
    "searchfilterhost.exe",
];

/// Whether a file (any path Windows gives: `C:\…`, `\\?\C:\…` or `\Device\HarddiskVolume3\…`) holds a saved secret,
/// and whose. A cheap test on the lower-case path: it runs for every file opened on the computer.
#[must_use]
pub fn que_es(ruta: &str) -> Option<Hallazgo> {
    let r = ruta.replace('/', "\\").to_ascii_lowercase();
    if !r.contains(r"\appdata\") && !r.contains(r"\steam\") {
        return None;
    }
    for (carpeta, tesoro, de, duenos) in APLICACIONES {
        if r.contains(carpeta) {
            return Some(Hallazgo {
                tesoro: *tesoro,
                de: *de,
                duenos: *duenos,
            });
        }
    }
    if let Some(i) = r.find(r"\appdata\roaming\mozilla\firefox\profiles\") {
        let resto = &r[i..];
        let tesoro = if resto.ends_with(r"\logins.json") {
            Tesoro::Contrasenas
        } else if resto.ends_with(r"\key4.db") {
            Tesoro::Llave
        } else if resto.ends_with(r"\cookies.sqlite") {
            Tesoro::Cookies
        } else {
            return None;
        };
        return Some(Hallazgo {
            tesoro,
            de: "Firefox",
            duenos: &["firefox.exe"],
        });
    }
    for (carpeta, de, duenos) in CHROMIUM {
        let Some(i) = r.find(carpeta) else {
            continue;
        };
        let resto = &r[i + carpeta.len()..];
        let tesoro = if let Some(id) = resto
            .split('\\')
            .skip_while(|p| *p != "local extension settings" && *p != "indexeddb")
            .nth(1)
        {
            match CARTERAS_EXTENSION.iter().find(|(e, _)| id.contains(e)) {
                Some((_, cartera)) => {
                    return Some(Hallazgo {
                        tesoro: Tesoro::Cartera,
                        de: *cartera,
                        duenos: *duenos,
                    })
                }
                None => return None,
            }
        } else if resto == "local state" {
            Tesoro::Llave
        } else if resto.ends_with(r"\login data")
            || resto.ends_with(r"\login data for account")
            || resto == "login data"
        {
            Tesoro::Contrasenas
        } else if resto.ends_with(r"\network\cookies")
            || resto.ends_with(r"\cookies")
            || resto == "cookies"
        {
            Tesoro::Cookies
        } else if resto.ends_with(r"\web data") || resto == "web data" {
            Tesoro::Tarjetas
        } else {
            return None;
        };
        return Some(Hallazgo {
            tesoro,
            de: *de,
            duenos: *duenos,
        });
    }
    None
}

/// Whether `programa` (its file name) may open what was found: the app itself or a program of the system that reads
/// every file. A program called like the app but without its signature does not count as the app (see [`juzga`]).
#[must_use]
pub fn es_de_casa(programa: &str, h: &Hallazgo) -> bool {
    let p = programa.to_ascii_lowercase();
    h.duenos.contains(&p.as_str()) || SISTEMA.contains(&p.as_str())
}

/// Turns «`programa` opened `archivo`» into a warning, when the file holds a saved secret and the program is not the
/// app's own, nor a program of the system. A program named like the app but without a signature is a disguise and
/// warns too. Louder (`grave`) without a signature.
#[must_use]
pub fn juzga(
    programa: &str,
    ruta: &str,
    firma: &Firma,
    archivo: &str,
    cuando_ms: u64,
) -> Option<(Aviso, bool)> {
    let h = que_es(archivo)?;
    let sin_firma = *firma == Firma::SinFirma;
    let disfrazado = h.duenos.contains(&programa.to_ascii_lowercase().as_str()) && sin_firma;
    if es_de_casa(programa, &h) && !disfrazado {
        return None;
    }
    let que = match h.tesoro {
        Tesoro::Contrasenas => format!("las contraseñas guardadas de {}", h.de),
        Tesoro::Cookies => format!(
            "las cookies de {} (con ellas se pueden abrir tus sesiones sin contraseña)",
            h.de
        ),
        Tesoro::Llave => format!(
            "la llave con la que {} protege tus contraseñas y cookies",
            h.de
        ),
        Tesoro::Tarjetas => format!("el autorrelleno y las tarjetas guardadas de {}", h.de),
        Tesoro::Cartera => format!("la cartera de criptomonedas {}", h.de),
        Tesoro::Sesion => format!("tu sesión de {}", h.de),
    };
    let quien = match firma {
        Firma::SinFirma if disfrazado => {
            " Se llama como el programa de verdad, pero no tiene su firma.".to_string()
        }
        Firma::SinFirma => {
            " No tiene firma: es lo que hacen los programas que roban datos.".to_string()
        }
        Firma::Firmado(f) => format!(
            " Lo firma {f}: si no lo has pedido tú (por ejemplo, importar contraseñas), revísalo."
        ),
        Firma::SinComprobar => String::new(),
    };
    let texto = format!("«{programa}» ha abierto {que}.{quien}");
    Some((
        Aviso {
            regla: "ladron",
            programa: programa.to_string(),
            ruta: ruta.to_string(),
            sitio: h.de.to_string(),
            nombre: h.tesoro.codigo().to_string(),
            cuando_ms,
            fuente: None,
            texto,
        },
        sin_firma,
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const CHROME: &str = r"C:\Users\ana\AppData\Local\Google\Chrome\User Data\Default\Login Data";

    #[test]
    fn donde_guarda_cada_uno_sus_secretos() {
        let t = |r: &str| que_es(r).map(|h| (h.tesoro, h.de));
        assert_eq!(t(CHROME), Some((Tesoro::Contrasenas, "Chrome")));
        assert_eq!(
            t(
                r"\Device\HarddiskVolume3\Users\ana\AppData\Local\Google\Chrome\User Data\Profile 2\Network\Cookies"
            ),
            Some((Tesoro::Cookies, "Chrome"))
        );
        assert_eq!(
            t(r"C:\Users\ana\AppData\Local\Microsoft\Edge\User Data\Local State"),
            Some((Tesoro::Llave, "Edge"))
        );
        assert_eq!(
            t(r"C:\Users\ana\AppData\Local\BraveSoftware\Brave-Browser\User Data\Default\Web Data"),
            Some((Tesoro::Tarjetas, "Brave"))
        );
        assert_eq!(
            t(r"C:\Users\ana\AppData\Roaming\Mozilla\Firefox\Profiles\x1.default\logins.json"),
            Some((Tesoro::Contrasenas, "Firefox"))
        );
        assert_eq!(
            t(r"C:\Users\ana\AppData\Roaming\Mozilla\Firefox\Profiles\x1.default\key4.db"),
            Some((Tesoro::Llave, "Firefox"))
        );
        assert_eq!(
            t(
                r"C:\Users\ana\AppData\Local\Google\Chrome\User Data\Default\Local Extension Settings\nkbihfbeogaeaoehlefnkodbefgpgknn\000003.log"
            ),
            Some((Tesoro::Cartera, "MetaMask"))
        );
        assert_eq!(
            t(r"C:\Users\ana\AppData\Roaming\discord\Local Storage\leveldb\000005.ldb"),
            Some((Tesoro::Sesion, "Discord"))
        );
        assert_eq!(
            t(r"C:\Users\ana\AppData\Roaming\Exodus\exodus.wallet\seed.seco"),
            Some((Tesoro::Cartera, "Exodus"))
        );
        // lo que no guarda secretos
        assert_eq!(
            t(r"C:\Users\ana\AppData\Local\Google\Chrome\User Data\Default\History"),
            None
        );
        assert_eq!(
            t(
                r"C:\Users\ana\AppData\Local\Google\Chrome\User Data\Default\Local Extension Settings\otraextension\1.log"
            ),
            None
        );
        assert_eq!(t(r"C:\Users\ana\Documents\Login Data"), None);
        assert_eq!(t(r"C:\Windows\System32\kernel32.dll"), None);
    }

    #[test]
    fn el_navegador_abre_lo_suyo_sin_avisar() {
        let firmado = Firma::Firmado("Google LLC".into());
        assert_eq!(
            juzga(
                "chrome.exe",
                r"C:\Program Files\Google\Chrome\Application\chrome.exe",
                &firmado,
                CHROME,
                1
            ),
            None
        );
        assert_eq!(
            juzga(
                "MsMpEng.exe",
                r"C:\ProgramData\Microsoft\Windows Defender\MsMpEng.exe",
                &firmado,
                CHROME,
                1
            ),
            None
        );
        assert_eq!(
            juzga(
                "notepad.exe",
                r"C:\Windows\notepad.exe",
                &firmado,
                r"C:\Users\ana\Documents\nota.txt",
                1
            ),
            None
        );
    }

    #[test]
    fn otro_programa_que_abre_las_contrasenas_avisa() {
        let (a, grave) = juzga(
            "factura.exe",
            r"C:\Users\ana\Downloads\factura.exe",
            &Firma::SinFirma,
            CHROME,
            7,
        )
        .unwrap();
        assert!(grave);
        assert_eq!(a.regla, "ladron");
        assert_eq!(a.nombre, "contrasenas");
        assert!(
            a.texto.contains("las contraseñas guardadas de Chrome"),
            "{}",
            a.texto
        );
        assert!(a.texto.contains("No tiene firma"), "{}", a.texto);
        let (a, grave) = juzga(
            "Gestor.exe",
            r"C:\Program Files\Gestor\Gestor.exe",
            &Firma::Firmado("Gestor Inc.".into()),
            CHROME,
            7,
        )
        .unwrap();
        assert!(!grave);
        assert!(a.texto.contains("Lo firma Gestor Inc."), "{}", a.texto);
    }

    #[test]
    fn un_disfraz_del_navegador_sin_firma_avisa() {
        let (a, grave) = juzga(
            "chrome.exe",
            r"C:\Users\ana\AppData\Local\Temp\chrome.exe",
            &Firma::SinFirma,
            CHROME,
            1,
        )
        .unwrap();
        assert!(grave);
        assert!(a.texto.contains("no tiene su firma"), "{}", a.texto);
    }
}
