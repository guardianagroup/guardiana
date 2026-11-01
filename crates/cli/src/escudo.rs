//! El escudo en marcha (1.0.52, in preparation; docs/ESCUDO.md): the antivirus' first behaviour rule, «primera vez»
//! (`guardiana_escudo::primera_vez`), fed with this computer's DNS events once the observer has said which program
//! asked. Its memory and its warnings live next to the ledger, in two small files: `escudo-memoria.json` (since when
//! this computer asks for each site) and `escudo-avisos.jsonl` (the newest warnings, one per line, for the panel).

use std::path::{Path, PathBuf};

use guardiana_apps::Observador;
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
                let programa = Programa {
                    nombre: &p.nombre,
                    ruta: &p.ruta,
                    sha256: &p.sha256,
                    firma: firma_de(obs, &p.ruta, &p.firmado_por),
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

    /// Looks again at the queries that were waiting for a signature check.
    pub(crate) fn repasa(&mut self, obs: &Observador, ahora_ms: u64) {
        if self.pendientes.is_empty() {
            return;
        }
        for p in std::mem::take(&mut self.pendientes) {
            match obs.firma_comprobada(&p.ruta) {
                Some(f) => {
                    let programa = Programa {
                        nombre: &p.programa,
                        ruta: &p.ruta,
                        sha256: &p.sha256,
                        firma: if f.is_empty() {
                            Firma::SinFirma
                        } else {
                            Firma::Firmado(f)
                        },
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
