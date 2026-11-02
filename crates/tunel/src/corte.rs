//! The VPN's cut-off switch, as a plan: the Windows Filtering Platform filters that keep everything inside the tunnel
//! while it is on (phase 1 of docs/VPN.md; ronda 87: «kill switch con Windows Filtering Platform, no con rutas»).
//!
//! The same set of rules the WireGuard application for Windows applies (its `tunnel/firewall/rules.go`, MIT, read at
//! its source on 1 Nov 2026), so two independent sources agree on it:
//!
//! - permit everything on the tunnel's own interface;
//! - permit GUARDIANA's service itself (the encrypted packets to the server leave from it);
//! - permit this computer talking to itself (loopback: GUARDIANA's DNS lives there);
//! - permit DHCP (v4 and v6) and IPv6 neighbour discovery, so the real network keeps working underneath;
//! - permit Hyper-V's virtual machines talking to each other;
//! - block DNS (port 53) to anything but the tunnel's resolvers, above the general permits;
//! - block everything else, at the lowest weight.
//!
//! Pure data: no Windows call here, so it is tested on every system; the service turns each [`Filtro`] into a filter
//! of a dynamic WFP session (removed by Windows itself if the service ends).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Where a filter applies: WFP's connection-authorisation layers (outgoing connections and accepted incoming ones),
/// for each IP version; and the Ethernet layers for Hyper-V.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capa {
    /// `FWPM_LAYER_ALE_AUTH_CONNECT_V4`.
    ConectaV4,
    /// `FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4`.
    AceptaV4,
    /// `FWPM_LAYER_ALE_AUTH_CONNECT_V6`.
    ConectaV6,
    /// `FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V6`.
    AceptaV6,
    /// `FWPM_LAYER_OUTBOUND_MAC_FRAME_NATIVE`.
    SaleMac,
    /// `FWPM_LAYER_INBOUND_MAC_FRAME_NATIVE`.
    EntraMac,
}

impl Capa {
    /// The four connection-authorisation layers.
    pub const ALE: [Self; 4] = [
        Self::ConectaV4,
        Self::AceptaV4,
        Self::ConectaV6,
        Self::AceptaV6,
    ];

    /// Whether the layer is IPv6's.
    #[must_use]
    pub const fn es_v6(self) -> bool {
        matches!(self, Self::ConectaV6 | Self::AceptaV6)
    }
}

/// UDP's protocol number.
pub const UDP: u8 = 17;
/// TCP's protocol number.
pub const TCP: u8 = 6;
/// ICMPv6's protocol number.
pub const ICMP6: u8 = 58;

/// One condition of a filter (all of a filter's conditions must hold; two of the same field mean either).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Condicion {
    /// `FWPM_CONDITION_IP_LOCAL_INTERFACE`: the interface, by its LUID.
    Interfaz(u64),
    /// `FWPM_CONDITION_ALE_APP_ID` and `FWPM_CONDITION_ALE_USER_ID`: GUARDIANA's service itself (its program file
    /// and its account), filled in by the service.
    Servicio,
    /// `FWPM_CONDITION_FLAGS` with `FWP_CONDITION_FLAG_IS_LOOPBACK`.
    Bucle,
    /// `FWPM_CONDITION_IP_PROTOCOL`.
    Protocolo(u8),
    /// `FWPM_CONDITION_IP_LOCAL_PORT`.
    PuertoLocal(u16),
    /// `FWPM_CONDITION_IP_REMOTE_PORT`.
    PuertoRemoto(u16),
    /// `FWPM_CONDITION_IP_REMOTE_ADDRESS`, one address.
    Remota(IpAddr),
    /// `FWPM_CONDITION_IP_REMOTE_ADDRESS`, an IPv6 network (address and prefix length).
    RemotaRed(Ipv6Addr, u8),
    /// `FWPM_CONDITION_IP_LOCAL_ADDRESS`, an IPv6 network.
    LocalRed(Ipv6Addr, u8),
    /// `FWPM_CONDITION_ICMP_TYPE` (the local port field) and `FWPM_CONDITION_ICMP_CODE` (the remote one).
    Icmp {
        /// The type.
        tipo: u8,
        /// The code.
        codigo: u8,
    },
    /// `FWPM_CONDITION_L2_FLAGS` with `FWP_CONDITION_L2_IS_VM2VM`.
    EntreMaquinasVirtuales,
}

/// One filter of the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filtro {
    /// What it is for, shown in Windows' own list of filters.
    pub nombre: &'static str,
    /// Where.
    pub capa: Capa,
    /// What it matches.
    pub condiciones: Vec<Condicion>,
    /// Permit (`true`) or block.
    pub permite: bool,
    /// Its weight in GUARDIANA's sublayer: the highest that matches decides.
    pub peso: u8,
}

/// The weights, from the highest: the service, the tunnel's interface, loopback, the network underneath, DNS allowed,
/// DNS blocked, everything blocked.
pub mod peso {
    /// GUARDIANA's service.
    pub const SERVICIO: u8 = 15;
    /// The tunnel's interface.
    pub const TUNEL: u8 = 14;
    /// Loopback.
    pub const BUCLE: u8 = 13;
    /// DHCP, neighbour discovery and Hyper-V.
    pub const RED: u8 = 12;
    /// DNS to the tunnel's resolvers.
    pub const DNS_SI: u8 = 11;
    /// DNS to anything else.
    pub const DNS_NO: u8 = 10;
    /// Everything else.
    pub const TODO: u8 = 0;
}

fn en_ale(nombre: &'static str, condiciones: &[Condicion], permite: bool, peso: u8) -> Vec<Filtro> {
    Capa::ALE
        .iter()
        .map(|&capa| Filtro {
            nombre,
            capa,
            condiciones: condiciones.to_vec(),
            permite,
            peso,
        })
        .collect()
}

fn filtro(nombre: &'static str, capa: Capa, condiciones: Vec<Condicion>, peso: u8) -> Filtro {
    Filtro {
        nombre,
        capa,
        condiciones,
        permite: true,
        peso,
    }
}

/// `fe80::/10`, the link's own IPv6 addresses.
fn enlace() -> (Ipv6Addr, u8) {
    (Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 0), 10)
}

/// The whole plan for the tunnel's interface (`luid`) and its resolvers (`dns`): the only DNS servers allowed.
#[must_use]
pub fn plan(luid: u64, dns: &[IpAddr]) -> Vec<Filtro> {
    use Condicion::{Icmp, LocalRed, Protocolo, PuertoLocal, PuertoRemoto, Remota, RemotaRed};
    let mut f = Vec::new();
    f.extend(en_ale(
        "GUARDIANA VPN: el servicio",
        &[Condicion::Servicio],
        true,
        peso::SERVICIO,
    ));
    f.extend(en_ale(
        "GUARDIANA VPN: el túnel",
        &[Condicion::Interfaz(luid)],
        true,
        peso::TUNEL,
    ));
    f.extend(en_ale(
        "GUARDIANA VPN: este equipo",
        &[Condicion::Bucle],
        true,
        peso::BUCLE,
    ));
    // DHCP v4: the request to the broadcast address, and its answer
    f.push(filtro(
        "GUARDIANA VPN: DHCP",
        Capa::ConectaV4,
        vec![
            Protocolo(UDP),
            PuertoLocal(68),
            PuertoRemoto(67),
            Remota(IpAddr::V4(Ipv4Addr::BROADCAST)),
        ],
        peso::RED,
    ));
    f.push(filtro(
        "GUARDIANA VPN: DHCP",
        Capa::AceptaV4,
        vec![Protocolo(UDP), PuertoLocal(68), PuertoRemoto(67)],
        peso::RED,
    ));
    // DHCP v6: to the relay agents' multicast addresses, from and to the link's addresses
    let (red, largo) = enlace();
    f.push(filtro(
        "GUARDIANA VPN: DHCPv6",
        Capa::ConectaV6,
        vec![
            Protocolo(UDP),
            Remota(IpAddr::V6(Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 1, 2))),
            Remota(IpAddr::V6(Ipv6Addr::new(0xff05, 0, 0, 0, 0, 0, 1, 3))),
            PuertoRemoto(547),
            LocalRed(red, largo),
            PuertoLocal(546),
        ],
        peso::RED,
    ));
    f.push(filtro(
        "GUARDIANA VPN: DHCPv6",
        Capa::AceptaV6,
        vec![
            Protocolo(UDP),
            RemotaRed(red, largo),
            PuertoRemoto(547),
            LocalRed(red, largo),
            PuertoLocal(546),
        ],
        peso::RED,
    ));
    // IPv6 neighbour discovery: router solicitation and advertisement, neighbour solicitation and advertisement,
    // redirect
    let icmp = |tipo: u8| vec![Protocolo(ICMP6), Icmp { tipo, codigo: 0 }];
    let con = |mut v: Vec<Condicion>, c: Condicion| {
        v.push(c);
        v
    };
    f.push(filtro(
        "GUARDIANA VPN: descubrimiento de vecinos",
        Capa::ConectaV6,
        con(
            icmp(133),
            Remota(IpAddr::V6(Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 2))),
        ),
        peso::RED,
    ));
    f.push(filtro(
        "GUARDIANA VPN: descubrimiento de vecinos",
        Capa::AceptaV6,
        con(icmp(134), RemotaRed(red, largo)),
        peso::RED,
    ));
    for tipo in [135, 136] {
        f.push(filtro(
            "GUARDIANA VPN: descubrimiento de vecinos",
            Capa::ConectaV6,
            icmp(tipo),
            peso::RED,
        ));
        f.push(filtro(
            "GUARDIANA VPN: descubrimiento de vecinos",
            Capa::AceptaV6,
            icmp(tipo),
            peso::RED,
        ));
    }
    f.push(filtro(
        "GUARDIANA VPN: descubrimiento de vecinos",
        Capa::AceptaV6,
        con(icmp(137), RemotaRed(red, largo)),
        peso::RED,
    ));
    // Hyper-V's virtual machines among themselves
    for capa in [Capa::SaleMac, Capa::EntraMac] {
        f.push(filtro(
            "GUARDIANA VPN: máquinas virtuales",
            capa,
            vec![Condicion::EntreMaquinasVirtuales],
            peso::RED,
        ));
    }
    // DNS: only to the tunnel's resolvers
    for capa in Capa::ALE {
        for protocolo in [UDP, TCP] {
            for d in dns.iter().filter(|d| d.is_ipv6() == capa.es_v6()) {
                f.push(Filtro {
                    nombre: "GUARDIANA VPN: DNS del túnel",
                    capa,
                    condiciones: vec![Protocolo(protocolo), PuertoRemoto(53), Remota(*d)],
                    permite: true,
                    peso: peso::DNS_SI,
                });
            }
            f.push(Filtro {
                nombre: "GUARDIANA VPN: otro DNS",
                capa,
                condiciones: vec![Protocolo(protocolo), PuertoRemoto(53)],
                permite: false,
                peso: peso::DNS_NO,
            });
        }
    }
    f.extend(en_ale(
        "GUARDIANA VPN: todo lo demás",
        &[],
        false,
        peso::TODO,
    ));
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dns() -> Vec<IpAddr> {
        vec![IpAddr::V4(Ipv4Addr::new(10, 66, 0, 1))]
    }

    #[test]
    fn todo_bloqueado_salvo_lo_que_hace_falta() {
        let p = plan(42, &dns());
        // bloquear todo, en las cuatro capas, con el peso más bajo
        let todo: Vec<_> = p.iter().filter(|f| f.condiciones.is_empty()).collect();
        assert_eq!(todo.len(), 4);
        assert!(todo.iter().all(|f| !f.permite && f.peso == peso::TODO));
        // el túnel y el servicio, permitidos en las cuatro capas, por encima de todo
        for c in [
            Condicion::Interfaz(42),
            Condicion::Servicio,
            Condicion::Bucle,
        ] {
            let n = p
                .iter()
                .filter(|f| f.condiciones == [c] && f.permite && f.peso > peso::DNS_SI)
                .count();
            assert_eq!(n, 4, "{c:?}");
        }
    }

    #[test]
    fn el_dns_solo_al_del_tunel() {
        let p = plan(42, &dns());
        let si: Vec<_> = p
            .iter()
            .filter(|f| f.nombre == "GUARDIANA VPN: DNS del túnel")
            .collect();
        // UDP y TCP, en las dos capas de IPv4; ninguno en IPv6 (el resolutor es de IPv4)
        assert_eq!(si.len(), 4);
        assert!(si.iter().all(|f| !f.capa.es_v6() && f.peso > peso::DNS_NO));
        let no = p
            .iter()
            .filter(|f| f.nombre == "GUARDIANA VPN: otro DNS")
            .count();
        assert_eq!(no, 8);
        // y el DNS bloqueado queda por encima de «todo lo demás» y por debajo del túnel y del servicio
        assert!(peso::DNS_NO > peso::TODO && peso::DNS_SI < peso::BUCLE);
    }

    #[test]
    fn la_red_de_debajo_sigue_funcionando() {
        let p = plan(42, &dns());
        let dhcp = p
            .iter()
            .filter(|f| f.nombre.starts_with("GUARDIANA VPN: DHCP"))
            .count();
        assert_eq!(dhcp, 4);
        let vecinos = p
            .iter()
            .filter(|f| f.nombre == "GUARDIANA VPN: descubrimiento de vecinos")
            .count();
        // 133, 134, 137 una vez; 135 y 136 en las dos capas
        assert_eq!(vecinos, 7);
        assert!(p.iter().all(|f| f.nombre.starts_with("GUARDIANA VPN: ")));
    }
}
