//! Local DNS resolver (brief §4): listen, forward, cache, block answers,
//! canary and checker name.
//!
//! The resolver itself decides nothing about *what* to block: a [`Policy`]
//! supplied by the caller answers "forward or block?" for every query and is
//! told afterwards what happened, so it can write the ledger. This crate
//! never stores anything and never opens a connection the user did not
//! configure as upstream.
//!
//! Built-in answers, always visible as such in the outcome:
//! - the Firefox canary `use-application-dns.net` → NXDOMAIN when enabled;
//! - the checker `comprobar.guardiana.hogar` → the LAN IP of this computer.
//!
//! Only queries are ever handed to the policy: name, type, client and time.
//! Answers are never handed out or stored (brief §4).

mod handler;
pub mod probe;
mod upstream;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use hickory_server::server::Server;
use tokio::net::{TcpListener, UdpSocket};

pub use hickory_proto::rr::RecordType;

/// Name Firefox queries to decide whether to use its own DoH (brief §4).
pub const CANARY_NAME: &str = "use-application-dns.net";
/// Name that only Guardiana resolves; loading it proves a device goes through us (brief §4).
pub const CHECKER_NAME: &str = "comprobar.guardiana.hogar";
/// TTL of blocked and built-in answers: short, so an undo takes effect fast.
pub const BUILTIN_TTL: u32 = 30;

/// How a blocked name is answered (brief §4: NXDOMAIN by default, `0.0.0.0` optional).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlockMode {
    /// Answer "this name does not exist".
    #[default]
    NxDomain,
    /// Answer `0.0.0.0` / `::` for A / AAAA and an empty answer otherwise.
    ZeroIp,
}

/// Resolver configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// Addresses to listen on, UDP and TCP. Each must be loopback or a private LAN address.
    pub listen: Vec<SocketAddr>,
    /// Upstream resolvers, tried in order: the ones the system had before Guardiana.
    pub upstreams: Vec<SocketAddr>,
    /// How blocked names are answered.
    pub block_mode: BlockMode,
    /// Whether the Firefox canary is answered NXDOMAIN.
    pub canary_enabled: bool,
    /// LAN IP answered for the checker name, if Home Mode is on.
    pub checker_ip: Option<Ipv4Addr>,
    /// Maximum cached answers.
    pub cache_size: u64,
    /// Upstream timeout per attempt.
    pub upstream_timeout: Duration,
}

impl Config {
    /// Listen on loopback port 53 and forward to `upstreams`, everything else default.
    #[must_use]
    pub fn local(upstreams: Vec<SocketAddr>) -> Self {
        Self {
            listen: vec![SocketAddr::from((Ipv4Addr::LOCALHOST, 53))],
            upstreams,
            block_mode: BlockMode::NxDomain,
            canary_enabled: false,
            checker_ip: None,
            cache_size: 4096,
            upstream_timeout: Duration::from_secs(2),
        }
    }
}

/// One incoming query, as handed to the [`Policy`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    /// Where the query came from.
    pub client: SocketAddr,
    /// Queried name, lowercase, without trailing dot.
    pub name: String,
    /// Query type.
    pub qtype: RecordType,
    /// Unix time in milliseconds when it arrived.
    pub ts: i64,
}

/// What the policy wants done with a query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Forward upstream (or answer from cache).
    Forward,
    /// Answer as blocked; `rule_id` names the user rule that caused it.
    Block {
        /// The rule that decided it, for the ledger.
        rule_id: Option<i64>,
    },
}

/// What actually happened to a query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Forwarded (possibly served from cache) and answered with this code.
    Forwarded {
        /// DNS response code sent back, e.g. `NoError` or `NXDomain`.
        rcode: hickory_proto::op::ResponseCode,
    },
    /// Upstream did not answer; SERVFAIL was returned.
    UpstreamFailed,
    /// Blocked by a rule.
    Blocked {
        /// The rule that decided it.
        rule_id: Option<i64>,
    },
    /// The Firefox canary was answered NXDOMAIN.
    Canary,
    /// The checker name was answered with the LAN IP.
    Checker,
    /// The request was malformed or unsupported and was refused.
    Refused,
}

/// Decides and records. Implemented by the caller (CLI now, service later).
/// Both methods run on the resolver's hot path: keep them fast.
pub trait Policy: Send + Sync + 'static {
    /// Forward or block? Called before answering.
    fn decide(&self, query: &Query) -> Decision;
    /// Called after the answer was sent, with what happened.
    fn record(&self, query: &Query, outcome: Outcome);
}

/// Errors starting the resolver.
#[derive(Debug)]
pub enum Error {
    /// A listen address is neither loopback nor a private LAN address (brief §4).
    NotPrivate(SocketAddr),
    /// No upstream resolver was configured.
    NoUpstream,
    /// Binding a socket failed.
    Bind(SocketAddr, std::io::Error),
    /// The upstream client could not be built.
    Upstream(String),
    /// The server stopped with an error.
    Server(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotPrivate(a) => write!(f, "refusing to listen on non-private address {a}"),
            Self::NoUpstream => f.write_str("no upstream resolver configured"),
            Self::Bind(a, e) => write!(f, "cannot bind {a}: {e}"),
            Self::Upstream(e) => write!(f, "upstream client: {e}"),
            Self::Server(e) => write!(f, "server: {e}"),
        }
    }
}

impl std::error::Error for Error {}

/// True for loopback, RFC 1918, link-local and IPv6 ULA/link-local/loopback.
/// `0.0.0.0` and `::` are never private: they would expose the port everywhere.
#[must_use]
pub fn is_private_listen_addr(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local(),
    }
}

/// A running resolver. Drop it or call [`Running::shutdown`] to stop.
pub struct Running {
    /// UDP addresses actually bound (useful when port 0 was requested).
    pub udp_addrs: Vec<SocketAddr>,
    /// TCP addresses actually bound.
    pub tcp_addrs: Vec<SocketAddr>,
    shutdown: Box<dyn Fn() + Send + Sync>,
    task: tokio::task::JoinHandle<Result<(), Error>>,
}

impl Running {
    /// Ask the server to stop.
    pub fn shutdown(&self) {
        (self.shutdown)();
    }

    /// Wait until the server stops.
    pub async fn wait(self) -> Result<(), Error> {
        match self.task.await {
            Ok(r) => r,
            Err(e) => Err(Error::Server(e.to_string())),
        }
    }
}

/// Bind every listen address and start serving with `policy`.
pub async fn start<P: Policy>(config: Config, policy: P) -> Result<Running, Error> {
    if config.upstreams.is_empty() {
        return Err(Error::NoUpstream);
    }
    for addr in &config.listen {
        if !is_private_listen_addr(addr.ip()) {
            return Err(Error::NotPrivate(*addr));
        }
    }
    let upstream = upstream::build(&config)?;
    let handler = handler::Handler {
        policy: Arc::new(policy),
        upstream,
        block_mode: config.block_mode,
        canary_enabled: config.canary_enabled,
        checker_ip: config.checker_ip,
    };
    let mut server = Server::new(handler);
    let mut udp_addrs = Vec::new();
    let mut tcp_addrs = Vec::new();
    for addr in &config.listen {
        let udp = UdpSocket::bind(addr)
            .await
            .map_err(|e| Error::Bind(*addr, e))?;
        udp_addrs.push(udp.local_addr().map_err(|e| Error::Bind(*addr, e))?);
        server.register_socket(udp);
        let tcp = TcpListener::bind(addr)
            .await
            .map_err(|e| Error::Bind(*addr, e))?;
        tcp_addrs.push(tcp.local_addr().map_err(|e| Error::Bind(*addr, e))?);
        server.register_listener(tcp, Duration::from_secs(5), 4096);
    }
    let token = server.shutdown_token().clone();
    let task = tokio::spawn(async move {
        server
            .block_until_done()
            .await
            .map_err(|e| Error::Server(e.to_string()))
    });
    Ok(Running {
        udp_addrs,
        tcp_addrs,
        shutdown: Box::new(move || token.cancel()),
        task,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn private_listen_addresses() {
        for ok in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.9",
            "192.168.1.20",
            "::1",
            "fd00::1",
        ] {
            let ip: IpAddr = ok.parse().expect("ip");
            assert!(is_private_listen_addr(ip), "{ok}");
        }
        for bad in ["0.0.0.0", "8.8.8.8", "203.0.113.5", "::", "2001:db8::1"] {
            let ip: IpAddr = bad.parse().expect("ip");
            assert!(!is_private_listen_addr(ip), "{bad}");
        }
    }
}
