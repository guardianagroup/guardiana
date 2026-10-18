//! «Tu visita, a subasta», «No me subastes» and «El rastro de tus datos» (1.0.46).
//!
//! When a web page with advertising opens, its ad program (most often Prebid.js, with Amazon's
//! `apstag` and Google's `googletag` beside it) offers your visit to several companies, which
//! answer with a bid for showing you an ad. GUARDIANA ZERO sees two sides of it without
//! decrypting anything:
//!
//! - **the requests** the page makes (every one passes the request filter): which auction
//!   companies are asked, which only cross identifiers between companies («cookie sync», with the
//!   next company announced inside the address), and whether each one was cut;
//! - **the page's own auction program**, read by the page script in the page itself (ZERO is the
//!   person's browser, the page runs on their computer): the companies asked, how many ad slots,
//!   the bids and their price per thousand (CPM, the bid; never «the price your data sold for»),
//!   the winner when the page says it, the time limit, the *names* of the fields sent (never
//!   their values), the identifier systems named in the request and whether a consent string
//!   went with it.
//!
//! What it cannot see, and says so every time: auctions run from the site's own servers
//! («server-side»), what each company does with what it got, private deals and clean rooms.
//!
//! Everything here is in memory for the open page; the day's two figures (auctions seen and
//! distinct companies) are kept for the summary. No address of the person, no value of a field,
//! no identifier is ever kept. Self-contained (only the lists and the URL parser) so it can be
//! read and tested on its own.

use std::collections::{BTreeMap, BTreeSet};

use guardiana_lists::dominio::sitio;
use guardiana_lists::subastas::{papel_de, sitio_de_pujador, Papel};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The most auctions one page keeps (an endless page must not grow without end).
pub const MAX_SUBASTAS: usize = 60;
/// The most companies one page keeps.
const MAX_EMPRESAS: usize = 150;
/// The most identifier crossings one page keeps.
const MAX_CADENAS: usize = 80;
/// The most names of the page seen, for telling «seen» from «announced».
const MAX_VISTOS: usize = 1500;
/// The most bidders in one auction.
const MAX_PUJADORES: usize = 60;
/// The most field names of one auction.
const MAX_CAMPOS: usize = 80;
/// Days of the day's figures kept.
const DIAS: usize = 60;

/// Text from the page, cut short and without control characters.
fn limpio(s: &str, n: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(n).collect()
}

fn texto<'a>(m: &'a Value, k: &str) -> &'a str {
    m.get(k).and_then(Value::as_str).unwrap_or("")
}

fn numero(m: &Value, k: &str) -> Option<f64> {
    m.get(k).and_then(Value::as_f64).filter(|x| x.is_finite())
}

fn entero(m: &Value, k: &str, max: u32) -> Option<u32> {
    numero(m, k)
        .filter(|x| *x >= 0.0)
        .map(|x| x.min(f64::from(max)) as u32)
}

/// A bidder code as Prebid writes them: letters, digits, `_` and `-`.
fn codigo(s: &str) -> Option<String> {
    let c: String = s.trim().chars().take(40).collect();
    (!c.is_empty()
        && c.chars()
            .all(|x| x.is_ascii_alphanumeric() || x == '_' || x == '-'))
    .then_some(c)
}

/// A host name as the page gave it (a field path, an identifier source, a seller system).
fn nombre(s: &str) -> Option<String> {
    let n = s.trim().trim_end_matches('.').to_ascii_lowercase();
    (n.len() <= 120
        && n.contains('.')
        && n.chars()
            .all(|x| x.is_ascii_alphanumeric() || x == '.' || x == '-'))
    .then_some(n)
}

/// A field path of an OpenRTB request (`device.ua`, `user.ext.eids`): names only.
fn campo(s: &str) -> Option<String> {
    let c = s.trim();
    (!c.is_empty()
        && c.len() <= 60
        && c.chars()
            .all(|x| x.is_ascii_alphanumeric() || x == '.' || x == '_'))
    .then(|| c.to_string())
}

/// The kinds of data the field names say went in a bid request, in the order the screen says
/// them: the page's address, the browser and system, the screen, the language, the approximate
/// place, identifiers that follow you, interest segments, what you are reading, and what you said
/// about consent. Each kind is a key of the texts (`sub_dato_<clave>`).
#[must_use]
pub fn clases_de_campos(campos: &BTreeSet<String>, ids: &BTreeSet<String>) -> Vec<&'static str> {
    let tiene = |f: &dyn Fn(&str) -> bool| campos.iter().any(|c| f(c));
    let mut out = Vec::new();
    if tiene(&|c| c.starts_with("site.page") || c.starts_with("site.ref") || c == "site.domain") {
        out.push("pagina");
    }
    if tiene(&|c| c.starts_with("device.ua") || c.starts_with("device.sua") || c == "device.os") {
        out.push("navegador");
    }
    if tiene(&|c| c == "device.w" || c == "device.h" || c.starts_with("device.ext.vpw")) {
        out.push("pantalla");
    }
    if tiene(&|c| c.starts_with("device.language") || c.starts_with("device.langb")) {
        out.push("idioma");
    }
    if tiene(&|c| c.starts_with("device.geo") || c.starts_with("user.geo")) {
        out.push("lugar");
    }
    if !ids.is_empty() || tiene(&|c| c.starts_with("user.eids") || c.starts_with("user.ext.eids")) {
        out.push("ids");
    }
    if tiene(&|c| c.starts_with("user.data") || c.starts_with("user.keywords")) {
        out.push("segmentos");
    }
    if tiene(&|c| {
        c.starts_with("site.content") || c.starts_with("site.keywords") || c.starts_with("site.cat")
    }) {
        out.push("lectura");
    }
    if tiene(&|c| c.starts_with("regs") || c.starts_with("user.ext.consent")) {
        out.push("consentimiento");
    }
    out
}

/// One bid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Puja {
    /// The bid per thousand ads, as the page received it.
    pub cpm: f64,
    /// Its currency (`USD`), as the page wrote it.
    pub moneda: String,
}

/// One company asked in one auction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pujador {
    /// Its code in Prebid.js (`criteo`).
    pub codigo: String,
    /// The site of its names, from the list (`criteo.com`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sitio: Option<String>,
    /// Who it is: the owner in `empresas.txt`, or its code.
    pub quien: String,
    /// The owner's country, only when `empresas.txt` has it checked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pais: Option<String>,
    /// Bid requests (one per ad slot it was asked about).
    pub peticiones: u32,
    /// Its bids.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pujas: Vec<Puja>,
    /// It answered «no bid».
    #[serde(default)]
    pub sin_puja: bool,
    /// It ran out of time.
    #[serde(default)]
    pub tarde: bool,
}

/// What went with the request about consent: whether a consent string of each kind was there
/// (never the string).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Consentimiento {
    /// IAB Europe's TCF string.
    pub tcf: bool,
    /// The US privacy string.
    pub usp: bool,
    /// IAB's GPP string.
    pub gpp: bool,
}

/// The winner, when the page said one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ganador {
    /// Its code in Prebid.js.
    pub codigo: String,
    /// Who it is.
    pub quien: String,
    /// Its bid.
    pub puja: Puja,
}

/// One auction of the page, as its own program told it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Subasta {
    /// When it ended (ms since 1970).
    pub ts: i64,
    /// How long it took, from start to end.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ms: Option<u32>,
    /// The time limit the page gave the bidders.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limite: Option<u32>,
    /// Ad slots offered.
    pub espacios: u32,
    /// The companies asked.
    pub pujadores: Vec<Pujador>,
    /// The winner, when the page says it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ganador: Option<Ganador>,
    /// The names of the fields that went in the request (never their values).
    pub campos: BTreeSet<String>,
    /// Identifier systems named in the request (`id5-sync.com`, `uidapi.com`).
    pub ids: BTreeSet<String>,
    /// Consent strings that went with it.
    pub consentimiento: Consentimiento,
    /// The chain of sellers the bids declared (`schain`): their advertising systems.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vendedores: Vec<String>,
    /// Names a bid said to call when it wins or is billed (`nurl`, `burl`).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub avisos: BTreeSet<String>,
    /// The page's id of the auction, to join its winner (in memory; never shown or written).
    #[serde(skip)]
    pub clave: String,
}

/// The page's ad programs, as found in the page.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Programas {
    /// Prebid.js and its version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prebid: Option<String>,
    /// Amazon's `apstag`.
    pub apstag: bool,
    /// Google's `googletag` (Google Ad Manager): its auction runs on Google's servers.
    pub gpt: bool,
    /// Ad slots `googletag` declared.
    pub gpt_espacios: u32,
    /// A consent manager answering IAB's TCF on the page, and whether it says the GDPR applies.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tcf: Option<bool>,
}

/// A company of the auction seen in the page's requests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Empresa {
    /// Its site.
    pub sitio: String,
    /// The owner in `empresas.txt`, or the site.
    pub quien: String,
    /// The owner's country, only when checked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pais: Option<String>,
    /// Its parts (`puja`, `sincroniza`, `id`, `servidor`).
    pub papeles: BTreeSet<String>,
    /// The identifier system, for an identifier company.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sistema: Option<String>,
    /// Where to opt out of that identifier, when published.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baja: Option<String>,
    /// Requests seen.
    pub peticiones: u32,
    /// Requests cut.
    pub cortadas: u32,
}

/// One crossing of identifiers: the company called, and the ones its address announces next.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cadena {
    /// The names, in order: the first was requested; the rest were written inside its address.
    pub saltos: Vec<String>,
    /// The first request was cut, so none of it happened.
    pub cortada: bool,
}

/// What the site declares in its `ads.txt` (read only when the person asks).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdsTxt {
    /// The file was there.
    pub hay: bool,
    /// Lines that declare a seller.
    pub lineas: u32,
    /// `DIRECT` lines.
    pub directos: u32,
    /// `RESELLER` lines.
    pub revendedores: u32,
    /// The advertising systems named, each once.
    pub sistemas: Vec<String>,
}

/// What one page of a tab showed of its auctions.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Pagina {
    /// The page's site.
    pub sitio: String,
    /// The page script's token for this page (to ask it for `ads.txt`); never shown.
    #[serde(skip)]
    pub zg: String,
    /// Its auctions, oldest first.
    pub subastas: Vec<Subasta>,
    /// Auctions not kept for being past [`MAX_SUBASTAS`].
    pub mas_subastas: u32,
    /// The companies of the auction seen in its requests, by site.
    pub empresas: BTreeMap<String, Empresa>,
    /// The crossings of identifiers.
    pub cadenas: Vec<Cadena>,
    /// The ad programs found.
    pub programas: Programas,
    /// Its `ads.txt`, once asked for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adstxt: Option<AdsTxt>,
    /// Names seen in its requests (to tell «seen» from «announced»).
    #[serde(skip)]
    vistos: BTreeSet<String>,
}

/// An address is a crossing of identifiers between companies by its path or its first name:
/// `/usersync`, `/cksync`, `/setuid`, `/match`, `sync.` … Only asked for another company's
/// address.
#[must_use]
pub fn es_sincronizacion(host: &str, ruta: &str) -> bool {
    let h = host.to_ascii_lowercase();
    let primero = h.split('.').next().unwrap_or("");
    if matches!(
        primero,
        "sync" | "usersync" | "user-sync" | "cookiesync" | "cm" | "match" | "um" | "cs"
    ) || primero.ends_with("-sync")
        || primero.starts_with("sync-")
    {
        return true;
    }
    let r = ruta.to_ascii_lowercase();
    r.split('/').any(|t| {
        let t = t.split('.').next().unwrap_or("");
        matches!(
            t,
            "usersync"
                | "user-sync"
                | "user_sync"
                | "usync"
                | "cksync"
                | "cookiesync"
                | "cookie-sync"
                | "setuid"
                | "getuid"
                | "pixelsync"
                | "cookie_sync"
                | "idsync"
        )
    })
}

/// The page's own auction program by its file name: `prebid.js`, `prebid9.12.min.js`,
/// `pbjs-bundle.js`. Cut with «No me subastes» wherever it is served from, even the site itself.
#[must_use]
pub fn es_programa_de_subasta(ruta: &str) -> bool {
    let archivo = ruta.rsplit('/').next().unwrap_or("").to_ascii_lowercase();
    archivo.ends_with(".js") && (archivo.contains("prebid") || archivo.starts_with("pbjs"))
}

/// Whether «No me subastes» cuts this request: the page's auction program by its name, any
/// name the auction list knows (bid requests, identifier crossings, the ad program), or another
/// company's address that is a crossing of identifiers.
#[must_use]
pub fn corta(host: &str, ruta: &str, otro_sitio: bool, de_publicidad: bool) -> bool {
    es_programa_de_subasta(ruta)
        || papel_de(host).is_some()
        || (otro_sitio && de_publicidad && es_sincronizacion(host, ruta))
}

/// The names announced inside an address: a value of its query that is itself an address
/// (`?redir=https%3A%2F%2Fb.example%2F…`), and the one inside that, up to four.
#[must_use]
pub fn anunciados(direccion: &url::Url) -> Vec<String> {
    let mut out = Vec::new();
    let mut actual = direccion.clone();
    for _ in 0..4 {
        let siguiente = actual
            .query_pairs()
            .map(|(_, v)| v.into_owned())
            .find(|v| v.starts_with("https://") || v.starts_with("http://"))
            .and_then(|v| url::Url::parse(&v).ok());
        let Some(s) = siguiente else { break };
        let Some(h) = s.host_str().and_then(nombre) else {
            break;
        };
        if out.last() == Some(&h) {
            break;
        }
        out.push(h);
        actual = s;
    }
    out
}

fn quien_y_pais(s: &str) -> (String, Option<String>) {
    (
        guardiana_lists::company_of(s).map_or_else(|| s.to_string(), str::to_string),
        guardiana_lists::country_of(s).map(str::to_string),
    )
}

impl Pagina {
    /// A new page of `sitio`.
    #[must_use]
    pub fn nueva(sitio: &str) -> Self {
        Self {
            sitio: sitio.to_string(),
            ..Self::default()
        }
    }

    /// Nothing of an auction was seen.
    #[must_use]
    pub fn vacia(&self) -> bool {
        self.subastas.is_empty()
            && self.empresas.is_empty()
            && self.cadenas.is_empty()
            && self.programas == Programas::default()
    }

    /// One request of the page, decided: `otro_sitio` when it goes to another site,
    /// `de_publicidad` when the lists call that name advertising or tracking, `cortada` when it
    /// did not leave. Returns whether the page's auction view changed.
    pub fn anota_peticion(
        &mut self,
        direccion: &url::Url,
        host: &str,
        otro_sitio: bool,
        de_publicidad: bool,
        cortada: bool,
    ) -> bool {
        if self.vistos.len() < MAX_VISTOS {
            self.vistos.insert(host.to_string());
        }
        if !otro_sitio {
            return false;
        }
        let rol = papel_de(host);
        let sincro = rol.as_ref().is_some_and(|r| r.papel == Papel::Sincroniza)
            || (de_publicidad && es_sincronizacion(host, direccion.path()));
        if rol.is_none() && !sincro {
            return false;
        }
        let s = sitio(host);
        if !self.empresas.contains_key(&s) && self.empresas.len() >= MAX_EMPRESAS {
            return false;
        }
        let e = self.empresas.entry(s.clone()).or_insert_with(|| {
            let (quien, pais) = quien_y_pais(&s);
            Empresa {
                sitio: s.clone(),
                quien,
                pais,
                papeles: BTreeSet::new(),
                sistema: None,
                baja: None,
                peticiones: 0,
                cortadas: 0,
            }
        });
        e.peticiones = e.peticiones.saturating_add(1);
        if cortada {
            e.cortadas = e.cortadas.saturating_add(1);
        }
        if let Some(r) = &rol {
            e.papeles.insert(r.papel.clave().to_string());
            if let Some(si) = &r.sistema {
                e.sistema = Some(si.nombre.to_string());
                e.baja = si.baja.map(str::to_string);
            }
        }
        if sincro {
            e.papeles.insert(Papel::Sincroniza.clave().to_string());
            let mut saltos = vec![host.to_string()];
            saltos.extend(anunciados(direccion));
            let ya = self
                .cadenas
                .iter()
                .position(|c| c.saltos.get(..2) == saltos.get(..2) || c.saltos == saltos);
            match ya {
                Some(i) => self.cadenas[i].cortada &= cortada,
                None if self.cadenas.len() < MAX_CADENAS => {
                    self.cadenas.push(Cadena { saltos, cortada })
                }
                None => {}
            }
        }
        true
    }

    /// A report of the page script (`zg_subasta`). Everything it says is checked and cut short:
    /// a page can post anything there. Returns the auction to add to the day's figures, when it
    /// was one.
    pub fn anota_mensaje(&mut self, m: &Value, ahora: i64) -> Option<Subasta> {
        match texto(m, "que") {
            "programas" => {
                let p = &mut self.programas;
                if let Some(v) = m.get("prebid").and_then(Value::as_str) {
                    let v = limpio(v, 24);
                    p.prebid = Some(v);
                }
                p.apstag |= m.get("apstag").and_then(Value::as_bool) == Some(true);
                p.gpt |= m.get("gpt").and_then(Value::as_bool) == Some(true);
                if let Some(n) = entero(m, "gpt_espacios", 200) {
                    p.gpt_espacios = p.gpt_espacios.max(n);
                }
                if let Some(t) = m.get("tcf") {
                    if t.is_boolean() {
                        p.tcf = t.as_bool();
                    } else if t.is_object() {
                        p.tcf = Some(t.get("gdpr").and_then(Value::as_bool) == Some(true));
                    }
                }
                None
            }
            "subasta" => {
                let s = self.subasta_de(m.get("s")?, ahora)?;
                if self.subastas.len() >= MAX_SUBASTAS {
                    self.mas_subastas = self.mas_subastas.saturating_add(1);
                } else {
                    self.subastas.push(s.clone());
                }
                Some(s)
            }
            "gana" => {
                let clave = limpio(texto(m, "id"), 64);
                let c = codigo(texto(m, "c"))?;
                let cpm = numero(m, "cpm").filter(|x| *x >= 0.0 && *x < 10_000.0)?;
                let moneda = limpio(texto(m, "moneda"), 3).to_ascii_uppercase();
                let s = self
                    .subastas
                    .iter_mut()
                    .rev()
                    .find(|s| !clave.is_empty() && s.clave == clave)?;
                let quien = s
                    .pujadores
                    .iter()
                    .find(|p| p.codigo == c)
                    .map_or_else(|| c.clone(), |p| p.quien.clone());
                s.ganador = Some(Ganador {
                    codigo: c,
                    quien,
                    puja: Puja { cpm, moneda },
                });
                None
            }
            "adstxt" => {
                let mut a = AdsTxt {
                    hay: m.get("hay").and_then(Value::as_bool) == Some(true),
                    lineas: entero(m, "lineas", 1_000_000).unwrap_or(0),
                    directos: entero(m, "directos", 1_000_000).unwrap_or(0),
                    revendedores: entero(m, "revendedores", 1_000_000).unwrap_or(0),
                    sistemas: Vec::new(),
                };
                let mut vistos = BTreeSet::new();
                for s in m
                    .get("sistemas")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .filter_map(nombre)
                {
                    if vistos.len() >= 300 {
                        break;
                    }
                    if vistos.insert(s.clone()) {
                        a.sistemas.push(s);
                    }
                }
                self.adstxt = Some(a);
                None
            }
            _ => None,
        }
    }

    fn subasta_de(&self, s: &Value, ahora: i64) -> Option<Subasta> {
        let mut pujadores: Vec<Pujador> = Vec::new();
        for p in s
            .get("pujadores")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if pujadores.len() >= MAX_PUJADORES {
                break;
            }
            let Some(c) = codigo(texto(p, "c")) else {
                continue;
            };
            let sitio = sitio_de_pujador(&c).map(str::to_string);
            let (quien, pais) = match &sitio {
                Some(si) => {
                    let (q, pa) = quien_y_pais(si);
                    // A site nobody owns in the lists: the bidder's own code says more.
                    (if &q == si { c.clone() } else { q }, pa)
                }
                None => (c.clone(), None),
            };
            let pujas: Vec<Puja> = p
                .get("pujas")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|x| {
                    let cpm = numero(x, "cpm").filter(|v| *v >= 0.0 && *v < 10_000.0)?;
                    let moneda = limpio(texto(x, "moneda"), 3).to_ascii_uppercase();
                    Some(Puja { cpm, moneda })
                })
                .take(50)
                .collect();
            match pujadores.iter_mut().find(|x| x.codigo == c) {
                Some(x) => {
                    x.peticiones = x
                        .peticiones
                        .saturating_add(entero(p, "n", 1000).unwrap_or(1));
                    x.pujas.extend(pujas);
                }
                None => pujadores.push(Pujador {
                    codigo: c,
                    sitio,
                    quien,
                    pais,
                    peticiones: entero(p, "n", 1000).unwrap_or(1),
                    pujas,
                    sin_puja: p.get("nb").and_then(Value::as_bool) == Some(true),
                    tarde: p.get("to").and_then(Value::as_bool) == Some(true),
                }),
            }
        }
        if pujadores.is_empty() {
            return None;
        }
        let lista = |k: &str, f: fn(&str) -> Option<String>, n: usize| -> BTreeSet<String> {
            s.get(k)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter_map(f)
                .take(n)
                .collect()
        };
        let c = s.get("consentimiento").cloned().unwrap_or(Value::Null);
        let si = |k: &str| c.get(k).and_then(Value::as_bool) == Some(true);
        Some(Subasta {
            ts: ahora,
            ms: entero(s, "ms", 120_000),
            limite: entero(s, "limite", 120_000),
            espacios: entero(s, "espacios", 500).unwrap_or(0),
            pujadores,
            ganador: None,
            campos: lista("campos", campo, MAX_CAMPOS),
            ids: lista("ids", nombre, 40),
            consentimiento: Consentimiento {
                tcf: si("tcf"),
                usp: si("usp"),
                gpp: si("gpp"),
            },
            vendedores: lista("vendedores", nombre, 20).into_iter().collect(),
            avisos: lista("avisos", nombre, 20),
            clave: limpio(texto(s, "id"), 64),
        })
    }

    /// What became of a bidder's requests on this page: `cortada` (all of those seen were cut),
    /// `salio` (some left), or `no_vista` (none of its names was seen: it may have gone by
    /// another name, or from the site's servers).
    #[must_use]
    pub fn estado_de(&self, p: &Pujador) -> &'static str {
        match p.sitio.as_ref().and_then(|s| self.empresas.get(s)) {
            Some(e) if e.peticiones > 0 && e.cortadas >= e.peticiones => "cortada",
            Some(e) if e.peticiones > 0 => "salio",
            _ => "no_vista",
        }
    }

    /// The page's auction view, for the side summary: the figures of the headline and every
    /// list, already worked out (the screen only words them).
    #[must_use]
    pub fn vista(&self) -> Value {
        // The companies the visit was offered to: every bidder of every auction, each once.
        let mut ofrecida: BTreeMap<String, Value> = BTreeMap::new();
        let mut peticiones = 0u32;
        let mut pujaron = BTreeSet::new();
        let mut cortadas = BTreeSet::new();
        let mut campos = BTreeSet::new();
        let mut ids = BTreeSet::new();
        let mut consentimiento = Consentimiento::default();
        let mut pujas: Vec<Value> = Vec::new();
        for s in &self.subastas {
            campos.extend(s.campos.iter().cloned());
            ids.extend(s.ids.iter().cloned());
            consentimiento.tcf |= s.consentimiento.tcf;
            consentimiento.usp |= s.consentimiento.usp;
            consentimiento.gpp |= s.consentimiento.gpp;
            for p in &s.pujadores {
                peticiones = peticiones.saturating_add(p.peticiones);
                let estado = self.estado_de(p);
                if estado == "cortada" {
                    cortadas.insert(p.codigo.clone());
                }
                if !p.pujas.is_empty() {
                    pujaron.insert(p.codigo.clone());
                }
                for b in &p.pujas {
                    pujas.push(json!({ "quien": p.quien, "cpm": b.cpm, "moneda": b.moneda }));
                }
                let e = ofrecida.entry(p.codigo.clone()).or_insert_with(|| {
                    json!({
                        "codigo": p.codigo,
                        "quien": p.quien,
                        "sitio": p.sitio,
                        "pais": p.pais,
                        "peticiones": 0,
                        "pujas": 0,
                        "estado": estado,
                    })
                });
                e["peticiones"] =
                    json!(e["peticiones"].as_u64().unwrap_or(0) + u64::from(p.peticiones));
                e["pujas"] = json!(e["pujas"].as_u64().unwrap_or(0) + p.pujas.len() as u64);
            }
        }
        pujas.sort_by(|a, b| {
            b["cpm"]
                .as_f64()
                .partial_cmp(&a["cpm"].as_f64())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        pujas.truncate(12);
        let ultima = self.subastas.last();
        let ganador = self.subastas.iter().rev().find_map(|s| s.ganador.clone());
        // Companies of the auction seen only in the requests (no auction program read): the
        // headline counts them instead.
        let empresas: Vec<&Empresa> = self.empresas.values().collect();
        let empresas_cortadas = empresas
            .iter()
            .filter(|e| e.peticiones > 0 && e.cortadas >= e.peticiones)
            .count();
        let vendedores: BTreeSet<&String> = self
            .subastas
            .iter()
            .flat_map(|s| s.vendedores.iter())
            .collect();
        let avisos: BTreeSet<&String> =
            self.subastas.iter().flat_map(|s| s.avisos.iter()).collect();
        json!({
            "sitio": self.sitio,
            "subastas": self.subastas.len() as u32 + self.mas_subastas,
            "ofrecida": ofrecida.values().collect::<Vec<_>>(),
            "peticiones": peticiones,
            "espacios": ultima.map_or(0, |s| s.espacios),
            "ms": ultima.and_then(|s| s.ms),
            "limite": ultima.and_then(|s| s.limite),
            "pujaron": pujaron.len(),
            "cortadas": cortadas.len(),
            "pujas": pujas,
            "ganador": ganador,
            "datos": clases_de_campos(&campos, &ids),
            "campos": campos,
            "ids": ids,
            "consentimiento": consentimiento,
            "programas": self.programas,
            "empresas": empresas,
            "empresas_cortadas": empresas_cortadas,
            "cadenas": self.cadenas,
            "adstxt": self.adstxt,
            "rastro": self.rastro(&vendedores, &avisos),
        })
    }

    /// «El rastro de tus datos»: where the visit went, as a tree with four marks. `visto`: a
    /// request this browser saw (a company called, the next company of a crossing that was
    /// actually requested, a seller the bids declared, a name a bid said to call); `declarado`:
    /// what the site declares in its `ads.txt`; `probable`: the next company a crossing
    /// announced, which was never requested here; `invisible`: what happens out of this
    /// computer's sight (always there).
    fn rastro(&self, vendedores: &BTreeSet<&String>, avisos: &BTreeSet<&String>) -> Value {
        let mut ramas: Vec<Value> = Vec::new();
        for e in self.empresas.values() {
            let mut hijos: Vec<Value> = Vec::new();
            for c in self
                .cadenas
                .iter()
                .filter(|c| c.saltos.first().is_some_and(|h| sitio(h) == e.sitio))
            {
                for h in c.saltos.iter().skip(1) {
                    let visto = self.vistos.contains(h);
                    let marca = if visto { "visto" } else { "probable" };
                    let s = sitio(h);
                    let (quien, pais) = quien_y_pais(&s);
                    if !hijos.iter().any(|x| x["nombre"] == json!(h)) {
                        hijos.push(json!({ "nombre": h, "quien": quien, "pais": pais, "marca": marca, "cortada": c.cortada }));
                    }
                }
            }
            ramas.push(json!({
                "nombre": e.sitio,
                "quien": e.quien,
                "pais": e.pais,
                "marca": "visto",
                "cortada": e.peticiones > 0 && e.cortadas >= e.peticiones,
                "papeles": e.papeles,
                "hijos": hijos,
            }));
        }
        for v in vendedores {
            let (quien, pais) = quien_y_pais(&sitio(v));
            ramas.push(json!({ "nombre": v, "quien": quien, "pais": pais, "marca": "visto", "papeles": ["vendedor"], "hijos": [] }));
        }
        for v in avisos {
            let (quien, pais) = quien_y_pais(&sitio(v));
            ramas.push(json!({ "nombre": v, "quien": quien, "pais": pais, "marca": "visto", "papeles": ["aviso"], "hijos": [] }));
        }
        let declarado: Vec<Value> = self
            .adstxt
            .iter()
            .flat_map(|a| a.sistemas.iter())
            .take(60)
            .map(|s| {
                let (quien, pais) = quien_y_pais(&sitio(s));
                json!({ "nombre": s, "quien": quien, "pais": pais, "marca": "declarado", "sellers": format!("https://{s}/sellers.json") })
            })
            .collect();
        json!({
            "raiz": self.sitio,
            "ramas": ramas,
            "declarado": declarado,
            "invisible": ["servidor", "salas_limpias", "acuerdos_privados", "despues"],
        })
    }

    /// The signed receipt's content for this page: what was seen, never a value.
    #[must_use]
    pub fn recibo(&self, version: &str, generado: i64) -> Value {
        let subastas: Vec<Value> = self
            .subastas
            .iter()
            .map(|s| {
                json!({
                    "ts": s.ts,
                    "ms": s.ms,
                    "limite": s.limite,
                    "espacios": s.espacios,
                    "pujadores": s.pujadores.iter().map(|p| json!({
                        "codigo": p.codigo,
                        "empresa": p.quien,
                        "pais": p.pais,
                        "sitio": p.sitio,
                        "peticiones": p.peticiones,
                        "pujas": p.pujas,
                        "estado": self.estado_de(p),
                    })).collect::<Vec<_>>(),
                    "ganador": s.ganador,
                    "campos": s.campos,
                    "ids": s.ids,
                    "consentimiento": s.consentimiento,
                })
            })
            .collect();
        json!({
            "programa": "GUARDIANA ZERO",
            "tipo": "recibo_de_subasta",
            "version": version,
            "generado": generado,
            "web": self.sitio,
            "programas": self.programas,
            "subastas": subastas,
            "empresas": self.empresas.values().collect::<Vec<_>>(),
            "cruces_de_identificadores": self.cadenas,
            "cobertura": {
                "observado": [
                    "peticiones_de_esta_pestana",
                    "programa_de_subasta_de_la_pagina",
                    "nombres_de_campos",
                ],
                "no_observado": [
                    "subastas_en_los_servidores_de_la_web",
                    "lo_que_cada_empresa_hace_con_lo_recibido",
                    "acuerdos_privados_y_salas_limpias",
                    "valores_de_los_campos",
                    "precio_final_del_anuncio",
                ],
            },
        })
    }
}

/// The figures of one day.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Dia {
    /// Auctions seen.
    pub subastas: u32,
    /// Bid requests in them.
    pub peticiones: u32,
    /// Bid requests that GUARDIANA cut.
    pub cortadas: u32,
    /// The distinct companies of the auction (sites).
    pub empresas: BTreeSet<String>,
}

/// The day's figures, kept [`DIAS`] days (`subastas.json`). Never an address or a page.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Diario {
    /// By day (`2026-10-18`).
    pub dias: BTreeMap<String, Dia>,
}

impl Diario {
    /// One auction, on `dia`; `cortada` tells, per bidder, whether its requests were cut.
    pub fn anota(&mut self, dia: &str, s: &Subasta, cortada: impl Fn(&Pujador) -> bool) {
        let d = self.dias.entry(dia.to_string()).or_default();
        d.subastas = d.subastas.saturating_add(1);
        for p in &s.pujadores {
            d.peticiones = d.peticiones.saturating_add(p.peticiones);
            if cortada(p) {
                d.cortadas = d.cortadas.saturating_add(p.peticiones);
            }
            if d.empresas.len() < 2000 {
                d.empresas
                    .insert(p.sitio.clone().unwrap_or_else(|| p.codigo.clone()));
            }
        }
        while self.dias.len() > DIAS {
            let Some(k) = self.dias.keys().next().cloned() else {
                break;
            };
            self.dias.remove(&k);
        }
    }

    /// A company of the auction seen in a request, on `dia`.
    pub fn empresa(&mut self, dia: &str, s: &str) {
        let d = self.dias.entry(dia.to_string()).or_default();
        if d.empresas.len() < 2000 {
            d.empresas.insert(s.to_string());
        }
    }

    /// The figures from `desde` to `hasta` (days as written): auctions, requests, cut, distinct
    /// companies.
    #[must_use]
    pub fn suma(&self, desde: &str, hasta: &str) -> Value {
        let mut subastas = 0u32;
        let mut peticiones = 0u32;
        let mut cortadas = 0u32;
        let mut empresas: BTreeSet<&String> = BTreeSet::new();
        for (_, d) in self.dias.range(desde.to_string()..=hasta.to_string()) {
            subastas = subastas.saturating_add(d.subastas);
            peticiones = peticiones.saturating_add(d.peticiones);
            cortadas = cortadas.saturating_add(d.cortadas);
            empresas.extend(d.empresas.iter());
        }
        json!({ "subastas": subastas, "peticiones": peticiones, "cortadas": cortadas, "empresas": empresas.len() })
    }
}

/// The kinds of letter to a company seen in an auction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Carta {
    /// GDPR art. 15: what data of mine do you have, from whom, to whom.
    Acceso,
    /// GDPR art. 21 and art. 17: stop using my data for advertising and delete it.
    Oposicion,
    /// Colombia, Ley 1581 de 2012 (Habeas Data), art. 8: know, update, delete.
    Habeas,
}

impl Carta {
    /// From its key.
    #[must_use]
    pub fn de(clave: &str) -> Option<Self> {
        match clave {
            "acceso" => Some(Self::Acceso),
            "oposicion" => Some(Self::Oposicion),
            "habeas" => Some(Self::Habeas),
            _ => None,
        }
    }

    /// Its key in the texts.
    #[must_use]
    pub const fn clave(self) -> &'static str {
        match self {
            Self::Acceso => "acceso",
            Self::Oposicion => "oposicion",
            Self::Habeas => "habeas",
        }
    }
}

/// The letter's holes, filled: `{empresa}`, `{web}`, `{fecha}`, `{hechos}` (what was seen, one
/// line each), `{recibo}` (the signed receipt's file and the key's fingerprint, or nothing),
/// `{nombre}`, `{correo}`. `plantilla` is the text in the language the company must answer in.
#[must_use]
pub fn rellena(plantilla: &str, huecos: &[(&str, &str)]) -> String {
    let mut s = plantilla.to_string();
    for (k, v) in huecos {
        s = s.replace(&format!("{{{k}}}"), v);
    }
    s
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// The auction of eltiempo.com on 18 Oct 2026 (Prebid.js 11.12.0): six bidders, four slots,
    /// 1 300 ms, no bid. «Datos de ejemplo» built from it.
    fn eltiempo() -> Value {
        json!({ "que": "subasta", "s": {
            "id": "a1", "ms": 1300, "limite": 1300, "espacios": 4,
            "pujadores": [
                { "c": "mathildeads", "n": 4, "nb": true },
                { "c": "criteo", "n": 4, "nb": true },
                { "c": "rubicon", "n": 4, "to": true },
                { "c": "teads", "n": 4, "nb": true },
                { "c": "smartadserver", "n": 4, "nb": true },
                { "c": "richaudience", "n": 4, "nb": true }
            ],
            "campos": ["site.page", "site.ref", "device.ua", "device.w", "device.h", "regs.ext.gdpr", "user.ext.eids"],
            "ids": ["id5-sync.com", "criteo.com"],
            "consentimiento": { "tcf": true }
        }})
    }

    fn u(s: &str) -> url::Url {
        url::Url::parse(s).unwrap()
    }

    #[test]
    fn the_eltiempo_auction_reads_as_six_companies_none_of_which_could_bid() {
        let mut p = Pagina::nueva("eltiempo.com");
        p.anota_mensaje(&json!({ "que": "programas", "prebid": "v11.12.0", "apstag": true, "gpt": true, "tcf": { "gdpr": false } }), 1);
        // GUARDIANA cut every bid request.
        for h in [
            "grid-bidder.criteo.com",
            "a.teads.tv",
            "prg.smartadserver.com",
            "shb.richaudience.com",
            "endpoint2.mathilde-ads.com",
            "fastlane.rubiconproject.com",
        ] {
            assert!(p.anota_peticion(&u(&format!("https://{h}/bid")), h, true, true, true));
        }
        let s = p.anota_mensaje(&eltiempo(), 2).unwrap();
        assert_eq!(s.pujadores.len(), 6);
        let v = p.vista();
        assert_eq!(v["ofrecida"].as_array().unwrap().len(), 6);
        assert_eq!(v["peticiones"], 24);
        assert_eq!(v["espacios"], 4);
        assert_eq!(v["ms"], 1300);
        assert_eq!(v["pujaron"], 0);
        assert_eq!(v["cortadas"], 6);
        assert_eq!(v["programas"]["prebid"], "v11.12.0");
        let quienes: Vec<&str> = v["ofrecida"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["quien"].as_str().unwrap())
            .collect();
        for q in [
            "Criteo",
            "Magnite",
            "Teads",
            "Equativ",
            "Rich Audience",
            "mathildeads",
        ] {
            assert!(quienes.contains(&q), "{q} {quienes:?}");
        }
        // Country only where the company list has it checked.
        let criteo = v["ofrecida"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["codigo"] == "criteo")
            .unwrap();
        assert_eq!(criteo["pais"], "FR");
        let mathilde = v["ofrecida"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["codigo"] == "mathildeads")
            .unwrap();
        assert!(mathilde["pais"].is_null());
        assert_eq!(
            v["datos"],
            json!(["pagina", "navegador", "pantalla", "ids", "consentimiento"])
        );
        // Field names travel; values never could (the message has none, and none are kept).
        assert!(!v.to_string().contains("eltiempo.com/") && !v.to_string().contains("Mozilla"));
    }

    #[test]
    fn a_bid_and_a_winner_are_said_as_bids_never_as_a_price_for_the_person() {
        let mut p = Pagina::nueva("ejemplo.co");
        let mut m = eltiempo();
        m["s"]["pujadores"][1] = json!({ "c": "criteo", "n": 4, "pujas": [{ "cpm": 0.42, "moneda": "usd" }, { "cpm": "x" }] });
        p.anota_mensaje(&m, 5);
        p.anota_mensaje(
            &json!({ "que": "gana", "id": "a1", "c": "criteo", "cpm": 0.42, "moneda": "USD" }),
            6,
        );
        let v = p.vista();
        assert_eq!(v["pujaron"], 1);
        assert_eq!(v["pujas"][0]["cpm"], 0.42);
        assert_eq!(v["pujas"][0]["moneda"], "USD");
        assert_eq!(v["ganador"]["quien"], "Criteo");
        // Nothing seen of its requests: not said to be cut.
        assert_eq!(v["cortadas"], 0);
        assert_eq!(v["ofrecida"][1]["estado"], "no_vista");
        // The auction's id stays in memory.
        assert!(!serde_json::to_string(&p.subastas[0])
            .unwrap()
            .contains("a1"));
    }

    #[test]
    fn whatever_the_page_posts_is_checked_and_cut_short() {
        let mut p = Pagina::nueva("x.co");
        let malo = json!({ "que": "subasta", "s": { "pujadores": [
            { "c": "<script>", "n": 5 }, { "c": "ok", "n": 1e12 }
        ], "campos": ["device.ua", "a b", "x".repeat(200)], "ids": ["javascript:alert(1)", "uidapi.com"] } });
        let s = p.anota_mensaje(&malo, 1).unwrap();
        assert_eq!(s.pujadores.len(), 1);
        assert_eq!(s.pujadores[0].peticiones, 1000);
        assert_eq!(s.campos.len(), 1);
        assert_eq!(s.ids.iter().collect::<Vec<_>>(), vec!["uidapi.com"]);
        assert!(p
            .anota_mensaje(&json!({ "que": "subasta", "s": { "pujadores": [] } }), 1)
            .is_none());
        for _ in 0..(MAX_SUBASTAS + 5) {
            p.anota_mensaje(&eltiempo(), 1);
        }
        assert_eq!(p.subastas.len(), MAX_SUBASTAS);
        assert_eq!(p.vista()["subastas"], MAX_SUBASTAS as u64 + 6);
    }

    #[test]
    fn a_crossing_of_identifiers_shows_the_next_company_as_announced_until_it_is_seen() {
        let mut p = Pagina::nueva("ejemplo.co");
        let a = u("https://sync.richaudience.com/usersync?redir=https%3A%2F%2Fmatch.adsrvr.org%2Ftrack%2Fcmf%3Fr%3Dhttps%253A%252F%252Fx.bidswitch.net%252Fsync");
        assert_eq!(anunciados(&a), vec!["match.adsrvr.org", "x.bidswitch.net"]);
        p.anota_peticion(&a, "sync.richaudience.com", true, true, true);
        let v = p.vista();
        assert_eq!(
            v["cadenas"][0]["saltos"],
            json!([
                "sync.richaudience.com",
                "match.adsrvr.org",
                "x.bidswitch.net"
            ])
        );
        assert_eq!(v["cadenas"][0]["cortada"], true);
        let rama = v["rastro"]["ramas"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["nombre"] == "richaudience.com")
            .unwrap();
        assert_eq!(rama["hijos"][0]["marca"], "probable");
        assert_eq!(rama["hijos"][0]["quien"], "The Trade Desk");
        // Once requested, it is seen.
        p.anota_peticion(
            &u("https://match.adsrvr.org/track/cmf"),
            "match.adsrvr.org",
            true,
            true,
            false,
        );
        let v = p.vista();
        let rama = v["rastro"]["ramas"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["nombre"] == "richaudience.com")
            .unwrap();
        assert_eq!(rama["hijos"][0]["marca"], "visto");
        assert_eq!(v["rastro"]["invisible"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn the_page_s_own_requests_and_ordinary_names_are_not_auctions() {
        let mut p = Pagina::nueva("ejemplo.co");
        assert!(!p.anota_peticion(
            &u("https://ejemplo.co/usersync"),
            "ejemplo.co",
            false,
            false,
            false
        ));
        assert!(!p.anota_peticion(
            &u("https://fonts.gstatic.com/x.woff2"),
            "fonts.gstatic.com",
            true,
            false,
            false
        ));
        // A crossing by its path counts only for a name the lists call advertising or tracking.
        assert!(!p.anota_peticion(
            &u("https://api.tienda.co/usersync"),
            "api.tienda.co",
            true,
            false,
            false
        ));
        assert!(p.anota_peticion(
            &u("https://ads.otra.co/usersync"),
            "ads.otra.co",
            true,
            true,
            false
        ));
        assert!(p.vista()["empresas"].as_array().unwrap().len() == 1);
        assert!(p.anota_peticion(
            &u("https://prod.uidapi.com/v2/token"),
            "prod.uidapi.com",
            true,
            false,
            false
        ));
        let e = &p.empresas["uidapi.com"];
        assert_eq!(e.sistema.as_deref(), Some("UID2"));
        assert_eq!(
            e.baja.as_deref(),
            Some("https://www.transparentadvertising.com/")
        );
    }

    #[test]
    fn no_me_subastes_cuts_the_program_the_list_and_the_crossings() {
        assert!(es_programa_de_subasta("/static/js/prebid9.12.min.js"));
        assert!(es_programa_de_subasta("/pbjs-bundle.js"));
        assert!(!es_programa_de_subasta("/prebid-guide.html"));
        assert!(!es_programa_de_subasta("/js/app.js"));
        assert!(corta("grid-bidder.criteo.com", "/openrtb", true, true));
        assert!(corta("eltiempo.com", "/assets/prebid.js", false, false));
        assert!(corta("ads.otra.co", "/cksync", true, true));
        assert!(!corta("ads.otra.co", "/cksync", true, false));
        assert!(!corta("www.eltiempo.com", "/", false, false));
    }

    #[test]
    fn the_day_counts_auctions_and_distinct_companies() {
        let mut d = Diario::default();
        let mut p = Pagina::nueva("x.co");
        let s = p.anota_mensaje(&eltiempo(), 1).unwrap();
        d.anota("2026-10-18", &s, |_| true);
        d.anota("2026-10-18", &s, |_| true);
        d.empresa("2026-10-18", "uidapi.com");
        let v = d.suma("2026-10-18", "2026-10-18");
        assert_eq!(v["subastas"], 2);
        assert_eq!(v["peticiones"], 48);
        assert_eq!(v["cortadas"], 48);
        assert_eq!(v["empresas"], 7);
        for i in 0..70 {
            d.anota(&format!("2026-08-{i:02}"), &s, |_| false);
        }
        assert!(d.dias.len() <= DIAS);
    }

    #[test]
    fn the_receipt_says_what_was_not_seen() {
        let mut p = Pagina::nueva("eltiempo.com");
        p.anota_mensaje(&eltiempo(), 1);
        let r = p.recibo("1.0.46", 9);
        assert_eq!(r["tipo"], "recibo_de_subasta");
        assert!(r["cobertura"]["no_observado"]
            .as_array()
            .unwrap()
            .contains(&json!("subastas_en_los_servidores_de_la_web")));
        assert!(!r.to_string().contains("\"a1\""));
    }

    #[test]
    fn a_letter_fills_its_holes() {
        assert_eq!(
            rellena(
                "A {empresa}: {web} {fecha}",
                &[
                    ("empresa", "Criteo"),
                    ("web", "eltiempo.com"),
                    ("fecha", "2026-10-18")
                ]
            ),
            "A Criteo: eltiempo.com 2026-10-18"
        );
        assert_eq!(Carta::de("habeas"), Some(Carta::Habeas));
        assert_eq!(Carta::de("x"), None);
    }
}
