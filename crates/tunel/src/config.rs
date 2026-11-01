//! The tunnel's configuration: everything goes through it (IPv4 and IPv6), names are resolved inside it, and the
//! connection is kept alive behind home routers. The Windows adapter takes it as it is; the INI form is WireGuard's own,
//! for tests and for a person who wants to check it (the private key and the preshared key only in [`Config::a_ini`]).

use std::net::Ipv4Addr;

use zeroize::Zeroizing;

use crate::alta::Alta;
use crate::b64;
use crate::llaves::Llaves;

/// WireGuard's usual MTU over IPv4 and IPv6 (1500 minus its 80 bytes of headers).
pub const MTU: u16 = 1420;
/// Seconds between keep-alive packets, so a home router does not forget the tunnel.
pub const MANTENER_S: u16 = 25;

/// A tunnel ready to go up.
pub struct Config {
    privada: Zeroizing<String>,
    psk: Zeroizing<String>,
    /// The computer's address inside the tunnel.
    pub direccion: Ipv4Addr,
    /// The resolver inside the tunnel.
    pub dns: Ipv4Addr,
    /// The server's public key, base64.
    pub servidor: String,
    /// `host:port` of the server.
    pub punto: String,
    /// What goes through the tunnel: everything.
    pub permitidas: [&'static str; 2],
}

impl Config {
    /// The configuration for this computer's keys and a checked sign-up.
    #[must_use]
    pub fn de(llaves: &Llaves, alta: &Alta) -> Self {
        Self {
            privada: llaves.secreta_base64(),
            psk: Zeroizing::new(b64(alta.psk.as_slice())),
            direccion: alta.direccion,
            dns: alta.dns,
            servidor: b64(&alta.servidor),
            punto: format!("{}:{}", alta.host, alta.puerto),
            permitidas: ["0.0.0.0/0", "::/0"],
        }
    }

    /// WireGuard's INI form, with the secrets; wiped from memory when dropped.
    #[must_use]
    pub fn a_ini(&self) -> Zeroizing<String> {
        Zeroizing::new(format!(
            "[Interface]\nPrivateKey = {}\nAddress = {}/32\nDNS = {}\nMTU = {}\n\n[Peer]\nPublicKey = {}\nPresharedKey = {}\nAllowedIPs = {}\nEndpoint = {}\nPersistentKeepalive = {}\n",
            self.privada.as_str(),
            self.direccion,
            self.dns,
            MTU,
            self.servidor,
            self.psk.as_str(),
            self.permitidas.join(", "),
            self.punto,
            MANTENER_S
        ))
    }

    /// The same without the secrets, for the panel and the logs.
    #[must_use]
    pub fn resumen(&self) -> String {
        format!(
            "túnel {}/32 · servidor {} ({}) · DNS {} · todo por el túnel ({}) · postcuántico (ML-KEM-768)",
            self.direccion,
            self.punto,
            self.servidor,
            self.dns,
            self.permitidas.join(", ")
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn alta() -> Alta {
        Alta {
            direccion: Ipv4Addr::new(10, 66, 3, 7),
            servidor: [3u8; 32],
            host: "motor.guardianagroup.com".into(),
            puerto: 51820,
            dns: Ipv4Addr::new(10, 66, 0, 1),
            psk: Zeroizing::new([9u8; 32]),
            caduca_ms: 0,
        }
    }

    #[test]
    fn la_configuracion_de_wireguard() {
        let ll = Llaves::nuevas().unwrap();
        let c = Config::de(&ll, &alta());
        let ini = c.a_ini();
        assert!(ini.contains(&format!("PrivateKey = {}\n", ll.secreta_base64().as_str())));
        assert!(ini.contains("Address = 10.66.3.7/32\n"));
        assert!(ini.contains("AllowedIPs = 0.0.0.0/0, ::/0\n"));
        assert!(ini.contains("Endpoint = motor.guardianagroup.com:51820\n"));
        assert!(ini.contains(&format!("PresharedKey = {}\n", b64(&[9u8; 32]))));
    }

    #[test]
    fn el_resumen_no_lleva_secretos() {
        let ll = Llaves::nuevas().unwrap();
        let c = Config::de(&ll, &alta());
        let r = c.resumen();
        assert!(!r.contains(ll.secreta_base64().as_str()));
        assert!(!r.contains(&b64(&[9u8; 32])));
        assert!(r.contains("motor.guardianagroup.com:51820"));
    }
}
