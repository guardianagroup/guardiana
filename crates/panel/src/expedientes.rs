//! «Expedientes» (the owner, 12 Oct 2026: «Expedientes, adelante»): one file per request the
//! person makes to a company, that it delete their data or say what data it holds. The file
//! keeps where the request stands (prepared, sent, answered, closed), the day of each step, the
//! legal deadline of the place chosen and whether it has passed, what the company answered, and
//! the complaint to the authority when it did not answer in time.
//!
//! What is kept: the company, the place, the kind of request, the dates, a result from a short
//! list, and a note the person writes. Never the person's name, address or the letter: those
//! live in the browser and in the mail they sent. The clock of «Haz que te borren» (the
//! `borrado_envios` setting) becomes the first files, once, the first time this is read.

// Handlers return a full HTTP `Response` as their error, as axum intends (as in api.rs).
#![allow(clippy::result_large_err)]

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use guardiana_core::i18n::{self, Texts};
use guardiana_core::time::now_ms;
use guardiana_core::{Ledger, NameSpan};
use serde::{Deserialize, Serialize};

use crate::api::{with_ledger, ApiResult};
use crate::auth::{Lang, Session, Zona};
use crate::borrado::{
    self, carta_de, corredor, corredores, dia_de, evidencias, fecha_iso, fecha_texto, jurisdiccion,
    parse_fecha, sumar, texto_jur, texto_jur_tipo, Carta, Corredor, Derecho, Envio, Jurisdiccion,
    Tipo, Unidad, MARCA_CORREO, MARCA_NOMBRE, SETTING_ENVIOS,
};
use crate::AppState;

/// The setting that keeps the files: a JSON list of [`Expediente`].
pub(crate) const SETTING: &str = "expedientes";
/// At most this many files: a list nobody can read is no help.
const MAX: usize = 300;
/// A note longer than this is cut: a file, not a diary.
const NOTA_MAX: usize = 600;
/// The panel warns this many days before a deadline.
const AVISO_DIAS: i64 = 3;

/// Where a file stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Estado {
    /// The letter is made; the person has not said they sent it.
    #[default]
    Preparada,
    /// Sent on `enviada`: the clock runs.
    Enviada,
    /// The company answered on `respondida`, with `resultado`.
    Respondida,
    /// Closed by the person, on `cerrada`, with or without a result.
    Cerrada,
}

impl Estado {
    pub(crate) fn clave(self) -> &'static str {
        match self {
            Estado::Preparada => "preparada",
            Estado::Enviada => "enviada",
            Estado::Respondida => "respondida",
            Estado::Cerrada => "cerrada",
        }
    }
}

/// What the company did, in the person's words, from a short list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Resultado {
    /// It confirmed it deleted the data.
    BorradoConfirmado,
    /// It handed over what it holds.
    DatosEntregados,
    /// It asked for proof of identity or more details.
    PidioIdentidad,
    /// It refused, in whole or in part.
    Denegado,
    /// It answered without resolving (an acknowledgement, a form, a redirect).
    SinResolver,
    /// It never answered.
    SinRespuesta,
}

impl Resultado {
    pub(crate) fn clave(self) -> &'static str {
        match self {
            Resultado::BorradoConfirmado => "borrado_confirmado",
            Resultado::DatosEntregados => "datos_entregados",
            Resultado::PidioIdentidad => "pidio_identidad",
            Resultado::Denegado => "denegado",
            Resultado::SinResolver => "sin_resolver",
            Resultado::SinRespuesta => "sin_respuesta",
        }
    }
    const TODOS: [Resultado; 6] = [
        Resultado::BorradoConfirmado,
        Resultado::DatosEntregados,
        Resultado::PidioIdentidad,
        Resultado::Denegado,
        Resultado::SinResolver,
        Resultado::SinRespuesta,
    ];
}

/// One file, as kept. No name, no address, no letter.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Expediente {
    pub(crate) id: u32,
    /// The company's name as the lists write it, or as the person typed it.
    pub(crate) empresa: String,
    /// Its id on «Haz que te borren», when it is one of those companies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) corredor: Option<String>,
    #[serde(default)]
    pub(crate) tipo: Tipo,
    pub(crate) jurisdiccion: String,
    #[serde(default)]
    pub(crate) estado: Estado,
    /// `AAAA-MM-DD`, each as the person said.
    pub(crate) creada: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) enviada: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) respondida: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) cerrada: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) resultado: Option<Resultado>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) notas: String,
}

impl Expediente {
    /// Whether the clock still counts: sent and neither answered nor closed.
    fn abierto(&self) -> bool {
        self.estado == Estado::Enviada
    }
}

/// A file as the page shows it.
#[derive(Serialize, Debug, Clone)]
pub(crate) struct ExpedienteView {
    id: u32,
    empresa: String,
    corredor: Option<String>,
    tipo: &'static str,
    tipo_texto: String,
    jurisdiccion: String,
    jurisdiccion_nombre: String,
    estado: &'static str,
    estado_texto: String,
    resultado: Option<&'static str>,
    resultado_texto: Option<String>,
    creada: String,
    creada_texto: String,
    enviada: Option<String>,
    enviada_texto: Option<String>,
    respondida: Option<String>,
    respondida_texto: Option<String>,
    cerrada: Option<String>,
    cerrada_texto: Option<String>,
    notas: String,
    /// The day the deadline ends, while the file is sent and the place fixes one.
    vence: Option<String>,
    /// Days left; negative once it has passed.
    dias: Option<i64>,
    vencido: bool,
    /// Whether the panel warns about it: sent, and overdue or due within a few days.
    aviso: bool,
    /// The one sentence about the clock.
    frase: String,
    prorroga: Option<String>,
    notas_plazo: Vec<String>,
    /// Once overdue: the authority and how to complain.
    autoridad: Option<&'static str>,
    autoridad_url: Option<&'static str>,
    queja: Option<String>,
    /// The company's own page for these requests, when known.
    url: Option<String>,
}

fn t_tipo(t: &Texts, tipo: Tipo) -> String {
    t.panel(&format!("exp_tipo_{}", tipo.clave())).to_owned()
}

fn t_estado(t: &Texts, e: Estado) -> String {
    t.panel(&format!("exp_estado_{}", e.clave())).to_owned()
}

fn t_resultado(t: &Texts, r: Resultado) -> String {
    t.panel(&format!("exp_resultado_{}", r.clave())).to_owned()
}

/// The deadline of a sent file: the day it ends and the days left.
fn plazo_de(e: &Expediente, j: Option<&Jurisdiccion>, hoy: i64) -> (Option<i64>, Option<i64>) {
    if !e.abierto() {
        return (None, None);
    }
    let Some(enviada) = e.enviada.as_deref().and_then(parse_fecha) else {
        return (None, None);
    };
    let plazo = j.and_then(|j| j.plazos(e.tipo).0);
    let vence = plazo.map(|p| sumar(enviada, p));
    (vence, vence.map(|v| v - hoy))
}

pub(crate) fn vista(t: &Texts, e: &Expediente, hoy: i64) -> ExpedienteView {
    let j = jurisdiccion(&e.jurisdiccion);
    let (vence, dias) = plazo_de(e, j, hoy);
    let vencido = dias.is_some_and(|d| d < 0);
    let aviso = e.abierto() && (vencido || dias.is_some_and(|d| d <= AVISO_DIAS));
    let frase = match (e.estado, vence, dias) {
        (Estado::Preparada, _, _) => t.panel("exp_frase_preparada").to_owned(),
        (Estado::Enviada, Some(v), Some(d)) if d > 0 => t
            .panel_n("borrado_reloj_quedan", d)
            .replace("{fecha}", &fecha_texto(t, v)),
        (Estado::Enviada, Some(v), Some(0)) => t
            .panel("borrado_reloj_hoy")
            .replace("{fecha}", &fecha_texto(t, v)),
        (Estado::Enviada, Some(v), Some(d)) => t
            .panel_n("borrado_reloj_vencido", -d)
            .replace("{fecha}", &fecha_texto(t, v)),
        (Estado::Enviada, _, _) => t
            .panel(
                if e.tipo == Tipo::Acceso && j.is_some_and(|j| j.plazo.is_some()) {
                    "exp_frase_sin_plazo_acceso"
                } else {
                    "borrado_reloj_sin_plazo"
                },
            )
            .to_owned(),
        (Estado::Respondida, _, _) => t.panel("exp_frase_respondida").replace(
            "{fecha}",
            &e.respondida
                .as_deref()
                .and_then(parse_fecha)
                .map(|d| fecha_texto(t, d))
                .unwrap_or_default(),
        ),
        (Estado::Cerrada, _, _) => t.panel("exp_frase_cerrada").to_owned(),
    };
    let prorroga = j.and_then(|j| {
        let v = vence?;
        let p = j.plazos(e.tipo).1?;
        Some(
            t.panel("borrado_reloj_prorroga")
                .replace("{fecha}", &fecha_texto(t, sumar(v, p))),
        )
    });
    let mut notas_plazo = Vec::new();
    if let (Some(j), true) = (j, e.abierto()) {
        if j.plazos(e.tipo)
            .0
            .is_some_and(|p| p.unidad == Unidad::DiasHabiles)
        {
            notas_plazo.push(t.panel("borrado_reloj_habiles").to_owned());
        }
        if j.es_referencia(e.tipo) && vence.is_some() {
            notas_plazo.push(texto_jur(t, "referencia", j));
        }
    }
    let fecha = |f: &Option<String>| {
        f.as_deref()
            .and_then(parse_fecha)
            .map(|d| fecha_texto(t, d))
    };
    let url = e
        .corredor
        .as_deref()
        .and_then(corredor)
        .and_then(|c| c.url.clone())
        .or_else(|| {
            guardiana_lists::profile_of(&e.empresa)
                .and_then(|f| f.get("borrar"))
                .and_then(|u| u.as_str())
                .filter(|u| u.starts_with("https://"))
                .map(str::to_owned)
        });
    ExpedienteView {
        id: e.id,
        empresa: e.empresa.clone(),
        corredor: e.corredor.clone(),
        tipo: e.tipo.clave(),
        tipo_texto: t_tipo(t, e.tipo),
        jurisdiccion: e.jurisdiccion.clone(),
        jurisdiccion_nombre: j.map_or_else(|| e.jurisdiccion.clone(), |j| texto_jur(t, "jur", j)),
        estado: e.estado.clave(),
        estado_texto: t_estado(t, e.estado),
        resultado: e.resultado.map(Resultado::clave),
        resultado_texto: e.resultado.map(|r| t_resultado(t, r)),
        creada_texto: fecha(&Some(e.creada.clone())).unwrap_or_default(),
        creada: e.creada.clone(),
        enviada_texto: fecha(&e.enviada),
        enviada: e.enviada.clone(),
        respondida_texto: fecha(&e.respondida),
        respondida: e.respondida.clone(),
        cerrada_texto: fecha(&e.cerrada),
        cerrada: e.cerrada.clone(),
        notas: e.notas.clone(),
        vence: vence.map(fecha_iso),
        dias,
        vencido,
        aviso,
        frase,
        prorroga,
        notas_plazo,
        autoridad: j.and_then(|j| j.autoridad).filter(|_| vencido),
        autoridad_url: j.and_then(|j| j.autoridad_url).filter(|_| vencido),
        queja: j.filter(|_| vencido).map(|j| texto_jur(t, "queja", j)),
        url,
    }
}

// ----- storage ------------------------------------------------------------------------------

/// The files kept in the settings. The first time, the clocks of «Haz que te borren» become
/// files (sent, deletion), and that setting is emptied so nothing is counted twice.
pub(crate) fn leer(l: &Ledger) -> guardiana_core::Result<Vec<Expediente>> {
    if let Some(v) = l.setting(SETTING)? {
        return Ok(serde_json::from_str(&v).unwrap_or_default());
    }
    let viejos: Vec<Envio> = l
        .setting(SETTING_ENVIOS)?
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_default();
    let lista: Vec<Expediente> = viejos
        .iter()
        .map(|e| Expediente {
            id: e.id,
            empresa: corredor(&e.corredor).map_or_else(|| e.corredor.clone(), |c| c.nombre.clone()),
            corredor: Some(e.corredor.clone()),
            tipo: Tipo::Borrado,
            jurisdiccion: e.jurisdiccion.clone(),
            estado: Estado::Enviada,
            creada: e.enviada.clone(),
            enviada: Some(e.enviada.clone()),
            respondida: None,
            cerrada: None,
            resultado: None,
            notas: String::new(),
        })
        .collect();
    guardar(l, &lista)?;
    if !viejos.is_empty() {
        l.set_setting(SETTING_ENVIOS, "[]")?;
    }
    Ok(lista)
}

pub(crate) fn guardar(l: &Ledger, e: &[Expediente]) -> guardiana_core::Result<()> {
    l.set_setting(
        SETTING,
        &serde_json::to_string(e).unwrap_or_else(|_| "[]".to_owned()),
    )
}

/// The files that still count for the panel's warning: sent, and overdue or due within a few
/// days. For the banner every page shows.
pub(crate) fn avisos(l: &Ledger, hoy: i64) -> guardiana_core::Result<(usize, usize)> {
    let lista = leer(l)?;
    let mut vencidos = 0;
    let mut pronto = 0;
    for e in &lista {
        let j = jurisdiccion(&e.jurisdiccion);
        let (_, dias) = plazo_de(e, j, hoy);
        match dias {
            Some(d) if d < 0 => vencidos += 1,
            Some(d) if d <= AVISO_DIAS => pronto += 1,
            _ => {}
        }
    }
    Ok((vencidos, pronto))
}

/// A deletion file sent to a company of «Haz que te borren», opened from that page's «¿La
/// enviaste? Sí»: the same file this page would open.
pub(crate) fn abrir_enviado(
    l: &Ledger,
    corredor_id: &str,
    jurisdiccion: &str,
    enviada: String,
) -> guardiana_core::Result<()> {
    let mut lista = leer(l)?;
    if lista.len() >= MAX {
        lista.remove(0);
    }
    let id = lista.iter().map(|e| e.id).max().unwrap_or(0) + 1;
    lista.push(Expediente {
        id,
        empresa: corredor(corredor_id).map_or_else(|| corredor_id.to_owned(), |c| c.nombre.clone()),
        corredor: Some(corredor_id.to_owned()),
        tipo: Tipo::Borrado,
        jurisdiccion: jurisdiccion.to_owned(),
        estado: Estado::Enviada,
        creada: enviada.clone(),
        enviada: Some(enviada),
        respondida: None,
        cerrada: None,
        resultado: None,
        notas: String::new(),
    });
    guardar(l, &lista)
}

pub(crate) fn quitar_de(l: &Ledger, id: u32) -> guardiana_core::Result<()> {
    let mut lista = leer(l)?;
    lista.retain(|e| e.id != id);
    guardar(l, &lista)
}

// ----- who the file is with --------------------------------------------------------------------

/// The company a file is with, as a letter's addressee: one of «Haz que te borren» by its id,
/// else any company of the lists or a site by name, else nothing.
fn quien(e: &Expediente) -> Option<Corredor> {
    if let Some(c) = e.corredor.as_deref().and_then(corredor) {
        return Some(c.clone());
    }
    crate::empresa::destinatario(&e.empresa)
}

fn no_encontrado(que: &str) -> Response {
    (StatusCode::NOT_FOUND, format!("unknown {que}")).into_response()
}

fn mal(que: &str) -> Response {
    (StatusCode::BAD_REQUEST, que.to_owned()).into_response()
}

/// A day the person typed, `AAAA-MM-DD`, not after today (a request is not sent tomorrow).
fn dia_dicho(s: Option<&str>, hoy: i64) -> Result<i64, Response> {
    match s.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(hoy),
        Some(s) => {
            let d = parse_fecha(s).ok_or_else(|| mal("bad date"))?;
            if d > hoy {
                return Err(mal("future date"));
            }
            Ok(d)
        }
    }
}

// ----- the page's API ------------------------------------------------------------------------

/// A company the house asked for, to pick from when opening a file.
#[derive(Serialize, Debug)]
pub(crate) struct EmpresaVista {
    empresa: String,
    consultas: i64,
    /// Its id on «Haz que te borren», when it is one of those.
    corredor: Option<String>,
}

#[derive(Serialize, Debug)]
pub(crate) struct Resumen {
    expedientes: Vec<ExpedienteView>,
    /// Sent and overdue; sent and due within a few days.
    vencidos: usize,
    pronto: usize,
    jurisdicciones: Vec<borrado::JurView>,
    /// The companies the house asked for most, with those of «Haz que te borren» marked.
    vistas: Vec<EmpresaVista>,
    /// Every company of «Haz que te borren», for the picker.
    corredores: Vec<EmpresaVista>,
    resultados: Vec<(&'static str, String)>,
    hoy: String,
}

/// The companies the house asked for, most asked first, by the company of the lists.
fn vistas_de(spans: &[NameSpan]) -> Vec<EmpresaVista> {
    let mut por: HashMap<&'static str, i64> = HashMap::new();
    for s in spans {
        if let Some(e) = crate::api::empresa_de(&s.qname) {
            *por.entry(e).or_insert(0) += s.queries;
        }
    }
    let mut v: Vec<EmpresaVista> = por
        .into_iter()
        .map(|(empresa, consultas)| EmpresaVista {
            corredor: corredores()
                .iter()
                .find(|c| c.empresas.iter().any(|x| x == empresa))
                .map(|c| c.id.clone()),
            empresa: empresa.to_owned(),
            consultas,
        })
        .collect();
    v.sort_by(|a, b| {
        b.consultas
            .cmp(&a.consultas)
            .then_with(|| a.empresa.cmp(&b.empresa))
    });
    v.truncate(60);
    v
}

pub(crate) async fn resumen(
    Lang(t): Lang,
    Zona(zona): Zona,
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<Resumen> {
    let hoy = dia_de(now_ms(), zona);
    let (lista, spans) = with_ledger(&state, |l| Ok((leer(l)?, l.name_device_spans()?)))?;
    Ok(Json(resumen_de(t, &lista, &spans, hoy)))
}

fn resumen_de(t: &Texts, lista: &[Expediente], spans: &[NameSpan], hoy: i64) -> Resumen {
    let vistas = vistas_de(spans);
    let expedientes: Vec<ExpedienteView> = lista.iter().rev().map(|e| vista(t, e, hoy)).collect();
    Resumen {
        vencidos: expedientes.iter().filter(|e| e.vencido).count(),
        pronto: expedientes.iter().filter(|e| e.aviso && !e.vencido).count(),
        expedientes,
        jurisdicciones: borrado::JURISDICCIONES
            .iter()
            .map(|j| borrado::jur_view(t, j))
            .collect(),
        vistas,
        corredores: corredores()
            .iter()
            .filter(|c| c.estado.is_none())
            .map(|c| EmpresaVista {
                empresa: c.nombre.clone(),
                consultas: 0,
                corredor: Some(c.id.clone()),
            })
            .collect(),
        resultados: Resultado::TODOS
            .iter()
            .map(|r| (r.clave(), t_resultado(t, *r)))
            .collect(),
        hoy: fecha_iso(hoy),
    }
}

/// What the page sends to open a file. Anything else is refused, so that a careless page can
/// never slip a name or an address into the settings.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub(crate) struct Nuevo {
    /// A company of «Haz que te borren», by id; or
    #[serde(default)]
    corredor: Option<String>,
    /// any company of the lists, or a site, by name.
    #[serde(default)]
    empresa: Option<String>,
    jurisdiccion: String,
    #[serde(default)]
    tipo: Tipo,
    /// `preparada` (default) or `enviada`, with `fecha` the day it was sent.
    #[serde(default)]
    estado: Option<Estado>,
    #[serde(default)]
    fecha: Option<String>,
}

pub(crate) async fn nuevo(
    Lang(t): Lang,
    Zona(zona): Zona,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Json(body): Json<Nuevo>,
) -> ApiResult<Vec<ExpedienteView>> {
    let hoy = dia_de(now_ms(), zona);
    jurisdiccion(&body.jurisdiccion).ok_or_else(|| no_encontrado("jurisdiction"))?;
    let (empresa, corredor_id) = match (&body.corredor, &body.empresa) {
        (Some(id), _) => {
            let c = corredor(id).ok_or_else(|| no_encontrado("company"))?;
            (c.nombre.clone(), Some(c.id.clone()))
        }
        (None, Some(e)) => {
            let q = crate::empresa::destinatario(e).ok_or_else(|| no_encontrado("company"))?;
            let id = (!q.id.starts_with("empresa:")).then(|| q.id.clone());
            (q.nombre, id)
        }
        (None, None) => return Err(mal("company missing")),
    };
    let estado = body.estado.unwrap_or_default();
    if !matches!(estado, Estado::Preparada | Estado::Enviada) {
        return Err(mal("a new file is prepared or sent"));
    }
    let dia = dia_dicho(body.fecha.as_deref(), hoy)?;
    let lista = with_ledger(&state, |l| {
        let mut lista = leer(l)?;
        if lista.len() >= MAX {
            lista.remove(0);
        }
        let id = lista.iter().map(|e| e.id).max().unwrap_or(0) + 1;
        lista.push(Expediente {
            id,
            empresa,
            corredor: corredor_id,
            tipo: body.tipo,
            jurisdiccion: body.jurisdiccion.clone(),
            estado,
            creada: fecha_iso(hoy),
            enviada: (estado == Estado::Enviada).then(|| fecha_iso(dia)),
            respondida: None,
            cerrada: None,
            resultado: None,
            notas: String::new(),
        });
        guardar(l, &lista)?;
        Ok(lista)
    })?;
    Ok(Json(lista.iter().rev().map(|e| vista(t, e, hoy)).collect()))
}

/// A step of a file, or its note. Anything else is refused.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub(crate) struct Cambio {
    /// `enviada`, `respondida`, `cerrada`, or `enviada` again to reopen a closed one.
    #[serde(default)]
    estado: Option<Estado>,
    /// The day of that step; today when missing.
    #[serde(default)]
    fecha: Option<String>,
    #[serde(default)]
    resultado: Option<Resultado>,
    #[serde(default)]
    notas: Option<String>,
}

pub(crate) async fn cambiar(
    Lang(t): Lang,
    Zona(zona): Zona,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Path(id): Path<u32>,
    Json(body): Json<Cambio>,
) -> ApiResult<Vec<ExpedienteView>> {
    let hoy = dia_de(now_ms(), zona);
    let dia = dia_dicho(body.fecha.as_deref(), hoy)?;
    let lista = with_ledger(&state, |l| {
        let mut lista = leer(l)?;
        let Some(e) = lista.iter_mut().find(|e| e.id == id) else {
            return Ok(None);
        };
        if let Some(estado) = body.estado {
            match estado {
                Estado::Preparada => {
                    e.estado = Estado::Preparada;
                    e.enviada = None;
                    e.respondida = None;
                    e.cerrada = None;
                    e.resultado = None;
                }
                Estado::Enviada => {
                    e.estado = Estado::Enviada;
                    // Reopening keeps the day it was sent; a first sending sets it.
                    if e.enviada.is_none() || body.fecha.is_some() {
                        e.enviada = Some(fecha_iso(dia));
                    }
                    e.respondida = None;
                    e.cerrada = None;
                    e.resultado = None;
                }
                Estado::Respondida => {
                    e.estado = Estado::Respondida;
                    e.respondida = Some(fecha_iso(dia));
                    e.cerrada = None;
                    if e.enviada.is_none() {
                        e.enviada = Some(fecha_iso(dia));
                    }
                }
                Estado::Cerrada => {
                    e.estado = Estado::Cerrada;
                    e.cerrada = Some(fecha_iso(dia));
                }
            }
        }
        if let Some(r) = body.resultado {
            e.resultado = Some(r);
        }
        if let Some(n) = &body.notas {
            e.notas = n.trim().chars().take(NOTA_MAX).collect();
        }
        guardar(l, &lista)?;
        Ok(Some(lista))
    })?
    .ok_or_else(|| no_encontrado("file"))?;
    Ok(Json(lista.iter().rev().map(|e| vista(t, e, hoy)).collect()))
}

pub(crate) async fn quitar(
    Lang(t): Lang,
    Zona(zona): Zona,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Path(id): Path<u32>,
) -> ApiResult<Vec<ExpedienteView>> {
    let hoy = dia_de(now_ms(), zona);
    let lista = with_ledger(&state, |l| {
        let mut lista = leer(l)?;
        lista.retain(|e| e.id != id);
        guardar(l, &lista)?;
        Ok(lista)
    })?;
    Ok(Json(lista.iter().rev().map(|e| vista(t, e, hoy)).collect()))
}

#[derive(Deserialize)]
pub(crate) struct CartaQuery {
    #[serde(default)]
    idioma: Option<String>,
    #[serde(default)]
    evidencia: Option<String>,
}

/// The letter of a file, again: the same one «Haz que te borren» or a profile made, for the
/// place and the kind of request of the file.
pub(crate) async fn carta(
    Lang(t_panel): Lang,
    Zona(zona): Zona,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Path(id): Path<u32>,
    Query(q): Query<CartaQuery>,
) -> ApiResult<Carta> {
    let hoy = dia_de(now_ms(), zona);
    let t = q.idioma.as_deref().map_or(t_panel, i18n::by_code);
    let (e, spans) = with_ledger(&state, |l| {
        let lista = leer(l)?;
        let e = lista.into_iter().find(|e| e.id == id);
        let spans = if q.evidencia.as_deref() == Some("0") {
            Vec::new()
        } else {
            l.name_device_spans()?
        };
        Ok((e, spans))
    })?;
    let e = e.ok_or_else(|| no_encontrado("file"))?;
    let j = jurisdiccion(&e.jurisdiccion).ok_or_else(|| no_encontrado("jurisdiction"))?;
    let c = quien(&e).ok_or_else(|| no_encontrado("company"))?;
    let ev = (!spans.is_empty())
        .then(|| evidencias(&spans, std::slice::from_ref(&c)).remove(&c.id))
        .flatten();
    Ok(Json(carta_de(t, j, &c, ev.as_ref(), hoy, zona, e.tipo)))
}

/// The complaint to the authority of the place, once the deadline passed: what was asked, when,
/// what the law gave the company, and that it did not answer (or answered as the file says).
/// The person's name and e-mail are filled in the browser, as in every letter.
pub(crate) async fn queja(
    Lang(t_panel): Lang,
    Zona(zona): Zona,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Path(id): Path<u32>,
    Query(q): Query<CartaQuery>,
) -> ApiResult<Carta> {
    let hoy = dia_de(now_ms(), zona);
    let t = q.idioma.as_deref().map_or(t_panel, i18n::by_code);
    let e = with_ledger(&state, |l| Ok(leer(l)?.into_iter().find(|e| e.id == id)))?
        .ok_or_else(|| no_encontrado("file"))?;
    let j = jurisdiccion(&e.jurisdiccion).ok_or_else(|| no_encontrado("jurisdiction"))?;
    Ok(Json(queja_de(t, &e, j, hoy)))
}

pub(crate) fn queja_de(t: &'static Texts, e: &Expediente, j: &Jurisdiccion, hoy: i64) -> Carta {
    let enviada = e.enviada.as_deref().and_then(parse_fecha);
    let (vence, _) = plazo_de(
        &Expediente {
            estado: Estado::Enviada,
            ..e.clone()
        },
        Some(j),
        hoy,
    );
    let autoridad = j.autoridad.map_or_else(
        || t.panel("exp_queja_autoridad_generica").to_owned(),
        str::to_owned,
    );
    let que_paso = match e.resultado {
        Some(r) if r != Resultado::SinRespuesta => t
            .panel("exp_queja_respondio")
            .replace("{resultado}", &t_resultado(t, r)),
        _ => t.panel("exp_queja_no_respondio").to_owned(),
    };
    let cuerpo = t
        .panel(if j.derecho == Derecho::Ninguno {
            "exp_queja_cuerpo_ninguno"
        } else {
            "exp_queja_cuerpo"
        })
        .replace("{autoridad}", &autoridad)
        .replace("{empresa}", &e.empresa)
        .replace(
            "{solicitud}",
            t.panel(&format!("exp_queja_solicitud_{}", e.tipo.clave())),
        )
        .replace("{ley}", &texto_jur(t, "ley", j))
        .replace("{articulo}", &texto_jur_tipo(t, "articulo", j, e.tipo))
        .replace("{plazo}", &texto_jur_tipo(t, "plazo", j, e.tipo))
        .replace(
            "{enviada}",
            &enviada.map(|d| fecha_texto(t, d)).unwrap_or_default(),
        )
        .replace(
            "{vence}",
            &vence.map(|d| fecha_texto(t, d)).unwrap_or_default(),
        )
        .replace("{que_paso}", &que_paso)
        .replace("{fecha}", &fecha_texto(t, hoy))
        .replace("{nombre}", MARCA_NOMBRE)
        .replace("{correo}", MARCA_CORREO);
    Carta {
        asunto: t.panel("exp_queja_asunto").replace("{empresa}", &e.empresa),
        cuerpo,
        para: None,
        url: j.autoridad_url.map(str::to_owned),
        idioma: i18n::code_of(t),
        hueco_nombre: t.panel("borrado_tu_nombre").to_owned(),
        hueco_correo: t.panel("borrado_tu_correo").to_owned(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn enviado(id: u32, jur: &str, tipo: Tipo, enviada: &str) -> Expediente {
        Expediente {
            id,
            empresa: "Criteo".to_owned(),
            corredor: None,
            tipo,
            jurisdiccion: jur.to_owned(),
            estado: Estado::Enviada,
            creada: enviada.to_owned(),
            enviada: Some(enviada.to_owned()),
            respondida: None,
            cerrada: None,
            resultado: None,
            notas: String::new(),
        }
    }

    /// Sent under Spain's law on the 1st: due on the 1st of the next month; three days before,
    /// the panel warns; the day after, it is overdue and names the authority.
    #[test]
    fn the_clock_counts_warns_and_names_the_authority() {
        let t = i18n::es();
        let e = enviado(1, "es", Tipo::Borrado, "2026-10-01");
        let hoy = parse_fecha("2026-10-20").unwrap();
        let v = vista(t, &e, hoy);
        assert_eq!(v.vence.as_deref(), Some("2026-11-01"));
        assert_eq!(v.dias, Some(12));
        assert!(!v.aviso && !v.vencido);
        let v = vista(t, &e, parse_fecha("2026-10-30").unwrap());
        assert!(v.aviso && !v.vencido, "{v:?}");
        let v = vista(t, &e, parse_fecha("2026-11-02").unwrap());
        assert!(v.vencido && v.aviso);
        assert!(v.autoridad.unwrap().contains("AEPD"));
        assert!(v.queja.is_some());
        // Closed, the clock stops and nothing is warned.
        let cerrada = Expediente {
            estado: Estado::Cerrada,
            cerrada: Some("2026-11-03".to_owned()),
            resultado: Some(Resultado::BorradoConfirmado),
            ..e.clone()
        };
        let v = vista(t, &cerrada, parse_fecha("2026-11-10").unwrap());
        assert!(!v.aviso && !v.vencido && v.vence.is_none());
        assert_eq!(v.resultado, Some("borrado_confirmado"));
    }

    /// Access and deletion have their own deadlines where the law gives them (Colombia: 10
    /// working days to answer a query, 15 to answer a claim), and where the access deadline was
    /// not checked (Peru) the clock says so instead of counting.
    #[test]
    fn access_has_its_own_deadline_where_the_law_fixes_one() {
        let t = i18n::es();
        let hoy = parse_fecha("2026-10-13").unwrap();
        let b = vista(t, &enviado(1, "co", Tipo::Borrado, "2026-10-13"), hoy);
        let a = vista(t, &enviado(2, "co", Tipo::Acceso, "2026-10-13"), hoy);
        assert!(b.dias.unwrap() > a.dias.unwrap(), "{b:?} {a:?}");
        let p = vista(t, &enviado(3, "pe", Tipo::Acceso, "2026-10-13"), hoy);
        assert!(p.vence.is_none());
        assert_eq!(p.frase, t.panel("exp_frase_sin_plazo_acceso"));
        for (name, tt) in [("es", i18n::es()), ("en", i18n::en()), ("pt", i18n::pt())] {
            let a = vista(tt, &enviado(2, "co", Tipo::Acceso, "2026-10-13"), hoy);
            assert!(
                !a.frase.contains("borrado_") && !a.frase.contains("exp_"),
                "{name}: {a:?}"
            );
        }
    }

    /// The letters: an access request under the GDPR cites article 15, and the complaint names
    /// the authority, the company, the day sent and the day the deadline ended; nothing in them
    /// is the person's name (the browser fills the marks).
    #[test]
    fn the_access_letter_and_the_complaint_say_the_law_and_the_dates() {
        let t = i18n::es();
        let j = jurisdiccion("es").unwrap();
        let c = crate::empresa::destinatario("Criteo").unwrap();
        let hoy = parse_fecha("2026-10-13").unwrap();
        let carta = carta_de(t, j, &c, None, hoy, 0, Tipo::Acceso);
        assert!(carta.cuerpo.contains("artículo 15"), "{}", carta.cuerpo);
        assert!(carta.cuerpo.contains(MARCA_NOMBRE) && carta.cuerpo.contains(MARCA_CORREO));
        assert!(!carta.cuerpo.contains("{"), "{}", carta.cuerpo);
        let e = enviado(1, "es", Tipo::Acceso, "2026-09-01");
        let q = queja_de(t, &e, j, parse_fecha("2026-10-13").unwrap());
        assert!(
            q.cuerpo.contains("AEPD") || q.cuerpo.contains("Agencia Española"),
            "{}",
            q.cuerpo
        );
        assert!(q.cuerpo.contains("Criteo") && q.cuerpo.contains("2026"));
        assert!(!q.cuerpo.contains("{"), "{}", q.cuerpo);
        assert!(q.url.unwrap().starts_with("https://"));
        for tt in [i18n::en(), i18n::pt()] {
            let q = queja_de(tt, &e, j, hoy);
            assert!(
                !q.cuerpo.contains("{") && !q.cuerpo.contains("exp_"),
                "{}",
                q.cuerpo
            );
            let a = carta_de(tt, j, &c, None, hoy, 0, Tipo::Acceso);
            assert!(
                !a.cuerpo.contains("{") && !a.cuerpo.contains("borrado_"),
                "{}",
                a.cuerpo
            );
        }
        // Where there is no law, the request is a request and the complaint says so.
        let us = jurisdiccion("us").unwrap();
        let a = carta_de(t, us, &c, None, hoy, 0, Tipo::Acceso);
        assert!(
            !a.cuerpo.contains("{") && !a.cuerpo.contains("borrado_"),
            "{}",
            a.cuerpo
        );
        let q = queja_de(t, &enviado(2, "us", Tipo::Acceso, "2026-09-01"), us, hoy);
        assert!(
            !q.cuerpo.contains("{") && !q.cuerpo.contains("exp_"),
            "{}",
            q.cuerpo
        );
    }

    /// Every place has a text for the access article and its deadline, in the three languages,
    /// or falls back to the deletion deadline on purpose (the same one).
    #[test]
    fn every_place_has_its_access_texts() {
        for (name, t) in [("es", i18n::es()), ("en", i18n::en()), ("pt", i18n::pt())] {
            for j in borrado::JURISDICCIONES {
                let k = format!("borrado_articulo_acceso_{}", j.codigo);
                assert!(t.panel_has(&k), "{name}: falta {k}");
                // A place whose access deadline differs from the deletion one says it in words.
                if j.plazo_acceso != j.plazo {
                    let k = format!("borrado_plazo_acceso_{}", j.codigo);
                    assert!(t.panel_has(&k), "{name}: falta {k}");
                }
            }
        }
    }

    /// Every place, language and kind: the letter and the complaint read as sentences. The
    /// article is dropped into «reconocido en …», «previsto …», «under …», «invocando …», so
    /// each language's articles must fit them (review of 12 Oct 2026: «com base em o artigo»,
    /// «al amparo de el artículo», «solicitud de borrado de mis datos de mis datos»).
    #[test]
    fn every_letter_and_complaint_reads_as_a_sentence() {
        let c = crate::empresa::destinatario("Criteo").unwrap();
        let hoy = parse_fecha("2026-10-13").unwrap();
        let malos = [
            "{",
            "borrado_",
            "exp_",
            " de el ",
            " a el ",
            " en en ",
            " em o ",
            " em a ",
            "base em ",
            "under the right",
            "de mis datos de mis datos",
            "reconhecido em",
        ];
        for (name, t) in [("es", i18n::es()), ("en", i18n::en()), ("pt", i18n::pt())] {
            for j in borrado::JURISDICCIONES {
                for tipo in [Tipo::Borrado, Tipo::Acceso] {
                    let carta = carta_de(t, j, &c, None, hoy, 0, tipo);
                    let e = Expediente {
                        tipo,
                        ..enviado(1, j.codigo, tipo, "2026-06-01")
                    };
                    let q = queja_de(t, &e, j, hoy);
                    for texto in [&carta.cuerpo, &carta.asunto, &q.cuerpo, &q.asunto] {
                        for m in malos {
                            assert!(
                                !texto.contains(m),
                                "{name}/{}/{tipo:?}: «{m}» in\n{texto}",
                                j.codigo
                            );
                        }
                    }
                }
            }
        }
    }
}
