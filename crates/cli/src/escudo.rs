//! El escudo en marcha (1.0.52, in preparation; docs/ESCUDO.md): the antivirus' behaviour rules. «Primera vez»
//! (`guardiana_escudo::primera_vez`), fed with this computer's DNS events once the observer has said which program
//! asked; and «ladrones de datos» (`guardiana_escudo::ladrones`), fed with the openings of saved secrets that the
//! watch in `guardiana_apps::Guardia` sees; and «cadenas» (`guardiana_escudo::cadenas`), fed with the program starts
//! that the same watch sees. Its memory and its warnings live next to the ledger, in two small files: `escudo-memoria.json` (since when
//! this computer asks for each site) and `escudo-avisos.jsonl` (the newest warnings, one per line, for the panel).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use guardiana_apps::{Apertura, Guardia, Observador};
use guardiana_core::NewEvent;
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
            self.anota(&a);
        }
    }

    /// Looks again at the queries that were waiting for a signature check, and at the openings of saved secrets.
    pub(crate) fn repasa(&mut self, obs: &Observador, ahora_ms: u64) {
        self.ladrones(obs, ahora_ms);
        self.cadenas();
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
                        self.anota(&a);
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
            let Some((aviso, _grave)) =
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
            self.anota(&aviso);
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
            let Some((aviso, _grave)) = guardiana_escudo::cadenas::juzga(
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
            self.anota(&aviso);
        }
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

    /// Adds a warning to the panel's file, keeping the newest [`MAX_AVISOS`].
    fn anota(&self, a: &Aviso) {
        let Ok(linea) = serde_json::to_string(a) else {
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
