//! `guardiana verify` (brief §10): what anyone can run to check that the
//! installed program is the published one and that nothing is open that
//! should not be. Every check reports a fact, never a promise.

use std::path::{Path, PathBuf};

use guardiana_core::{identity, paths, ChangeKind, ChangeWho, Ledger};
use guardiana_service::daemon;
use guardiana_service::home::{SETTING_HOME_APARCADO, SETTING_HOME_IP, SETTING_HOME_MODE};
use guardiana_service::sysdns::{self, SETTING_BACKUP};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Ports Guardiana may open on the LAN in Home Mode (brief §0, §7).
pub const LAN_PORTS: &[(&str, u16)] = &[("udp", 53), ("tcp", 53), ("tcp", 80), ("tcp", 7443)];

/// State of the signature check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureState {
    /// The binary carries the development placeholder key: nothing can be verified.
    DevKey,
    /// No `.minisig` file next to the binary.
    NoSignatureFile,
    /// The signature verifies against the embedded public key.
    Valid,
    /// The signature file exists but does not verify.
    Invalid,
}

/// One line of `ledger.jsonl` (brief §10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerLine {
    /// Version string.
    pub version: String,
    /// Git commit.
    pub commit: String,
    /// Publication date, RFC 3339.
    pub date: String,
    /// Files of that release.
    pub files: Vec<LedgerFile>,
    /// Rekor entry id.
    #[serde(default)]
    pub rekor_uuid: String,
}

/// One binary in a ledger line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerFile {
    /// File name.
    pub name: String,
    /// SHA-256 of the binary as built.
    pub sha256_unsigned: String,
    /// SHA-256 after Windows code signing (same as unsigned elsewhere).
    #[serde(default)]
    pub sha256_signed: String,
    /// The minisign signature text.
    #[serde(default)]
    pub minisign: String,
}

/// Where the installed binary stands against the public record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LedgerMatch {
    /// No `ledger.jsonl` was found locally.
    NoLedgerFile,
    /// The hash is not in the record.
    NotFound,
    /// The hash matches this version.
    Found {
        /// Version string.
        version: String,
        /// Publication date.
        date: String,
        /// Rekor entry id.
        rekor_uuid: String,
    },
    /// The hash is not in the record and, on this platform, it cannot be: the record carries
    /// the downloaded .zip, not the program inside it (macOS). "Does not match" here accused
    /// every Mac customer who followed the instructions (review of 1 Oct 2026, entry 19).
    ZipOnly {
        /// The .zip of this version as the record names it, with its SHA-256, when the local
        /// record has that version: what the person can compare by hand.
        zip: Option<(String, String)>,
    },
}

/// The full report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    /// Program version.
    pub version: String,
    /// Path of the running binary.
    pub binary: String,
    /// SHA-256 of the binary.
    pub sha256: String,
    /// Signature check.
    pub signature: SignatureState,
    /// Public key line embedded in the binary.
    pub public_key: String,
    /// Ledger check.
    pub ledger: LedgerMatch,
    /// On this platform the public record carries the downloaded .zip, not the installed
    /// program (macOS), so a hash of this file can never be found in it and comparing by hand
    /// means comparing the .zip (review of 1 Oct 2026, entries 19 and 22).
    #[serde(default)]
    pub ledger_records_zip: bool,
    /// `running`, `stopped`, `not_installed` or `unknown`.
    pub service: String,
    /// Resolvers the system uses now.
    pub system_dns: Vec<String>,
    /// Whether the system DNS points at Guardiana first (`None` if unknown).
    pub guardian_is_primary: Option<bool>,
    /// Whether Guardiana is the machine's only resolver (systemd-resolved) or the
    /// previous one stays as a secondary. Changes what `verify` can promise.
    pub guardian_is_sole: bool,
    /// Whether a DNS backup is stored (Guardiana changed the DNS).
    pub dns_changed_by_guardiana: bool,
    /// Whether a name asked through the system's own resolver really arrived at Guardiana.
    /// The settings are what the system was told; this is what it does (27 Sep 2026). Probed
    /// on every platform whenever the settings point at Guardiana or Guardiana changed them;
    /// `None` when there was no reason to ask. It is the only ground on which the report says
    /// "the queries go through Guardiana" (review of 1 Oct 2026, entry 14).
    #[serde(default)]
    pub camino_real: Option<bool>,
    /// The last DNS change written down was Guardiana stepping aside because the servers it
    /// forwards to went quiet: the machine uses its previous DNS on purpose, and the guardian
    /// comes back when they answer. Without this, the report blamed "the system" for a change
    /// Guardiana made itself.
    #[serde(default)]
    pub apartada_sin_arriba: bool,
    /// Home Mode on.
    pub home_mode: bool,
    /// Home Mode parked because the trial or the subscription ended: while the program stands
    /// aside, a relay on the LAN passes the house's queries on (port 53 only), without looking.
    /// Its port is expected, and the report says why it is open.
    #[serde(default)]
    pub home_parked: bool,
    /// LAN address Home Mode uses, if on.
    pub home_ip: Option<String>,
    /// Devices with Guard mode on: everything outside their declared scope is being cut
    /// (decision 154). `verify` exists to say what this installation is really doing, and
    /// cutting most of a machine's traffic is the loudest thing it can be doing.
    pub vigilante: Vec<String>,
    /// Guardiana's own ports open on non-loopback addresses, as `proto/port`. Only sockets the
    /// system says belong to Guardiana are here: the customer test of 4 Oct 2026 caught
    /// `verify` listing Docker's port 53 and Windows' own port 80 as Guardiana's.
    pub lan_ports_open: Vec<String>,
    /// The same ports held open towards the network by other programs, as
    /// `proto/port (program)`: said, so nothing is hidden, but never blamed on Guardiana.
    #[serde(default)]
    pub lan_ports_others: Vec<String>,
    /// The same ports open towards the network by a program the system would not name (on
    /// Linux and macOS, without administrator rights, other users' sockets come without owner).
    #[serde(default)]
    pub lan_ports_unknown: Vec<String>,
    /// Whether the port list could be read at all.
    pub lan_ports_checked: bool,
    /// Bundled lists: id, entries, fetched date.
    pub lists: Vec<(String, u64, String)>,
    /// Events in the ledger and whether its chain verifies.
    pub ledger_events: u64,
    /// Chain check result.
    pub chain_ok: Option<bool>,
    /// The ledger file is there but this user cannot open it. Without it, nothing
    /// can be said about the DNS or the extract: the answer is "run it as
    /// administrator", not a confident "Guardiana has not changed the DNS".
    pub ledger_unreadable: bool,
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    let digest = Sha256::digest(&bytes);
    Ok(digest.iter().map(|b| format!("{b:02x}")).collect())
}

/// The name this program has on the downloads page. The installers put it in place as
/// `guardiana` or `guardiana.exe`; the signature people download carries the published name.
#[must_use]
pub fn published_name() -> String {
    let v = env!("CARGO_PKG_VERSION");
    if cfg!(target_os = "windows") {
        format!("guardiana-{v}-windows-x86_64.exe")
    } else if cfg!(target_os = "macos") {
        let arch = match std::env::consts::ARCH {
            "aarch64" => "arm64",
            other => other,
        };
        format!("guardiana-{v}-macos-{arch}")
    } else {
        format!("guardiana-{v}-linux-x86_64")
    }
}

/// Where a signature for the installed program can be: next to it, or in the folder `verify` is
/// run from, under the installed name or the published one. The installers cannot carry the
/// signature (it is made after they are built), and copying it into Program Files or /usr/bin
/// takes administrator rights; until 1.0.2 those were the only places looked at, so the
/// customer test on clean machines never saw `verify` check a signature, while the install page
/// promised it (review of 1 Oct 2026, entry 22).
fn signature_candidates(binary: &Path) -> Vec<PathBuf> {
    let mut out = vec![PathBuf::from(format!("{}.minisig", binary.display()))];
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(name) = binary.file_name() {
            out.push(cwd.join(format!("{}.minisig", name.to_string_lossy())));
        }
        out.push(cwd.join(format!("{}.minisig", published_name())));
    }
    out.dedup();
    out
}

fn check_signature(binary: &Path) -> SignatureState {
    if identity::public_key_is_dev() {
        return SignatureState::DevKey;
    }
    let Ok(pk) = minisign_verify::PublicKey::decode(identity::PUBLIC_KEY_TEXT) else {
        return SignatureState::Invalid;
    };
    let Ok(bytes) = std::fs::read(binary) else {
        return SignatureState::Invalid;
    };
    // Any signature found that verifies is enough; one found that does not is said.
    let mut found = false;
    for path in signature_candidates(binary) {
        let Ok(sig_text) = std::fs::read_to_string(&path) else {
            continue;
        };
        found = true;
        if let Ok(sig) = minisign_verify::Signature::decode(&sig_text) {
            if pk.verify(&bytes, &sig, false).is_ok() {
                return SignatureState::Valid;
            }
        }
    }
    if found {
        SignatureState::Invalid
    } else {
        SignatureState::NoSignatureFile
    }
}

/// Find `ledger.jsonl` in the current directory first (the install page says to download it
/// there and run verify from that folder), then next to the binary, then in the data directory.
/// Until 1.0.5 the data directory won, so an old copy left there beat the one just downloaded
/// and verify said the hash was not in the ledger (review of 8 Oct 2026).
fn find_ledger_file(binary: &Path) -> Option<PathBuf> {
    let mut candidates = vec![PathBuf::from("ledger.jsonl")];
    if let Some(dir) = binary.parent() {
        candidates.push(dir.join("ledger.jsonl"));
    }
    candidates.push(paths::data_dir().join("ledger.jsonl"));
    candidates.into_iter().find(|p| p.is_file())
}

/// Look a hash up in ledger text.
#[must_use]
pub fn match_in_ledger(text: &str, sha256: &str) -> LedgerMatch {
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(entry) = serde_json::from_str::<LedgerLine>(line) else {
            continue;
        };
        if entry
            .files
            .iter()
            .any(|f| f.sha256_unsigned == sha256 || f.sha256_signed == sha256)
        {
            return LedgerMatch::Found {
                version: entry.version,
                date: entry.date,
                rekor_uuid: entry.rekor_uuid,
            };
        }
    }
    LedgerMatch::NotFound
}

/// What a "not found" means on a platform whose public record carries the downloaded .zip
/// and not the program inside it (macOS; review of 1 Oct 2026, entry 19).
///
/// `NotFound` is kept only when the record lists, for this `version`, a macOS file that is
/// not a .zip: then the hash of this program could have been there, and its absence means
/// something. Otherwise the hash cannot be in the record at all, and "does not match" would
/// accuse a copy that nothing contradicts; the honest answer names the .zip to compare.
#[must_use]
pub fn explain_zip_only(text: &str, version: &str) -> LedgerMatch {
    let mut zip = None;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(entry) = serde_json::from_str::<LedgerLine>(line) else {
            continue;
        };
        if entry.version != version {
            continue;
        }
        let mac: Vec<&LedgerFile> = entry
            .files
            .iter()
            .filter(|f| f.name.contains("macos"))
            .collect();
        if mac.iter().any(|f| !f.name.ends_with(".zip")) {
            return LedgerMatch::NotFound;
        }
        if zip.is_none() {
            zip = mac.iter().find(|f| f.name.ends_with(".zip")).map(|f| {
                let sha = if f.sha256_signed.is_empty() {
                    &f.sha256_unsigned
                } else {
                    &f.sha256_signed
                };
                (f.name.clone(), sha.clone())
            });
        }
    }
    LedgerMatch::ZipOnly { zip }
}

/// Parse `netstat -an` style output (Windows, macOS, Linux) into
/// `(proto, local ip, port)` for listening TCP sockets and bound UDP sockets.
#[must_use]
pub fn parse_netstat(text: &str) -> Vec<(String, String, u16)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.len() < 2 {
            continue;
        }
        let proto = tokens[0].to_ascii_lowercase();
        let (proto, local) = if proto.starts_with("tcp") {
            // Windows: TCP local foreign state; macOS/Linux: tcp4 recvq sendq local foreign state
            // The state is written in the machine's language (LISTENING, ESCUCHANDO, ESCUTANDO,
            // ABHÖREN…): a Brazilian Windows said ESCUTANDO and verify saw no port at all (review
            // of 5 Oct 2026). What does not change with the language: a listening socket on
            // Windows has no remote end, `0.0.0.0:0` or `[::]:0`.
            let sin_otro_extremo = tokens
                .get(2)
                .is_some_and(|f| *f == "0.0.0.0:0" || *f == "[::]:0" || *f == "*:*");
            let is_listen = line.contains("LISTEN")
                || line.contains("ESCUCHANDO")
                || line.contains("ESCUTANDO")
                || (proto == "tcp" && sin_otro_extremo);
            if !is_listen {
                continue;
            }
            (
                "tcp",
                tokens
                    .iter()
                    .find(|t| t.contains(':') || t.contains('.'))
                    .copied(),
            )
        } else if proto.starts_with("udp") {
            (
                "udp",
                tokens
                    .iter()
                    .skip(1)
                    .find(|t| t.contains(':') || t.rmatches('.').count() >= 4)
                    .copied(),
            )
        } else {
            continue;
        };
        let Some(local) = local else { continue };
        // Split "ip:port" (Windows/Linux) or "ip.port" (macOS).
        let (ip, port) = if let Some((ip, port)) = local.rsplit_once(':') {
            (ip.trim_matches(|c| c == '[' || c == ']'), port)
        } else if let Some((ip, port)) = local.rsplit_once('.') {
            (ip, port)
        } else {
            continue;
        };
        let Ok(port) = port.parse::<u16>() else {
            continue;
        };
        out.push((proto.to_owned(), ip.to_owned(), port));
    }
    out
}

fn read_listening() -> Option<Vec<(String, String, u16)>> {
    let attempts: &[(&str, &[&str])] = if cfg!(target_os = "windows") {
        &[("netstat", &["-an"])]
    } else if cfg!(target_os = "linux") {
        &[("netstat", &["-lntu"]), ("ss", &["-lntu"])]
    } else {
        &[("netstat", &["-an"])]
    };
    for (cmd, args) in attempts {
        if let Ok(text) = sysdns::run_checked(cmd, args) {
            return Some(parse_netstat(&text));
        }
    }
    None
}

fn is_loopback_text(ip: &str) -> bool {
    ip == "127.0.0.1" || ip == "::1" || ip.starts_with("127.") || ip == "localhost"
}

/// Guardiana's LAN ports found open on non-loopback addresses.
#[must_use]
pub fn lan_ports_open(listening: &[(String, String, u16)]) -> Vec<String> {
    let mut out = Vec::new();
    for (proto, port) in LAN_PORTS {
        let open = listening
            .iter()
            .any(|(p, ip, pt)| p == proto && *pt == *port && !is_loopback_text(ip));
        if open {
            out.push(format!("{proto}/{port}"));
        }
    }
    out
}

/// One socket open for listening: protocol, local address, port, and the program that owns
/// it when the system says so.
pub type OwnedSocket = (String, String, u16, Option<String>);

/// Whether a program name is Guardiana itself, as each system writes it (`guardiana.exe` on
/// Windows, `guardiana` elsewhere).
fn is_guardiana(name: &str) -> bool {
    let n = name.trim().to_ascii_lowercase();
    n == "guardiana" || n == "guardiana.exe"
}

/// `netstat -ano` (Windows): the PID is the last column of every TCP and UDP line.
fn parse_netstat_pids(text: &str) -> Vec<(String, String, u16, u32)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let Some(pid) = line
            .split_whitespace()
            .last()
            .and_then(|t| t.parse::<u32>().ok())
        else {
            continue;
        };
        for (proto, ip, port) in parse_netstat(line) {
            out.push((proto, ip, port, pid));
        }
    }
    out
}

/// `tasklist /FO CSV /NH` (Windows): `"image","pid",...` per line.
fn parse_tasklist(text: &str) -> Vec<(u32, String)> {
    text.lines()
        .filter_map(|l| {
            let mut f = l.split("\",\"");
            let image = f.next()?.trim_start_matches('"').to_owned();
            let pid = f.next()?.trim_end_matches('"').parse().ok()?;
            Some((pid, image))
        })
        .collect()
}

/// `ss -lntupH` (Linux): proto in the first column, local address in the fifth, and the owner
/// in `users:(("name",pid=N,fd=M))` when the caller may see it.
fn parse_ss(text: &str) -> Vec<OwnedSocket> {
    let mut out = Vec::new();
    for line in text.lines() {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        let (Some(proto), Some(local)) = (tokens.first(), tokens.get(4)) else {
            continue;
        };
        let proto = match *proto {
            "tcp" => "tcp",
            "udp" => "udp",
            _ => continue,
        };
        let Some((ip, port)) = local.rsplit_once(':') else {
            continue;
        };
        let Ok(port) = port.parse::<u16>() else {
            continue;
        };
        let ip = ip.trim_matches(|c| c == '[' || c == ']');
        let ip = ip.split('%').next().unwrap_or(ip);
        let owner = line
            .split_once("users:((\"")
            .and_then(|(_, rest)| rest.split('"').next())
            .map(str::to_owned);
        out.push((proto.to_owned(), ip.to_owned(), port, owner));
    }
    out
}

/// `lsof -nP -iTCP -sTCP:LISTEN -iUDP` (macOS): command first, `TCP`/`UDP` in the NODE
/// column, and `address:port` after it.
fn parse_lsof(text: &str) -> Vec<OwnedSocket> {
    let mut out = Vec::new();
    for line in text.lines().skip(1) {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        let Some(node) = tokens.iter().position(|t| *t == "TCP" || *t == "UDP") else {
            continue;
        };
        let (Some(command), Some(name)) = (tokens.first(), tokens.get(node + 1)) else {
            continue;
        };
        // `local->remote` is a socket talking to one other end, not a port open to the network.
        // Read as it was, the remote's port came out as one of ours: a connected UDP socket to a
        // resolver showed as "port 53 open" (review of 5 Oct 2026, Mac medium).
        if name.contains("->") {
            continue;
        }
        let Some((ip, port)) = name.rsplit_once(':') else {
            continue;
        };
        let Ok(port) = port.parse::<u16>() else {
            continue;
        };
        let ip = ip.trim_matches(|c| c == '[' || c == ']');
        let ip = if ip == "*" { "0.0.0.0" } else { ip };
        out.push((
            tokens[node].to_ascii_lowercase(),
            ip.to_owned(),
            port,
            Some((*command).to_owned()),
        ));
    }
    out
}

/// The listening sockets with their owners, as far as this system and these rights allow.
/// `None` when not even the list could be read.
fn read_owned() -> Option<Vec<OwnedSocket>> {
    if cfg!(target_os = "windows") {
        let text = sysdns::run_checked("netstat", &["-ano"]).ok()?;
        let names = sysdns::run_checked("tasklist", &["/FO", "CSV", "/NH"])
            .map(|t| parse_tasklist(&t))
            .unwrap_or_default();
        return Some(
            parse_netstat_pids(&text)
                .into_iter()
                .map(|(p, ip, port, pid)| {
                    let owner = names
                        .iter()
                        .find(|(n, _)| *n == pid)
                        .map(|(_, i)| i.clone());
                    (p, ip, port, owner)
                })
                .collect(),
        );
    }
    if cfg!(target_os = "linux") {
        if let Ok(text) = sysdns::run_checked("ss", &["-lntupH"]) {
            return Some(parse_ss(&text));
        }
    } else if let Ok(text) = sysdns::run_checked("lsof", &["-nP", "-iTCP", "-sTCP:LISTEN", "-iUDP"])
    {
        // lsof only lists what the caller may see; netstat sees every socket, without owner.
        let named = parse_lsof(&text);
        let mut all: Vec<OwnedSocket> = read_listening()
            .unwrap_or_default()
            .into_iter()
            .map(|(p, ip, port)| {
                let owner = named
                    .iter()
                    .find(|(np, nip, npt, _)| {
                        *np == p && *npt == port && (*nip == ip || nip == "0.0.0.0")
                    })
                    .and_then(|s| s.3.clone());
                (p, ip, port, owner)
            })
            .collect();
        for s in named {
            if !all.iter().any(|a| a.0 == s.0 && a.1 == s.1 && a.2 == s.2) {
                all.push(s);
            }
        }
        return Some(all);
    }
    read_listening().map(|v| {
        v.into_iter()
            .map(|(p, ip, port)| (p, ip, port, None))
            .collect()
    })
}

/// Guardiana's LAN ports split by who holds them: Guardiana's own (the only ones it is
/// blamed for), other programs' (named), and those whose owner the system would not say.
#[must_use]
pub fn classify_lan_ports(sockets: &[OwnedSocket]) -> (Vec<String>, Vec<String>, Vec<String>) {
    let (mut own, mut others, mut unknown) = (Vec::new(), Vec::new(), Vec::new());
    for (proto, port) in LAN_PORTS {
        let open: Vec<&OwnedSocket> = sockets
            .iter()
            .filter(|(p, ip, pt, _)| p == proto && pt == port && !is_loopback_text(ip))
            .collect();
        if open.is_empty() {
            continue;
        }
        let label = format!("{proto}/{port}");
        if open
            .iter()
            .any(|s| s.3.as_deref().is_some_and(is_guardiana))
        {
            own.push(label);
        } else if open.iter().any(|s| s.3.is_none()) {
            unknown.push(label);
        } else {
            let mut names: Vec<&str> = open.iter().filter_map(|s| s.3.as_deref()).collect();
            names.sort_unstable();
            names.dedup();
            others.push(format!("{label} ({})", names.join(", ")));
        }
    }
    (own, others, unknown)
}

/// Run every check.
#[must_use]
pub fn run() -> Report {
    let binary = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("guardiana"));
    let sha256 = sha256_file(&binary).unwrap_or_default();
    let signature = check_signature(&binary);
    // The release on macOS is a .app inside a .zip, and the public record signs and lists the
    // .zip: the program that runs is never in it (review of 1 Oct 2026, entry 19).
    let ledger_records_zip = cfg!(target_os = "macos");
    let ledger = match find_ledger_file(&binary) {
        Some(p) => match std::fs::read_to_string(&p) {
            Ok(text) => match match_in_ledger(&text, &sha256) {
                LedgerMatch::NotFound if ledger_records_zip => {
                    explain_zip_only(&text, env!("CARGO_PKG_VERSION"))
                }
                m => m,
            },
            Err(_) => LedgerMatch::NoLedgerFile,
        },
        None => LedgerMatch::NoLedgerFile,
    };
    let service = match daemon::state() {
        Ok(daemon::State::Running) => "running",
        Ok(daemon::State::Stopped) => "stopped",
        Ok(daemon::State::NotInstalled) => "not_installed",
        Err(_) => "unknown",
    }
    .to_owned();
    let mut system_dns: Vec<String> = sysdns::current_resolvers()
        .map(|c| c.servers.iter().map(|s| s.ip().to_string()).collect())
        .unwrap_or_default();
    let owned = read_owned();
    let lan_ports_checked = owned.is_some();
    let (lan_ports, lan_ports_others, lan_ports_unknown) =
        owned.as_deref().map(classify_lan_ports).unwrap_or_default();

    let stored_backup = Ledger::open(&paths::ledger_path(), identity::genesis())
        .ok()
        .and_then(|l| l.setting(SETTING_BACKUP).ok().flatten())
        .filter(|v| !v.is_empty())
        .and_then(|v| serde_json::from_str::<sysdns::Backup>(&v).ok());
    // Opening the ledger can fail for two very different reasons, and telling a
    // person the wrong one is worse than saying nothing: the file may not exist
    // yet (fresh install), or it may exist and belong to root (the normal case
    // on Linux, where the service runs as root and the user runs `verify`).
    let ledger_unreadable = paths::unreadable_here(&paths::ledger_path());
    let mut home_parked = false;
    let (dns_changed, home_mode, home_ip, ledger_events, chain_ok, vigilante, apartada) =
        match Ledger::open(&paths::ledger_path(), identity::genesis()) {
            Ok(l) => (
                {
                    home_parked = l
                        .setting(SETTING_HOME_APARCADO)
                        .ok()
                        .flatten()
                        .is_some_and(|v| v == "1");
                    l.setting(SETTING_BACKUP)
                        .ok()
                        .flatten()
                        .is_some_and(|v| !v.is_empty())
                },
                l.setting(SETTING_HOME_MODE)
                    .ok()
                    .flatten()
                    .is_some_and(|v| v == "1"),
                l.setting(SETTING_HOME_IP)
                    .ok()
                    .flatten()
                    .filter(|v| !v.is_empty()),
                l.event_count().unwrap_or(0),
                l.check().ok().map(|r| r.is_ok()),
                l.devices()
                    .map(|ds| {
                        ds.into_iter()
                            .filter(|d| {
                                l.setting(&format!("alcance_modo:{}", d.id))
                                    .ok()
                                    .flatten()
                                    .as_deref()
                                    == Some("cortar")
                                    && l.setting(&format!("alcance:{}", d.id))
                                        .ok()
                                        .flatten()
                                        .is_some_and(|v| !v.trim().is_empty())
                            })
                            .map(|d| d.name.unwrap_or(d.id))
                            .collect()
                    })
                    .unwrap_or_default(),
                // The newest DNS change on record, if it was the guardian stepping aside
                // because its upstreams went quiet (engine: `devolver_por_silencio`).
                l.changes(50)
                    .ok()
                    .and_then(|cs| {
                        cs.into_iter()
                            .find(|c| matches!(c.kind, ChangeKind::DnsOn | ChangeKind::DnsOff))
                    })
                    .is_some_and(|c| {
                        c.kind == ChangeKind::DnsOff && c.who == ChangeWho::SinArriba.as_str()
                    }),
            ),
            Err(_) => (false, false, None, 0, None, Vec::new(), false),
        };
    let guardian_is_primary = sysdns::guardian_is_primary();
    // With Guardiana as the only resolver there is nothing left on the system but
    // the loopback, which `current_resolvers` leaves out on purpose. Saying "-"
    // would read as "you have no DNS": show where the guardian forwards instead.
    // Only when the settings really point at the guardian: with them unreadable, the
    // forwarders would be printed as if they were the machine's DNS.
    if system_dns.is_empty() && guardian_is_primary == Some(true) {
        if let Some(b) = stored_backup.as_ref() {
            system_dns = sysdns::upstreams_for(b)
                .iter()
                .map(ToString::to_string)
                .collect();
        }
    }
    // Ask, on every platform. The settings said "127.0.0.1 first" on 27 Sep 2026 and nothing
    // arrived; on Linux and macOS the report then went on saying "the queries go through
    // Guardiana" with the service stopped, out of the settings alone (review of 1 Oct 2026,
    // entry 14). Asking is cheap and is the only check that means anything.
    let camino_real = (dns_changed || guardian_is_primary == Some(true))
        .then(|| sysdns::system_reaches_guardian(std::time::Duration::from_secs(4)));
    let lists = guardiana_lists::manifest()
        .map(|m| {
            m.lists
                .iter()
                .map(|l| (l.id.clone(), l.entries, m.fetched.clone()))
                .collect()
        })
        .unwrap_or_default();
    Report {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        binary: binary.display().to_string(),
        sha256,
        signature,
        public_key: identity::public_key_line().to_owned(),
        ledger,
        ledger_records_zip,
        service,
        system_dns,
        guardian_is_primary,
        guardian_is_sole: sysdns::guardian_is_sole_resolver(),
        dns_changed_by_guardiana: dns_changed,
        camino_real,
        apartada_sin_arriba: apartada,
        home_mode,
        home_parked,
        vigilante,
        home_ip,
        lan_ports_open: lan_ports,
        lan_ports_others,
        lan_ports_unknown,
        lan_ports_checked,
        lists,
        ledger_events,
        chain_ok,
        ledger_unreadable,
    }
}

/// The report as plain sentences, from the texts file, in the CLI's language.
#[must_use]
pub fn render(report: &Report) -> String {
    render_with(guardiana_core::i18n::current(), report)
}

/// The report as plain sentences in the given language (the panel asks per request).
#[must_use]
pub fn render_with(t: &guardiana_core::i18n::Texts, report: &Report) -> String {
    render_con(t, report, false)
}

/// The same, for the panel's Verify page. The panel runs `verify` inside the service, whose folder
/// is System32 or `/`: "download the file to a folder and run verify from it" cannot apply there,
/// and the page said it anyway (review of 5 Oct 2026). It says where to do it instead.
#[must_use]
pub fn render_para_panel(t: &guardiana_core::i18n::Texts, report: &Report) -> String {
    render_con(t, report, true)
}

fn render_con(t: &guardiana_core::i18n::Texts, report: &Report, en_panel: bool) -> String {
    let mut out = Vec::new();
    out.push(
        t.cli("verify.version")
            .replace("{v}", &report.version)
            .replace("{ruta}", &report.binary),
    );
    out.push(t.cli("verify.hash").replace("{sha}", &report.sha256));
    out.push(match report.signature {
        SignatureState::DevKey => t.cli("verify.firma.dev").to_owned(),
        // Says which file to download and from where to run it: the signature is looked for
        // in that folder too, under the name it has on the website.
        // On a Mac what is published and signed is the .zip, never the program inside it: asking
        // for a signature of this file sent people after one that does not exist (5 Oct 2026).
        SignatureState::NoSignatureFile if report.ledger_records_zip => {
            t.cli("verify.firma.mac_zip").to_owned()
        }
        SignatureState::NoSignatureFile => t
            .cli(if en_panel {
                "verify.firma.sin_archivo_panel"
            } else {
                "verify.firma.sin_archivo"
            })
            .replace("{archivo}", &format!("{}.minisig", published_name())),
        SignatureState::Valid => t
            .cli("verify.firma.ok")
            .replace("{clave}", report.public_key.trim()),
        SignatureState::Invalid => t.cli("verify.firma.mal").to_owned(),
    });
    out.push(match &report.ledger {
        // On a Mac the hand check the sentence invites would fail the same way the automatic
        // one does: the published record has the .zip, never this file. Say which to compare.
        LedgerMatch::NoLedgerFile if report.ledger_records_zip => {
            t.cli("verify.registro.mac_sin_archivo").to_owned()
        }
        LedgerMatch::NoLedgerFile => t
            .cli(if en_panel {
                "verify.registro.sin_archivo_panel"
            } else {
                "verify.registro.sin_archivo"
            })
            .to_owned(),
        LedgerMatch::NotFound => t.cli("verify.registro.no_esta").to_owned(),
        LedgerMatch::ZipOnly {
            zip: Some((name, sha)),
        } => t
            .cli("verify.registro.mac_zip_huella")
            .replace("{archivo}", name)
            .replace("{sha}", sha),
        LedgerMatch::ZipOnly { zip: None } => t.cli("verify.registro.mac_zip").to_owned(),
        LedgerMatch::Found {
            version,
            date,
            rekor_uuid,
        } => t
            .cli("verify.registro.ok")
            .replace("{v}", version)
            .replace("{fecha}", date)
            .replace(
                "{rekor}",
                if rekor_uuid.is_empty() {
                    "-"
                } else {
                    rekor_uuid
                },
            ),
    });
    // Un servicio parado sin motivo no dice nada. Si el arranque dejó escrito por qué —lo escribe
    // el propio servicio al fallar, porque no tiene dónde imprimir— se enseña aquí, que es donde
    // mira la persona cuando algo no va (20 sep 2026, ensayando la actualización a la 1.0).
    let parado = match guardiana_core::paths::leer_motivo() {
        Some((_, motivo)) => t
            .cli("servicio.estado.stopped_motivo")
            .replace("{motivo}", &motivo),
        None => t.cli("servicio.estado.stopped").to_owned(),
    };
    out.push(match report.service.as_str() {
        "running" => t.cli("servicio.estado.running").to_owned(),
        "stopped" => parado,
        "not_installed" => t.cli("servicio.estado.none").to_owned(),
        _ => t.cli("verify.servicio.desconocido").to_owned(),
    });
    let dns = if report.system_dns.is_empty() {
        "-".to_owned()
    } else {
        report.system_dns.join(", ")
    };
    // Three facts, kept apart on purpose: the settings point at the guardian (`apunta`), a
    // query asked just now arrived there (`llega`), and the service is running. Until the
    // review of 1 Oct 2026 (entry 14) the first alone produced "the queries go through
    // Guardiana", next to "service: stopped". Every sentence that claims traffic now needs
    // `llega`; the settings get a sentence of their own.
    let apunta = report.guardian_is_primary == Some(true);
    let llega = report.camino_real == Some(true);
    let changed = report.dns_changed_by_guardiana;
    // Different sentence when the guardian is the only resolver: the servers listed
    // are the ones it forwards to, not the ones the machine asks.
    let dns_key = if apunta && !changed && report.system_dns.is_empty() {
        // El equipo pregunta al guardián y no hay nada más: `current_resolvers` deja fuera el
        // loopback a propósito, así que sin esta rama la respuesta era «DNS del sistema: -»
        // seguida de «Guardiana no ha cambiado el DNS», y quien la leía entendía justo lo
        // contrario de lo que pasa (decisión 140).
        if llega {
            "verify.dns.solo_guardiana_sin_copia"
        } else {
            "verify.dns.solo_guardiana_ajustes"
        }
    } else if apunta && changed && report.guardian_is_sole {
        // "Which forwards to X" describes a guardian at work; stopped, it forwards nothing.
        if llega {
            "verify.dns.por_guardiana"
        } else {
            "verify.dns.solo_guardiana_ajustes"
        }
    } else if apunta && changed {
        // Windows keeps the old resolver as the second one so the machine never
        // loses its name service if the guardian stops. Saying only "System DNS:
        // 192.168.1.1" - which is what `current_resolvers` returns, because it
        // leaves the loopback out - reads as if the guardian were not there at
        // all, and hides the hole: while it is down, those queries go to the
        // fallback and are not written down. Say both, and say the consequence.
        "verify.dns.guardiana_con_reserva"
    } else {
        "verify.dns"
    };
    out.push(t.cli(dns_key).replace("{dns}", &dns));
    let parado = matches!(report.service.as_str(), "stopped" | "not_installed");
    out.push(if report.ledger_unreadable {
        // Sin el extracto no se sabe si el cambio de DNS lo hizo Guardiana. Decir
        // «no lo ha cambiado» sería mentir con seguridad, que es lo peor que puede
        // hacer aquí: se dice lo que pasa y cómo verlo de verdad.
        t.cli("verify.extracto.sin_permiso").to_owned()
    } else {
        match (changed, report.guardian_is_primary) {
            // What happened to the resolver that was there before is not the same
            // on every machine, so it is a second sentence and not a guess.
            (true, Some(true)) if llega => format!(
                "{} {}",
                t.cli("verify.dns.guardiana_primario"),
                t.cli(if report.guardian_is_sole {
                    sysdns::sole_or_secondary_key()
                } else {
                    "dns.reserva_secundario"
                })
            ),
            // The settings point at the guardian and that is all that is known: whether a
            // query arrives is the probe's sentence, below, and when it did not, why.
            (true, Some(true)) => t.cli("verify.dns.guardiana_apunta").to_owned(),
            // The service gives the machine its DNS back while it is stopped (1.0.2) and keeps
            // the copy: "the system changed it back, the service reapplies it every minute" was
            // false on both counts in the most ordinary case of all.
            (true, Some(false)) if parado => {
                t.cli("verify.dns.guardiana_no_primario_parado").to_owned()
            }
            // The guardian stepped aside itself because its upstreams went quiet: not the
            // system's doing, and it comes back on its own.
            (true, Some(false)) if report.apartada_sin_arriba => {
                t.cli("verify.dns.guardiana_apartada").to_owned()
            }
            (true, Some(false)) => t.cli("verify.dns.guardiana_no_primario").to_owned(),
            (true, None) => t.cli("verify.dns.guardiana_desconocido").to_owned(),
            // Apunta al guardián, pero el cambio no lo hizo él y no hay copia de lo que había
            // antes: hay que decir las dos cosas, porque la segunda es la que se nota el día
            // que se desinstale.
            (false, Some(true)) => t.cli("verify.dns.apunta_sin_copia").to_owned(),
            (false, _) => t.cli("verify.dns.sin_cambiar").to_owned(),
        }
    });
    match report.camino_real {
        Some(true) => out.push(t.cli("verify.dns.camino_ok").to_owned()),
        Some(false) => {
            // The fact first, then why, when the settings say the query should have arrived.
            // With the settings pointing elsewhere the sentence above already explains it.
            let motivo = match report.service.as_str() {
                "stopped" if apunta => Some("verify.dns.camino_no.parado"),
                "not_installed" if apunta => Some("verify.dns.camino_no.sin_servicio"),
                "running" if apunta => Some("verify.dns.camino_no.ajustes"),
                _ => None,
            };
            out.push(match motivo {
                Some(m) => format!("{} {}", t.cli("verify.dns.camino_no"), t.cli(m)),
                None => t.cli("verify.dns.camino_no").to_owned(),
            });
        }
        None => {}
    }
    if !report.vigilante.is_empty() {
        out.push(
            t.cli("verify.vigilante").replace(
                "{aparatos}",
                &report
                    .vigilante
                    .iter()
                    .map(|d| {
                        if d == guardiana_core::SELF_DEVICE_ID {
                            t.cli("verify.este_equipo").to_owned()
                        } else {
                            d.clone()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
        );
    }
    out.push(if report.home_mode {
        t.cli("verify.hogar.on")
            .replace("{ip}", report.home_ip.as_deref().unwrap_or("-"))
    } else if report.home_parked {
        t.cli("verify.hogar.aparcado").to_owned()
    } else {
        t.cli("verify.hogar.off").to_owned()
    });
    out.push(if !report.lan_ports_checked {
        t.cli("verify.puertos.no_leidos").to_owned()
    } else if report.lan_ports_open.is_empty() {
        t.cli("verify.puertos.ninguno").to_owned()
    } else {
        t.cli("verify.puertos.abiertos")
            .replace("{puertos}", &report.lan_ports_open.join(", "))
    });
    // Parked, the relay's DNS port is expected; anything else is not.
    let inesperados = report
        .lan_ports_open
        .iter()
        .any(|p| !(report.home_parked && p.ends_with("/53")));
    if !report.home_mode && inesperados {
        out.push(t.cli("verify.puertos.aviso").to_owned());
    }
    if !report.lan_ports_others.is_empty() {
        out.push(
            t.cli("verify.puertos.de_otros")
                .replace("{puertos}", &report.lan_ports_others.join(", ")),
        );
    }
    if !report.lan_ports_unknown.is_empty() {
        out.push(
            t.cli("verify.puertos.dueno_desconocido")
                .replace("{puertos}", &report.lan_ports_unknown.join(", ")),
        );
    }
    for (id, n, fetched) in &report.lists {
        out.push(
            t.cli("verify.lista")
                .replace("{id}", id)
                .replace("{n}", &n.to_string())
                .replace("{fecha}", fetched),
        );
    }
    out.push(match report.chain_ok {
        Some(true) => t.cli_n("verify.cadena.ok", report.ledger_events as i64),
        Some(false) => t.cli("verify.cadena.mal").to_owned(),
        None if report.ledger_unreadable => t.cli("verify.cadena.sin_permiso").to_owned(),
        None => t.cli("verify.cadena.sin_extracto").to_owned(),
    });
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn informe_vacio() -> Report {
        Report {
            version: "0.0.0".into(),
            binary: "/tmp/guardiana".into(),
            sha256: "0".repeat(64),
            signature: SignatureState::DevKey,
            public_key: String::new(),
            ledger: LedgerMatch::NoLedgerFile,
            ledger_records_zip: false,
            service: "running".into(),
            system_dns: Vec::new(),
            guardian_is_primary: None,
            guardian_is_sole: false,
            dns_changed_by_guardiana: false,
            camino_real: None,
            apartada_sin_arriba: false,
            home_mode: false,
            home_parked: false,
            home_ip: None,
            vigilante: Vec::new(),
            lan_ports_open: Vec::new(),
            lan_ports_others: Vec::new(),
            lan_ports_unknown: Vec::new(),
            lan_ports_checked: true,
            lists: Vec::new(),
            ledger_events: 0,
            chain_ok: None,
            ledger_unreadable: false,
        }
    }

    fn t() -> &'static guardiana_core::i18n::Texts {
        guardiana_core::i18n::current()
    }

    /// Every `verify.*` text this file asks for exists in the three languages. `Texts::cli`
    /// falls back to the key itself, so a missing text prints `verify.dns.guardiana_apunta` to
    /// the customer and the tests that compare against `t().cli(key)` still pass: branch 3 of
    /// the 1 Oct review shipped ten such keys and nothing failed.
    #[test]
    fn every_verify_text_exists_in_every_language() {
        let source = include_str!("lib.rs");
        let mut keys: Vec<&str> = source
            .split("cli(\"")
            .skip(1)
            .filter_map(|rest| rest.split('"').next())
            .filter(|k| k.starts_with("verify."))
            .collect();
        keys.sort_unstable();
        keys.dedup();
        assert!(keys.len() > 30, "the key scan found only {}", keys.len());
        for (lang, json) in [
            ("es", guardiana_core::i18n::ES_JSON),
            ("en", guardiana_core::i18n::EN_JSON),
            ("pt", guardiana_core::i18n::PT_JSON),
        ] {
            let all: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
            assert!(all.is_object(), "{lang}.json does not parse");
            let cli = &all["cli"];
            let missing: Vec<&&str> = keys.iter().filter(|k| cli.get(**k).is_none()).collect();
            assert!(missing.is_empty(), "{lang}.json lacks {missing:?}");
        }
    }

    /// Whether `salida` has a whole line equal to `linea`. A substring check would be fooled
    /// by a key that is the prefix of another (`camino_no` / `camino_no.parado`) while the
    /// texts file falls back to the key name.
    fn tiene_linea(salida: &str, linea: &str) -> bool {
        salida.lines().any(|l| l == linea)
    }

    #[test]
    fn dns_que_apunta_al_guardian_sin_copia_no_dice_que_no_ha_cambiado() {
        // El caso real del Mac del 19 sep 2026: los tres servicios de red apuntan a 127.0.0.1 y
        // `verify` respondía «DNS del sistema: -» y «Guardiana no ha cambiado el DNS del sistema»,
        // es decir, lo contrario de lo que pasaba, en la orden que existe para dar confianza.
        let mut r = informe_vacio();
        r.guardian_is_primary = Some(true);
        r.camino_real = Some(true);
        let salida = render(&r);
        assert!(
            salida.contains("127.0.0.1") && salida.contains("pasan por ella"),
            "tiene que decir que el equipo pregunta al guardián: {salida}"
        );
        assert!(
            !salida.contains("Guardiana no ha cambiado el DNS del sistema."),
            "no puede negar el cambio cuando el equipo apunta al guardián: {salida}"
        );
        assert!(
            salida.contains("no tiene anotado qué DNS había antes"),
            "y tiene que avisar de que no podrá devolverlo: {salida}"
        );
        // Sin guardián delante, el mensaje de siempre se mantiene.
        let mut r2 = informe_vacio();
        r2.guardian_is_primary = Some(false);
        r2.system_dns = vec!["192.168.1.1".into()];
        assert!(render(&r2).contains("Guardiana no ha cambiado el DNS del sistema."));
    }

    #[test]
    fn dice_si_las_consultas_llegan_de_verdad() {
        // 27 Sep 2026: los ajustes decían «127.0.0.1 primero» y no llegó ni una consulta.
        let mut r = informe_vacio();
        r.dns_changed_by_guardiana = true;
        r.guardian_is_primary = Some(true);
        r.camino_real = Some(false);
        assert!(render(&r).contains(t().cli("verify.dns.camino_no")));
        r.camino_real = Some(true);
        assert!(tiene_linea(&render(&r), t().cli("verify.dns.camino_ok")));
    }

    #[test]
    fn solo_afirma_que_las_consultas_pasan_cuando_la_prueba_real_lo_dice() {
        // Review of 1 Oct 2026, entry 14: on Linux and macOS the report said "the queries go
        // through Guardiana" out of the settings alone, next to "service: stopped". Every
        // combination without a confirmed probe must stay clear of the three claiming texts.
        let claims = |dns: &str| {
            [
                t().cli("verify.dns.guardiana_primario").to_owned(),
                t().cli("verify.dns.solo_guardiana_sin_copia")
                    .replace("{dns}", dns),
                t().cli("verify.dns.por_guardiana").replace("{dns}", dns),
            ]
        };
        for changed in [false, true] {
            for primary in [None, Some(false), Some(true)] {
                for sole in [false, true] {
                    for camino in [None, Some(false)] {
                        for service in ["running", "stopped", "not_installed", "unknown"] {
                            for dns in [Vec::new(), vec!["1.1.1.1".to_owned()]] {
                                let mut r = informe_vacio();
                                r.dns_changed_by_guardiana = changed;
                                r.guardian_is_primary = primary;
                                r.guardian_is_sole = sole;
                                r.camino_real = camino;
                                r.service = service.into();
                                r.system_dns = dns.clone();
                                let salida = render(&r);
                                let lista = if dns.is_empty() {
                                    "-".to_owned()
                                } else {
                                    dns.join(", ")
                                };
                                for c in claims(&lista) {
                                    assert!(
                                        !salida.lines().any(|l| l.starts_with(&c)),
                                        "claims traffic without a probe (changed={changed}, primary={primary:?}, sole={sole}, camino={camino:?}, service={service}):\n{salida}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        // And with the probe confirmed, the claim is back, with the sentence about the old
        // resolver after it.
        let mut r = informe_vacio();
        r.dns_changed_by_guardiana = true;
        r.guardian_is_primary = Some(true);
        r.guardian_is_sole = true;
        r.camino_real = Some(true);
        let salida = render(&r);
        assert!(salida
            .lines()
            .any(|l| l.starts_with(t().cli("verify.dns.guardiana_primario"))));
        assert!(tiene_linea(&salida, t().cli("verify.dns.camino_ok")));
    }

    #[test]
    fn con_el_servicio_parado_dice_que_no_llegan_y_por_que() {
        // The screen a person opens when the internet is gone: settings pointing at the
        // guardian, service stopped, nothing answering.
        let mut r = informe_vacio();
        r.dns_changed_by_guardiana = true;
        r.guardian_is_primary = Some(true);
        r.guardian_is_sole = true;
        r.camino_real = Some(false);
        r.service = "stopped".into();
        let salida = render(&r);
        assert!(tiene_linea(&salida, t().cli("verify.dns.guardiana_apunta")));
        assert!(tiene_linea(
            &salida,
            &format!(
                "{} {}",
                t().cli("verify.dns.camino_no"),
                t().cli("verify.dns.camino_no.parado")
            )
        ));
        // Without a service at all, the reason is a different one.
        r.service = "not_installed".into();
        assert!(tiene_linea(
            &render(&r),
            &format!(
                "{} {}",
                t().cli("verify.dns.camino_no"),
                t().cli("verify.dns.camino_no.sin_servicio")
            )
        ));
        // Running and still not arriving: the settings say one thing and the machine another.
        r.service = "running".into();
        assert!(tiene_linea(
            &render(&r),
            &format!(
                "{} {}",
                t().cli("verify.dns.camino_no"),
                t().cli("verify.dns.camino_no.ajustes")
            )
        ));
        // Settings pointing elsewhere: the fact alone, the sentence above explains it.
        r.guardian_is_primary = Some(false);
        assert!(tiene_linea(&render(&r), t().cli("verify.dns.camino_no")));
    }

    #[test]
    fn servicio_parado_con_el_dns_devuelto_no_culpa_al_sistema() {
        // Since 1.0.2 a stop gives the machine its DNS back and keeps the copy: the ordinary
        // stopped state is (changed, not primary). "The system changed it back, the service
        // reapplies it every minute" was false on both counts there.
        let mut r = informe_vacio();
        r.dns_changed_by_guardiana = true;
        r.guardian_is_primary = Some(false);
        r.camino_real = Some(false);
        r.system_dns = vec!["192.168.1.1".into()];
        r.service = "stopped".into();
        let salida = render(&r);
        assert!(tiene_linea(
            &salida,
            t().cli("verify.dns.guardiana_no_primario_parado")
        ));
        assert!(!tiene_linea(
            &salida,
            t().cli("verify.dns.guardiana_no_primario")
        ));
        assert!(tiene_linea(
            &salida,
            &t().cli("verify.dns").replace("{dns}", "192.168.1.1")
        ));
        // Running, and the last change on record was the guardian stepping aside because its
        // upstreams went quiet: its own doing, not the system's.
        r.service = "running".into();
        r.apartada_sin_arriba = true;
        assert!(tiene_linea(
            &render(&r),
            t().cli("verify.dns.guardiana_apartada")
        ));
        r.apartada_sin_arriba = false;
        assert!(tiene_linea(
            &render(&r),
            t().cli("verify.dns.guardiana_no_primario")
        ));
    }

    #[test]
    fn en_el_mac_el_registro_anota_el_zip_y_no_acusa() {
        // Review of 1 Oct 2026, entry 19: the published record lists the .app .zip, so the
        // program that runs on a Mac is never in it, and "something is off" was the answer
        // every Mac customer got for following the instructions.
        let text = "{\"version\":\"1.0.1\",\"commit\":\"c\",\"date\":\"2026-09-28\",\"files\":[{\"name\":\"guardiana-1.0.1-linux-x86_64\",\"sha256_unsigned\":\"aaa\"},{\"name\":\"guardiana-1.0.1-macos.app.zip\",\"sha256_unsigned\":\"zzz\",\"sha256_signed\":\"zzz\"}]}\n";
        assert_eq!(
            explain_zip_only(text, "1.0.1"),
            LedgerMatch::ZipOnly {
                zip: Some(("guardiana-1.0.1-macos.app.zip".into(), "zzz".into()))
            }
        );
        // A record without this version cannot name the .zip, but still cannot hold this file.
        assert_eq!(
            explain_zip_only(text, "1.0.2"),
            LedgerMatch::ZipOnly { zip: None }
        );
        // The day the record lists the program inside the .app, a missing hash means again
        // what it means on Linux.
        let con_binario = "{\"version\":\"1.0.3\",\"commit\":\"c\",\"date\":\"2026-10-10\",\"files\":[{\"name\":\"guardiana-1.0.3-macos.app.zip\",\"sha256_unsigned\":\"zzz\"},{\"name\":\"guardiana-1.0.3-macos-arm64\",\"sha256_unsigned\":\"bbb\"}]}\n";
        assert_eq!(
            explain_zip_only(con_binario, "1.0.3"),
            LedgerMatch::NotFound
        );
        // Rendered: the .zip and its fingerprint, to compare by hand.
        let mut r = informe_vacio();
        r.ledger_records_zip = true;
        r.ledger = LedgerMatch::ZipOnly {
            zip: Some(("guardiana-1.0.1-macos.app.zip".into(), "zzz".into())),
        };
        let salida = render(&r);
        assert!(tiene_linea(
            &salida,
            &t().cli("verify.registro.mac_zip_huella")
                .replace("{archivo}", "guardiana-1.0.1-macos.app.zip")
                .replace("{sha}", "zzz")
        ));
        assert!(!tiene_linea(&salida, t().cli("verify.registro.no_esta")));
        r.ledger = LedgerMatch::ZipOnly { zip: None };
        assert!(tiene_linea(&render(&r), t().cli("verify.registro.mac_zip")));
        // No record on the Mac: the hand check is against the .zip too.
        r.ledger = LedgerMatch::NoLedgerFile;
        assert!(tiene_linea(
            &render(&r),
            t().cli("verify.registro.mac_sin_archivo")
        ));
        // Elsewhere nothing changes.
        r.ledger_records_zip = false;
        assert!(tiene_linea(
            &render(&r),
            t().cli("verify.registro.sin_archivo")
        ));
        r.ledger = LedgerMatch::NotFound;
        assert!(tiene_linea(&render(&r), t().cli("verify.registro.no_esta")));
    }

    /// The signature is looked for where people can put it without administrator rights: the
    /// folder verify is run from, under the name it has on the website (review of 1 Oct 2026,
    /// entry 22). Next to the program still counts.
    #[test]
    fn la_firma_se_busca_tambien_en_la_carpeta_desde_la_que_se_ejecuta() {
        let v = env!("CARGO_PKG_VERSION");
        let nombre = published_name();
        assert!(nombre.starts_with(&format!("guardiana-{v}-")), "{nombre}");
        let programa = Path::new("/usr/bin/guardiana");
        let c = signature_candidates(programa);
        assert_eq!(c[0], PathBuf::from("/usr/bin/guardiana.minisig"));
        if let Ok(cwd) = std::env::current_dir() {
            assert!(c.contains(&cwd.join("guardiana.minisig")), "{c:?}");
            assert!(c.contains(&cwd.join(format!("{nombre}.minisig"))), "{c:?}");
        }
        // And the sentence for a missing one names that file.
        let texto = t()
            .cli("verify.firma.sin_archivo")
            .replace("{archivo}", &format!("{nombre}.minisig"));
        assert!(texto.contains(&nombre), "{texto}");
    }

    #[test]
    fn netstat_formats() {
        // A Brazilian Windows and a German one: the state in their languages.
        let pt = parse_netstat("  TCP    0.0.0.0:7443           0.0.0.0:0              ESCUTANDO       1234\n  TCP    192.168.1.39:53        0.0.0.0:0              ABHÖREN         1234\n  TCP    192.168.1.39:50000     142.250.0.1:443        ESTABELECIDA    77\n");
        assert!(pt.contains(&("tcp".into(), "0.0.0.0".into(), 7443)));
        assert!(pt.contains(&("tcp".into(), "192.168.1.39".into(), 53)));
        assert!(
            !pt.iter().any(|(_, _, p)| *p == 50000),
            "a connection is not a listener"
        );
        let win = "  TCP    0.0.0.0:7443           0.0.0.0:0              LISTENING       1234\n  TCP    127.0.0.1:53           0.0.0.0:0              LISTENING       1234\n  UDP    192.168.1.39:53        *:*                                    1234\n  TCP    [::]:22                [::]:0                 LISTENING       99\n";
        let v = parse_netstat(win);
        assert!(v.contains(&("tcp".into(), "0.0.0.0".into(), 7443)));
        assert!(v.contains(&("udp".into(), "192.168.1.39".into(), 53)));
        assert!(v.contains(&("tcp".into(), "::".into(), 22)));
        let mac = "tcp4       0      0  127.0.0.1.7443         *.*                    LISTEN     \nudp4       0      0  192.168.1.202.53       *.*                               \ntcp4       0      0  192.168.1.202.80       *.*                    LISTEN     \n";
        let v = parse_netstat(mac);
        assert!(v.contains(&("tcp".into(), "127.0.0.1".into(), 7443)));
        assert!(v.contains(&("udp".into(), "192.168.1.202".into(), 53)));
        let open = lan_ports_open(&v);
        assert_eq!(open, vec!["udp/53", "tcp/80"]);
        // Loopback only → nothing open on the LAN.
        let lo = parse_netstat("tcp4 0 0 127.0.0.1.7443 *.* LISTEN\nudp4 0 0 127.0.0.1.53 *.*\n");
        assert!(lan_ports_open(&lo).is_empty());
    }

    /// The customer test of 4 Oct 2026, Windows runner: Docker on 172.x:53 and the system on
    /// :80. Not Guardiana's, so not blamed; named, so not hidden.
    #[test]
    fn ports_of_other_programs_are_not_guardianas() {
        let netstat = "  TCP    0.0.0.0:80             0.0.0.0:0              LISTENING       4\n  TCP    172.21.160.1:53        0.0.0.0:0              LISTENING       3120\n  UDP    172.21.160.1:53        *:*                                    3120\n  TCP    127.0.0.1:7443         0.0.0.0:0              LISTENING       5000\n  UDP    127.0.0.1:53           *:*                                    5000\n";
        let tasks = "\"System\",\"4\",\"Services\",\"0\",\"144 K\"\n\"dockerd.exe\",\"3120\",\"Services\",\"0\",\"40.000 K\"\n\"guardiana.exe\",\"5000\",\"Services\",\"0\",\"20.000 K\"\n";
        let names = parse_tasklist(tasks);
        let socks: Vec<OwnedSocket> = parse_netstat_pids(netstat)
            .into_iter()
            .map(|(p, ip, port, pid)| {
                let owner = names
                    .iter()
                    .find(|(n, _)| *n == pid)
                    .map(|(_, i)| i.clone());
                (p, ip, port, owner)
            })
            .collect();
        let (own, others, unknown) = classify_lan_ports(&socks);
        assert!(own.is_empty(), "{own:?}");
        assert!(unknown.is_empty());
        assert_eq!(
            others,
            vec![
                "udp/53 (dockerd.exe)",
                "tcp/53 (dockerd.exe)",
                "tcp/80 (System)"
            ]
        );

        // Guardiana itself on the LAN (Home Mode): its own, and only its own.
        let home = vec![
            (
                "udp".into(),
                "192.168.1.39".into(),
                53,
                Some("guardiana.exe".into()),
            ),
            ("tcp".into(), "0.0.0.0".into(), 80, Some("System".into())),
        ];
        let (own, others, _) = classify_lan_ports(&home);
        assert_eq!(own, vec!["udp/53"]);
        assert_eq!(others, vec!["tcp/80 (System)"]);

        // Owner hidden (Linux without root): never blamed, said as unknown.
        let hidden = vec![("udp".into(), "0.0.0.0".into(), 53, None)];
        assert_eq!(
            classify_lan_ports(&hidden),
            (vec![], vec![], vec!["udp/53".into()])
        );
    }

    #[test]
    fn ss_and_lsof_name_the_owner() {
        let ss = "udp UNCONN 0 0 127.0.0.53%lo:53 0.0.0.0:* users:((\"systemd-resolve\",pid=600,fd=13))\nudp UNCONN 0 0 192.168.1.5:53 0.0.0.0:* users:((\"guardiana\",pid=900,fd=9))\ntcp LISTEN 0 4096 [::]:80 [::]:*\n";
        let v = parse_ss(ss);
        assert!(v.contains(&(
            "udp".into(),
            "127.0.0.53".into(),
            53,
            Some("systemd-resolve".into())
        )));
        assert!(v.contains(&(
            "udp".into(),
            "192.168.1.5".into(),
            53,
            Some("guardiana".into())
        )));
        assert!(v.contains(&("tcp".into(), "::".into(), 80, None)));
        let (own, _, unknown) = classify_lan_ports(&v);
        assert_eq!(own, vec!["udp/53"]);
        assert_eq!(unknown, vec!["tcp/80"]);

        let lsof = "COMMAND   PID USER   FD   TYPE DEVICE SIZE/OFF NODE NAME\nmDNSRespo 400 _mdns 8u IPv4 0x1 0t0 UDP *:5353\nguardiana 900 root 9u IPv4 0x2 0t0 UDP 127.0.0.1:53\nhttpd 77 root 4u IPv6 0x3 0t0 TCP *:80 (LISTEN)\nguardiana 900 root 12u IPv4 0x4 0t0 UDP 192.168.1.5:53001->8.8.8.8:53\n";
        let v = parse_lsof(lsof);
        assert!(v.contains(&(
            "udp".into(),
            "127.0.0.1".into(),
            53,
            Some("guardiana".into())
        )));
        assert!(v.contains(&("tcp".into(), "0.0.0.0".into(), 80, Some("httpd".into()))));
        let (own, others, _) = classify_lan_ports(&v);
        assert!(own.is_empty());
        assert_eq!(others, vec!["tcp/80 (httpd)"]);
    }

    #[test]
    fn ledger_lookup() {
        let text = "\n{\"version\":\"1.0.0\",\"commit\":\"abc\",\"date\":\"2026-10-06\",\"files\":[{\"name\":\"g.exe\",\"sha256_unsigned\":\"aaa\",\"sha256_signed\":\"bbb\",\"minisign\":\"\"}],\"rekor_uuid\":\"r1\"}\nnot json\n";
        assert!(
            matches!(match_in_ledger(text, "bbb"), LedgerMatch::Found { ref version, .. } if version == "1.0.0")
        );
        assert!(matches!(
            match_in_ledger(text, "aaa"),
            LedgerMatch::Found { .. }
        ));
        assert_eq!(match_in_ledger(text, "zzz"), LedgerMatch::NotFound);
    }
}
