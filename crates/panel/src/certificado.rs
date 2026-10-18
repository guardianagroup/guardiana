//! «Certificado de flujos» on the panel (1.0.45): the monthly certificate, or one of any
//! period, made on request from this computer's ledger (`guardiana_verify::certificado`), and
//! the check of a certificate file against the same ledger. The page saves what this returns as
//! three files: the sealed `.json`, its `.json.sha256` and the `.html` to read and print.
//!
//! Nothing leaves the computer: the certificate is built here and handed to the page, and the
//! page saves it where the person says.

// Handlers return a full HTTP `Response` as their error, as axum intends (as in api.rs).
#![allow(clippy::result_large_err)]

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use guardiana_core::i18n::Texts;
use guardiana_core::time::now_ms;
use guardiana_verify::certificado::{self, Certificado, Comprobacion, Opciones};
use guardiana_verify::certificado_html;
use serde::{Deserialize, Serialize};

use crate::api::{with_ledger, ApiResult};
use crate::auth::{Lang, Session, Zona};
use crate::AppState;

/// How many months back the page offers.
const MESES: usize = 12;
/// The longest period one certificate may cover: a year and a day of margin.
const MAX_DIAS: i64 = 367;

/// What the page asks for. A month (`mes=AAAA-MM`), or two days (`desde`, `hasta`, both
/// included), or nothing: the last whole month.
#[derive(Deserialize, Debug, Default)]
pub(crate) struct PeriodoQuery {
    #[serde(default)]
    pub(crate) mes: Option<String>,
    #[serde(default)]
    pub(crate) desde: Option<String>,
    #[serde(default)]
    pub(crate) hasta: Option<String>,
    /// `1` to list each company's names.
    #[serde(default)]
    pub(crate) dominios: Option<String>,
    /// Only this company: the extract for a letter.
    #[serde(default)]
    pub(crate) empresa: Option<String>,
}

fn mal(que: &str) -> Response {
    (StatusCode::BAD_REQUEST, que.to_owned()).into_response()
}

/// The options of a request, in the person's zone, or why they cannot be.
pub(crate) fn opciones(q: &PeriodoQuery, zona: i64, ahora: i64) -> Result<Opciones, Response> {
    fn vacio(s: &Option<String>) -> Option<&str> {
        s.as_deref().map(str::trim).filter(|s| !s.is_empty())
    }
    let (desde, hasta) = match (vacio(&q.desde), vacio(&q.hasta), vacio(&q.mes)) {
        (Some(d), Some(h), _) => certificado::periodo_dias(d, h, zona),
        (None, None, Some(m)) => certificado::periodo_mes(m, zona),
        (None, None, None) => {
            certificado::periodo_mes(&certificado::mes_anterior(ahora, zona), zona)
        }
        _ => None,
    }
    .ok_or_else(|| mal("bad period"))?;
    if hasta - desde > MAX_DIAS * guardiana_core::time::DAY_MS {
        return Err(mal("period too long"));
    }
    if desde >= ahora {
        return Err(mal("future period"));
    }
    Ok(Opciones {
        desde,
        hasta,
        zona_min: zona,
        dominios: q.dominios.as_deref() == Some("1"),
        solo_empresa: vacio(&q.empresa).map(str::to_owned),
    })
}

/// The three files of a certificate, as the page saves them.
#[derive(Serialize, Debug)]
pub(crate) struct Archivos {
    /// The sealed JSON, byte for byte: the seal is its SHA-256.
    pub(crate) json: String,
    pub(crate) sello: String,
    pub(crate) html: String,
    /// The `.sha256` file, for `sha256sum -c`.
    pub(crate) linea_sha256: String,
    pub(crate) archivo_json: String,
    pub(crate) archivo_html: String,
    pub(crate) archivo_sha256: String,
}

pub(crate) fn archivos(t: &'static Texts, c: &Certificado) -> Archivos {
    let json = certificado::a_json(c);
    let sello = certificado::sello(json.as_bytes());
    let archivo_json = certificado::nombre_archivo(c, "json");
    Archivos {
        html: certificado_html::html(t, c, &sello),
        linea_sha256: certificado::linea_sha256(&sello, &archivo_json),
        archivo_html: certificado::nombre_archivo(c, "html"),
        archivo_sha256: format!("{archivo_json}.sha256"),
        archivo_json,
        json,
        sello,
    }
}

/// The few figures the page shows before the person opens the document.
#[derive(Serialize, Debug)]
pub(crate) struct Resumen {
    desde_dia: String,
    hasta_dia: String,
    consultas: u64,
    cortadas: u64,
    empresas: usize,
    dias: usize,
    no_contactadas: usize,
    referencia: usize,
    comprobada: bool,
    cobertura_completa: bool,
    registro_desde: Option<i64>,
    sin_vigilar_ms: i64,
    medido: bool,
    /// The companies asked for most: (company, queries, cut).
    principales: Vec<(String, u64, u64)>,
}

fn resumen(c: &Certificado) -> Resumen {
    let k = &c.contenido;
    Resumen {
        desde_dia: c.desde_dia.clone(),
        hasta_dia: c.hasta_dia.clone(),
        consultas: k.totales.recuento.consultas,
        cortadas: k.totales.recuento.cortadas,
        empresas: k.totales.empresas,
        dias: k.totales.dias_con_actividad,
        no_contactadas: k.no_contactadas.len(),
        referencia: k.no_contactadas.len()
            + k.empresas
                .iter()
                .filter(|e| guardiana_lists::profile_of(&e.empresa).is_some())
                .count(),
        comprobada: c.cadena.comprobada,
        cobertura_completa: c.cobertura.completa,
        registro_desde: c.cobertura.registro_desde,
        sin_vigilar_ms: c.cobertura.sin_vigilar_ms,
        medido: c.medido.disponible,
        principales: k
            .empresas
            .iter()
            .take(8)
            .map(|e| (e.empresa.clone(), e.recuento.consultas, e.recuento.cortadas))
            .collect(),
    }
}

/// The months the page offers, newest first, starting with the last whole one.
pub(crate) fn meses(ahora: i64, zona: i64) -> Vec<String> {
    let primero = certificado::mes_anterior(ahora, zona);
    let mut y: i64 = primero
        .get(0..4)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1970);
    let mut m: i64 = primero.get(5..7).and_then(|s| s.parse().ok()).unwrap_or(1);
    let mut out = Vec::with_capacity(MESES);
    for _ in 0..MESES {
        out.push(format!("{y:04}-{m:02}"));
        m -= 1;
        if m == 0 {
            m = 12;
            y -= 1;
        }
    }
    out
}

#[derive(Serialize, Debug)]
pub(crate) struct CertView {
    #[serde(flatten)]
    archivos: Archivos,
    resumen: Resumen,
    meses: Vec<String>,
    /// The month the page offers first: the last whole one.
    mes_anterior: String,
}

pub(crate) async fn generar(
    Lang(t): Lang,
    Zona(zona): Zona,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Query(q): Query<PeriodoQuery>,
) -> ApiResult<CertView> {
    let ahora = now_ms();
    let op = opciones(&q, zona, ahora)?;
    let c = with_ledger(&state, |l| certificado::generar(l, &op, ahora))?;
    Ok(Json(CertView {
        archivos: archivos(t, &c),
        resumen: resumen(&c),
        meses: meses(ahora, zona),
        mes_anterior: certificado::mes_anterior(ahora, zona),
    }))
}

/// The check of a certificate file the page read from disk: its bytes, as the body.
pub(crate) async fn verificar(
    State(state): State<Arc<AppState>>,
    _s: Session,
    body: String,
) -> ApiResult<Comprobacion> {
    let r = with_ledger(&state, |l| Ok(certificado::comprobar(body.as_bytes(), l)))?;
    Ok(Json(r))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const ZONA: i64 = -300;

    fn q(mes: Option<&str>, desde: Option<&str>, hasta: Option<&str>) -> PeriodoQuery {
        PeriodoQuery {
            mes: mes.map(str::to_owned),
            desde: desde.map(str::to_owned),
            hasta: hasta.map(str::to_owned),
            ..PeriodoQuery::default()
        }
    }

    #[test]
    fn the_default_is_the_last_whole_month_and_bad_periods_are_refused() {
        let ahora = certificado::periodo_dias("2026-10-18", "2026-10-18", ZONA)
            .unwrap()
            .0
            + 12 * 3_600_000;
        let op = opciones(&PeriodoQuery::default(), ZONA, ahora).unwrap();
        assert_eq!(
            (op.desde, op.hasta),
            certificado::periodo_mes("2026-09", ZONA).unwrap()
        );
        assert!(!op.dominios && op.solo_empresa.is_none());
        assert!(opciones(&q(Some("2026-9"), None, None), ZONA, ahora).is_err());
        assert!(opciones(&q(None, Some("2026-09-01"), None), ZONA, ahora).is_err());
        assert!(opciones(
            &q(None, Some("2024-01-01"), Some("2026-01-01")),
            ZONA,
            ahora
        )
        .is_err());
        assert!(opciones(&q(Some("2027-01"), None, None), ZONA, ahora).is_err());
        let hoy = opciones(
            &q(None, Some("2026-10-01"), Some("2026-10-18")),
            ZONA,
            ahora,
        )
        .unwrap();
        assert!(
            hoy.hasta > ahora,
            "the end is capped when the certificate is made"
        );
        let m = meses(ahora, ZONA);
        assert_eq!(m.len(), MESES);
        assert_eq!(m[0], "2026-09");
        assert_eq!(m[9], "2025-12");
    }

    #[test]
    fn the_files_carry_the_seal_of_the_json() {
        let l =
            guardiana_core::Ledger::open_in_memory(guardiana_core::identity::genesis()).unwrap();
        let (desde, hasta) = certificado::periodo_mes("2026-09", ZONA).unwrap();
        let op = Opciones {
            desde,
            hasta,
            zona_min: ZONA,
            dominios: false,
            solo_empresa: None,
        };
        let c = certificado::generar(&l, &op, hasta + 1).unwrap();
        for t in [
            guardiana_core::i18n::es(),
            guardiana_core::i18n::en(),
            guardiana_core::i18n::pt(),
        ] {
            let a = archivos(t, &c);
            assert_eq!(a.sello, certificado::sello(a.json.as_bytes()));
            assert_eq!(a.linea_sha256, format!("{}  {}\n", a.sello, a.archivo_json));
            assert!(a.html.contains(&a.sello));
            assert!(a.html.starts_with("<!doctype html>"));
            // Every text of the document exists: none falls back to its key.
            assert!(!a.html.contains("cert_"), "{}", a.html);
            // And no placeholder is left unfilled (the style sheet has braces of its own).
            let cuerpo = a.html.split("</style>").nth(1).unwrap();
            assert!(!cuerpo.contains('{'), "{cuerpo}");
            assert_eq!(a.archivo_sha256, format!("{}.sha256", a.archivo_json));
        }
    }
}
