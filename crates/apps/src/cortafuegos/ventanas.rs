//! Windows half of the clean cut: declarative WFP filters that block a program's outgoing
//! connections, from a service in user mode, with no kernel driver of our own.
//!
//! The session is **dynamic**: every object GUARDIANA adds (its provider, its sublayer and its
//! filters) is removed by Windows itself the moment this process ends. So a crash can never leave
//! an antivirus blocked behind our back — the fail-safe the owner asked for («siempre
//! reversible»). While GUARDIANA runs, the service keeps the wanted set in the ledger and
//! re-applies it on start.
//!
//! `unsafe` is used here, as the project allows with justification (CLAUDE.md): there is no safe
//! wrapper for WFP. Every call is checked, every buffer is owned and sized, the engine handle is
//! closed on drop, and a failure returns an error, never a silent wrong state.

use std::net::IpAddr;

use windows_sys::core::GUID;
use windows_sys::Win32::Foundation::{HANDLE, HANDLE as HFW};
use windows_sys::Win32::NetworkManagement::WindowsFilteringPlatform::*;

use super::{Destino, ErrorCorte};

/// GUARDIANA's own provider and sublayer, so its filters can always be told apart and removed.
const PROVEEDOR: GUID = GUID::from_u128(0x6705_1a2e_9c33_4e77_b1a4_2f0e_7c11_9a01);
const SUBCAPA: GUID = GUID::from_u128(0x6705_1a2e_9c33_4e77_b1a4_2f0e_7c11_9a02);
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

/// The filter engine with GUARDIANA's provider and sublayer in place, and the ids of the filters
/// it has added this run.
pub(crate) struct Motor {
    engine: HFW,
    ids: std::sync::Mutex<Vec<u64>>,
}

// The handle is used only through the engine's own calls, which are thread-safe.
unsafe impl Send for Motor {}
unsafe impl Sync for Motor {}

impl Motor {
    pub(crate) fn abrir() -> Result<Self, ErrorCorte> {
        // A dynamic session: everything we add is cleaned up when this process ends.
        // SAFETY: a zeroed FWPM_SESSION0 is a valid "no options" session; we set only the flag.
        let mut sesion: FWPM_SESSION0 = unsafe { std::mem::zeroed() };
        sesion.flags = FWPM_SESSION_FLAG_DYNAMIC;
        let mut engine: HANDLE = std::ptr::null_mut();
        // SAFETY: server name null (local), our session, out-handle owned here.
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
        let motor = Self {
            engine,
            ids: std::sync::Mutex::new(Vec::new()),
        };
        motor.asegura_proveedor()?;
        motor.asegura_subcapa()?;
        Ok(motor)
    }

    fn asegura_proveedor(&self) -> Result<(), ErrorCorte> {
        let mut nombre = ancho("GUARDIANA");
        // SAFETY: zeroed provider with our key and a display name we own for the call.
        let mut p: FWPM_PROVIDER0 = unsafe { std::mem::zeroed() };
        p.providerKey = PROVEEDOR;
        p.displayData.name = nombre.as_mut_ptr();
        // SAFETY: valid provider pointer; security descriptor null (default).
        let r = unsafe { FwpmProviderAdd0(self.engine, &p, std::ptr::null_mut()) };
        if r == 0 || r == YA_EXISTE {
            Ok(())
        } else {
            Err(err("FwpmProviderAdd0", r))
        }
    }

    fn asegura_subcapa(&self) -> Result<(), ErrorCorte> {
        let mut nombre = ancho("GUARDIANA — cortes del antivirus");
        let mut clave = PROVEEDOR;
        // SAFETY: zeroed sublayer with our keys and an owned display name.
        let mut s: FWPM_SUBLAYER0 = unsafe { std::mem::zeroed() };
        s.subLayerKey = SUBCAPA;
        s.providerKey = &mut clave;
        s.displayData.name = nombre.as_mut_ptr();
        s.weight = 0x8000;
        // SAFETY: valid sublayer pointer; default security descriptor.
        let r = unsafe { FwpmSubLayerAdd0(self.engine, &s, std::ptr::null_mut()) };
        if r == 0 || r == YA_EXISTE {
            Ok(())
        } else {
            Err(err("FwpmSubLayerAdd0", r))
        }
    }

    pub(crate) fn cortar(&self, ruta: &str, destino: &Destino) -> Result<Vec<u64>, ErrorCorte> {
        let ruta_w = ancho(ruta);
        let mut app_id: *mut FWP_BYTE_BLOB = std::ptr::null_mut();
        // SAFETY: the path is a valid wide string; the out-pointer is owned and freed below.
        let r = unsafe { FwpmGetAppIdFromFileName0(ruta_w.as_ptr(), &mut app_id) };
        if r != 0 || app_id.is_null() {
            return Err(err("FwpmGetAppIdFromFileName0", r));
        }

        // Which address families to block: a given IP blocks only its own family; no IP blocks
        // both (so UDP/QUIC are cut too, never only TCP).
        let ip = (!destino.ip.is_empty())
            .then(|| destino.ip.parse::<IpAddr>().ok())
            .flatten();
        let familias: &[bool] = match ip {
            Some(IpAddr::V4(_)) => &[true],  // v4 only
            Some(IpAddr::V6(_)) => &[false], // v6 only
            None => &[true, false],
        };

        let mut nuevos = Vec::new();
        let mut resultado = Ok(());
        for &v4 in familias {
            match self.un_filtro(app_id, v4, ip.as_ref(), destino.puerto) {
                Ok(id) => nuevos.push(id),
                Err(e) => {
                    resultado = Err(e);
                    break;
                }
            }
        }
        // SAFETY: app_id was given to us by FwpmGetAppIdFromFileName0; free it once.
        unsafe {
            let mut p = app_id.cast::<core::ffi::c_void>();
            FwpmFreeMemory0(&mut p);
        }

        if let Err(e) = resultado {
            // Roll back whatever was added, so a half-applied cut never lingers.
            let _ = self.quitar(&nuevos);
            return Err(e);
        }
        if let Ok(mut g) = self.ids.lock() {
            g.extend_from_slice(&nuevos);
        }
        Ok(nuevos)
    }

    fn un_filtro(
        &self,
        app_id: *mut FWP_BYTE_BLOB,
        v4: bool,
        ip: Option<&IpAddr>,
        puerto: u16,
    ) -> Result<u64, ErrorCorte> {
        // The conditions, kept alive for the duration of the add call.
        let mut cond: Vec<FWPM_FILTER_CONDITION0> = Vec::new();

        // 1) The program (its path), always.
        // SAFETY: zeroed condition; we set its three fields.
        let mut c_app: FWPM_FILTER_CONDITION0 = unsafe { std::mem::zeroed() };
        c_app.fieldKey = FWPM_CONDITION_ALE_APP_ID;
        c_app.matchType = FWP_MATCH_EQUAL;
        c_app.conditionValue.r#type = FWP_BYTE_BLOB_TYPE;
        c_app.conditionValue.Anonymous.byteBlob = app_id;
        cond.push(c_app);

        // 2) The destination address, when given. The mask values must outlive the call.
        let mut v4m = FWP_V4_ADDR_AND_MASK { addr: 0, mask: 0 };
        let mut v6m = FWP_V6_ADDR_AND_MASK {
            addr: [0; 16],
            prefixLength: 0,
        };
        if let Some(ip) = ip {
            // SAFETY: zeroed condition; fields set below.
            let mut c_ip: FWPM_FILTER_CONDITION0 = unsafe { std::mem::zeroed() };
            c_ip.fieldKey = FWPM_CONDITION_IP_REMOTE_ADDRESS;
            c_ip.matchType = FWP_MATCH_EQUAL;
            match ip {
                IpAddr::V4(a) => {
                    v4m.addr = u32::from(*a);
                    v4m.mask = 0xFFFF_FFFF;
                    c_ip.conditionValue.r#type = FWP_V4_ADDR_MASK;
                    c_ip.conditionValue.Anonymous.v4AddrMask = &mut v4m;
                }
                IpAddr::V6(a) => {
                    v6m.addr = a.octets();
                    v6m.prefixLength = 128;
                    c_ip.conditionValue.r#type = FWP_V6_ADDR_MASK;
                    c_ip.conditionValue.Anonymous.v6AddrMask = &mut v6m;
                }
            }
            cond.push(c_ip);
        }

        // 3) The destination port, when given.
        if puerto != 0 {
            // SAFETY: zeroed condition; fields set below.
            let mut c_port: FWPM_FILTER_CONDITION0 = unsafe { std::mem::zeroed() };
            c_port.fieldKey = FWPM_CONDITION_IP_REMOTE_PORT;
            c_port.matchType = FWP_MATCH_EQUAL;
            c_port.conditionValue.r#type = FWP_UINT16;
            c_port.conditionValue.Anonymous.uint16 = puerto;
            cond.push(c_port);
        }

        let mut nombre = ancho("GUARDIANA corta el antivirus");
        let mut prov = PROVEEDOR;
        // SAFETY: zeroed filter; we set every field we rely on.
        let mut f: FWPM_FILTER0 = unsafe { std::mem::zeroed() };
        f.providerKey = &mut prov;
        f.layerKey = if v4 {
            FWPM_LAYER_ALE_AUTH_CONNECT_V4
        } else {
            FWPM_LAYER_ALE_AUTH_CONNECT_V6
        };
        f.subLayerKey = SUBCAPA;
        f.displayData.name = nombre.as_mut_ptr();
        f.action.r#type = FWP_ACTION_BLOCK;
        // Let the system weight it (FWP_EMPTY): no pointer to keep alive.
        f.weight.r#type = FWP_EMPTY;
        f.numFilterConditions = u32::try_from(cond.len()).unwrap_or(0);
        f.filterCondition = cond.as_mut_ptr();

        let mut id: u64 = 0;
        // SAFETY: filter and its conditions live through this call; out-id owned here.
        let r = unsafe { FwpmFilterAdd0(self.engine, &f, std::ptr::null_mut(), &mut id) };
        if r != 0 {
            return Err(err("FwpmFilterAdd0", r));
        }
        Ok(id)
    }

    pub(crate) fn quitar(&self, ids: &[u64]) -> Result<(), ErrorCorte> {
        for &id in ids {
            // SAFETY: deleting by id; a filter already gone returns not-found, which is fine.
            let r = unsafe { FwpmFilterDeleteById0(self.engine, id) };
            if r != 0 && r != 0x8032_0005
            /* FWP_E_FILTER_NOT_FOUND */
            {
                return Err(err("FwpmFilterDeleteById0", r));
            }
        }
        if let Ok(mut g) = self.ids.lock() {
            g.retain(|x| !ids.contains(x));
        }
        Ok(())
    }

    pub(crate) fn quitar_todo(&self) -> Result<u64, ErrorCorte> {
        let todos: Vec<u64> = self.ids.lock().map(|g| g.clone()).unwrap_or_default();
        let n = todos.len() as u64;
        self.quitar(&todos)?;
        Ok(n)
    }

    pub(crate) fn cuantos(&self) -> Result<usize, ErrorCorte> {
        Ok(self.ids.lock().map(|g| g.len()).unwrap_or(0))
    }
}

impl Drop for Motor {
    fn drop(&mut self) {
        if !self.engine.is_null() {
            // SAFETY: the handle came from FwpmEngineOpen0 and is not used after this. Closing a
            // dynamic session also removes every object we added.
            unsafe {
                FwpmEngineClose0(self.engine);
            }
        }
    }
}
