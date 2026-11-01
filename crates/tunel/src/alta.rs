//! The anonymous sign-up with a GUARDIANA VPN server (version 1 of the exchange; docs/VPN.md).
//!
//! The computer sends [`Pide`]: its WireGuard public key and a fresh ML-KEM-768 encapsulation key. The server answers
//! [`Respuesta`]: the tunnel address it gives this key, its own WireGuard public key, where to reach it, the ML-KEM
//! ciphertext and the resolver inside the tunnel. Nothing identifies the person: no account, no name, and the server
//! keeps the key only while the tunnel is in use.
//!
//! Everything the server says is checked before it reaches the adapter: the address and the resolver must be private
//! addresses (an answer cannot send the person's traffic or names to a public address), and the server must be a
//! GUARDIANA name.

use std::net::Ipv4Addr;

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::cuantico::{psk, Oferta};
use crate::llaves::{publica_de_base64, Llaves};
use crate::{b64, de_b64, Error};

/// The version of the exchange this program speaks.
pub const VERSION: u8 = 1;
/// The only domain a tunnel server may be under.
pub const DOMINIO: &str = ".guardianagroup.com";

/// What the computer sends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pide {
    /// Version of the exchange.
    pub v: u8,
    /// The computer's WireGuard public key, base64.
    pub llave: String,
    /// A fresh ML-KEM-768 encapsulation key, base64.
    pub kem: String,
}

/// What the server answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Respuesta {
    /// Version of the exchange.
    pub v: u8,
    /// The computer's address inside the tunnel, `10.x.y.z/32`.
    pub direccion: String,
    /// The server's WireGuard public key, base64.
    pub servidor: String,
    /// Where to reach the server: `motor.guardianagroup.com:51820`.
    pub punto: String,
    /// The ML-KEM-768 ciphertext, base64.
    pub kem: String,
    /// The resolver inside the tunnel (a private address).
    pub dns: String,
    /// Until when the server keeps this key without traffic (milliseconds since 1970).
    pub caduca_ms: u64,
}

/// A checked sign-up, ready for the configuration.
#[derive(Debug)]
pub struct Alta {
    /// The computer's address inside the tunnel.
    pub direccion: Ipv4Addr,
    /// The server's WireGuard public key.
    pub servidor: [u8; 32],
    /// The server's name.
    pub host: String,
    /// The server's port.
    pub puerto: u16,
    /// The resolver inside the tunnel.
    pub dns: Ipv4Addr,
    /// The post-quantum preshared key; wiped from memory when dropped.
    pub psk: Zeroizing<[u8; 32]>,
    /// Until when the server keeps the key without traffic.
    pub caduca_ms: u64,
}

/// The sign-up to send, and the ML-KEM half to keep until the answer arrives.
#[must_use]
pub fn pide(llaves: &Llaves) -> (Oferta, Pide) {
    let (oferta, ek) = Oferta::nueva();
    (
        oferta,
        Pide {
            v: VERSION,
            llave: llaves.publica_base64(),
            kem: b64(&ek),
        },
    )
}

/// Checks the server's answer and derives the preshared key.
pub fn completa(llaves: &Llaves, oferta: &Oferta, r: &Respuesta) -> Result<Alta, Error> {
    if r.v != VERSION {
        return Err(Error::Formato("versión del servidor"));
    }
    let direccion = r
        .direccion
        .strip_suffix("/32")
        .and_then(|d| d.parse::<Ipv4Addr>().ok())
        .filter(privada)
        .ok_or(Error::Formato("dirección del túnel"))?;
    let dns = r
        .dns
        .parse::<Ipv4Addr>()
        .ok()
        .filter(privada)
        .ok_or(Error::Formato("resolutor del túnel"))?;
    let servidor = publica_de_base64(&r.servidor)?;
    let (host, puerto) = r.punto.rsplit_once(':').ok_or(Error::Formato("servidor"))?;
    let puerto: u16 = puerto
        .parse()
        .ok()
        .filter(|p| *p > 0)
        .ok_or(Error::Formato("puerto del servidor"))?;
    let host = host.to_ascii_lowercase();
    let nombre_valido = host.ends_with(DOMINIO)
        && host.len() > DOMINIO.len()
        && host
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'.' || c == b'-');
    if !nombre_valido {
        return Err(Error::Formato("servidor"));
    }
    let ct = de_b64(&r.kem).ok_or(Error::Formato("cifrado ML-KEM"))?;
    let secreto = Zeroizing::new(oferta.abre(&ct)?);
    let psk = Zeroizing::new(psk(&secreto, &llaves.publica(), &servidor)?);
    Ok(Alta {
        direccion,
        servidor,
        host,
        puerto,
        dns,
        psk,
        caduca_ms: r.caduca_ms,
    })
}

/// Private addresses only (RFC 1918 and the shared range of RFC 6598).
fn privada(ip: &Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    ip.is_private() || (a == 100 && (64..128).contains(&b))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::cuantico::encapsula;

    /// What a GUARDIANA server does with a sign-up (here, to test the computer's side).
    fn servidor(p: &Pide, llaves_servidor: &Llaves) -> (Respuesta, [u8; 32]) {
        let ek = de_b64(&p.kem).unwrap();
        let (ct, ss) = encapsula(&ek).unwrap();
        let cliente = publica_de_base64(&p.llave).unwrap();
        let k = psk(&ss, &cliente, &llaves_servidor.publica()).unwrap();
        (
            Respuesta {
                v: 1,
                direccion: "10.66.3.7/32".into(),
                servidor: llaves_servidor.publica_base64(),
                punto: "motor-hel.guardianagroup.com:51820".into(),
                kem: b64(&ct),
                dns: "10.66.0.1".into(),
                caduca_ms: 99,
            },
            k,
        )
    }

    #[test]
    fn un_alta_completa_y_la_misma_llave_a_los_dos_lados() {
        let (equipo, srv) = (Llaves::nuevas().unwrap(), Llaves::nuevas().unwrap());
        let (oferta, p) = pide(&equipo);
        let (r, k_servidor) = servidor(&p, &srv);
        let a = completa(&equipo, &oferta, &r).unwrap();
        assert_eq!(*a.psk, k_servidor);
        assert_eq!(a.direccion, Ipv4Addr::new(10, 66, 3, 7));
        assert_eq!(
            (a.host.as_str(), a.puerto),
            ("motor-hel.guardianagroup.com", 51820)
        );
        assert_eq!(a.servidor, srv.publica());
    }

    #[test]
    fn una_respuesta_que_desvia_el_trafico_no_vale() {
        let (equipo, srv) = (Llaves::nuevas().unwrap(), Llaves::nuevas().unwrap());
        let (oferta, p) = pide(&equipo);
        let (bien, _) = servidor(&p, &srv);
        let cambia = |f: &dyn Fn(&mut Respuesta)| {
            let mut r = bien.clone();
            f(&mut r);
            completa(&equipo, &oferta, &r).is_err()
        };
        assert!(cambia(&|r| r.direccion = "8.8.8.8/32".into()));
        assert!(cambia(&|r| r.direccion = "10.66.3.7/8".into()));
        assert!(cambia(&|r| r.dns = "1.1.1.1".into()));
        assert!(cambia(&|r| r.punto = "evil.example.com:51820".into()));
        assert!(cambia(
            &|r| r.punto = "guardianagroup.com.evil.net:51820".into()
        ));
        assert!(cambia(&|r| r.punto = ".guardianagroup.com:51820".into()));
        assert!(cambia(&|r| r.punto = "motor.guardianagroup.com:0".into()));
        assert!(cambia(&|r| r.servidor = "AAAA".into()));
        assert!(cambia(&|r| r.kem = "no".into()));
        assert!(cambia(&|r| r.v = 2));
        assert!(!cambia(&|r| r.direccion = "100.64.0.9/32".into()));
    }

    #[test]
    fn lo_que_se_envia_no_lleva_nada_secreto() {
        let equipo = Llaves::nuevas().unwrap();
        let (_, p) = pide(&equipo);
        let json = serde_json::to_string(&p).unwrap();
        assert!(!json.contains(equipo.secreta_base64().as_str()));
        assert_eq!(de_b64(&p.kem).unwrap().len(), crate::cuantico::LLAVE_BYTES);
    }
}
