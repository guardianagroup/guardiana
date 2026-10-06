//! macOS (in 1.0 since 21 Sep 2026): per enabled network service through
//! `networksetup`, which needs root. The daemon has it under launchd; the CLI
//! needs `sudo`. The live upstream comes from the DHCP lease (`ipconfig getpacket`).

use std::net::IpAddr;

use super::{parse_ip, run_checked, undo_each, Backup, Error, InterfaceDns, Method};

/// Enabled services from `networksetup -listallnetworkservices`: the first line
/// is a notice and a leading `*` marks a disabled service.
pub(crate) fn parse_services(text: &str) -> Vec<String> {
    text.lines()
        .skip(1)
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('*'))
        .map(ToOwned::to_owned)
        .collect()
}

/// Every service in the same listing, disabled ones included (without their `*`): the ones
/// that exist, which is what an undo needs to know. A disabled service still keeps its DNS
/// setting and can be put back; a service that is not listed at all is gone.
pub(crate) fn parse_all_services(text: &str) -> Vec<String> {
    text.lines()
        .skip(1)
        .map(|l| l.trim().trim_start_matches('*').trim())
        .filter(|l| !l.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

/// Servers from `networksetup -getdnsservers <service>`: one per line, or a
/// sentence ("There aren't any DNS Servers set on Wi-Fi.") when automatic.
pub(crate) fn parse_servers(text: &str) -> Vec<IpAddr> {
    text.lines().filter_map(|l| l.trim().parse().ok()).collect()
}

/// The first nameserver of the first resolver in `scutil --dns`, loopback included.
///
/// Split at the first colon only: an IPv6 address is full of them. Until 1.0.2 the line was cut
/// at every colon, a Mac whose first resolver was IPv6 gave `None` here, `guardian_is_primary`
/// could not tell, and the Mac was never pointed back at Guardiana after a stop or a restart
/// (review of 5 Oct 2026, serious 7).
pub(crate) fn first_nameserver(text: &str) -> Option<IpAddr> {
    text.lines()
        .map(str::trim)
        .find(|l| l.starts_with("nameserver[0]"))
        .and_then(|l| l.split_once(':'))
        .and_then(|(_, v)| parse_ip(v.trim()))
}

pub(crate) fn snapshot(now: i64) -> Result<Backup, Error> {
    let list = run_checked("networksetup", &["-listallnetworkservices"])?;
    // Services on automatic DNS report nothing; the resolvers actually in use
    // (from scutil) are kept so the panel can show the secondary. Restoring an
    // automatic service goes back to automatic, whatever those were.
    let active: Vec<IpAddr> = run_checked("scutil", &["--dns"])
        .map(|t| super::parse_scutil_dns(&t))
        .unwrap_or_default();
    let mut interfaces = Vec::new();
    for svc in parse_services(&list) {
        let out = run_checked("networksetup", &["-getdnsservers", &svc])?;
        let servers = parse_servers(&out);
        let automatic = servers.is_empty();
        interfaces.push(InterfaceDns {
            id: svc.clone(),
            name: svc,
            servers: if automatic { active.clone() } else { servers },
            automatic,
            extra: None,
        });
    }
    if interfaces.is_empty() {
        return Err(Error::NothingToChange);
    }
    Ok(Backup {
        taken_at: now,
        method: Method::MacNetworkSetup,
        interfaces,
        resolv_conf: None,
        resolv_link: None,
        made_dropin_dir: false,
    })
}

fn set_servers(service: &str, servers: &[String]) -> Result<(), Error> {
    let mut args = vec!["-setdnsservers", service];
    args.extend(servers.iter().map(String::as_str));
    run_checked("networksetup", &args)?;
    Ok(())
}

/// Guardiana alone on every service, as on Windows. Until 1.0.2 the previous servers stayed
/// behind it as a reserve, and Chrome and Edge, seeing a resolver they know how to encrypt
/// (1.1.1.1, 8.8.8.8), took every name to it over HTTPS, where Guardiana cannot see it (review
/// of 5 Oct 2026, serious 8). The reserve lives inside Guardiana now (it forwards there), and the
/// service gives the Mac its DNS back whenever it stops.
pub(crate) fn apply(backup: &Backup, guardian: IpAddr) -> Result<(), Error> {
    for i in &backup.interfaces {
        set_servers(&i.id, &[guardian.to_string()])?;
    }
    Ok(())
}

/// Every service, to the end: the first failure is reported once all were tried (review of
/// 1 Oct 2026). A service that no longer exists (a USB adapter that was removed from the
/// network settings) is nothing to put back, not a failure; when the list of services cannot
/// be read, every one is tried.
pub(crate) fn restore(backup: &Backup) -> Result<(), Error> {
    let present = run_checked("networksetup", &["-listallnetworkservices"])
        .ok()
        .map(|t| parse_all_services(&t));
    undo_each(&backup.interfaces, |i| {
        if present.as_ref().is_some_and(|p| !p.contains(&i.id)) {
            return Ok(());
        }
        if i.automatic {
            set_servers(&i.id, &["empty".to_owned()])
        } else {
            let list: Vec<String> = i.servers.iter().map(ToString::to_string).collect();
            set_servers(&i.id, &list)
        }
    })
}

/// `networksetup -listallhardwareports`: `Hardware Port: Wi-Fi` followed by `Device: en0`,
/// one block per port. The port name is the network service name `snapshot` recorded.
pub(crate) fn parse_hardware_ports(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut port: Option<String> = None;
    for line in text.lines().map(str::trim) {
        if let Some(p) = line.strip_prefix("Hardware Port:") {
            port = Some(p.trim().to_owned());
        } else if let Some(d) = line.strip_prefix("Device:") {
            if let Some(p) = port.take() {
                out.push((p, d.trim().to_owned()));
            }
        }
    }
    out
}

/// DNS servers in the DHCP lease of one device, from `ipconfig getpacket <dev>`:
/// `domain_name_server (ip_mult): {10.50.0.1, 1.1.1.1}` or `domain_name_server (ip): 10.50.0.1`.
/// Nothing when the device has no lease (Wi-Fi off, cable out).
pub(crate) fn parse_dhcp_dns(text: &str) -> Vec<IpAddr> {
    text.lines()
        .map(str::trim)
        .filter(|l| l.starts_with("domain_name_server"))
        .filter_map(|l| l.split_once(':').map(|(_, v)| v))
        .flat_map(|v| {
            v.trim()
                .trim_matches(|c| c == '{' || c == '}')
                .split(',')
                .filter_map(|ip| ip.trim().parse::<IpAddr>().ok())
                .collect::<Vec<_>>()
        })
        .filter(|ip| !ip.is_loopback())
        .collect()
}

/// What the resolver should forward to **right now**: for every service the backup recorded as
/// automatic, the DNS servers of its current DHCP lease; for a service set by hand, what it had.
///
/// This is the macOS twin of the Windows one. Without it, the servers saved when Guardiana took
/// over stay frozen for ever: a laptop installed at home and opened in a café forwards to the
/// home router, which is not there, and every name times out until macOS gives up and uses the
/// café's DNS by itself. That is exactly what happened to the owner's Mac on 1 Oct 2026.
/// `ipconfig getpacket` needs no root and is what the lease says, not what `scutil` shows (which
/// is Guardiana itself once it is first). Empty when no automatic service has a lease: the
/// caller then keeps the saved servers.
pub(crate) fn upstreams_for(backup: &Backup) -> Vec<IpAddr> {
    let ports = run_checked("networksetup", &["-listallhardwareports"])
        .map(|t| parse_hardware_ports(&t))
        .unwrap_or_default();
    let mut out: Vec<IpAddr> = Vec::new();
    let mut any_lease = false;
    for i in &backup.interfaces {
        let now: Vec<IpAddr> = if i.automatic {
            let dev = ports
                .iter()
                .find(|(p, _)| *p == i.name)
                .map(|(_, d)| d.as_str());
            let lease = dev
                .and_then(|d| run_checked("ipconfig", &["getpacket", d]).ok())
                .map(|t| parse_dhcp_dns(&t))
                .unwrap_or_default();
            if lease.is_empty() {
                continue;
            }
            any_lease = true;
            lease
        } else {
            i.servers
                .iter()
                .copied()
                .filter(|ip| !ip.is_loopback())
                .collect()
        };
        for ip in now {
            if !out.contains(&ip) {
                out.push(ip);
            }
        }
    }
    if any_lease {
        out
    } else {
        Vec::new()
    }
}

pub(crate) fn guardian_is_primary() -> Option<bool> {
    let text = run_checked("scutil", &["--dns"]).ok()?;
    let first = first_nameserver(&text)?;
    Some(first == IpAddr::V4(std::net::Ipv4Addr::LOCALHOST))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn services_skip_notice_and_disabled() {
        let text = "An asterisk (*) denotes that a network service is disabled.\nUSB 10/100 LAN\n*Bluetooth PAN\nWi-Fi\n";
        assert_eq!(parse_services(text), vec!["USB 10/100 LAN", "Wi-Fi"]);
        // For the undo, a disabled service still exists; one that is not listed is gone.
        assert_eq!(
            parse_all_services(text),
            vec!["USB 10/100 LAN", "Bluetooth PAN", "Wi-Fi"]
        );
    }

    #[test]
    fn servers_and_automatic() {
        assert!(parse_servers("There aren't any DNS Servers set on Wi-Fi.\n").is_empty());
        let v = parse_servers("192.168.1.1\n1.1.1.1\n");
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn hardware_ports_pair_name_and_device() {
        let text = "\nHardware Port: USB 10/100/1000 LAN\nDevice: en3\nEthernet Address: 00:e0:4c:68:09:c2\n\nHardware Port: Wi-Fi\nDevice: en0\nEthernet Address: 10:a1:da:3d:e5:85\n\nVLAN Configurations\n===================\n";
        let v = parse_hardware_ports(text);
        assert_eq!(
            v,
            vec![
                ("USB 10/100/1000 LAN".to_owned(), "en3".to_owned()),
                ("Wi-Fi".to_owned(), "en0".to_owned())
            ]
        );
    }

    #[test]
    fn dhcp_dns_both_forms_and_no_loopback() {
        let multi = "op = BOOTREPLY\nyiaddr = 10.50.1.29\ndomain_name_server (ip_mult): {10.50.0.1, 1.1.1.1}\nrouter (ip_mult): {10.50.0.1}\n";
        let v = parse_dhcp_dns(multi);
        assert_eq!(v.len(), 2);
        assert_eq!(
            v[0],
            "10.50.0.1"
                .parse::<IpAddr>()
                .ok()
                .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED))
        );
        let single = "domain_name_server (ip): 192.168.1.1\n";
        assert_eq!(parse_dhcp_dns(single).len(), 1);
        assert!(parse_dhcp_dns("domain_name_server (ip): 127.0.0.1\n").is_empty());
        assert!(parse_dhcp_dns("").is_empty());
    }

    /// Only on a real Mac with a DHCP lease: `cargo test -p guardiana-service -- --ignored`.
    /// Proves the live path end to end on the development Mac (1 Oct 2026, network 10.50.x).
    #[test]
    #[ignore]
    fn live_upstreams_come_from_the_dhcp_lease() {
        let backup = Backup {
            taken_at: 0,
            method: Method::MacNetworkSetup,
            interfaces: vec![InterfaceDns {
                id: "Wi-Fi".to_owned(),
                name: "Wi-Fi".to_owned(),
                servers: vec!["192.168.1.1"
                    .parse()
                    .ok()
                    .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED))],
                automatic: true,
                extra: None,
            }],
            resolv_conf: None,
            resolv_link: None,
            made_dropin_dir: false,
        };
        let live = upstreams_for(&backup);
        eprintln!("live upstreams: {live:?}");
        assert!(!live.is_empty(), "no DHCP lease found for Wi-Fi");
        assert!(live.iter().all(|ip| !ip.is_loopback()));
    }

    #[test]
    fn first_nameserver_reads_ipv6_whole() {
        let text = "resolver #1\n  nameserver[0] : 2800:e2:5c00::1\n  nameserver[1] : 127.0.0.1\n";
        assert_eq!(
            first_nameserver(text).map(|i| i.to_string()),
            Some("2800:e2:5c00::1".to_owned())
        );
        let zoned = "resolver #1\n  nameserver[0] : fe80::1%en0\n";
        assert_eq!(
            first_nameserver(zoned).map(|i| i.to_string()),
            Some("fe80::1".to_owned())
        );
    }

    #[test]
    fn first_nameserver_keeps_loopback() {
        let text = "DNS configuration\n\nresolver #1\n  nameserver[0] : 127.0.0.1\n  flags    : Request A records\n\nresolver #2\n  nameserver[0] : 192.168.1.1\n";
        assert_eq!(
            first_nameserver(text),
            Some(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST))
        );
    }
}
