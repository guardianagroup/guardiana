//! Where a server sits and whose network serves it, from the address a DNS answer pointed to
//! (decision 203), looked up in data bundled with the program: nothing is asked of anybody.
//!
//! Two tables made by `build/red-ip.py` from DB-IP «IP to Country Lite» and «IP to ASN Lite»
//! (CC BY 4.0): sorted ranges, each starting where the previous one ends, searched by binary
//! search. The format is written down in that script. A third, `anycast.txt`, says which
//! networks and addresses answer from many places at once, where a country would mislead.
//!
//! What this can say, and what it cannot. The country is the one the base gives for the
//! address: where the server that answered is, approximately. It is not where the data ends up
//! (a front server forwards it on), nor the country of the company that owns the name
//! (`country_of`). For an anycast address not even that holds, and it is said instead.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// IP → country ranges (`build/red-ip.py`).
pub const IP_PAIS: &[u8] = include_bytes!("../data/ip-pais.bin");
/// IP → autonomous system ranges and names (`build/red-ip.py`).
pub const IP_RED: &[u8] = include_bytes!("../data/ip-red.bin");
/// What the two tables are: source, month, licence, hashes (`build/red-ip.py`).
pub const RED_IP_JSON: &str = include_str!("../data/red-ip.json");
/// Networks and addresses their own operator calls anycast, each with its source.
pub const ANYCAST: &str = include_str!("../data/anycast.txt");
/// IP → the place its own operator publishes for it: a cloud region (Amazon, Google Cloud,
/// Azure, Oracle) or the city of a geofeed (DigitalOcean, Akamai/Linode, Vultr). `build/geo.py`.
pub const NUBE_IP: &[u8] = include_bytes!("../data/nube-ip.bin");
/// The places `NUBE_IP` points to (`build/geo.py`).
pub const LUGARES_JSON: &str = include_str!("../data/lugares.json");
/// What those two are: sources, dates, licences, hashes (`build/geo.py`).
pub const GEO_JSON: &str = include_str!("../data/geo.json");

/// A place an operator publishes for its addresses (owner, 13 Oct 2026: «saber la ciudad o el
/// estado a dónde van… lo máximo que podáis de precisión»). Not a guess from a geolocation base:
/// the region of a cloud as the cloud names it, or the city its geofeed declares.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lugar {
    /// `aws`, `gcp`, `azure`, `oracle`, `do`, `linode`, `vultr`.
    pub proveedor: String,
    /// The region's code (`us-east-1`) or, for a geofeed, `ciudad|país`.
    pub region: String,
    /// The place in words (`Virginia del Norte (Ashburn)`, `Frankfurt am Main, Hesse`).
    pub ciudad: String,
    /// Two-letter country.
    pub pais: String,
    /// Latitude of that city, rounded to 0.1° (regions) or as GeoNames gives it (geofeeds).
    pub lat: f64,
    /// Longitude, likewise.
    pub lon: f64,
    /// `región del operador` or `geofeed del operador`.
    pub fuente: String,
}

impl Lugar {
    /// The stable key a measurement keeps: `aws:us-east-1`.
    #[must_use]
    pub fn clave(&self) -> String {
        format!("{}:{}", self.proveedor, self.region)
    }
}

fn lugares() -> &'static [Lugar] {
    static L: OnceLock<Vec<Lugar>> = OnceLock::new();
    L.get_or_init(|| serde_json::from_str(LUGARES_JSON).unwrap_or_default())
}

fn tabla_nube() -> Option<&'static Tabla> {
    static T: OnceLock<Option<Tabla>> = OnceLock::new();
    T.get_or_init(|| {
        let mut l = Lector {
            datos: NUBE_IP,
            pos: 0,
        };
        if l.bytes(8)? != b"GNUBE001" {
            return None;
        }
        l.rangos(2)
    })
    .as_ref()
}

/// The place the operator of this address publishes for it, when it does.
#[must_use]
pub fn lugar_de(ip: IpAddr) -> Option<&'static Lugar> {
    let ip = normal(ip);
    if es_local(ip) {
        return None;
    }
    let i = tabla_nube()?.buscar(ip);
    lugares().get(i.checked_sub(1)?)
}

/// The place a measurement kept, by its key (`aws:us-east-1`).
#[must_use]
pub fn lugar_por_clave(clave: &str) -> Option<&'static Lugar> {
    static M: OnceLock<std::collections::HashMap<String, usize>> = OnceLock::new();
    let m = M.get_or_init(|| {
        lugares()
            .iter()
            .enumerate()
            .map(|(i, l)| (l.clave(), i))
            .collect()
    });
    lugares().get(*m.get(clave)?)
}

/// A network (autonomous system): its number and the organisation name the base gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Red {
    /// The autonomous system number (`32934`).
    pub asn: u32,
    /// The organisation the base names for it (`Facebook, Inc.`), possibly empty.
    pub nombre: &'static str,
}

/// What the bundled data says about one server address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub struct Servidor {
    /// Two-letter country the base gives the address. `None` for local and reserved addresses,
    /// for anycast ones (a single country would be wrong there) and where the base has nothing.
    pub pais: Option<&'static str>,
    /// The network that announces the address, when the base knows it.
    pub red: Option<Red>,
    /// The address is anycast by its operator's own word (`anycast.txt`).
    pub anycast: bool,
    /// A private, local or reserved address (`192.168.x.x`, `fe80::`, `127.0.0.1`…): it is not
    /// on the internet and has no country.
    pub local: bool,
}

/// Where the tables come from, as `red-ip.json` says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FuenteRed {
    /// `DB-IP Lite`.
    pub fuente: String,
    /// Month of the base, `AAAA-MM`.
    pub fecha: String,
    /// `CC BY 4.0`.
    pub licencia: String,
    /// The licence's text.
    pub licencia_url: String,
    /// The credit line the licence asks for.
    pub atribucion: String,
    /// Where the credit links to.
    pub web: String,
    /// A hand-made test sample, not the real base: the panel says so wherever it shows a country.
    pub muestra: bool,
}

/// The source of the bundled tables. A manifest that does not parse reads as a sample, so the
/// panel never presents unknown data as DB-IP's.
#[must_use]
pub fn fuente_red() -> &'static FuenteRed {
    static F: OnceLock<FuenteRed> = OnceLock::new();
    F.get_or_init(|| {
        serde_json::from_str(RED_IP_JSON).unwrap_or(FuenteRed {
            muestra: true,
            ..FuenteRed::default()
        })
    })
}

/// One of the two tables, as slices of the embedded bytes.
struct Tabla {
    starts4: &'static [u8],
    vals4: &'static [u8],
    starts6: &'static [u8],
    vals6: &'static [u8],
    /// Bytes per value: 1 for a country index, 3 for a network index.
    ancho: usize,
}

struct Lector {
    datos: &'static [u8],
    pos: usize,
}

impl Lector {
    fn bytes(&mut self, n: usize) -> Option<&'static [u8]> {
        let fin = self.pos.checked_add(n)?;
        let s = self.datos.get(self.pos..fin)?;
        self.pos = fin;
        Some(s)
    }

    fn u32(&mut self) -> Option<u32> {
        let b = self.bytes(4)?;
        Some(u32::from_le_bytes(b.try_into().ok()?))
    }

    fn n(&mut self) -> Option<usize> {
        usize::try_from(self.u32()?).ok()
    }

    fn rangos(&mut self, ancho: usize) -> Option<Tabla> {
        let n4 = self.n()?;
        let starts4 = self.bytes(n4.checked_mul(4)?)?;
        let vals4 = self.bytes(n4.checked_mul(ancho)?)?;
        let n6 = self.n()?;
        let starts6 = self.bytes(n6.checked_mul(8)?)?;
        let vals6 = self.bytes(n6.checked_mul(ancho)?)?;
        Some(Tabla {
            starts4,
            vals4,
            starts6,
            vals6,
            ancho,
        })
    }
}

impl Tabla {
    /// Index of the range holding `key`: the last start not above it.
    fn rango(starts: &[u8], ancho: usize, key: u64) -> Option<usize> {
        let n = starts.len() / ancho;
        let at = |i: usize| -> Option<u64> {
            let b = starts.get(i * ancho..(i + 1) * ancho)?;
            Some(match ancho {
                4 => u64::from(u32::from_le_bytes(b.try_into().ok()?)),
                _ => u64::from_le_bytes(b.try_into().ok()?),
            })
        };
        let (mut lo, mut hi) = (0usize, n);
        // Invariant: every start before `lo` is <= key, every start from `hi` on is > key.
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if at(mid)? <= key {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo.checked_sub(1)
    }

    fn valor(vals: &[u8], ancho: usize, i: usize) -> Option<usize> {
        let b = vals.get(i * ancho..(i + 1) * ancho)?;
        let mut v = [0u8; 4];
        v.get_mut(..ancho)?.copy_from_slice(b);
        usize::try_from(u32::from_le_bytes(v)).ok()
    }

    /// The stored value for an address; 0 means "no data".
    fn buscar(&self, ip: IpAddr) -> usize {
        let r = match ip {
            IpAddr::V4(a) => Self::rango(self.starts4, 4, u64::from(u32::from(a)))
                .and_then(|i| Self::valor(self.vals4, self.ancho, i)),
            IpAddr::V6(a) => {
                let alto = u64::try_from(u128::from(a) >> 64).unwrap_or(u64::MAX);
                Self::rango(self.starts6, 8, alto)
                    .and_then(|i| Self::valor(self.vals6, self.ancho, i))
            }
        };
        r.unwrap_or(0)
    }
}

struct Paises {
    codigos: Vec<&'static str>,
    tabla: Tabla,
}

struct Redes {
    asns: &'static [u8],
    offsets: &'static [u8],
    nombres: &'static [u8],
    tabla: Tabla,
}

fn paises() -> Option<&'static Paises> {
    static P: OnceLock<Option<Paises>> = OnceLock::new();
    P.get_or_init(|| {
        let mut l = Lector {
            datos: IP_PAIS,
            pos: 0,
        };
        if l.bytes(8)? != b"GIPPAIS1" {
            return None;
        }
        let n = l.n()?;
        let codigos = l
            .bytes(n.checked_mul(2)?)?
            .chunks(2)
            .map(|c| std::str::from_utf8(c).unwrap_or("--"))
            .collect();
        let tabla = l.rangos(1)?;
        Some(Paises { codigos, tabla })
    })
    .as_ref()
}

fn redes() -> Option<&'static Redes> {
    static R: OnceLock<Option<Redes>> = OnceLock::new();
    R.get_or_init(|| {
        let mut l = Lector {
            datos: IP_RED,
            pos: 0,
        };
        if l.bytes(8)? != b"GIPRED01" {
            return None;
        }
        let n = l.n()?;
        let asns = l.bytes(n.checked_mul(4)?)?;
        let offsets = l.bytes(n.checked_add(1)?.checked_mul(4)?)?;
        let largo = l.n()?;
        let nombres = l.bytes(largo)?;
        let tabla = l.rangos(3)?;
        Some(Redes {
            asns,
            offsets,
            nombres,
            tabla,
        })
    })
    .as_ref()
}

fn u32_en(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        b.get(i * 4..i * 4 + 4)?.try_into().ok()?,
    ))
}

impl Redes {
    fn red(&self, indice: usize) -> Option<Red> {
        let i = indice.checked_sub(1)?;
        let asn = u32_en(self.asns, i)?;
        let desde = usize::try_from(u32_en(self.offsets, i)?).ok()?;
        let hasta = usize::try_from(u32_en(self.offsets, i + 1)?).ok()?;
        let nombre = std::str::from_utf8(self.nombres.get(desde..hasta)?).unwrap_or("");
        Some(Red { asn, nombre })
    }
}

/// A prefix of `anycast.txt`.
#[derive(Debug, Clone, Copy)]
struct Prefijo {
    red: IpAddr,
    bits: u8,
}

impl Prefijo {
    fn contiene(&self, ip: IpAddr) -> bool {
        match (self.red, ip) {
            (IpAddr::V4(r), IpAddr::V4(a)) => {
                let m = u32::MAX.checked_shl(32 - u32::from(self.bits)).unwrap_or(0);
                u32::from(r) & m == u32::from(a) & m
            }
            (IpAddr::V6(r), IpAddr::V6(a)) => {
                let m = u128::MAX
                    .checked_shl(128 - u32::from(self.bits))
                    .unwrap_or(0);
                u128::from(r) & m == u128::from(a) & m
            }
            _ => false,
        }
    }
}

struct Anycast {
    asns: Vec<u32>,
    prefijos: Vec<Prefijo>,
}

fn anycast() -> &'static Anycast {
    static A: OnceLock<Anycast> = OnceLock::new();
    A.get_or_init(|| {
        let mut asns = Vec::new();
        let mut prefijos = Vec::new();
        for linea in ANYCAST.lines() {
            let linea = linea.split('#').next().unwrap_or("").trim();
            if let Some(n) = linea.strip_prefix("AS").and_then(|n| n.parse().ok()) {
                asns.push(n);
            } else if let Some((red, bits)) = linea.split_once('/') {
                let (Ok(red), Ok(bits)) = (red.parse::<IpAddr>(), bits.parse::<u8>()) else {
                    continue;
                };
                let max = if red.is_ipv4() { 32 } else { 128 };
                if bits <= max {
                    prefijos.push(Prefijo { red, bits });
                }
            }
        }
        Anycast { asns, prefijos }
    })
}

/// The address a lookup should use: an IPv4 address written as IPv6 (`::ffff:1.2.3.4`, or the
/// NAT64 well-known prefix `64:ff9b::1.2.3.4`) is that IPv4 address.
fn normal(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(a) => {
            if let Some(v4) = a.to_ipv4_mapped() {
                return IpAddr::V4(v4);
            }
            let s = a.segments();
            if s[..6] == [0x64, 0xff9b, 0, 0, 0, 0] {
                let o = a.octets();
                return IpAddr::V4(Ipv4Addr::new(o[12], o[13], o[14], o[15]));
            }
            ip
        }
        IpAddr::V4(_) => ip,
    }
}

fn v4_local(a: Ipv4Addr) -> bool {
    let o = a.octets();
    a.is_private()
        || a.is_loopback()
        || a.is_link_local()
        || a.is_unspecified()
        || a.is_broadcast()
        || a.is_multicast()
        || a.is_documentation()
        || o[0] == 0
        // Shared address space for carrier-grade NAT (RFC 6598): 100.64.0.0/10.
        || (o[0] == 100 && (o[1] & 0xc0) == 64)
        // IETF protocol assignments (192.0.0.0/24) and benchmarking (198.18.0.0/15).
        || (o[0] == 192 && o[1] == 0 && o[2] == 0)
        || (o[0] == 198 && (o[1] & 0xfe) == 18)
        // Reserved for future use: 240.0.0.0/4.
        || o[0] >= 240
}

fn v6_local(a: Ipv6Addr) -> bool {
    let s = a.segments();
    a.is_loopback()
        || a.is_unspecified()
        || a.is_multicast()
        // Unique local fc00::/7 and link-local fe80::/10.
        || (s[0] & 0xfe00) == 0xfc00
        || (s[0] & 0xffc0) == 0xfe80
        // Documentation 2001:db8::/32 and 3fff::/20 (RFC 9637).
        || (s[0] == 0x2001 && s[1] == 0x0db8)
        || (s[0] & 0xfff0) == 0x3ff0
}

/// Whether an address is private, local or reserved: not a server on the internet.
#[must_use]
pub fn es_local(ip: IpAddr) -> bool {
    match normal(ip) {
        IpAddr::V4(a) => v4_local(a),
        IpAddr::V6(a) => v6_local(a),
    }
}

/// What the bundled data says about a server address. Local and reserved addresses get nothing
/// but `local`; anycast ones get their network and no country.
#[must_use]
pub fn servidor_de(ip: IpAddr) -> Servidor {
    let ip = normal(ip);
    if es_local(ip) {
        return Servidor {
            local: true,
            ..Servidor::default()
        };
    }
    let red = redes().and_then(|r| r.red(r.tabla.buscar(ip)));
    let any = anycast();
    let anycast = any.prefijos.iter().any(|p| p.contiene(ip))
        || red.is_some_and(|r| any.asns.contains(&r.asn));
    let pais = if anycast {
        None
    } else {
        paises().and_then(|p| p.codigos.get(p.tabla.buscar(ip)).copied())
    }
    .filter(|c| *c != "--");
    Servidor {
        pais,
        red,
        anycast,
        local: false,
    }
}

/// The name the extract stores for the bundled base: `dbip-2026-10`, or `muestra-2026-10` while
/// the tables are the hand-made test sample, so a row never passes for DB-IP's when it was not.
#[must_use]
pub fn base_de_red() -> String {
    let f = fuente_red();
    let fecha = if f.fecha.is_empty() {
        "?"
    } else {
        f.fecha.as_str()
    };
    if f.muestra {
        format!("muestra-{fecha}")
    } else {
        format!("dbip-{fecha}")
    }
}

/// What the extract keeps about the server that answered (decision 203): kind, country,
/// network and base, looked up now. The address goes no further than this call.
#[must_use]
pub fn server_info_de(ip: IpAddr) -> guardiana_core::ServerInfo {
    let s = servidor_de(ip);
    guardiana_core::ServerInfo {
        kind: if s.local {
            guardiana_core::ServerKind::Local
        } else if s.anycast {
            guardiana_core::ServerKind::Anycast
        } else {
            guardiana_core::ServerKind::Publico
        },
        country: s.pais.unwrap_or("").to_owned(),
        asn: s.red.map_or(0, |r| r.asn),
        base: base_de_red(),
    }
}

/// The organisation name the bundled base gives a network, when it knows the number.
#[must_use]
pub fn nombre_de_red(asn: u32) -> Option<&'static str> {
    let r = redes()?;
    let n = r.asns.len() / 4;
    let (mut lo, mut hi) = (0usize, n);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let v = u32_en(r.asns, mid)?;
        match v.cmp(&asn) {
            std::cmp::Ordering::Less => lo = mid + 1,
            std::cmp::Ordering::Greater => hi = mid,
            std::cmp::Ordering::Equal => {
                return r
                    .red(mid + 1)
                    .map(|x| x.nombre)
                    .filter(|x| !x.trim().is_empty())
            }
        }
    }
    None
}

/// [`servidor_de`] for an address written as text (tests, tools); `None` when it is empty or not an
/// address.
#[must_use]
pub fn servidor_de_texto(ip: &str) -> Option<Servidor> {
    ip.trim().parse::<IpAddr>().ok().map(servidor_de)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // These run against whatever `crates/lists/data` holds. The cases are chosen so they hold for
    // the hand-made sample (build/red-ip-muestra) and, the ones marked, for the real base too.

    fn s(ip: &str) -> Servidor {
        servidor_de_texto(ip).unwrap()
    }

    #[test]
    fn the_bundled_tables_parse() {
        assert!(paises().is_some(), "ip-pais.bin no se lee");
        assert!(redes().is_some(), "ip-red.bin no se lee");
        let f = fuente_red();
        assert_eq!(f.fuente, "DB-IP Lite");
        assert_eq!(f.licencia, "CC BY 4.0");
        assert_eq!(f.fecha.len(), 7, "{f:?}");
        // The manifest describes these very files: the hashes build/red-ip.py wrote for them.
        let m: serde_json::Value = serde_json::from_str(RED_IP_JSON).unwrap();
        assert_eq!(
            m["pais"]["sha256"].as_str(),
            Some(guardiana_core::Hash::of(IP_PAIS).to_hex().as_str())
        );
        assert_eq!(
            m["red"]["sha256"].as_str(),
            Some(guardiana_core::Hash::of(IP_RED).to_hex().as_str())
        );
    }

    #[test]
    fn private_and_reserved_addresses_have_no_country() {
        for ip in [
            "192.168.1.15",
            "10.0.0.1",
            "172.16.4.4",
            "127.0.0.1",
            "169.254.3.3",
            "100.64.0.1",
            "0.0.0.0",
            "255.255.255.255",
            "224.0.0.251",
            "192.0.2.10",
            "::1",
            "::",
            "fe80::1",
            "fd12:3456::1",
            "ff02::fb",
            "2001:db8::1",
            "::ffff:192.168.1.1",
        ] {
            let v = s(ip);
            assert!(v.local, "{ip}");
            assert_eq!(v.pais, None, "{ip}");
            assert_eq!(v.red, None, "{ip}");
        }
        assert!(!s("100.128.0.1").local, "fuera de 100.64.0.0/10");
        assert!(!s("172.32.0.1").local, "fuera de 172.16.0.0/12");
        assert!(servidor_de_texto("").is_none());
        assert!(servidor_de_texto("no es una ip").is_none());
    }

    #[test]
    fn anycast_addresses_get_no_country_but_keep_their_network() {
        // Google Public DNS is anycast by Google's own FAQ; the rest of Google's network is not
        // marked. True with the sample and with the real base.
        let dns = s("8.8.8.8");
        assert!(dns.anycast);
        assert_eq!(dns.pais, None);
        assert_eq!(dns.red.map(|r| r.asn), Some(15169));
        assert!(!s("8.8.8.9").anycast, "solo las direcciones de su FAQ");
        assert!(s("2001:4860:4860::8888").anycast);
        // Cloudflare's whole network is (AS13335 in `anycast.txt`).
        let cf = s("1.1.1.1");
        assert!(cf.anycast);
        assert_eq!(cf.pais, None);
        assert_eq!(cf.red.map(|r| r.asn), Some(13335));
        assert!(s("2606:4700::6810:84e5").anycast);
        // Quad9, by prefix even with no network data.
        assert!(s("9.9.9.9").anycast);
    }

    #[test]
    fn what_the_extract_keeps_has_no_address() {
        let ip: IpAddr = "8.8.8.8".parse().unwrap();
        let info = server_info_de(ip);
        assert_eq!(info.kind, guardiana_core::ServerKind::Anycast);
        assert_eq!(info.asn, 15169);
        assert!(info.country.is_empty());
        assert!(
            !info.hash_text().contains("8.8.8.8"),
            "{}",
            info.hash_text()
        );
        assert!(info.base.ends_with(&fuente_red().fecha));
        assert_eq!(
            info.base.starts_with("muestra-"),
            fuente_red().muestra,
            "a sample never passes for DB-IP"
        );
        let local = server_info_de("192.168.1.1".parse().unwrap());
        assert_eq!(local.kind, guardiana_core::ServerKind::Local);
        assert_eq!(local.asn, 0);
        assert!(nombre_de_red(15169).is_some());
        assert_eq!(nombre_de_red(4_000_000_000), None);
    }

    #[test]
    fn ipv4_ranges_hold_at_both_edges() {
        if !fuente_red().muestra {
            return; // the real base draws its own lines
        }
        let meta = s("157.240.0.0");
        assert_eq!(meta.pais, Some("IE"));
        assert_eq!(meta.red.map(|r| r.asn), Some(32934));
        assert_eq!(meta.red.map(|r| r.nombre), Some("Facebook, Inc."));
        assert_eq!(s("157.240.255.255").pais, Some("IE"));
        // One past the end: a gap in the sample, no data.
        let fuera = s("157.241.0.0");
        assert_eq!(fuera.pais, None);
        assert_eq!(fuera.red, None);
        assert!(!fuera.local);
        // One before the start.
        assert_eq!(s("157.239.255.255").pais, None);
        // A country with no network, and a network-less row the base calls ZZ.
        assert_eq!(s("1.0.0.7").pais, Some("AU"));
        assert_eq!(s("1.0.0.7").red, None);
        assert_eq!(s("23.0.1.1").pais, None);
        // The first and last addresses of the whole space.
        assert_eq!(s("0.0.0.1").pais, None);
        assert_eq!(s("239.255.255.254").pais, None);
        // An IPv4 address written as IPv6 is looked up as IPv4.
        assert_eq!(s("::ffff:157.240.1.35").pais, Some("IE"));
        assert_eq!(s("64:ff9b::157.240.1.35").pais, Some("IE"));
    }

    #[test]
    fn ipv6_ranges_hold_at_both_edges() {
        if !fuente_red().muestra {
            return;
        }
        assert_eq!(s("2a03:2880::").pais, Some("IE"));
        assert_eq!(
            s("2a03:2880:f0ff:ffff:ffff:ffff:ffff:ffff").pais,
            Some("IE")
        );
        // The next /64 changes country while the network stays Meta's.
        let siguiente = s("2a03:2880:f100::1");
        assert_eq!(siguiente.pais, Some("SE"));
        assert_eq!(siguiente.red.map(|r| r.asn), Some(32934));
        assert_eq!(s("2a03:2881::").pais, None);
        assert_eq!(s("2a03:287f:ffff:ffff::").pais, None);
        // A source range smaller than a /64 that does not start a /64 is not kept (the /64
        // keeps what covers its first address).
        assert_eq!(s("2a0b:4d00:0:1::9000").pais, None);
        assert_eq!(s("2a0b:4d00::1").pais, Some("CO"));
        assert_eq!(s("ffff:ffff:ffff:ffff:ffff:ffff:ffff:fffe").pais, None);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod lugar_tests {
    use super::*;

    #[test]
    fn the_places_operators_publish_are_bundled_and_found_by_key() {
        let l = lugar_por_clave("aws:us-east-1").unwrap();
        assert_eq!(l.pais, "US");
        assert!(l.ciudad.contains("Virginia"));
        assert!(lugar_por_clave("gcp:europe-west1").is_some());
        assert!(lugar_por_clave("azure:northeurope").is_some());
        assert!(lugar_por_clave("").is_none());
        // Every place has a country and coordinates on the Earth.
        for l in lugares() {
            assert_eq!(l.pais.len(), 2, "{l:?}");
            assert!(l.lat.abs() <= 90.0 && l.lon.abs() <= 180.0, "{l:?}");
        }
        // A private address has no place.
        assert!(lugar_de("192.168.1.1".parse().unwrap()).is_none());
    }
}
