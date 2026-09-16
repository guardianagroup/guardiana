//! macOS, development Mac only (DECISIONES #55; macOS stays out of 1.0): per
//! enabled network service through `networksetup`, which needs root. The
//! daemon has it under launchd; the CLI needs `sudo`.

use std::net::IpAddr;

use super::{guarded_servers, run_checked, Backup, Error, InterfaceDns, Method};

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

/// Servers from `networksetup -getdnsservers <service>`: one per line, or a
/// sentence ("There aren't any DNS Servers set on Wi-Fi.") when automatic.
pub(crate) fn parse_servers(text: &str) -> Vec<IpAddr> {
    text.lines().filter_map(|l| l.trim().parse().ok()).collect()
}

/// The first nameserver of the first resolver in `scutil --dns`, loopback included.
pub(crate) fn first_nameserver(text: &str) -> Option<IpAddr> {
    text.lines()
        .map(str::trim)
        .find(|l| l.starts_with("nameserver[0]"))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|s| s.trim().parse().ok())
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

pub(crate) fn apply(backup: &Backup, guardian: IpAddr) -> Result<(), Error> {
    for i in &backup.interfaces {
        let list: Vec<String> = guarded_servers(guardian, &i.servers)
            .iter()
            .map(ToString::to_string)
            .collect();
        set_servers(&i.id, &list)?;
    }
    Ok(())
}

pub(crate) fn restore(backup: &Backup) -> Result<(), Error> {
    for i in &backup.interfaces {
        if i.automatic {
            set_servers(&i.id, &["empty".to_owned()])?;
        } else {
            let list: Vec<String> = i.servers.iter().map(ToString::to_string).collect();
            set_servers(&i.id, &list)?;
        }
    }
    Ok(())
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
    }

    #[test]
    fn servers_and_automatic() {
        assert!(parse_servers("There aren't any DNS Servers set on Wi-Fi.\n").is_empty());
        let v = parse_servers("192.168.1.1\n1.1.1.1\n");
        assert_eq!(v.len(), 2);
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
