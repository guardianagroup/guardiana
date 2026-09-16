//! The engine: resolver + classifier + ledger + panel + retention +
//! heartbeat, assembled once and run until a shutdown signal. Used by
//! `guardiana observe` (foreground, Ctrl+C) and by the service (SCM or
//! systemd stop).

use std::collections::HashMap;
use std::error::Error;
use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use guardiana_classify::{Classified, Classifier, Input};
use guardiana_core::i18n::{self, Texts};
use guardiana_core::rules::{self, RuleInput};
use guardiana_core::time::now_ms;
use guardiana_core::{identity, paths, DecidedBy, Ledger, NewEvent, Retention, Rule, Verdict};
use guardiana_dns::{self as dns, BlockMode, Config, Decision, Outcome, Policy, Query};
use guardiana_lists::Catalog;
use guardiana_service::home::{self, SETTING_HOME_IP, SETTING_HOME_MODE};
use guardiana_service::sysdns::{self, Backup, SETTING_BACKUP};

/// Settings key: `nxdomain` (default) or `zero` (brief §4).
pub const SETTING_BLOCK_MODE: &str = "block_mode";

/// How the engine should run.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// DNS listen address on loopback (the LAN address is added in Home Mode).
    pub listen: SocketAddr,
    /// Upstream resolvers given explicitly; empty means detect.
    pub upstreams: Vec<SocketAddr>,
    /// Ledger path.
    pub db: PathBuf,
    /// Serve the panel.
    pub panel: bool,
    /// Loopback address of the panel.
    pub panel_listen: SocketAddr,
    /// Open the panel in the browser at start (foreground only).
    pub open_browser: bool,
    /// Send the six probe queries after start.
    pub self_test: bool,
    /// Force the Firefox canary on (Home Mode turns it on anyway).
    pub canary: bool,
    /// Print lines for every query (foreground) or stay quiet (service).
    pub print_events: bool,
}

impl EngineConfig {
    /// Defaults: port 53 on loopback, detect upstream, panel on, quiet off.
    #[must_use]
    pub fn default_service() -> Self {
        Self {
            listen: "127.0.0.1:53"
                .parse()
                .unwrap_or_else(|_| SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, 53))),
            upstreams: Vec::new(),
            db: paths::ledger_path(),
            panel: true,
            panel_listen: "127.0.0.1:7443"
                .parse()
                .unwrap_or_else(|_| SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, 7443))),
            open_browser: false,
            self_test: false,
            canary: false,
            print_events: false,
        }
    }
}

/// Rules loaded from the ledger, refreshed when their version changes.
struct RuleCache {
    rules: Vec<Rule>,
    version: u64,
    checked: Instant,
}

struct Inner {
    ledger: Ledger,
    classifier: Classifier,
    /// Device id → first time seen, for the 24-hour gate.
    devices_seen: HashMap<String, i64>,
    rules: RuleCache,
    recorded: u64,
}

/// What `decide` worked out, kept until `record` writes the event.
struct Pending {
    device_id: String,
    ip: String,
    classified: Classified,
}

type PendingKey = (SocketAddr, String, String, i64);

struct EnginePolicy {
    inner: Mutex<Inner>,
    pending: Mutex<HashMap<PendingKey, Pending>>,
    devices: guardiana_devices::Resolver,
    texts: &'static Texts,
    print_events: bool,
}

fn key_of(q: &Query) -> PendingKey {
    (q.client, q.name.clone(), q.qtype.to_string(), q.ts)
}

impl Inner {
    fn refresh_rules(&mut self) {
        if self.rules.checked.elapsed() < Duration::from_secs(2) {
            return;
        }
        self.rules.checked = Instant::now();
        let version = self.ledger.rules_version().unwrap_or(0);
        if version != self.rules.version {
            if let Ok(all) = self.ledger.rules() {
                self.rules.rules = all;
                self.rules.version = version;
            }
        }
    }
}

impl Policy for EnginePolicy {
    fn decide(&self, q: &Query) -> Decision {
        // Identify the device before taking the ledger lock: the neighbour
        // table read can take a few milliseconds the first time.
        let who = self.devices.identify(q.client.ip());
        let Ok(mut inner) = self.inner.lock() else {
            return Decision::Forward;
        };
        let ip = q.client.ip().to_string();
        let device_id = who.id;
        let first_seen = match inner.devices_seen.get(&device_id) {
            Some(t) => *t,
            None => {
                let first = inner
                    .ledger
                    .upsert_device(&device_id, who.mac.as_deref(), Some(&ip), q.ts)
                    .map_or(q.ts, |d| d.first_seen);
                inner.devices_seen.insert(device_id.clone(), first);
                first
            }
        };
        let first_time = inner
            .ledger
            .first_time(&device_id, &q.name, q.ts)
            .unwrap_or(false);
        let classified = inner.classifier.classify(&Input {
            device_id: &device_id,
            name: &q.name,
            ts: q.ts,
            first_time,
        });
        inner.refresh_rules();
        let decision = match rules::decide(
            &inner.rules.rules,
            RuleInput {
                device_id: &device_id,
                name: &q.name,
                category: classified.category,
                observed_ms: q.ts - first_seen,
            },
            q.ts,
        ) {
            Some(rule) if rule.action == guardiana_core::Action::Cortar => Decision::Block {
                rule_id: Some(rule.id),
            },
            _ => Decision::Forward,
        };
        drop(inner);
        if let Ok(mut pending) = self.pending.lock() {
            pending.insert(
                key_of(q),
                Pending {
                    device_id,
                    ip,
                    classified,
                },
            );
            // Never let a lost `record` grow the map without bound.
            if pending.len() > 10_000 {
                pending.clear();
            }
        }
        decision
    }

    fn record(&self, q: &Query, outcome: Outcome) {
        let (verdict, decided_by, rule_id) = match outcome {
            Outcome::Refused => return,
            Outcome::Forwarded { .. } | Outcome::UpstreamFailed => {
                (Verdict::Observado, DecidedBy::Nadie, None)
            }
            Outcome::Canary | Outcome::Checker => (Verdict::Respondido, DecidedBy::Nadie, None),
            Outcome::Blocked { rule_id } => (Verdict::Cortado, DecidedBy::ReglaUsuario, rule_id),
        };
        let Some(p) = self
            .pending
            .lock()
            .ok()
            .and_then(|mut m| m.remove(&key_of(q)))
        else {
            return;
        };
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };
        let mut event =
            NewEvent::observed(q.ts, &p.device_id, &p.ip, &q.name, &q.qtype.to_string());
        event.category = p.classified.category;
        event.list_source = p.classified.list_source;
        event.signals = p.classified.signals;
        event.verdict = verdict;
        event.decided_by = decided_by;
        event.rule_id = rule_id;
        match inner.ledger.append(event) {
            Ok(e) => {
                if self.print_events {
                    println!("{}", crate::show::line(self.texts, &e, None));
                }
            }
            Err(e) => eprintln!("guardiana: {e}"),
        }
        inner.recorded += 1;
        if inner.recorded % 50 == 0 {
            let dirty = inner.classifier.take_dirty_profiles();
            for (id, json) in dirty {
                let _ = inner.ledger.set_hours_profile(&id, &json);
            }
        }
    }
}

/// Why one pass of the engine ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Exit {
    /// The caller asked to stop.
    Shutdown,
    /// The home LAN appeared, went away or changed address, or Home Mode was
    /// switched: the listeners are rebuilt without stopping the process.
    Reconfigure,
}

/// Run the engine until `shutdown` resolves. Must be called inside a tokio
/// runtime. Listeners are rebuilt whenever the home LAN changes (found in the
/// reboot test: the service starts before the Wi-Fi has an address, so the
/// LAN ports must open later, not only at start).
pub async fn run<F: Future<Output = ()>>(
    cfg: EngineConfig,
    shutdown: F,
) -> Result<(), Box<dyn Error>> {
    tokio::pin!(shutdown);
    let mut first = true;
    loop {
        match run_once(&cfg, shutdown.as_mut(), first).await? {
            Exit::Shutdown => return Ok(()),
            Exit::Reconfigure => {
                println!("{}", i18n::current().cli("observe.reconfigurando"));
                first = false;
            }
        }
    }
}

/// The LAN address Home Mode should be listening on right now: `None` when
/// Home Mode is off, not allowed by the licence, or the computer has no
/// private LAN address (yet).
fn home_lan_wanted(ledger: &Ledger, secret: &str) -> Option<std::net::Ipv4Addr> {
    let on = ledger
        .setting(SETTING_HOME_MODE)
        .ok()
        .flatten()
        .is_some_and(|v| v == "1");
    if !on {
        return None;
    }
    // Home Mode is free (decision 52): the licence no longer gates the LAN.
    let _ = secret;
    guardiana_devices::local_lan_ipv4()
        .filter(|lan| guardiana_devices::is_private_lan(IpAddr::V4(*lan)))
}

/// One pass: open the ledger, build the listeners for the current network,
/// serve until shutdown or until the home LAN changes.
async fn run_once<F: Future<Output = ()>>(
    cfg: &EngineConfig,
    mut shutdown: Pin<&mut F>,
    first: bool,
) -> Result<Exit, Box<dyn Error>> {
    let t = i18n::current();
    if let Some(parent) = cfg.db.parent() {
        std::fs::create_dir_all(parent)?;
        // Token and extract readable only by the user (brief §8). Not fatal:
        // an unprivileged smoke test on a temp folder keeps working.
        let _ = paths::harden_data_dir(parent);
    }
    if first && identity::public_key_is_dev() {
        eprintln!("{}", t.cli("observe.clave_dev"));
    }
    let mut ledger = Ledger::open(&cfg.db, identity::genesis())?;
    if let Some(gap) = ledger.record_gap_since_last_heartbeat(now_ms())? {
        println!(
            "{}",
            t.cli("observe.hueco")
                .replace("{desde}", &guardiana_core::time::rfc3339_utc(gap.from_ts))
                .replace("{hasta}", &guardiana_core::time::rfc3339_utc(gap.to_ts))
        );
    }
    let home_on = ledger.setting(SETTING_HOME_MODE)?.is_some_and(|v| v == "1");

    // Home Mode needs the trial or the Home plan (brief §9): without either it
    // stays off with a notice, and the computer's own DNS never breaks.
    let secret =
        guardiana_panel::load_or_create_token(&paths::data_dir().join(guardiana_panel::TOKEN_FILE))
            .unwrap_or_default();
    // Home Mode is free (decision 52): no licence gate.
    let home_allowed = home_on;

    // Upstream: what the caller said; else the resolvers saved before Guardiana
    // changed the system DNS (brief §4); else whatever the system uses now.
    let saved: Option<Backup> = match ledger.setting(SETTING_BACKUP)? {
        Some(json) if !json.is_empty() => serde_json::from_str(&json).ok(),
        _ => None,
    };
    let upstreams: Vec<SocketAddr> = if !cfg.upstreams.is_empty() {
        cfg.upstreams.clone()
    } else if let Some(backup) = saved.filter(|b| !b.original_servers().is_empty()) {
        let servers: Vec<SocketAddr> = backup
            .original_servers()
            .into_iter()
            .map(|ip| SocketAddr::new(ip, 53))
            .collect();
        println!(
            "{}",
            t.cli("observe.upstream_copia").replace(
                "{servers}",
                &servers
                    .iter()
                    .map(|s| s.ip().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        );
        servers
    } else {
        let current = sysdns::current_resolvers().ok_or_else(|| t.cli("observe.sin_upstream"))?;
        println!(
            "{}",
            t.cli("observe.upstream_auto")
                .replace("{fuente}", &format!("{:?}", current.source))
        );
        current.servers
    };
    if upstreams.contains(&cfg.listen) {
        return Err(t.cli("observe.bucle").into());
    }

    let mut classifier = Classifier::new(Arc::new(Catalog::bundled()));
    for d in ledger.devices()? {
        if let Some(json) = d.hours_profile_json.as_deref() {
            let _ = classifier.load_profile(&d.id, json);
        }
    }
    let block_mode = match ledger.setting(SETTING_BLOCK_MODE)?.as_deref() {
        Some("zero") => BlockMode::ZeroIp,
        _ => BlockMode::NxDomain,
    };
    let initial_rules = ledger.rules().unwrap_or_default();
    let initial_version = ledger.rules_version().unwrap_or(0);
    let policy = EnginePolicy {
        inner: Mutex::new(Inner {
            ledger,
            classifier,
            devices_seen: HashMap::new(),
            rules: RuleCache {
                rules: initial_rules,
                version: initial_version,
                checked: Instant::now(),
            },
            recorded: 0,
        }),
        pending: Mutex::new(HashMap::new()),
        devices: guardiana_devices::Resolver::default(),
        texts: t,
        print_events: cfg.print_events,
    };

    let mut dns_cfg = Config::local(upstreams.clone());
    dns_cfg.listen = vec![cfg.listen];
    dns_cfg.canary_enabled = cfg.canary;
    dns_cfg.block_mode = block_mode;

    // Home Mode (brief §7): also listen on the private LAN address, answer the
    // checker name with it, and turn the Firefox canary on by default.
    let mut home_lan: Option<std::net::Ipv4Addr> = None;
    let mut panel_listen: Vec<SocketAddr> = vec![cfg.panel_listen];
    let mut panel_optional: Vec<SocketAddr> = Vec::new();
    if home_allowed {
        match guardiana_devices::local_lan_ipv4() {
            Some(lan) if guardiana_devices::is_private_lan(IpAddr::V4(lan)) => {
                let (dns_addr, panel_addr) = home::lan_listen_addrs(lan);
                let dns_addr = SocketAddr::new(dns_addr.ip(), cfg.listen.port());
                dns_cfg.listen.push(dns_addr);
                dns_cfg.checker_ip = Some(lan);
                dns_cfg.canary_enabled = true;
                panel_listen.push(panel_addr);
                panel_optional.push(home::lan_checker_addr(lan));
                home_lan = Some(lan);
                println!(
                    "{}",
                    t.cli("observe.hogar")
                        .replace("{addr}", &dns_addr.to_string())
                        .replace("{panel}", &panel_addr.to_string())
                );
            }
            Some(lan) => println!(
                "{}",
                t.cli("observe.hogar_no_privada")
                    .replace("{ip}", &lan.to_string())
            ),
            None => println!("{}", t.panel("hogar_sin_lan")),
        }
    }

    // The address Home Mode is bound to is what the panel, the QR and
    // `hogar status` must show: keep the setting in step when the LAN moved
    // (Wi-Fi to Ethernet, new DHCP lease) since it was switched on.
    if let Some(lan) = home_lan {
        if let Ok(guard) = policy.inner.lock() {
            let current = guard.ledger.setting(SETTING_HOME_IP).ok().flatten();
            if current.as_deref() != Some(&lan.to_string()) {
                let _ = guard.ledger.set_setting(SETTING_HOME_IP, &lan.to_string());
            }
        }
    }

    let panel_cfg = cfg.panel.then(|| guardiana_panel::Config {
        listen: panel_listen,
        optional_listen: panel_optional,
        db_path: cfg.db.clone(),
        genesis: identity::genesis(),
        token_path: paths::data_dir().join(guardiana_panel::TOKEN_FILE),
        extra_hosts: home_lan.iter().map(ToString::to_string).collect(),
        info: guardiana_panel::RuntimeInfo {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            listen_dns: dns_cfg.listen.iter().map(ToString::to_string).collect(),
            upstream: upstreams.iter().map(ToString::to_string).collect(),
            dev_key: identity::public_key_is_dev(),
        },
    });

    let running = match dns::start(dns_cfg, policy).await {
        Ok(r) => r,
        Err(dns::Error::Bind(addr, e)) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("{}", t.cli("observe.sin_privilegios"));
            return Err(Box::<dyn Error>::from(dns::Error::Bind(addr, e)));
        }
        Err(e) => return Err(Box::<dyn Error>::from(e)),
    };
    let ups = upstreams
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    println!(
        "{}",
        t.cli("observe.escuchando")
            .replace("{addr}", &running.udp_addrs[0].to_string())
            .replace("{upstream}", &ups)
    );
    if cfg!(target_os = "macos") {
        println!("{}", t.cli("observe.limite_mac"));
    }

    // The panel runs in the same process, with its own ledger connection.
    let panel = match panel_cfg {
        Some(pc) => match guardiana_panel::start(pc).await {
            Ok(p) => {
                println!("{}", t.cli("observe.panel").replace("{url}", &p.url()));
                println!("{}", t.cli("observe.panel_nota"));
                for (addr, why) in &p.failed_optional {
                    println!(
                        "{}",
                        t.cli("observe.puerto80")
                            .replace("{addr}", &addr.to_string())
                            .replace("{motivo}", why)
                    );
                }
                if first && cfg.open_browser {
                    open_in_browser(&p.url());
                }
                Some(p)
            }
            Err(e) => {
                eprintln!(
                    "{}",
                    t.cli("observe.panel_error")
                        .replace("{error}", &e.to_string())
                );
                None
            }
        },
        None => None,
    };

    // Housekeeping: heartbeat every 30 s, retention pass hourly (brief §3),
    // and, while the system DNS is changed, re-apply it if the network
    // configuration dropped it (brief §4: "reaplicar si cambia la red").
    if first {
        println!("{}", t.cli("observe.retencion"));
    }
    let keep_db = cfg.db.clone();
    let keep_dir = cfg.db.parent().map(std::path::Path::to_path_buf);
    let keep_secret = secret.clone();
    let (reconfigure_tx, mut reconfigure) = tokio::sync::oneshot::channel::<()>();
    let housekeeping = tokio::spawn(async move {
        let mut reconfigure_tx = Some(reconfigure_tx);
        let mut beat = tokio::time::interval(Duration::from_secs(30));
        let mut lan_check = tokio::time::interval(Duration::from_secs(15));
        let mut minute = tokio::time::interval(Duration::from_secs(60));
        let mut ten_min = tokio::time::interval(Duration::from_secs(600));
        let mut hour = tokio::time::interval(Duration::from_secs(3600));
        beat.tick().await;
        lan_check.tick().await;
        minute.tick().await;
        ten_min.tick().await;
        hour.tick().await;
        loop {
            tokio::select! {
                _ = beat.tick() => {
                    if let Ok(l) = Ledger::open(&keep_db, identity::genesis()) {
                        let _ = l.heartbeat(now_ms());
                    }
                }
                _ = lan_check.tick() => {
                    // Home LAN appeared, vanished, changed address, or Home
                    // Mode was switched: ask for the listeners to be rebuilt.
                    if reconfigure_tx.is_some() {
                        if let Ok(l) = Ledger::open(&keep_db, identity::genesis()) {
                            if home_lan_wanted(&l, &keep_secret) != home_lan {
                                if let Some(tx) = reconfigure_tx.take() {
                                    let _ = tx.send(());
                                }
                            }
                        }
                    }
                }
                _ = minute.tick() => {
                    if let Ok(l) = Ledger::open(&keep_db, identity::genesis()) {
                        reapply_dns_if_dropped(&l);
                    }
                }
                _ = ten_min.tick() => {
                    // On Windows the console user may have logged in after the
                    // service started: give them access to their own data.
                    if let Some(dir) = &keep_dir {
                        let _ = paths::harden_data_dir(dir);
                    }
                }
                _ = hour.tick() => {
                    if let Ok(mut l) = Ledger::open(&keep_db, identity::genesis()) {
                        // The once-per-period check of a Plus key (decision 52) and
                        // the retention of the plan in force: Plus keeps everything,
                        // free keeps 24 h of detail and 7 days of totals.
                        let _ = guardiana_license::check_if_due(&mut l, &keep_secret, now_ms());
                        let plus = guardiana_license::status(&l, &keep_secret, now_ms())
                            .map(|s| s.plus_activo)
                            .unwrap_or(false);
                        let policy = if plus { Retention::UNLIMITED } else { Retention::FREE };
                        let _ = l.prune(policy, now_ms());
                    }
                }
            }
        }
    });

    println!("{}", t.cli("observe.arrancando"));
    if first && cfg.self_test {
        let target = running.udp_addrs[0];
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(500)).await;
            println!("{}", t.cli("menu.self_test"));
            for name in crate::menu::SELF_TEST_NAMES {
                let _ = guardiana_dns::probe::probe(target, name, Duration::from_secs(3)).await;
            }
            println!("{}", t.cli("menu.self_test_hecho"));
        });
    }

    let exit = tokio::select! {
        _ = &mut shutdown => Exit::Shutdown,
        _ = &mut reconfigure => Exit::Reconfigure,
    };

    housekeeping.abort();
    if let Some(p) = panel {
        p.shutdown().await;
    }
    running.shutdown();
    let _ = running.wait().await;
    Ok(exit)
}

/// While a DNS backup exists, make sure the guardian is still first; the
/// network stack (DHCP renew, NetworkManager) can silently put the old
/// servers back.
fn reapply_dns_if_dropped(ledger: &Ledger) {
    let Ok(Some(json)) = ledger.setting(SETTING_BACKUP) else {
        return;
    };
    if json.is_empty() {
        return;
    }
    let Ok(backup) = serde_json::from_str::<Backup>(&json) else {
        return;
    };
    if sysdns::guardian_is_primary() == Some(false) {
        let _ = sysdns::apply(&backup, IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
    }
}

/// Open the panel URL in the default browser, best effort.
pub fn open_in_browser(url: &str) {
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).spawn();
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let result = std::process::Command::new("xdg-open").arg(url).spawn();
    let _ = result;
}
