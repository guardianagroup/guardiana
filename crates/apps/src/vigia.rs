//! «Vigía» (1.0.48): lo que una llamada o un chat de GUARDIANA necesita saber del ordenador,
//! cada pocos segundos. El dueño, 30 Oct 2026: que durante una comunicación el ordenador detecte
//! señales de espionaje «en todo el PC completo» y la comunicación se pare sola, diciendo qué ha
//! visto.
//!
//! Solo señales que el propio Windows anota y que se leen sin instalar nada, elegidas para que
//! casi nunca avisen de algo que no está pasando:
//!
//! 1. **Micrófono, cámara y captura de pantalla en uso ahora**, y por qué programa. Windows guarda
//!    por programa cuándo empezó y cuándo dejó de usarlos (`CapabilityAccessManager\ConsentStore`:
//!    `microphone`, `webcam`, `graphicsCaptureProgrammatic` y `graphicsCaptureWithoutBorder`), en
//!    el registro de cada cuenta con sesión abierta y en el del equipo. `LastUsedTimeStop = 0`
//!    quiere decir «lo está usando ahora». Un programa que ya no está en marcha no cuenta aunque
//!    Windows no llegara a anotar su final.
//! 2. **Escritorio remoto**: una sesión de Windows conectada por escritorio remoto (RDP), o una
//!    sesión que está observando otra.
//! 3. **Programas de control remoto en marcha** (TeamViewer, AnyDesk, RustDesk, VNC…), también
//!    cuando corren solo como servicio de Windows, y la asistencia remota de Windows.
//! 4. **Programas de grabar la pantalla abiertos** (OBS, Bandicam, la grabación de acciones de
//!    Windows…): graban sin pasar por el permiso de captura de Windows, así que se miran por nombre.
//! 5. **Microsoft Defender**: si su protección en tiempo real está apagada cuando es él quien
//!    protege, si ningún antivirus protege y si tiene amenazas activas (WMI,
//!    `root\Microsoft\Windows\Defender`).
//!
//! Lo que se descarta, y por qué, está en la página y aquí: los ganchos de teclado (Windows no
//! deja ver quién los tiene sin trucos no documentados, y los usan muchos programas normales:
//! accesibilidad, atajos, juegos), quién lee el portapapeles (tampoco se puede listar), la
//! captura por copia del escritorio (DXGI) y las grabadoras de audio del sistema (Windows no las
//! anota), los agentes de gestión de empresas (están siempre en marcha en los ordenadores de
//! empresa: avisarían siempre) y los proxys o certificados raíz (las comunicaciones de GUARDIANA
//! van cifradas de extremo a extremo, y muchos antivirus y empresas los usan de forma legítima).
//!
//! GUARDIANA nunca avisa de sí misma, ni del navegador desde el que se llama: ese es el que tiene
//! el micrófono y la cámara para la llamada. El navegador se reconoce por la conexión que hace la
//! pregunta (qué programa la abrió, según Windows) y, si eso falla, por cómo se presenta.
//!
//! Este módulo solo lee. La mitad de Windows está en `vigia/win.rs`; aquí está todo lo que decide,
//! igual en todos los sistemas y probado.

use std::collections::{BTreeMap, HashSet};

use crate::Error;

#[cfg(windows)]
mod win;

/// Si este sistema tiene vigía: solo Windows anota estas señales.
pub const DISPONIBLE: bool = cfg!(windows);

// ---------------------------------------------------------------------------------------------
// Qué se vio.

/// La clase de señal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tipo {
    /// Otro programa usa el micrófono.
    Microfono,
    /// Otro programa usa la cámara.
    Camara,
    /// Otro programa captura la pantalla con la captura de Windows.
    Pantalla,
    /// Una sesión de escritorio remoto, o una que observa a otra.
    EscritorioRemoto,
    /// Un programa de control remoto en marcha.
    ControlRemoto,
    /// Un programa de grabar la pantalla abierto.
    Grabador,
    /// El antivirus no protege, o no se pudo saber.
    Antivirus,
    /// Microsoft Defender tiene una amenaza activa.
    Amenaza,
}

impl Tipo {
    /// La palabra que viaja en el JSON.
    #[must_use]
    pub const fn como_texto(self) -> &'static str {
        match self {
            Self::Microfono => "microfono",
            Self::Camara => "camara",
            Self::Pantalla => "pantalla",
            Self::EscritorioRemoto => "escritorio_remoto",
            Self::ControlRemoto => "control_remoto",
            Self::Grabador => "grabador",
            Self::Antivirus => "antivirus",
            Self::Amenaza => "amenaza",
        }
    }
}

/// Las capacidades del `ConsentStore` que se miran, y qué es cada una.
pub const CAPACIDADES: [(&str, Tipo); 4] = [
    ("microphone", Tipo::Microfono),
    ("webcam", Tipo::Camara),
    ("graphicsCaptureProgrammatic", Tipo::Pantalla),
    ("graphicsCaptureWithoutBorder", Tipo::Pantalla),
];

/// Un uso del micrófono, la cámara o la captura de pantalla, tal como Windows lo anotó.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Uso {
    /// El programa: su archivo (`Zoom.exe`) o, si es de la Tienda, el nombre de su paquete.
    pub programa: String,
    /// Su ruta completa, si Windows la dio.
    pub ruta: Option<String>,
    /// Qué usó.
    pub tipo: Tipo,
    /// Cuándo empezó a usarlo por última vez, en ms Unix.
    pub desde: i64,
    /// Cuándo paró; `None` mientras lo sigue usando.
    pub hasta: Option<i64>,
    /// La cuenta (su SID) cuyo registro es, o `equipo`.
    pub cuenta: String,
}

/// Una entrada del `ConsentStore` tal como la da el registro: el nombre de la clave y sus dos
/// horas (FILETIME). Los programas `NonPackaged` se nombran por su ruta con `#` en vez de `\`; los
/// de la Tienda, por su paquete (`Microsoft.WindowsCamera_8wekyb3d8bbwe`). Un inicio cero quiere
/// decir que nunca lo usó. Misma lectura que la vigilancia constante (`vigilancia::de_registro`),
/// con las cuatro capacidades.
#[must_use]
pub fn de_registro(clave: &str, tipo: Tipo, inicio: u64, fin: u64, cuenta: &str) -> Option<Uso> {
    let desde = crate::vigilancia::de_filetime(inicio)?;
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
        tipo,
        desde,
        // Un final anterior al inicio es del uso de antes: lo está usando ahora.
        hasta: crate::vigilancia::de_filetime(fin).filter(|&h| fin >= inicio && h >= desde),
        cuenta: cuenta.chars().take(80).collect(),
    })
}

/// Un proceso en marcha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proceso {
    /// Su número.
    pub pid: u32,
    /// Su archivo, como lo da Windows (`AnyDesk.exe`).
    pub nombre: String,
    /// Su sesión de Windows (0 = servicios), cuando se pidió y se supo.
    pub sesion: Option<u32>,
}

/// Una sesión de Windows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sesion {
    /// Su número.
    pub id: u32,
    /// Su estado (`WTS_CONNECTSTATE_CLASS`: 0 activa, 1 conectada, 3 observando otra…).
    pub estado: i32,
    /// Su protocolo (`WTSClientProtocolType`: 0 la consola, 2 escritorio remoto).
    pub protocolo: u16,
    /// El nombre de su puesto (`Console`, `RDP-Tcp#0`…).
    pub puesto: String,
    /// La cuenta que la usa.
    pub usuario: String,
    /// El equipo desde el que se conecta, si es remota.
    pub cliente: String,
}

/// Lo que Microsoft Defender dice de sí mismo (`MSFT_MpComputerStatus`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Defender {
    /// Su servicio está en marcha.
    pub servicio: bool,
    /// Su antivirus está encendido.
    pub antivirus: bool,
    /// La protección en tiempo real está encendida.
    pub tiempo_real: bool,
    /// `Normal` cuando es él quien protege; `Passive Mode` y parecidos cuando lo hace otro.
    pub modo: String,
}

impl Defender {
    /// Si es Defender quien protege el equipo (no está apartado para otro antivirus). Sin modo
    /// (Windows antiguos), manda si su antivirus está encendido.
    #[must_use]
    pub fn al_mando(&self) -> bool {
        let m = self.modo.trim();
        m.eq_ignore_ascii_case("normal") || (m.is_empty() && self.antivirus)
    }
}

/// Una amenaza activa según Defender (`MSFT_MpThreat` con `IsActive`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmenazaActiva {
    /// Su nombre (`Trojan:Win32/…`).
    pub nombre: String,
    /// Su gravedad (`SeverityID`: 1 baja … 5 grave).
    pub gravedad: u8,
}

/// Lo que se sabe de la protección, de una lectura lenta.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Defensa {
    /// Cuándo se leyó, en ms Unix.
    pub cuando: i64,
    /// El estado de Defender, si contestó.
    pub defender: Option<Defender>,
    /// Sus amenazas activas, si contestó (vacío = ninguna).
    pub amenazas: Option<Vec<AmenazaActiva>>,
    /// Los antivirus que lista el Centro de seguridad de Windows y si protegen; `None` si no
    /// contestó (Windows Server no lo tiene).
    pub antivirus: Option<Vec<(String, bool)>>,
    /// Lo que falló, por parte.
    pub errores: BTreeMap<String, String>,
}

/// Una mirada entera del vigía.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Lectura {
    /// Cuándo se hizo la mirada rápida, en ms Unix.
    pub cuando: i64,
    /// Los usos anotados; `None` si no se pudieron leer.
    pub usos: Option<Vec<Uso>>,
    /// Los procesos en marcha; `None` si no se pudieron leer.
    pub procesos: Option<Vec<Proceso>>,
    /// Las sesiones de Windows; `None` si no se pudieron leer.
    pub sesiones: Option<Vec<Sesion>>,
    /// La protección (mirada lenta), si ya se leyó.
    pub defensa: Option<Defensa>,
}

// ---------------------------------------------------------------------------------------------
// Las listas.

/// Programas de control remoto: su archivo (en minúsculas) y el producto. Cuentan también
/// cuando corren solo como servicio de Windows: así pueden aceptar una conexión. El cliente que
/// controla OTRO ordenador (`mstsc.exe`) no está: no deja ver este.
pub const CONTROL_REMOTO: &[(&str, &str)] = &[
    ("teamviewer.exe", "TeamViewer"),
    ("teamviewer_service.exe", "TeamViewer"),
    ("tv_w32.exe", "TeamViewer"),
    ("tv_x64.exe", "TeamViewer"),
    ("anydesk.exe", "AnyDesk"),
    ("rustdesk.exe", "RustDesk"),
    ("remoting_host.exe", "Chrome Remote Desktop"),
    ("remote_assistance_host.exe", "Chrome Remote Desktop"),
    ("srservice.exe", "Splashtop"),
    ("srserver.exe", "Splashtop"),
    ("srmanager.exe", "Splashtop"),
    ("logmein.exe", "LogMeIn"),
    ("lmiguardiansvc.exe", "LogMeIn"),
    ("screenconnect.clientservice.exe", "ScreenConnect"),
    ("screenconnect.windowsclient.exe", "ScreenConnect"),
    ("winvnc.exe", "VNC"),
    ("winvnc4.exe", "VNC"),
    ("tvnserver.exe", "VNC"),
    ("vncserver.exe", "VNC"),
    ("rserver3.exe", "Radmin"),
    ("aa_v3.exe", "Ammyy Admin"),
    ("supremo.exe", "Supremo"),
    ("supremoservice.exe", "Supremo"),
    ("ultraviewer_desktop.exe", "UltraViewer"),
    ("ultraviewer_service.exe", "UltraViewer"),
    ("todesk.exe", "ToDesk"),
    ("todesk_service.exe", "ToDesk"),
    ("sunloginclient.exe", "Sunlogin"),
    ("awesun.exe", "AweSun"),
    ("dwagent.exe", "DWService"),
    ("dwagsvc.exe", "DWService"),
    ("rutserv.exe", "Remote Utilities"),
    ("rfusclient.exe", "Remote Utilities"),
    ("client32.exe", "NetSupport Manager"),
    ("meshagent.exe", "MeshCentral"),
    ("parsecd.exe", "Parsec"),
    ("nxserver.exe", "NoMachine"),
    ("nxnode.exe", "NoMachine"),
    ("hoptodesk.exe", "HopToDesk"),
    ("getscreen.exe", "Getscreen.me"),
    ("iperiusremote.exe", "Iperius Remote"),
    ("za_connect.exe", "Zoho Assist"),
    ("zaservice.exe", "Zoho Assist"),
    ("bomgar-scc.exe", "BeyondTrust Remote Support"),
    ("dwrcs.exe", "DameWare"),
    ("romserver.exe", "LiteManager"),
    ("aeroadmin.exe", "AeroAdmin"),
    ("anyviewer.exe", "AnyViewer"),
    ("msra.exe", "msra.exe"),
    ("quickassist.exe", "quickassist.exe"),
];

/// Programas de grabar la pantalla: abiertos, graban sin pasar por la captura de Windows.
pub const GRABADORES: &[(&str, &str)] = &[
    ("obs64.exe", "OBS Studio"),
    ("obs32.exe", "OBS Studio"),
    ("obs.exe", "OBS Studio"),
    ("bdcam.exe", "Bandicam"),
    ("camrecorder.exe", "Camtasia Recorder"),
    ("fraps.exe", "Fraps"),
    ("xsplit.core.exe", "XSplit"),
    ("apowerrec.exe", "ApowerREC"),
    ("psr.exe", "psr.exe"),
];

/// Nombres conocidos de programas que suelen usar el micrófono, la cámara o la pantalla, para que
/// la persona los reconozca. Los demás se nombran por su archivo sin `.exe`.
const CONOCIDOS: &[(&str, &str)] = &[
    ("zoom.exe", "Zoom"),
    ("ms-teams.exe", "Microsoft Teams"),
    ("teams.exe", "Microsoft Teams"),
    ("msteams", "Microsoft Teams"),
    ("microsoftteams", "Microsoft Teams"),
    ("discord.exe", "Discord"),
    ("skype.exe", "Skype"),
    ("microsoft.skypeapp", "Skype"),
    ("slack.exe", "Slack"),
    ("whatsapp.exe", "WhatsApp"),
    ("5319275a.whatsappdesktop", "WhatsApp"),
    ("telegram.exe", "Telegram"),
    ("telegrammessengerllp.telegramdesktop", "Telegram"),
    ("signal.exe", "Signal"),
    ("webex.exe", "Webex"),
    ("chrome.exe", "Google Chrome"),
    ("msedge.exe", "Microsoft Edge"),
    ("firefox.exe", "Mozilla Firefox"),
    ("opera.exe", "Opera"),
    ("brave.exe", "Brave"),
    ("vivaldi.exe", "Vivaldi"),
    ("msedgewebview2.exe", "Microsoft Edge WebView2"),
    ("clipchamp.clipchamp", "Clipchamp"),
    ("microsoft.yourphone", "Phone Link"),
    ("microsoft.xboxgamingoverlay", "Xbox Game Bar"),
];

/// Los programas de Windows, con su nombre en cada idioma.
fn de_windows(clave: &str, idioma: Idioma) -> Option<&'static str> {
    let (es, en, pt) = match clave {
        "msra.exe" => (
            "Asistencia remota de Windows",
            "Windows Remote Assistance",
            "Assistência Remota do Windows",
        ),
        "quickassist.exe" | "microsoftcorporationii.quickassist" => {
            ("Asistencia rápida", "Quick Assist", "Assistência Rápida")
        }
        "psr.exe" => (
            "Grabación de acciones de Windows",
            "Windows Steps Recorder",
            "Gravador de Passos do Windows",
        ),
        "microsoft.windowscamera" => ("Cámara de Windows", "Windows Camera", "Câmera do Windows"),
        "microsoft.windowssoundrecorder" => {
            ("Grabadora de sonido", "Sound Recorder", "Gravador de Som")
        }
        "microsoft.screensketch" | "snippingtool.exe" => {
            ("Recortes", "Snipping Tool", "Ferramenta de Captura")
        }
        _ => return None,
    };
    Some(match idioma {
        Idioma::Es => es,
        Idioma::En => en,
        Idioma::Pt => pt,
    })
}

/// Si el archivo es de un programa que el vigía mira por nombre (control remoto o grabador).
#[must_use]
pub fn es_vigilado(archivo: &str) -> bool {
    let a = archivo.to_ascii_lowercase();
    CONTROL_REMOTO
        .iter()
        .chain(GRABADORES)
        .any(|(x, _)| *x == a.as_str())
}

/// El nombre que una persona reconoce, para un archivo o paquete.
#[must_use]
pub fn nombre_legible(clave: &str, idioma: Idioma) -> String {
    let c = clave.trim().to_lowercase();
    if let Some(n) = de_windows(&c, idioma) {
        return n.to_owned();
    }
    if let Some((_, n)) = CONTROL_REMOTO
        .iter()
        .chain(GRABADORES)
        .chain(CONOCIDOS)
        .find(|(x, _)| *x == c.as_str())
    {
        return (*n).to_owned();
    }
    let sin_exe = if c.ends_with(".exe") {
        clave.trim().get(..clave.trim().len() - 4).unwrap_or(clave)
    } else {
        clave.trim()
    };
    sin_exe.chars().take(80).collect()
}

// ---------------------------------------------------------------------------------------------
// Quién pregunta: GUARDIANA y el navegador de la llamada no cuentan.

/// Cómo se supo cuál es el navegador que pregunta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Como {
    /// Por la conexión: Windows dice qué programa la abrió.
    Conexion,
    /// Por cómo se presenta el navegador (`User-Agent`), cuando lo anterior no se pudo.
    Presentacion,
}

/// El navegador desde el que se llama.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Llamante {
    /// Su archivo (`chrome.exe`).
    pub nombre: String,
    /// Su ruta completa, si se supo.
    pub ruta: Option<String>,
    /// Cómo se supo.
    pub como: Como,
}

/// Lo que nunca se cuenta como señal: GUARDIANA y el navegador de la llamada.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Propio {
    /// La carpeta de GUARDIANA, en minúsculas y acabada en `\`.
    pub carpeta: Option<String>,
    /// El navegador que pregunta.
    pub llamante: Option<Llamante>,
}

/// Los archivos de GUARDIANA, estén donde estén.
const DE_GUARDIANA: &[&str] = &["guardiana.exe", "guardiana-zero.exe", "guardiana zero.exe"];

impl Propio {
    /// GUARDIANA, por su archivo o por estar en su carpeta.
    #[must_use]
    pub fn es_guardiana(&self, programa: &str, ruta: Option<&str>) -> bool {
        let p = programa.to_lowercase();
        if DE_GUARDIANA.contains(&p.as_str()) {
            return true;
        }
        match (ruta, &self.carpeta) {
            (Some(r), Some(c)) => r.to_lowercase().starts_with(c.as_str()),
            _ => false,
        }
    }

    /// El navegador que pregunta: por su ruta si se saben las dos, si no por su archivo.
    #[must_use]
    pub fn es_llamante(&self, programa: &str, ruta: Option<&str>) -> bool {
        let Some(l) = &self.llamante else {
            return false;
        };
        match (ruta, &l.ruta) {
            (Some(r), Some(lr)) => r.to_lowercase() == lr.to_lowercase(),
            _ => programa.to_lowercase() == l.nombre.to_lowercase(),
        }
    }
}

/// La carpeta del programa que está corriendo, si es la de GUARDIANA (su nombre lo dice): todo
/// lo que hay dentro es GUARDIANA. Fuera de una carpeta así no se excluye nada por carpeta.
#[must_use]
pub fn carpeta_propia() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.to_string_lossy().to_lowercase();
    let dir = dir.trim_end_matches(['\\', '/']);
    (dir.contains("guardiana") && dir.len() > 3).then(|| format!("{dir}\\"))
}

/// El navegador por cómo se presenta (`User-Agent`), para cuando Windows no dijo qué programa abrió
/// la conexión. Solo el archivo habitual de cada uno: es una aproximación, y se dice.
#[must_use]
pub fn navegador_de_agente(agente: &str) -> Option<&'static str> {
    if agente.contains("Edg/") {
        Some("msedge.exe")
    } else if agente.contains("OPR/") {
        Some("opera.exe")
    } else if agente.contains("Firefox/") {
        Some("firefox.exe")
    } else if agente.contains("Chrome/") {
        Some("chrome.exe")
    } else {
        None
    }
}

/// El número del proceso dueño de la conexión que sale de `ip:puerto`, en una tabla
/// `MIB_TCPTABLE_OWNER_PID` leída como palabras de 32 bits: el número de filas y, después, seis
/// palabras por fila (estado, dirección local, puerto local, dirección remota, puerto remoto,
/// proceso). Las direcciones y los puertos van en el orden de la red.
#[must_use]
pub fn pid_de_tabla(tabla: &[u32], ip: [u8; 4], puerto: u16) -> Option<u32> {
    let n = usize::try_from(*tabla.first()?).ok()?;
    for i in 0..n.min(200_000) {
        let inicio = 1 + i * 6;
        let fila = tabla.get(inicio..inicio + 6)?;
        if let [_, local, puerto_local, _, _, pid] = *fila {
            let p = puerto_local.to_ne_bytes();
            if local.to_ne_bytes() == ip && u16::from_be_bytes([p[0], p[1]]) == puerto {
                return Some(pid);
            }
        }
    }
    None
}

/// El navegador que abrió la conexión de `cliente` (la otra punta de una petición que llegó al
/// panel). Solo Windows lo dice; en los demás sistemas, `None`.
#[must_use]
pub fn llamante_de(cliente: std::net::SocketAddr) -> Option<Llamante> {
    #[cfg(windows)]
    {
        let std::net::IpAddr::V4(ip) = cliente.ip() else {
            return None;
        };
        let pid = win::duenno(ip.octets(), cliente.port())?;
        let (nombre, ruta) = crate::ventanas::programa_de(pid);
        (!nombre.is_empty()).then(|| Llamante {
            nombre,
            ruta: (!ruta.is_empty()).then_some(ruta),
            como: Como::Conexion,
        })
    }
    #[cfg(not(windows))]
    {
        let _ = cliente;
        None
    }
}

// ---------------------------------------------------------------------------------------------
// Leer.

/// La mirada rápida: usos, procesos y sesiones. Cuesta milisegundos (el registro y dos listas
/// del sistema, leídos directamente). Fuera de Windows, una lectura vacía.
#[must_use]
pub fn lee_rapido(ahora: i64) -> Lectura {
    #[cfg(windows)]
    {
        Lectura {
            cuando: ahora,
            usos: Some(win::usos()),
            procesos: win::procesos(),
            sesiones: win::sesiones(),
            defensa: None,
        }
    }
    #[cfg(not(windows))]
    {
        Lectura {
            cuando: ahora,
            ..Lectura::default()
        }
    }
}

/// Quién está en la consola de este equipo (su pantalla y su teclado), para notar que alguien
/// entró o cambió de cuenta sin preguntárselo a PowerShell: `sesión:dominio\\cuenta`, vacío cuando
/// no hay nadie (1.0.55). `None` fuera de Windows o si Windows no lo dijo.
#[must_use]
pub fn cuenta_de_consola() -> Option<String> {
    #[cfg(windows)]
    {
        win::cuenta_de_consola()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// La mirada lenta: Microsoft Defender y el Centro de seguridad, con un PowerShell de solo
/// lectura y prioridad baja (uno o dos segundos). Llamar fuera del bucle asíncrono.
///
/// # Errors
/// [`Error::NoSoportado`] fuera de Windows; [`Error::Sistema`] si PowerShell no corrió.
pub fn lee_defensa(ahora: i64) -> Result<Defensa, Error> {
    #[cfg(windows)]
    {
        win::defensa().map(|texto| de_salida_defensa(&texto, ahora))
    }
    #[cfg(not(windows))]
    {
        let _ = ahora;
        Err(Error::NoSoportado)
    }
}

/// El guion de la mirada lenta, como viaja dentro del programa.
pub const SCRIPT_DEFENSA: &str = include_str!("vigia/defensa.ps1");

/// Lee lo que imprimió el guion de la mirada lenta: una línea por hecho, campos separados por
/// tabuladores. `D` el estado de Defender; `TA` que las amenazas se leyeron, y una `T` por
/// amenaza; `AA` que el Centro de seguridad contestó, y una `A` por antivirus; `E` lo que falló.
#[must_use]
pub fn de_salida_defensa(texto: &str, ahora: i64) -> Defensa {
    let si = |x: &str| x.trim().eq_ignore_ascii_case("true");
    let corta = |x: &str, n: usize| -> String { x.trim().chars().take(n).collect() };
    let mut d = Defensa {
        cuando: ahora,
        ..Defensa::default()
    };
    let mut antivirus: Vec<(String, bool)> = Vec::new();
    let mut amenazas: Vec<AmenazaActiva> = Vec::new();
    let (mut hay_antivirus, mut hay_amenazas) = (false, false);
    for linea in texto.lines() {
        let campos: Vec<&str> = linea.trim_end_matches('\r').split('\t').collect();
        match campos.as_slice() {
            ["D", servicio, av, tiempo_real, modo, ..] => {
                d.defender = Some(Defender {
                    servicio: si(servicio),
                    antivirus: si(av),
                    tiempo_real: si(tiempo_real),
                    modo: corta(modo, 40),
                });
            }
            ["TA", ..] => hay_amenazas = true,
            ["T", nombre, gravedad, ..] if amenazas.len() < 20 && !nombre.trim().is_empty() => {
                amenazas.push(AmenazaActiva {
                    nombre: corta(nombre, 160),
                    gravedad: gravedad.trim().parse().unwrap_or(0),
                });
            }
            ["AA", ..] => hay_antivirus = true,
            ["A", nombre, estado, ..] if antivirus.len() < 20 && !nombre.trim().is_empty() => {
                let e = estado.trim().parse::<u32>().unwrap_or(0);
                antivirus.push((corta(nombre, 120), crate::lee_estado(e).0));
            }
            ["E", parte, mensaje, ..] if d.errores.len() < 8 => {
                d.errores.insert(corta(parte, 20), corta(mensaje, 200));
            }
            _ => {}
        }
    }
    if hay_amenazas {
        d.amenazas = Some(amenazas);
    }
    if hay_antivirus {
        d.antivirus = Some(antivirus);
    }
    d
}

// ---------------------------------------------------------------------------------------------
// Decidir.

/// Si una señal debe parar la comunicación o solo decirse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Nivel {
    /// Para la comunicación.
    Alerta,
    /// Se dice, pero no para nada.
    Aviso,
}

/// Lo que precisa una señal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Detalle {
    /// Un uso que Windows anotó.
    Uso,
    /// Un programa en marcha; `servicio` si solo corre como servicio de Windows.
    Proceso {
        /// Solo como servicio (sesión 0).
        servicio: bool,
    },
    /// Una sesión de escritorio remoto.
    Sesion {
        /// La cuenta que la usa.
        usuario: String,
        /// El equipo desde el que se conecta.
        cliente: String,
    },
    /// Una sesión observa a otra.
    Sombra,
    /// La protección en tiempo real de Defender está apagada y es él quien protege.
    TiempoReal,
    /// Ningún antivirus protege.
    SinAntivirus,
    /// Defender está apartado y no se pudo saber qué otro protege.
    SegundoPlano,
    /// El estado de Defender no se pudo leer.
    NoLeido,
    /// Una amenaza activa, con su gravedad.
    Amenaza {
        /// `SeverityID`.
        gravedad: u8,
    },
}

/// Una señal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hallazgo {
    /// Qué clase.
    pub tipo: Tipo,
    /// Si para la comunicación.
    pub nivel: Nivel,
    /// El archivo, el paquete o el nombre de lo visto (`Zoom.exe`, `AnyDesk.exe`, el nombre de
    /// la amenaza); vacío cuando no hay programa (una sesión, el antivirus).
    pub clave: String,
    /// Su ruta, si se sabe (solo para el panel, con llave).
    pub ruta: Option<String>,
    /// Desde cuándo, si Windows lo dice.
    pub desde: Option<i64>,
    /// Lo que precisa.
    pub detalle: Detalle,
}

/// Si un uso anotado como «en curso» es de un programa que sigue en marcha. Solo un programa con
/// ruta se puede buscar entre los procesos; uno de la Tienda se toma por lo que dice Windows.
fn sigue_abierto(u: &Uso, en_marcha: Option<&HashSet<String>>) -> bool {
    match (en_marcha, &u.ruta) {
        (Some(m), Some(_)) if !m.is_empty() => m.contains(&u.programa.to_lowercase()),
        _ => true,
    }
}

/// Las señales de una lectura, sin GUARDIANA ni el navegador que pregunta, ordenadas: primero las
/// que paran la comunicación.
#[must_use]
pub fn analiza(l: &Lectura, propio: &Propio) -> Vec<Hallazgo> {
    let mut out: Vec<Hallazgo> = Vec::new();
    let en_marcha: Option<HashSet<String>> = l
        .procesos
        .as_ref()
        .map(|v| v.iter().map(|p| p.nombre.to_lowercase()).collect());

    // 1. Micrófono, cámara y pantalla en uso ahora, por otros programas. Un mismo programa en
    // dos cuentas, o con las dos capacidades de pantalla, es una sola señal.
    let mut vistos: HashSet<(Tipo, String)> = HashSet::new();
    for u in l.usos.iter().flatten() {
        if u.hasta.is_some()
            || propio.es_guardiana(&u.programa, u.ruta.as_deref())
            || propio.es_llamante(&u.programa, u.ruta.as_deref())
            || !sigue_abierto(u, en_marcha.as_ref())
        {
            continue;
        }
        if vistos.insert((u.tipo, u.programa.to_lowercase())) {
            out.push(Hallazgo {
                tipo: u.tipo,
                nivel: Nivel::Alerta,
                clave: u.programa.clone(),
                ruta: u.ruta.clone(),
                desde: Some(u.desde),
                detalle: Detalle::Uso,
            });
        }
    }

    // 2. Escritorio remoto.
    for s in l.sesiones.iter().flatten() {
        let remota = s.protocolo == 2 || s.puesto.to_ascii_lowercase().starts_with("rdp-tcp#");
        if s.estado == 3 {
            out.push(Hallazgo {
                tipo: Tipo::EscritorioRemoto,
                nivel: Nivel::Alerta,
                clave: String::new(),
                ruta: None,
                desde: None,
                detalle: Detalle::Sombra,
            });
        } else if remota && (s.estado == 0 || s.estado == 1) {
            out.push(Hallazgo {
                tipo: Tipo::EscritorioRemoto,
                nivel: Nivel::Alerta,
                clave: String::new(),
                ruta: None,
                desde: None,
                detalle: Detalle::Sesion {
                    usuario: s.usuario.chars().take(80).collect(),
                    cliente: s.cliente.chars().take(80).collect(),
                },
            });
        }
    }

    // 3 y 4. Control remoto y grabadores, por producto: varios procesos de TeamViewer son uno.
    for (lista, tipo) in [
        (CONTROL_REMOTO, Tipo::ControlRemoto),
        (GRABADORES, Tipo::Grabador),
    ] {
        // Producto → (su primer archivo visto, si alguno corre fuera de la sesión de servicios).
        let mut productos: BTreeMap<&str, (String, bool)> = BTreeMap::new();
        for p in l.procesos.iter().flatten() {
            let a = p.nombre.to_lowercase();
            if propio.es_guardiana(&p.nombre, None) || propio.es_llamante(&p.nombre, None) {
                continue;
            }
            if let Some((_, producto)) = lista.iter().find(|(x, _)| *x == a.as_str()) {
                let en_escritorio = p.sesion != Some(0);
                let e = productos
                    .entry(*producto)
                    .or_insert_with(|| (p.nombre.clone(), false));
                e.1 |= en_escritorio;
            }
        }
        for (_, (archivo, en_escritorio)) in productos {
            out.push(Hallazgo {
                tipo,
                nivel: Nivel::Alerta,
                clave: archivo,
                ruta: None,
                desde: None,
                detalle: Detalle::Proceso {
                    servicio: !en_escritorio,
                },
            });
        }
    }

    // 5. La protección.
    if let Some(d) = &l.defensa {
        for a in d.amenazas.iter().flatten().take(5) {
            out.push(Hallazgo {
                tipo: Tipo::Amenaza,
                nivel: Nivel::Alerta,
                clave: a.nombre.clone(),
                ruta: None,
                desde: None,
                detalle: Detalle::Amenaza {
                    gravedad: a.gravedad,
                },
            });
        }
        let otro_protege = d
            .antivirus
            .iter()
            .flatten()
            .any(|(n, activo)| *activo && !n.to_ascii_lowercase().contains("defender"));
        let antivirus = |nivel: Nivel, detalle: Detalle| Hallazgo {
            tipo: Tipo::Antivirus,
            nivel,
            clave: "Microsoft Defender".to_owned(),
            ruta: None,
            desde: None,
            detalle,
        };
        match &d.defender {
            Some(def) if def.al_mando() => {
                if !def.tiempo_real {
                    out.push(antivirus(Nivel::Alerta, Detalle::TiempoReal));
                }
            }
            Some(_) if otro_protege => {}
            // Apartado y el Centro de seguridad contestó sin ningún antivirus activo.
            Some(_) if d.antivirus.is_some() => {
                out.push(antivirus(Nivel::Alerta, Detalle::SinAntivirus));
            }
            Some(_) => out.push(antivirus(Nivel::Aviso, Detalle::SegundoPlano)),
            None if otro_protege => {}
            None => out.push(antivirus(Nivel::Aviso, Detalle::NoLeido)),
        }
    }

    out.sort_by_key(|h| (h.nivel, h.tipo, h.clave.to_lowercase()));
    out
}

// ---------------------------------------------------------------------------------------------
// Decirlo.

/// El idioma de las frases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Idioma {
    /// Español.
    #[default]
    Es,
    /// Inglés.
    En,
    /// Portugués.
    Pt,
}

impl Idioma {
    /// De un código (`en`, `pt-BR`) o de una cabecera `Accept-Language` entera: cuenta la
    /// primera lengua; cualquier otra cosa es español.
    #[must_use]
    pub fn de(codigo: &str) -> Self {
        let primera = codigo
            .split([',', ';'])
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        match primera.get(..2) {
            Some("en") => Self::En,
            Some("pt") => Self::Pt,
            _ => Self::Es,
        }
    }

    /// Su código.
    #[must_use]
    pub const fn codigo(self) -> &'static str {
        match self {
            Self::Es => "es",
            Self::En => "en",
            Self::Pt => "pt",
        }
    }
}

/// Una frase en el idioma pedido.
fn en(idioma: Idioma, es: &str, en: &str, pt: &str) -> String {
    match idioma {
        Idioma::Es => es,
        Idioma::En => en,
        Idioma::Pt => pt,
    }
    .to_owned()
}

impl Hallazgo {
    /// El nombre que una persona reconoce.
    #[must_use]
    pub fn programa(&self, idioma: Idioma) -> String {
        match self.tipo {
            Tipo::EscritorioRemoto => en(
                idioma,
                "Escritorio remoto",
                "Remote Desktop",
                "Área de trabalho remota",
            ),
            Tipo::Amenaza | Tipo::Antivirus => self.clave.clone(),
            _ => nombre_legible(&self.clave, idioma),
        }
    }

    /// La frase para la persona. `detallado` añade la cuenta y el equipo de una sesión remota:
    /// solo para el panel, que va con llave.
    #[must_use]
    pub fn texto(&self, idioma: Idioma, detallado: bool) -> String {
        let p = self.programa(idioma);
        match (&self.tipo, &self.detalle) {
            (Tipo::Microfono, _) => en(
                idioma,
                "{p} está usando el micrófono.",
                "{p} is using the microphone.",
                "{p} está usando o microfone.",
            ),
            (Tipo::Camara, _) => en(
                idioma,
                "{p} está usando la cámara.",
                "{p} is using the camera.",
                "{p} está usando a câmera.",
            ),
            (Tipo::Pantalla, _) => en(
                idioma,
                "{p} está capturando la pantalla.",
                "{p} is capturing the screen.",
                "{p} está capturando a tela.",
            ),
            (Tipo::EscritorioRemoto, Detalle::Sombra) => en(
                idioma,
                "Una sesión de Windows está observando otra (sombra de escritorio remoto).",
                "A Windows session is watching another one (Remote Desktop shadowing).",
                "Uma sessão do Windows está observando outra (sombra da área de trabalho remota).",
            ),
            (Tipo::EscritorioRemoto, Detalle::Sesion { usuario, cliente }) => {
                let mut f = en(
                    idioma,
                    "Hay una sesión de escritorio remoto abierta en este ordenador",
                    "A Remote Desktop session is open on this computer",
                    "Há uma sessão de área de trabalho remota aberta neste computador",
                );
                if detallado && !usuario.is_empty() {
                    f.push_str(&en(idioma, " (cuenta {u})", " (account {u})", " (conta {u})"));
                    f = f.replace("{u}", usuario);
                }
                if detallado && !cliente.is_empty() {
                    f.push_str(&en(
                        idioma,
                        ", desde el equipo {c}",
                        ", from the computer {c}",
                        ", a partir do computador {c}",
                    ));
                    f = f.replace("{c}", cliente);
                }
                f.push('.');
                f
            }
            (Tipo::EscritorioRemoto, _) => en(
                idioma,
                "Hay una sesión de escritorio remoto abierta en este ordenador.",
                "A Remote Desktop session is open on this computer.",
                "Há uma sessão de área de trabalho remota aberta neste computador.",
            ),
            (Tipo::ControlRemoto, Detalle::Proceso { servicio: true }) => en(
                idioma,
                "{p} está en marcha como servicio de Windows: puede dejar que otra persona vea o maneje este ordenador.",
                "{p} is running as a Windows service: it can let someone else see or control this computer.",
                "{p} está em execução como serviço do Windows: pode deixar outra pessoa ver ou controlar este computador.",
            ),
            (Tipo::ControlRemoto, _) => en(
                idioma,
                "{p} está en marcha: con él, otra persona puede ver o manejar este ordenador.",
                "{p} is running: with it, someone else can see or control this computer.",
                "{p} está em execução: com ele, outra pessoa pode ver ou controlar este computador.",
            ),
            (Tipo::Grabador, _) => en(
                idioma,
                "{p} está abierto: es un programa para grabar la pantalla.",
                "{p} is open: it is a screen recording program.",
                "{p} está aberto: é um programa para gravar a tela.",
            ),
            (Tipo::Amenaza, _) => en(
                idioma,
                "Microsoft Defender tiene una amenaza activa: {p}.",
                "Microsoft Defender has an active threat: {p}.",
                "O Microsoft Defender tem uma ameaça ativa: {p}.",
            ),
            (Tipo::Antivirus, Detalle::TiempoReal) => en(
                idioma,
                "La protección en tiempo real de Microsoft Defender está apagada.",
                "Microsoft Defender's real-time protection is off.",
                "A proteção em tempo real do Microsoft Defender está desligada.",
            ),
            (Tipo::Antivirus, Detalle::SinAntivirus) => en(
                idioma,
                "Ningún antivirus está protegiendo este ordenador en tiempo real.",
                "No antivirus is protecting this computer in real time.",
                "Nenhum antivírus está protegendo este computador em tempo real.",
            ),
            (Tipo::Antivirus, Detalle::SegundoPlano) => en(
                idioma,
                "Microsoft Defender está en segundo plano y no se pudo comprobar qué antivirus protege este ordenador.",
                "Microsoft Defender is in passive mode and the antivirus protecting this computer could not be checked.",
                "O Microsoft Defender está em segundo plano e não foi possível verificar qual antivírus protege este computador.",
            ),
            (Tipo::Antivirus, _) => en(
                idioma,
                "No se pudo leer el estado de Microsoft Defender.",
                "Microsoft Defender's status could not be read.",
                "Não foi possível ler o estado do Microsoft Defender.",
            ),
        }
        .replace("{p}", &p)
    }
}

/// Lo que el vigía no puede ver, en una frase: se dice siempre junto a lo que ve.
#[must_use]
pub fn limites(idioma: Idioma) -> String {
    en(
        idioma,
        "El vigía ve lo que Windows anota. Un programa que llega al micrófono, la cámara o la pantalla sin pasar por los permisos de Windows, o algo escondido dentro del propio navegador desde el que hablas, puede no aparecer. Una señal es un hecho que Windows registró, no una prueba de espionaje; y no ver ninguna no garantiza que no lo haya.",
        "The watch sees what Windows records. A program that reaches the microphone, the camera or the screen without going through Windows' permissions, or something hidden inside the very browser you are talking from, may not appear. A signal is a fact Windows recorded, not proof of spying; and seeing none does not guarantee there is none.",
        "O vigia vê o que o Windows registra. Um programa que chega ao microfone, à câmera ou à tela sem passar pelas permissões do Windows, ou algo escondido dentro do próprio navegador de onde você fala, pode não aparecer. Um sinal é um fato que o Windows registrou, não uma prova de espionagem; e não ver nenhum não garante que não exista.",
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Unix ms → FILETIME, como lo guarda Windows.
    fn ft(ms: i64) -> u64 {
        (u64::try_from(ms).unwrap() + 11_644_473_600_000) * 10_000
    }

    fn uso(clave: &str, tipo: Tipo, desde: i64, hasta: i64) -> Uso {
        de_registro(
            clave,
            tipo,
            ft(desde),
            if hasta == 0 { 0 } else { ft(hasta) },
            "S-1-5-21-1",
        )
        .unwrap()
    }

    fn proceso(nombre: &str, sesion: u32) -> Proceso {
        Proceso {
            pid: 100,
            nombre: nombre.to_owned(),
            sesion: Some(sesion),
        }
    }

    fn lectura(usos: Vec<Uso>, procesos: Vec<Proceso>) -> Lectura {
        Lectura {
            cuando: 5_000_000,
            usos: Some(usos),
            procesos: Some(procesos),
            sesiones: Some(Vec::new()),
            defensa: None,
        }
    }

    fn chrome() -> Propio {
        Propio {
            carpeta: Some("c:\\program files\\guardiana\\".to_owned()),
            llamante: Some(Llamante {
                nombre: "chrome.exe".to_owned(),
                ruta: Some("C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe".to_owned()),
                como: Como::Conexion,
            }),
        }
    }

    #[test]
    fn las_entradas_del_registro_se_leen_con_su_programa() {
        let u = uso(
            "C:#Program Files#Zoom#bin#Zoom.exe",
            Tipo::Microfono,
            1_000_000,
            0,
        );
        assert_eq!(u.programa, "Zoom.exe");
        assert_eq!(
            u.ruta.as_deref(),
            Some("C:\\Program Files\\Zoom\\bin\\Zoom.exe")
        );
        assert_eq!((u.desde, u.hasta), (1_000_000, None));
        let s = uso(
            "Microsoft.WindowsCamera_8wekyb3d8bbwe",
            Tipo::Camara,
            5_000,
            9_000,
        );
        assert_eq!(
            (s.programa.as_str(), s.ruta, s.hasta),
            ("Microsoft.WindowsCamera", None, Some(9_000))
        );
        let p = uso("C:#x#obs64.exe", Tipo::Pantalla, 7_000, 0);
        assert_eq!((p.tipo, p.cuenta.as_str()), (Tipo::Pantalla, "S-1-5-21-1"));
        // Nunca usado: sin inicio.
        assert!(de_registro("C:#x.exe", Tipo::Camara, 0, 0, "equipo").is_none());
        // Un final anterior al inicio es del uso de antes: lo usa ahora.
        assert_eq!(uso("C:#y.exe", Tipo::Camara, 20_000, 10_000).hasta, None);
        // Las cuatro capacidades del ConsentStore, y la pantalla dos veces.
        let nombres: Vec<&str> = CAPACIDADES.iter().map(|(c, _)| *c).collect();
        assert_eq!(
            nombres,
            [
                "microphone",
                "webcam",
                "graphicsCaptureProgrammatic",
                "graphicsCaptureWithoutBorder"
            ]
        );
    }

    #[test]
    fn solo_cuentan_los_usos_de_ahora_de_otros_programas_que_siguen_abiertos() {
        let usos = vec![
            // Otro programa con el micrófono ahora: señal.
            uso(
                "C:#Program Files#Zoom#bin#Zoom.exe",
                Tipo::Microfono,
                4_000_000,
                0,
            ),
            // El mismo, en otra cuenta: la misma señal.
            de_registro(
                "C:#Program Files#Zoom#bin#Zoom.exe",
                Tipo::Microfono,
                ft(4_100_000),
                0,
                "S-1-5-21-2",
            )
            .unwrap(),
            // Ya terminó: no.
            uso("C:#a#viejo.exe", Tipo::Camara, 1_000, 2_000),
            // El navegador de la llamada: no.
            uso(
                "C:#Program Files#Google#Chrome#Application#chrome.exe",
                Tipo::Microfono,
                4_000_000,
                0,
            ),
            uso(
                "C:#Program Files#Google#Chrome#Application#chrome.exe",
                Tipo::Camara,
                4_000_000,
                0,
            ),
            // GUARDIANA, por archivo y por carpeta: no.
            uso(
                "C:#Program Files#Guardiana#ZERO#GUARDIANA ZERO.exe",
                Tipo::Camara,
                4_000_000,
                0,
            ),
            uso(
                "C:#Program Files#Guardiana#otro.exe",
                Tipo::Camara,
                4_000_000,
                0,
            ),
            uso("D:#guardiana-zero.exe", Tipo::Microfono, 4_000_000, 0),
            // Windows lo da por abierto, pero el programa ya no corre: no.
            uso("C:#a#cerrado.exe", Tipo::Microfono, 4_000_000, 0),
            // Captura de pantalla por las dos capacidades: una señal.
            uso("C:#a#capturador.exe", Tipo::Pantalla, 4_000_000, 0),
            uso("C:#a#capturador.exe", Tipo::Pantalla, 4_200_000, 0),
            // Un paquete de la Tienda: se toma por lo que dice Windows.
            uso(
                "Microsoft.WindowsCamera_8wekyb3d8bbwe",
                Tipo::Camara,
                4_000_000,
                0,
            ),
            // Otro Chrome (otra ruta, otra cuenta): sí cuenta.
            uso(
                "C:#Users#ana#AppData#Local#Google#Chrome#Application#chrome.exe",
                Tipo::Microfono,
                4_000_000,
                0,
            ),
        ];
        let procesos = vec![
            proceso("Zoom.exe", 1),
            proceso("chrome.exe", 1),
            proceso("capturador.exe", 1),
        ];
        let h = analiza(&lectura(usos, procesos), &chrome());
        let vistos: Vec<(Tipo, &str)> = h.iter().map(|x| (x.tipo, x.clave.as_str())).collect();
        assert_eq!(
            vistos,
            [
                (Tipo::Microfono, "chrome.exe"),
                (Tipo::Microfono, "Zoom.exe"),
                (Tipo::Camara, "Microsoft.WindowsCamera"),
                (Tipo::Pantalla, "capturador.exe"),
            ],
            "{h:#?}"
        );
        assert!(h.iter().all(|x| x.nivel == Nivel::Alerta));
        assert_eq!(
            h[1].texto(Idioma::Es, false),
            "Zoom está usando el micrófono."
        );
        assert_eq!(
            h[2].texto(Idioma::En, false),
            "Windows Camera is using the camera."
        );
        assert_eq!(
            h[3].texto(Idioma::Pt, false),
            "capturador está capturando a tela."
        );
        // Sin saber quién pregunta, el Chrome de la llamada sí sería una señal.
        let sin = analiza(
            &lectura(
                vec![uso(
                    "C:#Program Files#Google#Chrome#Application#chrome.exe",
                    Tipo::Microfono,
                    1,
                    0,
                )],
                vec![proceso("chrome.exe", 1)],
            ),
            &Propio::default(),
        );
        assert_eq!(sin.len(), 1);
    }

    #[test]
    fn el_navegador_se_reconoce_por_su_archivo_si_no_hay_ruta() {
        let propio = Propio {
            carpeta: None,
            llamante: Some(Llamante {
                nombre: navegador_de_agente("Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:131.0) Gecko/20100101 Firefox/131.0")
                    .unwrap()
                    .to_owned(),
                ruta: None,
                como: Como::Presentacion,
            }),
        };
        let h = analiza(
            &lectura(
                vec![
                    uso(
                        "C:#Program Files#Mozilla Firefox#firefox.exe",
                        Tipo::Microfono,
                        1,
                        0,
                    ),
                    uso(
                        "C:#Program Files#Google#Chrome#Application#chrome.exe",
                        Tipo::Camara,
                        1,
                        0,
                    ),
                ],
                vec![proceso("firefox.exe", 1), proceso("chrome.exe", 1)],
            ),
            &propio,
        );
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].clave, "chrome.exe");
        let edge = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0";
        assert_eq!(navegador_de_agente(edge), Some("msedge.exe"));
        assert_eq!(
            navegador_de_agente("Mozilla/5.0 Chrome/130.0.0.0 Safari/537.36"),
            Some("chrome.exe")
        );
        assert_eq!(
            navegador_de_agente("Mozilla/5.0 Chrome/130 OPR/114.0"),
            Some("opera.exe")
        );
        assert_eq!(navegador_de_agente("curl/8.0"), None);
    }

    #[test]
    fn los_programas_de_control_remoto_y_los_grabadores_se_reconocen_por_producto() {
        let procesos = vec![
            proceso("TeamViewer_Service.exe", 0),
            proceso("TeamViewer.exe", 1),
            proceso("tv_x64.exe", 1),
            proceso("AnyDesk.exe", 0),
            proceso("rustdesk.exe", 2),
            proceso("QuickAssist.exe", 1),
            proceso("obs64.exe", 1),
            proceso("mstsc.exe", 1),
            proceso("notepad.exe", 1),
            proceso("svchost.exe", 0),
        ];
        let h = analiza(&lectura(Vec::new(), procesos), &chrome());
        let vistos: Vec<(Tipo, String, bool)> = h
            .iter()
            .map(|x| {
                let servicio = matches!(x.detalle, Detalle::Proceso { servicio: true });
                (x.tipo, x.programa(Idioma::Es), servicio)
            })
            .collect();
        assert_eq!(
            vistos,
            [
                (Tipo::ControlRemoto, "AnyDesk".to_owned(), true),
                (Tipo::ControlRemoto, "Asistencia rápida".to_owned(), false),
                (Tipo::ControlRemoto, "RustDesk".to_owned(), false),
                (Tipo::ControlRemoto, "TeamViewer".to_owned(), false),
                (Tipo::Grabador, "OBS Studio".to_owned(), false),
            ],
            "{h:#?}"
        );
        assert_eq!(
            h[0].texto(Idioma::Es, false),
            "AnyDesk está en marcha como servicio de Windows: puede dejar que otra persona vea o maneje este ordenador."
        );
        assert_eq!(
            h[3].texto(Idioma::En, false),
            "TeamViewer is running: with it, someone else can see or control this computer."
        );
        assert_eq!(
            h[4].texto(Idioma::Es, false),
            "OBS Studio está abierto: es un programa para grabar la pantalla."
        );
        // Ningún archivo está en las dos listas, y todos van en minúsculas.
        for (a, _) in CONTROL_REMOTO {
            assert_eq!(*a, a.to_lowercase());
            assert!(!GRABADORES.iter().any(|(g, _)| g == a), "{a}");
        }
        assert!(es_vigilado("AnyDesk.exe") && es_vigilado("PSR.EXE") && !es_vigilado("chrome.exe"));
        assert_eq!(
            nombre_legible("psr.exe", Idioma::En),
            "Windows Steps Recorder"
        );
        assert_eq!(nombre_legible("Desconocido.EXE", Idioma::Es), "Desconocido");
        assert_eq!(nombre_legible("MSTeams", Idioma::Es), "Microsoft Teams");
    }

    #[test]
    fn el_escritorio_remoto_se_ve_por_sus_sesiones() {
        let sesion = |id, estado, protocolo, puesto: &str| Sesion {
            id,
            estado,
            protocolo,
            puesto: puesto.to_owned(),
            usuario: "ana".to_owned(),
            cliente: "PORTATIL-X".to_owned(),
        };
        let mut l = lectura(Vec::new(), Vec::new());
        // La consola, los servicios y un puesto RDP esperando: nada.
        l.sesiones = Some(vec![
            sesion(0, 4, 0, "Services"),
            sesion(1, 0, 0, "Console"),
            sesion(65536, 6, 0, "RDP-Tcp"),
        ]);
        assert!(analiza(&l, &chrome()).is_empty());
        l.sesiones = Some(vec![
            sesion(1, 0, 0, "Console"),
            sesion(2, 0, 2, "RDP-Tcp#0"),
        ]);
        let h = analiza(&l, &chrome());
        assert_eq!(h.len(), 1);
        assert_eq!(
            h[0].texto(Idioma::Es, false),
            "Hay una sesión de escritorio remoto abierta en este ordenador."
        );
        assert_eq!(
            h[0].texto(Idioma::Es, true),
            "Hay una sesión de escritorio remoto abierta en este ordenador (cuenta ana), desde el equipo PORTATIL-X."
        );
        l.sesiones = Some(vec![sesion(3, 3, 0, "Console")]);
        let h = analiza(&l, &chrome());
        assert_eq!(h[0].detalle, Detalle::Sombra);
        assert_eq!(h[0].programa(Idioma::Pt), "Área de trabalho remota");
    }

    #[test]
    fn la_salida_del_guion_de_defender_se_lee() {
        let texto = "D\tTrue\tTrue\tFalse\tNormal\r\nTA\t1\nT\tTrojan:Win32/Prueba!ml\t5\nE\tcentro\tInvalid namespace\n";
        let d = de_salida_defensa(texto, 7);
        assert_eq!(d.cuando, 7);
        assert_eq!(
            d.defender,
            Some(Defender {
                servicio: true,
                antivirus: true,
                tiempo_real: false,
                modo: "Normal".to_owned()
            })
        );
        assert_eq!(
            d.amenazas.as_ref().unwrap()[0].nombre,
            "Trojan:Win32/Prueba!ml"
        );
        assert_eq!(d.amenazas.as_ref().unwrap()[0].gravedad, 5);
        assert!(d.antivirus.is_none(), "the Security Center did not answer");
        assert_eq!(
            d.errores.get("centro").map(String::as_str),
            Some("Invalid namespace")
        );
        // Sin la línea TA, las amenazas no se leyeron: no es «ninguna».
        let d = de_salida_defensa(
            "D\tTrue\tTrue\tTrue\tNormal\nAA\tok\nA\tWindows Defender\t397568\n",
            1,
        );
        assert!(d.amenazas.is_none());
        assert_eq!(
            d.antivirus,
            Some(vec![("Windows Defender".to_owned(), true)])
        );
        assert_eq!(
            de_salida_defensa("", 1),
            Defensa {
                cuando: 1,
                ..Defensa::default()
            }
        );
    }

    #[test]
    fn defender_avisa_solo_cuando_no_protege_nadie_o_hay_amenazas() {
        let def = |tiempo_real: bool, modo: &str| Defender {
            servicio: true,
            antivirus: true,
            tiempo_real,
            modo: modo.to_owned(),
        };
        let con = |defender: Option<Defender>,
                   antivirus: Option<Vec<(String, bool)>>,
                   amenazas: Vec<AmenazaActiva>| {
            let mut l = lectura(Vec::new(), Vec::new());
            l.defensa = Some(Defensa {
                cuando: 1,
                defender,
                amenazas: Some(amenazas),
                antivirus,
                errores: BTreeMap::new(),
            });
            analiza(&l, &chrome())
                .into_iter()
                .map(|h| (h.nivel, h.detalle))
                .collect::<Vec<_>>()
        };
        // Defender protege con el tiempo real encendido: nada.
        assert!(con(Some(def(true, "Normal")), None, Vec::new()).is_empty());
        // Apagado siendo él quien protege: alerta.
        assert_eq!(
            con(Some(def(false, "Normal")), None, Vec::new()),
            [(Nivel::Alerta, Detalle::TiempoReal)]
        );
        // Apartado porque protege otro: nada.
        let otro = Some(vec![
            ("Windows Defender".to_owned(), false),
            ("ESET Security".to_owned(), true),
        ]);
        assert!(con(Some(def(false, "Passive Mode")), otro, Vec::new()).is_empty());
        // Apartado y nadie protege: alerta.
        let nadie = Some(vec![("ESET Security".to_owned(), false)]);
        assert_eq!(
            con(Some(def(false, "Passive Mode")), nadie, Vec::new()),
            [(Nivel::Alerta, Detalle::SinAntivirus)]
        );
        // Apartado y sin Centro de seguridad (Windows Server): solo se dice.
        assert_eq!(
            con(Some(def(false, "Passive Mode")), None, Vec::new()),
            [(Nivel::Aviso, Detalle::SegundoPlano)]
        );
        // Sin poder leerlo: solo se dice.
        assert_eq!(
            con(None, None, Vec::new()),
            [(Nivel::Aviso, Detalle::NoLeido)]
        );
        // Una amenaza activa: alerta, con su nombre.
        let a = AmenazaActiva {
            nombre: "Trojan:Win32/X".to_owned(),
            gravedad: 5,
        };
        assert_eq!(
            con(Some(def(true, "Normal")), None, vec![a]),
            [(Nivel::Alerta, Detalle::Amenaza { gravedad: 5 })]
        );
        // Las alertas van antes que los avisos.
        let mut l = lectura(Vec::new(), vec![proceso("AnyDesk.exe", 1)]);
        l.defensa = Some(Defensa {
            cuando: 1,
            ..Defensa::default()
        });
        let h = analiza(&l, &chrome());
        assert_eq!((h[0].nivel, h[1].nivel), (Nivel::Alerta, Nivel::Aviso));
        assert_eq!(
            h[1].texto(Idioma::Es, false),
            "No se pudo leer el estado de Microsoft Defender."
        );
    }

    #[test]
    fn el_dueno_de_una_conexion_se_busca_en_la_tabla() {
        let ip = |a: [u8; 4]| u32::from_ne_bytes(a);
        let puerto = |p: u16| {
            let b = p.to_be_bytes();
            u32::from_ne_bytes([b[0], b[1], 0, 0])
        };
        let tabla = [
            2,
            5,
            ip([127, 0, 0, 1]),
            puerto(7443),
            ip([127, 0, 0, 1]),
            puerto(51000),
            900,
            5,
            ip([127, 0, 0, 1]),
            puerto(51000),
            ip([127, 0, 0, 1]),
            puerto(7443),
            4321,
        ];
        assert_eq!(pid_de_tabla(&tabla, [127, 0, 0, 1], 51000), Some(4321));
        assert_eq!(pid_de_tabla(&tabla, [127, 0, 0, 1], 7443), Some(900));
        assert_eq!(pid_de_tabla(&tabla, [127, 0, 0, 1], 51001), None);
        assert_eq!(pid_de_tabla(&tabla, [10, 0, 0, 1], 51000), None);
        // Una tabla que dice tener más filas de las que trae no se lee fuera de sí.
        assert_eq!(pid_de_tabla(&[9, 1, 2], [127, 0, 0, 1], 1), None);
        assert_eq!(pid_de_tabla(&[], [127, 0, 0, 1], 1), None);
    }

    #[test]
    fn los_idiomas_y_los_limites() {
        assert_eq!(Idioma::de("en-US,en;q=0.9,es;q=0.8"), Idioma::En);
        assert_eq!(Idioma::de("pt-BR"), Idioma::Pt);
        assert_eq!(Idioma::de("fr-FR,fr"), Idioma::Es);
        assert_eq!(Idioma::de(""), Idioma::Es);
        for i in [Idioma::Es, Idioma::En, Idioma::Pt] {
            let l = limites(i);
            // Nunca se promete lo que no se puede: ni «100 %» ni «imposible».
            for prohibido in ["100", "imposible", "impossible", "impossível"] {
                assert!(!l.to_lowercase().contains(prohibido), "{l}");
            }
        }
    }
}
