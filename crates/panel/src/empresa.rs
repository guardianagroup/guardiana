//! A company's profile («ficha»), as GUARDIANA ZERO shows it since 1.0.10: what the lists say of
//! the company (seat, group, the addresses it uses), its checked profile when there is one (what
//! it does, its money, its fines and its data breaches, each fact with its public source), what
//! the devices of this house asked of it according to the extract, and the letter that asks it
//! to delete the person's data (the owner, 12 Oct 2026: «que quede todo perfecto, igual que
//! GUARDIANA ZERO»).
//!
//! Nothing here talks to the network: the profiles travel inside the program, the evidence is
//! the extract on this computer, and the sources open only when the person clicks them.

// Handlers return a full HTTP `Response` as their error, as axum intends (as in api.rs).
#![allow(clippy::result_large_err)]

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use guardiana_core::i18n::Texts;
use guardiana_core::SELF_DEVICE_ID;
use serde::{Deserialize, Serialize};

use crate::api::{with_ledger, ApiResult};
use crate::auth::{Lang, Session};
use crate::borrado::{
    corredores, evidencias, jur_view, Corredor, JurView, NombreVisto, JURISDICCIONES,
};
use crate::AppState;

/// The longest company name the page may ask for. The longest in the lists is under 60.
const LARGO_MAX: usize = 120;
/// Addresses of a company the page lists; the rest are counted.
const DOMINIOS_MAX: usize = 40;
/// Addresses the house asked for that the page lists; the rest are counted.
const NOMBRES_MAX: usize = 30;

/// The name the page asked for, trimmed, or `None` when it is empty or absurdly long.
pub(crate) fn nombre_pedido(e: &str) -> Option<String> {
    let e = e.trim();
    (!e.is_empty() && e.chars().count() <= LARGO_MAX).then(|| e.to_owned())
}

/// A name the lists do not know is a site: its addresses are itself and what hangs from it.
fn sitio_pedido(e: &str) -> Option<String> {
    let s = e.trim().trim_end_matches('.').to_ascii_lowercase();
    (s.contains('.') && !s.contains(' ') && !s.contains('/')).then_some(s)
}

/// The company of the deletion list that answers for `empresa`, if it is one of them: the same
/// section of `empresas.txt`, or one of its addresses.
fn de_la_lista(empresa: &str) -> Option<&'static Corredor> {
    let registro = guardiana_lists::company_record(empresa);
    corredores().iter().find(|c| match &registro {
        Some(r) => {
            c.empresas.iter().any(|x| x == r.name) || r.domains.iter().any(|d| c.coincide(d))
        }
        None => sitio_pedido(empresa).is_some_and(|s| c.coincide(&s)),
    })
}

/// Who the letter goes to: the company of the deletion list when it is one (it carries the
/// address it gives for these letters); otherwise the company as the lists and its profile know
/// it, with no address: the person writes the one on its website.
pub(crate) fn destinatario(empresa: &str) -> Option<Corredor> {
    let empresa = nombre_pedido(empresa)?;
    if let Some(c) = de_la_lista(&empresa) {
        return Some(c.clone());
    }
    let registro = guardiana_lists::company_record(&empresa);
    let sitio = sitio_pedido(&empresa);
    if registro.is_none() && sitio.is_none() {
        return None;
    }
    let borrar = guardiana_lists::profile_of(&empresa)
        .and_then(|f| f.get("borrar"))
        .and_then(|u| u.as_str())
        .unwrap_or_default();
    // A big company that is not a broker may have its privacy channel curated, with its source
    // (`derechos-contactos.txt`, 1.0.45): the letter goes to that form, not to a general page.
    let curado = registro
        .as_ref()
        .and_then(|r| crate::borrado::contacto_de(r.name));
    let url = curado
        .and_then(|c| c.url.clone())
        .or_else(|| borrar.starts_with("https://").then(|| borrar.to_owned()));
    // A profile may give the company's own address for these requests (`mailto:`): the letter
    // goes there, under every law (review of 12 Oct 2026: Cloudflare's was ignored).
    let correos = curado
        .filter(|c| !c.correos.is_empty())
        .map(|c| c.correos.clone());
    let correos = correos.unwrap_or_else(|| {
        borrar
            .strip_prefix("mailto:")
            .map(|c| c.split('?').next().unwrap_or_default().trim())
            .filter(|c| c.contains('@') && !c.contains([' ', ',', ';', '<', '>']))
            .map(|c| vec![(c.to_owned(), vec!["*".to_owned()])])
            .unwrap_or_default()
    });
    Some(Corredor {
        id: format!("empresa:{empresa}"),
        nombre: registro
            .as_ref()
            .map_or(empresa.clone(), |r| r.name.to_owned()),
        pais: registro.as_ref().and_then(|r| r.country.map(str::to_owned)),
        tipo: "publicidad".to_owned(),
        url,
        correos,
        dominios: if registro.is_some() {
            Vec::new()
        } else {
            sitio.into_iter().collect()
        },
        empresas: registro
            .map(|r| vec![r.name.to_owned()])
            .unwrap_or_default(),
        ..Corredor::default()
    })
}

#[derive(Deserialize)]
pub(crate) struct FichaQuery {
    e: String,
}

/// The company as a group or a broker shows it on the page.
#[derive(Serialize, Debug)]
pub(crate) struct CorredorFicha {
    nombre: &'static str,
    url: Option<&'static str>,
}

/// What the devices of this house asked of the company, from the extract.
#[derive(Serialize, Debug, Default)]
pub(crate) struct EnCasa {
    consultas: i64,
    cortadas: i64,
    primera: Option<i64>,
    ultima: Option<i64>,
    /// The addresses asked for, most asked first.
    nombres: Vec<NombreVisto>,
    n_nombres: usize,
    /// The devices that asked: (id, name when it has one), this computer first.
    aparatos: Vec<(String, Option<String>)>,
    /// Devices of the house that keep their detail to themselves and so do not count here.
    callados: usize,
}

#[derive(Serialize, Debug)]
pub(crate) struct FichaView {
    empresa: String,
    /// Whether a section of `empresas.txt` is this company.
    conocida: bool,
    /// The country of its seat, named in the panel's language; for a name the lists do not
    /// know, the country of its two-letter ending, with `pais_por_dominio` set.
    pais: Option<String>,
    pais_codigo: Option<String>,
    pais_por_dominio: bool,
    ciudad: Option<&'static str>,
    grupo: Option<&'static str>,
    /// Whether the group is itself a company of the lists, so the page can link to it.
    grupo_conocido: bool,
    /// What it does, in the one sentence of the panel's list, when there is one.
    oficio: Option<String>,
    dominios: Vec<&'static str>,
    n_dominios: usize,
    /// When it is in the State of California's register of data brokers.
    corredor: Option<CorredorFicha>,
    /// Its id on «Haz que te borren», when that page already has it with its address.
    borrado: Option<String>,
    ficha: Option<&'static serde_json::Value>,
    revisada: &'static str,
    casa: EnCasa,
    /// The places whose law the letter can cite, as «Haz que te borren» lists them: the page
    /// needs only this of that page's summary, which reads the whole statement again.
    jurisdicciones: Vec<JurView>,
}

fn no_encontrado() -> Response {
    (StatusCode::NOT_FOUND, "unknown company").into_response()
}

/// The profile of one company. A name nobody knows and that is not a site is a 404: there is
/// nothing true to say about it.
pub(crate) async fn ficha(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Query(q): Query<FichaQuery>,
) -> ApiResult<FichaView> {
    let empresa = nombre_pedido(&q.e).ok_or_else(no_encontrado)?;
    let registro = guardiana_lists::company_record(&empresa);
    let perfil = guardiana_lists::profile_of(&empresa);
    let quien = destinatario(&empresa).ok_or_else(no_encontrado)?;
    let (spans, devices) = with_ledger(&state, |l| Ok((l.name_device_spans()?, l.devices()?)))?;
    let nombres: HashMap<String, Option<String>> = devices
        .iter()
        .map(|d| (d.id.clone(), d.name.clone()))
        .collect();
    let callados = devices
        .iter()
        .filter(|d| d.id != SELF_DEVICE_ID && !d.share_detail_with_home)
        .count();
    Ok(Json(vista(
        t,
        &empresa,
        registro.as_ref(),
        perfil,
        &quien,
        &spans,
        &nombres,
        callados,
    )))
}

#[allow(clippy::too_many_arguments)]
fn vista(
    t: &Texts,
    empresa: &str,
    registro: Option<&guardiana_lists::CompanyRecord>,
    perfil: Option<&'static serde_json::Value>,
    quien: &Corredor,
    spans: &[guardiana_core::NameSpan],
    nombres_de: &HashMap<String, Option<String>>,
    callados: usize,
) -> FichaView {
    // The evidence counts the company's own names only, not those of a broker that shares it.
    let propio = Corredor {
        id: "ficha".to_owned(),
        dominios: if registro.is_some() {
            Vec::new()
        } else {
            sitio_pedido(empresa).into_iter().collect()
        },
        empresas: registro
            .map(|r| vec![r.name.to_owned()])
            .unwrap_or_default(),
        ..Corredor::default()
    };
    let ev = evidencias(spans, std::slice::from_ref(&propio))
        .remove("ficha")
        .unwrap_or_default();
    let n_nombres = ev.nombres.len();
    let casa = EnCasa {
        consultas: ev.consultas,
        cortadas: ev.cortadas,
        primera: ev.primera,
        ultima: ev.ultima,
        aparatos: ev
            .aparatos
            .iter()
            .map(|d| (d.clone(), nombres_de.get(d).cloned().flatten()))
            .collect(),
        nombres: ev.nombres.into_iter().take(NOMBRES_MAX).collect(),
        n_nombres,
        callados,
    };
    let (codigo, por_dominio) = match registro.and_then(|r| r.country) {
        Some(c) => (Some(c.to_owned()), false),
        None if registro.is_none() => (
            sitio_pedido(empresa).and_then(|s| guardiana_lists::dominio::pais_del_dominio(&s)),
            true,
        ),
        None => (None, false),
    };
    let pais = codigo
        .as_deref()
        .map(|c| t.pais(c))
        .filter(|n| !n.is_empty())
        .map(str::to_owned);
    let dominios: Vec<&'static str> = registro.map(|r| r.domains.clone()).unwrap_or_default();
    let corredor = dominios
        .iter()
        .find_map(|d| guardiana_lists::data_broker_of(d))
        .or_else(|| sitio_pedido(empresa).and_then(|s| guardiana_lists::data_broker_of(&s)))
        .map(|b| CorredorFicha {
            nombre: b.name,
            url: b.rights_url,
        });
    let grupo = registro.and_then(|r| r.group);
    FichaView {
        empresa: registro.map_or_else(|| empresa.to_owned(), |r| r.name.to_owned()),
        conocida: registro.is_some(),
        pais_por_dominio: por_dominio && pais.is_some(),
        pais_codigo: codigo.filter(|_| pais.is_some()),
        pais,
        ciudad: registro.and_then(|r| r.city),
        grupo_conocido: grupo.is_some_and(|g| guardiana_lists::company_record(g).is_some()),
        grupo,
        oficio: registro.and_then(|r| t.oficio(r.name)).map(str::to_owned),
        n_dominios: dominios.len(),
        dominios: dominios.into_iter().take(DOMINIOS_MAX).collect(),
        corredor,
        borrado: (!quien.id.starts_with("empresa:")).then(|| quien.id.clone()),
        ficha: perfil,
        revisada: guardiana_lists::profiles_checked(),
        casa,
        jurisdicciones: JURISDICCIONES.iter().map(|j| jur_view(t, j)).collect(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use guardiana_core::NameSpan;

    fn span(q: &str, dev: &str, n: i64, cut: i64, t0: i64) -> NameSpan {
        NameSpan {
            qname: q.to_owned(),
            device_id: dev.to_owned(),
            queries: n,
            cut,
            first_ts: t0,
            last_ts: t0 + 1000,
        }
    }

    /// Criteo's profile: its seat, its checked facts, and what the house asked of its names and
    /// of nobody else's.
    #[test]
    fn a_profile_says_what_the_house_asked_of_that_company_only() {
        let t = guardiana_core::i18n::by_code("es");
        let registro = guardiana_lists::company_record("Criteo").unwrap();
        let dominio = registro.domains[0];
        let spans = vec![
            span(&format!("static.{dominio}"), SELF_DEVICE_ID, 7, 7, 1_000),
            span(dominio, "mac:aa", 3, 0, 2_000),
            span("graph.facebook.com", SELF_DEVICE_ID, 50, 0, 3_000),
        ];
        let nombres = HashMap::from([("mac:aa".to_owned(), Some("Móvil".to_owned()))]);
        let quien = destinatario("Criteo").unwrap();
        let v = vista(
            t,
            "Criteo",
            Some(&registro),
            guardiana_lists::profile_of("Criteo"),
            &quien,
            &spans,
            &nombres,
            1,
        );
        assert!(v.conocida);
        assert_eq!(v.pais.as_deref(), Some("Francia"));
        assert!(!v.pais_por_dominio);
        assert!(v.ficha.is_some(), "Criteo tiene ficha desde 1.0.11");
        assert_eq!(v.casa.consultas, 10);
        assert_eq!(v.casa.cortadas, 7);
        assert_eq!(v.casa.n_nombres, 2);
        assert_eq!(v.casa.aparatos[0].0, SELF_DEVICE_ID);
        assert_eq!(v.casa.aparatos[1].1.as_deref(), Some("Móvil"));
        assert_eq!(v.casa.callados, 1);
    }

    /// A site no company of the lists owns: it is its own addresses, and its ending says only
    /// where the name was registered.
    #[test]
    fn an_unknown_site_shows_the_country_of_its_ending_said_as_such() {
        let t = guardiana_core::i18n::by_code("es");
        let quien = destinatario("rastreo.ejemplo-desconocido.cz").unwrap();
        let spans = vec![span(
            "a.rastreo.ejemplo-desconocido.cz",
            SELF_DEVICE_ID,
            2,
            2,
            5,
        )];
        let v = vista(
            t,
            "rastreo.ejemplo-desconocido.cz",
            None,
            None,
            &quien,
            &spans,
            &HashMap::new(),
            0,
        );
        assert!(!v.conocida);
        assert!(v.pais_por_dominio);
        assert_eq!(v.pais_codigo.as_deref(), Some("CZ"));
        assert!(!v.jurisdicciones.is_empty());
        assert_eq!(v.casa.consultas, 2);
        assert!(v.borrado.is_none());
        // A name that is neither a company nor a site has no profile at all.
        assert!(destinatario("Una empresa que no existe").is_none());
        assert!(destinatario(&"x".repeat(200)).is_none());
    }

    /// A company that is on «Haz que te borren» gets that letter, with the address it gives; any
    /// other one gets the same letter addressed to it, with its own deletion page when its
    /// profile has one.
    #[test]
    fn the_letter_goes_to_the_deletion_list_company_or_to_the_company_itself() {
        let lista = corredores()
            .iter()
            .find(|c| !c.empresas.is_empty() && !c.correos.is_empty())
            .unwrap();
        let d = destinatario(&lista.empresas[0]).unwrap();
        assert_eq!(d.id, lista.id);
        assert!(!d.correos.is_empty());
        let criteo = destinatario("Criteo").unwrap();
        assert_eq!(criteo.nombre, "Criteo");
        let borrar = guardiana_lists::profile_of("Criteo")
            .and_then(|f| f.get("borrar"))
            .and_then(|u| u.as_str());
        if de_la_lista("Criteo").is_none() {
            assert_eq!(criteo.url.as_deref(), borrar);
        }
        // An address given as `mailto:` in the profile is where the letter goes.
        if de_la_lista("Cloudflare").is_none() {
            let cf = destinatario("Cloudflare").unwrap();
            assert_eq!(cf.correo_para("es"), Some("sar@cloudflare.com"));
        }
    }
}
