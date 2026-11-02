//! Windows' notices of el escudo's grave warnings (1.0.52; the owner, 1 Nov 2026, 19:29, to the proposal of a Windows
//! notice for each grave warning of the antivirus: «Me parece bien la propuesta, sí, buena idea»).
//!
//! ZERO started with `--avisos` when the person signs in to Windows, with no window, reads the warnings the service
//! writes for the panel ([`ARCHIVO`], in the system's data folder, which the person at the console can read) and shows a
//! notice for each new grave one. A click on it opens Órbita on the Antivirus ([`ENLACE`]; the PC Scanner until the
//! owner renamed it, 1 Nov 2026, 21:03: «cámbialo sí»). This module decides what to show and writes the notice; the
//! Windows program shows it. Nothing leaves the computer.

use std::path::PathBuf;
use std::time::Duration;

use serde_json::Value;

/// The name Windows shows the notices under (registered by the program for the person's account).
pub const AUMID: &str = "GuardianaGroup.Guardiana";
/// The file the service keeps the escudo's newest warnings in, one per line.
pub const ARCHIVO: &str = "escudo-avisos.jsonl";
/// What a click on a notice opens.
pub const ENLACE: &str = "guardiana://escaner";
/// How often the file is read.
pub const CADA: Duration = Duration::from_secs(4);
/// On the very first start for an account, the grave warnings of the last minutes are still shown.
pub const ATRAS_MS: u64 = 10 * 60_000;
/// At most this many notices at once; more become one notice that counts them.
pub const MAXIMO: usize = 3;
/// The longest text a notice carries (Windows cuts it anyway).
const LARGO: usize = 300;

/// Where the service writes the warnings.
#[must_use]
pub fn ruta() -> PathBuf {
    guardiana_core::paths::system_data_dir().join(ARCHIVO)
}

/// One grave warning to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grave {
    /// The program it is about (its file name).
    pub programa: String,
    /// What was seen, in plain words, as the service wrote it.
    pub texto: String,
    /// When (milliseconds since 1970).
    pub cuando_ms: u64,
}

/// The grave warnings in the file's text written after `visto` (milliseconds since 1970), oldest first, and the new
/// mark: the time of the newest warning read, grave or not. A warning goes by when it was written (`anotado_ms`), not
/// by when the thing happened (`cuando_ms`, the only one before 1.0.52's last builds): one can be written a minute
/// after the fact, once the program's signature is checked, and must not be taken for already seen. A line that is not
/// a warning is skipped.
#[must_use]
pub fn nuevos(texto: &str, visto: u64) -> (Vec<Grave>, u64) {
    let mut marca = visto;
    let mut out = Vec::new();
    for linea in texto.lines() {
        let Ok(v) = serde_json::from_str::<Value>(linea) else {
            continue;
        };
        let cuando = v["anotado_ms"]
            .as_u64()
            .or_else(|| v["cuando_ms"].as_u64())
            .unwrap_or(0);
        if cuando <= visto {
            continue;
        }
        marca = marca.max(cuando);
        let Some(t) = v["texto"].as_str() else {
            continue;
        };
        if v["grave"] != Value::Bool(true) {
            continue;
        }
        out.push(Grave {
            programa: v["programa"].as_str().unwrap_or_default().to_owned(),
            texto: t.to_owned(),
            cuando_ms: cuando,
        });
    }
    out.sort_by_key(|g| g.cuando_ms);
    (out, marca)
}

/// The notices to show for `graves`: one each, up to [`MAXIMO`]; more become a single notice that counts them.
#[must_use]
pub fn notificaciones(graves: &[Grave]) -> Vec<String> {
    if graves.len() > MAXIMO {
        let cuerpo = format!(
            "El escudo de GUARDIANA tiene {} avisos graves nuevos. Míralos en el Antivirus.",
            graves.len()
        );
        return vec![xml("GUARDIANA · Avisos graves", "", &cuerpo)];
    }
    graves
        .iter()
        .map(|g| xml("GUARDIANA · Aviso grave", &g.programa, &g.texto))
        .collect()
}

/// `s` with the five XML special characters written as entities and the control characters left out.
fn escapa(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// A notice in Windows' toast schema: its title, the program it is about (when there is one), the text, and a button.
/// A click on the notice or on the button opens [`ENLACE`].
#[must_use]
pub fn xml(titulo: &str, programa: &str, texto: &str) -> String {
    let texto: String = texto.chars().take(LARGO).collect();
    let programa: String = programa.chars().take(80).collect();
    let linea = if programa.is_empty() {
        String::new()
    } else {
        format!("<text>{}</text>", escapa(&programa))
    };
    format!(
        "<toast launch=\"{ENLACE}\" activationType=\"protocol\" duration=\"long\">\
         <visual><binding template=\"ToastGeneric\"><text>{}</text>{linea}<text>{}</text></binding></visual>\
         <actions><action content=\"Ver en el Antivirus\" activationType=\"protocol\" arguments=\"{ENLACE}\"/></actions>\
         </toast>",
        escapa(titulo),
        escapa(&texto)
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn solo_los_graves_nuevos() {
        let texto = [
            r#"{"regla":"primera_vez","programa":"a.exe","texto":"uno","cuando_ms":100}"#,
            r#"{"regla":"documento","programa":"WINWORD.EXE","texto":"dos","cuando_ms":200,"grave":true}"#,
            "no es json",
            r#"{"regla":"secuestro","programa":"x.exe","texto":"tres","cuando_ms":400,"grave":true}"#,
            r#"{"regla":"guion","programa":"wscript.exe","texto":"cuatro","cuando_ms":300}"#,
        ]
        .join("\n");
        // escrito después que otro más reciente: cuenta cuando se escribió
        let tarde = format!(
            "{texto}\n{}",
            r#"{"regla":"arranque","programa":"u.exe","texto":"cinco","cuando_ms":150,"anotado_ms":500,"grave":true}"#
        );
        let (g, marca) = nuevos(&tarde, 400);
        assert_eq!((g.len(), marca), (1, 500));
        assert_eq!(g[0].texto, "cinco");
        let (g, marca) = nuevos(&texto, 0);
        assert_eq!(marca, 400);
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].programa, "WINWORD.EXE");
        assert_eq!(g[1].texto, "tres");
        // lo ya visto no vuelve
        let (g, marca) = nuevos(&texto, 200);
        assert_eq!((g.len(), marca), (1, 400));
        let (g, marca) = nuevos(&texto, 400);
        assert!(g.is_empty());
        assert_eq!(marca, 400);
    }

    #[test]
    fn muchos_avisos_son_uno_que_los_cuenta() {
        let g = |n: u64| Grave {
            programa: format!("p{n}.exe"),
            texto: "t".into(),
            cuando_ms: n,
        };
        assert_eq!(notificaciones(&[g(1), g(2)]).len(), 2);
        let todas = notificaciones(&[g(1), g(2), g(3), g(4)]);
        assert_eq!(todas.len(), 1);
        assert!(todas[0].contains("4 avisos graves"));
    }

    #[test]
    fn el_aviso_es_xml_bien_escrito() {
        let x = xml(
            "GUARDIANA · Aviso grave",
            "a&b.exe",
            "«x» <script> \"y\"\u{7}",
        );
        assert!(x.starts_with("<toast launch=\"guardiana://escaner\" activationType=\"protocol\""));
        assert!(x.contains("<text>a&amp;b.exe</text>"));
        assert!(x.contains("«x» &lt;script&gt; &quot;y&quot; "));
        assert!(!x.contains('\u{7}'));
        // sin programa, dos líneas
        assert_eq!(xml("t", "", "c").matches("<text>").count(), 2);
    }
}
