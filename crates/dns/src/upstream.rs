//! The forwarding client: hickory's resolver with its TTL-respecting cache,
//! pointed at the upstreams the user had before Guardiana (brief §4).

use hickory_resolver::config::{
    ConnectionConfig, NameServerConfig, ResolveHosts, ResolverConfig, ResolverOpts,
    ServerOrderingStrategy,
};
use hickory_resolver::net::runtime::TokioRuntimeProvider;
use hickory_resolver::TokioResolver;

use crate::{Config, Error};

pub(crate) fn build(config: &Config) -> Result<TokioResolver, Error> {
    let mut servers = Vec::with_capacity(config.upstreams.len());
    for addr in &config.upstreams {
        let mut udp = ConnectionConfig::udp();
        udp.port = addr.port();
        let mut tcp = ConnectionConfig::tcp();
        tcp.port = addr.port();
        servers.push(NameServerConfig::new(addr.ip(), true, vec![udp, tcp]));
    }
    let resolver_config = ResolverConfig::from_name_servers(servers);
    let mut opts = ResolverOpts::default();
    opts.timeout = config.upstream_timeout;
    opts.attempts = 2;
    // One resolver at a time: hickory's default sends every query to two servers at once, so a
    // name went to the router and to 1.1.1.1 both, and "the user chooses the resolver" was not
    // true (review of 8 Oct 2026). The order follows the measured answers: the pool's deadline
    // is one timeout for the whole list, so a strict user order with a dead first server would
    // never reach the second; with the statistics, a server that does not answer is moved back
    // and the next query goes to the one that does.
    opts.num_concurrent_reqs = 1;
    opts.server_ordering_strategy = ServerOrderingStrategy::QueryStatistics;
    // Random letter case in the question (0x20): the answer has to echo it, which makes a
    // forged reply from outside that much harder to slip in.
    opts.case_randomization = true;
    opts.cache_size = config.cache_size;
    opts.use_hosts_file = ResolveHosts::Never;
    opts.edns0 = true;
    TokioResolver::builder_with_config(resolver_config, TokioRuntimeProvider::default())
        .with_options(opts)
        .build()
        .map_err(|e| Error::Upstream(e.to_string()))
}
