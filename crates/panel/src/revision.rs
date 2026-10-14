//! «Revisión del ordenador» (1.0.24). The owner, 13 Oct 2026: «quiero que GUARDIANA revise
//! absolutamente todo lo que se pueda revisar en un ordenador… incluso el internet por donde sale».
//! Designed with Grok and ChatGPT the same evening: an observatory that checks, notices what is
//! new and explains, not an alarm and not a «virus detector». It reads and never changes anything.
//!
//! Windows says the facts (`guardiana_apps::revision_del_equipo`, one read-only PowerShell); this
//! module turns them into checks with a traffic light, a sentence (a key of the i18n files) and
//! the list behind it, and remembers which startup entries, tasks, services, drivers, root
//! certificates and accounts it had already seen, so whatever appears later is marked «nuevo».
//!
//! Addresses: the DNS test and the hosts file carry addresses; they stay in memory for the page
//! and are never written (decision 203). What is written to disk is only the list of names seen.

// The handlers answer with axum's `Response` as their error, like the rest of the panel.
#![allow(clippy::result_large_err)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::Json;
use guardiana_core::time::now_ms;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::api::ApiResult;
use crate::auth::Session;
use crate::AppState;

const DIA: i64 = 24 * 3_600_000;
/// A review older than this is repeated when the page is opened.
const VIGENCIA_MS: i64 = 30 * 60_000;
/// How long PowerShell may take before it is stopped.
const LIMITE: std::time::Duration = std::time::Duration::from_secs(300);

/// What the panel keeps of the last review, in memory only.
#[derive(Default)]
pub(crate) struct Revisor {
    estado: Mutex<Cache>,
}

#[derive(Default)]
struct Cache {
    corriendo: bool,
    cuando: Option<i64>,
    datos: Option<Value>,
    error: Option<String>,
}

/// The names already seen, written next to the ledger (`revision-vistos.json`).
#[derive(Serialize, Deserialize, Default)]
struct Vistos {
    /// When the first review was made: what was there then is «what the computer had».
    creada: i64,
    /// Every item seen, with when it was first seen.
    vistos: BTreeMap<String, i64>,
}

fn ruta_vistos(db: &Path) -> PathBuf {
    db.with_file_name("revision-vistos.json")
}

fn lee_vistos(db: &Path) -> Option<Vistos> {
    let t = std::fs::read_to_string(ruta_vistos(db)).ok()?;
    serde_json::from_str(&t).ok()
}

impl Revisor {
    /// Starts a review in the background unless one is running.
    pub(crate) fn lanzar(state: &Arc<AppState>) {
        {
            let Ok(mut c) = state.revision.estado.lock() else {
                return;
            };
            if c.corriendo {
                return;
            }
            c.corriendo = true;
        }
        let st = Arc::clone(state);
        tokio::task::spawn_blocking(move || {
            let r = guardiana_apps::revision_del_equipo(LIMITE);
            let now = now_ms();
            if let Ok(datos) = &r {
                // Remember what was seen (names only), the first time with its own date.
                let mut v = lee_vistos(&st.db_path).unwrap_or(Vistos {
                    creada: now,
                    vistos: BTreeMap::new(),
                });
                for k in claves_inventario(datos) {
                    v.vistos.entry(k).or_insert(now);
                }
                if let Ok(t) = serde_json::to_string(&v) {
                    let _ = std::fs::write(ruta_vistos(&st.db_path), t);
                }
            }
            if let Ok(mut c) = st.revision.estado.lock() {
                c.corriendo = false;
                c.cuando = Some(now);
                match r {
                    Ok(d) => {
                        c.datos = Some(d);
                        c.error = None;
                    }
                    Err(guardiana_apps::Error::NoSoportado) => c.error = Some("no_windows".into()),
                    Err(e) => c.error = Some(e.to_string()),
                }
            }
        });
    }
}

/// Every six hours, from three minutes after the panel starts, so what appears on its own is
/// noticed even when nobody opens the page. Windows only.
pub(crate) fn vigilar(state: Arc<AppState>) {
    if !cfg!(windows) {
        return;
    }
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(180)).await;
        loop {
            Revisor::lanzar(&state);
            tokio::time::sleep(std::time::Duration::from_secs(6 * 3600)).await;
        }
    });
}

// ---------------------------------------------------------------------------------------------
// The page's answer.

#[derive(Serialize)]
pub(crate) struct Respuesta {
    disponible: bool,
    corriendo: bool,
    cuando: Option<i64>,
    error: Option<String>,
    /// Since when GUARDIANA remembers what this computer had (the first review).
    desde: Option<i64>,
    equipo: Option<Value>,
    grupos: Vec<Grupo>,
    resumen: Resumen,
}

#[derive(Serialize, Default, Debug, PartialEq, Eq)]
struct Resumen {
    ok: usize,
    info: usize,
    aviso: usize,
    alerta: usize,
    nuevos: usize,
}

#[derive(Serialize, Debug)]
struct Grupo {
    id: &'static str,
    checks: Vec<Check>,
}

#[derive(Serialize, Debug, Clone)]
struct Check {
    id: &'static str,
    /// `ok`, `info`, `aviso`, `alerta` or `nd` (Windows did not say).
    estado: &'static str,
    /// The sentence: a key of the `panel` section, with `{x}` filled from `datos`.
    frase: &'static str,
    datos: BTreeMap<&'static str, String>,
    lista: Vec<Item>,
}

#[derive(Serialize, Debug, Clone)]
struct Item {
    /// What it is (a name, a path).
    t: String,
    /// A second line (who signs it, where it is).
    d: String,
    /// Tags, each a key `rv_marca_<x>`: `nuevo`, `sin_firma`, `no_existe`, `en_uso`…
    marcas: Vec<&'static str>,
    /// A moment to show with it (last use, last sign-in, detection), ms.
    #[serde(skip_serializing_if = "Option::is_none")]
    cuando: Option<i64>,
}

impl Item {
    fn en(mut self, t: Option<i64>) -> Self {
        self.cuando = t.filter(|&x| x > 0);
        self
    }
}

/// `/api/revision`: the last review, and a new one started if it is old.
pub(crate) async fn ver(State(state): State<Arc<AppState>>, _s: Session) -> ApiResult<Respuesta> {
    let viejo = state
        .revision
        .estado
        .lock()
        .map(|c| !c.corriendo && c.cuando.is_none_or(|t| now_ms() - t > VIGENCIA_MS))
        .unwrap_or(false);
    if viejo && cfg!(windows) {
        Revisor::lanzar(&state);
    }
    Ok(Json(respuesta(&state)))
}

/// `POST /api/revision/ahora`: review again now.
pub(crate) async fn ahora(State(state): State<Arc<AppState>>, _s: Session) -> ApiResult<Respuesta> {
    if cfg!(windows) {
        Revisor::lanzar(&state);
    }
    Ok(Json(respuesta(&state)))
}

fn respuesta(state: &AppState) -> Respuesta {
    let (corriendo, cuando, datos, error) = state
        .revision
        .estado
        .lock()
        .map(|c| (c.corriendo, c.cuando, c.datos.clone(), c.error.clone()))
        .unwrap_or_default();
    let vistos = lee_vistos(&state.db_path);
    let now = now_ms();
    let grupos = datos
        .as_ref()
        .map(|d| evaluar(d, vistos.as_ref(), now))
        .unwrap_or_default();
    Respuesta {
        disponible: cfg!(windows),
        corriendo,
        cuando,
        error,
        desde: vistos.as_ref().map(|v| v.creada),
        equipo: datos.as_ref().map(|d| d["equipo"].clone()),
        resumen: resumir(&grupos),
        grupos,
    }
}

fn resumir(grupos: &[Grupo]) -> Resumen {
    let mut r = Resumen::default();
    for c in grupos.iter().flat_map(|g| &g.checks) {
        match c.estado {
            "ok" => r.ok += 1,
            "info" => r.info += 1,
            "aviso" => r.aviso += 1,
            "alerta" => r.alerta += 1,
            _ => {}
        }
        r.nuevos += c
            .lista
            .iter()
            .filter(|i| i.marcas.contains(&"nuevo"))
            .count();
    }
    r
}

// ---------------------------------------------------------------------------------------------
// What is remembered between reviews.

fn s(v: &Value) -> String {
    match v {
        Value::String(x) => x.trim().to_owned(),
        Value::Null => String::new(),
        otro => otro.to_string(),
    }
}

/// A list from PowerShell. Windows PowerShell can turn a list of one into the thing itself on
/// its way to JSON (Grok's review, 13 Oct 2026): a lone object is read as a list of one.
/// Like [`arr`], but `None` when Windows did not answer (missing, null or the word `error`).
fn lista_de(v: &Value) -> Option<&[Value]> {
    match v {
        Value::Null => None,
        Value::String(x) if x == "error" => None,
        otro => Some(arr(otro)),
    }
}

fn arr(v: &Value) -> &[Value] {
    match v {
        Value::Array(a) => a.as_slice(),
        Value::Object(_) | Value::String(_) => std::slice::from_ref(v),
        _ => &[],
    }
}

/// The key of each remembered item, by kind.
fn claves_inventario(d: &Value) -> Vec<String> {
    let mut k = Vec::new();
    let a = &d["arranque"];
    for x in arr(&a["inicio"]) {
        k.push(format!(
            "inicio:{}|{}",
            s(&x["nombre"]),
            s(&x["ruta"]).to_lowercase()
        ));
    }
    for x in arr(&a["tareas"]) {
        k.push(format!("tarea:{}{}", s(&x["carpeta"]), s(&x["nombre"])));
    }
    for x in arr(&a["servicios"]) {
        k.push(format!("servicio:{}", s(&x["nombre"]).to_lowercase()));
    }
    for x in arr(&a["controladores"]) {
        k.push(format!("controlador:{}", s(&x["nombre"]).to_lowercase()));
    }
    for x in arr(&d["certificados"]["fuera"]) {
        k.push(format!("raiz:{}", s(&x["huella"])));
    }
    for x in arr(&d["certificados"]["de_usuario"]) {
        k.push(format!("raiz:{}", s(&x["huella"])));
    }
    for x in arr(&d["acceso"]["cuentas"]) {
        k.push(format!("cuenta:{}", s(&x["sid"])));
    }
    for x in arr(&d["programas"]["instalados"]) {
        k.push(format!("programa:{}|{}", s(&x["nombre"]), s(&x["editor"])));
    }
    k
}

/// «nuevo»: first seen after the first review, within the last 30 days.
fn es_nuevo(vistos: Option<&Vistos>, clave: &str, now: i64) -> bool {
    let Some(v) = vistos else { return false };
    v.vistos
        .get(clave)
        .is_some_and(|&t| t > v.creada && now - t < 30 * DIA)
}

// ---------------------------------------------------------------------------------------------
// The checks.

fn check(id: &'static str, estado: &'static str, frase: &'static str) -> Check {
    Check {
        id,
        estado,
        frase,
        datos: BTreeMap::new(),
        lista: Vec::new(),
    }
}

impl Check {
    fn con(mut self, k: &'static str, v: impl ToString) -> Self {
        self.datos.insert(k, v.to_string());
        self
    }
    fn items(mut self, l: Vec<Item>) -> Self {
        self.lista = l;
        self
    }
}

fn item(t: impl Into<String>, d: impl Into<String>, marcas: Vec<&'static str>) -> Item {
    Item {
        t: t.into(),
        d: d.into(),
        marcas,
        cuando: None,
    }
}

fn dias(now: i64, t: &Value) -> Option<i64> {
    t.as_i64()
        .filter(|&x| x > 0)
        .map(|x| (now - x).max(0) / DIA)
}

/// Programs people install to let somebody else drive the computer. Legitimate, and also the
/// first thing a fake «technical support» call asks for.
const ACCESO_REMOTO: &[&str] = &[
    "teamviewer",
    "anydesk",
    "rustdesk",
    "splashtop",
    "logmein",
    "gotoassist",
    "goto resolve",
    "screenconnect",
    "connectwise control",
    "chrome remote desktop",
    "ultraviewer",
    "ammyy",
    "supremo",
    "remotepc",
    "realvnc",
    "vnc server",
    "tightvnc",
    "ultravnc",
    "tigervnc",
    "dwservice",
    "dwagent",
    "atera",
    "ninjarmm",
    "zoho assist",
    "aeroadmin",
    "hoptodesk",
    "remote utilities",
    "netsupport",
    "meshagent",
    "mesh agent",
    "action1",
    "kaseya",
    "dameware",
    "radmin",
    "parsec",
    "getscreen",
    "iperius remote",
];

/// Makers of computers whose own programs come installed from the factory.
const FABRICANTES: &[&str] = &[
    "dell",
    "hp inc",
    "hewlett",
    "lenovo",
    "asus",
    "acer",
    "samsung",
    "micro-star",
    "msi ",
    "huawei",
    "toshiba",
    "dynabook",
    "gigabyte",
    "razer",
    "alienware",
    "medion",
    "fujitsu",
    "lg electronics",
    "microsoft surface",
    "xiaomi",
    "honor",
];

/// Antivirus trials that often come installed from the factory.
const DE_PRUEBA: &[&str] = &[
    "mcafee",
    "norton",
    "wildtangent",
    "booking.com",
    "dropbox promotion",
];

fn contiene_alguno(t: &str, lista: &[&str]) -> bool {
    let t = t.to_lowercase();
    lista.iter().any(|x| t.contains(x))
}

/// Well-known public resolvers, by address.
fn resolutor_conocido(ip: &str) -> Option<&'static str> {
    Some(match ip {
        "8.8.8.8" | "8.8.4.4" | "2001:4860:4860::8888" | "2001:4860:4860::8844" => "Google",
        "1.1.1.1" | "1.0.0.1" | "2606:4700:4700::1111" | "2606:4700:4700::1001" => "Cloudflare",
        "1.1.1.2" | "1.0.0.2" | "1.1.1.3" | "1.0.0.3" => "Cloudflare (filtro)",
        "9.9.9.9" | "149.112.112.112" | "2620:fe::fe" | "2620:fe::9" => "Quad9",
        "208.67.222.222" | "208.67.220.220" => "OpenDNS (Cisco)",
        "94.140.14.14" | "94.140.15.15" => "AdGuard",
        "76.76.2.0" | "76.76.10.0" => "Control D",
        "185.228.168.9" | "185.228.169.9" => "CleanBrowsing",
        _ => return None,
    })
}

/// Where a resolver is, without saying its address: this computer, the home network or the
/// internet (a known public one by name, otherwise «the provider or another»).
fn tipo_resolutor(ip: &str) -> (&'static str, String) {
    if let Some(n) = resolutor_conocido(ip) {
        return ("publico", n.to_owned());
    }
    match ip.parse::<std::net::IpAddr>() {
        Ok(a) if a.is_loopback() => ("equipo", String::new()),
        Ok(std::net::IpAddr::V4(v)) if v.is_private() || v.is_link_local() => {
            ("casa", String::new())
        }
        Ok(std::net::IpAddr::V6(v))
            if (v.segments()[0] & 0xfe00) == 0xfc00 || (v.segments()[0] & 0xffc0) == 0xfe80 =>
        {
            ("casa", String::new())
        }
        Ok(_) => ("otro", String::new()),
        Err(_) => ("otro", String::new()),
    }
}

fn firma_marcas(x: &Value) -> Vec<&'static str> {
    let mut m = Vec::new();
    if x["existe"].as_bool() == Some(false) {
        m.push("no_existe");
    } else {
        match s(&x["firma"]).as_str() {
            "Valid" => {}
            "NotSigned" => m.push("sin_firma"),
            "" => {}
            _ => m.push("firma_mala"),
        }
    }
    m
}

fn firmante(x: &Value) -> String {
    let f = s(&x["firmante"]);
    if f.is_empty() {
        s(&x["ruta"])
    } else {
        format!("{f} · {}", s(&x["ruta"]))
    }
}

#[allow(clippy::too_many_lines)]
fn evaluar(d: &Value, vistos: Option<&Vistos>, now: i64) -> Vec<Grupo> {
    let nd = |id: &'static str| check(id, "nd", "rv_nd");
    let mut proteccion = Vec::new();
    let mut acceso = Vec::new();
    let mut arranque = Vec::new();
    let mut salida = Vec::new();
    let mut privacidad = Vec::new();
    let mut antivirus = Vec::new();

    // ---- Protección
    let u = &d["actualizaciones"];
    let ult = dias(now, &u["ultima_instalacion"]).or_else(|| dias(now, &u["ultimo_parche"]));
    proteccion.push(match ult {
        _ if u["politica_sin_auto"].as_bool() == Some(true) || s(&u["servicio"]) == "Disabled" => {
            check("actualizaciones", "alerta", "rv_actualizaciones_apagadas")
        }
        // Days since the last install say activity, not that nothing is pending (ChatGPT's
        // review): never red for that alone.
        Some(n) if n > 45 => {
            check("actualizaciones", "aviso", "rv_actualizaciones_viejas").con("n", n)
        }
        _ if u["pausada_hasta"].as_i64().is_some_and(|t| t > now) => {
            check("actualizaciones", "aviso", "rv_actualizaciones_pausadas").con(
                "dias",
                (u["pausada_hasta"].as_i64().unwrap_or(now) - now) / DIA,
            )
        }
        _ if u["reinicio_pendiente"].as_bool() == Some(true) => {
            check("actualizaciones", "aviso", "rv_actualizaciones_reinicio")
        }
        Some(n) => check("actualizaciones", "info", "rv_actualizaciones_ok").con("n", n),
        None => nd("actualizaciones"),
    });

    let def = &d["defensa"];
    let productos = arr(&def["productos"]);
    if def["productos"].is_array() && !productos.is_empty() {
        let lista: Vec<Item> = productos
            .iter()
            .map(|p| {
                let e = u32::try_from(p["estado"].as_u64().unwrap_or(0)).unwrap_or(0);
                let (activo, al_dia) = guardiana_apps::lee_estado(e);
                let mut m = Vec::new();
                if !activo {
                    m.push("apagado");
                } else if !al_dia {
                    m.push("sin_actualizar");
                }
                item(s(&p["nombre"]), "", m)
            })
            .collect();
        let activos = lista
            .iter()
            .filter(|i| !i.marcas.contains(&"apagado"))
            .count();
        let viejos = lista
            .iter()
            .filter(|i| i.marcas.contains(&"sin_actualizar"))
            .count();
        let c = if activos == 0 {
            check("antivirus", "alerta", "rv_antivirus_ninguno")
        } else if viejos > 0 {
            check("antivirus", "aviso", "rv_antivirus_viejo")
        } else {
            check("antivirus", "ok", "rv_antivirus_ok")
        };
        let nombres = lista
            .iter()
            .filter(|i| !i.marcas.contains(&"apagado"))
            .map(|i| i.t.clone())
            .collect::<Vec<_>>()
            .join(", ");
        proteccion.push(c.con("nombres", nombres).items(lista));
    } else if def["defender"].is_object() {
        let m = &def["defender"];
        proteccion.push(if m["tiempo_real"].as_bool() == Some(true) {
            check("antivirus", "ok", "rv_antivirus_ok").con("nombres", "Microsoft Defender")
        } else {
            check("antivirus", "alerta", "rv_antivirus_ninguno")
        });
    } else {
        proteccion.push(nd("antivirus"));
    }
    if let Some(m) = def["defender"].as_object() {
        let activo = m.get("antivirus").and_then(Value::as_bool) == Some(true)
            && !s(m.get("modo").unwrap_or(&Value::Null))
                .to_lowercase()
                .contains("passive");
        if activo {
            let firmas = m
                .get("firmas_dias")
                .and_then(Value::as_i64)
                .filter(|&n| n < 100_000);
            let n = firmas.unwrap_or(0);
            proteccion.push(if firmas.is_none() {
                nd("defender")
            } else if n > 14 {
                check("defender", "alerta", "rv_defender_firmas").con("n", n)
            } else if n > 7 {
                check("defender", "aviso", "rv_defender_firmas").con("n", n)
            } else if m.get("manipulacion").and_then(Value::as_bool) == Some(false) {
                check("defender", "aviso", "rv_defender_manipulacion")
            } else {
                check("defender", "ok", "rv_defender_ok").con("n", n)
            });
        } else {
            proteccion.push(check("defender", "info", "rv_defender_pasivo"));
        }
    }

    let sis = &d["sistema"];
    if let Some(perfiles) = lista_de(&sis["cortafuegos"]) {
        let apagados: Vec<Item> = perfiles
            .iter()
            .filter(|p| p["activo"].as_bool() != Some(true))
            .map(|p| item(s(&p["perfil"]), "", vec!["apagado"]))
            .collect();
        proteccion.push(if apagados.is_empty() {
            check("cortafuegos", "ok", "rv_cortafuegos_ok")
        } else {
            check("cortafuegos", "alerta", "rv_cortafuegos_apagado").items(apagados)
        });
    } else {
        proteccion.push(nd("cortafuegos"));
    }
    match sis["uac"].as_i64() {
        Some(0) => proteccion.push(check("uac", "alerta", "rv_uac_apagado")),
        Some(_) => proteccion.push(check("uac", "ok", "rv_uac_ok")),
        None => proteccion.push(nd("uac")),
    }
    match sis["smb1"].as_bool() {
        Some(true) => proteccion.push(check("smb1", "aviso", "rv_smb1_activo")),
        Some(false) => proteccion.push(check("smb1", "ok", "rv_smb1_ok")),
        None => {}
    }
    proteccion.push(match &sis["arranque_seguro"] {
        Value::Bool(true) => check("arranque_seguro", "ok", "rv_arranque_seguro_ok"),
        Value::Bool(false) => check("arranque_seguro", "aviso", "rv_arranque_seguro_apagado"),
        Value::String(x) if x == "sin_uefi" => {
            check("arranque_seguro", "info", "rv_arranque_seguro_sin_uefi")
        }
        _ => nd("arranque_seguro"),
    });
    proteccion.push(match &sis["tpm"] {
        Value::Object(t) if t.get("activo").and_then(Value::as_bool) == Some(true) => {
            check("tpm", "ok", "rv_tpm_ok")
                .con("version", s(t.get("version").unwrap_or(&Value::Null)))
        }
        Value::Object(_) => check("tpm", "aviso", "rv_tpm_apagado"),
        Value::String(x) if x == "ninguno" => check("tpm", "info", "rv_tpm_ninguno"),
        _ => nd("tpm"),
    });
    let unidad = s(&sis["unidad"]).to_uppercase();
    if let Some(vols) = lista_de(&sis["cifrado"]) {
        let sistema = vols
            .iter()
            .find(|v| s(&v["unidad"]).to_uppercase() == unidad);
        proteccion.push(match sistema.and_then(|v| v["proteccion"].as_i64()) {
            Some(1) => check("cifrado", "ok", "rv_cifrado_ok").con("unidad", &unidad),
            Some(0) => check("cifrado", "aviso", "rv_cifrado_no").con("unidad", &unidad),
            _ => check("cifrado", "info", "rv_cifrado_nd"),
        });
    } else {
        proteccion.push(check("cifrado", "info", "rv_cifrado_nd"));
    }
    match sis["integridad_memoria"].as_bool() {
        Some(true) => proteccion.push(check("integridad_memoria", "ok", "rv_integridad_ok")),
        Some(false) => proteccion.push(check("integridad_memoria", "info", "rv_integridad_no")),
        None => {}
    }

    // ---- Acceso
    let ac = &d["acceso"];
    if let Some(cuentas) = lista_de(&ac["cuentas"]) {
        let mut aviso = false;
        let lista: Vec<Item> = cuentas
            .iter()
            .filter(|c| c["activa"].as_bool() == Some(true))
            .map(|c| {
                let mut m = Vec::new();
                if c["admin"].as_bool() == Some(true) {
                    m.push("admin");
                }
                // «Password not required» is allowed, not proven empty (ChatGPT's review).
                if c["clave_opcional"].as_bool() == Some(true) {
                    m.push("clave_opcional");
                }
                if s(&c["sid"]).ends_with("-501") {
                    m.push("invitado");
                    aviso = true;
                }
                if es_nuevo(vistos, &format!("cuenta:{}", s(&c["sid"])), now) {
                    m.push("nuevo");
                    aviso = true;
                }
                item(s(&c["nombre"]), "", m).en(c["ultima"].as_i64())
            })
            .collect();
        let admins = lista.iter().filter(|i| i.marcas.contains(&"admin")).count();
        let creadas = arr(&ac["creadas_30d"]).len();
        let c = if aviso || creadas > 0 {
            check("cuentas", "aviso", "rv_cuentas_aviso")
        } else {
            check("cuentas", "ok", "rv_cuentas_ok")
        };
        acceso.push(
            c.con("n", lista.len())
                .con("admins", admins)
                .con("creadas", creadas)
                .items(lista),
        );
    } else {
        acceso.push(nd("cuentas"));
    }
    let entradas = ac["rdp_entradas_30d"].as_i64();
    acceso.push(match ac["rdp"].as_bool() {
        Some(true) if ac["rdp_nla"].as_bool() == Some(false) => {
            check("escritorio_remoto", "alerta", "rv_rdp_sin_nla")
        }
        Some(true) => check("escritorio_remoto", "aviso", "rv_rdp_activo").con(
            "n",
            entradas
                .map(|n| n.to_string())
                .unwrap_or_else(|| "—".into()),
        ),
        Some(false) => check("escritorio_remoto", "ok", "rv_rdp_ok"),
        None => nd("escritorio_remoto"),
    });
    if ac["asistencia"].as_bool() == Some(true) {
        acceso.push(check("asistencia_remota", "info", "rv_asistencia_activa"));
    }
    let instalados = arr(&d["programas"]["instalados"]);
    let mut remotos: Vec<Item> = instalados
        .iter()
        .filter(|p| contiene_alguno(&s(&p["nombre"]), ACCESO_REMOTO))
        .map(|p| {
            let mut m = Vec::new();
            if es_nuevo(
                vistos,
                &format!("programa:{}|{}", s(&p["nombre"]), s(&p["editor"])),
                now,
            ) {
                m.push("nuevo");
            }
            item(s(&p["nombre"]), s(&p["editor"]), m)
        })
        .collect();
    for sv in arr(&d["arranque"]["servicios"]) {
        let n = format!("{} {}", s(&sv["visible"]), s(&sv["ruta"]));
        if contiene_alguno(&n, ACCESO_REMOTO)
            && !remotos.iter().any(|r| {
                s(&sv["visible"])
                    .to_lowercase()
                    .contains(&r.t.to_lowercase())
            })
        {
            remotos.push(item(s(&sv["visible"]), firmante(sv), vec!["servicio"]));
        }
    }
    acceso.push(if remotos.is_empty() {
        check("acceso_remoto", "ok", "rv_acceso_remoto_ok")
    } else {
        check("acceso_remoto", "aviso", "rv_acceso_remoto_hay")
            .con("n", remotos.len())
            .items(remotos)
    });
    match ac["fallidos_7d"].as_i64() {
        // The script reads at most 5 000 events: at the cap, «5000+».
        Some(n) if n > 500 => acceso.push(check("fallidos", "alerta", "rv_fallidos").con(
            "n",
            if n >= 5000 {
                "5000+".to_owned()
            } else {
                n.to_string()
            },
        )),
        Some(n) if n > 30 => acceso.push(check("fallidos", "aviso", "rv_fallidos").con("n", n)),
        Some(n) => acceso.push(check("fallidos", "ok", "rv_fallidos_ok").con("n", n)),
        None => acceso.push(nd("fallidos")),
    }
    if ac["autologon"].as_bool() == Some(true) {
        acceso.push(if ac["autologon_clave"].as_bool() == Some(true) {
            check("autologon", "alerta", "rv_autologon_clave")
        } else {
            check("autologon", "info", "rv_autologon")
        });
    }
    if let Some(puertos) = lista_de(&ac["puertos"]) {
        const DE_WINDOWS: &[&str] = &[
            "system",
            "svchost",
            "lsass",
            "wininit",
            "services",
            "spoolsv",
            "guardiana",
        ];
        const DELICADOS: &[i64] = &[21, 22, 23, 3389, 5900, 5800, 5938, 7070];
        let mut grave = false;
        let lista: Vec<Item> = puertos
            .iter()
            .filter(|p| s(&p["alcance"]) == "red")
            .filter(|p| {
                let pr = s(&p["programa"]).to_lowercase();
                !DE_WINDOWS.iter().any(|w| pr.starts_with(w))
                    || DELICADOS.contains(&p["puerto"].as_i64().unwrap_or(0))
            })
            .map(|p| {
                let mut m = Vec::new();
                if DELICADOS.contains(&p["puerto"].as_i64().unwrap_or(0)) {
                    m.push("delicado");
                    grave = true;
                }
                item(
                    format!("{} · {}", s(&p["puerto"]), s(&p["programa"])),
                    "",
                    m,
                )
            })
            .collect();
        acceso.push(if grave {
            check("puertos", "aviso", "rv_puertos_delicados").items(lista)
        } else if lista.is_empty() {
            check("puertos", "ok", "rv_puertos_ok")
        } else {
            check("puertos", "info", "rv_puertos_hay")
                .con("n", lista.len())
                .items(lista)
        });
    }
    if let Some(carpetas) = lista_de(&ac["carpetas"]) {
        let raiz = carpetas.iter().any(|c| {
            let r = s(&c["ruta"]);
            r.len() <= 3 && r.ends_with(":\\")
        });
        let lista: Vec<Item> = carpetas
            .iter()
            .map(|c| item(s(&c["nombre"]), s(&c["ruta"]), Vec::new()))
            .collect();
        acceso.push(if lista.is_empty() {
            check("compartido", "ok", "rv_compartido_ok")
        } else if raiz {
            check("compartido", "aviso", "rv_compartido_disco").items(lista)
        } else {
            check("compartido", "info", "rv_compartido_hay")
                .con("n", lista.len())
                .items(lista)
        });
    }

    // ---- Lo que arranca solo
    let ar = &d["arranque"];
    let inventario = |id: &'static str,
                      clave: &dyn Fn(&Value) -> String,
                      nombre: &dyn Fn(&Value) -> String,
                      filas: &Value|
     -> Check {
        let Some(filas) = lista_de(filas) else {
            return nd(id);
        };
        let lista: Vec<Item> = filas
            .iter()
            .map(|x| {
                let mut m = firma_marcas(x);
                if es_nuevo(vistos, &clave(x), now) {
                    m.insert(0, "nuevo");
                }
                if x["oculta"].as_bool() == Some(true) {
                    m.push("oculta");
                }
                item(nombre(x), firmante(x), m)
            })
            .collect();
        let nuevos = lista.iter().filter(|i| i.marcas.contains(&"nuevo")).count();
        let raros = lista
            .iter()
            .filter(|i| {
                i.marcas
                    .iter()
                    .any(|m| matches!(*m, "sin_firma" | "firma_mala" | "no_existe"))
            })
            .count();
        let estado = if nuevos > 0 || raros > 0 {
            "aviso"
        } else {
            "info"
        };
        check(id, estado, "rv_inventario")
            .con("n", lista.len())
            .con("nuevos", nuevos)
            .con("raros", raros)
            .items(lista)
    };
    arranque.push(inventario(
        "inicio",
        &|x| {
            format!(
                "inicio:{}|{}",
                s(&x["nombre"]),
                s(&x["ruta"]).to_lowercase()
            )
        },
        &|x| s(&x["nombre"]),
        &ar["inicio"],
    ));
    arranque.push(inventario(
        "tareas",
        &|x| format!("tarea:{}{}", s(&x["carpeta"]), s(&x["nombre"])),
        &|x| format!("{}{}", s(&x["carpeta"]), s(&x["nombre"])),
        &ar["tareas"],
    ));
    arranque.push(inventario(
        "servicios",
        &|x| format!("servicio:{}", s(&x["nombre"]).to_lowercase()),
        &|x| s(&x["visible"]),
        &ar["servicios"],
    ));
    arranque.push(inventario(
        "controladores",
        &|x| format!("controlador:{}", s(&x["nombre"]).to_lowercase()),
        &|x| s(&x["visible"]),
        &ar["controladores"],
    ));

    // ---- Por dónde sale tu internet
    let sa = &d["salida"];
    if let Some(dns) = lista_de(&sa["dns"]) {
        let mut tipos: Vec<String> = Vec::new();
        for i in dns {
            for ip in arr(&i["servidores"]) {
                let (t, n) = tipo_resolutor(&s(ip));
                let x = if n.is_empty() {
                    t.to_owned()
                } else {
                    format!("{t}:{n}")
                };
                if !tipos.contains(&x) {
                    tipos.push(x);
                }
            }
        }
        let lista: Vec<Item> = tipos
            .iter()
            .map(|t| {
                let (k, n) = t.split_once(':').unwrap_or((t.as_str(), ""));
                item(
                    n,
                    "",
                    vec![match k {
                        "equipo" => "dns_equipo",
                        "casa" => "dns_casa",
                        "publico" => "dns_publico",
                        _ => "dns_otro",
                    }],
                )
            })
            .collect();
        // The test: two public names with known answers and one that does not exist.
        let mut falsea = false;
        let mut redirige = false;
        let mut probados = 0;
        for p in arr(&sa["prueba_dns"]) {
            let ok_de = |v: &Value, buenas: &[&str]| -> Option<bool> {
                let l = lista_de(v)?;
                Some(!l.is_empty() && l.iter().all(|x| buenas.contains(&s(x).as_str())))
            };
            let g = ok_de(&p["google"], &["8.8.8.8", "8.8.4.4"]);
            let c = ok_de(&p["cloudflare"], &["1.1.1.1", "1.0.0.1"]);
            if g.is_some() || c.is_some() {
                probados += 1;
            }
            if g == Some(false) || c == Some(false) {
                falsea = true;
            }
            if lista_de(&p["inexistente"]).is_some_and(|l| !l.is_empty()) {
                redirige = true;
            }
        }
        salida.push(if falsea {
            // Different answers can be a provider's filter as well as a diversion (ChatGPT's
            // review): something to look at, not a proven forgery.
            check("dns", "aviso", "rv_dns_falsea").items(lista)
        } else if redirige {
            check("dns", "aviso", "rv_dns_redirige").items(lista)
        } else if probados > 0 {
            check("dns", "ok", "rv_dns_ok").items(lista)
        } else {
            check("dns", "info", "rv_dns_sin_prueba").items(lista)
        });
    } else {
        salida.push(nd("dns"));
    }
    let mut proxys: Vec<Item> = arr(&sa["proxy"])
        .iter()
        .map(|p| {
            let srv = s(&p["servidor"]);
            let pac = s(&p["pac"]);
            item(if srv.is_empty() { pac } else { srv }, "", vec!["usuario"])
        })
        .collect();
    let ps = s(&sa["proxy_sistema"]);
    if !ps.is_empty() {
        proxys.push(item(ps, "", vec!["sistema"]));
    }
    salida.push(if proxys.is_empty() {
        check("proxy", "ok", "rv_proxy_ok")
    } else {
        check("proxy", "aviso", "rv_proxy_hay").items(proxys)
    });
    const VPN: &[&str] = &[
        "wintun",
        "tap-windows",
        "wireguard",
        "openvpn",
        "nordlynx",
        "proton",
        "fortinet",
        "anyconnect",
        "globalprotect",
        "tailscale",
        "zerotier",
        "expressvpn",
        "surfshark",
        "mullvad",
        "cloudflare warp",
        "pangp",
        "sonicwall",
    ];
    let mut vpn: Vec<Item> = arr(&sa["adaptadores"])
        .iter()
        .filter(|a| {
            let t = format!("{} {}", s(&a["nombre"]), s(&a["desc"])).to_lowercase();
            VPN.iter().any(|v| t.contains(v)) || t.contains("vpn")
        })
        .map(|a| item(s(&a["nombre"]), s(&a["desc"]), vec!["conectada"]))
        .collect();
    for v in arr(&sa["vpn"]) {
        if v["conectada"].as_bool() == Some(true) {
            vpn.push(item(s(&v["nombre"]), "", vec!["conectada"]));
        }
    }
    salida.push(if vpn.is_empty() {
        check("vpn", "info", "rv_vpn_no")
    } else {
        check("vpn", "info", "rv_vpn_si").items(vpn)
    });
    if let Some(h) = lista_de(&sa["hosts"]) {
        let mut redir = false;
        let lista: Vec<Item> = h
            .iter()
            .flat_map(|e| {
                let a = s(&e["direccion"]);
                let bloqueo = matches!(a.as_str(), "0.0.0.0" | "127.0.0.1" | "::1" | "::");
                arr(&e["nombres"])
                    .iter()
                    .map(s)
                    .filter(|n| n != "localhost")
                    .map(|n| (n, bloqueo))
                    .collect::<Vec<_>>()
            })
            .map(|(n, bloqueo)| {
                if !bloqueo {
                    redir = true;
                }
                item(
                    n,
                    "",
                    vec![if bloqueo { "bloqueado" } else { "redirigido" }],
                )
            })
            .collect();
        salida.push(if lista.is_empty() {
            check("hosts", "ok", "rv_hosts_ok")
        } else if redir {
            check("hosts", "aviso", "rv_hosts_redirige")
                .con("n", lista.len())
                .items(lista)
        } else {
            check("hosts", "info", "rv_hosts_bloquea")
                .con("n", lista.len())
                .items(lista)
        });
    }
    let ce = &d["certificados"];
    if ce["fuera"].is_array() {
        const DE_FABRICA: &[&str] = &[
            "microsoft",
            "thawte timestamping ca",
            "copyright (c) 1997 microsoft corp.",
            "symantec enterprise mobile root for microsoft",
            "verisign class 3 public primary certification authority",
        ];
        let mut lista: Vec<Item> = arr(&ce["fuera"])
            .iter()
            .filter(|c| {
                let n = s(&c["nombre"]).to_lowercase();
                let e = s(&c["emisor"]).to_lowercase();
                let fabrica = DE_FABRICA.iter().any(|f| n.starts_with(f)) && n == e;
                !fabrica || es_nuevo(vistos, &format!("raiz:{}", s(&c["huella"])), now)
            })
            .map(|c| {
                let mut m = Vec::new();
                if es_nuevo(vistos, &format!("raiz:{}", s(&c["huella"])), now) {
                    m.push("nuevo");
                }
                item(s(&c["nombre"]), s(&c["huella"]), m)
            })
            .collect();
        for c in arr(&ce["de_usuario"]) {
            let mut m = vec!["de_usuario"];
            if es_nuevo(vistos, &format!("raiz:{}", s(&c["huella"])), now) {
                m.insert(0, "nuevo");
            }
            lista.push(item(s(&c["huella"]), "", m));
        }
        // Outside Microsoft's automatic program says neither who added it nor when (ChatGPT's
        // review): the list is to know; what appears after the first review is to look at.
        let nuevos = lista.iter().filter(|i| i.marcas.contains(&"nuevo")).count();
        salida.push(if lista.is_empty() {
            check("certificados", "ok", "rv_certificados_ok").con("n", s(&ce["total"]))
        } else if nuevos > 0 {
            check("certificados", "aviso", "rv_certificados_nuevo")
                .con("n", nuevos)
                .items(lista)
        } else {
            check("certificados", "info", "rv_certificados_hay")
                .con("n", lista.len())
                .items(lista)
        });
    } else {
        salida.push(nd("certificados"));
    }
    if let Some(w) = sa["wifi"].as_object() {
        if w.get("conectado").and_then(Value::as_bool) == Some(true) {
            let a = s(w.get("autenticacion").unwrap_or(&Value::Null));
            let au = a.to_uppercase();
            salida.push(if au.starts_with("WPA3") || au.starts_with("WPA2") {
                check("wifi", "ok", "rv_wifi_ok").con("tipo", &a)
            } else if au.starts_with("WPA") || au.starts_with("WEP") {
                check("wifi", "alerta", "rv_wifi_viejo").con("tipo", &a)
            } else if a.is_empty() {
                check("wifi", "info", "rv_wifi_nd")
            } else {
                check("wifi", "alerta", "rv_wifi_abierta").con("tipo", &a)
            });
        }
    }
    if let Some(p) = lista_de(&sa["perfiles"]) {
        if p.iter().any(|x| s(&x["categoria"]) == "Private") {
            salida.push(check("perfil_red", "info", "rv_perfil_privada"));
        } else if !p.is_empty() {
            salida.push(check("perfil_red", "ok", "rv_perfil_publica"));
        }
    }

    // ---- Privacidad
    let pr = &d["privacidad"];
    for (id, cap) in [
        ("camara", "webcam"),
        ("microfono", "microphone"),
        ("ubicacion", "location"),
    ] {
        let mut usos: Vec<&Value> = arr(&pr["usos"])
            .iter()
            .filter(|u| s(&u["cap"]) == cap)
            .collect();
        usos.sort_by_key(|u| std::cmp::Reverse(u["desde"].as_i64().unwrap_or(0)));
        let ahora = usos.iter().any(|u| u["hasta"].as_i64() == Some(0));
        let lista: Vec<Item> = usos
            .iter()
            .map(|u| {
                let app = s(&u["app"]);
                let nombre = app
                    .rsplit('\\')
                    .next()
                    .unwrap_or(&app)
                    .split('_')
                    .next()
                    .unwrap_or(&app)
                    .to_owned();
                let mut m = Vec::new();
                if u["hasta"].as_i64() == Some(0) {
                    m.push("en_uso");
                }
                item(nombre, app.clone(), m).en(u["desde"].as_i64())
            })
            .collect();
        privacidad.push(if lista.is_empty() {
            check(id, "ok", "rv_uso_nunca")
        } else if ahora {
            check(id, "info", "rv_uso_ahora")
                .con("n", lista.len())
                .items(lista)
        } else {
            check(id, "info", "rv_uso_hay")
                .con("n", lista.len())
                .items(lista)
        });
    }
    let nivel = pr["telemetria_politica"]
        .as_i64()
        .or_else(|| pr["telemetria"].as_i64());
    privacidad.push(match nivel {
        Some(0 | 1) => check("telemetria", "ok", "rv_telemetria_minima"),
        Some(3) => check("telemetria", "info", "rv_telemetria_opcional"),
        Some(n) => check("telemetria", "info", "rv_telemetria_nivel").con("n", n),
        None => check("telemetria", "info", "rv_telemetria_sin_fijar"),
    });
    let anuncios = arr(&pr["publicidad"])
        .iter()
        .filter(|p| p["activo"].as_bool() == Some(true))
        .count();
    if pr["publicidad"].is_array() {
        privacidad.push(if anuncios > 0 {
            check("publicidad", "info", "rv_publicidad_si")
        } else {
            check("publicidad", "ok", "rv_publicidad_no")
        });
    }
    let fabricante = s(&d["equipo"]["fabricante"]).to_lowercase();
    let primera = fabricante
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_owned();
    let mut de_fabrica: Vec<Item> = instalados
        .iter()
        .filter(|p| {
            let e = s(&p["editor"]).to_lowercase();
            let n = s(&p["nombre"]).to_lowercase();
            (!primera.is_empty() && primera.len() > 2 && e.contains(&primera))
                || contiene_alguno(&e, FABRICANTES)
                || contiene_alguno(&n, DE_PRUEBA)
        })
        .map(|p| item(s(&p["nombre"]), s(&p["editor"]), Vec::new()))
        .collect();
    for a in arr(&d["programas"]["appx"]) {
        let e = s(&a["editor"]).to_lowercase();
        if (!primera.is_empty() && primera.len() > 2 && e.contains(&primera))
            || contiene_alguno(&e, FABRICANTES)
        {
            de_fabrica.push(item(s(&a["nombre"]), s(&a["editor"]), vec!["app"]));
        }
    }
    if d["programas"].is_object() {
        privacidad.push(if de_fabrica.is_empty() {
            check("fabricante", "ok", "rv_fabricante_no")
        } else {
            check("fabricante", "info", "rv_fabricante_hay")
                .con("n", de_fabrica.len())
                .items(de_fabrica)
        });
    }
    if let Some(usb) = lista_de(&pr["usb"]) {
        let lista: Vec<Item> = usb
            .iter()
            .map(|u| item(s(&u["nombre"]), "", Vec::new()))
            .collect();
        privacidad.push(
            check("usb", "info", "rv_usb")
                .con("n", lista.len())
                .items(lista),
        );
    }
    if let Some(bt) = lista_de(&pr["bluetooth"]) {
        if !bt.is_empty() {
            let lista: Vec<Item> = bt.iter().map(|b| item(s(b), "", Vec::new())).collect();
            privacidad.push(
                check("bluetooth", "info", "rv_bluetooth")
                    .con("n", lista.len())
                    .items(lista),
            );
        }
    }
    if let Some(n) = dias(now, &d["equipo"]["bios_fecha"]) {
        let anos = n / 365;
        privacidad.push(if anos >= 5 {
            check("bios", "info", "rv_bios_vieja").con("n", anos)
        } else {
            check("bios", "ok", "rv_bios_ok").con("n", anos)
        });
    }

    // ---- Lo que tu antivirus anotó (Microsoft Defender)
    if let Some(ev) = lista_de(&d["defender_sucesos"]["sucesos"]) {
        let cuenta = |ids: &[i64]| {
            ev.iter()
                .filter(|e| ids.contains(&e["id"].as_i64().unwrap_or(0)))
                .count()
        };
        let amenazas: Vec<Item> = ev
            .iter()
            .filter(|e| matches!(e["id"].as_i64(), Some(1116 | 1117)))
            .map(|e| {
                let mut m = vec![if e["id"].as_i64() == Some(1117) {
                    "actuo"
                } else {
                    "detectada"
                }];
                if s(&e["gravedad"]).is_empty() {
                    m.push("detectada");
                    m.dedup();
                }
                item(s(&e["amenaza"]), s(&e["accion"]), m).en(e["cuando"].as_i64())
            })
            .collect();
        antivirus.push(if amenazas.is_empty() {
            check("amenazas", "ok", "rv_amenazas_no")
        } else {
            check("amenazas", "aviso", "rv_amenazas_hay")
                .con("n", amenazas.len())
                .items(amenazas)
        });
        antivirus.push(
            check("analisis", "info", "rv_analisis")
                .con("analisis", cuenta(&[1001]))
                .con("firmas", cuenta(&[2000])),
        );
        if cuenta(&[5001]) > 0 {
            antivirus.push(
                check("tiempo_real_apagado", "aviso", "rv_tiempo_real_apagado")
                    .con("n", cuenta(&[5001])),
            );
        }
    }

    [
        ("proteccion", proteccion),
        ("acceso", acceso),
        ("arranque", arranque),
        ("salida", salida),
        ("privacidad", privacidad),
        ("antivirus", antivirus),
    ]
    .into_iter()
    .filter(|(_, c)| !c.is_empty())
    .map(|(id, checks)| Grupo { id, checks })
    .collect()
}

/// Every sentence key the checks can use (the test below checks the three languages have them).
#[cfg(test)]
pub(crate) const FRASES: &[&str] = &[
    "rv_nd",
    "rv_actualizaciones_apagadas",
    "rv_actualizaciones_viejas",
    "rv_actualizaciones_pausadas",
    "rv_actualizaciones_reinicio",
    "rv_actualizaciones_ok",
    "rv_antivirus_ninguno",
    "rv_antivirus_viejo",
    "rv_antivirus_ok",
    "rv_defender_firmas",
    "rv_defender_manipulacion",
    "rv_defender_ok",
    "rv_defender_pasivo",
    "rv_cortafuegos_ok",
    "rv_cortafuegos_apagado",
    "rv_uac_apagado",
    "rv_uac_ok",
    "rv_smb1_activo",
    "rv_smb1_ok",
    "rv_arranque_seguro_ok",
    "rv_arranque_seguro_apagado",
    "rv_arranque_seguro_sin_uefi",
    "rv_tpm_ok",
    "rv_tpm_apagado",
    "rv_tpm_ninguno",
    "rv_cifrado_ok",
    "rv_cifrado_no",
    "rv_cifrado_nd",
    "rv_integridad_ok",
    "rv_integridad_no",
    "rv_cuentas_aviso",
    "rv_cuentas_ok",
    "rv_rdp_sin_nla",
    "rv_rdp_activo",
    "rv_rdp_ok",
    "rv_asistencia_activa",
    "rv_acceso_remoto_ok",
    "rv_acceso_remoto_hay",
    "rv_fallidos",
    "rv_fallidos_ok",
    "rv_autologon_clave",
    "rv_autologon",
    "rv_puertos_delicados",
    "rv_puertos_ok",
    "rv_puertos_hay",
    "rv_compartido_ok",
    "rv_compartido_disco",
    "rv_compartido_hay",
    "rv_inventario",
    "rv_dns_falsea",
    "rv_dns_redirige",
    "rv_dns_ok",
    "rv_dns_sin_prueba",
    "rv_proxy_ok",
    "rv_proxy_hay",
    "rv_vpn_no",
    "rv_vpn_si",
    "rv_hosts_ok",
    "rv_hosts_redirige",
    "rv_hosts_bloquea",
    "rv_certificados_ok",
    "rv_certificados_hay",
    "rv_certificados_nuevo",
    "rv_wifi_ok",
    "rv_wifi_viejo",
    "rv_wifi_nd",
    "rv_wifi_abierta",
    "rv_perfil_privada",
    "rv_perfil_publica",
    "rv_uso_nunca",
    "rv_uso_ahora",
    "rv_uso_hay",
    "rv_telemetria_minima",
    "rv_telemetria_opcional",
    "rv_telemetria_nivel",
    "rv_telemetria_sin_fijar",
    "rv_publicidad_si",
    "rv_publicidad_no",
    "rv_fabricante_no",
    "rv_fabricante_hay",
    "rv_usb",
    "rv_bluetooth",
    "rv_bios_vieja",
    "rv_bios_ok",
    "rv_amenazas_no",
    "rv_amenazas_hay",
    "rv_analisis",
    "rv_tiempo_real_apagado",
];

/// Every check id (title `rv_t_<id>`), group (`rv_g_<id>`) and tag (`rv_marca_<x>`).
#[cfg(test)]
pub(crate) const IDS: &[&str] = &[
    "actualizaciones",
    "antivirus",
    "defender",
    "cortafuegos",
    "uac",
    "smb1",
    "arranque_seguro",
    "tpm",
    "cifrado",
    "integridad_memoria",
    "cuentas",
    "escritorio_remoto",
    "asistencia_remota",
    "acceso_remoto",
    "fallidos",
    "autologon",
    "puertos",
    "compartido",
    "inicio",
    "tareas",
    "servicios",
    "controladores",
    "dns",
    "proxy",
    "vpn",
    "hosts",
    "certificados",
    "wifi",
    "perfil_red",
    "camara",
    "microfono",
    "ubicacion",
    "telemetria",
    "publicidad",
    "fabricante",
    "usb",
    "bluetooth",
    "bios",
    "amenazas",
    "analisis",
    "tiempo_real_apagado",
];
#[cfg(test)]
pub(crate) const GRUPOS: &[&str] = &[
    "proteccion",
    "acceso",
    "arranque",
    "salida",
    "privacidad",
    "antivirus",
];
#[cfg(test)]
pub(crate) const MARCAS: &[&str] = &[
    "nuevo",
    "sin_firma",
    "firma_mala",
    "no_existe",
    "oculta",
    "apagado",
    "sin_actualizar",
    "admin",
    "clave_opcional",
    "invitado",
    "servicio",
    "delicado",
    "dns_equipo",
    "dns_casa",
    "dns_publico",
    "dns_otro",
    "usuario",
    "sistema",
    "conectada",
    "bloqueado",
    "redirigido",
    "de_usuario",
    "en_uso",
    "app",
    "actuo",
    "detectada",
];

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn muestra() -> Value {
        serde_json::from_str(include_str!("../tests/revision-muestra.json")).unwrap()
    }

    #[test]
    fn a_sample_computer_reads_as_expected() {
        let now = 1_791_900_000_000;
        let g = evaluar(&muestra(), None, now);
        let c = |id: &str| {
            g.iter()
                .flat_map(|g| &g.checks)
                .find(|c| c.id == id)
                .unwrap()
                .clone()
        };
        assert_eq!(c("actualizaciones").estado, "info");
        assert_eq!(c("antivirus").estado, "ok");
        assert_eq!(c("cortafuegos").estado, "alerta");
        assert_eq!(c("escritorio_remoto").estado, "ok");
        assert_eq!(c("acceso_remoto").estado, "aviso");
        assert_eq!(c("acceso_remoto").lista[0].t, "AnyDesk");
        assert_eq!(c("dns").estado, "aviso", "{:?}", c("dns"));
        assert_eq!(c("certificados").estado, "info");
        assert_eq!(c("certificados").lista.len(), 1, "{:?}", c("certificados"));
        assert_eq!(c("camara").estado, "info");
        assert!(c("camara").lista[0].marcas.contains(&"en_uso"));
        assert_eq!(c("fabricante").lista.len(), 2, "{:?}", c("fabricante"));
        assert_eq!(c("hosts").estado, "aviso");
        assert_eq!(c("wifi").estado, "ok");
        assert_eq!(c("inicio").estado, "aviso");
        // The addresses never reach the page's lists.
        let todo = serde_json::to_string(&g).unwrap();
        assert!(
            !todo.contains("192.168.1.1") && !todo.contains("200.75."),
            "{todo}"
        );
    }

    #[test]
    fn what_appears_after_the_first_review_is_new() {
        let now = 1_791_900_000_000;
        let d = muestra();
        let mut v = Vistos {
            creada: now - 10 * DIA,
            vistos: BTreeMap::new(),
        };
        for k in claves_inventario(&d) {
            v.vistos.insert(k, now - 10 * DIA);
        }
        v.vistos.insert(
            "inicio:Actualizador|c:\\users\\ana\\appdata\\roaming\\upd\\upd.exe".into(),
            now - DIA,
        );
        let g = evaluar(&d, Some(&v), now);
        let r = resumir(&g);
        assert_eq!(r.nuevos, 1, "{g:?}");
    }

    #[test]
    fn every_sentence_title_group_and_tag_exists_in_the_three_languages() {
        let now = 1_791_900_000_000;
        let g = evaluar(&muestra(), None, now);
        for c in g.iter().flat_map(|g| &g.checks) {
            assert!(FRASES.contains(&c.frase), "{}", c.frase);
            assert!(IDS.contains(&c.id), "{}", c.id);
            for i in &c.lista {
                for m in &i.marcas {
                    assert!(MARCAS.contains(m), "{m}");
                }
            }
        }
        for (l, t) in [
            ("es", include_str!("../i18n/es.json")),
            ("en", include_str!("../i18n/en.json")),
            ("pt", include_str!("../i18n/pt.json")),
        ] {
            let v: Value = serde_json::from_str(t).unwrap();
            let p = &v["panel"];
            let mut faltan: Vec<String> = Vec::new();
            for k in FRASES
                .iter()
                .map(|k| (*k).to_owned())
                .chain(IDS.iter().map(|k| format!("rv_t_{k}")))
                .chain(GRUPOS.iter().map(|k| format!("rv_g_{k}")))
                .chain(MARCAS.iter().map(|k| format!("rv_marca_{k}")))
            {
                if !p[&k].is_string() {
                    faltan.push(k);
                }
            }
            assert!(faltan.is_empty(), "{l}: {faltan:?}");
        }
    }

    #[test]
    fn resolvers_are_told_by_place_not_by_address() {
        assert_eq!(tipo_resolutor("192.168.1.1").0, "casa");
        assert_eq!(tipo_resolutor("127.0.0.1").0, "equipo");
        assert_eq!(tipo_resolutor("8.8.8.8"), ("publico", "Google".to_owned()));
        assert_eq!(tipo_resolutor("200.75.51.132").0, "otro");
    }
}
