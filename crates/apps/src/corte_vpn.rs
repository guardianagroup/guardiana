//! The VPN's cut-off switch on Windows (in preparation; docs/VPN.md): `guardiana_tunel::corte`'s plan turned into
//! Windows Filtering Platform filters, in a **dynamic** session of their own, under GUARDIANA's provider and a sublayer
//! of the VPN's own. Windows removes every one of them the moment the session closes: when this value is dropped, and
//! when the service ends for any reason, so a crash never leaves the computer without network.
//!
//! The plan names the conditions; here each becomes WFP's own: the tunnel's interface by its LUID, the service by its
//! program file (`FwpmGetAppIdFromFileName0`), loopback by its flag, protocols, ports, addresses and networks, ICMPv6
//! types and codes (which WFP keeps in the port fields), and Hyper-V's machine-to-machine flag.
//!
//! `unsafe` is used here, as the project allows with justification (see `cortafuegos/ventanas.rs`, the same API): every
//! value a condition points to is owned here and outlives the call that adds the filter, every result is checked, and
//! the engine is closed on drop.
#![allow(unsafe_code)]

use std::net::IpAddr;
use std::path::Path;

use guardiana_tunel::corte::{Capa, Condicion, Filtro};
use windows_sys::core::GUID;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::NetworkManagement::WindowsFilteringPlatform::*;

use crate::cortafuegos::ErrorCorte;

/// GUARDIANA's provider (the same as the program's other cuts) and the VPN's own sublayer.
const PROVEEDOR: GUID = GUID::from_u128(0x6705_1a2e_9c33_4e77_b1a4_2f0e_7c11_9a01);
const SUBCAPA_VPN: GUID = GUID::from_u128(0x6705_1a2e_9c33_4e77_b1a4_2f0e_7c11_9a03);
/// `RPC_C_AUTHN_WINNT`.
const RPC_C_AUTHN_WINNT: u32 = 10;
/// `FWP_E_ALREADY_EXISTS`.
const YA_EXISTE: u32 = 0x8032_0009;

fn err(llamada: &str, code: u32) -> ErrorCorte {
    ErrorCorte::Sistema(format!("{llamada} = 0x{code:08x}"))
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

/// The cut-off switch, on. Dropping it closes the dynamic session, and Windows removes every filter.
pub struct CorteVpn {
    engine: HANDLE,
}

// The handle is used only through the engine's own calls, which are thread-safe.
unsafe impl Send for CorteVpn {}
unsafe impl Sync for CorteVpn {}

impl CorteVpn {
    /// Opens a dynamic session and adds every filter of `plan`, with `servicio` as the service's program file (its
    /// traffic, the encrypted packets to the server, is the only one let out of the tunnel). All or nothing: when one
    /// filter is refused, the session is closed and none stays.
    ///
    /// # Errors
    /// [`ErrorCorte::Sistema`] with the call Windows refused.
    pub fn activa(plan: &[Filtro], servicio: &Path) -> Result<Self, ErrorCorte> {
        // SAFETY: a zeroed FWPM_SESSION0 is a valid "no options" session; only the dynamic flag is set.
        let mut sesion: FWPM_SESSION0 = unsafe { std::mem::zeroed() };
        sesion.flags = FWPM_SESSION_FLAG_DYNAMIC;
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
        let corte = Self { engine };
        corte.asegura_proveedor_y_subcapa()?;
        let ruta = ancho(&servicio.to_string_lossy());
        let mut app_id: *mut FWP_BYTE_BLOB = std::ptr::null_mut();
        // SAFETY: a NUL-terminated path owned for the call; the out-pointer is freed below.
        let r = unsafe { FwpmGetAppIdFromFileName0(ruta.as_ptr(), &mut app_id) };
        if r != 0 || app_id.is_null() {
            return Err(err("FwpmGetAppIdFromFileName0", r));
        }
        let mut resultado = Ok(());
        for f in plan {
            if let Err(e) = corte.anade(f, app_id) {
                resultado = Err(e);
                break;
            }
        }
        // SAFETY: app_id was given by FwpmGetAppIdFromFileName0; freed once.
        unsafe {
            let mut p = app_id.cast::<core::ffi::c_void>();
            FwpmFreeMemory0(&mut p);
        }
        // On an error `corte` is dropped here: the dynamic session closes and Windows removes what was added.
        resultado.map(|()| corte)
    }

    fn asegura_proveedor_y_subcapa(&self) -> Result<(), ErrorCorte> {
        let mut nombre = ancho("GUARDIANA");
        // SAFETY: zeroed provider with our key and a display name owned for the call.
        let mut p: FWPM_PROVIDER0 = unsafe { std::mem::zeroed() };
        p.providerKey = PROVEEDOR;
        p.displayData.name = nombre.as_mut_ptr();
        // SAFETY: valid provider pointer; default security descriptor.
        let r = unsafe { FwpmProviderAdd0(self.engine, &p, std::ptr::null_mut()) };
        if r != 0 && r != YA_EXISTE {
            return Err(err("FwpmProviderAdd0", r));
        }
        let mut nombre = ancho("GUARDIANA — VPN, interruptor de corte");
        let mut clave = PROVEEDOR;
        // SAFETY: zeroed sublayer with our keys and an owned display name; the highest weight, as WireGuard's.
        let mut s: FWPM_SUBLAYER0 = unsafe { std::mem::zeroed() };
        s.subLayerKey = SUBCAPA_VPN;
        s.providerKey = &mut clave;
        s.displayData.name = nombre.as_mut_ptr();
        s.weight = 0xFFFF;
        // SAFETY: valid sublayer pointer; default security descriptor.
        let r = unsafe { FwpmSubLayerAdd0(self.engine, &s, std::ptr::null_mut()) };
        if r != 0 && r != YA_EXISTE {
            return Err(err("FwpmSubLayerAdd0", r));
        }
        Ok(())
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
            Condicion::Interfaz(luid) => {
                let mut x = nueva();
                v.u64s.push(Box::new(luid));
                x.fieldKey = FWPM_CONDITION_IP_LOCAL_INTERFACE;
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

    fn anade(&self, f: &Filtro, app_id: *mut FWP_BYTE_BLOB) -> Result<u64, ErrorCorte> {
        let mut valores = Valores {
            u64s: Vec::new(),
            v6: Vec::new(),
            redes: Vec::new(),
        };
        let mut cond: Vec<FWPM_FILTER_CONDITION0> = f
            .condiciones
            .iter()
            .flat_map(|c| Self::condiciones(c, app_id, &mut valores))
            .collect();
        let mut nombre = ancho(f.nombre);
        let mut prov = PROVEEDOR;
        // SAFETY: zeroed filter; every field relied on is set below.
        let mut x: FWPM_FILTER0 = unsafe { std::mem::zeroed() };
        x.providerKey = &mut prov;
        x.layerKey = capa_wfp(f.capa);
        x.subLayerKey = SUBCAPA_VPN;
        x.displayData.name = nombre.as_mut_ptr();
        x.action.r#type = if f.permite {
            FWP_ACTION_PERMIT
        } else {
            FWP_ACTION_BLOCK
        };
        if f.permite && f.condiciones.contains(&Condicion::Servicio) {
            // the service's own traffic may not be blocked by a lower sublayer either
            x.flags = FWPM_FILTER_FLAG_CLEAR_ACTION_RIGHT;
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
        // SAFETY: the filter, its conditions and every value they point to (`valores`, `prov`, `nombre`) live through
        // this call; the out-id is owned here.
        let r = unsafe { FwpmFilterAdd0(self.engine, &x, std::ptr::null_mut(), &mut id) };
        drop(valores);
        if r != 0 {
            return Err(err(&format!("FwpmFilterAdd0 «{}»", f.nombre), r));
        }
        Ok(id)
    }
}

impl Drop for CorteVpn {
    fn drop(&mut self) {
        if !self.engine.is_null() {
            // SAFETY: the handle came from FwpmEngineOpen0 and is not used after this. Closing a dynamic session
            // removes every object it added.
            unsafe {
                FwpmEngineClose0(self.engine);
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// On a real Windows, as an administrator: every filter of the plan is accepted by Windows (each condition is
    /// of a type its field takes), except the ones that would block the test machine's own network (everything else,
    /// and other DNS), and the session closes cleanly.
    #[test]
    #[ignore = "necesita Windows como administrador"]
    fn windows_acepta_cada_filtro_del_plan() {
        let dns = ["10.66.0.1".parse().unwrap(), "fd66::1".parse().unwrap()];
        let plan: Vec<Filtro> = guardiana_tunel::corte::plan(0x0123_4567_89ab_cdef, &dns)
            .into_iter()
            .filter(|f| f.permite)
            .collect();
        assert!(plan.len() > 20);
        let yo = std::env::current_exe().unwrap();
        let corte = CorteVpn::activa(&plan, &yo).unwrap();
        drop(corte);
    }
}
