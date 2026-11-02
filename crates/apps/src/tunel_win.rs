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
        let todas = vec![
            (IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            (IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
        ];
        let par = nt::Par {
            publica: alta.servidor,
            psk: Some(alta.psk.clone()),
            extremo: Some(extremo),
            keepalive: KEEPALIVE,
            permitidas: todas,
        };
        adaptador.configura(&nt::configuracion(&llaves.secreta_bytes(), None, &[par]))?;
        let luid = adaptador.luid();
        red_vpn::pon_direccion(luid, &IpAddr::V4(alta.direccion), PREFIJO)?;
        if todo {
            red_vpn::pon_rutas(luid, &[(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)])?;
            red_vpn::ajusta(luid, false, Some(MTU), true)?;
            // IPv6 goes inside too when the adapter has it; when it does not, the cut-off switch keeps it from
            // leaving outside the tunnel anyway
            let _ = red_vpn::pon_rutas(luid, &[(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0)]);
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
        Ok(Self { corte, adaptador })
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
        t.cierra();
    }
}
