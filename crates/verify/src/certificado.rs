//! «Certificado de flujos» (1.0.45): a document, made on this computer from its own hash-chained
//! ledger, that says for a period which companies this computer asked for, how many times, what
//! was cut, from which countries the servers answered and in which categories, and which
//! companies of a reference list it did **not** ask for at all.
//!
//! The owner, 18 Oct 2026: «la experiencia del cliente debe ser sentir el monitoreo constante…
//! para saber que todo está bien y nada está vigilando o extrayendo sus datos». A certificate is
//! that feeling written down in a form somebody else can check.
//!
//! What it carries and what it never does:
//! - Companies, countries, categories and counts. Never an address (`client_ip` is not even
//!   read), never the program, never the full list of names: names of a company appear only when
//!   the person asks for them (`dominios`, off by default), and names that belong to no known
//!   company are only counted.
//! - No new cryptography. It points into the ledger's own chain (`row_hash = sha256(prev_hash ||
//!   fields)`, `docs/VERIFY.md`): the first and last row of the period, how many rows, and the
//!   SHA-256 of their `row_hash` one after the other. The file itself is sealed with a plain
//!   SHA-256 written in `sha256sum` format, so `sha256sum -c` checks it anywhere, and
//!   `guardiana verify --certificado` rebuilds it from the ledger and says whether it matches.
//! - There is no private key on this computer to sign with, and a key kept beside the ledger
//!   would prove nothing a hash does not: whoever holds the computer holds the key. The page and
//!   the document say so instead of calling the seal a signature.

use std::collections::{BTreeMap, BTreeSet};

use guardiana_core::time::{day_utc, rfc3339_utc, DAY_MS};
use guardiana_core::{
    identity, local_day_index, local_midnight, ChainFault, Ledger, Verdict, SELF_CHECK_SUFFIX,
    SELF_DEVICE_ID,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The format name written in every certificate. A reader that does not know it stops.
pub const FORMATO: &str = "guardiana-certificado-flujos/1";

/// What the certificate is about. Stored inside it, so it can be rebuilt exactly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Opciones {
    /// Start of the period, Unix ms (inclusive).
    pub desde: i64,
    /// End of the period, Unix ms (exclusive). Never later than the moment it was made.
    pub hasta: i64,
    /// Minutes east of UTC used to write the days.
    pub zona_min: i64,
    /// Whether each company lists the names of it this computer asked for. Off by default: the
    /// certificate is about companies, not a browsing history.
    #[serde(default)]
    pub dominios: bool,
    /// Only this company: the extract that goes with a letter to it. The other companies are
    /// none of its business, so they are left out; the chain reference still covers every row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solo_empresa: Option<String>,
}

/// Queries, answered and cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Recuento {
    /// Every query.
    pub consultas: u64,
    /// Answered with an address (observed or answered by GUARDIANA itself).
    pub respondidas: u64,
    /// Cut by a rule: no address was given, so nothing could connect through that answer.
    pub cortadas: u64,
}

impl Recuento {
    fn suma(&mut self, cortada: bool) {
        self.consultas += 1;
        if cortada {
            self.cortadas += 1;
        } else {
            self.respondidas += 1;
        }
    }
}

/// One name of a company and how often it was asked for (only with `dominios`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NombreCuenta {
    /// The name.
    pub nombre: String,
    /// Queries.
    pub consultas: u64,
    /// Of them, cut.
    pub cortadas: u64,
}

/// One company this computer asked for in the period.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilaEmpresa {
    /// The company as the lists name it.
    pub empresa: String,
    /// Two-letter country of its seat, when the lists write it. Not where the server was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pais_sede: Option<String>,
    /// The group it belongs to, when written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grupo: Option<String>,
    /// Queries, answered and cut.
    #[serde(flatten)]
    pub recuento: Recuento,
    /// First and last query, Unix ms.
    pub primera: i64,
    /// Last query, Unix ms.
    pub ultima: i64,
    /// The days of contact (`AAAA-MM-DD`, in the certificate's zone): days with at least one
    /// query answered with an address. Days whose queries were all cut are not in it.
    pub dias: Vec<String>,
    /// Queries per category of the lists.
    pub por_categoria: BTreeMap<String, u64>,
    /// Answered queries per country of the server that answered (`sin_pais`, `anycast`, `local`
    /// when the base gave no country).
    pub por_pais_servidor: BTreeMap<String, u64>,
    /// Its names, most asked first, only with `dominios`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dominios: Vec<NombreCuenta>,
}

/// A company of the reference list this computer did not ask for once in the period.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoContactada {
    /// The company.
    pub empresa: String,
    /// Its seat, when written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pais_sede: Option<String>,
    /// How many names the lists know of it: «none of these N names was asked for».
    pub nombres_conocidos: usize,
}

/// Totals of the period.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Totales {
    /// Every query of this computer to a name on the internet.
    #[serde(flatten)]
    pub recuento: Recuento,
    /// Companies asked for.
    pub empresas: usize,
    /// Days with at least one query.
    pub dias_con_actividad: usize,
    /// Queries for the local network itself (the printer, reverse lookups, GUARDIANA's own
    /// check), which are not a destination and are left out of everything else.
    pub locales: u64,
}

/// The names that belong to no company the lists know: only counted, never listed.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SinEmpresa {
    /// Queries, answered and cut.
    #[serde(flatten)]
    pub recuento: Recuento,
    /// How many different names.
    pub nombres: usize,
}

/// What the ledger says of the period.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Contenido {
    /// Totals.
    pub totales: Totales,
    /// The companies, most asked first.
    pub empresas: Vec<FilaEmpresa>,
    /// Names with no known company. Absent in an extract for one company.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sin_empresa: Option<SinEmpresa>,
    /// Queries per category.
    pub por_categoria: BTreeMap<String, Recuento>,
    /// Answered queries per country of the server that answered.
    pub por_pais_servidor: BTreeMap<String, u64>,
    /// Which reference list `no_contactadas` was checked against (`fichas 2026-10-12`); empty in
    /// an extract.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub referencia: String,
    /// The companies of that list this computer did not ask for.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub no_contactadas: Vec<NoContactada>,
}

/// Bytes either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Bytes {
    /// Sent from this computer.
    pub enviados: u64,
    /// Received.
    pub recibidos: u64,
}

impl Bytes {
    fn suma(&mut self, e: u64, r: u64) {
        self.enviados = self.enviados.saturating_add(e);
        self.recibidos = self.recibidos.saturating_add(r);
    }
}

/// What the network channel measured (Windows only). These are measurements, not chained rows:
/// they are sealed with the document, but the ledger's chain does not cover them.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Medido {
    /// Whether anything was measured in the period.
    pub disponible: bool,
    /// Bytes per company, by the name the program had asked for.
    pub por_empresa: BTreeMap<String, Bytes>,
    /// Bytes to names that belong to no known company.
    pub otros_nombres: Bytes,
    /// Bytes to addresses no name was asked for (a fixed address, another resolver). Absent in
    /// an extract.
    pub sin_nombre: Bytes,
    /// The same, per country of the address.
    pub sin_nombre_por_pais: BTreeMap<String, Bytes>,
}

/// A row of the chain, by its id and hashes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilaRef {
    /// Row id.
    pub id: i64,
    /// Unix ms.
    pub ts: i64,
    /// Its `prev_hash`.
    pub prev_hash: String,
    /// Its `row_hash`.
    pub row_hash: String,
}

/// Where the certificate stands in the ledger's chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cadena {
    /// How a row hash is made, for whoever recomputes it.
    pub algoritmo: String,
    /// The genesis of every ledger: SHA-256 of the program's public key.
    pub genesis: String,
    /// The program carries the development key (a test build).
    pub clave_desarrollo: bool,
    /// Whether the whole chain verified when the certificate was made.
    pub comprobada: bool,
    /// The first fault, when it did not (`1234 alterado`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallo: Option<String>,
    /// Rows verified then.
    pub filas_comprobadas: u64,
    /// Rows pruned before then, summarised by the anchor.
    pub recortadas: u64,
    /// The chain head then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cabeza: Option<String>,
    /// This computer's rows in the period.
    pub filas_periodo: u64,
    /// The first of them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primera: Option<FilaRef>,
    /// The last of them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ultima: Option<FilaRef>,
    /// SHA-256 of their `row_hash`, the 32 bytes of each one after the other, in id order.
    pub huella_filas: String,
}

/// How much of the period the ledger can speak for.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Cobertura {
    /// The oldest row still in the ledger, when there is any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registro_desde: Option<i64>,
    /// The ledger reaches back to the start of the period.
    pub completa: bool,
    /// Stretches inside the period when GUARDIANA was not watching (from, to), Unix ms.
    pub sin_vigilar: Vec<(i64, i64)>,
    /// Their total length, ms.
    pub sin_vigilar_ms: i64,
}

/// The certificate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Certificado {
    /// Always [`FORMATO`].
    pub formato: String,
    /// The version of GUARDIANA that made it.
    pub programa: String,
    /// When it was made, Unix ms.
    pub generado: i64,
    /// The same, RFC 3339 UTC.
    pub generado_utc: String,
    /// What it is about.
    pub opciones: Opciones,
    /// First day of the period, in its zone.
    pub desde_dia: String,
    /// Last day of the period (inclusive), in its zone.
    pub hasta_dia: String,
    /// How much of the period the ledger covers.
    pub cobertura: Cobertura,
    /// What the ledger says.
    pub contenido: Contenido,
    /// What was measured.
    pub medido: Medido,
    /// The chain reference.
    pub cadena: Cadena,
}

// ----- days ---------------------------------------------------------------------------------

fn dias_desde_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(m);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn dias_del_mes(y: i64, m: u32) -> u32 {
    match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// `AAAA-MM-DD` as a day number since 1970-01-01, `None` when it is not a real date.
#[must_use]
pub fn dia_de_texto(s: &str) -> Option<i64> {
    let mut p = s.trim().split('-');
    let (Some(y), Some(m), Some(d), None) = (p.next(), p.next(), p.next(), p.next()) else {
        return None;
    };
    if y.len() != 4 || m.len() != 2 || d.len() != 2 {
        return None;
    }
    let (y, m, d) = (
        y.parse::<i64>().ok()?,
        m.parse::<u32>().ok()?,
        d.parse::<u32>().ok()?,
    );
    if !(1..=12).contains(&m) || d == 0 || d > dias_del_mes(y, m) {
        return None;
    }
    Some(dias_desde_civil(y, m, d))
}

fn texto_de_dia(dia: i64) -> String {
    day_utc(dia * DAY_MS)
}

/// The period from day `desde` to day `hasta`, both included, as Unix ms in the zone.
#[must_use]
pub fn periodo_dias(desde: &str, hasta: &str, zona_min: i64) -> Option<(i64, i64)> {
    let (a, b) = (dia_de_texto(desde)?, dia_de_texto(hasta)?);
    (a <= b).then(|| (local_midnight(a, zona_min), local_midnight(b + 1, zona_min)))
}

/// The calendar month `AAAA-MM` in the zone, as Unix ms.
#[must_use]
pub fn periodo_mes(mes: &str, zona_min: i64) -> Option<(i64, i64)> {
    let (y, m) = mes.trim().split_once('-')?;
    if y.len() != 4 || m.len() != 2 {
        return None;
    }
    let (y, m) = (y.parse::<i64>().ok()?, m.parse::<u32>().ok()?);
    if !(1..=12).contains(&m) || !(1970..=9999).contains(&y) {
        return None;
    }
    let primero = dias_desde_civil(y, m, 1);
    let siguiente = if m == 12 {
        dias_desde_civil(y + 1, 1, 1)
    } else {
        dias_desde_civil(y, m + 1, 1)
    };
    Some((
        local_midnight(primero, zona_min),
        local_midnight(siguiente, zona_min),
    ))
}

/// `AAAA-MM` of the last whole month before `ahora`, in the zone: the monthly certificate.
#[must_use]
pub fn mes_anterior(ahora: i64, zona_min: i64) -> String {
    let hoy = texto_de_dia(local_day_index(ahora, zona_min));
    let y: i64 = hoy.get(0..4).and_then(|s| s.parse().ok()).unwrap_or(1970);
    let m: u32 = hoy.get(5..7).and_then(|s| s.parse().ok()).unwrap_or(1);
    let (y, m) = if m == 1 { (y - 1, 12) } else { (y, m - 1) };
    format!("{y:04}-{m:02}")
}

// ----- making it ----------------------------------------------------------------------------

/// The company of a name, its seat and its group: the lists' company, else the registry of data
/// brokers (the same order the panel uses).
fn empresa_de(qname: &str) -> Option<(&'static str, Option<&'static str>, Option<&'static str>)> {
    if let Some(e) = guardiana_lists::company_of(qname) {
        return Some((
            e,
            guardiana_lists::country_of(qname),
            guardiana_lists::group_of(qname),
        ));
    }
    guardiana_lists::data_broker_of(qname).map(|b| (b.name, b.country, None))
}

fn es_local(qname: &str) -> bool {
    guardiana_lists::is_local_name(qname)
        || qname == SELF_CHECK_SUFFIX
        || qname.ends_with(&format!(".{SELF_CHECK_SUFFIX}"))
}

/// Where the server that answered was, as a key: its country, or what kind of address it was.
/// `None` when no address was given (cut, or answered without one).
fn clave_servidor(kind: i64, country: &str) -> Option<String> {
    match kind {
        1 if !country.is_empty() => Some(country.to_ascii_uppercase()),
        1 => Some("sin_pais".to_owned()),
        2 => Some("anycast".to_owned()),
        3 => Some("local".to_owned()),
        _ => None,
    }
}

#[derive(Default)]
struct AcEmpresa {
    pais: Option<&'static str>,
    grupo: Option<&'static str>,
    rec: Recuento,
    primera: i64,
    ultima: i64,
    dias: BTreeSet<i64>,
    cat: BTreeMap<String, u64>,
    srv: BTreeMap<String, u64>,
    nombres: BTreeMap<String, (u64, u64)>,
}

fn texto_fallo(f: &ChainFault) -> &'static str {
    match f {
        ChainFault::BrokenLink { .. } => "enlace",
        ChainFault::AlteredRow { .. } => "alterado",
        ChainFault::Unreadable(_) => "ilegible",
        ChainFault::Truncated { .. } => "recortado",
        ChainFault::HeadMissing => "sin_cabeza",
    }
}

/// The reference list of «did not contact»: the companies with a checked profile in the program
/// (`fichas.json`, the ones people meet most), each with the names the lists know of it.
fn referencia() -> (String, Vec<(&'static str, Option<&'static str>, usize)>) {
    let v: serde_json::Value =
        serde_json::from_str(guardiana_lists::FICHAS).unwrap_or(serde_json::Value::Null);
    let mut out = Vec::new();
    if let Some(m) = v.get("empresas").and_then(|e| e.as_object()) {
        for nombre in m.keys() {
            if let Some(r) = guardiana_lists::company_record(nombre) {
                if !r.domains.is_empty() {
                    out.push((r.name, r.country, r.domains.len()));
                }
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(b.0));
    (
        format!("fichas {}", guardiana_lists::profiles_checked()),
        out,
    )
}

/// Make the certificate of `op` from the ledger, as of `ahora`.
pub fn generar(l: &Ledger, op: &Opciones, ahora: i64) -> guardiana_core::Result<Certificado> {
    let mut op = op.clone();
    op.hasta = op.hasta.min(ahora);
    if op.hasta < op.desde {
        op.hasta = op.desde;
    }
    let solo = op.solo_empresa.clone();
    let zona = op.zona_min;

    let mut por: BTreeMap<&'static str, AcEmpresa> = BTreeMap::new();
    let mut sin = SinEmpresa::default();
    let mut sin_nombres: BTreeSet<String> = BTreeSet::new();
    let mut tot = Totales::default();
    let mut dias_activos: BTreeSet<i64> = BTreeSet::new();
    let mut por_cat: BTreeMap<String, Recuento> = BTreeMap::new();
    let mut por_pais: BTreeMap<String, u64> = BTreeMap::new();
    let mut huella = Sha256::new();
    let mut primera: Option<FilaRef> = None;
    let mut ultima: Option<FilaRef> = None;

    let filas = l.recorre_flujos(SELF_DEVICE_ID, op.desde, op.hasta, |f| {
        huella.update(f.row_hash.0);
        let r = FilaRef {
            id: f.id,
            ts: f.ts,
            prev_hash: f.prev_hash.to_hex(),
            row_hash: f.row_hash.to_hex(),
        };
        if primera.is_none() {
            primera = Some(r.clone());
        }
        ultima = Some(r);

        let empresa = empresa_de(&f.qname);
        if let Some(s) = &solo {
            if empresa.map(|e| e.0) != Some(s.as_str()) {
                return;
            }
        }
        if es_local(&f.qname) {
            tot.locales += 1;
            return;
        }
        let cortada = f.verdict == Verdict::Cortado;
        let dia = local_day_index(f.ts, zona);
        let cat = f.category.as_str().to_owned();
        let srv = clave_servidor(f.server_kind, &f.server_country);
        tot.recuento.suma(cortada);
        dias_activos.insert(dia);
        por_cat.entry(cat.clone()).or_default().suma(cortada);
        if let Some(k) = &srv {
            *por_pais.entry(k.clone()).or_insert(0) += 1;
        }
        match empresa {
            None => {
                sin.recuento.suma(cortada);
                sin_nombres.insert(f.qname);
            }
            Some((e, pais, grupo)) => {
                let a = por.entry(e).or_insert_with(|| AcEmpresa {
                    pais,
                    grupo,
                    primera: f.ts,
                    ultima: f.ts,
                    ..AcEmpresa::default()
                });
                a.rec.suma(cortada);
                a.primera = a.primera.min(f.ts);
                a.ultima = a.ultima.max(f.ts);
                // A day of contact: a query answered with an address. A day whose queries were
                // all cut is not one: nothing could connect through those answers.
                if !cortada {
                    a.dias.insert(dia);
                }
                *a.cat.entry(cat).or_insert(0) += 1;
                if let Some(k) = srv {
                    *a.srv.entry(k).or_insert(0) += 1;
                }
                if op.dominios {
                    let n = a.nombres.entry(f.qname).or_insert((0, 0));
                    n.0 += 1;
                    if cortada {
                        n.1 += 1;
                    }
                }
            }
        }
    })?;

    let mut empresas: Vec<FilaEmpresa> = por
        .into_iter()
        .map(|(e, a)| {
            let mut dominios: Vec<NombreCuenta> = a
                .nombres
                .into_iter()
                .map(|(nombre, (consultas, cortadas))| NombreCuenta {
                    nombre,
                    consultas,
                    cortadas,
                })
                .collect();
            dominios.sort_by(|x, y| y.consultas.cmp(&x.consultas).then(x.nombre.cmp(&y.nombre)));
            FilaEmpresa {
                empresa: e.to_owned(),
                pais_sede: a.pais.map(str::to_owned),
                grupo: a.grupo.map(str::to_owned),
                recuento: a.rec,
                primera: a.primera,
                ultima: a.ultima,
                dias: a.dias.into_iter().map(texto_de_dia).collect(),
                por_categoria: a.cat,
                por_pais_servidor: a.srv,
                dominios,
            }
        })
        .collect();
    empresas.sort_by(|x, y| {
        y.recuento
            .consultas
            .cmp(&x.recuento.consultas)
            .then_with(|| x.empresa.cmp(&y.empresa))
    });
    tot.empresas = empresas.len();
    tot.dias_con_actividad = dias_activos.len();
    sin.nombres = sin_nombres.len();

    let (nombre_ref, lista_ref) = if solo.is_some() {
        (String::new(), Vec::new())
    } else {
        referencia()
    };
    let no_contactadas = lista_ref
        .into_iter()
        .filter(|(n, _, _)| !empresas.iter().any(|e| e.empresa == *n))
        .map(|(n, pais, k)| NoContactada {
            empresa: n.to_owned(),
            pais_sede: pais.map(str::to_owned),
            nombres_conocidos: k,
        })
        .collect();

    let contenido = Contenido {
        totales: tot,
        empresas,
        sin_empresa: solo.is_none().then_some(sin),
        por_categoria: por_cat,
        por_pais_servidor: por_pais,
        referencia: nombre_ref,
        no_contactadas,
    };

    // The bytes: what the channel measured, when it measured anything (Windows).
    let mut medido = Medido::default();
    if let Ok(filas_b) = l.bytes_por_destino(op.desde, op.hasta) {
        for b in filas_b {
            if b.destino.is_empty() {
                if solo.is_none() {
                    medido.sin_nombre.suma(b.enviados, b.recibidos);
                    let k = if b.pais.is_empty() {
                        "sin_pais".to_owned()
                    } else {
                        b.pais.to_ascii_uppercase()
                    };
                    medido
                        .sin_nombre_por_pais
                        .entry(k)
                        .or_default()
                        .suma(b.enviados, b.recibidos);
                }
            } else if es_local(&b.destino) {
                continue;
            } else {
                match empresa_de(&b.destino).map(|e| e.0) {
                    Some(e) if solo.as_deref().is_none_or(|s| s == e) => medido
                        .por_empresa
                        .entry(e.to_owned())
                        .or_default()
                        .suma(b.enviados, b.recibidos),
                    Some(_) => {}
                    None if solo.is_none() => medido.otros_nombres.suma(b.enviados, b.recibidos),
                    None => {}
                }
            }
            medido.disponible = true;
        }
    }

    let check = l.check()?;
    let cadena = Cadena {
        algoritmo: "row_hash = sha256(prev_hash || campos), docs/VERIFY.md".to_owned(),
        genesis: identity::genesis().to_hex(),
        clave_desarrollo: identity::public_key_is_dev(),
        comprobada: check.is_ok(),
        fallo: check
            .first_fault
            .as_ref()
            .map(|(id, f)| format!("{id} {}", texto_fallo(f))),
        filas_comprobadas: check.checked,
        recortadas: check.pruned,
        cabeza: l.cabeza_cadena()?,
        filas_periodo: filas,
        primera,
        ultima,
        huella_filas: hex(&huella.finalize()),
    };

    let registro_desde = l.primer_evento_ts()?;
    let mut sin_vigilar: Vec<(i64, i64)> = l
        .gaps()?
        .into_iter()
        .filter(|g| g.to_ts > op.desde && g.from_ts < op.hasta)
        .map(|g| (g.from_ts.max(op.desde), g.to_ts.min(op.hasta)))
        .collect();
    sin_vigilar.sort_unstable();
    let cobertura = Cobertura {
        completa: registro_desde.is_some_and(|t| t <= op.desde),
        registro_desde,
        sin_vigilar_ms: sin_vigilar.iter().map(|(a, b)| b - a).sum(),
        sin_vigilar,
    };

    Ok(Certificado {
        formato: FORMATO.to_owned(),
        programa: env!("CARGO_PKG_VERSION").to_owned(),
        generado: ahora,
        generado_utc: rfc3339_utc(ahora),
        desde_dia: texto_de_dia(local_day_index(op.desde, zona)),
        hasta_dia: texto_de_dia(local_day_index(op.hasta.max(op.desde + 1) - 1, zona)),
        opciones: op,
        cobertura,
        contenido,
        medido,
        cadena,
    })
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// The certificate as the file is written: pretty JSON and a final new line. The seal is the
/// SHA-256 of exactly these bytes.
#[must_use]
pub fn a_json(c: &Certificado) -> String {
    let mut s = serde_json::to_string_pretty(c).unwrap_or_default();
    s.push('\n');
    s
}

/// SHA-256 of a file's bytes, hex: the seal.
#[must_use]
pub fn sello(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// The seal file, in the format `sha256sum -c` reads: `<hex>  <file name>`.
#[must_use]
pub fn linea_sha256(sello: &str, archivo: &str) -> String {
    format!("{sello}  {archivo}\n")
}

/// The file name of a certificate: `guardiana-certificado-2026-09-01_2026-09-30.json`, with the
/// company for an extract.
#[must_use]
pub fn nombre_archivo(c: &Certificado, extension: &str) -> String {
    let mut base = format!("guardiana-certificado-{}_{}", c.desde_dia, c.hasta_dia);
    if let Some(e) = &c.opciones.solo_empresa {
        let limpio: String = e
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() {
                    ch.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect();
        base.push('-');
        base.push_str(limpio.trim_matches('-'));
    }
    format!("{base}.{extension}")
}

// ----- checking it --------------------------------------------------------------------------

/// What checking a certificate against this ledger found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Estado {
    /// Rebuilt from the ledger, it says exactly the same.
    Coincide,
    /// The rows are the same ones (same chain reference), but the program now names some
    /// companies differently (another version of the lists).
    MismasFilasOtraClasificacion,
    /// Rows of the period were pruned since (retention): only the seal can be checked now.
    FilasRecortadas,
    /// The ledger does not hold the rows this certificate points to: another computer, rows
    /// altered or removed, or the file was edited.
    NoCoincide,
    /// Not a certificate of this format.
    FormatoDesconocido,
    /// Not readable as a certificate.
    Ilegible,
}

/// The result of [`comprobar`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comprobacion {
    /// The verdict.
    pub estado: Estado,
    /// SHA-256 of the bytes given: compare it with the `.sha256` file or wherever it was shared.
    pub sello: String,
    /// Whether the ledger's chain verifies now.
    pub cadena_ok: bool,
    /// The version that made it, and this one.
    pub programa_certificado: String,
    /// This version.
    pub programa_ahora: String,
    /// Whether the measured bytes still add up the same (they are not chained; pruned with the
    /// detail).
    pub medido_coincide: bool,
    /// The certificate as read, when it was readable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub periodo: Option<(String, String)>,
}

/// Check a certificate file against this ledger: rebuild it with its own options and compare.
#[must_use]
pub fn comprobar(bytes: &[u8], l: &Ledger) -> Comprobacion {
    let mut out = Comprobacion {
        estado: Estado::Ilegible,
        sello: sello(bytes),
        cadena_ok: l.check().is_ok_and(|c| c.is_ok()),
        programa_certificado: String::new(),
        programa_ahora: env!("CARGO_PKG_VERSION").to_owned(),
        medido_coincide: false,
        periodo: None,
    };
    let Ok(c) = serde_json::from_slice::<Certificado>(bytes) else {
        let formato = serde_json::from_slice::<serde_json::Value>(bytes)
            .ok()
            .and_then(|v| v.get("formato").and_then(|f| f.as_str()).map(str::to_owned));
        if formato.is_some_and(|f| f != FORMATO) {
            out.estado = Estado::FormatoDesconocido;
        }
        return out;
    };
    out.programa_certificado = c.programa.clone();
    out.periodo = Some((c.desde_dia.clone(), c.hasta_dia.clone()));
    if c.formato != FORMATO {
        out.estado = Estado::FormatoDesconocido;
        return out;
    }
    let Ok(ahora) = generar(l, &c.opciones, c.generado) else {
        return out;
    };
    out.medido_coincide = ahora.medido == c.medido;
    let mismas_filas = ahora.cadena.filas_periodo == c.cadena.filas_periodo
        && ahora.cadena.huella_filas == c.cadena.huella_filas
        && ahora.cadena.primera == c.cadena.primera
        && ahora.cadena.ultima == c.cadena.ultima;
    out.estado = if mismas_filas && ahora.contenido == c.contenido {
        Estado::Coincide
    } else if mismas_filas && c.programa != ahora.programa {
        // Another version may name some companies differently. The same version giving other
        // figures for the same rows means the file was edited.
        Estado::MismasFilasOtraClasificacion
    } else if ahora.cadena.filas_periodo < c.cadena.filas_periodo
        && ahora
            .cobertura
            .registro_desde
            .is_none_or(|t| t > c.opciones.desde)
        && ahora.cadena.recortadas > c.cadena.recortadas
    {
        Estado::FilasRecortadas
    } else {
        Estado::NoCoincide
    };
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use guardiana_core::{Category, Hash, NewEvent, ServerInfo, ServerKind};

    const ZONA: i64 = -300; // Bogotá
    fn ms(dia: &str, hora: i64) -> i64 {
        local_midnight(dia_de_texto(dia).unwrap(), ZONA) + hora * 3_600_000
    }

    fn evento(ts: i64, dev: &str, q: &str, cat: Category, v: Verdict, pais: &str) -> NewEvent {
        let mut e = NewEvent::observed(ts, dev, "192.168.1.20", q, "A");
        e.category = cat;
        e.verdict = v;
        if v != Verdict::Cortado {
            e.server = Some(ServerInfo {
                kind: ServerKind::Publico,
                country: pais.to_owned(),
                asn: 0,
                base: "muestra-2026-10".to_owned(),
            });
        }
        e
    }

    fn libro() -> Ledger {
        let mut l = Ledger::open_in_memory(identity::genesis()).unwrap();
        for e in [
            evento(
                ms("2026-08-31", 23),
                "self",
                "www.google.com",
                Category::Esperado,
                Verdict::Observado,
                "US",
            ),
            evento(
                ms("2026-09-01", 10),
                "self",
                "www.google.com",
                Category::Esperado,
                Verdict::Observado,
                "US",
            ),
            evento(
                ms("2026-09-01", 11),
                "self",
                "googleads.g.doubleclick.net",
                Category::Publicidad,
                Verdict::Cortado,
                "",
            ),
            evento(
                ms("2026-09-03", 9),
                "self",
                "graph.facebook.com",
                Category::Rastreador,
                Verdict::Observado,
                "IE",
            ),
            evento(
                ms("2026-09-04", 9),
                "tel",
                "graph.facebook.com",
                Category::Rastreador,
                Verdict::Observado,
                "IE",
            ),
            evento(
                ms("2026-09-05", 9),
                "self",
                "algo-sin-dueno.example",
                Category::Desconocido,
                Verdict::Observado,
                "CO",
            ),
            evento(
                ms("2026-09-05", 10),
                "self",
                "1.1.168.192.in-addr.arpa",
                Category::Desconocido,
                Verdict::Observado,
                "",
            ),
            evento(
                ms("2026-10-01", 1),
                "self",
                "www.google.com",
                Category::Esperado,
                Verdict::Observado,
                "US",
            ),
        ] {
            l.append(e).unwrap();
        }
        l
    }

    fn septiembre(dominios: bool, solo: Option<&str>) -> Opciones {
        let (desde, hasta) = periodo_mes("2026-09", ZONA).unwrap();
        Opciones {
            desde,
            hasta,
            zona_min: ZONA,
            dominios,
            solo_empresa: solo.map(str::to_owned),
        }
    }

    const AHORA: i64 = 1_800_000_000_000;

    #[test]
    fn months_and_days_are_the_persons_own() {
        let (a, b) = periodo_mes("2026-09", ZONA).unwrap();
        assert_eq!(rfc3339_utc(a), "2026-09-01T05:00:00.000Z");
        assert_eq!(rfc3339_utc(b), "2026-10-01T05:00:00.000Z");
        let (a2, b2) = periodo_dias("2026-09-01", "2026-09-30", ZONA).unwrap();
        assert_eq!((a, b), (a2, b2));
        assert!(periodo_dias("2026-09-30", "2026-09-01", ZONA).is_none());
        assert!(periodo_mes("2026-13", ZONA).is_none());
        assert_eq!(mes_anterior(ms("2026-10-18", 12), ZONA), "2026-09");
        assert_eq!(mes_anterior(ms("2026-01-05", 12), ZONA), "2025-12");
    }

    #[test]
    fn aggregates_by_company_without_addresses_or_history() {
        let l = libro();
        let c = generar(&l, &septiembre(false, None), AHORA).unwrap();
        let k = &c.contenido;
        // Only this computer, only September: 5 rows (the phone's and August's/October's out).
        assert_eq!(c.cadena.filas_periodo, 5);
        assert_eq!(k.totales.locales, 1);
        assert_eq!(k.totales.recuento.consultas, 4);
        assert_eq!(k.totales.recuento.cortadas, 1);
        let google = k.empresas.iter().find(|e| e.empresa == "Google").unwrap();
        assert_eq!(google.recuento.consultas, 2);
        assert_eq!(google.recuento.cortadas, 1);
        assert_eq!(google.dias, ["2026-09-01"]);
        // Its cut query of the same day does not add a day of contact.
        assert_eq!(google.recuento.respondidas, 1);
        assert_eq!(google.por_pais_servidor.get("US"), Some(&1));
        assert_eq!(google.por_categoria.get("publicidad"), Some(&1));
        assert!(google.dominios.is_empty(), "names are off by default");
        let meta = k.empresas.iter().find(|e| e.empresa == "Meta").unwrap();
        assert_eq!(meta.recuento.consultas, 1);
        let sin = k.sin_empresa.as_ref().unwrap();
        assert_eq!((sin.recuento.consultas, sin.nombres), (1, 1));
        // Meta and Google were contacted; a reference company that was not is listed as such.
        assert!(!k
            .no_contactadas
            .iter()
            .any(|n| n.empresa == "Google" || n.empresa == "Meta"));
        assert!(k.no_contactadas.iter().any(|n| n.empresa == "Criteo"));
        assert!(k.referencia.starts_with("fichas"));
        let json = a_json(&c);
        assert!(!json.contains("192.168"), "never an address");
        assert!(
            !json.contains("algo-sin-dueno"),
            "unknown names are only counted"
        );
        assert!(
            !json.contains("graph.facebook.com"),
            "no names unless asked"
        );
        assert!(c.cadena.comprobada);
    }

    #[test]
    fn names_only_when_asked_and_extract_only_its_company() {
        let l = libro();
        let c = generar(&l, &septiembre(true, None), AHORA).unwrap();
        let google = c
            .contenido
            .empresas
            .iter()
            .find(|e| e.empresa == "Google")
            .unwrap();
        assert_eq!(google.dominios.len(), 2);
        let solo = generar(&l, &septiembre(false, Some("Meta")), AHORA).unwrap();
        assert_eq!(solo.contenido.empresas.len(), 1);
        assert_eq!(solo.contenido.empresas[0].empresa, "Meta");
        assert!(solo.contenido.sin_empresa.is_none());
        assert!(solo.contenido.no_contactadas.is_empty());
        assert_eq!(solo.contenido.totales.recuento.consultas, 1);
        // Same rows behind both: the chain reference is the same.
        assert_eq!(solo.cadena.huella_filas, c.cadena.huella_filas);
        assert!(nombre_archivo(&solo, "json").ends_with("_2026-09-30-meta.json"));
    }

    #[test]
    fn the_period_never_reaches_past_its_making() {
        let l = libro();
        let op = septiembre(false, None);
        let c = generar(&l, &op, ms("2026-09-02", 0)).unwrap();
        assert_eq!(c.opciones.hasta, ms("2026-09-02", 0));
        assert_eq!(c.cadena.filas_periodo, 2);
        assert_eq!(c.hasta_dia, "2026-09-01");
    }

    #[test]
    fn checking_rebuilds_and_compares() {
        let mut l = libro();
        let c = generar(&l, &septiembre(false, None), AHORA).unwrap();
        let json = a_json(&c);
        let r = comprobar(json.as_bytes(), &l);
        assert_eq!(r.estado, Estado::Coincide);
        assert_eq!(r.sello, sello(json.as_bytes()));
        assert!(r.cadena_ok);
        // An edited file: the totals no longer match the ledger.
        let editado = json.replacen("\"cortadas\": 1", "\"cortadas\": 0", 1);
        assert_ne!(editado, json);
        assert_eq!(comprobar(editado.as_bytes(), &l).estado, Estado::NoCoincide);
        // The same rows, other figures, made by another version of the program.
        let otra_version = editado.replacen(env!("CARGO_PKG_VERSION"), "0.0.1", 1);
        assert_eq!(
            comprobar(otra_version.as_bytes(), &l).estado,
            Estado::MismasFilasOtraClasificacion
        );
        // Another ledger (another computer): not the same rows.
        let otro = Ledger::open_in_memory(identity::genesis()).unwrap();
        assert_eq!(comprobar(json.as_bytes(), &otro).estado, Estado::NoCoincide);
        // A row appended later inside the period (it cannot happen on time, but a forged one
        // would): the reference no longer matches.
        l.append(evento(
            ms("2026-09-10", 1),
            "self",
            "x.example",
            Category::Desconocido,
            Verdict::Observado,
            "US",
        ))
        .unwrap();
        assert_eq!(comprobar(json.as_bytes(), &l).estado, Estado::NoCoincide);
        assert_eq!(comprobar(b"no es json", &l).estado, Estado::Ilegible);
        assert_eq!(
            comprobar(br#"{"formato":"otra-cosa/9"}"#, &l).estado,
            Estado::FormatoDesconocido
        );
    }

    #[test]
    fn the_seal_is_sha256sum() {
        let s = sello(b"abc");
        assert_eq!(
            s,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(linea_sha256(&s, "x.json"), format!("{s}  x.json\n"));
        // The chain reference of no rows is the hash of nothing, not a blank.
        let l = Ledger::open_in_memory(Hash::of(b"k")).unwrap();
        let c = generar(&l, &septiembre(false, None), AHORA).unwrap();
        assert_eq!(c.cadena.huella_filas, sello(b""));
        assert!(!c.cobertura.completa);
    }
}
