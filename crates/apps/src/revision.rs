//! «Revisión del ordenador» (1.0.24): what Windows itself says about this computer's protection,
//! access, what starts on its own, where its internet goes out and its privacy. One read-only
//! PowerShell (`revision.ps1`), fed through standard input so the command line stays short and
//! nothing is written to disk. It changes nothing and installs nothing.

use super::Error;

/// The script, as it travels inside the binary.
pub(crate) const SCRIPT: &str = include_str!("revision.ps1");

pub(crate) fn revisar(
    limite: std::time::Duration,
    avisa: super::AvisoRevision,
) -> Result<serde_json::Value, Error> {
    use std::io::{BufRead as _, Read as _, Write as _};
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
    let raiz = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_owned());
    // A 32-bit GUARDIANA on a 64-bit Windows would get the 32-bit PowerShell, which has no
    // local-accounts module (ChatGPT's review): «Sysnative» is the real System32 from there.
    let carpeta = if std::env::var_os("PROCESSOR_ARCHITEW6432").is_some() {
        "Sysnative"
    } else {
        "System32"
    };
    let exe = format!("{raiz}\\{carpeta}\\WindowsPowerShell\\v1.0\\powershell.exe");
    let mut hijo = Command::new(exe)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "& ([scriptblock]::Create([Console]::In.ReadToEnd()))",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS)
        .spawn()
        .map_err(|e| Error::Sistema(e.to_string()))?;
    if let Some(mut entrada) = hijo.stdin.take() {
        entrada
            .write_all(SCRIPT.as_bytes())
            .map_err(|e| Error::Sistema(e.to_string()))?;
    }
    let mut salida = hijo.stdout.take();
    let lector = std::thread::spawn(move || {
        let mut v = Vec::new();
        if let Some(s) = salida.as_mut() {
            let _ = s.read_to_end(&mut v);
        }
        v
    });
    // What the script says it is doing (`@@kind<TAB>text` on standard error), handed on as it
    // comes so the page can show it live. Read always, or a full pipe would stop PowerShell.
    let errores = hijo.stderr.take();
    let pasos = std::thread::spawn(move || {
        if let Some(e) = errores {
            for linea in std::io::BufReader::new(e).lines().map_while(Result::ok) {
                if let Some(resto) = linea.strip_prefix("@@") {
                    let (tipo, texto) = resto.split_once('\t').unwrap_or((resto, ""));
                    avisa(tipo, texto);
                }
            }
        }
    });
    let inicio = std::time::Instant::now();
    loop {
        match hijo.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if inicio.elapsed() > limite => {
                let _ = hijo.kill();
                let _ = hijo.wait();
                return Err(Error::Sistema(format!(
                    "la revisión tardó más de {} s y se paró",
                    limite.as_secs()
                )));
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(200)),
            Err(e) => return Err(Error::Sistema(e.to_string())),
        }
    }
    let bytes = lector.join().unwrap_or_default();
    let _ = pasos.join();
    let texto = String::from_utf8_lossy(&bytes);
    let texto = texto.trim().trim_start_matches('\u{feff}');
    serde_json::from_str(texto).map_err(|e| Error::Sistema(format!("respuesta ilegible: {e}")))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// The real thing, on the CI's Windows: it finishes, it is JSON, and the basic sections
    /// carry data instead of an error.
    #[test]
    fn the_review_runs_on_windows_and_reads_the_basics() {
        let inicio = std::time::Instant::now();
        let vistos = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let v2 = std::sync::Arc::clone(&vistos);
        let r = revisar(
            std::time::Duration::from_secs(400),
            Box::new(move |tipo, _| {
                if let Ok(mut v) = v2.lock() {
                    v.push(tipo.to_owned());
                }
            }),
        )
        .unwrap();
        let tipos = vistos.lock().unwrap().clone();
        assert!(tipos.iter().any(|t| t == "seccion"), "{tipos:?}");
        assert!(tipos.iter().any(|t| t == "hecho"), "{tipos:?}");
        let secs = inicio.elapsed().as_secs();
        assert!(
            r["equipo"]["sistema"]
                .as_str()
                .is_some_and(|s| !s.is_empty()),
            "equipo: {} · tiempos {}",
            r["equipo"],
            r["tiempos"]
        );
        for s in [
            "equipo",
            "sistema",
            "acceso",
            "arranque",
            "salida",
            "certificados",
        ] {
            assert!(
                r[s]["error"].is_null(),
                "{s}: {} · tiempos {}",
                r[s]["error"],
                r["tiempos"]
            );
        }
        assert!(
            r["certificados"]["total"].as_u64().unwrap_or(0) > 0,
            "{}",
            r["certificados"]
        );
        assert!(secs < 300, "tardó {secs} s · tiempos {}", r["tiempos"]);
    }
}
