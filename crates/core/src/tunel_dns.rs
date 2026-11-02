//! The VPN's own resolver, while the tunnel carries everything (1.0.54).
//!
//! GUARDIANA's DNS (the resolver on 127.0.0.1) forwards every name it does not cut to the resolvers the network gave,
//! usually the home router. With the VPN on, the router is outside the tunnel, and GUARDIANA's service is the one
//! program the cut-off switch lets out: the names went to the router in the clear while the screen said «conectada»
//! (second review of 2 Nov 2026). While a [`Desvio`] lives, the resolver asks only the tunnel's own resolver, inside the
//! tunnel; if that one does not answer, the name fails rather than going outside.
//!
//! One value for the whole process: the tunnel and the resolver live in the same service.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{PoisonError, RwLock};

/// The resolver in force and the [`Desvio`] that put it there.
static ACTUAL: RwLock<Option<(u64, SocketAddr)>> = RwLock::new(None);
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
    /// From now on, every name GUARDIANA's resolver forwards goes to `resolutor` and nowhere else.
    pub fn pon(resolutor: SocketAddr) -> Self {
        let id = SIGUIENTE.fetch_add(1, Ordering::Relaxed);
        *ACTUAL.write().unwrap_or_else(PoisonError::into_inner) = Some((id, resolutor));
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

/// Where every forwarded name must go right now: `Some` while the VPN carries everything, `None` otherwise (then the
/// network's resolvers, as always).
#[must_use]
pub fn actual() -> Option<SocketAddr> {
    let actual = *ACTUAL.read().unwrap_or_else(PoisonError::into_inner);
    actual.map(|(_, resolutor)| resolutor)
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
        assert_eq!(actual(), None);
        let primero = Desvio::pon(a);
        assert_eq!(actual(), Some(a));
        // a newer one takes its place, and the old one going away does not take the new one with it
        let segundo = Desvio::pon(b);
        assert_eq!(actual(), Some(b));
        drop(primero);
        assert_eq!(actual(), Some(b));
        drop(segundo);
        assert_eq!(actual(), None);
    }
}
