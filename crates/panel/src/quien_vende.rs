//! «Quién vende tus datos dentro de tu PC» (1.0.46, Diagnóstico): who, inside this computer,
//! talks to a company that buys or sells personal data, and the switches the system itself has
//! for handing the person's use to advertising.
//!
//! Everything is read here, on this computer, and nothing is sent:
//! - programs: the names each program asked for in the window (the ledger's own attribution),
//!   checked against the California registry of data brokers (`corredores.txt`) and the open
//!   list of documented cases (`quien-vende.json`);
//! - browser extensions: each manifest on disk (`extensiones.rs`), flagged when it may read every
//!   website (and so the person's chats with an AI) or when a case named it;
//! - the antivirus: what Windows Security Center lists, with the maker's documented case;
//! - Windows' advertising ID, optional diagnostic data and tailored experiences (`reg.exe`), and
//!   the Mac's personalised ads (read only);
//! - the computer maker's own programs (by their signer) and whom they talk to;
//! - Home Mode: TVs of the house that report what is on screen (ACR names);
//! - AI: which AI services this computer uses, by name, and the guide to keep each one from
//!   training on the person's chats.
//!
//! What GUARDIANA cannot see is said on the page: what a company does with the data once it has
//! it, and what happens inside a cloud account.
//!
//! The free level sees all of it (the history window is its 24 hours); the switches are the
//! person's own manual choices, as every manual cut is at the free level.

// Handlers answer with the same error type as the rest of the API (see api.rs).
#![allow(clippy::result_large_err)]

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use guardiana_apps::privacidad::{self, AjustePrivacidad};
use guardiana_core::time::{now_ms, DAY_MS};
use guardiana_core::{Action, MatchKind, Rule, SELF_DEVICE_ID};
use guardiana_lists::quien_vende::{self as lista, Caso};
use serde::{Deserialize, Serialize};

use crate::api::{with_ledger, ApiResult};
use crate::auth::Session;
use crate::extensiones;
use crate::AppState;

const HTML: &str = include_str!("../static/quien-vende.html");
const JS: &str = include_str!("../static/quien-vende.js");
const CSS: &str = include_str!("../static/lente-qv.css");

/// The routes of this page, merged into the panel's router.
pub(crate) fn rutas() -> Router<Arc<AppState>> {
    let pagina = || async { crate::pagina(HTML) };
    Router::new()
        .route("/quien-vende", get(pagina))
        .route("/quien-vende.html", get(pagina))
        .route(
            "/static/quien-vende.js",
            get(|| async { crate::static_response(JS, "application/javascript; charset=utf-8") }),
        )
        .route(
            "/static/lente-qv.css",
            get(|| async { crate::static_response(CSS, "text/css; charset=utf-8") }),
        )
        .route("/api/quien-vende", get(ver))
        .route("/api/quien-vende/ajuste", post(cambiar_ajuste))
}

/// A cut that covers a name: the rule, to undo it, and since when.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct Corte {
    id: i64,
    desde: i64,
}

#[derive(Serialize, Debug, Clone)]
pub(crate) struct DominioVista {
    dominio: String,
    consultas: u64,
    ultima: i64,
    corte: Option<Corte>,
}

/// A company a program talks to, and why it is on this page.
#[derive(Serialize, Debug, Clone)]
pub(crate) struct Comprador {
    empresa: String,
    /// `registro` (California's registry) or `caso` (a documented case).
    origen: &'static str,
    /// The case, for `caso`.
    caso: Option<String>,
    /// What the registrant ticked in the registry (letters, see `corredores.txt`).
    declara: String,
    pais: Option<String>,
    /// The page the registrant gave for deleting or not selling data.
    derechos: Option<String>,
    dominios: Vec<DominioVista>,
}

#[derive(Serialize, Debug, Clone)]
pub(crate) struct ProgramaVista {
    /// The program's file name, or empty when the system did not say which program asked.
    programa: String,
    firmante: String,
    consultas: u64,
    empresas: Vec<Comprador>,
}

#[derive(Serialize, Debug, Clone)]
pub(crate) struct ExtensionVista {
    #[serde(flatten)]
    ext: extensiones::Extension,
    /// It may read every website.
    todas_las_webs: bool,
    /// It may read the pages where people chat with an AI.
    chats_ia: bool,
    /// A documented case names it.
    caso: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
pub(crate) struct AntivirusVista {
    nombre: String,
    caso: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
pub(crate) struct FabricanteVista {
    marca: String,
    programa: String,
    firmante: String,
    consultas: u64,
    /// Whom it talks to: company (or the name itself), queries.
    destinos: Vec<(String, u64)>,
}

#[derive(Serialize, Debug, Clone)]
pub(crate) struct TvVista {
    empresa: String,
    dominio: String,
    caso: String,
    consultas: u64,
    /// Devices of the house that asked for it.
    aparatos: usize,
    /// The names of those whose owner shares their detail; the rest are only counted.
    nombres: Vec<String>,
}

#[derive(Serialize, Debug, Clone)]
pub(crate) struct IaVista {
    servicio: String,
    consultas: u64,
    ultima: i64,
    programas: Vec<String>,
    /// The guide's key (`qv_ia_guia_<key>`), when there is one.
    guia: Option<String>,
    caso: Option<String>,
}

#[derive(Serialize, Debug, Clone, Default)]
pub(crate) struct Resumen {
    /// Programs that talk to a data broker or a documented company.
    programas: usize,
    /// Companies of those.
    empresas: usize,
    /// Extensions that may read every website or that a case names.
    extensiones: usize,
    /// System settings that hand data to advertising and are on.
    ajustes_activos: usize,
    /// AI services this computer used.
    ia: usize,
}

#[derive(Serialize, Debug)]
pub(crate) struct Vista {
    desde: i64,
    hasta: i64,
    /// The oldest detail this computer's ledger holds, when later than `desde`.
    detalle_desde: Option<i64>,
    /// `windows`, `macos` or `otro`.
    sistema: &'static str,
    resumen: Resumen,
    programas: Vec<ProgramaVista>,
    extensiones: Vec<ExtensionVista>,
    /// Accounts whose browsers were looked at.
    cuentas_miradas: usize,
    antivirus: Vec<AntivirusVista>,
    /// Whether the system said which antivirus is installed (Windows only).
    antivirus_leido: bool,
    ajustes: Vec<AjustePrivacidad>,
    fabricante: Vec<FabricanteVista>,
    hogar_activo: bool,
    tv: Vec<TvVista>,
    ia: Vec<IaVista>,
    /// Every documented case of the list, for the page's «Casos documentados».
    casos: Vec<Caso>,
}

const fn sistema() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "otro"
    }
}

/// The active cut of `dominio` on this computer or the whole house, if any.
fn corte_de(reglas: &[Rule], dominio: &str) -> Option<Corte> {
    reglas
        .iter()
        .filter(|r| r.action == Action::Cortar)
        .filter(|r| r.device_id.as_deref().is_none_or(|d| d == SELF_DEVICE_ID))
        .find(|r| match r.match_kind {
            MatchKind::Domain => r.pattern == dominio,
            MatchKind::Suffix => {
                dominio == r.pattern
                    || dominio
                        .strip_suffix(r.pattern.as_str())
                        .is_some_and(|x| x.ends_with('.'))
            }
            MatchKind::Category => false,
        })
        .map(|r| Corte {
            id: r.id,
            desde: r.created_at,
        })
}

/// Who a name belongs to, for this page: the registry first (a fact the company declared), then
/// a documented case. Returned with no names (`dominios` empty).
fn comprador_de(qname: &str) -> Option<Comprador> {
    if let Some(b) = guardiana_lists::data_broker_of(qname) {
        return Some(Comprador {
            empresa: b.name.to_owned(),
            origen: "registro",
            caso: None,
            declara: b.declared.to_owned(),
            pais: b.country.map(str::to_owned),
            derechos: b.rights_url.map(str::to_owned),
            dominios: Vec::new(),
        });
    }
    lista::dominio_de(qname)
        .filter(|d| d.tipo != "acr")
        .map(|d| Comprador {
            empresa: d.empresa.clone(),
            origen: "caso",
            caso: Some(d.caso.clone()),
            declara: String::new(),
            pais: None,
            derechos: None,
            dominios: Vec::new(),
        })
}

/// Queries and last time, per name.
type PorDominio = BTreeMap<String, (u64, i64)>;
/// Per program, per company: the company and its names.
type PorPrograma = BTreeMap<String, BTreeMap<String, (Comprador, PorDominio)>>;
/// Per (program, signer): brand, queries, and queries per destination.
type PorFabricante = BTreeMap<(String, String), (String, u64, BTreeMap<String, u64>)>;
/// Per ACR name: company, case, queries, devices.
type PorTv = BTreeMap<String, (String, String, u64, BTreeSet<String>)>;

/// The programs of this computer that talk to a buyer or seller of data.
pub(crate) fn programas(
    grupos: &[guardiana_core::NameGroup],
    firmas: &HashMap<String, String>,
    reglas: &[Rule],
) -> Vec<ProgramaVista> {
    let mut por: PorPrograma = BTreeMap::new();
    for g in grupos.iter().filter(|g| g.device_id == SELF_DEVICE_ID) {
        let Some(c) = comprador_de(&g.qname) else {
            continue;
        };
        let programa = g.process.clone().unwrap_or_default();
        let e = por
            .entry(programa)
            .or_default()
            .entry(c.empresa.clone())
            .or_insert_with(|| (c, BTreeMap::new()));
        let d = e.1.entry(g.qname.clone()).or_insert((0, 0));
        d.0 += g.queries;
        d.1 = d.1.max(g.last_ts);
    }
    let mut out: Vec<ProgramaVista> = por
        .into_iter()
        .map(|(programa, empresas)| {
            let mut total = 0;
            let mut es: Vec<Comprador> = empresas
                .into_values()
                .map(|(mut c, ds)| {
                    c.dominios = ds
                        .into_iter()
                        .map(|(dominio, (consultas, ultima))| {
                            total += consultas;
                            DominioVista {
                                corte: corte_de(reglas, &dominio),
                                dominio,
                                consultas,
                                ultima,
                            }
                        })
                        .collect();
                    c.dominios.sort_by_key(|d| std::cmp::Reverse(d.consultas));
                    c
                })
                .collect();
            es.sort_by(|a, b| {
                let n = |c: &Comprador| c.dominios.iter().map(|d| d.consultas).sum::<u64>();
                n(b).cmp(&n(a)).then_with(|| a.empresa.cmp(&b.empresa))
            });
            ProgramaVista {
                firmante: firmas.get(&programa).cloned().unwrap_or_default(),
                programa,
                consultas: total,
                empresas: es,
            }
        })
        .collect();
    out.sort_by(|a, b| {
        b.consultas
            .cmp(&a.consultas)
            .then_with(|| a.programa.cmp(&b.programa))
    });
    out
}

/// An extension as the page shows it, with what it may read and any case naming it.
pub(crate) fn vista_extension(e: extensiones::Extension) -> ExtensionVista {
    let todas = e.patrones.iter().any(|p| lista::patron_todas_las_webs(p));
    let chats = e.patrones.iter().any(|p| lista::patron_chat_ia(p));
    let caso = lista::extension_conocida(&e.id, &e.nombre).map(|k| k.caso.clone());
    ExtensionVista {
        ext: e,
        todas_las_webs: todas,
        chats_ia: chats,
        caso,
    }
}

fn fabricante(grupos: &[guardiana_core::ProgramName]) -> Vec<FabricanteVista> {
    let mut por: PorFabricante = BTreeMap::new();
    for g in grupos.iter().filter(|g| g.device_id == SELF_DEVICE_ID) {
        let Some(m) = lista::fabricante_de(&g.signer) else {
            continue;
        };
        let e = por
            .entry((g.program.clone(), g.signer.clone()))
            .or_insert_with(|| (m.marca.clone(), 0, BTreeMap::new()));
        e.1 += g.queries;
        let quien =
            guardiana_lists::company_of(&g.qname).map_or_else(|| g.qname.clone(), str::to_owned);
        *e.2.entry(quien).or_default() += g.queries;
    }
    let mut out: Vec<FabricanteVista> = por
        .into_iter()
        .map(|((programa, firmante), (marca, consultas, d))| {
            let mut destinos: Vec<(String, u64)> = d.into_iter().collect();
            destinos.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            destinos.truncate(6);
            FabricanteVista {
                marca,
                programa,
                firmante,
                consultas,
                destinos,
            }
        })
        .collect();
    out.sort_by_key(|x| std::cmp::Reverse(x.consultas));
    out
}

fn tv(grupos: &[guardiana_core::NameGroup], devices: &[guardiana_core::Device]) -> Vec<TvVista> {
    let comparte: HashMap<&str, Option<&str>> = devices
        .iter()
        .map(|d| {
            let nombre = if d.share_detail_with_home {
                Some(d.name.as_deref().unwrap_or(d.id.as_str()))
            } else {
                None
            };
            (d.id.as_str(), nombre)
        })
        .collect();
    let mut por: PorTv = BTreeMap::new();
    for g in grupos.iter().filter(|g| g.device_id != SELF_DEVICE_ID) {
        let Some(d) = lista::dominio_de(&g.qname).filter(|d| d.tipo == "acr") else {
            continue;
        };
        let e = por
            .entry(d.dominio.clone())
            .or_insert_with(|| (d.empresa.clone(), d.caso.clone(), 0, BTreeSet::new()));
        e.2 += g.queries;
        e.3.insert(g.device_id.clone());
    }
    por.into_iter()
        .map(|(dominio, (empresa, caso, consultas, aparatos))| TvVista {
            nombres: aparatos
                .iter()
                .filter_map(|a| comparte.get(a.as_str()).copied().flatten())
                .map(str::to_owned)
                .collect(),
            aparatos: aparatos.len(),
            empresa,
            dominio,
            caso,
            consultas,
        })
        .collect()
}

fn ia(grupos: &[guardiana_core::NameGroup]) -> Vec<IaVista> {
    let mut por: BTreeMap<&'static str, (u64, i64, BTreeSet<String>)> = BTreeMap::new();
    for g in grupos.iter().filter(|g| g.device_id == SELF_DEVICE_ID) {
        let Some(s) = guardiana_lists::ai_service_of(&g.qname) else {
            continue;
        };
        let e = por.entry(s).or_default();
        e.0 += g.queries;
        e.1 = e.1.max(g.last_ts);
        if let Some(p) = &g.process {
            e.2.insert(p.clone());
        }
    }
    let mut out: Vec<IaVista> = por
        .into_iter()
        .map(|(servicio, (consultas, ultima, programas))| {
            let guia = lista::guia_ia(servicio);
            IaVista {
                servicio: servicio.to_owned(),
                consultas,
                ultima,
                programas: programas.into_iter().collect(),
                guia: guia.map(|g| g.guia.clone()),
                caso: guia.and_then(|g| g.caso.clone()),
            }
        })
        .collect();
    out.sort_by_key(|x| std::cmp::Reverse(x.consultas));
    out
}

/// What the system says about itself (slow: processes and files), away from the runtime.
struct Sistema {
    extensiones: Vec<extensiones::Extension>,
    cuentas: usize,
    antivirus: Option<Vec<String>>,
    ajustes: Vec<AjustePrivacidad>,
}

fn lee_sistema() -> Sistema {
    Sistema {
        cuentas: extensiones::hogares().len(),
        extensiones: extensiones::instaladas(),
        antivirus: guardiana_apps::antivirus_instalados()
            .ok()
            .map(|l| l.into_iter().map(|a| a.nombre).collect()),
        ajustes: privacidad::ajustes(),
    }
}

async fn ver(State(state): State<Arc<AppState>>, _s: Session) -> ApiResult<Vista> {
    let hasta = now_ms();
    let desde = crate::plan::desde(&state, hasta - 7 * DAY_MS, hasta);
    let (grupos, nombres_prog, reglas, devices, hogar, detalle) = with_ledger(&state, |l| {
        let g = l.names_grouped(desde, hasta + 1)?;
        let p = l.program_names(desde, hasta + 1, 0)?;
        let r = l.active_rules(hasta)?;
        let d = l.devices()?;
        let h = l
            .setting(guardiana_service::home::SETTING_HOME_MODE)?
            .is_some_and(|v| v == "1");
        let min: Option<i64> = l
            .connection()
            .query_row(
                "SELECT MIN(ts) FROM events WHERE device_id = ?1",
                [SELF_DEVICE_ID],
                |r| r.get(0),
            )
            .unwrap_or(None);
        Ok((g, p, r, d, h, min.filter(|m| *m > desde)))
    })?;
    let sis = tokio::task::spawn_blocking(lee_sistema)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())?;

    let firmas: HashMap<String, String> = nombres_prog
        .iter()
        .filter(|p| p.device_id == SELF_DEVICE_ID && !p.signer.is_empty())
        .map(|p| (p.program.clone(), p.signer.clone()))
        .collect();
    let programas = programas(&grupos, &firmas, &reglas);
    let mut extensiones: Vec<ExtensionVista> =
        sis.extensiones.into_iter().map(vista_extension).collect();
    // Named by a case first, then those that read every website, then the rest.
    extensiones.sort_by(|a, b| {
        (b.caso.is_some(), b.todas_las_webs, b.chats_ia)
            .cmp(&(a.caso.is_some(), a.todas_las_webs, a.chats_ia))
            .then_with(|| {
                a.ext
                    .nombre
                    .to_lowercase()
                    .cmp(&b.ext.nombre.to_lowercase())
            })
    });
    let antivirus: Vec<AntivirusVista> = sis
        .antivirus
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|nombre| AntivirusVista {
            caso: lista::antivirus_con_caso(&nombre).map(|a| a.caso.clone()),
            nombre,
        })
        .collect();
    let ia = ia(&grupos);
    let empresas: BTreeSet<&str> = programas
        .iter()
        .flat_map(|p| p.empresas.iter().map(|e| e.empresa.as_str()))
        .collect();
    let resumen = Resumen {
        programas: programas.len(),
        empresas: empresas.len(),
        extensiones: extensiones
            .iter()
            .filter(|e| e.caso.is_some() || e.todas_las_webs)
            .count(),
        ajustes_activos: sis
            .ajustes
            .iter()
            .filter(|a| a.activo == Some(true))
            .count(),
        ia: ia.len(),
    };
    Ok(Json(Vista {
        desde,
        hasta,
        detalle_desde: detalle,
        sistema: sistema(),
        resumen,
        fabricante: fabricante(&nombres_prog),
        tv: tv(&grupos, &devices),
        hogar_activo: hogar,
        programas,
        extensiones,
        cuentas_miradas: sis.cuentas,
        antivirus_leido: sis.antivirus.is_some(),
        antivirus,
        ajustes: sis.ajustes,
        ia,
        casos: lista::catalogo().casos.clone(),
    }))
}

#[derive(Deserialize)]
pub(crate) struct CambioAjuste {
    clave: String,
    activo: bool,
}

#[derive(Serialize)]
pub(crate) struct AjustesVista {
    ajustes: Vec<AjustePrivacidad>,
}

/// Switch one of the system's advertising settings, as the person asked on the page. A setting
/// an organisation fixed by policy is refused with `politica`.
async fn cambiar_ajuste(_s: Session, Json(c): Json<CambioAjuste>) -> ApiResult<AjustesVista> {
    if !matches!(
        c.clave.as_str(),
        "publicidad" | "experiencias" | "diagnostico"
    ) {
        return Err((StatusCode::BAD_REQUEST, "clave").into_response());
    }
    let r = tokio::task::spawn_blocking(move || {
        privacidad::cambia(&c.clave, c.activo).map(|()| privacidad::ajustes())
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())?;
    match r {
        Ok(ajustes) => Ok(Json(AjustesVista { ajustes })),
        Err(guardiana_apps::Error::NoSoportado) => {
            Err((StatusCode::NOT_IMPLEMENTED, "no_soportado").into_response())
        }
        Err(guardiana_apps::Error::Sistema(m)) => Err((StatusCode::CONFLICT, m).into_response()),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use guardiana_core::{NameGroup, Scope};

    fn g(dev: &str, q: &str, p: Option<&str>, n: u64) -> NameGroup {
        NameGroup {
            device_id: dev.to_owned(),
            qname: q.to_owned(),
            process: p.map(str::to_owned),
            queries: n,
            last_ts: 100,
        }
    }

    #[test]
    fn programs_are_grouped_by_company_and_a_cut_is_found() {
        let grupos = vec![
            g("self", "api.kochava.com", Some("juego.exe"), 5),
            g("self", "kochava.com", Some("juego.exe"), 2),
            g("self", "example.com", Some("juego.exe"), 50),
            g("tv", "api.kochava.com", None, 9),
        ];
        let reglas = vec![Rule {
            id: 7,
            scope: Scope::Device,
            device_id: Some("self".to_owned()),
            match_kind: MatchKind::Suffix,
            pattern: "kochava.com".to_owned(),
            action: Action::Cortar,
            created_at: 42,
            created_by: "usuario (panel)".to_owned(),
            expires_at: None,
            undone_at: None,
            confirmed: false,
        }];
        let p = programas(&grupos, &HashMap::new(), &reglas);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].programa, "juego.exe");
        assert_eq!(p[0].consultas, 7);
        let e = &p[0].empresas[0];
        assert_eq!(e.empresa, "Kochava");
        assert!(e
            .dominios
            .iter()
            .all(|d| d.corte == Some(Corte { id: 7, desde: 42 })));
    }

    #[test]
    fn a_tv_is_named_only_when_its_owner_shares_its_detail() {
        let grupos = vec![
            g("tv1", "acr-eu-prd.samsungacr.com", None, 30),
            g("tv2", "samsungacr.com", None, 3),
        ];
        let devs = vec![guardiana_core::Device {
            id: "tv1".to_owned(),
            mac: None,
            last_ip: None,
            name: Some("Salón".to_owned()),
            first_seen: 0,
            last_seen: 0,
            share_detail_with_home: false,
            hours_profile_json: None,
        }];
        let t = tv(&grupos, &devs);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].aparatos, 2);
        assert_eq!(t[0].consultas, 33);
        assert!(t[0].nombres.is_empty());
    }

    #[test]
    fn an_extension_reading_every_site_reads_ai_chats_too() {
        let e = vista_extension(extensiones::Extension {
            navegador: "Edge",
            id: "x".to_owned(),
            nombre: "1ClickVPN Proxy".to_owned(),
            version: "1".to_owned(),
            patrones: vec!["<all_urls>".to_owned()],
            permisos: Vec::new(),
        });
        assert!(e.todas_las_webs && e.chats_ia);
        assert_eq!(e.caso.as_deref(), Some("koi-urbanvpn-2025"));
    }
}
