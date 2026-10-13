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

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::State;
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
    /// The window, in days.
    dias: i64,
}

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

/// `/api/antivirus`: what each antivirus asked for this week, and every product's official record.
pub(crate) async fn vista(
    Zona(zona): Zona,
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<Vista> {
    const DIAS: i64 = 7;
    let now = now_ms();
    let (filas, devices) = with_ledger(&state, |l| {
        Ok((
            l.program_names(now - DIAS * 24 * HOUR_MS, now + 1, zona)?,
            l.devices()?,
        ))
    })?;
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
        dias: DIAS,
    }))
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
