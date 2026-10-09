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
            // At the first colon only: an IPv6 address is full of them (serious 7, 5 Oct 2026).
            if let Some(ip) = t.split_once(':').and_then(|(_, s)| parse_ip(s.trim())) {
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
    fn scutil_keeps_ipv6_nameservers_whole() {
        let text =
            "resolver #1\n  nameserver[0] : 2800:e2:5c00::1\n  nameserver[1] : 192.168.1.1\n";
        let v = parse_scutil_dns(text);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].to_string(), "2800:e2:5c00::1");
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

// Also compiled for the tests of any unix: the development machine is a Mac, and the parsers
// of what resolvectl, nmcli and networkctl print are pure, so their tests can run here. The
// functions that touch the system are never called outside Linux.
#[cfg(any(target_os = "linux", all(test, unix)))]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
mod linux;
// The parsers of what scutil, networksetup and ipconfig print are pure, and their tests run on
// every machine: since the Mac was sold there is none to run them on (5 Oct 2026).
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod macos;
// Same for Windows: the PowerShell it builds and the error ids it reads are strings, and the
// tests about them run on the development Mac.
#[cfg(any(target_os = "windows", test))]
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
mod windows;

/// Settings key holding the JSON [`Backup`] while Guardiana's change is applied.
pub const SETTING_BACKUP: &str = "dns_backup";

/// Settings key holding the [`Backup`] of a change that was undone because the trial or the
/// subscription ended: the system DNS is back as it was, and this copy says the person had chosen
/// Guardiana, so it is applied again the moment the licence is back. Until 1.0.2 the copy was
/// simply cleared, and a customer who paid got the watching back but not the DNS: nothing
/// arrived (review of 5 Oct 2026, serious 4). Undoing by hand (`dns --restore`, the panel)
/// clears it too: then the person has said no.
pub const SETTING_BACKUP_APARCADA: &str = "dns_backup_aparcada";

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

impl Error {
    /// Whether this failed for want of administrator rights, told by what does not change with
    /// the system's language: the I/O error kind, PowerShell's error category and id (its message
    /// comes out in Spanish or Portuguese), and the words `nmcli`, `resolvectl` and `networksetup`
    /// use. Until 1.0.2 the person got the raw command and its stderr instead of "run it as
    /// administrator" (review of 5 Oct 2026, Windows medium).
    #[must_use]
    pub fn falta_administrador(&self) -> bool {
        match self {
            Self::Io(e) => e.kind() == std::io::ErrorKind::PermissionDenied,
            Self::Command(s) => [
                "PermissionDenied",
                "Windows System Error 5,",
                "Access is denied",
                "Permission denied",
                "Operation not permitted",
                "Not authorized",
                "Insufficient privileges",
                "requires root",
                "must be root",
            ]
            .iter()
            .any(|m| s.contains(m)),
            _ => false,
        }
    }
}

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

/// Undo on every interface, whatever happens to the others: the first error is kept and
/// returned once all of them were tried.
///
/// Until the review of 1 Oct 2026 each platform's undo loop stopped at the first interface
/// that failed -- a VPN or a USB adapter that no longer existed -- and left the rest pointing
/// at a guardian that was stepping aside. Applying stays strict (a change that cannot be made
/// whole is rolled back); undoing goes to the end, because every interface put back is one
/// the person can use again.
#[cfg_attr(
    not(any(target_os = "linux", target_os = "windows", target_os = "macos")),
    allow(dead_code)
)]
pub(crate) fn undo_each<F>(interfaces: &[InterfaceDns], mut undo: F) -> Result<(), Error>
where
    F: FnMut(&InterfaceDns) -> Result<(), Error>,
{
    let mut first: Option<Error> = None;
    for i in interfaces {
        if let Err(e) = undo(i) {
            if first.is_none() {
                first = Some(e);
            }
        }
    }
    first.map_or(Ok(()), Err)
}

/// Whether an address can be forwarded to: never the loopback (that is the guardian itself,
/// or another resolver on this machine) and never an IPv6 link-local address, which needs the
/// zone it was read with and has none once stored.
#[must_use]
pub fn usable_upstream(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => !v4.is_loopback(),
        IpAddr::V6(v6) => !v6.is_loopback() && !v6.is_unicast_link_local(),
    }
}

/// Whether `ip` belongs to this machine: the loopback, "any", or an address one of its
/// interfaces holds (the system lets a socket be bound to it only then).
///
/// A resolver on this machine is never an upstream for Guardiana: it is Guardiana itself, or
/// something Guardiana is supposed to stand in front of. The case that matters is Home Mode: the
/// router hands every device the computer's own address as DNS, the computer takes it too from
/// the same DHCP, and Guardiana forwarded every query of the house back to itself until each one
/// timed out (review of 5 Oct 2026, critical 3).
#[must_use]
pub fn is_this_machine(ip: IpAddr) -> bool {
    if ip.is_loopback() || ip.is_unspecified() {
        return true;
    }
    std::net::UdpSocket::bind(SocketAddr::new(ip, 0)).is_ok()
}

/// `ips` without the addresses of this machine and without repeats, order kept. Two adapters
/// often share the router as DNS; the list compared minute by minute has to be the same list
/// the pass was built with, or the resolver is rebuilt every minute.
#[must_use]
pub fn away_from_self(ips: Vec<IpAddr>) -> Vec<IpAddr> {
    let mut out: Vec<IpAddr> = Vec::new();
    for ip in ips {
        if !is_this_machine(ip) && !out.contains(&ip) {
            out.push(ip);
        }
    }
    out
}

/// The default gateway: the router, on a home network. Asked only when every resolver the
/// machine knows of turned out to be the machine itself; most home routers answer DNS, and it
/// is the network's own, not a public resolver nobody chose.
#[must_use]
pub fn default_gateway() -> Option<IpAddr> {
    #[cfg(target_os = "linux")]
    {
        parse_proc_net_route(&std::fs::read_to_string("/proc/net/route").ok()?)
    }
    #[cfg(target_os = "macos")]
    {
        parse_route_get(&run("route", &["-n", "get", "default"])?)
    }
    #[cfg(target_os = "windows")]
    {
        parse_next_hops(&run(
            "powershell",
            &[
                "-NoProfile",
                "-Command",
                "Get-NetRoute -AddressFamily IPv4 -DestinationPrefix '0.0.0.0/0' -PolicyStore ActiveStore -ErrorAction SilentlyContinue | ForEach-Object { $m = $_.RouteMetric + (Get-NetIPInterface -InterfaceIndex $_.InterfaceIndex -AddressFamily IPv4 -ErrorAction SilentlyContinue).InterfaceMetric; [pscustomobject]@{ M = $m; H = $_.NextHop } } | Sort-Object M | Select-Object -ExpandProperty H",
            ],
        )?)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

fn a_gateway(ip: IpAddr) -> bool {
    !ip.is_unspecified() && !ip.is_loopback() && !ip.is_multicast()
}

/// The gateway of the default route with the lowest metric, from `/proc/net/route` (Linux):
/// addresses there are hexadecimal, in the machine's byte order (little-endian on every PC).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn parse_proc_net_route(text: &str) -> Option<IpAddr> {
    let mut best: Option<(u32, IpAddr)> = None;
    for line in text.lines().skip(1) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let [_, dest, gw, flags, _, _, metric, ..] = f.as_slice() else {
            continue;
        };
        let up_with_gateway = u32::from_str_radix(flags, 16).is_ok_and(|v| v & 0x3 == 0x3);
        if *dest != "00000000" || !up_with_gateway {
            continue;
        }
        let Ok(raw) = u32::from_str_radix(gw, 16) else {
            continue;
        };
        let ip = IpAddr::V4(std::net::Ipv4Addr::from(raw.swap_bytes()));
        let metric = metric.parse::<u32>().unwrap_or(u32::MAX);
        if a_gateway(ip) && best.is_none_or(|(m, _)| metric < m) {
            best = Some((metric, ip));
        }
    }
    best.map(|(_, ip)| ip)
}

/// The `gateway:` line of `route -n get default` (macOS).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_route_get(text: &str) -> Option<IpAddr> {
    text.lines()
        .filter_map(|l| l.trim().strip_prefix("gateway:"))
        .filter_map(|v| parse_ip(v.trim()))
        .find(|ip| a_gateway(*ip))
}

/// The first usable next hop, one per line, as PowerShell lists them (Windows).
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn parse_next_hops(text: &str) -> Option<IpAddr> {
    text.lines()
        .filter_map(|l| parse_ip(l.trim()))
        .find(|ip| a_gateway(*ip))
}

/// `backup` plus the interfaces of `fresh` it does not know yet, when both were taken the same
/// way and that way works interface by interface (Windows, Mac, NetworkManager). `None` when
/// there is nothing new.
///
/// The copy taken when the DNS was pointed here listed the interfaces of that day. One that
/// appeared later (a dock, the office Wi-Fi, a new network service on the Mac) was never
/// pointed at Guardiana, and the watchdog, seeing it, re-applied the old copy every minute for
/// nothing while the panel said "pointed at Guardiana" (review of 5 Oct 2026, serious 10). New
/// ones are added to the copy first, so the undo puts them back too, and then applied.
#[must_use]
pub fn with_new_interfaces(backup: &Backup, fresh: &Backup) -> Option<Backup> {
    let per_interface = matches!(
        backup.method,
        Method::WindowsDnsClient | Method::MacNetworkSetup | Method::NetworkManager
    );
    if !per_interface || fresh.method != backup.method {
        return None;
    }
    let nuevas: Vec<InterfaceDns> = fresh
        .interfaces
        .iter()
        .filter(|f| !backup.interfaces.iter().any(|b| b.id == f.id))
        .cloned()
        .collect();
    if nuevas.is_empty() {
        return None;
    }
    let mut out = backup.clone();
    out.interfaces.extend(nuevas);
    Some(out)
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

/// The resolvers Guardiana should forward to right now, given what the machine had before.
///
/// Resolvers that came automatically follow the network the machine is on now (a laptop set
/// up at home must not keep asking the home router at the office); resolvers typed by hand
/// stay the person's choice. Windows reads the DHCP servers from the registry, macOS the DHCP
/// lease, and Linux (since the review of 1 Oct 2026) what NetworkManager, systemd-networkd or
/// systemd-resolved say the link has. When no live source answers, the ones recorded in the
/// backup.
#[must_use]
pub fn upstreams_for(backup: &Backup) -> Vec<IpAddr> {
    #[cfg(target_os = "windows")]
    {
        let v = windows::upstreams_for(backup);
        if !v.is_empty() {
            return v;
        }
    }
    #[cfg(target_os = "macos")]
    {
        let v = macos::upstreams_for(backup);
        if !v.is_empty() {
            return v;
        }
    }
    #[cfg(target_os = "linux")]
    {
        let v = linux::upstreams_for(backup);
        if !v.is_empty() {
            return v;
        }
    }
    // The same sieve as the live sources, so the two lists agree whenever they say the same
    // thing. A copy taken by 1.0.1 may hold `fe80::1` (a link-local read with its zone and
    // stored without it): kept here and dropped there, the heartbeat would see a different
    // list each time the live source went quiet for a minute and rebuild the resolver back
    // and forth (review of 1 Oct 2026, second round).
    backup
        .original_servers()
        .into_iter()
        .filter(|ip| usable_upstream(*ip))
        .collect()
}

/// Whether a name asked through this machine's own resolver really arrives at Guardiana.
///
/// It asks a fresh `<something>.prueba.guardiana.hogar`, which only Guardiana answers (with
/// the loopback) and nobody writes down. The settings can say "127.0.0.1 first" while the
/// queries go elsewhere: that is what happened on 27 Sep 2026, and only asking tells.
#[must_use]
pub fn system_reaches_guardian(timeout: std::time::Duration) -> bool {
    use std::net::ToSocketAddrs;
    let name = format!(
        "g{}x{}.{}",
        guardiana_core::time::now_ms(),
        std::process::id(),
        guardiana_core::SELF_CHECK_SUFFIX
    );
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let ips: Vec<IpAddr> = (name.as_str(), 80)
            .to_socket_addrs()
            .map(|it| it.map(|a| a.ip()).collect())
            .unwrap_or_default();
        let _ = tx.send(ips);
    });
    rx.recv_timeout(timeout)
        .is_ok_and(|ips| !ips.is_empty() && ips.iter().all(IpAddr::is_loopback))
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

    fn iface(id: &str) -> InterfaceDns {
        InterfaceDns {
            id: id.to_owned(),
            name: id.to_owned(),
            servers: Vec::new(),
            automatic: true,
            extra: None,
        }
    }

    /// The undo of 1.0.1 stopped at the first interface that failed (review of 1 Oct 2026):
    /// a VPN that was no longer there left the Wi-Fi pointing at a guardian that had gone.
    #[test]
    fn undo_tries_every_interface_and_reports_the_first_failure() {
        let ifaces = vec![iface("wlan0"), iface("tun0"), iface("eth0"), iface("wg0")];
        let mut tried: Vec<String> = Vec::new();
        let r = undo_each(&ifaces, |i| {
            tried.push(i.id.clone());
            if i.id == "tun0" || i.id == "wg0" {
                Err(Error::Command(format!("{} is gone", i.id)))
            } else {
                Ok(())
            }
        });
        assert_eq!(tried, vec!["wlan0", "tun0", "eth0", "wg0"]);
        match r {
            Err(Error::Command(s)) => assert_eq!(s, "tun0 is gone"),
            other => unreachable!("expected the first error, got {other:?}"),
        }
        assert!(undo_each(&ifaces, |_| Ok(())).is_ok());
        assert!(undo_each(&[], |_| Err(Error::NothingToChange)).is_ok());
    }

    #[test]
    fn this_machine_is_its_loopback_its_any_and_its_own_addresses_only() {
        let ip = |s: &str| {
            s.parse::<IpAddr>()
                .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST))
        };
        assert!(is_this_machine(ip("127.0.0.1")));
        assert!(is_this_machine(ip("127.0.0.53")));
        assert!(is_this_machine(ip("0.0.0.0")));
        assert!(is_this_machine(ip("::1")));
        // Documentation addresses: never assigned to a real interface.
        assert!(!is_this_machine(ip("192.0.2.1")));
        assert!(!is_this_machine(ip("2001:db8::1")));
        assert_eq!(
            away_from_self(vec![
                ip("192.0.2.1"),
                ip("127.0.0.1"),
                ip("198.51.100.7"),
                ip("192.0.2.1")
            ]),
            vec![ip("192.0.2.1"), ip("198.51.100.7")]
        );
    }

    #[test]
    fn the_gateway_is_read_from_each_system() {
        // Linux: two default routes (Wi-Fi metric 600, cable 100) and a local network.
        let proc_route =
            "Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\t\tMTU\tWindow\tIRTT\n\
wlan0\t00000000\t0101A8C0\t0003\t0\t0\t600\t00000000\t0\t0\t0\n\
eth0\t00000000\tFE00000A\t0003\t0\t0\t100\t00000000\t0\t0\t0\n\
eth0\t0000000A\t00000000\t0001\t0\t0\t100\t00FFFFFF\t0\t0\t0\n";
        assert_eq!(
            parse_proc_net_route(proc_route).map(|i| i.to_string()),
            Some("10.0.0.254".to_owned())
        );
        assert_eq!(parse_proc_net_route("Iface\tDestination\n"), None);
        let mac = "   route to: default\ndestination: default\n       mask: default\n    gateway: 192.168.1.1\n  interface: en0\n";
        assert_eq!(
            parse_route_get(mac).map(|i| i.to_string()),
            Some("192.168.1.1".to_owned())
        );
        assert_eq!(
            parse_next_hops("0.0.0.0\r\n192.168.0.1\r\n").map(|i| i.to_string()),
            Some("192.168.0.1".to_owned())
        );
        assert_eq!(parse_next_hops(""), None);
    }

    #[test]
    fn wanting_administrator_rights_is_told_in_any_language() {
        let ps_es = Error::Command("powershell ...: Set-DnsClientServerAddress : Acceso denegado.\n    + CategoryInfo          : PermissionDenied: (MSFT_DNSClientServerAddress...) [Set-DnsClientServerAddress], CimException\n    + FullyQualifiedErrorId : Windows System Error 5,Set-DnsClientServerAddress".into());
        assert!(ps_es.falta_administrador());
        assert!(
            Error::Io(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
                .falta_administrador()
        );
        assert!(Error::Command(
            "nmcli connection modify x: Error: Not authorized to control networking.".into()
        )
        .falta_administrador());
        assert!(
            !Error::Command("nmcli: Error: unknown connection 'x'.".into()).falta_administrador()
        );
        assert!(!Error::NothingToChange.falta_administrador());
    }

    #[test]
    fn a_new_interface_joins_the_copy_and_nothing_else_changes() {
        let copia = |method: Method, ids: &[&str]| Backup {
            taken_at: 1,
            method,
            interfaces: ids.iter().map(|i| iface(i)).collect(),
            resolv_conf: None,
            resolv_link: None,
            made_dropin_dir: false,
        };
        let antes = copia(Method::WindowsDnsClient, &["12", "3"]);
        let ahora = copia(Method::WindowsDnsClient, &["3", "21"]);
        let junta = with_new_interfaces(&antes, &ahora).unwrap_or_else(|| antes.clone());
        let ids: Vec<&str> = junta.interfaces.iter().map(|i| i.id.as_str()).collect();
        // What was there stays (the undo still needs "12" even if it is gone), the new one is
        // added, and the copy keeps its date.
        assert_eq!(ids, vec!["12", "3", "21"]);
        assert_eq!(junta.taken_at, 1);
        assert!(with_new_interfaces(&antes, &copia(Method::WindowsDnsClient, &["3"])).is_none());
        // A machine-wide method has nothing to add, and two methods are never mixed.
        assert!(with_new_interfaces(
            &copia(Method::ResolvConf, &["eth0"]),
            &copia(Method::ResolvConf, &["wlan0"])
        )
        .is_none());
        assert!(with_new_interfaces(&antes, &copia(Method::MacNetworkSetup, &["Wi-Fi"])).is_none());
    }

    #[test]
    fn an_upstream_is_never_the_loopback_nor_a_link_local_v6() {
        let ok = |s: &str| s.parse::<IpAddr>().is_ok_and(usable_upstream);
        assert!(ok("192.168.1.1"));
        assert!(ok("2001:4860:4860::8888"));
        assert!(!ok("127.0.0.1"));
        assert!(!ok("::1"));
        // What `fe80::1%wlan0` becomes once the zone is stripped: unreachable as it stands.
        assert!(!ok("fe80::1"));
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
    // Windows too since 28 Sep 2026: with the old resolver left behind as a reserve, Edge saw
    // a resolver it knows how to encrypt (1.1.1.1) and took every name to Cloudflare over HTTPS,
    // where Guardiana cannot see it. The reserve now lives inside Guardiana (it forwards there)
    // and the service gives the machine its old DNS back whenever it stops.
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        true
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        false
    }
}

/// Which sentence tells the truth on this machine about the resolver that was there before:
/// gone from the system's list (Linux with systemd-resolved, Windows) or kept as a reserve.
#[must_use]
pub fn sole_or_secondary_key() -> &'static str {
    if cfg!(target_os = "windows") {
        "dns.solo_guardiana_windows"
    } else if cfg!(target_os = "macos") {
        "dns.solo_guardiana_mac"
    } else if guardian_is_sole_resolver() {
        "dns.solo_guardiana"
    } else {
        "dns.reserva_secundario"
    }
}

/// Whether the guardian (127.0.0.1) is currently the first resolver on every
/// connected interface that has resolvers. `None` when it cannot be told on this platform.
#[must_use]
pub fn guardian_is_primary() -> Option<bool> {
    #[cfg(target_os = "windows")]
    {
        windows::guardian_is_primary()
    }
    #[cfg(target_os = "linux")]
    {
        if linux::have_resolvectl() {
            let out = run_checked("resolvectl", &["dns"]).ok()?;
            // Only the links of the copy are Guardiana's business: a VPN tunnel or a virtual
            // bridge with a resolver of its own made this `false` every minute, and the
            // watchdog re-applied a copy that was already applied for as long as the VPN was
            // up (review of 8 Oct 2026). With no copy to read, every link counts.
            let copia: Vec<String> = linux::backup_link_ids().unwrap_or_default();
            let links: Vec<&str> = out
                .lines()
                .filter(|l| l.trim_start().starts_with("Link "))
                .filter(|l| {
                    copia.is_empty()
                        || l.split_whitespace()
                            .nth(2)
                            .map(|n| n.trim_matches(|c| c == '(' || c == ')'))
                            .is_some_and(|n| copia.iter().any(|c| c == n))
                })
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

/// Whether anything of the machine's DNS still points at Guardiana. The opposite reading of
/// [`guardian_is_primary`], which is true only when every interface does: an undo that put back
/// one interface and failed on another is *not* done, and a stood-down program that thought it
/// was left the other one asking a loopback nobody listened on (review of 5 Oct 2026, second
/// pass). `Some(false)` means nothing is left; `None` when it cannot be told.
#[must_use]
pub fn guardian_still_set() -> Option<bool> {
    #[cfg(target_os = "windows")]
    {
        windows::guardian_still_set()
    }
    #[cfg(target_os = "linux")]
    {
        if linux::dropin_present() || resolv_conf_points_at_guardian() {
            return Some(true);
        }
        if linux::have_resolvectl() {
            let out = run_checked("resolvectl", &["dns"]).ok()?;
            let alguna = out
                .lines()
                .filter(|l| l.trim_start().starts_with("Link "))
                .filter_map(|l| l.split_once(':').map(|(_, rest)| rest))
                .any(|rest| rest.split_whitespace().any(|a| a == "127.0.0.1"));
            return Some(alguna);
        }
        Some(false)
    }
    #[cfg(target_os = "macos")]
    {
        macos::guardian_still_set()
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
