//! The browser's brain, written once and tested everywhere: the tabs, the side panel, the
//! settings and every figure. The Windows shell (`zero/navegador`) owns the window and the
//! engine; it tells this module what happened (a message from the bar, a navigation, a request
//! about to leave) and does exactly what comes back ([`Orden`]). So everything the person sees
//! and every cut can be checked on any system, without a window.
//!
//! Three rules hold here and nowhere else:
//! - A web page never receives a message from the browser: only the browser's own pages
//!   (`https://zero.guardiana/…`) do, and the shell checks it again before posting.
//! - From a web page the browser accepts one thing, the form guard's question; everything else a
//!   page could post is ignored.
//! - Every figure is a count of requests decided here. Nothing is estimated.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::cartas::{carta, Ley};
use crate::cortes::{self, Corte, Lugar};
use crate::datos::{clases, Clase};
use crate::decision::{decide, Ajustes, Motivo, Peticion};
use crate::direccion::{a_direccion, buscador, codifica, BUSCADORES};
use crate::dominio::{host_de, sitio};
use crate::escudo::{self, Diario, Tercero};
use crate::favoritos::{self, Favoritos};
use crate::fecha;
use crate::libro::Libro;
use crate::licencia::{Estado as EstadoLicencia, Fallo, Lugar as LugarLicencia};
use crate::mandato::{Mandato, Recurso};
use crate::paginas;
use crate::recibo::{self, Recibo};
use crate::tachon::{self, Sustitucion};
use crate::textos::{Idioma, Textos};
use crate::tinta::{self, Forma, Marcado, Tipo};
use crate::url_limpia;

/// The browser's own pages live under this name, mapped to the program's folder.
pub const HOST_INTERNO: &str = "zero.guardiana";
/// The origin of the browser's own pages, with the slash.
pub const ORIGEN_INTERNO: &str = "https://zero.guardiana/";
/// The new tab.
pub const INICIO: &str = "https://zero.guardiana/inicio.html";
/// The bar at the top.
pub const BARRA: &str = "https://zero.guardiana/barra.html";
/// The side panel.
pub const PANEL: &str = "https://zero.guardiana/panel.html";
/// «Lo que se cortó», one by one.
pub const CORTES: &str = "https://zero.guardiana/cortes.html";

/// Whether an address is one of the browser's own pages.
#[must_use]
pub fn es_interna(url: &str) -> bool {
    url.starts_with(ORIGEN_INTERNO)
}

/// Who a message comes from or goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Origen {
    /// The bar at the top.
    Barra,
    /// The side panel.
    Panel,
    /// A tab, by id.
    Pestana(u32),
}

/// Where a tab keeps its cookies and sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Perfil {
    /// The person's normal browsing: kept between runs.
    General,
    /// An isolated tab: its own private session, gone when it closes.
    Aislada,
    /// A mandate's tab: a private session limited to the mandate's sites.
    Mandato,
}

/// Which script result the session is waiting for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Etiqueta {
    /// Pasting the redacted text into the AI's page.
    Pegar,
    /// Reading the selected answer, to put the data back in the panel.
    Seleccion,
}

/// What the shell must do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Orden {
    /// Create a tab. With `de`, it is the new window that tab `de` asked for: the shell hands
    /// it to the engine instead of navigating it.
    CreaPestana {
        /// Its id.
        id: u32,
        /// Its session.
        perfil: Perfil,
        /// What to open.
        url: Option<String>,
        /// The tab that opened it.
        de: Option<u32>,
    },
    /// Close a tab.
    CierraPestana {
        /// Its id.
        id: u32,
    },
    /// Show this tab (and hide the others).
    Activa {
        /// Its id.
        id: u32,
    },
    /// Open an address in a tab.
    Navega {
        /// The tab.
        id: u32,
        /// The address.
        url: String,
    },
    /// Back.
    Atras {
        /// The tab.
        id: u32,
    },
    /// Forward.
    Adelante {
        /// The tab.
        id: u32,
    },
    /// Reload.
    Recarga {
        /// The tab.
        id: u32,
    },
    /// Stop loading.
    Detiene {
        /// The tab.
        id: u32,
    },
    /// Open or close the side panel.
    Panel {
        /// Open.
        abierto: bool,
    },
    /// Post a JSON message to one of the browser's own pages.
    Envia {
        /// To whom.
        a: Origen,
        /// The message.
        json: String,
    },
    /// Run a script in a tab; with an `etiqueta`, its result comes back to
    /// [`Sesion::script_hecho`].
    Ejecuta {
        /// The tab.
        id: u32,
        /// The script.
        script: String,
        /// What the result is for.
        etiqueta: Option<Etiqueta>,
    },
    /// The answer to a page's form guard, on the channel only that page's guard listens to.
    /// The one message a web page ever gets from the browser: which question, yes or no.
    RespondeFormulario {
        /// The tab.
        id: u32,
        /// `{"zg": <the guard's own token>, "id": <its question>, "enviar": bool}`.
        json: String,
    },
    /// Save a tab showing one of the browser's own pages as a PDF file.
    GuardaPdf {
        /// The tab.
        id: u32,
        /// Where (in the person's Downloads).
        ruta: PathBuf,
    },
    /// Give the keyboard to a view.
    Foco {
        /// Which.
        a: Origen,
    },
    /// Open an address outside the browser (a `mailto:` in the person's mail program).
    AbreFuera {
        /// The address.
        url: String,
    },
    /// Delete cookies, cache and history of the kept session.
    BorraNavegacion,
    /// The engine's own tracking prevention: strict while cutting, balanced while watching.
    Seguimiento {
        /// Strict.
        estricto: bool,
    },
    /// Read the public ledger for a newer GUARDIANA ZERO (the person pressed «Buscar
    /// actualización»), away from the window; the answer comes back through
    /// [`Sesion::version_encontrada`].
    BuscaVersion,
    /// Day or night for the window's own frame (title bar): `Some(true)` night, `Some(false)`
    /// day, `None` like Windows.
    Tema {
        /// Night.
        oscuro: Option<bool>,
    },
    /// The window's title.
    Titulo {
        /// The text.
        texto: String,
    },
    /// Close the window (the last tab was closed).
    CierraVentana,
    /// Activate a subscription key: one connection to the payment gateway, asked for by the
    /// person. The shell does it away from the window ([`LugarLicencia::activar`]) and gives the result
    /// to [`Sesion::licencia_activada`].
    ActivaLicencia {
        /// The key, as typed.
        clave: String,
    },
    /// See whether the subscription's periodic check is due, and do it if so
    /// ([`LugarLicencia::comprobar`],
    /// away from the window); a new state goes to [`Sesion::pon_licencia`].
    ComprobarLicencia,
}

/// The answer for one request.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Respuesta {
    /// Do not send it.
    pub cortar: bool,
    /// For a page: what to show instead.
    pub pagina: Option<String>,
    /// Only when debugging (`pon_depuracion`): how it was read, for the browser's own log.
    pub nota: String,
}

/// How the shell starts a session.
#[derive(Debug, Clone)]
pub struct Arranque {
    /// The browser's data folder (settings, marked data, book, daily figures, receipts).
    pub datos: PathBuf,
    /// Where the person's downloads go (receipts and images are saved there when asked).
    pub descargas: PathBuf,
    /// The system's language tag (`es-CO`).
    pub idioma_sistema: String,
    /// The program's version.
    pub version: String,
    /// The clock: milliseconds since the epoch.
    pub reloj: fn() -> i64,
}

/// The system clock.
#[must_use]
pub fn ahora_sistema() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

fn azar(buf: &mut [u8]) {
    use ring::rand::SecureRandom as _;
    if ring::rand::SystemRandom::new().fill(buf).is_err() {
        // Without the system's randomness the ids are still unique within a run.
        let t = ahora_sistema().to_le_bytes();
        for (i, b) in buf.iter_mut().enumerate() {
            *b = t[i % t.len()] ^ (i as u8).wrapping_mul(151);
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// What the person chose, kept between runs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
struct Preferencias {
    reglas: Ajustes,
    /// The first-run question was answered.
    bienvenida: bool,
    buscador: String,
    /// `""` follows the system.
    idioma: String,
    /// Day or night for the browser's own pages: `"dia"`, `"noche"`, or `""` like Windows.
    tema: String,
    /// Leave cookie notices as they come (the person turned «reject cookie notices» off; it is
    /// on unless they do).
    cookies_sin_tocar: bool,
    /// Sites where the form guard does not ask (outside mandates).
    sin_preguntar: BTreeSet<String>,
    /// The person closed the shield: it no longer opens by itself at start (the owner, 10 Oct
    /// 2026: open from the start, live, until the person closes it).
    escudo_cerrado: bool,
}

#[derive(Debug, Clone)]
struct Pestana {
    id: u32,
    perfil: Perfil,
    url: String,
    titulo: String,
    cargando: bool,
    atras: bool,
    adelante: bool,
    /// The address the tab itself is loading: tells the page from the frames inside it.
    navegando: Option<String>,
    escudo: escudo::Pestana,
    /// Tracking parameters removed for the navigation that is about to start.
    quitados: u32,
    /// An address answered with the page that reopens it without tracking tags: it never left,
    /// and its short visit is not a page.
    limpiando: Option<String>,
    /// The last address the person typed or searched in this tab (their own data may go there).
    escrita: Option<String>,
    /// Sites already written in the book during this page (one line per page, not per request).
    al_libro: BTreeSet<String>,
    /// Its own page said it is ready.
    lista: bool,
    /// The site's icon as a `data:` PNG, as the engine fetched it (through the same checks).
    icono: String,
    /// The tab the person was on when this one opened for them (the list of cuts opens in a tab
    /// of its own): its «Back» button returns there.
    vuelve: Option<u32>,
}

impl Pestana {
    fn nueva(id: u32, perfil: Perfil, url: &str) -> Self {
        Self {
            id,
            perfil,
            url: url.to_string(),
            titulo: String::new(),
            cargando: false,
            atras: false,
            adelante: false,
            navegando: None,
            escudo: escudo::Pestana::default(),
            quitados: 0,
            limpiando: None,
            escrita: None,
            al_libro: BTreeSet::new(),
            lista: false,
            icono: String::new(),
            vuelve: None,
        }
    }
}

#[derive(Debug, Clone)]
struct Pregunta {
    pestana: u32,
    id_pagina: u64,
    ficha: String,
    sitio: String,
    host: String,
    clases: Vec<Clase>,
    mandato: bool,
}

/// The browser's state.
#[derive(Debug)]
pub struct Sesion {
    datos: PathBuf,
    descargas: PathBuf,
    version: String,
    motor: String,
    idioma_sistema: String,
    reloj: fn() -> i64,
    zona: i32,
    textos: Textos,
    prefs: Preferencias,
    tinta: Vec<Marcado>,
    formas: Vec<Forma>,
    formas_mandato: Vec<Forma>,
    libro: Libro,
    diario: Diario,
    favoritos: Favoritos,
    pestanas: Vec<Pestana>,
    activa: u32,
    siguiente: u32,
    panel: Option<String>,
    mandato: Option<Mandato>,
    terminado: Option<Mandato>,
    recibo: Option<Recibo>,
    tachon: Vec<Sustitucion>,
    preguntas: BTreeMap<u64, Pregunta>,
    siguiente_pregunta: u64,
    pulsos: Vec<Value>,
    sucios: BTreeSet<u32>,
    diario_sucio: bool,
    libro_sucio: bool,
    mandato_sucio: bool,
    hoy_sucio: bool,
    ultimo_guardado: i64,
    ultimo_hoy: i64,
    /// The day the figures and the cut log were last brought up to, to notice midnight.
    dia_visto: String,
    /// A version check is under way.
    buscando_version: bool,
    depura: bool,
    /// The engine reports requests of workers too (newer WebView2): said in the receipts.
    trabajadores: bool,
    /// Cuts not yet written to the day's file.
    cortes_pend: Vec<Corte>,
    /// The subscription. Unknown until the shell gives the place it lives in: the browser works.
    licencia: EstadoLicencia,
    /// Where the subscription lives (`None` in tests that do not need it).
    lugar: Option<LugarLicencia>,
    /// A key is being activated.
    licencia_ocupada: bool,
    /// What the last activation failed with, in words, until the next try.
    licencia_error: Option<String>,
    /// When the state was last read from disk, and when the periodic check was last asked for.
    licencia_leida: i64,
    licencia_pedida: i64,
}

fn lee<T: DeserializeOwned + Default>(ruta: &Path) -> T {
    fs::read(ruta)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Write a whole file or nothing: a crash never leaves half a settings file.
fn escribe(ruta: &Path, contenido: &[u8]) -> io::Result<()> {
    if let Some(dir) = ruta.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = ruta.with_extension("tmp");
    fs::write(&tmp, contenido)?;
    fs::rename(&tmp, ruta)
}

fn escribe_json<T: Serialize>(ruta: &Path, v: &T) {
    if let Ok(b) = serde_json::to_vec(v) {
        // Losing one save is better than stopping the browser; the next one retries.
        let _ = escribe(ruta, &b);
    }
}

fn texto<'a>(m: &'a Value, k: &str) -> &'a str {
    m.get(k).and_then(Value::as_str).unwrap_or("")
}

fn si(m: &Value, k: &str) -> bool {
    m.get(k).and_then(Value::as_bool).unwrap_or(false)
}

fn numero(m: &Value, k: &str) -> Option<u64> {
    m.get(k).and_then(Value::as_u64)
}

fn envia(a: Origen, v: &Value) -> Orden {
    Orden::Envia {
        a,
        json: v.to_string(),
    }
}

/// The engines as the settings and the new tab's search box show them.
fn lista_buscadores() -> Vec<Value> {
    BUSCADORES
        .iter()
        .map(|b| json!({ "id": b.id, "nombre": b.nombre, "privado": b.privado }))
        .collect()
}

fn sin_fragmento(url: &str) -> &str {
    url.split('#').next().unwrap_or(url)
}

/// A marked value as the panel shows it: enough to recognise it, not enough to read it over a
/// shoulder.
fn enmascara(tipo: Tipo, valor: &str) -> String {
    let v: Vec<char> = valor.chars().collect();
    if tipo == Tipo::Correo {
        if let Some((local, dominio)) = valor.split_once('@') {
            let l: String = local.chars().take(2).collect();
            return format!("{l}•••@{dominio}");
        }
    }
    if v.len() <= 4 {
        return "•••".into();
    }
    let ini: String = v.iter().take(2).collect();
    let fin: String = v[v.len() - 2..].iter().collect();
    format!("{ini}•••{fin}")
}

const fn clase_de(tipo: Tipo) -> Option<Clase> {
    match tipo {
        Tipo::Correo => Some(Clase::Correo),
        Tipo::Telefono => Some(Clase::Telefono),
        Tipo::Documento => Some(Clase::Documento),
        Tipo::Nombre => Some(Clase::Nombre),
        Tipo::Otro => Some(Clase::Otro),
        Tipo::Senuelo => None,
    }
}

/// The engine's resource context (`COREWEBVIEW2_WEB_RESOURCE_CONTEXT`) as a [`Recurso`].
#[must_use]
pub const fn recurso_de_contexto(contexto: i32) -> Recurso {
    match contexto {
        1 => Recurso::Documento,
        2 => Recurso::Estilo,
        3 => Recurso::Imagen,
        4 | 9 => Recurso::Media,
        5 => Recurso::Fuente,
        6 => Recurso::Script,
        7 | 8 | 10 | 12 => Recurso::Datos,
        11 => Recurso::Socket,
        14 | 15 => Recurso::Aviso,
        _ => Recurso::Otro,
    }
}

/// Engine switches that keep the engine itself quiet: no background connections of its own, no
/// pings for link auditing, and WebRTC never revealing the local network address.
pub const ARGUMENTOS_MOTOR: &str = "--disable-background-networking --disable-domain-reliability --disable-component-update --disable-sync --no-pings --force-webrtc-ip-handling-policy=default_public_interface_only";

impl Sesion {
    /// Open the session: read what was kept and open the first tab.
    #[must_use]
    pub fn abre(a: Arranque) -> (Self, Vec<Orden>) {
        let prefs: Preferencias = lee(&a.datos.join("preferencias.json"));
        let tinta: Vec<Marcado> = lee(&a.datos.join("tinta.json"));
        let libro: Libro = lee(&a.datos.join("libro.json"));
        let favoritos: Favoritos = lee(&a.datos.join("favoritos.json"));
        let mut diario: Diario = lee(&a.datos.join("diario.json"));
        while diario.dias.len() > 400 {
            diario.dias.pop_first();
        }
        let idioma = if prefs.idioma.is_empty() {
            Idioma::de_etiqueta(&a.idioma_sistema)
        } else {
            Idioma::de_etiqueta(&prefs.idioma)
        };
        let mut s = Self {
            datos: a.datos,
            descargas: a.descargas,
            version: a.version,
            motor: String::new(),
            idioma_sistema: a.idioma_sistema,
            reloj: a.reloj,
            zona: 0,
            textos: Textos::de(idioma),
            formas: tinta::formas(&tinta),
            formas_mandato: Vec::new(),
            tinta,
            prefs,
            libro,
            diario,
            favoritos,
            pestanas: Vec::new(),
            activa: 0,
            siguiente: 1,
            panel: None,
            mandato: None,
            terminado: None,
            recibo: None,
            tachon: Vec::new(),
            preguntas: BTreeMap::new(),
            siguiente_pregunta: 1,
            pulsos: Vec::new(),
            sucios: BTreeSet::new(),
            diario_sucio: false,
            libro_sucio: false,
            mandato_sucio: false,
            hoy_sucio: false,
            ultimo_guardado: 0,
            ultimo_hoy: 0,
            dia_visto: String::new(),
            buscando_version: false,
            depura: false,
            trabajadores: false,
            cortes_pend: Vec::new(),
            licencia: EstadoLicencia::Desconocido,
            lugar: None,
            licencia_ocupada: false,
            licencia_error: None,
            licencia_leida: 0,
            licencia_pedida: 0,
        };
        s.dia_visto = s.hoy();
        s.poda_cortes();
        s.ultimo_guardado = (s.reloj)();
        let o = s.nueva_pestana(Perfil::General, INICIO);
        (s, o)
    }

    /// The engine's version, once the shell knows it.
    pub fn pon_motor(&mut self, version: &str) {
        self.motor = version.to_string();
    }

    /// Whether the engine's request filter covers workers (said in the receipt's coverage).
    pub fn pon_trabajadores(&mut self, si: bool) {
        self.trabajadores = si;
    }

    /// Explain each decision in `Respuesta::nota` (for the automatic tests' log).
    pub fn pon_depuracion(&mut self, si: bool) {
        self.depura = si;
    }

    /// The script every web page gets first (see [`paginas::guion_paginas`]).
    #[must_use]
    pub fn guion_paginas(&self) -> String {
        paginas::guion_paginas()
    }

    /// Whether the engine's tracking prevention should be strict.
    #[must_use]
    pub const fn seguimiento_estricto(&self) -> bool {
        self.prefs.reglas.cortar_seguimiento && self.licencia.protege()
    }

    /// Whether the browser does its own work now: the trial or a subscription is on (or not known
    /// yet). Without either it still opens pages, and does nothing else (decided 9 Oct 2026).
    #[must_use]
    pub const fn protege(&self) -> bool {
        self.licencia.protege()
    }

    /// Where the subscription lives. Reads it at once (the first run ever starts the seven days).
    pub fn pon_lugar_licencia(&mut self, lugar: LugarLicencia) -> Vec<Orden> {
        let ahora = (self.reloj)();
        let estado = lugar.estado(ahora);
        self.lugar = Some(lugar);
        self.licencia_leida = ahora;
        estado.map(|e| self.pon_licencia(e)).unwrap_or_default()
    }

    /// The subscription's state, read again or after a check. When protection goes off, an
    /// open mandate ends (its sites are no longer enforced) and the engine's own tracking
    /// prevention goes back to its default.
    pub fn pon_licencia(&mut self, e: EstadoLicencia) -> Vec<Orden> {
        if e == self.licencia {
            return Vec::new();
        }
        let antes = self.licencia.protege();
        self.licencia = e;
        let mut o = Vec::new();
        if antes != self.licencia.protege() {
            if !self.licencia.protege() && self.mandato.is_some() {
                o.extend(self.cierra_mandato());
            }
            o.push(Orden::Seguimiento {
                estricto: self.seguimiento_estricto(),
            });
            o.extend(self.escudo_activa(true));
            o.extend(self.hoy_a_todos());
        }
        o.extend(self.avisa_licencia());
        o
    }

    /// The result of an activation the shell did ([`Orden::ActivaLicencia`]).
    pub fn licencia_activada(&mut self, r: Result<EstadoLicencia, Fallo>) -> Vec<Orden> {
        self.licencia_ocupada = false;
        match r {
            Ok(e) => {
                self.licencia_error = None;
                let mut o = self.pon_licencia(e);
                o.extend(self.avisa_licencia());
                o.push(self.aviso("licencia_activada", &[]));
                o
            }
            Err(f) => {
                let (clave, motivo) = f.clave();
                self.licencia_error = Some(self.tf(clave, &[("motivo", motivo)]));
                self.avisa_licencia()
            }
        }
    }

    fn avisa_licencia(&self) -> Vec<Orden> {
        let m = self.msg_licencia();
        vec![envia(Origen::Barra, &m), envia(Origen::Panel, &m)]
    }

    fn msg_licencia(&self) -> Value {
        let mut m = json!({
            "tipo": "licencia",
            "estado": self.licencia.nombre(),
            "protege": self.licencia.protege(),
            "ocupada": self.licencia_ocupada,
            "error": self.licencia_error,
            "comprar": self.t("licencia_comprar_url"),
        });
        match &self.licencia {
            EstadoLicencia::Desconocido => {}
            EstadoLicencia::Prueba { termina, dias } => {
                m["termina"] = json!(termina);
                m["dias"] = json!(dias);
            }
            EstadoLicencia::Suscrita {
                desde,
                periodo,
                proxima,
                caduca,
                fallida,
            } => {
                m["desde"] = json!(desde);
                m["periodo"] = json!(periodo);
                m["proxima"] = json!(proxima);
                m["caduca"] = json!(caduca);
                m["fallida"] = json!(fallida);
            }
            EstadoLicencia::PruebaTerminada { desde } => m["desde"] = json!(desde),
            EstadoLicencia::SuscripcionTerminada { desde, motivo } => {
                m["desde"] = json!(desde);
                m["motivo"] = json!(motivo);
            }
        }
        if let Some(l) = &self.lugar {
            let (datos, ancla) = l.donde();
            m["donde_datos"] = json!(datos);
            m["donde_ancla"] = json!(ancla);
            m["conexiones"] = Value::Array(
                l.conexiones()
                    .into_iter()
                    .map(|c| json!({ "ms": c.ms, "host": c.host, "pedida": c.pedida, "version": c.version }))
                    .collect(),
            );
        }
        m
    }

    /// The session a tab uses, if it exists.
    #[must_use]
    pub fn perfil_de(&self, id: u32) -> Option<Perfil> {
        self.pestana(id).map(|p| p.perfil)
    }

    /// The active tab.
    #[must_use]
    pub const fn activa(&self) -> u32 {
        self.activa
    }

    /// Whether the side panel is open.
    #[must_use]
    pub const fn panel_abierto(&self) -> bool {
        self.panel.is_some()
    }

    /// Whether requests of this tab need their body read (marked data or a mandate's decoy
    /// might travel in it).
    #[must_use]
    pub fn necesita_cuerpo(&self, id: u32) -> bool {
        !self.tinta.is_empty() || self.perfil_de(id) == Some(Perfil::Mandato)
    }

    fn t(&self, clave: &str) -> String {
        self.textos.t(clave)
    }

    fn tf(&self, clave: &str, huecos: &[(&str, &str)]) -> String {
        let mut s = self.t(clave);
        for (k, v) in huecos {
            s = s.replace(&format!("{{{k}}}"), v);
        }
        s
    }

    fn pestana(&self, id: u32) -> Option<&Pestana> {
        self.pestanas.iter().find(|p| p.id == id)
    }

    fn pestana_mut(&mut self, id: u32) -> Option<&mut Pestana> {
        self.pestanas.iter_mut().find(|p| p.id == id)
    }

    fn hoy(&self) -> String {
        fecha::dia((self.reloj)(), self.zona)
    }

    /// The cut log keeps today and the 30 days before it: the same span «Todo» shows.
    /// The list of cuts and the webs of each day are kept 31 days; the day's totals stay.
    fn poda_cortes(&mut self) {
        let dias = i64::try_from(cortes::DIAS).unwrap_or(31) - 1;
        let desde = fecha::dia_antes((self.reloj)(), self.zona, dias);
        cortes::poda(&self.datos.join("cortes"), &desde);
        if self.diario.olvida_webs(&desde) {
            self.diario_sucio = true;
        }
    }

    fn guarda_prefs(&self) {
        escribe_json(&self.datos.join("preferencias.json"), &self.prefs);
    }

    fn guarda_tinta(&self) {
        escribe_json(&self.datos.join("tinta.json"), &self.tinta);
    }

    fn guarda_favoritos(&self) {
        escribe_json(&self.datos.join("favoritos.json"), &self.favoritos);
    }

    /// The favourites for the new tab, and which other browsers' favourites can be brought over.
    fn msg_favoritos(&self) -> Value {
        json!({
            "tipo": "favoritos",
            "lista": self.favoritos.lista,
            "importar": favoritos::otros_navegadores().into_iter().map(|(n, _)| n).collect::<Vec<_>>(),
        })
    }

    /// The star in the address bar: keep the page of the active tab, or forget it.
    fn favorito_activa(&mut self) -> Vec<Orden> {
        let Some(p) = self.pestana(self.activa) else {
            return Vec::new();
        };
        if es_interna(&p.url) || p.url.is_empty() {
            return Vec::new();
        }
        let (url, titulo, icono) = (p.url.clone(), self.titulo_de(p), p.icono.clone());
        if self.favoritos.contiene(&url) {
            self.favoritos.quita(&url);
        } else {
            self.favoritos.anade(&url, &titulo, &icono);
        }
        self.guarda_favoritos();
        // The star itself says it (filled or not); the new tab gets the list.
        let mut o = vec![self.estado()];
        o.extend(self.a_paginas_propias(&self.msg_favoritos()));
        o
    }

    fn favorito_quita(&mut self, url: &str) -> Vec<Orden> {
        if !self.favoritos.quita(url) {
            return Vec::new();
        }
        self.guarda_favoritos();
        let mut o = vec![self.estado()];
        o.extend(self.a_paginas_propias(&self.msg_favoritos()));
        o
    }

    /// Bring the favourites of another browser on this computer, when the person asks.
    fn importa_favoritos(&mut self, de: &str) -> Vec<Orden> {
        let Some((nombre, ruta)) = favoritos::otros_navegadores()
            .into_iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(de))
        else {
            return vec![self.aviso("favoritos_no_hay", &[])];
        };
        let lista = fs::read_to_string(&ruta)
            .map(|t| favoritos::de_chromium(&t))
            .unwrap_or_default();
        let n = self.favoritos.importa(&lista);
        self.guarda_favoritos();
        let n_txt = n.to_string();
        let mut o = vec![
            self.estado(),
            self.aviso(
                "favoritos_importados",
                &[("n", n_txt.as_str()), ("navegador", nombre)],
            ),
        ];
        o.extend(self.a_paginas_propias(&self.msg_favoritos()));
        o.extend(self.a_paginas_propias(&json!({
            "tipo": "favoritos_importados",
            "texto": self.tf("favoritos_importados", &[("n", n_txt.as_str()), ("navegador", nombre)]),
        })));
        o
    }

    fn guarda_libro(&mut self) {
        escribe_json(&self.datos.join("libro.json"), &self.libro);
        self.libro_sucio = false;
    }

    fn guarda_diario(&mut self) {
        escribe_json(&self.datos.join("diario.json"), &self.diario);
        self.diario_sucio = false;
    }

    fn guarda_cortes(&mut self) {
        if self.cortes_pend.is_empty() {
            return;
        }
        let pend = std::mem::take(&mut self.cortes_pend);
        // A batch can cross midnight: each cut goes to its own day.
        let mut por_dia: BTreeMap<String, Vec<Corte>> = BTreeMap::new();
        for c in pend {
            por_dia
                .entry(fecha::dia(c.ts, self.zona))
                .or_default()
                .push(c);
        }
        for (dia, l) in por_dia {
            let _ = cortes::anota(&self.datos.join("cortes"), &dia, &l);
        }
    }

    fn rehace_formas(&mut self) {
        self.formas = tinta::formas(&self.tinta);
        self.formas_mandato = match &self.mandato {
            Some(m) => {
                let mut lista = self.tinta.clone();
                lista.push(Marcado {
                    tipo: Tipo::Senuelo,
                    valor: m.senuelo.clone(),
                });
                tinta::formas(&lista)
            }
            None => Vec::new(),
        };
    }

    // --- what the browser's pages are told ------------------------------------------------------

    fn titulo_de(&self, p: &Pestana) -> String {
        if p.url == INICIO || p.url.is_empty() {
            return self.t("pestana_nueva");
        }
        if !p.titulo.trim().is_empty() {
            return p.titulo.trim().to_string();
        }
        host_de(&p.url).unwrap_or_else(|| p.url.clone())
    }

    fn msg_estado(&self) -> Value {
        let pestanas: Vec<Value> = self
            .pestanas
            .iter()
            .map(|p| {
                json!({
                    "id": p.id,
                    "titulo": self.titulo_de(p),
                    "url": if es_interna(&p.url) { "" } else { p.url.as_str() },
                    "activa": p.id == self.activa,
                    "cargando": p.cargando,
                    "aislada": p.perfil == Perfil::Aislada,
                    "mandato": p.perfil == Perfil::Mandato,
                    "icono": if es_interna(&p.url) { "" } else { p.icono.as_str() },
                })
            })
            .collect();
        let activa = self.pestana(self.activa).map(|p| {
            json!({
                "id": p.id,
                "url": p.url,
                "puede_atras": p.atras,
                "puede_adelante": p.adelante,
                "cargando": p.cargando,
                "segura": p.url.starts_with("https://"),
                "interna": es_interna(&p.url),
                "mandato": p.perfil == Perfil::Mandato,
                "favorito": self.favoritos.contiene(&p.url),
            })
        });
        json!({ "tipo": "estado", "pestanas": pestanas, "activa": activa, "panel": self.panel })
    }

    fn estado(&self) -> Orden {
        envia(Origen::Barra, &self.msg_estado())
    }

    fn titulo_ventana(&self) -> Orden {
        let texto = match self.pestana(self.activa) {
            Some(p) if !es_interna(&p.url) && !p.url.is_empty() => {
                format!("{} — {}", self.titulo_de(p), self.t("app_nombre"))
            }
            _ => self.t("app_nombre"),
        };
        Orden::Titulo { texto }
    }

    /// Whether tab `id` is an isolated one (nothing of it is kept).
    fn es_aislada(&self, id: u32) -> bool {
        self.pestana(id)
            .is_some_and(|p| p.perfil == Perfil::Aislada)
    }

    /// Whether a site is cut now, and by which rule: what the shield's button must offer.
    /// Without the trial or a subscription nothing is cut, and the shield says so.
    fn estado_sitio(&self, t: &Tercero) -> (&'static str, Option<&'static str>) {
        if !self.licencia.protege() {
            return ("pasa", None);
        }
        let r = &self.prefs.reglas;
        if r.cortados.contains(&t.sitio) {
            return ("cortado", Some("tuya"));
        }
        if r.permitidos.contains(&t.sitio) {
            return ("pasa", Some("permitido"));
        }
        if self.lo_corta_la_lista(t) {
            return ("cortado", Some("lista"));
        }
        ("pasa", None)
    }

    /// Whether the open lists cut this site with the person's settings (no rule of their own).
    fn lo_corta_la_lista(&self, t: &Tercero) -> bool {
        let r = &self.prefs.reglas;
        let sigue = matches!(t.categoria.as_str(), "rastreador" | "publicidad")
            || t.corredor.is_some()
            || (r.maxima && (t.categoria == "telemetria" || t.balizas > 0));
        r.cortar_seguimiento && sigue
    }

    fn msg_escudo(&self, p: &Pestana) -> Value {
        let terceros: Vec<Value> = p
            .escudo
            .terceros
            .values()
            .map(|t| {
                let (ahora, regla) = self.estado_sitio(t);
                let mut v = serde_json::to_value(t).unwrap_or(Value::Null);
                if let Some(o) = v.as_object_mut() {
                    o.insert("ahora".into(), json!(ahora));
                    o.insert("regla".into(), json!(regla));
                }
                v
            })
            .collect();
        json!({
            "tipo": "escudo",
            "pestana": p.id,
            "sitio": if es_interna(&p.url) { "" } else { p.escudo.sitio.as_str() },
            "cortando": self.prefs.reglas.cortar_seguimiento && self.licencia.protege(),
            "maxima": self.prefs.reglas.cortar_seguimiento && self.prefs.reglas.maxima,
            "protege": self.licencia.protege(),
            "resumen": p.escudo.resumen(),
            "terceros": terceros,
            "parametros_quitados": p.escudo.parametros_quitados,
            "datos_salvados": p.escudo.datos_salvados,
            "cookies": p.escudo.cookies.as_ref().map(|(g, a)| json!({ "gestor": g, "accion": a })),
        })
    }

    fn escudo_activa(&self, tambien_panel: bool) -> Vec<Orden> {
        let Some(p) = self.pestana(self.activa) else {
            return Vec::new();
        };
        let m = self.msg_escudo(p);
        let mut o = vec![envia(Origen::Barra, &m)];
        if tambien_panel || self.panel.as_deref() == Some("escudo") {
            o.push(envia(Origen::Panel, &m));
        }
        o
    }

    /// What the public ledger said about the newest GUARDIANA ZERO.
    pub fn version_encontrada(&mut self, r: Result<String, String>) -> Vec<Orden> {
        self.buscando_version = false;
        let m = match r {
            Ok(v) if crate::licencia::compara_versiones(&v, &self.version).is_gt() => {
                json!({ "tipo": "version", "estado": "nueva", "version": v })
            }
            Ok(_) => json!({ "tipo": "version", "estado": "al_dia", "version": self.version }),
            Err(e) => json!({ "tipo": "version", "estado": "error", "error": e }),
        };
        let mut o = vec![envia(Origen::Panel, &m)];
        if self.panel.as_deref() == Some("licencia") {
            o.push(envia(Origen::Panel, &self.msg_licencia()));
        }
        o
    }

    /// The person's day or night for the window frame: `None` follows Windows.
    #[must_use]
    pub fn tema_oscuro(&self) -> Option<bool> {
        match self.prefs.tema.as_str() {
            "noche" => Some(true),
            "dia" => Some(false),
            _ => None,
        }
    }

    fn msg_tema(&self) -> Value {
        json!({ "tipo": "tema", "tema": self.prefs.tema })
    }

    fn msg_textos(&self) -> Value {
        json!({ "tipo": "textos", "textos": self.textos.todos(), "idioma": self.textos.idioma().codigo() })
    }

    fn msg_ajustes(&self) -> Value {
        let lista = lista_buscadores();
        json!({
            "tipo": "ajustes",
            "cortar_seguimiento": self.prefs.reglas.cortar_seguimiento,
            "buscador": buscador(&self.prefs.buscador).id,
            "buscadores": lista,
            "idioma": self.prefs.idioma,
            "tema": self.prefs.tema,
            "rechazar_cookies": !self.prefs.cookies_sin_tocar,
            "importar": favoritos::otros_navegadores().into_iter().map(|(n, _)| n).collect::<Vec<_>>(),
            "sin_preguntar": self.prefs.sin_preguntar,
            "huella_clave": self.huella_clave(),
        })
    }

    fn msg_acerca(&self) -> Value {
        json!({ "tipo": "acerca", "version": self.version, "motor": self.motor })
    }

    fn msg_tinta(&self) -> Value {
        let lista: Vec<Value> = self
            .tinta
            .iter()
            .map(|m| json!({ "tipo": m.tipo, "visible": enmascara(m.tipo, &m.valor) }))
            .collect();
        json!({ "tipo": "tinta", "lista": lista })
    }

    fn msg_libro(&self) -> Value {
        let entradas: Vec<Value> = self
            .libro
            .recientes()
            .into_iter()
            .take(300)
            .map(|e| {
                json!({
                    "sitio": e.sitio,
                    "empresa": e.empresa,
                    "pais": e.pais,
                    "corredor": e.corredor.as_ref().map(|c| c.0.clone()),
                    "clases": e.clases.iter().map(|c| c.clave().trim_start_matches("dato_")).collect::<Vec<_>>(),
                    "ultima": e.ultima,
                    "veces": e.veces,
                    "carta": e.carta,
                })
            })
            .collect();
        json!({ "tipo": "libro", "entradas": entradas })
    }

    fn cuentas(m: &Mandato) -> Value {
        let (visitas, sitios, cortes, datos) = m.cuentas();
        json!({ "visitas": visitas, "sitios": sitios, "cortes": cortes, "datos": datos })
    }

    fn mandato_visible(m: &Mandato) -> Value {
        let desde = m.pasos.len().saturating_sub(40);
        json!({
            "id": m.id,
            "tarea": m.tarea,
            "permitidos": m.permitidos,
            "estricto": m.estricto,
            "senuelo": m.senuelo,
            "inicio": m.inicio,
            "fin": m.fin,
            "caduca": m.caduca,
            "pasos": &m.pasos[desde..],
        })
    }

    fn msg_mandato(&self) -> Value {
        let sitio_actual = self
            .pestana(self.activa)
            .filter(|p| p.url.starts_with("http"))
            .map(|p| p.escudo.sitio.clone())
            .unwrap_or_default();
        if let Some(m) = &self.mandato {
            return json!({
                "tipo": "mandato", "estado": "activo", "mandato": Self::mandato_visible(m),
                "cuentas": Self::cuentas(m), "sitio_actual": sitio_actual,
            });
        }
        if let (Some(m), Some(r)) = (&self.terminado, &self.recibo) {
            return json!({
                "tipo": "mandato", "estado": "terminado", "mandato": Self::mandato_visible(m),
                "cuentas": Self::cuentas(m), "recibo": r, "sitio_actual": sitio_actual,
                "huella": recibo::huella_de(r),
            });
        }
        json!({ "tipo": "mandato", "estado": "ninguno", "sitio_actual": sitio_actual })
    }

    fn msg_hoy(&self) -> Value {
        let hoy = self.hoy();
        let mes = fecha::primero_del_mes(&hoy);
        json!({
            "tipo": "hoy",
            "cortando": self.prefs.reglas.cortar_seguimiento && self.licencia.protege(),
            "protege": self.licencia.protege(),
            "motor_id": buscador(&self.prefs.buscador).id,
            "motores": lista_buscadores(),
            "hoy": self.diario.total(&hoy, &hoy),
            "mes": self.diario.total(&mes, &hoy),
            "rastro_hoy": self.diario.rastro(&hoy, &hoy, 5),
            "rastro_mes": self.diario.rastro(&mes, &hoy, 5),
        })
    }

    /// A message for a tab, only while it shows one of the browser's own pages.
    fn a_pestana(&self, id: u32, v: &Value) -> Option<Orden> {
        self.pestana(id)
            .filter(|p| es_interna(&p.url))
            .map(|_| envia(Origen::Pestana(id), v))
    }

    /// Today's figures for the browser's own pages and for the side panel, whose shield shows
    /// the whole day above the page's own (the owner, 10 Oct 2026: the summary one click away
    /// from any web page, without opening a new tab).
    fn hoy_a_todos(&self) -> Vec<Orden> {
        let hoy = self.msg_hoy();
        let mut o = self.a_paginas_propias(&hoy);
        o.push(envia(Origen::Panel, &hoy));
        o
    }

    fn a_paginas_propias(&self, v: &Value) -> Vec<Orden> {
        self.pestanas
            .iter()
            .filter(|p| p.lista && es_interna(&p.url))
            .map(|p| envia(Origen::Pestana(p.id), v))
            .collect()
    }

    fn aviso(&self, clave: &str, huecos: &[(&str, &str)]) -> Orden {
        envia(
            Origen::Panel,
            &json!({ "tipo": "aviso", "texto": self.tf(clave, huecos) }),
        )
    }

    // --- tabs and panel ------------------------------------------------------------------------

    fn nueva_pestana(&mut self, perfil: Perfil, url: &str) -> Vec<Orden> {
        let id = self.siguiente;
        self.siguiente += 1;
        self.pestanas.push(Pestana::nueva(id, perfil, url));
        let mut o = vec![Orden::CreaPestana {
            id,
            perfil,
            url: Some(url.to_string()),
            de: None,
        }];
        o.extend(self.activar(id));
        o
    }

    /// The list of cuts, in a tab of its own that remembers where the person was. Already on
    /// it: it stays, no second copy.
    fn abre_cortes(&mut self) -> Vec<Orden> {
        let de = self.activa;
        if self
            .pestana(de)
            .is_some_and(|p| sin_fragmento(&p.url) == CORTES)
        {
            return self.activar(de);
        }
        let o = self.nueva_pestana(Perfil::General, CORTES);
        let nueva = self.activa;
        let vuelve = self.pestana(de).map(|q| q.id);
        if let Some(p) = self.pestana_mut(nueva) {
            p.vuelve = vuelve;
        }
        o
    }

    /// «Back» on the browser's own pages: the page before, if there is one; otherwise the tab
    /// it was opened from (this one closes); otherwise the new tab page.
    fn volver(&mut self, id: u32) -> Vec<Orden> {
        let Some(p) = self.pestana(id) else {
            return Vec::new();
        };
        if p.atras {
            return vec![Orden::Atras { id }];
        }
        match p.vuelve.filter(|v| self.pestana(*v).is_some()) {
            Some(v) => {
                self.activa = v;
                let mut o = self.cerrar(id);
                o.extend(self.activar(v));
                o
            }
            None => vec![Orden::Navega {
                id,
                url: INICIO.to_string(),
            }],
        }
    }

    fn activar(&mut self, id: u32) -> Vec<Orden> {
        if self.pestana(id).is_none() {
            return Vec::new();
        }
        self.activa = id;
        let mut o = vec![Orden::Activa { id }, self.estado(), self.titulo_ventana()];
        o.extend(self.escudo_activa(false));
        if self.panel.as_deref() == Some("mandato") {
            o.push(envia(Origen::Panel, &self.msg_mandato()));
        }
        o
    }

    fn cerrar(&mut self, id: u32) -> Vec<Orden> {
        let Some(i) = self.pestanas.iter().position(|p| p.id == id) else {
            return Vec::new();
        };
        let p = self.pestanas.remove(i);
        self.preguntas.retain(|_, q| q.pestana != id);
        let mut o = vec![Orden::CierraPestana { id }];
        if p.perfil == Perfil::Mandato
            && self.mandato.is_some()
            && !self.pestanas.iter().any(|q| q.perfil == Perfil::Mandato)
        {
            o.extend(self.termina_mandato());
        }
        if self.pestanas.is_empty() {
            o.push(Orden::CierraVentana);
            return o;
        }
        if self.activa == id || self.pestana(self.activa).is_none() {
            let j = i.min(self.pestanas.len() - 1);
            let nueva = self.pestanas[j].id;
            o.extend(self.activar(nueva));
        } else {
            o.push(self.estado());
        }
        o
    }

    fn abre_panel(&mut self, vista: &str) -> Vec<Orden> {
        let vista = match vista {
            // Mandates and redaction are the subscription's: without it, the panel says so.
            "mandato" | "tachon" if !self.licencia.protege() => "licencia",
            "escudo" | "mandato" | "tachon" | "datos" | "ajustes" | "formulario" | "bienvenida"
            | "licencia" => vista,
            _ => "escudo",
        };
        // Leaving a question unanswered is answering «no»: the form stays where it was.
        if self.panel.as_deref() == Some("formulario") && vista != "formulario" {
            self.preguntas.clear();
        }
        self.panel = Some(vista.to_string());
        let mut o = vec![
            Orden::Panel { abierto: true },
            envia(Origen::Panel, &json!({ "tipo": "vista", "vista": vista })),
        ];
        match vista {
            "escudo" => {
                o.extend(self.escudo_activa(true));
                o.push(envia(Origen::Panel, &self.msg_hoy()));
            }
            "mandato" => o.push(envia(Origen::Panel, &self.msg_mandato())),
            "datos" => {
                o.push(envia(Origen::Panel, &self.msg_tinta()));
                o.push(envia(Origen::Panel, &self.msg_libro()));
            }
            "ajustes" => {
                o.push(envia(Origen::Panel, &self.msg_ajustes()));
                o.push(envia(Origen::Panel, &self.msg_acerca()));
                o.push(envia(Origen::Panel, &self.msg_licencia()));
            }
            "licencia" => o.push(envia(Origen::Panel, &self.msg_licencia())),
            _ => {}
        }
        o.push(self.estado());
        o
    }

    /// Whether the shield stays open beside the pages: unless the person closed it, and only
    /// while there is protection to show.
    fn escudo_a_la_vista(&self) -> bool {
        !self.prefs.escudo_cerrado && self.prefs.bienvenida && self.licencia.protege()
    }

    /// Done with a view (a question answered, settings closed): back to the shield while the
    /// person keeps it open, else the panel closes.
    fn cierra_panel(&mut self) -> Vec<Orden> {
        if self.panel.as_deref() == Some("formulario") {
            self.preguntas.clear();
        }
        if self.escudo_a_la_vista() && self.panel.as_deref() != Some("escudo") {
            return self.abre_panel("escudo");
        }
        self.panel = None;
        self.cierra_del_todo()
    }

    fn cierra_del_todo(&self) -> Vec<Orden> {
        vec![
            Orden::Panel { abierto: false },
            self.estado(),
            Orden::Foco {
                a: Origen::Pestana(self.activa),
            },
        ]
    }

    // --- messages from the browser's own pages and the form guard -------------------------------

    /// A message posted by a view. `fuente` is the address of the document that posted it, as
    /// the engine says (not as the message claims).
    pub fn mensaje(&mut self, origen: Origen, fuente: &str, json: &str) -> Vec<Orden> {
        let Ok(m) = serde_json::from_str::<Value>(json) else {
            return Vec::new();
        };
        let tipo = texto(&m, "tipo").to_string();
        let propia = es_interna(fuente);
        match origen {
            Origen::Pestana(id) if !propia => {
                return match tipo.as_str() {
                    "zg_formulario" => self.formulario(id, fuente, &m),
                    "zg_cookies_pide" => self.cookies_pide(id, &m),
                    "zg_cookies" => self.cookies_hecho(id, &m),
                    _ => Vec::new(),
                };
            }
            Origen::Pestana(_) => {
                // The new tab may only do what its own buttons do.
                let permitido = matches!(
                    tipo.as_str(),
                    "listo"
                        | "navegar"
                        | "panel"
                        | "guardar_imagen"
                        | "cortes"
                        | "exportar_cortes"
                        | "volver"
                        | "favorito_quitar"
                        | "importar_favoritos"
                ) || (tipo == "ajuste"
                    && ((texto(&m, "clave") == "cortar_seguimiento" && si(&m, "valor"))
                        || matches!(texto(&m, "clave"), "buscador" | "tema")));
                if !permitido {
                    return Vec::new();
                }
            }
            Origen::Barra | Origen::Panel => {
                if !propia {
                    return Vec::new();
                }
            }
        }
        self.orden_de_pagina(origen, &tipo, &m)
    }

    fn orden_de_pagina(&mut self, origen: Origen, tipo: &str, m: &Value) -> Vec<Orden> {
        let activa = self.activa;
        let de_pago = matches!(
            tipo,
            "mandato_empezar"
                | "mandato_anadir"
                | "tachar"
                | "tachon_pegar"
                | "tinta_anadir"
                | "carta"
        );
        if de_pago && !self.licencia.protege() {
            let mut o = self.abre_panel("licencia");
            o.push(self.aviso("licencia_hace_falta", &[]));
            return o;
        }
        match tipo {
            "licencia_activar" => {
                if self.licencia_ocupada {
                    return Vec::new();
                }
                let clave: String = texto(m, "clave").trim().chars().take(200).collect();
                if clave.is_empty() {
                    self.licencia_error = Some(self.t("licencia_err_vacia"));
                    return self.avisa_licencia();
                }
                if self.lugar.is_none() {
                    self.licencia_error = Some(self.t("licencia_err_disco"));
                    return self.avisa_licencia();
                }
                self.licencia_ocupada = true;
                self.licencia_error = None;
                let mut o = self.avisa_licencia();
                o.push(Orden::ActivaLicencia { clave });
                o
            }
            "licencia_comprar" => {
                let url = self.t("licencia_comprar_url");
                self.nueva_pestana(Perfil::General, &url)
            }
            "listo" => self.listo(origen, m),
            "navegar" => {
                let id = match origen {
                    Origen::Pestana(id) => id,
                    _ => activa,
                };
                let url = a_direccion(texto(m, "texto"), buscador(&self.prefs.buscador));
                if let Some(p) = self.pestana_mut(id) {
                    p.escrita = Some(sin_fragmento(&url).to_string());
                }
                vec![
                    Orden::Navega { id, url },
                    Orden::Foco {
                        a: Origen::Pestana(id),
                    },
                ]
            }
            "atras" => vec![Orden::Atras { id: activa }],
            "adelante" => vec![Orden::Adelante { id: activa }],
            "recargar" => vec![Orden::Recarga { id: activa }],
            "detener" => vec![Orden::Detiene { id: activa }],
            "pestana_nueva" => self.pestana_nueva(Perfil::General),
            "pestana_aislada" => self.pestana_nueva(Perfil::Aislada),
            "pestana_cerrar" => numero(m, "id")
                .and_then(|n| u32::try_from(n).ok())
                .map(|id| self.cerrar(id))
                .unwrap_or_default(),
            "pestana_activar" => numero(m, "id")
                .and_then(|n| u32::try_from(n).ok())
                .map(|id| self.activar(id))
                .unwrap_or_default(),
            "panel" => match m.get("vista").and_then(Value::as_str) {
                Some(v) => {
                    if v == "escudo" && self.prefs.escudo_cerrado {
                        self.prefs.escudo_cerrado = false;
                        self.guarda_prefs();
                    }
                    self.abre_panel(v)
                }
                // The person closing the shield: it stays closed, now and at the next start.
                None if self.panel.as_deref() == Some("escudo") => {
                    self.prefs.escudo_cerrado = true;
                    self.guarda_prefs();
                    self.panel = None;
                    self.cierra_del_todo()
                }
                None => self.cierra_panel(),
            },
            "ajuste" => self.ajuste(texto(m, "clave"), m.get("valor").unwrap_or(&Value::Null)),
            "cortes" => match origen {
                Origen::Pestana(id) => self.cortes(id, texto(m, "periodo")),
                _ => Vec::new(),
            },
            "exportar_cortes" => match origen {
                Origen::Pestana(id) => {
                    self.exporta_cortes(id, texto(m, "periodo"), texto(m, "formato"))
                }
                _ => Vec::new(),
            },
            "abrir_cortes" => self.abre_cortes(),
            "buscar_version" => {
                if self.buscando_version {
                    return Vec::new();
                }
                self.buscando_version = true;
                vec![
                    Orden::BuscaVersion,
                    envia(
                        Origen::Panel,
                        &json!({ "tipo": "version", "estado": "buscando" }),
                    ),
                ]
            }
            "descargar_version" => {
                let url = self.t("zero_pagina_url");
                self.nueva_pestana(Perfil::General, &url)
            }
            "favorito" => self.favorito_activa(),
            "favorito_quitar" => self.favorito_quita(texto(m, "url")),
            "importar_favoritos" => self.importa_favoritos(texto(m, "de")),
            "volver" => match origen {
                Origen::Pestana(id) => self.volver(id),
                _ => Vec::new(),
            },
            "preguntar_otra_vez" => {
                let sitio_q = texto(m, "sitio").to_string();
                if self.prefs.sin_preguntar.remove(&sitio_q) {
                    self.guarda_prefs();
                }
                vec![envia(Origen::Panel, &self.msg_ajustes())]
            }
            "bloquear_sitio" | "desbloquear_sitio" => self.regla(tipo, texto(m, "sitio")),
            "tinta_anadir" => self.tinta_anadir(texto(m, "dato"), texto(m, "valor")),
            "tinta_quitar" => self.tinta_quitar(numero(m, "indice")),
            "carta" => self.carta(texto(m, "sitio"), texto(m, "ley")),
            "carta_hecha" => {
                let ahora = (self.reloj)();
                if let Some(e) = self.libro.entradas.get_mut(texto(m, "sitio")) {
                    e.carta = Some(ahora);
                    self.guarda_libro();
                }
                Vec::new()
            }
            "abrir_correo" => {
                let url = format!(
                    "mailto:?subject={}&body={}",
                    codifica(texto(m, "asunto")),
                    codifica(texto(m, "cuerpo"))
                );
                vec![Orden::AbreFuera { url }]
            }
            "mandato_empezar" => self.mandato_empezar(m),
            "mandato_previa" => {
                let webs: Vec<String> = m
                    .get("webs")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .take(80)
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default();
                vec![envia(
                    Origen::Panel,
                    &json!({ "tipo": "mandato_previa", "permitidos": crate::mandato::sitios_de(&webs) }),
                )]
            }
            "mandato_terminar" => self.cierra_mandato(),
            "mandato_nuevo" => {
                self.terminado = None;
                self.recibo = None;
                vec![envia(Origen::Panel, &self.msg_mandato())]
            }
            "mandato_anadir" => self.mandato_anadir(texto(m, "sitio")),
            "recibo_guardar" => self.recibo_guardar(),
            "tachar" => self.tachar(texto(m, "texto")),
            "tachon_pegar" => self.tachon_pegar(texto(m, "texto")),
            "tachon_respuesta" => {
                if self.pestana(activa).is_some_and(|p| !es_interna(&p.url)) {
                    vec![Orden::Ejecuta {
                        id: activa,
                        script:
                            "(() => String(window.getSelection ? window.getSelection() : ''))()"
                                .into(),
                        etiqueta: Some(Etiqueta::Seleccion),
                    }]
                } else {
                    vec![envia(
                        Origen::Panel,
                        &json!({ "tipo": "restaurado", "texto": "" }),
                    )]
                }
            }
            "formulario_respuesta" => self.formulario_respuesta(
                numero(m, "id").unwrap_or(0),
                si(m, "enviar"),
                si(m, "recordar"),
            ),
            "bienvenida" => {
                self.prefs.reglas.cortar_seguimiento = si(m, "cortar");
                self.prefs.bienvenida = true;
                self.guarda_prefs();
                let mut o = vec![Orden::Seguimiento {
                    estricto: self.seguimiento_estricto(),
                }];
                o.extend(self.cierra_panel());
                o.extend(self.escudo_activa(false));
                o.extend(self.hoy_a_todos());
                o
            }
            "borrar_todo" => self.borrar_todo(),
            "guardar_imagen" => self.guardar_imagen(origen, texto(m, "datos")),
            _ => Vec::new(),
        }
    }

    fn pestana_nueva(&mut self, perfil: Perfil) -> Vec<Orden> {
        let mut o = self.nueva_pestana(perfil, INICIO);
        o.push(Orden::Foco { a: Origen::Barra });
        o.push(envia(Origen::Barra, &json!({ "tipo": "foco_direccion" })));
        o
    }

    fn listo(&mut self, origen: Origen, m: &Value) -> Vec<Orden> {
        if let Some(z) = m.get("zona").and_then(Value::as_i64) {
            self.zona = i32::try_from(z.clamp(-900, 900)).unwrap_or(0);
        }
        match origen {
            Origen::Barra => {
                let mut o = vec![
                    envia(Origen::Barra, &self.msg_tema()),
                    envia(Origen::Barra, &self.msg_textos()),
                    self.estado(),
                    self.titulo_ventana(),
                ];
                o.extend(self.escudo_activa(false));
                o.push(envia(Origen::Barra, &self.msg_licencia()));
                if !self.prefs.bienvenida {
                    o.extend(self.abre_panel("bienvenida"));
                } else if self.panel.is_none() && self.escudo_a_la_vista() {
                    o.extend(self.abre_panel("escudo"));
                }
                o
            }
            Origen::Panel => {
                let mut o = vec![
                    envia(Origen::Panel, &self.msg_tema()),
                    envia(Origen::Panel, &self.msg_textos()),
                    envia(Origen::Panel, &self.msg_ajustes()),
                    envia(Origen::Panel, &self.msg_hoy()),
                    envia(Origen::Panel, &self.msg_acerca()),
                    envia(Origen::Panel, &self.msg_tinta()),
                    envia(Origen::Panel, &self.msg_libro()),
                    envia(Origen::Panel, &self.msg_mandato()),
                    envia(Origen::Panel, &self.msg_licencia()),
                ];
                if let Some(p) = self.pestana(self.activa) {
                    o.push(envia(Origen::Panel, &self.msg_escudo(p)));
                }
                if let Some(v) = &self.panel {
                    o.push(envia(
                        Origen::Panel,
                        &json!({ "tipo": "vista", "vista": v }),
                    ));
                }
                o
            }
            Origen::Pestana(id) => {
                if let Some(p) = self.pestana_mut(id) {
                    p.lista = true;
                }
                [
                    self.a_pestana(id, &self.msg_tema()),
                    self.a_pestana(id, &self.msg_textos()),
                    self.a_pestana(id, &self.msg_hoy()),
                    self.a_pestana(id, &self.msg_favoritos()),
                ]
                .into_iter()
                .flatten()
                .collect()
            }
        }
    }

    fn ajuste(&mut self, clave: &str, valor: &Value) -> Vec<Orden> {
        match clave {
            "cortar_seguimiento" | "maxima" => {
                let si = valor.as_bool().unwrap_or(false);
                let r = &mut self.prefs.reglas;
                if clave == "maxima" {
                    // Maximum protection is the cut, plus telemetry and beacons, plus cookie
                    // notices answered «no»: everything the browser knows how to refuse.
                    r.maxima = si;
                    if si {
                        r.cortar_seguimiento = true;
                        self.prefs.cookies_sin_tocar = false;
                    }
                } else {
                    r.cortar_seguimiento = si;
                    if !si {
                        r.maxima = false;
                    }
                }
                self.prefs.bienvenida = true;
                self.guarda_prefs();
                let mut o = vec![
                    Orden::Seguimiento {
                        estricto: self.seguimiento_estricto(),
                    },
                    envia(Origen::Panel, &self.msg_ajustes()),
                ];
                o.extend(self.escudo_activa(false));
                o.extend(self.hoy_a_todos());
                o
            }
            "buscador" => {
                let id = valor.as_str().unwrap_or("");
                if BUSCADORES.iter().any(|b| b.id == id) {
                    self.prefs.buscador = id.to_string();
                    self.guarda_prefs();
                }
                let mut o = vec![envia(Origen::Panel, &self.msg_ajustes())];
                o.extend(self.hoy_a_todos());
                o
            }
            "rechazar_cookies" => {
                self.prefs.cookies_sin_tocar = !valor.as_bool().unwrap_or(true);
                // Maximum protection includes refusing cookie notices: leaving them as they come
                // is no longer maximum, and the shield must not say it is.
                if self.prefs.cookies_sin_tocar {
                    self.prefs.reglas.maxima = false;
                }
                self.guarda_prefs();
                let mut o = vec![envia(Origen::Panel, &self.msg_ajustes())];
                o.extend(self.escudo_activa(false));
                o
            }
            "tema" => {
                let v = valor.as_str().unwrap_or("");
                if !matches!(v, "" | "dia" | "noche") {
                    return Vec::new();
                }
                self.prefs.tema = v.to_string();
                self.guarda_prefs();
                let tema = self.msg_tema();
                let mut o = vec![
                    Orden::Tema {
                        oscuro: self.tema_oscuro(),
                    },
                    envia(Origen::Barra, &tema),
                    envia(Origen::Panel, &tema),
                    envia(Origen::Panel, &self.msg_ajustes()),
                ];
                o.extend(self.a_paginas_propias(&tema));
                o
            }
            "idioma" => {
                let v = valor.as_str().unwrap_or("");
                if !matches!(v, "" | "es" | "en" | "pt") {
                    return Vec::new();
                }
                self.prefs.idioma = v.to_string();
                self.guarda_prefs();
                let etiqueta = if v.is_empty() {
                    self.idioma_sistema.clone()
                } else {
                    v.to_string()
                };
                self.textos = Textos::de(Idioma::de_etiqueta(&etiqueta));
                let textos = self.msg_textos();
                let mut o = vec![
                    envia(Origen::Barra, &textos),
                    envia(Origen::Panel, &textos),
                    self.estado(),
                    self.titulo_ventana(),
                    envia(Origen::Panel, &self.msg_ajustes()),
                    envia(Origen::Panel, &self.msg_acerca()),
                    envia(Origen::Panel, &self.msg_tinta()),
                    envia(Origen::Panel, &self.msg_libro()),
                    envia(Origen::Panel, &self.msg_mandato()),
                ];
                if let Some(vista) = &self.panel {
                    o.push(envia(
                        Origen::Panel,
                        &json!({ "tipo": "vista", "vista": vista }),
                    ));
                }
                o.extend(self.escudo_activa(false));
                o.extend(self.a_paginas_propias(&textos));
                o.extend(self.hoy_a_todos());
                o
            }
            _ => Vec::new(),
        }
    }

    fn regla(&mut self, tipo: &str, s: &str) -> Vec<Orden> {
        let s = sitio(s);
        if s.is_empty() || s.len() > 253 {
            return Vec::new();
        }
        // Both ways, always: «Bloquear» and «Desbloquear» undo each other, and a site goes back
        // to what the lists say for it rather than keeping a rule it no longer needs.
        // The site as the shield saw it; if no tab shows it any more (the page moved on between
        // the click and this message), as the lists see it.
        let visto = self
            .pestanas
            .iter()
            .find_map(|p| p.escudo.terceros.get(&s))
            .cloned();
        let t = visto.unwrap_or_else(|| {
            let d = crate::destino::clasifica(&s);
            Tercero {
                quien: d.quien().to_string(),
                sitio: s.clone(),
                pais: None,
                categoria: d.categoria.to_string(),
                corredor: d.corredor.as_ref().map(|c| c.nombre.to_string()),
                vistas: 0,
                cortadas: 0,
                motivo: None,
                balizas: 0,
            }
        });
        let lista = self.lo_corta_la_lista(&t);
        let r = &mut self.prefs.reglas;
        if tipo == "bloquear_sitio" {
            r.permitidos.remove(&s);
            if !lista {
                r.cortados.insert(s);
            }
        } else {
            r.cortados.remove(&s);
            if lista {
                r.permitidos.insert(s);
            }
        }
        self.guarda_prefs();
        self.escudo_activa(true)
    }

    fn tinta_anadir(&mut self, dato: &str, valor: &str) -> Vec<Orden> {
        let valor = valor.trim();
        let tipo = match dato {
            "correo" => Tipo::Correo,
            "telefono" => Tipo::Telefono,
            "documento" => Tipo::Documento,
            "nombre" => Tipo::Nombre,
            _ => Tipo::Otro,
        };
        let largo = valor.chars().count();
        if !(5..=200).contains(&largo) || self.tinta.len() >= 40 {
            return vec![self.aviso("tinta_no_valido", &[])];
        }
        if !self
            .tinta
            .iter()
            .any(|m| m.valor.eq_ignore_ascii_case(valor))
        {
            self.tinta.push(Marcado {
                tipo,
                valor: valor.to_string(),
            });
            self.rehace_formas();
            self.guarda_tinta();
        }
        vec![envia(Origen::Panel, &self.msg_tinta())]
    }

    fn tinta_quitar(&mut self, indice: Option<u64>) -> Vec<Orden> {
        if let Some(i) = indice.and_then(|i| usize::try_from(i).ok()) {
            if i < self.tinta.len() {
                self.tinta.remove(i);
                self.rehace_formas();
                self.guarda_tinta();
            }
        }
        vec![envia(Origen::Panel, &self.msg_tinta())]
    }

    fn carta(&self, s: &str, ley: &str) -> Vec<Orden> {
        let Some(e) = self.libro.entradas.get(s) else {
            return Vec::new();
        };
        let ley = match ley {
            "co" => Ley::Co,
            "ue" => Ley::Ue,
            "ca" => Ley::Ca,
            "br" => Ley::Br,
            _ => Ley::Otro,
        };
        let primero = |t: Tipo| {
            self.tinta
                .iter()
                .find(|m| m.tipo == t)
                .map(|m| m.valor.clone())
                .unwrap_or_default()
        };
        let c = carta(
            &self.textos,
            ley,
            e,
            &primero(Tipo::Nombre),
            &primero(Tipo::Correo),
            &self.hoy(),
        );
        vec![envia(
            Origen::Panel,
            &json!({ "tipo": "carta", "sitio": s, "asunto": c.asunto, "cuerpo": c.cuerpo }),
        )]
    }

    // --- mandates ------------------------------------------------------------------------------

    fn mandato_empezar(&mut self, m: &Value) -> Vec<Orden> {
        if self.mandato.is_some() {
            return vec![envia(Origen::Panel, &self.msg_mandato())];
        }
        let webs: Vec<String> = m
            .get("webs")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .take(40)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let mut semilla = [0u8; 16];
        azar(&mut semilla);
        let mut id = [0u8; 8];
        azar(&mut id);
        let mut md = Mandato::nuevo(
            hex(&id),
            texto(m, "tarea"),
            &webs,
            tinta::senuelo(&semilla),
            (self.reloj)(),
        );
        if md.permitidos.is_empty() {
            return vec![self.aviso("mandato_webs_falta", &[])];
        }
        md.estricto = si(m, "estricto");
        // A mandate can end on its own: the AI's permission does not outlive the task.
        let minutos = numero(m, "minutos").unwrap_or(0).min(8 * 60);
        if minutos > 0 {
            md.caduca = md.inicio + i64::try_from(minutos).unwrap_or(0) * 60_000;
        }
        let primera = format!("https://{}/", md.permitidos[0]);
        self.mandato = Some(md);
        self.terminado = None;
        self.recibo = None;
        self.rehace_formas();
        let mut o = self.nueva_pestana(Perfil::Mandato, &primera);
        o.push(envia(Origen::Panel, &self.msg_mandato()));
        o
    }

    fn mandato_anadir(&mut self, s: &str) -> Vec<Orden> {
        let s = sitio(s);
        let ahora = (self.reloj)();
        if let Some(md) = self.mandato.as_mut() {
            if !s.is_empty() && !md.permitidos.contains(&s) {
                md.permitidos.push(s.clone());
                md.anota(ahora, "permitido", &s, "");
            }
        }
        vec![envia(Origen::Panel, &self.msg_mandato())]
    }

    fn cobertura_vista(&self) -> Vec<&'static str> {
        let mut v = vec![
            "paginas",
            "marcos",
            "scripts",
            "imagenes",
            "estilos_y_fuentes",
            "peticiones_de_datos",
            "avisos",
            "websockets",
        ];
        if self.trabajadores {
            v.push("trabajadores");
        }
        v
    }

    /// The fingerprint of the key that signs this computer's receipts (none until the first
    /// receipt): what anyone checking a receipt compares.
    fn huella_clave(&self) -> String {
        let Ok(k) = fs::read(self.datos.join("clave-recibos.p8")) else {
            return String::new();
        };
        recibo::huella(&k).unwrap_or_default()
    }

    fn clave_recibos(&self) -> Option<Vec<u8>> {
        let ruta = self.datos.join("clave-recibos.p8");
        if let Ok(b) = fs::read(&ruta) {
            if !b.is_empty() {
                return Some(b);
            }
        }
        let nueva = recibo::clave_nueva().ok()?;
        escribe(&ruta, &nueva).ok()?;
        Some(nueva)
    }

    /// End the mandate: sign what happened and keep the receipt.
    fn termina_mandato(&mut self) -> Vec<Orden> {
        let Some(mut md) = self.mandato.take() else {
            return Vec::new();
        };
        md.fin = (self.reloj)();
        self.rehace_formas();
        // The receipt says what it saw and what it could not see (decision 197).
        let contenido = json!({
            "programa": "GUARDIANA ZERO",
            "version": self.version,
            "mandato": md,
            "cobertura": {
                "observado": self.cobertura_vista(),
                "no_observado": [
                    "otras_aplicaciones_del_equipo",
                    "lo_que_cada_servidor_hace_con_lo_recibido",
                    "otras_pestanas",
                    "conexiones_webrtc_entre_equipos",
                    "anticipacion_dns_del_motor",
                    "cabeceras_de_las_peticiones",
                ],
                "nota": self.t("recibo_cobertura"),
            },
        })
        .to_string();
        let recibo = self
            .clave_recibos()
            .and_then(|k| recibo::firma(&k, contenido).ok());
        if let Some(r) = &recibo {
            let dia = fecha::dia(md.fin, self.zona);
            escribe_json(
                &self
                    .datos
                    .join("recibos")
                    .join(format!("{dia}-{}.json", md.id)),
                r,
            );
        }
        self.recibo = recibo;
        self.terminado = Some(md);
        let mut o = self.abre_panel("mandato");
        if self.recibo.is_none() {
            o.push(self.aviso("recibo_error", &[]));
        }
        o
    }

    /// End the mandate and close its tabs: whatever drove them has no tab left to use.
    fn cierra_mandato(&mut self) -> Vec<Orden> {
        let mut o = self.termina_mandato();
        let ids: Vec<u32> = self
            .pestanas
            .iter()
            .filter(|p| p.perfil == Perfil::Mandato)
            .map(|p| p.id)
            .collect();
        for id in ids {
            o.extend(self.cerrar(id));
        }
        o
    }

    fn nombre_libre(&self, base: &str, ext: &str) -> PathBuf {
        let primera = self.descargas.join(format!("{base}.{ext}"));
        if !primera.exists() {
            return primera;
        }
        (2..1000)
            .map(|n| self.descargas.join(format!("{base} ({n}).{ext}")))
            .find(|p| !p.exists())
            .unwrap_or(primera)
    }

    fn recibo_guardar(&self) -> Vec<Orden> {
        let (Some(r), Some(md)) = (&self.recibo, &self.terminado) else {
            return Vec::new();
        };
        let dia = fecha::dia(md.fin, self.zona);
        let corto: String = md.id.chars().take(8).collect();
        let ruta = self.nombre_libre(&format!("guardiana-zero-recibo-{dia}-{corto}"), "json");
        let Ok(b) = serde_json::to_vec_pretty(r) else {
            return Vec::new();
        };
        match escribe(&ruta, &b) {
            Ok(()) => {
                let ruta = ruta.display().to_string();
                vec![envia(
                    Origen::Panel,
                    &json!({ "tipo": "guardado", "ruta": ruta, "texto": self.tf("guardado_en", &[("ruta", &ruta)]) }),
                )]
            }
            Err(_) => vec![self.aviso("guardar_error", &[])],
        }
    }

    // --- redaction -----------------------------------------------------------------------------

    fn tachar(&mut self, t: &str) -> Vec<Orden> {
        let t: String = t.chars().take(20_000).collect();
        let r = tachon::tacha(&t, &self.tinta, &self.tachon, &self.textos);
        self.tachon = r.sustituciones.clone();
        let usadas: Vec<Value> = r
            .sustituciones
            .iter()
            .filter(|s| r.texto.contains(&s.marca))
            .map(|s| json!({ "marca": s.marca, "clave": s.clase.clave() }))
            .collect();
        vec![envia(
            Origen::Panel,
            &json!({ "tipo": "tachado", "texto": r.texto, "sustituciones": usadas }),
        )]
    }

    fn tachon_pegar(&self, t: &str) -> Vec<Orden> {
        let activa = self.activa;
        if self.pestana(activa).is_none_or(|p| es_interna(&p.url)) {
            return vec![self.aviso("tachon_sin_campo", &[])];
        }
        let literal = serde_json::to_string(t).unwrap_or_else(|_| "\"\"".into());
        let script = format!(
            "(() => {{ const t = {literal}; let e = document.activeElement; \
             while (e && e.shadowRoot && e.shadowRoot.activeElement) e = e.shadowRoot.activeElement; \
             if (!e || !(e.isContentEditable || e.tagName === 'TEXTAREA' || (e.tagName === 'INPUT' && /^(text|search|email|url|tel|)$/.test(e.type)))) return 'sin_campo'; \
             e.focus(); if (!document.execCommand('insertText', false, t)) {{ e.value = (e.value || '') + t; e.dispatchEvent(new Event('input', {{ bubbles: true }})); }} return 'ok'; }})()"
        );
        vec![
            Orden::Foco {
                a: Origen::Pestana(activa),
            },
            Orden::Ejecuta {
                id: activa,
                script,
                etiqueta: Some(Etiqueta::Pegar),
            },
        ]
    }

    /// The days a period covers: `hoy`, `7` (days), `mes` (this month) or `31` (days).
    fn periodo(&self, periodo: &str) -> (String, String) {
        let ahora = (self.reloj)();
        let hoy = fecha::dia(ahora, self.zona);
        let desde = match periodo {
            "7" => fecha::dia_antes(ahora, self.zona, 6),
            "mes" => fecha::primero_del_mes(&hoy),
            "31" => fecha::dia_antes(ahora, self.zona, 30),
            _ => hoy.clone(),
        };
        (desde, hoy)
    }

    /// «Lo que se cortó» for a period: every cut, newest first, and the counts that summarise
    /// them (all of them, even when the list is cut short).
    fn cortes(&mut self, id: u32, periodo: &str) -> Vec<Orden> {
        self.guarda_cortes();
        let (desde, hasta) = self.periodo(periodo);
        let lista = cortes::lee(&self.datos.join("cortes"), &desde, &hasta);
        let total = lista.len();
        let mut paises_de: BTreeMap<&str, &str> = BTreeMap::new();
        for c in &lista {
            if let Some(p) = &c.pais {
                paises_de.entry(c.quien.as_str()).or_insert(p.as_str());
            }
        }
        let cuenta = |v: Vec<(String, usize)>, k: &str| -> Vec<Value> {
            v.into_iter()
                .map(|(a, n)| json!({ k: a, "n": n }))
                .collect()
        };
        let empresas: Vec<Value> = cortes::cuenta(&lista, |c| c.quien.clone())
            .into_iter()
            .map(|(q, n)| json!({ "quien": q, "pais": paises_de.get(q.as_str()), "n": n }))
            .collect();
        let paises = cuenta(
            cortes::cuenta(&lista, |c| c.pais.clone().unwrap_or_default()),
            "pais",
        );
        let motivos = cuenta(
            cortes::cuenta(&lista, |c| {
                c.motivo
                    .map(cortes::motivo_clave)
                    .unwrap_or_default()
                    .to_string()
            }),
            "motivo",
        );
        let tipos = cuenta(
            cortes::cuenta(&lista, |c| cortes::recurso_clave(c.recurso).to_string()),
            "recurso",
        );
        let paginas = cuenta(cortes::cuenta(&lista, |c| c.pagina.clone()), "pagina");
        let recientes: Vec<&Corte> = lista.iter().rev().take(cortes::MAX_LISTA).collect();
        let dia = self.diario.total(&desde, &hasta);
        let sin_anotar = (dia.cortadas as usize).saturating_sub(total);
        let v = json!({
            "tipo": "cortes", "periodo": periodo, "desde": desde, "hasta": hasta,
            "total": total, "empresas": empresas, "paises": paises, "motivos": motivos,
            "tipos": tipos, "paginas": paginas, "lista": recientes,
            "dia": dia,
            "sin_anotar": sin_anotar,
            "rastro": self.diario.rastro(&desde, &hasta, 10),
            "recortada": total > cortes::MAX_LISTA,
        });
        self.a_pestana(id, &v).into_iter().collect()
    }

    fn exporta_cortes(&mut self, id: u32, periodo: &str, formato: &str) -> Vec<Orden> {
        self.guarda_cortes();
        let (desde, hasta) = self.periodo(periodo);
        // The summary is the same page printed without the list (the page hides it itself).
        let que = if formato == "resumen" {
            "resumen"
        } else {
            "cortes"
        };
        let base = if desde == hasta {
            format!("guardiana-zero-{que}-{hasta}")
        } else {
            format!("guardiana-zero-{que}-{desde}-a-{hasta}")
        };
        if formato == "pdf" || formato == "resumen" {
            let ruta = self.nombre_libre(&base, "pdf");
            return vec![Orden::GuardaPdf { id, ruta }];
        }
        let lista = cortes::lee(&self.datos.join("cortes"), &desde, &hasta);
        let columnas: Vec<String> = [
            "cortes_col_hora",
            "cortes_col_pagina",
            "cortes_col_empresa",
            "cortes_col_pais",
            "cortes_col_destino",
            "cortes_col_ruta",
            "cortes_col_que",
            "cortes_col_tipo",
            "cortes_col_metodo",
            "cortes_col_motivo",
            "cortes_col_corredor",
            "cortes_col_dato",
        ]
        .iter()
        .map(|k| self.t(k))
        .collect();
        let zona = self.zona;
        let textos = |k: &str| self.t(k);
        let hora = |ts: i64| fecha::hora_local(ts, zona);
        let texto_csv = cortes::csv(&lista, &columnas, &textos, &hora);
        let ruta = self.nombre_libre(&base, "csv");
        let v = match escribe(&ruta, texto_csv.as_bytes()) {
            Ok(()) => {
                let r = ruta.display().to_string();
                json!({ "tipo": "guardado", "ruta": r, "texto": self.tf("guardado_en", &[("ruta", &r)]) })
            }
            Err(_) => json!({ "tipo": "guardado", "ruta": "", "texto": self.t("guardar_error") }),
        };
        self.a_pestana(id, &v).into_iter().collect()
    }

    /// The engine finished (or failed) saving a PDF the session asked for.
    pub fn pdf_hecho(&mut self, id: u32, ruta: &Path, ok: bool) -> Vec<Orden> {
        let ruta = ruta.display().to_string();
        let v = if ok {
            json!({ "tipo": "guardado", "ruta": ruta, "texto": self.tf("guardado_en", &[("ruta", &ruta)]) })
        } else {
            json!({ "tipo": "guardado", "ruta": "", "texto": self.t("guardar_error") })
        };
        self.a_pestana(id, &v).into_iter().collect()
    }

    /// The result of a script the session asked for (as JSON, the way the engine returns it).
    pub fn script_hecho(&mut self, etiqueta: Etiqueta, resultado: &str) -> Vec<Orden> {
        let valor: Value = serde_json::from_str(resultado).unwrap_or(Value::Null);
        match etiqueta {
            Etiqueta::Pegar => {
                if valor.as_str() == Some("ok") {
                    Vec::new()
                } else {
                    vec![self.aviso("tachon_sin_campo", &[])]
                }
            }
            Etiqueta::Seleccion => {
                let sel = valor.as_str().unwrap_or("").trim().to_string();
                let texto = if sel.is_empty() {
                    String::new()
                } else {
                    tachon::restaura(&sel, &self.tachon)
                };
                vec![envia(
                    Origen::Panel,
                    &json!({ "tipo": "restaurado", "texto": texto }),
                )]
            }
        }
    }

    // --- the form guard ------------------------------------------------------------------------

    /// A web page's guard asks whether to deal with cookie notices: yes with the subscription on
    /// and the setting on. The answer goes on the guard's own channel, like a form's.
    fn cookies_pide(&self, id: u32, m: &Value) -> Vec<Orden> {
        let ficha: String = texto(m, "zg").chars().take(64).collect();
        let si = self.licencia.protege() && !self.prefs.cookies_sin_tocar;
        vec![Orden::RespondeFormulario {
            id,
            json: json!({ "zg": ficha, "cookies": si }).to_string(),
        }]
    }

    /// The guard dealt with the page's cookie notice: the shield says so. Only the managers the
    /// guard knows, and only when it was told to act.
    fn cookies_hecho(&mut self, id: u32, m: &Value) -> Vec<Orden> {
        if !self.licencia.protege() || self.prefs.cookies_sin_tocar {
            return Vec::new();
        }
        let gestor: String = texto(m, "gestor").chars().take(40).collect();
        let accion = match texto(m, "accion") {
            "rechazado" => "rechazado",
            "escondido" => "escondido",
            _ => return Vec::new(),
        };
        if gestor.is_empty() || !gestor.chars().all(|c| c.is_alphanumeric() || c == ' ') {
            return Vec::new();
        }
        let hoy = self.hoy();
        let Some(p) = self.pestana_mut(id) else {
            return Vec::new();
        };
        let primera = p.escudo.cookies.is_none();
        p.escudo.cookies = Some((gestor, accion.to_string()));
        if primera {
            self.diario.dias.entry(hoy).or_default().avisos_cookies += 1;
            self.diario_sucio = true;
            self.hoy_sucio = true;
        }
        if id == self.activa {
            self.escudo_activa(false)
        } else {
            Vec::new()
        }
    }

    fn formulario(&mut self, id: u32, fuente: &str, m: &Value) -> Vec<Orden> {
        let Some(id_pagina) = numero(m, "id") else {
            return Vec::new();
        };
        let ficha: String = texto(m, "zg").chars().take(64).collect();
        let Some(p) = self.pestana(id) else {
            return Vec::new();
        };
        let titulo = self.titulo_de(p);
        let mandato = p.perfil == Perfil::Mandato && self.mandato.is_some();
        let valores: Vec<String> = m
            .get("valores")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .take(40)
                    .map(|s| s.chars().take(300).collect())
                    .collect()
            })
            .unwrap_or_default();
        let accion = texto(m, "accion");
        let host = host_de(accion)
            .filter(|_| accion.starts_with("http"))
            .or_else(|| host_de(fuente))
            .unwrap_or_default();
        let s = sitio(&host);
        let encontradas = clases(&valores.join("\n"), &self.tinta);
        let responde = |enviar: bool| Orden::RespondeFormulario {
            id,
            json: json!({ "zg": ficha, "id": id_pagina, "enviar": enviar }).to_string(),
        };
        if encontradas.is_empty() || host.is_empty() || !self.licencia.protege() {
            return vec![responde(true)];
        }
        // A site that already has these kinds of data from you is not asked about again; the
        // question is for the first time a site would get something new.
        let ya_lo_tiene = self
            .libro
            .entradas
            .get(&s)
            .is_some_and(|e| encontradas.iter().all(|c| e.clases.contains(c)));
        if !mandato && (ya_lo_tiene || self.prefs.sin_preguntar.contains(&s)) {
            let ahora = (self.reloj)();
            // An isolated tab leaves nothing behind, not even a line in the book.
            if !self.es_aislada(id) {
                self.libro.anota(&host, &encontradas, ahora);
                self.libro_sucio = true;
            }
            if let Some(p) = self.pestana_mut(id) {
                p.al_libro.insert(s.clone());
            }
            return vec![responde(true)];
        }
        if self.preguntas.values().any(|q| q.pestana == id) {
            return Vec::new();
        }
        let n = self.siguiente_pregunta;
        self.siguiente_pregunta += 1;
        let corredor = crate::destino::clasifica(&host).corredor.is_some();
        self.preguntas.insert(
            n,
            Pregunta {
                pestana: id,
                id_pagina,
                ficha: ficha.clone(),
                sitio: s.clone(),
                host,
                clases: encontradas.clone(),
                mandato,
            },
        );
        let mut o = self.abre_panel("formulario");
        let datos: Vec<&str> = encontradas.iter().map(|c| c.clave()).collect();
        o.push(envia(
            Origen::Panel,
            &json!({
                "tipo": "pregunta_formulario", "id": n, "sitio": s, "datos": datos,
                "corredor": corredor, "mandato": mandato, "pestana": titulo,
            }),
        ));
        o
    }

    fn formulario_respuesta(&mut self, n: u64, enviar: bool, recordar: bool) -> Vec<Orden> {
        let Some(q) = self.preguntas.remove(&n) else {
            return Vec::new();
        };
        let ahora = (self.reloj)();
        let aislada = self.es_aislada(q.pestana);
        if enviar {
            if !aislada {
                self.libro.anota(&q.host, &q.clases, ahora);
                self.guarda_libro();
            }
            // Written once: the request that carries it is the same sending.
            if let Some(p) = self.pestana_mut(q.pestana) {
                p.al_libro.insert(q.sitio.clone());
            }
            if q.mandato {
                if let Some(md) = self.mandato.as_mut() {
                    let detalle: Vec<&str> = q.clases.iter().map(|c| c.clave()).collect();
                    md.anota(ahora, "dato", &q.sitio, &detalle.join(","));
                    self.mandato_sucio = true;
                }
            }
            if recordar && !q.mandato && !aislada {
                self.prefs.sin_preguntar.insert(q.sitio.clone());
                self.guarda_prefs();
            }
        }
        let mut o = vec![Orden::RespondeFormulario {
            id: q.pestana,
            json: json!({ "zg": q.ficha, "id": q.id_pagina, "enviar": enviar }).to_string(),
        }];
        o.extend(self.cierra_panel());
        o
    }

    // --- everything else -------------------------------------------------------------------------

    fn borrar_todo(&mut self) -> Vec<Orden> {
        for f in [
            "tinta.json",
            "libro.json",
            "diario.json",
            "clave-recibos.p8",
            "registro.txt",
        ] {
            let _ = fs::remove_file(self.datos.join(f));
        }
        self.prefs.sin_preguntar.clear();
        self.guarda_prefs();
        let _ = fs::remove_dir_all(self.datos.join("recibos"));
        let _ = fs::remove_dir_all(self.datos.join("cortes"));
        self.cortes_pend.clear();
        self.tinta.clear();
        self.libro = Libro::default();
        self.diario = Diario::default();
        self.mandato = None;
        self.terminado = None;
        self.recibo = None;
        self.tachon.clear();
        self.preguntas.clear();
        self.rehace_formas();
        let mut o = vec![Orden::BorraNavegacion];
        // Every tab goes: what they hold in memory is part of «everything».
        let ids: Vec<u32> = self.pestanas.iter().map(|p| p.id).collect();
        o.extend(self.nueva_pestana(Perfil::General, INICIO));
        for id in ids {
            o.extend(self.cerrar(id));
        }
        o.extend([
            envia(Origen::Panel, &self.msg_tinta()),
            envia(Origen::Panel, &self.msg_libro()),
            envia(Origen::Panel, &self.msg_mandato()),
            self.aviso("borrado", &[]),
        ]);
        o
    }

    fn guardar_imagen(&self, origen: Origen, datos: &str) -> Vec<Orden> {
        let Some(b64) = datos.strip_prefix("data:image/png;base64,") else {
            return Vec::new();
        };
        if b64.len() > 12_000_000 {
            return Vec::new();
        }
        let Ok(png) = base64::engine::general_purpose::STANDARD.decode(b64) else {
            return Vec::new();
        };
        if !png.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Vec::new();
        }
        let mes: String = self.hoy().chars().take(7).collect();
        let ruta = self.nombre_libre(&format!("guardiana-zero-{mes}"), "png");
        let destino = match origen {
            Origen::Pestana(_) => origen,
            _ => Origen::Panel,
        };
        match escribe(&ruta, &png) {
            Ok(()) => {
                let ruta = ruta.display().to_string();
                vec![envia(
                    destino,
                    &json!({ "tipo": "guardado", "ruta": ruta, "texto": self.tf("guardado_en", &[("ruta", &ruta)]) }),
                )]
            }
            Err(_) => vec![envia(
                destino,
                &json!({ "tipo": "guardado", "ruta": "", "texto": self.t("guardar_error") }),
            )],
        }
    }

    // --- what the engine reports ------------------------------------------------------------------

    /// A tab is about to open an address. Nothing about the page changes yet: until the new
    /// page is really there (`pagina_nueva`), the bar and the shield keep the page that is
    /// showing, so a page that never arrives (a download, a 204, a stopped load) cannot make
    /// the bar show an address it is not at. Returns whether to cancel it.
    pub fn navegacion_empieza(
        &mut self,
        id: u32,
        url: &str,
        redirigida: bool,
    ) -> (bool, Vec<Orden>) {
        let es_web =
            (url.starts_with("http://") || url.starts_with("https://")) && !es_interna(url);
        let protege = self.licencia.protege();
        let Some(p) = self.pestana_mut(id) else {
            return (false, Vec::new());
        };
        // A server redirect to an address with tracking tags (a redirect is always a GET here):
        // cancelled and opened clean. Pages the person opens are cleaned when their request
        // leaves (`peticion`), which also keeps form posts intact.
        if redirigida && es_web && protege {
            if let Some((limpia, n)) = url_limpia::limpia(url) {
                p.quitados += u32::try_from(n).unwrap_or(u32::MAX);
                p.limpiando = Some(sin_fragmento(url).to_string());
                return (true, vec![Orden::Navega { id, url: limpia }]);
            }
        }
        p.navegando = Some(sin_fragmento(url).to_string());
        p.cargando = true;
        let mut o = Vec::new();
        if id == self.activa {
            o.push(self.estado());
        }
        (false, o)
    }

    /// The new page is there (the engine's ContentLoading, with the address it really loaded).
    /// From here on, what this tab asks for belongs to it.
    pub fn pagina_nueva(&mut self, id: u32, url: &str) -> Vec<Orden> {
        let hoy = self.hoy();
        let Some(p) = self.pestanas.iter_mut().find(|p| p.id == id) else {
            return Vec::new();
        };
        if p.limpiando.as_deref() == Some(sin_fragmento(url)) {
            // The one-line page that reopens the address without its tracking tags.
            return Vec::new();
        }
        p.limpiando = None;
        let es_web =
            (url.starts_with("http://") || url.starts_with("https://")) && !es_interna(url);
        let s = if es_web {
            host_de(url).map(|h| sitio(&h)).unwrap_or_default()
        } else {
            String::new()
        };
        if s != p.escudo.sitio {
            p.icono.clear();
        }
        let quitados = std::mem::take(&mut p.quitados);
        p.url = url.to_string();
        p.al_libro.clear();
        p.escudo = escudo::Pestana::nueva(&s);
        p.escudo.parametros_quitados = quitados;
        if !es_interna(url) {
            p.lista = false;
        }
        if quitados > 0 {
            self.diario.dias.entry(hoy).or_default().parametros_quitados += quitados;
            self.diario_sucio = true;
            self.hoy_sucio = true;
        }
        // A question from the page that left has nothing left to answer.
        let mut o = Vec::new();
        let antes = self.preguntas.len();
        self.preguntas.retain(|_, q| q.pestana != id);
        if antes != self.preguntas.len() && self.panel.as_deref() == Some("formulario") {
            o.extend(self.cierra_panel());
        }
        self.sucios.insert(id);
        if id == self.activa {
            o.push(self.estado());
            o.push(self.titulo_ventana());
        }
        o
    }

    /// A tab finished loading (or failed).
    pub fn navegacion_termina(&mut self, id: u32) -> Vec<Orden> {
        let Some(p) = self.pestana_mut(id) else {
            return Vec::new();
        };
        p.cargando = false;
        p.navegando = None;
        let mut o = Vec::new();
        if id == self.activa {
            o.push(self.estado());
            o.push(self.titulo_ventana());
        }
        o
    }

    /// The address a tab shows changed (also within the same page).
    pub fn fuente(&mut self, id: u32, url: &str) -> Vec<Orden> {
        let Some(p) = self.pestana_mut(id) else {
            return Vec::new();
        };
        if p.limpiando.as_deref() == Some(sin_fragmento(url)) {
            return Vec::new();
        }
        p.url = url.to_string();
        if !es_interna(url) {
            p.lista = false;
        }
        if id == self.activa {
            vec![self.estado(), self.titulo_ventana()]
        } else {
            vec![self.estado()]
        }
    }

    /// A tab's title changed.
    pub fn titulo(&mut self, id: u32, titulo: &str) -> Vec<Orden> {
        let Some(p) = self.pestana_mut(id) else {
            return Vec::new();
        };
        p.titulo = titulo.chars().take(300).collect();
        if id == self.activa {
            vec![self.estado(), self.titulo_ventana()]
        } else {
            vec![self.estado()]
        }
    }

    /// The engine fetched the page's icon (PNG). Kept small: it travels in every bar update.
    pub fn icono(&mut self, id: u32, png: &[u8]) -> Vec<Orden> {
        let Some(p) = self.pestana_mut(id) else {
            return Vec::new();
        };
        p.icono = if png.starts_with(b"\x89PNG\r\n\x1a\n") && png.len() <= 48 * 1024 {
            format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(png)
            )
        } else {
            String::new()
        };
        vec![self.estado()]
    }

    /// Whether a tab can go back or forward now.
    pub fn historial(&mut self, id: u32, atras: bool, adelante: bool) -> Vec<Orden> {
        let Some(p) = self.pestana_mut(id) else {
            return Vec::new();
        };
        p.atras = atras;
        p.adelante = adelante;
        if id == self.activa {
            vec![self.estado()]
        } else {
            Vec::new()
        }
    }

    /// A tab asks for a new window (a link with `target=_blank`, a sign-in popup). It becomes a
    /// tab in the same session as the one that asked.
    pub fn ventana_nueva(&mut self, de: u32) -> Option<(u32, Vec<Orden>)> {
        // A window from a tab the session does not know (closing) is not opened.
        let perfil = self.perfil_de(de)?;
        let id = self.siguiente;
        self.siguiente += 1;
        let pos = self
            .pestanas
            .iter()
            .position(|p| p.id == de)
            .map_or(self.pestanas.len(), |i| i + 1);
        self.pestanas.insert(pos, Pestana::nueva(id, perfil, ""));
        let mut o = vec![Orden::CreaPestana {
            id,
            perfil,
            url: None,
            de: Some(de),
        }];
        o.extend(self.activar(id));
        Some((id, o))
    }

    /// One request about to leave a tab. `contexto` is the engine's resource context.
    pub fn peticion(
        &mut self,
        id: u32,
        url: &str,
        metodo: &str,
        cuerpo: &[u8],
        contexto: i32,
    ) -> Respuesta {
        let web = ["http://", "https://", "ws://", "wss://"]
            .iter()
            .any(|e| url.starts_with(e));
        if !web || es_interna(url) || !self.licencia.protege() {
            // Without the trial or a subscription the browser opens pages and does nothing of
            // its own: no cut, no cleaning, no count.
            return Respuesta::default();
        }
        let ahora = (self.reloj)();
        let hoy = self.hoy();
        let corta = Respuesta {
            cortar: true,
            pagina: None,
            nota: String::new(),
        };
        let Some(i) = self.pestanas.iter().position(|p| p.id == id) else {
            // A tab the session no longer has (closing, or never known): nothing leaves it.
            return corta;
        };
        if contexto == 1 && self.pestanas[i].limpiando.as_deref() == Some(sin_fragmento(url)) {
            // The tagged address already answered (or cancelled): it never leaves.
            return corta;
        }
        if contexto == 1
            && metodo.eq_ignore_ascii_case("GET")
            && self.pestanas[i].navegando.as_deref() == Some(sin_fragmento(url))
        {
            if let Some((limpia, n)) = url_limpia::limpia(url) {
                // The page is answered here with one line that reopens it without its tracking
                // tags: the tagged address never reaches the site.
                let p = &mut self.pestanas[i];
                p.quitados += u32::try_from(n).unwrap_or(u32::MAX);
                p.limpiando = Some(sin_fragmento(url).to_string());
                return Respuesta {
                    cortar: true,
                    pagina: Some(paginas::reabre(&limpia)),
                    nota: String::new(),
                };
            }
        }
        if self.pestanas[i].perfil == Perfil::Mandato && self.mandato.is_none() {
            // Its mandate ended: the tab is closing and nothing more leaves it.
            let documento = recurso_de_contexto(contexto) == Recurso::Documento;
            return Respuesta {
                cortar: true,
                nota: String::new(),
                pagina: documento.then(|| {
                    paginas::pagina_cortada(
                        &self.textos,
                        &self.t("pagina_cortada_mandato_fin"),
                        url,
                        &self.prefs.tema,
                    )
                }),
            };
        }
        let d = {
            let p = &self.pestanas[i];
            let mut recurso = recurso_de_contexto(contexto);
            if recurso == Recurso::Documento && p.navegando.as_deref() != Some(sin_fragmento(url)) {
                // Only the tab's own navigation is «the page»; any other document is a frame.
                recurso = Recurso::Marco;
            }
            let sitio_pagina = if recurso == Recurso::Documento {
                host_de(url).map(|h| sitio(&h)).unwrap_or_default()
            } else {
                p.escudo.sitio.clone()
            };
            let mandato = if p.perfil == Perfil::Mandato {
                self.mandato.as_ref()
            } else {
                None
            };
            let (formas, senuelo) = if mandato.is_some() {
                (&self.formas_mandato, Some(self.tinta.len()))
            } else {
                (&self.formas, None)
            };
            let con_cuerpo = matches!(
                metodo.to_ascii_uppercase().as_str(),
                "POST" | "PUT" | "PATCH"
            );
            let peticion = Peticion {
                url,
                cuerpo: if con_cuerpo { cuerpo } else { &[] },
                recurso,
                sitio_pagina: &sitio_pagina,
            };
            (
                decide(&peticion, &self.prefs.reglas, formas, senuelo, mandato),
                recurso,
            )
        };
        let (mut d, recurso) = d;
        if recurso == Recurso::Documento && !d.cortar && !d.hallazgos.is_empty() {
            // A page sending your marked data to another site by opening it (a link, a redirect,
            // `location=`): cut, unless you typed that address yourself.
            let p = &self.pestanas[i];
            let escrita = p.escrita.as_deref() == Some(sin_fragmento(url));
            if !escrita
                && !p.escudo.sitio.is_empty()
                && crate::destino::es_tercero(&d.destino.host, &p.escudo.sitio)
            {
                d.cortar = true;
                d.motivo = Some(Motivo::Tinta);
                d.otro = true;
            }
        }
        let p = &mut self.pestanas[i];
        p.escudo.anota(&d);
        let perfil = p.perfil;
        // The person's own webs of the day: not an isolated tab, and not the AI's mandate tab.
        let web = (perfil == Perfil::General && !es_interna(&p.url) && !p.escudo.sitio.is_empty())
            .then(|| p.escudo.sitio.clone());
        self.diario.anota(&hoy, &d, web.as_deref());
        // A day's list of cuts stops at `cortes::MAX_DIA` lines (a page calling endless names
        // must not fill the disk); the day's figures still count every one, and the list says
        // how many were left out.
        let cabe = self
            .diario
            .dias
            .get(&hoy)
            .is_none_or(|e| e.cortadas as usize <= cortes::MAX_DIA);
        if recurso == Recurso::Documento && !d.cortar {
            self.diario.dias.entry(hoy).or_default().paginas += 1;
        }
        if d.cortar && d.de_fuera() && cabe && self.cortes_pend.len() < 20_000 {
            let dato = d
                .hallazgos
                .first()
                .and_then(|h| self.tinta.get(h.origen))
                .and_then(|m| clase_de(m.tipo))
                .map(Clase::clave);
            let p = &self.pestanas[i];
            let lugar = Lugar {
                ts: ahora,
                pagina: &p.escudo.sitio,
                url,
                metodo,
                recurso,
                dato,
                aislada: p.perfil == Perfil::Aislada,
                mandato: p.perfil == Perfil::Mandato,
            };
            self.cortes_pend.push(Corte::de(&lugar, &d));
        }
        if d.de_fuera() {
            if self.pulsos.len() < 2000 {
                self.pulsos.push(
                    json!({ "pestana": id, "cortado": d.cortar, "quien": d.destino.quien() }),
                );
            }
            self.hoy_sucio = true;
        }
        // What actually left with your marked data goes into the book, once per page.
        if !d.cortar && !d.hallazgos.is_empty() {
            let mut cl: Vec<Clase> = Vec::new();
            for h in &d.hallazgos {
                if let Some(c) = self.tinta.get(h.origen).and_then(|m| clase_de(m.tipo)) {
                    if !cl.contains(&c) {
                        cl.push(c);
                    }
                }
            }
            if !cl.is_empty()
                && perfil != Perfil::Aislada
                && self.pestanas[i].al_libro.insert(d.destino.sitio.clone())
            {
                self.libro.anota(&d.destino.host, &cl, ahora);
                self.libro_sucio = true;
            }
        }
        if perfil == Perfil::Mandato {
            if let Some(md) = self.mandato.as_mut() {
                if recurso == Recurso::Documento && !d.cortar {
                    md.anota(ahora, "visita", &d.destino.sitio, "");
                }
                if !d.cortar && !d.hallazgos.is_empty() {
                    let como = d.hallazgos.first().map_or("", |h| h.como);
                    md.anota(ahora, "dato", &d.destino.sitio, como);
                }
                match d.motivo {
                    Some(Motivo::FueraDeMandato) => {
                        md.anota(ahora, "cortado", &d.destino.sitio, "");
                    }
                    Some(Motivo::Tinta | Motivo::Senuelo) => {
                        let como = d.hallazgos.first().map_or("", |h| h.como);
                        md.anota(ahora, "tinta", &d.destino.sitio, como);
                    }
                    _ => {}
                }
                self.mandato_sucio = true;
            }
        }
        self.sucios.insert(id);
        self.diario_sucio = true;
        let nota = if self.depura {
            format!(
                "{recurso:?} pagina={} tercero={} cortar={} motivo={:?}",
                self.pestanas
                    .iter()
                    .find(|p| p.id == id)
                    .map_or("", |p| p.escudo.sitio.as_str()),
                d.tercero,
                d.cortar,
                d.motivo
            )
        } else {
            String::new()
        };
        if !d.cortar {
            return Respuesta {
                nota,
                ..Respuesta::default()
            };
        }
        let pagina = (recurso == Recurso::Documento).then(|| {
            let s = d.destino.sitio.as_str();
            let razon = match d.motivo {
                Some(Motivo::FueraDeMandato) => self.tf("pagina_cortada_mandato", &[("sitio", s)]),
                Some(Motivo::CorteTuyo) => self.tf("pagina_cortada_tuya", &[("sitio", s)]),
                Some(Motivo::Senuelo) => self.t("pagina_cortada_senuelo"),
                Some(Motivo::Tinta) => {
                    let dato = d
                        .hallazgos
                        .first()
                        .and_then(|h| self.tinta.get(h.origen))
                        .and_then(|m| clase_de(m.tipo))
                        .map(|c| self.t(c.clave()).to_lowercase())
                        .unwrap_or_default();
                    self.tf("pagina_cortada_tinta", &[("dato", &dato), ("sitio", s)])
                }
                Some(m) => self.t(&format!("motivo_{}", cortes::motivo_clave(m))),
                None => String::new(),
            };
            paginas::pagina_cortada(&self.textos, &razon, url, &self.prefs.tema)
        });
        Respuesta {
            cortar: true,
            pagina,
            nota,
        }
    }

    /// Called a few times a second: sends what changed, in batches, and saves now and then.
    pub fn tic(&mut self) -> Vec<Orden> {
        let ahora = (self.reloj)();
        let mut o = Vec::new();
        // The subscription: read again every minute (the trial ends at a time of day, not at a
        // start), and the periodic check asked for every ten minutes; it connects only when due,
        // and at most once an hour.
        if ahora - self.licencia_leida >= 60_000 {
            self.licencia_leida = ahora;
            if let Some(e) = self.lugar.as_ref().and_then(|l| l.estado(ahora)) {
                o.extend(self.pon_licencia(e));
            }
        }
        if self.lugar.is_some() && ahora - self.licencia_pedida >= 600_000 {
            self.licencia_pedida = ahora;
            o.push(Orden::ComprobarLicencia);
        }
        if self
            .mandato
            .as_ref()
            .is_some_and(|m| m.caduca > 0 && ahora >= m.caduca)
        {
            o.extend(self.cierra_mandato());
            o.push(self.aviso("mandato_caducado", &[]));
        }
        if !self.pulsos.is_empty() {
            let lista = std::mem::take(&mut self.pulsos);
            o.push(envia(
                Origen::Barra,
                &json!({ "tipo": "pulsos", "lista": lista }),
            ));
        }
        if self.sucios.remove(&self.activa) {
            o.extend(self.escudo_activa(false));
        }
        if self.mandato_sucio {
            self.mandato_sucio = false;
            if self.panel.as_deref() == Some("mandato") {
                o.push(envia(Origen::Panel, &self.msg_mandato()));
            }
        }
        if self.libro_sucio && self.panel.as_deref() == Some("datos") {
            o.push(envia(Origen::Panel, &self.msg_libro()));
        }
        // Midnight: yesterday's figures must not stay under today's date on an open new tab, and
        // the oldest day of the log goes.
        let hoy = self.hoy();
        if hoy != self.dia_visto {
            self.dia_visto = hoy;
            self.poda_cortes();
            self.hoy_sucio = true;
            self.ultimo_hoy = 0;
        }
        if self.hoy_sucio && ahora - self.ultimo_hoy >= 1000 {
            self.hoy_sucio = false;
            self.ultimo_hoy = ahora;
            o.extend(self.hoy_a_todos());
        }
        if ahora - self.ultimo_guardado >= 15_000 {
            self.ultimo_guardado = ahora;
            if self.diario_sucio {
                self.guarda_diario();
            }
            self.guarda_cortes();
            if self.libro_sucio {
                self.guarda_libro();
            }
        }
        o
    }

    /// A key the engine reports as an accelerator. `Some` when the browser handles it.
    pub fn tecla(&mut self, vk: u32, ctrl: bool, shift: bool, alt: bool) -> Option<Vec<Orden>> {
        let activa = self.activa;
        let n = self.pestanas.len();
        let pos = self
            .pestanas
            .iter()
            .position(|p| p.id == activa)
            .unwrap_or(0);
        let letra = |c: char| vk == u32::from(c);
        Some(match (ctrl, shift, alt) {
            (true, false, false) if letra('T') => self.pestana_nueva(Perfil::General),
            (true, true, false) if letra('N') => self.pestana_nueva(Perfil::Aislada),
            (true, _, false) if letra('W') || vk == 0x73 => self.cerrar(activa),
            (true, false, false) if letra('L') => self.foco_direccion(),
            (false, false, true) if letra('D') => self.foco_direccion(),
            (false, false, false) if vk == 0x75 => self.foco_direccion(),
            (true, _, false) if letra('R') => vec![Orden::Recarga { id: activa }],
            (false, _, false) if vk == 0x74 => vec![Orden::Recarga { id: activa }],
            (false, false, true) if vk == 0x25 => vec![Orden::Atras { id: activa }],
            (false, false, true) if vk == 0x27 => vec![Orden::Adelante { id: activa }],
            (_, _, _) if vk == 0xA6 => vec![Orden::Atras { id: activa }],
            (_, _, _) if vk == 0xA7 => vec![Orden::Adelante { id: activa }],
            (true, s, false) if vk == 0x09 && n > 1 => {
                let j = if s { (pos + n - 1) % n } else { (pos + 1) % n };
                let id = self.pestanas[j].id;
                self.activar(id)
            }
            (true, false, false) if (0x31..=0x39).contains(&vk) && n > 0 => {
                let j = if vk == 0x39 {
                    n - 1
                } else {
                    usize::try_from(vk - 0x31).unwrap_or(0).min(n - 1)
                };
                let id = self.pestanas[j].id;
                self.activar(id)
            }
            _ => return None,
        })
    }

    fn foco_direccion(&self) -> Vec<Orden> {
        vec![
            Orden::Foco { a: Origen::Barra },
            envia(Origen::Barra, &json!({ "tipo": "foco_direccion" })),
        ]
    }

    /// The window is closing: a running mandate ends with its receipt, and everything is saved.
    pub fn cierra(&mut self) {
        if self.mandato.is_some() {
            let _ = self.termina_mandato();
        }
        self.guarda_cortes();
        self.guarda_diario();
        self.guarda_libro();
        self.guarda_prefs();
    }
}

#[cfg(test)]
mod tests;
