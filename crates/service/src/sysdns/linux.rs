//! Linux: systemd-resolved (`resolvectl`), NetworkManager (`nmcli`) or a
//! plain `/etc/resolv.conf`, detected in that order (brief §4).

use std::net::IpAddr;
use std::path::{Path, PathBuf};

use super::{
    guarded_resolv_conf, guarded_servers, parse_ip, run_checked, undo_each, usable_upstream,
    Backup, Error, InterfaceDns, Method,
};

const RESOLV_CONF: &str = "/etc/resolv.conf";
const STUB_RESOLV: &str = "/run/systemd/resolve/stub-resolv.conf";
const RESOLVED_DROPIN_DIR: &str = "/etc/systemd/resolved.conf.d";
const RESOLVED_DROPIN: &str = "/etc/systemd/resolved.conf.d/guardiana.conf";
/// Where the kernel lists the network links that exist right now, one directory each.
const SYS_CLASS_NET: &str = "/sys/class/net";

fn have(cmd: &str) -> bool {
    std::process::Command::new(cmd)
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// A command that ended badly, with its exit code kept apart: `nmcli` says *what* went wrong
/// with the code, in every language, while its text changes with the locale.
struct Failed {
    code: Option<i32>,
    stderr: String,
}

impl Failed {
    fn into_error(self, cmd: &str, args: &[&str]) -> Error {
        Error::Command(format!("{cmd} {}: {}", args.join(" "), self.stderr.trim()))
    }
}

fn run_coded(cmd: &str, args: &[&str]) -> Result<String, Failed> {
    let out = std::process::Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| Failed {
            code: None,
            stderr: e.to_string(),
        })?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(Failed {
            code: out.status.code(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}

/// Whether the kernel still has a link of this name. A link that is gone (the trip's VPN, a
/// USB adapter back in its drawer) has nothing to put back: systemd-resolved forgets a link's
/// settings with the link.
fn link_exists(name: &str) -> bool {
    Path::new(SYS_CLASS_NET).join(name).exists()
}

/// What `resolvectl` says when the link it was given does not exist: `Failed to resolve
/// interface "tun0": No such device` on current versions, `Unknown interface 'tun0'` on older
/// ones. The first words are systemd's own and never translated; `No such device` is the
/// kernel's and may be, so the kernel is also asked directly (`link_exists`).
pub(crate) fn link_is_gone(stderr: &str) -> bool {
    stderr.contains("Failed to resolve interface")
        || stderr.contains("Unknown interface")
        || stderr.contains("No such device")
}

/// Whether `nmcli` failed because the connection profile no longer exists. Exit code 10 is
/// "connection, device or access point does not exist" in every language (nmcli(1)); the text
/// is there for an nmcli that printed it without that code.
pub(crate) fn nm_connection_gone(code: Option<i32>, stderr: &str) -> bool {
    code == Some(10) || stderr.to_ascii_lowercase().contains("unknown connection")
}

/// systemd-resolved is in charge. The stub file is the usual sign, but Guardiana
/// switches the stub off while it guards (see `dropin_text`), so `resolvectl`
/// answering is what decides afterwards.
pub(crate) fn have_resolvectl() -> bool {
    Path::new(STUB_RESOLV).exists() && have("resolvectl")
        || run_checked("resolvectl", &["--no-pager", "status"]).is_ok()
}

fn detect() -> Method {
    if have_resolvectl() {
        return Method::SystemdResolved;
    }
    if have("nmcli")
        && run_checked("nmcli", &["-t", "-f", "RUNNING", "general"])
            .is_ok_and(|s| s.trim() == "running")
    {
        return Method::NetworkManager;
    }
    Method::ResolvConf
}

/// Where the resolv.conf backup copy is written.
fn resolv_backup_path() -> PathBuf {
    guardiana_core::paths::data_dir().join("resolv.conf.guardiana-backup")
}

/// The links the copy covers, one name per line, next to the resolv.conf copy: what the
/// watchdog's check reads to leave the other links (a VPN, a bridge) out of the question.
fn links_path() -> PathBuf {
    guardiana_core::paths::data_dir().join("resolved-links.guardiana-backup")
}

/// The link names written by the last apply, if the copy is in place.
pub(crate) fn backup_link_ids() -> Option<Vec<String>> {
    let text = std::fs::read_to_string(links_path()).ok()?;
    Some(
        text.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect(),
    )
}

// ----- systemd-resolved -----------------------------------------------------

/// `resolvectl dns` prints `Global: ...` and `Link N (name): servers...`.
pub(crate) fn parse_resolvectl_dns(text: &str) -> Vec<InterfaceDns> {
    text.lines()
        .filter_map(|line| {
            let (head, rest) = line.split_once(':')?;
            let head = head.trim();
            let name = head
                .strip_prefix("Link ")?
                .split_once(" (")?
                .1
                .trim_end_matches(')')
                .to_owned();
            let servers: Vec<IpAddr> = rest.split_whitespace().filter_map(parse_ip).collect();
            (!servers.is_empty()).then_some(InterfaceDns {
                id: name.clone(),
                name,
                servers,
                automatic: true,
                extra: None,
            })
        })
        .collect()
}

/// The drop-in Guardiana writes while it guards. Every line earns its place:
/// `DNS` and `Domains=~.` make the guardian the resolver for every name;
/// `FallbackDNS=` empty stops systemd-resolved from quietly asking somewhere
/// else when a query fails; `DNSStubListener=no` takes 127.0.0.53 out of the
/// path so nothing resolves behind the guardian's back; `Cache=no` leaves the
/// caching to the guardian, which is the part that keeps the record.
fn dropin_text(guardian: IpAddr) -> String {
    let mut out = String::new();
    out.push_str("# Guardiana lo escribió al poner este equipo a vigilar.\n");
    out.push_str("# Para deshacerlo: guardiana dns --restore\n");
    out.push_str("# (o borra este archivo y ejecuta: systemctl restart systemd-resolved).\n");
    out.push_str("[Resolve]\n");
    out.push_str("DNS=");
    out.push_str(&guardian.to_string());
    out.push('\n');
    out.push_str("FallbackDNS=\n");
    out.push_str("Domains=~.\n");
    out.push_str("DNSStubListener=no\n");
    out.push_str("Cache=no\n");
    out
}

/// What `/etc/resolv.conf` is right now: `(file text, symlink target)`. At most
/// one is `Some`; both are `None` when the file is missing.
fn resolv_state() -> (Option<String>, Option<String>) {
    match std::fs::symlink_metadata(RESOLV_CONF) {
        Ok(m) if m.file_type().is_symlink() => (
            None,
            std::fs::read_link(RESOLV_CONF)
                .ok()
                .map(|t| t.display().to_string()),
        ),
        Ok(_) => (std::fs::read_to_string(RESOLV_CONF).ok(), None),
        Err(_) => (None, None),
    }
}

/// `resolvectl status` prints `+DNSOverTLS` only for the strict setting
/// (`opportunistic` prints its own name, `no` prints `-DNSOverTLS`), and
/// `+DNSSEC` only for strict validation. Either one would break every lookup:
/// the guardian speaks plain DNS on loopback. Better to refuse than to leave
/// the machine without names.
pub(crate) fn strict_encryption(status: &str) -> Option<&'static str> {
    for line in status.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("Protocols:") else {
            continue;
        };
        if rest.contains("+DNSOverTLS") {
            return Some("DNS cifrado (DNSOverTLS=yes)");
        }
        if rest.contains("+DNSSEC") {
            return Some("DNSSEC estricto (DNSSEC=yes)");
        }
    }
    None
}

/// Write `text` to `path` only when it differs, so the minute-by-minute watchdog
/// does not restart systemd-resolved over and over. `true` when it wrote.
fn write_if_different(path: &str, text: &str) -> Result<bool, Error> {
    if std::fs::read_to_string(path).is_ok_and(|old| old == text) {
        return Ok(false);
    }
    std::fs::write(path, text)?;
    Ok(true)
}

/// Point `/etc/resolv.conf` straight at the guardian, replacing the symlink to
/// systemd-resolved's stub with a real file. Programs that read this file
/// instead of asking resolved (`getent`, most language runtimes) then reach the
/// guardian too.
fn resolv_conf_to_guardian(guardian: IpAddr, backup: &Backup) -> Result<(), Error> {
    let original = backup.resolv_conf.as_deref().unwrap_or("");
    let backup_path = resolv_backup_path();
    if let Some(parent) = backup_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Never overwrite without a backup copy on disk (brief §4).
    let note = match backup.resolv_link.as_deref() {
        Some(target) => format!("enlace a {target}"),
        None => backup_path.display().to_string(),
    };
    std::fs::write(&backup_path, original)?;
    let links: Vec<&str> = backup.interfaces.iter().map(|i| i.id.as_str()).collect();
    std::fs::write(links_path(), links.join("\n") + "\n")?;
    let mut text = String::new();
    text.push_str("# Guardiana: el guardián es el único resolutor de este equipo.\n");
    text.push_str("# Lo que había antes: ");
    text.push_str(&note);
    text.push('\n');
    text.push_str("# Para devolverlo: guardiana dns --restore\n");
    text.push_str("nameserver ");
    text.push_str(&guardian.to_string());
    text.push('\n');
    // The search list and the domain of the original file stay: the programs that read this
    // file instead of asking resolved (Go without cgo, musl, scripts) lost `ssh nas` without
    // them (review of 8 Oct 2026). Only the resolver lines are replaced.
    for line in original.lines() {
        let l = line.trim();
        if l.starts_with("search ") || l.starts_with("domain ") {
            text.push_str(l);
            text.push('\n');
        }
    }
    text.push_str("options edns0 trust-ad\n");
    if std::fs::read_to_string(RESOLV_CONF).is_ok_and(|old| old == text)
        && !std::fs::symlink_metadata(RESOLV_CONF).is_ok_and(|m| m.file_type().is_symlink())
    {
        return Ok(());
    }
    // A symlink has to go before the real file can take its place.
    match std::fs::remove_file(RESOLV_CONF) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(Error::Io(e)),
    }
    std::fs::write(RESOLV_CONF, text)?;
    Ok(())
}

/// Put `/etc/resolv.conf` back exactly as it was: the same symlink, or the same
/// file, or nothing when there was nothing.
fn resolv_conf_restore(backup: &Backup) -> Result<(), Error> {
    // Report *this* failure rather than the "File exists" that would follow from
    // trying to put the symlink back on top of a file still there: when the
    // service cannot write in /etc, that is what has to reach the user.
    match std::fs::remove_file(RESOLV_CONF) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(Error::Io(e)),
    }
    if let Some(target) = backup.resolv_link.as_deref() {
        std::os::unix::fs::symlink(target, RESOLV_CONF)?;
    } else if let Some(text) = backup.resolv_conf.as_deref() {
        std::fs::write(RESOLV_CONF, text)?;
    }
    let _ = std::fs::remove_file(resolv_backup_path());
    let _ = std::fs::remove_file(links_path());
    Ok(())
}

// ----- NetworkManager -------------------------------------------------------

/// `nmcli -t -f NAME,DEVICE connection show --active` → `(name, device)`.
pub(crate) fn parse_nmcli_active(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| {
            // Names may contain `\:`; split on the last unescaped colon.
            let mut idx = None;
            let bytes = l.as_bytes();
            for (i, b) in bytes.iter().enumerate() {
                if *b == b':' && (i == 0 || bytes[i - 1] != b'\\') {
                    idx = Some(i);
                }
            }
            let i = idx?;
            let name = l[..i].replace("\\:", ":");
            let dev = l[i + 1..].to_owned();
            (!dev.is_empty() && dev != "--").then_some((name, dev))
        })
        .collect()
}

/// Servers from `nmcli -g IP4.DNS connection show NAME` (`|`, `,` or space separated).
pub(crate) fn parse_nmcli_servers(text: &str) -> Vec<IpAddr> {
    text.split(['|', ',', ' ', '\n'])
        .filter_map(parse_ip)
        .collect()
}

fn nm_snapshot() -> Result<Vec<InterfaceDns>, Error> {
    let active = run_checked(
        "nmcli",
        &["-t", "-f", "NAME,DEVICE", "connection", "show", "--active"],
    )?;
    let mut out = Vec::new();
    for (name, dev) in parse_nmcli_active(&active) {
        if dev == "lo" {
            continue;
        }
        let effective = run_checked("nmcli", &["-g", "IP4.DNS", "connection", "show", &name])?;
        let servers = parse_nmcli_servers(&effective);
        if servers.is_empty() {
            continue;
        }
        let original = run_checked(
            "nmcli",
            &[
                "-g",
                "ipv4.dns,ipv4.ignore-auto-dns",
                "connection",
                "show",
                &name,
            ],
        )?;
        let mut lines = original.lines();
        let dns_setting = lines.next().unwrap_or("").trim().to_owned();
        let ignore_auto = lines.next().unwrap_or("no").trim().to_owned();
        out.push(InterfaceDns {
            id: name.clone(),
            name: format!("{name} ({dev})"),
            servers,
            automatic: ignore_auto != "yes",
            extra: Some(format!("{dns_setting}\n{ignore_auto}")),
        });
    }
    Ok(out)
}

/// Why putting one NetworkManager profile back did not happen.
enum NmUndo {
    /// The profile no longer exists: nothing to put back.
    Gone,
    /// It exists and the change failed.
    Failed(Error),
}

/// `nmcli connection modify` alone: the profile's DNS settings, without activating it.
fn nm_set(name: &str, dns: &str, ignore_auto: &str) -> Result<(), NmUndo> {
    let args = [
        "connection",
        "modify",
        name,
        "ipv4.dns",
        dns,
        "ipv4.ignore-auto-dns",
        ignore_auto,
    ];
    match run_coded("nmcli", &args) {
        Ok(_) => Ok(()),
        Err(f) if nm_connection_gone(f.code, &f.stderr) => Err(NmUndo::Gone),
        Err(f) => Err(NmUndo::Failed(f.into_error("nmcli", &args))),
    }
}

/// Names of the connections NetworkManager has up right now (`lo` excluded).
fn nm_active_names() -> Option<Vec<String>> {
    run_checked(
        "nmcli",
        &["-t", "-f", "NAME,DEVICE", "connection", "show", "--active"],
    )
    .ok()
    .map(|t| {
        parse_nmcli_active(&t)
            .into_iter()
            .filter(|(_, dev)| dev != "lo")
            .map(|(name, _)| name)
            .collect()
    })
}

/// `domain_name_servers` from the DHCP options NetworkManager keeps for a connection
/// (`nmcli -g DHCP4.OPTION connection show NAME`): one line of ` | `-separated
/// `key = value` pairs, or one `DHCP4.OPTION[n]: key = value` line each in the long form.
/// These are what the network handed out, whatever the profile was later told to use.
pub(crate) fn parse_dhcp4_dns(text: &str) -> Vec<IpAddr> {
    let mut out = Vec::new();
    for item in text.split(['|', '\n']) {
        let Some((key, value)) = item.split_once('=') else {
            continue;
        };
        if !key.trim_end().ends_with("domain_name_servers") {
            continue;
        }
        for ip in value.split([' ', ',']).filter_map(parse_ip) {
            if !out.contains(&ip) {
                out.push(ip);
            }
        }
    }
    out
}

/// Servers from `networkctl status LINK`: the `DNS:` line, whose further servers come one per
/// indented line below it until the next `Label:` line. What systemd-networkd got from DHCP or
/// its `.network` file, which `resolvectl dns` overrides do not change.
pub(crate) fn parse_networkctl_dns(text: &str) -> Vec<IpAddr> {
    let mut out = Vec::new();
    let mut in_dns = false;
    for line in text.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("DNS:") {
            in_dns = true;
            if let Some(ip) = rest.split_whitespace().next().and_then(parse_ip) {
                out.push(ip);
            }
            continue;
        }
        if in_dns {
            match line.split_whitespace().next().and_then(parse_ip) {
                Some(ip) => out.push(ip),
                None => break,
            }
        }
    }
    out
}

/// What NetworkManager has for a device right now (DHCP plus whatever the profile sets).
/// Empty when NetworkManager is not there or does not manage the device.
fn nm_device_dns(dev: &str) -> Vec<IpAddr> {
    run_checked("nmcli", &["-g", "IP4.DNS", "device", "show", dev])
        .map(|t| parse_nmcli_servers(&t))
        .unwrap_or_default()
}

/// What systemd-networkd has for a link right now. Empty when networkd is not in charge.
fn networkd_link_dns(link: &str) -> Vec<IpAddr> {
    run_checked("networkctl", &["status", "--no-pager", link])
        .map(|t| parse_networkctl_dns(&t))
        .unwrap_or_default()
}

/// Add the usable ones, once each, keeping the order.
fn add_usable(out: &mut Vec<IpAddr>, ips: impl IntoIterator<Item = IpAddr>) {
    for ip in ips {
        if usable_upstream(ip) && !out.contains(&ip) {
            out.push(ip);
        }
    }
}

/// What the resolver should forward to **right now**: the Linux twin of the Windows and macOS
/// ones (review of 1 Oct 2026, finding 6). Without it the servers saved when Guardiana took
/// over stayed frozen: a laptop set up at home and opened at the office forwarded to the home
/// router, which was not there, and nothing on the machine resolved. Empty when no live source
/// answers; the caller then keeps the saved servers.
///
/// Only the links and profiles of the backup are asked, in both modes: those are the ones
/// `apply` put the guardian on, so they are the only ones whose queries reach it. A profile
/// made later (the trip's VPN, the office Ethernet, a phone sharing its data) was never put
/// under the guardian, and its resolver does not become the guardian's upstream without
/// anyone asking for it.
///
/// With systemd-resolved, `apply` puts the guardian alone on every link of the backup, so the
/// link itself reads back as 127.0.0.1 while guarded. The live servers are asked of whoever
/// configures the link underneath -- NetworkManager, then systemd-networkd -- which `apply`
/// never touches; failing both, `resolvectl dns` itself, which tells the truth whenever the
/// link is not guarded (service start, stepped aside) and nothing usable when it is. With
/// NetworkManager alone, `apply` rewrites the profile's DNS, so what the profile says now is
/// the guardian itself; the DHCP options of its lease, which `apply` never rewrites, are what
/// the network gives today, and what the person typed by hand is in the backup
/// (`nm_profile_upstreams`). With a plain resolv.conf there is no live source: Guardiana
/// rewrote the only one.
pub(crate) fn upstreams_for(backup: &Backup) -> Vec<IpAddr> {
    match backup.method {
        Method::SystemdResolved => resolved_live(backup),
        Method::NetworkManager => nm_live(backup),
        Method::ResolvConf | Method::WindowsDnsClient | Method::MacNetworkSetup => Vec::new(),
    }
}

fn resolved_live(backup: &Backup) -> Vec<IpAddr> {
    let links = run_checked("resolvectl", &["dns"])
        .map(|t| parse_resolvectl_dns(&t))
        .unwrap_or_default();
    let mut out = Vec::new();
    for i in &backup.interfaces {
        let mut live = nm_device_dns(&i.id);
        if live.is_empty() {
            live = networkd_link_dns(&i.id);
        }
        if live.is_empty() {
            live = links
                .iter()
                .find(|l| l.id == i.id)
                .map(|l| l.servers.clone())
                .unwrap_or_default();
        }
        add_usable(&mut out, live);
    }
    out
}

/// The servers the person typed into a NetworkManager profile before Guardiana: the first line
/// of what `nm_snapshot` kept in `extra` (`ipv4.dns`), which `apply` then overwrote.
fn nm_typed_servers(i: &InterfaceDns) -> Vec<IpAddr> {
    i.extra
        .as_deref()
        .and_then(|e| e.lines().next())
        .map(parse_nmcli_servers)
        .unwrap_or_default()
}

/// What one NetworkManager profile of the backup gives the guardian right now, given the
/// servers of the profile's DHCP lease today. Resolvers typed by hand stay the person's
/// choice, as on Windows and macOS (review of 1 Oct 2026, second round): a profile with
/// `ipv4.ignore-auto-dns=yes` used only what the person typed, whatever its lease offered, so
/// the lease is not looked at; a profile that used both puts the typed ones first, as
/// NetworkManager itself does, and today's lease after them; a profile that used the network's
/// alone gives today's lease, and nothing when it has none right now -- never the servers of
/// the day the copy was taken, which is what finding 6 was about. The loopback and link-local
/// addresses are never upstreams.
pub(crate) fn nm_profile_upstreams(i: &InterfaceDns, lease: &[IpAddr]) -> Vec<IpAddr> {
    let mut out = Vec::new();
    if !i.automatic {
        add_usable(&mut out, i.servers.iter().copied());
        return out;
    }
    add_usable(&mut out, nm_typed_servers(i));
    add_usable(&mut out, lease.iter().copied());
    out
}

/// `domain_name_servers` of the lease NetworkManager holds for a connection right now; empty
/// when it is not up, has a static address, or nmcli could not say.
fn nm_lease_dns(name: &str) -> Vec<IpAddr> {
    run_checked("nmcli", &["-g", "DHCP4.OPTION", "connection", "show", name])
        .map(|t| parse_dhcp4_dns(&t))
        .unwrap_or_default()
}

fn nm_live(backup: &Backup) -> Vec<IpAddr> {
    let Some(active) = nm_active_names() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for i in &backup.interfaces {
        // A profile of the backup that is not up now (the home Wi-Fi, seen from the office)
        // resolves nothing today, so it gives nothing.
        if !active.contains(&i.id) {
            continue;
        }
        let lease = if i.automatic {
            nm_lease_dns(&i.id)
        } else {
            Vec::new()
        };
        add_usable(&mut out, nm_profile_upstreams(i, &lease));
    }
    out
}

// ----- plain resolv.conf ----------------------------------------------------

fn resolv_snapshot() -> Result<(Vec<InterfaceDns>, String), Error> {
    let text = std::fs::read_to_string(RESOLV_CONF)?;
    let servers = super::parse_resolv_conf(&text);
    let iface = InterfaceDns {
        id: RESOLV_CONF.to_owned(),
        name: RESOLV_CONF.to_owned(),
        servers,
        automatic: false,
        extra: None,
    };
    Ok((vec![iface], text))
}

// ----- public entry points --------------------------------------------------

pub(crate) fn snapshot(now: i64) -> Result<Backup, Error> {
    let method = detect();
    let mut resolv_link = None;
    let (interfaces, resolv_conf) = match method {
        Method::SystemdResolved => {
            let text = run_checked("resolvectl", &["dns"])?;
            // Guardiana rewrites /etc/resolv.conf here too (decision 101), so its
            // exact shape — file or symlink — is part of the undo.
            let (conf, link) = resolv_state();
            resolv_link = link;
            (parse_resolvectl_dns(&text), conf)
        }
        Method::NetworkManager => (nm_snapshot()?, None),
        Method::ResolvConf => {
            let (i, text) = resolv_snapshot()?;
            (i, Some(text))
        }
        Method::WindowsDnsClient | Method::MacNetworkSetup => return Err(Error::Unsupported),
    };
    if interfaces.iter().all(|i| i.servers.is_empty()) {
        return Err(Error::NothingToChange);
    }
    Ok(Backup {
        taken_at: now,
        method,
        interfaces,
        resolv_conf,
        resolv_link,
        made_dropin_dir: method == Method::SystemdResolved
            && !Path::new(RESOLVED_DROPIN_DIR).exists(),
    })
}

pub(crate) fn apply(backup: &Backup, guardian: IpAddr) -> Result<(), Error> {
    match backup.method {
        Method::SystemdResolved => {
            // Leaving the old server alongside the guardian does not work here:
            // for systemd-resolved a link's list is a set it picks from, not an
            // order it obeys, and it kept choosing the router — so half the
            // queries never reached the guardian (decision 101). The guardian has
            // to be the only resolver, by three doors at once.
            if let Some(what) = run_checked("resolvectl", &["--no-pager", "status"])
                .ok()
                .as_deref()
                .and_then(strict_encryption)
            {
                return Err(Error::Command(format!(
                    "systemd-resolved tiene {what}; el guardián habla DNS normal por el bucle local. \
                     Apágalo antes de poner a vigilar este equipo, o el equipo se quedaría sin nombres."
                )));
            }
            let g = guardian.to_string();
            // Door one, first, so there is never an instant without a resolver:
            // the stub disappears in the next step, and this file is already the
            // guardian by then.
            resolv_conf_to_guardian(guardian, backup)?;
            // Door two: systemd-resolved itself, for the programs that ask it
            // over D-Bus instead of reading the file.
            std::fs::create_dir_all(RESOLVED_DROPIN_DIR)?;
            if write_if_different(RESOLVED_DROPIN, &dropin_text(guardian))? {
                run_checked("systemctl", &["restart", "systemd-resolved"])?;
            }
            // Door three, after the restart because per-link settings live in
            // memory and the restart clears them: the link itself, with nothing
            // but the guardian on it and every domain routed through it.
            // A link in the copy that is gone now (a VPN tunnel, a dock) is skipped, as in the
            // undo: until 1.0.5 it stopped the loop and the links after it, the Wi-Fi included,
            // were left pointing at the router (review of 8 Oct 2026).
            for i in &backup.interfaces {
                if !link_exists(&i.id) {
                    continue;
                }
                for args in [
                    ["dns", i.id.as_str(), g.as_str()],
                    ["domain", i.id.as_str(), "~."],
                ] {
                    match run_checked("resolvectl", &args) {
                        Ok(_) => {}
                        Err(e) if link_is_gone(&e.to_string()) => break,
                        Err(e) => return Err(e),
                    }
                }
            }
            Ok(())
        }
        Method::NetworkManager => {
            // Every profile in the copy gets the guardian, but only the ones up now are
            // re-activated, as in the undo. Until 1.0.2 each one was brought up: away from home
            // the watchdog ran `nmcli connection up` on the home Wi-Fi every minute, which fails
            // at best and at worst drops the network the laptop is on to look for it (review of
            // 5 Oct 2026, serious 11). A profile that no longer exists is skipped.
            let active = nm_active_names();
            for i in &backup.interfaces {
                let list = guarded_servers(guardian, &i.servers)
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                match nm_set(&i.id, &list, "yes") {
                    Ok(()) => {}
                    Err(NmUndo::Gone) => continue,
                    Err(NmUndo::Failed(e)) => return Err(e),
                }
                if active.as_ref().is_none_or(|a| a.contains(&i.id)) {
                    run_checked("nmcli", &["connection", "up", &i.id])?;
                }
            }
            Ok(())
        }
        Method::ResolvConf => {
            let original = backup.resolv_conf.as_deref().unwrap_or("");
            let backup_path = resolv_backup_path();
            if let Some(parent) = backup_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            // Never overwrite without a backup copy on disk (brief §4).
            std::fs::write(&backup_path, original)?;
            let text = guarded_resolv_conf(original, guardian, &backup_path.display().to_string());
            std::fs::write(RESOLV_CONF, text)?;
            Ok(())
        }
        Method::WindowsDnsClient | Method::MacNetworkSetup => Err(Error::Unsupported),
    }
}

/// Put back what `backup` recorded, to the end: every link or profile is tried whatever
/// happened to the others, and every later step runs whatever happened to the links. The
/// first error is the one returned (review of 1 Oct 2026: a `?` on the first link stopped the
/// whole undo, and a VPN that was no longer there left the machine pointing at nothing).
/// Whether Guardiana's hold on resolved (the drop-in file) is still in place.
pub(crate) fn dropin_present() -> bool {
    Path::new(RESOLVED_DROPIN).exists()
}

pub(crate) fn restore(backup: &Backup) -> Result<(), Error> {
    match backup.method {
        Method::SystemdResolved => {
            let links = undo_each(&backup.interfaces, |i| {
                if !link_exists(&i.id) {
                    return Ok(());
                }
                match run_checked("resolvectl", &["revert", &i.id]) {
                    Ok(_) => Ok(()),
                    // Gone between the two looks, or resolved says so itself.
                    Err(e) if !link_exists(&i.id) || link_is_gone(&e.to_string()) => Ok(()),
                    Err(e) => Err(e),
                }
            });
            // The drop-in is the guardian's hold on resolved itself: if it cannot be removed,
            // the undo did not happen, whatever the links say.
            let dropin = match std::fs::remove_file(RESOLVED_DROPIN) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(Error::Io(e)),
            };
            if backup.made_dropin_dir {
                let _ = std::fs::remove_dir(RESOLVED_DROPIN_DIR);
            }
            // El orden importa, y al revés de como parecía. Reiniciar primero deja
            // a systemd-resolved leyendo el /etc/resolv.conf que todavía es el
            // nuestro («nameserver 127.0.0.1»), y cuando resolv.conf no es su
            // enlace al stub, resolved toma esos servidores como DNS **global**:
            // medido en la VM el 16 sep 2026, después de un restaurado que decía
            // «exactamente como estaba» quedaba `Global: 127.0.0.1` hasta el
            // siguiente reinicio del servicio. Así que primero se devuelve
            // resolv.conf y después se reinicia, que además es el orden que deja
            // a resolved releyéndolo todo de cero. Los enlaces ya están revertidos
            // arriba, así que durante esos milisegundos el equipo resuelve por el
            // stub, no se queda sin DNS.
            let conf = resolv_conf_restore(backup);
            // Restarted even when a step above failed: it is what makes resolved forget the
            // guardian-only settings it holds in memory, so that what the person fixes by hand
            // afterwards takes effect.
            let restart = run_checked("systemctl", &["restart", "systemd-resolved"]).map(|_| ());
            links.and(dropin).and(conf).and(restart)
        }
        Method::NetworkManager => {
            // The profiles are put back whether or not they are up now; only the ones up now
            // are re-activated so the change reaches resolv.conf. `connection up` on the home
            // Wi-Fi from the office fails, and on a VPN profile it would connect the VPN. A
            // profile that no longer exists has nothing to put back.
            let active = nm_active_names();
            undo_each(&backup.interfaces, |i| {
                let extra = i.extra.as_deref().unwrap_or("\nno");
                let (dns, ignore) = extra.split_once('\n').unwrap_or(("", "no"));
                match nm_set(&i.id, dns, ignore) {
                    Ok(()) => {}
                    Err(NmUndo::Gone) => return Ok(()),
                    Err(NmUndo::Failed(e)) => return Err(e),
                }
                if active.as_ref().is_none_or(|a| a.contains(&i.id)) {
                    run_checked("nmcli", &["connection", "up", &i.id])?;
                }
                Ok(())
            })
        }
        Method::ResolvConf => {
            let original = backup
                .resolv_conf
                .as_deref()
                .ok_or_else(|| Error::Parse("backup has no resolv.conf text".into()))?;
            std::fs::write(RESOLV_CONF, original)?;
            let _ = std::fs::remove_file(resolv_backup_path());
            Ok(())
        }
        Method::WindowsDnsClient | Method::MacNetworkSetup => Err(Error::Unsupported),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolvectl_dns_lists_links_with_servers() {
        let text = "Global:\nLink 2 (enp0s3): 192.168.1.1 fe80::1%enp0s3\nLink 3 (wlan0):\n";
        let v = parse_resolvectl_dns(text);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].id, "enp0s3");
        assert_eq!(v[0].servers.len(), 2);
    }

    #[test]
    fn nmcli_active_handles_escaped_colons() {
        let text = "Wired connection 1:enp0s3\nCasa\\: Wi-Fi:wlan0\nlo:lo\nvpn:--\n";
        let v = parse_nmcli_active(text);
        assert_eq!(v.len(), 3);
        assert_eq!(v[1].0, "Casa: Wi-Fi");
        assert_eq!(v[1].1, "wlan0");
    }

    #[test]
    fn strict_encryption_only_trips_on_the_strict_settings() {
        let off = "  Protocols: -LLMNR -mDNS -DNSOverTLS DNSSEC=no/unsupported\n";
        assert_eq!(strict_encryption(off), None);
        let opportunistic = "  Protocols: -LLMNR DNSOverTLS=opportunistic DNSSEC=no/unsupported\n";
        assert_eq!(strict_encryption(opportunistic), None);
        let dot = "  Protocols: -LLMNR -mDNS +DNSOverTLS DNSSEC=no/unsupported\n";
        assert!(strict_encryption(dot).is_some_and(|s| s.contains("cifrado")));
        let dnssec = "  Protocols: -LLMNR -mDNS -DNSOverTLS +DNSSEC\n";
        assert!(strict_encryption(dnssec).is_some_and(|s| s.contains("DNSSEC")));
    }

    #[test]
    fn dropin_leaves_no_way_round_the_guardian() {
        let g: IpAddr = IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);
        let text = dropin_text(g);
        assert!(text.contains("DNS=127.0.0.1"));
        // An empty FallbackDNS is the point: otherwise a failed query goes to
        // whatever systemd was built with, behind the guardian's back.
        assert!(text.contains("FallbackDNS=\n"));
        assert!(text.contains("Domains=~.\n"));
        assert!(text.contains("DNSStubListener=no"));
        assert!(text.contains("Cache=no"));
        assert!(text.contains("guardiana dns --restore"));
    }

    #[test]
    fn nmcli_servers_split_on_any_separator() {
        assert_eq!(parse_nmcli_servers("192.168.1.1 | 1.1.1.1\n").len(), 2);
        assert_eq!(parse_nmcli_servers("192.168.1.1,8.8.8.8").len(), 2);
    }

    fn ip(s: &str) -> IpAddr {
        s.parse()
            .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED))
    }

    /// The DHCP options as `nmcli -g DHCP4.OPTION` prints them (one line, ` | ` between
    /// options) and as `nmcli -f DHCP4` prints them (one option per line).
    #[test]
    fn dhcp4_options_give_the_servers_the_network_handed_out() {
        let terse = "dhcp_lease_time = 86400 | dhcp_server_identifier = 192.168.1.1 | domain_name = lan | domain_name_servers = 192.168.1.1 8.8.8.8 | expiry = 1790000000 | ip_address = 192.168.1.50 | next_server = 192.168.1.1 | routers = 192.168.1.1 | subnet_mask = 255.255.255.0\n";
        assert_eq!(
            parse_dhcp4_dns(terse),
            vec![ip("192.168.1.1"), ip("8.8.8.8")]
        );
        let long = "DHCP4.OPTION[1]:                        dhcp_client_identifier = 01:08:00:27:4e:66:a1\nDHCP4.OPTION[2]:                        dhcp_lease_time = 86400\nDHCP4.OPTION[3]:                        dhcp_server_identifier = 10.0.2.2\nDHCP4.OPTION[4]:                        domain_name_servers = 10.0.2.3\nDHCP4.OPTION[5]:                        expiry = 1790000000\nDHCP4.OPTION[6]:                        ip_address = 10.0.2.15\n";
        assert_eq!(parse_dhcp4_dns(long), vec![ip("10.0.2.3")]);
        // A connection that is not up, or has a static address, prints nothing.
        assert!(parse_dhcp4_dns("").is_empty());
        assert!(parse_dhcp4_dns("ip_address = 10.0.0.5\n").is_empty());
    }

    /// A NetworkManager profile as `nm_snapshot` records it: `extra` is `ipv4.dns` and
    /// `ipv4.ignore-auto-dns`, one per line, and `servers` what the profile used that day.
    fn nm_profile(servers: &[&str], typed: &str, ignore_auto: &str) -> InterfaceDns {
        InterfaceDns {
            id: "Casa".into(),
            name: "Casa (wlan0)".into(),
            servers: servers.iter().map(|s| ip(s)).collect(),
            automatic: ignore_auto != "yes",
            extra: Some(format!("{typed}\n{ignore_auto}")),
        }
    }

    /// The skeptic's case of the second round: a profile with the address from DHCP and
    /// 9.9.9.9 typed by hand (`ipv4.ignore-auto-dns=yes`) still has a lease with the
    /// provider's router in it, and the first version of `nm_live` took the lease. The
    /// person's choice stays the person's choice.
    #[test]
    fn a_profile_with_hand_typed_servers_keeps_them_whatever_the_lease_says() {
        let router = [ip("192.168.1.1")];
        let typed = nm_profile(&["9.9.9.9"], "9.9.9.9", "yes");
        assert_eq!(nm_profile_upstreams(&typed, &router), vec![ip("9.9.9.9")]);
        assert_eq!(nm_profile_upstreams(&typed, &[]), vec![ip("9.9.9.9")]);

        // Typed and automatic together (`ignore-auto-dns=no` with `ipv4.dns` set): the
        // typed ones first, as NetworkManager orders them, then today's lease -- not the
        // router of the day the copy was taken.
        let both = nm_profile(&["9.9.9.9", "192.168.1.1"], "9.9.9.9", "no");
        assert_eq!(
            nm_profile_upstreams(&both, &[ip("10.0.0.1")]),
            vec![ip("9.9.9.9"), ip("10.0.0.1")]
        );

        // The network's alone: today's lease, and nothing when there is none right now.
        let auto = nm_profile(&["192.168.1.1"], "", "no");
        assert_eq!(
            nm_profile_upstreams(&auto, &[ip("10.0.0.1")]),
            vec![ip("10.0.0.1")]
        );
        assert!(nm_profile_upstreams(&auto, &[]).is_empty());
        // A copy from before `extra` existed is read as automatic too.
        let old = InterfaceDns {
            extra: None,
            ..nm_profile(&["192.168.1.1"], "", "no")
        };
        assert_eq!(
            nm_profile_upstreams(&old, &[ip("10.0.0.1")]),
            vec![ip("10.0.0.1")]
        );

        // Never the loopback nor a link-local address, from either side.
        let lo = nm_profile(&["127.0.0.1", "9.9.9.9"], "127.0.0.1,9.9.9.9", "yes");
        assert_eq!(nm_profile_upstreams(&lo, &router), vec![ip("9.9.9.9")]);
        assert_eq!(
            nm_profile_upstreams(&auto, &[ip("fe80::1"), ip("::1"), ip("10.0.0.1")]),
            vec![ip("10.0.0.1")]
        );
    }

    /// `networkctl status enp0s3`: the first server on the `DNS:` line, the rest indented
    /// below it, and the next label ends the list.
    #[test]
    fn networkctl_status_lists_the_dns_block_only() {
        let text = "\u{25cf} 2: enp0s3\n                     Link File: /usr/lib/systemd/network/99-default.link\n                  Network File: /run/systemd/network/10-netplan-enp0s3.network\n                         State: routable (configured)\n                       Address: 10.0.2.15 (DHCP4 via 10.0.2.2)\n                                fe80::a00:27ff:fe4e:66a1\n                       Gateway: 10.0.2.2\n                           DNS: 10.0.2.3\n                                1.1.1.1\n                Search Domains: lan\n             Activation Policy: up\n";
        assert_eq!(
            parse_networkctl_dns(text),
            vec![ip("10.0.2.3"), ip("1.1.1.1")]
        );
        // networkd not in charge of the link: no DNS line at all.
        assert!(
            parse_networkctl_dns("\u{25cf} 3: wlan0\n   State: routable (unmanaged)\n").is_empty()
        );
    }

    /// Two ways resolvectl says a link is not there, and one way it says something else.
    #[test]
    fn resolvectl_tells_a_missing_link_from_a_real_failure() {
        assert!(link_is_gone(
            "Failed to resolve interface \"tun0\": No such device\n"
        ));
        assert!(link_is_gone("Unknown interface 'tun0', ignoring.\n"));
        assert!(!link_is_gone(
            "Failed to set DNS configuration: Access denied\n"
        ));
    }

    /// nmcli's exit code 10 means the profile is gone, in every language; a permissions
    /// failure (exit 4) is a failure.
    #[test]
    fn nmcli_tells_a_missing_profile_from_a_real_failure() {
        assert!(nm_connection_gone(
            Some(10),
            "Error: unknown connection 'VPN oficina'.\n"
        ));
        assert!(nm_connection_gone(
            Some(10),
            "Error: conexión desconocida 'VPN oficina'.\n"
        ));
        assert!(nm_connection_gone(
            None,
            "Error: unknown connection 'VPN oficina'.\n"
        ));
        assert!(!nm_connection_gone(
            Some(4),
            "Error: Failed to modify connection 'Casa': Insufficient privileges.\n"
        ));
    }
}
