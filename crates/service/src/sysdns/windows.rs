//! Windows: per connected IPv4 interface through PowerShell's DnsClient
//! cmdlets, which are locale-independent (netsh prints localized text).

use std::net::IpAddr;

use serde::Deserialize;

use super::{guarded_servers, run_checked, Backup, Error, InterfaceDns, Method};

const SNAPSHOT_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$out = @()
Get-NetIPInterface -AddressFamily IPv4 | Where-Object { $_.ConnectionState -eq 'Connected' -and $_.InterfaceAlias -notlike 'Loopback*' } | ForEach-Object {
  $i = $_
  $d = Get-DnsClientServerAddress -InterfaceIndex $i.InterfaceIndex -AddressFamily IPv4
  $out += [pscustomobject]@{ index = $i.InterfaceIndex; alias = $i.InterfaceAlias; dhcp = ($i.Dhcp -eq 'Enabled'); servers = @($d.ServerAddresses) }
}
ConvertTo-Json -InputObject $out -Compress -Depth 3
"#;

#[derive(Debug, Deserialize)]
struct Raw {
    index: u32,
    alias: String,
    dhcp: bool,
    #[serde(default)]
    servers: Vec<String>,
}

fn powershell(script: &str) -> Result<String, Error> {
    run_checked(
        "powershell",
        &[
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ],
    )
}

/// Parse the JSON produced by [`SNAPSHOT_SCRIPT`] (an array, or one object when
/// PowerShell collapses a single element).
pub(crate) fn parse_snapshot(json: &str) -> Result<Vec<InterfaceDns>, Error> {
    let trimmed = json.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let raws: Vec<Raw> = if trimmed.starts_with('[') {
        serde_json::from_str(trimmed)?
    } else {
        vec![serde_json::from_str(trimmed)?]
    };
    Ok(raws
        .into_iter()
        .map(|r| InterfaceDns {
            id: r.index.to_string(),
            name: r.alias,
            servers: r.servers.iter().filter_map(|s| s.parse().ok()).collect(),
            automatic: r.dhcp,
            extra: None,
        })
        .filter(|i| !i.servers.is_empty())
        .collect())
}

pub(crate) fn snapshot(now: i64) -> Result<Backup, Error> {
    let json = powershell(SNAPSHOT_SCRIPT)?;
    let interfaces = parse_snapshot(&json)?;
    if interfaces.is_empty() {
        return Err(Error::NothingToChange);
    }
    Ok(Backup {
        taken_at: now,
        method: Method::WindowsDnsClient,
        interfaces,
        resolv_conf: None,
        resolv_link: None,
        made_dropin_dir: false,
    })
}

fn set_servers(index: &str, servers: &[IpAddr]) -> Result<(), Error> {
    let list = servers
        .iter()
        .map(|s| format!("'{s}'"))
        .collect::<Vec<_>>()
        .join(",");
    powershell(&format!(
        "$ErrorActionPreference='Stop'; Set-DnsClientServerAddress -InterfaceIndex {index} -ServerAddresses ({list})"
    ))?;
    Ok(())
}

pub(crate) fn apply(backup: &Backup, guardian: IpAddr) -> Result<(), Error> {
    for i in &backup.interfaces {
        set_servers(&i.id, &guarded_servers(guardian, &i.servers))?;
    }
    Ok(())
}

pub(crate) fn restore(backup: &Backup) -> Result<(), Error> {
    for i in &backup.interfaces {
        if i.automatic {
            powershell(&format!(
                "$ErrorActionPreference='Stop'; Set-DnsClientServerAddress -InterfaceIndex {} -ResetServerAddresses",
                i.id
            ))?;
        } else {
            set_servers(&i.id, &i.servers)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_array_and_single_object() {
        let arr = r#"[{"index":12,"alias":"Ethernet","dhcp":true,"servers":["192.168.1.1"]},{"index":3,"alias":"Wi-Fi","dhcp":false,"servers":[]}]"#;
        let v = parse_snapshot(arr).unwrap_or_default();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].id, "12");
        assert!(v[0].automatic);
        let one = r#"{"index":5,"alias":"Wi-Fi","dhcp":false,"servers":["8.8.8.8","8.8.4.4"]}"#;
        let v = parse_snapshot(one).unwrap_or_default();
        assert_eq!(v[0].servers.len(), 2);
        assert!(!v[0].automatic);
    }
}
