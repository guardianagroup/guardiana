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
//! - block everything else, at the lowest weight;
//! - block everything Windows forwards, at the lowest weight, but what goes into the tunnel's interface or comes out of
//!   it (1.0.56, below).
//!
//! Forwarding (1.0.56, review of 5 Nov 2026): what Windows forwards from one of its interfaces to another (WSL2,
//! Hyper-V's Default Switch, Windows Sandbox, Docker, the shared connection or the mobile hotspot, through Windows' own
//! NAT) never meets the connection-authorisation layers: it is classified in the forwarding layers,
//! `FWPM_LAYER_IPFORWARD_V4` and `_V6`, once forwarding is on for the interfaces, as Hyper-V's network service and ICS
//! turn it on for theirs (Microsoft, «WFP layer requirements and restrictions», «Forwarding layer»; WinNAT itself
//! translates there). Until 1.0.56 nothing of the plan was there, so a virtual machine's packets went out by the network
//! card, with the real address, wherever a route sent them: always with the tunnel down (strict mode reconnecting, the
//! computer starting, the service dead or stopped), and with it up to the house's network, by a route a stranger's DHCP
//! pushes (TunnelVision) and in IPv6 (the tunnel has none). WireGuard's rules leave forwarding alone (`rules.go` filters
//! only the connection-authorisation layers); VpnHood's Windows kill switch blocks it and permits the same two ways this
//! plan does (`WindowsVpnKillSwitch.cs`, read at its source on 5 Nov 2026); and simplewall's users found that a block
//! there cuts the mobile hotspot, ICS and NAT'd virtual switches (its `src/wfp.c` and its «Allow IP forwarding», read the
//! same day): that traffic does pass there. The switch of always blocks it too, for those same routes with the tunnel
//! up; into the tunnel and out of it, forwarding is permitted, so virtual machines still go through the tunnel.
//!
//! Which condition names the interface a forwarded packet leaves by: of the conditions Microsoft lists for the forwarding
//! layers («Filtering conditions available at each filtering layer»), `FWPM_CONDITION_IP_FORWARD_INTERFACE`, «the LUID
//! for the network interface on which the packet being forwarded is to be sent out» (`FWP_UINT64`), and
//! `FWPM_CONDITION_DESTINATION_INTERFACE_INDEX`, the same interface by the index the stack gives it (`FWP_UINT32`). The
//! plan takes the LUID ([`Condicion::Salida`]): it names the tunnel in every other layer and is what the adapter gives,
//! with nothing to translate; an index would be one more lookup (`ConvertInterfaceLuidToIndex`), another number for the
//! same adapter that could disagree with it. Not `FWPM_CONDITION_IP_LOCAL_INTERFACE`, the tunnel's condition in the
//! connection-authorisation layers: in the forwarding layers, which have no arrival-interface field, it is the interface
//! the packet arrived on (so VpnHood uses it there, and so luth.io's tests of these layers on a real Windows found it,
//! «Five ways to not firewall a Windows container», Aug 2026), and the plan uses it there for the answers that come out
//! of the tunnel ([`Condicion::Llegada`]). They need their own permit when Windows forwards them; when WinNAT translates
//! them it hands them on with `FwpsInjectForwardAsync0`, which no WFP layer sees again (Microsoft, its remarks).
//!
//! What a forwarding block cannot reach: packets the Hyper-V virtual switch translates itself, without Windows' IP stack
//! forwarding them (Windows containers on a NAT network on Windows Server, ephemerd's investigation of Aug 2026; Mullvad
//! saw WSL's NAT'd packets only as Ethernet frames, its blog of 30 Sep 2020, and blocks WSL with Hyper-V's own firewall
//! on Windows 11 22H2 and later), and machines on an external virtual switch, which are the network's, not this
//! computer's.
//!
//! IPv6 (1.0.55): the tunnel has no IPv6 address (the sign-up gives an IPv4 one), and the IPv6 default route on its
//! adapter may not go in. Neither lets IPv6 out: off the tunnel's interface and loopback, this plan permits of IPv6 only
//! DHCPv6 between link addresses and neighbour discovery (ICMPv6 133 to 137), and nothing of the service's unless the
//! server's endpoint is itself IPv6 (then UDP to that endpoint alone). Everything else of IPv6 meets «todo lo demás»
//! (the test `nada_de_ipv6_fuera_del_tunel`).
//!
//! Strict mode ([`estricto`]; ronda 91, docs/VPN.md «Modo estricto»): the same filters, split by how long each must
//! live in Windows. Everything but the tunnel's own interface goes in as persistent filters, which stay when the service
//! crashes, stops for an update or the computer restarts, with one more permit for the service: a new sign-up with its
//! server (TCP to its HTTPS), since with the tunnel down nothing else can reach it. The blocks (the forwarding ones too)
//! and the network underneath go in once more as boot-time filters, for the computer's start. The tunnel's own permit
//! ([`del_tunel`], with what is forwarded into the tunnel and out of it) stays in a dynamic session of the service, so it
//! goes with the adapter: with the tunnel down nothing forwarded leaves at all. The house's inbound permits stay too:
//! they answer the house's devices on the connections those open, and carry none of this computer's own traffic out (the
//! test `sin_excepcion_de_la_red_local`).
//!
//! Pure data: no Windows call here, so it is tested on every system; the service turns each [`Filtro`] into a filter
//! of a dynamic WFP session (removed by Windows itself if the service ends), or of strict mode's persistent and
//! boot-time sets.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

/// Where a filter applies: WFP's connection-authorisation layers (outgoing connections and accepted incoming ones),
/// for each IP version; its forwarding layers (what Windows forwards from one interface to another); and the Ethernet
/// layers for Hyper-V.
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
    /// `FWPM_LAYER_IPFORWARD_V4`.
    ReenviaV4,
    /// `FWPM_LAYER_IPFORWARD_V6`.
    ReenviaV6,
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

    /// The two forwarding layers.
    pub const REENVIO: [Self; 2] = [Self::ReenviaV4, Self::ReenviaV6];

    /// Whether the layer is IPv6's.
    #[must_use]
    pub const fn es_v6(self) -> bool {
        matches!(self, Self::ConectaV6 | Self::AceptaV6 | Self::ReenviaV6)
    }

    /// Whether the layer is a forwarding one.
    #[must_use]
    pub const fn es_reenvio(self) -> bool {
        matches!(self, Self::ReenviaV4 | Self::ReenviaV6)
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
    /// `FWPM_CONDITION_IP_LOCAL_INTERFACE`, in the connection-authorisation layers: the interface of this computer's
    /// own address, by its LUID.
    Interfaz(u64),
    /// `FWPM_CONDITION_IP_FORWARD_INTERFACE`, in the forwarding layers: the interface a forwarded packet is sent out on,
    /// by its LUID.
    Salida(u64),
    /// `FWPM_CONDITION_IP_LOCAL_INTERFACE`, in the forwarding layers: there, the interface a forwarded packet arrived on,
    /// by its LUID.
    Llegada(u64),
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
    /// The tunnel's interface, and what is forwarded through it.
    pub const TUNEL: u8 = 14;
    /// Loopback.
    pub const BUCLE: u8 = 13;
    /// DHCP, neighbour discovery and Hyper-V.
    pub const RED: u8 = 12;
    /// What Home Mode serves to the house, inbound.
    pub const HOGAR: u8 = 11;
    /// DNS outside the tunnel.
    pub const DNS_NO: u8 = 10;
    /// Everything else, and everything else forwarded.
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
    let mut f = al_servidor(extremo);
    f.extend(del_tunel(luid));
    f.extend(del_equipo());
    f.extend(de_la_red());
    f.extend(maquinas_virtuales());
    f.extend(del_hogar());
    f.extend(bloqueos());
    f
}

/// Where strict mode lets GUARDIANA's service reach its server outside the tunnel (docs/VPN.md, «Modo estricto»): the
/// encrypted packets' endpoint and the server's HTTPS, where a new sign-up goes while the tunnel is down.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Servidor {
    /// Where the encrypted packets go (UDP); `None` until a sign-up has said it.
    pub extremo: Option<SocketAddr>,
    /// Where a new sign-up goes (TCP, the server's HTTPS): with the block on, no name resolves outside the tunnel.
    pub alta: Option<SocketAddr>,
}

/// Strict mode's block (docs/VPN.md, «Modo estricto»), by how long each part lives in Windows. The tunnel's own
/// interface is not here: its permit ([`del_tunel`]) lives in a dynamic session of the service, so it goes with the
/// adapter, which goes with the service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Estricto {
    /// Persistent filters: they stay through the service's crash, an update and a restart of the computer, until
    /// GUARDIANA removes them. All of [`plan`] but the tunnel's interface, with the server known so far, and the
    /// service's sign-up to its server.
    pub persistentes: Vec<Filtro>,
    /// Boot-time filters, enforced from the computer's start until Windows' filtering service has loaded the persistent
    /// ones: the blocks (of what is forwarded too), this computer talking to itself, and what keeps the network
    /// underneath working (DHCP and IPv6 neighbour discovery). Nothing of the service's: it starts later.
    pub arranque: Vec<Filtro>,
}

/// Strict mode's block for the server known so far (`servidor`). With nothing known, nothing of the service's leaves
/// either.
#[must_use]
pub fn estricto(servidor: Servidor) -> Estricto {
    let mut persistentes = servidor.extremo.map(al_servidor).unwrap_or_default();
    persistentes.extend(servidor.alta.map(al_alta));
    persistentes.extend(del_equipo());
    persistentes.extend(de_la_red());
    persistentes.extend(maquinas_virtuales());
    persistentes.extend(del_hogar());
    persistentes.extend(bloqueos());
    let mut arranque = del_equipo();
    arranque.extend(de_la_red());
    arranque.extend(bloqueos());
    Estricto {
        persistentes,
        arranque,
    }
}

/// Everything on the tunnel's interface (`luid`): in the four connection-authorisation layers, and what Windows forwards
/// into the tunnel or out of it.
#[must_use]
pub fn del_tunel(luid: u64) -> Vec<Filtro> {
    let mut f = en_ale(
        "GUARDIANA VPN: el túnel",
        &[Condicion::Interfaz(luid)],
        true,
        peso::TUNEL,
    );
    f.extend(reenvio_del_tunel(luid));
    f
}

/// What Windows forwards through the tunnel (`luid`), in both forwarding layers: a virtual machine's or a shared
/// device's packets sent out on the tunnel's interface, and what comes back out of it to them. Nothing else forwarded:
/// what would leave by the network card meets [`bloqueos`]. A packet that arrived through the tunnel was the server's
/// (WireGuard takes only its authenticated peer's), so passing it on carries nothing of this computer's, or of its
/// machines', outside the tunnel; and Windows forwards from the tunnel only when forwarding is on for its interface.
fn reenvio_del_tunel(luid: u64) -> Vec<Filtro> {
    let mut f = Vec::new();
    for capa in Capa::REENVIO {
        f.push(filtro(
            "GUARDIANA VPN: reenviado al túnel",
            capa,
            vec![Condicion::Salida(luid)],
            peso::TUNEL,
        ));
        f.push(filtro(
            "GUARDIANA VPN: reenviado desde el túnel",
            capa,
            vec![Condicion::Llegada(luid)],
            peso::TUNEL,
        ));
    }
    f
}

/// The encrypted packets to the server and the server's answers: UDP, its address and its port, in its family's two
/// layers. Nothing else of the service's leaves outside the tunnel (but strict mode's sign-up, [`al_alta`]).
fn al_servidor(extremo: SocketAddr) -> Vec<Filtro> {
    use Condicion::{Protocolo, PuertoRemoto, Remota};
    Capa::de_la_familia(extremo.is_ipv6())
        .into_iter()
        .map(|capa| Filtro {
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
        })
        .collect()
}

/// Strict mode's only other way out for the service: a new sign-up with its server while the tunnel is down (TCP to the
/// server's HTTPS, its address and port). Outgoing only, the answers come back in the same connection; not firm: the
/// rest of Windows' firewall still decides after it.
fn al_alta(alta: SocketAddr) -> Filtro {
    use Condicion::{Protocolo, PuertoRemoto, Remota};
    Filtro {
        nombre: "GUARDIANA VPN: el servicio, el alta en su servidor",
        capa: Capa::de_la_familia(alta.is_ipv6())[0],
        condiciones: vec![
            Condicion::Servicio,
            Protocolo(TCP),
            Remota(alta.ip()),
            PuertoRemoto(alta.port()),
        ],
        permite: true,
        firme: false,
        peso: peso::SERVICIO,
    }
}

/// This computer talking to itself (loopback: GUARDIANA's DNS lives there), in the four layers.
fn del_equipo() -> Vec<Filtro> {
    en_ale(
        "GUARDIANA VPN: este equipo",
        &[Condicion::Bucle],
        true,
        peso::BUCLE,
    )
}

/// The network underneath: DHCP v4 and v6, and IPv6 neighbour discovery.
fn de_la_red() -> Vec<Filtro> {
    use Condicion::{Icmp, LocalRed, Protocolo, PuertoLocal, PuertoRemoto, Remota, RemotaRed};
    let mut f = Vec::new();
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
    f
}

/// Hyper-V's virtual machines among themselves (the Ethernet layers).
fn maquinas_virtuales() -> Vec<Filtro> {
    [Capa::SaleMac, Capa::EntraMac]
        .into_iter()
        .map(|capa| {
            filtro(
                "GUARDIANA VPN: máquinas virtuales",
                capa,
                vec![Condicion::EntreMaquinasVirtuales],
                peso::RED,
            )
        })
        .collect()
}

/// Home Mode: the house asking the service for its DNS, its panel and its checker page, on the LAN's IPv4 address
/// (where Home Mode listens). Inbound only; the answers go back in the connection each one opened. Not firm: Windows'
/// own rule for Home Mode (only the local network, only a private one) still decides after this.
fn del_hogar() -> Vec<Filtro> {
    HOGAR
        .into_iter()
        .map(|(protocolo, puerto)| {
            filtro(
                "GUARDIANA VPN: Modo Hogar",
                Capa::AceptaV4,
                vec![
                    Condicion::Servicio,
                    Condicion::Protocolo(protocolo),
                    Condicion::PuertoLocal(puerto),
                ],
                peso::HOGAR,
            )
        })
        .collect()
}

/// DNS outside the tunnel, to anyone, and everything else. «Todo lo demás» blocks DNS too; its own filter says so by
/// name in Windows' list of filters, and keeps it blocked if a permit is ever added below it. And everything Windows
/// forwards, in both forwarding layers (1.0.56): only the tunnel's own permit ([`del_tunel`]) lets anything forwarded
/// through, into the tunnel and out of it.
fn bloqueos() -> Vec<Filtro> {
    let mut f = Vec::new();
    for capa in Capa::ALE {
        for protocolo in [UDP, TCP] {
            f.push(Filtro {
                nombre: "GUARDIANA VPN: DNS fuera del túnel",
                capa,
                condiciones: vec![Condicion::Protocolo(protocolo), Condicion::PuertoRemoto(53)],
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
    f.extend(Capa::REENVIO.into_iter().map(|capa| Filtro {
        nombre: "GUARDIANA VPN: lo reenviado fuera del túnel",
        capa,
        condiciones: Vec::new(),
        permite: false,
        firme: false,
        peso: peso::TODO,
    }));
    f
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// The tunnel's interface and the network card's, by their LUIDs.
    const TUNEL: u64 = 42;
    const TARJETA: u64 = 7;
    /// A virtual machine's or a shared device's interface on this computer (WSL's, the Default Switch's, the hotspot's).
    const VIRTUAL: u64 = 9;
    /// Another adapter's interface (a tunnel of before, or another program's).
    const OTRO: u64 = 43;

    fn extremo() -> SocketAddr {
        "203.0.113.7:51820".parse().unwrap()
    }

    /// A connection as WFP sees it at one of the connection-authorisation layers, or a packet at one of the forwarding
    /// layers, to ask the plan what it decides.
    #[derive(Debug, Clone, Copy)]
    struct Flujo {
        capa: Capa,
        /// `FWPM_CONDITION_IP_LOCAL_INTERFACE`: the interface of the local address; forwarded, the one it arrived on.
        interfaz: u64,
        /// `FWPM_CONDITION_IP_FORWARD_INTERFACE`: forwarded, the interface it leaves by.
        salida: u64,
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
            salida: interfaz,
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

    /// What Windows forwards: a virtual machine's or a shared device's packet to the internet (or its answer), of IPv4
    /// or IPv6, that arrived on `llegada` and leaves by `salida`. The program is nobody's: forwarding is not a socket's.
    fn reenvia(v6: bool, llegada: u64, salida: u64) -> Flujo {
        let a = if v6 {
            "[2606:4700::1111]:443"
        } else {
            "1.1.1.1:443"
        };
        let mut f = sale(false, llegada, TCP, a);
        f.capa = if v6 { Capa::ReenviaV6 } else { Capa::ReenviaV4 };
        f.salida = salida;
        f
    }

    /// Forwarded packets of every kind: each family, between every two of the network card, the tunnel, a virtual
    /// machine's interface and another adapter's.
    fn reenviados() -> Vec<Flujo> {
        let mut v = Vec::new();
        for v6 in [false, true] {
            for llegada in [TARJETA, TUNEL, VIRTUAL, OTRO] {
                for salida in [TARJETA, TUNEL, VIRTUAL, OTRO] {
                    if llegada != salida {
                        v.push(reenvia(v6, llegada, salida));
                    }
                }
            }
        }
        v
    }

    /// WFP's fields, as `corte_vpn` fills them from the plan's conditions.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Campo {
        Interfaz,
        Salida,
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
            // the same field: in the forwarding layers, where the packet arrived
            Condicion::Interfaz(luid) | Condicion::Llegada(luid) => {
                vec![(Campo::Interfaz, flujo.interfaz == luid)]
            }
            Condicion::Salida(luid) => vec![(Campo::Salida, flujo.salida == luid)],
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
        // bloquear todo, en las cuatro capas de las conexiones y en las dos del reenvío, con el peso más bajo
        let todo: Vec<_> = p.iter().filter(|f| f.condiciones.is_empty()).collect();
        assert_eq!(todo.len(), 6);
        assert!(todo.iter().all(|f| !f.permite && f.peso == peso::TODO));
        for capa in Capa::ALE.into_iter().chain(Capa::REENVIO) {
            assert_eq!(
                todo.iter().filter(|f| f.capa == capa).count(),
                1,
                "{capa:?}"
            );
        }
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
        // lo único de IPv6 permitido fuera del túnel y del propio equipo: DHCPv6 y el descubrimiento de vecinos (lo
        // reenviado, solo hacia el túnel o desde él)
        for f in p.iter().filter(|f| f.permite && f.capa.es_v6()) {
            let dentro = f.condiciones.contains(&Condicion::Interfaz(TUNEL))
                || f.condiciones.contains(&Condicion::Salida(TUNEL))
                || f.condiciones.contains(&Condicion::Llegada(TUNEL))
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

    /// The server's HTTPS in the tests, where strict mode's new sign-up goes.
    fn alta() -> SocketAddr {
        "203.0.113.7:443".parse().unwrap()
    }

    /// The server as strict mode knows it once a sign-up answered.
    fn conocido() -> Servidor {
        Servidor {
            extremo: Some(extremo()),
            alta: Some(alta()),
        }
    }

    /// Flows of every kind: of the service or of another program, out or in, through the network card or the tunnel;
    /// and what Windows forwards.
    fn flujos() -> Vec<Flujo> {
        let mut v = reenviados();
        for servicio in [true, false] {
            for (protocolo, a) in [
                (UDP, "203.0.113.7:51820"),
                (UDP, "203.0.113.7:51821"),
                (TCP, "203.0.113.8:443"),
                (TCP, "192.168.1.20:80"),
                (UDP, "192.168.1.1:53"),
                (UDP, "10.66.0.1:53"),
                (TCP, "198.51.100.9:443"),
                (UDP, "8.8.8.8:53"),
                (TCP, "[2606:4700::1111]:443"),
                (UDP, "[2001:4860:4860::8888]:53"),
            ] {
                for interfaz in [TARJETA, TUNEL] {
                    v.push(sale(servicio, interfaz, protocolo, a));
                    for puerto in [53, 80, 7443, 50000] {
                        v.push(entra(servicio, interfaz, protocolo, a, puerto));
                    }
                }
            }
        }
        v
    }

    /// Strict mode with the tunnel up decides as the switch of always (the block in Windows and the tunnel's own permit
    /// share one sublayer): the only thing it adds is the service's new sign-up with its server, going out.
    #[test]
    fn el_modo_estricto_conectado_decide_como_el_corte_de_siempre() {
        let normal = plan(TUNEL, extremo());
        let mut conectado = estricto(conocido()).persistentes;
        conectado.extend(del_tunel(TUNEL));
        for f in flujos() {
            assert_eq!(decide(&conectado, &f), decide(&normal, &f), "{f:?}");
        }
        assert_eq!(
            decide(&conectado, &sale(true, TARJETA, TCP, "203.0.113.7:443")),
            Some(true)
        );
        assert_eq!(
            decide(&normal, &sale(true, TARJETA, TCP, "203.0.113.7:443")),
            Some(false)
        );
        assert_eq!(
            decide(&conectado, &sale(false, TARJETA, TCP, "203.0.113.7:443")),
            Some(false)
        );
        assert_eq!(
            decide(
                &conectado,
                &entra(true, TARJETA, TCP, "203.0.113.7:443", 50000)
            ),
            Some(false)
        );
        // y el único permiso firme sigue siendo el de los paquetes encriptados
        assert_eq!(conectado.iter().filter(|f| f.firme).count(), 2);
    }

    /// Strict mode with the tunnel down (the server does not answer, the service crashed or stopped for an update, the
    /// computer restarting): nothing leaves but the service's way to its server, and the house still reaches Home Mode.
    #[test]
    fn sin_tunel_nada_sale_salvo_el_camino_al_servidor() {
        let p = estricto(conocido()).persistentes;
        assert_eq!(
            decide(&p, &sale(true, TARJETA, UDP, "203.0.113.7:51820")),
            Some(true)
        );
        assert_eq!(
            decide(&p, &entra(true, TARJETA, UDP, "203.0.113.7:51820", 50000)),
            Some(true)
        );
        assert_eq!(
            decide(&p, &sale(true, TARJETA, TCP, "203.0.113.7:443")),
            Some(true)
        );
        for servicio in [true, false] {
            for (protocolo, a) in [
                (TCP, "198.51.100.9:443"),
                (UDP, "198.51.100.9:443"),
                (UDP, "8.8.8.8:53"),
                (TCP, "1.1.1.1:53"),
                (UDP, "10.66.0.1:53"),
                (TCP, "192.168.1.1:80"),
                (UDP, "192.168.1.20:5353"),
                (TCP, "[2606:4700::1111]:443"),
                (UDP, "[fe80::1]:53"),
            ] {
                // por la tarjeta, y por la interfaz de un túnel que ya no está
                for interfaz in [TARJETA, TUNEL] {
                    assert_eq!(
                        decide(&p, &sale(servicio, interfaz, protocolo, a)),
                        Some(false),
                        "{servicio} {interfaz} {protocolo} {a}"
                    );
                }
            }
        }
        // otro programa, ni al servidor
        assert_eq!(
            decide(&p, &sale(false, TARJETA, UDP, "203.0.113.7:51820")),
            Some(false)
        );
        assert_eq!(
            decide(&p, &sale(false, TARJETA, TCP, "203.0.113.7:443")),
            Some(false)
        );
        // el propio equipo, DHCP y lo que el Modo Hogar sirve a la casa siguen
        let mut propio = sale(false, 1, UDP, "127.0.0.1:53");
        propio.bucle = true;
        assert_eq!(decide(&p, &propio), Some(true));
        let mut pide = sale(false, TARJETA, UDP, "255.255.255.255:67");
        pide.puerto_local = 68;
        assert_eq!(decide(&p, &pide), Some(true));
        assert_eq!(
            decide(&p, &entra(true, TARJETA, UDP, "192.168.1.20:51515", 53)),
            Some(true)
        );
        // sin servidor conocido, nada del servicio sale: de él, solo lo que el Modo Hogar contesta a la casa
        let nada = estricto(Servidor::default()).persistentes;
        assert_eq!(
            decide(&nada, &sale(true, TARJETA, UDP, "203.0.113.7:51820")),
            Some(false)
        );
        assert_eq!(
            decide(&nada, &sale(true, TARJETA, TCP, "203.0.113.7:443")),
            Some(false)
        );
        for f in &nada {
            if f.permite && f.condiciones.contains(&Condicion::Servicio) {
                assert_eq!(f.nombre, "GUARDIANA VPN: Modo Hogar");
            }
        }
    }

    /// While the computer starts, before Windows' filtering service has loaded anything: only this computer talking to
    /// itself and the network underneath; nothing else, nothing of the service's (it starts later), and nothing
    /// forwarded (the virtual machines that start with the computer, WSL among them).
    #[test]
    fn al_arrancar_el_equipo_solo_la_red_de_debajo() {
        let arranque = estricto(conocido()).arranque;
        assert!(arranque
            .iter()
            .all(|f| !f.condiciones.contains(&Condicion::Servicio)
                && (Capa::ALE.contains(&f.capa) || (f.capa.es_reenvio() && !f.permite))
                && !f.firme));
        for f in reenviados() {
            assert_eq!(decide(&arranque, &f), Some(false), "{f:?}");
        }
        for servicio in [true, false] {
            for (protocolo, a) in [
                (UDP, "203.0.113.7:51820"),
                (TCP, "203.0.113.7:443"),
                (TCP, "198.51.100.9:443"),
                (UDP, "192.168.1.1:53"),
                (TCP, "[2606:4700::1111]:443"),
            ] {
                assert_eq!(
                    decide(&arranque, &sale(servicio, TARJETA, protocolo, a)),
                    Some(false),
                    "{servicio} {protocolo} {a}"
                );
            }
            assert_eq!(
                decide(
                    &arranque,
                    &entra(servicio, TARJETA, TCP, "192.168.1.20:51515", 7443)
                ),
                Some(false)
            );
        }
        let mut propio = sale(false, 1, TCP, "127.0.0.1:7443");
        propio.bucle = true;
        assert_eq!(decide(&arranque, &propio), Some(true));
        let mut pide = sale(false, TARJETA, UDP, "255.255.255.255:67");
        pide.puerto_local = 68;
        assert_eq!(decide(&arranque, &pide), Some(true));
        let mut vecino = sale(false, TARJETA, ICMP6, "[fe80::1]:0");
        vecino.puerto_local = 135;
        assert_eq!(decide(&arranque, &vecino), Some(true));
    }

    /// TunnelVision and TunnelCrack (ronda 91): strict mode has no exception for the local network going out. Of what
    /// goes out off the tunnel, only the service to its server, DHCP's request to the broadcast address, DHCPv6 to its
    /// multicast addresses and neighbour discovery; the house, and a «local» network a stranger's DHCP announces over
    /// public addresses, are blocked like anywhere else.
    #[test]
    fn sin_excepcion_de_la_red_local() {
        let e = estricto(conocido());
        for p in [&e.persistentes, &e.arranque] {
            for f in p
                .iter()
                .filter(|f| f.permite && matches!(f.capa, Capa::ConectaV4 | Capa::ConectaV6))
            {
                let al_servidor = f.condiciones.contains(&Condicion::Servicio)
                    && f.condiciones
                        .iter()
                        .any(|c| matches!(c, Condicion::Remota(ip) if *ip == extremo().ip()));
                assert!(
                    al_servidor
                        || f.condiciones == [Condicion::Bucle]
                        || f.nombre.starts_with("GUARDIANA VPN: DHCP")
                        || f.nombre == "GUARDIANA VPN: descubrimiento de vecinos",
                    "{}",
                    f.nombre
                );
            }
            for servicio in [true, false] {
                for (protocolo, a) in [
                    (TCP, "192.168.1.20:80"),
                    (UDP, "192.168.1.1:53"),
                    (TCP, "10.0.0.5:445"),
                    (TCP, "172.16.0.9:443"),
                    (UDP, "[fe80::20]:5353"),
                    (TCP, "203.0.113.50:443"),
                    (TCP, "198.51.100.1:80"),
                ] {
                    assert_eq!(
                        decide(p, &sale(servicio, TARJETA, protocolo, a)),
                        Some(false),
                        "{servicio} {protocolo} {a}"
                    );
                }
            }
            // ni lo que Windows reenvía: sin el permiso del túnel, ningún permiso en las capas del reenvío
            assert!(p.iter().all(|f| !f.capa.es_reenvio() || !f.permite));
        }
    }

    /// What Windows forwards (WSL2, Hyper-V's Default Switch, Windows Sandbox, Docker, the shared connection), in
    /// strict mode with the tunnel down (the server does not answer, the service dead or stopped, the computer
    /// starting): nothing, in IPv4 or IPv6, between any two interfaces, the old tunnel's included. The block is in both
    /// forwarding layers of both sets, persistent and boot-time, with the server known or not, at the lowest weight, and
    /// nothing there permits (review of 5 Nov 2026: until then those packets left by the network card).
    #[test]
    fn sin_tunel_nada_reenviado_sale() {
        for servidor in [conocido(), Servidor::default()] {
            let e = estricto(servidor);
            for p in [&e.persistentes, &e.arranque] {
                for capa in Capa::REENVIO {
                    let ahi: Vec<_> = p.iter().filter(|f| f.capa == capa).collect();
                    assert_eq!(ahi.len(), 1, "{capa:?}");
                    assert!(ahi.iter().all(|f| !f.permite
                        && !f.firme
                        && f.condiciones.is_empty()
                        && f.peso == peso::TODO
                        && f.nombre == "GUARDIANA VPN: lo reenviado fuera del túnel"));
                }
                for f in reenviados() {
                    assert_eq!(decide(p, &f), Some(false), "{f:?}");
                }
            }
        }
    }

    /// With the tunnel up, in strict mode (its block and the tunnel's dynamic permit, with the persistent set or still
    /// the boot-time one) and in the switch of always: what is forwarded into the tunnel and what comes out of it
    /// passes, both ways of a virtual machine's connection through the tunnel; nothing else forwarded does. Not by the
    /// network card (the house's network, the server's own address, a route a stranger's DHCP pushes, IPv6, which the
    /// tunnel does not carry) and not by another adapter. Out of the tunnel to the card passes too: what came through
    /// the tunnel was the server's, and it carries nothing of this computer's outside it.
    #[test]
    fn con_el_tunel_lo_reenviado_solo_por_el_tunel() {
        let e = estricto(conocido());
        let mut planes = vec![plan(TUNEL, extremo())];
        for mut p in [e.persistentes, e.arranque] {
            p.extend(del_tunel(TUNEL));
            planes.push(p);
        }
        for p in &planes {
            for f in reenviados() {
                let por_el_tunel = f.salida == TUNEL || f.interfaz == TUNEL;
                assert_eq!(decide(p, &f), Some(por_el_tunel), "{f:?}");
            }
            // lo de siempre: una máquina virtual hacia el túnel y su respuesta, en las dos familias
            for v6 in [false, true] {
                assert_eq!(decide(p, &reenvia(v6, VIRTUAL, TUNEL)), Some(true));
                assert_eq!(decide(p, &reenvia(v6, TUNEL, VIRTUAL)), Some(true));
                assert_eq!(decide(p, &reenvia(v6, VIRTUAL, TARJETA)), Some(false));
                assert_eq!(decide(p, &reenvia(v6, TARJETA, VIRTUAL)), Some(false));
            }
        }
    }

    /// The tunnel's own permit (strict mode's dynamic one, and part of the switch of always): in each forwarding layer,
    /// what leaves by the tunnel's interface and what arrived on it, by the tunnel's LUID, at the tunnel's weight, not
    /// firm; and the tunnel's interface in the four connection-authorisation layers. By itself it permits nothing else
    /// forwarded, not even through another adapter.
    #[test]
    fn el_permiso_del_tunel_cubre_el_reenvio() {
        let t = del_tunel(TUNEL);
        assert_eq!(t.len(), 8);
        assert!(t
            .iter()
            .all(|f| f.permite && !f.firme && f.peso == peso::TUNEL));
        for capa in Capa::ALE {
            let ahi: Vec<_> = t.iter().filter(|f| f.capa == capa).collect();
            assert_eq!(ahi.len(), 1, "{capa:?}");
            assert_eq!(ahi[0].condiciones, [Condicion::Interfaz(TUNEL)]);
        }
        for capa in Capa::REENVIO {
            let ahi: Vec<_> = t.iter().filter(|f| f.capa == capa).collect();
            assert_eq!(ahi.len(), 2, "{capa:?}");
            assert!(ahi
                .iter()
                .any(|f| f.condiciones == [Condicion::Salida(TUNEL)]
                    && f.nombre == "GUARDIANA VPN: reenviado al túnel"));
            assert!(ahi
                .iter()
                .any(|f| f.condiciones == [Condicion::Llegada(TUNEL)]
                    && f.nombre == "GUARDIANA VPN: reenviado desde el túnel"));
        }
        for v6 in [false, true] {
            assert_eq!(decide(&t, &reenvia(v6, VIRTUAL, TUNEL)), Some(true));
            assert_eq!(decide(&t, &reenvia(v6, TUNEL, VIRTUAL)), Some(true));
            // lo demás no lo decide él: lo corta el bloqueo
            assert_eq!(decide(&t, &reenvia(v6, VIRTUAL, TARJETA)), None);
            assert_eq!(decide(&t, &reenvia(v6, VIRTUAL, OTRO)), None);
            assert_eq!(decide(&t, &reenvia(v6, OTRO, VIRTUAL)), None);
        }
    }

    /// Every condition is in a layer where Windows offers it (Microsoft, «Filtering conditions available at each
    /// filtering layer» and «Filtering condition flags», read on 5 Nov 2026), or Windows refuses the filter: the
    /// forwarding layers have the interface a packet leaves by and the one it arrived on, but no program, protocol,
    /// port or remote address, and loopback's flag does not apply there; the plan's `Interfaz` is the local address's
    /// interface, so only in the connection-authorisation layers; Hyper-V's flag, only in the Ethernet ones.
    #[test]
    fn cada_condicion_donde_windows_la_ofrece() {
        let la_hay = |c: &Condicion, capa: Capa| match c {
            Condicion::Salida(_) | Condicion::Llegada(_) => capa.es_reenvio(),
            Condicion::EntreMaquinasVirtuales => matches!(capa, Capa::SaleMac | Capa::EntraMac),
            _ => Capa::ALE.contains(&capa),
        };
        let mut planes = vec![
            plan(TUNEL, extremo()),
            plan(TUNEL, "[2001:db8::7]:51820".parse().unwrap()),
            del_tunel(TUNEL),
        ];
        for servidor in [conocido(), Servidor::default()] {
            let e = estricto(servidor);
            planes.push(e.persistentes);
            planes.push(e.arranque);
        }
        for p in &planes {
            for f in p {
                for c in &f.condiciones {
                    assert!(la_hay(c, f.capa), "{} {:?} {c:?}", f.nombre, f.capa);
                }
            }
        }
    }
}
