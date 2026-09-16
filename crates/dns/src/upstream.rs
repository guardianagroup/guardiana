//! The forwarding client: hickory's resolver with its TTL-respecting cache,
//! pointed at the upstreams the user had before Guardiana (brief §4).

use hickory_resolver::config::{
    ConnectionConfig, NameServerConfig, ResolveHosts, ResolverConfig, ResolverOpts,
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
    opts.cache_size = config.cache_size;
    opts.use_hosts_file = ResolveHosts::Never;
    opts.edns0 = true;
    TokioResolver::builder_with_config(resolver_config, TokioRuntimeProvider::default())
        .with_options(opts)
        .build()
        .map_err(|e| Error::Upstream(e.to_string()))
}
