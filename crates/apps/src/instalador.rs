//! «Antes de instalar» (customer request, 18 Oct 2026): who signed an installer, as Windows
//! itself checks it, before the person runs it.
//!
//! `Get-AuthenticodeSignature` asks WinVerifyTrust, the same check Explorer's «Firmas digitales»
//! tab and SmartScreen start from: whether the file carries an Authenticode signature, whether
//! the file is still what was signed, whether the chain reaches a root this Windows trusts, who
//! the publisher is and whether a timestamp authority countersigned it. Nothing is uploaded: the
//! file is read on this computer and nowhere else. The same PowerShell route as `firmas.rs`
//! (1.0.21), because this project allows no `unsafe` for a direct call.
//!
//! What it does **not** say, and the panel says so too: that a signed program is safe. A
//! signature says who published it and that it was not changed since; publishers with valid
//! certificates have shipped harmful software.

use serde::Serialize;

/// What Windows said about the signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Estado {
    /// Signed, the file is what was signed, and the chain reaches a trusted root.
    Verificada,
    /// No signature at all.
    SinFirma,
    /// A signature that does not hold: the file changed after signing, the certificate is not
    /// trusted or was revoked, or the chain is broken.
    NoValida,
    /// A kind of file Authenticode does not cover.
    NoAdmitido,
    /// Not on this system (only Windows checks Authenticode), or Windows could not be asked.
    NoDisponible,
}

/// The answer, as the panel shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Firma {
    /// The verdict.
    pub estado: Estado,
    /// Windows' own status word (`Valid`, `NotSigned`, `HashMismatch`, `NotTrusted`…).
    pub windows: String,
    /// The publisher: the signer certificate's common name. Only given when the signature is
    /// valid: the name on a broken one proves nothing and is not repeated.
    pub editor: String,
    /// Who issued the publisher's certificate.
    pub emisor: String,
    /// Until when the publisher's certificate was valid (`yyyy-MM-dd`).
    pub valido_hasta: String,
    /// Who countersigned the time of signing, when someone did: with it, the signature stays
    /// valid after the certificate expires.
    pub sello_tiempo: String,
    /// `Authenticode` (inside the file) or `Catalog` (in Windows' catalogue).
    pub tipo: String,
    /// What Windows said in words, in its own language.
    pub mensaje: String,
}

impl Firma {
    fn no_disponible(mensaje: &str) -> Self {
        Self {
            estado: Estado::NoDisponible,
            windows: String::new(),
            editor: String::new(),
            emisor: String::new(),
            valido_hasta: String::new(),
            sello_tiempo: String::new(),
            tipo: String::new(),
            mensaje: mensaje.to_owned(),
        }
    }
}

fn corto(s: &str) -> String {
    s.trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(200)
        .collect()
}

/// Read what the PowerShell script printed (one JSON object).
#[must_use]
pub fn interpreta(salida: &str) -> Firma {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(salida.trim()) else {
        return Firma::no_disponible("Windows no respondió a la comprobación");
    };
    let s = |k: &str| corto(v.get(k).and_then(serde_json::Value::as_str).unwrap_or(""));
    let windows = s("estado");
    let estado = match windows.as_str() {
        "Valid" => Estado::Verificada,
        "NotSigned" => Estado::SinFirma,
        "NotSupportedFileFormat" => Estado::NoAdmitido,
        "" => Estado::NoDisponible,
        _ => Estado::NoValida,
    };
    let valida = estado == Estado::Verificada;
    Firma {
        estado,
        windows,
        editor: if valida { s("editor") } else { String::new() },
        emisor: if valida { s("emisor") } else { String::new() },
        valido_hasta: if valida { s("hasta") } else { String::new() },
        sello_tiempo: if valida { s("sello") } else { String::new() },
        tipo: s("tipo"),
        mensaje: s("mensaje"),
    }
}

/// The environment variable the file's path travels in, from this program to the PowerShell.
pub const VARIABLE: &str = "GUARDIANA_INSTALADOR";

/// The script, the same for every file: the path is read from [`VARIABLE`], as data, and never
/// written into the command (1.0.55). Until then it went between single quotes with its own quotes
/// doubled, and PowerShell also takes Windows' typographic quotes (‘ ’, and “ ” in a double-quoted
/// text) for quotes, so those characters in a path could close the literal: the panel's copy has a
/// name of the service's own (random digits and a known extension), but a path with them must
/// never be able to turn into a command in a PowerShell the service runs.
pub const GUION: &str = "[Console]::OutputEncoding = [Text.Encoding]::UTF8; \
     $s = Get-AuthenticodeSignature -LiteralPath $env:GUARDIANA_INSTALADOR; \
     $c = $s.SignerCertificate; $t = $s.TimeStamperCertificate; \
     [pscustomobject]@{ estado = [string]$s.Status; mensaje = [string]$s.StatusMessage; \
     tipo = [string]$s.SignatureType; \
     editor = $(if ($c) { $c.GetNameInfo(0, $false) } else { '' }); \
     emisor = $(if ($c) { $c.GetNameInfo(0, $true) } else { '' }); \
     hasta = $(if ($c) { $c.NotAfter.ToString('yyyy-MM-dd') } else { '' }); \
     sello = $(if ($t) { $t.GetNameInfo(0, $false) } else { '' }) } | ConvertTo-Json -Compress";

/// Check the file at `ruta`. Blocks for the second or two PowerShell takes: call it off the
/// async runtime.
#[must_use]
pub fn comprobar(ruta: &std::path::Path) -> Firma {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console flashing when this runs from the service.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        match std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", GUION])
            .env(VARIABLE, ruta)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
        {
            Ok(salida) => interpreta(&String::from_utf8_lossy(&salida.stdout)),
            Err(_) => Firma::no_disponible("no se pudo preguntar a Windows"),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = ruta;
        Firma::no_disponible("solo Windows comprueba firmas Authenticode")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_valid_signature_names_its_publisher_issuer_and_timestamp() {
        let f = interpreta(
            r#"{"estado":"Valid","mensaje":"Signature verified.","tipo":"Authenticode","editor":"Mozilla Corporation","emisor":"DigiCert Trusted G4 Code Signing RSA4096 SHA384 2021 CA1","hasta":"2027-06-30","sello":"DigiCert Timestamp 2024"}"#,
        );
        assert_eq!(f.estado, Estado::Verificada);
        assert_eq!(f.editor, "Mozilla Corporation");
        assert!(f.emisor.starts_with("DigiCert"));
        assert_eq!(f.valido_hasta, "2027-06-30");
        assert_eq!(f.sello_tiempo, "DigiCert Timestamp 2024");
    }

    #[test]
    fn a_broken_signature_does_not_repeat_the_name_it_claims() {
        for w in ["HashMismatch", "NotTrusted", "UnknownError", "Incompatible"] {
            let f = interpreta(&format!(
                r#"{{"estado":"{w}","mensaje":"x","tipo":"Authenticode","editor":"Microsoft Corporation","emisor":"Fake CA","hasta":"2030-01-01","sello":""}}"#
            ));
            assert_eq!(f.estado, Estado::NoValida, "{w}");
            assert_eq!(f.windows, w);
            assert!(f.editor.is_empty() && f.emisor.is_empty(), "{w}");
        }
    }

    #[test]
    fn unsigned_unsupported_and_silence_are_told_apart() {
        assert_eq!(
            interpreta(r#"{"estado":"NotSigned","editor":"","tipo":"None"}"#).estado,
            Estado::SinFirma
        );
        assert_eq!(
            interpreta(r#"{"estado":"NotSupportedFileFormat"}"#).estado,
            Estado::NoAdmitido
        );
        assert_eq!(interpreta("").estado, Estado::NoDisponible);
        assert_eq!(
            interpreta("Get-Authenticode… error").estado,
            Estado::NoDisponible
        );
    }

    /// The path is no part of the command: the script reads it from its variable, and nothing in
    /// it is written from outside (1.0.55).
    #[test]
    fn the_path_travels_as_data_never_in_the_command() {
        assert!(GUION.contains(&format!("-LiteralPath $env:{VARIABLE};")));
        assert!(!GUION.contains("-LiteralPath '"));
        assert!(GUION.is_ascii());
    }

    /// On a real Windows: a file whose name carries every kind of quote PowerShell knows (‘ ’ “ ”
    /// and the plain one), its escape (`) and a subexpression is found and checked, and nothing in
    /// its name runs; a file that is not there says nothing.
    #[cfg(windows)]
    #[test]
    fn on_windows_no_quote_in_a_file_name_becomes_a_command() {
        let carpeta =
            std::env::temp_dir().join(format!("guardiana-instalador-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&carpeta);
        // Relative: a command that ran would make it in the PowerShell's working folder, this
        // test's.
        let marca = format!("ligero-instalador-{}", std::process::id());
        let orden = format!("New-Item -ItemType File -Path {marca}");
        let nombres = [
            format!("a\u{2018}; {orden}; \u{2019}b.txt"),
            format!("c\u{201C}$({orden})\u{201D}d.txt"),
            format!("e'; {orden}; 'f`$({orden})`g.txt"),
        ];
        for nombre in &nombres {
            let ruta = carpeta.join(nombre);
            assert!(std::fs::write(&ruta, b"x").is_ok(), "{nombre}");
            let f = comprobar(&ruta);
            println!("{nombre}: {f:?}");
            // Windows answered about this very file (a missing one gives no status at all).
            assert_ne!(f.estado, Estado::NoDisponible, "{nombre}: {f:?}");
            let falta = comprobar(&carpeta.join(format!("no-{nombre}")));
            assert_eq!(falta.estado, Estado::NoDisponible, "{nombre}: {falta:?}");
        }
        let ejecutada = std::path::Path::new(&marca).exists();
        let _ = std::fs::remove_file(&marca);
        let _ = std::fs::remove_dir_all(&carpeta);
        assert!(!ejecutada, "un nombre de archivo ejecutó una orden");
    }

    #[cfg(not(windows))]
    #[test]
    fn elsewhere_it_says_it_is_not_available() {
        let f = comprobar(std::path::Path::new("/tmp/x.exe"));
        assert_eq!(f.estado, Estado::NoDisponible);
    }
}
