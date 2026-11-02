//! Containment (the owner, 2 Nov 2026, 07:55: «tenemos que poder detectarlos y sacarlos»): which programs may be
//! frozen at once, and the orders that delete Windows' backup copies, the step a ransomware takes before it encrypts so
//! nothing can be restored.
//!
//! Freezing a program (Windows suspends all its threads) stops it where it is and loses nothing: it can carry on, or be
//! stopped and put in the quarantine. What is never frozen are Windows' own processes, whose stop would hang the
//! computer; Windows' tools that a ransomware uses (the command interpreters, the backups' tools) are, because they run
//! what it asks.

use crate::primera_vez::Aviso;

/// Windows' tools a ransomware runs, which may be frozen even though they live in Windows' folder.
const HERRAMIENTAS: &[&str] = &[
    "cmd.exe",
    "powershell.exe",
    "pwsh.exe",
    "wscript.exe",
    "cscript.exe",
    "mshta.exe",
    "rundll32.exe",
    "regsvr32.exe",
    "vssadmin.exe",
    "wmic.exe",
    "wbadmin.exe",
    "bcdedit.exe",
    "diskshadow.exe",
    "cipher.exe",
    "certutil.exe",
    "bitsadmin.exe",
];

/// Whether the program at `ruta` may be frozen: anything outside Windows' folder, and Windows' tools of
/// [`HERRAMIENTAS`]; never the rest of Windows (its services, its session and logon processes, the explorer).
#[must_use]
pub fn se_puede_congelar(programa: &str, ruta: &str) -> bool {
    let r = ruta.replace('/', "\\").to_ascii_lowercase();
    let r = r.trim_start_matches(r"\\?\");
    if r.is_empty() || programa.is_empty() {
        return false;
    }
    let de_windows = r.get(1..).is_some_and(|x| x.starts_with(r":\windows\"));
    !de_windows
        || HERRAMIENTAS
            .iter()
            .any(|h| programa.eq_ignore_ascii_case(h))
}

/// Whether a program's command line deletes Windows' backup copies (shadow copies), its catalogue of backups, or turns
/// off the recovery: `vssadmin delete shadows`, `vssadmin resize shadowstorage` to a tiny size, `wmic shadowcopy delete`,
/// `wbadmin delete catalog` or `systemstatebackup`, `bcdedit` turning recovery off, `diskshadow delete shadows`, and the
/// same through PowerShell (`Win32_ShadowCopy` removed), also when its command comes encoded (`-EncodedCommand`).
#[must_use]
pub fn borra_copias(programa: &str, linea: &str) -> bool {
    let p = programa.to_ascii_lowercase();
    let l = linea.to_ascii_lowercase();
    let tiene = |s: &str| l.contains(s);
    match p.as_str() {
        "vssadmin.exe" => {
            (tiene("delete") && tiene("shadows"))
                || (tiene("resize") && tiene("shadowstorage") && tiene("maxsize"))
        }
        "wmic.exe" => tiene("shadowcopy") && tiene("delete"),
        "wbadmin.exe" => {
            tiene("delete") && (tiene("catalog") || tiene("systemstatebackup") || tiene("backup"))
        }
        "bcdedit.exe" => {
            (tiene("recoveryenabled") && (tiene(" no") || tiene("=no") || tiene("off")))
                || tiene("ignoreallfailures")
        }
        "diskshadow.exe" => tiene("delete") && tiene("shadows"),
        "powershell.exe" | "pwsh.exe" => {
            // the encoded command from the line as written: Base64 tells upper from lower case
            let claro = powershell_en_claro(linea).unwrap_or_default();
            let todo = format!("{l} {claro}");
            todo.contains("shadowcopy")
                && (todo.contains("remove-")
                    || todo.contains("delete")
                    || todo.contains(".delete("))
        }
        _ => false,
    }
}

/// The command a PowerShell received encoded (`-EncodedCommand`, `-enc`, `-e`: Base64 of UTF-16LE), in lower case.
fn powershell_en_claro(linea: &str) -> Option<String> {
    let mut partes = linea.split_whitespace();
    while let Some(p) = partes.next() {
        let p = p.trim_start_matches(['-', '/']).to_ascii_lowercase();
        if !p.is_empty() && "encodedcommand".starts_with(p.as_str()) && p.starts_with('e') {
            let b64 = partes.next()?.trim_matches('"');
            let bytes = base64_decodifica(b64)?;
            let u: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            return Some(String::from_utf16_lossy(&u).to_ascii_lowercase());
        }
    }
    None
}

/// Standard Base64 (with or without padding).
fn base64_decodifica(s: &str) -> Option<Vec<u8>> {
    let valor = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => u32::from(c - b'A'),
            b'a'..=b'z' => u32::from(c - b'a') + 26,
            b'0'..=b'9' => u32::from(c - b'0') + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        })
    };
    let limpio: Vec<u8> = s.bytes().filter(|c| *c != b'=').collect();
    let mut out = Vec::with_capacity(limpio.len() * 3 / 4);
    for trozo in limpio.chunks(4) {
        let mut n = 0u32;
        for (i, c) in trozo.iter().enumerate() {
            n |= valor(*c)? << (18 - 6 * i);
        }
        let bytes = n.to_be_bytes();
        out.extend_from_slice(&bytes[1..trozo.len()]);
    }
    Some(out)
}

/// The warning for a program that deletes the backup copies: `programa` (the tool) started by `padre`.
#[must_use]
pub fn aviso_copias(programa: &str, ruta: &str, padre: &str, cuando_ms: u64) -> Aviso {
    let quien = if padre.is_empty() {
        String::new()
    } else {
        format!(", abierto por «{padre}»,")
    };
    Aviso {
        regla: "copias_borradas",
        programa: programa.to_string(),
        ruta: ruta.to_string(),
        sitio: String::new(),
        nombre: programa.to_ascii_lowercase(),
        cuando_ms,
        fuente: None,
        texto: format!(
            "«{programa}»{quien} ha intentado borrar las copias de seguridad de Windows (instantáneas) o desactivar su \
             recuperación. Es lo primero que hace un programa de secuestro de archivos (ransomware) para que no puedas \
             recuperar nada. GUARDIANA lo ha congelado: elige detenerlo o dejarlo seguir si eres tú quien lo ordenó."
        ),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn que_se_puede_congelar() {
        assert!(se_puede_congelar(
            "cifra.exe",
            r"C:\Users\ana\AppData\Local\Temp\cifra.exe"
        ));
        assert!(se_puede_congelar("x.exe", r"C:\Program Files\Algo\x.exe"));
        assert!(se_puede_congelar(
            "vssadmin.exe",
            r"C:\Windows\System32\vssadmin.exe"
        ));
        assert!(se_puede_congelar(
            "powershell.exe",
            r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"
        ));
        for (p, r) in [
            ("csrss.exe", r"C:\Windows\System32\csrss.exe"),
            ("lsass.exe", r"\\?\C:\Windows\System32\lsass.exe"),
            ("svchost.exe", r"c:\windows\system32\svchost.exe"),
            ("explorer.exe", r"C:\Windows\explorer.exe"),
            ("winlogon.exe", r"C:\Windows\System32\winlogon.exe"),
            ("", r"C:\x.exe"),
            ("x.exe", ""),
        ] {
            assert!(!se_puede_congelar(p, r), "{p} {r}");
        }
    }

    #[test]
    fn las_ordenes_que_borran_las_copias() {
        for (p, l) in [
            ("vssadmin.exe", "vssadmin.exe Delete Shadows /All /Quiet"),
            (
                "vssadmin.exe",
                "vssadmin resize shadowstorage /for=c: /on=c: /maxsize=401MB",
            ),
            ("wmic.exe", "wmic SHADOWCOPY DELETE /nointeractive"),
            ("WMIC.exe", "wmic.exe shadowcopy where \"ID='x'\" delete"),
            (
                "wbadmin.exe",
                "wbadmin DELETE SYSTEMSTATEBACKUP -keepVersions:0",
            ),
            ("wbadmin.exe", "wbadmin delete catalog -quiet"),
            ("bcdedit.exe", "bcdedit /set {default} recoveryenabled No"),
            (
                "bcdedit.exe",
                "bcdedit /set {default} bootstatuspolicy ignoreallfailures",
            ),
            ("diskshadow.exe", "diskshadow delete shadows all"),
            (
                "powershell.exe",
                "powershell -c \"Get-WmiObject Win32_Shadowcopy | ForEach-Object { $_.Delete() }\"",
            ),
            (
                "pwsh.exe",
                "pwsh -NoP -Command Get-CimInstance Win32_ShadowCopy | Remove-CimInstance",
            ),
        ] {
            assert!(borra_copias(p, l), "{l}");
        }
        // the same PowerShell, encoded: «Get-CimInstance Win32_ShadowCopy | Remove-CimInstance» as UTF-16LE Base64
        let orden = "Get-CimInstance Win32_ShadowCopy | Remove-CimInstance";
        let bytes: Vec<u8> = orden.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let b64 = codifica(&bytes);
        assert!(borra_copias(
            "powershell.exe",
            &format!("powershell.exe -NoProfile -enc {b64}")
        ));
        assert!(borra_copias(
            "powershell.exe",
            &format!("powershell -EncodedCommand {b64}")
        ));
        for (p, l) in [
            ("vssadmin.exe", "vssadmin list shadows"),
            ("wmic.exe", "wmic shadowcopy list brief"),
            ("wbadmin.exe", "wbadmin get versions"),
            ("bcdedit.exe", "bcdedit /enum"),
            (
                "powershell.exe",
                "powershell Get-CimInstance Win32_ShadowCopy",
            ),
            ("cmd.exe", "cmd /c vssadmin delete shadows /all /quiet"),
            ("notepad.exe", "notepad delete shadows"),
        ] {
            assert!(!borra_copias(p, l), "{l}");
        }
        let a = aviso_copias(
            "vssadmin.exe",
            r"C:\Windows\System32\vssadmin.exe",
            "cifra.exe",
            7,
        );
        assert_eq!(a.regla, "copias_borradas");
        assert!(a.texto.contains("«cifra.exe»") && a.texto.contains("congelado"));
    }

    fn codifica(b: &[u8]) -> String {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s = String::new();
        for c in b.chunks(3) {
            let n = (u32::from(c[0]) << 16)
                | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
                | u32::from(*c.get(2).unwrap_or(&0));
            for i in 0..4 {
                if i <= c.len() {
                    s.push(char::from(T[((n >> (18 - 6 * i)) & 63) as usize]));
                } else {
                    s.push('=');
                }
            }
        }
        s
    }
}
