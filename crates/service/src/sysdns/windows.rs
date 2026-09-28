//! Windows: per connected interface through PowerShell's DnsClient cmdlets,
//! which are locale-independent (netsh prints localized text).
//!
//! Both families, and Guardiana alone (28 Sep 2026). Until then only the IPv4
//! list was touched, with the old resolvers left behind 127.0.0.1 as a reserve.
//! Two things went wrong on the first real PC:
//!
//! - At home the Wi-Fi (Claro) also handed out IPv6 resolvers, and Windows asked
//!   those: in ten minutes not one query reached Guardiana, while `verify` said
//!   "Guardiana is first". The same PC at the office, on a network without IPv6,
//!   showed everything. So now `::1` goes first in the IPv6 list as well, and
//!   Guardiana listens there.
//! - With 1.1.1.1 left as a reserve, Edge (and Copilot, WebView2) saw a resolver
//!   it knows how to encrypt and sent every name to Cloudflare over HTTPS: only
//!   `chrome.cloudflare-dns.com` reached Guardiana, 430 times in two hours. So
//!   Windows now sees Guardiana and nothing else; Guardiana keeps asking the old
//!   resolvers on its own, and when the service stops it gives the machine its
//!   old DNS back until it starts again (engine::run).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use serde::{Deserialize, Serialize};

use super::{run_checked, Backup, Error, InterfaceDns, Method};

const SNAPSHOT_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$out = @()
Get-NetIPInterface -AddressFamily IPv4 | Where-Object { $_.ConnectionState -eq 'Connected' -and $_.InterfaceAlias -notlike 'Loopback*' } | ForEach-Object {
  $i = $_
  $d = Get-DnsClientServerAddress -InterfaceIndex $i.InterfaceIndex -AddressFamily IPv4
  $g = $null
  try { $g = (Get-NetAdapter -InterfaceIndex $i.InterfaceIndex -ErrorAction Stop).InterfaceGuid } catch { $g = $null }
  $s4 = ''
  $s6 = ''
  if ($g) {
    $s4 = [string](Get-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\$g" -Name NameServer -ErrorAction SilentlyContinue).NameServer
    $s6 = [string](Get-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip6\Parameters\Interfaces\$g" -Name NameServer -ErrorAction SilentlyContinue).NameServer
  }
  $out += [pscustomobject]@{ index = $i.InterfaceIndex; alias = $i.InterfaceAlias; dhcp = ($i.Dhcp -eq 'Enabled'); servers = @($d.ServerAddresses); guid = [bool]$g; static4 = $s4; static6 = $s6 }
}
ConvertTo-Json -InputObject $out -Compress -Depth 3
"#;

/// One line per connected interface with resolvers: `v4,list|v6,list`.
const PRIMARY_SCRIPT: &str = r#"
Get-NetIPInterface -AddressFamily IPv4 | Where-Object { $_.ConnectionState -eq 'Connected' -and $_.InterfaceAlias -notlike 'Loopback*' } | ForEach-Object {
  $v4 = @((Get-DnsClientServerAddress -InterfaceIndex $_.InterfaceIndex -AddressFamily IPv4).ServerAddresses | Where-Object { $_ })
  $d6 = Get-DnsClientServerAddress -InterfaceIndex $_.InterfaceIndex -AddressFamily IPv6 -ErrorAction SilentlyContinue
  $v6 = @()
  if ($d6) { $v6 = @($d6.ServerAddresses | Where-Object { $_ }) }
  if ($v4.Count -gt 0) { ($v4 -join ',') + '|' + ($v6 -join ',') }
}
"#;

#[derive(Debug, Deserialize)]
struct Raw {
    index: u32,
    alias: String,
    dhcp: bool,
    #[serde(default)]
    servers: Vec<String>,
    #[serde(default)]
    guid: bool,
    #[serde(default)]
    static4: String,
    #[serde(default)]
    static6: String,
}

/// What Windows needs, beyond the common fields, to put an interface back exactly.
/// Stored as JSON in [`InterfaceDns::extra`]; absent in backups taken before 28 Sep 2026,
/// which are restored the old way.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct WinExtra {
    /// IPv4 resolvers typed by hand (the registry's `NameServer`); empty when they came
    /// from DHCP.
    #[serde(default)]
    manual4: Vec<IpAddr>,
    /// Same for IPv6.
    #[serde(default)]
    manual6: Vec<IpAddr>,
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

fn parse_ip(s: &str) -> Option<IpAddr> {
    s.trim().split('%').next().and_then(|b| b.parse().ok())
}

/// The registry keeps hand-typed resolvers as one string, separated by commas or spaces.
fn parse_list(s: &str) -> Vec<IpAddr> {
    s.split(|c: char| c == ',' || c.is_whitespace())
        .filter_map(parse_ip)
        .collect()
}

/// IPv6 resolvers Windows really uses: the `fec0:0:0:ffff::` placeholders it lists when
/// nothing is configured are not resolvers.
fn real_v6(list: &[IpAddr]) -> Vec<IpAddr> {
    list.iter()
        .copied()
        .filter(|ip| match ip {
            IpAddr::V6(v6) => (v6.segments()[0] & 0xffc0) != 0xfec0,
            IpAddr::V4(_) => false,
        })
        .collect()
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
        .map(|r| {
            let extra = r.guid.then(|| WinExtra {
                manual4: parse_list(&r.static4),
                manual6: parse_list(&r.static6),
            });
            InterfaceDns {
                id: r.index.to_string(),
                name: r.alias,
                // The loopback is never an upstream: if something else was already answering
                // on this machine, forwarding to it would be forwarding to ourselves.
                servers: r
                    .servers
                    .iter()
                    .filter_map(|s| parse_ip(s))
                    .filter(|ip| !ip.is_loopback())
                    .collect(),
                automatic: match &extra {
                    Some(x) => x.manual4.is_empty(),
                    None => r.dhcp,
                },
                extra: extra.and_then(|x| serde_json::to_string(&x).ok()),
            }
        })
        .filter(|i| !i.servers.is_empty())
        .collect())
}

fn win_extra(i: &InterfaceDns) -> Option<WinExtra> {
    i.extra
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
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

fn quoted(servers: &[IpAddr]) -> String {
    servers
        .iter()
        .map(|s| format!("'{s}'"))
        .collect::<Vec<_>>()
        .join(",")
}

fn set_servers(index: &str, servers: &[IpAddr]) -> Result<(), Error> {
    powershell(&format!(
        "$ErrorActionPreference='Stop'; Set-DnsClientServerAddress -InterfaceIndex {index} -ServerAddresses ({}); Clear-DnsClientCache",
        quoted(servers)
    ))?;
    Ok(())
}

/// Guardiana alone, in both families. If the interface has no IPv6 at all the cmdlet
/// refuses the `::1`, and then IPv4 alone is the honest best: with no IPv6 there is no
/// IPv6 resolver to go around it.
pub(crate) fn apply(backup: &Backup, guardian: IpAddr) -> Result<(), Error> {
    let guardian6 = IpAddr::V6(Ipv6Addr::LOCALHOST);
    for i in &backup.interfaces {
        if set_servers(&i.id, &[guardian, guardian6]).is_err() {
            set_servers(&i.id, &[guardian])?;
        }
    }
    Ok(())
}

pub(crate) fn restore(backup: &Backup) -> Result<(), Error> {
    for i in &backup.interfaces {
        match win_extra(i) {
            Some(x) => {
                // Back to automatic in both families, then the hand-typed ones, if there were
                // any, exactly as they were. `-ResetServerAddresses` is what Settings calls
                // "Automatic (DHCP)"; the `netsh` line is there because on some builds the reset
                // leaves the IPv6 list alone, and a `::1` left behind would point at a program
                // that is no longer there.
                let mut manual = x.manual4.clone();
                manual.extend(x.manual6.iter().copied());
                let set = if manual.is_empty() {
                    String::new()
                } else {
                    format!(
                        "Set-DnsClientServerAddress -InterfaceIndex {} -ServerAddresses ({}); ",
                        i.id,
                        quoted(&manual)
                    )
                };
                let fix6 = if x.manual6.is_empty() {
                    format!(
                        "$d6 = Get-DnsClientServerAddress -InterfaceIndex {id} -AddressFamily IPv6 -ErrorAction SilentlyContinue; if ($d6 -and (@($d6.ServerAddresses) -contains '::1')) {{ netsh interface ipv6 set dnsservers name={id} source=dhcp | Out-Null }}; ",
                        id = i.id
                    )
                } else {
                    String::new()
                };
                powershell(&format!(
                    "$ErrorActionPreference='Stop'; Set-DnsClientServerAddress -InterfaceIndex {} -ResetServerAddresses; {set}{fix6}Clear-DnsClientCache",
                    i.id
                ))?;
            }
            None if i.automatic => {
                powershell(&format!(
                    "$ErrorActionPreference='Stop'; Set-DnsClientServerAddress -InterfaceIndex {} -ResetServerAddresses",
                    i.id
                ))?;
            }
            None => set_servers(&i.id, &i.servers)?,
        }
    }
    Ok(())
}

/// Parse one line of [`PRIMARY_SCRIPT`]: whether Windows sees Guardiana and nothing else.
fn line_is_guarded(line: &str) -> bool {
    let (v4, v6) = line.split_once('|').unwrap_or((line, ""));
    let v4: Vec<IpAddr> = parse_list(v4);
    let v6 = real_v6(&parse_list(v6));
    v4 == [IpAddr::V4(Ipv4Addr::LOCALHOST)]
        && (v6.is_empty() || v6 == [IpAddr::V6(Ipv6Addr::LOCALHOST)])
}

/// Whether every connected interface with resolvers asks Guardiana and only Guardiana,
/// in both families. `None` when PowerShell could not say.
pub(crate) fn guardian_is_primary() -> Option<bool> {
    let out = powershell(PRIMARY_SCRIPT).ok()?;
    let lines: Vec<&str> = out
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return None;
    }
    Some(lines.iter().all(|l| line_is_guarded(l)))
}

/// What Guardiana should forward to right now. Hand-typed resolvers stay the person's
/// choice wherever the machine goes. Automatic ones follow the network the machine is on
/// now: a laptop set up at home carries a router address (192.168.1.1) that does not exist
/// at the office, and with Guardiana as the only resolver that would be no internet at all.
pub(crate) fn upstreams_for(backup: &Backup) -> Vec<IpAddr> {
    let automatic: Vec<&InterfaceDns> = backup
        .interfaces
        .iter()
        .filter(|i| win_extra(i).is_some_and(|x| x.manual4.is_empty()))
        .collect();
    let mut dhcp: std::collections::HashMap<String, Vec<IpAddr>> = std::collections::HashMap::new();
    if !automatic.is_empty() {
        let script = automatic
            .iter()
            .map(|i| {
                format!(
                    "$g = (Get-NetAdapter -InterfaceIndex {id} -ErrorAction SilentlyContinue).InterfaceGuid; if ($g) {{ '{id}|' + [string](Get-ItemProperty -Path \"HKLM:\\SYSTEM\\CurrentControlSet\\Services\\Tcpip\\Parameters\\Interfaces\\$g\" -Name DhcpNameServer -ErrorAction SilentlyContinue).DhcpNameServer }}",
                    id = i.id
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        if let Ok(out) = powershell(&script) {
            for line in out.lines() {
                if let Some((id, list)) = line.trim().split_once('|') {
                    dhcp.insert(id.to_owned(), parse_list(list));
                }
            }
        }
    }
    let mut out: Vec<IpAddr> = Vec::new();
    for i in &backup.interfaces {
        let now: Vec<IpAddr> = match win_extra(i) {
            Some(x) if !x.manual4.is_empty() => x.manual4,
            Some(_) => dhcp.get(&i.id).cloned().unwrap_or_default(),
            None => Vec::new(),
        };
        let chosen = if now.is_empty() {
            i.servers.clone()
        } else {
            now
        };
        for ip in chosen {
            if !ip.is_loopback() && !out.contains(&ip) {
                out.push(ip);
            }
        }
    }
    out
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

    #[test]
    fn reads_ipv6_and_what_was_typed_by_hand() {
        let one = r#"{"index":3,"alias":"Wi-Fi","dhcp":true,"servers":["1.1.1.1","1.0.0.1"],"servers6":["2800:e0::ac1d:f00d:3","fec0:0:0:ffff::1%1"],"guid":true,"static4":"1.1.1.1,1.0.0.1","static6":""}"#;
        let v = parse_snapshot(one).unwrap_or_default();
        assert_eq!(v.len(), 1);
        // Typed by hand, so not automatic even though the address itself comes from DHCP.
        assert!(!v[0].automatic);
        let x = win_extra(&v[0]).unwrap_or_default();
        assert_eq!(x.manual4.len(), 2);
        assert!(x.manual6.is_empty());

        let dhcp = r#"{"index":3,"alias":"Wi-Fi","dhcp":true,"servers":["192.168.1.1"],"servers6":"2800:e0::ac1d:f00d:3","guid":true,"static4":"","static6":""}"#;
        let v = parse_snapshot(dhcp).unwrap_or_default();
        assert!(v[0].automatic);
        assert!(win_extra(&v[0]).is_some());
    }

    #[test]
    fn the_loopback_is_never_an_upstream() {
        let one = r#"{"index":3,"alias":"Wi-Fi","dhcp":true,"servers":["127.0.0.1","192.168.1.1"],"guid":false}"#;
        let v = parse_snapshot(one).unwrap_or_default();
        assert_eq!(v[0].servers.len(), 1);
        assert!(v[0].extra.is_none());
    }

    #[test]
    fn guarded_means_guardiana_alone_in_both_families() {
        assert!(line_is_guarded("127.0.0.1|::1"));
        assert!(line_is_guarded("127.0.0.1|"));
        assert!(line_is_guarded(
            "127.0.0.1|fec0:0:0:ffff::1%1,fec0:0:0:ffff::2%1"
        ));
        // What the PC had on 27 Sep 2026: IPv4 guarded, IPv6 going round it.
        assert!(!line_is_guarded(
            "127.0.0.1,1.1.1.1,1.0.0.1|2800:e0::ac1d:f00d:3"
        ));
        assert!(!line_is_guarded("127.0.0.1|2800:e0::ac1d:f00d:3"));
        // A reserve that Edge upgrades to encrypted DNS is a way round it too.
        assert!(!line_is_guarded("127.0.0.1,1.1.1.1|::1"));
    }
}
