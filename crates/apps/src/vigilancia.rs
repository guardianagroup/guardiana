//! «Vigilancia» (1.0.45): the constant, read-only watch over this computer. The owner, 18 Oct
//! 2026: «la experiencia del cliente debe ser sentir el monitoreo constante… para saber que todo
//! está bien y nada está vigilando o extrayendo sus datos».
//!
//! Four things, each read from what the system itself records, never guessed:
//!
//! 1. **The camera and the microphone in use now**, and by which program: Windows keeps, per
//!    program, when it last started and stopped using them (the privacy settings' «recent
//!    activity», the registry's `CapabilityAccessManager\ConsentStore`). ZERO already read it
//!    during a SECURE call (`crates/zero/src/vigia.rs`); here the service reads it for every
//!    signed-in account, all the time, a few times a minute. Its limit, said on the screen: a
//!    program that reaches the device without going through Windows' permission (some old desktop
//!    programs and drivers) is not recorded there, so it cannot be named.
//! 2. **New programs that start on their own**: the Run and RunOnce keys of the computer and of
//!    each signed-in account, the Startup folders, the scheduled tasks and the services. The first
//!    look is the starting point; anything that appears later is new, with the time it was first
//!    seen.
//! 3. **Privacy settings that changed**: the ones the check-up reads (`revision.ps1`: telemetry,
//!    the diagnostics service, the advertising id) and the camera, microphone and location
//!    switches.
//! 4. **The antivirus or the firewall turned off**: Windows Security Center's products, Microsoft
//!    Defender's own status and the firewall's three profiles.
//!
//! 2, 3 and 4 come from one read-only PowerShell (`vigilancia.ps1`) every few minutes, at low
//! priority; 1 is read directly from the registry, which costs nothing. On a Mac the startup items
//! (LaunchAgents and LaunchDaemons) and the firewall are read; the rest says «no disponible en Mac
//! todavía». On Linux, the autostart folders and the enabled systemd units.
//!
//! This module also decides what is new against what it remembers ([`Base`]), so the decision is
//! the same, and tested, everywhere. It writes nothing itself: the panel keeps the base and
//! writes every finding to the watch's chain in the ledger.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Error;

/// The script, as it travels inside the binary.
pub const SCRIPT: &str = include_str!("vigilancia.ps1");

/// How long the script may take before it is stopped.
pub const LIMITE_SCRIPT: std::time::Duration = std::time::Duration::from_secs(120);

const MINUTO: i64 = 60_000;
const HORA: i64 = 60 * MINUTO;
const DIA: i64 = 24 * HORA;

// ---------------------------------------------------------------------------------------------
// What the system says.

/// What was used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Recurso {
    /// The camera.
    Camara,
    /// The microphone.
    Microfono,
}

impl Recurso {
    /// The stored word.
    #[must_use]
    pub const fn como_texto(self) -> &'static str {
        match self {
            Self::Camara => "camara",
            Self::Microfono => "microfono",
        }
    }

    /// «la cámara», «el micrófono», for the chain's sentences.
    #[must_use]
    pub const fn nombre(self) -> &'static str {
        match self {
            Self::Camara => "la cámara",
            Self::Microfono => "el micrófono",
        }
    }
}

/// One use of the camera or the microphone, as Windows recorded it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Uso {
    /// The program: its file name (`Zoom.exe`) or, for a Store app, its package name.
    pub programa: String,
    /// Its full path when Windows gave one.
    pub ruta: Option<String>,
    /// What it used.
    pub recurso: Recurso,
    /// Unix ms when it last started using it.
    pub desde: i64,
    /// Unix ms when it stopped; `None` while it is still using it.
    pub hasta: Option<i64>,
    /// The account (its SID) whose record it is, or `equipo`.
    pub cuenta: String,
}

/// Windows' FILETIME (100 ns since 1601) as Unix ms; `None` for zero (never, or «still»).
#[must_use]
pub fn de_filetime(ft: u64) -> Option<i64> {
    const DESDE_1601_MS: u64 = 11_644_473_600_000;
    if ft == 0 {
        return None;
    }
    (ft / 10_000)
        .checked_sub(DESDE_1601_MS)
        .and_then(|ms| i64::try_from(ms).ok())
}

/// One ConsentStore entry as the registry gives it: the key's name and its two times.
/// `NonPackaged` programs are named by their path with `#` for `\`; Store apps by their package
/// family name (`Microsoft.WindowsCamera_8wekyb3d8bbwe`). A start of zero means it never used it.
/// The same reading as ZERO's (`crates/zero/src/vigia.rs`), in milliseconds and with the account.
#[must_use]
pub fn de_registro(
    clave: &str,
    recurso: Recurso,
    inicio: u64,
    fin: u64,
    cuenta: &str,
) -> Option<Uso> {
    let desde = de_filetime(inicio)?;
    let (programa, ruta) = if clave.contains('#') {
        let ruta = clave.replace('#', "\\");
        let nombre = ruta.rsplit('\\').next().unwrap_or_default().to_owned();
        (nombre, Some(ruta))
    } else {
        let nombre = clave.split('_').next().unwrap_or(clave).to_owned();
        (nombre, None)
    };
    if programa.is_empty() {
        return None;
    }
    Some(Uso {
        programa: programa.chars().take(120).collect(),
        ruta: ruta.map(|r| r.chars().take(400).collect()),
        recurso,
        desde,
        // A stop older than the start is the previous use's: it is using it now.
        hasta: de_filetime(fin).filter(|&h| fin >= inicio && h >= desde),
        cuenta: cuenta.chars().take(80).collect(),
    })
}

/// Whether the program is GUARDIANA itself (ZERO's window during a SECURE call).
#[must_use]
pub fn es_propio(programa: &str) -> bool {
    let p = programa.to_ascii_lowercase();
    p == "guardiana-zero.exe" || p == "guardiana.exe"
}

/// Whether the program is the WebView2 engine, which draws ZERO and many other programs: Windows
/// names the engine, not the program inside it, and the screen says so.
#[must_use]
pub fn es_motor_web(programa: &str) -> bool {
    programa.eq_ignore_ascii_case("msedgewebview2.exe")
}

/// Something that starts on its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Arranque {
    /// `run`, `runonce`, `carpeta`, `tarea`, `servicio`, `launchd`, `autostart`, `systemd`.
    pub tipo: String,
    /// Where it is (the registry key, the folder, the task's folder, the service's start mode).
    pub donde: String,
    /// Its name (the value's name, the file, the task, the service).
    pub nombre: String,
    /// Its readable name when the system has one (a service's display name).
    #[serde(default)]
    pub visible: String,
    /// What it runs, as the system says it.
    pub ruta: String,
    /// The account (its SID or user name) or `equipo`.
    pub cuenta: String,
}

impl Arranque {
    /// What identifies it: kind, place and name. Not what it runs, so a program that updates
    /// itself into a new folder is not «new» every week.
    #[must_use]
    pub fn clave(&self) -> String {
        match self.tipo.as_str() {
            // A service is its name; its start mode lives in `donde` and may change.
            "servicio" => format!("servicio|{}", self.nombre.to_lowercase()),
            _ => format!(
                "{}|{}|{}",
                self.tipo,
                self.donde.to_lowercase(),
                self.nombre.to_lowercase()
            ),
        }
    }

    /// The name a person reads.
    #[must_use]
    pub fn titulo(&self) -> &str {
        if self.visible.trim().is_empty() {
            &self.nombre
        } else {
            &self.visible
        }
    }

    /// Whether it is part of the system itself: a task in Windows' own folder or a service that
    /// runs from the Windows folder. Recorded like the rest, but not worth a notice: Windows
    /// updates add them every month.
    #[must_use]
    pub fn es_del_sistema(&self) -> bool {
        match self.tipo.as_str() {
            "tarea" => self.donde.to_ascii_lowercase().starts_with("\\microsoft\\"),
            "servicio" => {
                let r = self
                    .ruta
                    .trim()
                    .trim_start_matches('"')
                    .to_ascii_lowercase()
                    .replace('/', "\\");
                let r = r.strip_prefix("\\??\\").unwrap_or(&r);
                r.starts_with("\\systemroot\\")
                    || r.starts_with("system32\\")
                    || r.starts_with("%systemroot%\\")
                    || r.get(1..).is_some_and(|x| x.starts_with(":\\windows\\"))
            }
            _ => false,
        }
    }
}

/// A security product as Windows Security Center lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Producto {
    /// Its name.
    pub nombre: String,
    /// Whether its protection is on, as its `productState` says.
    pub activo: bool,
}

/// Microsoft Defender's own status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Defender {
    /// Its service is running.
    pub servicio: bool,
    /// Its antivirus is on.
    pub antivirus: bool,
    /// Real-time protection is on.
    pub tiempo_real: bool,
    /// Tamper protection is on.
    pub manipulacion: bool,
    /// `Normal` when it is the antivirus doing the work; `Passive Mode` when another one is.
    pub modo: String,
}

impl Defender {
    /// Whether Defender is the antivirus in charge (not standing aside for another one).
    #[must_use]
    pub fn al_mando(&self) -> bool {
        self.modo.trim().eq_ignore_ascii_case("normal")
    }
}

/// Whether the antivirus and the firewall are on.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct Proteccion {
    /// The antivirus products Security Center lists; `None` when it did not answer.
    pub antivirus: Option<Vec<Producto>>,
    /// Other firewalls Security Center lists.
    pub cortafuegos_otros: Option<Vec<Producto>>,
    /// Defender's status, when it answered.
    pub defender: Option<Defender>,
    /// Each firewall profile and whether it is on; on a Mac, one entry (`macOS`).
    pub cortafuegos: Option<Vec<(String, bool)>>,
}

/// One look at what changes slowly.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Foto {
    /// What starts on its own; `None` when it could not be read on this system.
    pub arranque: Option<Vec<Arranque>>,
    /// Privacy setting → value (`sin_fijar` when not set); `None` when not read here.
    pub privacidad: Option<BTreeMap<String, String>>,
    /// The protection's state; `None` when not read here.
    pub proteccion: Option<Proteccion>,
    /// The accounts whose settings could be read now (SID → user name).
    pub cuentas: BTreeMap<String, String>,
    /// What failed, by part.
    pub errores: BTreeMap<String, String>,
}

fn s(v: &Value) -> String {
    match v {
        Value::String(x) => x.trim().to_owned(),
        Value::Null => String::new(),
        otro => otro.to_string(),
    }
}

fn arr(v: &Value) -> Vec<&Value> {
    match v {
        Value::Array(a) => a.iter().collect(),
        Value::Null => Vec::new(),
        otro => vec![otro],
    }
}

/// The script's answer, read. Missing parts stay `None`, never empty: «not read» is not «none».
#[must_use]
pub fn de_json(v: &Value) -> Foto {
    let mut f = Foto::default();
    if !v["arranque"].is_null() {
        f.arranque = Some(
            arr(&v["arranque"])
                .into_iter()
                .filter_map(|x| {
                    let nombre: String = s(&x["nombre"]).chars().take(200).collect();
                    if nombre.is_empty() {
                        return None;
                    }
                    Some(Arranque {
                        tipo: s(&x["tipo"]).chars().take(16).collect(),
                        donde: s(&x["donde"]).chars().take(300).collect(),
                        nombre,
                        visible: s(&x["visible"]).chars().take(200).collect(),
                        ruta: s(&x["ruta"]).chars().take(300).collect(),
                        cuenta: s(&x["cuenta"]).chars().take(80).collect(),
                    })
                })
                .collect(),
        );
    }
    if let Some(o) = v["privacidad"].as_object() {
        f.privacidad = Some(
            o.iter()
                .take(200)
                .map(|(k, x)| {
                    (
                        k.chars().take(120).collect(),
                        s(x).chars().take(60).collect(),
                    )
                })
                .collect(),
        );
    }
    let p = &v["proteccion"];
    if p.is_object() {
        let productos = |x: &Value| -> Option<Vec<Producto>> {
            if x.is_null() {
                return None;
            }
            Some(
                arr(x)
                    .into_iter()
                    .filter_map(|p| {
                        let nombre = s(&p["nombre"]);
                        let estado = u32::try_from(p["estado"].as_u64().unwrap_or(0)).unwrap_or(0);
                        (!nombre.is_empty()).then(|| Producto {
                            nombre: nombre.chars().take(120).collect(),
                            activo: crate::lee_estado(estado).0,
                        })
                    })
                    .collect(),
            )
        };
        let d = &p["defender"];
        f.proteccion = Some(Proteccion {
            antivirus: productos(&p["antivirus"]),
            cortafuegos_otros: productos(&p["cortafuegos_otros"]),
            defender: d.is_object().then(|| Defender {
                servicio: d["servicio"].as_bool().unwrap_or(false),
                antivirus: d["antivirus"].as_bool().unwrap_or(false),
                tiempo_real: d["tiempo_real"].as_bool().unwrap_or(false),
                manipulacion: d["manipulacion"].as_bool().unwrap_or(false),
                modo: s(&d["modo"]).chars().take(40).collect(),
            }),
            cortafuegos: (!p["cortafuegos"].is_null()).then(|| {
                arr(&p["cortafuegos"])
                    .into_iter()
                    .map(|x| (s(&x["perfil"]), x["activo"].as_bool().unwrap_or(false)))
                    .filter(|(n, _)| !n.is_empty())
                    .collect()
            }),
        });
    }
    if let Some(o) = v["cuentas"].as_object() {
        f.cuentas = o
            .iter()
            .take(64)
            .map(|(k, x)| (k.clone(), s(x).chars().take(80).collect()))
            .collect();
    }
    if let Some(o) = v["errores"].as_object() {
        f.errores = o
            .iter()
            .map(|(k, x)| (k.clone(), s(x).chars().take(200).collect()))
            .collect();
    }
    f
}

/// The camera and microphone uses Windows recorded, for every signed-in account and for the
/// computer. Windows only; read straight from the registry, nothing is written.
///
/// # Errors
/// [`Error::NoSoportado`] off Windows.
pub fn usos() -> Result<Vec<Uso>, Error> {
    #[cfg(windows)]
    {
        Ok(win::usos())
    }
    #[cfg(not(windows))]
    {
        Err(Error::NoSoportado)
    }
}

/// The file names (lowercase) of the programs running now, to tell a use Windows still marks as
/// going on from one whose program closed without saying so. `None` when it could not be asked.
#[must_use]
pub fn en_marcha() -> Option<HashSet<String>> {
    #[cfg(windows)]
    {
        win::en_marcha()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// One look at what starts on its own, the privacy settings and the protection. Takes a few
/// seconds on Windows (one PowerShell, low priority); call it off the async runtime.
///
/// # Errors
/// [`Error::Sistema`] when the system could not be asked at all.
pub fn foto() -> Result<Foto, Error> {
    #[cfg(windows)]
    {
        win::foto()
    }
    #[cfg(target_os = "macos")]
    {
        Ok(mac::foto())
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        Ok(linux::foto())
    }
}

// ---------------------------------------------------------------------------------------------
// What the watch remembers, and what is new against it.

/// What one look found worth writing down.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Hallazgo {
    /// Which watch: `camara_microfono`, `arranque`, `privacidad`, `proteccion`.
    pub tipo: &'static str,
    /// What identifies the fact.
    pub clave: String,
    /// Worth a calm notice (after the repeat filter); otherwise it is history only.
    pub aviso: bool,
    /// The screen's sentence: a key of the panel's texts (`vg_ev_*`).
    pub frase: &'static str,
    /// What fills the sentence.
    pub datos: BTreeMap<String, String>,
    /// The sentence in Spanish, for the chain.
    pub texto: String,
}

impl Hallazgo {
    fn nuevo(tipo: &'static str, clave: String, aviso: bool, frase: &'static str) -> Self {
        Self {
            tipo,
            clave,
            aviso,
            frase,
            datos: BTreeMap::new(),
            texto: String::new(),
        }
    }
    fn con(mut self, k: &str, v: impl ToString) -> Self {
        self.datos.insert(k.to_owned(), v.to_string());
        self
    }
    fn texto(mut self, t: String) -> Self {
        self.texto = t;
        self
    }
}

/// One startup entry remembered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recordado {
    /// When it was first seen (Unix ms).
    pub visto: i64,
    /// The name a person reads.
    pub titulo: String,
    /// Its kind.
    pub tipo: String,
    /// Whether it was there at the last look.
    pub presente: bool,
    /// Part of the system itself (see [`Arranque::es_del_sistema`]).
    #[serde(default)]
    pub sistema: bool,
}

/// A use of the camera or the microphone going on, remembered between looks (so a restart of the
/// service does not write it twice).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsoActivo {
    /// The program.
    pub programa: String,
    /// What it uses.
    pub recurso: Recurso,
    /// Since when (Unix ms, Windows' record).
    pub desde: i64,
}

/// What the watch remembers between looks. Names, settings and on/off states only; kept by the
/// panel in the ledger's settings, so «Borrar todo» takes it too.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Base {
    /// When the camera and microphone watch started (its first look).
    #[serde(default)]
    pub usos_desde: Option<i64>,
    /// The last look at the uses.
    #[serde(default)]
    pub usos_ultima: i64,
    /// Uses going on at the last look, by key.
    #[serde(default)]
    pub usos_activos: BTreeMap<String, UsoActivo>,
    /// When the startup watch took its starting point.
    #[serde(default)]
    pub arranque_desde: Option<i64>,
    /// Every startup entry seen, by key.
    #[serde(default)]
    pub arranque: BTreeMap<String, Recordado>,
    /// When the privacy watch took its starting point.
    #[serde(default)]
    pub privacidad_desde: Option<i64>,
    /// Each privacy setting's last value.
    #[serde(default)]
    pub privacidad: BTreeMap<String, String>,
    /// When the protection watch took its first look.
    #[serde(default)]
    pub proteccion_desde: Option<i64>,
    /// Each protection fact's last state (`av:Microsoft Defender Antivirus` → on).
    #[serde(default)]
    pub proteccion: BTreeMap<String, bool>,
    /// The attention conditions at the last look (`sin_antivirus`, `tiempo_real`…).
    #[serde(default)]
    pub atencion: BTreeSet<String>,
    /// When each notice was last given, so the same one is not repeated within its window.
    #[serde(default)]
    pub avisados: BTreeMap<String, i64>,
}

/// The most startup entries remembered; beyond it, the oldest absent ones are forgotten.
const MAX_RECORDADOS: usize = 6_000;
/// More new startup entries than this in one look (an installation) make one notice, not many.
const MAX_AVISOS_ARRANQUE: usize = 3;
/// A camera or microphone notice for the same program is not repeated within this window: a call
/// that reconnects is one call.
pub const VENTANA_USO: i64 = 30 * MINUTO;
/// Any other notice with the same key, within this window, is history only.
pub const VENTANA_AVISO: i64 = HORA;

fn uso_clave(u: &Uso) -> String {
    format!(
        "{}|{}|{}",
        u.programa.to_lowercase(),
        u.recurso.como_texto(),
        u.desde
    )
}

/// The readable label of a privacy setting, for the chain («Telemetría (directiva)»). The screen
/// uses `vg_aj_<base>` in the person's language.
#[must_use]
pub fn etiqueta_ajuste(base: &str) -> &'static str {
    match base {
        "telemetria_politica" => "Datos de diagnóstico (directiva)",
        "telemetria" => "Datos de diagnóstico",
        "diagtrack_inicio" => "Servicio de telemetría de Windows (inicio)",
        "publicidad" => "Identificador de publicidad",
        "acceso_camara" => "Acceso a la cámara",
        "acceso_microfono" => "Acceso al micrófono",
        "acceso_ubicacion" => "Acceso a la ubicación",
        _ => "Ajuste de privacidad",
    }
}

impl Base {
    /// Whether a notice with this key may be given now; if so, it is remembered.
    pub fn avisa(&mut self, clave: &str, ahora: i64, ventana: i64) -> bool {
        self.avisados.retain(|_, t| ahora - *t < 7 * DIA);
        if self
            .avisados
            .get(clave)
            .is_some_and(|&t| ahora - t < ventana)
        {
            return false;
        }
        self.avisados.insert(clave.to_owned(), ahora);
        true
    }

    /// Looks at the camera and microphone uses. `en_marcha` is the programs running now, to set
    /// aside a use Windows still marks as going on whose program is closed.
    pub fn mira_usos(
        &mut self,
        usos: &[Uso],
        en_marcha: Option<&HashSet<String>>,
        ahora: i64,
    ) -> Vec<Hallazgo> {
        let mut out = Vec::new();
        let primera = self.usos_desde.is_none();
        let anterior = self.usos_ultima;
        let mut vigentes: BTreeMap<String, UsoActivo> = BTreeMap::new();
        for u in usos {
            match u.hasta {
                None => {
                    if !sigue_abierto(u, en_marcha) {
                        continue;
                    }
                    vigentes.insert(
                        uso_clave(u),
                        UsoActivo {
                            programa: u.programa.clone(),
                            recurso: u.recurso,
                            desde: u.desde,
                        },
                    );
                }
                // Started and ended between two looks: a short use, still a use.
                Some(h) if !primera && u.desde > anterior && h >= u.desde => {
                    let clave = uso_clave(u);
                    if self.usos_activos.contains_key(&clave) {
                        continue;
                    }
                    let propio = es_propio(&u.programa);
                    let aviso = !propio
                        && self.avisa(
                            &format!(
                                "uso|{}|{}",
                                u.programa.to_lowercase(),
                                u.recurso.como_texto()
                            ),
                            ahora,
                            VENTANA_USO,
                        );
                    out.push(
                        Hallazgo::nuevo("camara_microfono", clave, aviso, "vg_ev_uso_breve")
                            .con("programa", &u.programa)
                            .con("recurso", u.recurso.como_texto())
                            .con("desde", u.desde)
                            .con("hasta", h)
                            .texto(format!(
                                "{} usó {} un momento",
                                u.programa,
                                u.recurso.nombre()
                            )),
                    );
                }
                Some(_) => {}
            }
        }
        for (clave, a) in &vigentes {
            if self.usos_activos.contains_key(clave) {
                continue;
            }
            let propio = es_propio(&a.programa);
            if primera {
                out.push(
                    Hallazgo::nuevo(
                        "camara_microfono",
                        clave.clone(),
                        false,
                        "vg_ev_uso_al_empezar",
                    )
                    .con("programa", &a.programa)
                    .con("recurso", a.recurso.como_texto())
                    .con("desde", a.desde)
                    .texto(format!(
                        "{} ya estaba usando {} cuando empezó la vigilancia",
                        a.programa,
                        a.recurso.nombre()
                    )),
                );
                continue;
            }
            let aviso = !propio
                && self.avisa(
                    &format!(
                        "uso|{}|{}",
                        a.programa.to_lowercase(),
                        a.recurso.como_texto()
                    ),
                    ahora,
                    VENTANA_USO,
                );
            out.push(
                Hallazgo::nuevo(
                    "camara_microfono",
                    clave.clone(),
                    aviso,
                    "vg_ev_uso_empieza",
                )
                .con("programa", &a.programa)
                .con("recurso", a.recurso.como_texto())
                .con("desde", a.desde)
                .texto(format!(
                    "{} está usando {}",
                    a.programa,
                    a.recurso.nombre()
                )),
            );
        }
        for (clave, a) in &self.usos_activos {
            if vigentes.contains_key(clave) {
                continue;
            }
            // Ended: Windows' stop time when it has one, otherwise the moment it was missed.
            let hasta = usos
                .iter()
                .find(|u| uso_clave(u) == *clave)
                .and_then(|u| u.hasta)
                .unwrap_or(ahora);
            let minutos = (hasta - a.desde).max(0) / MINUTO;
            out.push(
                Hallazgo::nuevo(
                    "camara_microfono",
                    format!("{clave}|fin"),
                    false,
                    "vg_ev_uso_termina",
                )
                .con("programa", &a.programa)
                .con("recurso", a.recurso.como_texto())
                .con("desde", a.desde)
                .con("hasta", hasta)
                .con("minutos", minutos)
                .texto(format!(
                    "{} dejó de usar {} ({minutos} min)",
                    a.programa,
                    a.recurso.nombre()
                )),
            );
        }
        self.usos_activos = vigentes;
        self.usos_ultima = ahora;
        if primera {
            self.usos_desde = Some(ahora);
        }
        out
    }

    /// Looks at what starts on its own. `cargadas` are the accounts whose registry could be read
    /// now: an account that signed out takes its own entries with it, and that is not a removal.
    pub fn mira_arranque(
        &mut self,
        lista: &[Arranque],
        cargadas: &BTreeMap<String, String>,
        ahora: i64,
    ) -> Vec<Hallazgo> {
        let mut out = Vec::new();
        let primera = self.arranque_desde.is_none();
        let mut presentes: BTreeSet<String> = BTreeSet::new();
        let mut nuevos: Vec<&Arranque> = Vec::new();
        for a in lista {
            let clave = a.clave();
            if !presentes.insert(clave.clone()) {
                continue;
            }
            match self.arranque.get_mut(&clave) {
                Some(r) => r.presente = true,
                None => {
                    self.arranque.insert(
                        clave,
                        Recordado {
                            visto: ahora,
                            titulo: a.titulo().chars().take(200).collect(),
                            tipo: a.tipo.clone(),
                            presente: true,
                            sistema: a.es_del_sistema(),
                        },
                    );
                    if !primera {
                        nuevos.push(a);
                    }
                }
            }
        }
        if primera {
            self.arranque_desde = Some(ahora);
            out.push(
                Hallazgo::nuevo(
                    "arranque",
                    format!("base|{ahora}"),
                    false,
                    "vg_ev_arranque_base",
                )
                .con("n", presentes.len())
                .texto(format!(
                    "Punto de partida: {} cosas que arrancan solas anotadas",
                    presentes.len()
                )),
            );
            return out;
        }
        let avisables = nuevos.iter().filter(|a| !a.es_del_sistema()).count();
        let resumen = avisables > MAX_AVISOS_ARRANQUE;
        for a in &nuevos {
            let del_sistema = a.es_del_sistema();
            let clave = a.clave();
            let aviso = !del_sistema && !resumen && self.avisa(&clave, ahora, VENTANA_AVISO);
            out.push(
                Hallazgo::nuevo("arranque", clave, aviso, "vg_ev_arranque_nuevo")
                    .con("x", a.titulo())
                    .con("tipo", &a.tipo)
                    .con("ruta", &a.ruta)
                    .con("cuenta", cuenta_de(&a.cuenta, cargadas))
                    .con("sistema", del_sistema)
                    .texto(format!(
                        "Nuevo programa que arranca solo: {} ({}{})",
                        a.titulo(),
                        donde_texto(&a.tipo),
                        if a.ruta.is_empty() {
                            String::new()
                        } else {
                            format!(" · {}", a.ruta)
                        }
                    )),
            );
        }
        if resumen && self.avisa("arranque|varios", ahora, VENTANA_AVISO) {
            out.push(
                Hallazgo::nuevo(
                    "arranque",
                    format!("varios|{ahora}"),
                    true,
                    "vg_ev_arranque_varios",
                )
                .con("n", avisables)
                .texto(format!(
                    "{avisables} programas nuevos que arrancan solos, a la vez (una instalación)"
                )),
            );
        }
        // What is gone, where it could be read (its account is signed in).
        let mut idos = Vec::new();
        for (clave, r) in &mut self.arranque {
            if !r.presente || presentes.contains(clave) {
                continue;
            }
            let legible = cuenta_de_clave(clave)
                .is_none_or(|sid| cargadas.keys().any(|k| k.eq_ignore_ascii_case(sid)));
            if !legible {
                continue;
            }
            r.presente = false;
            // RunOnce entries go away by design once they ran.
            if r.tipo != "runonce" {
                idos.push(
                    Hallazgo::nuevo(
                        "arranque",
                        format!("{clave}|ido|{ahora}"),
                        false,
                        "vg_ev_arranque_ido",
                    )
                    .con("x", &r.titulo)
                    .con("tipo", &r.tipo)
                    .texto(format!("Ya no arranca solo: {}", r.titulo)),
                );
            }
        }
        out.extend(idos);
        if self.arranque.len() > MAX_RECORDADOS {
            let mut viejos: Vec<(i64, String)> = self
                .arranque
                .iter()
                .filter(|(_, r)| !r.presente)
                .map(|(k, r)| (r.visto, k.clone()))
                .collect();
            viejos.sort();
            let sobra = self.arranque.len() - MAX_RECORDADOS;
            for (_, k) in viejos.into_iter().take(sobra) {
                self.arranque.remove(&k);
            }
        }
        out
    }

    /// Looks at the privacy settings. A setting Windows did not give this time (an account that
    /// signed out) is left as it was.
    pub fn mira_privacidad(
        &mut self,
        actual: &BTreeMap<String, String>,
        cargadas: &BTreeMap<String, String>,
        ahora: i64,
    ) -> Vec<Hallazgo> {
        let mut out = Vec::new();
        if self.privacidad_desde.is_none() {
            self.privacidad_desde = Some(ahora);
            self.privacidad = actual.clone();
            out.push(
                Hallazgo::nuevo(
                    "privacidad",
                    format!("base|{ahora}"),
                    false,
                    "vg_ev_privacidad_base",
                )
                .con("n", actual.len())
                .texto(format!(
                    "Punto de partida: {} ajustes de privacidad anotados",
                    actual.len()
                )),
            );
            return out;
        }
        for (k, v) in actual {
            let antes = self.privacidad.insert(k.clone(), v.clone());
            let Some(antes) = antes else {
                // First time this setting is readable (a new account): its starting point.
                continue;
            };
            if antes == *v {
                continue;
            }
            let (base, sid) = k
                .split_once(':')
                .map_or((k.as_str(), None), |(b, s)| (b, Some(s)));
            let cuenta = sid.map(|s| cuenta_de(s, cargadas)).unwrap_or_default();
            let aviso = self.avisa(&format!("privacidad|{k}|{v}"), ahora, VENTANA_AVISO);
            out.push(
                Hallazgo::nuevo(
                    "privacidad",
                    format!("{k}|{ahora}"),
                    aviso,
                    "vg_ev_privacidad_cambio",
                )
                .con("ajuste", base)
                .con("cuenta", &cuenta)
                .con("antes", &antes)
                .con("despues", v)
                .texto(format!(
                    "Cambió un ajuste de privacidad: {}{} ({} → {})",
                    etiqueta_ajuste(base),
                    if cuenta.is_empty() {
                        String::new()
                    } else {
                        format!(", cuenta {cuenta}")
                    },
                    antes,
                    v
                )),
            );
        }
        out
    }

    /// Looks at the antivirus and the firewall.
    pub fn mira_proteccion(&mut self, p: &Proteccion, ahora: i64) -> Vec<Hallazgo> {
        let mut out = Vec::new();
        let primera = self.proteccion_desde.is_none();
        let hechos = hechos_proteccion(p);
        let atencion: BTreeSet<String> = atencion_proteccion(p).into_iter().collect();
        if !primera {
            // The plain on/off of each product or profile, as history.
            for (k, &v) in &hechos {
                match self.proteccion.get(k) {
                    Some(&antes) if antes != v => out.push(
                        Hallazgo::nuevo(
                            "proteccion",
                            format!("{k}|{v}|{ahora}"),
                            false,
                            "vg_ev_prot_cambio",
                        )
                        .con("x", nombre_hecho(k))
                        .con("activo", v)
                        .texto(format!(
                            "{}: {}",
                            nombre_hecho(k),
                            if v { "activado" } else { "desactivado" }
                        )),
                    ),
                    None => out.push(
                        Hallazgo::nuevo(
                            "proteccion",
                            format!("{k}|nuevo|{ahora}"),
                            false,
                            "vg_ev_prot_nuevo",
                        )
                        .con("x", nombre_hecho(k))
                        .con("activo", v)
                        .texto(format!(
                            "Aparece en Seguridad de Windows: {} ({})",
                            nombre_hecho(k),
                            if v { "activado" } else { "desactivado" }
                        )),
                    ),
                    _ => {}
                }
            }
        }
        // What matters: a condition of attention that starts, or that was already there.
        for c in &atencion {
            if self.atencion.contains(c) {
                continue;
            }
            let aviso = self.avisa(&format!("proteccion|{c}"), ahora, VENTANA_AVISO);
            let (frase, texto) = frase_atencion(c, primera);
            out.push(
                Hallazgo::nuevo("proteccion", format!("{c}|apagada|{ahora}"), aviso, frase)
                    .con("x", detalle_atencion(c))
                    .texto(texto),
            );
        }
        for c in &self.atencion {
            if atencion.contains(c) {
                continue;
            }
            out.push(
                Hallazgo::nuevo(
                    "proteccion",
                    format!("{c}|vuelve|{ahora}"),
                    false,
                    frase_vuelve(c),
                )
                .con("x", detalle_atencion(c))
                .texto(format!("{} vuelve a estar activo", texto_atencion(c))),
            );
        }
        self.proteccion = hechos;
        self.atencion = atencion;
        if primera {
            self.proteccion_desde = Some(ahora);
        }
        out
    }
}

fn sigue_abierto(u: &Uso, en_marcha: Option<&HashSet<String>>) -> bool {
    // Only a program with a path can be matched to a running one; a Store app is taken at
    // Windows' word.
    match (en_marcha, &u.ruta) {
        (Some(m), Some(_)) if !m.is_empty() => m.contains(&u.programa.to_lowercase()),
        _ => true,
    }
}

/// The account's user name when known, otherwise empty for the computer and the SID as it is.
fn cuenta_de(cuenta: &str, cargadas: &BTreeMap<String, String>) -> String {
    if cuenta == "equipo" || cuenta.is_empty() {
        return String::new();
    }
    cargadas
        .get(cuenta)
        .cloned()
        .unwrap_or_else(|| cuenta.to_owned())
}

/// The SID inside a startup key (`run|hkey_users\s-1-5-21-…\software\…|name`), lowercased as the
/// key is, if it is an account's.
fn cuenta_de_clave(clave: &str) -> Option<&str> {
    let i = clave.find("hkey_users\\")?;
    clave[i + "hkey_users\\".len()..].split('\\').next()
}

fn donde_texto(tipo: &str) -> &'static str {
    match tipo {
        "run" => "clave Run del registro",
        "runonce" => "clave RunOnce del registro",
        "carpeta" => "carpeta Inicio",
        "tarea" => "tarea programada",
        "servicio" => "servicio",
        "launchd" => "LaunchAgents/LaunchDaemons",
        "autostart" => "inicio automático del escritorio",
        "systemd" => "servicio de systemd",
        _ => "arranque",
    }
}

/// The plain facts: each product and profile, on or off.
fn hechos_proteccion(p: &Proteccion) -> BTreeMap<String, bool> {
    let mut h = BTreeMap::new();
    for a in p.antivirus.iter().flatten() {
        h.insert(format!("av:{}", a.nombre), a.activo);
    }
    for a in p.cortafuegos_otros.iter().flatten() {
        h.insert(format!("fw:{}", a.nombre), a.activo);
    }
    if let Some(d) = &p.defender {
        if d.al_mando() {
            h.insert("defender_tiempo_real".to_owned(), d.tiempo_real);
            h.insert("defender_manipulacion".to_owned(), d.manipulacion);
        }
    }
    for (perfil, on) in p.cortafuegos.iter().flatten() {
        h.insert(format!("perfil:{perfil}"), *on);
    }
    h
}

/// The conditions that deserve attention now. Only what is really off: an antivirus standing
/// aside for another one that is on is not «off».
#[must_use]
pub fn atencion_proteccion(p: &Proteccion) -> Vec<String> {
    let mut a = Vec::new();
    let defender_sin_tiempo_real = p
        .defender
        .as_ref()
        .is_some_and(|d| d.al_mando() && !d.tiempo_real);
    if defender_sin_tiempo_real {
        a.push("tiempo_real".to_owned());
    }
    let hay_antivirus = match &p.antivirus {
        Some(l) if !l.is_empty() => l.iter().any(|x| x.activo),
        // No list from Security Center: Defender's own word, when it gave it.
        _ => p
            .defender
            .as_ref()
            .is_none_or(|d| d.antivirus && d.tiempo_real),
    };
    if !hay_antivirus && !defender_sin_tiempo_real {
        a.push("sin_antivirus".to_owned());
    }
    let otro_cortafuegos = p.cortafuegos_otros.iter().flatten().any(|x| x.activo);
    if !otro_cortafuegos {
        for (perfil, on) in p.cortafuegos.iter().flatten() {
            if !on {
                a.push(format!("cortafuegos:{perfil}"));
            }
        }
    }
    a
}

fn nombre_hecho(k: &str) -> String {
    if let Some(n) = k.strip_prefix("av:").or_else(|| k.strip_prefix("fw:")) {
        return n.to_owned();
    }
    if let Some(p) = k.strip_prefix("perfil:") {
        return format!("Cortafuegos de Windows, perfil {p}");
    }
    match k {
        "defender_tiempo_real" => "Protección en tiempo real de Microsoft Defender".to_owned(),
        "defender_manipulacion" => {
            "Protección contra alteraciones de Microsoft Defender".to_owned()
        }
        otro => otro.to_owned(),
    }
}

fn detalle_atencion(c: &str) -> String {
    c.strip_prefix("cortafuegos:")
        .unwrap_or_default()
        .to_owned()
}

fn texto_atencion(c: &str) -> String {
    if let Some(p) = c.strip_prefix("cortafuegos:") {
        return format!("El cortafuegos (perfil {p})");
    }
    match c {
        "tiempo_real" => "La protección en tiempo real de Microsoft Defender".to_owned(),
        _ => "El antivirus".to_owned(),
    }
}

fn frase_atencion(c: &str, primera: bool) -> (&'static str, String) {
    let (frase, que) = if c.starts_with("cortafuegos:") {
        (
            if primera {
                "vg_ev_cortafuegos_ya"
            } else {
                "vg_ev_cortafuegos_apagado"
            },
            texto_atencion(c),
        )
    } else if c == "tiempo_real" {
        (
            if primera {
                "vg_ev_tiempo_real_ya"
            } else {
                "vg_ev_tiempo_real_apagado"
            },
            texto_atencion(c),
        )
    } else {
        (
            if primera {
                "vg_ev_sin_antivirus_ya"
            } else {
                "vg_ev_sin_antivirus"
            },
            "Ningún antivirus tiene la protección encendida".to_owned(),
        )
    };
    let texto = if c == "sin_antivirus" {
        if primera {
            format!("{que} (ya era así al empezar la vigilancia)")
        } else {
            que
        }
    } else if primera {
        format!("{que} está desactivado (ya era así al empezar la vigilancia)")
    } else {
        format!("{que} se ha desactivado")
    };
    (frase, texto)
}

fn frase_vuelve(c: &str) -> &'static str {
    if c.starts_with("cortafuegos:") {
        "vg_ev_cortafuegos_vuelve"
    } else if c == "tiempo_real" {
        "vg_ev_tiempo_real_vuelve"
    } else {
        "vg_ev_sin_antivirus_vuelve"
    }
}

/// Every sentence a finding can carry (keys of the panel's texts), so a test there can check that
/// each one exists in the three languages.
pub const FRASES: &[&str] = &[
    "vg_ev_uso_empieza",
    "vg_ev_uso_termina",
    "vg_ev_uso_breve",
    "vg_ev_uso_al_empezar",
    "vg_ev_arranque_base",
    "vg_ev_arranque_nuevo",
    "vg_ev_arranque_varios",
    "vg_ev_arranque_ido",
    "vg_ev_privacidad_base",
    "vg_ev_privacidad_cambio",
    "vg_ev_prot_cambio",
    "vg_ev_prot_nuevo",
    "vg_ev_tiempo_real_apagado",
    "vg_ev_tiempo_real_ya",
    "vg_ev_tiempo_real_vuelve",
    "vg_ev_sin_antivirus",
    "vg_ev_sin_antivirus_ya",
    "vg_ev_sin_antivirus_vuelve",
    "vg_ev_cortafuegos_apagado",
    "vg_ev_cortafuegos_ya",
    "vg_ev_cortafuegos_vuelve",
];

/// The privacy settings watched (the part before `:` of each key), for the same check.
pub const AJUSTES: &[&str] = &[
    "telemetria_politica",
    "telemetria",
    "diagtrack_inicio",
    "publicidad",
    "acceso_camara",
    "acceso_microfono",
    "acceso_ubicacion",
];

/// Every kind of startup entry, for the same check.
pub const TIPOS_ARRANQUE: &[&str] = &[
    "run",
    "runonce",
    "carpeta",
    "tarea",
    "servicio",
    "launchd",
    "autostart",
    "systemd",
];

// ---------------------------------------------------------------------------------------------
// Windows.

#[cfg(windows)]
#[allow(unsafe_code)]
mod win {
    //! The registry, read directly: `unsafe` is denied in this project unless justified, and this
    //! is the justification. A PowerShell every fifteen seconds to read a dozen numbers would cost
    //! more than everything else the watch does; these are Windows' own registry functions, with
    //! read-only access, each handle closed when it goes out of scope, every buffer passed with
    //! its length and cut to what Windows says it wrote. Anything that fails reads as nothing.

    use std::collections::HashSet;

    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_SUCCESS, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE,
        HKEY_USERS, KEY_READ, KEY_WOW64_64KEY, REG_QWORD,
    };

    use super::{de_registro, Recurso, Uso};
    use crate::Error;

    fn ancho(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    struct Clave(HKEY);

    impl Drop for Clave {
        fn drop(&mut self) {
            // SAFETY: the handle was opened by `abre` and is closed once, here.
            unsafe {
                RegCloseKey(self.0);
            }
        }
    }

    fn abre(raiz: HKEY, ruta: &str) -> Option<Clave> {
        let r = ancho(ruta);
        let mut h: HKEY = std::ptr::null_mut();
        // SAFETY: `r` is a NUL-terminated wide string that outlives the call; `h` is ours.
        let e =
            unsafe { RegOpenKeyExW(raiz, r.as_ptr(), 0, KEY_READ | KEY_WOW64_64KEY, &raw mut h) };
        (e == ERROR_SUCCESS && !h.is_null()).then_some(Clave(h))
    }

    impl Clave {
        fn subclaves(&self) -> Vec<String> {
            let mut v = Vec::new();
            for i in 0..20_000u32 {
                // Registry key names are at most 255 characters.
                let mut buf = [0u16; 512];
                let mut largo = u32::try_from(buf.len()).unwrap_or(0);
                // SAFETY: `buf` is ours and `largo` carries its size; the optional pointers are null.
                let e = unsafe {
                    RegEnumKeyExW(
                        self.0,
                        i,
                        buf.as_mut_ptr(),
                        &raw mut largo,
                        std::ptr::null(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    )
                };
                if e != ERROR_SUCCESS {
                    break;
                }
                let n = (largo as usize).min(buf.len());
                v.push(String::from_utf16_lossy(&buf[..n]));
            }
            v
        }

        fn qword(&self, nombre: &str) -> u64 {
            let n = ancho(nombre);
            let mut v: u64 = 0;
            let mut tipo: u32 = 0;
            let mut largo: u32 = 8;
            // SAFETY: `v` is eight bytes and `largo` says so; Windows writes at most that.
            let e = unsafe {
                RegQueryValueExW(
                    self.0,
                    n.as_ptr(),
                    std::ptr::null(),
                    &raw mut tipo,
                    (&raw mut v).cast::<u8>(),
                    &raw mut largo,
                )
            };
            if e == ERROR_SUCCESS && tipo == REG_QWORD && largo == 8 {
                v
            } else {
                0
            }
        }
    }

    /// The signed-in accounts (their registry is loaded under HKEY_USERS).
    fn cuentas() -> Vec<String> {
        abre(HKEY_USERS, "")
            .map(|k| k.subclaves())
            .unwrap_or_default()
            .into_iter()
            .filter(|s| s.starts_with("S-1-5-21-") && !s.ends_with("_Classes"))
            .collect()
    }

    pub(super) fn usos() -> Vec<Uso> {
        let mut v = Vec::new();
        let mut raices: Vec<(HKEY, String, String)> = cuentas()
            .into_iter()
            .map(|sid| (HKEY_USERS, format!("{sid}\\Software"), sid))
            .collect();
        raices.push((
            HKEY_LOCAL_MACHINE,
            "SOFTWARE".to_owned(),
            "equipo".to_owned(),
        ));
        for (raiz, prefijo, cuenta) in &raices {
            for (cap, recurso) in [
                ("webcam", Recurso::Camara),
                ("microphone", Recurso::Microfono),
            ] {
                let base = format!(
                    "{prefijo}\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\{cap}"
                );
                let Some(k) = abre(*raiz, &base) else {
                    continue;
                };
                for sub in k.subclaves() {
                    if sub.eq_ignore_ascii_case("NonPackaged") {
                        let np = format!("{base}\\NonPackaged");
                        let Some(knp) = abre(*raiz, &np) else {
                            continue;
                        };
                        for app in knp.subclaves() {
                            if let Some(ka) = abre(*raiz, &format!("{np}\\{app}")) {
                                let (i, f) =
                                    (ka.qword("LastUsedTimeStart"), ka.qword("LastUsedTimeStop"));
                                v.extend(de_registro(&app, recurso, i, f, cuenta));
                            }
                        }
                    } else if let Some(ka) = abre(*raiz, &format!("{base}\\{sub}")) {
                        let (i, f) = (ka.qword("LastUsedTimeStart"), ka.qword("LastUsedTimeStop"));
                        v.extend(de_registro(&sub, recurso, i, f, cuenta));
                    }
                }
            }
        }
        v
    }

    pub(super) fn en_marcha() -> Option<HashSet<String>> {
        // SAFETY: a snapshot of the system's processes; the handle is checked and closed below.
        let foto = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if foto == INVALID_HANDLE_VALUE || foto.is_null() {
            return None;
        }
        let mut out = HashSet::new();
        let mut e = PROCESSENTRY32W {
            dwSize: u32::try_from(std::mem::size_of::<PROCESSENTRY32W>()).unwrap_or(0),
            ..PROCESSENTRY32W::default()
        };
        // SAFETY: `e` carries its size in dwSize and the snapshot is valid.
        let mut hay = unsafe { Process32FirstW(foto, &raw mut e) } != 0;
        while hay {
            let largo = e
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(e.szExeFile.len());
            out.insert(
                String::from_utf16_lossy(e.szExeFile.get(..largo).unwrap_or(&[])).to_lowercase(),
            );
            // SAFETY: as above.
            hay = unsafe { Process32NextW(foto, &raw mut e) } != 0;
        }
        // SAFETY: the snapshot was opened here and is not used after.
        unsafe { CloseHandle(foto) };
        Some(out)
    }

    pub(super) fn foto() -> Result<super::Foto, Error> {
        use std::io::{Read as _, Write as _};
        use std::os::windows::process::CommandExt;
        use std::process::{Command, Stdio};
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
        let raiz = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_owned());
        // A 32-bit GUARDIANA on a 64-bit Windows would get the 32-bit PowerShell: «Sysnative».
        let carpeta = if std::env::var_os("PROCESSOR_ARCHITEW6432").is_some() {
            "Sysnative"
        } else {
            "System32"
        };
        let exe = format!("{raiz}\\{carpeta}\\WindowsPowerShell\\v1.0\\powershell.exe");
        let mut hijo = Command::new(exe)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "& ([scriptblock]::Create([Console]::In.ReadToEnd()))",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS)
            .spawn()
            .map_err(|e| Error::Sistema(e.to_string()))?;
        if let Some(mut entrada) = hijo.stdin.take() {
            entrada
                .write_all(super::SCRIPT.as_bytes())
                .map_err(|e| Error::Sistema(e.to_string()))?;
        }
        let mut salida = hijo.stdout.take();
        let lector = std::thread::spawn(move || {
            let mut v = Vec::new();
            if let Some(s) = salida.as_mut() {
                let _ = s.read_to_end(&mut v);
            }
            v
        });
        let inicio = std::time::Instant::now();
        loop {
            match hijo.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if inicio.elapsed() > super::LIMITE_SCRIPT => {
                    let _ = hijo.kill();
                    let _ = hijo.wait();
                    return Err(Error::Sistema(format!(
                        "la comprobación tardó más de {} s y se paró",
                        super::LIMITE_SCRIPT.as_secs()
                    )));
                }
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(200)),
                Err(e) => return Err(Error::Sistema(e.to_string())),
            }
        }
        let bytes = lector.join().unwrap_or_default();
        let texto = String::from_utf8_lossy(&bytes);
        let texto = texto.trim().trim_start_matches('\u{feff}');
        let v: serde_json::Value = serde_json::from_str(texto)
            .map_err(|e| Error::Sistema(format!("respuesta ilegible: {e}")))?;
        Ok(super::de_json(&v))
    }
}

// ---------------------------------------------------------------------------------------------
// macOS and Linux: folders and one command, nothing more.

/// The files in a folder (not recursive), as startup entries of one kind.
#[cfg_attr(windows, allow(dead_code))]
fn de_carpeta(dir: &std::path::Path, tipo: &str, cuenta: &str, ext: &[&str]) -> Vec<Arranque> {
    let Ok(lee) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut v: Vec<Arranque> = lee
        .flatten()
        .filter_map(|e| {
            let nombre = e.file_name().to_string_lossy().into_owned();
            let vale =
                ext.is_empty() || ext.iter().any(|x| nombre.to_ascii_lowercase().ends_with(x));
            (vale && !nombre.starts_with('.')).then(|| Arranque {
                tipo: tipo.to_owned(),
                donde: dir.display().to_string(),
                nombre,
                visible: String::new(),
                ruta: e.path().display().to_string(),
                cuenta: cuenta.to_owned(),
            })
        })
        .take(2_000)
        .collect();
    v.sort_by(|a, b| a.nombre.cmp(&b.nombre));
    v
}

/// The home folders under `raiz` (`/Users`, `/home`), with their user names.
#[cfg_attr(windows, allow(dead_code))]
fn casas(raiz: &str) -> Vec<(String, std::path::PathBuf)> {
    std::fs::read_dir(raiz)
        .map(|l| {
            l.flatten()
                .filter(|e| e.path().is_dir())
                .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
                .filter(|(n, _)| !n.starts_with('.') && n != "Shared")
                .take(64)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(target_os = "macos")]
mod mac {
    use std::path::Path;

    use super::{casas, de_carpeta, Foto, Proteccion};

    /// `socketfilterfw --getglobalstate`: «Firewall is enabled. (State = 1)» or «… disabled».
    pub(super) fn lee_cortafuegos(texto: &str) -> Option<bool> {
        let t = texto.to_ascii_lowercase();
        if t.contains("disabled") || t.contains("state = 0") {
            Some(false)
        } else if t.contains("enabled") || t.contains("state = 1") || t.contains("state = 2") {
            Some(true)
        } else {
            None
        }
    }

    pub(super) fn foto() -> Foto {
        let mut l = Vec::new();
        for d in ["/Library/LaunchAgents", "/Library/LaunchDaemons"] {
            l.extend(de_carpeta(Path::new(d), "launchd", "equipo", &[".plist"]));
        }
        for (usuario, casa) in casas("/Users") {
            l.extend(de_carpeta(
                &casa.join("Library/LaunchAgents"),
                "launchd",
                &usuario,
                &[".plist"],
            ));
        }
        let salida = std::process::Command::new("/usr/libexec/ApplicationFirewall/socketfilterfw")
            .arg("--getglobalstate")
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
        let cortafuegos = salida
            .as_deref()
            .and_then(lee_cortafuegos)
            .map(|on| vec![("macOS".to_owned(), on)]);
        Foto {
            arranque: Some(l),
            privacidad: None,
            proteccion: cortafuegos.map(|c| Proteccion {
                // macOS has no antivirus switch to read (XProtect cannot be turned off): only
                // the firewall is watched, and the screen says so.
                antivirus: Some(Vec::new()),
                cortafuegos_otros: None,
                defender: None,
                cortafuegos: Some(c),
            }),
            ..Foto::default()
        }
    }
}

#[cfg(all(not(windows), not(target_os = "macos")))]
mod linux {
    use std::path::Path;

    use super::{casas, de_carpeta, Foto};

    pub(super) fn foto() -> Foto {
        let mut l = de_carpeta(
            Path::new("/etc/xdg/autostart"),
            "autostart",
            "equipo",
            &[".desktop"],
        );
        for (usuario, casa) in casas("/home") {
            l.extend(de_carpeta(
                &casa.join(".config/autostart"),
                "autostart",
                &usuario,
                &[".desktop"],
            ));
        }
        // What systemd starts: the links in each `*.wants` folder.
        if let Ok(lee) = std::fs::read_dir("/etc/systemd/system") {
            for e in lee.flatten() {
                let n = e.file_name().to_string_lossy().into_owned();
                if n.ends_with(".wants") && e.path().is_dir() {
                    l.extend(de_carpeta(&e.path(), "systemd", "equipo", &[]));
                }
            }
        }
        Foto {
            arranque: Some(l),
            ..Foto::default()
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn ft(unix_ms: i64) -> u64 {
        (u64::try_from(unix_ms).unwrap() + 11_644_473_600_000) * 10_000
    }

    fn uso(p: &str, r: Recurso, desde: i64, hasta: Option<i64>) -> Uso {
        Uso {
            programa: p.to_owned(),
            ruta: Some(format!("C:\\x\\{p}")),
            recurso: r,
            desde,
            hasta,
            cuenta: "S-1-5-21-1".to_owned(),
        }
    }

    fn entrada(tipo: &str, donde: &str, nombre: &str, ruta: &str) -> Arranque {
        Arranque {
            tipo: tipo.into(),
            donde: donde.into(),
            nombre: nombre.into(),
            visible: String::new(),
            ruta: ruta.into(),
            cuenta: "equipo".into(),
        }
    }

    #[test]
    fn registry_entries_become_named_uses_in_milliseconds() {
        let u = de_registro(
            "C:#Program Files#Zoom#bin#Zoom.exe",
            Recurso::Microfono,
            ft(1_000_000),
            0,
            "S-1-5-21-9",
        )
        .unwrap();
        assert_eq!(u.programa, "Zoom.exe");
        assert_eq!((u.desde, u.hasta), (1_000_000, None));
        assert_eq!(u.cuenta, "S-1-5-21-9");
        let s = de_registro(
            "Microsoft.WindowsCamera_8wekyb3d8bbwe",
            Recurso::Camara,
            ft(5_000),
            ft(9_000),
            "equipo",
        )
        .unwrap();
        assert_eq!(
            (s.programa.as_str(), s.hasta),
            ("Microsoft.WindowsCamera", Some(9_000))
        );
        assert!(de_registro("C:#x.exe", Recurso::Camara, 0, 0, "equipo").is_none());
        let otra = de_registro("C:#y.exe", Recurso::Camara, ft(20_000), ft(10_000), "e").unwrap();
        assert_eq!(otra.hasta, None, "started again after its last stop");
    }

    #[test]
    fn a_use_is_noticed_once_when_it_starts_and_written_when_it_ends() {
        let mut b = Base::default();
        // First look: nothing in use, the watch starts.
        assert!(b.mira_usos(&[], None, 1_000).is_empty());
        let zoom = uso("Zoom.exe", Recurso::Microfono, 2_000, None);
        let h = b.mira_usos(std::slice::from_ref(&zoom), None, 3_000);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].frase, "vg_ev_uso_empieza");
        assert!(h[0].aviso);
        assert_eq!(h[0].texto, "Zoom.exe está usando el micrófono");
        // Still in use: nothing new.
        assert!(b
            .mira_usos(std::slice::from_ref(&zoom), None, 4_000)
            .is_empty());
        // It stops: history only.
        let fin = uso(
            "Zoom.exe",
            Recurso::Microfono,
            2_000,
            Some(2_000 + 5 * MINUTO),
        );
        let h = b.mira_usos(std::slice::from_ref(&fin), None, 3_000 + 5 * MINUTO);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].frase, "vg_ev_uso_termina");
        assert!(!h[0].aviso);
        assert_eq!(h[0].datos["minutos"], "5");
        // A reconnect ten minutes later is written, but not noticed again within the window.
        let otra = uso("Zoom.exe", Recurso::Microfono, 15 * MINUTO, None);
        let h = b.mira_usos(&[otra], None, 16 * MINUTO);
        assert_eq!(h.len(), 1);
        assert!(!h[0].aviso, "same program and device within 30 minutes");
    }

    #[test]
    fn short_uses_between_two_looks_count_and_old_ones_do_not() {
        let mut b = Base::default();
        let viejo = uso("viejo.exe", Recurso::Camara, 10, Some(20));
        assert!(b
            .mira_usos(std::slice::from_ref(&viejo), None, 1_000)
            .is_empty());
        let breve = uso("Snap.exe", Recurso::Camara, 1_500, Some(1_600));
        let h = b.mira_usos(&[viejo.clone(), breve.clone()], None, 2_000);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].frase, "vg_ev_uso_breve");
        assert!(b.mira_usos(&[viejo, breve], None, 3_000).is_empty());
    }

    #[test]
    fn uses_on_at_the_first_look_and_guardianas_own_are_not_notices() {
        let mut b = Base::default();
        let h = b.mira_usos(&[uso("Teams.exe", Recurso::Camara, 10, None)], None, 1_000);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].frase, "vg_ev_uso_al_empezar");
        assert!(!h[0].aviso);
        let h = b.mira_usos(
            &[
                uso("Teams.exe", Recurso::Camara, 10, None),
                uso("guardiana-zero.exe", Recurso::Microfono, 1_500, None),
            ],
            None,
            2_000,
        );
        assert_eq!(h.len(), 1);
        assert!(!h[0].aviso, "GUARDIANA's own call is history, not a notice");
    }

    #[test]
    fn a_use_whose_program_is_closed_is_not_in_use() {
        let mut b = Base::default();
        b.mira_usos(&[], None, 1_000);
        let colgado = uso("Cerrado.exe", Recurso::Microfono, 1_500, None);
        let abiertos: HashSet<String> = ["explorer.exe".to_owned()].into_iter().collect();
        assert!(b.mira_usos(&[colgado], Some(&abiertos), 2_000).is_empty());
        assert!(b.usos_activos.is_empty());
    }

    #[test]
    fn the_first_startup_look_is_the_starting_point_and_later_ones_say_whats_new() {
        let mut b = Base::default();
        let cargadas = BTreeMap::new();
        let mut l = vec![
            entrada("run", "HKLM:\\SOFTWARE\\X\\Run", "OneDrive", "C:\\o.exe"),
            entrada(
                "servicio",
                "Auto",
                "Spooler",
                "C:\\Windows\\System32\\spoolsv.exe",
            ),
        ];
        let h = b.mira_arranque(&l, &cargadas, 1_000);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].frase, "vg_ev_arranque_base");
        assert!(!h[0].aviso);
        assert!(b.mira_arranque(&l, &cargadas, 2_000).is_empty());
        l.push(entrada("tarea", "\\", "Actualizador", "C:\\u\\a.exe"));
        l.push(entrada(
            "servicio",
            "Auto",
            "NuevoWin",
            "C:\\WINDOWS\\system32\\svchost.exe -k x",
        ));
        let h = b.mira_arranque(&l, &cargadas, 3_000);
        assert_eq!(h.len(), 2);
        let tarea = h.iter().find(|x| x.datos["x"] == "Actualizador").unwrap();
        assert!(tarea.aviso);
        assert!(tarea
            .texto
            .starts_with("Nuevo programa que arranca solo: Actualizador"));
        let win = h.iter().find(|x| x.datos["x"] == "NuevoWin").unwrap();
        assert!(
            !win.aviso,
            "a service in the Windows folder is history only"
        );
        // The start mode of a service changing is not a new service.
        l[1].donde = "Manual".into();
        assert!(b.mira_arranque(&l, &cargadas, 4_000).is_empty());
        // Gone: written once.
        l.remove(0);
        let h = b.mira_arranque(&l, &cargadas, 5_000);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].frase, "vg_ev_arranque_ido");
        assert!(b.mira_arranque(&l, &cargadas, 6_000).is_empty());
        // Back again: known, silent.
        l.push(entrada(
            "run",
            "HKLM:\\SOFTWARE\\X\\Run",
            "OneDrive",
            "C:\\o2.exe",
        ));
        assert!(b.mira_arranque(&l, &cargadas, 7_000).is_empty());
    }

    #[test]
    fn an_installation_is_one_notice_and_a_signed_out_account_is_not_a_removal() {
        let mut b = Base::default();
        let sid = "S-1-5-21-7".to_owned();
        let mut cargadas: BTreeMap<String, String> = [(sid.clone(), "fran".to_owned())].into();
        let mut cuenta = entrada(
            "run",
            &format!("HKEY_USERS\\{sid}\\Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            "Spotify",
            "s.exe",
        );
        cuenta.cuenta = sid.clone();
        b.mira_arranque(std::slice::from_ref(&cuenta), &cargadas, 1_000);
        let mut l = vec![cuenta.clone()];
        for i in 0..6 {
            l.push(entrada("tarea", "\\Suite\\", &format!("t{i}"), "x.exe"));
        }
        let h = b.mira_arranque(&l, &cargadas, 2_000);
        assert_eq!(h.iter().filter(|x| x.aviso).count(), 1);
        assert_eq!(
            h.iter().find(|x| x.aviso).unwrap().frase,
            "vg_ev_arranque_varios"
        );
        assert_eq!(h.len(), 7);
        // The account signs out: its entries cannot be read, and nothing is «gone».
        cargadas.clear();
        let h = b.mira_arranque(&l[1..], &cargadas, 3_000);
        assert!(h.is_empty(), "{h:?}");
    }

    #[test]
    fn a_changed_privacy_setting_says_which_and_from_what_to_what() {
        let mut b = Base::default();
        let cargadas: BTreeMap<String, String> =
            [("S-1-5-21-7".to_owned(), "fran".to_owned())].into();
        let mut a: BTreeMap<String, String> = [
            ("telemetria_politica".to_owned(), "sin_fijar".to_owned()),
            ("publicidad:S-1-5-21-7".to_owned(), "0".to_owned()),
        ]
        .into();
        let h = b.mira_privacidad(&a, &cargadas, 1_000);
        assert_eq!(h[0].frase, "vg_ev_privacidad_base");
        assert!(b.mira_privacidad(&a, &cargadas, 2_000).is_empty());
        a.insert("publicidad:S-1-5-21-7".into(), "1".into());
        let h = b.mira_privacidad(&a, &cargadas, 3_000);
        assert_eq!(h.len(), 1);
        assert!(h[0].aviso);
        assert_eq!(h[0].datos["ajuste"], "publicidad");
        assert_eq!(h[0].datos["cuenta"], "fran");
        assert_eq!(
            (h[0].datos["antes"].as_str(), h[0].datos["despues"].as_str()),
            ("0", "1")
        );
        assert_eq!(
            h[0].texto,
            "Cambió un ajuste de privacidad: Identificador de publicidad, cuenta fran (0 → 1)"
        );
        // Not read this time (signed out): left as it was, no change.
        a.remove("publicidad:S-1-5-21-7");
        assert!(b.mira_privacidad(&a, &cargadas, 4_000).is_empty());
        // Back and forth within the hour: both written, the second one not a notice.
        a.insert("telemetria_politica".into(), "0".into());
        assert!(b.mira_privacidad(&a, &cargadas, 5_000)[0].aviso);
        a.insert("telemetria_politica".into(), "sin_fijar".into());
        assert!(
            b.mira_privacidad(&a, &cargadas, 6_000)[0].aviso,
            "a different value"
        );
        a.insert("telemetria_politica".into(), "0".into());
        assert!(
            !b.mira_privacidad(&a, &cargadas, 7_000)[0].aviso,
            "the same change again"
        );
    }

    fn defender(tiempo_real: bool, modo: &str) -> Defender {
        Defender {
            servicio: true,
            antivirus: tiempo_real,
            tiempo_real,
            manipulacion: true,
            modo: modo.into(),
        }
    }

    fn prot(av: &[(&str, bool)], d: Defender, perfiles: &[(&str, bool)]) -> Proteccion {
        Proteccion {
            antivirus: Some(
                av.iter()
                    .map(|(n, a)| Producto {
                        nombre: (*n).into(),
                        activo: *a,
                    })
                    .collect(),
            ),
            cortafuegos_otros: Some(Vec::new()),
            defender: Some(d),
            cortafuegos: Some(perfiles.iter().map(|(p, a)| ((*p).into(), *a)).collect()),
        }
    }

    #[test]
    fn real_time_protection_turned_off_is_one_notice_and_coming_back_is_history() {
        let mut b = Base::default();
        let bien = prot(
            &[("Microsoft Defender Antivirus", true)],
            defender(true, "Normal"),
            &[("Domain", true), ("Private", true), ("Public", true)],
        );
        assert!(b.mira_proteccion(&bien, 1_000).is_empty());
        let mal = prot(
            &[("Microsoft Defender Antivirus", false)],
            defender(false, "Normal"),
            &[("Domain", true), ("Private", true), ("Public", true)],
        );
        assert_eq!(atencion_proteccion(&mal), vec!["tiempo_real".to_owned()]);
        let h = b.mira_proteccion(&mal, 2_000);
        let avisos: Vec<_> = h.iter().filter(|x| x.aviso).collect();
        assert_eq!(avisos.len(), 1, "{h:?}");
        assert_eq!(avisos[0].frase, "vg_ev_tiempo_real_apagado");
        assert_eq!(
            avisos[0].texto,
            "La protección en tiempo real de Microsoft Defender se ha desactivado"
        );
        assert!(
            b.mira_proteccion(&mal, 3_000).is_empty(),
            "no repeat while it stays off"
        );
        let h = b.mira_proteccion(&bien, 4_000);
        assert!(h.iter().all(|x| !x.aviso));
        assert!(h.iter().any(|x| x.frase == "vg_ev_tiempo_real_vuelve"));
    }

    #[test]
    fn defender_standing_aside_for_another_antivirus_is_not_off() {
        let otro = prot(
            &[
                ("Microsoft Defender Antivirus", false),
                ("ESET Security", true),
            ],
            defender(false, "Passive Mode"),
            &[("Public", true)],
        );
        assert!(atencion_proteccion(&otro).is_empty());
        let ninguno = prot(
            &[
                ("Microsoft Defender Antivirus", false),
                ("ESET Security", false),
            ],
            defender(false, "Passive Mode"),
            &[("Public", true)],
        );
        assert_eq!(
            atencion_proteccion(&ninguno),
            vec!["sin_antivirus".to_owned()]
        );
    }

    #[test]
    fn a_firewall_profile_off_counts_unless_another_firewall_is_on() {
        let mut p = prot(
            &[("Microsoft Defender Antivirus", true)],
            defender(true, "Normal"),
            &[("Private", true), ("Public", false)],
        );
        assert_eq!(
            atencion_proteccion(&p),
            vec!["cortafuegos:Public".to_owned()]
        );
        let mut b = Base::default();
        let h = b.mira_proteccion(&p, 1_000);
        assert_eq!(h.len(), 1);
        assert!(h[0].aviso, "already off at the first look: said once");
        assert_eq!(h[0].frase, "vg_ev_cortafuegos_ya");
        p.cortafuegos_otros = Some(vec![Producto {
            nombre: "Norton Firewall".into(),
            activo: true,
        }]);
        assert!(atencion_proteccion(&p).is_empty());
    }

    #[test]
    fn the_scripts_answer_reads_into_a_look() {
        let v = serde_json::json!({
            "arranque": [
                { "tipo": "run", "donde": "HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run", "nombre": "SecurityHealth", "ruta": "C:\\Windows\\system32\\SecurityHealthSystray.exe", "cuenta": "equipo" },
                { "tipo": "servicio", "donde": "Auto", "nombre": "Spooler", "visible": "Cola de impresión", "ruta": "C:\\Windows\\System32\\spoolsv.exe", "cuenta": "equipo" },
                { "tipo": "tarea", "donde": "\\", "nombre": "", "ruta": "" }
            ],
            "privacidad": { "telemetria": "sin_fijar", "acceso_camara": "Allow" },
            "proteccion": {
                "antivirus": { "nombre": "Microsoft Defender Antivirus", "estado": 397568 },
                "cortafuegos_otros": [],
                "defender": { "servicio": true, "antivirus": true, "tiempo_real": true, "manipulacion": true, "modo": "Normal" },
                "cortafuegos": [ { "perfil": "Domain", "activo": true } ]
            },
            "cuentas": { "S-1-5-21-1": "fran" },
            "errores": {}
        });
        let f = de_json(&v);
        let a = f.arranque.unwrap();
        assert_eq!(a.len(), 2, "an entry without a name is dropped");
        assert_eq!(a[1].titulo(), "Cola de impresión");
        assert!(a[1].es_del_sistema());
        assert!(!a[0].es_del_sistema(), "a Run entry always counts");
        assert_eq!(f.privacidad.unwrap()["acceso_camara"], "Allow");
        let p = f.proteccion.unwrap();
        assert!(p.antivirus.unwrap()[0].activo);
        assert!(p.defender.unwrap().al_mando());
        assert_eq!(f.cuentas["S-1-5-21-1"], "fran");
        let vacio = de_json(&serde_json::json!({ "arranque": null, "privacidad": null }));
        assert!(
            vacio.arranque.is_none() && vacio.privacidad.is_none() && vacio.proteccion.is_none()
        );
    }

    #[test]
    fn the_same_notice_is_not_given_twice_within_its_window() {
        let mut b = Base::default();
        assert!(b.avisa("k", 0, HORA));
        assert!(!b.avisa("k", HORA - 1, HORA));
        assert!(b.avisa("k", HORA, HORA));
        // Old keys are forgotten after a week, so the base does not grow for ever.
        b.avisa("otra", 0, HORA);
        b.avisa("k", 8 * DIA, HORA);
        assert!(!b.avisados.contains_key("otra"));
    }

    #[test]
    fn startup_folders_are_read_as_entries() {
        let d = std::env::temp_dir().join(format!("guardiana-vg-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("com.ejemplo.agente.plist"), "x").unwrap();
        std::fs::write(d.join(".oculto.plist"), "x").unwrap();
        std::fs::write(d.join("nota.txt"), "x").unwrap();
        let l = de_carpeta(&d, "launchd", "equipo", &[".plist"]);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].nombre, "com.ejemplo.agente.plist");
        assert_eq!(l[0].tipo, "launchd");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Every sentence written in this file is in the list the panel checks against its texts.
    #[test]
    fn every_sentence_used_is_listed() {
        let fuente = include_str!("vigilancia.rs");
        let mut usadas: BTreeSet<&str> = BTreeSet::new();
        for parte in fuente.split("\"vg_ev_").skip(1) {
            match parte.split('"').next() {
                Some(resto)
                    if !resto.is_empty()
                        && resto.chars().all(|c| c.is_ascii_lowercase() || c == '_') =>
                {
                    usadas.insert(resto);
                }
                _ => {}
            }
        }
        assert!(usadas.len() > 15, "{usadas:?}");
        for u in usadas {
            assert!(FRASES.contains(&format!("vg_ev_{u}").as_str()), "vg_ev_{u}");
        }
    }

    #[cfg(all(not(windows), not(target_os = "macos")))]
    #[test]
    fn on_linux_the_look_reads_startup_and_says_the_rest_is_not_read() {
        let f = foto().unwrap();
        assert!(f.arranque.is_some());
        assert!(f.privacidad.is_none() && f.proteccion.is_none());
        assert!(matches!(usos(), Err(Error::NoSoportado)));
    }

    /// Nothing in the script can change the computer, like the check-up's.
    #[test]
    fn the_watch_script_only_reads() {
        let l = SCRIPT.to_ascii_lowercase();
        assert!(SCRIPT.is_ascii());
        for prohibido in [
            "set-",
            "remove-",
            "new-item",
            "stop-",
            "start-",
            "restart-",
            "disable-",
            "enable-",
            "invoke-webrequest",
            "invoke-restmethod",
            "downloadstring",
            "out-file",
            "add-content",
            "reg add",
            "reg delete",
            "invoke-expression",
            "iex ",
            "resolve-dnsname",
            "-encodedcommand",
        ] {
            assert!(!l.contains(prohibido), "{prohibido}");
        }
    }
}
