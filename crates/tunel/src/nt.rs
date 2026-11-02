//! The configuration WireGuardNT takes (`WireGuardSetConfiguration` in `wireguard.h`, the Windows driver of WireGuard;
//! its signed `wireguard.dll` is shipped as downloaded, see docs/VPN.md): one interface, followed by its peers, each
//! followed by its allowed IPs, as bytes laid out like the C structures (each aligned to 8 bytes, little-endian, the
//! ports and addresses in network order). Pure: no Windows call here, so it is tested on every system; the service
//! hands these bytes to the driver.
//!
//! Layout, from `wireguard.h` (checked against the header on 1 Nov 2026):
//! - `WIREGUARD_INTERFACE`, 80 bytes: `Flags` u32 @0, `ListenPort` u16 @4, `PrivateKey` [32] @6, `PublicKey` [32] @38,
//!   `PeersCount` u32 @72.
//! - `WIREGUARD_PEER`, 136 bytes: `Flags` u32 @0, `Reserved` u32 @4 (zero), `PublicKey` [32] @8, `PresharedKey` [32]
//!   @40, `PersistentKeepalive` u16 @72, `Endpoint` (`SOCKADDR_INET`, 28) @76, `TxBytes` u64 @104, `RxBytes` u64 @112,
//!   `LastHandshake` u64 @120, `AllowedIPsCount` u32 @128.
//! - `WIREGUARD_ALLOWED_IP`, 24 bytes: `Address` (`IN_ADDR` or `IN6_ADDR`, 16) @0, `AddressFamily` u16 @16, `Cidr` u8
//!   @18, `Flags` u32 @20 (zero: add, not remove).

use std::net::{IpAddr, SocketAddr};

use zeroize::Zeroizing;

/// Size of `WIREGUARD_INTERFACE`.
pub const TAM_INTERFAZ: usize = 80;
/// Size of `WIREGUARD_PEER`.
pub const TAM_PAR: usize = 136;
/// Size of `WIREGUARD_ALLOWED_IP`.
pub const TAM_IP: usize = 24;

/// Windows' `AF_INET`.
const AF_INET: u16 = 2;
/// Windows' `AF_INET6`.
const AF_INET6: u16 = 23;

const INTERFAZ_TIENE_PRIVADA: u32 = 1 << 1;
const INTERFAZ_TIENE_PUERTO: u32 = 1 << 2;
const INTERFAZ_REEMPLAZA_PARES: u32 = 1 << 3;
const PAR_TIENE_PUBLICA: u32 = 1 << 0;
const PAR_TIENE_PSK: u32 = 1 << 1;
const PAR_TIENE_KEEPALIVE: u32 = 1 << 2;
const PAR_TIENE_EXTREMO: u32 = 1 << 3;
const PAR_REEMPLAZA_IPS: u32 = 1 << 5;

/// One peer: the server.
#[derive(Clone)]
pub struct Par {
    /// Its public key.
    pub publica: [u8; 32],
    /// The preshared key mixed into the handshake (the post-quantum half), when there is one.
    pub psk: Option<Zeroizing<[u8; 32]>>,
    /// Where it answers.
    pub extremo: Option<SocketAddr>,
    /// Seconds between keepalives (0: none).
    pub keepalive: u16,
    /// What goes to it: address and prefix length.
    pub permitidas: Vec<(IpAddr, u8)>,
}

fn pon(buf: &mut [u8], en: usize, datos: &[u8]) {
    if let Some(d) = buf.get_mut(en..en + datos.len()) {
        d.copy_from_slice(datos);
    }
}

/// `SOCKADDR_INET` for an address and port: the family (little-endian), the port and the address in network order.
fn sockaddr(extremo: &SocketAddr) -> [u8; 28] {
    let mut s = [0u8; 28];
    match extremo {
        SocketAddr::V4(a) => {
            pon(&mut s, 0, &AF_INET.to_le_bytes());
            pon(&mut s, 2, &a.port().to_be_bytes());
            pon(&mut s, 4, &a.ip().octets());
        }
        SocketAddr::V6(a) => {
            pon(&mut s, 0, &AF_INET6.to_le_bytes());
            pon(&mut s, 2, &a.port().to_be_bytes());
            pon(&mut s, 4, &a.flowinfo().to_be_bytes());
            pon(&mut s, 8, &a.ip().octets());
            pon(&mut s, 24, &a.scope_id().to_le_bytes());
        }
    }
    s
}

/// The whole configuration: this computer's private key, a listen port (`None`: Windows picks one) and the peers.
/// Every peer is replaced, and every peer's allowed IPs too: what is set is exactly this. The bytes hold the private
/// key and are wiped when dropped.
#[must_use]
pub fn configuracion(privada: &[u8; 32], puerto: Option<u16>, pares: &[Par]) -> Zeroizing<Vec<u8>> {
    let total = TAM_INTERFAZ
        + pares
            .iter()
            .map(|p| TAM_PAR + TAM_IP * p.permitidas.len())
            .sum::<usize>();
    let mut b = Zeroizing::new(vec![0u8; total]);
    let mut banderas = INTERFAZ_TIENE_PRIVADA | INTERFAZ_REEMPLAZA_PARES;
    if puerto.is_some() {
        banderas |= INTERFAZ_TIENE_PUERTO;
    }
    pon(&mut b, 0, &banderas.to_le_bytes());
    pon(&mut b, 4, &puerto.unwrap_or(0).to_le_bytes());
    pon(&mut b, 6, privada);
    pon(
        &mut b,
        72,
        &u32::try_from(pares.len()).unwrap_or(0).to_le_bytes(),
    );
    let mut en = TAM_INTERFAZ;
    for p in pares {
        let mut banderas = PAR_TIENE_PUBLICA | PAR_TIENE_KEEPALIVE | PAR_REEMPLAZA_IPS;
        if p.psk.is_some() {
            banderas |= PAR_TIENE_PSK;
        }
        if p.extremo.is_some() {
            banderas |= PAR_TIENE_EXTREMO;
        }
        pon(&mut b, en, &banderas.to_le_bytes());
        pon(&mut b, en + 8, &p.publica);
        if let Some(psk) = &p.psk {
            pon(&mut b, en + 40, psk.as_slice());
        }
        pon(&mut b, en + 72, &p.keepalive.to_le_bytes());
        if let Some(e) = &p.extremo {
            pon(&mut b, en + 76, &sockaddr(e));
        }
        pon(
            &mut b,
            en + 128,
            &u32::try_from(p.permitidas.len()).unwrap_or(0).to_le_bytes(),
        );
        en += TAM_PAR;
        for (ip, cidr) in &p.permitidas {
            match ip {
                IpAddr::V4(a) => {
                    pon(&mut b, en, &a.octets());
                    pon(&mut b, en + 16, &AF_INET.to_le_bytes());
                    pon(&mut b, en + 18, &[(*cidr).min(32)]);
                }
                IpAddr::V6(a) => {
                    pon(&mut b, en, &a.octets());
                    pon(&mut b, en + 16, &AF_INET6.to_le_bytes());
                    pon(&mut b, en + 18, &[(*cidr).min(128)]);
                }
            }
            en += TAM_IP;
        }
    }
    b
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    fn u16_en(b: &[u8], en: usize) -> u16 {
        u16::from_le_bytes([b[en], b[en + 1]])
    }
    fn u32_en(b: &[u8], en: usize) -> u32 {
        u32::from_le_bytes([b[en], b[en + 1], b[en + 2], b[en + 3]])
    }

    #[test]
    fn la_disposicion_de_wireguard_h() {
        let par = Par {
            publica: [7; 32],
            psk: Some(Zeroizing::new([9; 32])),
            extremo: Some("198.51.100.4:51820".parse().unwrap()),
            keepalive: 25,
            permitidas: vec![
                (IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
                (IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
            ],
        };
        let b = configuracion(&[5; 32], None, std::slice::from_ref(&par));
        assert_eq!(b.len(), 80 + 136 + 2 * 24);
        // la interfaz: llave privada y reemplazar los pares; sin puerto (lo elige Windows)
        assert_eq!(u32_en(&b, 0), (1 << 1) | (1 << 3));
        assert_eq!(u16_en(&b, 4), 0);
        assert_eq!(&b[6..38], &[5; 32]);
        assert_eq!(&b[38..70], &[0; 32]);
        assert_eq!(u32_en(&b, 72), 1);
        // el par
        let p = 80;
        assert_eq!(u32_en(&b, p), 1 | 2 | 4 | 8 | 32);
        assert_eq!(u32_en(&b, p + 4), 0);
        assert_eq!(&b[p + 8..p + 40], &[7; 32]);
        assert_eq!(&b[p + 40..p + 72], &[9; 32]);
        assert_eq!(u16_en(&b, p + 72), 25);
        // SOCKADDR_IN: familia 2, puerto 51820 en orden de red, 198.51.100.4
        assert_eq!(u16_en(&b, p + 76), 2);
        assert_eq!(&b[p + 78..p + 80], &51820u16.to_be_bytes());
        assert_eq!(&b[p + 80..p + 84], &[198, 51, 100, 4]);
        assert_eq!(&b[p + 104..p + 128], &[0; 24]);
        assert_eq!(u32_en(&b, p + 128), 2);
        // las IP permitidas
        let ip = p + 136;
        assert_eq!(&b[ip..ip + 16], &[0; 16]);
        assert_eq!(u16_en(&b, ip + 16), 2);
        assert_eq!(b[ip + 18], 0);
        assert_eq!(u32_en(&b, ip + 20), 0);
        assert_eq!(u16_en(&b, ip + 24 + 16), 23);
    }

    #[test]
    fn un_extremo_ipv6_y_un_puerto_fijo() {
        let par = Par {
            publica: [1; 32],
            psk: None,
            extremo: Some("[2001:db8::1]:443".parse().unwrap()),
            keepalive: 0,
            permitidas: vec![("10.66.0.0".parse().unwrap(), 40)],
        };
        let b = configuracion(&[2; 32], Some(51820), &[par]);
        assert_eq!(u32_en(&b, 0), (1 << 1) | (1 << 2) | (1 << 3));
        assert_eq!(u16_en(&b, 4), 51820);
        let p = 80;
        // sin llave precompartida
        assert_eq!(u32_en(&b, p), 1 | 4 | 8 | 32);
        assert_eq!(u16_en(&b, p + 76), 23);
        assert_eq!(&b[p + 78..p + 80], &443u16.to_be_bytes());
        assert_eq!(b[p + 84], 0x20);
        assert_eq!(b[p + 85], 0x01);
        // un prefijo imposible se queda en 32
        assert_eq!(b[p + 136 + 18], 32);
    }
}
