//! El escudo en marcha (1.0.52, in preparation; docs/ESCUDO.md): the antivirus' behaviour rules. «Primera vez»
//! (`guardiana_escudo::primera_vez`), fed with this computer's DNS events once the observer has said which program
//! asked; and «ladrones de datos» (`guardiana_escudo::ladrones`), fed with the openings of saved secrets that the
//! watch in `guardiana_apps::Guardia` sees; and «cadenas» (`guardiana_escudo::cadenas`), fed with the program starts
//! that the same watch sees. Its memory and its warnings live next to the ledger, in two small files: `escudo-memoria.json` (since when
//! this computer asks for each site) and `escudo-avisos.jsonl` (the newest warnings, one per line, for the panel).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use guardiana_apps::{Apertura, Guardia, Observador, Rafaga};
use guardiana_core::{Ledger, NewEvent};
use guardiana_escudo::arranque::Archivo;
use guardiana_escudo::{Aviso, Firma, Huellas, Memoria, Mirada, Programa};

/// Warnings kept, the newest.
const MAX_AVISOS: usize = 500;
/// Queries waiting for Windows to check their program's signature, at most.
const MAX_PENDIENTES: usize = 1_000;
/// How long a query waits for that check.
const ESPERA_FIRMA_MS: u64 = 60_000;
/// How often the memory is written down when it changed.
const GUARDAR_CADA_MS: u64 = 5 * 60_000;
/// The same program touching the same kind of secret of the same app warns once a day.
const REPETIR_LADRON_MS: u64 = 24 * 60 * 60_000;
/// The same program starting the same program warns once an hour.
const REPETIR_CADENA_MS: u64 = 60 * 60_000;
/// The same program renaming or deleting many of the person's files warns once an hour.
const REPETIR_SECUESTRO_MS: u64 = 60 * 60_000;
/// How often the continuous watch's history is read for new autostarts.
const ARRANQUE_CADA_MS: u64 = 30_000;
/// On the first read, the autostarts the watch noted in the last day are looked at too.
const ARRANQUE_ATRAS_MS: i64 = 24 * 60 * 60_000;

/// A new autostart's program file waiting for Windows to check its signature.
struct PendienteArranque {
    titulo: String,
    tipo: String,
    clave: String,
    archivo: Archivo,
    cuando_ms: u64,
    visto_ms: u64,
}

/// A query whose program's signature was not checked yet.
struct Pendiente {
    nombre: String,
    sitio: String,
    cuando_ms: u64,
    programa: String,
    ruta: String,
    sha256: String,
}

/// The rule's state while the service runs.
pub(crate) struct EscudoVivo {
    memoria: Memoria,
    listas: Vec<Huellas>,
    pendientes: Vec<Pendiente>,
    ruta_memoria: PathBuf,
    ruta_avisos: PathBuf,
    guardada_ms: u64,
    cambios: bool,
    /// Program file → whether Windows marked it as come from the internet (read once per file).
    marcas: HashMap<String, bool>,
    /// The watch over saved secrets (Windows only; `None` when it could not start).
    guardia: Option<Guardia>,
    /// Openings of saved secrets whose program's signature was not checked yet.
    aperturas: Vec<Apertura>,
    /// (program, app, kind of secret) → when it last warned.
    ladrones_vistos: HashMap<(String, String, String), u64>,
    /// (parent's path, started program) → when it last warned.
    cadenas_vistas: HashMap<(String, String), u64>,
    /// The watch's history is read up to this row (its id) and this time (its `ts`).
    arranque_id: i64,
    arranque_ts: i64,
    /// When the history was last read.
    arranque_leido_ms: u64,
    /// New autostarts whose program's signature was not checked yet.
    arranques: Vec<PendienteArranque>,
    /// Bursts of the person's files renamed or deleted whose program's signature was not checked yet.
    rafagas: Vec<Rafaga>,
    /// Program path → when its last burst warned.
    secuestros_vistos: HashMap<String, u64>,
}

impl EscudoVivo {
    /// The memory kept next to the ledger at `db`, or a new one that starts learning now.
    pub(crate) fn abrir(db: &Path, ahora_ms: u64) -> Self {
        let ruta_memoria = db.with_file_name("escudo-memoria.json");
        let memoria = std::fs::read_to_string(&ruta_memoria)
            .ok()
            .and_then(|j| Memoria::desde_json(&j).ok())
            .unwrap_or_else(|| Memoria::nueva(ahora_ms));
        Self {
            memoria,
            listas: Vec::new(),
            pendientes: Vec::new(),
            ruta_memoria,
            ruta_avisos: db.with_file_name("escudo-avisos.jsonl"),
            guardada_ms: ahora_ms,
            cambios: false,
            marcas: HashMap::new(),
            guardia: match Guardia::arrancar() {
                Ok(g) => Some(g),
                Err(guardiana_apps::Error::NoSoportado) => None,
                Err(e) => {
                    eprintln!("guardiana: escudo: {e}");
                    None
                }
            },
            aperturas: Vec::new(),
            ladrones_vistos: HashMap::new(),
            cadenas_vistas: HashMap::new(),
            arranque_id: 0,
            arranque_ts: i64::try_from(ahora_ms).unwrap_or(0) - ARRANQUE_ATRAS_MS,
            arranque_leido_ms: 0,
            arranques: Vec::new(),
            rafagas: Vec::new(),
            secuestros_vistos: HashMap::new(),
        }
    }

    /// Looks at one of this computer's queries, with the program the observer gave it.
    pub(crate) fn mira(&mut self, e: &NewEvent, obs: &Observador) {
        let sitio = guardiana_lists::dominio::sitio(&e.qname);
        let cuando = u64::try_from(e.ts).unwrap_or(0);
        let antes = self.memoria.sitios();
        let mirada = match e.process.as_ref() {
            None => self
                .memoria
                .mira(&e.qname, &sitio, cuando, None, &self.listas),
            Some(p) => {
                let descargado = self.descargado(&p.ruta);
                let programa = Programa {
                    nombre: &p.nombre,
                    ruta: &p.ruta,
                    sha256: &p.sha256,
                    firma: firma_de(obs, &p.ruta, &p.firmado_por),
                    descargado,
                };
                let m = self
                    .memoria
                    .mira(&e.qname, &sitio, cuando, Some(&programa), &self.listas);
                if m == Mirada::Pendiente && self.pendientes.len() < MAX_PENDIENTES {
                    self.pendientes.push(Pendiente {
                        nombre: e.qname.clone(),
                        sitio: sitio.clone(),
                        cuando_ms: cuando,
                        programa: p.nombre.clone(),
                        ruta: p.ruta.clone(),
                        sha256: p.sha256.clone(),
                    });
                }
                m
            }
        };
        if self.memoria.sitios() != antes {
            self.cambios = true;
        }
        if let Mirada::Aviso(a) = mirada {
            self.anota(&a, a.regla == "muestra_conocida");
        }
    }

    /// Looks again at the queries that were waiting for a signature check, and at the openings of saved secrets.
    pub(crate) fn repasa(&mut self, obs: &Observador, ahora_ms: u64) {
        self.secuestros(obs, ahora_ms);
        self.ladrones(obs, ahora_ms);
        self.cadenas();
        self.arranques_pendientes(obs, ahora_ms);
        if self.pendientes.is_empty() {
            return;
        }
        for p in std::mem::take(&mut self.pendientes) {
            match obs.firma_comprobada(&p.ruta) {
                Some(f) => {
                    let descargado = self.descargado(&p.ruta);
                    let programa = Programa {
                        nombre: &p.programa,
                        ruta: &p.ruta,
                        sha256: &p.sha256,
                        firma: if f.is_empty() {
                            Firma::SinFirma
                        } else {
                            Firma::Firmado(f)
                        },
                        descargado,
                    };
                    if let Some(a) =
                        self.memoria
                            .revisa(&p.nombre, &p.sitio, p.cuando_ms, &programa)
                    {
                        self.anota(&a, a.regla == "muestra_conocida");
                    }
                }
                None if ahora_ms.saturating_sub(p.cuando_ms) < ESPERA_FIRMA_MS => {
                    self.pendientes.push(p)
                }
                None => {}
            }
        }
    }

    /// Whether Windows marked the program's file as come from the internet: its `Zone.Identifier` stream says zone 3
    /// (the internet) or 4 (untrusted sites). Read once per file; the address it came from is never read.
    fn descargado(&mut self, ruta: &str) -> bool {
        if let Some(m) = self.marcas.get(ruta) {
            return *m;
        }
        if self.marcas.len() >= 4_096 {
            self.marcas.clear();
        }
        let m = marca_de_internet(ruta);
        self.marcas.insert(ruta.to_owned(), m);
        m
    }

    /// The openings of saved secrets (browser passwords, cookies and keys, wallets, sessions) by programs that are not
    /// the app's own: each one, once Windows has checked the program's signature, through the rule
    /// (`guardiana_escudo::ladrones::juzga`). Another antivirus product reading files is its job, not a theft.
    fn ladrones(&mut self, obs: &Observador, ahora_ms: u64) {
        let nuevas = self.guardia.as_ref().map(Guardia::toma).unwrap_or_default();
        if nuevas.is_empty() && self.aperturas.is_empty() {
            return;
        }
        let mut todas = std::mem::take(&mut self.aperturas);
        todas.extend(nuevas);
        for a in todas {
            let cuando = u64::try_from(a.ts).unwrap_or(0);
            let firma = match obs.firma(&a.ruta) {
                Some(f) if f.is_empty() => Firma::SinFirma,
                Some(f) => Firma::Firmado(f),
                None if ahora_ms.saturating_sub(cuando) < ESPERA_FIRMA_MS => {
                    if self.aperturas.len() < MAX_PENDIENTES {
                        self.aperturas.push(a);
                    }
                    continue;
                }
                None => Firma::SinComprobar,
            };
            if matches!(firma, Firma::Firmado(_))
                && guardiana_lists::antivirus::antivirus_de_programa(&a.programa).is_some()
            {
                continue;
            }
            let Some((aviso, grave)) =
                guardiana_escudo::ladrones::juzga(&a.programa, &a.ruta, &firma, &a.archivo, cuando)
            else {
                continue;
            };
            let clave = (
                a.ruta.to_ascii_lowercase(),
                aviso.sitio.clone(),
                aviso.nombre.clone(),
            );
            if self
                .ladrones_vistos
                .get(&clave)
                .is_some_and(|t| cuando.saturating_sub(*t) < REPETIR_LADRON_MS)
            {
                continue;
            }
            if self.ladrones_vistos.len() > 4_096 {
                self.ladrones_vistos.clear();
            }
            self.ladrones_vistos.insert(clave, cuando);
            self.anota(&aviso, grave);
        }
    }

    /// The program starts the watch saw, each with the program that started it, through the rule
    /// (`guardiana_escudo::cadenas::juzga`), with whether Windows marked the parent's file as come from the internet.
    fn cadenas(&mut self) {
        let nuevos = self
            .guardia
            .as_ref()
            .map(Guardia::lanzamientos)
            .unwrap_or_default();
        for l in nuevos {
            let cuando = u64::try_from(l.ts).unwrap_or(0);
            let descargado = self.descargado(&l.padre_ruta);
            let Some((aviso, grave)) = guardiana_escudo::cadenas::juzga(
                &l.padre,
                &l.padre_ruta,
                descargado,
                &l.hijo,
                cuando,
            ) else {
                continue;
            };
            let clave = (l.padre_ruta.to_ascii_lowercase(), aviso.nombre.clone());
            if self
                .cadenas_vistas
                .get(&clave)
                .is_some_and(|t| cuando.saturating_sub(*t) < REPETIR_CADENA_MS)
            {
                continue;
            }
            if self.cadenas_vistas.len() > 4_096 {
                self.cadenas_vistas.clear();
            }
            self.cadenas_vistas.insert(clave, cuando);
            self.anota(&aviso, grave);
        }
    }

    /// The bursts of the person's files renamed or deleted by one program, once Windows has checked its signature,
    /// through the rule (`guardiana_escudo::secuestro::juzga`). The same program warns once an hour.
    fn secuestros(&mut self, obs: &Observador, ahora_ms: u64) {
        let nuevas = self
            .guardia
            .as_ref()
            .map(Guardia::rafagas)
            .unwrap_or_default();
        if nuevas.is_empty() && self.rafagas.is_empty() {
            return;
        }
        let mut todas = std::mem::take(&mut self.rafagas);
        todas.extend(nuevas);
        for r in todas {
            let cuando = u64::try_from(r.ts).unwrap_or(0);
            let firma = match obs.firma(&r.ruta) {
                Some(f) if f.is_empty() => Firma::SinFirma,
                Some(f) => Firma::Firmado(f),
                None if ahora_ms.saturating_sub(cuando) < ESPERA_FIRMA_MS => {
                    if self.rafagas.len() < MAX_PENDIENTES {
                        self.rafagas.push(r);
                    }
                    continue;
                }
                None => Firma::SinComprobar,
            };
            let descargado = self.descargado(&r.ruta);
            let Some((aviso, grave)) = guardiana_escudo::secuestro::juzga(
                &r.programa,
                &r.ruta,
                &firma,
                descargado,
                r.n,
                cuando,
            ) else {
                continue;
            };
            let clave = r.ruta.to_ascii_lowercase();
            if self
                .secuestros_vistos
                .get(&clave)
                .is_some_and(|t| cuando.saturating_sub(*t) < REPETIR_SECUESTRO_MS)
            {
                continue;
            }
            if self.secuestros_vistos.len() > 4_096 {
                self.secuestros_vistos.clear();
            }
            self.secuestros_vistos.insert(clave, cuando);
            self.anota(&aviso, grave);
        }
    }

    /// Whether it is time to read the continuous watch's history again.
    pub(crate) fn toca_arranques(&self, ahora_ms: u64) -> bool {
        ahora_ms.saturating_sub(self.arranque_leido_ms) >= ARRANQUE_CADA_MS
    }

    /// The new autostarts the continuous watch noted since the last read (not Windows' own): each program file in
    /// what they run waits for its signature check (`guardiana_escudo::arranque`).
    pub(crate) fn arranques(&mut self, l: &Ledger, obs: &Observador, ahora_ms: u64) {
        self.arranque_leido_ms = ahora_ms;
        let Ok(filas) = l.vigilancia(500, Some(self.arranque_ts)) else {
            return;
        };
        // the newest come first: oldest first here, so the row read last is the newest
        for s in filas.iter().rev() {
            if s.id <= self.arranque_id {
                continue;
            }
            self.arranque_id = s.id;
            self.arranque_ts = s.ts;
            if s.tipo != "arranque" || s.datos["frase"] != "vg_ev_arranque_nuevo" {
                continue;
            }
            let d = &s.datos["datos"];
            if d["sistema"] == "true" {
                continue;
            }
            let texto = |k: &str| d[k].as_str().unwrap_or_default().to_owned();
            for archivo in guardiana_escudo::arranque::archivos_en(&texto("ruta")) {
                if archivo.exacta {
                    // asks for the check now; the answer comes in a later tick
                    let _ = obs.firma(&archivo.ruta);
                }
                if self.arranques.len() < MAX_PENDIENTES {
                    self.arranques.push(PendienteArranque {
                        titulo: texto("x"),
                        tipo: texto("tipo"),
                        clave: s.clave.clone(),
                        archivo,
                        cuando_ms: u64::try_from(s.ts).unwrap_or(0),
                        visto_ms: ahora_ms,
                    });
                }
            }
        }
    }

    /// The new autostarts waiting for a signature check, through the rule once Windows has answered (or after
    /// [`ESPERA_FIRMA_MS`]). A file that is not there, or whose exact path is not known, goes without a signature
    /// check. The same autostart warns once, also across restarts (the panel's file is the record).
    fn arranques_pendientes(&mut self, obs: &Observador, ahora_ms: u64) {
        if self.arranques.is_empty() {
            return;
        }
        for p in std::mem::take(&mut self.arranques) {
            let existe = p.archivo.exacta && Path::new(&p.archivo.ruta).is_file();
            let firma = if existe {
                match obs.firma(&p.archivo.ruta) {
                    Some(f) if f.is_empty() => Firma::SinFirma,
                    Some(f) => Firma::Firmado(f),
                    None if ahora_ms.saturating_sub(p.visto_ms) < ESPERA_FIRMA_MS => {
                        self.arranques.push(p);
                        continue;
                    }
                    None => Firma::SinComprobar,
                }
            } else {
                Firma::SinComprobar
            };
            let descargado = existe && self.descargado(&p.archivo.ruta);
            let Some((aviso, grave)) = guardiana_escudo::arranque::juzga(
                &p.titulo,
                &p.tipo,
                &p.clave,
                &p.archivo,
                &firma,
                descargado,
                p.cuando_ms,
            ) else {
                continue;
            };
            if self.ya_avisado(aviso.regla, &aviso.nombre) {
                continue;
            }
            self.anota(&aviso, grave);
        }
    }

    /// Whether the panel's file already holds a warning of this rule about this thing.
    fn ya_avisado(&self, regla: &str, nombre: &str) -> bool {
        std::fs::read_to_string(&self.ruta_avisos).is_ok_and(|t| {
            t.lines()
                .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                .any(|v| v["regla"] == regla && v["nombre"] == nombre)
        })
    }

    /// Writes the memory down when it changed and enough time has passed.
    pub(crate) fn guarda_si_toca(&mut self, ahora_ms: u64) {
        if !self.cambios || ahora_ms.saturating_sub(self.guardada_ms) < GUARDAR_CADA_MS {
            return;
        }
        if let Ok(j) = self.memoria.a_json() {
            if escribe(&self.ruta_memoria, j.as_bytes()).is_ok() {
                self.cambios = false;
                self.guardada_ms = ahora_ms;
            }
        }
    }

    /// Adds a warning to the panel's file, keeping the newest [`MAX_AVISOS`]; a grave one carries `"grave": true` (a
    /// known sample, a document starting an interpreter, the backups' tools, a program in disguise).
    fn anota(&self, a: &Aviso, grave: bool) {
        let Ok(mut valor) = serde_json::to_value(a) else {
            return;
        };
        if let (true, Some(o)) = (grave, valor.as_object_mut()) {
            o.insert("grave".to_owned(), serde_json::Value::Bool(true));
        }
        let Ok(linea) = serde_json::to_string(&valor) else {
            return;
        };
        let mut lineas: Vec<String> = std::fs::read_to_string(&self.ruta_avisos)
            .map(|t| {
                t.lines()
                    .filter(|l| !l.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        lineas.push(linea);
        let sobra = lineas.len().saturating_sub(MAX_AVISOS);
        let mut texto = lineas.split_off(sobra).join("\n");
        texto.push('\n');
        if let Err(e) = escribe(&self.ruta_avisos, texto.as_bytes()) {
            eprintln!("guardiana: escudo: {e}");
        }
    }
}

#[cfg(windows)]
fn marca_de_internet(ruta: &str) -> bool {
    !ruta.is_empty()
        && std::fs::read_to_string(format!("{ruta}:Zone.Identifier"))
            .ok()
            .and_then(|t| guardiana_escudo::zona_de(&t))
            .is_some_and(|z| z >= 3)
}

#[cfg(not(windows))]
fn marca_de_internet(_ruta: &str) -> bool {
    false
}

/// What is known of a program's signature: Windows' check if it was done, else what the observer already said.
fn firma_de(obs: &Observador, ruta: &str, firmado_por: &str) -> Firma {
    match obs.firma_comprobada(ruta) {
        Some(f) if !f.is_empty() => Firma::Firmado(f),
        Some(_) => Firma::SinFirma,
        None if !firmado_por.is_empty() => Firma::Firmado(firmado_por.to_owned()),
        None => Firma::SinComprobar,
    }
}

/// Writes a whole file through a temporary name, so that it is either all there or not at all.
fn escribe(ruta: &Path, datos: &[u8]) -> std::io::Result<()> {
    let tmp = ruta.with_extension("tmp");
    std::fs::write(&tmp, datos)?;
    std::fs::rename(&tmp, ruta)
}
