//! «GUARDIANA vigila a tu antivirus» (the owner, 13 Oct 2026: «alguien de código abierto debe
//! vigilar al vigilante, y hay muchas multas a antivirus»).
//!
//! What it shows, for each antivirus whose processes asked for names on this computer in the
//! last seven days:
//!
//! - every name its own processes asked for, how many times, how many at night and how many were
//!   cut;
//! - for each name, what the maker itself publishes it is for (updates, cloud scanning, samples,
//!   licence, telemetry, offers…), with the maker's page; or, when the maker documents nothing
//!   about it, that it does not, and whose name it is when the lists know (never more than that:
//!   an undocumented name is a question, not a verdict);
//! - who signed the process, as Windows said: a process named like an antivirus and signed by
//!   somebody else is not that antivirus.
//!
//! And for every product the list knows, installed or not, the official decisions of authorities
//! about it (fines, bans, warnings), each with the authority's own page.
//!
//! What it cannot see is said on the page: what the antivirus sends (Guardiana sees names, never
//! content), and anything it sends without asking the system for a name (its own resolver, or a
//! fixed address).

// The handler answers with the same error type as the rest of the API (see api.rs).
#![allow(clippy::result_large_err)]

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, LazyLock, Mutex, PoisonError};

use axum::extract::State;
use axum::http::header;
use axum::response::IntoResponse;
use guardiana_core::time::{now_ms, HOUR_MS};
use guardiana_core::{Category, SELF_DEVICE_ID};
use guardiana_lists::antivirus::{self, Antivirus};
use serde::Serialize;

use crate::api::{with_lectura, with_ledger, ApiResult};
use crate::auth::{Session, Zona};
use crate::AppState;

/// Settings key: the antivirus products whose outgoing traffic the person has chosen to cut.
const CLAVE_CORTES: &str = "antivirus_cortes";

/// Manages the clean cut of what an antivirus sends out (1.0.22). It holds the one long-lived
/// firewall session (dynamic: the cuts vanish if GUARDIANA stops) and, per product and path, the
/// ids of the filters in place. The wanted set is kept in the ledger, so a restart re-applies it.
/// Once in place, a cut stays until the person lets the product through (review of
/// arreglos-1055): putting it back only adds the paths not cut yet.
///
/// The process's, not each panel's, as the VPN is (review of vpn-1055b): the engine rebuilds the
/// panel when the network changes, and a slow look could keep the old panel's state alive a while
/// after, and with it, until then, its own firewall session. GUARDIANA's provider and sublayer had
/// fixed keys then, and Windows does not let a dynamic session hang anything from another's
/// (`FWP_E_WRONG_SESSION`: seen for the VPN's sublayer in 1.0.54, and for these cuts by this
/// module's Windows test): the new panel's cuts were refused without a word, and when the old state
/// went, its session took every cut with it. Since 1.0.55 every session has keys of its own, and a
/// session Windows lost is opened again with every cut put back (review of arreglos-1055).
#[derive(Default)]
pub(crate) struct CortesAntivirus {
    /// The firewall, what is cut, and what waits for a file to be cut. Its lock makes one change of
    /// the cuts at a time, an old panel's and the new one's alike.
    estado: Mutex<Estado>,
}

/// The firewall, the cuts in place and the paths waiting for a file, behind [`CortesAntivirus`]'s
/// one lock.
#[derive(Default)]
struct Estado {
    /// The session: opened by the first panel start that can (on Windows, running as the system),
    /// and opened again, with every cut put back in it, when Windows no longer has it or one of its
    /// cuts (its filtering service restarted).
    cortafuegos: Option<guardiana_apps::Cortafuegos>,
    /// Product name → each of its executables' paths → the ids of the filters blocking it.
    puestos: HashMap<String, BTreeMap<String, Vec<u64>>>,
    /// Product name → its paths skipped because no file was there, looked at again every
    /// [`CADA_SALTADAS`] until one appears.
    saltadas: HashMap<String, BTreeSet<String>>,
    /// Whether the thread that looks at them is running: it ends when nothing waits any more.
    vigilando: bool,
}

/// The cuts of the process: the same for every pass of the engine and every panel it starts.
pub(crate) fn global() -> &'static CortesAntivirus {
    static CORTES: LazyLock<CortesAntivirus> = LazyLock::new(CortesAntivirus::default);
    &CORTES
}

/// How often the paths skipped because no file was there are looked at again.
const CADA_SALTADAS: std::time::Duration = std::time::Duration::from_secs(15);

/// Whether no file is at `ruta`: only «not found» counts; a path that cannot be looked at is tried,
/// and Windows says what it says.
fn no_esta(ruta: &str) -> bool {
    matches!(std::path::Path::new(ruta).try_exists(), Ok(false))
}

/// Keeps `esperan` as the paths of `producto` waiting for a file, and makes sure something looks at
/// them: a thread of its own, started only when some path waits, that looks every
/// [`CADA_SALTADAS`] and cuts a file that appeared, and ends when nothing waits any more. Called
/// with the cuts' lock held, as is the thread's look, so one of them always sees the other.
fn esperan_un_archivo(
    saltadas: &mut HashMap<String, BTreeSet<String>>,
    vigilando: &mut bool,
    producto: &str,
    esperan: BTreeSet<String>,
) {
    if esperan.is_empty() {
        saltadas.remove(producto);
        return;
    }
    saltadas.insert(producto.to_owned(), esperan);
    if !*vigilando {
        *vigilando = true;
        std::thread::spawn(|| loop {
            std::thread::sleep(CADA_SALTADAS);
            if !global().corta_lo_que_aparecio() {
                return;
            }
        });
    }
}

impl CortesAntivirus {
    /// At every panel start: opens the firewall if it is not open yet (when on Windows and running
    /// as the system), or again, with every cut put back, if Windows lost it; and cuts, for whatever
    /// the person had chosen to cut, the paths seen this week that are not cut yet; what is cut
    /// stays. Never fails the panel: if the firewall cannot open, cutting is simply unavailable and
    /// every attempt says so. A cut that cannot be put is written down (until 1.0.55 nothing said
    /// it).
    pub(crate) fn al_arrancar(&self, ledger: &guardiana_core::Ledger) {
        {
            let mut estado = self.estado.lock().unwrap_or_else(PoisonError::into_inner);
            estado.rehace_si_perdida();
            if estado.cortafuegos.is_none() {
                return;
            }
        }
        for prod in self.querados(ledger) {
            let rutas = Self::rutas_de(ledger, &prod);
            if let Err(e) = self.aplica_rutas(&prod, true, &rutas) {
                eprintln!("guardiana: el corte de {prod} no se pudo volver a poner: {e}");
            }
        }
    }

    /// The products the person asked to cut, from the ledger.
    fn querados(&self, ledger: &guardiana_core::Ledger) -> Vec<String> {
        ledger
            .setting(CLAVE_CORTES)
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
            .unwrap_or_default()
    }

    /// The full paths of a product's running executables, from what this computer has seen in the
    /// last week. Matching is by the product's documented process file names.
    fn rutas_de(ledger: &guardiana_core::Ledger, producto: &str) -> Vec<String> {
        let nombres: Vec<String> = antivirus::antivirus()
            .iter()
            .find(|a| a.nombre == producto)
            .map(|a| a.programas.iter().map(|p| p.to_ascii_lowercase()).collect())
            .unwrap_or_default();
        let desde = now_ms() - 7 * 24 * HOUR_MS;
        let mut rutas: Vec<String> = ledger
            .program_paths(desde)
            .unwrap_or_default()
            .into_iter()
            .filter(|(nombre, _)| nombres.contains(&nombre.to_ascii_lowercase()))
            .map(|(_, ruta)| ruta)
            .collect();
        rutas.sort();
        rutas.dedup();
        rutas
    }

    /// Every known product with the executable paths this computer has seen of it in the last
    /// week; products with none are left out (there is nothing to cut).
    pub(crate) fn rutas_por_producto(
        ledger: &guardiana_core::Ledger,
    ) -> Vec<(&'static str, Vec<String>)> {
        let desde = now_ms() - 7 * 24 * HOUR_MS;
        let vistos = ledger.program_paths(desde).unwrap_or_default();
        antivirus::antivirus()
            .iter()
            .filter_map(|a| {
                let mut rutas: Vec<String> = vistos
                    .iter()
                    .filter(|(n, _)| a.programas.iter().any(|p| p.eq_ignore_ascii_case(n)))
                    .map(|(_, r)| r.clone())
                    .collect();
                rutas.sort();
                rutas.dedup();
                (!rutas.is_empty()).then_some((a.nombre, rutas))
            })
            .collect()
    }

    /// Put the cut on (`cortar`) or take it off. On, over these executable paths: the ones not cut
    /// yet are added, and every cut already in place stays (review of arreglos-1055: putting a cut
    /// back with only this week's paths took off those of a program that asked no name for a week,
    /// at every rebuild of the panel). Off, every filter of the product goes: only the person's
    /// «let through» takes a cut off. Returns the number of filters now in place for the product.
    /// A session Windows no longer has is opened again first, with every cut put back in it; and
    /// once more if this change fails because it went meanwhile (review of arreglos-1055).
    fn aplica_rutas(
        &self,
        producto: &str,
        cortar: bool,
        rutas: &[String],
    ) -> Result<usize, guardiana_apps::ErrorCorte> {
        let mut estado = self.estado.lock().unwrap_or_else(PoisonError::into_inner);
        estado.rehace_si_perdida();
        let r = estado.aplica(producto, cortar, rutas);
        if r.is_err() && estado.rehace_si_perdida() {
            return estado.aplica(producto, cortar, rutas);
        }
        r
    }

    /// The look at the paths waiting for a file (see [`esperan_un_archivo`]), in a session Windows
    /// still has. Whether anything still waits.
    fn corta_lo_que_aparecio(&self) -> bool {
        let mut estado = self.estado.lock().unwrap_or_else(PoisonError::into_inner);
        estado.rehace_si_perdida();
        estado.corta_lo_que_aparecio()
    }

    /// Which products are cut right now (have filters in place).
    pub(crate) fn cortados(&self) -> Vec<String> {
        self.estado
            .lock()
            .map(|e| {
                e.puestos
                    .iter()
                    .filter(|(_, r)| r.values().any(|ids| !ids.is_empty()))
                    .map(|(k, _)| k.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Whether cutting is available at all on this machine.
    pub(crate) fn disponible(&self) -> bool {
        self.estado
            .lock()
            .map(|e| e.cortafuegos.is_some())
            .unwrap_or(false)
    }
}

impl Estado {
    /// Whether Windows still has the session, and every cut in place in it.
    fn sana(&self) -> bool {
        let Some(cf) = &self.cortafuegos else {
            return false;
        };
        let ids: Vec<u64> = self
            .puestos
            .values()
            .flat_map(|r| r.values().flatten().copied())
            .collect();
        cf.sigue_abierto() && cf.cuantos_de(&ids).is_ok_and(|n| n == ids.len())
    }

    /// When there is no session yet, or Windows no longer has it or one of its cuts (its filtering
    /// service restarted): a new session, with every cut in place put back in it before the old one
    /// goes; a path whose file is gone waits for it. Whether a new session was opened.
    fn rehace_si_perdida(&mut self) -> bool {
        if self.sana() {
            return false;
        }
        let Ok(nuevo) = guardiana_apps::Cortafuegos::abrir() else {
            return false;
        };
        if self.cortafuegos.is_some() {
            eprintln!(
                "guardiana: Windows ya no tenía la sesión de los cortes del antivirus: se abre otra y se vuelven a poner"
            );
        }
        let mut puestos: HashMap<String, BTreeMap<String, Vec<u64>>> = HashMap::new();
        let mut sin_archivo: Vec<(String, String)> = Vec::new();
        for (producto, rutas) in &self.puestos {
            let cortadas = puestos.entry(producto.clone()).or_default();
            for ruta in rutas.keys() {
                if no_esta(ruta) {
                    sin_archivo.push((producto.clone(), ruta.clone()));
                    continue;
                }
                match nuevo.cortar(ruta, &guardiana_apps::Destino::default()) {
                    Ok(ids) => {
                        cortadas.insert(ruta.clone(), ids);
                    }
                    Err(e) => eprintln!(
                        "guardiana: el corte de {producto} no se pudo volver a poner en {ruta}: {e}"
                    ),
                }
            }
        }
        self.puestos = puestos;
        // The old session goes here, once the new one holds every cut.
        self.cortafuegos = Some(nuevo);
        for (producto, ruta) in sin_archivo {
            let mut esperan = self.saltadas.remove(&producto).unwrap_or_default();
            esperan.insert(ruta);
            esperan_un_archivo(&mut self.saltadas, &mut self.vigilando, &producto, esperan);
        }
        true
    }

    /// [`CortesAntivirus::aplica_rutas`], in the session there is.
    fn aplica(
        &mut self,
        producto: &str,
        cortar: bool,
        rutas: &[String],
    ) -> Result<usize, guardiana_apps::ErrorCorte> {
        let Estado {
            cortafuegos,
            puestos,
            saltadas,
            vigilando,
        } = self;
        let Some(cf) = cortafuegos.as_ref() else {
            return Err(guardiana_apps::ErrorCorte::NoSoportado);
        };
        if !cortar {
            let ids: Vec<u64> = puestos
                .get(producto)
                .map(|r| r.values().flatten().copied().collect())
                .unwrap_or_default();
            cf.quitar(&ids)?;
            puestos.remove(producto);
            saltadas.remove(producto);
            return Ok(0);
        }
        let cortadas = puestos.entry(producto.to_owned()).or_default();
        let mut esperan = saltadas.get(producto).cloned().unwrap_or_default();
        let mut saltadas_ahora = 0;
        // The paths not cut yet. One with no file at it (an update moved the program, or it was
        // uninstalled) has nothing to cut, and Windows refuses it: it is skipped, said once, and waits
        // for a file (review of arreglos-1055: until then it failed the whole set). If one that is
        // there cannot be cut, the ones added now are taken off again and what was there before
        // stays as it was.
        let mut nuevas: Vec<(String, Vec<u64>)> = Vec::new();
        for ruta in rutas {
            if cortadas.contains_key(ruta) || nuevas.iter().any(|(r, _)| r == ruta) {
                continue;
            }
            if no_esta(ruta) {
                if esperan.insert(ruta.clone()) {
                    eprintln!(
                        "guardiana: el corte de {producto} salta {ruta}: ya no está en el disco"
                    );
                    saltadas_ahora += 1;
                }
                continue;
            }
            match cf.cortar(ruta, &guardiana_apps::Destino::default()) {
                Ok(ids) => nuevas.push((ruta.clone(), ids)),
                Err(e) => {
                    let hechos: Vec<u64> = nuevas.into_iter().flat_map(|(_, ids)| ids).collect();
                    let _ = cf.quitar(&hechos);
                    esperan_un_archivo(saltadas, vigilando, producto, esperan);
                    return Err(e);
                }
            }
        }
        if saltadas_ahora > 0 && rutas.iter().all(|r| no_esta(r)) {
            eprintln!("guardiana: nada que cortar de {producto}: ninguno de sus archivos está en el disco");
        }
        for (ruta, _) in &nuevas {
            esperan.remove(ruta);
        }
        cortadas.extend(nuevas);
        let n: usize = cortadas.values().map(Vec::len).sum();
        esperan_un_archivo(saltadas, vigilando, producto, esperan);
        Ok(n)
    }

    /// Cuts what appeared at the paths skipped because no file was there; what is still missing
    /// keeps waiting. A path Windows will not cut is said once and left to the next time the cut is
    /// put (a panel start, or the person). Whether anything still waits: when nothing does, the
    /// thread that looks ends, marked under the same lock.
    fn corta_lo_que_aparecio(&mut self) -> bool {
        let Estado {
            cortafuegos,
            puestos,
            saltadas,
            vigilando,
        } = self;
        let Some(cf) = cortafuegos.as_ref() else {
            *vigilando = false;
            return false;
        };
        for (producto, rutas) in saltadas.iter_mut() {
            let cortadas = puestos.entry(producto.clone()).or_default();
            rutas.retain(|ruta| {
                if no_esta(ruta) {
                    return true;
                }
                if let std::collections::btree_map::Entry::Vacant(sitio) =
                    cortadas.entry(ruta.clone())
                {
                    match cf.cortar(ruta, &guardiana_apps::Destino::default()) {
                        Ok(ids) => {
                            sitio.insert(ids);
                        }
                        Err(e) => {
                            eprintln!(
                                "guardiana: el corte de {producto} no pudo poner {ruta}: {e}"
                            );
                        }
                    }
                }
                false
            });
        }
        saltadas.retain(|_, rutas| !rutas.is_empty());
        *vigilando = !saltadas.is_empty();
        *vigilando
    }
}

/// How many names of one product are listed; the totals count all of them.
const MAX_DESTINOS: usize = 300;

#[derive(Serialize)]
pub(crate) struct Vista {
    /// One entry per (device, product) that asked for names in the window.
    vistos: Vec<Visto>,
    /// Devices that do not share their detail with the home panel: left out, and counted.
    ocultos: usize,
    /// Every product the list knows, with its official record.
    catalogo: Vec<Ficha>,
    /// What each antivirus on this computer sent and received, by country and destination
    /// (1.0.21). Only this computer: the traffic channel sees no other device.
    trafico: Vec<TraficoProducto>,
    /// How many hours of traffic there are (the plan's detail): the page says it.
    trafico_horas: i64,
    /// The latest minutes of antivirus traffic, newest first, for the live panel (1.0.22).
    vivo: Vec<VivoFila>,
    /// Whether cutting an antivirus's outgoing traffic is available here (Windows, as the system).
    cortar_disponible: bool,
    /// The products whose outgoing traffic is cut right now.
    cortados: Vec<String>,
    /// GUARDIANA is counting what the antivirus sends and receives (the person can turn it off).
    vigilando: bool,
    /// «Cortar todo lo que envía el antivirus» is on (until 13 Oct 2026 «Protección máxima del
    /// antivirus»): every product this computer has seen is cut.
    maxima: bool,
    /// The last 24 hours of all the antivirus on this computer, together.
    resumen: Resumen,
    /// The window, in days.
    dias: i64,
}

/// The last 24 hours of the antivirus on this computer, always on top of the page (1.0.22).
#[derive(Serialize, Default)]
struct Resumen {
    /// Bytes they sent out and received.
    subidos: u64,
    bajados: u64,
    /// Of the bytes sent, how many between midnight and six.
    subidos_noche: u64,
    /// How many different countries they talked to (real ones; marks are not countries).
    paises: usize,
    /// How many different destinations (a name, or a network when no name was seen).
    destinos: usize,
    /// The products that talked in these 24 hours, most bytes first.
    productos: Vec<&'static str>,
}

/// One line of the live panel: a minute of an antivirus talking to a destination (1.0.22).
#[derive(Serialize)]
struct VivoFila {
    /// Start of the minute, Unix ms.
    minuto: i64,
    producto: &'static str,
    programa: String,
    /// The name asked for the address, or empty.
    destino: String,
    /// The network, or a mark (`@local`, `@anycast`).
    red: String,
    pais: String,
    subidos: u64,
    bajados: u64,
}

/// How many minutes of the live feed are returned.
const VIVO_MINUTOS: i64 = 20;
/// The most rows of the live feed.
const VIVO_MAX: usize = 120;

/// What one antivirus product sent and received in the window, measured on this computer.
#[derive(Serialize)]
struct TraficoProducto {
    producto: &'static str,
    /// Bytes its processes sent out, in all.
    subidos: u64,
    /// Bytes they received.
    bajados: u64,
    /// By country of the server, most bytes first.
    paises: Vec<PaisTrafico>,
    /// By destination (the name it asked, or its network when it asked none), most bytes first.
    destinos: Vec<DestinoTrafico>,
    /// Of the bytes sent, how many between midnight and six.
    subidos_noche: u64,
}

#[derive(Serialize)]
struct PaisTrafico {
    /// Two-letter country, or a mark (`@local`, `@anycast`) the page translates.
    pais: String,
    subidos: u64,
    bajados: u64,
}

#[derive(Serialize)]
struct DestinoTrafico {
    /// The name the program asked for the address, or empty when it asked none GUARDIANA saw.
    destino: String,
    /// The network that announces the address (`Amazon.com, Inc.`), or a mark.
    red: String,
    pais: String,
    subidos: u64,
    bajados: u64,
}

/// How many of a product's rows are listed per country and per destination.
const MAX_TRAFICO: usize = 50;

#[derive(Serialize)]
struct Visto {
    producto: &'static str,
    empresa: &'static str,
    pais: &'static str,
    device_id: String,
    device_name: Option<String>,
    /// Each process of the product that asked, with who signed it and how many times it asked.
    programas: Vec<Programa>,
    consultas: u64,
    nocturnas: u64,
    cortadas: u64,
    ultima: i64,
    /// Distinct names the maker documents / does not document.
    documentados: usize,
    no_documentados: usize,
    destinos: Vec<Destino>,
    /// How many names there were in all (the list stops at [`MAX_DESTINOS`]).
    destinos_total: usize,
}

#[derive(Serialize)]
struct Programa {
    nombre: String,
    firma: String,
    consultas: u64,
}

#[derive(Serialize)]
struct Destino {
    qname: String,
    /// What the maker says it is for, when it documents it.
    uso: Option<&'static str>,
    /// The maker's page that says so.
    fuente: Option<&'static str>,
    /// Whose name it is, when the lists know and it is not the maker's own documented name.
    empresa: Option<&'static str>,
    categoria: Category,
    consultas: u64,
    nocturnas: u64,
    cortadas: u64,
    ultima: i64,
}

#[derive(Serialize)]
struct Ficha {
    producto: &'static str,
    empresa: &'static str,
    pais: &'static str,
    programas: Vec<&'static str>,
    /// How many names the maker documents.
    dominios: usize,
    sanciones: Vec<Sancion>,
    /// What its own documents say, in plain words and with their sources (13 Oct 2026).
    terminos: Option<&'static antivirus::TerminosAv>,
}

#[derive(Serialize)]
struct Sancion {
    fecha: &'static str,
    autoridad: &'static str,
    pais: &'static str,
    importe: &'static str,
    tipo: &'static str,
    clave: &'static str,
    fuente: &'static str,
}

fn ficha(a: &Antivirus) -> Ficha {
    Ficha {
        producto: a.nombre,
        empresa: a.empresa,
        pais: a.pais,
        programas: a.programas.clone(),
        dominios: a.dominios.len(),
        terminos: antivirus::terminos_de(a.nombre),
        sanciones: a
            .sanciones
            .iter()
            .map(|s| Sancion {
                fecha: s.fecha,
                autoridad: s.autoridad,
                pais: s.pais,
                importe: s.importe,
                tipo: s.tipo,
                clave: s.clave,
                fuente: s.fuente,
            })
            .collect(),
    }
}

/// How much a name says about the person, for the order of the list: what nobody documents and
/// a list calls tracking or advertising first.
fn peso(d: &Destino) -> u8 {
    match (d.uso.is_some(), d.categoria) {
        (false, Category::Rastreador | Category::Publicidad) => 0,
        (false, Category::Telemetria) => 1,
        (false, _) => 2,
        (true, _) if d.uso == Some("publicidad") || d.uso == Some("telemetria") => 3,
        _ => 4,
    }
}

/// Work the page out from the ledger's rows.
fn calcula(
    filas: &[guardiana_core::ProgramName],
    nombres: &HashMap<String, Option<String>>,
    comparte: &dyn Fn(&str) -> bool,
) -> Vec<Visto> {
    type Clave = (String, &'static str);
    let mut por: HashMap<Clave, (Vec<&guardiana_core::ProgramName>, &'static Antivirus)> =
        HashMap::new();
    for f in filas {
        let Some(av) = antivirus::antivirus_de_programa(&f.program) else {
            continue;
        };
        if !comparte(&f.device_id) {
            continue;
        }
        por.entry((f.device_id.clone(), av.nombre))
            .or_insert_with(|| (Vec::new(), av))
            .0
            .push(f);
    }
    let mut vistos: Vec<Visto> = por
        .into_iter()
        .map(|((device_id, _), (filas, av))| {
            let mut programas: HashMap<(String, String), u64> = HashMap::new();
            let mut destinos: HashMap<&str, Destino> = HashMap::new();
            for f in &filas {
                *programas
                    .entry((f.program.clone(), f.signer.clone()))
                    .or_insert(0) += f.queries;
                let d = destinos.entry(f.qname.as_str()).or_insert_with(|| {
                    let doc = antivirus::uso_documentado(av, &f.qname);
                    Destino {
                        qname: f.qname.clone(),
                        uso: doc.map(|d| d.uso),
                        fuente: doc.map(|d| d.fuente),
                        empresa: if doc.is_some() {
                            None
                        } else {
                            // The company people know (Google), not its group (Alphabet).
                            guardiana_lists::company_of(&f.qname)
                                .or_else(|| guardiana_lists::group_of(&f.qname))
                        },
                        categoria: f.category,
                        consultas: 0,
                        nocturnas: 0,
                        cortadas: 0,
                        ultima: 0,
                    }
                });
                d.consultas += f.queries;
                d.nocturnas += f.night;
                d.cortadas += f.cut;
                d.ultima = d.ultima.max(f.last_ts);
            }
            let mut destinos: Vec<Destino> = destinos.into_values().collect();
            destinos.sort_by(|a, b| {
                peso(a)
                    .cmp(&peso(b))
                    .then(b.consultas.cmp(&a.consultas))
                    .then(a.qname.cmp(&b.qname))
            });
            let mut programas: Vec<Programa> = programas
                .into_iter()
                .map(|((nombre, firma), consultas)| Programa {
                    nombre,
                    firma,
                    consultas,
                })
                .collect();
            programas.sort_by(|a, b| b.consultas.cmp(&a.consultas).then(a.nombre.cmp(&b.nombre)));
            let destinos_total = destinos.len();
            let documentados = destinos.iter().filter(|d| d.uso.is_some()).count();
            let visto = Visto {
                producto: av.nombre,
                empresa: av.empresa,
                pais: av.pais,
                device_name: nombres.get(&device_id).cloned().flatten(),
                device_id,
                programas,
                consultas: destinos.iter().map(|d| d.consultas).sum(),
                nocturnas: destinos.iter().map(|d| d.nocturnas).sum(),
                cortadas: destinos.iter().map(|d| d.cortadas).sum(),
                ultima: destinos.iter().map(|d| d.ultima).max().unwrap_or(0),
                documentados,
                no_documentados: destinos_total - documentados,
                destinos_total,
                destinos: {
                    destinos.truncate(MAX_DESTINOS);
                    destinos
                },
            };
            visto
        })
        .collect();
    vistos.sort_by(|a, b| {
        b.consultas
            .cmp(&a.consultas)
            .then(a.producto.cmp(b.producto))
    });
    vistos
}

/// The night, in a program's traffic minute, by the person's own clock.
pub(crate) fn es_noche(minuto: i64, zona: i64) -> bool {
    let local = (minuto + zona * 60_000).rem_euclid(24 * 3_600_000);
    local < 6 * 3_600_000
}

/// Group the traffic measurements of this computer by antivirus product (1.0.21).
fn calcula_trafico(filas: &[guardiana_core::Trafico], zona: i64) -> Vec<TraficoProducto> {
    struct Acc {
        subidos: u64,
        bajados: u64,
        noche: u64,
        por_pais: HashMap<String, (u64, u64)>,
        por_destino: HashMap<(String, String, String), (u64, u64)>,
    }
    let mut por: HashMap<&'static str, Acc> = HashMap::new();
    for f in filas {
        let Some(av) = antivirus::antivirus_de_programa(&f.programa) else {
            continue;
        };
        let a = por.entry(av.nombre).or_insert_with(|| Acc {
            subidos: 0,
            bajados: 0,
            noche: 0,
            por_pais: HashMap::new(),
            por_destino: HashMap::new(),
        });
        a.subidos += f.enviados;
        a.bajados += f.recibidos;
        if es_noche(f.minuto, zona) {
            a.noche += f.enviados;
        }
        let pais = if f.pais.is_empty() {
            f.red.clone() // may be @local / @anycast; else empty
        } else {
            f.pais.clone()
        };
        let p = a.por_pais.entry(pais.clone()).or_insert((0, 0));
        p.0 += f.enviados;
        p.1 += f.recibidos;
        let d = a
            .por_destino
            .entry((f.destino.clone(), f.red.clone(), pais))
            .or_insert((0, 0));
        d.0 += f.enviados;
        d.1 += f.recibidos;
    }
    fn corta<K>(mut v: Vec<(K, (u64, u64))>) -> Vec<(K, (u64, u64))> {
        v.sort_by_key(|x| std::cmp::Reverse(x.1 .0 + x.1 .1));
        v.truncate(MAX_TRAFICO);
        v
    }
    let mut out: Vec<TraficoProducto> = por
        .into_iter()
        .map(|(producto, a)| TraficoProducto {
            producto,
            subidos: a.subidos,
            bajados: a.bajados,
            subidos_noche: a.noche,
            paises: corta(a.por_pais.into_iter().collect())
                .into_iter()
                .map(|(pais, (s, b))| PaisTrafico {
                    pais,
                    subidos: s,
                    bajados: b,
                })
                .collect(),
            destinos: corta(a.por_destino.into_iter().collect())
                .into_iter()
                .map(|((destino, red, pais), (s, b))| DestinoTrafico {
                    destino,
                    red,
                    pais,
                    subidos: s,
                    bajados: b,
                })
                .collect(),
        })
        .collect();
    out.sort_by_key(|p| std::cmp::Reverse(p.subidos + p.bajados));
    out
}

/// The totals of all the products together: bytes, countries, destinations.
fn resume(por_producto: &[TraficoProducto]) -> Resumen {
    let mut paises = std::collections::HashSet::new();
    let mut destinos = std::collections::HashSet::new();
    let mut r = Resumen::default();
    for p in por_producto {
        r.subidos += p.subidos;
        r.bajados += p.bajados;
        r.subidos_noche += p.subidos_noche;
        for x in &p.paises {
            if !x.pais.is_empty() && !x.pais.starts_with('@') {
                paises.insert(x.pais.clone());
            }
        }
        for d in &p.destinos {
            destinos.insert((d.destino.clone(), d.red.clone()));
        }
    }
    r.paises = paises.len();
    r.destinos = destinos.len();
    // `por_producto` is not sorted yet at this point; order here by bytes.
    let mut orden: Vec<&TraficoProducto> = por_producto.iter().collect();
    orden.sort_by_key(|p| std::cmp::Reverse(p.subidos + p.bajados));
    r.productos = orden.iter().map(|p| p.producto).collect();
    r
}

/// `/api/antivirus`: what each antivirus asked for this week, and every product's official record.
pub(crate) async fn vista(
    Zona(zona): Zona,
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<Vista> {
    const DIAS: i64 = 7;
    let now = now_ms();
    // The week with Plus, the last 24 hours at the free level (display only).
    let semana = crate::plan::desde(&state, now - DIAS * 24 * HOUR_MS, now);
    let (filas, devices, trafico, recientes, vigilando, con_ruta) = with_lectura(&state, |l| {
        Ok((
            l.program_names(semana, now + 1, zona)?,
            l.devices()?,
            l.trafico(semana, now + 1, &[])?,
            l.trafico(now - VIVO_MINUTOS * 60_000, now + 1, &[])?,
            l.vigilando_antivirus(),
            CortesAntivirus::rutas_por_producto(l)
                .into_iter()
                .map(|(p, _)| p)
                .collect::<Vec<_>>(),
        ))
    })?;
    let cortados = state.cortes.cortados();
    let maxima = !con_ruta.is_empty() && con_ruta.iter().all(|p| cortados.iter().any(|c| c == p));
    let trafico_horas = trafico
        .iter()
        .map(|f| f.minuto)
        .min()
        .map_or(0, |m| (now - m) / HOUR_MS + 1);
    let ultimas_24h: Vec<guardiana_core::Trafico> = trafico
        .iter()
        .filter(|f| f.minuto >= now - 24 * HOUR_MS)
        .cloned()
        .collect();
    let resumen = resume(&calcula_trafico(&ultimas_24h, zona));
    let trafico = calcula_trafico(&trafico, zona);
    // The live feed: the latest minutes, newest first, only of a known antivirus.
    let mut vivo: Vec<VivoFila> = recientes
        .into_iter()
        .filter_map(|f| {
            antivirus::antivirus_de_programa(&f.programa).map(|av| VivoFila {
                minuto: f.minuto,
                producto: av.nombre,
                programa: f.programa,
                destino: f.destino,
                red: f.red,
                pais: f.pais,
                subidos: f.enviados,
                bajados: f.recibidos,
            })
        })
        .collect();
    vivo.sort_by_key(|v| std::cmp::Reverse((v.minuto, v.subidos + v.bajados)));
    vivo.truncate(VIVO_MAX);
    let comparten: Vec<&str> = devices
        .iter()
        .filter(|d| d.id == SELF_DEVICE_ID || d.share_detail_with_home)
        .map(|d| d.id.as_str())
        .collect();
    let nombres: HashMap<String, Option<String>> = devices
        .iter()
        .map(|d| (d.id.clone(), d.name.clone()))
        .collect();
    let vistos = calcula(&filas, &nombres, &|id| {
        id == SELF_DEVICE_ID || comparten.contains(&id)
    });
    Ok(axum::Json(Vista {
        vistos,
        ocultos: devices.len() - comparten.len(),
        catalogo: antivirus::antivirus().iter().map(ficha).collect(),
        trafico,
        trafico_horas,
        vivo,
        cortar_disponible: state.cortes.disponible(),
        cortados,
        vigilando,
        maxima,
        resumen,
        dias: DIAS,
    }))
}

#[derive(serde::Deserialize)]
pub(crate) struct CortarBody {
    /// The product to cut or let through.
    producto: String,
    /// true to cut everything it sends, false to let it through again.
    cortar: bool,
}

#[derive(Serialize)]
pub(crate) struct CortarView {
    /// How many filters are in place for the product now (0 when let through).
    filtros: usize,
    /// All the products cut right now.
    cortados: Vec<String>,
}

/// Cut or let through one product, and record the wanted set so a restart re-applies it.
fn pon_corte(
    state: &Arc<AppState>,
    producto: &str,
    cortar: bool,
    rutas: &[String],
) -> Result<usize, axum::response::Response> {
    // Change the firewall (outside the ledger lock): its error is its own.
    let filtros = state
        .cortes
        .aplica_rutas(producto, cortar, rutas)
        .map_err(|e| {
            (axum::http::StatusCode::SERVICE_UNAVAILABLE, e.to_string()).into_response()
        })?;
    with_ledger(state, |l| {
        let mut set = l
            .setting(CLAVE_CORTES)?
            .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
            .unwrap_or_default();
        set.retain(|p| p != producto);
        if cortar {
            set.push(producto.to_owned());
        }
        l.set_setting(
            CLAVE_CORTES,
            &serde_json::to_string(&set).unwrap_or_else(|_| "[]".to_owned()),
        )
    })?;
    Ok(filtros)
}

/// `/api/antivirus/cortar`: cut (or let through) everything an antivirus sends out (1.0.22).
/// Always the person's explicit choice; always reversible.
pub(crate) async fn cortar(
    State(state): State<Arc<AppState>>,
    _s: Session,
    axum::Json(body): axum::Json<CortarBody>,
) -> ApiResult<CortarView> {
    if !antivirus::antivirus()
        .iter()
        .any(|a| a.nombre == body.producto)
    {
        return Err((axum::http::StatusCode::BAD_REQUEST, "unknown product").into_response());
    }
    // The executable paths of the product, from what this computer has seen (needs the ledger).
    let rutas = with_ledger(&state, |l| Ok(CortesAntivirus::rutas_de(l, &body.producto)))?;
    let filtros = pon_corte(&state, &body.producto, body.cortar, &rutas)?;
    Ok(axum::Json(CortarView {
        filtros,
        cortados: state.cortes.cortados(),
    }))
}

#[derive(serde::Deserialize)]
pub(crate) struct ActivaBody {
    activa: bool,
}

/// `/api/antivirus/maxima`: «Cortar todo lo que envía el antivirus» (1.0.22; «Protección máxima
/// del antivirus» until 13 Oct 2026, a name that hid that the antivirus then protects less): cut
/// every product this computer has seen, or let them all through again. One click each way.
pub(crate) async fn maxima(
    State(state): State<Arc<AppState>>,
    _s: Session,
    axum::Json(body): axum::Json<ActivaBody>,
) -> ApiResult<CortarView> {
    let todos = with_ledger(&state, |l| Ok(CortesAntivirus::rutas_por_producto(l)))?;
    let mut filtros = 0;
    if body.activa {
        if todos.is_empty() {
            return Err((
                axum::http::StatusCode::CONFLICT,
                "no antivirus seen on this computer yet",
            )
                .into_response());
        }
        for (producto, rutas) in &todos {
            filtros += pon_corte(&state, producto, true, rutas)?;
        }
    } else {
        // Let through everything that is cut, seen or not (it may no longer be listed).
        for producto in state.cortes.cortados() {
            pon_corte(&state, &producto, false, &[])?;
        }
    }
    Ok(axum::Json(CortarView {
        filtros,
        cortados: state.cortes.cortados(),
    }))
}

/// `/api/antivirus/vigilar`: GUARDIANA counts what the antivirus sends and receives, or stops
/// counting (1.0.22). It never undoes a cut: that is the person's other choice.
pub(crate) async fn vigilar(
    State(state): State<Arc<AppState>>,
    _s: Session,
    axum::Json(body): axum::Json<ActivaBody>,
) -> ApiResult<serde_json::Value> {
    with_ledger(&state, |l| l.vigila_antivirus(body.activa))?;
    Ok(axum::Json(serde_json::json!({ "vigilando": body.activa })))
}

/// A plain local date-time `YYYY-MM-DD HH:MM` from a Unix-ms minute, for the CSV.
fn fecha_hora_local(ms: i64, zona_min: i64) -> String {
    let t = ms.div_euclid(1000) + zona_min * 60;
    let (dias, seg) = (t.div_euclid(86_400), t.rem_euclid(86_400));
    // Civil date from a day number (Howard Hinnant's algorithm), days since 1970-01-01.
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}",
        seg.div_euclid(3600),
        (seg.rem_euclid(3600)).div_euclid(60)
    )
}

/// `/api/antivirus/export`: all the antivirus traffic there is, as a CSV the person can keep or
/// show (1.0.22). One row per minute, program and destination; the address is never in it.
pub(crate) async fn export(
    Zona(zona): Zona,
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> Result<axum::response::Response, axum::response::Response> {
    use std::fmt::Write as _;
    let now = now_ms();
    let filas = with_ledger(&state, |l| l.trafico(0, now + 1, &[]))?;
    let mut csv = String::new();
    writeln!(
        csv,
        "minuto;fecha_hora;producto;programa;firma;destino;pais;red;asn;protocolo;puerto;bytes_enviados;bytes_recibidos;de_noche"
    )
    .ok();
    let celda = |s: &str| {
        if s.contains([';', '"', '\n']) {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_owned()
        }
    };
    for f in &filas {
        let Some(av) = antivirus::antivirus_de_programa(&f.programa) else {
            continue;
        };
        let resto = (f.minuto + zona * 60_000).rem_euclid(86_400_000);
        let noche = resto < 6 * 3_600_000;
        let fecha = fecha_hora_local(f.minuto, zona);
        let _ = writeln!(
            csv,
            "{};{};{};{};{};{};{};{};{};{};{};{};{};{}",
            f.minuto,
            celda(&fecha),
            celda(av.nombre),
            celda(&f.programa),
            celda(&f.firma),
            celda(&f.destino),
            celda(&f.pais),
            celda(&f.red),
            f.asn,
            celda(&f.proto),
            f.puerto,
            f.enviados,
            f.recibidos,
            if noche { "sí" } else { "no" },
        );
    }
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"guardiana-antivirus-trafico.csv\"".to_owned(),
            ),
        ],
        csv,
    )
        .into_response())
}

/// `/api/antivirus/detectar`: which antivirus Windows Security Center lists as installed, and
/// whether each is on and up to date (1.0.21, the «Analizar mi antivirus» button). Read-only.
pub(crate) async fn detectar(_s: Session) -> ApiResult<DeteccionView> {
    // a PowerShell on Windows: off the page's threads, which the resolver shares
    let (lista, aviso) = match tokio::task::spawn_blocking(guardiana_apps::antivirus_instalados)
        .await
    {
        Ok(Ok(l)) => (l, None),
        Ok(Err(guardiana_apps::Error::NoSoportado)) => (Vec::new(), Some("no_windows".to_owned())),
        Ok(Err(e)) => (Vec::new(), Some(e.to_string())),
        Err(e) => (Vec::new(), Some(e.to_string())),
    };
    // Say which of the detected products GUARDIANA's list knows (so the page can link to it).
    let conocido = |n: &str| {
        antivirus::antivirus().iter().any(|a| {
            n.to_ascii_lowercase()
                .contains(&a.nombre.to_ascii_lowercase())
        })
    };
    let detectados: Vec<Detectado> = lista
        .into_iter()
        .map(|a| Detectado {
            conocido: conocido(&a.nombre),
            nombre: a.nombre,
            ruta: a.ruta,
            activo: a.activo,
            al_dia: a.al_dia,
        })
        .collect();
    Ok(axum::Json(DeteccionView { detectados, aviso }))
}

#[derive(Serialize)]
pub(crate) struct DeteccionView {
    detectados: Vec<Detectado>,
    /// Why nothing was detected, when that is the reason rather than «none installed».
    aviso: Option<String>,
}

#[derive(Serialize)]
struct Detectado {
    nombre: String,
    ruta: String,
    activo: bool,
    al_dia: bool,
    /// Whether GUARDIANA's list has this product (then the page offers its record).
    conocido: bool,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use guardiana_core::ProgramName;

    fn fila(programa: &str, firma: &str, qname: &str, n: u64, cat: Category) -> ProgramName {
        ProgramName {
            device_id: SELF_DEVICE_ID.into(),
            qname: qname.into(),
            program: programa.into(),
            signer: firma.into(),
            queries: n,
            night: n / 2,
            cut: 0,
            last_ts: 1_000 + i64::try_from(n).unwrap(),
            category: cat,
        }
    }

    #[test]
    fn the_csv_date_is_the_persons_local_day_and_minute() {
        // 1791900000000 ms = 2026-10-13 14:00 UTC; in UTC-5 it is 09:00 the same day.
        assert_eq!(
            super::fecha_hora_local(1_791_900_000_000, 0),
            "2026-10-13 14:00"
        );
        assert_eq!(
            super::fecha_hora_local(1_791_900_000_000, -300),
            "2026-10-13 09:00"
        );
    }

    #[test]
    fn each_antivirus_gets_its_names_with_what_its_maker_says_and_what_it_does_not() {
        let av = antivirus::antivirus_de_programa("MsMpEng.exe").unwrap();
        let documentado = av.dominios[0].dominio;
        let filas = vec![
            fila(
                "MsMpEng.exe",
                "Microsoft Windows Publisher",
                documentado,
                40,
                Category::Esperado,
            ),
            fila(
                "MsMpEng.exe",
                "Microsoft Windows Publisher",
                "doubleclick.net",
                2,
                Category::Publicidad,
            ),
            fila(
                "MpCmdRun.exe",
                "Microsoft Windows",
                documentado,
                4,
                Category::Esperado,
            ),
            fila(
                "chrome.exe",
                "Google LLC",
                "doubleclick.net",
                90,
                Category::Publicidad,
            ),
            {
                let mut f = fila("MsMpEng.exe", "", "a.example", 1, Category::Desconocido);
                f.device_id = "mac:otro".into();
                f
            },
        ];
        let v = calcula(&filas, &HashMap::new(), &|id| id == SELF_DEVICE_ID);
        // Chrome is no antivirus, and the device that does not share is left out.
        assert_eq!(v.len(), 1);
        let d = &v[0];
        assert_eq!(d.producto, "Microsoft Defender");
        assert_eq!(d.consultas, 46);
        assert_eq!((d.documentados, d.no_documentados), (1, 1));
        // What the maker does not document, and a list calls advertising, goes first.
        assert_eq!(d.destinos[0].qname, "doubleclick.net");
        assert!(d.destinos[0].uso.is_none());
        assert_eq!(d.destinos[0].empresa, Some("Google"));
        assert_eq!(d.destinos[1].uso, Some(av.dominios[0].uso));
        assert!(d.destinos[1].fuente.unwrap().starts_with("https://"));
        assert!(d.destinos[1].empresa.is_none());
        // Both processes, each with its signer.
        assert_eq!(d.programas.len(), 2);
        assert_eq!(d.programas[0].nombre, "MsMpEng.exe");
    }
    #[test]
    fn every_decision_use_and_kind_has_its_words_in_every_language() {
        for (lengua, texto) in [
            ("es", include_str!("../i18n/es.json")),
            ("en", include_str!("../i18n/en.json")),
            ("pt", include_str!("../i18n/pt.json")),
        ] {
            let v: serde_json::Value = serde_json::from_str(texto).unwrap();
            let panel = &v["panel"];
            let tiene = |k: &str| panel[k].as_str().is_some_and(|x| !x.is_empty());
            for u in antivirus::USOS {
                assert!(tiene(&format!("av_uso_{u}")), "{lengua}: av_uso_{u}");
            }
            for t in antivirus::TIPOS {
                assert!(tiene(&format!("av_tipo_{t}")), "{lengua}: av_tipo_{t}");
            }
            for a in antivirus::antivirus() {
                for s in &a.sanciones {
                    assert!(
                        tiene(&format!("av_sancion_{}", s.clave)),
                        "{lengua}: {}",
                        s.clave
                    );
                }
            }
        }
    }
}

/// On a real Windows, as an administrator (reviews of vpn-1055b and arreglos-1055): the cuts are the
/// process's, in a session of keys of its own, opened again when Windows lost it. What is cut is
/// always an empty file of the test's own, which nothing runs, seen as one of Malwarebytes'
/// programs; every test lets it through at the end (and it would go with the process anyway). They
/// share the process's cuts, as the panels of one service do, so they run one at a time.
#[cfg(all(test, windows))]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod en_windows {
    use super::*;
    use guardiana_core::{Hash, Ledger, NewEvent, Proceso};

    /// A folder of the test's own, empty but for an empty file named as one of Malwarebytes'
    /// programs, which nothing runs: what is cut. Its path, as the ledger would give it.
    fn carpeta(nombre: &str) -> (std::path::PathBuf, String) {
        let dir =
            std::env::temp_dir().join(format!("guardiana-cortes-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let programa = dir.join("mbamservice.exe");
        std::fs::write(&programa, b"").unwrap();
        (dir, programa.display().to_string())
    }

    /// One more empty file in `dir`, named as another of Malwarebytes' programs.
    fn otro(dir: &std::path::Path, nombre: &str) -> String {
        let p = dir.join(nombre);
        std::fs::write(&p, b"").unwrap();
        p.display().to_string()
    }

    /// A panel start for the cuts: the process's firewall opened if it is not, or again if Windows
    /// lost it (with a ledger that wants nothing).
    fn arranca() {
        let (dir, _) = carpeta("abrir");
        let l = Ledger::open(&dir.join("ledger.db"), Hash::of(b"antivirus cuts test key")).unwrap();
        global().al_arrancar(&l);
        drop(l);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `f` with the process's firewall session.
    fn con_sesion<T>(f: impl FnOnce(&guardiana_apps::Cortafuegos) -> T) -> T {
        let e = global().estado.lock().unwrap();
        f(e.cortafuegos.as_ref().expect("the firewall opened"))
    }

    /// How many of its session's filters Windows has, asked to Windows.
    fn en_windows() -> usize {
        con_sesion(|cf| cf.cuantos().unwrap())
    }

    /// The ids of the filters in place for `producto`, path by path.
    fn ids_de(producto: &str) -> Vec<u64> {
        global()
            .estado
            .lock()
            .unwrap()
            .puestos
            .get(producto)
            .map(|r| r.values().flatten().copied().collect())
            .unwrap_or_default()
    }

    /// Letting a product through goes through even when one of its filters is already gone, as
    /// after a removal that failed part-way (review of arreglos-1055: «not found» was read under the
    /// code of another error, and every let-through stopped at it until the service restarted); and
    /// asking Windows how many filters are in place counts a gone one as gone, not as a failure.
    #[test]
    #[ignore = "necesita Windows como administrador"]
    fn dejar_pasar_sigue_con_un_filtro_que_ya_no_esta() {
        arranca();
        let (dir, programa) = carpeta("ya-no-esta");
        let cortes = global();
        assert_eq!(
            cortes
                .aplica_rutas("Malwarebytes", true, &[programa])
                .unwrap(),
            2
        );
        let ids = ids_de("Malwarebytes");
        assert_eq!(ids.len(), 2);
        // One goes behind the panel's back; asked again, it is gone, not an error.
        con_sesion(|cf| cf.quitar(&ids[..1])).unwrap();
        con_sesion(|cf| cf.quitar(&ids[..1])).unwrap();
        assert_eq!(en_windows(), 1);
        // The let-through still goes through, and nothing of it stays in Windows.
        assert_eq!(cortes.aplica_rutas("Malwarebytes", false, &[]).unwrap(), 0);
        assert_eq!(en_windows(), 0);
        assert!(cortes.cortados().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The process's cuts outlive a panel rebuilt while the old panel's state is still alive (a
    /// slow look kept it, until 1.0.55): the cut in place stays in the same session, not put twice,
    /// and Windows still has it once the old state is gone (review of vpn-1055b).
    #[tokio::test]
    #[ignore = "necesita Windows como administrador"]
    async fn los_cortes_siguen_al_rehacer_el_panel_con_el_viejo_vivo() {
        let (dir, programa) = carpeta("rehacer");
        let db = dir.join("ledger.db");
        let genesis = Hash::of(b"antivirus cuts test key");
        {
            let mut l = Ledger::open(&db, genesis).unwrap();
            let mut e = NewEvent::observed(
                now_ms() - 60_000,
                SELF_DEVICE_ID,
                "127.0.0.1",
                "telemetry.malwarebytes.com",
                "A",
            );
            e.process = Some(Proceso {
                pid: 4242,
                nombre: "mbamservice.exe".into(),
                ruta: programa,
                sha256: String::new(),
                firmado_por: String::new(),
                version: String::new(),
            });
            l.append(e).unwrap();
            l.set_setting(CLAVE_CORTES, r#"["Malwarebytes"]"#).unwrap();
        }
        let config = || crate::Config {
            listen: vec![std::net::TcpListener::bind("127.0.0.1:0")
                .unwrap()
                .local_addr()
                .unwrap()],
            optional_listen: Vec::new(),
            db_path: db.clone(),
            genesis,
            token_path: dir.join("panel.token"),
            extra_hosts: Vec::new(),
            info: crate::RuntimeInfo::default(),
        };
        let cortes = global();
        let ids = || ids_de("Malwarebytes");

        let viejo = crate::start(config()).await.expect("the first panel");
        let primeros = ids();
        assert_eq!(primeros.len(), 2, "one filter per family");
        assert_eq!(en_windows(), 2);
        // The next pass, while the first one's state is still alive; and then the old one goes.
        let nuevo = crate::start(config()).await.expect("the panel rebuilt");
        let puestos = ids();
        assert_eq!(puestos, primeros, "the cut in place stays, not put twice");
        viejo.shutdown().await;
        assert_eq!(ids(), puestos);
        assert_eq!(
            en_windows(),
            2,
            "Windows still has the cut once the old panel is gone"
        );
        assert_eq!(cortes.cortados(), vec!["Malwarebytes".to_owned()]);
        // Let through, it is gone from Windows too.
        cortes.aplica_rutas("Malwarebytes", false, &[]).unwrap();
        assert_eq!(en_windows(), 0);
        nuevo.shutdown().await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A session opened first by another process (an elevated `guardiana`, for a moment) no longer
    /// owns what this process's cuts hang from (review of arreglos-1055): this process cuts, the
    /// other one's cut goes in too, and when the other session ends this process's cuts are still in
    /// Windows. With GUARDIANA's fixed keys, until then, this process's cut was refused with
    /// «FwpmFilterAdd0 = 0x8032000c» (`FWP_E_WRONG_SESSION`), as this module's tests saw.
    #[test]
    #[ignore = "necesita Windows como administrador"]
    fn otra_sesion_abierta_antes_no_se_lleva_los_cortes() {
        let (dir, programa) = carpeta("otra-sesion");
        let otra = guardiana_apps::Cortafuegos::abrir().unwrap();
        // This process's session, opened again after the other one.
        global().estado.lock().unwrap().cortafuegos = None;
        arranca();
        let cortes = global();
        assert_eq!(
            cortes
                .aplica_rutas("Malwarebytes", true, &[programa.clone()])
                .unwrap(),
            2
        );
        let suyo = otra.cortar(&programa, &guardiana_apps::Destino::default());
        assert!(suyo.as_ref().is_ok_and(|ids| ids.len() == 2), "{suyo:?}");
        drop(otra);
        assert_eq!(en_windows(), 2, "the other session took nothing with it");
        assert_eq!(cortes.aplica_rutas("Malwarebytes", false, &[]).unwrap(), 0);
        assert_eq!(en_windows(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// When Windows no longer has the session, nor so its cuts (its filtering service restarted),
    /// the next change of a cut, or the next panel start, opens a new session and puts every cut
    /// back in it (review of arreglos-1055). Here the session really closes, as with that restart:
    /// Windows removes its cuts, and the one put in its place has none of them.
    #[test]
    #[ignore = "necesita Windows como administrador"]
    fn una_sesion_perdida_se_vuelve_a_abrir_con_sus_cortes() {
        arranca();
        let (dir, programa) = carpeta("perdida");
        let cortes = global();
        assert_eq!(
            cortes
                .aplica_rutas("Malwarebytes", true, &[programa])
                .unwrap(),
            2
        );
        let pierde = || {
            let antes = ids_de("Malwarebytes");
            let mut e = cortes.estado.lock().unwrap();
            e.cortafuegos = Some(guardiana_apps::Cortafuegos::abrir().unwrap());
            let cf = e.cortafuegos.as_ref().unwrap();
            assert!(cf.sigue_abierto());
            assert_eq!(cf.cuantos_de(&antes).unwrap(), 0, "Windows removed them");
            antes
        };
        // The next change of a cut (another product's, with nothing to cut) puts them back.
        let antes = pierde();
        assert_eq!(
            cortes
                .aplica_rutas("Microsoft Defender", true, &[])
                .unwrap(),
            0
        );
        let despues = ids_de("Malwarebytes");
        assert_eq!(despues.len(), 2);
        assert_ne!(despues, antes);
        assert_eq!(en_windows(), 2);
        // And the next panel start.
        let antes = pierde();
        arranca();
        let despues = ids_de("Malwarebytes");
        assert_eq!(despues.len(), 2);
        assert_ne!(despues, antes);
        assert_eq!(en_windows(), 2);
        assert_eq!(cortes.aplica_rutas("Malwarebytes", false, &[]).unwrap(), 0);
        assert_eq!(en_windows(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A path with no file at it is skipped, and the paths that are there are cut; with every path
    /// gone nothing is cut and nothing fails; a file that appears later at a skipped path is cut at
    /// the next look at the skipped paths (review of arreglos-1055).
    #[test]
    #[ignore = "necesita Windows como administrador"]
    fn una_ruta_que_ya_no_esta_se_salta_y_se_corta_cuando_aparece() {
        arranca();
        let (dir, esta) = carpeta("saltadas");
        let falta = dir.join("otra").join("mbamservice.exe");
        let falta_s = falta.display().to_string();
        let cortes = global();
        let pon = |rutas: &[String]| cortes.aplica_rutas("Malwarebytes", true, rutas);
        // Every path gone: nothing is cut, and it is no error.
        assert_eq!(pon(&[falta_s.clone()]).unwrap(), 0);
        assert_eq!(en_windows(), 0);
        assert!(cortes.cortados().is_empty());
        // One gone and one there: the one there is cut.
        assert_eq!(pon(&[esta, falta_s]).unwrap(), 2);
        assert_eq!(en_windows(), 2);
        // The file appears at the skipped path: cut at the next look.
        std::fs::create_dir_all(dir.join("otra")).unwrap();
        std::fs::write(&falta, b"").unwrap();
        let hasta = std::time::Instant::now() + CADA_SALTADAS * 3;
        while ids_de("Malwarebytes").len() < 4 && std::time::Instant::now() < hasta {
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        assert_eq!(ids_de("Malwarebytes").len(), 4, "cut once it appeared");
        assert_eq!(en_windows(), 4);
        assert_eq!(cortes.aplica_rutas("Malwarebytes", false, &[]).unwrap(), 0);
        assert_eq!(en_windows(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Putting a product's cut back with fewer paths (one of its programs asked no name this week)
    /// keeps the cuts already in place, and nothing is put twice: only the paths not cut yet are
    /// added, and only letting the product through takes filters off (review of arreglos-1055).
    #[test]
    #[ignore = "necesita Windows como administrador"]
    fn volver_a_poner_con_menos_rutas_deja_los_cortes_de_antes() {
        arranca();
        let (dir, a) = carpeta("menos-rutas");
        let b = otro(&dir, "Malwarebytes.exe");
        let cortes = global();
        let pon = |rutas: &[String]| cortes.aplica_rutas("Malwarebytes", true, rutas).unwrap();
        assert_eq!(pon(&[a.clone(), b.clone()]), 4);
        let antes = ids_de("Malwarebytes");
        assert_eq!(pon(&[a.clone()]), 4, "the cut of the path left out stays");
        assert_eq!(pon(&[]), 4);
        assert_eq!(pon(&[b.clone(), a.clone()]), 4, "nothing is put twice");
        assert_eq!(ids_de("Malwarebytes"), antes);
        assert_eq!(en_windows(), 4);
        // A path not cut yet is added to them.
        let c = otro(&dir, "MBVpnTunnelService.exe");
        assert_eq!(pon(&[c]), 6);
        assert_eq!(en_windows(), 6);
        // Only letting it through takes them off.
        assert_eq!(cortes.aplica_rutas("Malwarebytes", false, &[]).unwrap(), 0);
        assert_eq!(en_windows(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
