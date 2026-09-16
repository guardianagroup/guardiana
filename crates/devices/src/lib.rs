//! LAN device identification (brief §7): the IP a query came from → the MAC
//! in the system's neighbour table (ARP/NDP) → the name the user gave it.
//! If the IP changes and the MAC matches, it is the same device.
//!
//! Nothing here sends packets. The neighbour table is read from the OS and
//! cached for a short time so the resolver's hot path never waits on it.

mod neighbors;

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use guardiana_core::SELF_DEVICE_ID;

pub use neighbors::{parse_arp_a, parse_ip_neigh, parse_proc_net_arp, read_neighbors, Neighbor};

/// Who a query came from, as the ledger wants it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    /// Stable id: `self`, `mac:aa:bb:cc:dd:ee:ff`, or `ip:<ip>` while the MAC is unknown.
    pub id: String,
    /// MAC address, lowercase with colons, when known.
    pub mac: Option<String>,
    /// The IP the query came from.
    pub ip: IpAddr,
}

/// Normalise a MAC to lowercase, colon separated.
#[must_use]
pub fn normalize_mac(raw: &str) -> Option<String> {
    let parts: Vec<String> = raw
        .split(['-', ':', '.'])
        .map(|p| p.to_ascii_lowercase())
        .collect();
    if parts.len() != 6
        || !parts
            .iter()
            .all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_hexdigit()))
    {
        return None;
    }
    let mac = parts.join(":");
    // Incomplete or broadcast entries are not devices.
    if mac == "00:00:00:00:00:00" || mac == "ff:ff:ff:ff:ff:ff" {
        return None;
    }
    Some(mac)
}

/// Device id for a MAC.
#[must_use]
pub fn id_for_mac(mac: &str) -> String {
    format!("mac:{mac}")
}

/// Device id used while the MAC is not known yet.
#[must_use]
pub fn id_for_ip(ip: IpAddr) -> String {
    format!("ip:{ip}")
}

struct Cache {
    table: HashMap<IpAddr, String>,
    refreshed: Option<Instant>,
}

/// Resolves client IPs to identities with a cached neighbour table.
pub struct Resolver {
    cache: Mutex<Cache>,
    ttl: Duration,
}

impl Default for Resolver {
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}

impl Resolver {
    /// A resolver that re-reads the neighbour table at most every `ttl`.
    #[must_use]
    pub fn new(ttl: Duration) -> Self {
        Self {
            cache: Mutex::new(Cache {
                table: HashMap::new(),
                refreshed: None,
            }),
            ttl,
        }
    }

    fn refresh_if_stale(&self, cache: &mut Cache, force: bool) {
        let stale = cache.refreshed.is_none_or(|t| t.elapsed() > self.ttl);
        if stale || force {
            for n in read_neighbors() {
                cache.table.insert(n.ip, n.mac);
            }
            cache.refreshed = Some(Instant::now());
        }
    }

    /// Identify `ip`. Loopback is this computer. A LAN IP not yet in the
    /// neighbour table gets a temporary `ip:` id; the next refresh may
    /// upgrade it to its MAC.
    pub fn identify(&self, ip: IpAddr) -> Identity {
        // Loopback is this computer; so is a query that arrives through this computer's own
        // LAN address (the PC asking the guardian by its LAN IP is still the PC, not a device).
        if ip.is_loopback() || local_lan_ipv4().is_some_and(|lan| ip == IpAddr::V4(lan)) {
            return Identity {
                id: SELF_DEVICE_ID.to_owned(),
                mac: None,
                ip,
            };
        }
        let Ok(mut cache) = self.cache.lock() else {
            return Identity {
                id: id_for_ip(ip),
                mac: None,
                ip,
            };
        };
        self.refresh_if_stale(&mut cache, false);
        if !cache.table.contains_key(&ip) {
            // A new client: the table has probably just learned it. Read once
            // more, but only if the last read is older than a couple of seconds.
            let recent = cache
                .refreshed
                .is_some_and(|t| t.elapsed() < Duration::from_secs(2));
            if !recent {
                self.refresh_if_stale(&mut cache, true);
            }
        }
        match cache.table.get(&ip) {
            Some(mac) => Identity {
                id: id_for_mac(mac),
                mac: Some(mac.clone()),
                ip,
            },
            None => Identity {
                id: id_for_ip(ip),
                mac: None,
                ip,
            },
        }
    }
}

/// The IPv4 address of the interface that reaches the LAN/default route.
/// Uses a connected UDP socket, which sends nothing. `None` when offline.
#[must_use]
pub fn local_lan_ipv4() -> Option<Ipv4Addr> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    // Documentation-range address: never actually contacted.
    sock.connect(SocketAddr::from((Ipv4Addr::new(192, 0, 2, 1), 53)))
        .ok()?;
    match sock.local_addr().ok()?.ip() {
        IpAddr::V4(v4) if !v4.is_loopback() && !v4.is_unspecified() => Some(v4),
        _ => None,
    }
}

/// True for RFC 1918 and link-local IPv4, or IPv6 ULA/link-local: a home network.
#[must_use]
pub fn is_private_lan(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => v6.is_unique_local() || v6.is_unicast_link_local(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_normalisation() {
        assert_eq!(
            normalize_mac("A4-B1-C1-D2-E3-F4").as_deref(),
            Some("a4:b1:c1:d2:e3:f4")
        );
        assert_eq!(
            normalize_mac("a4:b1:c1:d2:e3:f4").as_deref(),
            Some("a4:b1:c1:d2:e3:f4")
        );
        assert_eq!(normalize_mac("ff-ff-ff-ff-ff-ff"), None);
        assert_eq!(normalize_mac("(incomplete)"), None);
        assert_eq!(normalize_mac("a4:b1:c1"), None);
    }

    #[test]
    fn loopback_is_self_and_unknown_lan_gets_ip_id() {
        let r = Resolver::new(Duration::from_secs(3600));
        let me = r.identify(
            "127.0.0.1"
                .parse()
                .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST)),
        );
        assert_eq!(me.id, SELF_DEVICE_ID);
        let ip: IpAddr = "198.51.100.77"
            .parse()
            .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
        let who = r.identify(ip);
        assert!(who.id == "ip:198.51.100.77" || who.id.starts_with("mac:"));
    }

    #[test]
    fn lan_ip_detection_does_not_panic() {
        let _ = local_lan_ipv4();
        assert!(is_private_lan(
            "192.168.1.5"
                .parse()
                .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
        ));
        assert!(!is_private_lan(
            "8.8.8.8".parse().unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
        ));
    }
}
