//! The VPN on Windows, put together (in preparation; docs/VPN.md): from a checked sign-up with a GUARDIANA server
//! (`guardiana_tunel::alta`) to a tunnel that carries everything, in this order, and undone in the reverse one:
//!
//! 1. WireGuardNT's adapter, «GUARDIANA» (`wireguard_nt`), with its whole configuration: this computer's key, the
//!    server's, the post-quantum preshared key, the server's address and everything allowed (`guardiana_tunel::nt`);
//! 2. its address, its routes, its settings and its resolver (`red_vpn`);
//! 3. the cut-off switch (`corte_vpn`, with `guardiana_tunel::corte`'s plan): nothing leaves outside the tunnel but
//!    the encrypted packets to the server;
//! 4. the adapter up.
//!
//! When anything fails, what was done is undone (dropping the cut-off removes its filters; closing the adapter removes
//! its address and routes). `todo == false` keeps the computer's own routes and adds no cut-off: only the tunnel's own
//! network goes inside, for the real test on a machine whose network must stay as it is.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::Path;
use std::sync::Arc;

use guardiana_tunel::alta::Alta;
use guardiana_tunel::conexion::{self, Abierto, Conectado, ErrorConexion};
use guardiana_tunel::llaves::Llaves;
use guardiana_tunel::{corte, nt};

use crate::cortafuegos::ErrorCorte;
use crate::corte_vpn::CorteVpn;
use crate::red_vpn::{self, ErrorRed};
use crate::wireguard_nt::{Adaptador, Biblioteca, ErrorNt};

/// The adapter's name, and the group Windows lists it under.
pub const NOMBRE: &str = "GUARDIANA";
/// The tunnel's MTU: WireGuard's own default.
pub const MTU: u32 = 1420;
/// Seconds between keepalives, so a home router keeps the way open.
pub const KEEPALIVE: u16 = 25;
/// The prefix of the servers' private network (docs/VPN.md: `10.66.0.0/16`).
const PREFIJO: u8 = 16;

/// Why the tunnel could not be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorTunel {
    /// The adapter.
    Adaptador(ErrorNt),
    /// Its address, routes, settings or resolver.
    Red(ErrorRed),
    /// The cut-off switch.
    Corte(String),
}

impl std::fmt::Display for ErrorTunel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Adaptador(e) => write!(f, "{e}"),
            Self::Red(e) => write!(f, "{e}"),
            Self::Corte(e) => write!(f, "el interruptor de corte: {e}"),
        }
    }
}

impl From<ErrorNt> for ErrorTunel {
    fn from(e: ErrorNt) -> Self {
        Self::Adaptador(e)
    }
}

impl From<ErrorRed> for ErrorTunel {
    fn from(e: ErrorRed) -> Self {
        Self::Red(e)
    }
}

impl From<ErrorCorte> for ErrorTunel {
    fn from(e: ErrorCorte) -> Self {
        Self::Corte(e.to_string())
    }
}

/// The tunnel, open. Its fields drop in this order: the cut-off switch first, then the adapter.
pub struct Tunel {
    corte: Option<CorteVpn>,
    adaptador: Adaptador,
    /// This computer's address in the tunnel now, and the tunnel's resolver (the cut-off switch lets DNS go to it only).
    direccion: Ipv4Addr,
    dns: Ipv4Addr,
    /// Whether everything goes inside (the default routes on the adapter, and the cut-off switch).
    todo: bool,
}

/// The default routes, IPv4 and IPv6, as the tunnel puts them on its adapter.
const POR_DEFECTO_V4: (IpAddr, u8) = (IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0);
const POR_DEFECTO_V6: (IpAddr, u8) = (IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0);

/// Everything goes inside the tunnel: IPv4 and IPv6.
fn todas() -> Vec<(IpAddr, u8)> {
    vec![
        (IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
        (IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
    ]
}

impl Tunel {
    /// Opens the tunnel with WireGuardNT's library at `dll`, the service's program file `servicio` (the only one let
    /// out, for the encrypted packets), this computer's keys, the checked sign-up and the server's address `extremo`
    /// (its name already resolved). With `todo`, everything goes inside and the cut-off switch is on.
    ///
    /// # Errors
    /// [`ErrorTunel`] with the step that failed; everything done before it is undone.
    pub fn abre(
        dll: &Path,
        servicio: &Path,
        llaves: &Llaves,
        alta: &Alta,
        extremo: SocketAddr,
        todo: bool,
    ) -> Result<Self, ErrorTunel> {
        let biblioteca = Arc::new(Biblioteca::carga(dll)?);
        let adaptador = Adaptador::crea(&biblioteca, NOMBRE, NOMBRE)?;
        let par = nt::Par {
            publica: alta.servidor,
            psk: Some(alta.psk.clone()),
            extremo: Some(extremo),
            keepalive: KEEPALIVE,
            permitidas: todas(),
        };
        adaptador.configura(&nt::configuracion(&llaves.secreta_bytes(), None, &[par]))?;
        let luid = adaptador.luid();
        red_vpn::pon_direccion(luid, &IpAddr::V4(alta.direccion), PREFIJO)?;
        if todo {
            red_vpn::pon_rutas(luid, &[POR_DEFECTO_V4])?;
            red_vpn::ajusta(luid, false, Some(MTU), true)?;
            // IPv6 goes inside too when the adapter has it; when it does not, the cut-off switch keeps it from
            // leaving outside the tunnel anyway
            let _ = red_vpn::pon_rutas(luid, &[POR_DEFECTO_V6]);
            let _ = red_vpn::ajusta(luid, true, Some(MTU), true);
        } else {
            let [a, b, _, _] = alta.direccion.octets();
            red_vpn::pon_rutas(luid, &[(IpAddr::V4(Ipv4Addr::new(a, b, 0, 0)), PREFIJO)])?;
            red_vpn::ajusta(luid, false, Some(MTU), false)?;
        }
        red_vpn::pon_dns(NOMBRE, &[IpAddr::V4(alta.dns)])?;
        let corte = if todo {
            Some(CorteVpn::activa(
                &corte::plan(luid, &[IpAddr::V4(alta.dns)]),
                servicio,
            )?)
        } else {
            None
        };
        adaptador.pon(true)?;
        Ok(Self {
            corte,
            adaptador,
            direccion: alta.direccion,
            dns: alta.dns,
            todo,
        })
    }

    /// Steps aside for a new sign-up (`guardiana_tunel::conexion::renueva`): the default routes off the adapter, so this
    /// program's request to the server goes by the computer's own network. The cut-off switch stays on: no other program
    /// can send anything outside the tunnel meanwhile, and DNS still goes only to the tunnel's resolver. Without the
    /// default routes (the tunnel of the real test), nothing to do.
    ///
    /// # Errors
    /// [`ErrorTunel`] when Windows refuses.
    pub fn aparta(&mut self) -> Result<(), ErrorTunel> {
        if self.todo {
            let luid = self.adaptador.luid();
            red_vpn::quita_rutas(luid, &[POR_DEFECTO_V4])?;
            let _ = red_vpn::quita_rutas(luid, &[POR_DEFECTO_V6]);
        }
        Ok(())
    }

    /// The default routes back on the adapter, after [`Tunel::aparta`].
    ///
    /// # Errors
    /// [`ErrorTunel`] when Windows refuses: then nothing goes out at all (the cut-off switch is still on).
    pub fn vuelve(&mut self) -> Result<(), ErrorTunel> {
        if self.todo {
            let luid = self.adaptador.luid();
            red_vpn::pon_rutas(luid, &[POR_DEFECTO_V4])?;
            let _ = red_vpn::pon_rutas(luid, &[POR_DEFECTO_V6]);
        }
        Ok(())
    }

    /// A new sign-up with the same server, on the tunnel as it is (`guardiana_tunel::conexion::renueva`): the whole
    /// configuration again (the same key, the server's new preshared key, its address) and, if the server gave another
    /// address, that one in place of the old. The cut-off switch is not touched: nothing leaves outside the tunnel
    /// meanwhile. A different resolver would need another cut-off switch, so it is refused.
    ///
    /// # Errors
    /// [`ErrorTunel`] with the step that failed; the cut-off switch stays on.
    pub fn renueva(
        &mut self,
        llaves: &Llaves,
        alta: &Alta,
        extremo: SocketAddr,
    ) -> Result<(), ErrorTunel> {
        if alta.dns != self.dns {
            return Err(ErrorTunel::Corte(
                "el servidor ha cambiado de resolutor: hay que volver a conectar".to_owned(),
            ));
        }
        let par = nt::Par {
            publica: alta.servidor,
            psk: Some(alta.psk.clone()),
            extremo: Some(extremo),
            keepalive: KEEPALIVE,
            permitidas: todas(),
        };
        self.adaptador
            .configura(&nt::configuracion(&llaves.secreta_bytes(), None, &[par]))?;
        if alta.direccion != self.direccion {
            let luid = self.adaptador.luid();
            red_vpn::pon_direccion(luid, &IpAddr::V4(alta.direccion), PREFIJO)?;
            // the old one, off: with it still there, Windows could keep sending from it
            let _ = red_vpn::quita_direccion(luid, &IpAddr::V4(self.direccion));
            self.direccion = alta.direccion;
        }
        Ok(())
    }

    /// This computer's address in the tunnel now.
    #[must_use]
    pub fn direccion(&self) -> Ipv4Addr {
        self.direccion
    }

    /// The adapter's LUID.
    #[must_use]
    pub fn luid(&self) -> u64 {
        self.adaptador.luid()
    }

    /// The server's peer as the driver has it now: bytes each way and the last handshake (none until the server
    /// answers); `None` when it cannot be read.
    #[must_use]
    pub fn estado(&self) -> Option<nt::EstadoPar> {
        self.adaptador
            .lee()
            .ok()
            .and_then(|b| nt::estado_del_par(&b))
    }

    /// Whether the cut-off switch is on.
    #[must_use]
    pub fn con_corte(&self) -> bool {
        self.corte.is_some()
    }

    /// Closes the tunnel: the cut-off switch off, the adapter down and gone.
    pub fn cierra(mut self) {
        self.corte = None;
        let _ = self.adaptador.pon(false);
    }
}

/// A connection made with [`conecta`], its tunel open.
pub type Conexion = Conectado<Tunel>;

impl Abierto for Tunel {
    fn saludo_ms(&self) -> Option<u64> {
        self.estado().and_then(|e| e.ultimo_saludo_ms)
    }

    fn cierra(self) {
        Tunel::cierra(self);
    }

    fn renueva(&mut self, llaves: &Llaves, alta: &Alta, extremo: SocketAddr) -> Result<(), String> {
        Tunel::renueva(self, llaves, alta, extremo).map_err(|e| e.to_string())
    }

    fn aparta(&mut self) -> Result<(), String> {
        Tunel::aparta(self).map_err(|e| e.to_string())
    }

    fn vuelve(&mut self) -> Result<(), String> {
        Tunel::vuelve(self).map_err(|e| e.to_string())
    }
}

/// A whole connection with the GUARDIANA server called `host` (`guardiana_tunel::conexion`): fresh keys, the sign-up
/// over HTTPS, the server's address, the tunnel with everything inside and the cut-off switch on (WireGuardNT's library
/// at `dll`; `servicio` is the service's program file, the only one let out), and the server's first handshake. If the
/// server does not answer in time, the tunnel is closed again and the computer's network stays as it was.
///
/// # Errors
/// [`ErrorConexion`] with the step that failed.
pub fn conecta(host: &str, dll: &Path, servicio: &Path) -> Result<Conectado<Tunel>, ErrorConexion> {
    conexion::conecta(
        host,
        conexion::pide_alta,
        conexion::resuelve,
        |llaves, alta, extremo| {
            Tunel::abre(dll, servicio, llaves, alta, extremo, true).map_err(|e| e.to_string())
        },
        conexion::ESPERA,
        ahora_ms,
    )
}

/// A new sign-up for a connection that dropped (the server stopped answering: `guardiana_tunel::conexion::caida`), with
/// fresh keys, straight to the server's address known from the first one (with the cut-off switch on nothing resolves
/// outside the tunnel; the server's certificate is still checked against its name), with the tunnel stepped aside for
/// that one request ([`Tunel::aparta`]). The cut-off switch stays on throughout.
///
/// # Errors
/// [`ErrorConexion`] with the step that failed; the tunnel stays open, closed to everything outside, to try again.
pub fn renueva(c: &mut Conexion) -> Result<(), ErrorConexion> {
    let (servidor, ip) = (c.servidor.clone(), c.extremo.ip());
    conexion::renueva(
        c,
        move |host, pide| {
            if host == servidor {
                conexion::pide_alta_en(host, SocketAddr::new(ip, 443), pide)
            } else {
                // the sign-up was asked of another name than the tunnel's end: nothing can find it with the cut-off
                // switch on (DNS goes only into the tunnel); disconnecting and connecting again does
                Err(ErrorConexion::Alta(
                    "el alta y el túnel tienen nombres distintos: desconecta y vuelve a conectar"
                        .to_owned(),
                ))
            }
        },
        conexion::ESPERA,
        ahora_ms,
    )
}

fn ahora_ms() -> u64 {
    u64::try_from(guardiana_core::time::now_ms()).unwrap_or(0)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// On a real Windows, as an administrator, with WireGuardNT at `GUARDIANA_WG_DLL`: the whole sequence with a
    /// sign-up made up here (no server answers at 192.0.2.1, a documentation address), only the tunnel's own network
    /// inside and no cut-off, so the test machine's network stays as it is; then closed.
    #[test]
    #[ignore = "necesita Windows como administrador y wireguard.dll en GUARDIANA_WG_DLL"]
    fn el_tunel_entero_sin_tocar_la_red_del_equipo() {
        let ruta = std::env::var_os("GUARDIANA_WG_DLL").unwrap();
        let llaves = Llaves::nuevas().unwrap();
        let alta = Alta {
            direccion: Ipv4Addr::new(10, 66, 7, 3),
            servidor: [7; 32],
            host: "prueba.guardianagroup.com".into(),
            puerto: 51820,
            dns: Ipv4Addr::new(10, 66, 0, 1),
            psk: zeroize::Zeroizing::new([9; 32]),
            caduca_ms: 0,
        };
        let yo = std::env::current_exe().unwrap();
        let t = Tunel::abre(
            Path::new(&ruta),
            &yo,
            &llaves,
            &alta,
            "192.0.2.1:51820".parse().unwrap(),
            false,
        )
        .unwrap();
        assert_ne!(t.luid(), 0);
        assert!(!t.con_corte());
        let e = t.estado().unwrap();
        assert_eq!(e.publica, [7; 32]);
        assert_eq!(e.ultimo_saludo_ms, None);
        // a new sign-up with the same server: another preshared key and another address, on the same adapter
        let mut t = t;
        let otra = Alta {
            direccion: Ipv4Addr::new(10, 66, 7, 4),
            servidor: [6; 32],
            psk: zeroize::Zeroizing::new([8; 32]),
            ..alta
        };
        t.renueva(&llaves, &otra, "192.0.2.1:51820".parse().unwrap())
            .unwrap();
        assert_eq!(t.direccion(), Ipv4Addr::new(10, 66, 7, 4));
        let e = t.estado().unwrap();
        assert_eq!(e.publica, [6; 32]);
        assert_eq!(e.ultimo_saludo_ms, None);
        // the old address is gone: taking it off again fails
        assert!(
            red_vpn::quita_direccion(t.luid(), &IpAddr::V4(Ipv4Addr::new(10, 66, 7, 3))).is_err()
        );
        // stepping aside and back (no default routes in this test: nothing to do), and the test network's own route taken
        // off (twice: gone is gone) and put back, as a new sign-up does with the default ones
        t.aparta().unwrap();
        t.vuelve().unwrap();
        let red = [(IpAddr::V4(Ipv4Addr::new(10, 66, 0, 0)), 16)];
        red_vpn::quita_rutas(t.luid(), &red).unwrap();
        red_vpn::quita_rutas(t.luid(), &red).unwrap();
        red_vpn::pon_rutas(t.luid(), &red).unwrap();
        // another resolver is refused: it would need another cut-off switch
        let otro_dns = Alta {
            dns: Ipv4Addr::new(10, 66, 0, 2),
            psk: zeroize::Zeroizing::new([8; 32]),
            ..otra
        };
        assert!(t
            .renueva(&llaves, &otro_dns, "192.0.2.1:51820".parse().unwrap())
            .is_err());
        t.cierra();
    }
}
