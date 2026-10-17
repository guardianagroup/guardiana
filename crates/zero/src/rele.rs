//! The computer's side of the blind relay (`guardiana-rele`): used only when no door could be
//! opened in the router (offices, hotels, operators with shared addresses). Both computers
//! connect out to the relay with the same room; the relay joins them and passes bytes it
//! cannot read. The invitation code for this path is `GR-XXXX-XXXX-XX`; the room is a hash of
//! it, so the relay never learns the code either.
//!
//! Since 16 Oct 2026 the computer reaches GUARDIANA's relay inside TLS, checked against the
//! relay's public certificate, exactly as a browser opens a web page: a home line of Movistar in
//! Cartagena cut the plain line on port 443 and let the HTTPS page of the same relay through.
//! The plain line stays as the last resort, and the relay accepts both.
//!
//! The relay's name is fixed here and can be changed with `GUARDIANA_RELE` (the tests run one
//! on this computer, plain).

use std::io::{Read as _, Write as _};
use std::net::{Ipv4Addr, TcpListener, TcpStream, ToSocketAddrs as _};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use sha2::{Digest, Sha256};

/// One of GUARDIANA's relays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rele {
    /// The name in its certificate.
    pub nombre: &'static str,
    /// Its address, for a computer whose DNS cannot resolve the name (tried after the name;
    /// empty while the server is not up yet).
    pub ip: &'static str,
}

impl Rele {
    /// `nombre:443`.
    #[must_use]
    pub fn direccion(&self) -> String {
        format!("{}:443", self.nombre)
    }
}

/// The relays, in a fixed order the invitation code refers to (two bits of it, since 16 Oct
/// 2026: «el relé de América»). The first one is the original, Nuremberg; the second, America.
/// A code made by an older version carries random bits there, so the joining side tries every
/// relay at once and the one where the other person waits wins.
pub const RELES: &[Rele] = &[
    Rele {
        nombre: "rele.guardianagroup.com",
        ip: "5.75.173.240",
    },
    Rele {
        nombre: "rele-am.guardianagroup.com",
        // Vultr, Miami (16 Oct 2026).
        ip: "207.246.115.38",
    },
];
/// Where the first relay lives (443 so it passes any office firewall).
pub const RELE: &str = "rele.guardianagroup.com:443";
/// The same relay by address, for a computer whose DNS cannot resolve the name (15 Oct 2026:
/// the first server, Nuremberg). Tried only after the name.
pub const RELE_IP: &str = "5.75.173.240:443";
/// The name in the relay's certificate (checked even when it is reached by address).
pub const RELE_NOMBRE: &str = "rele.guardianagroup.com";
/// The letters of a code: no 0/O/1/I, so it reads well aloud.
const ALFABETO: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// The nearest relay, measured once (the time to open a TCP connection to each, two seconds
/// at most); 0 until measured, or when the environment names a relay.
static PREFERIDO: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

/// Measure once which relay answers fastest from here, in its own thread; `preferido()` reads
/// the result when it is in. Nothing is sent but a connection that is closed at once.
pub fn mide_cercania() {
    if PREFERIDO.get().is_some() || std::env::var_os("GUARDIANA_RELE").is_some() {
        return;
    }
    std::thread::spawn(|| {
        let mut mejor = (0usize, Duration::MAX);
        for (i, r) in RELES.iter().enumerate() {
            let t = std::time::Instant::now();
            if abre_hacia(&r.direccion()).is_ok() {
                let d = t.elapsed();
                if d < mejor.1 {
                    mejor = (i, d);
                }
            }
        }
        let _ = PREFERIDO.set(mejor.0);
    });
}

/// The relay this computer prefers (index into [`RELES`]).
#[must_use]
pub fn preferido() -> usize {
    PREFERIDO.get().copied().unwrap_or(0)
}

/// The relay's address: the environment's, or the preferred one's.
pub fn direccion() -> String {
    std::env::var("GUARDIANA_RELE").unwrap_or_else(|_| RELES[preferido()].direccion())
}

/// A fresh invitation code for the relay path: the relay to wait at (two bits), 48 random bits
/// and a check letter.
pub fn codigo_nuevo() -> String {
    codigo_nuevo_en(preferido())
}

/// A fresh code that names the relay `indice` (see [`RELES`]).
pub fn codigo_nuevo_en(indice: usize) -> String {
    let mut azar = [0u8; 8];
    crate::sesion::azar(&mut azar);
    let bits = (u64::from_be_bytes(azar) & ((1u64 << 48) - 1)) | (((indice & 3) as u64) << 48);
    let mut letras = Vec::with_capacity(11);
    for i in (0..10).rev() {
        letras.push(ALFABETO[((bits >> (i * 5)) & 31) as usize]);
    }
    let suma: u32 = letras.iter().map(|&b| u32::from(b)).sum();
    letras.push(ALFABETO[(suma % 32) as usize]);
    let s: String = letras.iter().map(|&b| b as char).collect();
    format!("GR-{}-{}-{}", &s[..4], &s[4..8], &s[8..])
}

/// The relay a code names: the two top bits of its first letter. A code from an older version
/// has random bits there, so the joining side still tries every relay.
#[must_use]
pub fn rele_de_codigo(texto: &str) -> usize {
    let limpio: String = texto
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    let Some(cuerpo) = limpio.strip_prefix("GR") else {
        return 0;
    };
    let Some(pos) = cuerpo
        .bytes()
        .next()
        .and_then(|b| ALFABETO.iter().position(|&x| x == b))
    else {
        return 0;
    };
    let i = (pos >> 3) & 3;
    if i < RELES.len() {
        i
    } else {
        0
    }
}

/// The room a code names, or `None` if the text is not a relay code. Spaces, dashes and case do
/// not matter.
pub fn sala_de(texto: &str) -> Option<String> {
    let limpio: String = texto
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    let cuerpo = limpio.strip_prefix("GR")?;
    if cuerpo.len() != 11 {
        return None;
    }
    let b = cuerpo.as_bytes();
    if !b.iter().all(|x| ALFABETO.contains(x)) {
        return None;
    }
    let suma: u32 = b[..10].iter().map(|&x| u32::from(x)).sum();
    if ALFABETO[(suma % 32) as usize] != b[10] {
        return None;
    }
    let h = Sha256::digest(format!("GUARDIANA SECURE relé {cuerpo}").as_bytes());
    Some(h[..16].iter().map(|x| format!("{x:02x}")).collect())
}

fn abre_hacia(rele: &str) -> Result<TcpStream, String> {
    let addr = rele
        .to_socket_addrs()
        .map_err(|e| e.to_string())?
        .next()
        .ok_or_else(|| "relé sin dirección".to_owned())?;
    TcpStream::connect_timeout(&addr, Duration::from_secs(10)).map_err(|e| e.to_string())
}

/// The public authorities a browser trusts, for the relay's certificate (Let's Encrypt).
fn raices_publicas() -> Arc<rustls::RootCertStore> {
    Arc::new(webpki_roots::TLS_SERVER_ROOTS.iter().cloned().collect())
}

/// A link to the relay inside TLS, checked against `nombre`'s certificate, handed back as a
/// plain socket on this computer (a thread carries the bytes between the two), so the rest of
/// SECURE does not change: the same `TcpStream` as the plain path, cloned for its reader.
///
/// Why (16 Oct 2026, a home line of Movistar in Cartagena): some networks pass port 443 only
/// when it looks like a web page; the plain relay line never got through, the HTTPS page of the
/// same relay did. Inside TLS the relay still sees only what it saw before: a room name and
/// bytes already encrypted end to end.
fn por_tls(
    destino: &str,
    nombre: &str,
    raices: Arc<rustls::RootCertStore>,
) -> Result<TcpStream, String> {
    let tcp = abre_hacia(destino)?;
    let _ = tcp.set_nodelay(true);
    let nombre =
        rustls::pki_types::ServerName::try_from(nombre.to_owned()).map_err(|e| e.to_string())?;
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|e| e.to_string())?
    .with_root_certificates(raices)
    .with_no_client_auth();
    // The local end SECURE uses, joined to its twin through loopback only.
    let escucha = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|e| e.to_string())?;
    let nuestro = TcpStream::connect(escucha.local_addr().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let (puente, de) = escucha.accept().map_err(|e| e.to_string())?;
    if Some(de) != nuestro.local_addr().ok() {
        return Err("puente local ajeno".to_owned());
    }
    drop(escucha);
    let _ = nuestro.set_nodelay(true);
    let _ = puente.set_nodelay(true);
    let (tx, rx) = mpsc::channel::<Result<(), String>>();
    std::thread::Builder::new()
        .name("guardiana-rele-tls".to_owned())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = tx.send(Err(e.to_string()));
                    return;
                }
            };
            rt.block_on(async move {
                let abrir = async {
                    tcp.set_nonblocking(true)?;
                    let tcp = tokio::net::TcpStream::from_std(tcp)?;
                    tokio_rustls::TlsConnector::from(Arc::new(config))
                        .connect(nombre, tcp)
                        .await
                };
                let mut tls = match tokio::time::timeout(Duration::from_secs(10), abrir).await {
                    Ok(Ok(t)) => t,
                    Ok(Err(e)) => {
                        let _ = tx.send(Err(format!("TLS: {e}")));
                        return;
                    }
                    Err(_) => {
                        let _ = tx.send(Err("TLS: tiempo agotado".to_owned()));
                        return;
                    }
                };
                let local = puente
                    .set_nonblocking(true)
                    .and_then(|()| tokio::net::TcpStream::from_std(puente));
                let mut local = match local {
                    Ok(l) => l,
                    Err(e) => {
                        let _ = tx.send(Err(e.to_string()));
                        return;
                    }
                };
                let _ = tx.send(Ok(()));
                let _ = tokio::io::copy_bidirectional(&mut local, &mut tls).await;
            });
        })
        .map_err(|e| e.to_string())?;
    match rx.recv_timeout(Duration::from_secs(25)) {
        Ok(Ok(())) => Ok(nuestro),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("TLS: sin respuesta".to_owned()),
    }
}

/// A link to the relay: for GUARDIANA's own relay, inside TLS first (by name, then by
/// address), and the plain line only if TLS could not be opened at all; any other relay (the
/// tests) plain, as before.
pub fn abre(rele: &str) -> Result<TcpStream, String> {
    let Some(r) = RELES.iter().find(|r| r.direccion() == rele) else {
        return abre_hacia(rele);
    };
    let raices = raices_publicas();
    let mut fallos = Vec::new();
    let mut destinos = vec![r.direccion()];
    if !r.ip.is_empty() {
        destinos.push(format!("{}:443", r.ip));
    }
    for destino in &destinos {
        match por_tls(destino, r.nombre, Arc::clone(&raices)) {
            Ok(s) => return Ok(s),
            Err(e) => fallos.push(e),
        }
    }
    for destino in &destinos {
        match abre_hacia(destino) {
            Ok(s) => return Ok(s),
            Err(e) => fallos.push(e),
        }
    }
    Err(fallos.join("; "))
}

/// The joining side: go to every relay at once with the same room (the other person waits at
/// one of them, the one its code names, or the first for an older version) and keep the first
/// link the pairing completes. With `GUARDIANA_RELE` set (the tests), only that one.
pub fn conecta_a_cualquiera(sala: &str) -> Result<TcpStream, String> {
    if let Ok(uno) = std::env::var("GUARDIANA_RELE") {
        return conecta(&uno, sala);
    }
    let (tx, rx) = std::sync::mpsc::channel();
    for r in RELES {
        let (tx, sala, d) = (tx.clone(), sala.to_owned(), r.direccion());
        std::thread::spawn(move || {
            let _ = tx.send(conecta(&d, &sala));
        });
    }
    drop(tx);
    let mut fallos = Vec::new();
    while let Ok(r) = rx.recv() {
        match r {
            Ok(s) => return Ok(s),
            Err(e) => fallos.push(e),
        }
    }
    Err(fallos.join("; "))
}

/// Connect to the relay, name the room and wait (up to ten minutes) for the other computer.
/// Returns the stream once the relay says the pair is complete.
pub fn conecta(rele: &str, sala: &str) -> Result<TcpStream, String> {
    empareja(abre(rele)?, sala)
}

/// Name the room on an open link to the relay and wait for the pair.
fn empareja(mut s: TcpStream, sala: &str) -> Result<TcpStream, String> {
    let _ = s.set_nodelay(true);
    s.write_all(format!("GUARDIANA-RELE 1 {sala}\n").as_bytes())
        .map_err(|e| e.to_string())?;
    s.set_read_timeout(Some(Duration::from_secs(11 * 60)))
        .map_err(|e| e.to_string())?;
    // The answer, byte by byte up to the newline, so nothing that follows it is swallowed.
    let mut linea = Vec::new();
    loop {
        let mut b = [0u8; 1];
        s.read_exact(&mut b).map_err(|e| e.to_string())?;
        if b[0] == b'\n' {
            break;
        }
        linea.push(b[0]);
        if linea.len() > 8 {
            return Err("respuesta del relé ilegible".to_owned());
        }
    }
    if linea != b"OK" {
        return Err("el relé no emparejó".to_owned());
    }
    let _ = s.set_read_timeout(None);
    Ok(s)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_relay_code_names_a_room_and_a_typo_does_not() {
        let c = codigo_nuevo();
        assert!(c.starts_with("GR-"));
        assert_eq!(c.len(), 16);
        let sala = sala_de(&c).unwrap();
        assert_eq!(sala.len(), 32);
        assert_eq!(sala_de(&c.to_lowercase().replace('-', " ")).unwrap(), sala);
        let mut mal = c.clone().into_bytes();
        mal[3] = if mal[3] == b'A' { b'B' } else { b'A' };
        assert!(sala_de(std::str::from_utf8(&mal).unwrap()).is_none());
        assert!(sala_de("GS-ABCD-EFGH-JK").is_none());
        assert_ne!(sala_de(&codigo_nuevo()), Some(sala));
        // The relay a code names (16 Oct 2026, the relay in America): two bits of the first
        // letter; an index past the list reads as the first relay.
        assert_eq!(rele_de_codigo(&codigo_nuevo_en(0)), 0);
        assert_eq!(rele_de_codigo(&codigo_nuevo_en(1)), 1);
        assert_eq!(rele_de_codigo(&codigo_nuevo_en(3)), 0);
        assert_eq!(rele_de_codigo("nada"), 0);
        let c1 = codigo_nuevo_en(1);
        assert_eq!(sala_de(&c1).unwrap().len(), 32);
    }

    /// 16 Oct 2026, a Movistar home line in Cartagena: the plain line never reached the relay,
    /// its HTTPS page did. Two computers now meet inside TLS (a certificate made up for the test
    /// and trusted only here), a big piece crosses whole, a wrong name or an unknown authority
    /// is refused, an older computer on the plain line still meets a newer one, and closing one
    /// side closes the other.
    #[test]
    fn two_computers_meet_inside_tls_and_an_older_one_still_meets_a_newer_one() {
        use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

        let id = wtransport::Identity::self_signed(["rele.prueba"]).unwrap();
        let cert = CertificateDer::from(id.certificate_chain().as_slice()[0].der().to_vec());
        let clave = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
            id.private_key().secret_der().to_vec(),
        ));
        let servidor = guardiana_rele::tls::config(vec![cert.clone()], clave).unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let listener = rt
            .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
            .unwrap();
        let dir = listener.local_addr().unwrap().to_string();
        rt.spawn(guardiana_rele::sirve_con(listener, Some(servidor)));
        let mut raices = rustls::RootCertStore::empty();
        raices.add(cert).unwrap();
        let raices = Arc::new(raices);

        let sala = sala_de(&codigo_nuevo()).unwrap();
        let (d2, s2, r2) = (dir.clone(), sala.clone(), Arc::clone(&raices));
        let otro = std::thread::spawn(move || {
            empareja(por_tls(&d2, "rele.prueba", r2).unwrap(), &s2).unwrap()
        });
        std::thread::sleep(Duration::from_millis(200));
        let mut a = empareja(
            por_tls(&dir, "rele.prueba", Arc::clone(&raices)).unwrap(),
            &sala,
        )
        .unwrap();
        let mut b = otro.join().unwrap();
        a.write_all(b"hola desde a").unwrap();
        let mut buf = [0u8; 12];
        b.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"hola desde a");
        let grande = vec![7u8; 3_000_000];
        let mut b2 = b.try_clone().unwrap();
        let lector = std::thread::spawn(move || {
            let mut v = vec![0u8; 3_000_000];
            b2.read_exact(&mut v).unwrap();
            v
        });
        a.write_all(&grande).unwrap();
        assert_eq!(lector.join().unwrap(), grande);

        assert!(por_tls(&dir, "otro.nombre", Arc::clone(&raices)).is_err());
        assert!(por_tls(
            &dir,
            "rele.prueba",
            Arc::new(rustls::RootCertStore::empty())
        )
        .is_err());

        let sala2 = sala_de(&codigo_nuevo()).unwrap();
        let (d3, s3) = (dir.clone(), sala2.clone());
        let viejo = std::thread::spawn(move || empareja(abre_hacia(&d3).unwrap(), &s3).unwrap());
        std::thread::sleep(Duration::from_millis(200));
        let mut nuevo = empareja(por_tls(&dir, "rele.prueba", raices).unwrap(), &sala2).unwrap();
        let mut viejo = viejo.join().unwrap();
        viejo.write_all(b"v").unwrap();
        let mut c = [0u8; 1];
        nuevo.read_exact(&mut c).unwrap();
        assert_eq!(&c, b"v");
        drop(nuevo);
        viejo
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut fin = [0u8; 1];
        assert_eq!(viejo.read(&mut fin).unwrap(), 0);
    }
}
