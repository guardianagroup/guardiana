//! «GUARDIANA vigila a tu antivirus» (the owner, 13 Oct 2026: «alguien de código abierto debe
//! vigilar al vigilante, y hay muchas multas a antivirus»).
//!
//! What it shows, for each antivirus whose processes asked for names on this computer in the
//! last seven days:
//!
//! - every name its own processes asked for, how many times, how many at night and how many were
//!   cut;
//! - for each name, what the maker itself publishes it is for (updates, cloud scanning, samples,
//!   licence, telemetry, offers…), with the maker's page; or, when the maker documents nothing
//!   about it, that it does not, and whose name it is when the lists know (never more than that:
//!   an undocumented name is a question, not a verdict);
//! - who signed the process, as Windows said: a process named like an antivirus and signed by
//!   somebody else is not that antivirus.
//!
//! And for every product the list knows, installed or not, the official decisions of authorities
//! about it (fines, bans, warnings), each with the authority's own page.
//!
//! What it cannot see is said on the page: what the antivirus sends (Guardiana sees names, never
//! content), and anything it sends without asking the system for a name (its own resolver, or a
//! fixed address).

// The handler answers with the same error type as the rest of the API (see api.rs).
#![allow(clippy::result_large_err)]

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::State;
use axum::http::header;
use axum::response::IntoResponse;
use guardiana_core::time::{now_ms, HOUR_MS};
use guardiana_core::{Category, SELF_DEVICE_ID};
use guardiana_lists::antivirus::{self, Antivirus};
use serde::Serialize;

use crate::api::{with_ledger, ApiResult};
use crate::auth::{Session, Zona};
use crate::AppState;

/// How many names of one product are listed; the totals count all of them.
const MAX_DESTINOS: usize = 300;

#[derive(Serialize)]
pub(crate) struct Vista {
    /// One entry per (device, product) that asked for names in the window.
    vistos: Vec<Visto>,
    /// Devices that do not share their detail with the home panel: left out, and counted.
    ocultos: usize,
    /// Every product the list knows, with its official record.
    catalogo: Vec<Ficha>,
    /// What each antivirus on this computer sent and received, by country and destination
    /// (1.0.21). Only this computer: the traffic channel sees no other device.
    trafico: Vec<TraficoProducto>,
    /// How many hours of traffic there are (the plan's detail): the page says it.
    trafico_horas: i64,
    /// The latest minutes of antivirus traffic, newest first, for the live panel (1.0.22).
    vivo: Vec<VivoFila>,
    /// The window, in days.
    dias: i64,
}

/// One line of the live panel: a minute of an antivirus talking to a destination (1.0.22).
#[derive(Serialize)]
struct VivoFila {
    /// Start of the minute, Unix ms.
    minuto: i64,
    producto: &'static str,
    programa: String,
    /// The name asked for the address, or empty.
    destino: String,
    /// The network, or a mark (`@local`, `@anycast`).
    red: String,
    pais: String,
    subidos: u64,
    bajados: u64,
}

/// How many minutes of the live feed are returned.
const VIVO_MINUTOS: i64 = 20;
/// The most rows of the live feed.
const VIVO_MAX: usize = 120;

/// What one antivirus product sent and received in the window, measured on this computer.
#[derive(Serialize)]
struct TraficoProducto {
    producto: &'static str,
    /// Bytes its processes sent out, in all.
    subidos: u64,
    /// Bytes they received.
    bajados: u64,
    /// By country of the server, most bytes first.
    paises: Vec<PaisTrafico>,
    /// By destination (the name it asked, or its network when it asked none), most bytes first.
    destinos: Vec<DestinoTrafico>,
    /// Of the bytes sent, how many between midnight and six.
    subidos_noche: u64,
}

#[derive(Serialize)]
struct PaisTrafico {
    /// Two-letter country, or a mark (`@local`, `@anycast`) the page translates.
    pais: String,
    subidos: u64,
    bajados: u64,
}

#[derive(Serialize)]
struct DestinoTrafico {
    /// The name the program asked for the address, or empty when it asked none GUARDIANA saw.
    destino: String,
    /// The network that announces the address (`Amazon.com, Inc.`), or a mark.
    red: String,
    pais: String,
    subidos: u64,
    bajados: u64,
}

/// How many of a product's rows are listed per country and per destination.
const MAX_TRAFICO: usize = 50;

#[derive(Serialize)]
struct Visto {
    producto: &'static str,
    empresa: &'static str,
    pais: &'static str,
    device_id: String,
    device_name: Option<String>,
    /// Each process of the product that asked, with who signed it and how many times it asked.
    programas: Vec<Programa>,
    consultas: u64,
    nocturnas: u64,
    cortadas: u64,
    ultima: i64,
    /// Distinct names the maker documents / does not document.
    documentados: usize,
    no_documentados: usize,
    destinos: Vec<Destino>,
    /// How many names there were in all (the list stops at [`MAX_DESTINOS`]).
    destinos_total: usize,
}

#[derive(Serialize)]
struct Programa {
    nombre: String,
    firma: String,
    consultas: u64,
}

#[derive(Serialize)]
struct Destino {
    qname: String,
    /// What the maker says it is for, when it documents it.
    uso: Option<&'static str>,
    /// The maker's page that says so.
    fuente: Option<&'static str>,
    /// Whose name it is, when the lists know and it is not the maker's own documented name.
    empresa: Option<&'static str>,
    categoria: Category,
    consultas: u64,
    nocturnas: u64,
    cortadas: u64,
    ultima: i64,
}

#[derive(Serialize)]
struct Ficha {
    producto: &'static str,
    empresa: &'static str,
    pais: &'static str,
    programas: Vec<&'static str>,
    /// How many names the maker documents.
    dominios: usize,
    sanciones: Vec<Sancion>,
}

#[derive(Serialize)]
struct Sancion {
    fecha: &'static str,
    autoridad: &'static str,
    pais: &'static str,
    importe: &'static str,
    tipo: &'static str,
    clave: &'static str,
    fuente: &'static str,
}

fn ficha(a: &Antivirus) -> Ficha {
    Ficha {
        producto: a.nombre,
        empresa: a.empresa,
        pais: a.pais,
        programas: a.programas.clone(),
        dominios: a.dominios.len(),
        sanciones: a
            .sanciones
            .iter()
            .map(|s| Sancion {
                fecha: s.fecha,
                autoridad: s.autoridad,
                pais: s.pais,
                importe: s.importe,
                tipo: s.tipo,
                clave: s.clave,
                fuente: s.fuente,
            })
            .collect(),
    }
}

/// How much a name says about the person, for the order of the list: what nobody documents and
/// a list calls tracking or advertising first.
fn peso(d: &Destino) -> u8 {
    match (d.uso.is_some(), d.categoria) {
        (false, Category::Rastreador | Category::Publicidad) => 0,
        (false, Category::Telemetria) => 1,
        (false, _) => 2,
        (true, _) if d.uso == Some("publicidad") || d.uso == Some("telemetria") => 3,
        _ => 4,
    }
}

/// Work the page out from the ledger's rows.
fn calcula(
    filas: &[guardiana_core::ProgramName],
    nombres: &HashMap<String, Option<String>>,
    comparte: &dyn Fn(&str) -> bool,
) -> Vec<Visto> {
    type Clave = (String, &'static str);
    let mut por: HashMap<Clave, (Vec<&guardiana_core::ProgramName>, &'static Antivirus)> =
        HashMap::new();
    for f in filas {
        let Some(av) = antivirus::antivirus_de_programa(&f.program) else {
            continue;
        };
        if !comparte(&f.device_id) {
            continue;
        }
        por.entry((f.device_id.clone(), av.nombre))
            .or_insert_with(|| (Vec::new(), av))
            .0
            .push(f);
    }
    let mut vistos: Vec<Visto> = por
        .into_iter()
        .map(|((device_id, _), (filas, av))| {
            let mut programas: HashMap<(String, String), u64> = HashMap::new();
            let mut destinos: HashMap<&str, Destino> = HashMap::new();
            for f in &filas {
                *programas
                    .entry((f.program.clone(), f.signer.clone()))
                    .or_insert(0) += f.queries;
                let d = destinos.entry(f.qname.as_str()).or_insert_with(|| {
                    let doc = antivirus::uso_documentado(av, &f.qname);
                    Destino {
                        qname: f.qname.clone(),
                        uso: doc.map(|d| d.uso),
                        fuente: doc.map(|d| d.fuente),
                        empresa: if doc.is_some() {
                            None
                        } else {
                            // The company people know (Google), not its group (Alphabet).
                            guardiana_lists::company_of(&f.qname)
                                .or_else(|| guardiana_lists::group_of(&f.qname))
                        },
                        categoria: f.category,
                        consultas: 0,
                        nocturnas: 0,
                        cortadas: 0,
                        ultima: 0,
                    }
                });
                d.consultas += f.queries;
                d.nocturnas += f.night;
                d.cortadas += f.cut;
                d.ultima = d.ultima.max(f.last_ts);
            }
            let mut destinos: Vec<Destino> = destinos.into_values().collect();
            destinos.sort_by(|a, b| {
                peso(a)
                    .cmp(&peso(b))
                    .then(b.consultas.cmp(&a.consultas))
                    .then(a.qname.cmp(&b.qname))
            });
            let mut programas: Vec<Programa> = programas
                .into_iter()
                .map(|((nombre, firma), consultas)| Programa {
                    nombre,
                    firma,
                    consultas,
                })
                .collect();
            programas.sort_by(|a, b| b.consultas.cmp(&a.consultas).then(a.nombre.cmp(&b.nombre)));
            let destinos_total = destinos.len();
            let documentados = destinos.iter().filter(|d| d.uso.is_some()).count();
            let visto = Visto {
                producto: av.nombre,
                empresa: av.empresa,
                pais: av.pais,
                device_name: nombres.get(&device_id).cloned().flatten(),
                device_id,
                programas,
                consultas: destinos.iter().map(|d| d.consultas).sum(),
                nocturnas: destinos.iter().map(|d| d.nocturnas).sum(),
                cortadas: destinos.iter().map(|d| d.cortadas).sum(),
                ultima: destinos.iter().map(|d| d.ultima).max().unwrap_or(0),
                documentados,
                no_documentados: destinos_total - documentados,
                destinos_total,
                destinos: {
                    destinos.truncate(MAX_DESTINOS);
                    destinos
                },
            };
            visto
        })
        .collect();
    vistos.sort_by(|a, b| {
        b.consultas
            .cmp(&a.consultas)
            .then(a.producto.cmp(b.producto))
    });
    vistos
}

/// The night, in a program's traffic minute, by the person's own clock.
fn es_noche(minuto: i64, zona: i64) -> bool {
    let local = (minuto + zona * 60_000).rem_euclid(24 * 3_600_000);
    local < 6 * 3_600_000
}

/// Group the traffic measurements of this computer by antivirus product (1.0.21).
fn calcula_trafico(filas: &[guardiana_core::Trafico], zona: i64) -> Vec<TraficoProducto> {
    struct Acc {
        subidos: u64,
        bajados: u64,
        noche: u64,
        por_pais: HashMap<String, (u64, u64)>,
        por_destino: HashMap<(String, String, String), (u64, u64)>,
    }
    let mut por: HashMap<&'static str, Acc> = HashMap::new();
    for f in filas {
        let Some(av) = antivirus::antivirus_de_programa(&f.programa) else {
            continue;
        };
        let a = por.entry(av.nombre).or_insert_with(|| Acc {
            subidos: 0,
            bajados: 0,
            noche: 0,
            por_pais: HashMap::new(),
            por_destino: HashMap::new(),
        });
        a.subidos += f.enviados;
        a.bajados += f.recibidos;
        if es_noche(f.minuto, zona) {
            a.noche += f.enviados;
        }
        let pais = if f.pais.is_empty() {
            f.red.clone() // may be @local / @anycast; else empty
        } else {
            f.pais.clone()
        };
        let p = a.por_pais.entry(pais.clone()).or_insert((0, 0));
        p.0 += f.enviados;
        p.1 += f.recibidos;
        let d = a
            .por_destino
            .entry((f.destino.clone(), f.red.clone(), pais))
            .or_insert((0, 0));
        d.0 += f.enviados;
        d.1 += f.recibidos;
    }
    fn corta<K>(mut v: Vec<(K, (u64, u64))>) -> Vec<(K, (u64, u64))> {
        v.sort_by_key(|x| std::cmp::Reverse(x.1 .0 + x.1 .1));
        v.truncate(MAX_TRAFICO);
        v
    }
    let mut out: Vec<TraficoProducto> = por
        .into_iter()
        .map(|(producto, a)| TraficoProducto {
            producto,
            subidos: a.subidos,
            bajados: a.bajados,
            subidos_noche: a.noche,
            paises: corta(a.por_pais.into_iter().collect())
                .into_iter()
                .map(|(pais, (s, b))| PaisTrafico {
                    pais,
                    subidos: s,
                    bajados: b,
                })
                .collect(),
            destinos: corta(a.por_destino.into_iter().collect())
                .into_iter()
                .map(|((destino, red, pais), (s, b))| DestinoTrafico {
                    destino,
                    red,
                    pais,
                    subidos: s,
                    bajados: b,
                })
                .collect(),
        })
        .collect();
    out.sort_by_key(|p| std::cmp::Reverse(p.subidos + p.bajados));
    out
}

/// `/api/antivirus`: what each antivirus asked for this week, and every product's official record.
pub(crate) async fn vista(
    Zona(zona): Zona,
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<Vista> {
    const DIAS: i64 = 7;
    let now = now_ms();
    let (filas, devices, trafico, recientes) = with_ledger(&state, |l| {
        Ok((
            l.program_names(now - DIAS * 24 * HOUR_MS, now + 1, zona)?,
            l.devices()?,
            l.trafico(now - DIAS * 24 * HOUR_MS, now + 1, &[])?,
            l.trafico(now - VIVO_MINUTOS * 60_000, now + 1, &[])?,
        ))
    })?;
    let trafico_horas = trafico
        .iter()
        .map(|f| f.minuto)
        .min()
        .map_or(0, |m| (now - m) / HOUR_MS + 1);
    let trafico = calcula_trafico(&trafico, zona);
    // The live feed: the latest minutes, newest first, only of a known antivirus.
    let mut vivo: Vec<VivoFila> = recientes
        .into_iter()
        .filter_map(|f| {
            antivirus::antivirus_de_programa(&f.programa).map(|av| VivoFila {
                minuto: f.minuto,
                producto: av.nombre,
                programa: f.programa,
                destino: f.destino,
                red: f.red,
                pais: f.pais,
                subidos: f.enviados,
                bajados: f.recibidos,
            })
        })
        .collect();
    vivo.sort_by_key(|v| std::cmp::Reverse((v.minuto, v.subidos + v.bajados)));
    vivo.truncate(VIVO_MAX);
    let comparten: Vec<&str> = devices
        .iter()
        .filter(|d| d.id == SELF_DEVICE_ID || d.share_detail_with_home)
        .map(|d| d.id.as_str())
        .collect();
    let nombres: HashMap<String, Option<String>> = devices
        .iter()
        .map(|d| (d.id.clone(), d.name.clone()))
        .collect();
    let vistos = calcula(&filas, &nombres, &|id| {
        id == SELF_DEVICE_ID || comparten.contains(&id)
    });
    Ok(axum::Json(Vista {
        vistos,
        ocultos: devices.len() - comparten.len(),
        catalogo: antivirus::antivirus().iter().map(ficha).collect(),
        trafico,
        trafico_horas,
        vivo,
        dias: DIAS,
    }))
}

/// A plain local date-time `YYYY-MM-DD HH:MM` from a Unix-ms minute, for the CSV.
fn fecha_hora_local(ms: i64, zona_min: i64) -> String {
    let t = ms.div_euclid(1000) + zona_min * 60;
    let (dias, seg) = (t.div_euclid(86_400), t.rem_euclid(86_400));
    // Civil date from a day number (Howard Hinnant's algorithm), days since 1970-01-01.
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}",
        seg.div_euclid(3600),
        (seg.rem_euclid(3600)).div_euclid(60)
    )
}

/// `/api/antivirus/export`: all the antivirus traffic there is, as a CSV the person can keep or
/// show (1.0.22). One row per minute, program and destination; the address is never in it.
pub(crate) async fn export(
    Zona(zona): Zona,
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> Result<axum::response::Response, axum::response::Response> {
    use std::fmt::Write as _;
    let now = now_ms();
    let filas = with_ledger(&state, |l| l.trafico(0, now + 1, &[]))?;
    let mut csv = String::new();
    writeln!(
        csv,
        "minuto;fecha_hora;producto;programa;firma;destino;pais;red;asn;protocolo;puerto;bytes_enviados;bytes_recibidos;de_noche"
    )
    .ok();
    let celda = |s: &str| {
        if s.contains([';', '"', '\n']) {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_owned()
        }
    };
    for f in &filas {
        let Some(av) = antivirus::antivirus_de_programa(&f.programa) else {
            continue;
        };
        let resto = (f.minuto + zona * 60_000).rem_euclid(86_400_000);
        let noche = resto < 6 * 3_600_000;
        let fecha = fecha_hora_local(f.minuto, zona);
        let _ = writeln!(
            csv,
            "{};{};{};{};{};{};{};{};{};{};{};{};{};{}",
            f.minuto,
            celda(&fecha),
            celda(av.nombre),
            celda(&f.programa),
            celda(&f.firma),
            celda(&f.destino),
            celda(&f.pais),
            celda(&f.red),
            f.asn,
            celda(&f.proto),
            f.puerto,
            f.enviados,
            f.recibidos,
            if noche { "sí" } else { "no" },
        );
    }
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"guardiana-antivirus-trafico.csv\"".to_owned(),
            ),
        ],
        csv,
    )
        .into_response())
}

/// `/api/antivirus/detectar`: which antivirus Windows Security Center lists as installed, and
/// whether each is on and up to date (1.0.21, the «Analizar mi antivirus» button). Read-only.
pub(crate) async fn detectar(_s: Session) -> ApiResult<DeteccionView> {
    let (lista, aviso) = match guardiana_apps::antivirus_instalados() {
        Ok(l) => (l, None),
        Err(guardiana_apps::Error::NoSoportado) => (Vec::new(), Some("no_windows".to_owned())),
        Err(e) => (Vec::new(), Some(e.to_string())),
    };
    // Say which of the detected products GUARDIANA's list knows (so the page can link to it).
    let conocido = |n: &str| {
        antivirus::antivirus().iter().any(|a| {
            n.to_ascii_lowercase()
                .contains(&a.nombre.to_ascii_lowercase())
        })
    };
    let detectados: Vec<Detectado> = lista
        .into_iter()
        .map(|a| Detectado {
            conocido: conocido(&a.nombre),
            nombre: a.nombre,
            ruta: a.ruta,
            activo: a.activo,
            al_dia: a.al_dia,
        })
        .collect();
    Ok(axum::Json(DeteccionView { detectados, aviso }))
}

#[derive(Serialize)]
pub(crate) struct DeteccionView {
    detectados: Vec<Detectado>,
    /// Why nothing was detected, when that is the reason rather than «none installed».
    aviso: Option<String>,
}

#[derive(Serialize)]
struct Detectado {
    nombre: String,
    ruta: String,
    activo: bool,
    al_dia: bool,
    /// Whether GUARDIANA's list has this product (then the page offers its record).
    conocido: bool,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use guardiana_core::ProgramName;

    fn fila(programa: &str, firma: &str, qname: &str, n: u64, cat: Category) -> ProgramName {
        ProgramName {
            device_id: SELF_DEVICE_ID.into(),
            qname: qname.into(),
            program: programa.into(),
            signer: firma.into(),
            queries: n,
            night: n / 2,
            cut: 0,
            last_ts: 1_000 + i64::try_from(n).unwrap(),
            category: cat,
        }
    }

    #[test]
    fn the_csv_date_is_the_persons_local_day_and_minute() {
        // 1791900000000 ms = 2026-10-13 14:00 UTC; in UTC-5 it is 09:00 the same day.
        assert_eq!(
            super::fecha_hora_local(1_791_900_000_000, 0),
            "2026-10-13 14:00"
        );
        assert_eq!(
            super::fecha_hora_local(1_791_900_000_000, -300),
            "2026-10-13 09:00"
        );
    }

    #[test]
    fn each_antivirus_gets_its_names_with_what_its_maker_says_and_what_it_does_not() {
        let av = antivirus::antivirus_de_programa("MsMpEng.exe").unwrap();
        let documentado = av.dominios[0].dominio;
        let filas = vec![
            fila(
                "MsMpEng.exe",
                "Microsoft Windows Publisher",
                documentado,
                40,
                Category::Esperado,
            ),
            fila(
                "MsMpEng.exe",
                "Microsoft Windows Publisher",
                "doubleclick.net",
                2,
                Category::Publicidad,
            ),
            fila(
                "MpCmdRun.exe",
                "Microsoft Windows",
                documentado,
                4,
                Category::Esperado,
            ),
            fila(
                "chrome.exe",
                "Google LLC",
                "doubleclick.net",
                90,
                Category::Publicidad,
            ),
            {
                let mut f = fila("MsMpEng.exe", "", "a.example", 1, Category::Desconocido);
                f.device_id = "mac:otro".into();
                f
            },
        ];
        let v = calcula(&filas, &HashMap::new(), &|id| id == SELF_DEVICE_ID);
        // Chrome is no antivirus, and the device that does not share is left out.
        assert_eq!(v.len(), 1);
        let d = &v[0];
        assert_eq!(d.producto, "Microsoft Defender");
        assert_eq!(d.consultas, 46);
        assert_eq!((d.documentados, d.no_documentados), (1, 1));
        // What the maker does not document, and a list calls advertising, goes first.
        assert_eq!(d.destinos[0].qname, "doubleclick.net");
        assert!(d.destinos[0].uso.is_none());
        assert_eq!(d.destinos[0].empresa, Some("Google"));
        assert_eq!(d.destinos[1].uso, Some(av.dominios[0].uso));
        assert!(d.destinos[1].fuente.unwrap().starts_with("https://"));
        assert!(d.destinos[1].empresa.is_none());
        // Both processes, each with its signer.
        assert_eq!(d.programas.len(), 2);
        assert_eq!(d.programas[0].nombre, "MsMpEng.exe");
    }
    #[test]
    fn every_decision_use_and_kind_has_its_words_in_every_language() {
        for (lengua, texto) in [
            ("es", include_str!("../i18n/es.json")),
            ("en", include_str!("../i18n/en.json")),
            ("pt", include_str!("../i18n/pt.json")),
        ] {
            let v: serde_json::Value = serde_json::from_str(texto).unwrap();
            let panel = &v["panel"];
            let tiene = |k: &str| panel[k].as_str().is_some_and(|x| !x.is_empty());
            for u in antivirus::USOS {
                assert!(tiene(&format!("av_uso_{u}")), "{lengua}: av_uso_{u}");
            }
            for t in antivirus::TIPOS {
                assert!(tiene(&format!("av_tipo_{t}")), "{lengua}: av_tipo_{t}");
            }
            for a in antivirus::antivirus() {
                for s in &a.sanciones {
                    assert!(
                        tiene(&format!("av_sancion_{}", s.clave)),
                        "{lengua}: {}",
                        s.clave
                    );
                }
            }
        }
    }
}
