//! The browser extensions installed on this computer (1.0.46, «Quién vende tus datos»): Chrome,
//! Edge, Brave and Firefox, for every account, read from each extension's own manifest on disk.
//! Nothing is opened in the browser, nothing is sent, nothing is changed: an extension is
//! removed by the person, in the browser, and the page says how.
//!
//! What is kept of each one: the browser, its id, its name and version, the web addresses it may
//! read (its patterns) and its other permissions. Never the profile's history or any page.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

/// One extension of one browser profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Extension {
    /// `Chrome`, `Edge`, `Brave` or `Firefox`.
    pub(crate) navegador: &'static str,
    /// Its id in its store.
    pub(crate) id: String,
    /// Its name (from its own translations when the manifest points at them).
    pub(crate) nombre: String,
    /// Its version.
    pub(crate) version: String,
    /// The web addresses it may read or change (`<all_urls>`, `https://*/*`…).
    pub(crate) patrones: Vec<String>,
    /// Its other permissions (`tabs`, `history`, `cookies`…).
    pub(crate) permisos: Vec<String>,
}

/// Where each Chromium browser keeps its profiles, under an account's home.
fn chromium(home: &Path) -> Vec<(&'static str, PathBuf)> {
    if cfg!(windows) {
        let l = home.join("AppData").join("Local");
        vec![
            ("Chrome", l.join(r"Google\Chrome\User Data")),
            ("Edge", l.join(r"Microsoft\Edge\User Data")),
            ("Brave", l.join(r"BraveSoftware\Brave-Browser\User Data")),
        ]
    } else if cfg!(target_os = "macos") {
        let a = home.join("Library").join("Application Support");
        vec![
            ("Chrome", a.join("Google/Chrome")),
            ("Edge", a.join("Microsoft Edge")),
            ("Brave", a.join("BraveSoftware/Brave-Browser")),
        ]
    } else {
        let c = home.join(".config");
        vec![
            ("Chrome", c.join("google-chrome")),
            ("Edge", c.join("microsoft-edge")),
            ("Brave", c.join("BraveSoftware/Brave-Browser")),
        ]
    }
}

fn firefox(home: &Path) -> PathBuf {
    if cfg!(windows) {
        home.join(r"AppData\Roaming\Mozilla\Firefox\Profiles")
    } else if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Firefox/Profiles")
    } else {
        home.join(".mozilla/firefox")
    }
}

/// The homes of the people's accounts of this computer.
pub(crate) fn hogares() -> Vec<PathBuf> {
    let base = if cfg!(windows) {
        let unidad = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_owned());
        PathBuf::from(format!("{unidad}\\Users"))
    } else if cfg!(target_os = "macos") {
        PathBuf::from("/Users")
    } else {
        PathBuf::from("/home")
    };
    let mut out: Vec<PathBuf> = std::fs::read_dir(&base)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().is_dir())
                .filter(|e| {
                    let n = e.file_name().to_string_lossy().to_ascii_lowercase();
                    !n.starts_with('.')
                        && !matches!(
                            n.as_str(),
                            "public"
                                | "default"
                                | "default user"
                                | "all users"
                                | "shared"
                                | "guest"
                        )
                })
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// Every extension of every browser of every account.
pub(crate) fn instaladas() -> Vec<Extension> {
    let mut out = Vec::new();
    for h in hogares() {
        out.extend(en_hogar(&h));
    }
    out
}

/// The extensions under one account's home.
pub(crate) fn en_hogar(home: &Path) -> Vec<Extension> {
    let mut out = Vec::new();
    for (navegador, raiz) in chromium(home) {
        let Ok(perfiles) = std::fs::read_dir(&raiz) else {
            continue;
        };
        for p in perfiles.flatten() {
            let ext = p.path().join("Extensions");
            if ext.is_dir() {
                out.extend(de_chromium(navegador, &ext));
            }
        }
    }
    if let Ok(perfiles) = std::fs::read_dir(firefox(home)) {
        for p in perfiles.flatten() {
            if let Ok(texto) = std::fs::read_to_string(p.path().join("extensions.json")) {
                out.extend(de_firefox(&texto));
            }
        }
    }
    out
}

fn textos(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn es_patron(p: &str) -> bool {
    p == "<all_urls>" || p.contains("://")
}

/// A `__MSG_name__` resolved from the extension's default translation.
fn nombre_traducido(dir: &Path, m: &Value, nombre: &str) -> String {
    let Some(clave) = nombre
        .strip_prefix("__MSG_")
        .and_then(|r| r.strip_suffix("__"))
    else {
        return nombre.to_owned();
    };
    let idioma = m
        .get("default_locale")
        .and_then(Value::as_str)
        .unwrap_or("en");
    let Ok(texto) =
        std::fs::read_to_string(dir.join("_locales").join(idioma).join("messages.json"))
    else {
        return clave.to_owned();
    };
    let Ok(Value::Object(mensajes)) =
        serde_json::from_str::<Value>(texto.trim_start_matches('\u{feff}'))
    else {
        return clave.to_owned();
    };
    mensajes
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(clave))
        .and_then(|(_, v)| v.get("message"))
        .and_then(Value::as_str)
        .map_or_else(|| clave.to_owned(), str::to_owned)
}

/// One manifest (`<id>/<version>/manifest.json`) as an extension.
pub(crate) fn de_manifiesto(
    navegador: &'static str,
    id: &str,
    dir: &Path,
    texto: &str,
) -> Option<Extension> {
    let m: Value = serde_json::from_str(texto.trim_start_matches('\u{feff}')).ok()?;
    let nombre = nombre_traducido(dir, &m, m.get("name").and_then(Value::as_str).unwrap_or(id));
    let mut patrones: Vec<String> = Vec::new();
    let mut permisos: Vec<String> = Vec::new();
    for p in textos(m.get("permissions"))
        .into_iter()
        .chain(textos(m.get("host_permissions")))
    {
        if es_patron(&p) {
            patrones.push(p);
        } else {
            permisos.push(p);
        }
    }
    if let Some(cs) = m.get("content_scripts").and_then(Value::as_array) {
        for c in cs {
            patrones.extend(textos(c.get("matches")));
        }
    }
    patrones.sort();
    patrones.dedup();
    permisos.sort();
    permisos.dedup();
    Some(Extension {
        navegador,
        id: id.to_owned(),
        nombre,
        version: m
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        patrones,
        permisos,
    })
}

fn de_chromium(navegador: &'static str, extensiones: &Path) -> Vec<Extension> {
    let mut out = Vec::new();
    let Ok(ids) = std::fs::read_dir(extensiones) else {
        return out;
    };
    for id in ids.flatten() {
        let nombre_id = id.file_name().to_string_lossy().into_owned();
        // Chromium ids are 32 letters a–p; anything else in the folder is not an extension.
        if nombre_id.len() != 32 || !nombre_id.chars().all(|c| ('a'..='p').contains(&c)) {
            continue;
        }
        let Ok(versiones) = std::fs::read_dir(id.path()) else {
            continue;
        };
        // The newest version folder is the one in use.
        let mut vs: Vec<PathBuf> = versiones
            .flatten()
            .map(|v| v.path())
            .filter(|p| p.join("manifest.json").is_file())
            .collect();
        vs.sort();
        let Some(dir) = vs.pop() else {
            continue;
        };
        if let Ok(texto) = std::fs::read_to_string(dir.join("manifest.json")) {
            if let Some(e) = de_manifiesto(navegador, &nombre_id, &dir, &texto) {
                out.push(e);
            }
        }
    }
    out
}

/// A Firefox profile's `extensions.json`: the extensions the person installed (not the ones
/// that come with Firefox).
pub(crate) fn de_firefox(texto: &str) -> Vec<Extension> {
    let Ok(v) = serde_json::from_str::<Value>(texto) else {
        return Vec::new();
    };
    let Some(addons) = v.get("addons").and_then(Value::as_array) else {
        return Vec::new();
    };
    addons
        .iter()
        .filter(|a| a.get("type").and_then(Value::as_str) == Some("extension"))
        .filter(|a| {
            let l = a.get("location").and_then(Value::as_str).unwrap_or("");
            !l.starts_with("app-system") && !l.starts_with("app-builtin")
        })
        .map(|a| {
            let up = a.get("userPermissions");
            let mut patrones = textos(up.and_then(|u| u.get("origins")));
            patrones.sort();
            patrones.dedup();
            Extension {
                navegador: "Firefox",
                id: a.get("id").and_then(Value::as_str).unwrap_or("").to_owned(),
                nombre: a
                    .get("defaultLocale")
                    .and_then(|d| d.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
                version: a
                    .get("version")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
                patrones,
                permisos: textos(up.and_then(|u| u.get("permissions"))),
            }
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_chromium_profile_is_read_from_its_manifests() {
        let home = std::env::temp_dir().join(format!("qv-ext-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let (_, raiz) = chromium(&home).into_iter().next().unwrap();
        let id = "eppiocemhmnlbhjplcgkofciiegomcon";
        let dir = raiz
            .join("Default")
            .join("Extensions")
            .join(id)
            .join("5.0.1_0");
        std::fs::create_dir_all(dir.join("_locales").join("en")).unwrap();
        std::fs::write(
            dir.join("manifest.json"),
            r#"{"name":"__MSG_appName__","default_locale":"en","version":"5.0.1","manifest_version":3,
               "permissions":["tabs","storage"],"host_permissions":["<all_urls>"],
               "content_scripts":[{"matches":["https://chatgpt.com/*"]}]}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("_locales").join("en").join("messages.json"),
            r#"{"appName":{"message":"Urban VPN Proxy"}}"#,
        )
        .unwrap();
        let l = en_hogar(&home);
        assert_eq!(l.len(), 1);
        let e = &l[0];
        assert_eq!(e.navegador, "Chrome");
        assert_eq!(e.nombre, "Urban VPN Proxy");
        assert_eq!(e.version, "5.0.1");
        assert_eq!(e.patrones, vec!["<all_urls>", "https://chatgpt.com/*"]);
        assert_eq!(e.permisos, vec!["storage", "tabs"]);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn firefox_lists_only_what_the_person_installed() {
        let t = r#"{"addons":[
          {"id":"ublock@raymondhill.net","type":"extension","location":"app-profile","version":"1.60",
           "defaultLocale":{"name":"uBlock Origin"},"userPermissions":{"permissions":["storage"],"origins":["<all_urls>"]}},
          {"id":"formautofill@mozilla.org","type":"extension","location":"app-builtin-addons","version":"1",
           "defaultLocale":{"name":"Form Autofill"}},
          {"id":"tema@x","type":"theme","location":"app-profile","version":"1","defaultLocale":{"name":"Tema"}}]}"#;
        let l = de_firefox(t);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].nombre, "uBlock Origin");
        assert_eq!(l[0].patrones, vec!["<all_urls>"]);
        assert!(de_firefox("no es json").is_empty());
    }
}
