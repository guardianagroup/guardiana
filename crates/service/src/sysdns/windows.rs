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

use super::{run_checked, undo_each, Backup, Error, InterfaceDns, Method};

/// Tunnel (131) and virtual (53) adapters are left alone, in the copy and in the watch: a VPN
/// brings its own resolver for its own names, and pointing it here sent the office names to the
/// home router and fought the VPN client every minute (review of 8 Oct 2026). What the VPN
/// resolves, Guardiana does not see, and the panel says so.
const SNAPSHOT_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$out = @()
Get-NetIPInterface -AddressFamily IPv4 | Where-Object { $_.ConnectionState -eq 'Connected' -and $_.InterfaceAlias -notlike 'Loopback*' } | ForEach-Object {
  if (Get-Command Get-NetAdapter -ErrorAction SilentlyContinue) {
    $a = Get-NetAdapter -InterfaceIndex $_.InterfaceIndex -ErrorAction SilentlyContinue
    if ($a -and ($a.InterfaceType -in 131, 53)) { return }
  }
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
  if (Get-Command Get-NetAdapter -ErrorAction SilentlyContinue) {
    $a = Get-NetAdapter -InterfaceIndex $_.InterfaceIndex -ErrorAction SilentlyContinue
    if ($a -and ($a.InterfaceType -in 131, 53)) { return }
  }
  $v4 = @((Get-DnsClientServerAddress -InterfaceIndex $_.InterfaceIndex -AddressFamily IPv4).ServerAddresses | Where-Object { $_ })
  $d6 = Get-DnsClientServerAddress -InterfaceIndex $_.InterfaceIndex -AddressFamily IPv6 -ErrorAction SilentlyContinue
  $v6 = @()
  if ($d6) { $v6 = @($d6.ServerAddresses | Where-Object { $_ }) }
  if ($v4.Count -gt 0 -or $v6.Count -gt 0) { ($v4 -join ',') + '|' + ($v6 -join ',') }
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

/// No `-ExecutionPolicy Bypass`: the policy only governs script *files*, never a `-Command`
/// string, so the flag changed nothing here -- and it is the most recognised marker of malware
/// that starts PowerShell. Defender called guardiana.exe `Trojan:Win32/Wacatac.H!ml` on
/// 5 Oct 2026 (a machine-learning guess); this is one fewer thing in it that looks like malware.
fn powershell(script: &str) -> Result<String, Error> {
    run_checked(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-Command", script],
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

/// The interface index, the only part of a backup that goes into a PowerShell line as text.
///
/// The copy lives in the extract (and in a plain file beside it), which the console user can
/// write without being an administrator, while the service runs the line as SYSTEM: an `id` of
/// `1; <anything>` ran that anything as SYSTEM within a minute (review of 10 Oct 2026, system
/// serious 2). Snapshot writes a number; anything else is not an interface Windows has, and is
/// left alone. The resolvers and `extra` are already typed (`IpAddr`), never raw text.
fn indice(i: &InterfaceDns) -> Option<u32> {
    let id = i.id.trim();
    if id.is_empty() || id.len() > 10 || !id.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    id.parse().ok()
}

fn set_servers(index: u32, servers: &[IpAddr]) -> Result<(), Error> {
    powershell(&format!(
        "$ErrorActionPreference='Stop'; Set-DnsClientServerAddress -InterfaceIndex {index} -ServerAddresses ({}); Clear-DnsClientCache",
        quoted(servers)
    ))?;
    Ok(())
}

/// Guardiana alone, in both families. If the interface has no IPv6 at all the cmdlet
/// refuses the `::1`, and then IPv4 alone is the honest best: with no IPv6 there is no
/// IPv6 resolver to go around it.
///
/// An interface that is gone (the trip's VPN, a dock left at the office) is skipped, not an
/// error. Until 1.0.2 it stopped the loop: every interface after it in the copy, the Wi-Fi
/// included, was never pointed at Guardiana again, and the watchdog failed on it every minute
/// for ever (review of 5 Oct 2026, serious 10).
pub(crate) fn apply(backup: &Backup, guardian: IpAddr) -> Result<(), Error> {
    let guardian6 = IpAddr::V6(Ipv6Addr::LOCALHOST);
    for i in &backup.interfaces {
        let Some(n) = indice(i) else {
            continue;
        };
        if set_servers(n, &[guardian, guardian6]).is_ok() {
            continue;
        }
        match set_servers(n, &[guardian]) {
            Ok(()) => {}
            Err(e) if interface_is_gone(&e.to_string()) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// The PowerShell that puts one interface back, built apart so a test can read it.
///
/// It starts by asking whether the interface is still there: one that is gone (the trip's
/// VPN, a USB adapter back in its drawer) has nothing to put back, and the cmdlets would fail
/// on it in the language of the machine, which no text match can rely on.
/// `None` for an entry whose index is not a number (see [`indice`]): there is nothing of
/// Windows' own to put back there.
fn restore_script(i: &InterfaceDns) -> Option<String> {
    let id = indice(i)?;
    let guard = format!(
        "$ErrorActionPreference='Stop'; if (-not (Get-NetIPInterface -InterfaceIndex {id} -ErrorAction SilentlyContinue)) {{ exit 0 }}; "
    );
    Some(match win_extra(i) {
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
                    "Set-DnsClientServerAddress -InterfaceIndex {id} -ServerAddresses ({}); ",
                    quoted(&manual)
                )
            };
            let fix6 = if x.manual6.is_empty() {
                format!(
                    "$d6 = Get-DnsClientServerAddress -InterfaceIndex {id} -AddressFamily IPv6 -ErrorAction SilentlyContinue; if ($d6 -and (@($d6.ServerAddresses) -contains '::1')) {{ netsh interface ipv6 set dnsservers name={id} source=dhcp | Out-Null }}; "
                )
            } else {
                String::new()
            };
            format!(
                "{guard}Set-DnsClientServerAddress -InterfaceIndex {id} -ResetServerAddresses; {set}{fix6}Clear-DnsClientCache"
            )
        }
        None if i.automatic => {
            format!("{guard}Set-DnsClientServerAddress -InterfaceIndex {id} -ResetServerAddresses")
        }
        None => format!(
            "{guard}Set-DnsClientServerAddress -InterfaceIndex {id} -ServerAddresses ({}); Clear-DnsClientCache",
            quoted(&i.servers)
        ),
    })
}

/// What PowerShell prints when the interface index matches nothing: the error identifier is
/// the same in every language, unlike the sentence above it.
pub(crate) fn interface_is_gone(stderr: &str) -> bool {
    stderr.contains("CmdletizationQuery_NotFound")
}

/// Every interface, to the end: the first failure is reported once all were tried (review of
/// 1 Oct 2026). An interface that is gone is nothing to put back, not a failure.
pub(crate) fn restore(backup: &Backup) -> Result<(), Error> {
    undo_each(&backup.interfaces, |i| {
        let Some(script) = restore_script(i) else {
            return Ok(());
        };
        match powershell(&script) {
            Ok(_) => Ok(()),
            Err(e) if interface_is_gone(&e.to_string()) => Ok(()),
            Err(e) => Err(e),
        }
    })
}

/// Parse one line of [`PRIMARY_SCRIPT`]: whether Windows sees Guardiana and nothing else.
fn line_is_guarded(line: &str) -> bool {
    let (v4, v6) = line.split_once('|').unwrap_or((line, ""));
    let v4: Vec<IpAddr> = parse_list(v4);
    let v6 = real_v6(&parse_list(v6));
    // An interface with IPv6 resolvers only (a network that hands out no IPv4 DNS) is
    // guarded when those are the guardian: until 1.0.5 it was not even listed, so it was
    // never re-pointed and never seen as still pointed (review of 8 Oct 2026).
    // And an interface with no real resolvers at all (a virtual switch, a VM adapter: only the
    // fec0: markers) is not Guardiana's business: the copy never takes it, so counting it as
    // «not guarded» re-applied the copy every minute (second pass, 8 Oct 2026).
    // The same for an interface with no IPv4 resolvers at all: the copy is taken from the IPv4
    // list, so it never holds that interface, and «not guarded» there re-applied the copy every
    // minute without ever pointing it (review of 9 Oct 2026). A network that hands out only IPv6
    // resolvers is a limit, written in WHAT_IT_DOES_NOT_DO, not something to retry for ever.
    if v4.is_empty() {
        return true;
    }
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

/// Whether any connected interface still asks Guardiana. The mirror of [`guardian_is_primary`]:
/// a undo that left one interface behind is not an undo that is done. `None` when PowerShell
/// could not say.
pub(crate) fn guardian_still_set() -> Option<bool> {
    let out = powershell(PRIMARY_SCRIPT).ok()?;
    let mut hay = false;
    let mut alguna = false;
    for l in out.lines().map(str::trim).filter(|l| !l.is_empty()) {
        hay = true;
        let (v4, v6) = l.split_once('|').unwrap_or((l, ""));
        alguna |= parse_list(v4).contains(&IpAddr::V4(Ipv4Addr::LOCALHOST))
            || parse_list(v6).contains(&IpAddr::V6(Ipv6Addr::LOCALHOST));
    }
    hay.then_some(alguna)
}

/// What Guardiana should forward to right now. Hand-typed resolvers stay the person's
/// choice wherever the machine goes. Automatic ones follow the network the machine is on
/// now: a laptop set up at home carries a router address (192.168.1.1) that does not exist
/// at the office, and with Guardiana as the only resolver that would be no internet at all.
pub(crate) fn upstreams_for(backup: &Backup) -> Vec<IpAddr> {
    let automatic: Vec<(u32, &InterfaceDns)> = backup
        .interfaces
        .iter()
        .filter(|i| win_extra(i).is_some_and(|x| x.manual4.is_empty()))
        .filter_map(|i| indice(i).map(|n| (n, i)))
        .collect();
    let mut dhcp: std::collections::HashMap<String, Vec<IpAddr>> = std::collections::HashMap::new();
    if !automatic.is_empty() {
        let script = automatic
            .iter()
            .map(|(id, _)| {
                format!(
                    "$g = (Get-NetAdapter -InterfaceIndex {id} -ErrorAction SilentlyContinue).InterfaceGuid; if ($g) {{ '{id}|' + [string](Get-ItemProperty -Path \"HKLM:\\SYSTEM\\CurrentControlSet\\Services\\Tcpip\\Parameters\\Interfaces\\$g\" -Name DhcpNameServer -ErrorAction SilentlyContinue).DhcpNameServer }}"
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

    /// Each of the three shapes of undo asks first whether the interface is still there, and
    /// then does what the backup says: automatic, hand-typed, or the pre-28-Sep shape.
    #[test]
    fn restore_script_checks_the_interface_exists_then_puts_it_back() {
        let dhcp = r#"{"index":3,"alias":"Wi-Fi","dhcp":true,"servers":["192.168.1.1"],"guid":true,"static4":"","static6":""}"#;
        let v = parse_snapshot(dhcp).unwrap_or_default();
        let s = restore_script(&v[0]).unwrap_or_default();
        assert!(s.starts_with("$ErrorActionPreference='Stop'; if (-not (Get-NetIPInterface -InterfaceIndex 3 -ErrorAction SilentlyContinue)) { exit 0 }; "));
        assert!(s.contains("-InterfaceIndex 3 -ResetServerAddresses"));
        assert!(s.contains("netsh interface ipv6 set dnsservers name=3 source=dhcp"));
        assert!(!s.contains("-ServerAddresses ("));

        let typed = r#"{"index":7,"alias":"Ethernet","dhcp":true,"servers":["1.1.1.1","1.0.0.1"],"guid":true,"static4":"1.1.1.1,1.0.0.1","static6":""}"#;
        let v = parse_snapshot(typed).unwrap_or_default();
        let s = restore_script(&v[0]).unwrap_or_default();
        assert!(s.contains("Get-NetIPInterface -InterfaceIndex 7"));
        assert!(s.contains("-InterfaceIndex 7 -ResetServerAddresses; Set-DnsClientServerAddress -InterfaceIndex 7 -ServerAddresses ('1.1.1.1','1.0.0.1')"));

        let old = r#"{"index":12,"alias":"Ethernet","dhcp":false,"servers":["8.8.8.8"]}"#;
        let v = parse_snapshot(old).unwrap_or_default();
        let s = restore_script(&v[0]).unwrap_or_default();
        assert!(s.contains("Get-NetIPInterface -InterfaceIndex 12"));
        assert!(s.ends_with("Set-DnsClientServerAddress -InterfaceIndex 12 -ServerAddresses ('8.8.8.8'); Clear-DnsClientCache"));
    }

    /// A copy edited by someone who is not an administrator cannot put a command into the line
    /// the service runs as SYSTEM: an index that is not a number is no interface, and is left
    /// alone (review of 10 Oct 2026, system serious 2).
    #[test]
    fn an_index_that_is_not_a_number_never_reaches_powershell() {
        let malo = InterfaceDns {
            id: "1; Start-Process calc".into(),
            name: "Wi-Fi".into(),
            servers: vec!["192.168.1.1"
                .parse()
                .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))],
            automatic: true,
            extra: Some(r#"{"manual4":[],"manual6":[]}"#.into()),
        };
        assert_eq!(indice(&malo), None);
        assert_eq!(restore_script(&malo), None);
        for id in ["", " ", "-1", "3 ", "12345678901", "0x10", "3;", "3\nexit"] {
            let i = InterfaceDns {
                id: id.into(),
                ..malo.clone()
            };
            assert!(restore_script(&i).is_none() || id.trim() == "3", "{id:?}");
        }
        let bueno = InterfaceDns {
            id: "7".into(),
            ..malo
        };
        assert_eq!(indice(&bueno), Some(7));
        assert!(restore_script(&bueno).is_some_and(|s| s.contains("-InterfaceIndex 7 ")));
    }

    /// The error record PowerShell prints for an index that matches nothing; the identifier on
    /// the last line is what is matched, because the first line comes out in the machine's
    /// language.
    #[test]
    fn a_missing_interface_is_told_by_its_error_id() {
        let gone = "Set-DnsClientServerAddress : No MSFT_DNSClientServerAddress objects found with property 'InterfaceIndex' equal to '99'.  Verify the value of the property and retry.\nAt line:1 char:1\n    + CategoryInfo          : ObjectNotFound: (99:UInt32) [Set-DnsClientServerAddress], CimJobException\n    + FullyQualifiedErrorId : CmdletizationQuery_NotFound_InterfaceIndex,Set-DnsClientServerAddress\n";
        assert!(interface_is_gone(gone));
        let denied = "Set-DnsClientServerAddress : Access is denied.\n    + CategoryInfo          : PermissionDenied: (MSFT_DNSClientServerAddress:root/StandardCimv2/...) [Set-DnsClientServerAddress], CimException\n    + FullyQualifiedErrorId : Windows System Error 5,Set-DnsClientServerAddress\n";
        assert!(!interface_is_gone(denied));
    }
}
