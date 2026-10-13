//! Where the server of a name nobody owns is (the owner, 13 Oct 2026: «que no salga ninguna sin
//! país»).
//!
//! Most cut names belong to a company the lists know, with its seat. The rest, tens of
//! thousands of throwaway advert and malware names in EasyList and URLhaus, belong to nobody
//! anyone has named. For those the one honest fact left is where their server is: the name is
//! resolved (the same question the browser would have asked the system before connecting, and
//! nothing is sent to the name itself) and its address is looked up in the country and network
//! tables that come inside the program (DB-IP, decision 203 of GUARDIANA). The address is not
//! kept: only the country and the network's name, in memory, for as long as ZERO is open.
//!
//! It runs on a thread of its own, only when the person opens «Lo que se cortó», and the page
//! asks again once the answers are in.

use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::{Arc, Mutex};

/// What the tables say about a name's server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Info {
    /// Its country, when the tables give one (never for anycast servers).
    pub pais: Option<&'static str>,
    /// The organisation that announces the address (`Amazon.com, Inc.`), when known.
    pub red: Option<&'static str>,
    /// Served from many countries at once by its operator's own word.
    pub anycast: bool,
}

/// How a name is turned into an address: the system's resolver, or a fixed answer in tests.
pub type Resuelve = fn(&str) -> Option<IpAddr>;

/// The system's resolver.
#[must_use]
pub fn del_sistema(host: &str) -> Option<IpAddr> {
    (host, 443)
        .to_socket_addrs()
        .ok()?
        .map(|s| s.ip())
        .find(|ip| !guardiana_lists::es_local(*ip))
}

/// The names asked for and the answers, shared with the thread that resolves them.
#[derive(Default)]
struct Estado {
    hechas: HashMap<String, Option<Info>>,
    cola: VecDeque<String>,
    trabajando: bool,
}

/// Looks up the servers of names, on its own thread, remembering the answers.
#[derive(Clone)]
pub struct Servidores {
    estado: Arc<Mutex<Estado>>,
    resuelve: Option<Resuelve>,
}

impl std::fmt::Debug for Servidores {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Servidores")
    }
}

/// How many names are remembered at most.
const MAX: usize = 20_000;

impl Servidores {
    /// With this way of resolving; `None` never resolves anything (tests that must not touch
    /// the network).
    #[must_use]
    pub fn con(resuelve: Option<Resuelve>) -> Self {
        Self {
            estado: Arc::new(Mutex::new(Estado::default())),
            resuelve,
        }
    }

    /// What is known about `host`'s server: `Some(answer)` once looked up (`Some(None)` when it
    /// could not be), `None` while pending; asking queues it.
    pub fn mira(&self, host: &str) -> Option<Option<Info>> {
        let resuelve = self.resuelve?;
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        let Ok(mut e) = self.estado.lock() else {
            return Some(None);
        };
        if let Some(r) = e.hechas.get(&host) {
            return Some(r.clone());
        }
        if !e.cola.contains(&host) && e.hechas.len() < MAX {
            e.cola.push_back(host);
        }
        if !e.trabajando {
            e.trabajando = true;
            let estado = Arc::clone(&self.estado);
            std::thread::spawn(move || trabaja(&estado, resuelve));
        }
        None
    }

    /// How many names are still waiting for an answer.
    #[must_use]
    pub fn pendientes(&self) -> usize {
        self.estado.lock().map_or(0, |e| e.cola.len())
    }
}

fn trabaja(estado: &Mutex<Estado>, resuelve: Resuelve) {
    loop {
        let host = {
            let Ok(mut e) = estado.lock() else { return };
            match e.cola.pop_front() {
                Some(h) => h,
                None => {
                    e.trabajando = false;
                    return;
                }
            }
        };
        let info = resuelve(&host).map(|ip| {
            let s = guardiana_lists::servidor_de(ip);
            Info {
                pais: s.pais,
                red: s.red.map(|r| r.nombre).filter(|n| !n.is_empty()),
                anycast: s.anycast,
            }
        });
        if let Ok(mut e) = estado.lock() {
            e.hechas.insert(host, info);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn fijo(_: &str) -> Option<IpAddr> {
        "8.8.4.4".parse().ok()
    }

    #[test]
    fn a_name_is_looked_up_once_on_its_own_thread() {
        let s = Servidores::con(Some(fijo));
        assert!(s.mira("x.example").is_none());
        let mut r = None;
        for _ in 0..200 {
            if let Some(v) = s.mira("x.example") {
                r = Some(v);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(r.is_some(), "never answered");
        assert_eq!(s.pendientes(), 0);
        // Without a resolver nothing is ever asked.
        let nada = Servidores::con(None);
        assert!(nada.mira("x.example").is_none());
        assert_eq!(nada.pendientes(), 0);
    }
}
