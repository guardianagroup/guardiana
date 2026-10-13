//! What Windows Security Center says is installed (1.0.21, «Analizar mi antivirus»).
//!
//! Every antivirus registers itself with Windows Security Center, and Windows publishes the list
//! in `root/SecurityCenter2`, class `AntiVirusProduct`: its name, the program it is and its state.
//! Read-only, asked when the person presses the button.

use super::{lee_estado, AntivirusInstalado, Error};

pub(crate) fn instalados() -> Result<Vec<AntivirusInstalado>, Error> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let orden = "Get-CimInstance -Namespace root/SecurityCenter2 -ClassName AntiVirusProduct \
                 | Select-Object displayName, pathToSignedProductExe, productState \
                 | ConvertTo-Json -Compress";
    let salida = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", orden])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| Error::Sistema(e.to_string()))?;
    let texto = String::from_utf8_lossy(&salida.stdout);
    let texto = texto.trim();
    if texto.is_empty() {
        // Windows Server and some editions have no Security Center: nothing listed is an answer.
        return Ok(Vec::new());
    }
    let valor: serde_json::Value =
        serde_json::from_str(texto).map_err(|e| Error::Sistema(e.to_string()))?;
    let lista = match valor {
        serde_json::Value::Array(v) => v,
        otro => vec![otro],
    };
    Ok(lista
        .iter()
        .filter_map(|p| {
            let nombre = p["displayName"].as_str()?.trim().to_owned();
            let estado = u32::try_from(p["productState"].as_u64().unwrap_or(0)).unwrap_or(0);
            let (activo, al_dia) = lee_estado(estado);
            Some(AntivirusInstalado {
                nombre,
                ruta: p["pathToSignedProductExe"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                estado,
                activo,
                al_dia,
            })
        })
        .collect())
}
