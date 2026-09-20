//! `guardiana verify` (brief §10): what anyone can run to check that the
//! installed program is the published one and that nothing is open that
//! should not be. Every check reports a fact, never a promise.

use std::path::{Path, PathBuf};

use guardiana_core::{identity, paths, Ledger};
use guardiana_service::daemon;
use guardiana_service::home::{SETTING_HOME_IP, SETTING_HOME_MODE};
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
    /// `running`, `stopped` or `not_installed`.
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
    /// Home Mode on.
    pub home_mode: bool,
    /// LAN address Home Mode uses, if on.
    pub home_ip: Option<String>,
    /// Devices with Guard mode on: everything outside their declared scope is being cut
    /// (decision 154). `verify` exists to say what this installation is really doing, and
    /// cutting most of a machine's traffic is the loudest thing it can be doing.
    pub vigilante: Vec<String>,
    /// Guardiana ports found open on non-loopback addresses, as `proto/port`.
    pub lan_ports_open: Vec<String>,
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

fn check_signature(binary: &Path) -> SignatureState {
    if identity::public_key_is_dev() {
        return SignatureState::DevKey;
    }
    let sig_path = PathBuf::from(format!("{}.minisig", binary.display()));
    let Ok(sig_text) = std::fs::read_to_string(&sig_path) else {
        return SignatureState::NoSignatureFile;
    };
    let (Ok(pk), Ok(sig)) = (
        minisign_verify::PublicKey::decode(identity::PUBLIC_KEY_TEXT),
        minisign_verify::Signature::decode(&sig_text),
    ) else {
        return SignatureState::Invalid;
    };
    let Ok(bytes) = std::fs::read(binary) else {
        return SignatureState::Invalid;
    };
    if pk.verify(&bytes, &sig, false).is_ok() {
        SignatureState::Valid
    } else {
        SignatureState::Invalid
    }
}

/// Find `ledger.jsonl` next to the binary, in the data directory, or in the
/// current directory.
fn find_ledger_file(binary: &Path) -> Option<PathBuf> {
    let mut candidates = vec![
        paths::data_dir().join("ledger.jsonl"),
        PathBuf::from("ledger.jsonl"),
    ];
    if let Some(dir) = binary.parent() {
        candidates.insert(0, dir.join("ledger.jsonl"));
    }
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
            let is_listen = line.contains("LISTEN") || line.contains("ESCUCHANDO");
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

/// Run every check.
#[must_use]
pub fn run() -> Report {
    let binary = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("guardiana"));
    let sha256 = sha256_file(&binary).unwrap_or_default();
    let signature = check_signature(&binary);
    let ledger = match find_ledger_file(&binary) {
        Some(p) => match std::fs::read_to_string(&p) {
            Ok(text) => match_in_ledger(&text, &sha256),
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
    let listening = read_listening();
    let lan_ports_checked = listening.is_some();
    let lan_ports = listening.as_deref().map(lan_ports_open).unwrap_or_default();

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
    let (dns_changed, home_mode, home_ip, ledger_events, chain_ok, vigilante) =
        match Ledger::open(&paths::ledger_path(), identity::genesis()) {
            Ok(l) => (
                l.setting(SETTING_BACKUP)
                    .ok()
                    .flatten()
                    .is_some_and(|v| !v.is_empty()),
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
            ),
            Err(_) => (false, false, None, 0, None, Vec::new()),
        };
    // With Guardiana as the only resolver there is nothing left on the system but
    // the loopback, which `current_resolvers` leaves out on purpose. Saying "-"
    // would read as "you have no DNS": show where the guardian forwards instead.
    if system_dns.is_empty() {
        if let Some(b) = stored_backup.as_ref() {
            system_dns = b
                .original_servers()
                .iter()
                .map(ToString::to_string)
                .collect();
        }
    }
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
        service,
        system_dns,
        guardian_is_primary: sysdns::guardian_is_primary(),
        guardian_is_sole: sysdns::guardian_is_sole_resolver(),
        dns_changed_by_guardiana: dns_changed,
        home_mode,
        vigilante,
        home_ip,
        lan_ports_open: lan_ports,
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
    let mut out = Vec::new();
    out.push(
        t.cli("verify.version")
            .replace("{v}", &report.version)
            .replace("{ruta}", &report.binary),
    );
    out.push(t.cli("verify.hash").replace("{sha}", &report.sha256));
    out.push(
        match report.signature {
            SignatureState::DevKey => t.cli("verify.firma.dev"),
            SignatureState::NoSignatureFile => t.cli("verify.firma.sin_archivo"),
            SignatureState::Valid => t.cli("verify.firma.ok"),
            SignatureState::Invalid => t.cli("verify.firma.mal"),
        }
        .to_owned(),
    );
    out.push(match &report.ledger {
        LedgerMatch::NoLedgerFile => t.cli("verify.registro.sin_archivo").to_owned(),
        LedgerMatch::NotFound => t.cli("verify.registro.no_esta").to_owned(),
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
    // Different sentence when the guardian is the only resolver: the servers listed
    // are the ones it forwards to, not the ones the machine asks.
    let dns_key = if !report.dns_changed_by_guardiana
        && report.system_dns.is_empty()
        && report.guardian_is_primary == Some(true)
    {
        // El equipo pregunta al guardián y no hay nada más: `current_resolvers` deja fuera el
        // loopback a propósito, así que sin esta rama la respuesta era «DNS del sistema: -»
        // seguida de «Guardiana no ha cambiado el DNS», y quien la leía entendía justo lo
        // contrario de lo que pasa (decisión 140).
        "verify.dns.solo_guardiana_sin_copia"
    } else if report.dns_changed_by_guardiana && report.guardian_is_sole {
        "verify.dns.por_guardiana"
    } else if report.dns_changed_by_guardiana && report.guardian_is_primary == Some(true) {
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
    out.push(if report.ledger_unreadable {
        // Sin el extracto no se sabe si el cambio de DNS lo hizo Guardiana. Decir
        // «no lo ha cambiado» sería mentir con seguridad, que es lo peor que puede
        // hacer aquí: se dice lo que pasa y cómo verlo de verdad.
        t.cli("verify.extracto.sin_permiso").to_owned()
    } else {
        match (report.dns_changed_by_guardiana, report.guardian_is_primary) {
            // What happened to the resolver that was there before is not the same
            // on every machine, so it is a second sentence and not a guess.
            (true, Some(true)) => format!(
                "{} {}",
                t.cli("verify.dns.guardiana_primario"),
                t.cli(if report.guardian_is_sole {
                    "dns.solo_guardiana"
                } else {
                    "dns.reserva_secundario"
                })
            ),
            (true, Some(false)) => t.cli("verify.dns.guardiana_no_primario").to_owned(),
            (true, None) => t.cli("verify.dns.guardiana_desconocido").to_owned(),
            // Apunta al guardián, pero el cambio no lo hizo él y no hay copia de lo que había
            // antes: hay que decir las dos cosas, porque la segunda es la que se nota el día
            // que se desinstale.
            (false, Some(true)) => t.cli("verify.dns.apunta_sin_copia").to_owned(),
            (false, _) => t.cli("verify.dns.sin_cambiar").to_owned(),
        }
    });
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
    if !report.home_mode && !report.lan_ports_open.is_empty() {
        out.push(t.cli("verify.puertos.aviso").to_owned());
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
        Some(true) => t
            .cli("verify.cadena.ok")
            .replace("{n}", &report.ledger_events.to_string()),
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
            service: "running".into(),
            system_dns: Vec::new(),
            guardian_is_primary: None,
            guardian_is_sole: false,
            dns_changed_by_guardiana: false,
            home_mode: false,
            home_ip: None,
            vigilante: Vec::new(),
            lan_ports_open: Vec::new(),
            lan_ports_checked: true,
            lists: Vec::new(),
            ledger_events: 0,
            chain_ok: None,
            ledger_unreadable: false,
        }
    }

    #[test]
    fn dns_que_apunta_al_guardian_sin_copia_no_dice_que_no_ha_cambiado() {
        // El caso real del Mac del 19 sep 2026: los tres servicios de red apuntan a 127.0.0.1 y
        // `verify` respondía «DNS del sistema: -» y «Guardiana no ha cambiado el DNS del sistema»,
        // es decir, lo contrario de lo que pasaba, en la orden que existe para dar confianza.
        let mut r = informe_vacio();
        r.guardian_is_primary = Some(true);
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
    fn netstat_formats() {
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
