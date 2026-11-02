//! What the service asks Windows' DNS every minute, read with Windows' own functions instead of a PowerShell
//! (1.0.55, «ligero»; the owner, 2 Nov 2026, 16:53: «se me traba», the simple PCs most).
//!
//! Until 1.0.54 the watchdog started two PowerShells a minute for this, each loading the network cmdlets and asking
//! WMI: on GitHub's Windows runner, measured with `.github/workflows/ligero.yml`, each start cost more processor time
//! than a whole hour of these native reads. The answer is the same, read from the same places:
//!
//! - the IPv4 interfaces and whether each is connected: `GetIpInterfaceTable`, the table behind `Get-NetIPInterface`
//!   (its `ConnectionState` is the row's `Connected`);
//! - each one's alias (`ConvertInterfaceLuidToAlias`): the loopback is left out by its alias, as there;
//! - its type, which the interface's LUID carries (IANA's numbers: 131 tunnel, 53 virtual), and whether `Get-NetAdapter`
//!   shows the adapter, which is whether the network class in the registry lists it without `NCF_HIDDEN`:
//!   `Get-NetAdapter` leaves hidden adapters out, so the PowerShell only skipped the visible ones, and so does this;
//! - its resolvers in both families, in the order Windows tries them: `GetAdaptersAddresses`, the list that
//!   `ipconfig /all` and `Get-DnsClientServerAddress` give, the `fec0:0:0:ffff::` markers included;
//! - the resolvers DHCP gave it: the registry value `DhcpNameServer`, the key the PowerShell read.
//!
//! Anything that fails returns an error, and the caller asks PowerShell as before: an answer can cost more, never be
//! different. `crates/service/src/sysdns/windows.rs`'s test compares both on a real Windows at every push.
//!
//! `unsafe` is denied in this project unless justified, and this is the justification: IP Helper and the registry are
//! C APIs with no safe wrapper, and the other way to read them is a PowerShell. Every table Windows hands back is read
//! within the length it gives and freed once with its own function; every buffer goes with its size; every key is
//! closed when it goes out of scope; nothing is written.

#![allow(unsafe_code)]

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use windows_sys::core::GUID;
use windows_sys::Win32::Foundation::{
    ERROR_BUFFER_OVERFLOW, ERROR_NO_DATA, ERROR_SUCCESS, NO_ERROR,
};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    ConvertInterfaceIndexToLuid, ConvertInterfaceLuidToAlias, ConvertInterfaceLuidToGuid,
    FreeMibTable, GetAdaptersAddresses, GetIpForwardTable2, GetIpInterfaceEntry,
    GetIpInterfaceTable, InitializeIpInterfaceEntry, GAA_FLAG_INCLUDE_ALL_INTERFACES,
    GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_FRIENDLY_NAME, GAA_FLAG_SKIP_MULTICAST,
    GAA_FLAG_SKIP_UNICAST, IP_ADAPTER_ADDRESSES_LH, MIB_IPFORWARD_ROW2, MIB_IPFORWARD_TABLE2,
    MIB_IPINTERFACE_ROW, MIB_IPINTERFACE_TABLE,
};
use windows_sys::Win32::NetworkManagement::Ndis::NET_LUID_LH;
use windows_sys::Win32::Networking::WinSock::{
    AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR_IN, SOCKADDR_IN6,
};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE,
    KEY_READ, KEY_WOW64_64KEY, REG_DWORD, REG_EXPAND_SZ, REG_MULTI_SZ, REG_SZ,
};

use super::Linea;

/// IANA's interface types the watch leaves alone (`windows.rs`, review of 8 Oct 2026): tunnel and virtual.
const TUNEL: u32 = 131;
const VIRTUAL: u32 = 53;
/// The network adapters' class in the registry (`GUID_DEVCLASS_NET`): one subkey per adapter.
const CLASE_RED: &str =
    r"SYSTEM\CurrentControlSet\Control\Class\{4d36e972-e325-11ce-bfc1-08002be10318}";
/// `NCF_HIDDEN`: the adapter appears in no user interface, and `Get-NetAdapter` leaves it out.
const NCF_HIDDEN: u32 = 0x8;
/// Each interface's IPv4 settings, by its GUID.
const INTERFACES_TCPIP: &str = r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces";

fn ancho(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// A registry key opened for reading, closed when it goes out of scope.
struct Clave(HKEY);

impl Drop for Clave {
    fn drop(&mut self) {
        // SAFETY: the handle was opened by `abre_en` and is closed once, here.
        unsafe {
            RegCloseKey(self.0);
        }
    }
}

fn abre_en(raiz: HKEY, ruta: &str) -> Option<Clave> {
    let r = ancho(ruta);
    let mut h: HKEY = std::ptr::null_mut();
    // SAFETY: `r` is a NUL-terminated wide string that outlives the call; `h` is ours.
    let e = unsafe { RegOpenKeyExW(raiz, r.as_ptr(), 0, KEY_READ | KEY_WOW64_64KEY, &raw mut h) };
    (e == ERROR_SUCCESS && !h.is_null()).then_some(Clave(h))
}

fn abre(ruta: &str) -> Option<Clave> {
    abre_en(HKEY_LOCAL_MACHINE, ruta)
}

impl Clave {
    fn hija(&self, nombre: &str) -> Option<Clave> {
        abre_en(self.0, nombre)
    }

    fn subclaves(&self) -> Vec<String> {
        let mut v = Vec::new();
        for i in 0..4_096u32 {
            // Registry key names are at most 255 characters.
            let mut buf = [0u16; 512];
            let mut largo = u32::try_from(buf.len()).unwrap_or(0);
            // SAFETY: `buf` is ours and `largo` carries its size; the optional pointers are null.
            let e = unsafe {
                RegEnumKeyExW(
                    self.0,
                    i,
                    buf.as_mut_ptr(),
                    &raw mut largo,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            };
            if e != ERROR_SUCCESS {
                break;
            }
            let n = usize::try_from(largo).unwrap_or(0).min(buf.len());
            v.push(String::from_utf16_lossy(&buf[..n]));
        }
        v
    }

    /// The raw bytes and the type of value `nombre`, or `None` when there is no such value.
    fn valor(&self, nombre: &str) -> Option<(u32, Vec<u8>)> {
        let n = ancho(nombre);
        let mut tipo: u32 = 0;
        let mut largo: u32 = 0;
        // SAFETY: a size query: no data pointer, Windows writes the type and the size into our two numbers.
        let e = unsafe {
            RegQueryValueExW(
                self.0,
                n.as_ptr(),
                std::ptr::null(),
                &raw mut tipo,
                std::ptr::null_mut(),
                &raw mut largo,
            )
        };
        if e != ERROR_SUCCESS || largo > 1 << 20 {
            return None;
        }
        // Two spare bytes: a text stored without its final NUL still ends.
        let mut datos = vec![0u8; usize::try_from(largo).ok()? + 2];
        let mut cabe = u32::try_from(datos.len()).ok()?;
        // SAFETY: `datos` is ours and `cabe` says how many bytes it holds; Windows writes at most that.
        let e = unsafe {
            RegQueryValueExW(
                self.0,
                n.as_ptr(),
                std::ptr::null(),
                &raw mut tipo,
                datos.as_mut_ptr(),
                &raw mut cabe,
            )
        };
        if e != ERROR_SUCCESS {
            return None;
        }
        datos.truncate(usize::try_from(cabe).unwrap_or(0).min(datos.len()));
        Some((tipo, datos))
    }

    /// A value as PowerShell's `[string]` of it holds addresses: a text as it is (up to its first NUL), a list of texts
    /// joined by spaces, anything else empty (a number or bytes are no resolvers). `None` when there is no such value.
    fn texto(&self, nombre: &str) -> Option<String> {
        let (tipo, datos) = self.valor(nombre)?;
        let unidades: Vec<u16> = datos
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        Some(match tipo {
            REG_SZ | REG_EXPAND_SZ => {
                let fin = unidades
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(unidades.len());
                String::from_utf16_lossy(&unidades[..fin])
            }
            REG_MULTI_SZ => unidades
                .split(|&c| c == 0)
                .filter(|p| !p.is_empty())
                .map(String::from_utf16_lossy)
                .collect::<Vec<_>>()
                .join(" "),
            _ => String::new(),
        })
    }

    fn dword(&self, nombre: &str) -> Option<u32> {
        match self.valor(nombre)? {
            (REG_DWORD, d) if d.len() >= 4 => Some(u32::from_le_bytes([d[0], d[1], d[2], d[3]])),
            _ => None,
        }
    }
}

/// `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}`, as `Get-NetAdapter` writes an interface's GUID and the registry names it.
fn texto_guid(g: &GUID) -> String {
    let d = g.data4;
    format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        g.data1, g.data2, g.data3, d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]
    )
}

/// The GUID of the interface `luid`, or `None` when Windows does not know it.
fn guid(luid: u64) -> Option<String> {
    let l = NET_LUID_LH { Value: luid };
    let mut g = GUID::default();
    // SAFETY: `l` and `g` are ours; Windows reads the first and writes the second.
    let e = unsafe { ConvertInterfaceLuidToGuid(&raw const l, &raw mut g) };
    (e == NO_ERROR).then(|| texto_guid(&g))
}

/// The interface type, which a LUID carries in its top 16 bits (`NET_LUID_LH.Info.IfType`).
fn tipo(luid: u64) -> u32 {
    u32::try_from(luid >> 48).unwrap_or(0)
}

/// The interface's alias ("Wi-Fi", "Ethernet", "Loopback Pseudo-Interface 1").
fn alias(luid: u64) -> Result<String, String> {
    let l = NET_LUID_LH { Value: luid };
    // NDIS_IF_MAX_STRING_SIZE + 1.
    let mut buf = [0u16; 257];
    // SAFETY: `l` is ours; `buf` is ours and its length in characters goes with it.
    let e = unsafe { ConvertInterfaceLuidToAlias(&raw const l, buf.as_mut_ptr(), buf.len()) };
    if e != NO_ERROR {
        return Err(format!("ConvertInterfaceLuidToAlias = {e}"));
    }
    let fin = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Ok(String::from_utf16_lossy(&buf[..fin]))
}

/// `-notlike 'Loopback*'`: PowerShell compares without case.
fn es_loopback(alias: &str) -> bool {
    alias
        .get(..8)
        .is_some_and(|p| p.eq_ignore_ascii_case("loopback"))
}

/// Each IPv4 interface, in the table's order: its LUID and whether it is connected.
fn interfaces_ipv4() -> Result<Vec<(u64, bool)>, String> {
    let mut tabla: *mut MIB_IPINTERFACE_TABLE = std::ptr::null_mut();
    // SAFETY: Windows allocates the table and writes its address into `tabla`, which is ours.
    let e = unsafe { GetIpInterfaceTable(AF_INET, &raw mut tabla) };
    if e != NO_ERROR || tabla.is_null() {
        return Err(format!("GetIpInterfaceTable = {e}"));
    }
    // SAFETY: the table holds `NumEntries` rows from `Table` on; they are read within that, and the table is freed
    // once, right after, with Windows' own function.
    let filas: Vec<(u64, bool)> = unsafe {
        let n = usize::try_from((*tabla).NumEntries)
            .unwrap_or(0)
            .min(65_536);
        let primera = (&raw const (*tabla).Table).cast::<MIB_IPINTERFACE_ROW>();
        (0..n)
            .map(|i| {
                let f = &*primera.add(i);
                (f.InterfaceLuid.Value, f.Connected)
            })
            .collect()
    };
    // SAFETY: as above; the table is not used after this.
    unsafe { FreeMibTable(tabla.cast_const().cast()) };
    Ok(filas)
}

/// Each adapter's resolvers by LUID: IPv4 and IPv6, each in the order Windows tries them.
fn resolutores() -> Result<HashMap<u64, Linea>, String> {
    let banderas = GAA_FLAG_SKIP_UNICAST
        | GAA_FLAG_SKIP_ANYCAST
        | GAA_FLAG_SKIP_MULTICAST
        | GAA_FLAG_SKIP_FRIENDLY_NAME
        | GAA_FLAG_INCLUDE_ALL_INTERFACES;
    let mut bytes: u32 = 32 * 1024;
    for _ in 0..4 {
        // In 8-byte words: the list's structures need that alignment.
        let mut buf: Vec<u64> = vec![0; usize::try_from(bytes).unwrap_or(0).div_ceil(8)];
        let mut cabe = u32::try_from(buf.len() * 8).unwrap_or(u32::MAX);
        let primero = buf.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
        // SAFETY: `buf` is ours, aligned for the list, and `cabe` says how many bytes it holds.
        let e = unsafe {
            GetAdaptersAddresses(
                u32::from(AF_UNSPEC),
                banderas,
                std::ptr::null(),
                primero,
                &raw mut cabe,
            )
        };
        if e == ERROR_BUFFER_OVERFLOW {
            bytes = cabe.max(bytes.saturating_mul(2));
            continue;
        }
        if e == ERROR_NO_DATA {
            return Ok(HashMap::new());
        }
        if e != NO_ERROR {
            return Err(format!("GetAdaptersAddresses = {e}"));
        }
        let mut out = HashMap::new();
        let mut a = primero;
        let mut nodos = 0;
        // SAFETY: Windows wrote a linked list inside `buf`, which lives until the end of this function; each adapter
        // and each of its resolvers is read through the pointers Windows gave, a socket address only within the
        // length it gives, and nothing is kept but copies.
        unsafe {
            while !a.is_null() && nodos < 4_096 {
                nodos += 1;
                let mut v4 = Vec::new();
                let mut v6 = Vec::new();
                let mut s = (*a).FirstDnsServerAddress;
                let mut n = 0;
                while !s.is_null() && n < 256 {
                    n += 1;
                    let sa = (*s).Address.lpSockaddr;
                    let largo = usize::try_from((*s).Address.iSockaddrLength).unwrap_or(0);
                    if !sa.is_null() {
                        let familia = std::ptr::read_unaligned(sa.cast::<u16>());
                        if familia == AF_INET && largo >= std::mem::size_of::<SOCKADDR_IN>() {
                            let v = std::ptr::read_unaligned(sa.cast::<SOCKADDR_IN>());
                            v4.push(IpAddr::V4(Ipv4Addr::from(
                                v.sin_addr.S_un.S_addr.to_ne_bytes(),
                            )));
                        } else if familia == AF_INET6
                            && largo >= std::mem::size_of::<SOCKADDR_IN6>()
                        {
                            let v = std::ptr::read_unaligned(sa.cast::<SOCKADDR_IN6>());
                            v6.push(IpAddr::V6(Ipv6Addr::from(v.sin6_addr.u.Byte)));
                        }
                    }
                    s = (*s).Next;
                }
                out.insert((*a).Luid.Value, (v4, v6));
                a = (*a).Next;
            }
        }
        return Ok(out);
    }
    Err("GetAdaptersAddresses: la lista no cupo".to_owned())
}

/// The adapters the network class lists, by their interface's GUID (upper case, with braces), and whether each is
/// hidden. An error when the class cannot be read at all: then nobody can say which adapters `Get-NetAdapter` shows.
pub(crate) fn adaptadores() -> Result<HashMap<String, bool>, String> {
    let clase = abre(CLASE_RED).ok_or_else(|| format!("no se pudo abrir {CLASE_RED}"))?;
    let mut out = HashMap::new();
    // "0000", "0001"…: one per adapter. "Properties" is no adapter, and is not readable.
    for sub in clase.subclaves() {
        let Some(k) = clase.hija(&sub) else {
            continue;
        };
        let Some(id) = k.texto("NetCfgInstanceId") else {
            continue;
        };
        let oculto = k
            .dword("Characteristics")
            .is_some_and(|c| c & NCF_HIDDEN != 0);
        out.insert(id.trim().to_ascii_uppercase(), oculto);
    }
    if out.is_empty() {
        return Err(format!("{CLASE_RED} no lista ningún adaptador"));
    }
    Ok(out)
}

/// Whether `Get-NetAdapter` shows the adapter with this interface GUID: listed, and not hidden.
pub(crate) fn visible(adaptadores: &HashMap<String, bool>, guid: &str) -> bool {
    adaptadores.get(&guid.to_ascii_uppercase()) == Some(&false)
}

/// What `PRIMARY_SCRIPT` prints, without PowerShell: one entry per connected IPv4 interface that is not the loopback
/// nor a visible tunnel or virtual adapter and has resolvers, in the table's order.
///
/// # Errors
/// The call of Windows that failed; the caller then asks PowerShell.
pub(crate) fn lineas() -> Result<Vec<Linea>, String> {
    let interfaces = interfaces_ipv4()?;
    let dns = resolutores()?;
    // Read only when a tunnel or virtual adapter is there: on most computers, never.
    let mut clase: Option<HashMap<String, bool>> = None;
    let mut out = Vec::new();
    for (luid, conectada) in interfaces {
        if !conectada || es_loopback(&alias(luid)?) {
            continue;
        }
        let t = tipo(luid);
        if t == TUNEL || t == VIRTUAL {
            if clase.is_none() {
                clase = Some(adaptadores()?);
            }
            let visto = clase
                .as_ref()
                .zip(guid(luid))
                .is_some_and(|(c, g)| visible(c, &g));
            if visto {
                continue;
            }
        }
        let (v4, v6) = dns.get(&luid).cloned().unwrap_or_default();
        if !v4.is_empty() || !v6.is_empty() {
            out.push((v4, v6));
        }
    }
    Ok(out)
}

/// The GUID of the interface with index `indice`, when Windows has it.
pub(crate) fn guid_de_indice(indice: u32) -> Option<String> {
    let mut l = NET_LUID_LH::default();
    // SAFETY: `l` is ours; Windows writes into it the LUID of interface `indice`, or fails.
    if unsafe { ConvertInterfaceIndexToLuid(indice, &raw mut l) } != NO_ERROR {
        return None;
    }
    // SAFETY: a LUID Windows just wrote whole.
    guid(unsafe { l.Value })
}

/// What the DHCP line of `upstreams_for` prints, without PowerShell: for each index whose adapter `Get-NetAdapter`
/// shows, the resolvers of the registry's `DhcpNameServer` (none when the value is missing). An index without such an
/// adapter gives nothing, as there.
///
/// # Errors
/// When the network class cannot be read; the caller then asks PowerShell.
pub(crate) fn dhcp(indices: &[u32]) -> Result<Vec<(u32, Vec<IpAddr>)>, String> {
    let clase = adaptadores()?;
    let mut out = Vec::new();
    for &n in indices {
        let Some(g) = guid_de_indice(n) else {
            continue;
        };
        if !visible(&clase, &g) {
            continue;
        }
        let lista = abre(&format!("{INTERFACES_TCPIP}\\{g}"))
            .and_then(|k| k.texto("DhcpNameServer"))
            .unwrap_or_default();
        out.push((n, super::parse_list(&lista)));
    }
    Ok(out)
}

/// The next hops of the IPv4 default routes (0.0.0.0/0, the active ones), the lowest route metric
/// plus its interface's metric first, as `default_gateway`'s PowerShell lists them (`Get-NetRoute`
/// and `Get-NetIPInterface`): the caller takes the first that is a gateway.
///
/// # Errors
/// The call of Windows that failed; the caller then asks PowerShell.
pub(crate) fn saltos_por_defecto() -> Result<Vec<IpAddr>, String> {
    let mut tabla: *mut MIB_IPFORWARD_TABLE2 = std::ptr::null_mut();
    // SAFETY: Windows allocates the table and writes its address into `tabla`, which is ours.
    let e = unsafe { GetIpForwardTable2(AF_INET, &raw mut tabla) };
    if e != NO_ERROR || tabla.is_null() {
        return Err(format!("GetIpForwardTable2 = {e}"));
    }
    // SAFETY: the table holds `NumEntries` rows from `Table` on; they are read within that (an
    // IPv4 table: its addresses are IPv4 ones), and the table is freed once, right after.
    let rutas: Vec<(u64, u32, Ipv4Addr)> = unsafe {
        let n = usize::try_from((*tabla).NumEntries)
            .unwrap_or(0)
            .min(1 << 20);
        let primera = (&raw const (*tabla).Table).cast::<MIB_IPFORWARD_ROW2>();
        (0..n)
            .filter_map(|i| {
                let f = &*primera.add(i);
                let destino = f.DestinationPrefix.Prefix.Ipv4;
                let por_defecto = f.DestinationPrefix.PrefixLength == 0
                    && destino.sin_family == AF_INET
                    && destino.sin_addr.S_un.S_addr == 0;
                por_defecto.then(|| {
                    let salto = f.NextHop.Ipv4.sin_addr.S_un.S_addr.to_ne_bytes();
                    (f.InterfaceLuid.Value, f.Metric, Ipv4Addr::from(salto))
                })
            })
            .collect()
    };
    // SAFETY: as above; the table is not used after this.
    unsafe { FreeMibTable(tabla.cast_const().cast()) };
    let mut con_metrica: Vec<(u64, IpAddr)> = Vec::with_capacity(rutas.len());
    for (luid, metrica, salto) in rutas {
        let mut fila = MIB_IPINTERFACE_ROW::default();
        // SAFETY: a row owned here, initialised by Windows' own function before it is filled.
        unsafe { InitializeIpInterfaceEntry(&raw mut fila) };
        fila.Family = AF_INET;
        fila.InterfaceLuid = NET_LUID_LH { Value: luid };
        // SAFETY: a row with its family and interface set, owned for the call; Windows fills the
        // rest. An interface it no longer knows adds nothing, as `$null` added nothing there.
        let de_la_interfaz = if unsafe { GetIpInterfaceEntry(&raw mut fila) } == NO_ERROR {
            fila.Metric
        } else {
            0
        };
        con_metrica.push((
            u64::from(metrica) + u64::from(de_la_interfaz),
            IpAddr::V4(salto),
        ));
    }
    con_metrica.sort_by_key(|(m, _)| *m);
    Ok(con_metrica.into_iter().map(|(_, ip)| ip).collect())
}
