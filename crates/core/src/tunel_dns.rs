//! The VPN's own resolver, while the tunnel carries everything (1.0.54), and this computer's LAN address from before the
//! tunnel (1.0.55).
//!
//! GUARDIANA's DNS (the resolver on 127.0.0.1) forwards every name it does not cut to the resolvers the network gave,
//! usually the home router. With the VPN on, the router is outside the tunnel, and until 1.0.55 GUARDIANA's service was
//! let out of the cut-off switch anywhere: the names went to the router in the clear while the screen said «conectada»
//! (second review of 2 Nov 2026). While a [`Desvio`] lives, the resolver asks only the tunnel's own resolver, inside the
//! tunnel; if that one does not answer, the name fails rather than going outside.
//!
//! The LAN address: once the tunnel has the default route, the interface that reaches «anywhere» is the tunnel's, and
//! the usual way of reading this computer's LAN address gave the tunnel's own. Home Mode moved its listeners there, and
//! the house lost its DNS (third review of 2 Nov 2026). The tunnel reads the address before it takes the route and puts
//! it here with its resolver; while the [`Desvio`] lives, [`lan`] answers that one.
//!
//! One value for the whole process: the tunnel and the resolver live in the same service.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{PoisonError, RwLock};

/// What a [`Desvio`] keeps in force.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnVigor {
    /// The tunnel's resolver: every forwarded name goes there and nowhere else.
    pub resolutor: SocketAddr,
    /// This computer's LAN address from before the tunnel took the default route; `None` when it had none.
    pub lan: Option<Ipv4Addr>,
}

/// What is in force and the [`Desvio`] that put it there.
static ACTUAL: RwLock<Option<(u64, EnVigor)>> = RwLock::new(None);
/// The number of the next [`Desvio`].
static SIGUIENTE: AtomicU64 = AtomicU64::new(1);

/// The tunnel's resolver, in force while this value lives. Dropping it gives the DNS back to the network's resolvers,
/// unless a newer one has taken its place.
#[derive(Debug)]
#[must_use = "the tunnel's resolver is in force only while this value lives"]
pub struct Desvio {
    id: u64,
}

impl Desvio {
    /// From now on, every name GUARDIANA's resolver forwards goes to `resolutor` and nowhere else, and [`lan`] answers
    /// `lan`: this computer's LAN address, read ([`lan`]) before the tunnel took the default route.
    pub fn pon(resolutor: SocketAddr, lan: Option<Ipv4Addr>) -> Self {
        let id = SIGUIENTE.fetch_add(1, Ordering::Relaxed);
        *ACTUAL.write().unwrap_or_else(PoisonError::into_inner) =
            Some((id, EnVigor { resolutor, lan }));
        Self { id }
    }
}

impl Drop for Desvio {
    fn drop(&mut self) {
        let mut actual = ACTUAL.write().unwrap_or_else(PoisonError::into_inner);
        if matches!(*actual, Some((id, _)) if id == self.id) {
            *actual = None;
        }
    }
}

/// What the newest living [`Desvio`] keeps in force; `None` while the VPN does not carry everything.
#[must_use]
pub fn en_vigor() -> Option<EnVigor> {
    let actual = *ACTUAL.read().unwrap_or_else(PoisonError::into_inner);
    actual.map(|(_, e)| e)
}

/// Where every forwarded name must go right now: `Some` while the VPN carries everything, `None` otherwise (then the
/// network's resolvers, as always).
#[must_use]
pub fn actual() -> Option<SocketAddr> {
    en_vigor().map(|e| e.resolutor)
}

/// This computer's LAN address (IPv4), the one Home Mode listens on and the panel and the devices take as this
/// computer's: while a [`Desvio`] lives, the one from before the tunnel took the default route, as long as this
/// computer still holds it (after a new lease under the tunnel there is none until the tunnel closes: never the
/// tunnel's address, and never one Home Mode could not listen on, which would keep the panel from starting);
/// otherwise the address of the interface that reaches the default route now. `None` when offline.
#[must_use]
pub fn lan() -> Option<Ipv4Addr> {
    match en_vigor() {
        Some(e) => e.lan.filter(|l| UdpSocket::bind((*l, 0)).is_ok()),
        None => hacia_fuera(),
    }
}

/// The address of the interface the default route leaves from, read from a connected UDP socket, which sends nothing.
fn hacia_fuera() -> Option<Ipv4Addr> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    // Documentation-range address: never actually contacted.
    sock.connect(SocketAddr::from((Ipv4Addr::new(192, 0, 2, 1), 53)))
        .ok()?;
    match sock.local_addr().ok()?.ip() {
        IpAddr::V4(v4) if !v4.is_loopback() && !v4.is_unspecified() => Some(v4),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// One test for the whole module: the value is the process's, and tests run in parallel.
    #[test]
    fn el_desvio_dura_lo_que_dura_su_valor() {
        let a: SocketAddr = "10.66.0.1:53".parse().unwrap();
        let b: SocketAddr = "10.66.0.2:53".parse().unwrap();
        // an address this computer holds (the test machine's LAN may be anything), and one it does not
        let casa = Ipv4Addr::LOCALHOST;
        let ida = Ipv4Addr::new(192, 0, 2, 10);
        assert_eq!(actual(), None);
        assert_eq!(en_vigor(), None);
        // without the VPN, the LAN address is read from the default route
        assert_eq!(lan(), hacia_fuera());
        let primero = Desvio::pon(a, Some(casa));
        assert_eq!(actual(), Some(a));
        // with it, the one from before the tunnel, never the tunnel's own
        assert_eq!(lan(), Some(casa));
        // a newer one takes its place, and the old one going away does not take the new one with it
        let segundo = Desvio::pon(b, None);
        assert_eq!(actual(), Some(b));
        assert_eq!(lan(), None);
        drop(primero);
        assert_eq!(actual(), Some(b));
        assert_eq!(
            en_vigor(),
            Some(EnVigor {
                resolutor: b,
                lan: None
            })
        );
        drop(segundo);
        assert_eq!(actual(), None);
        assert_eq!(lan(), hacia_fuera());
        // the address from before the tunnel is gone (a new lease under the tunnel): none, not that one
        let tercero = Desvio::pon(a, Some(ida));
        assert_eq!(en_vigor().and_then(|e| e.lan), Some(ida));
        assert_eq!(lan(), None);
        drop(tercero);
        assert_eq!(actual(), None);
    }
}
