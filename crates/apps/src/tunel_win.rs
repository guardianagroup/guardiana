//! The VPN on Windows, put together (in preparation; docs/VPN.md): from a checked sign-up with a GUARDIANA server
//! (`guardiana_tunel::alta`) to a tunnel that carries everything, in this order, and undone in the reverse one:
//!
//! 1. WireGuardNT's adapter, «GUARDIANA» (`wireguard_nt`), with its whole configuration: this computer's key, the
//!    server's, the post-quantum preshared key, the server's address and everything allowed (`guardiana_tunel::nt`);
//! 2. its address, its routes, its settings and its resolver (`red_vpn`);
//! 3. the cut-off switch (`corte_vpn`, with `guardiana_tunel::corte`'s plan): nothing leaves outside the tunnel but
//!    the encrypted packets to the server (UDP, its address and its port; since 1.0.55 nothing else of the service's
//!    either), and the answers of Home Mode's connections to the house; and nothing Windows forwards for its virtual
//!    machines or a shared connection goes anywhere but into the tunnel (1.0.56);
//! 4. the adapter up.
//!
//! Before any of it, with everything inside, this computer's LAN address is read while the default route is still the
//! network's, and kept with the tunnel's resolver (`guardiana_core::tunel_dns`): Home Mode stays on it (1.0.55).
//!
//! When anything fails, what was done is undone (dropping the cut-off removes its filters; closing the adapter removes
//! its address and routes). `todo == false` keeps the computer's own routes and adds no cut-off: only the tunnel's own
//! network goes inside, for the real test on a machine whose network must stay as it is.
//!
//! Strict mode (ronda 91; docs/VPN.md, «Modo estricto», [`Modo::Estricto`]): the block is not the tunnel's. It is put
//! in Windows apart, persistent (`corte_vpn::pon_estricto`), before the tunnel opens, and it stays when the tunnel
//! closes, the server does not answer or the service ends; the tunnel only adds its own interface's permit to it
//! (`corte_vpn::PermisoTunel`), which goes with the adapter. [`intento_estricto`] is one attempt at a connection that
//! way: when the server does not answer, the tunnel closes and the block stays.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use guardiana_core::tunel_dns::Desvio;
use guardiana_tunel::alta::Alta;
use guardiana_tunel::conexion::{self, Abierto, Conectado, ErrorConexion};
use guardiana_tunel::llaves::Llaves;
use guardiana_tunel::{corte, nt};

use crate::cortafuegos::ErrorCorte;
use crate::corte_vpn::{CorteVpn, PermisoTunel};
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

/// Which cut-off switch a tunnel opens with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    /// None, and only the tunnel's own network inside: the real test on a machine whose network must stay as it is.
    Prueba,
    /// Everything inside, and the switch of always: a dynamic session with the whole plan, gone with the process.
    Normal,
    /// Everything inside, in strict mode: the block is already in Windows apart (`corte_vpn::pon_estricto`, put by the
    /// caller); the tunnel adds only its own interface's permit to it, which goes with the adapter.
    Estricto,
}

/// The switch a tunnel has. Each is held only for what dropping it does: its filters leave Windows.
#[allow(dead_code)]
enum Interruptor {
    Ninguno,
    Normal(CorteVpn),
    Estricto(PermisoTunel),
}

/// The tunnel, open. Its fields drop in this order: the cut-off switch first, then the adapter, and last the tunnel's
/// resolver as GUARDIANA's DNS upstream (with the LAN address from before the tunnel), so no name goes to the network's
/// resolvers, and Home Mode does not move, while the tunnel is still there.
pub struct Tunel {
    corte: Interruptor,
    adaptador: Adaptador,
    /// This computer's address in the tunnel now, and the tunnel's resolver (reached on the tunnel's interface: the
    /// cut-off switch lets no DNS out anywhere else).
    direccion: Ipv4Addr,
    dns: Ipv4Addr,
    /// The server's address and port: the cut-off switch lets the encrypted packets go there and nowhere else.
    extremo: SocketAddr,
    /// With everything inside: GUARDIANA's own resolver forwards only to the tunnel's (`guardiana_core::tunel_dns`).
    /// The service's packets used to be let out anywhere, so until 1.0.54 the names it forwarded went to the home
    /// router outside the tunnel (second review of 2 Nov 2026). It also keeps the LAN address from before the tunnel,
    /// for Home Mode (1.0.55).
    desvio: Option<Desvio>,
    /// The service's program file: the switch of always is made again with it when strict mode is turned off.
    servicio: PathBuf,
}

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
        let modo = if todo { Modo::Normal } else { Modo::Prueba };
        Self::abre_en(dll, servicio, llaves, alta, extremo, modo)
    }

    /// [`Tunel::abre`] with the switch `modo` says. In [`Modo::Estricto`] strict mode's block must already be in
    /// Windows (`corte_vpn::pon_estricto`): the tunnel only adds its own interface's permit to it.
    ///
    /// # Errors
    /// [`ErrorTunel`] with the step that failed; everything done before it is undone (strict mode's block stays).
    pub fn abre_en(
        dll: &Path,
        servicio: &Path,
        llaves: &Llaves,
        alta: &Alta,
        extremo: SocketAddr,
        modo: Modo,
    ) -> Result<Self, ErrorTunel> {
        let todo = modo != Modo::Prueba;
        // This computer's LAN address, read before the tunnel takes the default route: from then on the way to
        // anywhere is the tunnel's, and the same reading gives the tunnel's own address. Home Mode listened there, and
        // the house lost its DNS (third review of 2 Nov 2026). While the tunnel lives, this one is the LAN's
        // (`guardiana_core::tunel_dns::lan`).
        let lan = if todo {
            guardiana_core::tunel_dns::lan()
        } else {
            None
        };
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
            red_vpn::pon_rutas(luid, &[(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)])?;
            red_vpn::ajusta(luid, false, Some(MTU), true)?;
            // IPv6 goes inside too when the adapter takes it. The tunnel has no IPv6 address, so this may fail, and it
            // need not succeed: the cut-off switch lets no IPv6 out of the tunnel but DHCPv6 and neighbour discovery on
            // the link (`guardiana_tunel::corte`, «IPv6», and its test `nada_de_ipv6_fuera_del_tunel`)
            let _ = red_vpn::pon_rutas(luid, &[(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0)]);
            let _ = red_vpn::ajusta(luid, true, Some(MTU), true);
        } else {
            let [a, b, _, _] = alta.direccion.octets();
            red_vpn::pon_rutas(luid, &[(IpAddr::V4(Ipv4Addr::new(a, b, 0, 0)), PREFIJO)])?;
            red_vpn::ajusta(luid, false, Some(MTU), false)?;
        }
        red_vpn::pon_dns(NOMBRE, &[IpAddr::V4(alta.dns)])?;
        // From here on GUARDIANA's resolver asks only the tunnel's, which answers once the tunnel is up: until then a
        // name fails instead of leaving outside. If a later step fails, this goes away with it.
        let desvio = todo.then(|| Desvio::pon(SocketAddr::new(IpAddr::V4(alta.dns), 53), lan));
        // Of the service, only the encrypted packets to `extremo` go out of the tunnel (1.0.55)
        let corte = match modo {
            Modo::Prueba => Interruptor::Ninguno,
            Modo::Normal => {
                Interruptor::Normal(CorteVpn::activa(&corte::plan(luid, extremo), servicio)?)
            }
            Modo::Estricto => Interruptor::Estricto(PermisoTunel::pon(&corte::del_tunel(luid))?),
        };
        adaptador.pon(true)?;
        Ok(Self {
            corte,
            adaptador,
            direccion: alta.direccion,
            dns: alta.dns,
            extremo,
            desvio,
            servicio: servicio.to_path_buf(),
        })
    }

    /// Strict mode turned on with the tunnel open: its block must already be in Windows (`corte_vpn::pon_estricto`);
    /// the tunnel's own permit goes into it, and only then the switch of before goes.
    ///
    /// # Errors
    /// [`ErrorTunel::Corte`] when Windows refuses the tunnel's permit; the switch of before stays.
    pub fn a_estricto(&mut self) -> Result<(), ErrorTunel> {
        if matches!(self.corte, Interruptor::Normal(_)) {
            let permiso = PermisoTunel::pon(&corte::del_tunel(self.luid()))?;
            self.corte = Interruptor::Estricto(permiso);
        }
        Ok(())
    }

    /// Strict mode turned off with the tunnel open: the switch of always goes on first, and only then the tunnel's
    /// permit in strict mode's block goes (the caller removes that block after).
    ///
    /// # Errors
    /// [`ErrorTunel::Corte`] when Windows refuses the switch; strict mode's permit stays.
    pub fn a_normal(&mut self) -> Result<(), ErrorTunel> {
        if matches!(self.corte, Interruptor::Estricto(_)) {
            let normal = CorteVpn::activa(&corte::plan(self.luid(), self.extremo), &self.servicio)?;
            self.corte = Interruptor::Normal(normal);
        }
        Ok(())
    }

    /// Whether the tunnel is in strict mode (its block in Windows, apart from it).
    #[must_use]
    pub fn es_estricto(&self) -> bool {
        matches!(self.corte, Interruptor::Estricto(_))
    }

    /// The server's address and port, where the encrypted packets go.
    #[must_use]
    pub fn extremo(&self) -> SocketAddr {
        self.extremo
    }

    /// A new sign-up with the same server, on the tunnel as it is (`guardiana_tunel::conexion::renueva`): the whole
    /// configuration again (the same key, the server's new preshared key, its address) and, if the server gave another
    /// address, that one in place of the old. The cut-off switch is not touched: nothing leaves outside the tunnel
    /// meanwhile. A different resolver, or with the cut-off switch on a different address or port of the server (the
    /// switch lets the encrypted packets go to the first one only), would need another cut-off switch, so it is
    /// refused.
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
        if self.con_corte() && extremo != self.extremo {
            return Err(ErrorTunel::Corte(
                "el servidor ha cambiado de dirección: hay que volver a conectar".to_owned(),
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
        self.extremo = extremo;
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

    /// Whether the cut-off switch is on: the switch of always, or strict mode's block with the tunnel's permit in it.
    #[must_use]
    pub fn con_corte(&self) -> bool {
        !matches!(self.corte, Interruptor::Ninguno)
    }

    /// Whether GUARDIANA's resolver is forwarding only to the tunnel's (with everything inside).
    #[must_use]
    pub fn con_dns_del_tunel(&self) -> bool {
        self.desvio.is_some()
    }

    /// Closes the tunnel: the cut-off switch off (in strict mode, only the tunnel's own permit: the block stays), the
    /// adapter down and gone.
    pub fn cierra(mut self) {
        self.corte = Interruptor::Ninguno;
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

/// One attempt of strict mode's connection with the GUARDIANA server called `host` (docs/VPN.md, «Modo estricto»):
/// fresh keys; the sign-up over HTTPS at `alta`, the server's address, already known (with the block on no name resolves
/// outside the tunnel; the server's certificate is still checked against its name); the endpoint's address, from
/// `sin_resolver` when that knows it, from the system otherwise; then `bloquea`, where the caller puts strict mode's
/// block for that endpoint and keeps what it learnt; then the tunnel, with everything inside and only its own permit in
/// the block, and the server's first handshake. When it does not come in time ([`conexion::ESPERA`]), the tunnel closes
/// and the block stays: nothing leaves outside the tunnel until the next attempt, or until the person turns the VPN
/// off.
///
/// # Errors
/// [`ErrorConexion`] with the step that failed; strict mode's block is never removed here.
pub fn intento_estricto(
    host: &str,
    alta: SocketAddr,
    sin_resolver: impl Fn(&str, u16) -> Option<SocketAddr>,
    dll: &Path,
    servicio: &Path,
    bloquea: impl FnOnce(&Alta, SocketAddr) -> Result<(), String>,
) -> Result<Conexion, ErrorConexion> {
    conexion::conecta(
        host,
        |h, pide| conexion::pide_alta_en(h, alta, pide),
        |nombre, puerto| match sin_resolver(nombre, puerto) {
            Some(a) => Ok(a),
            None => conexion::resuelve(nombre, puerto),
        },
        |llaves, a, extremo| {
            bloquea(a, extremo)?;
            Tunel::abre_en(dll, servicio, llaves, a, extremo, Modo::Estricto)
                .map_err(|e| e.to_string())
        },
        conexion::ESPERA,
        ahora_ms,
    )
}

/// A new sign-up for a connection that dropped (the server stopped answering: `guardiana_tunel::conexion::caida`),
/// straight to the server's address known from the first one, because with the cut-off switch on nothing resolves
/// outside the tunnel; the server's certificate is still checked against its name. The tunnel and its cut-off switch stay
/// on throughout.
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
                // the sign-up was asked of another name than the tunnel's end: only the system can find it
                conexion::pide_alta(host, pide)
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
        // one tunnel at a time: the tunnel's resolver is the whole process's (strict mode's test puts it too)
        let _uno = crate::corte_vpn::EN_PRUEBA
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        // only the tunnel's own network inside: GUARDIANA's resolver keeps the network's resolvers
        assert!(!t.con_dns_del_tunel());
        assert_eq!(guardiana_core::tunel_dns::actual(), None);
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

    /// On a real Windows, as an administrator, with WireGuardNT at `GUARDIANA_WG_DLL` (strict mode, ronda 91): strict
    /// mode's block for a server that never answers (192.0.2.1, a documentation address: a black hole), and the tunnel
    /// with everything inside. No handshake comes, and nothing gets out: not with the tunnel up, and not by the network
    /// card once the tunnel closes, as a connection closes it when the server does not answer; there Windows refuses the
    /// connection itself (WSAEACCES). The block stays until it is removed, and then the network is back. The test
    /// machine's network is cut for those seconds.
    #[test]
    #[ignore = "necesita Windows como administrador y wireguard.dll en GUARDIANA_WG_DLL; corta la red unos segundos"]
    fn el_modo_estricto_con_un_servidor_que_no_contesta() {
        use crate::corte_vpn::{en_windows, pon_estricto, quita_estricto};
        use std::net::TcpStream;
        use std::time::Duration;

        /// Removes strict mode's block when dropped, a panic included: the test machine's network comes back.
        struct Quita;
        impl Drop for Quita {
            fn drop(&mut self) {
                let _ = quita_estricto();
            }
        }

        let _uno = crate::corte_vpn::EN_PRUEBA
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let fuera: SocketAddr = "1.1.1.1:443".parse().unwrap();
        let antes = TcpStream::connect_timeout(&fuera, Duration::from_secs(5)).is_ok();
        let ruta = std::env::var_os("GUARDIANA_WG_DLL").unwrap();
        let yo = std::env::current_exe().unwrap();
        let extremo: SocketAddr = "192.0.2.1:51820".parse().unwrap();
        let alta = Alta {
            direccion: Ipv4Addr::new(10, 66, 7, 9),
            servidor: [7; 32],
            host: "prueba.guardianagroup.com".into(),
            puerto: 51820,
            dns: Ipv4Addr::new(10, 66, 0, 1),
            psk: zeroize::Zeroizing::new([9; 32]),
            caduca_ms: 0,
        };
        let _quita = Quita;
        let plan = corte::estricto(corte::Servidor {
            extremo: Some(extremo),
            alta: Some("192.0.2.1:443".parse().unwrap()),
        });
        let puesto = pon_estricto(&plan, &yo).unwrap();
        eprintln!("bloqueo puesto: {puesto:?}");
        let llaves = Llaves::nuevas().unwrap();
        let t = Tunel::abre_en(
            Path::new(&ruta),
            &yo,
            &llaves,
            &alta,
            extremo,
            Modo::Estricto,
        )
        .unwrap();
        assert!(t.con_corte() && t.es_estricto());
        std::thread::sleep(Duration::from_secs(3));
        assert_eq!(t.estado().and_then(|e| e.ultimo_saludo_ms), None);
        // con el túnel puesto y el servidor callado, nada llega fuera
        assert!(TcpStream::connect_timeout(&fuera, Duration::from_secs(3)).is_err());
        // el túnel se cierra, como lo cierra una conexión cuyo servidor no contesta; el bloqueo sigue
        t.cierra();
        std::thread::sleep(Duration::from_secs(1));
        let en = en_windows().unwrap();
        assert_eq!(en.persistentes, plan.persistentes.len(), "{en:?}");
        // y el bloqueo de lo que Windows reenvía (las máquinas virtuales, la conexión compartida), en sus dos capas
        let reenvio = |p: &[corte::Filtro]| p.iter().filter(|f| f.capa.es_reenvio()).count();
        let esperado = reenvio(&plan.persistentes)
            + if puesto.arranque.is_ok() {
                reenvio(&plan.arranque)
            } else {
                0
            };
        assert_eq!(reenvio(&plan.persistentes), 2);
        assert_eq!(en.reenvio, esperado, "{en:?}");
        let r = TcpStream::connect_timeout(&fuera, Duration::from_secs(3));
        assert_eq!(
            r.as_ref().err().and_then(std::io::Error::raw_os_error),
            Some(10013),
            "por la tarjeta, sin túnel: {r:?}"
        );
        // apagada: la red vuelve
        quita_estricto().unwrap();
        assert!(!en_windows().unwrap().alguno());
        if antes {
            assert!(TcpStream::connect_timeout(&fuera, Duration::from_secs(5)).is_ok());
        }
    }
}
