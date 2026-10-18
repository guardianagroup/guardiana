//! «Visión profunda» (1.0.45): per program, which companies it talks to, how much goes up and
//! down, how much goes to addresses no name was seen for, and the «posible transferencia»
//! warnings; with the antivirus and the AI apps first.
//!
//! The owner, 18 Oct 2026: «la experiencia del cliente debe ser sentir el monitoreo constante…
//! para saber que todo está bien y nada está vigilando o extrayendo sus datos».
//!
//! Everything is read from what the service already measures (the per-minute traffic and the
//! hourly chained summary of `guardiana_core::vision`); nothing is decrypted, no packet is
//! captured, no certificate is installed. What this cannot see is said on the page: the content,
//! the name inside an encrypted connection (SNI is not read: that would need capturing packets),
//! the server's certificate (TLS 1.3 encrypts it), and anything on another device.
//!
//! The company of a measurement, in order: the AI service when the name is one (so «Google
//! Gemini» stays apart from the rest of Google), else the group or company that owns the name,
//! else the company that owns the network (its `AS` line), else the network's own name. A name
//! served through a delivery network or a cloud adds up to the network: the road, honestly named.

// The handlers answer with the same error type as the rest of the API (see api.rs).
#![allow(clippy::result_large_err)]

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

use axum::extract::{Query, State};
use guardiana_core::i18n::Texts;
use guardiana_core::time::{now_ms, DAY_MS, HOUR_MS};
use guardiana_core::vision::{
    hora_de, Envio, FilaResumen, PosibleTransferencia, MAX_FILAS_HORA, OTRAS,
};
use guardiana_core::{Ledger, Trafico};
use guardiana_lists::antivirus;
use serde::Serialize;

use crate::api::{with_ledger, ApiResult};
use crate::auth::{Lang, Session, Zona};
use crate::AppState;

/// The company label of a local address (the house's own network).
pub const LOCAL: &str = "@local";
/// The company label of an anycast address whose network is not known.
pub const ANYCAST: &str = "@anycast";

/// The AI apps installed on the computer, by the file name Windows reports, with the service of
/// `ia.txt` they belong to. The names are the ones their makers ship as of October 2026; one
/// renamed is simply counted as any other program until it is added here.
const APPS_IA: &[(&str, &str)] = &[
    ("chatgpt.exe", "OpenAI (ChatGPT)"),
    ("claude.exe", "Anthropic (Claude)"),
    ("copilot.exe", "Microsoft Copilot"),
    ("m365copilot.exe", "Microsoft Copilot"),
    ("perplexity.exe", "Perplexity"),
    ("cursor.exe", "Cursor"),
    ("windsurf.exe", "Windsurf"),
    ("ollama.exe", "Ollama"),
    ("ollama app.exe", "Ollama"),
    ("lm studio.exe", "LM Studio"),
];

/// Browsers, whose AI is a website: it is told apart by the name it asked for.
const NAVEGADORES: &[&str] = &[
    "chrome.exe",
    "msedge.exe",
    "firefox.exe",
    "brave.exe",
    "opera.exe",
    "vivaldi.exe",
    "arc.exe",
    "iexplore.exe",
];

/// The services of `ia.txt`, by their section name.
fn servicios_ia() -> &'static HashSet<&'static str> {
    static S: OnceLock<HashSet<&'static str>> = OnceLock::new();
    S.get_or_init(|| {
        guardiana_lists::IA
            .lines()
            .filter_map(|l| l.trim().strip_prefix('@'))
            .map(|n| n.split('|').next().unwrap_or(n).trim())
            .filter(|n| !n.is_empty())
            .collect()
    })
}

/// The company one traffic measurement went to (see the module's head).
#[must_use]
pub fn empresa_de(destino: &str, asn: u32, red: &str) -> String {
    if red == LOCAL {
        return LOCAL.to_owned();
    }
    if !destino.is_empty() {
        if let Some(ia) = guardiana_lists::ai_service_of(destino) {
            return ia.to_owned();
        }
        if let Some(g) = crate::destinos::grupo_de(destino) {
            return g.to_owned();
        }
    }
    if let Some(n) = crate::destinos::nombre_red(asn) {
        return n;
    }
    if red == ANYCAST {
        return ANYCAST.to_owned();
    }
    red.to_owned()
}

/// Whether a measurement went to an address no name was seen for: a fixed address, the
/// program's own resolver (its own DoH), or QUIC to an address it learned elsewhere.
#[must_use]
pub fn sin_dns(f: &Trafico) -> bool {
    f.destino.is_empty() && f.red != LOCAL
}

/// What one take of traffic sent to each company outside the house, per program, for the
/// «posible transferencia» detector.
#[must_use]
pub fn envios_de(filas: &[Trafico], ts: i64) -> Vec<Envio> {
    let mut por: HashMap<(String, String), Envio> = HashMap::new();
    for f in filas {
        let empresa = empresa_de(&f.destino, f.asn, &f.red);
        if empresa == LOCAL || f.enviados == 0 {
            continue;
        }
        let e = por
            .entry((f.programa.clone(), empresa.clone()))
            .or_insert_with(|| Envio {
                ts,
                programa: f.programa.clone(),
                empresa,
                pais: f.pais.clone(),
                enviados: 0,
                recibidos: 0,
            });
        e.enviados += f.enviados;
        e.recibidos += f.recibidos;
        if e.pais != f.pais {
            e.pais.clear();
        }
    }
    let mut v: Vec<Envio> = por.into_values().collect();
    v.sort_by(|a, b| (&a.programa, &a.empresa).cmp(&(&b.programa, &b.empresa)));
    v
}

/// The rows of one hour of the chained summary, from that hour's traffic and warnings.
#[must_use]
pub fn filas_de_hora(
    trafico: &[Trafico],
    transferencias: &[PosibleTransferencia],
    hora: i64,
) -> Vec<FilaResumen> {
    let fin = hora + HOUR_MS;
    let mut por: HashMap<(String, String), FilaResumen> = HashMap::new();
    let mut paises: HashMap<(String, String), Option<String>> = HashMap::new();
    for f in trafico
        .iter()
        .filter(|f| f.minuto >= hora && f.minuto < fin)
    {
        let empresa = empresa_de(&f.destino, f.asn, &f.red);
        let k = (f.programa.clone(), empresa.clone());
        let r = por.entry(k.clone()).or_insert_with(|| FilaResumen {
            hora,
            programa: f.programa.clone(),
            empresa,
            ..FilaResumen::default()
        });
        r.enviados += f.enviados;
        r.recibidos += f.recibidos;
        r.reenviados += f.reenviados;
        r.conexiones += f.conexiones;
        if sin_dns(f) {
            r.sin_dns += f.enviados + f.recibidos;
        }
        // One country when every measurement agrees; several or none say nothing.
        let p = paises.entry(k).or_insert_with(|| Some(f.pais.clone()));
        if p.as_deref() != Some(f.pais.as_str()) {
            *p = None;
        }
    }
    for t in transferencias.iter().filter(|t| t.ts >= hora && t.ts < fin) {
        let k = (t.programa.clone(), t.empresa.clone());
        let r = por.entry(k).or_insert_with(|| FilaResumen {
            hora,
            programa: t.programa.clone(),
            empresa: t.empresa.clone(),
            pais: t.pais.clone(),
            ..FilaResumen::default()
        });
        r.transferencias += 1;
    }
    let mut filas: Vec<FilaResumen> = por
        .into_iter()
        .map(|(k, mut r)| {
            if let Some(Some(p)) = paises.get(&k) {
                r.pais.clone_from(p);
            }
            r
        })
        .collect();
    filas.sort_by(|a, b| {
        (b.enviados + b.recibidos, b.transferencias)
            .cmp(&(a.enviados + a.recibidos, a.transferencias))
            .then_with(|| (&a.programa, &a.empresa).cmp(&(&b.programa, &b.empresa)))
    });
    if filas.len() > MAX_FILAS_HORA {
        // A warning is never folded away: the rows with one stay whole.
        let (mut dentro, resto): (Vec<_>, Vec<_>) = filas
            .into_iter()
            .enumerate()
            .partition(|(i, f)| *i < MAX_FILAS_HORA || f.transferencias > 0);
        let mut otras = FilaResumen {
            hora,
            programa: OTRAS.to_owned(),
            empresa: OTRAS.to_owned(),
            ..FilaResumen::default()
        };
        for (_, f) in resto {
            otras.enviados += f.enviados;
            otras.recibidos += f.recibidos;
            otras.reenviados += f.reenviados;
            otras.conexiones += f.conexiones;
            otras.sin_dns += f.sin_dns;
        }
        dentro.push((usize::MAX, otras));
        filas = dentro.into_iter().map(|(_, f)| f).collect();
    }
    filas
}

/// The open hours, from `desde` to now, as rows like the summary's (not chained yet).
fn filas_abiertas(
    trafico: &[Trafico],
    transferencias: &[PosibleTransferencia],
    desde: i64,
    ahora: i64,
) -> Vec<FilaResumen> {
    let mut out = Vec::new();
    let mut h = hora_de(desde);
    while h <= ahora {
        out.extend(filas_de_hora(trafico, transferencias, h));
        h += HOUR_MS;
    }
    out
}

/// The most hours closed at once: after a long stop the minutes are gone anyway.
const MAX_HORAS_CIERRE: i64 = 48;

/// Close every complete hour not yet in the chained summary (the service calls this every few
/// seconds; it does nothing until an hour ends).
///
/// # Errors
/// Whatever the ledger says.
pub fn cierra_horas(l: &mut Ledger, ahora: i64) -> guardiana_core::Result<()> {
    let actual = hora_de(ahora);
    let hasta = l
        .vision_hasta()
        .unwrap_or(actual - 24 * HOUR_MS)
        .max(actual - MAX_HORAS_CIERRE * HOUR_MS);
    if hasta >= actual {
        return Ok(());
    }
    let trafico = l.trafico(hasta, actual, &[])?;
    let transferencias: Vec<PosibleTransferencia> = l
        .transferencias(hasta)?
        .into_iter()
        .filter(|t| t.ts < actual)
        .collect();
    let mut filas = Vec::new();
    let mut h = hasta;
    while h < actual {
        filas.extend(filas_de_hora(&trafico, &transferencias, h));
        h += HOUR_MS;
    }
    l.cierra_horas_vision(&filas, actual)
}

/// What a program is, for the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tipo {
    Antivirus(&'static antivirus::Antivirus),
    Ia(&'static str),
    Navegador,
    Otro,
}

fn tipo_de(programa: &str) -> Tipo {
    let p = programa.to_ascii_lowercase();
    if let Some(av) = antivirus::antivirus_de_programa(programa) {
        return Tipo::Antivirus(av);
    }
    if let Some((_, s)) = APPS_IA.iter().find(|(n, _)| *n == p) {
        return Tipo::Ia(s);
    }
    if NAVEGADORES.contains(&p.as_str()) {
        return Tipo::Navegador;
    }
    Tipo::Otro
}

/// Whether a company label is the maker's: the same name, or one inside the other (`OpenAI`
/// and `OpenAI (ChatGPT)`), compared without case.
fn es_de(empresa: &str, fabricante: &str) -> bool {
    let (a, b) = (empresa.to_lowercase(), fabricante.to_lowercase());
    if a.len() < 3 || b.len() < 3 || a.starts_with('@') {
        return false;
    }
    let primera = |s: &str| {
        s.split([' ', '(', ',', '.'])
            .find(|w| w.len() >= 3)
            .unwrap_or("")
            .to_owned()
    };
    a == b
        || a.contains(&b)
        || b.contains(&a)
        || (!primera(&b).is_empty() && primera(&a) == primera(&b))
}

fn del_fabricante(tipo: Tipo, empresa: &str) -> bool {
    match tipo {
        Tipo::Antivirus(av) => es_de(empresa, av.empresa) || es_de(empresa, av.nombre),
        Tipo::Ia(s) => es_de(empresa, s),
        Tipo::Navegador | Tipo::Otro => false,
    }
}

/// One company a program talked to.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct EmpresaVista {
    empresa: String,
    pais: String,
    enviados: u64,
    recibidos: u64,
    sin_dns: u64,
    reenviados: u64,
    conexiones: u64,
    /// Its own maker (an antivirus to its company, an AI app to its service).
    fabricante: bool,
    /// An AI service (the label is one of `ia.txt`).
    ia: bool,
}

/// One local day of a program's traffic.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct DiaVista {
    /// Local midnight, Unix ms.
    dia: i64,
    enviados: u64,
    /// Of `enviados`, to its own maker.
    al_fabricante: u64,
}

/// One program, as the page shows it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct ProgramaVista {
    programa: String,
    /// `antivirus`, `ia`, `navegador` or `otro`.
    tipo: &'static str,
    /// The antivirus product or the AI service, when it is one.
    producto: Option<&'static str>,
    /// The maker's company, for an antivirus.
    fabricante: Option<&'static str>,
    enviados: u64,
    recibidos: u64,
    reenviados: u64,
    conexiones: u64,
    sin_dns: u64,
    /// Bytes sent to its own maker.
    al_fabricante: u64,
    transferencias: u64,
    empresas: Vec<EmpresaVista>,
    /// How many companies in all (the list keeps the first ones).
    n_empresas: usize,
    /// Per local day, oldest first; only for the antivirus and the AI apps.
    por_dia: Vec<DiaVista>,
    /// AI services it talked to (a browser's AI is a website).
    ia: Vec<EmpresaVista>,
}

/// The most programs and companies per program on the page.
const MAX_PROGRAMAS: usize = 40;
const MAX_EMPRESAS: usize = 15;

fn agrupa(filas: &[FilaResumen], zona: i64) -> Vec<ProgramaVista> {
    struct Acc {
        empresas: HashMap<String, EmpresaVista>,
        dias: HashMap<i64, (u64, u64)>,
        transferencias: u64,
    }
    let mut por: HashMap<String, Acc> = HashMap::new();
    for f in filas.iter().filter(|f| f.programa != OTRAS) {
        let tipo = tipo_de(&f.programa);
        let a = por.entry(f.programa.clone()).or_insert_with(|| Acc {
            empresas: HashMap::new(),
            dias: HashMap::new(),
            transferencias: 0,
        });
        a.transferencias += f.transferencias;
        let fab = del_fabricante(tipo, &f.empresa);
        let e = a
            .empresas
            .entry(f.empresa.clone())
            .or_insert_with(|| EmpresaVista {
                empresa: f.empresa.clone(),
                pais: f.pais.clone(),
                enviados: 0,
                recibidos: 0,
                sin_dns: 0,
                reenviados: 0,
                conexiones: 0,
                fabricante: fab,
                ia: servicios_ia().contains(f.empresa.as_str()),
            });
        e.enviados += f.enviados;
        e.recibidos += f.recibidos;
        e.sin_dns += f.sin_dns;
        e.reenviados += f.reenviados;
        e.conexiones += f.conexiones;
        if e.pais != f.pais {
            e.pais.clear();
        }
        let dia =
            guardiana_core::local_midnight(guardiana_core::local_day_index(f.hora, zona), zona);
        let d = a.dias.entry(dia).or_insert((0, 0));
        d.0 += f.enviados;
        if fab {
            d.1 += f.enviados;
        }
    }
    let mut out: Vec<ProgramaVista> = por
        .into_iter()
        .map(|(programa, a)| {
            let tipo = tipo_de(&programa);
            let mut empresas: Vec<EmpresaVista> = a.empresas.into_values().collect();
            empresas.sort_by(|x, y| {
                (y.enviados + y.recibidos)
                    .cmp(&(x.enviados + x.recibidos))
                    .then_with(|| x.empresa.cmp(&y.empresa))
            });
            let suma = |f: fn(&EmpresaVista) -> u64| empresas.iter().map(f).sum::<u64>();
            let enviados = suma(|e| e.enviados);
            let recibidos = suma(|e| e.recibidos);
            let reenviados = suma(|e| e.reenviados);
            let conexiones = suma(|e| e.conexiones);
            let sin = suma(|e| e.sin_dns);
            let al_fabricante = empresas
                .iter()
                .filter(|e| e.fabricante)
                .map(|e| e.enviados)
                .sum();
            let ia: Vec<EmpresaVista> = if matches!(tipo, Tipo::Ia(_)) {
                Vec::new()
            } else {
                empresas.iter().filter(|e| e.ia).cloned().collect()
            };
            let n_empresas = empresas.len();
            empresas.truncate(MAX_EMPRESAS);
            let mut por_dia: Vec<DiaVista> = if matches!(tipo, Tipo::Antivirus(_) | Tipo::Ia(_)) {
                a.dias
                    .into_iter()
                    .map(|(dia, (e, f))| DiaVista {
                        dia,
                        enviados: e,
                        al_fabricante: f,
                    })
                    .collect()
            } else {
                Vec::new()
            };
            por_dia.sort_by_key(|d| d.dia);
            let (tipo_txt, producto, fabricante) = match tipo {
                Tipo::Antivirus(av) => ("antivirus", Some(av.nombre), Some(av.empresa)),
                Tipo::Ia(s) => ("ia", Some(s), None),
                Tipo::Navegador => ("navegador", None, None),
                Tipo::Otro => ("otro", None, None),
            };
            ProgramaVista {
                programa,
                tipo: tipo_txt,
                producto,
                fabricante,
                enviados,
                recibidos,
                reenviados,
                conexiones,
                sin_dns: sin,
                al_fabricante,
                transferencias: a.transferencias,
                empresas,
                n_empresas,
                por_dia,
                ia,
            }
        })
        .collect();
    let peso = |p: &ProgramaVista| match p.tipo {
        "antivirus" => 0,
        "ia" => 1,
        _ if !p.ia.is_empty() => 2,
        _ => 3,
    };
    out.sort_by(|a, b| {
        (b.transferencias > 0)
            .cmp(&(a.transferencias > 0))
            .then_with(|| peso(a).cmp(&peso(b)))
            .then_with(|| (b.enviados + b.recibidos).cmp(&(a.enviados + a.recibidos)))
            .then_with(|| a.programa.cmp(&b.programa))
    });
    out.truncate(MAX_PROGRAMAS);
    out
}

/// A «posible transferencia» as the page shows it, with its sentence.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct TransferenciaVista {
    #[serde(flatten)]
    t: PosibleTransferencia,
    frase: String,
}

fn megas(b: u64) -> String {
    #[allow(clippy::cast_precision_loss)]
    let mb = b as f64 / (1024.0 * 1024.0);
    if mb >= 10.0 {
        format!("{mb:.0} MB")
    } else {
        format!("{mb:.1} MB")
    }
}

/// «Posible transferencia: Sync.exe leyó 3 archivos (informe.pdf…) y envió 41 MB a Acme
/// (Estados Unidos) en 2 min». Always «posible»; «leyó» only when reads were seen, «abrió»
/// otherwise; the names only when they are still kept.
pub(crate) fn frase_de(t: &Texts, p: &PosibleTransferencia) -> String {
    let empresa = match p.empresa.as_str() {
        "" => t.panel("vp_empresa_desconocida").to_owned(),
        e => e.to_owned(),
    };
    let pais = if p.pais.is_empty() {
        String::new()
    } else {
        let n = t.pais(&p.pais);
        format!(" ({})", if n.is_empty() { p.pais.as_str() } else { n })
    };
    let verbo = match (p.leidos > 0, p.n_archivos == 1) {
        (true, false) => "vp_frase_leyo",
        (true, true) => "vp_frase_leyo_1",
        (false, false) => "vp_frase_abrio",
        (false, true) => "vp_frase_abrio_1",
    };
    let archivos = if p.archivos.is_empty() {
        String::new()
    } else {
        let mut s = p
            .archivos
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        if p.n_archivos > 3 {
            s.push('…');
        }
        format!(" ({s})")
    };
    let minutos = ((p.ts - p.desde).max(0) + 59_999) / 60_000;
    t.panel(verbo)
        .replace("{programa}", &p.programa)
        .replace("{n}", &p.n_archivos.to_string())
        .replace("{archivos}", &archivos)
        .replace("{mb}", &megas(p.enviados))
        .replace("{empresa}", &empresa)
        .replace("{pais}", &pais)
        .replace("{min}", &minutos.max(1).to_string())
}

/// Defender's cloud settings, asked of Windows at most every ten minutes (each is a `reg.exe`).
fn defender() -> guardiana_apps::DefenderNube {
    static CACHE: Mutex<Option<(i64, guardiana_apps::DefenderNube)>> = Mutex::new(None);
    let ahora = now_ms();
    if let Ok(c) = CACHE.lock() {
        if let Some((cuando, v)) = *c {
            if ahora - cuando < 10 * 60_000 {
                return v;
            }
        }
    }
    let leer = guardiana_apps::defender_nube;
    let v = match tokio::runtime::Handle::try_current().map(|h| h.runtime_flavor()) {
        Ok(tokio::runtime::RuntimeFlavor::MultiThread) => tokio::task::block_in_place(leer),
        _ => leer(),
    };
    if let Ok(mut c) = CACHE.lock() {
        *c = Some((ahora, v));
    }
    v
}

/// What the page needs.
#[derive(Serialize)]
pub(crate) struct Vista {
    /// This system measures per program (Windows). Elsewhere the page says «no disponible
    /// todavía» and nothing else is filled.
    disponible: bool,
    /// The file-name watch is on.
    archivos: bool,
    /// Days asked for.
    dias: i64,
    /// Hours the figures actually cover (the traffic minutes follow the detail's retention, the
    /// hourly summary 90 days).
    horas: i64,
    programas: Vec<ProgramaVista>,
    transferencias: Vec<TransferenciaVista>,
    /// Totals of everything above.
    enviados: u64,
    recibidos: u64,
    sin_dns: u64,
    reenviados: u64,
    /// Microsoft Defender's cloud settings, when Windows says them.
    defender: guardiana_apps::DefenderNube,
    /// The chained hourly summary checks out.
    cadena: guardiana_core::vision::CadenaVision,
}

fn dias_de(q: &HashMap<String, String>) -> i64 {
    q.get("dias")
        .and_then(|d| d.parse::<i64>().ok())
        .unwrap_or(1)
        .clamp(1, 30)
}

/// `/api/vision?dias=N`.
pub(crate) async fn vista(
    Lang(t): Lang,
    Zona(zona): Zona,
    State(state): State<Arc<AppState>>,
    Query(q): Query<HashMap<String, String>>,
    _s: Session,
) -> ApiResult<Vista> {
    let dias = dias_de(&q);
    let ahora = now_ms();
    let desde = ahora - dias * DAY_MS;
    let (filas, transferencias, archivos, cadena) = with_ledger(&state, |l| {
        let hasta = l.vision_hasta().unwrap_or(desde).max(desde);
        let mut filas = l.resumen_vision(desde, hasta)?;
        let trafico = l.trafico(hasta, ahora + 1, &[])?;
        let transf = l.transferencias(desde)?;
        filas.extend(filas_abiertas(&trafico, &transf, hasta, ahora));
        Ok((filas, transf, l.vision_archivos(), l.comprueba_vision()?))
    })?;
    let horas = filas
        .iter()
        .map(|f| f.hora)
        .min()
        .map_or(0, |m| (ahora - m) / HOUR_MS + 1);
    let programas = agrupa(&filas, zona);
    let suma = |f: fn(&ProgramaVista) -> u64| programas.iter().map(f).sum::<u64>();
    Ok(axum::Json(Vista {
        disponible: cfg!(windows),
        archivos,
        dias,
        horas,
        enviados: suma(|p| p.enviados),
        recibidos: suma(|p| p.recibidos),
        sin_dns: suma(|p| p.sin_dns),
        reenviados: suma(|p| p.reenviados),
        transferencias: transferencias
            .into_iter()
            .map(|p| TransferenciaVista {
                frase: frase_de(t, &p),
                t: p,
            })
            .collect(),
        programas,
        defender: defender(),
        cadena,
    }))
}

/// One company of a program, for the globe's inspector: where its servers are, by the place
/// keys the globe already knows (`aws:us-east-1`, cities), and the bytes each way.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct GloboEmpresa {
    empresa: String,
    pais: String,
    lugares: Vec<String>,
    enviados: u64,
    recibidos: u64,
    sin_dns: u64,
}

fn globo_de(trafico: &[Trafico], programa: &str) -> Vec<GloboEmpresa> {
    let mut por: HashMap<String, GloboEmpresa> = HashMap::new();
    for f in trafico
        .iter()
        .filter(|f| f.programa.eq_ignore_ascii_case(programa))
    {
        let empresa = empresa_de(&f.destino, f.asn, &f.red);
        if empresa == LOCAL {
            continue;
        }
        let g = por.entry(empresa.clone()).or_insert_with(|| GloboEmpresa {
            empresa,
            pais: f.pais.clone(),
            lugares: Vec::new(),
            enviados: 0,
            recibidos: 0,
            sin_dns: 0,
        });
        g.enviados += f.enviados;
        g.recibidos += f.recibidos;
        if sin_dns(f) {
            g.sin_dns += f.enviados + f.recibidos;
        }
        if g.pais != f.pais {
            g.pais.clear();
        }
        if !f.lugar.is_empty() && !g.lugares.contains(&f.lugar) && g.lugares.len() < 8 {
            g.lugares.push(f.lugar.clone());
        }
    }
    let mut v: Vec<GloboEmpresa> = por.into_values().collect();
    v.sort_by(|a, b| {
        (b.enviados + b.recibidos)
            .cmp(&(a.enviados + a.recibidos))
            .then_with(|| a.empresa.cmp(&b.empresa))
    });
    v.truncate(30);
    v
}

/// `/api/vision/globo?programa=X`: bytes per company for one program, last 24 hours.
pub(crate) async fn globo(
    State(state): State<Arc<AppState>>,
    Query(q): Query<HashMap<String, String>>,
    _s: Session,
) -> ApiResult<serde_json::Value> {
    let programa = q.get("programa").cloned().unwrap_or_default();
    let ahora = now_ms();
    let trafico = with_ledger(&state, |l| l.trafico(ahora - DAY_MS, ahora + 1, &[]))?;
    Ok(axum::Json(serde_json::json!({
        "programa": programa,
        "empresas": globo_de(&trafico, &programa),
    })))
}

#[derive(serde::Deserialize)]
pub(crate) struct ArchivosBody {
    activa: bool,
}

/// `/api/vision/archivos`: the file-name watch on or off. Off deletes the names kept.
pub(crate) async fn archivos(
    State(state): State<Arc<AppState>>,
    _s: Session,
    axum::Json(body): axum::Json<ArchivosBody>,
) -> ApiResult<serde_json::Value> {
    with_ledger(&state, |l| l.pon_vision_archivos(body.activa))?;
    Ok(axum::Json(serde_json::json!({ "archivos": body.activa })))
}

/// The page's own script and style sheet (1.0.45): apart from `app.js`, so the other pages do not
/// carry their weight, and small enough to be asked for every time.
const VISION_JS: &str = include_str!("../static/vision.js");
const VISION_CSS: &str = include_str!("../static/vision.css");

/// Every route of «Visión profunda», merged into the panel's router.
pub(crate) fn rutas() -> axum::Router<Arc<AppState>> {
    use axum::routing::{get, post};
    axum::Router::new()
        .route(
            "/vision",
            get(|| async { crate::pagina(include_str!("../static/vision.html")) }),
        )
        .route(
            "/vision.html",
            get(|| async { crate::pagina(include_str!("../static/vision.html")) }),
        )
        .route(
            "/static/vision.js",
            get(|| async {
                crate::static_response(VISION_JS, "application/javascript; charset=utf-8")
            }),
        )
        .route(
            "/static/vision.css",
            get(|| async { crate::static_response(VISION_CSS, "text/css; charset=utf-8") }),
        )
        .route("/api/vision", get(vista))
        .route("/api/vision/globo", get(globo))
        .route("/api/vision/archivos", post(archivos))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const MB: u64 = 1024 * 1024;

    fn tr(minuto: i64, programa: &str, destino: &str, red: &str, env: u64, rec: u64) -> Trafico {
        Trafico {
            minuto,
            programa: programa.into(),
            destino: destino.into(),
            red: red.into(),
            pais: "US".into(),
            proto: "tcp".into(),
            puerto: 443,
            enviados: env,
            recibidos: rec,
            ..Trafico::default()
        }
    }

    #[test]
    fn an_ai_name_keeps_its_service_and_a_local_address_is_the_house() {
        assert_eq!(empresa_de("chatgpt.com", 0, ""), "OpenAI (ChatGPT)");
        assert_eq!(empresa_de("", 0, LOCAL), LOCAL);
        assert_eq!(empresa_de("", 0, "Some Net"), "Some Net");
        assert_eq!(empresa_de("", 0, ""), "");
    }

    #[test]
    fn an_hour_adds_up_per_program_and_company_and_counts_what_had_no_name() {
        let h = 10 * HOUR_MS;
        let filas = filas_de_hora(
            &[
                tr(h, "a.exe", "", "Net X", 100, 10),
                tr(h + 60_000, "a.exe", "", "Net X", 50, 5),
                tr(h, "a.exe", "", LOCAL, 7, 7),
                tr(h + HOUR_MS, "a.exe", "", "Net X", 1, 1),
            ],
            &[],
            h,
        );
        assert_eq!(filas.len(), 2);
        let x = filas.iter().find(|f| f.empresa == "Net X").unwrap();
        assert_eq!((x.enviados, x.recibidos, x.sin_dns), (150, 15, 165));
        assert_eq!(x.pais, "US");
        let l = filas.iter().find(|f| f.empresa == LOCAL).unwrap();
        assert_eq!(
            l.sin_dns, 0,
            "the house's own network is not «without a name»"
        );
    }

    #[test]
    fn past_the_limit_the_rest_folds_into_one_row_but_never_a_warning() {
        let h = 0;
        let trafico: Vec<Trafico> = (0..(MAX_FILAS_HORA as u64 + 10))
            .map(|i| tr(h, &format!("p{i}.exe"), "", "Net", 1_000 + i, 0))
            .collect();
        let aviso = PosibleTransferencia {
            id: 1,
            ts: h + 1,
            desde: h,
            programa: "p0.exe".into(),
            empresa: "Net".into(),
            pais: "US".into(),
            enviados: 1,
            recibidos: 0,
            n_archivos: 1,
            archivos: vec![],
            leidos: 0,
        };
        let filas = filas_de_hora(&trafico, &[aviso], h);
        // The first rows, the one with a warning (the smallest, past the limit) and the sum.
        assert_eq!(filas.len(), MAX_FILAS_HORA + 2);
        assert!(filas
            .iter()
            .any(|f| f.programa == "p0.exe" && f.transferencias == 1));
        let otras = filas.iter().find(|f| f.programa == OTRAS).unwrap();
        assert!(otras.enviados > 0);
    }

    #[test]
    fn sends_are_grouped_per_program_and_company_and_the_house_stays_out() {
        let e = envios_de(
            &[
                tr(0, "a.exe", "", "Net X", 3 * MB, 0),
                tr(0, "a.exe", "", "Net X", 2 * MB, 0),
                tr(0, "a.exe", "", LOCAL, 9 * MB, 0),
            ],
            42,
        );
        assert_eq!(e.len(), 1);
        assert_eq!((e[0].enviados, e[0].ts), (5 * MB, 42));
    }

    #[test]
    fn the_antivirus_and_the_ai_apps_come_first_with_what_went_to_their_maker() {
        let prog = "MsMpEng.exe";
        let av = antivirus::antivirus_de_programa(prog).unwrap();
        let filas = vec![
            FilaResumen {
                hora: 0,
                programa: "grande.exe".into(),
                empresa: "Net".into(),
                enviados: 900 * MB,
                ..FilaResumen::default()
            },
            FilaResumen {
                hora: 0,
                programa: prog.into(),
                empresa: av.empresa.into(),
                enviados: 5,
                ..FilaResumen::default()
            },
            FilaResumen {
                hora: 0,
                programa: prog.into(),
                empresa: "Otra Net".into(),
                enviados: 3,
                ..FilaResumen::default()
            },
            FilaResumen {
                hora: 0,
                programa: "ChatGPT.exe".into(),
                empresa: "OpenAI (ChatGPT)".into(),
                enviados: 2,
                ..FilaResumen::default()
            },
            FilaResumen {
                hora: 0,
                programa: "chrome.exe".into(),
                empresa: "Google Gemini".into(),
                enviados: 1,
                ..FilaResumen::default()
            },
        ];
        let p = agrupa(&filas, 0);
        assert_eq!(p[0].programa, prog);
        assert_eq!(p[0].tipo, "antivirus");
        assert_eq!((p[0].enviados, p[0].al_fabricante), (8, 5));
        assert_eq!(p[0].por_dia.len(), 1);
        assert_eq!(p[0].por_dia[0].al_fabricante, 5);
        assert_eq!(p[1].tipo, "ia");
        assert_eq!(p[1].al_fabricante, 2);
        assert_eq!(p[2].programa, "chrome.exe");
        assert_eq!(p[2].ia.len(), 1, "a browser's AI is told apart");
        assert!(p[2].por_dia.is_empty());
        assert_eq!(p[3].programa, "grande.exe");
    }

    #[test]
    fn the_sentence_says_possible_never_more_and_reads_only_when_read() {
        let mut p = PosibleTransferencia {
            id: 1,
            ts: 120_000,
            desde: 0,
            programa: "Sync.exe".into(),
            empresa: "Acme".into(),
            pais: "US".into(),
            enviados: 41 * MB,
            recibidos: 0,
            n_archivos: 4,
            archivos: vec![
                "informe.pdf".into(),
                "b.txt".into(),
                "c.txt".into(),
                "d.txt".into(),
            ],
            leidos: 0,
        };
        for (lang, t) in [
            ("es", guardiana_core::i18n::es()),
            ("en", guardiana_core::i18n::en()),
            ("pt", guardiana_core::i18n::pt()),
        ] {
            let f = frase_de(t, &p);
            assert!(
                f.contains("Sync.exe") && f.contains("41 MB") && f.contains("Acme"),
                "{lang}: {f}"
            );
            assert!(f.contains("informe.pdf, b.txt, c.txt…"), "{lang}: {f}");
            assert!(!f.contains('{'), "{lang}: {f}");
        }
        let es = guardiana_core::i18n::es();
        assert!(frase_de(es, &p).starts_with("Posible transferencia"));
        assert!(frase_de(es, &p).contains("abrió 4 archivos"));
        assert!(frase_de(es, &p).contains("en 2 min"));
        p.leidos = 10;
        p.archivos.clear();
        let f = frase_de(es, &p);
        assert!(f.contains("leyó 4 archivos y envió"), "{f}");
        for prohibida in ["robó", "subió tus", "robou", "stole"] {
            for t in [es, guardiana_core::i18n::en(), guardiana_core::i18n::pt()] {
                assert!(!frase_de(t, &p).to_lowercase().contains(prohibida));
            }
        }
    }

    #[test]
    fn the_globe_gets_bytes_and_places_per_company_of_one_program() {
        let mut a = tr(0, "a.exe", "", "Net X", 10, 1);
        a.lugar = "aws:us-east-1".into();
        let g = globo_de(
            &[
                a,
                tr(0, "A.EXE", "", "Net X", 5, 0),
                tr(0, "b.exe", "", "Net X", 99, 0),
            ],
            "a.exe",
        );
        assert_eq!(g.len(), 1);
        assert_eq!((g[0].enviados, g[0].sin_dns), (15, 16));
        assert_eq!(g[0].lugares, vec!["aws:us-east-1"]);
    }

    /// The page's texts exist in the three languages (the shared test in lib.rs reads the
    /// other pages; this one reads «Visión profunda»'s own).
    #[test]
    fn every_text_of_the_page_exists_in_three_languages() {
        let mut claves: Vec<String> = Vec::new();
        for parte in include_str!("../static/vision.html")
            .split("data-t=\"")
            .skip(1)
        {
            if let Some(k) = parte.split('"').next() {
                claves.push(k.to_owned());
            }
        }
        let js = include_str!("../static/vision.js");
        for (i, _) in js.match_indices("t('") {
            let antes = js
                .as_bytes()
                .get(i.wrapping_sub(1))
                .copied()
                .unwrap_or(b' ');
            if antes.is_ascii_alphanumeric() || antes == b'_' || antes == b'.' {
                continue;
            }
            if let Some(k) = js[i + 3..].split('\'').next() {
                if !k.is_empty() && !k.ends_with('_') {
                    claves.push(k.to_owned());
                }
            }
        }
        for k in [
            "vp_frase_leyo",
            "vp_frase_abrio",
            "vp_frase_leyo_1",
            "vp_frase_abrio_1",
            "vp_empresa_desconocida",
        ] {
            claves.push(k.to_owned());
        }
        assert!(claves.len() > 20, "{}", claves.len());
        for (name, json) in [
            ("es", guardiana_core::i18n::ES_JSON),
            ("en", guardiana_core::i18n::EN_JSON),
            ("pt", guardiana_core::i18n::PT_JSON),
        ] {
            let v: serde_json::Value = serde_json::from_str(json).unwrap();
            let panel = v["panel"].as_object().unwrap();
            let faltan: Vec<&String> = claves.iter().filter(|k| !panel.contains_key(*k)).collect();
            assert!(faltan.is_empty(), "{name}: faltan {faltan:?}");
        }
    }
}
