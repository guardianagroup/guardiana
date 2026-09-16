//! Linux: systemd-resolved (`resolvectl`), NetworkManager (`nmcli`) or a
//! plain `/etc/resolv.conf`, detected in that order (brief §4).

use std::net::IpAddr;
use std::path::{Path, PathBuf};

use super::{
    guarded_resolv_conf, guarded_servers, parse_ip, run_checked, Backup, Error, InterfaceDns,
    Method,
};

const RESOLV_CONF: &str = "/etc/resolv.conf";
const STUB_RESOLV: &str = "/run/systemd/resolve/stub-resolv.conf";
const RESOLVED_DROPIN_DIR: &str = "/etc/systemd/resolved.conf.d";
const RESOLVED_DROPIN: &str = "/etc/systemd/resolved.conf.d/guardiana.conf";

fn have(cmd: &str) -> bool {
    std::process::Command::new(cmd)
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
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
    let mut text = String::new();
    text.push_str("# Guardiana: el guardián es el único resolutor de este equipo.\n");
    text.push_str("# Lo que había antes: ");
    text.push_str(&note);
    text.push('\n');
    text.push_str("# Para devolverlo: guardiana dns --restore\n");
    text.push_str("nameserver ");
    text.push_str(&guardian.to_string());
    text.push('\n');
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

fn nm_modify(name: &str, dns: &str, ignore_auto: &str) -> Result<(), Error> {
    run_checked(
        "nmcli",
        &[
            "connection",
            "modify",
            name,
            "ipv4.dns",
            dns,
            "ipv4.ignore-auto-dns",
            ignore_auto,
        ],
    )?;
    // Re-activate so the change reaches resolv.conf now, not at the next reconnect.
    run_checked("nmcli", &["connection", "up", name])?;
    Ok(())
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
            for i in &backup.interfaces {
                run_checked("resolvectl", &["dns", i.id.as_str(), g.as_str()])?;
                run_checked("resolvectl", &["domain", i.id.as_str(), "~."])?;
            }
            Ok(())
        }
        Method::NetworkManager => {
            for i in &backup.interfaces {
                let list = guarded_servers(guardian, &i.servers)
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                nm_modify(&i.id, &list, "yes")?;
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

pub(crate) fn restore(backup: &Backup) -> Result<(), Error> {
    match backup.method {
        Method::SystemdResolved => {
            for i in &backup.interfaces {
                run_checked("resolvectl", &["revert", &i.id])?;
            }
            let _ = std::fs::remove_file(RESOLVED_DROPIN);
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
            resolv_conf_restore(backup)?;
            run_checked("systemctl", &["restart", "systemd-resolved"])?;
            Ok(())
        }
        Method::NetworkManager => {
            for i in &backup.interfaces {
                let extra = i.extra.as_deref().unwrap_or("\nno");
                let (dns, ignore) = extra.split_once('\n').unwrap_or(("", "no"));
                nm_modify(&i.id, dns, ignore)?;
            }
            Ok(())
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
}
