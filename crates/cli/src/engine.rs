//! The engine: resolver + classifier + ledger + panel + retention +
//! heartbeat, assembled once and run until a shutdown signal. Used by
//! `guardiana observe` (foreground, Ctrl+C) and by the service (SCM or
//! systemd stop).

use std::collections::{HashMap, VecDeque};
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
use guardiana_core::{identity, paths, DecidedBy, Ledger, NewEvent, Rule, Verdict};
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
    /// Declared scope per device: whether to cut what is outside it, and the patterns.
    /// Read from the settings and refreshed on the same 2-second beat as the rules,
    /// because `decide` runs on the resolver's hot path and must not touch the disk
    /// on every query.
    alcances: HashMap<String, Alcance>,
    alcances_vistos: Option<Instant>,
}

/// What a device's declared scope says (decision 154).
#[derive(Clone, Default)]
struct Alcance {
    /// The user asked for what falls outside to be cut, not just shown.
    cortar: bool,
    /// Domains allowed for this device; a name matches if it is one of them or under it.
    patrones: Vec<String>,
}

impl Alcance {
    /// Whether the name falls inside the declared scope.
    fn cubre(&self, name: &str) -> bool {
        let n = name.trim_end_matches('.').to_ascii_lowercase();
        self.patrones
            .iter()
            .any(|p| n == *p || n.ends_with(&format!(".{p}")))
    }
}

/// What `decide` worked out, kept until `record` writes the event.
struct Pending {
    device_id: String,
    ip: String,
    classified: Classified,
    /// Cut because it fell outside the declared scope, not because of a rule.
    fuera_de_alcance: bool,
}

type PendingKey = (SocketAddr, String, String, i64);

/// How the upstreams are doing, counted by the policy as answers come back: forwarded queries
/// that got any answer, and queries the upstreams never answered. Read and reset once a minute.
#[derive(Default)]
struct Salud {
    ok: std::sync::atomic::AtomicU64,
    fallos: std::sync::atomic::AtomicU64,
}

impl Salud {
    fn anota(&self, outcome: Outcome) {
        use std::sync::atomic::Ordering::Relaxed;
        match outcome {
            Outcome::Forwarded { .. } => {
                self.ok.fetch_add(1, Relaxed);
            }
            Outcome::UpstreamFailed => {
                self.fallos.fetch_add(1, Relaxed);
            }
            _ => {}
        }
    }

    /// The minute's numbers, and back to zero.
    fn minuto(&self) -> (u64, u64) {
        use std::sync::atomic::Ordering::Relaxed;
        (self.ok.swap(0, Relaxed), self.fallos.swap(0, Relaxed))
    }
}

/// A minute in which the upstreams answered nothing at all, with enough queries to mean it.
/// Three is the floor: one lost packet is not a dead network, and a quiet minute (nobody asked)
/// says nothing either way.
fn minuto_sin_arriba(ok: u64, fallos: u64) -> bool {
    ok == 0 && fallos >= 3
}

/// Stepping aside when the upstreams go quiet, and coming back when they answer.
///
/// The guardian is the machine's resolver; when what it forwards to stops answering (a laptop
/// that woke up on another network, a router that died, a captive portal), every lookup on the
/// machine fails, which the person experiences as "no internet". After a minute in which the
/// upstreams answered nothing, the guardian asks them one name itself; if that goes unanswered
/// too, it gives the DNS back exactly as it was, writes it in the ledger, and then asks once a
/// minute until one answers, when it takes the DNS again. The owner's own Mac spent a day like
/// this on 1 Oct 2026, with nothing telling him why.
#[derive(Default)]
struct Aparte {
    apartado: bool,
}

/// What the minute asks the engine to do.
#[derive(Debug, PartialEq, Eq)]
enum Paso {
    Nada,
    /// A silent minute: ask the upstreams directly, and step aside if they stay silent. The
    /// question is what decides: a machine that only asked for names that do not resolve is
    /// not a machine without a network.
    Comprobar,
    /// Already aside: ask whether the upstreams answer again, and if so take the DNS back.
    Sondear,
}

impl Aparte {
    fn minuto(&self, ok: u64, fallos: u64) -> Paso {
        if self.apartado {
            Paso::Sondear
        } else if minuto_sin_arriba(ok, fallos) {
            Paso::Comprobar
        } else {
            Paso::Nada
        }
    }
}

/// How long the guardian waits for the upstreams when it asks them itself.
const SONDA: Duration = Duration::from_secs(2);

struct EnginePolicy {
    inner: Mutex<Inner>,
    pending: Mutex<HashMap<PendingKey, Pending>>,
    /// Answers and silences of the upstreams, for the minute watch that steps aside.
    salud: Arc<Salud>,
    devices: guardiana_devices::Resolver,
    texts: &'static Texts,
    print_events: bool,
    /// Quién pidió cada nombre, cuando el sistema lo dice (Windows, este equipo). `None` en los
    /// demás sistemas y también en Windows si la sesión de sucesos no se pudo abrir: entonces el
    /// extracto queda como siempre, con el aparato y sin el programa.
    apps: Option<Arc<guardiana_apps::Observador>>,
    /// Consultas de este equipo esperando a saber qué programa las pidió.
    ///
    /// Windows entrega los sucesos por tandas, con un temporizador cuyo mínimo es **un segundo**,
    /// y Guardiana anota la consulta a los pocos milisegundos: cuando llega el aviso de quién
    /// preguntó, la fila ya estaría escrita, y una fila escrita no se toca —la cadena de hashes
    /// existe justamente para eso—. Así que la anotación de este equipo espera un momento en esta
    /// cola, en el mismo orden en que llegó, y se escribe cuando ya se puede decir quién fue.
    /// Lo que no espera es la respuesta al programa, que sale igual de rápido que siempre.
    cola_apps: Arc<Mutex<VecDeque<guardiana_core::NewEvent>>>,
}

/// Cuánto espera una consulta de este equipo antes de anotarse, para darle tiempo a Windows a
/// decir quién la pidió. Un poco más que el segundo del temporizador de sucesos.
const ESPERA_APPS_MS: i64 = 1_300;

fn key_of(q: &Query) -> PendingKey {
    (q.client, q.name.clone(), q.qtype.to_string(), q.ts)
}

impl Inner {
    /// The declared scope of one device, from the settings, re-read at most every two
    /// seconds. A device with no scope written gets an empty one, which never cuts.
    fn alcance_de(&mut self, device_id: &str) -> Alcance {
        let caducado = self
            .alcances_vistos
            .is_none_or(|t| t.elapsed() >= Duration::from_secs(2));
        if caducado {
            self.alcances.clear();
            self.alcances_vistos = Some(Instant::now());
        }
        if let Some(a) = self.alcances.get(device_id) {
            return a.clone();
        }
        let patrones = self
            .ledger
            .setting(&format!("alcance:{device_id}"))
            .unwrap_or_default()
            .unwrap_or_default()
            .lines()
            .map(|x| x.trim().to_ascii_lowercase())
            .filter(|x| !x.is_empty())
            .collect();
        // The mode is "cortar", "observar", or a temporary pass: "observar:<ms>", which is
        // for the person who sends a long job and leaves. Until that moment nothing is cut,
        // so the work does not die halfway with nobody there to answer; after it, the cut is
        // back by itself. A pass that has to be turned off by hand is a pass somebody forgets.
        let modo = self
            .ledger
            .setting(&format!("alcance_modo:{device_id}"))
            .unwrap_or_default()
            .unwrap_or_default();
        let cortar = match modo.split_once(':') {
            Some(("observar", hasta)) => hasta
                .parse::<i64>()
                .is_ok_and(|t| guardiana_core::time::now_ms() >= t),
            _ => modo == "cortar",
        };
        let a = Alcance { cortar, patrones };
        self.alcances.insert(device_id.to_owned(), a.clone());
        a
    }

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
        // The declared scope, when the person asked for it to cut (decision 154). It only
        // applies where a cut already applies: an explicit rule wins, a rule that allows
        // wins, and nothing is cut on a device with less than 24 hours observed. The name
        // is not resolved, so the connection never starts; the panel then asks whether to
        // add it to the scope. It cannot undo what was already sent, and it does not reach
        // an agent that skips this resolver.
        let mut fuera_de_alcance = false;
        let decision = if matches!(decision, Decision::Forward) {
            let alcance = inner.alcance_de(&device_id);
            if alcance.cortar
                && !alcance.patrones.is_empty()
                && !alcance.cubre(&q.name)
                && guardiana_core::rules::observation_complete(q.ts - first_seen)
            {
                fuera_de_alcance = true;
                Decision::Block { rule_id: None }
            } else {
                decision
            }
        } else {
            decision
        };
        drop(inner);
        if let Ok(mut pending) = self.pending.lock() {
            pending.insert(
                key_of(q),
                Pending {
                    device_id,
                    ip,
                    classified,
                    fuera_de_alcance,
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
        self.salud.anota(outcome);
        let (verdict, mut decided_by, rule_id) = match outcome {
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
        // A cut with no rule behind it came from the declared scope: the ledger has to say
        // so, because the person never named this destination.
        if p.fuera_de_alcance && verdict == Verdict::Cortado {
            decided_by = DecidedBy::AlcanceDeclarado;
        }
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
        // Solo para este equipo: de un teléfono en Modo Hogar se ve el nombre y nada más, y eso
        // lo dice cada pantalla. Si el sistema no lo dijo, el hueco se queda vacío.
        if p.device_id == guardiana_core::SELF_DEVICE_ID {
            if let Some(cola) = self.apps.as_ref().and(Some(&self.cola_apps)) {
                // A la cola, en orden. Quien la vacía pregunta por el programa y anota.
                if let Ok(mut c) = cola.lock() {
                    if c.len() < 10_000 {
                        c.push_back(event);
                        return;
                    }
                }
            }
        }
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
    /// The trial or the subscription ended while running: stand down and come back as the
    /// stopped program (panel only), without killing the process or the machine's DNS.
    Caducado,
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
    let result = loop {
        match run_once(&cfg, shutdown.as_mut(), first).await {
            Ok(Exit::Shutdown) => break Ok(()),
            Ok(Exit::Reconfigure) => {
                println!("{}", i18n::current().cli("observe.reconfigurando"));
                first = false;
            }
            Ok(Exit::Caducado) => first = false,
            Err(e) => break Err(e),
        }
    };
    give_dns_back_while_stopped(&cfg.db);
    result
}

/// While it runs, Guardiana is the machine's resolver, so a stopped service must not leave the
/// machine pointing at a port nobody answers: that is a machine without internet. The old DNS
/// goes back, the backup stays, and the next start points the machine at Guardiana again
/// (`run_once`, right after the resolver is listening).
///
/// Until 1 Oct 2026 this ran on Windows only. On Linux (resolved drop-in or a rewritten
/// resolv.conf) Guardiana is the only resolver just the same, and `systemctl stop guardiana`
/// left Ubuntu without names while the screen said "Servicio parado." On a Mac the original
/// servers stay behind 127.0.0.1 as a fallback, so it limps instead of dying, but every lookup
/// waits for a timeout first. A crash does not reach this function; the next start re-applies.
fn give_dns_back_while_stopped(db: &std::path::Path) {
    let Ok(ledger) = Ledger::open(db, identity::genesis()) else {
        return;
    };
    let Ok(Some(json)) = ledger.setting(SETTING_BACKUP) else {
        return;
    };
    if json.is_empty() {
        return;
    }
    if let Ok(backup) = serde_json::from_str::<Backup>(&json) {
        let _ = sysdns::restore(&backup);
    }
}

/// The LAN address Home Mode should be listening on right now: `None` when
/// Home Mode is off, not allowed by the licence, or the computer has no
/// private LAN address (yet).
/// How a cut name is answered, as the panel last saved it (Rules › how to answer).
fn block_mode_of(ledger: &Ledger) -> BlockMode {
    match ledger.setting(SETTING_BLOCK_MODE).ok().flatten().as_deref() {
        Some("zero") => BlockMode::ZeroIp,
        _ => BlockMode::NxDomain,
    }
}

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

/// Stand down: the trial or the subscription is over.
///
/// The upstreams of this pass, as the person reads them.
fn lista_de(upstreams: &[SocketAddr]) -> String {
    upstreams
        .iter()
        .map(|s| s.ip().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The upstreams are silent: give the system DNS back from the saved copy and write it down.
/// The copy stays (this is not the end of anything: the guardian comes back when they answer).
/// `true` when this call undid the change; `false` when there was no copy (nothing of ours to
/// undo), the machine already pointed elsewhere, or the undo failed -- then the next minute
/// tries again, because the machine is still pointing at a resolver that cannot answer.
fn devolver_por_silencio(ledger: &Ledger, lista: &str) -> bool {
    let Ok(Some(json)) = ledger.setting(SETTING_BACKUP) else {
        return false;
    };
    if json.is_empty() {
        return false;
    }
    let Ok(backup) = serde_json::from_str::<Backup>(&json) else {
        return false;
    };
    if sysdns::guardian_is_primary() == Some(false) || sysdns::restore(&backup).is_err() {
        return false;
    }
    let _ = ledger.record_change(
        now_ms(),
        guardiana_core::ChangeKind::DnsOff,
        guardiana_core::ChangeWho::SinArriba,
        lista,
    );
    true
}

/// An upstream answers again: point the machine at the guardian as before. `true` when done,
/// or when there is no copy to apply (the person undid the change meanwhile, or never made it).
fn volver_con_arriba(ledger: &Ledger) -> bool {
    let t = i18n::current();
    let copia = match ledger.setting(SETTING_BACKUP) {
        Ok(Some(json)) if !json.is_empty() => serde_json::from_str::<Backup>(&json).ok(),
        Ok(_) => {
            println!("{}", t.cli("observe.vuelve_arriba"));
            return true;
        }
        Err(_) => return false,
    };
    let Some(backup) = copia else {
        return false;
    };
    if sysdns::apply(&backup, IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)).is_err() {
        return false;
    }
    let _ = ledger.record_change(
        now_ms(),
        guardiana_core::ChangeKind::DnsOn,
        guardiana_core::ChangeWho::ConArriba,
        "",
    );
    println!("{}", t.cli("observe.vuelve_arriba"));
    true
}

/// The program stops being the guardian, but it must never leave the machine without a resolver:
/// the system DNS goes back to exactly what it was before Guardiana touched it, Home Mode is
/// switched off so nothing on the LAN is left pointing at a listener that is closing, and both
/// changes are written into the ledger like any other. The extract stays on the disk, whole: it
/// belongs to the person, whatever they decide about paying.
fn stand_down(ledger: &Ledger) {
    if let Ok(Some(json)) = ledger.setting(SETTING_BACKUP) {
        if !json.is_empty() {
            if let Ok(backup) = serde_json::from_str::<Backup>(&json) {
                // The copy is cleared only once the undo really happened. It used to be cleared
                // first: one interface that no longer exists (the trip's VPN) made restore()
                // fail, and the only record of the previous DNS was gone -- a machine pointing
                // at a guardian that had stepped aside, and a panel saying there was nothing to
                // undo. Found in the review of 1 Oct 2026. The other three undo paths already
                // checked first; this was the one that did not.
                if sysdns::restore(&backup).is_ok() {
                    let _ = ledger.set_setting(SETTING_BACKUP, "");
                    let _ = ledger.record_change(
                        now_ms(),
                        guardiana_core::ChangeKind::DnsOff,
                        guardiana_core::ChangeWho::Licencia,
                        "",
                    );
                }
            }
        }
    }
    if ledger.setting(SETTING_HOME_MODE).ok().flatten().as_deref() == Some("1") {
        let _ = ledger.set_setting(SETTING_HOME_MODE, "0");
        let _ = ledger.record_change(
            now_ms(),
            guardiana_core::ChangeKind::HogarOff,
            guardiana_core::ChangeWho::Licencia,
            "",
        );
    }
}

/// Whether the program may watch right now: the trial or a subscription is on.
fn puede_funcionar(ledger: &Ledger, secret: &str) -> bool {
    guardiana_license::status(ledger, secret, now_ms())
        .map(|s| s.puede_funcionar)
        .unwrap_or(true)
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
    } else if let Some(servers) = saved
        .as_ref()
        .map(sysdns::upstreams_for)
        .filter(|v| !v.is_empty())
    {
        let servers: Vec<SocketAddr> = servers
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

    // La prueba o la suscripción terminaron: Guardiana se aparta. No abre el resolutor, devuelve
    // el DNS del sistema a como estaba y apaga el Modo Hogar; solo queda el panel en pie para que
    // se pueda activar Plus o llevarse el extracto. Nunca se queda en medio sin resolver: eso
    // dejaría el equipo sin internet el día que caduca una suscripción.
    if !puede_funcionar(&ledger, &secret) {
        stand_down(&ledger);
        println!("{}", t.cli("observe.caducado"));
        let panel_cfg = cfg.panel.then(|| guardiana_panel::Config {
            listen: vec![cfg.panel_listen],
            optional_listen: Vec::new(),
            db_path: cfg.db.clone(),
            genesis: identity::genesis(),
            token_path: paths::data_dir().join(guardiana_panel::TOKEN_FILE),
            extra_hosts: Vec::new(),
            info: guardiana_panel::RuntimeInfo {
                version: env!("CARGO_PKG_VERSION").to_owned(),
                listen_dns: Vec::new(),
                upstream: Vec::new(),
                dev_key: identity::public_key_is_dev(),
            },
        });
        let panel = match panel_cfg {
            Some(pc) => match guardiana_panel::start(pc).await {
                Ok(p) => {
                    println!("{}", t.cli("observe.panel").replace("{url}", &p.url()));
                    if first && cfg.open_browser {
                        open_in_browser(&p.url());
                    }
                    Some(p)
                }
                Err(_) => None,
            },
            None => None,
        };
        shutdown.as_mut().await;
        if let Some(p) = panel {
            p.shutdown().await;
        }
        return Ok(Exit::Shutdown);
    }

    let mut classifier = Classifier::new(Arc::new(Catalog::bundled()));
    for d in ledger.devices()? {
        if let Some(json) = d.hours_profile_json.as_deref() {
            let _ = classifier.load_profile(&d.id, json);
        }
    }
    let block_mode = block_mode_of(&ledger);
    let initial_rules = ledger.rules().unwrap_or_default();
    let initial_version = ledger.rules_version().unwrap_or(0);
    let salud = Arc::new(Salud::default());
    let policy = EnginePolicy {
        salud: Arc::clone(&salud),
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
            alcances: HashMap::new(),
            alcances_vistos: None,
        }),
        pending: Mutex::new(HashMap::new()),
        devices: guardiana_devices::Resolver::default(),
        texts: t,
        print_events: cfg.print_events,
        apps: match guardiana_apps::Observador::arrancar() {
            Ok(o) => Some(Arc::new(o)),
            Err(guardiana_apps::Error::NoSoportado) => None,
            Err(e) => {
                // Se dice y se sigue: saber qué programa pidió cada nombre es un extra, y sin él
                // Guardiana hace exactamente lo que hacía antes.
                eprintln!("guardiana: {e}");
                None
            }
        },
        cola_apps: Arc::default(),
    };

    let mut dns_cfg = Config::local(upstreams.clone());
    dns_cfg.listen = vec![cfg.listen];
    // The IPv6 loopback too, when the machine has it: Windows asks its IPv6 resolvers first,
    // and Guardiana is the first of those (sysdns::windows). Where it cannot be bound, IPv4
    // alone keeps working.
    if cfg.listen.ip().is_loopback() {
        dns_cfg.optional_listen = vec![SocketAddr::new(
            IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
            cfg.listen.port(),
        )];
    }
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

    // Quien vacía la cola: cada poco, escribe las consultas de este equipo que ya han esperado
    // bastante, preguntando primero qué programa las pidió. Va en orden y con su propia conexión
    // al extracto, como hace el resto de tareas de fondo.
    // Una copia del asa de la cola para poder vaciarla al cerrar, cuando la política ya se la ha
    // llevado el resolutor.
    let policy_cola = Arc::clone(&policy.cola_apps);
    let vaciador = policy.apps.as_ref().map(|obs| {
        let obs = Arc::clone(obs);
        let cola = Arc::clone(&policy.cola_apps);
        let db = cfg.db.clone();
        let imprimir = cfg.print_events;
        tokio::spawn(async move {
            let mut cada = tokio::time::interval(Duration::from_millis(250));
            loop {
                cada.tick().await;
                let ahora = now_ms();
                let mut listos: Vec<guardiana_core::NewEvent> = Vec::new();
                if let Ok(mut c) = cola.lock() {
                    while let Some(e) = c.front() {
                        if ahora - e.ts >= ESPERA_APPS_MS {
                            if let Some(e) = c.pop_front() {
                                listos.push(e);
                            }
                        } else {
                            break;
                        }
                    }
                }
                if listos.is_empty() {
                    continue;
                }
                let Ok(mut l) = Ledger::open(&db, identity::genesis()) else {
                    continue;
                };
                for mut e in listos {
                    e.process = obs.quien_pidio(&e.qname, e.ts);
                    match l.append(e) {
                        Ok(anotado) => {
                            if imprimir {
                                println!("{}", crate::show::line(t, &anotado, None));
                            }
                        }
                        Err(err) => eprintln!("guardiana: {err}"),
                    }
                }
            }
        })
    });

    let running = match dns::start(dns_cfg, policy).await {
        Ok(r) => r,
        Err(dns::Error::Bind(addr, e)) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("{}", t.cli("observe.sin_privilegios"));
            return Err(Box::<dyn Error>::from(dns::Error::Bind(addr, e)));
        }
        Err(e) => return Err(Box::<dyn Error>::from(e)),
    };
    // Now that something answers on 127.0.0.1 and ::1, point the machine back at it: the last
    // stop gave the old DNS back (give_dns_back_while_stopped), and an update from a version that
    // left a reserve behind or ignored IPv6 is corrected here, on the first start, not a minute
    // later. On every system since 1.0.2, because every system now gives the DNS back on stop.
    {
        let db = cfg.db.clone();
        let _ = tokio::task::spawn_blocking(move || {
            if let Ok(l) = Ledger::open(&db, identity::genesis()) {
                reapply_dns_if_dropped(&l);
            }
        })
        .await;
    }
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
    // What the resolver forwards to in this pass, to notice when the network changes it.
    // Only when the upstreams came automatically: a person who typed --upstream chose. It was
    // Windows-only until 1 Oct 2026; macOS reads the DHCP lease now, and Linux falls back to the
    // saved servers until it has a live source of its own (then this just starts working).
    let seguir_red = cfg.upstreams.is_empty();
    let keep_upstreams: Vec<IpAddr> = upstreams.iter().map(SocketAddr::ip).collect();
    let keep_upstream_addrs: Vec<SocketAddr> = upstreams.clone();
    let keep_salud = Arc::clone(&salud);
    let mut aparte = Aparte::default();
    let keep_block_mode = block_mode;
    let (reconfigure_tx, mut reconfigure) = tokio::sync::oneshot::channel::<()>();
    let (caducado_tx, mut caducado) = tokio::sync::oneshot::channel::<()>();
    let housekeeping = tokio::spawn(async move {
        let mut reconfigure_tx = Some(reconfigure_tx);
        let mut caducado_tx = Some(caducado_tx);
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
                    // Mode was switched: ask for the listeners to be rebuilt. The same for
                    // the way a cut name is answered: the resolver takes it when it is built,
                    // and until 1.0.1 a change saved in the panel waited for the next restart.
                    if reconfigure_tx.is_some() {
                        if let Ok(l) = Ledger::open(&keep_db, identity::genesis()) {
                            if home_lan_wanted(&l, &keep_secret) != home_lan
                                || block_mode_of(&l) != keep_block_mode
                            {
                                if let Some(tx) = reconfigure_tx.take() {
                                    let _ = tx.send(());
                                }
                            }
                        }
                    }
                }
                _ = minute.tick() => {
                    if let Ok(l) = Ledger::open(&keep_db, identity::genesis()) {
                        // Se mira cada minuto, no cada hora: el día que termina la prueba, el
                        // programa tiene que apartarse ese día, no hasta una hora después.
                        if caducado_tx.is_some() && !puede_funcionar(&l, &keep_secret) {
                            stand_down(&l);
                            if let Some(tx) = caducado_tx.take() {
                                let _ = tx.send(());
                            }
                        } else {
                            let (ok, fallos) = keep_salud.minuto();
                            match aparte.minuto(ok, fallos) {
                                Paso::Nada => {}
                                Paso::Comprobar => {
                                    if !dns::upstream_answers(&keep_upstream_addrs, SONDA).await {
                                        let lista = lista_de(&keep_upstream_addrs);
                                        println!(
                                            "{}",
                                            i18n::current()
                                                .cli("observe.sin_arriba")
                                                .replace("{upstream}", &lista)
                                        );
                                        devolver_por_silencio(&l, &lista);
                                        aparte.apartado = true;
                                    }
                                }
                                Paso::Sondear => {
                                    if dns::upstream_answers(&keep_upstream_addrs, SONDA).await {
                                        if volver_con_arriba(&l) {
                                            aparte.apartado = false;
                                        }
                                    } else if sysdns::guardian_is_primary() == Some(true) {
                                        // The undo failed last minute, or someone pointed the
                                        // machine back here while the network is still quiet:
                                        // it goes back again, or they are left without names.
                                        devolver_por_silencio(&l, &lista_de(&keep_upstream_addrs));
                                    }
                                }
                            }
                            if !aparte.apartado {
                                reapply_dns_if_dropped(&l);
                            }
                            // A laptop that moved from home to the office: the resolvers that
                            // came automatically are now the office's. Rebuild with them.
                            if seguir_red && reconfigure_tx.is_some() {
                                if let Ok(Some(json)) = l.setting(SETTING_BACKUP) {
                                    if let Ok(b) = serde_json::from_str::<Backup>(&json) {
                                        let ahora = sysdns::upstreams_for(&b);
                                        if !ahora.is_empty() && ahora != keep_upstreams {
                                            if let Some(tx) = reconfigure_tx.take() {
                                                let _ = tx.send(());
                                            }
                                        }
                                    }
                                }
                            }
                        }
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
                        // The once-per-period check of a Plus key (decision 52). Nothing is
                        // pruned any more: there is one plan, and while it is on the whole
                        // history is kept. The extract is the person's, and deleting a piece of
                        // it to sell them the rest is not something this program does.
                        let _ = guardiana_license::check_if_due(&mut l, &keep_secret, now_ms());
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
        _ = &mut caducado => Exit::Caducado,
    };

    housekeeping.abort();
    // Lo que quedara esperando a saber su programa se anota igual, sin él: al cerrar, una consulta
    // sin anotar sería una consulta perdida, y eso sí que no.
    if let Some(v) = vaciador {
        v.abort();
        let pendientes: Vec<guardiana_core::NewEvent> = match policy_cola.lock() {
            Ok(mut c) => c.drain(..).collect(),
            Err(_) => Vec::new(),
        };
        if !pendientes.is_empty() {
            if let Ok(mut l) = Ledger::open(&cfg.db, identity::genesis()) {
                for e in pendientes {
                    let _ = l.append(e);
                }
            }
        }
    }
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

#[cfg(test)]
mod tests_alcance {
    use super::Alcance;

    /// The scope covers a domain and everything under it, and nothing else. A cut that
    /// depended on a sloppy match here would break a work tool, so this is exact.
    #[test]
    fn scope_covers_the_domain_and_its_subdomains_only() {
        let a = Alcance {
            cortar: true,
            patrones: vec!["github.com".into(), "api.anthropic.com".into()],
        };
        assert!(a.cubre("github.com"));
        assert!(a.cubre("api.github.com"));
        assert!(a.cubre("GitHub.com")); // asked in any case
        assert!(a.cubre("github.com.")); // with the root dot, as the wire carries it
        assert!(a.cubre("api.anthropic.com"));
        // Not covered: a different domain that merely ends the same way, which is how
        // a look-alike name would slip through.
        assert!(!a.cubre("notgithub.com"));
        assert!(!a.cubre("github.com.evil.net"));
        assert!(!a.cubre("anthropic.com")); // the parent of an allowed subdomain
        assert!(!a.cubre("mixpanel.com"));
    }

    /// An empty scope never cuts: nothing is written, nothing is enforced.
    #[test]
    fn an_empty_scope_covers_nothing_and_is_never_enforced() {
        let a = Alcance::default();
        assert!(!a.cortar);
        assert!(a.patrones.is_empty());
        assert!(!a.cubre("github.com"));
    }
}

#[cfg(test)]
mod aparte_tests {
    use super::*;

    #[test]
    fn a_quiet_minute_or_one_lost_packet_is_not_a_dead_network() {
        assert!(!minuto_sin_arriba(0, 0));
        assert!(!minuto_sin_arriba(0, 2));
        assert!(!minuto_sin_arriba(1, 50));
        assert!(minuto_sin_arriba(0, 3));
    }

    #[test]
    fn a_silent_minute_asks_and_once_aside_it_only_probes() {
        let mut a = Aparte::default();
        assert_eq!(a.minuto(5, 0), Paso::Nada);
        assert_eq!(a.minuto(0, 2), Paso::Nada);
        assert_eq!(a.minuto(3, 40), Paso::Nada);
        assert_eq!(a.minuto(0, 10), Paso::Comprobar);
        a.apartado = true;
        assert_eq!(a.minuto(0, 10), Paso::Sondear);
        assert_eq!(a.minuto(9, 0), Paso::Sondear);
        a.apartado = false;
        assert_eq!(a.minuto(9, 0), Paso::Nada);
    }

    #[test]
    fn the_health_counter_reads_and_resets() {
        let s = Salud::default();
        s.anota(Outcome::UpstreamFailed);
        s.anota(Outcome::UpstreamFailed);
        s.anota(Outcome::Forwarded {
            rcode: dns::ResponseCode::NoError,
        });
        s.anota(Outcome::Canary);
        assert_eq!(s.minuto(), (1, 2));
        assert_eq!(s.minuto(), (0, 0));
    }
}
