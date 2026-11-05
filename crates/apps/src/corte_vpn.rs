//! The VPN's cut-off switch on Windows (in preparation; docs/VPN.md): `guardiana_tunel::corte`'s plan turned into
//! Windows Filtering Platform filters, in a **dynamic** session of their own, under a provider and a sublayer of that
//! session's own. Windows removes every one of them the moment the session closes: when this value is dropped, and
//! when the service ends for any reason, so a crash never leaves the computer without network.
//!
//! The provider and the sublayer get new keys in every session, as WireGuard's own client does: what a dynamic session
//! adds belongs to it, and no other session can hang anything from it. With fixed keys (GUARDIANA's provider, the same as
//! the antivirus cuts', `cortafuegos`), the panel's long-lived firewall session owned the provider, and Windows refused
//! the VPN's sublayer with `FWP_E_WRONG_SESSION` (0x8032000C; «FwpmSubLayerAdd0 = 0x8032000c» on a person's computer,
//! 2 Nov 2026, 14:07); and two switches at once would refuse each other.
//!
//! The plan names the conditions; here each becomes WFP's own: the tunnel's interface by its LUID, the service by its
//! program file (`FwpmGetAppIdFromFileName0`), loopback by its flag, protocols, ports, addresses and networks, ICMPv6
//! types and codes (which WFP keeps in the port fields), and Hyper-V's machine-to-machine flag. The service's permits
//! (1.0.55) combine its program file with a protocol, the server's address and port, or a local port: each a field of
//! both connection-authorisation layers, outgoing and accepted incoming. Only the plan's firm permit, the encrypted
//! packets to the server, gets `FWPM_FILTER_FLAG_CLEAR_ACTION_RIGHT`.
//!
//! What Windows forwards (1.0.56; `guardiana_tunel::corte`, «Forwarding»): the plan's forwarding layers are
//! `FWPM_LAYER_IPFORWARD_V4` and `_V6`, and the tunnel is named there by its LUID too, as the interface a forwarded
//! packet leaves by (`FWPM_CONDITION_IP_FORWARD_INTERFACE`) or the one it arrived on (`FWPM_CONDITION_IP_LOCAL_INTERFACE`,
//! which means that in those layers). Strict mode's sets carry the block on everything forwarded, persistent and
//! boot-time, under the same keys as the rest, so removing them takes it too, whichever version put it; the tunnel's
//! dynamic permit carries the two ways through the tunnel.
//!
//! Strict mode (ronda 91; docs/VPN.md, «Modo estricto»): the block must hold when GUARDIANA does not, so it is **not**
//! in a dynamic session. [`pon_estricto`] adds `guardiana_tunel::corte::estricto`'s plan from a session that is not
//! dynamic, under a persistent provider and a persistent sublayer with fixed keys (`guardiana_tunel::estricto::claves`),
//! as persistent filters (`FWPM_FILTER_FLAG_PERSISTENT`: they stay through a crash, an update, a restart of Windows'
//! filtering service and of the computer) and boot-time filters (`FWPM_FILTER_FLAG_BOOTTIME`: enforced from the
//! computer's start until that service has loaded the persistent ones; Microsoft does not let one filter be both). Each
//! filter's key is a fixed base plus its place, so every version finds and removes every one ([`quita_estricto`]), and
//! a set is replaced in one transaction, never leaving a moment without one. The provider names GUARDIANA's service:
//! Windows loads its persistent filters at start only while that service exists and starts with the computer. The
//! tunnel's own permit goes in the same sublayer from a dynamic session ([`PermisoTunel`]), so it goes with the process,
//! as the tunnel's adapter does.
//!
//! `unsafe` is used here, as the project allows with justification (see `cortafuegos/ventanas.rs`, the same API): every
//! value a condition points to is owned here and outlives the call that adds the filter, every result is checked, and
//! the engine is closed on drop.
#![allow(unsafe_code)]

use std::net::IpAddr;
use std::path::Path;

use guardiana_tunel::corte::{Capa, Condicion, Estricto, Filtro};
use guardiana_tunel::estricto::{claves, SERVICIO};
use windows_sys::core::GUID;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::NetworkManagement::WindowsFilteringPlatform::*;
use windows_sys::Win32::System::Rpc::UuidCreate;

use crate::cortafuegos::ErrorCorte;

/// `RPC_C_AUTHN_WINNT`.
const RPC_C_AUTHN_WINNT: u32 = 10;
/// `FWPM_PROVIDER_FLAG_PERSISTENT` (fwpmtypes.h): the provider outlives every session.
const PROVEEDOR_PERSISTENTE: u32 = 0x1;
/// `FWPM_SUBLAYER_FLAG_PERSISTENT` (fwpmtypes.h): the sublayer outlives every session.
const SUBCAPA_PERSISTENTE: u32 = 0x1;
/// `FWP_E_FILTER_NOT_FOUND`.
const FILTRO_NO_ESTA: u32 = 0x8032_0003;
/// `FWP_E_PROVIDER_NOT_FOUND`.
const PROVEEDOR_NO_ESTA: u32 = 0x8032_0005;
/// `FWP_E_SUBLAYER_NOT_FOUND`.
const SUBCAPA_NO_ESTA: u32 = 0x8032_0007;
/// `FWP_E_IN_USE`: another object still hangs from this one.
const EN_USO: u32 = 0x8032_000a;
/// `FWPM_SUBLAYER_UNIVERSAL`, Windows' own: where the boot-time filters go if Windows does not take them in strict
/// mode's own sublayer.
const SUBCAPA_UNIVERSAL: u128 = 0xeebe_cc03_ced4_4380_819a_2734_397b_2b74;

fn err(llamada: &str, code: u32) -> ErrorCorte {
    ErrorCorte::Sistema(format!("{llamada} = 0x{code:08x}"))
}

/// Whether Windows refused for lack of permission (`ERROR_ACCESS_DENIED`, also as an `HRESULT`): the program is not
/// running as an administrator or as the system.
#[must_use]
pub fn sin_permiso(e: &ErrorCorte) -> bool {
    matches!(e, ErrorCorte::Sistema(t) if t.ends_with("= 0x00000005") || t.ends_with("= 0x80070005"))
}

/// A new key for one of this session's objects.
fn clave_nueva() -> Result<GUID, ErrorCorte> {
    let mut g = GUID::from_u128(0);
    // SAFETY: an out-pointer to a GUID owned here.
    let r = unsafe { UuidCreate(&mut g) };
    // RPC_S_OK (0), or RPC_S_UUID_LOCAL_ONLY (1824): unique on this computer only, which is all a session's key needs
    if r != 0 && r != 1824 {
        return Err(err("UuidCreate", u32::from_ne_bytes(r.to_ne_bytes())));
    }
    Ok(g)
}

/// The key of strict mode's object `x` (`guardiana_tunel::estricto::claves`).
fn guid(x: u128) -> GUID {
    GUID::from_u128(x)
}

/// The key of the `i`-th filter of the strict set whose keys start at `base`.
fn clave_de(base: u128, i: usize) -> GUID {
    guid(base + i as u128)
}

/// A key as the number it was made from (the inverse of `GUID::from_u128`): windows-sys's `GUID` has no `==`.
fn numero(g: &GUID) -> u128 {
    (u128::from(g.data1) << 96)
        | (u128::from(g.data2) << 80)
        | (u128::from(g.data3) << 64)
        | u128::from(u64::from_be_bytes(g.data4))
}

/// Whether `capa` is one of WFP's forwarding layers.
fn de_reenvio(capa: &GUID) -> bool {
    [FWPM_LAYER_IPFORWARD_V4, FWPM_LAYER_IPFORWARD_V6]
        .iter()
        .any(|c| numero(c) == numero(capa))
}

fn ancho(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn capa_wfp(c: Capa) -> GUID {
    match c {
        Capa::ConectaV4 => FWPM_LAYER_ALE_AUTH_CONNECT_V4,
        Capa::AceptaV4 => FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4,
        Capa::ConectaV6 => FWPM_LAYER_ALE_AUTH_CONNECT_V6,
        Capa::AceptaV6 => FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V6,
        Capa::ReenviaV4 => FWPM_LAYER_IPFORWARD_V4,
        Capa::ReenviaV6 => FWPM_LAYER_IPFORWARD_V6,
        Capa::SaleMac => FWPM_LAYER_OUTBOUND_MAC_FRAME_NATIVE,
        Capa::EntraMac => FWPM_LAYER_INBOUND_MAC_FRAME_NATIVE,
    }
}

/// The values the conditions of one filter point to, kept alive (and not moved) while the filter is added.
// Boxes inside the vectors on purpose: the conditions hold pointers into each value, and a vector of the values
// themselves would move them when it grows.
#[allow(clippy::vec_box)]
struct Valores {
    u64s: Vec<Box<u64>>,
    v6: Vec<Box<FWP_BYTE_ARRAY16>>,
    redes: Vec<Box<FWP_V6_ADDR_AND_MASK>>,
}

/// A session of Windows' filter engine, closed on drop. A dynamic one takes everything it added with it; strict mode's
/// persistent and boot-time objects are added only from one that is not.
struct Motor(HANDLE);

// The handle is used only through the engine's own calls, which are thread-safe.
unsafe impl Send for Motor {}
unsafe impl Sync for Motor {}

impl Motor {
    fn abre(dinamico: bool) -> Result<Self, ErrorCorte> {
        // SAFETY: a zeroed FWPM_SESSION0 is a valid "no options" session; only the dynamic flag may be set.
        let mut sesion: FWPM_SESSION0 = unsafe { std::mem::zeroed() };
        if dinamico {
            sesion.flags = FWPM_SESSION_FLAG_DYNAMIC;
        }
        let mut engine: HANDLE = std::ptr::null_mut();
        // SAFETY: local engine (null server name), our session, out-handle owned here.
        let r = unsafe {
            FwpmEngineOpen0(
                std::ptr::null(),
                RPC_C_AUTHN_WINNT,
                std::ptr::null(),
                &sesion,
                &mut engine,
            )
        };
        if r != 0 {
            return Err(err("FwpmEngineOpen0", r));
        }
        Ok(Self(engine))
    }

    /// `hacer` inside one transaction: all of it or nothing, and nobody sees it half done.
    fn en_transaccion<T>(
        &self,
        hacer: impl FnOnce() -> Result<T, ErrorCorte>,
    ) -> Result<T, ErrorCorte> {
        // SAFETY: an engine handle owned here; no flags.
        let r = unsafe { FwpmTransactionBegin0(self.0, 0) };
        if r != 0 {
            return Err(err("FwpmTransactionBegin0", r));
        }
        match hacer() {
            Ok(v) => {
                // SAFETY: the transaction begun above.
                let r = unsafe { FwpmTransactionCommit0(self.0) };
                if r == 0 {
                    Ok(v)
                } else {
                    Err(err("FwpmTransactionCommit0", r))
                }
            }
            Err(e) => {
                // SAFETY: the transaction begun above; what it did is undone.
                unsafe {
                    FwpmTransactionAbort0(self.0);
                }
                Err(e)
            }
        }
    }

    /// The keys of the strict set starting at `base` that Windows has now, each with whether its filter is in one of
    /// the forwarding layers.
    fn existentes(&self, base: u128) -> Result<Vec<(GUID, bool)>, ErrorCorte> {
        let mut hay = Vec::new();
        for i in 0..usize::from(claves::MAXIMO) {
            let clave = clave_de(base, i);
            let mut f: *mut FWPM_FILTER0 = std::ptr::null_mut();
            // SAFETY: a lookup by a key owned here into an out-pointer owned here; what Windows gives back is freed
            // once, right after.
            let r = unsafe { FwpmFilterGetByKey0(self.0, &clave, &mut f) };
            if r == 0 && !f.is_null() {
                // SAFETY: the filter Windows just gave, read (its layer's key, copied) before it is freed below.
                let capa = unsafe { (*f).layerKey };
                // SAFETY: given by FwpmFilterGetByKey0 and not used after this.
                unsafe {
                    let mut p = f.cast::<core::ffi::c_void>();
                    FwpmFreeMemory0(&mut p);
                }
                hay.push((clave, de_reenvio(&capa)));
            } else if r != 0 && r != FILTRO_NO_ESTA {
                return Err(err("FwpmFilterGetByKey0", r));
            }
        }
        Ok(hay)
    }

    /// Whether strict mode's provider is in Windows.
    fn hay_proveedor(&self) -> Result<bool, ErrorCorte> {
        let clave = guid(claves::PROVEEDOR);
        let mut p: *mut FWPM_PROVIDER0 = std::ptr::null_mut();
        // SAFETY: a lookup by a key owned here into an out-pointer owned here, freed right after.
        let r = unsafe { FwpmProviderGetByKey0(self.0, &clave, &mut p) };
        if r == 0 && !p.is_null() {
            // SAFETY: given by FwpmProviderGetByKey0 and not used after this.
            unsafe {
                let mut x = p.cast::<core::ffi::c_void>();
                FwpmFreeMemory0(&mut x);
            }
            return Ok(true);
        }
        if r == 0 || r == PROVEEDOR_NO_ESTA {
            Ok(false)
        } else {
            Err(err("FwpmProviderGetByKey0", r))
        }
    }

    /// Whether strict mode's sublayer is in Windows.
    fn hay_subcapa(&self) -> Result<bool, ErrorCorte> {
        let clave = guid(claves::SUBCAPA);
        let mut s: *mut FWPM_SUBLAYER0 = std::ptr::null_mut();
        // SAFETY: a lookup by a key owned here into an out-pointer owned here, freed right after.
        let r = unsafe { FwpmSubLayerGetByKey0(self.0, &clave, &mut s) };
        if r == 0 && !s.is_null() {
            // SAFETY: given by FwpmSubLayerGetByKey0 and not used after this.
            unsafe {
                let mut x = s.cast::<core::ffi::c_void>();
                FwpmFreeMemory0(&mut x);
            }
            return Ok(true);
        }
        if r == 0 || r == SUBCAPA_NO_ESTA {
            Ok(false)
        } else {
            Err(err("FwpmSubLayerGetByKey0", r))
        }
    }

    /// Removes the filter with key `clave`; one already gone is gone.
    fn borra_filtro(&self, clave: &GUID) -> Result<(), ErrorCorte> {
        // SAFETY: deleting by a key owned by the caller.
        let r = unsafe { FwpmFilterDeleteByKey0(self.0, clave) };
        if r == 0 || r == FILTRO_NO_ESTA {
            Ok(())
        } else {
            Err(err("FwpmFilterDeleteByKey0", r))
        }
    }
}

impl Drop for Motor {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: the handle came from FwpmEngineOpen0 and is not used after this. Closing a dynamic session
            // removes every object it added; a session that is not dynamic leaves them.
            unsafe {
                FwpmEngineClose0(self.0);
            }
        }
    }
}

/// The service's program file as Windows' filter engine names it (`FwpmGetAppIdFromFileName0`), freed on drop.
struct AppId(*mut FWP_BYTE_BLOB);

impl AppId {
    fn de(ruta: &Path) -> Result<Self, ErrorCorte> {
        let ancha = ancho(&ruta.to_string_lossy());
        let mut app_id: *mut FWP_BYTE_BLOB = std::ptr::null_mut();
        // SAFETY: a NUL-terminated path owned for the call; the out-pointer is freed on drop.
        let r = unsafe { FwpmGetAppIdFromFileName0(ancha.as_ptr(), &mut app_id) };
        if r != 0 || app_id.is_null() {
            return Err(err("FwpmGetAppIdFromFileName0", r));
        }
        Ok(Self(app_id))
    }
}

impl Drop for AppId {
    fn drop(&mut self) {
        // SAFETY: given by FwpmGetAppIdFromFileName0; freed once.
        unsafe {
            let mut p = self.0.cast::<core::ffi::c_void>();
            FwpmFreeMemory0(&mut p);
        }
    }
}

/// How one filter goes into Windows: its sublayer, its provider, its key (none: Windows gives one) and its lifetime's
/// flags (none: the session's).
struct Como {
    subcapa: GUID,
    proveedor: Option<GUID>,
    clave: Option<GUID>,
    banderas: u32,
}

/// One condition of WFP's, for one of the plan's; the values it points to go into `v`.
fn condiciones(
    c: &Condicion,
    app_id: *mut FWP_BYTE_BLOB,
    v: &mut Valores,
) -> Vec<FWPM_FILTER_CONDITION0> {
    // SAFETY: a zeroed condition is "empty"; every one returned has its three fields set.
    let nueva = || -> FWPM_FILTER_CONDITION0 { unsafe { std::mem::zeroed() } };
    let mut out = Vec::new();
    match *c {
        Condicion::Interfaz(luid) | Condicion::Llegada(luid) | Condicion::Salida(luid) => {
            let mut x = nueva();
            v.u64s.push(Box::new(luid));
            // by its LUID: in the connection-authorisation layers the interface of this computer's address; in the
            // forwarding layers the same field is the one a packet arrived on, and the forward interface the one it
            // leaves by
            x.fieldKey = if matches!(c, Condicion::Salida(_)) {
                FWPM_CONDITION_IP_FORWARD_INTERFACE
            } else {
                FWPM_CONDITION_IP_LOCAL_INTERFACE
            };
            x.matchType = FWP_MATCH_EQUAL;
            x.conditionValue.r#type = FWP_UINT64;
            if let Some(caja) = v.u64s.last_mut() {
                x.conditionValue.Anonymous.uint64 = &mut **caja;
            }
            out.push(x);
        }
        Condicion::Servicio => {
            let mut x = nueva();
            x.fieldKey = FWPM_CONDITION_ALE_APP_ID;
            x.matchType = FWP_MATCH_EQUAL;
            x.conditionValue.r#type = FWP_BYTE_BLOB_TYPE;
            x.conditionValue.Anonymous.byteBlob = app_id;
            out.push(x);
        }
        Condicion::Bucle => {
            let mut x = nueva();
            x.fieldKey = FWPM_CONDITION_FLAGS;
            x.matchType = FWP_MATCH_FLAGS_ALL_SET;
            x.conditionValue.r#type = FWP_UINT32;
            x.conditionValue.Anonymous.uint32 = FWP_CONDITION_FLAG_IS_LOOPBACK;
            out.push(x);
        }
        Condicion::Protocolo(p) => {
            let mut x = nueva();
            x.fieldKey = FWPM_CONDITION_IP_PROTOCOL;
            x.matchType = FWP_MATCH_EQUAL;
            x.conditionValue.r#type = FWP_UINT8;
            x.conditionValue.Anonymous.uint8 = p;
            out.push(x);
        }
        Condicion::PuertoLocal(p) | Condicion::PuertoRemoto(p) => {
            let mut x = nueva();
            x.fieldKey = if matches!(c, Condicion::PuertoLocal(_)) {
                FWPM_CONDITION_IP_LOCAL_PORT
            } else {
                FWPM_CONDITION_IP_REMOTE_PORT
            };
            x.matchType = FWP_MATCH_EQUAL;
            x.conditionValue.r#type = FWP_UINT16;
            x.conditionValue.Anonymous.uint16 = p;
            out.push(x);
        }
        Condicion::Remota(IpAddr::V4(a)) => {
            let mut x = nueva();
            x.fieldKey = FWPM_CONDITION_IP_REMOTE_ADDRESS;
            x.matchType = FWP_MATCH_EQUAL;
            x.conditionValue.r#type = FWP_UINT32;
            x.conditionValue.Anonymous.uint32 = u32::from(a);
            out.push(x);
        }
        Condicion::Remota(IpAddr::V6(a)) => {
            let mut x = nueva();
            v.v6.push(Box::new(FWP_BYTE_ARRAY16 {
                byteArray16: a.octets(),
            }));
            x.fieldKey = FWPM_CONDITION_IP_REMOTE_ADDRESS;
            x.matchType = FWP_MATCH_EQUAL;
            x.conditionValue.r#type = FWP_BYTE_ARRAY16_TYPE;
            if let Some(caja) = v.v6.last_mut() {
                x.conditionValue.Anonymous.byteArray16 = &mut **caja;
            }
            out.push(x);
        }
        Condicion::RemotaRed(a, largo) | Condicion::LocalRed(a, largo) => {
            let mut x = nueva();
            v.redes.push(Box::new(FWP_V6_ADDR_AND_MASK {
                addr: a.octets(),
                prefixLength: largo,
            }));
            x.fieldKey = if matches!(c, Condicion::RemotaRed(..)) {
                FWPM_CONDITION_IP_REMOTE_ADDRESS
            } else {
                FWPM_CONDITION_IP_LOCAL_ADDRESS
            };
            x.matchType = FWP_MATCH_EQUAL;
            x.conditionValue.r#type = FWP_V6_ADDR_MASK;
            if let Some(caja) = v.redes.last_mut() {
                x.conditionValue.Anonymous.v6AddrMask = &mut **caja;
            }
            out.push(x);
        }
        Condicion::Icmp { tipo, codigo } => {
            // WFP keeps the ICMP type in the local port field and the code in the remote one
            // (FWPM_CONDITION_ICMP_TYPE and FWPM_CONDITION_ICMP_CODE are those fields' names)
            for (campo, valor) in [
                (FWPM_CONDITION_IP_LOCAL_PORT, tipo),
                (FWPM_CONDITION_IP_REMOTE_PORT, codigo),
            ] {
                let mut x = nueva();
                x.fieldKey = campo;
                x.matchType = FWP_MATCH_EQUAL;
                x.conditionValue.r#type = FWP_UINT16;
                x.conditionValue.Anonymous.uint16 = u16::from(valor);
                out.push(x);
            }
        }
        Condicion::EntreMaquinasVirtuales => {
            let mut x = nueva();
            x.fieldKey = FWPM_CONDITION_L2_FLAGS;
            x.matchType = FWP_MATCH_EQUAL;
            x.conditionValue.r#type = FWP_UINT32;
            x.conditionValue.Anonymous.uint32 = FWP_CONDITION_L2_IS_VM2VM;
            out.push(x);
        }
    }
    out
}

/// Adds the filter `f` of the plan as `como` says, with `app_id` as the service's program file.
fn anade_filtro(
    engine: HANDLE,
    f: &Filtro,
    app_id: *mut FWP_BYTE_BLOB,
    como: &Como,
) -> Result<u64, ErrorCorte> {
    let mut valores = Valores {
        u64s: Vec::new(),
        v6: Vec::new(),
        redes: Vec::new(),
    };
    let mut cond: Vec<FWPM_FILTER_CONDITION0> = f
        .condiciones
        .iter()
        .flat_map(|c| condiciones(c, app_id, &mut valores))
        .collect();
    let mut nombre = ancho(f.nombre);
    let mut prov = como.proveedor.unwrap_or(GUID::from_u128(0));
    // SAFETY: zeroed filter; every field relied on is set below.
    let mut x: FWPM_FILTER0 = unsafe { std::mem::zeroed() };
    if let Some(clave) = como.clave {
        x.filterKey = clave;
    }
    if como.proveedor.is_some() {
        x.providerKey = &mut prov;
    }
    x.layerKey = capa_wfp(f.capa);
    x.subLayerKey = como.subcapa;
    x.displayData.name = nombre.as_mut_ptr();
    x.action.r#type = if f.permite {
        FWP_ACTION_PERMIT
    } else {
        FWP_ACTION_BLOCK
    };
    x.flags = como.banderas;
    if f.permite && f.firme {
        // the encrypted packets to the server may not be blocked by a lower sublayer either; every other permit
        // (Home Mode's among them) leaves the rest of Windows' firewall to decide after this one (1.0.55)
        x.flags |= FWPM_FILTER_FLAG_CLEAR_ACTION_RIGHT;
    }
    x.weight.r#type = FWP_UINT8;
    x.weight.Anonymous.uint8 = f.peso;
    x.numFilterConditions = u32::try_from(cond.len()).unwrap_or(0);
    x.filterCondition = if cond.is_empty() {
        std::ptr::null_mut()
    } else {
        cond.as_mut_ptr()
    };
    let mut id: u64 = 0;
    // SAFETY: the filter, its conditions and every value they point to (`valores`, `prov`, `nombre`) live through this
    // call; the out-id is owned here.
    let r = unsafe { FwpmFilterAdd0(engine, &x, std::ptr::null_mut(), &mut id) };
    drop(valores);
    if r != 0 {
        return Err(err(&format!("FwpmFilterAdd0 «{}»", f.nombre), r));
    }
    Ok(id)
}

/// The cut-off switch, on. Dropping it closes the dynamic session, and Windows removes every filter.
pub struct CorteVpn {
    motor: Motor,
    /// This session's provider and sublayer (new keys in every session; see the module's notes).
    proveedor: GUID,
    subcapa: GUID,
}

impl CorteVpn {
    /// Opens a dynamic session and adds every filter of `plan`, with `servicio` as the service's program file (its
    /// traffic, the encrypted packets to the server, is the only one let out of the tunnel). All or nothing: when one
    /// filter is refused, the session is closed and none stays.
    ///
    /// # Errors
    /// [`ErrorCorte::Sistema`] with the call Windows refused.
    pub fn activa(plan: &[Filtro], servicio: &Path) -> Result<Self, ErrorCorte> {
        let proveedor = clave_nueva()?;
        let subcapa = clave_nueva()?;
        let corte = Self {
            motor: Motor::abre(true)?,
            proveedor,
            subcapa,
        };
        corte.pon_proveedor_y_subcapa()?;
        let app = AppId::de(servicio)?;
        let como = Como {
            subcapa,
            proveedor: Some(proveedor),
            clave: None,
            banderas: 0,
        };
        for f in plan {
            // On an error `corte` is dropped: the dynamic session closes and Windows removes what was added.
            anade_filtro(corte.motor.0, f, app.0, &como)?;
        }
        Ok(corte)
    }

    /// This session's provider and sublayer: new keys, so neither can already exist (and nothing else hangs from them).
    fn pon_proveedor_y_subcapa(&self) -> Result<(), ErrorCorte> {
        let mut nombre = ancho("GUARDIANA — VPN");
        // SAFETY: zeroed provider with this session's key and a display name owned for the call.
        let mut p: FWPM_PROVIDER0 = unsafe { std::mem::zeroed() };
        p.providerKey = self.proveedor;
        p.displayData.name = nombre.as_mut_ptr();
        // SAFETY: valid provider pointer; default security descriptor.
        let r = unsafe { FwpmProviderAdd0(self.motor.0, &p, std::ptr::null_mut()) };
        if r != 0 {
            return Err(err("FwpmProviderAdd0", r));
        }
        let mut nombre = ancho("GUARDIANA — VPN, interruptor de corte");
        let mut clave = self.proveedor;
        // SAFETY: zeroed sublayer with this session's keys and an owned display name; the highest weight, as WireGuard's.
        let mut s: FWPM_SUBLAYER0 = unsafe { std::mem::zeroed() };
        s.subLayerKey = self.subcapa;
        s.providerKey = &mut clave;
        s.displayData.name = nombre.as_mut_ptr();
        s.weight = 0xFFFF;
        // SAFETY: valid sublayer pointer; default security descriptor.
        let r = unsafe { FwpmSubLayerAdd0(self.motor.0, &s, std::ptr::null_mut()) };
        if r != 0 {
            return Err(err("FwpmSubLayerAdd0", r));
        }
        Ok(())
    }
}

// ---- Strict mode (ronda 91; docs/VPN.md, «Modo estricto») ----

/// One test of strict mode's objects at a time: they share their fixed keys in Windows (this module's tests and
/// `tunel_win`'s).
#[cfg(test)]
pub(crate) static EN_PRUEBA: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// What [`pon_estricto`] put in Windows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puesto {
    /// The persistent filters.
    pub persistentes: usize,
    /// The boot-time filters and where they went («subcapa propia», or Windows' «subcapa universal» if it did not take
    /// them in strict mode's own); or why Windows refused them both ways. Strict mode then still holds with the
    /// persistent ones, from the moment Windows' filtering service starts.
    pub arranque: Result<(usize, &'static str), String>,
}

/// Puts strict mode's block `plan` in Windows, with `servicio` as the service's program file: its persistent filters
/// in one transaction that also takes away the ones of before (the computer is never without one), under strict mode's
/// provider and sublayer (added if missing); then its boot-time filters, in another.
///
/// # Errors
/// [`ErrorCorte::Sistema`] with the call Windows refused; the persistent filters of before then stay as they were.
pub fn pon_estricto(plan: &Estricto, servicio: &Path) -> Result<Puesto, ErrorCorte> {
    let maximo = usize::from(claves::MAXIMO);
    if plan.persistentes.len() > maximo || plan.arranque.len() > maximo {
        return Err(ErrorCorte::Sistema(format!(
            "el modo estricto tiene más de {maximo} filtros"
        )));
    }
    let motor = Motor::abre(false)?;
    let app = AppId::de(servicio)?;
    // asked outside the transaction: inside it, nothing is asked that could fail
    let hay_proveedor = motor.hay_proveedor()?;
    let hay_subcapa = motor.hay_subcapa()?;
    let viejos = motor.existentes(claves::PERSISTENTES)?;
    let persistentes = motor.en_transaccion(|| {
        if !hay_proveedor {
            pon_proveedor(&motor)?;
        }
        if !hay_subcapa {
            pon_subcapa(&motor)?;
        }
        for (clave, _) in &viejos {
            motor.borra_filtro(clave)?;
        }
        for (i, f) in plan.persistentes.iter().enumerate() {
            let como = Como {
                subcapa: guid(claves::SUBCAPA),
                proveedor: Some(guid(claves::PROVEEDOR)),
                clave: Some(clave_de(claves::PERSISTENTES, i)),
                banderas: FWPM_FILTER_FLAG_PERSISTENT,
            };
            anade_filtro(motor.0, f, app.0, &como)?;
        }
        Ok(plan.persistentes.len())
    })?;
    let arranque = pon_arranque(&motor, &plan.arranque, &app);
    Ok(Puesto {
        persistentes,
        arranque,
    })
}

/// Strict mode's provider, persistent and naming GUARDIANA's service.
fn pon_proveedor(motor: &Motor) -> Result<(), ErrorCorte> {
    let mut nombre = ancho("GUARDIANA — VPN, modo estricto");
    let mut servicio = ancho(SERVICIO);
    // SAFETY: zeroed provider with its fixed key, and a display name and a service name owned for the call.
    let mut p: FWPM_PROVIDER0 = unsafe { std::mem::zeroed() };
    p.providerKey = guid(claves::PROVEEDOR);
    p.displayData.name = nombre.as_mut_ptr();
    p.flags = PROVEEDOR_PERSISTENTE;
    p.serviceName = servicio.as_mut_ptr();
    // SAFETY: valid provider pointer; default security descriptor.
    let r = unsafe { FwpmProviderAdd0(motor.0, &p, std::ptr::null_mut()) };
    if r != 0 {
        return Err(err("FwpmProviderAdd0", r));
    }
    Ok(())
}

/// Strict mode's sublayer, persistent, under its provider, with the highest weight.
fn pon_subcapa(motor: &Motor) -> Result<(), ErrorCorte> {
    let mut nombre = ancho("GUARDIANA — VPN, modo estricto: el bloqueo");
    let mut proveedor = guid(claves::PROVEEDOR);
    // SAFETY: zeroed sublayer with its fixed keys and an owned display name.
    let mut s: FWPM_SUBLAYER0 = unsafe { std::mem::zeroed() };
    s.subLayerKey = guid(claves::SUBCAPA);
    s.displayData.name = nombre.as_mut_ptr();
    s.flags = SUBCAPA_PERSISTENTE;
    s.providerKey = &mut proveedor;
    s.weight = 0xFFFF;
    // SAFETY: valid sublayer pointer; default security descriptor.
    let r = unsafe { FwpmSubLayerAdd0(motor.0, &s, std::ptr::null_mut()) };
    if r != 0 {
        return Err(err("FwpmSubLayerAdd0", r));
    }
    Ok(())
}

/// The boot-time filters, in place of the ones of before: in strict mode's own sublayer, under its provider; if Windows
/// does not take them there, in its universal sublayer.
fn pon_arranque(
    motor: &Motor,
    filtros: &[Filtro],
    app: &AppId,
) -> Result<(usize, &'static str), String> {
    let viejos = motor
        .existentes(claves::ARRANQUE)
        .map_err(|e| e.to_string())?;
    let mut fallos = Vec::new();
    for (donde, subcapa, proveedor) in [
        (
            "subcapa propia",
            guid(claves::SUBCAPA),
            Some(guid(claves::PROVEEDOR)),
        ),
        ("subcapa universal", guid(SUBCAPA_UNIVERSAL), None),
    ] {
        let r = motor.en_transaccion(|| {
            for (clave, _) in &viejos {
                motor.borra_filtro(clave)?;
            }
            for (i, f) in filtros.iter().enumerate() {
                let como = Como {
                    subcapa,
                    proveedor,
                    clave: Some(clave_de(claves::ARRANQUE, i)),
                    banderas: FWPM_FILTER_FLAG_BOOTTIME,
                };
                anade_filtro(motor.0, f, app.0, &como)?;
            }
            Ok(filtros.len())
        });
        match r {
            Ok(n) => return Ok((n, donde)),
            Err(e) => fallos.push(format!("{donde}: {e}")),
        }
    }
    Err(fallos.join("; "))
}

/// What [`quita_estricto`] removed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Quitado {
    /// Filters removed, persistent and boot-time.
    pub filtros: usize,
    /// Whether the sublayer and the provider had to stay: while the service has the tunnel's own permit in the
    /// sublayer ([`PermisoTunel`]), Windows keeps them until it goes. By themselves they block nothing.
    pub en_uso: bool,
}

/// Removes everything of strict mode from Windows: its filters (every key it may have used, persistent and boot-time,
/// whatever its layer: the forwarding ones of 1.0.56 too), its sublayer and its provider. Nothing there is no error.
///
/// # Errors
/// [`ErrorCorte::Sistema`] with the call Windows refused ([`sin_permiso`] says when it is for want of an
/// administrator).
pub fn quita_estricto() -> Result<Quitado, ErrorCorte> {
    let motor = Motor::abre(false)?;
    let mut viejos = motor.existentes(claves::PERSISTENTES)?;
    viejos.extend(motor.existentes(claves::ARRANQUE)?);
    if !viejos.is_empty() {
        motor.en_transaccion(|| {
            for (clave, _) in &viejos {
                motor.borra_filtro(clave)?;
            }
            Ok(())
        })?;
    }
    let mut en_uso = false;
    let subcapa = guid(claves::SUBCAPA);
    // SAFETY: deleting by a key owned here.
    match unsafe { FwpmSubLayerDeleteByKey0(motor.0, &subcapa) } {
        0 | SUBCAPA_NO_ESTA => {}
        EN_USO => en_uso = true,
        r => return Err(err("FwpmSubLayerDeleteByKey0", r)),
    }
    let proveedor = guid(claves::PROVEEDOR);
    // SAFETY: deleting by a key owned here.
    match unsafe { FwpmProviderDeleteByKey0(motor.0, &proveedor) } {
        0 | PROVEEDOR_NO_ESTA => {}
        EN_USO => en_uso = true,
        r => return Err(err("FwpmProviderDeleteByKey0", r)),
    }
    Ok(Quitado {
        filtros: viejos.len(),
        en_uso,
    })
}

/// What of strict mode Windows has right now, asked to Windows itself.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EnWindows {
    /// Its provider.
    pub proveedor: bool,
    /// Its sublayer.
    pub subcapa: bool,
    /// Its persistent filters.
    pub persistentes: usize,
    /// Its boot-time filters.
    pub arranque: usize,
    /// Of those, persistent and boot-time, the ones in the forwarding layers (the block on what Windows forwards).
    pub reenvio: usize,
}

impl EnWindows {
    /// Whether anything of strict mode is in Windows.
    #[must_use]
    pub fn alguno(&self) -> bool {
        self.proveedor || self.subcapa || self.persistentes > 0 || self.arranque > 0
    }
}

/// What of strict mode Windows has right now.
///
/// # Errors
/// [`ErrorCorte::Sistema`] with the call Windows refused.
pub fn en_windows() -> Result<EnWindows, ErrorCorte> {
    let motor = Motor::abre(false)?;
    let persistentes = motor.existentes(claves::PERSISTENTES)?;
    let arranque = motor.existentes(claves::ARRANQUE)?;
    let reenvio = persistentes
        .iter()
        .chain(&arranque)
        .filter(|(_, en_reenvio)| *en_reenvio)
        .count();
    Ok(EnWindows {
        proveedor: motor.hay_proveedor()?,
        subcapa: motor.hay_subcapa()?,
        persistentes: persistentes.len(),
        arranque: arranque.len(),
        reenvio,
    })
}

/// The tunnel's own permit (`guardiana_tunel::corte::del_tunel`) in strict mode's sublayer, from a dynamic session of
/// its own: Windows removes it when this value is dropped or the process ends, as it removes the tunnel's adapter.
/// Strict mode's block, apart, stays.
pub struct PermisoTunel {
    _motor: Motor,
}

impl PermisoTunel {
    /// Puts `plan`, the tunnel's own permit, in strict mode's sublayer.
    ///
    /// # Errors
    /// [`ErrorCorte::Sistema`] when strict mode's block is not in Windows (its sublayer is missing) or Windows refuses a
    /// filter; then nothing of it stays.
    pub fn pon(plan: &[Filtro]) -> Result<Self, ErrorCorte> {
        let motor = Motor::abre(true)?;
        let como = Como {
            subcapa: guid(claves::SUBCAPA),
            proveedor: None,
            clave: None,
            banderas: 0,
        };
        for f in plan {
            anade_filtro(motor.0, f, std::ptr::null_mut(), &como)?;
        }
        Ok(Self { _motor: motor })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use guardiana_tunel::corte::Servidor;

    /// A server reached over IPv4 and one over IPv6 (documentation addresses: nothing is sent to them).
    const EXTREMOS: [&str; 2] = ["192.0.2.1:51820", "[2001:db8::1]:51820"];

    /// Whether `f` may go into Windows here without cutting the test machine's own network: every permit, and the block
    /// on what Windows forwards (the test machine forwards nothing of its own: its programs leave by the interface of
    /// their own address). Not «todo lo demás» nor DNS outside the tunnel.
    fn no_corta_la_red(f: &Filtro) -> bool {
        f.permite || f.capa.es_reenvio()
    }

    /// The plan for a server at `extremo` without the filters that would block the test machine's own network.
    fn sin_cortar_la_red(extremo: &str) -> Vec<Filtro> {
        guardiana_tunel::corte::plan(0x0123_4567_89ab_cdef, extremo.parse().unwrap())
            .into_iter()
            .filter(no_corta_la_red)
            .collect()
    }

    /// What Windows answers when a new dynamic session hangs a sublayer from `proveedor`.
    fn subcapa_colgada_de(proveedor: GUID) -> u32 {
        // SAFETY: as in `activa`: a dynamic session of this test's own, with owned values, closed before returning.
        unsafe {
            let mut sesion: FWPM_SESSION0 = std::mem::zeroed();
            sesion.flags = FWPM_SESSION_FLAG_DYNAMIC;
            let mut engine: HANDLE = std::ptr::null_mut();
            let r = FwpmEngineOpen0(
                std::ptr::null(),
                RPC_C_AUTHN_WINNT,
                std::ptr::null(),
                &sesion,
                &mut engine,
            );
            assert_eq!(r, 0, "FwpmEngineOpen0 = 0x{r:08x}");
            let mut nombre = ancho("GUARDIANA — prueba");
            let mut clave = proveedor;
            let mut s: FWPM_SUBLAYER0 = std::mem::zeroed();
            s.subLayerKey = clave_nueva().unwrap();
            s.providerKey = &mut clave;
            s.displayData.name = nombre.as_mut_ptr();
            let r = FwpmSubLayerAdd0(engine, &s, std::ptr::null_mut());
            FwpmEngineClose0(engine);
            r
        }
    }

    #[test]
    fn la_falta_de_permiso_se_reconoce() {
        assert!(sin_permiso(&err("FwpmFilterDeleteByKey0", 5)));
        assert!(sin_permiso(&err("FwpmEngineOpen0", 0x8007_0005)));
        assert!(!sin_permiso(&err("FwpmFilterDeleteByKey0", EN_USO)));
        assert!(!sin_permiso(&ErrorCorte::NoSoportado));
    }

    /// On a real Windows, as an administrator: every filter of the plan is accepted by Windows (each condition is
    /// of a type its field takes, in a layer that has that field), except the ones that would block the test machine's
    /// own network, and the session closes cleanly. For a server of each family (1.0.55: the service's permit goes in
    /// its family's layers, with the server's address, port and protocol), with Home Mode's permits (the service's
    /// program file, a protocol and a local port, in the accepted incoming layer), and with what Windows forwards
    /// (1.0.56): the tunnel's LUID as the interface a forwarded packet leaves by and as the one it arrived on, and the
    /// block on the rest, in both forwarding layers.
    #[test]
    #[ignore = "necesita Windows como administrador"]
    fn windows_acepta_cada_filtro_del_plan() {
        let yo = std::env::current_exe().unwrap();
        for extremo in EXTREMOS {
            let plan = sin_cortar_la_red(extremo);
            assert!(plan.len() > 20);
            // the firm permit is the one to the server, in its two layers; Home Mode's are not firm
            assert_eq!(plan.iter().filter(|f| f.firme).count(), 2, "{extremo}");
            assert_eq!(
                plan.iter()
                    .filter(|f| f.nombre == "GUARDIANA VPN: Modo Hogar" && !f.firme)
                    .count(),
                4
            );
            // lo reenviado: hacia el túnel y desde él, y el bloqueo de lo demás, en las dos capas del reenvío
            assert_eq!(
                plan.iter().filter(|f| f.capa.es_reenvio()).count(),
                6,
                "{extremo}"
            );
            let corte = CorteVpn::activa(&plan, &yo).unwrap();
            drop(corte);
        }
    }

    /// With the antivirus cuts' firewall session open (the service holds it for as long as it runs, under a provider of
    /// that session's own since 1.0.55), the switch still goes on: what failed on a person's computer on 2 Nov 2026
    /// («FwpmSubLayerAdd0 = 0x8032000c»), when that provider had a fixed key. The cause is checked too: a sublayer hung
    /// from that session's provider by another session is refused.
    #[test]
    #[ignore = "necesita Windows como administrador"]
    fn con_los_cortes_del_antivirus_abiertos() {
        let cortes = crate::cortafuegos::Cortafuegos::abrir().unwrap();
        let r = subcapa_colgada_de(cortes.proveedor());
        eprintln!("otra sesión, con el proveedor de los cortes del antivirus: 0x{r:08x}");
        // FWP_E_WRONG_SESSION: lo que se vio en el equipo de esa persona (0x8032000c)
        assert_eq!(
            r, 0x8032_000c,
            "otra sesión con el proveedor de los cortes del antivirus: 0x{r:08x}"
        );
        let yo = std::env::current_exe().unwrap();
        let corte = CorteVpn::activa(&sin_cortar_la_red(EXTREMOS[0]), &yo).unwrap();
        drop(corte);
        drop(cortes);
    }

    /// Two switches at once (a reconnection that turns the new one on before the old one is gone) do not refuse
    /// each other.
    #[test]
    #[ignore = "necesita Windows como administrador"]
    fn dos_a_la_vez() {
        let yo = std::env::current_exe().unwrap();
        let a = CorteVpn::activa(&sin_cortar_la_red(EXTREMOS[0]), &yo).unwrap();
        let b = CorteVpn::activa(&sin_cortar_la_red(EXTREMOS[0]), &yo).unwrap();
        drop(a);
        drop(b);
    }

    /// Strict mode's block without the blocks that would cut the test machine's network: its permits, and its block on
    /// what Windows forwards.
    fn estricto_sin_cortar_la_red(extremo: &str) -> Estricto {
        let extremo: std::net::SocketAddr = extremo.parse().unwrap();
        let e = guardiana_tunel::corte::estricto(Servidor {
            extremo: Some(extremo),
            alta: Some(std::net::SocketAddr::new(extremo.ip(), 443)),
        });
        Estricto {
            persistentes: e.persistentes.into_iter().filter(no_corta_la_red).collect(),
            arranque: e.arranque.into_iter().filter(no_corta_la_red).collect(),
        }
    }

    /// On a real Windows, as an administrator (strict mode, ronda 91): Windows takes every persistent filter and the
    /// boot-time ones under strict mode's fixed keys, and says so when asked; putting the set again replaces it and
    /// never doubles it; the tunnel's own permit goes into the same sublayer from a dynamic session; removing takes
    /// every filter at once, and the sublayer and the provider once the tunnel's permit is gone. The block on what
    /// Windows forwards (1.0.56) goes in both forwarding layers, persistent and boot-time, as Windows itself says when
    /// asked for each filter's layer, and goes with the rest; the tunnel's permit carries what is forwarded through
    /// it. Only permits and that block: the test machine's network is never cut.
    #[test]
    #[ignore = "necesita Windows como administrador"]
    fn el_modo_estricto_en_windows() {
        let _uno = EN_PRUEBA
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let yo = std::env::current_exe().unwrap();
        let reenvio = |p: &[Filtro]| p.iter().filter(|f| f.capa.es_reenvio()).count();
        // a clean start: what a test of before may have left
        let _ = quita_estricto().unwrap();
        assert!(!en_windows().unwrap().alguno());
        for extremo in EXTREMOS {
            let plan = estricto_sin_cortar_la_red(extremo);
            // el bloqueo de lo reenviado, uno en cada capa del reenvío, en los dos conjuntos
            assert_eq!(
                (reenvio(&plan.persistentes), reenvio(&plan.arranque)),
                (2, 2)
            );
            let puesto = pon_estricto(&plan, &yo).unwrap();
            eprintln!("modo estricto puesto ({extremo}): {puesto:?}");
            assert_eq!(puesto.persistentes, plan.persistentes.len());
            let en = en_windows().unwrap();
            assert!(en.proveedor && en.subcapa, "{en:?}");
            assert_eq!(en.persistentes, plan.persistentes.len());
            match &puesto.arranque {
                Ok((n, donde)) => {
                    eprintln!("filtros de arranque: {n}, en la {donde}");
                    assert_eq!(en.arranque, *n);
                    assert_eq!(en.reenvio, 4, "{en:?}");
                }
                Err(e) => {
                    eprintln!("Windows no tomó los filtros de arranque: {e}");
                    assert_eq!(en.reenvio, 2, "{en:?}");
                }
            }
            // otra vez (el servicio que vuelve a arrancar, una actualización): se cambian, nunca se duplican
            let otra = pon_estricto(&plan, &yo).unwrap();
            assert_eq!(otra.persistentes, plan.persistentes.len());
            assert_eq!(en_windows().unwrap(), en);
        }
        // el permiso del túnel, en la misma subcapa, desde una sesión dinámica: también lo reenviado hacia el túnel y
        // desde él
        let del_tunel = guardiana_tunel::corte::del_tunel(0x0123_4567_89ab_cdef);
        assert_eq!(reenvio(&del_tunel), 4);
        let permiso = PermisoTunel::pon(&del_tunel).unwrap();
        let q = quita_estricto().unwrap();
        assert!(q.filtros > 0 && q.en_uso, "{q:?}");
        let en = en_windows().unwrap();
        assert_eq!(
            (en.persistentes, en.arranque, en.reenvio),
            (0, 0, 0),
            "{en:?}"
        );
        drop(permiso);
        let q = quita_estricto().unwrap();
        assert_eq!(q, Quitado::default());
        assert!(!en_windows().unwrap().alguno());
        // sin bloqueo puesto, el permiso del túnel no tiene dónde ir
        assert!(PermisoTunel::pon(&guardiana_tunel::corte::del_tunel(1)).is_err());
    }
}
