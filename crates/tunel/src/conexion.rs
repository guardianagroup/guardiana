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
//!
//! And once it is open, if the server stops answering (it restarted and no longer knows this computer's key): a new
//! sign-up with the same server and the same keys ([`caida`], [`renueva`]), straight to the address already known
//! ([`pide_alta_en`]), because with the cut-off switch on nothing resolves outside the tunnel. The tunnel and its
//! cut-off switch stay as they are meanwhile: nothing leaves outside, and the computer is back in a few seconds.

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
/// How long without the server's handshake before the tunnel is taken as dropped. WireGuard renews its session every
/// two minutes while there is traffic (the keepalive keeps some going), and keys older than three minutes are no longer
/// accepted; this, with a margin.
pub const SIN_SALUDO: Duration = Duration::from_secs(200);
/// How long between two attempts at a new sign-up while the tunnel is dropped.
pub const REINTENTO: Duration = Duration::from_secs(30);
/// The largest answer read from the server.
const MAXIMO_RESPUESTA: u64 = 16 * 1024;

/// A tunnel the system's half opened.
pub trait Abierto {
    /// When the server's last handshake was (milliseconds since 1970), if there has been one.
    fn saludo_ms(&self) -> Option<u64>;
    /// Closes it: the cut-off switch off and the adapter gone.
    fn cierra(self);
    /// Takes a new sign-up with the same server (`extremo`, its address): the server's preshared key and, if it changed,
    /// this computer's address in the tunnel, without closing the tunnel or its cut-off switch.
    ///
    /// # Errors
    /// What the system refused; the tunnel stays open, and closed to everything outside it.
    fn renueva(&mut self, llaves: &Llaves, alta: &Alta, extremo: SocketAddr) -> Result<(), String>;
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
    /// After a new sign-up, the server did not answer the handshake in time; the tunnel stays as it is, closed to
    /// everything outside it, and it is tried again.
    SinSaludo,
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
            Self::SinSaludo => f.write_str(
                "el servidor aún no contesta: el túnel sigue y nada sale fuera de él; se vuelve a intentar",
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
    pide_alta_a(&format!("https://{host}/vpn/alta"), pide)
}

/// The same exchange with a whole address: a test server, or one reached by its address. The program itself uses
/// [`pide_alta`]; the answer is checked by [`crate::alta::completa`] either way.
///
/// # Errors
/// [`ErrorConexion::Alta`] or [`ErrorConexion::Respuesta`] when the exchange fails.
pub fn pide_alta_a(url: &str, pide: &Pide) -> Result<Respuesta, ErrorConexion> {
    envia(&ureq::Agent::new_with_config(configuracion()), url, pide)
}

/// The sign-up over HTTPS with the server called `host`, reached at the address `en` without resolving its name: for a
/// new sign-up while the tunnel is dropped, when the cut-off switch lets nothing resolve outside it. The server's
/// certificate is still checked against `host`.
///
/// # Errors
/// As [`pide_alta`].
pub fn pide_alta_en(host: &str, en: SocketAddr, pide: &Pide) -> Result<Respuesta, ErrorConexion> {
    if !alta::es_servidor(host) {
        return Err(ErrorConexion::Servidor);
    }
    pide_alta_a_en(&format!("https://{host}/vpn/alta"), en, pide)
}

/// [`pide_alta_en`] with a whole address (a test server).
fn pide_alta_a_en(url: &str, en: SocketAddr, pide: &Pide) -> Result<Respuesta, ErrorConexion> {
    let agente = ureq::Agent::with_parts(
        configuracion(),
        ureq::unversioned::transport::DefaultConnector::default(),
        Fija(en),
    );
    envia(&agente, url, pide)
}

/// A resolver that resolves nothing: the address already known.
#[derive(Debug)]
struct Fija(SocketAddr);

impl ureq::unversioned::resolver::Resolver for Fija {
    fn resolve(
        &self,
        _uri: &ureq::http::Uri,
        _config: &ureq::config::Config,
        _timeout: ureq::unversioned::transport::NextTimeout,
    ) -> Result<ureq::unversioned::resolver::ResolvedSocketAddrs, ureq::Error> {
        let mut todas = self.empty();
        todas.push(self.0);
        Ok(todas)
    }
}

/// How the sign-up is asked for: a time limit, every answer read (its code is checked here), no redirection.
fn configuracion() -> ureq::config::Config {
    ureq::config::Config::builder()
        .timeout_global(Some(PLAZO_ALTA))
        .http_status_as_error(false)
        .max_redirects(0)
        .build()
}

fn envia(agente: &ureq::Agent, url: &str, pide: &Pide) -> Result<Respuesta, ErrorConexion> {
    let cuerpo = serde_json::to_vec(pide).map_err(|e| ErrorConexion::Alta(e.to_string()))?;
    let respuesta = agente
        .post(url)
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
    /// The server's address, resolved before the tunnel opened: a new sign-up goes straight to it ([`renueva`]).
    pub extremo: SocketAddr,
    /// The server the sign-up was asked of.
    pedida_a: String,
    /// This computer's keys for this connection: the same ones for a new sign-up with the same server.
    llaves: Llaves,
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
        extremo,
        pedida_a: host.to_owned(),
        llaves,
    })
}

/// Whether a connection opened at `desde_ms` has dropped and should try a new sign-up now (all in milliseconds since
/// 1970): the server's last handshake, or the opening when there has been none since, is older than [`SIN_SALUDO`]; and
/// the last attempt, if any, is older than [`REINTENTO`].
#[must_use]
pub fn caida(
    ahora_ms: u64,
    desde_ms: u64,
    ultimo_saludo_ms: Option<u64>,
    ultimo_intento_ms: Option<u64>,
) -> bool {
    let vivo = ultimo_saludo_ms.unwrap_or(desde_ms).max(desde_ms);
    let callado = ahora_ms.saturating_sub(vivo) >= en_ms(SIN_SALUDO);
    let reciente = ultimo_intento_ms.is_some_and(|t| ahora_ms.saturating_sub(t) < en_ms(REINTENTO));
    callado && !reciente
}

fn en_ms(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// A new sign-up for a connection that dropped ([`caida`]), with the same server and the same keys: the sign-up made by
/// `pide` (in the program, [`pide_alta_en`] at the server's known address), its answer checked, and the tunnel given the
/// server's new preshared key and, if it changed, the new address, its cut-off switch on throughout. Then the server's
/// handshake is waited for `espera`.
///
/// # Errors
/// [`ErrorConexion`] with the step that failed ([`ErrorConexion::SinSaludo`] when the handshake does not come). The
/// tunnel is never closed here: it stays open, closed to everything outside, to try again.
pub fn renueva<A: Abierto>(
    c: &mut Conectado<A>,
    pide: impl FnOnce(&str, &Pide) -> Result<Respuesta, ErrorConexion>,
    espera: Duration,
    ahora_ms: impl Fn() -> u64,
) -> Result<(), ErrorConexion> {
    let (oferta, peticion) = alta::pide(&c.llaves);
    let respuesta = pide(&c.pedida_a, &peticion)?;
    let alta = alta::completa(&c.llaves, &oferta, &respuesta)
        .map_err(|e| ErrorConexion::Respuesta(e.to_string()))?;
    if alta.host != c.servidor {
        // nothing resolves outside the tunnel now: another name would be another connection
        return Err(ErrorConexion::Respuesta(
            "el servidor ha cambiado de nombre".to_owned(),
        ));
    }
    let antes = ahora_ms();
    c.tunel
        .renueva(&c.llaves, &alta, c.extremo)
        .map_err(ErrorConexion::Tunel)?;
    c.direccion = alta.direccion;
    c.caduca_ms = alta.caduca_ms;
    let limite = Instant::now() + espera;
    while !c.tunel.saludo_ms().is_some_and(|s| s >= antes) {
        if Instant::now() >= limite {
            return Err(ErrorConexion::SinSaludo);
        }
        std::thread::sleep(CADA.min(espera));
    }
    Ok(())
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

    /// A tunnel that answers after `tras` questions, and says when it is closed. After a new sign-up it keeps what it
    /// was given and answers after `tras_renovar` questions.
    #[derive(Debug)]
    struct Falso {
        preguntas: Cell<u32>,
        tras: Option<u32>,
        cerrado: Rc<Cell<bool>>,
        psk: [u8; 32],
        renovado: Option<Ipv4Addr>,
        tras_renovar: Option<u32>,
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
        fn renueva(
            &mut self,
            _llaves: &Llaves,
            alta: &Alta,
            extremo: SocketAddr,
        ) -> Result<(), String> {
            assert_eq!(extremo, "192.0.2.1:51820".parse().unwrap());
            self.psk = *alta.psk;
            self.renovado = Some(alta.direccion);
            self.preguntas.set(0);
            self.tras = self.tras_renovar;
            Ok(())
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
                    renovado: None,
                    tras_renovar: None,
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
        let r = pide_alta_a(&url, &p).unwrap();
        let a = alta::completa(&equipo, &oferta, &r).unwrap();
        assert_eq!(*a.psk, *k_servidor.lock().unwrap());
        assert_eq!(a.direccion, Ipv4Addr::new(10, 66, 9, 9));
        // a server that refuses: its code, and nothing else
        let url = servidor_local(503, |_| "{}".to_owned());
        assert_eq!(
            pide_alta_a(&url, &p),
            Err(ErrorConexion::Alta("el servidor respondió 503".to_owned()))
        );
        // an answer that is not a sign-up
        let url = servidor_local(200, |_| "{\"hola\":1}".to_owned());
        assert!(matches!(
            pide_alta_a(&url, &p),
            Err(ErrorConexion::Respuesta(_))
        ));
    }

    /// A connection made against a server made up here (as in [`intenta`]), with that server, for a new sign-up; its
    /// tunnel answers after a new sign-up when `tras_renovar` says.
    fn conectado(tras_renovar: Option<u32>) -> (Conectado<Falso>, Llaves, Rc<Cell<bool>>) {
        let srv = Llaves::nuevas().unwrap();
        let cerrado = Rc::new(Cell::new(false));
        let c2 = Rc::clone(&cerrado);
        let c = conecta(
            "vpn.guardianagroup.com",
            |_, p| Ok(servidor(p, &srv, "10.66.3.7/32").0),
            |_, _| Ok("192.0.2.1:51820".parse().unwrap()),
            move |_, alta, _| {
                Ok(Falso {
                    preguntas: Cell::new(0),
                    tras: Some(0),
                    cerrado: c2,
                    psk: *alta.psk,
                    renovado: None,
                    tras_renovar,
                })
            },
            Duration::from_secs(5),
            || 5,
        )
        .unwrap();
        (c, srv, cerrado)
    }

    #[test]
    fn cuando_una_conexion_se_da_por_caida() {
        let (s, r) = (en_ms(SIN_SALUDO), en_ms(REINTENTO));
        // recién abierta y sin saludo todavía: hasta SIN_SALUDO, no
        assert!(!caida(1_000 + s - 1, 1_000, None, None));
        assert!(caida(1_000 + s, 1_000, None, None));
        // con saludos, se cuenta desde el último
        assert!(!caida(900_000, 1_000, Some(900_000 - s + 1), None));
        assert!(caida(900_000, 1_000, Some(900_000 - s), None));
        // un saludo de antes de abrir no cuenta
        assert!(caida(1_000 + s, 1_000, Some(10), None));
        // tras un intento, se espera REINTENTO para el siguiente
        assert!(!caida(900_000, 1_000, Some(1_000), Some(900_000 - r + 1)));
        assert!(caida(900_000, 1_000, Some(1_000), Some(900_000 - r)));
    }

    #[test]
    fn una_conexion_caida_vuelve_con_un_alta_nueva_sin_cerrar_el_tunel() {
        let (mut c, srv, cerrado) = conectado(Some(1));
        let mia = c.llaves.publica_base64();
        let k = Rc::new(Cell::new([0u8; 32]));
        let k2 = Rc::clone(&k);
        renueva(
            &mut c,
            |host, p| {
                assert_eq!(host, "vpn.guardianagroup.com", "al mismo servidor");
                assert_eq!(p.llave, mia, "con las mismas llaves");
                let (r, psk) = servidor(p, &srv, "10.66.4.4/32");
                k2.set(psk);
                Ok(r)
            },
            Duration::from_secs(5),
            || 6,
        )
        .unwrap();
        assert_eq!(c.direccion, Ipv4Addr::new(10, 66, 4, 4));
        assert_eq!(c.tunel.renovado, Some(Ipv4Addr::new(10, 66, 4, 4)));
        assert_eq!(
            c.tunel.psk,
            k.get(),
            "la llave postcuántica nueva, la misma a los dos lados"
        );
        assert!(!cerrado.get());
    }

    #[test]
    fn si_el_servidor_no_saluda_tras_el_alta_nueva_el_tunel_sigue_y_se_reintenta() {
        let (mut c, srv, cerrado) = conectado(None);
        let r = renueva(
            &mut c,
            |_, p| Ok(servidor(p, &srv, "10.66.4.4/32").0),
            Duration::from_millis(600),
            || 6,
        );
        assert_eq!(r, Err(ErrorConexion::SinSaludo));
        assert!(
            !cerrado.get(),
            "el túnel no se cierra: nada sale fuera de él"
        );
        assert!(ErrorConexion::SinSaludo
            .to_string()
            .contains("nada sale fuera"));
    }

    #[test]
    fn un_alta_nueva_que_no_vale_no_toca_el_tunel() {
        let (mut c, srv, cerrado) = conectado(Some(1));
        let negada = ErrorConexion::Alta("el servidor respondió 503".to_owned());
        let n2 = negada.clone();
        assert_eq!(
            renueva(&mut c, move |_, _| Err(n2), ESPERA, || 6),
            Err(negada)
        );
        let r = renueva(
            &mut c,
            |_, p| {
                let (mut r, _) = servidor(p, &srv, "10.66.4.4/32");
                r.punto = "otro.guardianagroup.com:51820".into();
                Ok(r)
            },
            ESPERA,
            || 6,
        );
        assert!(matches!(r, Err(ErrorConexion::Respuesta(_))));
        assert_eq!(c.tunel.renovado, None);
        assert_eq!(c.direccion, Ipv4Addr::new(10, 66, 3, 7));
        assert!(!cerrado.get());
    }

    #[test]
    fn el_alta_a_una_direccion_ya_sabida_sin_resolver_el_nombre() {
        let srv = Llaves::nuevas().unwrap();
        let equipo = Llaves::nuevas().unwrap();
        let (oferta, p) = alta::pide(&equipo);
        let url = servidor_local(200, move |cuerpo| {
            let p: Pide = serde_json::from_slice(cuerpo).unwrap();
            serde_json::to_string(&servidor(&p, &srv, "10.66.9.9/32").0).unwrap()
        });
        let puerto: u16 = url
            .rsplit(':')
            .next()
            .and_then(|r| r.split('/').next())
            .unwrap()
            .parse()
            .unwrap();
        // el mismo servidor, por un nombre que no resuelve en ningún sitio, en la dirección ya sabida
        let r = pide_alta_a_en(
            &format!("http://alta.invalid:{puerto}/vpn/alta"),
            SocketAddr::from(([127, 0, 0, 1], puerto)),
            &p,
        )
        .unwrap();
        assert_eq!(
            alta::completa(&equipo, &oferta, &r).unwrap().direccion,
            Ipv4Addr::new(10, 66, 9, 9)
        );
        assert_eq!(
            pide_alta_en(
                "guardianagroup.com.evil.net",
                SocketAddr::from(([127, 0, 0, 1], 443)),
                &p
            ),
            Err(ErrorConexion::Servidor)
        );
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
