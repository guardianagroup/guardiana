//! The VPN's cut-off switch, as a plan: the Windows Filtering Platform filters that keep everything inside the tunnel
//! while it is on (phase 1 of docs/VPN.md; ronda 87: «kill switch con Windows Filtering Platform, no con rutas»).
//!
//! The same set of rules the WireGuard application for Windows applies (its `tunnel/firewall/rules.go`, MIT, read at
//! its source on 1 Nov 2026), so two independent sources agree on it, with the service's own permit narrowed further
//! (1.0.55, third review of 2 Nov 2026):
//!
//! - permit everything on the tunnel's own interface;
//! - permit GUARDIANA's service to the server's endpoint and nowhere else: UDP, the server's address and its port, in
//!   that address's family (the encrypted packets; WireGuardNT's sockets belong to the process that brings the adapter
//!   up, `PsGetCurrentProcess` in its driver's `Up`, read at its source on 2 Nov 2026). Until 1.0.55 the service was
//!   permitted everything, so whatever the routes sent outside the tunnel left: the home network's, and one pushed by a
//!   DHCP server that is not the house's (TunnelVision, CVE-2024-3661);
//! - permit what Home Mode serves to the house ([`HOGAR`]), inbound to the service only: an accepted connection carries
//!   its own answers, so nothing of the service's goes out to the house;
//! - permit this computer talking to itself (loopback: GUARDIANA's DNS lives there);
//! - permit DHCP (v4 and v6) and IPv6 neighbour discovery, so the real network keeps working underneath;
//! - permit Hyper-V's virtual machines talking to each other;
//! - block DNS (port 53) outside the tunnel, to anyone: the tunnel's resolver is reached on the tunnel's interface, and
//!   an exception for its address (until 1.0.55) let a route for 10.66.0.0/17 from a stranger's DHCP carry DNS out;
//! - block everything else, at the lowest weight.
//!
//! IPv6 (1.0.55): the tunnel has no IPv6 address (the sign-up gives an IPv4 one), and the IPv6 default route on its
//! adapter may not go in. Neither lets IPv6 out: off the tunnel's interface and loopback, this plan permits of IPv6 only
//! DHCPv6 between link addresses and neighbour discovery (ICMPv6 133 to 137), and nothing of the service's unless the
//! server's endpoint is itself IPv6 (then UDP to that endpoint alone). Everything else of IPv6 meets «todo lo demás»
//! (the test `nada_de_ipv6_fuera_del_tunel`).
//!
//! Pure data: no Windows call here, so it is tested on every system; the service turns each [`Filtro`] into a filter
//! of a dynamic WFP session (removed by Windows itself if the service ends).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

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

    /// The two connection-authorisation layers of one family: outgoing and accepted incoming.
    #[must_use]
    pub const fn de_la_familia(v6: bool) -> [Self; 2] {
        if v6 {
            [Self::ConectaV6, Self::AceptaV6]
        } else {
            [Self::ConectaV4, Self::AceptaV4]
        }
    }
}

/// UDP's protocol number.
pub const UDP: u8 = 17;
/// TCP's protocol number.
pub const TCP: u8 = 6;
/// ICMPv6's protocol number.
pub const ICMP6: u8 = 58;

/// What Home Mode serves to the house, as protocol and local port: DNS on port 53 (UDP and TCP), the panel (7443) and
/// the checker page (80), `guardiana_service::home`'s `lan_listen_addrs` and `lan_checker_addr` (the panel's tests
/// check they agree). The only connections the cut-off switch lets in to the service from outside the tunnel.
pub const HOGAR: [(u8, u16); 4] = [(UDP, 53), (TCP, 53), (TCP, 7443), (TCP, 80)];

/// One condition of a filter (all of a filter's conditions must hold; two of the same field mean either).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Condicion {
    /// `FWPM_CONDITION_IP_LOCAL_INTERFACE`: the interface, by its LUID.
    Interfaz(u64),
    /// `FWPM_CONDITION_ALE_APP_ID`: GUARDIANA's service itself, by its program file, filled in by the service. The
    /// encrypted packets carry it too: WireGuardNT's sockets belong to the process that brought the adapter up.
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
    /// A permit no lower sublayer can undo (`FWPM_FILTER_FLAG_CLEAR_ACTION_RIGHT`): only the encrypted packets to the
    /// server, so another firewall cannot cut the tunnel. Every other permit leaves the rest of Windows' firewall in
    /// charge: Home Mode's keeps the rule that lets in only the local network.
    pub firme: bool,
    /// Its weight in GUARDIANA's sublayer: the highest that matches decides.
    pub peso: u8,
}

/// The weights, from the highest: the service to the server, the tunnel's interface, loopback, the network underneath,
/// Home Mode, DNS outside the tunnel, everything else. WFP takes a weight given as one byte as a range from 0 to 15.
pub mod peso {
    /// GUARDIANA's service to the server's endpoint.
    pub const SERVICIO: u8 = 15;
    /// The tunnel's interface.
    pub const TUNEL: u8 = 14;
    /// Loopback.
    pub const BUCLE: u8 = 13;
    /// DHCP, neighbour discovery and Hyper-V.
    pub const RED: u8 = 12;
    /// What Home Mode serves to the house, inbound.
    pub const HOGAR: u8 = 11;
    /// DNS outside the tunnel.
    pub const DNS_NO: u8 = 10;
    /// Everything else.
    pub const TODO: u8 = 0;
}

// The order the plan relies on, checked when compiling: every permit above the DNS block (Home Mode's DNS is asked of
// the service, it never leaves), the DNS block above everything blocked, and all within WFP's range for one byte.
const _: () = assert!(
    peso::SERVICIO <= 15
        && peso::SERVICIO > peso::TUNEL
        && peso::TUNEL > peso::BUCLE
        && peso::BUCLE > peso::RED
        && peso::RED > peso::HOGAR
        && peso::HOGAR > peso::DNS_NO
        && peso::DNS_NO > peso::TODO
);

fn en_ale(nombre: &'static str, condiciones: &[Condicion], permite: bool, peso: u8) -> Vec<Filtro> {
    Capa::ALE
        .iter()
        .map(|&capa| Filtro {
            nombre,
            capa,
            condiciones: condiciones.to_vec(),
            permite,
            firme: false,
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
        firme: false,
        peso,
    }
}

/// `fe80::/10`, the link's own IPv6 addresses.
fn enlace() -> (Ipv6Addr, u8) {
    (Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 0), 10)
}

/// The whole plan for the tunnel's interface (`luid`) and the server's endpoint (`extremo`, where the encrypted packets
/// go: the one thing of the service's let out of the tunnel).
#[must_use]
pub fn plan(luid: u64, extremo: SocketAddr) -> Vec<Filtro> {
    use Condicion::{Icmp, LocalRed, Protocolo, PuertoLocal, PuertoRemoto, Remota, RemotaRed};
    let mut f = Vec::new();
    // The encrypted packets to the server and the server's answers: UDP, its address and its port, in its family's two
    // layers. Nothing else of the service's leaves outside the tunnel.
    for capa in Capa::de_la_familia(extremo.is_ipv6()) {
        f.push(Filtro {
            nombre: "GUARDIANA VPN: el servicio, solo al servidor",
            capa,
            condiciones: vec![
                Condicion::Servicio,
                Protocolo(UDP),
                Remota(extremo.ip()),
                PuertoRemoto(extremo.port()),
            ],
            permite: true,
            firme: true,
            peso: peso::SERVICIO,
        });
    }
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
    // Home Mode: the house asking the service for its DNS, its panel and its checker page, on the LAN's IPv4 address
    // (where Home Mode listens). Inbound only; the answers go back in the connection each one opened. Not firm: Windows'
    // own rule for Home Mode (only the local network, only a private one) still decides after this.
    for (protocolo, puerto) in HOGAR {
        f.push(filtro(
            "GUARDIANA VPN: Modo Hogar",
            Capa::AceptaV4,
            vec![
                Condicion::Servicio,
                Protocolo(protocolo),
                PuertoLocal(puerto),
            ],
            peso::HOGAR,
        ));
    }
    // DNS outside the tunnel, to anyone. «Todo lo demás» blocks it too; this says so by name in Windows' list of
    // filters, and keeps it blocked if a permit is ever added below it.
    for capa in Capa::ALE {
        for protocolo in [UDP, TCP] {
            f.push(Filtro {
                nombre: "GUARDIANA VPN: DNS fuera del túnel",
                capa,
                condiciones: vec![Protocolo(protocolo), PuertoRemoto(53)],
                permite: false,
                firme: false,
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// The tunnel's interface and the network card's, by their LUIDs.
    const TUNEL: u64 = 42;
    const TARJETA: u64 = 7;

    fn extremo() -> SocketAddr {
        "203.0.113.7:51820".parse().unwrap()
    }

    /// A connection as WFP sees it at one of the connection-authorisation layers, to ask the plan what it decides.
    #[derive(Debug, Clone, Copy)]
    struct Flujo {
        capa: Capa,
        interfaz: u64,
        servicio: bool,
        bucle: bool,
        protocolo: u8,
        local: IpAddr,
        puerto_local: u16,
        remota: IpAddr,
        puerto_remoto: u16,
    }

    /// A program (the service or another) going out through `interfaz` with `protocolo` to `remota`.
    fn sale(servicio: bool, interfaz: u64, protocolo: u8, remota: &str) -> Flujo {
        let r: SocketAddr = remota.parse().unwrap();
        let v6 = r.is_ipv6();
        Flujo {
            capa: Capa::de_la_familia(v6)[0],
            interfaz,
            servicio,
            bucle: false,
            protocolo,
            local: if v6 {
                "2001:db8:1::10".parse().unwrap()
            } else {
                "192.168.1.10".parse().unwrap()
            },
            puerto_local: 50000,
            remota: r.ip(),
            puerto_remoto: r.port(),
        }
    }

    /// `remota` coming in through `interfaz` to this computer's `puerto_local`, on a program (the service or another).
    fn entra(
        servicio: bool,
        interfaz: u64,
        protocolo: u8,
        remota: &str,
        puerto_local: u16,
    ) -> Flujo {
        let mut f = sale(servicio, interfaz, protocolo, remota);
        f.capa = Capa::de_la_familia(f.capa.es_v6())[1];
        f.puerto_local = puerto_local;
        f
    }

    /// WFP's fields, as `corte_vpn` fills them from the plan's conditions.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Campo {
        Interfaz,
        Programa,
        Marcas,
        Protocolo,
        PuertoLocal,
        PuertoRemoto,
        Remota,
        Local,
        Ethernet,
    }

    /// Whether `ip` is in the IPv6 network `red`/`largo`.
    fn en_red(ip: IpAddr, red: Ipv6Addr, largo: u8) -> bool {
        let IpAddr::V6(a) = ip else {
            return false;
        };
        let mascara = u128::MAX
            .checked_shl(128u32.saturating_sub(u32::from(largo)))
            .unwrap_or(0);
        (u128::from(a) & mascara) == (u128::from(red) & mascara)
    }

    /// The fields a condition looks at, each with whether `flujo` holds there (ICMP's type and code live in the port
    /// fields, as in WFP).
    fn campos(c: &Condicion, flujo: &Flujo) -> Vec<(Campo, bool)> {
        match *c {
            Condicion::Interfaz(luid) => vec![(Campo::Interfaz, flujo.interfaz == luid)],
            Condicion::Servicio => vec![(Campo::Programa, flujo.servicio)],
            Condicion::Bucle => vec![(Campo::Marcas, flujo.bucle)],
            Condicion::Protocolo(p) => vec![(Campo::Protocolo, flujo.protocolo == p)],
            Condicion::PuertoLocal(p) => vec![(Campo::PuertoLocal, flujo.puerto_local == p)],
            Condicion::PuertoRemoto(p) => vec![(Campo::PuertoRemoto, flujo.puerto_remoto == p)],
            Condicion::Remota(a) => vec![(Campo::Remota, flujo.remota == a)],
            Condicion::RemotaRed(red, largo) => {
                vec![(Campo::Remota, en_red(flujo.remota, red, largo))]
            }
            Condicion::LocalRed(red, largo) => {
                vec![(Campo::Local, en_red(flujo.local, red, largo))]
            }
            Condicion::Icmp { tipo, codigo } => vec![
                (Campo::PuertoLocal, flujo.puerto_local == u16::from(tipo)),
                (
                    Campo::PuertoRemoto,
                    flujo.puerto_remoto == u16::from(codigo),
                ),
            ],
            Condicion::EntreMaquinasVirtuales => vec![(Campo::Ethernet, false)],
        }
    }

    /// Whether `f` matches `flujo`: on its layer, and for every field it names, one of its conditions on that field
    /// holds (two of the same field mean either; all fields must hold).
    fn casa(f: &Filtro, flujo: &Flujo) -> bool {
        let vistos: Vec<(Campo, bool)> = f
            .condiciones
            .iter()
            .flat_map(|c| campos(c, flujo))
            .collect();
        f.capa == flujo.capa
            && vistos
                .iter()
                .all(|(campo, _)| vistos.iter().any(|(otro, si)| otro == campo && *si))
    }

    /// What the plan decides for `flujo` in GUARDIANA's sublayer: the highest weight among the filters that match, as
    /// WFP decides inside one sublayer (`None` when none does). A block here is final for the whole of Windows.
    fn decide(p: &[Filtro], flujo: &Flujo) -> Option<bool> {
        p.iter()
            .filter(|f| casa(f, flujo))
            .max_by_key(|f| f.peso)
            .map(|f| f.permite)
    }

    #[test]
    fn todo_bloqueado_salvo_lo_que_hace_falta() {
        let p = plan(TUNEL, extremo());
        // bloquear todo, en las cuatro capas, con el peso más bajo
        let todo: Vec<_> = p.iter().filter(|f| f.condiciones.is_empty()).collect();
        assert_eq!(todo.len(), 4);
        assert!(todo.iter().all(|f| !f.permite && f.peso == peso::TODO));
        // el túnel y el propio equipo, permitidos en las cuatro capas, por encima de la red de debajo
        for c in [Condicion::Interfaz(TUNEL), Condicion::Bucle] {
            let n = p
                .iter()
                .filter(|f| f.condiciones == [c] && f.permite && f.peso > peso::RED)
                .count();
            assert_eq!(n, 4, "{c:?}");
        }
        // el servicio nunca va solo: siempre con lo que lo limita (el servidor, o lo que sirve a la casa)
        assert!(p.iter().all(|f| f.condiciones != [Condicion::Servicio]));
        // en un mismo peso, o todo permite o todo bloquea; y todos dentro de lo que WFP acepta en un byte
        for f in &p {
            assert!(f.peso <= 15, "{}", f.nombre);
            assert!(
                p.iter()
                    .filter(|g| g.peso == f.peso)
                    .all(|g| g.permite == f.permite),
                "{}",
                f.nombre
            );
        }
        assert!(p.iter().all(|f| f.nombre.starts_with("GUARDIANA VPN: ")));
    }

    /// 1.0.55: of the service, outside the tunnel, only the encrypted packets to the server: UDP, its address and its
    /// port. Not the server's other ports, not the house's network, not a route a stranger's DHCP pushed.
    #[test]
    fn el_servicio_solo_hacia_el_servidor() {
        let p = plan(TUNEL, extremo());
        // los paquetes encriptados, de ida y de vuelta
        assert_eq!(
            decide(&p, &sale(true, TARJETA, UDP, "203.0.113.7:51820")),
            Some(true)
        );
        assert_eq!(
            decide(&p, &entra(true, TARJETA, UDP, "203.0.113.7:51820", 50000)),
            Some(true)
        );
        // nada más del servicio fuera del túnel: otro puerto u otro protocolo del servidor, otra dirección, la red de
        // casa, el resolutor del túnel por una ruta de fuera (10.66.0.0/17 de un DHCP ajeno), cualquier web
        for (protocolo, a) in [
            (UDP, "203.0.113.7:51821"),
            (TCP, "203.0.113.7:51820"),
            (TCP, "203.0.113.7:443"),
            (UDP, "203.0.113.8:51820"),
            (TCP, "192.168.1.20:80"),
            (UDP, "192.168.1.20:5353"),
            (UDP, "10.66.0.1:53"),
            (TCP, "198.51.100.9:443"),
        ] {
            assert_eq!(
                decide(&p, &sale(true, TARJETA, protocolo, a)),
                Some(false),
                "{protocolo} {a}"
            );
        }
        // otro programa, ni hacia el servidor
        assert_eq!(
            decide(&p, &sale(false, TARJETA, UDP, "203.0.113.7:51820")),
            Some(false)
        );
        // por el túnel, el servicio va a donde haga falta: su DNS al resolutor del túnel, sus comprobaciones
        assert_eq!(
            decide(&p, &sale(true, TUNEL, UDP, "10.66.0.1:53")),
            Some(true)
        );
        assert_eq!(
            decide(&p, &sale(true, TUNEL, TCP, "198.51.100.9:443")),
            Some(true)
        );
        // ese permiso es el único que una subcapa de debajo no puede deshacer, en las dos capas de su familia
        let firmes: Vec<_> = p.iter().filter(|f| f.firme).collect();
        assert_eq!(firmes.len(), 2);
        assert!(firmes
            .iter()
            .all(|f| f.permite && !f.capa.es_v6() && f.peso == peso::SERVICIO));
    }

    /// A server reached over IPv6: the same permit, in IPv6's layers, and nothing of the service's in IPv4's.
    #[test]
    fn con_un_servidor_de_ipv6() {
        let p = plan(TUNEL, "[2001:db8::7]:51820".parse().unwrap());
        assert_eq!(
            decide(&p, &sale(true, TARJETA, UDP, "[2001:db8::7]:51820")),
            Some(true)
        );
        assert_eq!(
            decide(&p, &entra(true, TARJETA, UDP, "[2001:db8::7]:51820", 50000)),
            Some(true)
        );
        assert_eq!(
            decide(&p, &sale(true, TARJETA, UDP, "[2001:db8::8]:51820")),
            Some(false)
        );
        assert_eq!(
            decide(&p, &sale(true, TARJETA, UDP, "203.0.113.7:51820")),
            Some(false)
        );
        let firmes: Vec<_> = p.iter().filter(|f| f.firme).collect();
        assert_eq!(firmes.len(), 2);
        assert!(firmes.iter().all(|f| f.capa.es_v6()));
    }

    /// The tunnel has no IPv6 address and its IPv6 default route may not go in: still no IPv6 leaves outside the
    /// tunnel. Off the tunnel and loopback, IPv6 has only DHCPv6 between link addresses and neighbour discovery; the
    /// service gets nothing of IPv6 when the server is reached over IPv4.
    #[test]
    fn nada_de_ipv6_fuera_del_tunel() {
        let p = plan(TUNEL, extremo());
        for servicio in [true, false] {
            for protocolo in [UDP, TCP] {
                for a in [
                    "[2001:4860:4860::8888]:53",
                    "[2606:4700::1111]:443",
                    "[2001:db8::7]:51820",
                    "[fe80::1]:80",
                ] {
                    assert_eq!(
                        decide(&p, &sale(servicio, TARJETA, protocolo, a)),
                        Some(false),
                        "sale {servicio} {protocolo} {a}"
                    );
                    for puerto in [53, 7443, 80, 50000] {
                        assert_eq!(
                            decide(&p, &entra(servicio, TARJETA, protocolo, a, puerto)),
                            Some(false),
                            "entra {servicio} {protocolo} {a} {puerto}"
                        );
                    }
                }
            }
            // ICMPv6 que no es del descubrimiento de vecinos: un eco (128)
            let mut eco = sale(servicio, TARJETA, ICMP6, "[2001:4860:4860::8888]:0");
            eco.puerto_local = 128;
            assert_eq!(decide(&p, &eco), Some(false));
        }
        // lo único de IPv6 permitido fuera del túnel y del propio equipo: DHCPv6 y el descubrimiento de vecinos
        for f in p.iter().filter(|f| f.permite && f.capa.es_v6()) {
            let dentro = f.condiciones.contains(&Condicion::Interfaz(TUNEL))
                || f.condiciones.contains(&Condicion::Bucle);
            assert!(
                dentro
                    || f.nombre == "GUARDIANA VPN: DHCPv6"
                    || f.nombre == "GUARDIANA VPN: descubrimiento de vecinos",
                "{}",
                f.nombre
            );
        }
        // y eso sigue pasando: la red de debajo funciona
        let mut vecino = sale(false, TARJETA, ICMP6, "[fe80::1]:0");
        vecino.puerto_local = 135;
        assert_eq!(decide(&p, &vecino), Some(true));
    }

    /// DNS on the tunnel and to this computer itself (GUARDIANA's resolver), yes; outside the tunnel, never, not even
    /// to the tunnel's resolver's address: until 1.0.55 «DNS del túnel» let it out with no interface of its own, and a
    /// route for 10.66.0.0/17 from a stranger's DHCP would have carried it to them.
    #[test]
    fn el_dns_solo_por_el_tunel() {
        let p = plan(TUNEL, extremo());
        for protocolo in [UDP, TCP] {
            for servicio in [true, false] {
                for a in ["10.66.0.1:53", "192.168.1.1:53", "8.8.8.8:53"] {
                    assert_eq!(
                        decide(&p, &sale(servicio, TARJETA, protocolo, a)),
                        Some(false),
                        "{servicio} {protocolo} {a}"
                    );
                }
                assert_eq!(
                    decide(&p, &sale(servicio, TUNEL, protocolo, "10.66.0.1:53")),
                    Some(true)
                );
                let mut propio = sale(servicio, 1, protocolo, "127.0.0.1:53");
                propio.bucle = true;
                assert_eq!(decide(&p, &propio), Some(true));
            }
        }
        let dns: Vec<_> = p
            .iter()
            .filter(|f| f.nombre == "GUARDIANA VPN: DNS fuera del túnel")
            .collect();
        assert_eq!(dns.len(), 8);
        assert!(dns.iter().all(|f| !f.permite && f.peso == peso::DNS_NO));
    }

    /// Home Mode keeps serving the house through the cut-off switch: its DNS, its panel and its checker page, coming in
    /// to the service only. The answers go back in the same connection; nothing of the service's goes out to the house.
    #[test]
    fn el_modo_hogar_sigue_sirviendo_a_la_casa() {
        let p = plan(TUNEL, extremo());
        assert_eq!(HOGAR, [(UDP, 53), (TCP, 53), (TCP, 7443), (TCP, 80)]);
        for (protocolo, puerto) in HOGAR {
            for desde in ["192.168.1.20:51515", "192.168.1.20:53"] {
                assert_eq!(
                    decide(&p, &entra(true, TARJETA, protocolo, desde, puerto)),
                    Some(true),
                    "{protocolo} {puerto} {desde}"
                );
            }
            // a otro programa en ese puerto, no
            assert_eq!(
                decide(
                    &p,
                    &entra(false, TARJETA, protocolo, "192.168.1.20:51515", puerto)
                ),
                Some(false)
            );
            // ni por IPv6: el Modo Hogar escucha en la dirección IPv4 de la red de casa
            assert_eq!(
                decide(
                    &p,
                    &entra(true, TARJETA, protocolo, "[fe80::20]:51515", puerto)
                ),
                Some(false)
            );
            // y el servicio no abre nada hacia la casa
            let mut fuera = sale(true, TARJETA, protocolo, "192.168.1.20:51515");
            fuera.puerto_local = puerto;
            assert_eq!(decide(&p, &fuera), Some(false));
        }
        // otros puertos del servicio, no
        assert_eq!(
            decide(&p, &entra(true, TARJETA, TCP, "192.168.1.20:51515", 8080)),
            Some(false)
        );
        assert_eq!(
            decide(&p, &entra(true, TARJETA, UDP, "192.168.1.20:51515", 7443)),
            Some(false)
        );
        // permisos que la regla del Modo Hogar en el cortafuegos de Windows (solo la red local) aún puede negar
        let hogar: Vec<_> = p
            .iter()
            .filter(|f| f.nombre == "GUARDIANA VPN: Modo Hogar")
            .collect();
        assert_eq!(hogar.len(), 4);
        assert!(hogar
            .iter()
            .all(|f| f.permite && !f.firme && f.capa == Capa::AceptaV4));
    }

    #[test]
    fn la_red_de_debajo_sigue_funcionando() {
        let p = plan(TUNEL, extremo());
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
        // DHCP de verdad: la petición a la dirección de difusión
        let mut pide = sale(false, TARJETA, UDP, "255.255.255.255:67");
        pide.puerto_local = 68;
        assert_eq!(decide(&p, &pide), Some(true));
    }
}
