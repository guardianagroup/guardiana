//! Reading the neighbour table on each platform. Parsers are pure and tested
//! with real samples; only `read_neighbors` touches the system.

use std::net::IpAddr;
use std::process::Command;

use crate::normalize_mac;

/// One row of the neighbour table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Neighbor {
    /// IP address.
    pub ip: IpAddr,
    /// MAC, lowercase with colons.
    pub mac: String,
}

fn parse_ip(token: &str) -> Option<IpAddr> {
    token.split('%').next()?.parse().ok()
}

/// `arp -a` / `arp -an` output, Windows and macOS/BSD flavours:
/// `  192.168.1.1   a4-b1-c1-d2-e3-f4   dinámico`
/// `? (192.168.1.1) at a4:b1:c1:d2:e3:f4 on en0 ifscope [ethernet]`
#[must_use]
pub fn parse_arp_a(text: &str) -> Vec<Neighbor> {
    let mut out = Vec::new();
    for line in text.lines() {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        let mut ip = None;
        let mut mac = None;
        for t in &tokens {
            let bare = t.trim_matches(|c| c == '(' || c == ')');
            if ip.is_none() {
                if let Some(p) = parse_ip(bare) {
                    ip = Some(p);
                    continue;
                }
            }
            if mac.is_none() {
                if let Some(m) = normalize_mac(bare) {
                    mac = Some(m);
                }
            }
        }
        if let (Some(ip), Some(mac)) = (ip, mac) {
            out.push(Neighbor { ip, mac });
        }
    }
    out
}

/// `ip neigh show` output: `192.168.1.20 dev wlan0 lladdr aa:bb:cc:dd:ee:ff REACHABLE`.
/// Rows marked FAILED or INCOMPLETE have no lladdr and are skipped.
#[must_use]
pub fn parse_ip_neigh(text: &str) -> Vec<Neighbor> {
    text.lines()
        .filter_map(|line| {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            let ip = parse_ip(tokens.first()?)?;
            let pos = tokens.iter().position(|t| *t == "lladdr")?;
            let mac = normalize_mac(tokens.get(pos + 1)?)?;
            Some(Neighbor { ip, mac })
        })
        .collect()
}

/// `/proc/net/arp`: `IP address HW type Flags HW address Mask Device`, flags `0x2` = complete.
#[must_use]
pub fn parse_proc_net_arp(text: &str) -> Vec<Neighbor> {
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            let ip = parse_ip(tokens.first()?)?;
            let flags = tokens.get(2)?;
            if *flags == "0x0" {
                return None;
            }
            let mac = normalize_mac(tokens.get(3)?)?;
            Some(Neighbor { ip, mac })
        })
        .collect()
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The current neighbour table, empty if it cannot be read.
#[must_use]
pub fn read_neighbors() -> Vec<Neighbor> {
    #[cfg(target_os = "linux")]
    {
        if let Some(text) = run("ip", &["neigh", "show"]) {
            let v = parse_ip_neigh(&text);
            if !v.is_empty() {
                return v;
            }
        }
        if let Ok(text) = std::fs::read_to_string("/proc/net/arp") {
            return parse_proc_net_arp(&text);
        }
        Vec::new()
    }
    #[cfg(target_os = "windows")]
    {
        run("arp", &["-a"])
            .map(|t| parse_arp_a(&t))
            .unwrap_or_default()
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        run("arp", &["-an"])
            .map(|t| parse_arp_a(&t))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_and_mac_arp_formats() {
        let win = "\nInterfaz: 192.168.1.10 --- 0xc\n  Dirección de Internet   Dirección física      Tipo\n  192.168.1.1           a4-b1-c1-d2-e3-f4     dinámico\n  192.168.1.255         ff-ff-ff-ff-ff-ff     estático\n  224.0.0.22            01-00-5e-00-00-16     estático\n";
        let v = parse_arp_a(win);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].ip.to_string(), "192.168.1.1");
        assert_eq!(v[0].mac, "a4:b1:c1:d2:e3:f4");
        let mac = "? (192.168.1.1) at a4:b1:c1:d2:e3:f4 on en0 ifscope [ethernet]\n? (192.168.1.44) at (incomplete) on en0 ifscope [ethernet]\n";
        let v = parse_arp_a(mac);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].mac, "a4:b1:c1:d2:e3:f4");
    }

    #[test]
    fn linux_formats() {
        let neigh = "192.168.1.20 dev wlan0 lladdr aa:bb:cc:dd:ee:ff REACHABLE\n192.168.1.99 dev wlan0 FAILED\nfe80::1 dev wlan0 lladdr a4:b1:c1:d2:e3:f4 router STALE\n";
        let v = parse_ip_neigh(neigh);
        assert_eq!(v.len(), 2);
        assert_eq!(v[1].ip.to_string(), "fe80::1");
        let proc = "IP address       HW type     Flags       HW address            Mask     Device\n192.168.1.20     0x1         0x2         aa:bb:cc:dd:ee:ff     *        wlan0\n192.168.1.99     0x1         0x0         00:00:00:00:00:00     *        wlan0\n";
        let v = parse_proc_net_arp(proc);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].mac, "aa:bb:cc:dd:ee:ff");
    }
}
