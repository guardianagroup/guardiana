//! A connection, from the sign-up to the server's first handshake, the same on every system (in preparation;
//! docs/VPN.md, «Pendiente» 1). The system's half (on Windows, `guardiana_apps::tunel_win`) only opens and closes the
//! tunnel; the rest is decided, and tested, here:
//!
//! 1. fresh keys for each connection, so nothing joins two sessions;
//! 2. the sign-up over HTTPS with a server under GUARDIANA's name ([`pide_alta`]), and its answer checked
//!    ([`crate::alta::completa`]);
//! 3. the server's address, resolved before the tunnel opens (once the cut-off switch is on, nothing resolves
//!    outside the tunnel);
//! 4. the tunnel, opened by the system's half;
//! 5. the server's first handshake, waited for a few seconds ([`ESPERA`]). If it does not come, the tunnel is closed
//!    again, and the cut-off switch with it: a server that does not answer never leaves the computer without internet.

use std::net::{Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::time::{Duration, Instant};

use crate::alta::{self, Alta, Pide, Respuesta};
use crate::llaves::Llaves;

/// How long the server's first handshake is waited for.
pub const ESPERA: Duration = Duration::from_secs(8);
/// How often the tunnel is asked about it meanwhile.
const CADA: Duration = Duration::from_millis(250);
/// How long the sign-up may take.
const PLAZO_ALTA: Duration = Duration::from_secs(15);
/// The largest answer read from the server.
const MAXIMO_RESPUESTA: u64 = 16 * 1024;

/// A tunnel the system's half opened.
pub trait Abierto {
    /// When the server's last handshake was (milliseconds since 1970), if there has been one.
    fn saludo_ms(&self) -> Option<u64>;
    /// Closes it: the cut-off switch off and the adapter gone.
    fn cierra(self);
}

/// Why a connection could not be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorConexion {
    /// The server named is not under GUARDIANA's name.
    Servidor,
    /// The sign-up could not be made: the network, or the server's answer code.
    Alta(String),
    /// The server's answer is not valid.
    Respuesta(String),
    /// The server's name did not resolve.
    Nombre(String),
    /// The system could not open the tunnel.
    Tunel(String),
    /// The server did not answer the handshake in time: the tunnel was closed again.
    NoContesta,
}

impl std::fmt::Display for ErrorConexion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Servidor => f.write_str("ese servidor no es de GUARDIANA"),
            Self::Alta(e) => write!(f, "no se pudo pedir el alta al servidor: {e}"),
            Self::Respuesta(e) => write!(f, "la respuesta del servidor no es válida: {e}"),
            Self::Nombre(e) => write!(f, "no se encontró la dirección del servidor: {e}"),
            Self::Tunel(e) => write!(f, "no se pudo abrir el túnel: {e}"),
            Self::NoContesta => f.write_str(
                "el servidor no contesta: el túnel se ha cerrado y la conexión del equipo sigue como estaba",
            ),
        }
    }
}

impl std::error::Error for ErrorConexion {}

/// The sign-up over HTTPS: `POST https://{host}/vpn/alta` with `pide` as JSON; the server's answer, not yet checked.
/// Redirections are not followed: a GUARDIANA server answers itself.
///
/// # Errors
/// [`ErrorConexion::Servidor`] when `host` is not a GUARDIANA name; [`ErrorConexion::Alta`] or
/// [`ErrorConexion::Respuesta`] when the exchange fails.
pub fn pide_alta(host: &str, pide: &Pide) -> Result<Respuesta, ErrorConexion> {
    if !alta::es_servidor(host) {
        return Err(ErrorConexion::Servidor);
    }
    pide_alta_en(&format!("https://{host}/vpn/alta"), pide)
}

/// The same exchange, with a whole address (the tests use a local one).
fn pide_alta_en(url: &str, pide: &Pide) -> Result<Respuesta, ErrorConexion> {
    let cuerpo = serde_json::to_vec(pide).map_err(|e| ErrorConexion::Alta(e.to_string()))?;
    let respuesta = ureq::post(url)
        .config()
        .timeout_global(Some(PLAZO_ALTA))
        .http_status_as_error(false)
        .max_redirects(0)
        .build()
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .send(cuerpo.as_slice())
        .map_err(|e| ErrorConexion::Alta(e.to_string()))?;
    let codigo = respuesta.status().as_u16();
    let texto = respuesta
        .into_body()
        .with_config()
        .limit(MAXIMO_RESPUESTA)
        .read_to_string()
        .map_err(|e| ErrorConexion::Alta(e.to_string()))?;
    if codigo != 200 {
        return Err(ErrorConexion::Alta(format!(
            "el servidor respondió {codigo}"
        )));
    }
    serde_json::from_str(&texto)
        .map_err(|_| ErrorConexion::Respuesta("no es una respuesta de alta".to_owned()))
}

/// The server's address: its name resolved, an IPv4 address first (the tunnel's own network is IPv4).
///
/// # Errors
/// [`ErrorConexion::Nombre`] when the name does not resolve.
pub fn resuelve(host: &str, puerto: u16) -> Result<SocketAddr, ErrorConexion> {
    let todas: Vec<SocketAddr> = (host, puerto)
        .to_socket_addrs()
        .map_err(|e| ErrorConexion::Nombre(e.to_string()))?
        .collect();
    todas
        .iter()
        .find(|a| a.is_ipv4())
        .or_else(|| todas.first())
        .copied()
        .ok_or_else(|| ErrorConexion::Nombre("sin direcciones".to_owned()))
}

/// A connection made: the tunnel open and the server answering.
#[derive(Debug)]
pub struct Conectado<A> {
    /// The tunnel, as the system's half opened it.
    pub tunel: A,
    /// The server's name.
    pub servidor: String,
    /// This computer's address inside the tunnel.
    pub direccion: Ipv4Addr,
    /// When the tunnel was opened (milliseconds since 1970).
    pub desde_ms: u64,
    /// Until when the server keeps this computer's key without traffic.
    pub caduca_ms: u64,
}

/// A whole connection with the server called `host`. `pide` makes the sign-up (in the program, [`pide_alta`]),
/// `resolver` resolves the server's name (in the program, [`resuelve`]), `abre` is the system's half, `espera` how long
/// the first handshake is waited for (in the program, [`ESPERA`]) and `ahora_ms` the clock.
///
/// # Errors
/// [`ErrorConexion`] with the step that failed. The tunnel is never left open after an error: when the server does not
/// answer the handshake, it is closed before this returns.
pub fn conecta<A: Abierto>(
    host: &str,
    pide: impl FnOnce(&str, &Pide) -> Result<Respuesta, ErrorConexion>,
    resolver: impl FnOnce(&str, u16) -> Result<SocketAddr, ErrorConexion>,
    abre: impl FnOnce(&Llaves, &Alta, SocketAddr) -> Result<A, String>,
    espera: Duration,
    ahora_ms: impl Fn() -> u64,
) -> Result<Conectado<A>, ErrorConexion> {
    if !alta::es_servidor(host) {
        return Err(ErrorConexion::Servidor);
    }
    let llaves = Llaves::nuevas().map_err(|e| ErrorConexion::Alta(e.to_string()))?;
    let (oferta, peticion) = alta::pide(&llaves);
    let respuesta = pide(host, &peticion)?;
    let alta = alta::completa(&llaves, &oferta, &respuesta)
        .map_err(|e| ErrorConexion::Respuesta(e.to_string()))?;
    let extremo = resolver(&alta.host, alta.puerto)?;
    let desde_ms = ahora_ms();
    let tunel = abre(&llaves, &alta, extremo).map_err(ErrorConexion::Tunel)?;
    let limite = Instant::now() + espera;
    while tunel.saludo_ms().is_none() {
        if Instant::now() >= limite {
            tunel.cierra();
            return Err(ErrorConexion::NoContesta);
        }
        std::thread::sleep(CADA.min(espera));
    }
    Ok(Conectado {
        tunel,
        servidor: alta.host.clone(),
        direccion: alta.direccion,
        desde_ms,
        caduca_ms: alta.caduca_ms,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::cuantico::{encapsula, psk};
    use crate::llaves::publica_de_base64;
    use crate::{b64, de_b64};
    use std::cell::Cell;
    use std::io::{Read, Write};
    use std::rc::Rc;
    use std::sync::Arc;

    /// What a GUARDIANA server answers to a sign-up (its other half lives in `guardiana-tunel-servidor`), and the
    /// preshared key it derived.
    fn servidor(p: &Pide, srv: &Llaves, direccion: &str) -> (Respuesta, [u8; 32]) {
        let ek = de_b64(&p.kem).unwrap();
        let (ct, ss) = encapsula(&ek).unwrap();
        let cliente = publica_de_base64(&p.llave).unwrap();
        let k = psk(&ss, &cliente, &srv.publica()).unwrap();
        (
            Respuesta {
                v: 1,
                direccion: direccion.into(),
                servidor: srv.publica_base64(),
                punto: "motor-hel.guardianagroup.com:51820".into(),
                kem: b64(&ct),
                dns: "10.66.0.1".into(),
                caduca_ms: 86_400_000,
            },
            k,
        )
    }

    /// A tunnel that answers after `tras` questions, and says when it is closed.
    #[derive(Debug)]
    struct Falso {
        preguntas: Cell<u32>,
        tras: Option<u32>,
        cerrado: Rc<Cell<bool>>,
        psk: [u8; 32],
    }

    impl Abierto for Falso {
        fn saludo_ms(&self) -> Option<u64> {
            let n = self.preguntas.get() + 1;
            self.preguntas.set(n);
            self.tras.filter(|t| n > *t).map(|_| 1_000)
        }
        fn cierra(self) {
            self.cerrado.set(true);
        }
    }

    fn intenta(
        host: &str,
        direccion: &str,
        tras: Option<u32>,
        espera: Duration,
    ) -> (
        Result<Conectado<Falso>, ErrorConexion>,
        Rc<Cell<bool>>,
        bool,
    ) {
        let srv = Llaves::nuevas().unwrap();
        let cerrado = Rc::new(Cell::new(false));
        let abierto = Rc::new(Cell::new(false));
        let (c, a) = (Rc::clone(&cerrado), Rc::clone(&abierto));
        let psk_servidor = Rc::new(Cell::new([0u8; 32]));
        let ps = Rc::clone(&psk_servidor);
        let r = conecta(
            host,
            |_, p| {
                let (r, k) = servidor(p, &srv, direccion);
                ps.set(k);
                Ok(r)
            },
            |h, puerto| {
                assert_eq!((h, puerto), ("motor-hel.guardianagroup.com", 51820));
                Ok("192.0.2.1:51820".parse().unwrap())
            },
            move |_, alta, extremo| {
                assert_eq!(extremo, "192.0.2.1:51820".parse().unwrap());
                a.set(true);
                Ok(Falso {
                    preguntas: Cell::new(0),
                    tras,
                    cerrado: c,
                    psk: *alta.psk,
                })
            },
            espera,
            || 5,
        );
        if let Ok(con) = &r {
            assert_eq!(
                con.tunel.psk,
                psk_servidor.get(),
                "la misma llave a los dos lados"
            );
        }
        let fue_abierto = abierto.get();
        (r, cerrado, fue_abierto)
    }

    #[test]
    fn una_conexion_entera_hasta_el_primer_saludo() {
        let (r, cerrado, abierto) = intenta(
            "motor.guardianagroup.com",
            "10.66.3.7/32",
            Some(2),
            Duration::from_secs(5),
        );
        let c = r.unwrap();
        assert!(abierto && !cerrado.get());
        assert_eq!(c.direccion, Ipv4Addr::new(10, 66, 3, 7));
        assert_eq!(c.servidor, "motor-hel.guardianagroup.com");
        assert_eq!((c.desde_ms, c.caduca_ms), (5, 86_400_000));
    }

    #[test]
    fn sin_saludo_el_tunel_se_cierra() {
        let (r, cerrado, abierto) = intenta(
            "motor.guardianagroup.com",
            "10.66.3.7/32",
            None,
            Duration::from_millis(600),
        );
        assert_eq!(r.unwrap_err(), ErrorConexion::NoContesta);
        assert!(abierto && cerrado.get(), "abierto y vuelto a cerrar");
        assert!(ErrorConexion::NoContesta
            .to_string()
            .contains("no contesta"));
    }

    #[test]
    fn nada_se_abre_con_un_servidor_ajeno_o_una_respuesta_que_desvia() {
        let (r, _, abierto) = intenta("evil.example.com", "10.66.3.7/32", Some(0), ESPERA);
        assert_eq!(r.unwrap_err(), ErrorConexion::Servidor);
        assert!(!abierto);
        let (r, _, abierto) = intenta("motor.guardianagroup.com", "8.8.8.8/32", Some(0), ESPERA);
        assert!(matches!(r.unwrap_err(), ErrorConexion::Respuesta(_)));
        assert!(!abierto);
        assert_eq!(
            pide_alta(
                "guardianagroup.com.evil.net",
                &alta::pide(&Llaves::nuevas().unwrap()).1
            ),
            Err(ErrorConexion::Servidor)
        );
    }

    /// A one-request HTTP server on this computer that answers `codigo` with what `responde` makes of the request's
    /// body; its address.
    fn servidor_local(codigo: u16, responde: impl Fn(&[u8]) -> String + Send + 'static) -> String {
        let escucha = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let puerto = escucha.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut s, _) = escucha.accept().unwrap();
            let mut leido = Vec::new();
            let mut buf = [0u8; 4096];
            // the headers, then as many bytes as Content-Length says
            let fin = loop {
                let n = s.read(&mut buf).unwrap();
                if n == 0 {
                    return;
                }
                leido.extend_from_slice(&buf[..n]);
                if let Some(i) = leido.windows(4).position(|w| w == b"\r\n\r\n") {
                    break i + 4;
                }
            };
            let cabeceras = String::from_utf8_lossy(&leido[..fin]).to_lowercase();
            let largo: usize = cabeceras
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0);
            while leido.len() < fin + largo {
                let n = s.read(&mut buf).unwrap();
                if n == 0 {
                    return;
                }
                leido.extend_from_slice(&buf[..n]);
            }
            let cuerpo = responde(&leido[fin..fin + largo]);
            let _ = write!(
                s,
                "HTTP/1.1 {codigo} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{cuerpo}",
                cuerpo.len()
            );
        });
        format!("http://127.0.0.1:{puerto}/vpn/alta")
    }

    #[test]
    fn el_alta_por_la_red_de_verdad() {
        let srv = Arc::new(Llaves::nuevas().unwrap());
        let equipo = Llaves::nuevas().unwrap();
        let (oferta, p) = alta::pide(&equipo);
        let k_servidor = Arc::new(std::sync::Mutex::new([0u8; 32]));
        let (s2, k2) = (Arc::clone(&srv), Arc::clone(&k_servidor));
        let url = servidor_local(200, move |cuerpo| {
            let p: Pide = serde_json::from_slice(cuerpo).unwrap();
            let (r, k) = servidor(&p, &s2, "10.66.9.9/32");
            *k2.lock().unwrap() = k;
            serde_json::to_string(&r).unwrap()
        });
        let r = pide_alta_en(&url, &p).unwrap();
        let a = alta::completa(&equipo, &oferta, &r).unwrap();
        assert_eq!(*a.psk, *k_servidor.lock().unwrap());
        assert_eq!(a.direccion, Ipv4Addr::new(10, 66, 9, 9));
        // a server that refuses: its code, and nothing else
        let url = servidor_local(503, |_| "{}".to_owned());
        assert_eq!(
            pide_alta_en(&url, &p),
            Err(ErrorConexion::Alta("el servidor respondió 503".to_owned()))
        );
        // an answer that is not a sign-up
        let url = servidor_local(200, |_| "{\"hola\":1}".to_owned());
        assert!(matches!(
            pide_alta_en(&url, &p),
            Err(ErrorConexion::Respuesta(_))
        ));
    }

    #[test]
    fn la_direccion_del_servidor_se_resuelve() {
        let a = resuelve("127.0.0.1", 51820).unwrap();
        assert_eq!(a, "127.0.0.1:51820".parse().unwrap());
        assert!(matches!(
            resuelve("no-existe.invalid", 1),
            Err(ErrorConexion::Nombre(_))
        ));
    }
}
