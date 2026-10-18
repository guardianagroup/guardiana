//! «Contrato vivo» (1.0.45): which companies each program on this computer normally contacts,
//! learned from what it asked for, and a calm notice when it starts contacting new ones.
//!
//! The owner, 18 Oct 2026: «la experiencia del cliente debe ser sentir el monitoreo constante…
//! para saber que todo está bien y nada está vigilando o extrayendo sus datos». A program that
//! updates and, from one day to the next, starts asking for the names of three companies it
//! never asked for before is exactly the change a person would want to hear about, and only that.
//!
//! How it works:
//! - The source is the extract itself: every row of this computer where Windows said which
//!   program asked (`process_json`). Nothing new is collected for this.
//! - Each program has a *contract*: the companies it was seen contacting while it was being
//!   learned. For [`APRENDE_DIAS`] days after GUARDIANA first sees a program (or sooner, once it
//!   was started [`APRENDE_LANZAMIENTOS`] times over more than a day) nothing is flagged: that is
//!   the learning ([`EstadoContrato::Aprendiendo`]).
//! - After that, a company outside the contract opens a notice: «Zoom 6.2 ha cambiado: empezó a
//!   contactar con 3 empresas nuevas», with the contract before and the new companies after. The
//!   person can accept them as normal, cut only the new ones, or look at the detail.
//! - A contract can be *fixed* ([`EstadoContrato::Fijado`]): then whatever new company the program
//!   starts contacting is cut automatically, and the notice says it was.
//! - Browsers and shared Windows hosts have no contract ([`EstadoContrato::NoAplica`]): they talk
//!   to whatever the person visits, or on behalf of many services, so «new» means nothing there.
//!
//! What it is not: a name asked for is not data sent. GUARDIANA sees that a program asked the
//! system where a company's server is; it does not see what, if anything, was sent there. Every
//! sentence says «empezó a contactar», never more. And the cut is the same DNS cut as anywhere
//! else in GUARDIANA, so its limits are the same: the first query of a new company has already
//! been answered by the time Windows says which program asked (a second or so later); the cut
//! starts from the next one, and the page says so.
//!
//! Everything that happens to a contract (learnt, version changed, new companies, accepted, cut,
//! fixed) is written in its own hash-chained register, `contratos_registro`, rooted in the same
//! genesis as the extract (the hash of the installer's key) and carrying, row by row, the head of
//! the extract's chain at that moment. The cuts themselves are ordinary rules, so every query they
//! stop is a row of the extract with its rule's number.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::hash::Hash;
use crate::ledger::{Ledger, KEY_GENESIS, KEY_HEAD};
use crate::model::{Action, MatchKind, NewRule, Proceso, Scope};
use crate::time::DAY_MS;
use crate::SELF_DEVICE_ID;

/// Days a program is learned before anything is flagged.
pub const APRENDE_DIAS: i64 = 7;
/// Launches (distinct processes) that end the learning sooner, if they span more than a day.
pub const APRENDE_LANZAMIENTOS: u32 = 10;
/// A program seen contacting more companies than this while learning has no contract: it is a
/// browser or something like one, and «new» would be every page the person opens.
pub const MAX_EMPRESAS_CONTRATO: usize = 60;
/// Names kept per company in a contract: enough to recognise it, not a diary.
const MAX_NOMBRES: usize = 12;
/// Sites kept per company.
const MAX_SITIOS: usize = 8;
/// Programs with a contract at most.
const MAX_CONTRATOS: usize = 400;
/// Notices kept at most; the oldest resolved ones go first.
const MAX_AVISOS: usize = 500;
/// Versions remembered per program.
const MAX_VERSIONES: usize = 20;
/// Rows read from the extract per pass.
const LOTE: i64 = 5_000;
/// `created_by` of the rules a contract makes.
pub const CREADA_POR_CONTRATO: &str = "contrato vivo";

/// Settings key: the last extract row a pass has read.
const KEY_CURSOR: &str = "contratos_cursor";
/// Settings key: the register's chain head, as hex.
const KEY_REG_HEAD: &str = "contratos_registro_head";
/// Settings key: rows the register has written since its genesis.
const KEY_REG_COUNT: &str = "contratos_registro_count";

/// Programs that talk to whatever the person visits: no contract can describe them.
const NAVEGADORES: [&str; 11] = [
    "chrome.exe",
    "msedge.exe",
    "firefox.exe",
    "brave.exe",
    "opera.exe",
    "vivaldi.exe",
    "iexplore.exe",
    "chromium.exe",
    "arc.exe",
    "msedgewebview2.exe",
    "librewolf.exe",
];
/// Windows hosts that ask on behalf of many services at once: Windows does not say which.
const COMPARTIDOS: [&str; 3] = ["svchost.exe", "system", "dllhost.exe"];

/// Where a program's contract stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoContrato {
    /// Being learned: nothing is flagged yet.
    Aprendiendo,
    /// Learned: a new company opens a notice.
    Vigilando,
    /// Fixed by the person: a new company is cut automatically and recorded.
    Fijado,
    /// No contract makes sense for it (a browser, a shared Windows host, or too varied).
    NoAplica,
}

/// One company in a contract or in a notice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EmpresaVista {
    /// The names asked for under it (at most [`MAX_NOMBRES`]).
    pub nombres: BTreeSet<String>,
    /// The sites (registrable domains) of those names: what a cut would cut.
    pub sitios: BTreeSet<String>,
    /// First time, Unix ms.
    pub desde: i64,
    /// Last time, Unix ms.
    pub ultimo: i64,
    /// Times it was asked for.
    pub veces: u64,
    /// The program's version when it was first seen asking for it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
    /// One of its names is something the computer needs (certificates, updates, time): never
    /// cut by a contract, as no rule of GUARDIANA cuts it without the person's confirmation.
    #[serde(default)]
    pub esperado: bool,
}

impl EmpresaVista {
    fn anota(&mut self, o: &Observacion<'_>) {
        if self.desde == 0 || o.ts < self.desde {
            self.desde = o.ts;
        }
        self.ultimo = self.ultimo.max(o.ts);
        self.veces += 1;
        if self.nombres.len() < MAX_NOMBRES {
            self.nombres.insert(o.qname.to_owned());
        }
        if !o.sitio.is_empty() && self.sitios.len() < MAX_SITIOS {
            self.sitios.insert(o.sitio.to_owned());
        }
        self.esperado |= o.esperado;
    }
}

/// A version a program was seen with, and since when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionVista {
    /// The label shown (the declared version, else `huella` and the start of the file's hash).
    pub etiqueta: String,
    /// First time seen with it, Unix ms.
    pub desde: i64,
}

/// What GUARDIANA has learnt about one program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contrato {
    /// Identity: the program's path with version folders folded (see [`clave_de`]).
    pub clave: String,
    /// The program's file name (`Zoom.exe`).
    pub nombre: String,
    /// The last full path seen.
    pub ruta: String,
    /// The version its file declares, last seen; may be empty.
    #[serde(default)]
    pub version: String,
    /// SHA-256 of its file, last seen; may be empty.
    #[serde(default)]
    pub huella: String,
    /// Who signs it, last seen; may be empty.
    #[serde(default)]
    pub firmado_por: String,
    /// The version label when the contract was last settled (learnt or accepted).
    #[serde(default)]
    pub version_contrato: String,
    /// First seen, Unix ms.
    pub desde: i64,
    /// Last seen, Unix ms.
    pub ultimo: i64,
    /// Distinct processes seen: launches, as close as Windows lets one count them.
    pub lanzamientos: u32,
    /// The last processes seen, to count launches.
    #[serde(default)]
    pids: Vec<u32>,
    /// Where it stands.
    pub estado: EstadoContrato,
    /// Why there is no contract (`navegador`, `compartido`, `variado`), when there is none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub motivo: String,
    /// When the learning ended, Unix ms.
    #[serde(default)]
    pub aprendido: Option<i64>,
    /// The contract: company → what was seen of it.
    pub empresas: BTreeMap<String, EmpresaVista>,
    /// New companies in a notice not resolved yet.
    #[serde(default)]
    pub pendientes: BTreeSet<String>,
    /// New companies the person (or the fixed contract) cut: not flagged again.
    #[serde(default)]
    pub cortadas: BTreeSet<String>,
    /// Versions seen, oldest first (at most [`MAX_VERSIONES`]).
    #[serde(default)]
    pub versiones: Vec<VersionVista>,
}

/// One query of a program, as the contract reads it.
#[derive(Debug, Clone, Copy)]
pub struct Observacion<'a> {
    /// Unix ms.
    pub ts: i64,
    /// The name asked for, lowercase, no final dot.
    pub qname: &'a str,
    /// The company it belongs to (the lists' name, else its site).
    pub empresa: &'a str,
    /// Its site (registrable domain).
    pub sitio: &'a str,
    /// The name is something the computer needs (category `esperado`).
    pub esperado: bool,
    /// The program, as Windows said.
    pub proceso: &'a Proceso,
}

/// What one observation changed in a contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Efecto {
    /// The program's version (or its file) changed.
    Version {
        /// Label before.
        antes: String,
        /// Label now.
        ahora: String,
    },
    /// The learning ended.
    Aprendido,
    /// It turned out to have no contract.
    NoAplica,
    /// A company outside the contract. `primera` the first time it is seen.
    Novedad {
        /// The company.
        empresa: String,
        /// First time for this company (it was not pending already).
        primera: bool,
    },
}

/// The identity of a program across its updates: its path, lowercase, with the folders that carry
/// a version folded into `*`. Discord, Slack, Teams and others install every update in a folder
/// of its own (`app-1.0.9013`), and without this every update would look like a new program
/// learning from zero, which is exactly when the contract is wanted.
#[must_use]
pub fn clave_de(ruta: &str, nombre: &str) -> String {
    let base = if ruta.trim().is_empty() { nombre } else { ruta };
    base.trim()
        .to_lowercase()
        .split(['\\', '/'])
        .map(|seg| if parece_version(seg) { "*" } else { seg })
        .collect::<Vec<_>>()
        .join("\\")
}

/// `1.2.3`, `app-1.0.9013`, `v6.2`, `6.2.1_x64`: a folder named after a version.
fn parece_version(seg: &str) -> bool {
    let s = seg
        .strip_prefix("app-")
        .or_else(|| seg.strip_prefix('v'))
        .unwrap_or(seg);
    let s = s.split(['_', '-']).next().unwrap_or(s);
    s.contains('.')
        && s.chars().next().is_some_and(|c| c.is_ascii_digit())
        && s.chars().all(|c| c.is_ascii_digit() || c == '.')
}

/// The label of a version: the declared one, else `huella` and the first eight characters of
/// the file's hash, else nothing.
#[must_use]
pub fn etiqueta_version(version: &str, huella: &str) -> String {
    if !version.is_empty() {
        version.to_owned()
    } else if huella.len() >= 8 {
        format!("huella {}", &huella[..8])
    } else {
        String::new()
    }
}

/// Why a program has no contract from its name alone, if it has none.
#[must_use]
pub fn sin_contrato(nombre: &str) -> Option<&'static str> {
    let n = nombre.to_ascii_lowercase();
    if NAVEGADORES.contains(&n.as_str()) {
        Some("navegador")
    } else if COMPARTIDOS.contains(&n.as_str()) {
        Some("compartido")
    } else {
        None
    }
}

impl Contrato {
    /// A program seen for the first time.
    #[must_use]
    pub fn nuevo(p: &Proceso, ts: i64) -> Self {
        let motivo = sin_contrato(&p.nombre);
        let etiqueta = etiqueta_version(&p.version, &p.sha256);
        Self {
            clave: clave_de(&p.ruta, &p.nombre),
            nombre: p.nombre.clone(),
            ruta: p.ruta.clone(),
            version: p.version.clone(),
            huella: p.sha256.clone(),
            firmado_por: p.firmado_por.clone(),
            version_contrato: etiqueta.clone(),
            desde: ts,
            ultimo: ts,
            lanzamientos: 0,
            pids: Vec::new(),
            estado: if motivo.is_some() {
                EstadoContrato::NoAplica
            } else {
                EstadoContrato::Aprendiendo
            },
            motivo: motivo.unwrap_or_default().to_owned(),
            aprendido: None,
            empresas: BTreeMap::new(),
            pendientes: BTreeSet::new(),
            cortadas: BTreeSet::new(),
            versiones: if etiqueta.is_empty() {
                Vec::new()
            } else {
                vec![VersionVista {
                    etiqueta,
                    desde: ts,
                }]
            },
        }
    }

    /// The version label now.
    #[must_use]
    pub fn etiqueta(&self) -> String {
        etiqueta_version(&self.version, &self.huella)
    }

    /// Whether the learning is over at `ts`.
    #[must_use]
    pub fn aprendizaje_cumplido(&self, ts: i64) -> bool {
        let edad = ts - self.desde;
        edad >= APRENDE_DIAS * DAY_MS
            || (self.lanzamientos >= APRENDE_LANZAMIENTOS && edad > DAY_MS)
    }

    /// Ends the learning if its time has come (also called for programs that went quiet).
    pub fn revisa_aprendizaje(&mut self, ts: i64) -> Option<Efecto> {
        if self.estado == EstadoContrato::Aprendiendo && self.aprendizaje_cumplido(ts) {
            self.estado = EstadoContrato::Vigilando;
            self.aprendido = Some(ts);
            self.version_contrato = self.etiqueta();
            return Some(Efecto::Aprendido);
        }
        None
    }

    /// Reads one query of the program.
    pub fn observa(&mut self, o: &Observacion<'_>) -> Vec<Efecto> {
        let mut efectos = Vec::new();
        let p = o.proceso;
        // The learning is judged before this query changes anything: a contract learnt on 6.1
        // whose first query after the week comes from 6.2 was settled on 6.1.
        if let Some(e) = self.revisa_aprendizaje(o.ts) {
            efectos.push(e);
        }
        // A version changed only when both sides are known and differ: the first queries of a
        // program go without its version (Windows is still reading the file), and an empty
        // value turning into «6.2» is not an update.
        let antes = self.etiqueta();
        let cambio_version =
            !p.version.is_empty() && !self.version.is_empty() && p.version != self.version;
        let cambio_archivo =
            !p.sha256.is_empty() && !self.huella.is_empty() && p.sha256 != self.huella;
        if !p.version.is_empty() {
            self.version.clone_from(&p.version);
        }
        if !p.sha256.is_empty() {
            self.huella.clone_from(&p.sha256);
        }
        if !p.firmado_por.is_empty() {
            self.firmado_por.clone_from(&p.firmado_por);
        }
        if !p.ruta.is_empty() {
            self.ruta.clone_from(&p.ruta);
        }
        let ahora = self.etiqueta();
        if (cambio_version || cambio_archivo) && ahora != antes {
            efectos.push(Efecto::Version {
                antes,
                ahora: ahora.clone(),
            });
        }
        if !ahora.is_empty() && self.versiones.last().is_none_or(|v| v.etiqueta != ahora) {
            self.versiones.push(VersionVista {
                etiqueta: ahora.clone(),
                desde: o.ts,
            });
            if self.versiones.len() > MAX_VERSIONES {
                self.versiones.remove(0);
            }
        }
        if self.version_contrato.is_empty() && self.estado == EstadoContrato::Aprendiendo {
            self.version_contrato = ahora;
        }
        if p.pid != 0 && !self.pids.contains(&p.pid) {
            self.lanzamientos = self.lanzamientos.saturating_add(1);
            self.pids.push(p.pid);
            if self.pids.len() > 16 {
                self.pids.remove(0);
            }
        }
        self.ultimo = self.ultimo.max(o.ts);
        match self.estado {
            EstadoContrato::NoAplica => {}
            _ if self.empresas.contains_key(o.empresa) => {
                if let Some(e) = self.empresas.get_mut(o.empresa) {
                    e.anota(o);
                }
            }
            EstadoContrato::Aprendiendo => {
                let mut e = EmpresaVista {
                    version: self.etiqueta(),
                    ..EmpresaVista::default()
                };
                e.anota(o);
                self.empresas.insert(o.empresa.to_owned(), e);
                if self.empresas.len() > MAX_EMPRESAS_CONTRATO {
                    self.estado = EstadoContrato::NoAplica;
                    "variado".clone_into(&mut self.motivo);
                    self.empresas.clear();
                    efectos.push(Efecto::NoAplica);
                }
            }
            EstadoContrato::Vigilando | EstadoContrato::Fijado => {
                if !self.cortadas.contains(o.empresa) {
                    let primera = self.pendientes.insert(o.empresa.to_owned());
                    efectos.push(Efecto::Novedad {
                        empresa: o.empresa.to_owned(),
                        primera,
                    });
                }
            }
        }
        efectos
    }
}

/// Where a notice stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoAviso {
    /// Waiting for the person.
    Abierto,
    /// The person accepted the new companies as normal.
    Aceptado,
    /// The person cut the new companies.
    Cortado,
    /// The contract was fixed: the new companies were cut automatically.
    CortadoAuto,
    /// A cut the person undid: the companies became part of the contract.
    Deshecho,
}

/// A company of a notice that was not cut, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SinCortar {
    /// The company.
    pub empresa: String,
    /// `esperado` (the computer needs it) or `compartido` (another program's contract has it).
    pub motivo: String,
    /// The other program, when shared.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub con: String,
}

/// «Zoom 6.2 ha cambiado: empezó a contactar con 3 empresas nuevas».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Aviso {
    /// Row id.
    pub id: i64,
    /// The program's identity.
    pub clave: String,
    /// The program's file name.
    pub programa: String,
    /// The version the contract was settled with.
    pub version_antes: String,
    /// The version when the new companies were seen.
    pub version_ahora: String,
    /// First new company seen, Unix ms.
    pub desde: i64,
    /// Last time any of them was asked for, Unix ms.
    pub ultimo: i64,
    /// The contract's companies when the notice opened (the «before»).
    pub antes: Vec<String>,
    /// The new companies (the «after»).
    pub nuevas: BTreeMap<String, EmpresaVista>,
    /// Where it stands.
    pub estado: EstadoAviso,
    /// When it was resolved, Unix ms.
    #[serde(default)]
    pub resuelto: Option<i64>,
    /// The rules the cut made.
    #[serde(default)]
    pub reglas: Vec<i64>,
    /// The companies left uncut, and why.
    #[serde(default)]
    pub sin_cortar: Vec<SinCortar>,
}

impl Aviso {
    /// The version changed between the contract and the notice.
    #[must_use]
    pub fn version_cambio(&self) -> bool {
        !self.version_antes.is_empty()
            && !self.version_ahora.is_empty()
            && self.version_antes != self.version_ahora
    }
}

/// One row of the register.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Registro {
    /// Row id.
    pub id: i64,
    /// Unix ms.
    pub ts: i64,
    /// The program's identity.
    pub clave: String,
    /// What happened: `aprendiendo`, `aprendido`, `no_aplica`, `version`, `novedad`, `aceptado`,
    /// `cortado`, `cortado_auto`, `deshecho`, `fijado`, `soltado`.
    pub tipo: String,
    /// Its detail, as JSON.
    pub detalle: String,
    /// The extract's chain head when it was written.
    pub cabeza_extracto: String,
    /// This row's hash.
    pub hash: String,
}

/// What [`Ledger::comprueba_contratos`] found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ComprobacionRegistro {
    /// Every row links to the one before and the last one is where the head says.
    pub ok: bool,
    /// Rows checked.
    pub filas: u64,
    /// The first row that does not verify, if any; `0` when rows are missing at the end.
    pub fallo: Option<i64>,
}

/// What one pass did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Pasada {
    /// Extract rows read.
    pub leidas: usize,
    /// Notices opened.
    pub avisos_nuevos: usize,
    /// Rules made by fixed contracts.
    pub cortes_auto: usize,
}

/// How a name is read: its company and site, or `None` for names that say nothing about a
/// program's contract (local names, reverse lookups, GUARDIANA's own self-check).
pub struct Lectura {
    /// The company (the lists' name, else the site).
    pub empresa: String,
    /// The site (registrable domain).
    pub sitio: String,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS contratos (
    clave TEXT PRIMARY KEY,
    json  TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS contratos_avisos (
    id     INTEGER PRIMARY KEY AUTOINCREMENT,
    clave  TEXT    NOT NULL,
    estado TEXT    NOT NULL,
    ts     INTEGER NOT NULL,
    json   TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS contratos_avisos_clave ON contratos_avisos(clave, estado);
CREATE TABLE IF NOT EXISTS contratos_registro (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    ts              INTEGER NOT NULL,
    clave           TEXT    NOT NULL,
    tipo            TEXT    NOT NULL,
    detalle         TEXT    NOT NULL,
    cabeza_extracto TEXT    NOT NULL,
    prev_hash       BLOB    NOT NULL,
    row_hash        BLOB    NOT NULL
);
";

/// The register's genesis: the extract's genesis, under a name of its own, so a row of one chain
/// can never be passed off as a row of the other.
fn genesis_registro(genesis_hex: &str) -> Hash {
    let mut b = b"guardiana-contratos:".to_vec();
    b.extend_from_slice(genesis_hex.as_bytes());
    Hash::of(&b)
}

/// `sha256(prev || len ts || len clave || len tipo || len detalle || len cabeza)`, each field as
/// a 4-byte big-endian length and its UTF-8 bytes, like the extract's rows (`hash.rs`).
#[must_use]
pub fn hash_registro(
    prev: Hash,
    ts: i64,
    clave: &str,
    tipo: &str,
    detalle: &str,
    cabeza: &str,
) -> Hash {
    let mut b = prev.0.to_vec();
    for campo in [ts.to_string().as_str(), clave, tipo, detalle, cabeza] {
        let n = u32::try_from(campo.len()).unwrap_or(u32::MAX);
        b.extend_from_slice(&n.to_be_bytes());
        b.extend_from_slice(campo.as_bytes());
    }
    Hash::of(&b)
}

fn estado_aviso_texto(e: EstadoAviso) -> &'static str {
    match e {
        EstadoAviso::Abierto => "abierto",
        EstadoAviso::Aceptado => "aceptado",
        EstadoAviso::Cortado => "cortado",
        EstadoAviso::CortadoAuto => "cortado_auto",
        EstadoAviso::Deshecho => "deshecho",
    }
}

impl Ledger {
    pub(crate) fn ensure_contratos_tables(&self) -> Result<()> {
        self.conn.execute_batch(SCHEMA)?;
        Ok(())
    }

    /// Every contract, by identity.
    pub fn contratos(&self) -> Result<Vec<Contrato>> {
        let mut stmt = self
            .conn
            .prepare("SELECT json FROM contratos ORDER BY clave")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for r in rows {
            if let Ok(c) = serde_json::from_str::<Contrato>(&r?) {
                out.push(c);
            }
        }
        Ok(out)
    }

    /// One contract.
    pub fn contrato(&self, clave: &str) -> Result<Option<Contrato>> {
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT json FROM contratos WHERE clave = ?1",
                [clave],
                |r| r.get(0),
            )
            .optional()?;
        Ok(json.and_then(|j| serde_json::from_str(&j).ok()))
    }

    fn guarda_contrato(&self, c: &Contrato) -> Result<()> {
        self.conn.execute(
            "INSERT INTO contratos(clave, json) VALUES (?1, ?2) \
             ON CONFLICT(clave) DO UPDATE SET json = excluded.json",
            params![c.clave, serde_json::to_string(c)?],
        )?;
        Ok(())
    }

    /// The notices, newest first; `solo_abiertos` keeps the ones waiting for the person.
    pub fn avisos_contrato(&self, solo_abiertos: bool, limite: usize) -> Result<Vec<Aviso>> {
        let sql = if solo_abiertos {
            "SELECT id, json FROM contratos_avisos WHERE estado = 'abierto' \
             ORDER BY ts DESC, id DESC LIMIT ?1"
        } else {
            "SELECT id, json FROM contratos_avisos ORDER BY ts DESC, id DESC LIMIT ?1"
        };
        let mut stmt = self.conn.prepare(sql)?;
        let lim = i64::try_from(limite).unwrap_or(i64::MAX);
        let rows = stmt.query_map([lim], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        let mut out = Vec::new();
        for r in rows {
            let (id, json) = r?;
            if let Ok(mut a) = serde_json::from_str::<Aviso>(&json) {
                a.id = id;
                out.push(a);
            }
        }
        Ok(out)
    }

    /// One notice.
    pub fn aviso_contrato(&self, id: i64) -> Result<Option<Aviso>> {
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT json FROM contratos_avisos WHERE id = ?1",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(json
            .and_then(|j| serde_json::from_str::<Aviso>(&j).ok())
            .map(|mut a| {
                a.id = id;
                a
            }))
    }

    fn guarda_aviso(&self, a: &mut Aviso) -> Result<()> {
        let json = serde_json::to_string(&a)?;
        let estado = estado_aviso_texto(a.estado);
        if a.id == 0 {
            self.conn.execute(
                "INSERT INTO contratos_avisos(clave, estado, ts, json) VALUES (?1, ?2, ?3, ?4)",
                params![a.clave, estado, a.desde, json],
            )?;
            a.id = self.conn.last_insert_rowid();
            // Too many: the oldest resolved ones go.
            self.conn.execute(
                "DELETE FROM contratos_avisos WHERE estado != 'abierto' AND id IN \
                 (SELECT id FROM contratos_avisos WHERE estado != 'abierto' ORDER BY ts ASC \
                  LIMIT MAX(0, (SELECT COUNT(*) FROM contratos_avisos) - ?1))",
                [i64::try_from(MAX_AVISOS).unwrap_or(i64::MAX)],
            )?;
        } else {
            self.conn.execute(
                "UPDATE contratos_avisos SET estado = ?2, json = ?3 WHERE id = ?1",
                params![a.id, estado, json],
            )?;
        }
        Ok(())
    }

    /// Writes one row of the register, chained to the one before.
    pub(crate) fn registra_contrato(
        &self,
        ts: i64,
        clave: &str,
        tipo: &str,
        detalle: &serde_json::Value,
    ) -> Result<()> {
        let genesis = genesis_registro(&self.setting(KEY_GENESIS)?.unwrap_or_default());
        let prev = match self.setting(KEY_REG_HEAD)? {
            Some(h) => Hash::from_hex(&h)?,
            None => genesis,
        };
        let cabeza = self.setting(KEY_HEAD)?.unwrap_or_default();
        let detalle = detalle.to_string();
        let h = hash_registro(prev, ts, clave, tipo, &detalle, &cabeza);
        let n: u64 = self
            .setting(KEY_REG_COUNT)?
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        self.conn.execute(
            "INSERT INTO contratos_registro(ts, clave, tipo, detalle, cabeza_extracto, prev_hash, \
             row_hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                ts,
                clave,
                tipo,
                detalle,
                cabeza,
                prev.0.to_vec(),
                h.0.to_vec()
            ],
        )?;
        self.set_setting(KEY_REG_HEAD, &h.to_hex())?;
        self.set_setting(KEY_REG_COUNT, &(n + 1).to_string())?;
        Ok(())
    }

    /// The register, newest first; `clave` keeps one program's rows.
    pub fn registro_contratos(&self, clave: Option<&str>, limite: usize) -> Result<Vec<Registro>> {
        let lim = i64::try_from(limite).unwrap_or(i64::MAX);
        let mut stmt = self.conn.prepare(
            "SELECT id, ts, clave, tipo, detalle, cabeza_extracto, row_hash FROM contratos_registro \
             WHERE (?1 IS NULL OR clave = ?1) ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![clave, lim], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, Vec<u8>>(6)?,
            ))
        })?;
        let mut out = Vec::new();
        for r in rows {
            let (id, ts, clave, tipo, detalle, cabeza_extracto, h) = r?;
            out.push(Registro {
                id,
                ts,
                clave,
                tipo,
                detalle,
                cabeza_extracto,
                hash: Hash::from_bytes(&h).map(Hash::to_hex).unwrap_or_default(),
            });
        }
        Ok(out)
    }

    /// Recomputes the whole register from its genesis and checks it ends where its head says.
    pub fn comprueba_contratos(&self) -> Result<ComprobacionRegistro> {
        let mut prev = genesis_registro(&self.setting(KEY_GENESIS)?.unwrap_or_default());
        let mut stmt = self.conn.prepare(
            "SELECT id, ts, clave, tipo, detalle, cabeza_extracto, prev_hash, row_hash \
             FROM contratos_registro ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, Vec<u8>>(6)?,
                r.get::<_, Vec<u8>>(7)?,
            ))
        })?;
        let mut filas = 0u64;
        for r in rows {
            let (id, ts, clave, tipo, detalle, cabeza, p, h) = r?;
            filas += 1;
            let calculado = hash_registro(prev, ts, &clave, &tipo, &detalle, &cabeza);
            let guardado_prev = Hash::from_bytes(&p).ok();
            let guardado = Hash::from_bytes(&h).ok();
            if guardado_prev != Some(prev) || guardado != Some(calculado) {
                return Ok(ComprobacionRegistro {
                    ok: false,
                    filas,
                    fallo: Some(id),
                });
            }
            prev = calculado;
        }
        let cabeza = self.setting(KEY_REG_HEAD)?;
        let contadas: Option<u64> = self.setting(KEY_REG_COUNT)?.and_then(|c| c.parse().ok());
        let cuadra = match (cabeza, contadas) {
            (Some(h), Some(n)) => h == prev.to_hex() && n == filas,
            (None, _) => filas == 0,
            _ => false,
        };
        Ok(ComprobacionRegistro {
            ok: cuadra,
            filas,
            fallo: (!cuadra).then_some(0),
        })
    }

    /// One pass: reads the extract rows since the last pass where Windows said which program
    /// asked, learns, and opens (or adds to) notices. A fixed contract's new companies are cut
    /// here, in the same pass. `lee` says the company and site of a name, `None` to skip it.
    pub fn contratos_pasada(
        &mut self,
        lee: &dyn Fn(&str) -> Option<Lectura>,
        ahora: i64,
    ) -> Result<Pasada> {
        let cursor: i64 = self
            .setting(KEY_CURSOR)?
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        let filas: Vec<(i64, i64, String, String, String)> = {
            let mut stmt = self.conn.prepare(
                "SELECT id, ts, qname, process_json, category FROM events \
                 WHERE id > ?1 AND device_id = ?2 AND process_json != '' ORDER BY id LIMIT ?3",
            )?;
            let rows = stmt.query_map(params![cursor, SELF_DEVICE_ID, LOTE], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        let mut pasada = Pasada {
            leidas: filas.len(),
            ..Pasada::default()
        };
        let mut contratos: HashMap<String, Contrato> = self
            .contratos()?
            .into_iter()
            .map(|c| (c.clave.clone(), c))
            .collect();
        let mut tocados: BTreeSet<String> = BTreeSet::new();
        let mut ultimo_id = cursor;
        for (id, ts, qname, process_json, category) in filas {
            ultimo_id = id;
            let Ok(p) = serde_json::from_str::<Proceso>(&process_json) else {
                continue;
            };
            if p.nombre.trim().is_empty() {
                continue;
            }
            let qname = qname.trim_end_matches('.').to_ascii_lowercase();
            let Some(lectura) = lee(&qname) else {
                continue;
            };
            let clave = clave_de(&p.ruta, &p.nombre);
            if !contratos.contains_key(&clave) {
                if contratos.len() >= MAX_CONTRATOS {
                    continue;
                }
                let c = Contrato::nuevo(&p, ts);
                let tipo = if c.estado == EstadoContrato::NoAplica {
                    "no_aplica"
                } else {
                    "aprendiendo"
                };
                self.registra_contrato(
                    ts,
                    &clave,
                    tipo,
                    &serde_json::json!({
                        "programa": c.nombre, "version": c.etiqueta(), "motivo": c.motivo,
                        "firmado_por": c.firmado_por,
                    }),
                )?;
                contratos.insert(clave.clone(), c);
            }
            let Some(c) = contratos.get_mut(&clave) else {
                continue;
            };
            tocados.insert(clave.clone());
            let o = Observacion {
                ts,
                qname: &qname,
                empresa: &lectura.empresa,
                sitio: &lectura.sitio,
                esperado: category == "esperado",
                proceso: &p,
            };
            let efectos = c.observa(&o);
            let c = c.clone();
            for e in efectos {
                self.aplica_efecto(&c, &o, e, &mut pasada)?;
            }
        }
        // Programs that went quiet finish their learning too.
        for c in contratos.values_mut() {
            if let Some(e) = c.revisa_aprendizaje(ahora) {
                tocados.insert(c.clave.clone());
                self.registra_aprendido(c, ahora, &e)?;
            }
        }
        // Fixed contracts cut what is new, and the cuts change the contracts (`cortadas`).
        let fijados: Vec<String> = contratos
            .values()
            .filter(|c| c.estado == EstadoContrato::Fijado)
            .map(|c| c.clave.clone())
            .collect();
        for clave in fijados {
            for mut a in self.avisos_de(&clave, EstadoAviso::Abierto)? {
                pasada.cortes_auto += self.corta_aviso(&mut a, &mut contratos, ahora, true)?;
                tocados.insert(clave.clone());
            }
        }
        for clave in &tocados {
            if let Some(c) = contratos.get(clave) {
                // The pending set follows the notices: a cut or accepted company is not pending.
                self.guarda_contrato(c)?;
            }
        }
        self.set_setting(KEY_CURSOR, &ultimo_id.to_string())?;
        Ok(pasada)
    }

    fn registra_aprendido(&self, c: &Contrato, ts: i64, e: &Efecto) -> Result<()> {
        if *e == Efecto::Aprendido {
            self.registra_contrato(
                ts,
                &c.clave,
                "aprendido",
                &serde_json::json!({
                    "programa": c.nombre, "version": c.version_contrato,
                    "empresas": c.empresas.keys().collect::<Vec<_>>(),
                }),
            )?;
        }
        Ok(())
    }

    fn avisos_de(&self, clave: &str, estado: EstadoAviso) -> Result<Vec<Aviso>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, json FROM contratos_avisos WHERE clave = ?1 AND estado = ?2 ORDER BY id",
        )?;
        let rows = stmt.query_map(params![clave, estado_aviso_texto(estado)], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })?;
        let mut out = Vec::new();
        for r in rows {
            let (id, json) = r?;
            if let Ok(mut a) = serde_json::from_str::<Aviso>(&json) {
                a.id = id;
                out.push(a);
            }
        }
        Ok(out)
    }

    fn aplica_efecto(
        &self,
        c: &Contrato,
        o: &Observacion<'_>,
        e: Efecto,
        pasada: &mut Pasada,
    ) -> Result<()> {
        match e {
            Efecto::Version { antes, ahora } => self.registra_contrato(
                o.ts,
                &c.clave,
                "version",
                &serde_json::json!({ "programa": c.nombre, "antes": antes, "ahora": ahora }),
            ),
            Efecto::Aprendido => self.registra_aprendido(c, o.ts, &Efecto::Aprendido),
            Efecto::NoAplica => self.registra_contrato(
                o.ts,
                &c.clave,
                "no_aplica",
                &serde_json::json!({ "programa": c.nombre, "motivo": c.motivo }),
            ),
            Efecto::Novedad { empresa, primera } => {
                let mut aviso = self
                    .avisos_de(&c.clave, EstadoAviso::Abierto)?
                    .into_iter()
                    .next_back()
                    .unwrap_or_else(|| Aviso {
                        id: 0,
                        clave: c.clave.clone(),
                        programa: c.nombre.clone(),
                        version_antes: c.version_contrato.clone(),
                        version_ahora: c.etiqueta(),
                        desde: o.ts,
                        ultimo: o.ts,
                        antes: c.empresas.keys().cloned().collect(),
                        nuevas: BTreeMap::new(),
                        estado: EstadoAviso::Abierto,
                        resuelto: None,
                        reglas: Vec::new(),
                        sin_cortar: Vec::new(),
                    });
                let nuevo = aviso.id == 0;
                aviso.ultimo = aviso.ultimo.max(o.ts);
                if !c.etiqueta().is_empty() {
                    aviso.version_ahora = c.etiqueta();
                }
                let ev = aviso
                    .nuevas
                    .entry(empresa.clone())
                    .or_insert_with(|| EmpresaVista {
                        version: c.etiqueta(),
                        ..EmpresaVista::default()
                    });
                ev.anota(o);
                self.guarda_aviso(&mut aviso)?;
                if nuevo {
                    pasada.avisos_nuevos += 1;
                }
                if primera {
                    self.registra_contrato(
                        o.ts,
                        &c.clave,
                        "novedad",
                        &serde_json::json!({
                            "programa": c.nombre, "aviso": aviso.id, "empresa": empresa,
                            "nombre": o.qname, "version": c.etiqueta(),
                            "version_contrato": c.version_contrato,
                        }),
                    )?;
                }
                Ok(())
            }
        }
    }

    /// Cuts the new companies of a notice: one rule per site, for this computer, unless the
    /// computer needs it or another program's contract has it. Returns the rules made.
    fn corta_aviso(
        &mut self,
        a: &mut Aviso,
        contratos: &mut HashMap<String, Contrato>,
        ahora: i64,
        automatico: bool,
    ) -> Result<usize> {
        let activas: Vec<crate::model::Rule> = self
            .active_rules(ahora)?
            .into_iter()
            .filter(|r| {
                r.action == Action::Cortar
                    && r.scope == Scope::Device
                    && r.device_id.as_deref() == Some(SELF_DEVICE_ID)
            })
            .collect();
        let mut hechas = 0usize;
        let mut cortadas = Vec::new();
        a.sin_cortar.clear();
        for (empresa, ev) in &a.nuevas {
            if ev.esperado {
                a.sin_cortar.push(SinCortar {
                    empresa: empresa.clone(),
                    motivo: "esperado".into(),
                    con: String::new(),
                });
                continue;
            }
            let compartida = contratos.values().find(|otro| {
                otro.clave != a.clave
                    && otro.estado != EstadoContrato::NoAplica
                    && otro
                        .empresas
                        .values()
                        .any(|e| e.sitios.iter().any(|s| ev.sitios.contains(s)))
            });
            if let Some(otro) = compartida {
                a.sin_cortar.push(SinCortar {
                    empresa: empresa.clone(),
                    motivo: "compartido".into(),
                    con: otro.nombre.clone(),
                });
                continue;
            }
            for sitio in &ev.sitios {
                let ya = activas.iter().any(|r| {
                    (r.match_kind == MatchKind::Suffix || r.match_kind == MatchKind::Domain)
                        && r.pattern == *sitio
                });
                if ya {
                    continue;
                }
                let r = self.add_rule(NewRule {
                    scope: Scope::Device,
                    device_id: Some(SELF_DEVICE_ID.to_owned()),
                    match_kind: MatchKind::Suffix,
                    pattern: sitio.clone(),
                    action: Action::Cortar,
                    created_at: ahora,
                    created_by: CREADA_POR_CONTRATO.to_owned(),
                    expires_at: None,
                    confirmed: false,
                })?;
                a.reglas.push(r.id);
                hechas += 1;
            }
            cortadas.push(empresa.clone());
        }
        a.estado = if automatico {
            EstadoAviso::CortadoAuto
        } else {
            EstadoAviso::Cortado
        };
        a.resuelto = Some(ahora);
        if let Some(c) = contratos.get_mut(&a.clave) {
            for e in a.nuevas.keys() {
                c.pendientes.remove(e);
            }
            // The ones left uncut become part of the contract: they are what it is now.
            for s in &a.sin_cortar {
                if let Some(ev) = a.nuevas.get(&s.empresa) {
                    c.empresas.insert(s.empresa.clone(), ev.clone());
                }
            }
            c.cortadas.extend(cortadas.iter().cloned());
        }
        self.guarda_aviso(a)?;
        self.registra_contrato(
            ahora,
            &a.clave,
            if automatico {
                "cortado_auto"
            } else {
                "cortado"
            },
            &serde_json::json!({
                "programa": a.programa, "aviso": a.id, "empresas": cortadas, "reglas": a.reglas,
                "sin_cortar": a.sin_cortar,
            }),
        )?;
        Ok(hechas)
    }

    /// «Cortar solo lo nuevo»: cuts the new companies of an open notice. `None` when there is no
    /// such open notice.
    pub fn contrato_cortar(&mut self, id: i64, ahora: i64) -> Result<Option<Aviso>> {
        let Some(mut a) = self.aviso_contrato(id)? else {
            return Ok(None);
        };
        if a.estado != EstadoAviso::Abierto {
            return Ok(None);
        }
        let mut contratos: HashMap<String, Contrato> = self
            .contratos()?
            .into_iter()
            .map(|c| (c.clave.clone(), c))
            .collect();
        self.corta_aviso(&mut a, &mut contratos, ahora, false)?;
        if let Some(c) = contratos.get(&a.clave) {
            self.guarda_contrato(c)?;
        }
        Ok(Some(a))
    }

    /// «Aceptar como normal»: the new companies of an open notice join the contract.
    pub fn contrato_aceptar(&mut self, id: i64, ahora: i64) -> Result<Option<Aviso>> {
        let Some(mut a) = self.aviso_contrato(id)? else {
            return Ok(None);
        };
        if a.estado != EstadoAviso::Abierto {
            return Ok(None);
        }
        if let Some(mut c) = self.contrato(&a.clave)? {
            for (e, ev) in &a.nuevas {
                c.pendientes.remove(e);
                c.empresas.insert(e.clone(), ev.clone());
            }
            c.version_contrato = c.etiqueta();
            self.guarda_contrato(&c)?;
        }
        a.estado = EstadoAviso::Aceptado;
        a.resuelto = Some(ahora);
        self.guarda_aviso(&mut a)?;
        self.registra_contrato(
            ahora,
            &a.clave,
            "aceptado",
            &serde_json::json!({
                "programa": a.programa, "aviso": a.id,
                "empresas": a.nuevas.keys().collect::<Vec<_>>(),
            }),
        )?;
        Ok(Some(a))
    }

    /// Undoes a cut: its rules are undone and the companies join the contract.
    pub fn contrato_deshacer(&mut self, id: i64, ahora: i64) -> Result<Option<Aviso>> {
        let Some(mut a) = self.aviso_contrato(id)? else {
            return Ok(None);
        };
        if !matches!(a.estado, EstadoAviso::Cortado | EstadoAviso::CortadoAuto) {
            return Ok(None);
        }
        for r in &a.reglas {
            self.undo_rule(*r, ahora)?;
        }
        if let Some(mut c) = self.contrato(&a.clave)? {
            for (e, ev) in &a.nuevas {
                c.cortadas.remove(e);
                c.empresas.insert(e.clone(), ev.clone());
            }
            self.guarda_contrato(&c)?;
        }
        a.estado = EstadoAviso::Deshecho;
        a.resuelto = Some(ahora);
        self.guarda_aviso(&mut a)?;
        self.registra_contrato(
            ahora,
            &a.clave,
            "deshecho",
            &serde_json::json!({ "programa": a.programa, "aviso": a.id, "reglas": a.reglas }),
        )?;
        Ok(Some(a))
    }

    /// «Fijar» (`fijar`) or let go of a contract. Fixing ends the learning with what was learnt.
    /// `None` when there is no such program or it has no contract.
    pub fn contrato_fijar(
        &mut self,
        clave: &str,
        fijar: bool,
        ahora: i64,
    ) -> Result<Option<Contrato>> {
        let Some(mut c) = self.contrato(clave)? else {
            return Ok(None);
        };
        if c.estado == EstadoContrato::NoAplica {
            return Ok(None);
        }
        let antes = c.estado;
        c.estado = if fijar {
            EstadoContrato::Fijado
        } else if antes == EstadoContrato::Fijado {
            EstadoContrato::Vigilando
        } else {
            antes
        };
        if fijar && c.aprendido.is_none() {
            c.aprendido = Some(ahora);
            c.version_contrato = c.etiqueta();
        }
        if c.estado != antes {
            self.guarda_contrato(&c)?;
            self.registra_contrato(
                ahora,
                clave,
                if fijar { "fijado" } else { "soltado" },
                &serde_json::json!({
                    "programa": c.nombre, "version": c.etiqueta(),
                    "empresas": c.empresas.keys().collect::<Vec<_>>(),
                }),
            )?;
        }
        Ok(Some(c))
    }

    /// Deletes everything about contracts (part of «Borrar todo»).
    pub(crate) fn borra_contratos_en(tx: &rusqlite::Transaction<'_>) -> Result<()> {
        tx.execute_batch(
            "DELETE FROM contratos; DELETE FROM contratos_avisos; DELETE FROM contratos_registro; \
             DELETE FROM sqlite_sequence WHERE name IN ('contratos_avisos', 'contratos_registro'); \
             DELETE FROM settings WHERE key IN ('contratos_cursor', 'contratos_registro_head', \
             'contratos_registro_count');",
        )?;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::model::NewEvent;

    const H: i64 = 3_600_000;

    fn zoom(pid: u32, version: &str, sha: &str) -> Proceso {
        Proceso {
            pid,
            nombre: "Zoom.exe".into(),
            ruta: r"C:\Users\ana\AppData\Roaming\Zoom\bin\Zoom.exe".into(),
            sha256: sha.into(),
            firmado_por: "Zoom Video Communications, Inc.".into(),
            version: version.into(),
        }
    }

    fn lee(q: &str) -> Option<Lectura> {
        if q.ends_with(".arpa") || q.ends_with(".local") {
            return None;
        }
        let partes: Vec<&str> = q.rsplitn(3, '.').collect();
        let sitio = if partes.len() >= 2 {
            format!("{}.{}", partes[1], partes[0])
        } else {
            q.to_owned()
        };
        let empresa = match sitio.as_str() {
            "zoom.us" | "zoom.com" => "Zoom".to_owned(),
            "amplitude.com" => "Amplitude".to_owned(),
            "doubleclick.net" | "google.com" => "Google".to_owned(),
            other => other.to_owned(),
        };
        Some(Lectura { empresa, sitio })
    }

    fn obs<'a>(ts: i64, q: &'a str, empresa: &'a str, p: &'a Proceso) -> Observacion<'a> {
        Observacion {
            ts,
            qname: q,
            empresa,
            sitio: q,
            esperado: false,
            proceso: p,
        }
    }

    fn evento(l: &mut Ledger, ts: i64, q: &str, p: &Proceso) {
        let mut e = NewEvent::observed(ts, SELF_DEVICE_ID, "127.0.0.1", q, "A");
        e.process = Some(p.clone());
        l.append(e).unwrap();
    }

    fn ledger() -> Ledger {
        Ledger::open_in_memory(Hash::of(b"contratos")).unwrap()
    }

    #[test]
    fn the_baseline_is_what_it_contacted_while_learning() {
        let p = zoom(1, "6.1.0", "aa".repeat(32).as_str());
        let mut c = Contrato::nuevo(&p, 0);
        assert_eq!(c.estado, EstadoContrato::Aprendiendo);
        assert!(c.observa(&obs(H, "zoom.us", "Zoom", &p)).is_empty());
        assert!(c.observa(&obs(2 * H, "api.zoom.us", "Zoom", &p)).is_empty());
        assert!(c
            .observa(&obs(3 * H, "google.com", "Google", &p))
            .is_empty());
        assert_eq!(
            c.empresas.keys().cloned().collect::<Vec<_>>(),
            vec!["Google".to_owned(), "Zoom".to_owned()]
        );
        assert_eq!(c.empresas["Zoom"].nombres.len(), 2);
        assert_eq!(c.empresas["Zoom"].veces, 2);
    }

    #[test]
    fn nothing_is_flagged_during_the_learning_window() {
        let p = zoom(1, "6.1.0", "");
        let mut c = Contrato::nuevo(&p, 0);
        assert!(c.observa(&obs(H, "zoom.us", "Zoom", &p)).is_empty());
        // Six days in: a company never seen is simply learnt.
        let e = c.observa(&obs(6 * DAY_MS, "amplitude.com", "Amplitude", &p));
        assert!(e.is_empty(), "{e:?}");
        assert!(c.empresas.contains_key("Amplitude"));
        // Day seven: the learning ends, and only then is something new a novelty.
        let e = c.observa(&obs(7 * DAY_MS, "zoom.us", "Zoom", &p));
        assert_eq!(e, vec![Efecto::Aprendido]);
        assert_eq!(c.estado, EstadoContrato::Vigilando);
        let e = c.observa(&obs(7 * DAY_MS + H, "segment.io", "Segment", &p));
        assert_eq!(
            e,
            vec![Efecto::Novedad {
                empresa: "Segment".into(),
                primera: true
            }]
        );
    }

    #[test]
    fn enough_launches_over_more_than_a_day_end_the_learning_sooner() {
        let p0 = zoom(100, "", "");
        let mut c = Contrato::nuevo(&p0, 0);
        for i in 0..APRENDE_LANZAMIENTOS {
            let p = zoom(100 + i, "", "");
            c.observa(&obs(H * i64::from(i), "zoom.us", "Zoom", &p));
        }
        // Ten launches within ten hours: still learning, a day has not passed.
        assert_eq!(c.estado, EstadoContrato::Aprendiendo);
        let p = zoom(999, "", "");
        let e = c.observa(&obs(DAY_MS + H, "zoom.us", "Zoom", &p));
        assert_eq!(e, vec![Efecto::Aprendido]);
    }

    #[test]
    fn a_new_company_is_flagged_once_and_a_known_one_never() {
        let p = zoom(1, "6.1.0", "");
        let mut c = Contrato::nuevo(&p, 0);
        c.observa(&obs(H, "zoom.us", "Zoom", &p));
        c.revisa_aprendizaje(8 * DAY_MS);
        assert!(c
            .observa(&obs(8 * DAY_MS, "zoom.us", "Zoom", &p))
            .is_empty());
        let e1 = c.observa(&obs(8 * DAY_MS + 1, "amplitude.com", "Amplitude", &p));
        let e2 = c.observa(&obs(8 * DAY_MS + 2, "api2.amplitude.com", "Amplitude", &p));
        assert_eq!(
            e1,
            vec![Efecto::Novedad {
                empresa: "Amplitude".into(),
                primera: true
            }]
        );
        assert_eq!(
            e2,
            vec![Efecto::Novedad {
                empresa: "Amplitude".into(),
                primera: false
            }]
        );
        // Cut: not flagged again.
        c.pendientes.clear();
        c.cortadas.insert("Amplitude".into());
        assert!(c
            .observa(&obs(9 * DAY_MS, "amplitude.com", "Amplitude", &p))
            .is_empty());
    }

    #[test]
    fn an_update_is_a_version_change_and_an_unknown_version_turning_known_is_not() {
        let a = "aa".repeat(32);
        let b = "bb".repeat(32);
        // The first queries go without a version: Windows is still reading the file.
        let sin = zoom(1, "", &a);
        let mut c = Contrato::nuevo(&sin, 0);
        assert_eq!(c.etiqueta(), "huella aaaaaaaa");
        let con = zoom(1, "6.1.0", &a);
        assert!(c.observa(&obs(H, "zoom.us", "Zoom", &con)).is_empty());
        assert_eq!(c.etiqueta(), "6.1.0");
        // The update: another file, another version.
        let nueva = zoom(2, "6.2.0", &b);
        let e = c.observa(&obs(2 * H, "zoom.us", "Zoom", &nueva));
        assert_eq!(
            e,
            vec![Efecto::Version {
                antes: "6.1.0".into(),
                ahora: "6.2.0".into()
            }]
        );
        assert_eq!(
            c.versiones.last().map(|v| v.etiqueta.as_str()),
            Some("6.2.0")
        );
        // Same file declaring no version: changed by its hash alone.
        let mut d = Contrato::nuevo(&zoom(1, "", &a), 0);
        let e = d.observa(&obs(H, "zoom.us", "Zoom", &zoom(1, "", &b)));
        assert_eq!(
            e,
            vec![Efecto::Version {
                antes: "huella aaaaaaaa".into(),
                ahora: "huella bbbbbbbb".into()
            }]
        );
    }

    #[test]
    fn an_update_in_a_folder_of_its_own_is_the_same_program() {
        let a = clave_de(
            r"C:\Users\ana\AppData\Local\Discord\app-1.0.9013\Discord.exe",
            "",
        );
        let b = clave_de(
            r"C:\Users\ana\AppData\Local\Discord\app-1.0.9014\Discord.exe",
            "",
        );
        assert_eq!(a, b);
        let t1 = clave_de(r"C:\Program Files\Foo\6.2.1\foo.exe", "foo.exe");
        let t2 = clave_de(r"C:\Program Files\Foo\6.3.0_x64\foo.exe", "foo.exe");
        assert_eq!(t1, t2);
        assert_ne!(
            clave_de(r"C:\Program Files\Foo\foo.exe", ""),
            clave_de(r"C:\Program Files\Bar\foo.exe", "")
        );
        assert_eq!(clave_de("", "Zoom.exe"), "zoom.exe");
    }

    #[test]
    fn browsers_and_shared_hosts_have_no_contract() {
        let mut p = zoom(1, "", "");
        p.nombre = "chrome.exe".into();
        let mut c = Contrato::nuevo(&p, 0);
        assert_eq!(c.estado, EstadoContrato::NoAplica);
        assert_eq!(c.motivo, "navegador");
        assert!(c
            .observa(&obs(10 * DAY_MS, "x.com", "x.com", &p))
            .is_empty());
        p.nombre = "svchost.exe".into();
        assert_eq!(Contrato::nuevo(&p, 0).motivo, "compartido");
        // Too many companies while learning: something like a browser.
        let q = zoom(1, "", "");
        let mut v = Contrato::nuevo(&q, 0);
        let nombres: Vec<String> = (0..=MAX_EMPRESAS_CONTRATO)
            .map(|i| format!("s{i}.com"))
            .collect();
        let mut ultimo = Vec::new();
        for n in &nombres {
            ultimo = v.observa(&obs(H, n, n, &q));
        }
        assert_eq!(ultimo, vec![Efecto::NoAplica]);
        assert_eq!(v.motivo, "variado");
        assert!(v.empresas.is_empty());
    }

    #[test]
    fn a_pass_learns_then_opens_one_notice_with_before_and_after() {
        let mut l = ledger();
        let v1 = zoom(1, "6.1.0", &"aa".repeat(32));
        evento(&mut l, 0, "zoom.us", &v1);
        evento(&mut l, H, "api.zoom.us", &v1);
        evento(&mut l, 2 * H, "1.0.0.127.in-addr.arpa", &v1);
        let p = l.contratos_pasada(&lee, 2 * H).unwrap();
        assert_eq!(p.leidas, 3);
        let c = l.contrato(&clave_de(&v1.ruta, "")).unwrap().unwrap();
        assert_eq!(c.estado, EstadoContrato::Aprendiendo);
        assert_eq!(c.empresas.len(), 1);
        // A week later Zoom 6.2 starts contacting two new companies.
        let v2 = zoom(2, "6.2.0", &"bb".repeat(32));
        evento(&mut l, 8 * DAY_MS, "zoom.us", &v2);
        evento(&mut l, 8 * DAY_MS + 1, "api2.amplitude.com", &v2);
        evento(&mut l, 8 * DAY_MS + 2, "stats.doubleclick.net", &v2);
        evento(&mut l, 8 * DAY_MS + 3, "api2.amplitude.com", &v2);
        let p = l.contratos_pasada(&lee, 8 * DAY_MS + 4).unwrap();
        assert_eq!(p.avisos_nuevos, 1);
        let avisos = l.avisos_contrato(true, 10).unwrap();
        assert_eq!(avisos.len(), 1);
        let a = &avisos[0];
        assert_eq!(a.programa, "Zoom.exe");
        assert_eq!(a.version_antes, "6.1.0");
        assert_eq!(a.version_ahora, "6.2.0");
        assert!(a.version_cambio());
        assert_eq!(a.antes, vec!["Zoom".to_owned()]);
        assert_eq!(
            a.nuevas.keys().cloned().collect::<Vec<_>>(),
            vec!["Amplitude".to_owned(), "Google".to_owned()]
        );
        assert_eq!(a.nuevas["Amplitude"].veces, 2);
        // Nothing read twice: a second pass with nothing new changes nothing.
        let p = l.contratos_pasada(&lee, 8 * DAY_MS + 5).unwrap();
        assert_eq!(p.leidas, 0);
        assert_eq!(l.avisos_contrato(true, 10).unwrap().len(), 1);
        // The register says it all, in order, and verifies.
        let tipos: Vec<String> = l
            .registro_contratos(None, 50)
            .unwrap()
            .into_iter()
            .rev()
            .map(|r| r.tipo)
            .collect();
        assert_eq!(
            tipos,
            vec!["aprendiendo", "aprendido", "version", "novedad", "novedad"]
        );
        assert!(l.comprueba_contratos().unwrap().ok);
    }

    fn con_aviso() -> (Ledger, Aviso) {
        let mut l = ledger();
        let v1 = zoom(1, "6.1.0", "");
        evento(&mut l, 0, "zoom.us", &v1);
        l.contratos_pasada(&lee, 0).unwrap();
        evento(&mut l, 8 * DAY_MS, "api2.amplitude.com", &v1);
        l.contratos_pasada(&lee, 8 * DAY_MS).unwrap();
        let a = l.avisos_contrato(true, 1).unwrap().remove(0);
        (l, a)
    }

    #[test]
    fn accepting_makes_the_new_companies_part_of_the_contract() {
        let (mut l, a) = con_aviso();
        let r = l.contrato_aceptar(a.id, 9 * DAY_MS).unwrap().unwrap();
        assert_eq!(r.estado, EstadoAviso::Aceptado);
        assert!(l.avisos_contrato(true, 10).unwrap().is_empty());
        let c = l.contrato(&a.clave).unwrap().unwrap();
        assert!(c.empresas.contains_key("Amplitude"));
        assert!(c.pendientes.is_empty());
        // Asked for again: nothing to say.
        evento(
            &mut l,
            9 * DAY_MS + 1,
            "amplitude.com",
            &zoom(1, "6.1.0", ""),
        );
        let p = l.contratos_pasada(&lee, 9 * DAY_MS + 2).unwrap();
        assert_eq!(p.avisos_nuevos, 0);
        // Accepting twice does nothing.
        assert!(l.contrato_aceptar(a.id, 9 * DAY_MS).unwrap().is_none());
    }

    #[test]
    fn cutting_only_the_new_makes_one_rule_per_site_for_this_computer() {
        let (mut l, a) = con_aviso();
        let r = l.contrato_cortar(a.id, 9 * DAY_MS).unwrap().unwrap();
        assert_eq!(r.estado, EstadoAviso::Cortado);
        assert_eq!(r.reglas.len(), 1);
        let reglas = l.active_rules(9 * DAY_MS).unwrap();
        assert_eq!(reglas.len(), 1);
        assert_eq!(reglas[0].pattern, "amplitude.com");
        assert_eq!(reglas[0].scope, Scope::Device);
        assert_eq!(reglas[0].device_id.as_deref(), Some(SELF_DEVICE_ID));
        assert_eq!(reglas[0].created_by, CREADA_POR_CONTRATO);
        // Zoom's own site was never touched.
        assert!(!reglas.iter().any(|r| r.pattern == "zoom.us"));
        // Undone: the rule goes and the company joins the contract.
        let d = l.contrato_deshacer(a.id, 10 * DAY_MS).unwrap().unwrap();
        assert_eq!(d.estado, EstadoAviso::Deshecho);
        assert!(l.active_rules(10 * DAY_MS).unwrap().is_empty());
        assert!(l
            .contrato(&a.clave)
            .unwrap()
            .unwrap()
            .empresas
            .contains_key("Amplitude"));
        assert!(l.comprueba_contratos().unwrap().ok);
    }

    #[test]
    fn a_fixed_contract_cuts_what_is_new_by_itself_and_records_it() {
        let mut l = ledger();
        let v1 = zoom(1, "6.1.0", "");
        let clave = clave_de(&v1.ruta, "");
        evento(&mut l, 0, "zoom.us", &v1);
        l.contratos_pasada(&lee, 0).unwrap();
        // Fixed while still learning: the learning ends with what was learnt.
        let c = l.contrato_fijar(&clave, true, H).unwrap().unwrap();
        assert_eq!(c.estado, EstadoContrato::Fijado);
        assert!(c.aprendido.is_some());
        evento(&mut l, 2 * H, "api2.amplitude.com", &v1);
        let p = l.contratos_pasada(&lee, 2 * H).unwrap();
        assert_eq!(p.cortes_auto, 1);
        let todos = l.avisos_contrato(false, 10).unwrap();
        assert_eq!(todos[0].estado, EstadoAviso::CortadoAuto);
        assert!(l.avisos_contrato(true, 10).unwrap().is_empty());
        assert_eq!(l.active_rules(2 * H).unwrap()[0].pattern, "amplitude.com");
        // Asked for again (now cut): no new notice, no new rule.
        evento(&mut l, 3 * H, "amplitude.com", &v1);
        let p = l.contratos_pasada(&lee, 3 * H).unwrap();
        assert_eq!((p.avisos_nuevos, p.cortes_auto), (0, 0));
        let tipos: Vec<String> = l
            .registro_contratos(Some(&clave), 50)
            .unwrap()
            .into_iter()
            .map(|r| r.tipo)
            .collect();
        assert_eq!(tipos[0], "cortado_auto");
        assert!(tipos.contains(&"fijado".to_owned()));
        // Let go: back to watching.
        let c = l.contrato_fijar(&clave, false, 4 * H).unwrap().unwrap();
        assert_eq!(c.estado, EstadoContrato::Vigilando);
    }

    #[test]
    fn what_the_computer_needs_or_another_program_uses_is_never_cut() {
        let mut l = ledger();
        let v1 = zoom(1, "6.1.0", "");
        let mut otro = zoom(5, "1.0", "");
        otro.nombre = "Teams.exe".into();
        otro.ruta = r"C:\Program Files\Teams\Teams.exe".into();
        evento(&mut l, 0, "zoom.us", &v1);
        evento(&mut l, 0, "amplitude.com", &otro);
        l.contratos_pasada(&lee, 0).unwrap();
        let mut cert = NewEvent::observed(
            8 * DAY_MS,
            SELF_DEVICE_ID,
            "127.0.0.1",
            "ocsp.digicert.com",
            "A",
        );
        cert.process = Some(v1.clone());
        cert.category = crate::model::Category::Esperado;
        l.append(cert).unwrap();
        evento(&mut l, 8 * DAY_MS, "api2.amplitude.com", &v1);
        l.contratos_pasada(&lee, 8 * DAY_MS).unwrap();
        let a = l
            .avisos_contrato(true, 10)
            .unwrap()
            .into_iter()
            .find(|a| a.programa == "Zoom.exe")
            .unwrap();
        let r = l.contrato_cortar(a.id, 9 * DAY_MS).unwrap().unwrap();
        assert!(r.reglas.is_empty(), "{r:?}");
        let motivos: Vec<(String, String)> = r
            .sin_cortar
            .iter()
            .map(|s| (s.motivo.clone(), s.con.clone()))
            .collect();
        assert!(motivos.contains(&("esperado".into(), String::new())));
        assert!(motivos.contains(&("compartido".into(), "Teams.exe".into())));
    }

    #[test]
    fn the_register_breaks_when_a_row_is_altered_or_the_last_one_removed() {
        let (l, _) = con_aviso();
        assert!(l.comprueba_contratos().unwrap().ok);
        l.conn
            .execute(
                "UPDATE contratos_registro SET detalle = '{}' WHERE id = 2",
                [],
            )
            .unwrap();
        let c = l.comprueba_contratos().unwrap();
        assert!(!c.ok);
        assert_eq!(c.fallo, Some(2));
        let (l, _) = con_aviso();
        l.conn
            .execute(
                "DELETE FROM contratos_registro WHERE id = (SELECT MAX(id) FROM contratos_registro)",
                [],
            )
            .unwrap();
        let c = l.comprueba_contratos().unwrap();
        assert!(!c.ok);
        assert_eq!(c.fallo, Some(0));
    }

    #[test]
    fn wiping_the_ledger_forgets_every_contract() {
        let (mut l, _) = con_aviso();
        l.wipe().unwrap();
        assert!(l.contratos().unwrap().is_empty());
        assert!(l.avisos_contrato(false, 10).unwrap().is_empty());
        assert!(l.registro_contratos(None, 10).unwrap().is_empty());
        assert!(l.comprueba_contratos().unwrap().ok);
    }
}
