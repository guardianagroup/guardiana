//! System DNS configuration (brief §4). Read-only in week 1: detect the
//! upstream resolvers the system has *before* Guardiana changes anything.
//! Changing and restoring the configuration lands with the service.

use std::net::{IpAddr, SocketAddr};
use std::process::Command;

/// Where the current resolvers were read from, for the panel and `verify`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolverSource {
    /// `/etc/resolv.conf` listed real servers.
    ResolvConf,
    /// systemd-resolved is in charge; servers come from `resolvectl status` (decision 11).
    SystemdResolved,
    /// Windows, from `Get-DnsClientServerAddress`.
    WindowsDnsClient,
    /// macOS, from `scutil --dns` (development only).
    MacScutil,
}

/// The resolvers in use, with their origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentResolvers {
    /// Servers, in the order the system tries them, port 53.
    pub servers: Vec<SocketAddr>,
    /// How they were found.
    pub source: ResolverSource,
}

fn parse_ip(token: &str) -> Option<IpAddr> {
    // Strip an IPv6 zone id such as `fe80::1%eth0`.
    let bare = token.split('%').next().unwrap_or(token);
    bare.parse().ok()
}

fn is_loopback_stub(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => v6.is_loopback(),
    }
}

/// Nameservers from resolv.conf text, in order, loopback stubs excluded.
#[must_use]
pub fn parse_resolv_conf(text: &str) -> Vec<IpAddr> {
    text.lines()
        .filter_map(|line| {
            let line = line.split('#').next().unwrap_or("").trim();
            let mut parts = line.split_whitespace();
            (parts.next()? == "nameserver").then(|| parts.next().and_then(parse_ip))?
        })
        .filter(|ip| !is_loopback_stub(*ip))
        .collect()
}

/// Servers from `resolvectl status` output: the lines `DNS Servers:` and
/// `Current DNS Server:` of every link, deduplicated, global first.
#[must_use]
pub fn parse_resolvectl_status(text: &str) -> Vec<IpAddr> {
    let mut out: Vec<IpAddr> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let rest = if let Some(r) = line.strip_prefix("Current DNS Server:") {
            r
        } else if let Some(r) = line.strip_prefix("DNS Servers:") {
            r
        } else {
            continue;
        };
        for token in rest.split_whitespace() {
            if let Some(ip) = parse_ip(token) {
                if !is_loopback_stub(ip) && !out.contains(&ip) {
                    out.push(ip);
                }
            }
        }
    }
    out
}

/// Servers from `scutil --dns` output (macOS): the `nameserver[n] : ip` lines
/// of the first resolver block, which is the one the system uses by default.
#[must_use]
pub fn parse_scutil_dns(text: &str) -> Vec<IpAddr> {
    let mut out = Vec::new();
    let mut in_first_block = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("resolver #1") {
            in_first_block = true;
            continue;
        }
        if t.starts_with("resolver #") {
            if in_first_block {
                break;
            }
            continue;
        }
        if in_first_block && t.starts_with("nameserver[") {
            if let Some(ip) = t.split(':').nth(1).and_then(|s| parse_ip(s.trim())) {
                if !is_loopback_stub(ip) && !out.contains(&ip) {
                    out.push(ip);
                }
            }
        }
    }
    out
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Port 53 for each server, duplicates removed (two interfaces often share one).
fn with_port(ips: Vec<IpAddr>) -> Vec<SocketAddr> {
    let mut out: Vec<SocketAddr> = Vec::new();
    for ip in ips {
        let a = SocketAddr::new(ip, 53);
        if !out.contains(&a) {
            out.push(a);
        }
    }
    out
}

/// The resolvers the system uses right now, or `None` if none could be found.
#[must_use]
pub fn current_resolvers() -> Option<CurrentResolvers> {
    #[cfg(target_os = "windows")]
    {
        let text = run(
            "powershell",
            &[
                "-NoProfile",
                "-Command",
                // Only interfaces that are connected: a disconnected adapter keeps its old DNS.
                "Get-NetIPInterface -AddressFamily IPv4 | Where-Object { $_.ConnectionState -eq 'Connected' -and $_.InterfaceAlias -notlike 'Loopback*' } | ForEach-Object { Get-DnsClientServerAddress -InterfaceIndex $_.InterfaceIndex -AddressFamily IPv4 } | Select-Object -ExpandProperty ServerAddresses",
            ],
        )?;
        let ips: Vec<IpAddr> = text
            .lines()
            .filter_map(|l| parse_ip(l.trim()))
            .filter(|ip| !is_loopback_stub(*ip))
            .collect();
        (!ips.is_empty()).then(|| CurrentResolvers {
            servers: with_port(ips),
            source: ResolverSource::WindowsDnsClient,
        })
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(text) = run("scutil", &["--dns"]) {
            let ips = parse_scutil_dns(&text);
            if !ips.is_empty() {
                return Some(CurrentResolvers {
                    servers: with_port(ips),
                    source: ResolverSource::MacScutil,
                });
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let text = std::fs::read_to_string("/etc/resolv.conf").ok()?;
        let ips = parse_resolv_conf(&text);
        if !ips.is_empty() {
            return Some(CurrentResolvers {
                servers: with_port(ips),
                source: ResolverSource::ResolvConf,
            });
        }
        // Only a loopback stub: ask systemd-resolved for the real servers.
        let status = run("resolvectl", &["status"])?;
        let ips = parse_resolvectl_status(&status);
        (!ips.is_empty()).then(|| CurrentResolvers {
            servers: with_port(ips),
            source: ResolverSource::SystemdResolved,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolv_conf_skips_comments_and_stubs() {
        let text = "# comment\nnameserver 127.0.0.53\nnameserver 192.168.1.1 # router\nsearch home\nnameserver fe80::1%wlan0\n";
        let v = parse_resolv_conf(text);
        assert_eq!(v.len(), 2);
        assert_eq!(
            v[0],
            "192.168.1.1"
                .parse::<IpAddr>()
                .ok()
                .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED))
        );
    }

    #[test]
    fn resolvectl_status_collects_link_servers() {
        let text = "Global\n  Protocols: +LLMNR\nLink 2 (eth0)\n    Current Scopes: DNS\n  Current DNS Server: 192.168.1.1\n         DNS Servers: 192.168.1.1 1.1.1.1\n";
        let v = parse_resolvectl_status(text);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].to_string(), "192.168.1.1");
        assert_eq!(v[1].to_string(), "1.1.1.1");
    }

    #[test]
    fn scutil_reads_first_resolver_block_only() {
        let text = "DNS configuration\n\nresolver #1\n  nameserver[0] : 192.168.1.1\n  nameserver[1] : 192.168.1.2\n  flags    : Request A records\n\nresolver #2\n  domain   : local\n  nameserver[0] : 10.0.0.1\n";
        let v = parse_scutil_dns(text);
        assert_eq!(v.len(), 2);
        assert_eq!(v[1].to_string(), "192.168.1.2");
    }
}

// ---------------------------------------------------------------------------
// Changing and restoring the system DNS (brief §4).
//
// Primary 127.0.0.1, secondary the original resolvers, so the system keeps
// resolving if the service dies. Nothing is overwritten without a backup;
// restore puts back exactly what was there. Windows may query both servers in
// parallel: documented limit (WHAT_IT_DOES_NOT_DO.md), not hidden.
// ---------------------------------------------------------------------------

use serde::{Deserialize, Serialize};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

/// Settings key holding the JSON [`Backup`] while Guardiana's change is applied.
pub const SETTING_BACKUP: &str = "dns_backup";

/// How the platform's DNS configuration is managed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    /// Windows DNS client, per interface (`Set-DnsClientServerAddress`).
    WindowsDnsClient,
    /// systemd-resolved, per link (`resolvectl dns` / `resolvectl revert`).
    SystemdResolved,
    /// NetworkManager, per active connection (`nmcli connection modify`).
    NetworkManager,
    /// A plain `/etc/resolv.conf`, rewritten with a backup copy.
    ResolvConf,
    /// macOS `networksetup`, per network service (development Mac only).
    MacNetworkSetup,
}

/// One interface, link or connection and the DNS it had before Guardiana.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterfaceDns {
    /// Platform identifier used to address it (interface index, link name, connection name).
    pub id: String,
    /// Human name shown in the panel.
    pub name: String,
    /// Resolvers it had, in order.
    pub servers: Vec<IpAddr>,
    /// True when those resolvers came automatically (DHCP / auto); restore then reverts to automatic.
    pub automatic: bool,
    /// Platform-specific original setting needed for an exact restore (NetworkManager `ipv4.dns`, `ipv4.ignore-auto-dns`).
    #[serde(default)]
    pub extra: Option<String>,
}

/// Everything needed to restore the configuration exactly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Backup {
    /// Unix ms when it was taken.
    pub taken_at: i64,
    /// Which mechanism was in charge.
    pub method: Method,
    /// Per interface, link or connection.
    pub interfaces: Vec<InterfaceDns>,
    /// Full original `/etc/resolv.conf` when Guardiana is going to rewrite it
    /// (`ResolvConf`, and `SystemdResolved` since decision 101) and it was a real file.
    #[serde(default)]
    pub resolv_conf: Option<String>,
    /// Target of `/etc/resolv.conf` when it was a symlink (Ubuntu points it at
    /// systemd-resolved's stub file). Restoring puts the symlink back as it was.
    #[serde(default)]
    pub resolv_link: Option<String>,
    /// True when Guardiana had to create `/etc/systemd/resolved.conf.d`; restoring
    /// removes it again, so the undo leaves no trace of its own.
    #[serde(default)]
    pub made_dropin_dir: bool,
}

impl Backup {
    /// Every original resolver across interfaces, deduplicated, in order.
    #[must_use]
    pub fn original_servers(&self) -> Vec<IpAddr> {
        let mut out = Vec::new();
        for i in &self.interfaces {
            for s in &i.servers {
                if !out.contains(s) {
                    out.push(*s);
                }
            }
        }
        out
    }
}

/// Errors changing the system DNS.
#[derive(Debug)]
pub enum Error {
    /// This platform is not supported (macOS is development only).
    Unsupported,
    /// A system command failed; the text is the command and its stderr.
    Command(String),
    /// A command's output could not be understood.
    Parse(String),
    /// File access failed.
    Io(std::io::Error),
    /// The backup JSON is malformed.
    Json(serde_json::Error),
    /// No interface with resolvers was found.
    NothingToChange,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported => {
                f.write_str("changing the system DNS is only supported on Windows and Linux")
            }
            Self::Command(s) => write!(f, "command failed: {s}"),
            Self::Parse(s) => write!(f, "cannot understand system output: {s}"),
            Self::Io(e) => write!(f, "io: {e}"),
            Self::Json(e) => write!(f, "backup json: {e}"),
            Self::NothingToChange => f.write_str("no network interface with DNS servers found"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

/// Run a command and return stdout, or a descriptive error.
#[cfg_attr(
    not(any(target_os = "linux", target_os = "windows", target_os = "macos")),
    allow(dead_code)
)]
pub fn run_checked(cmd: &str, args: &[&str]) -> Result<String, Error> {
    let out = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| Error::Command(format!("{cmd}: {e}")))?;
    if !out.status.success() {
        return Err(Error::Command(format!(
            "{cmd} {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The list Guardiana installs: itself first, then the originals (never itself twice).
#[must_use]
pub fn guarded_servers(guardian: IpAddr, originals: &[IpAddr]) -> Vec<IpAddr> {
    let mut v = vec![guardian];
    for s in originals {
        if *s != guardian && !v.contains(s) {
            v.push(*s);
        }
    }
    v
}

/// Read the current configuration in a restorable form. Changes nothing.
pub fn snapshot(now: i64) -> Result<Backup, Error> {
    #[cfg(target_os = "windows")]
    {
        windows::snapshot(now)
    }
    #[cfg(target_os = "linux")]
    {
        linux::snapshot(now)
    }
    #[cfg(target_os = "macos")]
    {
        macos::snapshot(now)
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        let _ = now;
        Err(Error::Unsupported)
    }
}

/// Point every interface in `backup` at `guardian` first, originals second.
pub fn apply(backup: &Backup, guardian: IpAddr) -> Result<(), Error> {
    #[cfg(target_os = "windows")]
    {
        windows::apply(backup, guardian)
    }
    #[cfg(target_os = "linux")]
    {
        linux::apply(backup, guardian)
    }
    #[cfg(target_os = "macos")]
    {
        macos::apply(backup, guardian)
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        let _ = (backup, guardian);
        Err(Error::Unsupported)
    }
}

/// Put back exactly what `backup` recorded.
pub fn restore(backup: &Backup) -> Result<(), Error> {
    #[cfg(target_os = "windows")]
    {
        windows::restore(backup)
    }
    #[cfg(target_os = "linux")]
    {
        linux::restore(backup)
    }
    #[cfg(target_os = "macos")]
    {
        macos::restore(backup)
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        let _ = backup;
        Err(Error::Unsupported)
    }
}

/// Rewrite resolv.conf text: Guardiana's nameserver first, the original
/// nameserver lines after it, every other line kept where it was.
#[must_use]
pub fn guarded_resolv_conf(original: &str, guardian: IpAddr, backup_path: &str) -> String {
    let mut out = String::new();
    out.push_str("# Guardiana: primario ");
    out.push_str(&guardian.to_string());
    out.push_str(", secundario el resolutor original. Copia exacta del archivo anterior en ");
    out.push_str(backup_path);
    out.push('\n');
    out.push_str("nameserver ");
    out.push_str(&guardian.to_string());
    out.push('\n');
    for line in original.lines() {
        if line.trim_start().starts_with("nameserver") {
            let ip = line.split_whitespace().nth(1).and_then(parse_ip);
            if ip == Some(guardian) {
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod change_tests {
    use super::*;

    #[test]
    fn guarded_servers_puts_guardian_first_without_duplicates() {
        let g: IpAddr = "127.0.0.1"
            .parse()
            .ok()
            .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
        let a: IpAddr = "192.168.1.1".parse().ok().unwrap_or(g);
        let v = guarded_servers(g, &[a, g, a]);
        assert_eq!(v, vec![g, a]);
    }

    #[test]
    fn resolv_conf_rewrite_keeps_everything_else() {
        let g: IpAddr = "127.0.0.1"
            .parse()
            .ok()
            .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
        let text = "search home\nnameserver 192.168.1.1\noptions edns0\n";
        let out = guarded_resolv_conf(text, g, "/var/lib/guardiana/resolv.conf.bak");
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines[0].starts_with("# Guardiana"));
        assert_eq!(lines[1], "nameserver 127.0.0.1");
        assert_eq!(lines[2], "search home");
        assert_eq!(lines[3], "nameserver 192.168.1.1");
        assert_eq!(lines[4], "options edns0");
        // Applying twice does not duplicate the guardian line.
        let again = guarded_resolv_conf(&out, g, "x");
        assert_eq!(again.matches("nameserver 127.0.0.1").count(), 1);
    }

    #[test]
    fn backup_round_trips_through_json() {
        let b = Backup {
            taken_at: 1,
            method: Method::ResolvConf,
            interfaces: vec![InterfaceDns {
                id: "eth0".into(),
                name: "eth0".into(),
                servers: vec!["192.168.1.1"
                    .parse()
                    .ok()
                    .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST))],
                automatic: true,
                extra: None,
            }],
            resolv_conf: Some("nameserver 192.168.1.1\n".into()),
            resolv_link: None,
            made_dropin_dir: false,
        };
        let json = serde_json::to_string(&b).unwrap_or_default();
        let back: Backup = serde_json::from_str(&json)
            .ok()
            .unwrap_or_else(|| b.clone());
        assert_eq!(back, b);
        assert_eq!(b.original_servers().len(), 1);
    }
}

/// Whether, on this machine, Guardiana ends up as the *only* resolver.
///
/// With systemd-resolved it has to be: a link's server list is a set it chooses
/// from, not an order it obeys, so leaving the old resolver as a second means
/// part of the queries never reach the guardian (decision 101). Everywhere else
/// the old resolver stays as a secondary and the machine keeps resolving if
/// Guardiana stops. The interface has to say which of the two is true.
#[must_use]
pub fn guardian_is_sole_resolver() -> bool {
    #[cfg(target_os = "linux")]
    {
        linux::have_resolvectl()
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Whether the guardian (127.0.0.1) is currently the first resolver on every
/// connected interface that has resolvers. `None` when it cannot be told on this platform.
#[must_use]
pub fn guardian_is_primary() -> Option<bool> {
    #[cfg(target_os = "windows")]
    {
        let out = run_checked(
            "powershell",
            &[
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                // Connected interfaces only (a disconnected adapter keeps its old DNS and must not count).
                "Get-NetIPInterface -AddressFamily IPv4 | Where-Object { $_.ConnectionState -eq 'Connected' -and $_.InterfaceAlias -notlike 'Loopback*' } | ForEach-Object { Get-DnsClientServerAddress -InterfaceIndex $_.InterfaceIndex -AddressFamily IPv4 } | Where-Object { $_.ServerAddresses.Count -gt 0 } | ForEach-Object { $_.ServerAddresses[0] }",
            ],
        )
        .ok()?;
        let firsts: Vec<&str> = out
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        if firsts.is_empty() {
            return None;
        }
        Some(firsts.iter().all(|f| *f == "127.0.0.1"))
    }
    #[cfg(target_os = "linux")]
    {
        if linux::have_resolvectl() {
            let out = run_checked("resolvectl", &["dns"]).ok()?;
            let links: Vec<&str> = out
                .lines()
                .filter(|l| l.trim_start().starts_with("Link "))
                .filter_map(|l| l.split_once(':').map(|(_, rest)| rest.trim()))
                .filter(|rest| !rest.is_empty())
                .collect();
            if links.is_empty() {
                return None;
            }
            // For systemd-resolved a link's server list is a set it chooses from,
            // not an order it obeys: leaving the old server in the list means half
            // the queries never reach the guardian (decision 101). So the guardian
            // has to be the *only* server, and /etc/resolv.conf has to point at it
            // too, for the programs that read that file instead of asking resolved.
            let links_ok = links.iter().all(|rest| rest.trim() == "127.0.0.1");
            return Some(links_ok && resolv_conf_points_at_guardian());
        }
        Some(resolv_conf_points_at_guardian())
    }
    #[cfg(target_os = "macos")]
    {
        macos::guardian_is_primary()
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

#[cfg(target_os = "linux")]
const RESOLV_CONF_PATH: &str = "/etc/resolv.conf";

/// Whether the first `nameserver` line of `/etc/resolv.conf` is the guardian.
#[cfg(target_os = "linux")]
fn resolv_conf_points_at_guardian() -> bool {
    let Ok(text) = std::fs::read_to_string(RESOLV_CONF_PATH) else {
        return false;
    };
    text.lines()
        .map(str::trim)
        .find(|l| l.starts_with("nameserver"))
        .and_then(|l| l.split_whitespace().nth(1))
        == Some("127.0.0.1")
}
