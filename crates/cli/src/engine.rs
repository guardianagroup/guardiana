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
use guardiana_service::home::{
    self, SETTING_HOME_APARCADO, SETTING_HOME_IP, SETTING_HOME_MODE, SETTING_HOME_RED,
};
use guardiana_service::sysdns::{self, Backup, SETTING_BACKUP, SETTING_BACKUP_APARCADA};

/// Settings key: `nxdomain` (default) or `zero` (brief §4).
pub const SETTING_BLOCK_MODE: &str = "block_mode";
/// The last upstreams that came automatically from the network and were not this machine,
/// comma-separated. Used only when every resolver the network hands out is this machine
/// itself: the router of a house in Home Mode gives the computer its own address as DNS.
const SETTING_UPSTREAM_BUENO: &str = "upstream_bueno";
/// "1" once the program looked, the first time it stood aside, for what 1.0.1 left without
/// parking (`heredar_apartado`).
const SETTING_APARTADO_HEREDADO: &str = "apartado_heredado_1_0_2";

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

/// How often a device's last activity and address are written while it keeps asking.
const DEVICE_TOUCH_MS: i64 = 60_000;

struct Inner {
    ledger: Ledger,
    classifier: Classifier,
    /// Device id → first time seen, for the 24-hour gate.
    devices_seen: HashMap<String, i64>,
    /// Device id → when its last activity and address were last written. The device row is
    /// refreshed at most once a minute: until 1.0.2 it was written only on the first query of
    /// each run, so «Última actividad» stayed frozen (review of 28 Sep 2026, G5).
    devices_touched: HashMap<String, i64>,
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
    /// Devices whose queries `observe` is not printing because their owner does not share the
    /// detail; each is named once, so the person knows why it is silent.
    callados_avisados: Mutex<std::collections::HashSet<String>>,
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

/// The rules as the resolver matches them. A rule saved before 1.0.2 may hold what was typed
/// (`https://x.com/…`, `*.x.com`) and never match a query: it is read as the name a query carries
/// (G4, 28 Sep 2026). At start and at every reload, the same.
fn reglas_legibles(mut all: Vec<Rule>) -> Vec<Rule> {
    for r in &mut all {
        if r.match_kind != guardiana_core::MatchKind::Category {
            if let Some((n, comodin)) = guardiana_core::rules::normalizar_nombre(&r.pattern) {
                r.pattern = n;
                if comodin {
                    r.match_kind = guardiana_core::MatchKind::Suffix;
                }
            }
        }
    }
    all
}

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
            // The name a query carries, as the panel now saves it; a scope saved before 1.0.2 may
            // still hold a line it would not have kept.
            .filter_map(|x| guardiana_core::rules::normalizar_nombre(x).map(|(n, _)| n))
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
                self.rules.rules = reglas_legibles(all);
                self.rules.version = version;
            }
            // «Borrar todo» bumps the version too: what this pass remembered about the devices
            // (first seen, hours profile) is read again from the ledger, so a scope declared
            // after the wipe waits its day like the panel says (review of 8 Oct 2026).
            self.devices_seen.clear();
            self.devices_touched.clear();
            self.alcances_vistos = None;
        }
    }
}

impl EnginePolicy {
    /// The hours profiles the classifier changed, written to the ledger.
    fn volcar_perfiles(inner: &mut Inner) {
        let dirty = inner.classifier.take_dirty_profiles();
        for (id, json) in dirty {
            let _ = inner.ledger.set_hours_profile(&id, &json);
        }
    }

    /// Everything the classifier learned and had not written yet: called when a pass ends, so
    /// the last fifty queries' worth of profile is not lost with it.
    fn al_cerrar(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            Self::volcar_perfiles(&mut inner);
        }
    }

    /// What `decide` would have noted for a query the resolver answered by itself.
    fn sin_decidir(&self, q: &Query) -> Option<Pending> {
        let who = self.devices.identify(q.client.ip());
        let mut inner = self.inner.lock().ok()?;
        let first_time = inner
            .ledger
            .first_time(&who.id, &q.name, q.ts)
            .unwrap_or(false);
        let classified = inner.classifier.classify(&Input {
            device_id: &who.id,
            name: &q.name,
            ts: q.ts,
            first_time,
        });
        Some(Pending {
            device_id: who.id,
            ip: q.client.ip().to_string(),
            classified,
            fuera_de_alcance: false,
        })
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
                inner.devices_touched.insert(device_id.clone(), q.ts);
                first
            }
        };
        if inner
            .devices_touched
            .get(&device_id)
            .is_some_and(|t| q.ts - *t >= DEVICE_TOUCH_MS)
        {
            let _ = inner
                .ledger
                .upsert_device(&device_id, who.mac.as_deref(), Some(&ip), q.ts);
            inner.devices_touched.insert(device_id.clone(), q.ts);
        }
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
        let regla = rules::decide(
            &inner.rules.rules,
            RuleInput {
                device_id: &device_id,
                name: &q.name,
                category: classified.category,
                observed_ms: q.ts - first_seen,
            },
            q.ts,
        );
        let decision = match regla {
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
        // An explicit rule that allows the name wins over the declared scope too: until 1.0.5
        // the allow was lost on the way here and Guard mode cut the name regardless of what the
        // rules page said (review of 8 Oct 2026).
        let decision = if matches!(decision, Decision::Forward) && regla.is_none() {
            let alcance = inner.alcance_de(&device_id);
            // Never what the machine needs to keep itself alive (updates, time, certificate
            // checks, resolvers, messaging): Guard mode is a wide cut, and the inviolable rule
            // of brief §6 covers it like a category rule. Until 1.0.2 it cut them while the
            // panel left them out of its count (G3, 28 Sep 2026).
            if alcance.cortar
                && !alcance.patrones.is_empty()
                && !alcance.cubre(&q.name)
                && classified.category != guardiana_core::Category::Esperado
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
        let pendiente = self
            .pending
            .lock()
            .ok()
            .and_then(|mut m| m.remove(&key_of(q)));
        let Some(p) = pendiente.or_else(|| {
            // Firefox's canary is answered by the resolver itself, before anything is asked of
            // the policy, so nothing was waiting for it here and it was never written down
            // (review of 5 Oct 2026, core medium). It is a query this device made, and the
            // answer it got decides whether Firefox goes around Guardiana: it goes in the
            // extract like any other. The self-check names stay out on purpose.
            matches!(outcome, Outcome::Canary).then(|| self.sin_decidir(q))?
        }) else {
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
        // Counted here, before the Windows queue takes the event: until 1.0.5 the count lived
        // after the append, the queue returned early, and on Windows the hours profile was never
        // written down, so «out of hours» started from zero at every restart (review of 8 Oct
        // 2026).
        inner.recorded += 1;
        if inner.recorded % 50 == 0 {
            Self::volcar_perfiles(&mut inner);
        }
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
                    // The live list on the screen is the household reading too: a phone that
                    // does not share its detail is not printed name by name (review of 5 Oct
                    // 2026, privacy item 5). It is named once, so its silence has a reason.
                    if inner.ledger.shares_detail(&e.device_id).unwrap_or(false) {
                        println!("{}", crate::show::line(self.texts, &e, None));
                    } else if self
                        .callados_avisados
                        .lock()
                        .is_ok_and(|mut v| v.insert(e.device_id.clone()))
                    {
                        let nombre = inner
                            .ledger
                            .device(&e.device_id)
                            .ok()
                            .flatten()
                            .and_then(|d| d.name)
                            .unwrap_or_else(|| e.device_id.clone());
                        println!(
                            "{}",
                            self.texts
                                .cli("observe.no_comparte")
                                .replace("{dispositivo}", &nombre)
                        );
                    }
                }
            }
            Err(e) => eprintln!("guardiana: {e}"),
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
    // Nothing points here any more: nothing to give back. A start that fails every five seconds
    // (port 53 taken) ran the restore each time, and on Linux-resolved that is a restart of
    // resolved every few seconds for as long as it lasts (review of 8 Oct 2026).
    if sysdns::guardian_still_set() == Some(false) {
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
        .filter(|_| en_red_de_casa(ledger))
}

/// Whether the computer is on the network Home Mode was switched on in (`SETTING_HOME_RED`).
///
/// Until 1.0.2 Home Mode opened its ports on whatever private network the computer was on: a
/// laptop took it to a café or a hotel, where anyone on the Wi-Fi could use it as their DNS and
/// their queries went into the owner's extract (review of 5 Oct 2026, Linux medium; Windows
/// limits the firewall rule to the local subnet, which is the café's there). Now only at home.
/// A Home Mode switched on before 1.0.2 has no network written down: the one it is on the first
/// time is taken as home. When the router cannot be told (no gateway), it is not held against
/// the person: the ports stay as they were.
///
/// Asked at most every five minutes for the same address and the same stored network: on
/// Windows reading the router is a PowerShell, and the LAN is checked every 15 seconds.
fn en_red_de_casa(ledger: &Ledger) -> bool {
    type Visto = Option<(Option<std::net::Ipv4Addr>, String, Instant, bool)>;
    static VISTO: Mutex<Visto> = Mutex::new(None);
    let lan = guardiana_devices::local_lan_ipv4();
    let guardada = ledger
        .setting(SETTING_HOME_RED)
        .ok()
        .flatten()
        .unwrap_or_default();
    if let Ok(v) = VISTO.lock() {
        if let Some((l, g, t, r)) = v.as_ref() {
            if *l == lan && *g == guardada && t.elapsed() < Duration::from_secs(300) {
                return *r;
            }
        }
    }
    let actual = sysdns::default_gateway().map(guardiana_devices::Red::de_puerta);
    let casa = guardiana_devices::Red::de_texto(&guardada);
    let (resultado, nueva) = match (casa, actual) {
        (Some(c), Some(a)) if c.misma(&a) => {
            // The router's MAC learned since: written down, so a café with the same address is
            // told apart from now on.
            let mejor = (c.mac.is_none() && a.mac.is_some()).then(|| a.texto());
            (true, mejor)
        }
        (Some(_), Some(_)) => (false, None),
        (None, Some(a)) => (true, Some(a.texto())),
        (_, None) => (true, None),
    };
    let guardada = match nueva {
        Some(t) if ledger.set_setting(SETTING_HOME_RED, &t).is_ok() => t,
        _ => guardada,
    };
    if let Ok(mut v) = VISTO.lock() {
        *v = Some((lan, guardada, Instant::now(), resultado));
    }
    resultado
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
    if sysdns::guardian_still_set() == Some(false) || sysdns::restore(&backup).is_err() {
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
    if reapuntar(ledger, backup).is_err() {
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
/// switched off, and both changes are written into the ledger like any other. The extract stays
/// on the disk, whole: it belongs to the person, whatever they decide about paying.
///
/// Both are parked, not forgotten: the person chose them, and they come back on by themselves
/// when the licence does (`volver_del_apartado`). Until 1.0.2 they were simply cleared, and a
/// customer who paid got a program that watched nothing, because nothing pointed at it any more
/// (review of 5 Oct 2026, serious 4). A parked Home Mode also keeps a relay on the LAN while the
/// program is stood down, because the router still sends the whole house here (critical 2).
fn stand_down(ledger: &Ledger) {
    heredar_apartado(ledger);
    devolver_dns_al_apartarse(ledger);
    if ledger.setting(SETTING_HOME_MODE).ok().flatten().as_deref() == Some("1") {
        let _ = ledger.set_setting(SETTING_HOME_APARCADO, "1");
        let _ = ledger.set_setting(SETTING_HOME_MODE, "0");
        let _ = ledger.record_change(
            now_ms(),
            guardiana_core::ChangeKind::HogarOff,
            guardiana_core::ChangeWho::Licencia,
            "",
        );
    }
}

/// 1.0.1 stood aside without parking anything: it cleared the DNS copy and switched Home Mode
/// off, and wrote both into the ledger as done by the licence. The trials of the first
/// downloads ended on 2 Oct 2026, so there are machines in that state today, and houses whose
/// router still points at one of them. Once, the first time 1.0.2 stands aside: if the last
/// Home Mode change was that one, Home Mode is parked (the relay starts, and it comes back on
/// with the licence); if the last DNS change was that one, the DNS as it is now is parked, so
/// it is pointed at Guardiana again with the licence. Anything the person did since (a change
/// by the panel or the terminal is newer) leaves it alone.
fn heredar_apartado(ledger: &Ledger) {
    use guardiana_core::{ChangeKind, ChangeWho};
    if ledger
        .setting(SETTING_APARTADO_HEREDADO)
        .ok()
        .flatten()
        .as_deref()
        == Some("1")
    {
        return;
    }
    let _ = ledger.set_setting(SETTING_APARTADO_HEREDADO, "1");
    let Ok(cambios) = ledger.changes(500) else {
        return;
    };
    let vacio = |k: &str| {
        ledger
            .setting(k)
            .ok()
            .flatten()
            .is_none_or(|v| v.is_empty())
    };
    let por_licencia = |c: &guardiana_core::Change, kind: ChangeKind| {
        c.kind == kind && c.who == ChangeWho::Licencia.as_str()
    };
    let ultimo_hogar = cambios
        .iter()
        .find(|c| matches!(c.kind, ChangeKind::HogarOn | ChangeKind::HogarOff));
    if ultimo_hogar.is_some_and(|c| por_licencia(c, ChangeKind::HogarOff))
        && ledger.setting(SETTING_HOME_MODE).ok().flatten().as_deref() != Some("1")
        && vacio(SETTING_HOME_APARCADO)
    {
        let _ = ledger.set_setting(SETTING_HOME_APARCADO, "1");
    }
    let ultimo_dns = cambios
        .iter()
        .find(|c| matches!(c.kind, ChangeKind::DnsOn | ChangeKind::DnsOff));
    if ultimo_dns.is_some_and(|c| por_licencia(c, ChangeKind::DnsOff))
        && vacio(SETTING_BACKUP)
        && vacio(SETTING_BACKUP_APARCADA)
    {
        if let Some(json) = sysdns::snapshot(now_ms())
            .ok()
            .and_then(|b| serde_json::to_string(&b).ok())
        {
            let _ = ledger.set_setting(SETTING_BACKUP_APARCADA, &json);
        }
    }
}

/// Give the system DNS back on standing down and park the copy. `true` when nothing of ours is
/// left pointing at the guardian.
///
/// The copy moves only once the undo really happened. It used to be cleared first: one interface
/// that no longer exists (the trip's VPN) made restore() fail, and the only record of the
/// previous DNS was gone (review of 1 Oct 2026). An undo that failed on such an interface while
/// every connected one no longer points here is done all the same: retrying it would fail
/// forever. Otherwise the stood-down program tries again every minute (`esperar_licencia`).
fn devolver_dns_al_apartarse(ledger: &Ledger) -> bool {
    let Ok(Some(json)) = ledger.setting(SETTING_BACKUP) else {
        return true;
    };
    if json.is_empty() {
        return true;
    }
    let Ok(backup) = serde_json::from_str::<Backup>(&json) else {
        return true;
    };
    // Done when the undo worked, or when it failed only on something that no longer exists and
    // nothing at all still asks the guardian. Not "some interface does not": one put back and
    // another left behind is a loopback nobody listens on.
    let hecho = sysdns::restore(&backup).is_ok() || sysdns::guardian_still_set() == Some(false);
    if hecho {
        let _ = ledger.set_setting(SETTING_BACKUP_APARCADA, &json);
        let _ = ledger.set_setting(SETTING_BACKUP, "");
        let _ = ledger.record_change(
            now_ms(),
            guardiana_core::ChangeKind::DnsOff,
            guardiana_core::ChangeWho::Licencia,
            "",
        );
    }
    hecho
}

/// The licence is back: what standing down parked comes back on. Returns whether the system DNS
/// (the copy, to be applied by the next pass once the resolver listens) and Home Mode came back.
///
/// The copy taken now, not the parked one: while the program stood aside the person may have
/// moved to another network or changed the DNS by hand, and the undo has to put back what there
/// is today. The parked one only if a fresh one cannot be taken.
fn volver_del_apartado(ledger: &Ledger) -> (bool, bool) {
    let mut dns = false;
    if let Ok(Some(aparcada)) = ledger.setting(SETTING_BACKUP_APARCADA) {
        if !aparcada.is_empty() {
            let libre = ledger
                .setting(SETTING_BACKUP)
                .ok()
                .flatten()
                .is_none_or(|v| v.is_empty());
            if libre {
                let copia = sysdns::snapshot(now_ms())
                    .ok()
                    .or_else(|| serde_json::from_str::<Backup>(&aparcada).ok());
                if let Some(json) = copia.as_ref().and_then(|b| serde_json::to_string(b).ok()) {
                    if ledger.set_setting(SETTING_BACKUP, &json).is_ok() {
                        dns = true;
                        let originales = copia
                            .map(|b| {
                                b.original_servers()
                                    .iter()
                                    .map(ToString::to_string)
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            })
                            .unwrap_or_default();
                        let _ = ledger.record_change(
                            now_ms(),
                            guardiana_core::ChangeKind::DnsOn,
                            guardiana_core::ChangeWho::Licencia,
                            &originales,
                        );
                    }
                }
            }
            let _ = ledger.set_setting(SETTING_BACKUP_APARCADA, "");
        }
    }
    let mut hogar = false;
    if ledger
        .setting(SETTING_HOME_APARCADO)
        .ok()
        .flatten()
        .as_deref()
        == Some("1")
    {
        let _ = ledger.set_setting(SETTING_HOME_MODE, "1");
        let _ = ledger.set_setting(SETTING_HOME_APARCADO, "");
        let _ = ledger.record_change(
            now_ms(),
            guardiana_core::ChangeKind::HogarOn,
            guardiana_core::ChangeWho::Licencia,
            "",
        );
        hogar = true;
    }
    (dns, hogar)
}

/// The LAN address the relay of a stood-down program should listen on: `None` unless Home Mode
/// was parked and the computer has a private LAN address.
fn relevo_wanted(ledger: &Ledger) -> Option<std::net::Ipv4Addr> {
    let aparcado = ledger
        .setting(SETTING_HOME_APARCADO)
        .ok()
        .flatten()
        .is_some_and(|v| v == "1");
    if !aparcado {
        return None;
    }
    guardiana_devices::local_lan_ipv4()
        .filter(|lan| guardiana_devices::is_private_lan(IpAddr::V4(*lan)))
        .filter(|_| en_red_de_casa(ledger))
}

/// The relay's policy: pass everything on, write nothing down. The program is not watching, and
/// the household's queries are not its to keep.
struct Relevo;

impl Policy for Relevo {
    fn decide(&self, _: &Query) -> Decision {
        Decision::Forward
    }
    fn record(&self, _: &Query, _: Outcome) {}
}

/// Start the relay of a stood-down program on the LAN, or say why it could not.
async fn arrancar_relevo(
    lan: std::net::Ipv4Addr,
    port: u16,
    upstreams: &[SocketAddr],
) -> Option<dns::Running> {
    let t = i18n::current();
    let addr = SocketAddr::new(IpAddr::V4(lan), port);
    let arriba: Vec<SocketAddr> = upstreams.iter().copied().filter(|u| *u != addr).collect();
    if arriba.is_empty() {
        println!("{}", t.cli("observe.relevo_sin_arriba"));
        return None;
    }
    let mut c = Config::local(arriba.clone());
    c.listen = vec![addr];
    c.self_check = false;
    match dns::start(c, Relevo).await {
        Ok(r) => {
            println!(
                "{}",
                t.cli("observe.relevo")
                    .replace("{addr}", &addr.to_string())
                    .replace("{upstream}", &lista_de(&arriba))
            );
            Some(r)
        }
        Err(e) => {
            eprintln!(
                "{}",
                t.cli("observe.relevo_error")
                    .replace("{error}", &e.to_string())
            );
            None
        }
    }
}

/// Why a stood-down program stops waiting.
enum Vuelta {
    /// The program may work again.
    Licencia,
    /// The relay must be rebuilt or closed: Home Mode was switched off, or the LAN changed.
    Relevo,
}

/// How often a stood-down program reads the licence again: a key typed in the panel brings it
/// back within this, not at the next restart.
const LICENCIA_RELECTURA: Duration = Duration::from_secs(5);
/// How often a stood-down program retries a check that could not be done, like a running one.
const LICENCIA_REINTENTO: Duration = Duration::from_secs(60 * 60);
/// How often a stood-down program tries again to give back a DNS it could not give back.
const DEVOLVER_REINTENTO: Duration = Duration::from_secs(60);

/// Returns once the program may work again: a key was activated, the gateway answered a retried
/// check, or the clock says so; or once the relay has to change. Each read opens the ledger
/// afresh, so what the panel wrote is seen.
async fn esperar_licencia(
    db: std::path::PathBuf,
    secret: String,
    relevo: Option<std::net::Ipv4Addr>,
) -> Vuelta {
    let desde = Instant::now();
    let mut ultimo_intento: Option<Instant> = None;
    let mut ultima_devolucion = Instant::now();
    loop {
        tokio::time::sleep(LICENCIA_RELECTURA).await;
        let toca = ultimo_intento.is_none_or(|t| t.elapsed() >= LICENCIA_REINTENTO);
        let devolver = ultima_devolucion.elapsed() >= DEVOLVER_REINTENTO;
        if devolver {
            ultima_devolucion = Instant::now();
        }
        let (db2, secret2) = (db.clone(), secret.clone());
        // The ledger and the gateway are blocking: off the runtime's threads.
        let lista = tokio::task::spawn_blocking(move || {
            let mut l = Ledger::open(&db2, identity::genesis()).ok()?;
            if devolver {
                // A DNS that could not be given back when standing down (licence medium item).
                devolver_dns_al_apartarse(&l);
            }
            if toca {
                // What 1.0.1 left that is not taken on trust: asked once (licence item 7).
                let _ = guardiana_license::revisar_herencia(&mut l, &secret2, now_ms());
                if guardiana_license::check_due(&l, &secret2, now_ms()).unwrap_or(false) {
                    let _ = guardiana_license::check_if_due(&mut l, &secret2, now_ms());
                }
            }
            let puede =
                guardiana_license::status(&l, &secret2, now_ms()).is_ok_and(|s| s.puede_funcionar);
            Some((puede, relevo_wanted(&l)))
        })
        .await
        .ok()
        .flatten();
        // Whatever came of it, a turn that could ask has asked: the next one is in an hour. Until
        // 1.0.2 only an answer counted, and with no network the inherited licence was asked
        // about every 15 seconds, 239 times an hour (review of 5 Oct 2026, licence medium).
        if toca {
            ultimo_intento = Some(Instant::now());
        }
        match lista {
            Some((true, _)) => return Vuelta::Licencia,
            // A relay that is wanted and is not running (it could not start: the address was not
            // ready yet, or there was no upstream) is tried again every minute, not never. One
            // that has to change or close is rebuilt at once (second pass of 5 Oct 2026).
            Some((false, ahora))
                if ahora != relevo
                    && (relevo.is_some()
                        || ahora.is_none()
                        || desde.elapsed() >= DEVOLVER_REINTENTO) =>
            {
                return Vuelta::Relevo
            }
            _ => {}
        }
    }
}

/// Whether the program may watch right now: the trial or a subscription is on.
fn puede_funcionar(ledger: &Ledger, secret: &str) -> bool {
    guardiana_license::status(ledger, secret, now_ms())
        .map(|s| s.puede_funcionar)
        .unwrap_or(true)
}

/// The upstreams that come automatically, as `IpAddr:53`, never this machine itself, with the
/// start-up line that says where they came from.
///
/// In order: the live resolvers of the network behind the saved copy (or the parked one while
/// the program stands aside), what the system uses now, the last good ones the network gave,
/// the ones recorded in the copy, and the router. The live source alone was not enough: with
/// Home Mode the router hands every device the computer's own address as DNS, the computer
/// takes it too, and until 1.0.2 Guardiana forwarded the whole house back to itself until each
/// query timed out (review of 5 Oct 2026, critical 3). Never a public resolver nobody chose.
fn arriba_automatico(ledger: &Ledger) -> Result<Vec<SocketAddr>, String> {
    let t = i18n::current();
    match elegir_arriba(ledger) {
        Ok((fuente, v, fuente_sistema)) => {
            let linea = match fuente {
                Fuente::Copia => t.cli("observe.upstream_copia"),
                Fuente::Sistema => t.cli("observe.upstream_auto"),
                Fuente::Respaldo => t.cli("observe.upstream_propio"),
            };
            println!(
                "{}",
                linea
                    .replace("{servers}", &lista_de(&v))
                    .replace("{fuente}", &fuente_sistema)
            );
            Ok(v)
        }
        Err(todo_propio) => Err(t
            .cli(if todo_propio {
                "observe.solo_propio"
            } else {
                "observe.sin_upstream"
            })
            .to_owned()),
    }
}

/// [`arriba_automatico`] without saying anything: where the list came from, the list, and the
/// name the system gave its source. `Err(true)` when every resolver was this machine.
fn elegir_arriba(ledger: &Ledger) -> Result<(Fuente, Vec<SocketAddr>, String), bool> {
    let saved: Option<Backup> = [SETTING_BACKUP, SETTING_BACKUP_APARCADA]
        .iter()
        .find_map(|k| match ledger.setting(k) {
            Ok(Some(json)) if !json.is_empty() => serde_json::from_str(&json).ok(),
            _ => None,
        });
    let vivos = saved
        .as_ref()
        .map(sysdns::upstreams_for)
        .unwrap_or_default();
    let buenos: Vec<IpAddr> = ledger
        .setting(SETTING_UPSTREAM_BUENO)
        .ok()
        .flatten()
        .unwrap_or_default()
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let copia: Vec<IpAddr> = saved
        .as_ref()
        .map(|b| {
            b.original_servers()
                .into_iter()
                .filter(|ip| sysdns::usable_upstream(*ip))
                .collect()
        })
        .unwrap_or_default();
    let mut fuente_sistema = String::new();
    let sistema = || {
        sysdns::current_resolvers()
            .map(|c| {
                fuente_sistema = format!("{:?}", c.source);
                c.servers.iter().map(SocketAddr::ip).collect()
            })
            .unwrap_or_default()
    };
    let (fuente, v) = escoger_arriba(
        vivos,
        sistema,
        buenos,
        copia,
        sysdns::default_gateway,
        responden_ahora,
    )?;
    if fuente != Fuente::Respaldo {
        guardar_buenos(ledger, &v);
    }
    Ok((fuente, v, fuente_sistema))
}

/// Whether these upstreams answer a question right now. On a thread of its own, with a runtime
/// of its own: it is called from the middle of the start-up, whatever runtime that is on.
fn responden_ahora(v: &[SocketAddr]) -> bool {
    let v = v.to_vec();
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .is_ok_and(|rt| rt.block_on(dns::upstream_answers(&v, SONDA)))
    })
    .join()
    .unwrap_or(false)
}

/// Where the automatic upstreams of a pass came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fuente {
    /// The live resolvers behind the saved copy.
    Copia,
    /// What the system uses now.
    Sistema,
    /// A fallback, because every live answer was this machine.
    Respaldo,
}

/// The first list, in the order of [`arriba_automatico`], that holds an address that is not this
/// machine, with port 53 and only those addresses. The system and the router are asked only
/// when needed: on Windows each is a PowerShell. `Err(true)` when there were resolvers and every
/// one was this machine; `Err(false)` when there were none at all.
fn escoger_arriba(
    vivos: Vec<IpAddr>,
    sistema: impl FnOnce() -> Vec<IpAddr>,
    buenos: Vec<IpAddr>,
    copia: Vec<IpAddr>,
    puerta: impl FnOnce() -> Option<IpAddr>,
    mut responde: impl FnMut(&[SocketAddr]) -> bool,
) -> Result<(Fuente, Vec<SocketAddr>), bool> {
    let ajenas = |ips: Vec<IpAddr>| -> Vec<SocketAddr> {
        let mut out: Vec<SocketAddr> = Vec::new();
        for ip in sysdns::away_from_self(ips) {
            let a = SocketAddr::new(ip, 53);
            if !out.contains(&a) {
                out.push(a);
            }
        }
        out
    };
    let mut habia = !vivos.is_empty();
    let v = ajenas(vivos);
    if !v.is_empty() {
        return Ok((Fuente::Copia, v));
    }
    let sistema = sistema();
    habia |= !sistema.is_empty();
    let v = ajenas(sistema);
    if !v.is_empty() {
        return Ok((Fuente::Sistema, v));
    }
    // Only the first of these that answers: the last good list may be the café's, kept from a
    // day when the computer was there, and at home it answers nothing (review of 5 Oct 2026,
    // second pass). When none answers, the first one there is, as before.
    let mut primera: Option<Vec<SocketAddr>> = None;
    for lista in [buenos, copia] {
        let v = ajenas(lista);
        if v.is_empty() {
            continue;
        }
        if responde(&v) {
            return Ok((Fuente::Respaldo, v));
        }
        primera.get_or_insert(v);
    }
    let v = ajenas(puerta().into_iter().collect());
    if !v.is_empty() && (primera.is_none() || responde(&v)) {
        return Ok((Fuente::Respaldo, v));
    }
    if let Some(v) = primera {
        return Ok((Fuente::Respaldo, v));
    }
    Err(habia)
}

/// Remember the upstreams the network gave this time, for the day it gives only this machine.
fn guardar_buenos(ledger: &Ledger, v: &[SocketAddr]) {
    let texto = v
        .iter()
        .map(|s| s.ip().to_string())
        .collect::<Vec<_>>()
        .join(",");
    if ledger
        .setting(SETTING_UPSTREAM_BUENO)
        .ok()
        .flatten()
        .as_deref()
        != Some(texto.as_str())
    {
        let _ = ledger.set_setting(SETTING_UPSTREAM_BUENO, &texto);
    }
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
    // Home Mode needs the trial or the Home plan (brief §9): without either it
    // stays off with a notice, and the computer's own DNS never breaks.
    let secret =
        guardiana_panel::load_or_create_token(&paths::data_dir().join(guardiana_panel::TOKEN_FILE))
            .unwrap_or_default();
    // A licence that came back while nobody was waiting for it: a customer of 1.0.1 who paid
    // after it stood aside, or a key typed while the service was stopped. What standing down
    // parked, or 1.0.1 left behind, comes back now, before this pass reads Home Mode and the
    // DNS copy (review of 5 Oct 2026, second pass).
    if puede_funcionar(&ledger, &secret) {
        heredar_apartado(&ledger);
        let _ = volver_del_apartado(&ledger);
    }
    let home_on = ledger.setting(SETTING_HOME_MODE)?.is_some_and(|v| v == "1");
    // Home Mode is free (decision 52): no licence gate.
    let home_allowed = home_on;
    // Only on the network it was switched on in (see `en_red_de_casa`).
    let fuera_de_casa = home_allowed && !en_red_de_casa(&ledger);

    // Upstream: what the caller said; else the resolvers saved before Guardiana changed the
    // system DNS (brief §4), the parked copy while it stands aside, or whatever the system uses
    // now; never this machine itself (critical 3 of 5 Oct 2026, see `arriba_automatico`).
    let arriba: Result<Vec<SocketAddr>, String> = if cfg.upstreams.is_empty() {
        arriba_automatico(&ledger)
    } else {
        Ok(cfg.upstreams.clone())
    };

    // La prueba o la suscripción terminaron: Guardiana se aparta. No abre el resolutor, devuelve
    // el DNS del sistema a como estaba y aparca el Modo Hogar; solo queda el panel en pie para que
    // se pueda activar Plus o llevarse el extracto. Nunca se queda en medio sin resolver: eso
    // dejaría el equipo sin internet el día que caduca una suscripción. Y si el Modo Hogar estaba
    // encendido, el router sigue mandando aquí a toda la casa: un relevo en la LAN pasa sus
    // consultas sin mirarlas hasta que el router se devuelva o vuelva la licencia.
    if !puede_funcionar(&ledger, &secret) {
        stand_down(&ledger);
        // A customer whose subscription lapsed is not told "the trial is over".
        let fue_plus = guardiana_license::status(&ledger, &secret, now_ms())
            .is_ok_and(|s| matches!(s.plan, guardiana_license::Plan::PlusTerminado { .. }));
        println!(
            "{}",
            t.cli(if fue_plus {
                "observe.caducado_plus"
            } else {
                "observe.caducado"
            })
        );
        let relevo_lan = relevo_wanted(&ledger);
        let relevo = match (relevo_lan, arriba.as_ref()) {
            (Some(lan), Ok(ups)) => arrancar_relevo(lan, cfg.listen.port(), ups).await,
            (Some(_), Err(e)) => {
                println!("{e}");
                println!("{}", t.cli("observe.relevo_sin_arriba"));
                None
            }
            (None, _) => None,
        };
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
                    println!(
                        "{}",
                        t.cli("observe.panel")
                            .replace("{url}", &panel_link(&p, cfg.print_events))
                    );
                    if first && cfg.open_browser {
                        open_in_browser(&p.url());
                    }
                    Some(p)
                }
                Err(_) => None,
            },
            None => None,
        };
        // Stood down is not stopped. Until 1.0.2 this branch only waited for shutdown: a
        // subscription cut because it could not be checked was never asked about again, and a
        // key activated in the panel, or a payment the gateway took, changed nothing until the
        // machine restarted. A customer who paid stayed unwatched (review of 5 Oct 2026, licence
        // item 3). Now the licence is read again every few seconds, a check that could not be
        // done is retried every hour, and the moment the program may work it comes back, with
        // the DNS and Home Mode it had.
        let exit = tokio::select! {
            () = shutdown.as_mut() => Exit::Shutdown,
            v = esperar_licencia(
                cfg.db.clone(),
                secret.clone(),
                relevo.as_ref().and(relevo_lan),
            ) => match v {
                Vuelta::Licencia => {
                    let db = cfg.db.clone();
                    let (dns_vuelve, hogar_vuelve) = tokio::task::spawn_blocking(move || {
                        Ledger::open(&db, identity::genesis())
                            .map(|l| volver_del_apartado(&l))
                            .unwrap_or_default()
                    })
                    .await
                    .unwrap_or_default();
                    println!(
                        "{}",
                        t.cli(if dns_vuelve {
                            "observe.vuelve_licencia"
                        } else {
                            "observe.vuelve_licencia_sin_dns"
                        })
                    );
                    if hogar_vuelve {
                        println!("{}", t.cli("observe.vuelve_hogar"));
                    }
                    Exit::Reconfigure
                }
                Vuelta::Relevo => Exit::Reconfigure,
            }
        };
        if let Some(p) = panel {
            p.shutdown().await;
        }
        if let Some(r) = relevo {
            r.shutdown();
            let _ = r.wait().await;
        }
        return Ok(exit);
    }

    let upstreams = arriba.map_err(Box::<dyn Error>::from)?;
    // A typed upstream that is one of the addresses this pass listens on would send every query
    // back to itself until it timed out.
    let mut escucha_propia = vec![cfg.listen];
    if cfg.listen.ip().is_loopback() {
        escucha_propia.push(SocketAddr::new(
            IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
            cfg.listen.port(),
        ));
    }
    if let Some(lan) = home_allowed
        .then(guardiana_devices::local_lan_ipv4)
        .flatten()
    {
        escucha_propia.push(SocketAddr::new(IpAddr::V4(lan), cfg.listen.port()));
    }
    if upstreams.iter().any(|u| escucha_propia.contains(u)) {
        return Err(t.cli("observe.bucle").into());
    }

    let mut classifier = Classifier::new(Arc::new(Catalog::bundled()));
    for d in ledger.devices()? {
        if let Some(json) = d.hours_profile_json.as_deref() {
            let _ = classifier.load_profile(&d.id, json);
        }
    }
    let block_mode = block_mode_of(&ledger);
    let initial_rules = reglas_legibles(ledger.rules().unwrap_or_default());
    let initial_version = ledger.rules_version().unwrap_or(0);
    let salud = Arc::new(Salud::default());
    let policy = Arc::new(EnginePolicy {
        salud: Arc::clone(&salud),
        inner: Mutex::new(Inner {
            ledger,
            classifier,
            devices_seen: HashMap::new(),
            devices_touched: HashMap::new(),
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
        callados_avisados: Mutex::default(),
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
    });
    let policy_al_cerrar = Arc::clone(&policy);

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
    if fuera_de_casa {
        println!("{}", t.cli("observe.hogar_otra_red"));
    }
    if home_allowed && !fuera_de_casa {
        match guardiana_devices::local_lan_ipv4() {
            Some(lan) if guardiana_devices::is_private_lan(IpAddr::V4(lan)) => {
                let (dns_addr, panel_addr) = home::lan_listen_addrs(lan);
                let dns_addr = SocketAddr::new(dns_addr.ip(), cfg.listen.port());
                // Optional: if the LAN address moved between the check and the bind, or something
                // else holds <lan>:53, the home's phones wait for the next check, but this
                // machine keeps its guardian. Until 1.0.5 the whole pass failed, the DNS was given
                // back, and the service restarted every five seconds (review of 8 Oct 2026).
                dns_cfg.optional_listen.push(dns_addr);
                dns_cfg.checker_ip = Some(lan);
                dns_cfg.canary_enabled = true;
                panel_listen.push(panel_addr);
                panel_optional.push(home::lan_checker_addr(lan));
                home_lan = Some(lan);
                // The rules of the firewall, written again by this program on every start:
                // those 1.0.1 wrote let any program and any address in, and an update never
                // touched them; and `program=` must be the binary that really listens, which is
                // the service, not whichever copy ran `hogar on` (review of 5 Oct 2026, second
                // pass). Windows only; without administrator rights it changes nothing.
                if cfg!(windows) {
                    let _ = tokio::task::spawn_blocking(home::firewall_allow).await;
                }
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
            let mut conexion: Option<Ledger> = None;
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
                // One connection, opened when first needed and kept: opening the ledger
                // (schema, migrations, pragmas) four times a second was most of this task's
                // work (review of 8 Oct 2026).
                if conexion.is_none() {
                    conexion = Ledger::open(&db, identity::genesis()).ok();
                }
                let Some(l) = conexion.as_mut() else {
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
    if let Some(lan) = home_lan {
        if !running.udp_addrs.iter().any(|a| a.ip() == IpAddr::V4(lan)) {
            eprintln!(
                "{}",
                t.cli("observe.hogar_puerto_ocupado")
                    .replace("{addr}", &format!("{lan}:{}", cfg.listen.port()))
            );
        }
    }
    if cfg!(target_os = "macos") {
        println!("{}", t.cli("observe.limite_mac"));
    }

    // The panel runs in the same process, with its own ledger connection.
    let panel = match panel_cfg {
        Some(pc) => match guardiana_panel::start(pc).await {
            Ok(p) => {
                println!(
                    "{}",
                    t.cli("observe.panel")
                        .replace("{url}", &panel_link(&p, cfg.print_events))
                );
                if cfg.print_events {
                    println!("{}", t.cli("observe.panel_nota"));
                }
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
    // Windows-only until 1 Oct 2026; macOS reads the DHCP lease now, and Linux asks whoever
    // configures the link (NetworkManager, systemd-networkd, resolved) since the review of that
    // day. A system with no live source returns nothing, and the saved servers stay.
    let seguir_red = cfg.upstreams.is_empty();
    // The address this pass started on. When it changes (the laptop woke up in another network,
    // the cable went in) the pass is rebuilt within 15 seconds with the new network's resolvers.
    // Until 1.0.2 only the minute check noticed, and a Mac waking up elsewhere could go up to a
    // minute without names, asking a router that was not there (review of 5 Oct 2026).
    let lan_inicial = guardiana_devices::local_lan_ipv4();
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
                                || (seguir_red
                                    && guardiana_devices::local_lan_ipv4() != lan_inicial)
                            {
                                if let Some(tx) = reconfigure_tx.take() {
                                    let _ = tx.send(());
                                }
                            }
                        }
                    }
                }
                _ = minute.tick() => {
                    // A licence check that is due, at most once an hour counted in the ledger
                    // (review of 8 Oct 2026: the hourly timer restarted with every rebuild of
                    // the pass, and a laptop changing networks never checked).
                    {
                        let (db, secreto) = (keep_db.clone(), keep_secret.clone());
                        let _ = tokio::task::spawn_blocking(move || {
                            if let Ok(mut l) = Ledger::open(&db, identity::genesis()) {
                                let _ = guardiana_license::check_if_due_hourly(&mut l, &secreto, now_ms());
                            }
                        })
                        .await;
                    }
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
                                        aparte_del_resolutor(|| devolver_por_silencio(&l, &lista));
                                        aparte.apartado = true;
                                    }
                                }
                                Paso::Sondear => {
                                    if dns::upstream_answers(&keep_upstream_addrs, SONDA).await {
                                        if aparte_del_resolutor(|| volver_con_arriba(&l)) {
                                            aparte.apartado = false;
                                        }
                                    } else if aparte_del_resolutor(sysdns::guardian_is_primary) == Some(true) {
                                        // The undo failed last minute, or someone pointed the
                                        // machine back here while the network is still quiet:
                                        // it goes back again, or they are left without names.
                                        aparte_del_resolutor(|| devolver_por_silencio(&l, &lista_de(&keep_upstream_addrs)));
                                    }
                                }
                            }
                            if !aparte.apartado {
                                aparte_del_resolutor(|| reapply_dns_if_dropped(&l));
                            }
                            // A laptop that moved from home to the office: the resolvers that
                            // came automatically are now the office's. Rebuild with them.
                            if seguir_red && reconfigure_tx.is_some() {
                                if let Ok(Some(json)) = l.setting(SETTING_BACKUP) {
                                    if let Ok(b) = serde_json::from_str::<Backup>(&json) {
                                        // The same sieve as at the start of the pass: a list
                                        // that is only this machine is no reason to rebuild.
                                        let ahora = aparte_del_resolutor(|| {
                                            sysdns::away_from_self(sysdns::upstreams_for(&b))
                                        });
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
                    // The once-per-period check of a Plus key (decision 52). Nothing is pruned any
                    // more: there is one plan, and while it is on the whole history is kept. The
                    // extract is the person's, and deleting a piece of it to sell them the rest is
                    // not something this program does. Off the runtime's threads: the call can
                    // wait 20 seconds for the gateway, and the resolver runs on these threads
                    // (review of 5 Oct 2026, licence medium). The minute's tick does the same
                    // with the hour counted in the ledger, so a pass rebuilt often still checks;
                    // this one stays for a pass that lives the whole hour.
                    let (db, secreto) = (keep_db.clone(), keep_secret.clone());
                    let _ = tokio::task::spawn_blocking(move || {
                        if let Ok(mut l) = Ledger::open(&db, identity::genesis()) {
                            let _ = guardiana_license::check_if_due_hourly(&mut l, &secreto, now_ms());
                        }
                    })
                    .await;
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

    // The resolver's own pulse: if its task ended on its own (a socket error the server could
    // not recover from), the machine still points here and nobody answers. It is rebuilt, as
    // after a network change (review of 8 Oct 2026).
    let mut pulso = tokio::time::interval(Duration::from_secs(15));
    pulso.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let exit = loop {
        tokio::select! {
            _ = &mut shutdown => break Exit::Shutdown,
            _ = &mut reconfigure => break Exit::Reconfigure,
            _ = &mut caducado => break Exit::Caducado,
            _ = pulso.tick() => {
                if running.is_dead() {
                    eprintln!("{}", t.cli("observe.resolutor_caido"));
                    break Exit::Reconfigure;
                }
            }
        }
    };

    housekeeping.abort();
    if let Some(p) = panel {
        p.shutdown().await;
    }
    // The resolver first, the queue after: what `record` put in the queue while the resolver was
    // still answering is written down too (until 1.0.5 the queue was emptied first and those
    // last queries were lost; review of 8 Oct 2026).
    running.shutdown();
    let _ = running.wait().await;
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
    policy_al_cerrar.al_cerrar();
    Ok(exit)
}

/// Run a call that talks to the system (PowerShell, resolvectl, networksetup: one to three
/// seconds each on Windows) off the thread that answers queries. On the multi-thread runtime
/// the worker steps aside; on a current-thread one (tests) it just runs (review of 8 Oct 2026:
/// the minute's checks ran on the resolver's own threads while the hour's were already moved).
fn aparte_del_resolutor<R>(f: impl FnOnce() -> R) -> R {
    match tokio::runtime::Handle::try_current().map(|h| h.runtime_flavor()) {
        Ok(tokio::runtime::RuntimeFlavor::MultiThread) => tokio::task::block_in_place(f),
        _ => f(),
    }
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
        let _ = reapuntar(ledger, backup);
    }
}

/// Point the machine at the guardian with `backup`, after adding to it any interface that
/// appeared since it was taken (`sysdns::with_new_interfaces`). The copy that grew is written
/// down before anything changes, so the undo covers what this changes.
fn reapuntar(ledger: &Ledger, backup: Backup) -> Result<(), sysdns::Error> {
    let backup = match sysdns::snapshot(now_ms())
        .ok()
        .and_then(|fresca| sysdns::with_new_interfaces(&backup, &fresca))
    {
        Some(junta) => match serde_json::to_string(&junta) {
            Ok(json) if ledger.set_setting(SETTING_BACKUP, &json).is_ok() => junta,
            _ => backup,
        },
        None => backup,
    };
    sysdns::apply(&backup, IpAddr::V4(std::net::Ipv4Addr::LOCALHOST))
}

/// The panel's address as the start-up line shows it: with the session key when a person is
/// reading the terminal (`guardiana observe`), without it when the line goes to the service
/// log. The log is kept by the system, survives reboots and "delete everything", and on Linux
/// is readable by every member of `adm` or `systemd-journal`: a key written there is a key
/// shared (review of 1 Oct 2026, finding 15). The launcher reads the key from its file instead.
fn panel_link(p: &guardiana_panel::Running, with_token: bool) -> String {
    link_text(p.addrs.first().copied(), &p.token, with_token)
}

fn link_text(addr: Option<SocketAddr>, token: &str, with_token: bool) -> String {
    match addr {
        None => String::new(),
        Some(a) if with_token => guardiana_panel::panel_url(a, token),
        Some(a) => format!("http://{a}/"),
    }
}

/// The text of `url` as an AppleScript string literal, quotes and backslashes escaped.
#[cfg(any(test, target_os = "macos"))]
fn applescript_string(url: &str) -> String {
    let mut out = String::with_capacity(url.len() + 2);
    out.push('"');
    for c in url.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

/// Open `url` on macOS without putting it on any command line: `osascript` reads the script
/// from its standard input, and `open location` hands the URL to the default browser through
/// Launch Services. `open <url>` would show the session key to `ps` for as long as it runs
/// (review of 1 Oct 2026, finding 15).
#[cfg(target_os = "macos")]
fn open_via_osascript(url: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new("osascript")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(format!("open location {}\n", applescript_string(url)).as_bytes())?;
    }
    if child.wait()?.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("osascript could not open the url"))
    }
}

/// Open the panel URL in the default browser, best effort.
///
/// On Windows another standard user cannot read this process's command line, and on macOS
/// the URL travels on standard input (`open_via_osascript`). On Linux `xdg-open` and the
/// browser it starts carry the URL in their arguments, which `/proc` shows to every local
/// user: the honest fix is a one-use ticket issued by the panel instead of the key in the
/// URL, which belongs to the panel crate (review of 1 Oct 2026, finding 15).
pub fn open_in_browser(url: &str) {
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn()
        .map(|_| ());
    #[cfg(target_os = "macos")]
    let result = open_via_osascript(url).or_else(|_| {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map(|_| ())
    });
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    // Its complaints ("www-browser: not found" ... on a server with no browser) say nothing the
    // person can use: the link was printed above, to open where there is one.
    let result = std::process::Command::new("xdg-open")
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ());
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
mod panel_link_tests {
    use super::{applescript_string, link_text};

    /// The session key is never written where a log can keep it: the service line shows the
    /// address alone, the terminal line the full link (review of 1 Oct 2026, finding 15).
    #[test]
    fn the_service_line_has_no_key_and_the_terminal_line_has_it() {
        let addr: std::net::SocketAddr =
            "127.0.0.1:7443".parse().unwrap_or_else(|_| unreachable!());
        assert_eq!(
            link_text(Some(addr), "s3cret", true),
            "http://127.0.0.1:7443/?t=s3cret"
        );
        let service = link_text(Some(addr), "s3cret", false);
        assert_eq!(service, "http://127.0.0.1:7443/");
        assert!(!service.contains("s3cret"));
        assert_eq!(link_text(None, "s3cret", false), "");
    }

    #[test]
    fn applescript_literal_escapes_quotes_and_backslashes() {
        assert_eq!(
            applescript_string("http://127.0.0.1:7443/?t=abc"),
            "\"http://127.0.0.1:7443/?t=abc\""
        );
        assert_eq!(applescript_string("a\"b\\c"), "\"a\\\"b\\\\c\"");
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod apartarse_tests {
    use super::*;
    use guardiana_core::ChangeKind;

    fn ledger(tag: &str) -> Ledger {
        let dir = std::env::temp_dir().join(format!("guardiana-cli-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Ledger::open(&dir.join("ledger.db"), identity::genesis()).unwrap()
    }

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    /// Critical 3 of 5 Oct 2026: the router gives the computer its own address as DNS. That
    /// address, the loopback and "any" are never an upstream; the next list is used, and the
    /// system and the router are asked only when the lists before them had nothing.
    #[test]
    fn the_upstream_is_never_this_machine_and_falls_back_in_order() {
        let mut preguntado = false;
        let r = escoger_arriba(
            vec![ip("127.0.0.1"), ip("0.0.0.0")],
            || vec![ip("::1")],
            vec![ip("192.0.2.1")],
            vec![ip("198.51.100.7")],
            || {
                preguntado = true;
                Some(ip("192.0.2.254"))
            },
            |_| true,
        );
        assert_eq!(
            r,
            Ok((Fuente::Respaldo, vec!["192.0.2.1:53".parse().unwrap()]))
        );
        assert!(
            !preguntado,
            "the router is asked only when nothing else is left"
        );

        // The live list wins when it holds something else, and keeps only that.
        let r = escoger_arriba(
            vec![ip("127.0.0.53"), ip("192.0.2.9"), ip("192.0.2.9")],
            Vec::new,
            Vec::new(),
            Vec::new(),
            || None,
            |_| true,
        );
        assert_eq!(
            r,
            Ok((Fuente::Copia, vec!["192.0.2.9:53".parse().unwrap()]))
        );

        // The router, when it is all there is.
        let r = escoger_arriba(
            vec![ip("127.0.0.1")],
            Vec::new,
            Vec::new(),
            Vec::new(),
            || Some(ip("192.0.2.254")),
            |_| true,
        );
        assert_eq!(
            r,
            Ok((Fuente::Respaldo, vec!["192.0.2.254:53".parse().unwrap()]))
        );

        // Only this machine: said as such, not as "no resolver found".
        assert_eq!(
            escoger_arriba(
                vec![ip("127.0.0.1")],
                Vec::new,
                Vec::new(),
                Vec::new(),
                || None,
                |_| true
            ),
            Err(true)
        );
        assert_eq!(
            escoger_arriba(
                Vec::new(),
                Vec::new,
                Vec::new(),
                Vec::new(),
                || None,
                |_| true
            ),
            Err(false)
        );
    }

    /// The computer's real LAN address counts as this machine too, whatever it is here.
    #[test]
    fn the_lan_address_of_this_computer_is_this_machine() {
        if let Some(lan) = guardiana_devices::local_lan_ipv4() {
            assert!(sysdns::is_this_machine(IpAddr::V4(lan)));
            assert_eq!(
                escoger_arriba(
                    vec![IpAddr::V4(lan)],
                    Vec::new,
                    Vec::new(),
                    Vec::new(),
                    || None,
                    |_| true
                ),
                Err(true)
            );
        }
    }

    /// Second pass of 5 Oct 2026: the last good list is the café's, and at home it answers
    /// nothing. The one that answers wins, and the router is asked only if none before it did.
    #[test]
    fn a_fallback_that_does_not_answer_gives_way_to_one_that_does() {
        let cafe: Vec<SocketAddr> = vec!["10.10.0.1:53".parse().unwrap()];
        let r = escoger_arriba(
            vec![ip("127.0.0.1")],
            Vec::new,
            vec![ip("10.10.0.1")],
            vec![ip("198.51.100.7")],
            || Some(ip("192.168.1.1")),
            |v| v != cafe.as_slice(),
        );
        assert_eq!(
            r,
            Ok((Fuente::Respaldo, vec!["198.51.100.7:53".parse().unwrap()]))
        );
        // Only the router answers.
        let r = escoger_arriba(
            vec![ip("127.0.0.1")],
            Vec::new,
            vec![ip("10.10.0.1")],
            Vec::new(),
            || Some(ip("192.168.1.1")),
            |v| v[0].ip() == ip("192.168.1.1"),
        );
        assert_eq!(
            r,
            Ok((Fuente::Respaldo, vec!["192.168.1.1:53".parse().unwrap()]))
        );
        // Nobody answers (no network at all): the first list there is, as before.
        let r = escoger_arriba(
            vec![ip("127.0.0.1")],
            Vec::new,
            vec![ip("10.10.0.1")],
            Vec::new(),
            || Some(ip("192.168.1.1")),
            |_| false,
        );
        assert_eq!(
            r,
            Ok((Fuente::Respaldo, vec!["10.10.0.1:53".parse().unwrap()]))
        );
    }

    /// Critical 2 and serious 4: Home Mode is parked, not forgotten, and comes back with the
    /// licence, written into the ledger both times. (No DNS copy here: undoing it would touch
    /// the system DNS of the machine running the tests.)
    #[test]
    fn home_mode_is_parked_on_standing_down_and_back_with_the_licence() {
        let l = ledger("aparcar-hogar");
        l.set_setting(SETTING_HOME_MODE, "1").unwrap();
        stand_down(&l);
        assert_eq!(l.setting(SETTING_HOME_MODE).unwrap().as_deref(), Some("0"));
        assert_eq!(
            l.setting(SETTING_HOME_APARCADO).unwrap().as_deref(),
            Some("1")
        );
        // Standing down twice (each pass of a stood-down program does it) changes nothing.
        stand_down(&l);
        assert_eq!(
            l.setting(SETTING_HOME_APARCADO).unwrap().as_deref(),
            Some("1")
        );
        let (dns, hogar) = volver_del_apartado(&l);
        assert!(!dns, "there was no DNS copy to bring back");
        assert!(hogar);
        assert_eq!(l.setting(SETTING_HOME_MODE).unwrap().as_deref(), Some("1"));
        assert_eq!(
            l.setting(SETTING_HOME_APARCADO).unwrap().as_deref(),
            Some("")
        );
        let kinds: Vec<(ChangeKind, String)> = l
            .changes(10)
            .unwrap()
            .into_iter()
            .map(|c| (c.kind, c.who))
            .collect();
        assert!(kinds.contains(&(ChangeKind::HogarOff, "licencia".to_owned())));
        assert!(kinds.contains(&(ChangeKind::HogarOn, "licencia".to_owned())));
        assert_eq!(relevo_wanted(&l), None, "nothing parked, no relay");
    }

    /// The parked DNS copy becomes the copy again on return, so the next pass points the
    /// machine back at Guardiana once it listens; and it never overwrites a newer one.
    #[test]
    fn the_parked_dns_copy_comes_back_with_the_licence() {
        let l = ledger("aparcar-dns");
        let parked = Backup {
            taken_at: 1,
            method: sysdns::Method::ResolvConf,
            interfaces: vec![sysdns::InterfaceDns {
                id: "eth0".into(),
                name: "eth0".into(),
                servers: vec![ip("192.0.2.1")],
                automatic: true,
                extra: None,
            }],
            resolv_conf: Some("nameserver 192.0.2.1\n".into()),
            resolv_link: None,
            made_dropin_dir: false,
        };
        l.set_setting(
            SETTING_BACKUP_APARCADA,
            &serde_json::to_string(&parked).unwrap(),
        )
        .unwrap();
        let (dns, hogar) = volver_del_apartado(&l);
        assert!(dns);
        assert!(!hogar);
        let copia = l.setting(SETTING_BACKUP).unwrap().unwrap_or_default();
        assert!(serde_json::from_str::<Backup>(&copia).is_ok());
        assert_eq!(
            l.setting(SETTING_BACKUP_APARCADA).unwrap().as_deref(),
            Some("")
        );
        assert!(l
            .changes(10)
            .unwrap()
            .iter()
            .any(|c| c.kind == ChangeKind::DnsOn && c.who == "licencia"));

        // A copy that is already there (the person pointed the DNS again by hand) stays.
        l.set_setting(SETTING_BACKUP, "{\"mine\":1}").unwrap();
        l.set_setting(
            SETTING_BACKUP_APARCADA,
            &serde_json::to_string(&parked).unwrap(),
        )
        .unwrap();
        let (dns, _) = volver_del_apartado(&l);
        assert!(!dns);
        assert_eq!(
            l.setting(SETTING_BACKUP).unwrap().as_deref(),
            Some("{\"mine\":1}")
        );
        assert_eq!(
            l.setting(SETTING_BACKUP_APARCADA).unwrap().as_deref(),
            Some("")
        );
    }
}

#[cfg(test)]
mod reglas_tests {
    use super::*;
    use guardiana_core::{Action, MatchKind, Scope};

    /// A rule 1.0.1 stored as typed matches the name it meant (G4, 28 Sep 2026).
    #[test]
    fn a_rule_stored_as_typed_is_read_as_a_name() {
        let regla = |pattern: &str, kind: MatchKind| Rule {
            id: 1,
            scope: Scope::Home,
            device_id: None,
            match_kind: kind,
            pattern: pattern.to_owned(),
            action: Action::Cortar,
            created_at: 0,
            created_by: "panel".to_owned(),
            expires_at: None,
            undone_at: None,
            confirmed: false,
        };
        let r = reglas_legibles(vec![
            regla("https://www.tiktok.com/@x", MatchKind::Domain),
            regla("*.ejemplo.com", MatchKind::Domain),
            regla("rastreador", MatchKind::Category),
        ]);
        assert_eq!(r[0].pattern, "www.tiktok.com");
        assert_eq!(r[0].match_kind, MatchKind::Domain);
        assert_eq!(r[1].pattern, "ejemplo.com");
        assert_eq!(r[1].match_kind, MatchKind::Suffix);
        assert_eq!(r[2].pattern, "rastreador");
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod heredado_tests {
    use super::*;
    use guardiana_core::{ChangeKind, ChangeWho};

    fn ledger(tag: &str) -> Ledger {
        let dir = std::env::temp_dir().join(format!("guardiana-cli-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Ledger::open(&dir.join("ledger.db"), identity::genesis()).unwrap()
    }

    /// What 1.0.1 left on a machine whose trial ended: Home Mode off "by the licence", nothing
    /// parked. 1.0.2 parks it once, so the house gets its relay and Home Mode returns with Plus.
    #[test]
    fn what_101_left_is_parked_once() {
        let l = ledger("heredado-101");
        l.record_change(1, ChangeKind::HogarOn, ChangeWho::Panel, "192.168.1.20")
            .unwrap();
        l.record_change(2, ChangeKind::HogarOff, ChangeWho::Licencia, "")
            .unwrap();
        l.set_setting(SETTING_HOME_MODE, "0").unwrap();
        stand_down(&l);
        assert_eq!(
            l.setting(SETTING_HOME_APARCADO).unwrap().as_deref(),
            Some("1")
        );
        // Switched off by hand afterwards: never parked again.
        l.set_setting(SETTING_HOME_APARCADO, "").unwrap();
        stand_down(&l);
        assert_eq!(
            l.setting(SETTING_HOME_APARCADO).unwrap().as_deref(),
            Some("")
        );
    }

    /// A change the person made after the licence one wins: nothing is parked.
    #[test]
    fn a_newer_change_by_the_person_is_left_alone() {
        let l = ledger("heredado-persona");
        l.record_change(2, ChangeKind::HogarOff, ChangeWho::Licencia, "")
            .unwrap();
        l.record_change(3, ChangeKind::HogarOff, ChangeWho::Panel, "")
            .unwrap();
        stand_down(&l);
        assert_eq!(l.setting(SETTING_HOME_APARCADO).unwrap(), None);
    }
}
