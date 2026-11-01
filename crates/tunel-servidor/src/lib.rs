//! The server half of GUARDIANA's VPN (docs/VPN.md, «Los servidores»; in preparation, not installed anywhere yet).
//!
//! One thing only: the anonymous sign-up. A computer sends its WireGuard public key and a fresh ML-KEM-768 key
//! ([`guardiana_tunel::alta::Pide`]); this side encapsulates a secret to it, derives the same post-quantum preshared key
//! the computer will derive, gives the key an address inside the tunnel and adds it as a WireGuard peer
//! ([`guardiana_tunel::alta::Respuesta`]). No account, no name, nothing written to disk: the peers live in the kernel's
//! memory and in this process's, and a peer without traffic for a day is removed ([`Estado::limpia`]).
//!
//! Against abuse, without keeping anything about anyone: a cap of sign-ups per minute for the whole server.

use std::collections::{HashMap, HashSet, VecDeque};
use std::net::Ipv4Addr;
use std::sync::Mutex;

use guardiana_tunel::alta::{Pide, Respuesta, VERSION};
use guardiana_tunel::cuantico::{encapsula, psk};
use guardiana_tunel::llaves::publica_de_base64;

/// Sign-ups the whole server takes per minute.
pub const ALTAS_POR_MINUTO: usize = 60;
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
    /// Public key (base64) → its address and when it signed up (seconds since 1970).
    por_llave: HashMap<String, (Ipv4Addr, u64)>,
    usadas: HashSet<Ipv4Addr>,
    siguiente: u32,
    altas: VecDeque<u64>,
}

/// The service's state.
pub struct Estado {
    servidor: [u8; 32],
    punto: String,
    dns: Ipv4Addr,
    wg: Box<dyn Wg>,
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
            pares: Mutex::new(Pares {
                por_llave: HashMap::new(),
                usadas: HashSet::new(),
                siguiente: PRIMERA,
                altas: VecDeque::new(),
            }),
        }
    }

    /// A sign-up: checks it, adds the peer with its post-quantum preshared key and answers.
    pub fn alta(&self, p: &Pide, ahora_s: u64) -> Result<Respuesta, ErrorAlta> {
        if p.v != VERSION {
            return Err(ErrorAlta::Formato("versión"));
        }
        let cliente = publica_de_base64(&p.llave).map_err(|_| ErrorAlta::Formato("llave"))?;
        let llave = b64(&cliente);
        let kem = de_b64(&p.kem).ok_or(ErrorAlta::Formato("kem"))?;
        let (ct, secreto) = encapsula(&kem).map_err(|_| ErrorAlta::Formato("kem"))?;
        let k = psk(&secreto, &cliente, &self.servidor).map_err(|_| ErrorAlta::Formato("kem"))?;
        let direccion = {
            let mut pares = self.pares.lock().map_err(|_| ErrorAlta::Ocupado)?;
            while pares
                .altas
                .front()
                .is_some_and(|t| ahora_s.saturating_sub(*t) >= 60)
            {
                pares.altas.pop_front();
            }
            if pares.altas.len() >= ALTAS_POR_MINUTO {
                return Err(ErrorAlta::Ocupado);
            }
            pares.altas.push_back(ahora_s);
            let direccion = match pares.por_llave.get(&llave) {
                Some((d, _)) => *d,
                None => asigna(&mut pares).ok_or(ErrorAlta::Ocupado)?,
            };
            pares.por_llave.insert(llave.clone(), (direccion, ahora_s));
            pares.usadas.insert(direccion);
            direccion
        };
        if let Err(e) = self.wg.pon(&llave, &b64(&k), direccion) {
            // sin par en WireGuard, tampoco aquí
            if let Ok(mut pares) = self.pares.lock() {
                if pares.por_llave.remove(&llave).is_some() {
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
    }

    impl Wg for Arc<WgDePrueba> {
        fn pon(&self, llave: &str, psk: &str, direccion: Ipv4Addr) -> Result<(), String> {
            self.pares
                .lock()
                .unwrap()
                .insert(llave.into(), (psk.into(), direccion));
            Ok(())
        }
        fn quita(&self, llave: &str) -> Result<(), String> {
            self.pares.lock().unwrap().remove(llave);
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
    }

    fn servidor() -> (Estado, Arc<WgDePrueba>, Llaves) {
        let srv = Llaves::nuevas().unwrap();
        let wg = Arc::new(WgDePrueba::default());
        let e = Estado::nuevo(
            srv.publica(),
            "motor.guardianagroup.com:51820",
            Ipv4Addr::new(10, 66, 0, 1),
            Box::new(Arc::clone(&wg)),
        );
        (e, wg, srv)
    }

    #[test]
    fn de_punta_a_punta_la_misma_llave_postcuantica_en_los_dos_lados() {
        let (e, wg, _) = servidor();
        let equipo = Llaves::nuevas().unwrap();
        let (oferta, p) = pide(&equipo);
        let r = e.alta(&p, 1_000).unwrap();
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
        let r1 = e.alta(&pide(&uno).1, 1).unwrap();
        let r2 = e.alta(&pide(&dos).1, 2).unwrap();
        let r1b = e.alta(&pide(&uno).1, 3).unwrap();
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
        assert_eq!(e.alta(&p, 1), Err(ErrorAlta::Formato("versión")));
        let (_, mut p) = pide(&Llaves::nuevas().unwrap());
        p.llave = "corta".into();
        assert_eq!(e.alta(&p, 1), Err(ErrorAlta::Formato("llave")));
        let (_, mut p) = pide(&Llaves::nuevas().unwrap());
        p.kem = b64(&[0u8; 32]);
        assert_eq!(e.alta(&p, 1), Err(ErrorAlta::Formato("kem")));
        assert_eq!(e.pares(), 0);
    }

    #[test]
    fn un_tope_de_altas_por_minuto() {
        let (e, _, _) = servidor();
        for _ in 0..ALTAS_POR_MINUTO {
            assert!(e.alta(&pide(&Llaves::nuevas().unwrap()).1, 100).is_ok());
        }
        assert_eq!(
            e.alta(&pide(&Llaves::nuevas().unwrap()).1, 110),
            Err(ErrorAlta::Ocupado)
        );
        assert!(e.alta(&pide(&Llaves::nuevas().unwrap()).1, 161).is_ok());
    }

    #[test]
    fn se_quitan_los_que_no_se_usan() {
        let (e, wg, _) = servidor();
        let (usada, olvidada, nueva) = (
            Llaves::nuevas().unwrap(),
            Llaves::nuevas().unwrap(),
            Llaves::nuevas().unwrap(),
        );
        e.alta(&pide(&usada).1, 0).unwrap();
        e.alta(&pide(&olvidada).1, 0).unwrap();
        e.alta(&pide(&nueva).1, 2 * SIN_USO_S).unwrap();
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
            .alta(&pide(&Llaves::nuevas().unwrap()).1, 2 * SIN_USO_S + 2)
            .unwrap();
        assert_eq!(r.direccion, "10.66.0.5/32");
    }
}
