//! «Tu derecho con un clic» (1.0.45): the companies this computer contacted in a period,
//! according to its «Certificado de flujos», and for each one the letter that asks it what data
//! it holds, or that it delete it, under the law of the person's place, addressed to the privacy
//! channel the company publishes, with the extract of the certificate that concerns it.
//!
//! The same letters, laws, deadlines and files as «Haz que te borren» and «Expedientes»: this
//! module only writes a different evidence paragraph, from the certificate, and the page opens
//! the file in «Expedientes» when the person says they sent it.
//!
//! What it never does: send anything (the person sends it from their own mail, or pastes it in
//! the company's form), and claim more than was measured. The paragraph says «this computer
//! contacted your services on these dates», never that personal data was sent: GUARDIANA sees
//! which names were asked for, not what travelled.

// Handlers return a full HTTP `Response` as their error, as axum intends (as in api.rs).
#![allow(clippy::result_large_err)]

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use guardiana_core::i18n::{self, Texts};
use guardiana_core::time::now_ms;
use guardiana_verify::certificado::{self as cert, Contenido, FilaEmpresa};
use guardiana_verify::certificado_html::rangos_de_dias;
use serde::{Deserialize, Serialize};

use crate::api::{with_ledger, ApiResult};
use crate::auth::{Lang, Session, Zona};
use crate::borrado::{
    self, carta_con, contacto_de, dia_de, fecha_iso, fecha_texto, jur_view, jurisdiccion,
    parse_fecha, Carta, Corredor, JurView, Tipo,
};
use crate::certificado::{archivos, meses, opciones, Archivos, PeriodoQuery};
use crate::expedientes::{self, ExpedienteView};
use crate::AppState;

/// At most this many stretches of days are written in the letter; the rest are counted.
const RANGOS_MAX: usize = 10;

/// The companies of a certificate that were contacted: at least one query answered with an
/// address. A company whose every query was cut was never reached through GUARDIANA's answers,
/// so no letter says it was.
pub(crate) fn con_contacto(k: &Contenido) -> impl Iterator<Item = &FilaEmpresa> {
    k.empresas
        .iter()
        .filter(|e| e.recuento.respondidas > 0 && !e.dias.is_empty())
}

/// The evidence paragraph of the letter, from the certificate: the days this computer
/// contacted the company, how many queries were answered, and the seal of the extract that goes
/// with it.
pub(crate) fn evidencia(t: &Texts, e: &FilaEmpresa, sello: &str) -> String {
    let rangos = rangos_de_dias(&e.dias);
    let dia = |s: &str| parse_fecha(s).map_or_else(|| s.to_owned(), |d| fecha_texto(t, d));
    let mut lineas: Vec<String> = rangos
        .iter()
        .take(RANGOS_MAX)
        .map(|(a, z)| {
            if a == z {
                t.panel("der_carta_linea_dia").replace("{dia}", &dia(a))
            } else {
                t.panel("der_carta_linea")
                    .replace("{desde}", &dia(a))
                    .replace("{hasta}", &dia(z))
            }
        })
        .collect();
    if rangos.len() > RANGOS_MAX {
        let resto: i64 = rangos[RANGOS_MAX..]
            .iter()
            .filter_map(|(a, z)| Some(parse_fecha(z)? - parse_fecha(a)? + 1))
            .sum();
        lineas.push(t.panel_n("der_carta_mas_dias", resto));
    }
    let veces = i64::try_from(e.recuento.respondidas).unwrap_or(i64::MAX);
    let dias = i64::try_from(e.dias.len()).unwrap_or(i64::MAX);
    t.panel("der_carta_evidencia")
        .replace("{lineas}", &lineas.join("\n"))
        .replace("{veces}", &t.panel_n("der_carta_veces", veces))
        .replace("{dias}", &t.panel_n("der_carta_dias", dias))
        .replace("{sello}", sello)
}

/// Who the letter goes to: the company's curated privacy channel, its profile's, or none (the
/// person writes the one on its website).
fn destinatario(empresa: &str) -> Corredor {
    crate::empresa::destinatario(empresa).unwrap_or_else(|| Corredor {
        id: format!("empresa:{empresa}"),
        nombre: empresa.to_owned(),
        ..Corredor::default()
    })
}

/// One company the page lists.
#[derive(Serialize, Debug)]
pub(crate) struct EmpresaDerecho {
    empresa: String,
    pais_sede: Option<String>,
    pais_nombre: Option<String>,
    /// Queries answered with an address: the contacts.
    contactos: u64,
    cortadas: u64,
    dias: usize,
    primer_dia: Option<String>,
    ultimo_dia: Option<String>,
    /// Stretches of consecutive days, `AAAA-MM-DD`.
    rangos: Vec<(String, String)>,
    /// The company's page or form for these requests, when known.
    url: Option<String>,
    /// The addresses it publishes, with the places each one is for (`*` for all).
    correos: Vec<(String, Vec<String>)>,
    /// Its privacy channel is one GUARDIANA checked for this page, with its source.
    curado: bool,
    fuentes: Vec<(String, String)>,
    /// The latest file of «Expedientes» with this company, when there is one.
    expediente: Option<ExpedienteView>,
}

#[derive(Serialize, Debug)]
pub(crate) struct DerechosView {
    desde_dia: String,
    hasta_dia: String,
    empresas: Vec<EmpresaDerecho>,
    /// Companies asked for whose every query was cut: never reached, so no letter.
    solo_cortadas: Vec<String>,
    jurisdicciones: Vec<JurView>,
    meses: Vec<String>,
    mes_anterior: String,
    cobertura_completa: bool,
    comprobada: bool,
    hoy: String,
}

pub(crate) async fn resumen(
    Lang(t): Lang,
    Zona(zona): Zona,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Query(q): Query<PeriodoQuery>,
) -> ApiResult<DerechosView> {
    let ahora = now_ms();
    let hoy = dia_de(ahora, zona);
    let mut op = opciones(&q, zona, ahora)?;
    op.dominios = false;
    op.solo_empresa = None;
    let (c, lista) = with_ledger(&state, |l| {
        Ok((cert::generar(l, &op, ahora)?, expedientes::leer(l)?))
    })?;
    let empresas = con_contacto(&c.contenido)
        .map(|e| {
            let quien = destinatario(&e.empresa);
            let expediente = lista
                .iter()
                .rev()
                .find(|x| x.empresa == quien.nombre || x.empresa == e.empresa)
                .map(|x| expedientes::vista(t, x, hoy));
            let curado = contacto_de(&e.empresa);
            EmpresaDerecho {
                empresa: e.empresa.clone(),
                pais_nombre: e
                    .pais_sede
                    .as_deref()
                    .map(|p| t.pais(p).to_owned())
                    .filter(|n| !n.is_empty()),
                pais_sede: e.pais_sede.clone(),
                contactos: e.recuento.respondidas,
                cortadas: e.recuento.cortadas,
                dias: e.dias.len(),
                primer_dia: e.dias.first().cloned(),
                ultimo_dia: e.dias.last().cloned(),
                rangos: rangos_de_dias(&e.dias),
                url: quien.url.clone(),
                correos: quien.correos.clone(),
                curado: curado.is_some(),
                fuentes: curado.map(|c| c.fuentes.clone()).unwrap_or_default(),
                expediente,
            }
        })
        .collect();
    let solo_cortadas = c
        .contenido
        .empresas
        .iter()
        .filter(|e| e.recuento.respondidas == 0)
        .map(|e| e.empresa.clone())
        .collect();
    Ok(Json(DerechosView {
        desde_dia: c.desde_dia.clone(),
        hasta_dia: c.hasta_dia.clone(),
        empresas,
        solo_cortadas,
        jurisdicciones: borrado::JURISDICCIONES
            .iter()
            .map(|j| jur_view(t, j))
            .collect(),
        meses: meses(ahora, zona),
        mes_anterior: cert::mes_anterior(ahora, zona),
        cobertura_completa: c.cobertura.completa,
        comprobada: c.cadena.comprobada,
        hoy: fecha_iso(hoy),
    }))
}

/// What the page asks a letter for. Anything else is refused.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub(crate) struct CartaQuery {
    empresa: String,
    jurisdiccion: String,
    #[serde(default)]
    tipo: Tipo,
    /// The letter's language; the place's own when missing.
    #[serde(default)]
    idioma: Option<String>,
    #[serde(default)]
    mes: Option<String>,
    #[serde(default)]
    desde: Option<String>,
    #[serde(default)]
    hasta: Option<String>,
    /// `1` to list the company's names in the extract.
    #[serde(default)]
    dominios: Option<String>,
}

/// A letter and the extract of the certificate that goes with it.
#[derive(Serialize, Debug)]
pub(crate) struct CartaDerecho {
    carta: Carta,
    extracto: Archivos,
}

fn no_encontrado(que: &str) -> Response {
    (StatusCode::NOT_FOUND, format!("unknown {que}")).into_response()
}

pub(crate) async fn carta(
    Zona(zona): Zona,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Query(q): Query<CartaQuery>,
) -> ApiResult<CartaDerecho> {
    let ahora = now_ms();
    let hoy = dia_de(ahora, zona);
    let j = jurisdiccion(&q.jurisdiccion).ok_or_else(|| no_encontrado("jurisdiction"))?;
    let t = i18n::by_code(q.idioma.as_deref().unwrap_or(j.idioma));
    let empresa =
        crate::empresa::nombre_pedido(&q.empresa).ok_or_else(|| no_encontrado("company"))?;
    let periodo = PeriodoQuery {
        mes: q.mes.clone(),
        desde: q.desde.clone(),
        hasta: q.hasta.clone(),
        dominios: q.dominios.clone(),
        empresa: Some(empresa.clone()),
    };
    let op = opciones(&periodo, zona, ahora)?;
    let c = with_ledger(&state, |l| cert::generar(l, &op, ahora))?;
    let fila = con_contacto(&c.contenido)
        .find(|e| e.empresa == empresa)
        .ok_or_else(|| no_encontrado("contact in this period"))?;
    let extracto = archivos(t, &c);
    let texto = evidencia(t, fila, &extracto.sello);
    let quien = destinatario(&empresa);
    Ok(Json(CartaDerecho {
        carta: carta_con(t, j, &quien, &texto, hoy, q.tipo),
        extracto,
    }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::borrado::{parse_corredores, MARCA_CORREO, MARCA_NOMBRE};
    use guardiana_verify::certificado::Recuento;
    use std::collections::BTreeMap;

    fn fila(empresa: &str, dias: &[&str], respondidas: u64, cortadas: u64) -> FilaEmpresa {
        FilaEmpresa {
            empresa: empresa.to_owned(),
            pais_sede: Some("US".to_owned()),
            grupo: None,
            recuento: Recuento {
                consultas: respondidas + cortadas,
                respondidas,
                cortadas,
            },
            primera: 0,
            ultima: 0,
            dias: dias.iter().map(|d| (*d).to_owned()).collect(),
            por_categoria: BTreeMap::new(),
            por_pais_servidor: BTreeMap::new(),
            dominios: Vec::new(),
        }
    }

    const SELLO: &str = "ab12cd34ab12cd34ab12cd34ab12cd34ab12cd34ab12cd34ab12cd34ab12cd34";

    #[test]
    fn only_contacted_companies_get_a_letter() {
        let k = Contenido {
            empresas: vec![
                fila("Google", &["2026-09-01"], 3, 1),
                fila("Criteo", &[], 0, 4),
            ],
            ..Contenido::default()
        };
        let v: Vec<&str> = con_contacto(&k).map(|e| e.empresa.as_str()).collect();
        assert_eq!(v, ["Google"]);
    }

    #[test]
    fn the_evidence_says_contact_on_dates_never_data_sent() {
        let e = fila(
            "Meta",
            &["2026-09-01", "2026-09-02", "2026-09-03", "2026-09-10"],
            41,
            0,
        );
        for (lang, t) in [("es", i18n::es()), ("en", i18n::en()), ("pt", i18n::pt())] {
            let s = evidencia(t, &e, SELLO);
            assert!(s.contains(SELLO), "{lang}: {s}");
            assert!(!s.contains('{'), "{lang}: {s}");
            assert!(s.contains("41"), "{lang}: {s}");
            assert!(
                s.contains(&fecha_texto(t, parse_fecha("2026-09-10").unwrap())),
                "{lang}"
            );
            // Two stretches: 1-3 and 10.
            assert_eq!(
                s.lines().filter(|l| l.starts_with("- ")).count(),
                2,
                "{lang}: {s}"
            );
        }
        let es = evidencia(i18n::es(), &e, SELLO);
        assert!(
            es.contains("contactó con sus servicios en estas fechas"),
            "{es}"
        );
        assert!(es.contains("no demuestra"), "{es}");
        let en = evidencia(i18n::en(), &e, SELLO);
        assert!(
            en.contains("contacted your services on these dates"),
            "{en}"
        );
        // Many stretches: the first ten, then the rest counted.
        let muchos: Vec<String> = (1..=30)
            .step_by(2)
            .map(|d| format!("2026-09-{d:02}"))
            .collect();
        let refs: Vec<&str> = muchos.iter().map(String::as_str).collect();
        let s = evidencia(i18n::es(), &fila("Google", &refs, 15, 0), SELLO);
        assert_eq!(
            s.lines().filter(|l| l.starts_with("- ")).count(),
            RANGOS_MAX + 1,
            "{s}"
        );
        assert!(s.contains('5'), "five more days: {s}");
    }

    #[test]
    fn every_place_writes_the_letter_with_the_certificate() {
        let e = fila("Microsoft", &["2026-09-04", "2026-09-05"], 9, 0);
        let quien = destinatario("Microsoft");
        assert_eq!(
            quien.url.as_deref(),
            Some("https://www.microsoft.com/privacy/privacy-support-requests"),
            "the curated channel wins over a general page"
        );
        let hoy = parse_fecha("2026-10-18").unwrap();
        let citas: &[(&str, &str)] = &[
            ("es", "15"),
            ("gb", "UK GDPR"),
            ("us_ca", "1798.110"),
            ("br", "18"),
            ("co", "1581"),
            ("mx", "2025"),
            ("ar", "25"),
        ];
        for (lang, t) in [("es", i18n::es()), ("en", i18n::en()), ("pt", i18n::pt())] {
            for j in borrado::JURISDICCIONES {
                for tipo in [Tipo::Acceso, Tipo::Borrado] {
                    let ev = evidencia(t, &e, SELLO);
                    let c = carta_con(t, j, &quien, &ev, hoy, tipo);
                    assert!(c.cuerpo.contains(SELLO), "{lang}/{}", j.codigo);
                    assert!(c.cuerpo.contains(MARCA_NOMBRE) && c.cuerpo.contains(MARCA_CORREO));
                    assert!(!c.cuerpo.contains('{'), "{lang}/{}: {}", j.codigo, c.cuerpo);
                    assert!(c.cuerpo.contains("Microsoft"));
                }
            }
            for (cod, cita) in citas {
                let j = jurisdiccion(cod).unwrap();
                let c = carta_con(t, j, &quien, &evidencia(t, &e, SELLO), hoy, Tipo::Acceso);
                assert!(c.cuerpo.contains(cita), "{lang}/{cod}: {}", c.cuerpo);
            }
            // The fallback for any other country claims no law.
            let otro = carta_con(
                t,
                jurisdiccion("otro").unwrap(),
                &quien,
                "",
                hoy,
                Tipo::Acceso,
            );
            assert!(!otro.cuerpo.contains("GDPR") && !otro.cuerpo.contains("1798"));
        }
    }

    #[test]
    fn the_curated_channels_read_whole_with_their_sources() {
        let lista = parse_corredores(include_str!("../data/derechos-contactos.txt")).unwrap();
        assert_eq!(lista.len(), borrado::contactos().len());
        assert!(lista.len() >= 4);
        for c in &lista {
            assert!(c.url.is_some(), "{}", c.id);
            assert!(!c.fuentes.is_empty(), "{}", c.id);
            assert_eq!(c.tipo, "servicios");
            assert_eq!(c.empresas.len(), 1, "{}", c.id);
            // `=Empresa` is a real section of empresas.txt, or it would match nothing.
            assert!(
                guardiana_lists::company_record(&c.empresas[0]).is_some(),
                "{}",
                c.id
            );
            // Not a broker of «Haz que te borren» too: one company, one channel.
            assert!(borrado::corredores().iter().all(|b| b.id != c.id));
        }
        assert!(contacto_de("Meta").is_some());
        assert!(contacto_de("Nadie").is_none());
    }
}
