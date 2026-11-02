//! The VPN's address, routes and interface settings on Windows (in preparation; docs/VPN.md): what WireGuard's own
//! application for Windows does to its adapter (`tunnel/addressconfig.go`, MIT, read at its source on 1 Nov 2026),
//! through Windows' IP Helper:
//!
//! - the tunnel's address on the adapter (`CreateUnicastIpAddressEntry`), and the old one taken off when a new sign-up
//!   gives another (`DeleteUnicastIpAddressEntry`);
//! - a route for each allowed network on the adapter, next hop none, metric 0 (`CreateIpForwardEntry2`), and the default
//!   ones taken off and put back around a new sign-up (`DeleteIpForwardEntry2`);
//! - the adapter's settings: no router discovery, no duplicate address detection, no DHCPv6, its MTU, and metric 0 with
//!   the automatic metric off when it carries the default route (`GetIpInterfaceEntry`, `SetIpInterfaceEntry`);
//! - its DNS servers, the tunnel's resolvers, with Windows' own `netsh` (every Windows 10 and 11 has it; the newer
//!   `SetInterfaceDnsSettings` is missing before Windows 10 2004, and linking it would stop the program from starting
//!   there).
//!
//! The encrypted packets to the server do not loop into the tunnel: WireGuardNT itself sends them by the best route
//! other than its own adapter (WireGuard's application adds no exception route either). All of it goes with the
//! adapter: closing it removes its address and its routes.
//!
//! `unsafe` is used here, as the project allows with justification: IP Helper is a C API with no safe wrapper. Every row
//! is initialised by Windows' own function before it is filled, owned here, and every result is checked.
#![allow(unsafe_code)]

use std::net::IpAddr;

use windows_sys::Win32::Foundation::{ERROR_NOT_FOUND, ERROR_OBJECT_ALREADY_EXISTS, NO_ERROR};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    CreateIpForwardEntry2, CreateUnicastIpAddressEntry, DeleteIpForwardEntry2,
    DeleteUnicastIpAddressEntry, GetIpInterfaceEntry, InitializeIpForwardEntry,
    InitializeIpInterfaceEntry, InitializeUnicastIpAddressEntry, SetIpInterfaceEntry,
    MIB_IPFORWARD_ROW2, MIB_IPINTERFACE_ROW, MIB_UNICASTIPADDRESS_ROW,
};
use windows_sys::Win32::Networking::WinSock::{
    IpDadStatePreferred, RouterDiscoveryDisabled, AF_INET, AF_INET6, IN6_ADDR_0, IN_ADDR_0,
    SOCKADDR_IN, SOCKADDR_IN6, SOCKADDR_INET,
};

/// Why the adapter's network could not be set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorRed(pub String);

impl std::fmt::Display for ErrorRed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Windows no dejó preparar la red del túnel: {}", self.0)
    }
}

fn comprueba(llamada: &str, r: u32) -> Result<(), ErrorRed> {
    if r == NO_ERROR || r == ERROR_OBJECT_ALREADY_EXISTS {
        Ok(())
    } else {
        Err(ErrorRed(format!("{llamada} = {r}")))
    }
}

/// An address as `SOCKADDR_INET` (port 0): the variant of its family, written whole.
fn sockaddr(ip: &IpAddr) -> SOCKADDR_INET {
    // SAFETY: zeroed socket addresses are "no address"; the family and the address are set below.
    let mut s: SOCKADDR_INET = unsafe { std::mem::zeroed() };
    match ip {
        IpAddr::V4(a) => {
            // SAFETY: as above.
            let mut v4: SOCKADDR_IN = unsafe { std::mem::zeroed() };
            v4.sin_family = AF_INET;
            // the address in network order: its four bytes as they are
            v4.sin_addr.S_un = IN_ADDR_0 {
                S_addr: u32::from_ne_bytes(a.octets()),
            };
            s.Ipv4 = v4;
        }
        IpAddr::V6(a) => {
            // SAFETY: as above.
            let mut v6: SOCKADDR_IN6 = unsafe { std::mem::zeroed() };
            v6.sin6_family = AF_INET6;
            v6.sin6_addr.u = IN6_ADDR_0 { Byte: a.octets() };
            s.Ipv6 = v6;
        }
    }
    s
}

/// Puts `direccion`/`prefijo` on the adapter `luid`.
///
/// # Errors
/// [`ErrorRed`] with the call Windows refused.
pub fn pon_direccion(luid: u64, direccion: &IpAddr, prefijo: u8) -> Result<(), ErrorRed> {
    // SAFETY: a zeroed row, initialised by Windows' own function before any field is set.
    let mut fila: MIB_UNICASTIPADDRESS_ROW = unsafe { std::mem::zeroed() };
    // SAFETY: a row owned here.
    unsafe { InitializeUnicastIpAddressEntry(&mut fila) };
    fila.InterfaceLuid.Value = luid;
    fila.Address = sockaddr(direccion);
    fila.OnLinkPrefixLength = prefijo;
    fila.DadState = IpDadStatePreferred;
    // SAFETY: a row initialised and filled above, owned for the call.
    comprueba("CreateUnicastIpAddressEntry", unsafe {
        CreateUnicastIpAddressEntry(&fila)
    })
}

/// Takes the routes of `redes` (as [`pon_rutas`] put them) off the adapter `luid`. A route that is not there is not an
/// error: what is asked is that it be gone.
///
/// # Errors
/// [`ErrorRed`] with the call Windows refused.
pub fn quita_rutas(luid: u64, redes: &[(IpAddr, u8)]) -> Result<(), ErrorRed> {
    for (red, prefijo) in redes {
        // SAFETY: a zeroed row, initialised by Windows' own function before any field is set.
        let mut fila: MIB_IPFORWARD_ROW2 = unsafe { std::mem::zeroed() };
        // SAFETY: a row owned here.
        unsafe { InitializeIpForwardEntry(&mut fila) };
        fila.InterfaceLuid.Value = luid;
        fila.DestinationPrefix.Prefix = sockaddr(red);
        fila.DestinationPrefix.PrefixLength = *prefijo;
        fila.NextHop = sockaddr(&match red {
            IpAddr::V4(_) => IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
            IpAddr::V6(_) => IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED),
        });
        // SAFETY: a row initialised and filled above, owned for the call.
        let r = unsafe { DeleteIpForwardEntry2(&fila) };
        if r != ERROR_NOT_FOUND {
            comprueba("DeleteIpForwardEntry2", r)?;
        }
    }
    Ok(())
}

/// Takes `direccion` off the adapter `luid`.
///
/// # Errors
/// [`ErrorRed`] with the call Windows refused (also when the adapter does not have that address).
pub fn quita_direccion(luid: u64, direccion: &IpAddr) -> Result<(), ErrorRed> {
    // SAFETY: a zeroed row, initialised by Windows' own function before any field is set.
    let mut fila: MIB_UNICASTIPADDRESS_ROW = unsafe { std::mem::zeroed() };
    // SAFETY: a row owned here.
    unsafe { InitializeUnicastIpAddressEntry(&mut fila) };
    fila.InterfaceLuid.Value = luid;
    fila.Address = sockaddr(direccion);
    // SAFETY: a row initialised and filled above, owned for the call.
    comprueba("DeleteUnicastIpAddressEntry", unsafe {
        DeleteUnicastIpAddressEntry(&fila)
    })
}

/// A route on the adapter `luid` for each network in `redes` (address and prefix length), no next hop, metric 0.
///
/// # Errors
/// [`ErrorRed`] with the call Windows refused.
pub fn pon_rutas(luid: u64, redes: &[(IpAddr, u8)]) -> Result<(), ErrorRed> {
    for (red, prefijo) in redes {
        // SAFETY: a zeroed row, initialised by Windows' own function before any field is set.
        let mut fila: MIB_IPFORWARD_ROW2 = unsafe { std::mem::zeroed() };
        // SAFETY: a row owned here.
        unsafe { InitializeIpForwardEntry(&mut fila) };
        fila.InterfaceLuid.Value = luid;
        fila.DestinationPrefix.Prefix = sockaddr(red);
        fila.DestinationPrefix.PrefixLength = *prefijo;
        fila.NextHop = sockaddr(&match red {
            IpAddr::V4(_) => IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
            IpAddr::V6(_) => IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED),
        });
        fila.Metric = 0;
        // SAFETY: a row initialised and filled above, owned for the call.
        comprueba("CreateIpForwardEntry2", unsafe {
            CreateIpForwardEntry2(&fila)
        })?;
    }
    Ok(())
}

/// The adapter's settings for one family (`v6`): no router discovery, no duplicate address detection, no DHCPv6, the
/// MTU when given, and metric 0 with the automatic metric off when it carries the default route.
///
/// # Errors
/// [`ErrorRed`] with the call Windows refused.
pub fn ajusta(luid: u64, v6: bool, mtu: Option<u32>, por_defecto: bool) -> Result<(), ErrorRed> {
    // SAFETY: a zeroed row, initialised by Windows' own function before any field is set.
    let mut fila: MIB_IPINTERFACE_ROW = unsafe { std::mem::zeroed() };
    // SAFETY: a row owned here.
    unsafe { InitializeIpInterfaceEntry(&mut fila) };
    fila.Family = if v6 { AF_INET6 } else { AF_INET };
    fila.InterfaceLuid.Value = luid;
    // SAFETY: a row with its family and interface set, owned for the call; Windows fills the rest.
    comprueba("GetIpInterfaceEntry", unsafe {
        GetIpInterfaceEntry(&mut fila)
    })?;
    fila.RouterDiscoveryBehavior = RouterDiscoveryDisabled;
    fila.DadTransmits = 0;
    fila.ManagedAddressConfigurationSupported = false;
    fila.OtherStatefulConfigurationSupported = false;
    if let Some(m) = mtu {
        fila.NlMtu = m;
    }
    if por_defecto {
        fila.UseAutomaticMetric = false;
        fila.Metric = 0;
    }
    if !v6 {
        // Windows refuses an IPv4 row that carries a site prefix length (documented for SetIpInterfaceEntry).
        fila.SitePrefixLength = 0;
    }
    // SAFETY: the row read above, changed in the fields Windows lets be set, owned for the call.
    comprueba("SetIpInterfaceEntry", unsafe {
        SetIpInterfaceEntry(&mut fila)
    })
}

/// The tunnel's resolvers as the DNS servers of the adapter called `alias` (the name it was created with), for each
/// family that has one; nothing registered in DNS.
///
/// # Errors
/// [`ErrorRed`] when `netsh` could not run or refused.
pub fn pon_dns(alias: &str, servidores: &[IpAddr]) -> Result<(), ErrorRed> {
    let netsh = std::env::var_os("SystemRoot")
        .map_or_else(
            || std::path::PathBuf::from(r"C:\Windows"),
            std::path::PathBuf::from,
        )
        .join(r"System32\netsh.exe");
    for (familia, v6) in [("ipv4", false), ("ipv6", true)] {
        let de_esta: Vec<&IpAddr> = servidores.iter().filter(|s| s.is_ipv6() == v6).collect();
        for (i, s) in de_esta.iter().enumerate() {
            let nombre = format!("name={alias}");
            let direccion = s.to_string();
            let mut orden = std::process::Command::new(&netsh);
            if i == 0 {
                orden.args([
                    "interface",
                    familia,
                    "set",
                    "dnsservers",
                    &nombre,
                    "source=static",
                    &format!("address={direccion}"),
                    "register=none",
                    "validate=no",
                ]);
            } else {
                orden.args([
                    "interface",
                    familia,
                    "add",
                    "dnsservers",
                    &nombre,
                    &format!("address={direccion}"),
                    &format!("index={}", i + 1),
                    "validate=no",
                ]);
            }
            let salida = orden
                .output()
                .map_err(|e| ErrorRed(format!("netsh: {e}")))?;
            if !salida.status.success() {
                return Err(ErrorRed(format!(
                    "netsh {familia} {direccion}: {}",
                    String::from_utf8_lossy(&salida.stdout).trim()
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::wireguard_nt::{Adaptador, Biblioteca};
    use std::path::Path;
    use std::sync::Arc;

    /// On a real Windows, as an administrator, with WireGuardNT at `GUARDIANA_WG_DLL`: an adapter gets the tunnel's
    /// address, a route to the tunnel's own network (never the default route: the test machine's network stays as it
    /// is), its settings and its resolver; closing the adapter takes them away.
    #[test]
    #[ignore = "necesita Windows como administrador y wireguard.dll en GUARDIANA_WG_DLL"]
    fn la_red_del_tunel_en_un_adaptador_de_verdad() {
        let ruta = std::env::var_os("GUARDIANA_WG_DLL").unwrap();
        let b = Arc::new(Biblioteca::carga(Path::new(&ruta)).unwrap());
        let a = Adaptador::crea(&b, "GUARDIANA red", "GUARDIANA").unwrap();
        let luid = a.luid();
        pon_direccion(luid, &"10.66.7.2".parse().unwrap(), 16).unwrap();
        pon_rutas(luid, &[("10.66.0.0".parse().unwrap(), 16)]).unwrap();
        ajusta(luid, false, Some(1420), false).unwrap();
        pon_dns("GUARDIANA red", &["10.66.0.1".parse().unwrap()]).unwrap();
        a.pon(true).unwrap();
        a.pon(false).unwrap();
    }
}
