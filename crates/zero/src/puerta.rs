//! The door in the router, opened by the program itself (UPnP IGD), so two people in two
//! countries can talk without touching anything: Francisco, 15 Oct 2026, «que solo le aparezca
//! en pantalla y darle OK». The result is a short invitation code (the public address and the
//! port, encoded) the person reads to the other. Nothing here is written to disk, and the code
//! is shown as a code, never as an address.
//!
//! Plain UPnP: SSDP on the local network to find the router, its description over HTTP, two SOAP
//! calls (`AddPortMapping`, `GetExternalIPAddress`). No third party is contacted: the router is
//! the only one asked. When the router says no (UPnP off, or an operator's shared address),
//! the program says so and the other person has to be the one who waits.

use std::io::{Read as _, Write as _};
use std::net::{Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs as _, UdpSocket};
use std::time::Duration;

const SSDP: &str = "239.255.255.250:1900";
const ESPERA: Duration = Duration::from_secs(3);
const DESCRIPCION: &str = "GUARDIANA SECURE";

/// What the router gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puerta {
    /// The address the world sees. Shown only inside the code.
    pub externa: Ipv4Addr,
    /// The port, outside and inside (the same).
    pub puerto: u16,
    /// Where to undo the mapping later: the control URL.
    control: String,
    tipo: String,
}

/// Why no door could be opened, in words for the screen (never an address).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SinPuerta {
    /// No router answered on the local network (UPnP off, or not a home network).
    SinRouter,
    /// The router answered but refused the mapping.
    Rechazada(String),
    /// The router's public address is an operator's shared one (CGNAT): nothing can reach it.
    DireccionCompartida,
    /// The router gave no usable answer.
    Ilegible,
}

impl std::fmt::Display for SinPuerta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SinPuerta::SinRouter => f.write_str("sin router UPnP"),
            SinPuerta::Rechazada(e) => write!(f, "el router dijo que no ({e})"),
            SinPuerta::DireccionCompartida => f.write_str("dirección compartida del operador"),
            SinPuerta::Ilegible => f.write_str("respuesta del router ilegible"),
        }
    }
}

/// Ask the local network's router to open `puerto` to this computer, for `segundos`.
pub fn abre(puerto: u16, segundos: u32) -> Result<Puerta, SinPuerta> {
    let ubicacion = descubre().ok_or(SinPuerta::SinRouter)?;
    let (host, ruta) = parte_url(&ubicacion).ok_or(SinPuerta::Ilegible)?;
    let descripcion = http(&host, "GET", &ruta, &[], "").map_err(|_| SinPuerta::SinRouter)?;
    let (tipo, control) = servicio(&descripcion).ok_or(SinPuerta::Ilegible)?;
    let control = if control.starts_with("http") {
        control
    } else {
        format!(
            "http://{host}{}{control}",
            if control.starts_with('/') { "" } else { "/" }
        )
    };
    let (chost, cruta) = parte_url(&control).ok_or(SinPuerta::Ilegible)?;
    let interna = ip_local_hacia(&chost).ok_or(SinPuerta::Ilegible)?;
    let externa = soap(&chost, &cruta, &tipo, "GetExternalIPAddress", "")
        .ok()
        .and_then(|r| entre(&r, "NewExternalIPAddress"))
        .and_then(|s| s.trim().parse::<Ipv4Addr>().ok())
        .ok_or(SinPuerta::Ilegible)?;
    if compartida(externa) {
        return Err(SinPuerta::DireccionCompartida);
    }
    let args = format!(
        "<NewRemoteHost></NewRemoteHost><NewExternalPort>{puerto}</NewExternalPort><NewProtocol>TCP</NewProtocol><NewInternalPort>{puerto}</NewInternalPort><NewInternalClient>{interna}</NewInternalClient><NewEnabled>1</NewEnabled><NewPortMappingDescription>{DESCRIPCION}</NewPortMappingDescription><NewLeaseDuration>{segundos}</NewLeaseDuration>"
    );
    match soap(&chost, &cruta, &tipo, "AddPortMapping", &args) {
        Ok(r) if !r.contains("UPnPError") => {}
        Ok(r) if r.contains("<errorCode>725</errorCode>") => {
            // Only permanent leases: ask again without a lease; it is undone on close.
            let args = args.replace(
                &format!("<NewLeaseDuration>{segundos}</NewLeaseDuration>"),
                "<NewLeaseDuration>0</NewLeaseDuration>",
            );
            let r2 = soap(&chost, &cruta, &tipo, "AddPortMapping", &args)
                .map_err(|_| SinPuerta::Ilegible)?;
            if r2.contains("UPnPError") {
                return Err(SinPuerta::Rechazada(codigo_error(&r2)));
            }
        }
        Ok(r) => return Err(SinPuerta::Rechazada(codigo_error(&r))),
        Err(_) => return Err(SinPuerta::Ilegible),
    }
    Ok(Puerta {
        externa,
        puerto,
        control,
        tipo,
    })
}

impl Puerta {
    /// Close the door again.
    pub fn cierra(&self) {
        if let Some((host, ruta)) = parte_url(&self.control) {
            let args = format!(
                "<NewRemoteHost></NewRemoteHost><NewExternalPort>{}</NewExternalPort><NewProtocol>TCP</NewProtocol>",
                self.puerto
            );
            let _ = soap(&host, &ruta, &self.tipo, "DeletePortMapping", &args);
        }
    }

    /// The invitation code for the other person.
    pub fn codigo(&self) -> String {
        codigo(self.externa, self.puerto)
    }
}

/// An operator's shared address (CGNAT, RFC 6598) or a private one: unreachable from outside.
pub fn compartida(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    (o[0] == 100 && (64..=127).contains(&o[1]))
        || ip.is_private()
        || ip.is_loopback()
        || ip.is_unspecified()
}

fn descubre() -> Option<String> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.set_read_timeout(Some(ESPERA)).ok()?;
    let m = "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 2\r\nST: urn:schemas-upnp-org:device:InternetGatewayDevice:1\r\n\r\n";
    let dest: SocketAddr = SSDP.parse().ok()?;
    s.send_to(m.as_bytes(), dest).ok()?;
    let _ = s.send_to(
        m.replace("InternetGatewayDevice:1", "InternetGatewayDevice:2")
            .as_bytes(),
        dest,
    );
    let mut buf = [0u8; 2048];
    let empezo = std::time::Instant::now();
    while empezo.elapsed() < ESPERA {
        let Ok((n, _)) = s.recv_from(&mut buf) else {
            break;
        };
        let texto = String::from_utf8_lossy(&buf[..n]);
        if let Some(loc) = ubicacion_de(&texto) {
            return Some(loc);
        }
    }
    None
}

/// The LOCATION header of an SSDP answer.
pub(crate) fn ubicacion_de(respuesta: &str) -> Option<String> {
    respuesta.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        if k.trim().eq_ignore_ascii_case("location") {
            let v = v.trim();
            if v.starts_with("http://") {
                return Some(v.to_owned());
            }
        }
        None
    })
}

/// host:port and path of an http URL.
pub(crate) fn parte_url(url: &str) -> Option<(String, String)> {
    let resto = url.strip_prefix("http://")?;
    let (host, ruta) = match resto.find('/') {
        Some(i) => (&resto[..i], &resto[i..]),
        None => (resto, "/"),
    };
    let host = if host.contains(':') {
        host.to_owned()
    } else {
        format!("{host}:80")
    };
    Some((host, ruta.to_owned()))
}

/// The WAN connection service in the router's description: its type and control URL.
pub(crate) fn servicio(xml: &str) -> Option<(String, String)> {
    for tipo in [
        "urn:schemas-upnp-org:service:WANIPConnection:2",
        "urn:schemas-upnp-org:service:WANIPConnection:1",
        "urn:schemas-upnp-org:service:WANPPPConnection:1",
    ] {
        if let Some(i) = xml.find(tipo) {
            let desde = &xml[i..];
            if let Some(c) = entre(desde, "controlURL") {
                return Some((tipo.to_owned(), c.trim().to_owned()));
            }
        }
    }
    None
}

fn entre(xml: &str, etiqueta: &str) -> Option<String> {
    let abre = format!("<{etiqueta}>");
    let cierra = format!("</{etiqueta}>");
    let i = xml.find(&abre)? + abre.len();
    let j = xml[i..].find(&cierra)? + i;
    Some(xml[i..j].to_owned())
}

fn codigo_error(r: &str) -> String {
    let c = entre(r, "errorCode").unwrap_or_default();
    let d = entre(r, "errorDescription").unwrap_or_default();
    format!("{} {}", c.trim(), d.trim()).trim().to_owned()
}

fn ip_local_hacia(host: &str) -> Option<Ipv4Addr> {
    let addr = host.to_socket_addrs().ok()?.next()?;
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect(addr).ok()?;
    match s.local_addr().ok()? {
        SocketAddr::V4(v4) => Some(*v4.ip()),
        SocketAddr::V6(_) => None,
    }
}

/// One SOAP call to the router.
fn soap(host: &str, ruta: &str, tipo: &str, accion: &str, args: &str) -> Result<String, ()> {
    let cuerpo = format!(
        "<?xml version=\"1.0\"?><s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body><u:{accion} xmlns:u=\"{tipo}\">{args}</u:{accion}></s:Body></s:Envelope>"
    );
    let cabeceras = [
        ("Content-Type", "text/xml; charset=\"utf-8\"".to_owned()),
        ("SOAPAction", format!("\"{tipo}#{accion}\"")),
    ];
    http(host, "POST", ruta, &cabeceras, &cuerpo)
}

/// A small HTTP/1.1 exchange, enough for a router. Returns the body.
fn http(
    host: &str,
    metodo: &str,
    ruta: &str,
    cabeceras: &[(&str, String)],
    cuerpo: &str,
) -> Result<String, ()> {
    let addr = host.to_socket_addrs().map_err(|_| ())?.next().ok_or(())?;
    let mut s = TcpStream::connect_timeout(&addr, ESPERA).map_err(|_| ())?;
    s.set_read_timeout(Some(ESPERA)).map_err(|_| ())?;
    s.set_write_timeout(Some(ESPERA)).map_err(|_| ())?;
    let mut req = format!("{metodo} {ruta} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nUser-Agent: GUARDIANA\r\n");
    for (k, v) in cabeceras {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str(&format!("Content-Length: {}\r\n\r\n{cuerpo}", cuerpo.len()));
    s.write_all(req.as_bytes()).map_err(|_| ())?;
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        match s.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                out.extend_from_slice(&buf[..n]);
                if out.len() > 512 * 1024 {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let texto = String::from_utf8_lossy(&out).into_owned();
    let i = texto.find("\r\n\r\n").map_or(0, |i| i + 4);
    Ok(texto[i..].to_owned())
}

// ---- the invitation code ----

const ALFABETO: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// `GS-XXXX-XXXX-XX`: the public address and port, 48 bits in a 32-letter alphabet without the
/// letters that look like digits, plus a check letter so a typo is caught.
pub fn codigo(ip: Ipv4Addr, puerto: u16) -> String {
    let mut bits: u64 = 0;
    for b in ip.octets() {
        bits = (bits << 8) | u64::from(b);
    }
    bits = (bits << 16) | u64::from(puerto);
    let mut letras = Vec::new();
    for i in (0..10).rev() {
        letras.push(ALFABETO[((bits >> (i * 5)) & 31) as usize]);
    }
    let suma: u32 = letras.iter().map(|&b| u32::from(b)).sum();
    letras.push(ALFABETO[(suma % 32) as usize]);
    let s: String = letras.iter().map(|&b| b as char).collect();
    format!("GS-{}-{}-{}", &s[..4], &s[4..8], &s[8..])
}

/// Read a code back. Spaces, dashes and case do not matter.
pub fn lee_codigo(texto: &str) -> Option<(Ipv4Addr, u16)> {
    let limpio: String = texto
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    let cuerpo = limpio.strip_prefix("GS").unwrap_or(&limpio);
    if cuerpo.len() != 11 {
        return None;
    }
    let b = cuerpo.as_bytes();
    let suma: u32 = b[..10].iter().map(|&x| u32::from(x)).sum();
    if ALFABETO[(suma % 32) as usize] != b[10] {
        return None;
    }
    let mut bits: u64 = 0;
    for &l in &b[..10] {
        let v = ALFABETO.iter().position(|&a| a == l)? as u64;
        bits = (bits << 5) | v;
    }
    let puerto = (bits & 0xFFFF) as u16;
    let ip = (bits >> 16) as u32;
    Some((Ipv4Addr::from(ip), puerto))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn the_code_goes_round_and_a_typo_is_caught() {
        let ip = Ipv4Addr::new(190, 85, 12, 7);
        let c = codigo(ip, 47300);
        assert!(c.starts_with("GS-") && c.len() == 16, "{c}");
        assert_eq!(lee_codigo(&c), Some((ip, 47300)));
        assert_eq!(
            lee_codigo(&c.to_lowercase().replace('-', " ")),
            Some((ip, 47300))
        );
        let mut mal = c.clone();
        mal.replace_range(4..5, if &c[4..5] == "A" { "B" } else { "A" });
        assert_eq!(lee_codigo(&mal), None);
        assert_eq!(lee_codigo("hola"), None);
        assert!(!c.contains('0') && !c.contains('O') && !c.contains('1') && !c.contains('I'));
    }

    #[test]
    fn ssdp_and_description_are_read_and_shared_addresses_are_known() {
        let r = "HTTP/1.1 200 OK\r\nCACHE-CONTROL: max-age=120\r\nST: urn:schemas-upnp-org:device:InternetGatewayDevice:1\r\nLocation: http://192.168.1.1:5000/rootDesc.xml\r\n\r\n";
        assert_eq!(
            ubicacion_de(r).unwrap(),
            "http://192.168.1.1:5000/rootDesc.xml"
        );
        assert_eq!(
            parte_url("http://192.168.1.1:5000/rootDesc.xml").unwrap(),
            ("192.168.1.1:5000".to_owned(), "/rootDesc.xml".to_owned())
        );
        assert_eq!(
            parte_url("http://router").unwrap(),
            ("router:80".to_owned(), "/".to_owned())
        );
        let xml = "<root><service><serviceType>urn:schemas-upnp-org:service:Layer3Forwarding:1</serviceType><controlURL>/l3f</controlURL></service><service><serviceType>urn:schemas-upnp-org:service:WANIPConnection:1</serviceType><controlURL>/ctl/IPConn</controlURL></service></root>";
        assert_eq!(
            servicio(xml).unwrap(),
            (
                "urn:schemas-upnp-org:service:WANIPConnection:1".to_owned(),
                "/ctl/IPConn".to_owned()
            )
        );
        assert!(compartida(Ipv4Addr::new(100, 64, 3, 9)));
        assert!(compartida(Ipv4Addr::new(100, 127, 255, 1)));
        assert!(!compartida(Ipv4Addr::new(100, 128, 0, 1)));
        assert!(compartida(Ipv4Addr::new(192, 168, 1, 10)));
        assert!(!compartida(Ipv4Addr::new(190, 85, 12, 7)));
        assert_eq!(
            codigo_error("<UPnPError><errorCode>718</errorCode><errorDescription>ConflictInMappingEntry</errorDescription></UPnPError>"),
            "718 ConflictInMappingEntry"
        );
    }
}
