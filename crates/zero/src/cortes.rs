//! «Lo que se cortó»: every request the browser cut, one by one, for the person to examine:
//! when, on which site they were, where it tried to go, which company and country, what it is,
//! what kind of request, and why it was cut. One line per cut in a file per day, kept 31 days
//! on this computer only, erased with «Borrar todo».
//!
//! What is written is what the person needs to understand the cut, and no more: the page's
//! site (never its address), the destination's host and path without the query string (a
//! query can carry identifiers, even the person's own), and nothing at all about the page in an
//! isolated tab.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::decision::{Decision, Motivo};
use crate::mandato::Recurso;

/// Days kept.
pub const DIAS: usize = 31;
/// Lines sent to the page at most (the summaries count all of them).
pub const MAX_LISTA: usize = 3000;
/// Lines written per day at most; the day's figures go on counting past it.
pub const MAX_DIA: usize = 25_000;

/// One cut.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Corte {
    /// Milliseconds since the epoch.
    pub ts: i64,
    /// The site of the page the person was on (empty in an isolated tab).
    pub pagina: String,
    /// Where it tried to go.
    pub host: String,
    /// The path, without the query string, at most 120 characters.
    pub ruta: String,
    /// How many query parameters it carried (their values are not kept).
    pub parametros: u32,
    /// Their names, each once, at most [`crate::campos::MAX_CAMPOS`]: what the request
    /// carried can be read from them (`dl` is the page, `sr` the screen). Never the values.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub campos: Vec<String>,
    /// The names of the fields of its body (a JSON's keys, a form's names), never the values.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub campos_cuerpo: Vec<String>,
    /// The names of the cookies it carried, never their values: how the destination knows you
    /// from one site to the next.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cookies: Vec<String>,
    /// The registrable site of the destination.
    pub sitio: String,
    /// The company behind it, or the site when unknown.
    pub quien: String,
    /// Its country (ISO code).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pais: Option<String>,
    /// What the open lists say it is (`rastreador`, `publicidad`, `telemetria`…).
    pub categoria: String,
    /// The data broker registry name, when it is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corredor: Option<String>,
    /// Where to ask that broker to delete your data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corredor_url: Option<String>,
    /// Why it was cut.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motivo: Option<Motivo>,
    /// What kind of request it was.
    pub recurso: Recurso,
    /// GET, POST…
    pub metodo: String,
    /// For marked data: which kind (`dato_correo`…).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dato: Option<String>,
    /// For marked data: in which form it travelled (`tal_cual`, `sha256`…).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub como: Option<String>,
    /// It happened in an isolated tab.
    #[serde(default)]
    pub aislada: bool,
    /// It happened in a mandate's tab.
    #[serde(default)]
    pub mandato: bool,
    /// The outside companies whose code the page had loaded before this request, in order:
    /// the chain «page → their code → this request» as far as the browser can see it. The
    /// engine does not say which script made a request, so this is what was running, not who
    /// fired it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub codigo: Vec<String>,
    /// It happened within seconds of the person sending a form on the page.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tras_envio: bool,
}

/// Where and how a cut happened (what the decision itself does not know).
#[derive(Debug, Clone, Copy)]
pub struct Lugar<'a> {
    /// Milliseconds since the epoch.
    pub ts: i64,
    /// The page's site.
    pub pagina: &'a str,
    /// The address asked for.
    pub url: &'a str,
    /// GET, POST…
    pub metodo: &'a str,
    /// What it sends (empty when the engine did not hand it over).
    pub cuerpo: &'a [u8],
    /// Its `Cookie` header (empty when none).
    pub cookies: &'a str,
    /// What kind of request.
    pub recurso: Recurso,
    /// The kind of marked data found (`dato_correo`…), if any.
    pub dato: Option<&'a str>,
    /// In an isolated tab.
    pub aislada: bool,
    /// In a mandate's tab.
    pub mandato: bool,
    /// The outside companies whose code the page had loaded so far.
    pub codigo: &'a [String],
    /// Within seconds of a form the person sent.
    pub tras_envio: bool,
}

impl Corte {
    /// The line for a cut decision.
    #[must_use]
    pub fn de(l: &Lugar<'_>, d: &Decision) -> Self {
        let (ruta, parametros) = url::Url::parse(l.url).map_or((String::new(), 0), |u| {
            let ruta: String = u.path().chars().take(120).collect();
            let n = u.query_pairs().count();
            (ruta, u32::try_from(n).unwrap_or(u32::MAX))
        });
        let corredor = d.destino.corredor.as_ref();
        Self {
            ts: l.ts,
            pagina: if l.aislada {
                String::new()
            } else {
                l.pagina.to_string()
            },
            host: d.destino.host.clone(),
            ruta,
            parametros,
            campos: crate::campos::nombres(l.url),
            campos_cuerpo: crate::campos::nombres_cuerpo(l.cuerpo),
            cookies: crate::campos::nombres_cookies(l.cookies),
            sitio: d.destino.sitio.clone(),
            quien: d.destino.quien().to_string(),
            pais: d.destino.pais.map(str::to_string),
            categoria: d.destino.categoria.to_string(),
            corredor: corredor.map(|c| c.nombre.to_string()),
            corredor_url: corredor.and_then(|c| c.url_derechos.map(str::to_string)),
            motivo: d.motivo,
            recurso: l.recurso,
            metodo: l.metodo.to_ascii_uppercase(),
            dato: l.dato.map(str::to_string),
            como: d.hallazgos.first().map(|h| h.como.to_string()),
            aislada: l.aislada,
            mandato: l.mandato,
            codigo: l.codigo.to_vec(),
            tras_envio: l.tras_envio,
        }
    }
}

fn archivo(carpeta: &Path, dia: &str) -> PathBuf {
    carpeta.join(format!("{dia}.jsonl"))
}

/// Append the day's cuts to its file.
///
/// # Errors
/// When the folder or the file cannot be written.
pub fn anota(carpeta: &Path, dia: &str, cortes: &[Corte]) -> std::io::Result<()> {
    if cortes.is_empty() {
        return Ok(());
    }
    fs::create_dir_all(carpeta)?;
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(archivo(carpeta, dia))?;
    let mut buf = Vec::new();
    for c in cortes {
        if let Ok(l) = serde_json::to_vec(c) {
            buf.extend_from_slice(&l);
            buf.push(b'\n');
        }
    }
    f.write_all(&buf)
}

/// The cuts of the days from `desde` to `hasta` (`YYYY-MM-DD`, both included), oldest first.
#[must_use]
pub fn lee(carpeta: &Path, desde: &str, hasta: &str) -> Vec<Corte> {
    let mut dias: Vec<String> = fs::read_dir(carpeta)
        .map(|l| {
            l.filter_map(|e| e.ok())
                .filter_map(|e| {
                    e.file_name()
                        .to_str()
                        .and_then(|n| n.strip_suffix(".jsonl"))
                        .map(str::to_string)
                })
                .filter(|d| d.as_str() >= desde && d.as_str() <= hasta)
                .collect()
        })
        .unwrap_or_default();
    dias.sort();
    let mut out = Vec::new();
    for d in dias {
        if let Ok(texto) = fs::read_to_string(archivo(carpeta, &d)) {
            out.extend(
                texto
                    .lines()
                    .filter_map(|l| serde_json::from_str::<Corte>(l).ok())
                    .map(actualiza),
            );
        }
    }
    out
}

/// A cut as the lists know it today. A cut is written once, with what the lists said that day;
/// when the lists learn a company or its country later (the owner, 13 Oct 2026: 287 «país
/// desconocido» on a screen, most of them names the lists already knew), the old lines must
/// not keep saying nobody. Only the gaps are filled: what was written stays.
fn actualiza(mut c: Corte) -> Corte {
    if c.pais.is_some() && c.quien != c.sitio {
        return c;
    }
    let d = crate::destino::clasifica(&c.host);
    if c.quien == c.sitio && d.quien() != d.sitio {
        c.quien = d.quien().to_string();
    }
    if c.pais.is_none() {
        c.pais = d.pais.map(str::to_string);
    }
    if c.corredor.is_none() {
        if let Some(b) = &d.corredor {
            c.corredor = Some(b.nombre.to_string());
            c.corredor_url = b.url_derechos.map(str::to_string);
        }
    }
    c
}

/// Keep only the last [`DIAS`] days: every day file dated before `desde` (`YYYY-MM-DD`, the
/// first day still kept) goes. Until 1.0.1 it kept the 31 newest *files*, so for someone who
/// browses now and then, cuts from months ago stayed on disk while the screen said 31 days.
pub fn poda(carpeta: &Path, desde: &str) {
    let Ok(l) = fs::read_dir(carpeta) else {
        return;
    };
    for p in l.filter_map(|e| e.ok().map(|e| e.path())) {
        let viejo = p.extension().is_some_and(|x| x == "jsonl")
            && p.file_stem()
                .and_then(|n| n.to_str())
                .is_some_and(|dia| dia < desde);
        if viejo {
            let _ = fs::remove_file(p);
        }
    }
}

/// Counts by a key, largest first.
#[must_use]
pub fn cuenta<'a>(
    cortes: &'a [Corte],
    clave: impl Fn(&'a Corte) -> String,
) -> Vec<(String, usize)> {
    let mut m: BTreeMap<String, usize> = BTreeMap::new();
    for c in cortes {
        *m.entry(clave(c)).or_insert(0) += 1;
    }
    let mut v: Vec<(String, usize)> = m.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v
}

/// One CSV field, quoted when needed.
fn campo(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r', ';']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// The cuts as CSV, with the column names and values already in the person's language
/// (`textos` turns a key into its words; `hora` a timestamp into the local time).
#[must_use]
pub fn csv(
    cortes: &[Corte],
    columnas: &[String],
    textos: &dyn Fn(&str) -> String,
    hora: &dyn Fn(i64) -> String,
) -> String {
    let mut out = String::from("\u{feff}");
    out.push_str(
        &columnas
            .iter()
            .map(|c| campo(c))
            .collect::<Vec<_>>()
            .join(","),
    );
    out.push_str("\r\n");
    for c in cortes.iter().rev() {
        let motivo = c
            .motivo
            .map(|m| textos(&format!("motivo_{}", motivo_clave(m))))
            .unwrap_or_default();
        let fila = [
            hora(c.ts),
            c.pagina.clone(),
            c.quien.clone(),
            c.pais.clone().unwrap_or_default(),
            c.host.clone(),
            c.ruta.clone(),
            textos(&format!("cat_{}", c.categoria)),
            textos(&format!("recurso_{}", recurso_clave(c.recurso))),
            c.metodo.clone(),
            motivo,
            c.corredor.clone().unwrap_or_default(),
            c.dato.as_deref().map(textos).unwrap_or_default(),
        ];
        out.push_str(&fila.iter().map(|f| campo(f)).collect::<Vec<_>>().join(","));
        out.push_str("\r\n");
    }
    out
}

/// The key of a reason in the texts (`motivo_<clave>`).
#[must_use]
pub const fn motivo_clave(m: Motivo) -> &'static str {
    match m {
        Motivo::Rastreador => "rastreador",
        Motivo::Publicidad => "publicidad",
        Motivo::Corredor => "corredor",
        Motivo::CorteTuyo => "corte_tuyo",
        Motivo::FueraDeMandato => "fuera_de_mandato",
        Motivo::Tinta => "tinta",
        Motivo::Senuelo => "senuelo",
        Motivo::Telemetria => "telemetria",
        Motivo::Baliza => "baliza",
        Motivo::Grabacion => "grabacion",
        Motivo::ListaAnuncios => "lista_anuncios",
        Motivo::ListaRastreo => "lista_rastreo",
        Motivo::Peligro => "peligro",
        Motivo::Subasta => "subasta",
    }
}

/// The key of a kind of request in the texts (`recurso_<clave>`).
#[must_use]
pub const fn recurso_clave(r: Recurso) -> &'static str {
    match r {
        Recurso::Documento => "documento",
        Recurso::Ventana => "ventana",
        Recurso::Marco => "marco",
        Recurso::Script => "script",
        Recurso::Imagen => "imagen",
        Recurso::Estilo => "estilo",
        Recurso::Fuente => "fuente",
        Recurso::Media => "media",
        Recurso::Datos => "datos",
        Recurso::Aviso => "aviso",
        Recurso::Socket => "socket",
        Recurso::Otro => "otro",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linea_de(c: &Corte) -> String {
        serde_json::to_string(c).unwrap_or_default()
    }
    use crate::decision::{decide, Ajustes, Peticion};

    #[test]
    fn a_cut_keeps_what_explains_it_and_not_the_query() {
        let a = Ajustes {
            cortar_seguimiento: true,
            ..Ajustes::default()
        };
        let url = "https://stats.g.doubleclick.net/g/collect?cid=123secreto&uid=ana%40correo.co";
        let d = decide(
            &Peticion {
                url,
                cuerpo: b"",
                recurso: Recurso::Imagen,
                sitio_pagina: "eltiempo.com",
            },
            &a,
            &[],
            None,
            None,
        );
        let mut l = Lugar {
            ts: 10,
            pagina: "eltiempo.com",
            url,
            metodo: "get",
            cuerpo: b"",
            cookies: "_ga=GA1.secreto; _fbp=fb.secreto",
            recurso: Recurso::Imagen,
            dato: None,
            aislada: false,
            mandato: false,
            codigo: &["Google".to_string()],
            tras_envio: false,
        };
        let c = Corte::de(&l, &d);
        assert_eq!(c.codigo, vec!["Google"]);
        assert_eq!(c.ruta, "/g/collect");
        assert_eq!(c.parametros, 2);
        assert_eq!(c.campos.len(), 2);
        assert_eq!(c.cookies, vec!["_ga", "_fbp"]);
        assert!(
            !linea_de(&c).contains("secreto"),
            "values are never written"
        );
        assert_eq!(c.quien, "Google");
        assert_eq!(c.metodo, "GET");
        let linea = serde_json::to_string(&c).unwrap_or_default();
        assert!(!linea.contains("correo"));
        // In an isolated tab the page is not written down.
        l.aislada = true;
        let c = Corte::de(&l, &d);
        assert!(c.pagina.is_empty());
        // Written and read back by day.
        let dir = std::env::temp_dir().join(format!("zero-cortes-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        assert!(anota(&dir, "2026-10-09", &[c.clone(), c.clone()]).is_ok());
        assert!(anota(&dir, "2026-10-10", std::slice::from_ref(&c)).is_ok());
        assert_eq!(lee(&dir, "2026-10-09", "2026-10-09").len(), 2);
        assert_eq!(lee(&dir, "2026-10-01", "2026-10-31").len(), 3);
        let t = |k: &str| k.to_string();
        let h = |ts: i64| ts.to_string();
        let csv = csv(
            &lee(&dir, "2026-10-09", "2026-10-10"),
            &["a".into()],
            &t,
            &h,
        );
        assert_eq!(csv.lines().count(), 4);
        assert!(csv.contains("cat_publicidad") || csv.contains("cat_rastreador"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_log_keeps_the_last_31_days_by_date_not_by_count() {
        let dir = std::env::temp_dir().join(format!("zero-poda-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap_or_default();
        for f in [
            "2026-06-01.jsonl",
            "2026-09-08.jsonl",
            "2026-09-09.jsonl",
            "2026-10-09.jsonl",
            "nota.txt",
        ] {
            fs::write(dir.join(f), "").unwrap_or_default();
        }
        poda(&dir, "2026-09-09");
        let mut quedan: Vec<String> = fs::read_dir(&dir)
            .map(|l| {
                l.filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        quedan.sort();
        assert_eq!(quedan, ["2026-09-09.jsonl", "2026-10-09.jsonl", "nota.txt"]);
        let _ = fs::remove_dir_all(&dir);
    }
}
