//! «Audita a GUARDIANA» (1.0.46, Privacidad): the person audits the program that audits their
//! computer, in one click and at the free level too.
//!
//! 1. The installed program is the published one: the SHA-256 of the running binary (and of the
//!    other programs in its folder, on Windows) against the public record `ledger.jsonl` and its
//!    Rekor entry, with the signature check (`guardiana_verify`, the same as «Verificar»).
//! 2. Every connection GUARDIANA itself made, with its date, from its own records: the service's
//!    `outbound` table (licence, version checks) and GUARDIANA ZERO's (licence, version, the
//!    Comunicaciones relay and direct links), read without changing them.
//! 3. What it keeps on disk: the files of its data folder with their sizes, and every table of
//!    the ledger with its columns, rows and bytes, with the count of stored values that are an
//!    internet address (`guardiana_core::inventario`), so «no IPs» is measured, not promised.
//! 4. The report, as JSON and as a page to print, sealed the way the «Certificado» is sealed: the
//!    SHA-256 of the exact JSON bytes, written in `sha256sum` format, and the head of the ledger's
//!    hash chain at that moment. There is no private key on this computer, so it is «sealed», not
//!    «signed», and the page and the report say so.

// Handlers answer with the same error type as the rest of the API (see api.rs).
#![allow(clippy::result_large_err)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use guardiana_core::i18n::{self, Texts};
use guardiana_core::inventario::{self, Inventario};
use guardiana_core::time::{now_ms, rfc3339_utc};
use guardiana_core::Outbound;
use guardiana_verify::certificado;
use guardiana_verify::certificado_html::miles;
use guardiana_verify::{LedgerMatch, SignatureState};
use serde::Serialize;

use crate::api::{with_ledger, ApiResult};
use crate::auth::{Lang, Session};
use crate::AppState;

const HTML: &str = include_str!("../static/audita.html");
const JS: &str = include_str!("../static/audita.js");

/// The format name written in every report.
pub(crate) const FORMATO: &str = "guardiana-auditoria/1";
/// Where the public code lives.
pub(crate) const CODIGO: &str = "https://github.com/guardianagroup/guardiana";
/// The public page that explains every audit.
pub(crate) const TRANSPARENCIA: &str = "https://guardianagroup.com/transparencia";
/// The most connections a report lists one by one (all of them are counted).
const MAX_CONEXIONES: usize = 2000;
/// The most files of the data folder listed.
const MAX_ARCHIVOS: usize = 300;

/// The routes of this page, merged into the panel's router. Not in `plan::RUTAS_PLUS`: the free
/// level audits too.
pub(crate) fn rutas() -> Router<Arc<AppState>> {
    let pagina = || async { crate::pagina(HTML) };
    Router::new()
        .route("/audita", get(pagina))
        .route("/audita.html", get(pagina))
        .route(
            "/static/audita.js",
            get(|| async { crate::static_response(JS, "application/javascript; charset=utf-8") }),
        )
        .route("/api/audita", get(auditar))
}

/// A program file of the installation and where it stands in the public record.
#[derive(Serialize, Debug, Clone)]
pub(crate) struct Archivo {
    nombre: String,
    sha256: String,
    registro: LedgerMatch,
}

/// Part 1: the installed program against the published one.
#[derive(Serialize, Debug, Clone)]
pub(crate) struct Instalado {
    version: String,
    binario: String,
    sha256: String,
    firma: SignatureState,
    registro: LedgerMatch,
    /// On this system the record carries the downloaded .zip, not the program inside it (macOS).
    registro_zip: bool,
    /// The Rekor entry of the matching release, to look up on the public log.
    rekor_url: Option<String>,
    /// The other program files of the same folder (Windows), each against the record.
    otros: Vec<Archivo>,
    /// The record file was found on this computer.
    registro_local: bool,
    /// Every program file found is in the public record.
    coincide: bool,
}

/// One connection GUARDIANA made.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct Conexion {
    ts: i64,
    utc: String,
    /// `licencia`, `version`, `listas`, `rele`, `directo`.
    motivo: String,
    /// To whom: a name, never an address.
    host: String,
    bytes: i64,
    /// The person asked for it (an activation, a call) or it was due (a periodic check).
    pedida: bool,
    /// `servicio` (the GUARDIANA service) or `zero` (GUARDIANA ZERO).
    origen: &'static str,
}

/// Part 2: every connection, from GUARDIANA's own records.
#[derive(Serialize, Debug, Clone, Default)]
pub(crate) struct Conexiones {
    total: usize,
    /// Per reason: (reason, how many).
    por_motivo: Vec<(String, usize)>,
    lista: Vec<Conexion>,
    /// GUARDIANA ZERO files read (one per account that uses it).
    archivos_zero: usize,
}

/// Part 3: what GUARDIANA keeps on disk.
#[derive(Serialize, Debug, Clone)]
pub(crate) struct Disco {
    carpeta: String,
    archivos: Vec<(String, u64)>,
    bytes_carpeta: u64,
    base: Inventario,
}

/// Where the report stands in the ledger's chain.
#[derive(Serialize, Debug, Clone)]
pub(crate) struct Cadena {
    algoritmo: String,
    cabeza: Option<String>,
    comprobada: bool,
    filas_comprobadas: u64,
}

/// The report.
#[derive(Serialize, Debug, Clone)]
pub(crate) struct Informe {
    formato: String,
    programa: String,
    generado: i64,
    generado_utc: String,
    sistema: String,
    /// Sealed (SHA-256), not signed: there is no private key on this computer.
    sello: String,
    instalado: Instalado,
    conexiones: Conexiones,
    disco: Disco,
    cadena: Cadena,
    transparencia: String,
    codigo: String,
}

/// What the page gets: the report, its exact bytes, the seal and the files to save.
#[derive(Serialize, Debug)]
pub(crate) struct AuditaVista {
    informe: Informe,
    json: String,
    sello: String,
    linea_sha256: String,
    html: String,
    archivo_json: String,
    archivo_html: String,
    archivo_sha256: String,
}

fn sha256_de(ruta: &Path) -> Option<String> {
    std::fs::read(ruta).ok().map(|b| certificado::sello(&b))
}

/// The other program files in the binary's folder (Windows: `.exe`), each against the record.
fn otros_archivos(binario: &Path, registro: Option<&str>) -> Vec<Archivo> {
    if !cfg!(windows) {
        return Vec::new();
    }
    let Some(dir) = binario.parent() else {
        return Vec::new();
    };
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in rd.flatten().take(200) {
        let p = e.path();
        let es_exe = p.extension().is_some_and(|x| x.eq_ignore_ascii_case("exe"));
        if !es_exe || p == binario {
            continue;
        }
        if !e.metadata().is_ok_and(|m| m.len() <= 400 * 1024 * 1024) {
            continue;
        }
        let Some(sha) = sha256_de(&p) else {
            continue;
        };
        out.push(Archivo {
            nombre: e.file_name().to_string_lossy().into_owned(),
            registro: registro.map_or(LedgerMatch::NoLedgerFile, |t| {
                guardiana_verify::match_in_ledger(t, &sha)
            }),
            sha256: sha,
        });
        if out.len() >= 10 {
            break;
        }
    }
    out.sort_by(|a, b| a.nombre.cmp(&b.nombre));
    out
}

fn instalado() -> Instalado {
    let r = guardiana_verify::run();
    let binario = PathBuf::from(&r.binary);
    let texto =
        guardiana_verify::find_ledger_file(&binario).and_then(|p| std::fs::read_to_string(p).ok());
    let otros = otros_archivos(&binario, texto.as_deref());
    let rekor_url = match &r.ledger {
        LedgerMatch::Found { rekor_uuid, .. } if !rekor_uuid.is_empty() => {
            Some(format!("https://search.sigstore.dev/?uuid={rekor_uuid}"))
        }
        _ => None,
    };
    let encontrado = |m: &LedgerMatch| matches!(m, LedgerMatch::Found { .. });
    Instalado {
        coincide: encontrado(&r.ledger) && otros.iter().all(|o| encontrado(&o.registro)),
        version: r.version,
        binario: r.binary,
        sha256: r.sha256,
        firma: r.signature,
        registro_local: texto.is_some(),
        registro: r.ledger,
        registro_zip: r.ledger_records_zip,
        rekor_url,
        otros,
    }
}

/// GUARDIANA ZERO's own files, one per account that uses it (Windows).
fn archivos_zero() -> Vec<PathBuf> {
    if !cfg!(windows) {
        return Vec::new();
    }
    crate::extensiones::hogares()
        .into_iter()
        .map(|h| {
            h.join("AppData")
                .join("Local")
                .join("GUARDIANA ZERO")
                .join("datos")
                .join("licencia.db")
        })
        .filter(|p| p.is_file())
        .collect()
}

fn conexion(o: Outbound, origen: &'static str) -> Conexion {
    Conexion {
        utc: rfc3339_utc(o.ts),
        ts: o.ts,
        motivo: o.purpose.as_str().to_owned(),
        host: o.host,
        bytes: o.bytes,
        pedida: o.initiated_by_user,
        origen,
    }
}

/// Every connection, newest first, from the service's records and ZERO's.
pub(crate) fn junta_conexiones(propias: Vec<Outbound>, zero: Vec<Vec<Outbound>>) -> Conexiones {
    let archivos_zero = zero.len();
    let mut lista: Vec<Conexion> = propias
        .into_iter()
        .map(|o| conexion(o, "servicio"))
        .chain(zero.into_iter().flatten().map(|o| conexion(o, "zero")))
        .collect();
    lista.sort_by(|a, b| b.ts.cmp(&a.ts).then_with(|| a.host.cmp(&b.host)));
    let mut por: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for c in &lista {
        *por.entry(c.motivo.clone()).or_default() += 1;
    }
    let total = lista.len();
    lista.truncate(MAX_CONEXIONES);
    Conexiones {
        total,
        por_motivo: por.into_iter().collect(),
        lista,
        archivos_zero,
    }
}

/// The files of the data folder, two levels down, with their sizes.
fn archivos_de(carpeta: &Path) -> (Vec<(String, u64)>, u64) {
    let mut out = Vec::new();
    let mut total = 0u64;
    let mut pendientes = vec![(carpeta.to_path_buf(), 0u8)];
    while let Some((dir, nivel)) = pendientes.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            let Ok(m) = e.metadata() else {
                continue;
            };
            if m.is_dir() {
                if nivel < 2 {
                    pendientes.push((p, nivel + 1));
                }
                continue;
            }
            total += m.len();
            if out.len() < MAX_ARCHIVOS {
                let rel = p
                    .strip_prefix(carpeta)
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((rel, m.len()));
            }
        }
    }
    out.sort();
    (out, total)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn bytes_texto(b: u64, idioma: &str) -> String {
    if b < 1024 {
        return format!("{} B", miles(b, idioma));
    }
    let kb = b.div_ceil(1024);
    if kb < 10 * 1024 {
        return format!("{} KB", miles(kb, idioma));
    }
    format!("{} MB", miles(b.div_ceil(1024 * 1024), idioma))
}

/// A row of the record's verdict, in words.
fn registro_texto(t: &Texts, m: &LedgerMatch) -> String {
    match m {
        LedgerMatch::Found { version, date, .. } => t
            .panel("au_reg_coincide")
            .replace("{v}", version)
            .replace("{fecha}", date.get(..10).unwrap_or(date)),
        LedgerMatch::NotFound => t.panel("au_reg_no").to_owned(),
        LedgerMatch::NoLedgerFile => t.panel("au_reg_sin_archivo").to_owned(),
        LedgerMatch::ZipOnly { .. } => t.panel("au_reg_zip").to_owned(),
    }
}

/// The report as a page to read and print: no script, nothing fetched, the seal on it.
pub(crate) fn html(t: &'static Texts, i: &Informe) -> String {
    let idioma = i18n::code_of(t);
    let mut h = String::new();
    h.push_str(&format!(
        "<!doctype html><html lang=\"{idioma}\"><head><meta charset=\"utf-8\"><title>{}</title><style>{}</style></head><body>",
        esc(t.panel("au_inf_titulo")),
        "body{font:14px/1.5 system-ui,sans-serif;color:#111;max-width:900px;margin:32px auto;padding:0 16px}h1{font-size:24px;margin:0}h2{font-size:16px;margin:28px 0 8px;border-bottom:1px solid #ccc;padding-bottom:4px}table{width:100%;border-collapse:collapse;font-size:12.5px}th,td{text-align:left;border-bottom:1px solid #e3e3e3;padding:4px 6px;vertical-align:top}td.n{text-align:right;white-space:nowrap}.m{font-family:ui-monospace,Menlo,Consolas,monospace;font-size:11.5px;word-break:break-all}.s{color:#555}.ok{color:#0a6b47}.no{color:#a33}.sello{border:1px solid #999;padding:10px 12px;margin-top:20px}"
    ));
    h.push_str(&format!(
        "<h1>{}</h1><p class=\"s\">{} · GUARDIANA {} · {}</p>",
        esc(t.panel("au_inf_titulo")),
        esc(&i.generado_utc),
        esc(&i.programa),
        esc(&i.sistema)
    ));
    // 1. Installed = published.
    let ins = &i.instalado;
    h.push_str(&format!("<h2>1. {}</h2>", esc(t.panel("au_inst_titulo"))));
    h.push_str(&format!(
        "<p class=\"{}\">{}</p>",
        if ins.coincide { "ok" } else { "no" },
        esc(t.panel(if ins.coincide {
            "au_inst_coincide"
        } else {
            "au_inst_no_coincide"
        }))
    ));
    h.push_str("<table><tr><th>");
    h.push_str(&esc(t.panel("au_col_archivo")));
    h.push_str("</th><th>SHA-256</th><th>");
    h.push_str(&esc(t.panel("au_col_registro")));
    h.push_str("</th></tr>");
    h.push_str(&format!(
        "<tr><td>{}</td><td class=\"m\">{}</td><td>{}</td></tr>",
        esc(&ins.binario),
        esc(&ins.sha256),
        esc(&registro_texto(t, &ins.registro))
    ));
    for o in &ins.otros {
        h.push_str(&format!(
            "<tr><td>{}</td><td class=\"m\">{}</td><td>{}</td></tr>",
            esc(&o.nombre),
            esc(&o.sha256),
            esc(&registro_texto(t, &o.registro))
        ));
    }
    h.push_str("</table>");
    if let Some(u) = &ins.rekor_url {
        h.push_str(&format!(
            "<p>Rekor: <span class=\"m\">{}</span></p>",
            esc(u)
        ));
    }
    // 2. Connections.
    let c = &i.conexiones;
    h.push_str(&format!("<h2>2. {}</h2>", esc(t.panel("au_con_titulo"))));
    h.push_str(&format!(
        "<p>{}</p>",
        esc(&t
            .panel(if c.total == 1 && t.panel_has("au_con_total_uno") {
                "au_con_total_uno"
            } else {
                "au_con_total"
            })
            .replace("{n}", &miles(u64::try_from(c.total).unwrap_or(0), idioma)))
    ));
    h.push_str(&format!(
        "<table><tr><th>{}</th><th>{}</th><th>{}</th><th>{}</th><th>{}</th></tr>",
        esc(t.panel("au_col_fecha")),
        esc(t.panel("au_col_motivo")),
        esc(t.panel("au_col_destino")),
        esc(t.panel("au_col_bytes")),
        esc(t.panel("au_col_quien"))
    ));
    for x in &c.lista {
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td class=\"m\">{}</td><td class=\"n\">{}</td><td>{}</td></tr>",
            esc(&x.utc),
            esc(t.panel(&format!("proposito_{}", x.motivo))),
            esc(&x.host),
            miles(u64::try_from(x.bytes).unwrap_or(0), idioma),
            esc(t.panel(if x.pedida { "au_pedida" } else { "au_programada" }))
        ));
    }
    h.push_str("</table>");
    // 3. Disk.
    let d = &i.disco;
    h.push_str(&format!("<h2>3. {}</h2>", esc(t.panel("au_disco_titulo"))));
    h.push_str(&format!(
        "<p>{} <span class=\"m\">{}</span> · {}</p>",
        esc(t.panel("au_disco_carpeta")),
        esc(&d.carpeta),
        bytes_texto(d.bytes_carpeta, idioma)
    ));
    h.push_str(&format!(
        "<p class=\"{}\">{}</p>",
        if d.base.direcciones_publicas == 0 {
            "ok"
        } else {
            "no"
        },
        esc(&if d.base.direcciones_publicas == 0 {
            t.panel("au_ip_ninguna").to_owned()
        } else {
            t.panel("au_ip_hay")
                .replace("{n}", &miles(d.base.direcciones_publicas, idioma))
        })
    ));
    h.push_str(&format!(
        "<table><tr><th>{}</th><th>{}</th><th>{}</th><th>{}</th></tr>",
        esc(t.panel("au_col_tabla")),
        esc(t.panel("au_col_columnas")),
        esc(t.panel("au_col_filas")),
        esc(t.panel("au_col_tamano"))
    ));
    for tb in &d.base.tablas {
        let cols: Vec<String> = tb.columnas.iter().map(|c| c.nombre.clone()).collect();
        h.push_str(&format!(
            "<tr><td class=\"m\">{}</td><td class=\"m\">{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td></tr>",
            esc(&tb.nombre),
            esc(&cols.join(", ")),
            miles(tb.filas, idioma),
            tb.bytes.map_or_else(|| "—".to_owned(), |b| bytes_texto(b, idioma))
        ));
    }
    h.push_str("</table>");
    h.push_str(&format!(
        "<table><tr><th>{}</th><th>{}</th></tr>",
        esc(t.panel("au_col_archivo")),
        esc(t.panel("au_col_tamano"))
    ));
    for (n, b) in &d.archivos {
        h.push_str(&format!(
            "<tr><td class=\"m\">{}</td><td class=\"n\">{}</td></tr>",
            esc(n),
            bytes_texto(*b, idioma)
        ));
    }
    h.push_str("</table>");
    // The seal.
    h.push_str(&format!(
        "<div class=\"sello\"><b>{}</b><p class=\"m\">SHA-256 {}</p><p>{}</p><p>{} <span class=\"m\">{}</span></p><p>{} {} · {} {}</p></div>",
        esc(t.panel("au_sellado")),
        esc(&i.sello),
        esc(t.panel("au_sellado_que")),
        esc(t.panel("au_cadena_cabeza")),
        esc(i.cadena.cabeza.as_deref().unwrap_or("—")),
        esc(t.panel("au_transparencia")),
        esc(&i.transparencia),
        esc(t.panel("au_codigo")),
        esc(&i.codigo)
    ));
    h.push_str("</body></html>\n");
    h
}

/// The seal of a report: the SHA-256 of its JSON with the `sello` field empty, so the seal can
/// travel inside the report and still be recomputed by anyone (empty the field, hash the bytes).
fn sellar(mut i: Informe) -> (Informe, String, String) {
    i.sello = String::new();
    let mut base = serde_json::to_string_pretty(&i).unwrap_or_default();
    base.push('\n');
    let sello = certificado::sello(base.as_bytes());
    i.sello = sello.clone();
    let mut json = serde_json::to_string_pretty(&i).unwrap_or_default();
    json.push('\n');
    (i, json, sello)
}

async fn auditar(
    Lang(t): Lang,
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<AuditaVista> {
    let ahora = now_ms();
    let (propias, inv, cabeza, check) = with_ledger(&state, |l| {
        Ok((
            l.outbound()?,
            l.inventario()?,
            l.cabeza_cadena()?,
            l.check()?,
        ))
    })?;
    let carpeta = state
        .db_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let (ins, zero, (archivos, bytes_carpeta)) = tokio::task::spawn_blocking(move || {
        let zero: Vec<Vec<Outbound>> = archivos_zero()
            .iter()
            .filter_map(|p| inventario::salidas_en_archivo(p).ok())
            .collect();
        (instalado(), zero, archivos_de(&carpeta))
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())?;
    let informe = Informe {
        formato: FORMATO.to_owned(),
        programa: env!("CARGO_PKG_VERSION").to_owned(),
        generado: ahora,
        generado_utc: rfc3339_utc(ahora),
        sistema: std::env::consts::OS.to_owned(),
        sello: String::new(),
        instalado: ins,
        conexiones: junta_conexiones(propias, zero),
        disco: Disco {
            carpeta: state
                .db_path
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            archivos,
            bytes_carpeta,
            base: inv,
        },
        cadena: Cadena {
            algoritmo: "row_hash = sha256(prev_hash || campos), docs/VERIFY.md".to_owned(),
            cabeza,
            comprobada: check.is_ok(),
            filas_comprobadas: check.checked,
        },
        transparencia: TRANSPARENCIA.to_owned(),
        codigo: CODIGO.to_owned(),
    };
    let (informe, json, sello) = sellar(informe);
    let dia = informe
        .generado_utc
        .get(..10)
        .unwrap_or("informe")
        .to_owned();
    let archivo_json = format!("guardiana-auditoria-{dia}.json");
    Ok(Json(AuditaVista {
        html: html(t, &informe),
        linea_sha256: certificado::linea_sha256(&sello, &archivo_json),
        archivo_html: format!("guardiana-auditoria-{dia}.html"),
        archivo_sha256: format!("{archivo_json}.sha256"),
        archivo_json,
        informe,
        json,
        sello,
    }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use guardiana_core::Purpose;

    fn o(id: i64, ts: i64, p: Purpose, host: &str) -> Outbound {
        Outbound {
            id,
            ts,
            purpose: p,
            host: host.to_owned(),
            bytes: 100,
            initiated_by_user: true,
        }
    }

    #[test]
    fn connections_from_both_records_come_together_newest_first() {
        let c = junta_conexiones(
            vec![o(1, 10, Purpose::Licencia, "live.dodopayments.com")],
            vec![vec![
                o(1, 30, Purpose::Rele, "rele.guardianagroup.com"),
                o(2, 20, Purpose::Version, "raw.githubusercontent.com"),
            ]],
        );
        assert_eq!(c.total, 3);
        assert_eq!(c.archivos_zero, 1);
        assert_eq!(c.lista[0].motivo, "rele");
        assert_eq!(c.lista[0].origen, "zero");
        assert_eq!(c.lista[2].origen, "servicio");
        assert_eq!(c.por_motivo.len(), 3);
    }

    #[test]
    fn the_seal_is_the_hash_of_the_report_with_its_seal_field_empty() {
        let i = Informe {
            formato: FORMATO.to_owned(),
            programa: "1.0.46".to_owned(),
            generado: 1,
            generado_utc: rfc3339_utc(1),
            sistema: "windows".to_owned(),
            sello: "x".to_owned(),
            instalado: Instalado {
                version: "1.0.46".to_owned(),
                binario: "guardiana.exe".to_owned(),
                sha256: "ab".to_owned(),
                firma: SignatureState::DevKey,
                registro: LedgerMatch::NotFound,
                registro_zip: false,
                rekor_url: None,
                otros: Vec::new(),
                registro_local: false,
                coincide: false,
            },
            conexiones: Conexiones::default(),
            disco: Disco {
                carpeta: "C:/ProgramData/GUARDIANA".to_owned(),
                archivos: vec![("ledger.db".to_owned(), 4096)],
                bytes_carpeta: 4096,
                base: Inventario::default(),
            },
            cadena: Cadena {
                algoritmo: String::new(),
                cabeza: None,
                comprobada: true,
                filas_comprobadas: 0,
            },
            transparencia: TRANSPARENCIA.to_owned(),
            codigo: CODIGO.to_owned(),
        };
        let (i, json, sello) = sellar(i);
        assert_eq!(i.sello, sello);
        // Anyone can check it: empty the field, hash the bytes.
        let vacio = json.replace(&format!("\"sello\": \"{sello}\""), "\"sello\": \"\"");
        assert_eq!(certificado::sello(vacio.as_bytes()), sello);
        let h = html(i18n::es(), &i);
        assert!(h.contains(&sello));
        assert!(!h.contains("<script"));
    }
}
