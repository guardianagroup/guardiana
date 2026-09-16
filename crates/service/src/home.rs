//! Home Mode (brief §7, HOGAR.md): the only case in which Guardiana opens
//! ports beyond loopback, and only on the private LAN interface. Off by
//! default, turned on by the user, recorded in settings, closed when turned
//! off. This module holds the checks and the firewall rule; binding the LAN
//! sockets happens where the resolver and panel start.

use std::net::{IpAddr, Ipv4Addr};

#[cfg(any(target_os = "windows", target_os = "linux"))]
use super::sysdns::run_checked;

/// Settings key: `1` while Home Mode is on.
pub const SETTING_HOME_MODE: &str = "home_mode";
/// Settings key: the LAN IPv4 Home Mode was activated with.
pub const SETTING_HOME_IP: &str = "home_ip";
/// Settings key: Unix ms when Home Mode was last activated.
pub const SETTING_HOME_SINCE: &str = "home_since";
/// Name of the Windows firewall rule (brief §7).
pub const FIREWALL_RULE_NAME: &str = "Guardiana Modo Hogar";
/// Panel port opened on the LAN.
pub const PANEL_PORT: u16 = 7443;
/// Plain web port opened on the LAN so `http://comprobar.guardiana.hogar`
/// works without typing a port (brief §4, HOGAR.md §2).
pub const CHECKER_PORT: u16 = 80;

/// Whether the LAN address is handed out by DHCP (then it may change and
/// phones pointed at it would lose DNS). `None` when it cannot be told.
#[must_use]
pub fn ip_is_dynamic(ip: Ipv4Addr) -> Option<bool> {
    #[cfg(target_os = "windows")]
    {
        let script = format!(
            "$ErrorActionPreference='Stop'; (Get-NetIPAddress -AddressFamily IPv4 -IPAddress '{ip}').PrefixOrigin"
        );
        let out = run_checked(
            "powershell",
            &["-NoProfile", "-NonInteractive", "-Command", &script],
        )
        .ok()?;
        let origin = out.trim().to_ascii_lowercase();
        if origin.is_empty() {
            return None;
        }
        Some(origin == "dhcp")
    }
    #[cfg(target_os = "linux")]
    {
        let out = run_checked("ip", &["-4", "-o", "addr", "show"]).ok()?;
        let needle = format!("inet {ip}/");
        let line = out.lines().find(|l| l.contains(&needle))?;
        Some(line.contains(" dynamic"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        let _ = ip;
        None
    }
}

/// Minutes before the system suspends itself on mains and on battery
/// (0 = never), or `None` when it cannot be read. A sleeping guardian leaves
/// the phones that point only at it without DNS, so Home Mode warns about it.
/// Windows only: `powercfg /query` for the "sleep after" setting. Its output
/// is localised, so only the last two hex values (AC, then DC) are used.
pub fn sleep_after_minutes() -> Option<(u32, u32)> {
    #[cfg(target_os = "windows")]
    {
        let out = run_checked(
            "powercfg",
            &["/query", "SCHEME_CURRENT", "SUB_SLEEP", "STANDBYIDLE"],
        )
        .ok()?;
        let values: Vec<u32> = out
            .lines()
            .filter_map(|l| {
                let hex = l.rsplit(':').next()?.trim().strip_prefix("0x")?;
                u32::from_str_radix(hex, 16).ok()
            })
            .collect();
        let n = values.len();
        if n < 2 {
            return None;
        }
        let ac = values.get(n - 2)?;
        let dc = values.get(n - 1)?;
        Some((ac / 60, dc / 60))
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

/// Errors of the firewall step.
#[derive(Debug)]
pub enum FirewallError {
    /// The command failed (text includes stderr).
    Command(String),
    /// No automatic firewall step on this platform; instructions are documented instead.
    Manual,
}

impl std::fmt::Display for FirewallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Command(s) => write!(f, "firewall: {s}"),
            Self::Manual => f.write_str("firewall rule must be added by hand on this platform"),
        }
    }
}

impl std::error::Error for FirewallError {}

/// Allow DNS (53 UDP/TCP) and the panel port, inbound, private profile only.
pub fn firewall_allow() -> Result<(), FirewallError> {
    #[cfg(target_os = "windows")]
    {
        // Idempotent: turning Home Mode on twice must not leave the rules
        // twice (seen on the test Windows with eight rules of the same name).
        let _ = firewall_remove();
        let rules = [("UDP", "53"), ("TCP", "53"), ("TCP", "7443"), ("TCP", "80")];
        for (proto, port) in rules {
            run_checked(
                "netsh",
                &[
                    "advfirewall",
                    "firewall",
                    "add",
                    "rule",
                    &format!("name={FIREWALL_RULE_NAME}"),
                    "dir=in",
                    "action=allow",
                    "profile=private",
                    &format!("protocol={proto}"),
                    &format!("localport={port}"),
                ],
            )
            .map_err(|e| FirewallError::Command(e.to_string()))?;
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(FirewallError::Manual)
    }
}

/// Remove every rule named [`FIREWALL_RULE_NAME`].
pub fn firewall_remove() -> Result<(), FirewallError> {
    #[cfg(target_os = "windows")]
    {
        run_checked(
            "netsh",
            &[
                "advfirewall",
                "firewall",
                "delete",
                "rule",
                &format!("name={FIREWALL_RULE_NAME}"),
            ],
        )
        .map(|_| ())
        .map_err(|e| FirewallError::Command(e.to_string()))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(FirewallError::Manual)
    }
}

/// Addresses the resolver and panel must listen on in Home Mode.
#[must_use]
pub fn lan_listen_addrs(lan: Ipv4Addr) -> (std::net::SocketAddr, std::net::SocketAddr) {
    (
        std::net::SocketAddr::new(IpAddr::V4(lan), 53),
        std::net::SocketAddr::new(IpAddr::V4(lan), PANEL_PORT),
    )
}

/// The plain-web address for the checker page in Home Mode.
#[must_use]
pub fn lan_checker_addr(lan: Ipv4Addr) -> std::net::SocketAddr {
    std::net::SocketAddr::new(IpAddr::V4(lan), CHECKER_PORT)
}

/// The URL a phone opens from the QR (brief §7).
#[must_use]
pub fn home_url(lan: Ipv4Addr) -> String {
    format!("http://{lan}:{PANEL_PORT}/hogar")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_and_addrs() {
        let ip = Ipv4Addr::new(192, 168, 1, 10);
        assert_eq!(home_url(ip), "http://192.168.1.10:7443/hogar");
        let (dns, panel) = lan_listen_addrs(ip);
        assert_eq!(dns.port(), 53);
        assert_eq!(panel.port(), 7443);
    }
}
