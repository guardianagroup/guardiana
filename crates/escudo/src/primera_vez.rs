//! «Primera vez» (phase A3, the first behaviour rule; docs/ESCUDO.md): what only GUARDIANA can see, because it is at the
//! same time the home's DNS and the computer's watcher.
//!
//! The program already knows, for each name a program on this computer asks for, which program asked (Windows' own DNS
//! client events, `crates/apps/src/ventanas.rs`), the SHA-256 of that program's file and who signs it. This module
//! remembers since when the home talks to each site and warns, in plain words, when all of these hold at once:
//!
//! - the site was never asked for in this home before (and the first days, while the memory fills up, never warn);
//! - the program that asks has been checked and **has no signature**;
//! - it runs from a folder any program can write to (the person's folders, temporary folders, `ProgramData`).
//!
//! That is the shape of a program that came in a download or an attachment and calls home. It is a question for the
//! person, never a verdict: the warning says what was seen and offers to cut the program's network or analyse its
//! file. And a program whose file is a known malicious sample warns at once, whatever else holds.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::huellas::{de_hex32, Huellas};

/// Days during which the memory only learns, so that the first days do not warn about everything.
pub const DIAS_APRENDIENDO: u64 = 7;
const DIA_MS: u64 = 24 * 60 * 60 * 1000;

/// What is known of a program's signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Firma {
    /// Windows checked it: this is who signs it.
    Firmado(String),
    /// Windows checked it: it has no valid signature.
    SinFirma,
    /// Not checked yet (the first query of a program goes without it).
    SinComprobar,
}

/// The program behind a query, as the program's DNS observer saw it.
#[derive(Debug, Clone)]
pub struct Programa<'a> {
    /// Its file name (`factura.exe`).
    pub nombre: &'a str,
    /// Its full path.
    pub ruta: &'a str,
    /// The SHA-256 of its file, in hexadecimal (empty when not read).
    pub sha256: &'a str,
    /// What is known of its signature.
    pub firma: Firma,
}

/// A warning for the person.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Aviso {
    /// Which rule: `muestra_conocida` or `primera_vez`.
    pub regla: &'static str,
    /// The program's file name.
    pub programa: String,
    /// Its full path.
    pub ruta: String,
    /// The site it asked for (the name a person would recognise: `ejemplo.com`).
    pub sitio: String,
    /// The exact name it asked for.
    pub nombre: String,
    /// When (milliseconds since 1970).
    pub cuando_ms: u64,
    /// The list that knows the program's file, for `muestra_conocida`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fuente: Option<String>,
    /// What was seen, in plain words.
    pub texto: String,
}

/// Since when the home talks to each site.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Memoria {
    /// When the memory started; until [`DIAS_APRENDIENDO`] days later it only learns.
    desde_ms: u64,
    /// Site → when it was first asked for.
    vistos: HashMap<String, u64>,
}

impl Memoria {
    /// An empty memory that starts learning now.
    #[must_use]
    pub fn nueva(ahora_ms: u64) -> Self {
        Self {
            desde_ms: ahora_ms,
            vistos: HashMap::new(),
        }
    }

    /// How many sites it knows.
    #[must_use]
    pub fn sitios(&self) -> usize {
        self.vistos.len()
    }

    /// Whether it is still in its first days, when it only learns.
    #[must_use]
    pub fn aprendiendo(&self, ahora_ms: u64) -> bool {
        ahora_ms < self.desde_ms.saturating_add(DIAS_APRENDIENDO * DIA_MS)
    }

    /// Looks at one query and remembers its site. Returns a warning when the rules in the module notes hold.
    pub fn mira(
        &mut self,
        nombre: &str,
        sitio: &str,
        cuando_ms: u64,
        programa: Option<&Programa<'_>>,
        listas: &[Huellas],
    ) -> Option<Aviso> {
        let sitio = sitio.trim_end_matches('.').to_ascii_lowercase();
        if sitio.is_empty() {
            return None;
        }
        let nuevo = !self.vistos.contains_key(&sitio);
        if nuevo {
            self.vistos.insert(sitio.clone(), cuando_ms);
        }
        let p = programa?;
        let aviso = |regla, fuente: Option<String>, texto: String| Aviso {
            regla,
            programa: p.nombre.to_string(),
            ruta: p.ruta.to_string(),
            sitio: sitio.clone(),
            nombre: nombre.trim_end_matches('.').to_string(),
            cuando_ms,
            fuente,
            texto,
        };
        // a known malicious sample warns at once, learning or not, new site or not
        if let Some(h) = de_hex32(p.sha256) {
            if let Some(l) = listas.iter().find(|l| l.contiene(&h)) {
                let texto = format!(
                    "«{}» es una muestra maliciosa conocida (según {}) y acaba de pedir «{}».",
                    p.nombre, l.fuente, sitio
                );
                return Some(aviso("muestra_conocida", Some(l.fuente.clone()), texto));
            }
        }
        if !nuevo || self.aprendiendo(cuando_ms) || p.firma != Firma::SinFirma {
            return None;
        }
        let lugar = carpeta_escribible(p.ruta)?;
        let texto = format!(
            "«{}», sin firma y desde {}, ha pedido «{}», un sitio que nunca se había pedido en tu casa.",
            p.nombre, lugar, sitio
        );
        Some(aviso("primera_vez", None, texto))
    }

    /// The memory as JSON, to keep it between runs.
    pub fn a_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    /// The memory from JSON.
    pub fn desde_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }
}

/// When a Windows path is in a folder any program can write to, how to say where, in plain words.
#[must_use]
pub fn carpeta_escribible(ruta: &str) -> Option<&'static str> {
    let r = ruta.replace('/', "\\").to_lowercase();
    let r = r.trim_start_matches("\\\\?\\");
    let dentro = |trozo: &str| r.contains(trozo);
    if dentro("\\downloads\\") || dentro("\\descargas\\") {
        Some("la carpeta de Descargas")
    } else if dentro("\\appdata\\local\\temp\\")
        || dentro("\\windows\\temp\\")
        || dentro("\\temp\\")
    {
        Some("una carpeta temporal")
    } else if dentro("\\desktop\\") || dentro("\\escritorio\\") {
        Some("el Escritorio")
    } else if dentro("\\appdata\\") {
        Some("la carpeta de datos de tu usuario (AppData)")
    } else if r.contains(":\\programdata\\") {
        Some("ProgramData")
    } else if r.contains(":\\users\\") {
        Some("tu carpeta de usuario")
    } else {
        None
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const DIA: u64 = DIA_MS;

    fn programa<'a>(ruta: &'a str, firma: Firma, sha256: &'a str) -> Programa<'a> {
        Programa {
            nombre: ruta.rsplit(['\\', '/']).next().unwrap_or(ruta),
            ruta,
            sha256,
            firma,
        }
    }

    #[test]
    fn un_programa_descargado_sin_firma_que_llama_a_un_sitio_nuevo() {
        let mut m = Memoria::nueva(0);
        let p = programa(r"C:\Users\ana\Downloads\factura.exe", Firma::SinFirma, "");
        // mientras aprende, nada
        assert_eq!(
            m.mira("c2.ejemplo.top", "ejemplo.top", DIA, Some(&p), &[]),
            None
        );
        let a = m
            .mira(
                "x.otro-sitio.xyz.",
                "otro-sitio.xyz",
                8 * DIA,
                Some(&p),
                &[],
            )
            .unwrap();
        assert_eq!(a.regla, "primera_vez");
        assert_eq!(a.programa, "factura.exe");
        assert_eq!(a.nombre, "x.otro-sitio.xyz");
        assert!(a.texto.contains("la carpeta de Descargas"), "{}", a.texto);
        // la segunda vez ya no es la primera
        assert_eq!(
            m.mira("y.otro-sitio.xyz", "otro-sitio.xyz", 9 * DIA, Some(&p), &[]),
            None
        );
    }

    #[test]
    fn firmado_sin_comprobar_o_instalado_no_avisa() {
        let mut m = Memoria::nueva(0);
        let firmado = programa(
            r"C:\Users\ana\Downloads\setup.exe",
            Firma::Firmado("Empresa S.A.".into()),
            "",
        );
        let sin_mirar = programa(r"C:\Users\ana\Downloads\otro.exe", Firma::SinComprobar, "");
        let instalado = programa(r"C:\Program Files\App\app.exe", Firma::SinFirma, "");
        assert_eq!(
            m.mira("a.uno.com", "uno.com", 10 * DIA, Some(&firmado), &[]),
            None
        );
        assert_eq!(
            m.mira("a.dos.com", "dos.com", 10 * DIA, Some(&sin_mirar), &[]),
            None
        );
        assert_eq!(
            m.mira("a.tres.com", "tres.com", 10 * DIA, Some(&instalado), &[]),
            None
        );
        assert_eq!(
            m.mira("a.cuatro.com", "cuatro.com", 10 * DIA, None, &[]),
            None
        );
        assert_eq!(m.sitios(), 4);
    }

    #[test]
    fn una_muestra_conocida_avisa_siempre() {
        let h = "aa".repeat(32);
        let lista = Huellas::desde_texto("lista de prueba", &h);
        let mut m = Memoria::nueva(0);
        m.mira("www.visto.com", "visto.com", 1, None, &[]);
        let p = programa(
            r"C:\Program Files\Firmado\x.exe",
            Firma::Firmado("Alguien".into()),
            &h,
        );
        let a = m
            .mira(
                "www.visto.com",
                "visto.com",
                2,
                Some(&p),
                std::slice::from_ref(&lista),
            )
            .unwrap();
        assert_eq!(a.regla, "muestra_conocida");
        assert_eq!(a.fuente.as_deref(), Some("lista de prueba"));
    }

    #[test]
    fn la_memoria_se_guarda_y_se_recupera() {
        let mut m = Memoria::nueva(5);
        m.mira("a.uno.com", "uno.com", 6, None, &[]);
        let json = m.a_json().unwrap();
        let mut r = Memoria::desde_json(&json).unwrap();
        assert_eq!(r.sitios(), 1);
        assert!(r.aprendiendo(6));
        let p = programa(
            r"C:\Users\ana\AppData\Local\Temp\a.exe",
            Firma::SinFirma,
            "",
        );
        // ya visto antes de guardarse: no es la primera vez
        assert_eq!(
            r.mira("b.uno.com", "UNO.com", 20 * DIA, Some(&p), &[]),
            None
        );
    }

    #[test]
    fn carpetas_donde_cualquiera_escribe() {
        assert_eq!(
            carpeta_escribible(r"C:\Users\ana\Downloads\a.exe"),
            Some("la carpeta de Descargas")
        );
        assert_eq!(
            carpeta_escribible(r"\\?\C:\Users\ana\AppData\Local\Temp\x\a.exe"),
            Some("una carpeta temporal")
        );
        assert_eq!(
            carpeta_escribible(r"C:\Users\ana\AppData\Roaming\x\a.exe"),
            Some("la carpeta de datos de tu usuario (AppData)")
        );
        assert_eq!(
            carpeta_escribible(r"C:\ProgramData\x\a.exe"),
            Some("ProgramData")
        );
        assert_eq!(carpeta_escribible(r"C:\Windows\System32\svchost.exe"), None);
        assert_eq!(carpeta_escribible(r"C:\Program Files\App\app.exe"), None);
    }
}
