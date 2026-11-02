//! The server half of GUARDIANA's VPN (docs/VPN.md, «Los servidores»). On a server it is built and installed by the
//! website's `zero-motor/vpn.sh`, from a commit fixed there, behind Caddy and with WireGuard's own privilege only.
//!
//! One thing only: the anonymous sign-up. A computer sends its WireGuard public key and a fresh ML-KEM-768 key
//! ([`guardiana_tunel::alta::Pide`]); this side encapsulates a secret to it, derives the same post-quantum preshared key
//! the computer will derive, gives the key an address inside the tunnel and adds it as a WireGuard peer
//! ([`guardiana_tunel::alta::Respuesta`]). No account, no name, nothing written to disk: the peers live in the kernel's
//! memory and in this process's, and a peer without traffic for a day is removed ([`Estado::limpia`]).
//!
//! When the service starts again (an update of the server, a crash), it first takes over the peers WireGuard already
//! has ([`Estado::adopta`]): the computers connected keep their tunnel, and none of their addresses is given to another.
//!
//! Against abuse, without keeping anything about anyone: a cap of sign-ups per minute for the whole server, and a smaller
//! one per origin. The origin is the address the web server in front saw, held only as a hash whose key is drawn again
//! at every start, in this process's memory, and only for that minute.

use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::BuildHasher as _;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Mutex;

use guardiana_tunel::alta::{Pide, Respuesta, VERSION};
use guardiana_tunel::cuantico::{encapsula, psk};
use guardiana_tunel::llaves::publica_de_base64;

/// Sign-ups the whole server takes per minute.
pub const ALTAS_POR_MINUTO: usize = 600;
/// Sign-ups one origin may make per minute (an IPv4 address, or an IPv6 /64: what one home or one phone gets).
pub const ALTAS_POR_ORIGEN: usize = 10;
/// A peer without a handshake for this long is removed (seconds).
pub const SIN_USO_S: u64 = 24 * 60 * 60;
/// A peer that never shook hands is removed after this long (seconds).
pub const SIN_ESTRENAR_S: u64 = 10 * 60;

/// The machine's WireGuard, as this service uses it.
pub trait Wg: Send + Sync {
    /// Adds a peer, or updates it, with its preshared key (base64) and its only address.
    fn pon(&self, llave: &str, psk: &str, direccion: Ipv4Addr) -> Result<(), String>;
    /// Removes a peer.
    fn quita(&self, llave: &str) -> Result<(), String>;
    /// Each peer's last handshake (seconds since 1970; 0 = never).
    fn saludos(&self) -> Result<Vec<(String, u64)>, String>;
    /// Each peer WireGuard has now, with its allowed addresses as WireGuard writes them (`10.66.0.2/32`).
    fn direcciones(&self) -> Result<Vec<(String, Vec<String>)>, String>;
}

/// Why a sign-up was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorAlta {
    /// The request is not a valid sign-up.
    Formato(&'static str),
    /// Too many sign-ups this minute, or no address left.
    Ocupado,
    /// WireGuard did not take the peer.
    Wg(String),
}

struct Pares {
    /// Public key (base64) → its address and when it signed up, or was taken over (seconds since 1970).
    por_llave: HashMap<String, (Ipv4Addr, u64)>,
    usadas: HashSet<Ipv4Addr>,
    siguiente: u32,
    /// The sign-ups of the last minute: when, and the hash of their origin.
    altas: VecDeque<(u64, u64)>,
    /// How many of those came from each origin (by its hash).
    por_origen: HashMap<u64, usize>,
}

/// The service's state.
pub struct Estado {
    servidor: [u8; 32],
    punto: String,
    dns: Ipv4Addr,
    wg: Box<dyn Wg>,
    /// The key of the origins' hash: random, drawn at every start, never written anywhere.
    clave_origen: RandomState,
    pares: Mutex<Pares>,
}

/// The tunnel's addresses: 10.66.0.2 to 10.66.255.254 (10.66.0.1 is the server and its resolver).
const PRIMERA: u32 = u32::from_be_bytes([10, 66, 0, 2]);
const ULTIMA: u32 = u32::from_be_bytes([10, 66, 255, 254]);

impl Estado {
    /// The state for a server with this WireGuard public key, reachable at `punto` (`host:port`), whose resolver
    /// inside the tunnel is `dns`.
    #[must_use]
    pub fn nuevo(servidor: [u8; 32], punto: &str, dns: Ipv4Addr, wg: Box<dyn Wg>) -> Self {
        Self {
            servidor,
            punto: punto.to_string(),
            dns,
            wg,
            clave_origen: RandomState::new(),
            pares: Mutex::new(Pares {
                por_llave: HashMap::new(),
                usadas: HashSet::new(),
                siguiente: PRIMERA,
                altas: VecDeque::new(),
                por_origen: HashMap::new(),
            }),
        }
    }

    /// Takes over the peers WireGuard already has, once, when the service starts: each keeps its address (and its
    /// tunnel), none of them is given again, and the next new address comes after the highest one taken. A peer whose
    /// allowed addresses are not exactly one of this tunnel's was not added by this service, and is removed. Each one
    /// taken over counts as signed up now: [`Estado::limpia`] removes it after [`SIN_ESTRENAR_S`] if it never shakes
    /// hands, or [`SIN_USO_S`] after its last handshake. Returns how many it took over.
    ///
    /// # Errors
    /// When WireGuard cannot be read, or a peer cannot be removed.
    pub fn adopta(&self, ahora_s: u64) -> Result<usize, String> {
        let actuales = self.wg.direcciones()?;
        let mut fuera = Vec::new();
        let tomados = {
            let mut pares = self.pares.lock().map_err(|_| "estado".to_string())?;
            let mut mayor: Option<u32> = None;
            let mut tomados = 0;
            for (llave, ips) in actuales {
                match del_tunel(&ips) {
                    Some(d)
                        if !pares.usadas.contains(&d) && !pares.por_llave.contains_key(&llave) =>
                    {
                        pares.usadas.insert(d);
                        pares.por_llave.insert(llave, (d, ahora_s));
                        let n = u32::from(d);
                        mayor = Some(mayor.map_or(n, |m| m.max(n)));
                        tomados += 1;
                    }
                    _ => fuera.push(llave),
                }
            }
            if let Some(m) = mayor {
                pares.siguiente = if m >= ULTIMA { PRIMERA } else { m + 1 };
            }
            tomados
        };
        for llave in &fuera {
            self.wg.quita(llave)?;
        }
        Ok(tomados)
    }

    /// A sign-up from `origen` (the address the web server in front saw, if it said one): checks it, adds the peer with
    /// its post-quantum preshared key and answers.
    pub fn alta(
        &self,
        p: &Pide,
        origen: Option<IpAddr>,
        ahora_s: u64,
    ) -> Result<Respuesta, ErrorAlta> {
        if p.v != VERSION {
            return Err(ErrorAlta::Formato("versión"));
        }
        let cliente = publica_de_base64(&p.llave).map_err(|_| ErrorAlta::Formato("llave"))?;
        let llave = b64(&cliente);
        let kem = de_b64(&p.kem).ok_or(ErrorAlta::Formato("kem"))?;
        let (ct, secreto) = encapsula(&kem).map_err(|_| ErrorAlta::Formato("kem"))?;
        let k = psk(&secreto, &cliente, &self.servidor).map_err(|_| ErrorAlta::Formato("kem"))?;
        let origen = self.origen(origen);
        let (direccion, nueva) = {
            let mut pares = self.pares.lock().map_err(|_| ErrorAlta::Ocupado)?;
            while pares
                .altas
                .front()
                .is_some_and(|(t, _)| ahora_s.saturating_sub(*t) >= 60)
            {
                if let Some((_, o)) = pares.altas.pop_front() {
                    let quedan = pares.por_origen.get(&o).map_or(0, |n| n.saturating_sub(1));
                    if quedan == 0 {
                        pares.por_origen.remove(&o);
                    } else {
                        pares.por_origen.insert(o, quedan);
                    }
                }
            }
            let de_este = pares.por_origen.get(&origen).copied().unwrap_or(0);
            if pares.altas.len() >= ALTAS_POR_MINUTO || de_este >= ALTAS_POR_ORIGEN {
                return Err(ErrorAlta::Ocupado);
            }
            pares.altas.push_back((ahora_s, origen));
            pares.por_origen.insert(origen, de_este + 1);
            let (direccion, nueva) = match pares.por_llave.get(&llave) {
                Some((d, _)) => (*d, false),
                None => (asigna(&mut pares).ok_or(ErrorAlta::Ocupado)?, true),
            };
            pares.por_llave.insert(llave.clone(), (direccion, ahora_s));
            pares.usadas.insert(direccion);
            (direccion, nueva)
        };
        if let Err(e) = self.wg.pon(&llave, &b64(&k), direccion) {
            // a new one WireGuard did not take is not kept here either; one that was already there is still in
            // WireGuard with its address, and keeps it (were it freed, it could go to another computer and be taken
            // from this one)
            if nueva {
                if let Ok(mut pares) = self.pares.lock() {
                    pares.por_llave.remove(&llave);
                    pares.usadas.remove(&direccion);
                }
            }
            return Err(ErrorAlta::Wg(e));
        }
        Ok(Respuesta {
            v: VERSION,
            direccion: format!("{direccion}/32"),
            servidor: b64(&self.servidor),
            punto: self.punto.clone(),
            kem: b64(&ct),
            dns: self.dns.to_string(),
            caduca_ms: (ahora_s + SIN_USO_S) * 1000,
        })
    }

    /// Removes the peers without a handshake for [`SIN_USO_S`], and those that never shook hands after
    /// [`SIN_ESTRENAR_S`]. Returns how many it removed.
    pub fn limpia(&self, ahora_s: u64) -> Result<usize, String> {
        let saludos: HashMap<String, u64> = self.wg.saludos()?.into_iter().collect();
        let mut pares = self.pares.lock().map_err(|_| "estado".to_string())?;
        let viejas: Vec<String> = pares
            .por_llave
            .iter()
            .filter(
                |(llave, (_, alta))| match saludos.get(*llave).copied().unwrap_or(0) {
                    0 => ahora_s.saturating_sub(*alta) >= SIN_ESTRENAR_S,
                    ultimo => ahora_s.saturating_sub(ultimo) >= SIN_USO_S,
                },
            )
            .map(|(llave, _)| llave.clone())
            .collect();
        for llave in &viejas {
            self.wg.quita(llave)?;
            if let Some((d, _)) = pares.por_llave.remove(llave) {
                pares.usadas.remove(&d);
            }
        }
        Ok(viejas.len())
    }

    /// How many peers it has now.
    #[must_use]
    pub fn pares(&self) -> usize {
        self.pares.lock().map(|p| p.por_llave.len()).unwrap_or(0)
    }

    /// The hash of an origin, with this start's key: an IPv4 address, or the /64 of an IPv6 one (one home or one phone
    /// gets a whole /64). Without an origin, all such sign-ups share one.
    fn origen(&self, ip: Option<IpAddr>) -> u64 {
        let bytes: [u8; 16] = match ip {
            None => [0; 16],
            Some(IpAddr::V4(v4)) => v4.to_ipv6_mapped().octets(),
            Some(IpAddr::V6(v6)) => match v6.to_ipv4_mapped() {
                Some(v4) => v4.to_ipv6_mapped().octets(),
                None => {
                    let mut o = v6.octets();
                    o[8..].fill(0);
                    o
                }
            },
        };
        self.clave_origen.hash_one(bytes)
    }
}

/// The one address of this tunnel in a peer's allowed addresses (`10.66.x.y/32`), if that is all it has.
fn del_tunel(ips: &[String]) -> Option<Ipv4Addr> {
    let [una] = ips else {
        return None;
    };
    let d: Ipv4Addr = una.strip_suffix("/32")?.parse().ok()?;
    (PRIMERA..=ULTIMA).contains(&u32::from(d)).then_some(d)
}

fn asigna(p: &mut Pares) -> Option<Ipv4Addr> {
    let total = ULTIMA - PRIMERA + 1;
    for i in 0..total {
        let n = PRIMERA + (p.siguiente - PRIMERA + i) % total;
        let d = Ipv4Addr::from(n);
        if !p.usadas.contains(&d) {
            p.siguiente = if n == ULTIMA { PRIMERA } else { n + 1 };
            return Some(d);
        }
    }
    None
}

fn b64(b: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(b)
}

fn de_b64(s: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(s.trim())
        .ok()
}

/// The machine's WireGuard through the `wg` command (wireguard-tools), on interface `interfaz`. The preshared key goes
/// through the command's standard input, never through a file or its arguments.
pub struct WgReal {
    /// The WireGuard interface (`wg0`).
    pub interfaz: String,
}

impl WgReal {
    fn wg(&self, args: &[&str], entrada: Option<&str>) -> Result<String, String> {
        use std::io::Write as _;
        let mut orden = std::process::Command::new("wg");
        orden
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let mut hijo = orden.spawn().map_err(|e| e.to_string())?;
        if let (Some(texto), Some(mut stdin)) = (entrada, hijo.stdin.take()) {
            stdin
                .write_all(texto.as_bytes())
                .map_err(|e| e.to_string())?;
        }
        let salida = hijo.wait_with_output().map_err(|e| e.to_string())?;
        if salida.status.success() {
            Ok(String::from_utf8_lossy(&salida.stdout).into_owned())
        } else {
            Err(String::from_utf8_lossy(&salida.stderr).trim().to_string())
        }
    }

    /// The interface's public key, as `wg show <interfaz> public-key` gives it.
    pub fn publica(&self) -> Result<[u8; 32], String> {
        let s = self.wg(&["show", &self.interfaz, "public-key"], None)?;
        publica_de_base64(s.trim()).map_err(|e| e.to_string())
    }
}

impl Wg for WgReal {
    fn pon(&self, llave: &str, psk: &str, direccion: Ipv4Addr) -> Result<(), String> {
        let ips = format!("{direccion}/32");
        self.wg(
            &[
                "set",
                &self.interfaz,
                "peer",
                llave,
                "preshared-key",
                "/dev/stdin",
                "allowed-ips",
                &ips,
            ],
            Some(psk),
        )
        .map(|_| ())
    }

    fn quita(&self, llave: &str) -> Result<(), String> {
        self.wg(&["set", &self.interfaz, "peer", llave, "remove"], None)
            .map(|_| ())
    }

    fn saludos(&self) -> Result<Vec<(String, u64)>, String> {
        let s = self.wg(&["show", &self.interfaz, "latest-handshakes"], None)?;
        Ok(s.lines()
            .filter_map(|l| {
                let mut c = l.split_whitespace();
                Some((c.next()?.to_string(), c.next()?.parse().ok()?))
            })
            .collect())
    }

    fn direcciones(&self) -> Result<Vec<(String, Vec<String>)>, String> {
        // one line per peer: its public key, then its allowed addresses separated by spaces, or «(none)»
        let s = self.wg(&["show", &self.interfaz, "allowed-ips"], None)?;
        Ok(s.lines()
            .filter_map(|l| {
                let mut c = l.split_whitespace();
                let llave = c.next()?.to_string();
                let ips = c.filter(|x| *x != "(none)").map(str::to_string).collect();
                Some((llave, ips))
            })
            .collect())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use guardiana_tunel::alta::{completa, pide};
    use guardiana_tunel::llaves::Llaves;
    use std::sync::Arc;

    #[derive(Default)]
    struct WgDePrueba {
        pares: Mutex<HashMap<String, (String, Ipv4Addr)>>,
        saludos: Mutex<HashMap<String, u64>>,
        /// Peers this service did not add, with their allowed addresses as WireGuard writes them.
        ajenos: Mutex<Vec<(String, Vec<String>)>>,
        /// When set, WireGuard does not take any peer.
        falla: std::sync::atomic::AtomicBool,
    }

    impl Wg for Arc<WgDePrueba> {
        fn pon(&self, llave: &str, psk: &str, direccion: Ipv4Addr) -> Result<(), String> {
            if self.falla.load(std::sync::atomic::Ordering::SeqCst) {
                return Err("WireGuard no lo toma".into());
            }
            self.pares
                .lock()
                .unwrap()
                .insert(llave.into(), (psk.into(), direccion));
            Ok(())
        }
        fn quita(&self, llave: &str) -> Result<(), String> {
            self.pares.lock().unwrap().remove(llave);
            self.ajenos.lock().unwrap().retain(|(k, _)| k != llave);
            Ok(())
        }
        fn saludos(&self) -> Result<Vec<(String, u64)>, String> {
            Ok(self
                .saludos
                .lock()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect())
        }
        fn direcciones(&self) -> Result<Vec<(String, Vec<String>)>, String> {
            let mut todos: Vec<(String, Vec<String>)> = self
                .pares
                .lock()
                .unwrap()
                .iter()
                .map(|(k, (_, d))| (k.clone(), vec![format!("{d}/32")]))
                .collect();
            todos.extend(self.ajenos.lock().unwrap().iter().cloned());
            Ok(todos)
        }
    }

    fn con(wg: &Arc<WgDePrueba>, srv: &Llaves) -> Estado {
        Estado::nuevo(
            srv.publica(),
            "motor.guardianagroup.com:51820",
            Ipv4Addr::new(10, 66, 0, 1),
            Box::new(Arc::clone(wg)),
        )
    }

    fn servidor() -> (Estado, Arc<WgDePrueba>, Llaves) {
        let srv = Llaves::nuevas().unwrap();
        let wg = Arc::new(WgDePrueba::default());
        (con(&wg, &srv), wg, srv)
    }

    fn nueva() -> Pide {
        pide(&Llaves::nuevas().unwrap()).1
    }

    fn ip(s: &str) -> Option<IpAddr> {
        Some(s.parse().unwrap())
    }

    #[test]
    fn de_punta_a_punta_la_misma_llave_postcuantica_en_los_dos_lados() {
        let (e, wg, _) = servidor();
        let equipo = Llaves::nuevas().unwrap();
        let (oferta, p) = pide(&equipo);
        let r = e.alta(&p, None, 1_000).unwrap();
        let a = completa(&equipo, &oferta, &r).unwrap();
        let (psk_servidor, direccion) = wg
            .pares
            .lock()
            .unwrap()
            .get(&equipo.publica_base64())
            .cloned()
            .unwrap();
        assert_eq!(psk_servidor, b64(a.psk.as_slice()));
        assert_eq!(direccion, a.direccion);
        assert_eq!(a.direccion, Ipv4Addr::new(10, 66, 0, 2));
    }

    #[test]
    fn cada_llave_su_direccion_y_la_misma_si_vuelve() {
        let (e, _, _) = servidor();
        let (uno, dos) = (Llaves::nuevas().unwrap(), Llaves::nuevas().unwrap());
        let r1 = e.alta(&pide(&uno).1, None, 1).unwrap();
        let r2 = e.alta(&pide(&dos).1, None, 2).unwrap();
        let r1b = e.alta(&pide(&uno).1, None, 3).unwrap();
        assert_eq!(r1.direccion, "10.66.0.2/32");
        assert_eq!(r2.direccion, "10.66.0.3/32");
        assert_eq!(r1b.direccion, r1.direccion);
        assert_ne!(r1b.kem, r1.kem);
        assert_eq!(e.pares(), 2);
    }

    #[test]
    fn lo_que_no_es_un_alta_no_entra() {
        let (e, _, _) = servidor();
        let (_, mut p) = pide(&Llaves::nuevas().unwrap());
        p.v = 9;
        assert_eq!(e.alta(&p, None, 1), Err(ErrorAlta::Formato("versión")));
        let (_, mut p) = pide(&Llaves::nuevas().unwrap());
        p.llave = "corta".into();
        assert_eq!(e.alta(&p, None, 1), Err(ErrorAlta::Formato("llave")));
        let (_, mut p) = pide(&Llaves::nuevas().unwrap());
        p.kem = b64(&[0u8; 32]);
        assert_eq!(e.alta(&p, None, 1), Err(ErrorAlta::Formato("kem")));
        assert_eq!(e.pares(), 0);
    }

    #[test]
    fn un_tope_de_altas_por_minuto() {
        let (e, _, _) = servidor();
        let origen = |i: u32| Some(IpAddr::V4(Ipv4Addr::from(0xC633_6400 + i)));
        for i in 0..u32::try_from(ALTAS_POR_MINUTO).unwrap() {
            assert!(e.alta(&nueva(), origen(i), 100).is_ok());
        }
        assert_eq!(
            e.alta(&nueva(), origen(100_000), 110),
            Err(ErrorAlta::Ocupado)
        );
        assert!(e.alta(&nueva(), origen(100_001), 161).is_ok());
    }

    #[test]
    fn un_tope_por_origen_que_se_olvida_al_pasar_el_minuto() {
        let (e, _, _) = servidor();
        let casa = ip("203.0.113.7");
        for _ in 0..ALTAS_POR_ORIGEN {
            assert!(e.alta(&nueva(), casa, 100).is_ok());
        }
        assert_eq!(e.alta(&nueva(), casa, 110), Err(ErrorAlta::Ocupado));
        // la misma dirección escrita como IPv6 es el mismo origen
        assert_eq!(
            e.alta(&nueva(), ip("::ffff:203.0.113.7"), 110),
            Err(ErrorAlta::Ocupado)
        );
        // otro origen entra; el primero vuelve a entrar pasado el minuto
        assert!(e.alta(&nueva(), ip("203.0.113.8"), 110).is_ok());
        assert!(e.alta(&nueva(), casa, 161).is_ok());
        // y de los de hace más de un minuto no queda nada: solo los dos orígenes de este último minuto
        let pares = e.pares.lock().unwrap();
        assert_eq!(pares.por_origen.len(), 2);
        assert_eq!(pares.altas.len(), 2);
        assert_eq!(pares.por_origen.get(&e.origen(casa)), Some(&1));
    }

    #[test]
    fn en_ipv6_una_casa_es_su_64() {
        let (e, _, _) = servidor();
        for i in 1..=ALTAS_POR_ORIGEN {
            assert!(e
                .alta(&nueva(), ip(&format!("2001:db8:0:1::{i:x}")), 100)
                .is_ok());
        }
        assert_eq!(
            e.alta(&nueva(), ip("2001:db8:0:1:ffff::1"), 100),
            Err(ErrorAlta::Ocupado)
        );
        assert!(e.alta(&nueva(), ip("2001:db8:0:2::1"), 100).is_ok());
        // sin origen (la comprobación del propio servidor) es otro
        assert!(e.alta(&nueva(), None, 100).is_ok());
    }

    #[test]
    fn al_volver_a_arrancar_los_conectados_siguen_con_su_direccion() {
        let (e, wg, srv) = servidor();
        let (uno, dos, tres) = (
            Llaves::nuevas().unwrap(),
            Llaves::nuevas().unwrap(),
            Llaves::nuevas().unwrap(),
        );
        let r1 = e.alta(&pide(&uno).1, None, 1).unwrap();
        let r2 = e.alta(&pide(&dos).1, None, 2).unwrap();
        // lo que WireGuard tiene y este servicio no dio: se quita al arrancar
        wg.ajenos.lock().unwrap().extend([
            ("ajeno-red".to_string(), vec!["10.1.2.3/32".to_string()]),
            (
                "ajeno-servidor".to_string(),
                vec!["10.66.0.1/32".to_string()],
            ),
            (
                "ajeno-dos".to_string(),
                vec!["10.66.9.9/32".to_string(), "10.66.9.10/32".to_string()],
            ),
            ("ajeno-nada".to_string(), vec![]),
            ("ajeno-ancho".to_string(), vec!["10.66.7.0/24".to_string()]),
        ]);
        drop(e);
        // el servicio vuelve a arrancar con el mismo WireGuard
        let e = con(&wg, &srv);
        assert_eq!(e.adopta(1_000).unwrap(), 2);
        assert!(wg.ajenos.lock().unwrap().is_empty());
        assert_eq!(e.pares(), 2);
        assert_eq!(wg.pares.lock().unwrap().len(), 2);
        // uno nuevo recibe una dirección nueva, después de la más alta tomada
        let r3 = e.alta(&pide(&tres).1, None, 1_001).unwrap();
        assert_eq!(r3.direccion, "10.66.0.4/32");
        assert!(r3.direccion != r1.direccion && r3.direccion != r2.direccion);
        // uno que ya estaba y vuelve a pedir el alta conserva la suya
        assert_eq!(
            e.alta(&pide(&uno).1, None, 1_002).unwrap().direccion,
            r1.direccion
        );
        // los tomados se limpian como los demás: «dos» no vuelve a saludar y se va pasados SIN_ESTRENAR_S
        let ahora = 1_000 + SIN_ESTRENAR_S;
        for k in [&uno, &tres] {
            wg.saludos.lock().unwrap().insert(k.publica_base64(), ahora);
        }
        assert_eq!(e.limpia(ahora + 1).unwrap(), 1);
        assert!(!wg.pares.lock().unwrap().contains_key(&dos.publica_base64()));
        assert_eq!(e.pares(), 2);
    }

    #[test]
    fn si_wireguard_no_toma_el_par_la_direccion_de_uno_que_ya_estaba_no_se_suelta() {
        use std::sync::atomic::Ordering;
        let (e, wg, _) = servidor();
        let uno = Llaves::nuevas().unwrap();
        let r1 = e.alta(&pide(&uno).1, None, 1).unwrap();
        wg.falla.store(true, Ordering::SeqCst);
        // uno que ya estaba: falla, pero sigue en WireGuard con su dirección y aquí también
        assert!(matches!(
            e.alta(&pide(&uno).1, None, 2),
            Err(ErrorAlta::Wg(_))
        ));
        // uno nuevo: falla y no se queda
        assert!(matches!(e.alta(&nueva(), None, 3), Err(ErrorAlta::Wg(_))));
        assert_eq!(e.pares(), 1);
        wg.falla.store(false, Ordering::SeqCst);
        let r2 = e.alta(&nueva(), None, 4).unwrap();
        assert_ne!(r2.direccion, r1.direccion);
        assert_eq!(
            e.alta(&pide(&uno).1, None, 5).unwrap().direccion,
            r1.direccion
        );
    }

    #[test]
    fn tras_la_ultima_direccion_se_vuelve_a_la_primera() {
        let srv = Llaves::nuevas().unwrap();
        let wg = Arc::new(WgDePrueba::default());
        wg.pares.lock().unwrap().insert(
            "ultimo".into(),
            ("psk".into(), Ipv4Addr::new(10, 66, 255, 254)),
        );
        let e = con(&wg, &srv);
        assert_eq!(e.adopta(5).unwrap(), 1);
        assert_eq!(e.alta(&nueva(), None, 6).unwrap().direccion, "10.66.0.2/32");
    }

    #[test]
    fn se_quitan_los_que_no_se_usan() {
        let (e, wg, _) = servidor();
        let (usada, olvidada, nueva) = (
            Llaves::nuevas().unwrap(),
            Llaves::nuevas().unwrap(),
            Llaves::nuevas().unwrap(),
        );
        e.alta(&pide(&usada).1, None, 0).unwrap();
        e.alta(&pide(&olvidada).1, None, 0).unwrap();
        e.alta(&pide(&nueva).1, None, 2 * SIN_USO_S).unwrap();
        wg.saludos
            .lock()
            .unwrap()
            .insert(usada.publica_base64(), 2 * SIN_USO_S - 5);
        assert_eq!(e.limpia(2 * SIN_USO_S + 1).unwrap(), 1);
        assert_eq!(e.pares(), 2);
        assert!(!wg
            .pares
            .lock()
            .unwrap()
            .contains_key(&olvidada.publica_base64()));
        // las direcciones siguen adelante; la liberada volverá a darse al dar la vuelta
        let r = e
            .alta(&pide(&Llaves::nuevas().unwrap()).1, None, 2 * SIN_USO_S + 2)
            .unwrap();
        assert_eq!(r.direccion, "10.66.0.5/32");
    }
}
