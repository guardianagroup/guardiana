//! «Vigilancia» (1.0.45): the constant, read-only watch, and the page and the top bar that show
//! it. The owner, 18 Oct 2026: «la experiencia del cliente debe ser sentir el monitoreo
//! constante… para saber que todo está bien y nada está vigilando o extrayendo sus datos».
//!
//! What is read and how it is judged lives in `guardiana_apps::vigilancia`; this module runs it in
//! the service, all the time and at low frequency:
//!
//! - the camera and the microphone every [`CADA_RAPIDA`] (the registry, read directly: free);
//! - what starts on its own, the privacy settings and the protection every [`CADA_LENTA`] (one
//!   low-priority PowerShell on Windows), or now when the page asks.
//!
//! Every finding is written to the watch's chain in the ledger (`guardiana_core::vigilancia`).
//! Only what matters becomes a notice, once (the repeat filter is in the apps crate), and a
//! notice is one calm line. The base the comparisons start from is kept in the ledger's settings,
//! so «Borrar todo» takes it with everything else and the next look starts again from zero.
//!
//! Two answers: `/api/vigilancia` for the page (each watch's live state and the history) and
//! `/api/vigilancia/resumen` for the top bar's pill and the «Pulso del día» (small, cheap, asked
//! often).

// The handlers answer with axum's `Response` as their error, like the rest of the panel.
#![allow(clippy::result_large_err)]

use std::collections::{BTreeMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use axum::extract::State;
use axum::Json;
use guardiana_apps::vigilancia::{self as vg, Base, Foto, Hallazgo, Uso};
use guardiana_core::time::now_ms;
use guardiana_core::vigilancia::{NuevoSuceso, Suceso};
use guardiana_core::{local_day_index, local_midnight, Ledger};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::api::{with_ledger, ApiResult};
use crate::auth::{Session, Zona};
use crate::AppState;

/// How often the camera and the microphone are looked at.
pub(crate) const CADA_RAPIDA: Duration = Duration::from_secs(15);
/// How often what starts on its own, the privacy settings and the protection are looked at.
pub(crate) const CADA_LENTA: Duration = Duration::from_secs(5 * 60);
/// The first slow look waits this long after the service starts: the computer is busy starting
/// then, and a panel opened for a moment (the tests) never pays for a PowerShell.
const PRIMERA_LENTA: Duration = Duration::from_secs(45);

const MINUTO: i64 = 60_000;
const HORA: i64 = 60 * MINUTO;
const DIA: i64 = 24 * HORA;

/// Ledger settings (under `vigilancia_`) where the two bases live: the camera and microphone one
/// is small and written often; the other one changes every few minutes at most.
const BASE_USOS: &str = "base_usos";
const BASE_LENTA: &str = "base";
/// The last notice the person has seen (its id in the chain).
const VISTO: &str = "visto";

/// The four watches, in the page's order.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const VIGILANCIAS: [&str; 4] =
    ["camara_microfono", "arranque", "privacidad", "proteccion"];

/// What the watch keeps in memory: when each one last looked, and what it saw.
#[derive(Default)]
pub(crate) struct Vigilancia {
    estado: Mutex<Estado>,
    /// The page asked for a slow look now.
    pedido: AtomicBool,
    /// A slow look is running.
    corriendo: AtomicBool,
}

#[derive(Default)]
struct Estado {
    /// When this process started watching.
    desde: Option<i64>,
    /// Each watch's last look.
    ultimas: BTreeMap<&'static str, Mirada>,
    /// The uses Windows recorded at the last look (in use now, and recent ones).
    usos: Vec<Uso>,
    /// The programs running at the last look, when asked.
    en_marcha: Option<HashSet<String>>,
    /// The last slow look.
    foto: Option<Foto>,
    /// The moments of the looks of the last day and a bit, for the «Pulso del día»: the fast
    /// watch counts at most once a minute.
    comprobaciones: VecDeque<(i64, &'static str)>,
}

#[derive(Clone, Default, Serialize)]
struct Mirada {
    /// Unix ms of the last look that answered.
    ultima: Option<i64>,
    /// Whether this system can say it at all.
    disponible: bool,
    /// Why not: `no_mac` or `no_sistema`.
    motivo: Option<&'static str>,
    /// What failed at the last look, if it failed.
    error: Option<String>,
}

fn sistema() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

fn no_disponible() -> &'static str {
    if cfg!(target_os = "macos") {
        "no_mac"
    } else {
        "no_sistema"
    }
}

impl Vigilancia {
    /// Starts the watch in the background. It ends by itself when the panel stops.
    pub(crate) fn arrancar(state: &Arc<AppState>) {
        if let Ok(mut e) = state.vigilancia.estado.lock() {
            e.desde = Some(now_ms());
        }
        let debil: Weak<AppState> = Arc::downgrade(state);
        tokio::spawn(async move {
            let inicio = tokio::time::Instant::now();
            let mut ultima_lenta: Option<tokio::time::Instant> = None;
            tokio::time::sleep(Duration::from_secs(2)).await;
            loop {
                let Some(st) = debil.upgrade() else { break };
                let s2 = Arc::clone(&st);
                let _ = tokio::task::spawn_blocking(move || mira_rapido(&s2)).await;
                let toca = match ultima_lenta {
                    None => inicio.elapsed() >= PRIMERA_LENTA,
                    Some(t) => t.elapsed() >= CADA_LENTA,
                };
                if st.vigilancia.pedido.swap(false, Ordering::SeqCst) || toca {
                    ultima_lenta = Some(tokio::time::Instant::now());
                    lanza_lenta(&st);
                }
                drop(st);
                // Short naps, so a «Comprobar ahora» does not wait for the next quarter minute.
                for _ in 0..CADA_RAPIDA.as_secs() {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    match debil.upgrade() {
                        Some(st) if st.vigilancia.pedido.load(Ordering::SeqCst) => break,
                        Some(_) => {}
                        None => return,
                    }
                }
            }
        });
    }
}

/// Runs a slow look off the runtime, unless one is running.
fn lanza_lenta(state: &Arc<AppState>) {
    if state.vigilancia.corriendo.swap(true, Ordering::SeqCst) {
        return;
    }
    let st = Arc::clone(state);
    tokio::task::spawn_blocking(move || {
        mira_lento(&st);
        st.vigilancia.corriendo.store(false, Ordering::SeqCst);
    });
}

fn lee_base(l: &Ledger, nombre: &str) -> (Base, String) {
    let texto = l
        .ajuste_vigilancia(nombre)
        .ok()
        .flatten()
        .unwrap_or_default();
    (serde_json::from_str(&texto).unwrap_or_default(), texto)
}

fn guarda_base(l: &Ledger, nombre: &str, base: &Base, antes: &str) {
    if let Ok(t) = serde_json::to_string(base) {
        if t != antes {
            let _ = l.pon_ajuste_vigilancia(nombre, &t);
        }
    }
}

/// Writes the findings to the watch's chain.
fn anota(l: &Ledger, hallazgos: &[Hallazgo], ahora: i64) {
    for h in hallazgos {
        let _ = l.anota_vigilancia(&NuevoSuceso {
            ts: ahora,
            tipo: h.tipo.to_owned(),
            clave: h.clave.clone(),
            importancia: if h.aviso { "aviso" } else { "info" }.to_owned(),
            texto: h.texto.clone(),
            datos: json!({ "frase": h.frase, "datos": h.datos }),
        });
    }
}

fn cuenta_comprobacion(e: &mut Estado, ahora: i64, id: &'static str) {
    let reciente = e
        .comprobaciones
        .iter()
        .rev()
        .find(|(_, x)| *x == id)
        .is_some_and(|(t, _)| ahora - t < MINUTO);
    if id != "camara_microfono" || !reciente {
        e.comprobaciones.push_back((ahora, id));
    }
    while e
        .comprobaciones
        .front()
        .is_some_and(|(t, _)| ahora - t > DIA + 2 * HORA)
    {
        e.comprobaciones.pop_front();
    }
}

/// The camera and the microphone, now.
fn mira_rapido(state: &AppState) {
    let ahora = now_ms();
    let r = vg::usos();
    let mut mirada = Mirada {
        disponible: true,
        ..Mirada::default()
    };
    let mut usos = Vec::new();
    let mut en_marcha = None;
    match r {
        Err(guardiana_apps::Error::NoSoportado) => {
            mirada.disponible = false;
            mirada.motivo = Some(no_disponible());
        }
        Err(e) => mirada.error = Some(e.to_string()),
        Ok(u) => {
            // Only when something is marked in use: then it is worth asking what is running.
            if u.iter().any(|x| x.hasta.is_none()) {
                en_marcha = vg::en_marcha();
            }
            if let Ok(l) = state.ledger.lock() {
                let (mut base, antes) = lee_base(&l, BASE_USOS);
                let h = base.mira_usos(&u, en_marcha.as_ref(), ahora);
                anota(&l, &h, ahora);
                guarda_base(&l, BASE_USOS, &base, &antes);
            }
            mirada.ultima = Some(ahora);
            usos = u;
        }
    }
    if let Ok(mut e) = state.vigilancia.estado.lock() {
        if mirada.ultima.is_some() {
            cuenta_comprobacion(&mut e, ahora, "camara_microfono");
        } else if let Some(previa) = e.ultimas.get("camara_microfono") {
            // A failed look keeps the time of the last one that answered.
            mirada.ultima = previa.ultima;
        }
        e.ultimas.insert("camara_microfono", mirada);
        e.usos = usos;
        e.en_marcha = en_marcha;
    }
}

/// What starts on its own, the privacy settings and the protection, now.
fn mira_lento(state: &AppState) {
    let r = vg::foto();
    let ahora = now_ms();
    let mut miradas: BTreeMap<&'static str, Mirada> = BTreeMap::new();
    match r {
        Err(e) => {
            for id in ["arranque", "privacidad", "proteccion"] {
                miradas.insert(
                    id,
                    Mirada {
                        disponible: true,
                        error: Some(e.to_string()),
                        ..Mirada::default()
                    },
                );
            }
        }
        Ok(f) => {
            if let Ok(l) = state.ledger.lock() {
                let (mut base, antes) = lee_base(&l, BASE_LENTA);
                let mut h = Vec::new();
                // A list read only in part (the tasks or the services did not answer) is not
                // compared: everything missing would look removed, and back next time as new.
                let parcial = ["arranque", "tareas", "servicios"]
                    .iter()
                    .find_map(|k| f.errores.get(*k).cloned());
                miradas.insert(
                    "arranque",
                    match (&f.arranque, parcial) {
                        (Some(_), Some(e)) => Mirada {
                            disponible: true,
                            error: Some(e),
                            ..Mirada::default()
                        },
                        (Some(a), None) => {
                            h.extend(base.mira_arranque(a, &f.cuentas, ahora));
                            listo()
                        }
                        (None, e) => falta(e),
                    },
                );
                miradas.insert(
                    "privacidad",
                    match &f.privacidad {
                        Some(p) => {
                            h.extend(base.mira_privacidad(p, &f.cuentas, ahora));
                            listo()
                        }
                        None => falta(f.errores.get("privacidad").cloned()),
                    },
                );
                miradas.insert(
                    "proteccion",
                    match &f.proteccion {
                        Some(p) => {
                            h.extend(base.mira_proteccion(p, ahora));
                            listo()
                        }
                        None => falta(f.errores.get("proteccion").cloned()),
                    },
                );
                anota(&l, &h, ahora);
                guarda_base(&l, BASE_LENTA, &base, &antes);
            }
            if let Ok(mut e) = state.vigilancia.estado.lock() {
                e.foto = Some(f);
            }
        }
    }
    if let Ok(mut e) = state.vigilancia.estado.lock() {
        for (id, mut m) in miradas {
            if m.ultima.is_some() {
                cuenta_comprobacion(&mut e, ahora, id);
            } else if let Some(previa) = e.ultimas.get(id) {
                m.ultima = previa.ultima;
            }
            e.ultimas.insert(id, m);
        }
    }
    fn listo() -> Mirada {
        Mirada {
            ultima: Some(now_ms()),
            disponible: true,
            ..Mirada::default()
        }
    }
    fn falta(error: Option<String>) -> Mirada {
        match error {
            Some(e) => Mirada {
                disponible: true,
                error: Some(e),
                ..Mirada::default()
            },
            None => Mirada {
                disponible: false,
                motivo: Some(no_disponible()),
                ..Mirada::default()
            },
        }
    }
}

// ---------------------------------------------------------------------------------------------
// What the screens get.

/// One watch, as the page shows it.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct VistaVigilancia {
    id: &'static str,
    disponible: bool,
    motivo: Option<&'static str>,
    ultima: Option<i64>,
    error: Option<String>,
    /// `esperando` (not looked yet), `ok`, `en_uso`, `aviso`, `atencion`, `nd`.
    estado: &'static str,
    /// The one-line state: a key of the panel's texts, filled from `datos`.
    frase: &'static str,
    datos: BTreeMap<&'static str, String>,
    /// What is behind it, as the page draws it for this watch.
    lista: Vec<Value>,
}

/// One condition of attention, for the pill and the page.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub(crate) struct Atencion {
    frase: &'static str,
    x: String,
}

fn atenciones(foto: Option<&Foto>) -> Vec<Atencion> {
    let Some(p) = foto.and_then(|f| f.proteccion.as_ref()) else {
        return Vec::new();
    };
    vg::atencion_proteccion(p)
        .into_iter()
        .map(|c| match c.strip_prefix("cortafuegos:") {
            Some(perfil) => Atencion {
                frase: "vg_atencion_cortafuegos",
                x: perfil.to_owned(),
            },
            None if c == "tiempo_real" => Atencion {
                frase: "vg_atencion_tiempo_real",
                x: String::new(),
            },
            None => Atencion {
                frase: "vg_atencion_sin_antivirus",
                x: String::new(),
            },
        })
        .collect()
}

fn vista(
    id: &'static str,
    m: Option<&Mirada>,
    estado: &'static str,
    frase: &'static str,
) -> VistaVigilancia {
    let m = m.cloned().unwrap_or_default();
    let (estado, frase) = if !m.disponible && m.ultima.is_none() && m.motivo.is_some() {
        (
            "nd",
            if m.motivo == Some("no_mac") {
                "vg_no_mac"
            } else {
                "vg_no_sistema"
            },
        )
    } else if m.ultima.is_none() {
        ("esperando", "vg_esperando")
    } else {
        (estado, frase)
    };
    VistaVigilancia {
        id,
        disponible: m.disponible,
        motivo: m.motivo,
        ultima: m.ultima,
        error: m.error,
        estado,
        frase,
        datos: BTreeMap::new(),
        lista: Vec::new(),
    }
}

/// The live state of the four watches, from what was seen and what is remembered.
fn vistas(
    e: &Estado,
    base_usos: &Base,
    base: &Base,
    historial: &[Suceso],
    ahora: i64,
) -> Vec<VistaVigilancia> {
    let mut out = Vec::new();

    // ---- Camera and microphone.
    let abiertos = e.en_marcha.as_ref();
    let mut en_uso: Vec<&Uso> = e
        .usos
        .iter()
        .filter(|u| {
            u.hasta.is_none()
                && match (abiertos, &u.ruta) {
                    (Some(m), Some(_)) if !m.is_empty() => m.contains(&u.programa.to_lowercase()),
                    _ => true,
                }
        })
        .collect();
    en_uso.sort_by_key(|u| std::cmp::Reverse(u.desde));
    let mut recientes: Vec<&Uso> = e
        .usos
        .iter()
        .filter(|u| u.hasta.is_some_and(|h| ahora - h < DIA))
        .collect();
    recientes.sort_by_key(|u| std::cmp::Reverse(u.hasta));
    recientes.truncate(20);
    let otros = en_uso
        .iter()
        .filter(|u| !vg::es_propio(&u.programa))
        .count();
    let mut v = if otros > 0 {
        let mut v = vista(
            "camara_microfono",
            e.ultimas.get("camara_microfono"),
            "en_uso",
            "vg_cm_en_uso",
        );
        v.datos.insert("n", otros.to_string());
        v
    } else {
        vista(
            "camara_microfono",
            e.ultimas.get("camara_microfono"),
            "ok",
            "vg_cm_nadie",
        )
    };
    let item = |u: &Uso, ahora_mismo: bool| {
        json!({
            "programa": u.programa,
            "ruta": u.ruta,
            "recurso": u.recurso.como_texto(),
            "desde": u.desde,
            "hasta": u.hasta,
            "ahora": ahora_mismo,
            "propio": vg::es_propio(&u.programa),
            "motor_web": vg::es_motor_web(&u.programa),
        })
    };
    v.lista = en_uso
        .iter()
        .map(|&u| item(u, true))
        .chain(recientes.iter().map(|&u| item(u, false)))
        .collect();
    if let Some(d) = base_usos.usos_desde {
        v.datos.insert("desde", d.to_string());
    }
    out.push(v);

    // ---- What starts on its own.
    let presentes = base.arranque.values().filter(|r| r.presente).count();
    let desde = base.arranque_desde.unwrap_or(i64::MAX);
    let mut nuevos: Vec<(&String, &vg::Recordado)> = base
        .arranque
        .iter()
        .filter(|(_, r)| r.visto > desde && ahora - r.visto < 7 * DIA)
        .collect();
    nuevos.sort_by_key(|(_, r)| std::cmp::Reverse(r.visto));
    let hoy = nuevos
        .iter()
        .filter(|(_, r)| !r.sistema && ahora - r.visto < DIA)
        .count();
    let mut v = if hoy > 0 {
        let mut v = vista(
            "arranque",
            e.ultimas.get("arranque"),
            "aviso",
            "vg_ar_nuevos",
        );
        v.datos.insert("n", hoy.to_string());
        v
    } else {
        vista("arranque", e.ultimas.get("arranque"), "ok", "vg_ar_ok")
    };
    v.datos.insert("total", presentes.to_string());
    if let Some(d) = base.arranque_desde {
        v.datos.insert("desde", d.to_string());
    }
    v.lista = nuevos
        .iter()
        .take(50)
        .map(|(_, r)| {
            json!({ "titulo": r.titulo, "tipo": r.tipo, "visto": r.visto, "presente": r.presente, "sistema": r.sistema })
        })
        .collect();
    out.push(v);

    // ---- Privacy settings.
    let cambios: Vec<&Suceso> = historial
        .iter()
        .filter(|s| s.tipo == "privacidad" && ahora - s.ts < 7 * DIA)
        .filter(|s| s.datos["frase"] == "vg_ev_privacidad_cambio")
        .collect();
    let mut v = if cambios.iter().any(|s| ahora - s.ts < DIA) {
        vista(
            "privacidad",
            e.ultimas.get("privacidad"),
            "aviso",
            "vg_pr_cambios",
        )
    } else {
        vista("privacidad", e.ultimas.get("privacidad"), "ok", "vg_pr_ok")
    };
    v.datos.insert("n", base.privacidad.len().to_string());
    v.datos.insert("cambios", cambios.len().to_string());
    let cuentas = e.foto.as_ref().map(|f| &f.cuentas);
    v.lista = e
        .foto
        .as_ref()
        .and_then(|f| f.privacidad.as_ref())
        .map(|p| {
            p.iter()
                .map(|(k, valor)| {
                    let (ajuste, sid) = k
                        .split_once(':')
                        .map_or((k.as_str(), None), |(a, s)| (a, Some(s)));
                    let cuenta = sid
                        .map(|s| {
                            cuentas
                                .and_then(|c| c.get(s).cloned())
                                .unwrap_or_else(|| s.to_owned())
                        })
                        .unwrap_or_default();
                    json!({ "ajuste": ajuste, "cuenta": cuenta, "valor": valor })
                })
                .collect()
        })
        .unwrap_or_default();
    out.push(v);

    // ---- Antivirus and firewall.
    let at = atenciones(e.foto.as_ref());
    let mut v = if at.is_empty() {
        vista("proteccion", e.ultimas.get("proteccion"), "ok", "vg_pt_ok")
    } else {
        vista(
            "proteccion",
            e.ultimas.get("proteccion"),
            "atencion",
            "vg_pt_atencion",
        )
    };
    if let Some(p) = e.foto.as_ref().and_then(|f| f.proteccion.as_ref()) {
        let mut l = Vec::new();
        for a in p.antivirus.iter().flatten() {
            l.push(json!({ "clase": "antivirus", "nombre": a.nombre, "activo": a.activo }));
        }
        if let Some(d) = &p.defender {
            l.push(json!({ "clase": "tiempo_real", "nombre": "Microsoft Defender", "activo": d.tiempo_real, "al_mando": d.al_mando() }));
            l.push(json!({ "clase": "manipulacion", "nombre": "Microsoft Defender", "activo": d.manipulacion, "al_mando": d.al_mando() }));
        }
        for (perfil, on) in p.cortafuegos.iter().flatten() {
            l.push(json!({ "clase": "perfil", "nombre": perfil, "activo": on }));
        }
        for a in p.cortafuegos_otros.iter().flatten() {
            l.push(json!({ "clase": "cortafuegos", "nombre": a.nombre, "activo": a.activo }));
        }
        v.lista = l;
    }
    out.push(v);
    out
}

/// The pill's word: `atencion` when something is off or a notice is unseen, `observando` while
/// the watch answers, `iniciando` before its first look, `no_disponible` when nothing can be
/// watched here.
fn estado_global(vistas: &[VistaVigilancia], hay_avisos: bool) -> &'static str {
    if vistas.iter().any(|v| v.estado == "atencion") || hay_avisos {
        "atencion"
    } else if vistas.iter().all(|v| v.estado == "nd") {
        "no_disponible"
    } else if vistas.iter().all(|v| v.ultima.is_none()) {
        "iniciando"
    } else {
        "observando"
    }
}

/// The «Pulso del día»: the looks, the facts written and the notices of each hour of today.
#[derive(Serialize, Debug, PartialEq, Eq, Default)]
pub(crate) struct Pulso {
    /// Unix ms of today's local midnight.
    desde: i64,
    comprobaciones: usize,
    sucesos: usize,
    avisos: usize,
    /// One per hour from midnight: `[looks, facts, notices]`.
    horas: Vec<[usize; 3]>,
}

fn pulso(
    comprobaciones: &VecDeque<(i64, &'static str)>,
    sucesos: &[Suceso],
    ahora: i64,
    zona: i64,
) -> Pulso {
    let desde = local_midnight(local_day_index(ahora, zona), zona);
    let hora = |t: i64| usize::try_from((t - desde) / HORA).ok().filter(|h| *h < 24);
    let mut p = Pulso {
        desde,
        horas: vec![[0; 3]; 24],
        ..Pulso::default()
    };
    for (t, _) in comprobaciones.iter().filter(|(t, _)| *t >= desde) {
        p.comprobaciones += 1;
        if let Some(h) = hora(*t) {
            p.horas[h][0] += 1;
        }
    }
    for s in sucesos.iter().filter(|s| s.ts >= desde) {
        p.sucesos += 1;
        let aviso = s.importancia == "aviso";
        if aviso {
            p.avisos += 1;
        }
        if let Some(h) = hora(s.ts) {
            p.horas[h][1] += 1;
            if aviso {
                p.horas[h][2] += 1;
            }
        }
    }
    p
}

/// The notices not seen yet: of the last week, newest first.
fn sin_ver(historial: &[Suceso], visto: i64, ahora: i64) -> Vec<&Suceso> {
    historial
        .iter()
        .filter(|s| s.importancia == "aviso" && s.id > visto && ahora - s.ts < 7 * DIA)
        .take(20)
        .collect()
}

#[derive(Serialize)]
pub(crate) struct Respuesta {
    sistema: &'static str,
    estado: &'static str,
    ultima_senal: Option<i64>,
    vigila_desde: Option<i64>,
    comprobando: bool,
    vigilancias: Vec<VistaVigilancia>,
    atencion: Vec<Atencion>,
    avisos: Vec<Suceso>,
    historial: Vec<Suceso>,
    /// The chain checked: `null` when it verifies, the first row that does not otherwise.
    cadena_rota: Option<i64>,
    pulso: Pulso,
    cada_rapida_s: u64,
    cada_lenta_s: u64,
}

/// The top bar's answer: the pill and the «Pulso del día», nothing heavy.
#[derive(Serialize)]
pub(crate) struct Resumen {
    sistema: &'static str,
    /// `observando`, `atencion`, `iniciando` or `no_disponible`.
    estado: &'static str,
    ultima_senal: Option<i64>,
    atencion: Vec<Atencion>,
    /// The notices not seen yet (the newest five), each one line.
    avisos: Vec<Suceso>,
    avisos_sin_ver: usize,
    /// What uses the camera or the microphone now (other programs).
    en_uso: Vec<Value>,
    vigilancias: Vec<Value>,
    pulso: Pulso,
}

struct Lectura {
    vistas: Vec<VistaVigilancia>,
    atencion: Vec<Atencion>,
    historial: Vec<Suceso>,
    visto: i64,
    pulso: Pulso,
    desde: Option<i64>,
    cadena_rota: Option<i64>,
}

fn lee(
    state: &AppState,
    zona: i64,
    historial_n: usize,
    con_cadena: bool,
) -> Result<Lectura, axum::response::Response> {
    let ahora = now_ms();
    let medianoche = local_midnight(local_day_index(ahora, zona), zona);
    let (base_usos, base, historial, hoy, visto, cadena_rota) = with_ledger(state, |l| {
        let (bu, _) = lee_base(l, BASE_USOS);
        let (b, _) = lee_base(l, BASE_LENTA);
        let historial = l.vigilancia(historial_n.max(50), None)?;
        let hoy = l.vigilancia(5_000, Some(medianoche))?;
        let visto = l
            .ajuste_vigilancia(VISTO)?
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0);
        let rota = if con_cadena {
            l.comprueba_vigilancia()?
        } else {
            None
        };
        Ok((bu, b, historial, hoy, visto, rota))
    })?;
    let e = state.vigilancia.estado.lock().map_err(|_| {
        use axum::response::IntoResponse as _;
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "vigilancia: estado envenenado",
        )
            .into_response()
    })?;
    let vistas = vistas(&e, &base_usos, &base, &historial, ahora);
    Ok(Lectura {
        atencion: atenciones(e.foto.as_ref()),
        pulso: pulso(&e.comprobaciones, &hoy, ahora, zona),
        desde: e.desde,
        vistas,
        historial,
        visto,
        cadena_rota,
    })
}

fn ultima_senal(v: &[VistaVigilancia]) -> Option<i64> {
    v.iter().filter_map(|x| x.ultima).max()
}

/// `GET /api/vigilancia`: the page.
pub(crate) async fn ver(
    State(state): State<Arc<AppState>>,
    _s: Session,
    Zona(zona): Zona,
) -> ApiResult<Respuesta> {
    let ahora = now_ms();
    let l = lee(&state, zona, 300, true)?;
    let avisos: Vec<Suceso> = sin_ver(&l.historial, l.visto, ahora)
        .into_iter()
        .cloned()
        .collect();
    Ok(Json(Respuesta {
        sistema: sistema(),
        estado: estado_global(&l.vistas, !avisos.is_empty()),
        ultima_senal: ultima_senal(&l.vistas),
        vigila_desde: l.desde,
        comprobando: state.vigilancia.corriendo.load(Ordering::SeqCst)
            || state.vigilancia.pedido.load(Ordering::SeqCst),
        atencion: l.atencion,
        avisos,
        historial: l.historial,
        cadena_rota: l.cadena_rota,
        pulso: l.pulso,
        vigilancias: l.vistas,
        cada_rapida_s: CADA_RAPIDA.as_secs(),
        cada_lenta_s: CADA_LENTA.as_secs(),
    }))
}

/// `GET /api/vigilancia/resumen`: the top bar's pill and the «Pulso del día».
pub(crate) async fn resumen(
    State(state): State<Arc<AppState>>,
    _s: Session,
    Zona(zona): Zona,
) -> ApiResult<Resumen> {
    let ahora = now_ms();
    let l = lee(&state, zona, 100, false)?;
    let sin = sin_ver(&l.historial, l.visto, ahora);
    let en_uso = l
        .vistas
        .iter()
        .find(|v| v.id == "camara_microfono")
        .map(|v| {
            v.lista
                .iter()
                .filter(|x| x["ahora"] == true && x["propio"] == false)
                .map(|x| json!({ "programa": x["programa"], "recurso": x["recurso"], "desde": x["desde"] }))
                .collect()
        })
        .unwrap_or_default();
    Ok(Json(Resumen {
        sistema: sistema(),
        estado: estado_global(&l.vistas, !sin.is_empty()),
        ultima_senal: ultima_senal(&l.vistas),
        atencion: l.atencion,
        avisos_sin_ver: sin.len(),
        avisos: sin.into_iter().take(5).cloned().collect(),
        en_uso,
        vigilancias: l
            .vistas
            .iter()
            .map(|v| json!({ "id": v.id, "estado": v.estado, "ultima": v.ultima, "disponible": v.disponible }))
            .collect(),
        pulso: l.pulso,
    }))
}

#[derive(Deserialize)]
pub(crate) struct VistoBody {
    /// The newest notice the person has seen.
    hasta: i64,
}

/// `POST /api/vigilancia/visto`: the person has seen the notices up to this one.
pub(crate) async fn visto(
    State(state): State<Arc<AppState>>,
    _s: Session,
    Json(body): Json<VistoBody>,
) -> ApiResult<Value> {
    let hasta = with_ledger(&state, |l| {
        let antes = l
            .ajuste_vigilancia(VISTO)?
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0);
        let nuevo = antes.max(body.hasta.max(0));
        l.pon_ajuste_vigilancia(VISTO, &nuevo.to_string())?;
        Ok(nuevo)
    })?;
    Ok(Json(json!({ "visto": hasta })))
}

/// `POST /api/vigilancia/ahora`: look at the slow things now (the page's «Comprobar ahora»).
pub(crate) async fn ahora(State(state): State<Arc<AppState>>, _s: Session) -> ApiResult<Value> {
    state.vigilancia.pedido.store(true, Ordering::SeqCst);
    Ok(Json(json!({ "pedido": true })))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn suceso(id: i64, ts: i64, importancia: &str) -> Suceso {
        Suceso {
            id,
            ts,
            tipo: "arranque".into(),
            clave: format!("k{id}"),
            importancia: importancia.into(),
            texto: String::new(),
            datos: json!({ "frase": "vg_ev_arranque_nuevo", "datos": {} }),
            hash: String::new(),
        }
    }

    #[test]
    fn the_pulse_of_the_day_counts_from_local_midnight_by_hour() {
        // 18 Oct 2026, 15:30 in Bogotá (UTC-5).
        let zona = -300;
        let medianoche = local_midnight(local_day_index(1_792_355_400_000, zona), zona);
        let ahora = medianoche + 15 * HORA + 30 * MINUTO;
        let mut c = VecDeque::new();
        c.push_back((medianoche - HORA, "arranque"));
        c.push_back((medianoche + 8 * HORA, "arranque"));
        c.push_back((medianoche + 8 * HORA + 5, "proteccion"));
        let s = vec![
            suceso(3, medianoche + 8 * HORA + 12 * MINUTO, "aviso"),
            suceso(2, medianoche + 9 * HORA, "info"),
            suceso(1, medianoche - 10, "aviso"),
        ];
        let p = pulso(&c, &s, ahora, zona);
        assert_eq!(p.desde, medianoche);
        assert_eq!((p.comprobaciones, p.sucesos, p.avisos), (2, 2, 1));
        assert_eq!(p.horas.len(), 24);
        assert_eq!(p.horas[8], [2, 1, 1]);
        assert_eq!(p.horas[9], [0, 1, 0]);
    }

    #[test]
    fn notices_already_seen_or_older_than_a_week_do_not_count() {
        let ahora = 100 * DIA;
        let h = vec![
            suceso(5, ahora - HORA, "aviso"),
            suceso(4, ahora - 2 * HORA, "info"),
            suceso(3, ahora - 3 * HORA, "aviso"),
            suceso(2, ahora - 8 * DIA, "aviso"),
        ];
        assert_eq!(sin_ver(&h, 0, ahora).len(), 2);
        assert_eq!(sin_ver(&h, 3, ahora).len(), 1);
        assert!(sin_ver(&h, 5, ahora).is_empty());
    }

    #[test]
    fn the_fast_watch_counts_once_a_minute_and_the_pulse_forgets_after_a_day() {
        let mut e = Estado::default();
        cuenta_comprobacion(&mut e, 0, "camara_microfono");
        cuenta_comprobacion(&mut e, 15_000, "camara_microfono");
        cuenta_comprobacion(&mut e, 15_000, "arranque");
        cuenta_comprobacion(&mut e, MINUTO, "camara_microfono");
        assert_eq!(e.comprobaciones.len(), 3);
        cuenta_comprobacion(&mut e, 2 * DIA, "arranque");
        assert_eq!(e.comprobaciones.len(), 1);
    }

    fn mirada(ultima: i64) -> Mirada {
        Mirada {
            ultima: Some(ultima),
            disponible: true,
            ..Mirada::default()
        }
    }

    #[test]
    fn the_pill_says_attention_only_for_what_is_off_or_unseen() {
        let ahora = 10 * DIA;
        let mut e = Estado::default();
        let base = Base::default();
        let v = vistas(&e, &base, &base, &[], ahora);
        assert_eq!(estado_global(&v, false), "iniciando");
        assert!(v.iter().all(|x| x.estado == "esperando"));
        for id in VIGILANCIAS {
            e.ultimas.insert(id, mirada(ahora - 1_000));
        }
        // A program using the microphone is shown, not an alarm.
        e.usos = vec![Uso {
            programa: "Zoom.exe".into(),
            ruta: None,
            recurso: vg::Recurso::Microfono,
            desde: ahora - MINUTO,
            hasta: None,
            cuenta: "equipo".into(),
        }];
        let v = vistas(&e, &base, &base, &[], ahora);
        assert_eq!(v[0].estado, "en_uso");
        assert_eq!(estado_global(&v, false), "observando");
        assert_eq!(ultima_senal(&v), Some(ahora - 1_000));
        assert_eq!(estado_global(&v, true), "atencion", "an unseen notice");
        // Real-time protection off: attention, with its sentence.
        e.foto = Some(Foto {
            proteccion: Some(vg::Proteccion {
                antivirus: Some(vec![vg::Producto {
                    nombre: "Microsoft Defender Antivirus".into(),
                    activo: false,
                }]),
                cortafuegos_otros: Some(Vec::new()),
                defender: Some(vg::Defender {
                    servicio: true,
                    antivirus: true,
                    tiempo_real: false,
                    manipulacion: true,
                    modo: "Normal".into(),
                }),
                cortafuegos: Some(vec![("Public".into(), true)]),
            }),
            ..Foto::default()
        });
        let v = vistas(&e, &base, &base, &[], ahora);
        assert_eq!(v[3].estado, "atencion");
        assert_eq!(estado_global(&v, false), "atencion");
        assert_eq!(
            atenciones(e.foto.as_ref()),
            vec![Atencion {
                frase: "vg_atencion_tiempo_real",
                x: String::new()
            }]
        );
    }

    #[test]
    fn a_system_that_cannot_say_it_says_so() {
        let mut e = Estado::default();
        for id in VIGILANCIAS {
            e.ultimas.insert(
                id,
                Mirada {
                    disponible: false,
                    motivo: Some("no_mac"),
                    ..Mirada::default()
                },
            );
        }
        let base = Base::default();
        let v = vistas(&e, &base, &base, &[], 0);
        assert!(v.iter().all(|x| x.estado == "nd" && x.frase == "vg_no_mac"));
        assert_eq!(estado_global(&v, false), "no_disponible");
    }

    /// Every sentence the watch, the page and the pill can show exists in the three languages.
    #[test]
    fn every_sentence_of_the_watch_exists_in_the_three_languages() {
        let mut claves: Vec<String> = vg::FRASES.iter().map(|s| (*s).to_owned()).collect();
        claves.extend(vg::AJUSTES.iter().map(|a| format!("vg_aj_{a}")));
        claves.extend(vg::TIPOS_ARRANQUE.iter().map(|a| format!("vg_tipo_{a}")));
        claves.extend(VIGILANCIAS.iter().map(|a| format!("vg_check_{a}")));
        for k in [
            "vg_no_mac",
            "vg_no_sistema",
            "vg_esperando",
            "vg_cm_en_uso",
            "vg_cm_nadie",
            "vg_ar_nuevos",
            "vg_ar_ok",
            "vg_pr_cambios",
            "vg_pr_ok",
            "vg_pt_ok",
            "vg_pt_atencion",
            "vg_atencion_cortafuegos",
            "vg_atencion_tiempo_real",
            "vg_atencion_sin_antivirus",
            "vg_estado_observando",
            "vg_estado_atencion",
            "vg_estado_iniciando",
            "vg_estado_no_disponible",
            "vg_estado_ok",
            "vg_estado_en_uso",
            "vg_estado_aviso",
            "vg_estado_esperando",
            "vg_estado_nd",
            "vg_recurso_camara",
            "vg_recurso_microfono",
            "vg_rec_camara",
            "vg_rec_microfono",
            "vg_pt_clase_antivirus",
            "vg_pt_clase_tiempo_real",
            "vg_pt_clase_manipulacion",
            "vg_pt_clase_perfil",
            "vg_pt_clase_cortafuegos",
        ] {
            claves.push(k.to_owned());
        }
        for (nombre, json) in [
            ("es", guardiana_core::i18n::ES_JSON),
            ("en", guardiana_core::i18n::EN_JSON),
            ("pt", guardiana_core::i18n::PT_JSON),
        ] {
            let v: Value = serde_json::from_str(json).unwrap();
            let panel = v["panel"].as_object().unwrap();
            let faltan: Vec<&String> = claves.iter().filter(|k| !panel.contains_key(*k)).collect();
            assert!(faltan.is_empty(), "{nombre}: faltan {faltan:?}");
        }
    }
}
