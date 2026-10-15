//! Whether this computer's traffic is leaving through a VPN right now (15 Oct 2026: the
//! planet placed the person where the VPN comes out, and «Precisar» measured the VPN's city).
//! Read-only: the adapters that are up, and the VPN connections Windows itself keeps, nothing
//! else. The names are what Windows shows; no address is read or kept.

// Off Windows only the decision and its test compile; the rest waits for Windows.
#![cfg_attr(not(windows), allow(dead_code))]

use super::Error;

/// A VPN seen active: the adapter or connection name, as Windows shows it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Vpn {
    /// What it is called in Windows.
    pub nombre: String,
    /// `adaptador` (a tunnel adapter that is up) or `conexion` (a Windows VPN connection
    /// reported as connected).
    pub via: &'static str,
}

const TUNELES: &[&str] = &[
    "wintun",
    "tap-windows",
    "wireguard",
    "openvpn",
    "nordlynx",
    "proton",
    "fortinet",
    "anyconnect",
    "globalprotect",
    "tailscale",
    "zerotier",
    "expressvpn",
    "surfshark",
    "mullvad",
    "cloudflare warp",
    "pangp",
    "sonicwall",
    "vpn",
];

/// Decide from what Windows listed. Separate from the call so it can be tested anywhere.
pub(crate) fn decide(adaptadores: &[(String, String)], conexiones: &[(String, bool)]) -> Vec<Vpn> {
    let mut v: Vec<Vpn> = adaptadores
        .iter()
        .filter(|(n, d)| {
            let t = format!("{n} {d}").to_lowercase();
            TUNELES.iter().any(|x| t.contains(x))
        })
        .map(|(n, _)| Vpn {
            nombre: n.clone(),
            via: "adaptador",
        })
        .collect();
    for (n, conectada) in conexiones {
        if *conectada && !v.iter().any(|x| &x.nombre == n) {
            v.push(Vpn {
                nombre: n.clone(),
                via: "conexion",
            });
        }
    }
    v
}

/// Ask Windows. Takes about a second; the caller keeps the answer for a while.
#[cfg(windows)]
pub(crate) fn activas() -> Result<Vec<Vpn>, Error> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let orden = "$a = @(Get-NetAdapter | Where-Object { $_.Status -eq 'Up' } | ForEach-Object { @{ n = [string]$_.Name; d = [string]$_.InterfaceDescription } }); \
                 $c = @(@(Get-VpnConnection -AllUserConnection -ErrorAction SilentlyContinue) + @(Get-VpnConnection -ErrorAction SilentlyContinue) | Where-Object { $_ } | ForEach-Object { @{ n = [string]$_.Name; c = ([string]$_.ConnectionStatus -eq 'Connected') } }); \
                 @{ a = $a; c = $c } | ConvertTo-Json -Compress -Depth 3";
    let salida = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", orden])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| Error::Sistema(e.to_string()))?;
    let texto = String::from_utf8_lossy(&salida.stdout);
    let v: serde_json::Value =
        serde_json::from_str(texto.trim()).map_err(|e| Error::Sistema(e.to_string()))?;
    let lista = |x: &serde_json::Value| x.as_array().cloned().unwrap_or_default();
    let adaptadores: Vec<(String, String)> = lista(&v["a"])
        .iter()
        .map(|a| {
            (
                a["n"].as_str().unwrap_or_default().to_owned(),
                a["d"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    let conexiones: Vec<(String, bool)> = lista(&v["c"])
        .iter()
        .map(|c| {
            (
                c["n"].as_str().unwrap_or_default().to_owned(),
                c["c"].as_bool().unwrap_or(false),
            )
        })
        .collect();
    Ok(decide(&adaptadores, &conexiones))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tunnel_adapter_or_a_connected_connection_counts_and_a_plain_wifi_does_not() {
        let a = vec![
            ("Wi-Fi".to_owned(), "Intel(R) Wi-Fi 6 AX201".to_owned()),
            ("NordLynx".to_owned(), "NordLynx Tunnel".to_owned()),
        ];
        let c = vec![("VPN oficina".to_owned(), true), ("Casa".to_owned(), false)];
        let v = decide(&a, &c);
        assert_eq!(v.len(), 2);
        assert_eq!((v[0].nombre.as_str(), v[0].via), ("NordLynx", "adaptador"));
        assert_eq!(
            (v[1].nombre.as_str(), v[1].via),
            ("VPN oficina", "conexion")
        );
        assert!(decide(&a[..1], &c[1..]).is_empty());
    }
}
