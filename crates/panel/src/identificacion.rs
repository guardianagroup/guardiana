//! «¿Quién es y dónde está?» for every destination the pages show (1.0.45).
//!
//! The work is in `guardiana_lists::atribucion` and `guardiana_lists::ubicacion`; this module
//! turns their answers into what a page shows, in the panel's language, and serves the week's
//! «% identificado» (`/api/identificacion`). The owner, 18 Oct 2026: «la experiencia del cliente
//! debe ser sentir el monitoreo constante… para saber que todo está bien y nada está vigilando o
//! extrayendo sus datos». Feeling it starts by every row saying who, how sure, and why.
//!
//! The JSON a row carries (`identidad`, `ubicacion`), for whoever draws the inspector:
//!
//! - `identidad.nivel`: `identificada` | `probable` | `sin_identificar`; `identidad.nivel_texto`
//!   the same in words.
//! - `identidad.empresa`, `grupo`, `pais_empresa`, `ciudad_empresa`: the company behind the
//!   service and its seat (not the server's place).
//! - `identidad.bajo_nombre_de`: the site whose name the company answered under (an alias);
//!   `identidad.servicio_de`: another company's service a name with its own owner uses.
//! - `identidad.alojado_en` / `identidad.reparto`: the name is a rented server / a delivery name.
//! - `identidad.infraestructura`: `{operador, asn, tipo, empresa_operador, misma_empresa,
//!   prueba}`; `tipo` is `propia` | `mixta` | `nube` | `reparto` | `seguridad` | `alojamiento` |
//!   `acceso` | `desconocido`. `identidad.infraestructura_texto` in words.
//! - `identidad.operador_red`: what to show when nobody is identified.
//! - `identidad.pruebas[]`: `{fuente, detalle, fecha}`, the deciding one first; `fuente` is
//!   `base_guardiana` | `fabricante` | `registro_corredores` | `lista_ia` | `alias_dns` |
//!   `lista_redes` | `base_red` | `lista_alojamiento` | `lista_entrega`; `pruebas_texto[]` the same
//!   in words, one line each.
//! - `identidad.frase`: the one line for the row.
//! - `ubicacion.precision`: `ciudad` | `region` | `pais` | `repartido` | `local` | `sin_dato`;
//!   `evidencia`: `geofeed_operador` | `region_operador` | `base_geoip` | `anycast` | `ninguna`;
//!   `aprox` (write «aprox.»); `ciudad` (only with an operator's geofeed); `cerca_de` (the base's
//!   guess, always approximate); `region`, `codigo_region`, `pais`, `lat`, `lon`, `operador`,
//!   `fuente`, `fecha`; `texto` the line in words. There is never a radius.

// Handlers return a full HTTP `Response` as their error, as axum intends.
#![allow(clippy::result_large_err)]

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::Response;
use axum::Json;
use guardiana_core::i18n::Texts;
use guardiana_core::time::now_ms;
use guardiana_core::{AliasSites, ServerInfo};
use guardiana_lists::atribucion::{
    self, Atribucion, Cobertura, Fuente, Infraestructura, Nivel, Pistas, TipoRed,
};
use guardiana_lists::ubicacion::{self, Evidencia, Precision, Ubicacion};
use serde::Serialize;

use crate::api::{with_ledger, ApiResult};
use crate::auth::{Lang, Session};
use crate::AppState;

/// Who is behind a row, as the pages show it: the attribution with its words.
#[derive(Serialize, Clone, Debug)]
pub(crate) struct IdentidadView {
    #[serde(flatten)]
    pub(crate) atribucion: Atribucion,
    pub(crate) nivel_texto: String,
    pub(crate) frase: String,
    pub(crate) pruebas_texto: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) infraestructura_texto: Option<String>,
}

/// Where a row's server is, as the pages show it.
#[derive(Serialize, Clone, Debug)]
pub(crate) struct UbicacionView {
    #[serde(flatten)]
    pub(crate) ubicacion: Ubicacion,
    pub(crate) texto: String,
}

fn nivel_texto(t: &Texts, n: Nivel) -> String {
    t.panel(match n {
        Nivel::Identificada => "id_nivel_identificada",
        Nivel::Probable => "id_nivel_probable",
        Nivel::SinIdentificar => "id_nivel_sin_identificar",
    })
    .to_owned()
}

fn fuente_texto(t: &Texts, f: Fuente) -> &str {
    t.panel(match f {
        Fuente::BaseGuardiana => "id_fuente_base_guardiana",
        Fuente::Fabricante => "id_fuente_fabricante",
        Fuente::RegistroCorredores => "id_fuente_registro_corredores",
        Fuente::ListaIa => "id_fuente_lista_ia",
        Fuente::AliasDns => "id_fuente_alias_dns",
        Fuente::ListaRedes => "id_fuente_lista_redes",
        Fuente::BaseRed => "id_fuente_base_red",
        Fuente::ListaAlojamiento => "id_fuente_lista_alojamiento",
        Fuente::ListaEntrega => "id_fuente_lista_entrega",
    })
}

fn infra_texto(t: &Texts, i: &Infraestructura) -> String {
    let clave = if i.misma_empresa {
        "id_tipo_propia"
    } else {
        match i.tipo {
            TipoRed::Propia => "id_tipo_propia_de",
            TipoRed::Mixta => "id_tipo_mixta",
            TipoRed::Nube => "id_tipo_nube",
            TipoRed::Reparto => "id_tipo_reparto",
            TipoRed::Seguridad => "id_tipo_seguridad",
            TipoRed::Alojamiento => "id_tipo_alojamiento",
            TipoRed::Acceso => "id_tipo_acceso",
            TipoRed::Desconocido => "id_tipo_desconocido",
        }
    };
    let red = t.panel(clave).replace("{operador}", &i.operador);
    match i.asn {
        Some(n) => format!("{red} (AS{n})"),
        None => red,
    }
}

/// An attribution in the panel's words.
pub(crate) fn vista(t: &Texts, a: Atribucion) -> IdentidadView {
    let frase = match a.nivel {
        Nivel::Identificada => {
            let e = a.empresa.unwrap_or("");
            if let Some(s) = a.bajo_nombre_de.as_deref() {
                t.panel("id_frase_bajo_nombre")
                    .replace("{empresa}", e)
                    .replace("{sitio}", s)
            } else if let Some(s) = a.servicio_de {
                t.panel("id_frase_servicio")
                    .replace("{empresa}", e)
                    .replace("{servicio}", s)
            } else {
                e.to_owned()
            }
        }
        Nivel::Probable => t
            .panel("id_frase_probable")
            .replace("{empresa}", a.empresa.unwrap_or("")),
        Nivel::SinIdentificar => {
            if let Some(n) = a.alojado_en {
                t.panel("id_frase_alojado").replace("{nube}", n)
            } else if let Some(r) = a.reparto {
                t.panel("id_frase_reparto").replace("{red}", r)
            } else if let Some(o) = a.operador_red.as_deref() {
                t.panel("id_frase_sin").replace("{operador}", o)
            } else {
                t.panel("id_frase_sin_nada").to_owned()
            }
        }
    };
    let pruebas_texto = a
        .pruebas
        .iter()
        .map(|p| {
            t.panel("id_prueba")
                .replace("{fuente}", fuente_texto(t, p.fuente))
                .replace("{detalle}", &p.detalle)
                .replace("{fecha}", &p.fecha)
        })
        .collect();
    IdentidadView {
        nivel_texto: nivel_texto(t, a.nivel),
        infraestructura_texto: a.infraestructura.as_ref().map(|i| infra_texto(t, i)),
        frase,
        pruebas_texto,
        atribucion: a,
    }
}

/// Who is behind `qname`: with the alias sites kept for it when there are, else with the disguise
/// the row itself kept (decision 205), and the network that answered.
pub(crate) fn identidad(
    t: &Texts,
    qname: &str,
    alias: Option<&AliasSites>,
    cname_target: Option<&str>,
    asn: u32,
) -> IdentidadView {
    let propio: Vec<String>;
    let (sitios, visto): (&[String], Option<String>) = match alias {
        Some(a) => (
            a.sites.as_slice(),
            Some(guardiana_core::time::day_utc(a.last_seen)),
        ),
        None => {
            propio = cname_target.map(str::to_owned).into_iter().collect();
            (propio.as_slice(), None)
        }
    };
    vista(t, atribuida(qname, sitios, visto.as_deref(), asn))
}

/// The attribution of the same clues is the same answer: a receipt of a week reads one name tens
/// of thousands of times, so the answers are kept in memory, a bounded few, and forgotten whole
/// when there are too many. Names only, as everything else here.
fn atribuida(qname: &str, sitios: &[String], visto: Option<&str>, asn: u32) -> Atribucion {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<HashMap<String, Atribucion>>> =
        std::sync::OnceLock::new();
    const MAX: usize = 20_000;
    let clave = format!("{qname}|{asn}|{}|{}", sitios.join(" "), visto.unwrap_or(""));
    let cache = CACHE.get_or_init(Default::default);
    if let Some(a) = cache.lock().ok().and_then(|c| c.get(&clave).cloned()) {
        return a;
    }
    let a = atribucion::atribuir(&Pistas {
        qname,
        alias: sitios,
        alias_visto: visto,
        asn,
    });
    if let Ok(mut c) = cache.lock() {
        if c.len() >= MAX {
            c.clear();
        }
        c.insert(clave, a.clone());
    }
    a
}

fn pais_dicho(t: &Texts, codigo: Option<&str>) -> String {
    let Some(c) = codigo else {
        return String::new();
    };
    let n = t.pais(c);
    if n.is_empty() {
        c.to_owned()
    } else {
        n.to_owned()
    }
}

/// A place in the panel's words.
pub(crate) fn ubicacion_vista(t: &Texts, u: Ubicacion) -> UbicacionView {
    let pais = pais_dicho(t, u.pais.as_deref());
    let operador = u.operador.clone().unwrap_or_default();
    let mut texto = match (u.precision, u.evidencia) {
        (Precision::Ciudad, _) => t
            .panel("id_ub_ciudad")
            .replace("{ciudad}", u.ciudad.as_deref().unwrap_or(""))
            .replace("{pais}", &pais)
            .replace("{operador}", &operador),
        (Precision::Region, Evidencia::RegionOperador) => t
            .panel("id_ub_region_operador")
            .replace("{region}", u.region.as_deref().unwrap_or(""))
            .replace("{pais}", &pais)
            .replace("{operador}", &operador),
        (Precision::Region, _) => t
            .panel("id_ub_region_base")
            .replace("{region}", u.region.as_deref().unwrap_or(""))
            .replace("{pais}", &pais)
            .replace("{fuente}", &u.fuente),
        (Precision::Pais, _) => t
            .panel("id_ub_pais")
            .replace("{pais}", &pais)
            .replace("{fuente}", &u.fuente),
        (Precision::Repartido, _) => t.panel("id_ub_repartido").to_owned(),
        (Precision::Local, _) => t.panel("id_ub_local").to_owned(),
        (Precision::SinDato, _) => t.panel("id_ub_sin_dato").to_owned(),
    };
    if let Some(c) = u.cerca_de.as_deref() {
        texto = format!(
            "{texto} · {}",
            t.panel("id_ub_cerca").replace("{ciudad}", c)
        );
    }
    UbicacionView {
        ubicacion: u,
        texto,
    }
}

/// The place of a query's server as the row kept it (its country at most).
pub(crate) fn ubicacion_servidor(t: &Texts, s: Option<&ServerInfo>) -> Option<UbicacionView> {
    s.map(|s| ubicacion_vista(t, ubicacion::ubicacion_de_servidor(s)))
}

/// The alias sites of every name in `qnames`, read once.
pub(crate) fn alias_de(
    state: &AppState,
    qnames: Vec<String>,
) -> Result<HashMap<String, AliasSites>, Response> {
    if qnames.is_empty() {
        return Ok(HashMap::new());
    }
    with_ledger(state, move |l| {
        let refs: Vec<&str> = qnames.iter().map(String::as_str).collect();
        l.alias_sites_for(&refs)
    })
}

// ----- /api/identificacion --------------------------------------------------------------------

/// One name of the week that is not identified, or only probable.
#[derive(Serialize, Debug)]
pub(crate) struct NombreVista {
    qname: String,
    consultas: i64,
    identidad: IdentidadView,
}

/// The week's «% identificado».
#[derive(Serialize, Debug)]
pub(crate) struct IdentificacionView {
    /// From when to when, Unix ms.
    desde: i64,
    hasta: i64,
    cobertura: Cobertura,
    /// «78 % identificado».
    pct_texto: String,
    /// The names that are only probable, most queried first (at most 30).
    probables: Vec<NombreVista>,
    /// The names nobody identified, most queried first (at most 30), each with its network.
    sin_identificar: Vec<NombreVista>,
    /// Which source decided how many queries, by `Fuente` in snake case.
    por_fuente: HashMap<String, i64>,
    /// The editions behind every answer.
    ediciones: Ediciones,
}

/// The dates of the data the answers come from.
#[derive(Serialize, Debug)]
pub(crate) struct Ediciones {
    empresas_mundo: String,
    redes: String,
    listas: &'static str,
    base_red: String,
    base_red_fuente: String,
}

fn ediciones() -> Ediciones {
    let f = guardiana_lists::fuente_red();
    Ediciones {
        empresas_mundo: atribucion::edicion_de(guardiana_lists::EMPRESAS_MUNDO)
            .unwrap_or(atribucion::EDICION_LISTAS)
            .to_owned(),
        redes: atribucion::edicion_de(guardiana_lists::REDES)
            .unwrap_or(atribucion::EDICION_LISTAS)
            .to_owned(),
        listas: atribucion::EDICION_LISTAS,
        base_red: f.fecha.clone(),
        base_red_fuente: f.fuente.clone(),
    }
}

fn fuente_clave(f: Fuente) -> String {
    serde_json::to_value(f)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// The coverage of a set of (name, network, queries) rows, with the alias sites known for them.
pub(crate) fn calcula(
    t: &Texts,
    filas: &[guardiana_core::NameNetwork],
    alias: &HashMap<String, AliasSites>,
    desde: i64,
    hasta: i64,
) -> IdentificacionView {
    let mut atribs: Vec<(&str, i64, IdentidadView)> = Vec::with_capacity(filas.len());
    for f in filas {
        let v = identidad(t, &f.qname, alias.get(&f.qname), None, f.asn);
        atribs.push((f.qname.as_str(), f.queries, v));
    }
    let cobertura = atribucion::cobertura(atribs.iter().map(|(_, n, v)| (&v.atribucion, *n)));
    let mut por_fuente: HashMap<String, i64> = HashMap::new();
    // One name can come with several networks: the lists below show each name once, with the
    // queries of all its rows at the level of its most-queried row.
    let mut probables: HashMap<&str, (i64, IdentidadView)> = HashMap::new();
    let mut sin: HashMap<&str, (i64, IdentidadView)> = HashMap::new();
    for (q, n, v) in &atribs {
        if v.atribucion.local {
            continue;
        }
        if let Some(p) = v.atribucion.pruebas.first() {
            *por_fuente.entry(fuente_clave(p.fuente)).or_default() += n;
        }
        let destino = match v.atribucion.nivel {
            Nivel::Probable => &mut probables,
            Nivel::SinIdentificar => &mut sin,
            Nivel::Identificada => continue,
        };
        destino
            .entry(q)
            .and_modify(|(m, _)| *m += n)
            .or_insert((*n, v.clone()));
    }
    let lista = |m: HashMap<&str, (i64, IdentidadView)>| {
        let mut v: Vec<NombreVista> = m
            .into_iter()
            .map(|(q, (n, identidad))| NombreVista {
                qname: q.to_owned(),
                consultas: n,
                identidad,
            })
            .collect();
        v.sort_by(|a, b| b.consultas.cmp(&a.consultas).then(a.qname.cmp(&b.qname)));
        v.truncate(30);
        v
    };
    IdentificacionView {
        desde,
        hasta,
        pct_texto: t
            .panel("id_pct")
            .replace("{pct}", &cobertura.pct_identificada.to_string()),
        cobertura,
        probables: lista(probables),
        sin_identificar: lista(sin),
        por_fuente,
        ediciones: ediciones(),
    }
}

/// `GET /api/identificacion?dias=7`: how much of what left the house in the window GUARDIANA can
/// say who is behind, and what it cannot.
pub(crate) async fn api(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<IdentificacionView> {
    let dias: i64 = q
        .get("dias")
        .and_then(|d| d.parse().ok())
        .unwrap_or(7)
        .clamp(1, 30);
    let hasta = now_ms();
    // The last 24 hours at the free level, whatever was asked (display only: nothing is pruned).
    let desde = crate::plan::desde(&state, hasta - dias * 24 * 3_600_000, hasta);
    let (filas, alias) = with_ledger(&state, move |l| {
        let filas = l.destinations_by_network(desde, hasta)?;
        let mut nombres: Vec<&str> = filas.iter().map(|f| f.qname.as_str()).collect();
        nombres.dedup();
        let alias = l.alias_sites_for(&nombres)?;
        Ok((filas, alias))
    })?;
    Ok(Json(calcula(t, &filas, &alias, desde, hasta)))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use guardiana_core::i18n;

    #[test]
    fn a_row_says_who_how_sure_and_why() {
        let t = i18n::es();
        let v = identidad(t, "graph.facebook.com", None, None, 32934);
        assert_eq!(v.nivel_texto, t.panel("id_nivel_identificada"));
        assert_eq!(v.frase, "Meta");
        assert!(v.pruebas_texto[0].contains("facebook.com"), "{v:?}");
        assert_eq!(
            v.infraestructura_texto.as_deref(),
            Some(format!("{} (AS32934)", t.panel("id_tipo_propia")).as_str())
        );
        // The JSON carries the fields the inspector reads.
        let j = serde_json::to_value(&v).unwrap();
        for k in [
            "nivel",
            "empresa",
            "pruebas",
            "infraestructura",
            "frase",
            "nivel_texto",
        ] {
            assert!(j.get(k).is_some(), "{k}: {j}");
        }
        assert_eq!(j["nivel"], "identificada");
        assert_eq!(j["pruebas"][0]["fuente"], "base_guardiana");
        assert_eq!(j["infraestructura"]["tipo"], "propia");
    }

    #[test]
    fn a_cloud_is_said_apart_from_the_company() {
        let t = i18n::es();
        let v = identidad(t, "api.algo-desconocido.example", None, None, 16509);
        assert_eq!(v.atribucion.nivel, Nivel::SinIdentificar);
        assert!(v.atribucion.empresa.is_none());
        assert!(v.frase.contains("Amazon Web Services"), "{}", v.frase);
        assert!(v
            .infraestructura_texto
            .unwrap()
            .contains("Amazon Web Services"));
    }

    #[test]
    fn the_rows_disguise_counts_as_an_alias_when_no_chain_was_kept() {
        let t = i18n::es();
        let v = identidad(t, "metricas.tienda-x.com", None, Some("omtrdc.net"), 0);
        assert_eq!(v.atribucion.empresa, Some("Adobe"));
        assert!(v.frase.contains("tienda-x.com"), "{}", v.frase);
        let a = AliasSites {
            sites: vec!["omtrdc.net".into()],
            last_seen: 1_792_000_000_000,
        };
        let v = identidad(t, "metricas.tienda-x.com", Some(&a), None, 0);
        assert_eq!(
            v.atribucion.pruebas[0].fecha,
            guardiana_core::time::day_utc(1_792_000_000_000)
        );
    }

    #[test]
    fn places_are_said_with_their_evidence() {
        let t = i18n::es();
        let u = ubicacion_vista(t, ubicacion::ubicacion_de_clave("aws:us-east-1", "US"));
        assert!(u.texto.contains("Amazon Web Services"), "{}", u.texto);
        let u = ubicacion_vista(t, ubicacion::ubicacion_de_clave("", "DE"));
        assert!(u.texto.contains("Alemania"), "{}", u.texto);
        assert!(u.ubicacion.aprox);
        let j = serde_json::to_value(&u).unwrap();
        assert_eq!(j["precision"], "pais");
        assert_eq!(j["evidencia"], "base_geoip");
        assert_eq!(j["aprox"], true);
    }

    #[test]
    fn the_week_adds_up_to_a_percentage() {
        let t = i18n::es();
        let fila = |q: &str, asn: u32, n: i64| guardiana_core::NameNetwork {
            qname: q.into(),
            asn,
            queries: n,
        };
        let filas = vec![
            fila("graph.facebook.com", 32934, 6),
            fila("www.google.com", 15169, 2),
            fila("nadie-sabe.example", 16509, 1),
            fila("nadie-sabe.example", 0, 1),
            fila("1.1.168.192.in-addr.arpa", 0, 9),
        ];
        let v = calcula(t, &filas, &HashMap::new(), 0, 1);
        assert_eq!(v.cobertura.consultas, 10);
        assert_eq!(v.cobertura.identificada, 8);
        assert_eq!(v.cobertura.pct_identificada, 80);
        assert_eq!(v.sin_identificar.len(), 1);
        assert_eq!(v.sin_identificar[0].consultas, 2);
        assert!(v.pct_texto.contains("80"), "{}", v.pct_texto);
        assert_eq!(v.por_fuente.get("base_guardiana"), Some(&8));
    }
}
