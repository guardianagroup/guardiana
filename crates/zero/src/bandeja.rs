//! The VPN next to Windows' clock (5 Nov 2026; the owner, 3 Nov 2026, on the icon and the notices of the VPN: «Hazlo
//! claro»): GUARDIANA's icon in the notification area with the VPN's colour, the lines under the pointer, its menu, and
//! Windows' notices when the VPN connects, stops answering, comes back, stops or goes off.
//!
//! Decided here, and tested on every system; `GUARDIANA ZERO.exe --avisos` (zero/navegador/src/avisos.rs) draws it: the
//! process without a window that already shows el escudo's notices, started at every sign-in by the installer's Run
//! entry and by every start of ZERO, one per session, so the icon stays with ZERO closed too (but for a repair, or an
//! update without the installer's whole interface, which close it and do not open ZERO again: then it is back at the
//! next sign-in or the next start of ZERO; docs/VPN.md). Without GUARDIANA's service there is no icon ([`se_ve`]). It
//! asks the service on this computer as the CLI does (`crates/cli/src/vpn_cmd.rs`): `GET /api/vpn` on 127.0.0.1:7443
//! with the panel's key in `X-Guardiana-Token`, every [`CADA`], and only when the one listening there is the service's
//! own process ([`llave_al_puerto`]); «Conectar» and «Desconectar» are the panel's own orders.
//!
//! The icon says what the VPN chip of ZERO's bar (zero/navegador/interfaz/barra.js) says, with the chip's own words (its
//! texts, `vpn_*`) wherever the chip says it, and what Órbita's VPN page says where the chip says nothing ([`Cara`]):
//!
//! - The colour ([`color`]): green connected; amber connecting; red while nothing of the computer's programs reaches the
//!   internet (its server does not answer, strict mode trying again, or the VPN stopped with its block still in
//!   Windows); grey off, not on this computer, or GUARDIANA not answering, with why when Windows tells it ([`Falta`]).
//! - The lines under the pointer ([`linea`]): «GUARDIANA», the state, and where it goes out («Finlandia · Helsinki»,
//!   written as the VPN page writes it).
//! - The menu ([`menu`]): the state and the place (not clickable), «Abrir la VPN», «Conectar a la salida más rápida»
//!   when it can connect, «Desconectar» as the chip offers it, the Start menu's «Apagar la VPN de GUARDIANA» while
//!   GUARDIANA does not answer, and «Abrir GUARDIANA».
//! - The notices ([`aviso`]): one per change of what the icon says; none for the first reading or a service that stops
//!   answering, and none for the connection or the disconnection the person has just ordered from this menu
//!   ([`PROPIO`]); and one for an order of the menu the service did not take ([`aviso_de_orden`]).

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use crate::textos::{Idioma, Textos};

/// What a click on the icon, on «Abrir la VPN» or on a notice opens: Órbita on the VPN.
pub const ENLACE: &str = "guardiana://vpn";
/// How often the icon asks the service.
pub const CADA: Duration = Duration::from_secs(3);
/// After an order from this menu, how long its own outcome is not announced: the person has just pressed it.
pub const PROPIO: Duration = Duration::from_secs(60);
/// The longest line under the pointer Windows keeps (`NOTIFYICONDATAW.szTip`: 128 UTF-16 units with its end).
pub const LARGO_LINEA: usize = 127;
/// How long a reading of the VPN may take.
pub const ESPERA_LECTURA: Duration = Duration::from_secs(4);
/// How long an order may take: closing the tunnel can take a few seconds (the CLI waits up to 40).
pub const ESPERA_ORDEN: Duration = Duration::from_secs(40);
/// The service's name in Windows (`guardiana_service::daemon::SERVICE_NAME`, build/wix/guardiana.wxs).
pub const SERVICIO: &str = "guardiana";

/// In what the VPN is, as the service says it (`crates/panel/src/vpn.rs`, `Fase`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Fase {
    /// The service did not answer (it is starting or stopped, the key could not be read, or someone else listens on
    /// its port).
    #[default]
    Desconocida,
    /// Off.
    Apagada,
    /// Connecting.
    Conectando,
    /// Connected and its server answering.
    Conectada,
    /// Its server stopped answering; the tunnel and its cut-off stay, so nothing leaves outside it.
    SinRespuesta,
    /// Strict mode: its server does not answer, the tunnel is closed and the block stays while it tries again.
    Reconectando,
    /// It could not connect.
    Error,
}

/// Why GUARDIANA did not answer the icon ([`Fase::Desconocida`]), as far as it can be told without the panel's key
/// (review of bandeja-vpn-2, finding 5): causes that are not the same are not said with the same words.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Falta {
    /// The one listening is the service and the key was read, but the reading got no answer.
    #[default]
    Respuesta,
    /// Windows' service manager answered that GUARDIANA's service is not installed (ZERO alone): there is no VPN on
    /// this computer, and no icon ([`se_ve`]).
    SinGuardiana,
    /// Windows' service manager says the service is stopped (`SERVICE_STOPPED`).
    Parada,
    /// Windows is starting or stopping the service, or it runs but no one listens yet on the panel's port, or it cannot
    /// be told who does.
    Arrancando,
    /// The service runs, and another program listens on the panel's port.
    OtroEnPuerto,
    /// This Windows account cannot read the panel's key (the folder is the console account's, the system's and the
    /// administrators').
    SinLlave,
}

/// Why the panel's key may not go to the one listening on the panel's port ([`llave_al_puerto`] said no), from what
/// Windows says: whether the service is not installed (`sin_servicio`), its state ([`estado_del_servicio`]; `None` when
/// the service manager could not be asked), the process it runs in (`servicio`), and the one listening on the port
/// without doubt (`escucha`). «Stopped» only when Windows says so; while it starts or stops, «not answering yet»; when it
/// cannot be told, the general words (second review of bandeja-vpn-2, point 5).
#[must_use]
pub fn falta_del_puerto(
    sin_servicio: bool,
    estado: Option<u32>,
    servicio: Option<u32>,
    escucha: Option<u32>,
) -> Falta {
    if sin_servicio {
        Falta::SinGuardiana
    } else if servicio.is_some() {
        if escucha.is_none() {
            Falta::Arrancando
        } else {
            Falta::OtroEnPuerto
        }
    } else {
        match estado {
            // SERVICE_STOPPED
            Some(1) => Falta::Parada,
            // SERVICE_START_PENDING, SERVICE_STOP_PENDING, SERVICE_CONTINUE_PENDING, SERVICE_PAUSE_PENDING
            Some(2 | 3 | 5 | 6) => Falta::Arrancando,
            // running without a process to name, paused, or no answer from the service manager
            _ => Falta::Respuesta,
        }
    }
}

/// `guardiana.exe` from the program Windows' service manager runs as GUARDIANA's service (its `lpBinaryPathName`, as
/// `QueryServiceConfigW` gives it: `"…\Guardiana\guardiana.exe" service run`, build/wix/guardiana.wxs), which only an
/// administrator can change; never from where ZERO runs, which can be a folder of the account (second review of
/// bandeja-vpn-2, point 5). `None` unless it is an absolute path to a file called `guardiana.exe`.
#[must_use]
pub fn guardiana_del_servicio(imagen: &str) -> Option<String> {
    let imagen = imagen.trim();
    let ruta = if let Some(resto) = imagen.strip_prefix('"') {
        &resto[..resto.find('"')?]
    } else {
        // without quotes, up to the first «.exe», which must end the path (a space or nothing after it): the strictest
        // reading, stricter than Windows', which with spaces would first try «C:\Program.exe» (second review of the
        // icon's fixes, finding 1)
        let fin = imagen.to_ascii_lowercase().find(".exe")? + ".exe".len();
        if imagen
            .as_bytes()
            .get(fin)
            .is_some_and(|b| !b.is_ascii_whitespace())
        {
            return None;
        }
        &imagen[..fin]
    };
    let nombre = ruta.rsplit(['\\', '/']).next()?;
    let unidad = ruta.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        && ruta.as_bytes().get(1..3) == Some(b":\\".as_slice());
    let absoluta = unidad || ruta.starts_with(r"\\");
    (absoluta && nombre.eq_ignore_ascii_case("guardiana.exe") && !ruta.contains(".."))
        .then(|| ruta.to_owned())
}

/// The VPN as the service says it (`GET /api/vpn`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Vpn {
    /// In what it is.
    pub fase: Fase,
    /// Why it is not known ([`Fase::Desconocida`] only).
    pub falta: Falta,
    /// Strict mode is on.
    pub estricto: bool,
    /// «Desconectar» is needed (`"desconectar"`): the VPN is on, or its block is still in Windows.
    pub desconectar: bool,
    /// The exit's country, two letters (`FI`); empty when it is not one of GUARDIANA's exits.
    pub pais: String,
    /// The exit's city.
    pub ciudad: String,
    /// This computer can have the VPN (Windows, with its driver).
    pub disponible: bool,
}

impl Vpn {
    /// What `GET /api/vpn` says. Anything that is not its answer is [`Fase::Desconocida`].
    #[must_use]
    pub fn de_json(v: &Value) -> Self {
        let fase = match v["fase"].as_str().unwrap_or_default() {
            "apagada" => Fase::Apagada,
            "conectando" => Fase::Conectando,
            "conectada" => Fase::Conectada,
            "sin_respuesta" => Fase::SinRespuesta,
            "reconectando" => Fase::Reconectando,
            "error" => Fase::Error,
            _ => return Self::default(),
        };
        let texto = |x: &Value, largo: usize| -> String {
            x.as_str()
                .unwrap_or_default()
                .chars()
                .filter(|c| !c.is_control())
                .take(largo)
                .collect()
        };
        let pais = texto(&v["salida"]["pais"], 2);
        Self {
            fase,
            falta: Falta::default(),
            estricto: v["estricto"].as_bool() == Some(true),
            desconectar: v["desconectar"].as_bool() == Some(true),
            pais: if pais.len() == 2 && pais.bytes().all(|b| b.is_ascii_uppercase()) {
                pais
            } else {
                String::new()
            },
            ciudad: texto(&v["salida"]["ciudad"], 40),
            disponible: v["disponible"].as_bool() == Some(true),
        }
    }

    /// The VPN not known, for the reason `falta`.
    #[must_use]
    pub fn sin(falta: Falta) -> Self {
        Self {
            falta,
            ..Self::default()
        }
    }

    /// The VPN is on: connecting, connected, or without its server.
    #[must_use]
    pub fn encendida(&self) -> bool {
        matches!(
            self.fase,
            Fase::Conectando | Fase::Conectada | Fase::SinRespuesta | Fase::Reconectando
        )
    }

    /// The VPN is not on but its block is still in Windows: no program of Windows reaches the internet until
    /// «Desconectar» takes it away (the chip's «VPN parada, con su bloqueo puesto»).
    #[must_use]
    pub fn bloqueada(&self) -> bool {
        self.desconectar && !self.encendida() && self.fase != Fase::Desconocida
    }
}

/// What the icon says: the chip's states, and those the chip does not show because it hides while the VPN is off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cara {
    /// GUARDIANA's service does not answer on this computer, for this reason.
    SinServicio(Falta),
    /// This computer cannot have the VPN.
    NoDisponible,
    /// Off.
    Desconectada,
    /// It could not connect, and nothing of it stayed in Windows.
    Error,
    /// Connecting.
    Conectando,
    /// Connected.
    Conectada,
    /// Its server does not answer.
    SinRespuesta,
    /// Strict mode: it does not connect and the block stays.
    Reconectando,
    /// Stopped, with its block still in Windows.
    Bloqueada,
}

impl Cara {
    /// What the icon says for `v`.
    #[must_use]
    pub fn de(v: &Vpn) -> Self {
        match v.fase {
            Fase::Desconocida => Self::SinServicio(v.falta),
            Fase::Conectando => Self::Conectando,
            Fase::Conectada => Self::Conectada,
            Fase::SinRespuesta => Self::SinRespuesta,
            Fase::Reconectando => Self::Reconectando,
            Fase::Apagada | Fase::Error if v.desconectar => Self::Bloqueada,
            Fase::Apagada if !v.disponible => Self::NoDisponible,
            Fase::Apagada => Self::Desconectada,
            Fase::Error => Self::Error,
        }
    }

    /// The key of its words: the chip's own text wherever the chip says it.
    #[must_use]
    pub fn clave(self) -> &'static str {
        match self {
            Self::SinServicio(Falta::Respuesta) => "bandeja_sin_servicio",
            Self::SinServicio(Falta::SinGuardiana) => "bandeja_sin_guardiana",
            Self::SinServicio(Falta::Parada) => "bandeja_parada",
            Self::SinServicio(Falta::Arrancando) => "bandeja_arrancando",
            Self::SinServicio(Falta::OtroEnPuerto) => "bandeja_otro_puerto",
            Self::SinServicio(Falta::SinLlave) => "bandeja_sin_llave",
            Self::NoDisponible => "bandeja_no_disponible",
            Self::Desconectada => "bandeja_desconectada",
            Self::Error => "bandeja_error",
            Self::Conectando => "vpn_conectando",
            Self::Conectada => "vpn_conectada",
            Self::SinRespuesta => "vpn_sin_respuesta",
            Self::Reconectando => "vpn_reconectando",
            Self::Bloqueada => "vpn_bloqueo_puesto",
        }
    }

    /// Nothing of the computer's programs reaches the internet: its server is silent, or the block stays.
    #[must_use]
    pub fn sin_internet(self) -> bool {
        matches!(
            self,
            Self::SinRespuesta | Self::Reconectando | Self::Bloqueada
        )
    }
}

/// The icon's colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    /// Connected.
    Verde,
    /// Off, not on this computer, or GUARDIANA not answering.
    Gris,
    /// Connecting.
    Ambar,
    /// Nothing reaches the internet: its server does not answer, or its block stays.
    Rojo,
}

impl Color {
    /// The name of its icon file (`vpn-<name>.ico`, build/wix/iconos_vpn.py).
    #[must_use]
    pub fn nombre(self) -> &'static str {
        match self {
            Self::Verde => "verde",
            Self::Gris => "gris",
            Self::Ambar => "ambar",
            Self::Rojo => "rojo",
        }
    }
}

/// Whether the icon is shown for `v`: always, but on a computer without GUARDIANA's service (ZERO alone), which has no
/// VPN.
#[must_use]
pub fn se_ve(v: &Vpn) -> bool {
    !(v.fase == Fase::Desconocida && v.falta == Falta::SinGuardiana)
}

/// The icon's colour for `v`.
#[must_use]
pub fn color(v: &Vpn) -> Color {
    let c = Cara::de(v);
    match c {
        Cara::Conectada => Color::Verde,
        Cara::Conectando => Color::Ambar,
        _ if c.sin_internet() => Color::Rojo,
        _ => Color::Gris,
    }
}

/// Where the VPN goes out, as Órbita's VPN page writes it: «Finlandia · Helsinki», the country by its name in the
/// person's language (the panel's own list, `guardiana_core::i18n`); empty when it is not one of GUARDIANA's exits.
#[must_use]
pub fn lugar(v: &Vpn, t: &Textos) -> String {
    let pais = if v.pais.is_empty() {
        String::new()
    } else {
        let nombre = guardiana_core::i18n::by_code(t.idioma().codigo()).pais(&v.pais);
        if nombre.is_empty() {
            v.pais.clone()
        } else {
            nombre.to_owned()
        }
    };
    [pais, v.ciudad.clone()]
        .into_iter()
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

/// What the VPN is doing, in the chip's words, for the menu's first line, the line under the pointer and the notices.
#[must_use]
pub fn estado(v: &Vpn, t: &Textos) -> String {
    t.t(Cara::de(v).clave())
}

/// The lines under the pointer: «GUARDIANA», the state and, when the service says it, where it goes out; cut to what
/// Windows keeps.
#[must_use]
pub fn linea(v: &Vpn, t: &Textos) -> String {
    let mut s = format!("GUARDIANA\n{}", estado(v, t));
    let donde = lugar(v, t);
    if !donde.is_empty() {
        s.push('\n');
        s.push_str(&donde);
    }
    let mut out = String::new();
    let mut unidades = 0;
    for c in s.chars() {
        unidades += c.len_utf16();
        if unidades > LARGO_LINEA {
            break;
        }
        out.push(c);
    }
    out
}

/// What the menu offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orden {
    /// The state, as a line that cannot be clicked.
    Estado,
    /// Órbita on the VPN ([`ENLACE`]).
    AbrirVpn,
    /// Connect to the fastest exit.
    Conectar,
    /// Disconnect, in any phase: also the way out of a block that stayed.
    Desconectar,
    /// GUARDIANA's window.
    AbrirGuardiana,
    /// Where it goes out, as a line that cannot be clicked.
    Lugar,
    /// What the Start menu's «Apagar la VPN de GUARDIANA» runs (`guardiana.exe vpn apaga --menu`, which asks Windows
    /// for an administrator's rights itself): the way out of a block that stayed while GUARDIANA does not answer.
    ApagarVpn,
}

impl Orden {
    /// Its number in Windows' menu (never 0: `TrackPopupMenu` answers 0 when nothing was chosen).
    #[must_use]
    pub fn id(self) -> u32 {
        match self {
            Self::Estado => 1,
            Self::AbrirVpn => 2,
            Self::Conectar => 3,
            Self::Desconectar => 4,
            Self::AbrirGuardiana => 5,
            Self::Lugar => 6,
            Self::ApagarVpn => 7,
        }
    }

    /// The order with number `id`.
    #[must_use]
    pub fn de_id(id: u32) -> Option<Self> {
        [
            Self::Estado,
            Self::AbrirVpn,
            Self::Conectar,
            Self::Desconectar,
            Self::AbrirGuardiana,
            Self::Lugar,
            Self::ApagarVpn,
        ]
        .into_iter()
        .find(|o| o.id() == id)
    }
}

/// One line of the menu: the order, its words, whether it can be chosen, and whether a separator goes before it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entrada {
    /// What it does.
    pub orden: Orden,
    /// Its words.
    pub texto: String,
    /// It can be chosen.
    pub activa: bool,
    /// A separator goes before it.
    pub raya: bool,
}

/// The menu for `v`, with `desconectando` while a «Desconectar» from it has not answered yet: the state and the place;
/// «Abrir la VPN» (what a click on the icon does); «Conectar a la salida más rápida» when it can connect, and
/// «Desconectar» while it is on or its block stays, as the chip offers it; while GUARDIANA does not answer and
/// `guardiana.exe` is there (`apagar`), the Start menu's «Apagar la VPN de GUARDIANA», since a block of strict mode
/// stays in Windows without the service and no one can tell it from here; «Abrir GUARDIANA».
#[must_use]
pub fn menu(v: &Vpn, desconectando: bool, apagar: bool, t: &Textos) -> Vec<Entrada> {
    let entrada = |orden, texto: String, activa, raya| Entrada {
        orden,
        texto,
        activa,
        raya,
    };
    let mut m = vec![entrada(Orden::Estado, estado(v, t), false, false)];
    let donde = lugar(v, t);
    if !donde.is_empty() {
        m.push(entrada(Orden::Lugar, donde, false, false));
    }
    m.push(entrada(
        Orden::AbrirVpn,
        t.t("bandeja_abrir_vpn"),
        true,
        true,
    ));
    if v.disponible && matches!(v.fase, Fase::Apagada | Fase::Error) {
        m.push(entrada(
            Orden::Conectar,
            t.t("bandeja_conectar"),
            true,
            false,
        ));
    }
    if v.encendida() || v.bloqueada() {
        let clave = if desconectando {
            "vpn_desconectando"
        } else {
            "vpn_desconectar"
        };
        m.push(entrada(
            Orden::Desconectar,
            t.t(clave),
            !desconectando,
            false,
        ));
    }
    if apagar && v.fase == Fase::Desconocida && v.falta != Falta::SinGuardiana {
        m.push(entrada(
            Orden::ApagarVpn,
            t.t("bandeja_apagar_vpn"),
            true,
            false,
        ));
    }
    m.push(entrada(
        Orden::AbrirGuardiana,
        t.t("bandeja_abrir_guardiana"),
        true,
        true,
    ));
    m
}

/// A Windows notice of the VPN.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aviso {
    /// Its title: the state, in the chip's words.
    pub titulo: String,
    /// Where it goes out, on its own line (empty: no line).
    pub lugar: String,
    /// Its text.
    pub texto: String,
}

/// The notice for the change from `antes` to `ahora`, if it deserves one. `propio`: the order the person gave from the
/// menu a moment ago ([`PROPIO`]), whose own outcome is not announced (a connection or a disconnection); a server that
/// stops answering or comes back, a block that stays and a connection that fails always are. The first reading
/// (`antes` unknown) and a service that stops answering are not announced.
#[must_use]
pub fn aviso(antes: &Vpn, ahora: &Vpn, propio: Option<Orden>, t: &Textos) -> Option<Aviso> {
    let (a, b) = (Cara::de(antes), Cara::de(ahora));
    if a == b || matches!(a, Cara::SinServicio(_)) || matches!(b, Cara::SinServicio(_)) {
        return None;
    }
    let callada = |c: Cara| matches!(c, Cara::SinRespuesta | Cara::Reconectando);
    let clave = match b {
        _ if callada(b) && callada(a) => return None,
        // the chip's own explanation when it is hovered
        _ if callada(b) && ahora.estricto => "vpn_estricto_no_conecta",
        _ if callada(b) => "vpn_sin_respuesta_explica",
        Cara::Conectada if callada(a) => "aviso_vpn_vuelve",
        Cara::Conectada if propio != Some(Orden::Conectar) => "aviso_vpn_conectada",
        Cara::Bloqueada => "vpn_bloqueo_puesto_explica",
        Cara::Error => "aviso_vpn_error",
        Cara::Desconectada
            if !matches!(a, Cara::Error | Cara::NoDisponible)
                && propio != Some(Orden::Desconectar) =>
        {
            "aviso_vpn_apagada"
        }
        _ => return None,
    };
    Some(Aviso {
        titulo: estado(ahora, t),
        lugar: if b == Cara::Conectada {
            lugar(ahora, t)
        } else {
            String::new()
        },
        texto: t.t(clave),
    })
}

/// What the service answered to an order of the menu ([`ordena`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Respuesta {
    /// It took it (`{"ok": true}`): what it does comes with the next readings.
    Hecha,
    /// The VPN is already connecting, or connected to that exit (`"motivo"` says «conectando»): nothing to say, the next
    /// reading tells it.
    EnCurso,
    /// It is GUARDIANA Plus's (`402`, `crates/panel/src/plan.rs`), with the service's own line in the language asked for
    /// (empty if it gave none).
    Plus(String),
    /// It did not take it, or did not answer.
    No,
}

/// The [`Respuesta`] of the service's answer to an order (its status code and its body), or of no answer at all.
#[must_use]
pub fn respuesta_de_orden(r: Option<(u16, String)>) -> Respuesta {
    let Some((codigo, cuerpo)) = r else {
        return Respuesta::No;
    };
    let v = serde_json::from_str::<Value>(&cuerpo).unwrap_or(Value::Null);
    match codigo {
        200 if v["ok"].as_bool() == Some(true) => Respuesta::Hecha,
        200 if v["motivo"]
            .as_str()
            .is_some_and(|m| m.contains("conectando")) =>
        {
            Respuesta::EnCurso
        }
        402 => Respuesta::Plus(
            v["texto"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .filter(|c| !c.is_control())
                .take(200)
                .collect(),
        ),
        _ => Respuesta::No,
    }
}

/// The notice for an order of the menu, with what the service answered (`r`) and the VPN as it was last read; `None`
/// when there is nothing to say (the order was taken, or a «Conectar» found it already connecting: the next readings
/// tell it).
///
/// - «Conectar» that is GUARDIANA Plus's: the state, and the line the service gives (as Órbita's VPN page shows it).
/// - «Conectar» not taken with the VPN stopped and its block still in Windows: the state, and the sentence the VPN page
///   says in that state («El bloqueo sigue puesto en Windows…»), never «sin corte».
/// - «Conectar» not taken while GUARDIANA does not answer: «VPN: no se pudo conectar», and why.
/// - Any other «Conectar» not taken: «VPN: no se pudo conectar», and that the computer was left as it was.
/// - «Desconectar» as the chip says it: `vpn_sin_panel` when GUARDIANA does not answer (with the Start menu's way out),
///   `vpn_espera_conectar` while it is still connecting, else `vpn_no_desconecta`.
#[must_use]
pub fn aviso_de_orden(o: Orden, r: &Respuesta, v: &Vpn, t: &Textos) -> Option<Aviso> {
    let aviso = |titulo: String, texto: String| {
        Some(Aviso {
            titulo,
            lugar: String::new(),
            texto,
        })
    };
    match (o, r) {
        (_, Respuesta::Hecha) | (Orden::Conectar, Respuesta::EnCurso) => None,
        (Orden::Conectar, Respuesta::Plus(linea)) => aviso(
            estado(v, t),
            if linea.trim().is_empty() {
                t.t("aviso_vpn_plus")
            } else {
                linea.clone()
            },
        ),
        (Orden::Conectar, _) if v.bloqueada() => {
            aviso(estado(v, t), t.t("aviso_vpn_bloqueo_sigue"))
        }
        (Orden::Conectar, _) if v.fase == Fase::Desconocida => {
            aviso(t.t("bandeja_error"), estado(v, t))
        }
        (Orden::Conectar, _) => aviso(t.t("bandeja_error"), t.t("aviso_vpn_error")),
        (Orden::Desconectar, _) => {
            let clave = match (r, v.fase) {
                (_, Fase::Desconocida) => "vpn_sin_panel",
                (Respuesta::EnCurso, _) | (_, Fase::Conectando) => "vpn_espera_conectar",
                _ => "vpn_no_desconecta",
            };
            aviso(estado(v, t), t.t(clave))
        }
        _ => None,
    }
}

/// The notice in Windows' toast schema: a click on it, or on its button, opens Órbita on the VPN ([`ENLACE`]).
#[must_use]
pub fn notificacion(a: &Aviso, t: &Textos) -> String {
    crate::avisos::xml_con(
        ENLACE,
        &t.t("bandeja_abrir_vpn"),
        &a.titulo,
        &a.lugar,
        &a.texto,
    )
}

/// The language of the icon: the one the person chose in ZERO (its `preferencias.json` in the data folder `datos`),
/// else the system's (`sistema`, a tag like `es-CO`), as ZERO's window decides it (`Sesion::abre`).
#[must_use]
pub fn idioma(datos: &Path, sistema: &str) -> Idioma {
    let elegido = std::fs::read_to_string(datos.join("preferencias.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v["idioma"].as_str().map(str::to_owned))
        .filter(|x| !x.trim().is_empty());
    Idioma::de_etiqueta(elegido.as_deref().unwrap_or(sistema))
}

/// What the icon keeps between readings: the last one, and the order the person gave from its menu, with when.
#[derive(Debug, Clone, Default)]
pub struct Memoria {
    /// The last reading.
    pub vpn: Vpn,
    /// The last order from the menu that changes the VPN, with the moment it was given.
    propio: Option<(Orden, std::time::Instant)>,
}

impl Memoria {
    /// The person ordered `o` from the menu at `cuando`.
    pub fn ordenada(&mut self, o: Orden, cuando: std::time::Instant) {
        if matches!(o, Orden::Conectar | Orden::Desconectar) {
            self.propio = Some((o, cuando));
        }
    }

    /// The order from the menu still too recent to announce its own outcome at `ahora` ([`PROPIO`]).
    #[must_use]
    pub fn propio(&self, ahora: std::time::Instant) -> Option<Orden> {
        self.propio
            .filter(|(_, cuando)| ahora.saturating_duration_since(*cuando) < PROPIO)
            .map(|(o, _)| o)
    }

    /// A new reading at `ahora`: the notice it deserves, if any. It is kept as the last one.
    pub fn lectura(&mut self, nueva: Vpn, ahora: std::time::Instant, t: &Textos) -> Option<Aviso> {
        let a = aviso(&self.vpn, &nueva, self.propio(ahora), t);
        self.vpn = nueva;
        a
    }
}

/// The body of «Conectar» from the menu: the fastest exit the service measured.
pub const CUERPO_CONECTAR: &str = "{\"salida\":\"rapida\"}";

/// One request to GUARDIANA's service on this computer, with the panel's key, as the CLI makes it, and the language its
/// answer's words should be in (`idioma`, a code like `es` or `pt-pt`, sent as the pages send it in
/// `X-Guardiana-Lang`; empty or anything else, not sent, and the panel answers in Spanish): its status code and its
/// body, or `None` when it does not answer within `espera`. Only once [`es_el_servicio`] said the one listening is the
/// service.
#[must_use]
pub fn pide(
    metodo: &str,
    ruta: &str,
    cuerpo: &str,
    llave: &str,
    idioma: &str,
    espera: Duration,
) -> Option<(u16, String)> {
    let puerto = crate::lanzador::PUERTO_PANEL;
    let dir = SocketAddr::from(([127, 0, 0, 1], puerto));
    let mut s = TcpStream::connect_timeout(&dir, Duration::from_millis(800)).ok()?;
    s.set_read_timeout(Some(espera)).ok()?;
    s.set_write_timeout(Some(Duration::from_secs(2))).ok()?;
    let lengua = if (2..=8).contains(&idioma.len())
        && idioma.bytes().all(|b| b.is_ascii_alphabetic() || b == b'-')
    {
        format!("X-Guardiana-Lang: {idioma}\r\n")
    } else {
        String::new()
    };
    let peticion = format!(
        "{metodo} {ruta} HTTP/1.1\r\nHost: 127.0.0.1:{puerto}\r\nX-Guardiana-Token: {llave}\r\n{lengua}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{cuerpo}",
        cuerpo.len()
    );
    s.write_all(peticion.as_bytes()).ok()?;
    let mut r = Vec::new();
    s.take(512 * 1024).read_to_end(&mut r).ok()?;
    respuesta(&r)
}

/// The status code and the body of an HTTP/1.1 answer, its body whole or in chunks.
#[must_use]
pub fn respuesta(r: &[u8]) -> Option<(u16, String)> {
    let fin = r.windows(4).position(|w| w == b"\r\n\r\n")?;
    let cabeza = std::str::from_utf8(&r[..fin]).ok()?;
    let codigo = cabeza
        .lines()
        .next()?
        .split_whitespace()
        .nth(1)?
        .parse::<u16>()
        .ok()?;
    let cuerpo = &r[fin + 4..];
    let en_trozos = cabeza.lines().any(|l| {
        l.split_once(':').is_some_and(|(k, v)| {
            k.trim().eq_ignore_ascii_case("transfer-encoding")
                && v.to_ascii_lowercase().contains("chunked")
        })
    });
    let cuerpo = if en_trozos {
        trozos(cuerpo)?
    } else {
        cuerpo.to_vec()
    };
    Some((codigo, String::from_utf8_lossy(&cuerpo).into_owned()))
}

/// A body sent in chunks (`Transfer-Encoding: chunked`), put back together.
fn trozos(mut b: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let fin = b.windows(2).position(|w| w == b"\r\n")?;
        let largo = std::str::from_utf8(&b[..fin]).ok()?;
        let largo = usize::from_str_radix(largo.split(';').next()?.trim(), 16).ok()?;
        b = &b[fin + 2..];
        if largo == 0 {
            return Some(out);
        }
        out.extend_from_slice(b.get(..largo)?);
        b = b.get(largo + 2..)?;
    }
}

/// The VPN as the service says it now; [`Fase::Desconocida`] without its key or its answer.
#[must_use]
pub fn lee(llave: Option<&str>) -> Vpn {
    let Some(llave) = llave else {
        return Vpn::default();
    };
    match pide("GET", "/api/vpn", "", llave, "", ESPERA_LECTURA) {
        Some((200, cuerpo)) => serde_json::from_str::<Value>(&cuerpo)
            .map(|v| Vpn::de_json(&v))
            .unwrap_or_default(),
        _ => Vpn::default(),
    }
}

/// «Conectar» or «Desconectar» from the menu, with the icon's language (`idioma`) for the words of the answer: what
/// the service answered ([`Respuesta`]).
#[must_use]
pub fn ordena(orden: Orden, llave: &str, idioma: &str) -> Respuesta {
    let (ruta, cuerpo) = match orden {
        Orden::Conectar => ("/api/vpn/conecta", CUERPO_CONECTAR),
        Orden::Desconectar => ("/api/vpn/desconecta", ""),
        _ => return Respuesta::No,
    };
    respuesta_de_orden(pide("POST", ruta, cuerpo, llave, idioma, ESPERA_ORDEN))
}

/// Whether the one listening on the panel's port is GUARDIANA's service, as the CLI decides it before it sends the
/// panel's key (review of the 1.0.56 VPN, `crates/cli/src/vpn_cmd.rs`): the process Windows' service manager says the
/// service runs in (`servicio`) and the one Windows' table of listening sockets names (`escucha`) must both be known
/// and be the same. Otherwise any program of any account could take the port while the service is stopped and receive
/// the key every few seconds.
#[must_use]
pub fn es_el_servicio(servicio: Option<u32>, escucha: Option<u32>) -> bool {
    matches!((servicio, escucha), (Some(s), Some(e)) if s == e && s != 0)
}

/// Whether the panel's key may go to the one listening on the panel's port: only to GUARDIANA's service
/// ([`es_el_servicio`]). The automatic tests (`prueba`: `GUARDIANA_ZERO_DATOS`, zero/prueba/prueba.js) bring a panel of
/// their own to a computer without GUARDIANA, and may use it only when Windows answered that the service does not exist
/// there (`sin_servicio`): on a computer with GUARDIANA the rule always holds, whatever the environment says (review of
/// bandeja-vpn-2, finding 4).
#[must_use]
pub fn llave_al_puerto(
    prueba: bool,
    sin_servicio: bool,
    servicio: Option<u32>,
    escucha: Option<u32>,
) -> bool {
    (prueba && sin_servicio) || es_el_servicio(servicio, escucha)
}

/// The state of GUARDIANA's service in what Windows' service manager answers (`SERVICE_STATUS_PROCESS`, as
/// `QueryServiceStatusEx` writes it: nine 32-bit words, the state second): 1 stopped, 2 starting, 3 stopping, 4 running,
/// 5 continuing, 6 pausing, 7 paused.
#[must_use]
pub fn estado_del_servicio(estado: &[u8]) -> Option<u32> {
    let b = estado.get(4..8)?;
    Some(u32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
}

/// The process GUARDIANA's service runs in, from what Windows' service manager answers (the same nine words as
/// [`estado_del_servicio`], the process eighth): only while it runs.
#[must_use]
pub fn pid_del_servicio(estado: &[u8]) -> Option<u32> {
    let palabra = |i: usize| -> Option<u32> {
        let b = estado.get(i * 4..i * 4 + 4)?;
        Some(u32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
    };
    // SERVICE_RUNNING
    (palabra(1)? == 4)
        .then_some(palabra(7)?)
        .filter(|pid| *pid != 0)
}

/// The process that listens on `ip:puerto` in Windows' table of listening sockets (`MIB_TCPTABLE_OWNER_PID` as 32-bit
/// words: their count, then six per row), only when there is no doubt it is the one that answers there: a listener on
/// exactly `ip:puerto`, and none of another process on that port, on `ip` or on every address. The same reading as
/// `guardiana_apps::vigia::pid_que_escucha`, which ZERO does not link.
#[must_use]
pub fn pid_que_escucha(tabla: &[u32], ip: [u8; 4], puerto: u16) -> Option<u32> {
    let n = usize::try_from(*tabla.first()?).ok()?;
    let mut quien = None;
    let mut en_ip = false;
    for i in 0..n.min(200_000) {
        let inicio = 1 + i * 6;
        let fila = tabla.get(inicio..inicio + 6)?;
        if let [_, local, puerto_local, _, _, pid] = *fila {
            let p = puerto_local.to_ne_bytes();
            let local = local.to_ne_bytes();
            if u16::from_be_bytes([p[0], p[1]]) != puerto || (local != ip && local != [0; 4]) {
                continue;
            }
            if quien.is_some_and(|q| q != pid) {
                return None;
            }
            quien = Some(pid);
            en_ip |= local == ip;
        }
    }
    quien.filter(|_| en_ip)
}

/// The image of an icon file (`.ico`) for a side of `lado` pixels: the smallest one at least that big, else the
/// biggest; its bytes as `CreateIconFromResourceEx` takes them (PNG or DIB). `None` when the file is not an icon.
#[must_use]
pub fn imagen_ico(ico: &[u8], lado: u32) -> Option<&[u8]> {
    let n = usize::from(u16::from_le_bytes([*ico.get(4)?, *ico.get(5)?]));
    if ico.get(..4)? != [0, 0, 1, 0] || n == 0 {
        return None;
    }
    let mut mejor: Option<(u32, &[u8])> = None;
    for i in 0..n {
        let e = ico.get(6 + i * 16..6 + i * 16 + 16)?;
        let ancho = if e[0] == 0 { 256 } else { u32::from(e[0]) };
        let largo = usize::try_from(u32::from_le_bytes([e[8], e[9], e[10], e[11]])).ok()?;
        let desde = usize::try_from(u32::from_le_bytes([e[12], e[13], e[14], e[15]])).ok()?;
        let bits = ico.get(desde..desde.checked_add(largo)?)?;
        let vale = match mejor {
            None => true,
            Some((m, _)) if m >= lado => ancho >= lado && ancho < m,
            Some((m, _)) => ancho > m,
        };
        if vale {
            mejor = Some((ancho, bits));
        }
    }
    mejor.map(|(_, bits)| bits)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    fn vpn(fase: &str) -> Vpn {
        Vpn::de_json(&json!({
            "disponible": true, "fase": fase, "estricto": true,
            "desconectar": matches!(fase, "conectando" | "conectada" | "sin_respuesta" | "reconectando"),
            "salida": { "id": "hel", "pais": "FI", "ciudad": "Helsinki" }
        }))
    }

    fn sin_salida(fase: &str) -> Vpn {
        Vpn {
            pais: String::new(),
            ciudad: String::new(),
            ..vpn(fase)
        }
    }

    /// The VPN stopped by an error with its block still in Windows (the chip's «VPN parada, con su bloqueo puesto»).
    fn bloqueada() -> Vpn {
        Vpn {
            desconectar: true,
            ..vpn("error")
        }
    }

    #[test]
    fn lo_que_dice_el_servicio() {
        let v = vpn("conectada");
        assert_eq!(v.fase, Fase::Conectada);
        assert!(v.estricto && v.disponible && v.desconectar);
        assert_eq!((v.pais.as_str(), v.ciudad.as_str()), ("FI", "Helsinki"));
        assert_eq!(vpn("sin_respuesta").fase, Fase::SinRespuesta);
        assert_eq!(vpn("reconectando").fase, Fase::Reconectando);
        assert_eq!(vpn("apagada").fase, Fase::Apagada);
        assert_eq!(vpn("otra cosa").fase, Fase::Desconocida);
        assert_eq!(Vpn::de_json(&json!("no")).fase, Fase::Desconocida);
        // sin salida de la tabla (el programa de antes, o apagada), sin país
        let v = Vpn::de_json(&json!({ "fase": "conectada", "salida": null }));
        assert!(v.pais.is_empty() && v.ciudad.is_empty());
        let v = Vpn::de_json(
            &json!({ "fase": "conectada", "salida": { "pais": "fi\u{7}x", "ciudad": "A\u{7}B" } }),
        );
        assert!(v.pais.is_empty());
        assert_eq!(v.ciudad, "AB");
        // el bloqueo que queda tras un error, o con lo guardado aún encendido
        assert!(bloqueada().bloqueada());
        assert!(!vpn("error").bloqueada() && !vpn("conectada").bloqueada());
        assert!(!Vpn {
            desconectar: true,
            ..Vpn::default()
        }
        .bloqueada());
    }

    /// The icon says what the chip of ZERO's bar says, with the chip's own texts (barra.js, `VPN_FASES` and
    /// `vpn_bloqueo_puesto`), and what the chip does not show while the VPN is off.
    #[test]
    fn las_palabras_del_chip() {
        for (f, clave) in [
            ("conectando", "vpn_conectando"),
            ("conectada", "vpn_conectada"),
            ("sin_respuesta", "vpn_sin_respuesta"),
            ("reconectando", "vpn_reconectando"),
        ] {
            assert_eq!(Cara::de(&vpn(f)).clave(), clave, "{f}");
        }
        assert_eq!(Cara::de(&bloqueada()), Cara::Bloqueada);
        assert_eq!(Cara::Bloqueada.clave(), "vpn_bloqueo_puesto");
        let apagada_con_bloqueo = Vpn {
            desconectar: true,
            ..vpn("apagada")
        };
        assert_eq!(Cara::de(&apagada_con_bloqueo), Cara::Bloqueada);
        assert_eq!(Cara::de(&vpn("apagada")), Cara::Desconectada);
        assert_eq!(Cara::de(&vpn("error")), Cara::Error);
        assert_eq!(
            Cara::de(&Vpn::default()),
            Cara::SinServicio(Falta::Respuesta)
        );
        let fuera = Vpn {
            disponible: false,
            ..vpn("apagada")
        };
        assert_eq!(Cara::de(&fuera), Cara::NoDisponible);
        let es = Textos::de(Idioma::Es);
        assert_eq!(estado(&vpn("conectada"), &es), "VPN conectada");
        assert_eq!(estado(&vpn("reconectando"), &es), "VPN: no conecta");
        assert_eq!(
            estado(&bloqueada(), &es),
            "VPN parada, con su bloqueo puesto"
        );
        // every state has its words in the three languages (what they promise: los_textos_nuevos_no_prometen_de_mas)
        for idioma in [Idioma::Es, Idioma::En, Idioma::Pt] {
            let t = Textos::de(idioma);
            for v in [
                Vpn::default(),
                fuera.clone(),
                vpn("apagada"),
                vpn("error"),
                vpn("conectando"),
                vpn("conectada"),
                vpn("sin_respuesta"),
                vpn("reconectando"),
                bloqueada(),
            ] {
                let k = Cara::de(&v).clave();
                assert_ne!(t.t(k), k, "{idioma:?} {k}");
            }
        }
    }

    #[test]
    fn el_color_y_la_linea() {
        let es = Textos::de(Idioma::Es);
        assert_eq!(color(&vpn("conectada")), Color::Verde);
        assert_eq!(color(&vpn("conectando")), Color::Ambar);
        assert_eq!(color(&vpn("sin_respuesta")), Color::Rojo);
        assert_eq!(color(&vpn("reconectando")), Color::Rojo);
        assert_eq!(color(&bloqueada()), Color::Rojo);
        assert_eq!(color(&vpn("apagada")), Color::Gris);
        assert_eq!(color(&vpn("error")), Color::Gris);
        assert_eq!(color(&Vpn::default()), Color::Gris);
        assert_eq!(Color::Ambar.nombre(), "ambar");
        assert_eq!(
            linea(&vpn("conectada"), &es),
            "GUARDIANA\nVPN conectada\nFinlandia · Helsinki"
        );
        assert_eq!(
            linea(&sin_salida("apagada"), &es),
            "GUARDIANA\nVPN desconectada"
        );
        assert!(linea(&vpn("conectada"), &Textos::de(Idioma::En))
            .ends_with("VPN connected\nFinland · Helsinki"));
        assert!(linea(&vpn("conectada"), &Textos::de(Idioma::Pt)).ends_with("Finlândia · Helsinki"));
        // a country without a name in the texts: its two letters
        let v = Vpn {
            pais: "ZZ".into(),
            ..vpn("conectada")
        };
        assert_eq!(lugar(&v, &es), "ZZ · Helsinki");
        // never longer than what Windows keeps
        let v = Vpn {
            ciudad: "x".repeat(40),
            ..vpn("conectada")
        };
        for idioma in [Idioma::Es, Idioma::En, Idioma::Pt] {
            let t = Textos::de(idioma);
            for v in [v.clone(), Vpn::default(), bloqueada(), vpn("reconectando")] {
                let l = linea(&v, &t);
                assert!(l.encode_utf16().count() <= LARGO_LINEA, "{l}");
                assert!(
                    l.starts_with("GUARDIANA\n") && !l.contains("bandeja_") && !l.contains("vpn_"),
                    "{l}"
                );
            }
        }
    }

    #[test]
    fn el_menu_segun_el_estado() {
        let es = Textos::de(Idioma::Es);
        let ordenes = |v: &Vpn| {
            menu(v, false, false, &es)
                .iter()
                .map(|e| e.orden)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            ordenes(&sin_salida("apagada")),
            [
                Orden::Estado,
                Orden::AbrirVpn,
                Orden::Conectar,
                Orden::AbrirGuardiana
            ]
        );
        for f in ["conectando", "conectada", "sin_respuesta", "reconectando"] {
            assert_eq!(
                ordenes(&vpn(f)),
                [
                    Orden::Estado,
                    Orden::Lugar,
                    Orden::AbrirVpn,
                    Orden::Desconectar,
                    Orden::AbrirGuardiana
                ],
                "{f}"
            );
        }
        // stopped with its block in Windows: both ways, as the VPN page offers them
        assert_eq!(
            ordenes(&bloqueada()),
            [
                Orden::Estado,
                Orden::Lugar,
                Orden::AbrirVpn,
                Orden::Conectar,
                Orden::Desconectar,
                Orden::AbrirGuardiana
            ]
        );
        // without the service, or without the VPN on this computer, nothing to connect or disconnect
        assert_eq!(
            ordenes(&Vpn::default()),
            [Orden::Estado, Orden::AbrirVpn, Orden::AbrirGuardiana]
        );
        let v = Vpn {
            disponible: false,
            ..vpn("apagada")
        };
        assert!(!ordenes(&v).contains(&Orden::Conectar));
        let m = menu(&vpn("conectada"), false, false, &es);
        assert_eq!(m[0].texto, "VPN conectada");
        assert_eq!(m[1].texto, "Finlandia · Helsinki");
        assert!(!m[0].activa && !m[1].activa && m[2..].iter().all(|e| e.activa));
        assert!(m[2].raya && !m[3].raya && m[4].raya);
        assert_eq!(m[3].texto, "Desconectar");
        // a «Desconectar» from it still in flight: said with the chip's word, and not twice
        let d = menu(&vpn("conectada"), true, false, &es);
        assert_eq!(
            (d[3].orden, d[3].texto.as_str(), d[3].activa),
            (Orden::Desconectar, "Desconectando…", false)
        );
        for idioma in [Idioma::Es, Idioma::En, Idioma::Pt] {
            let t = Textos::de(idioma);
            for v in [bloqueada(), sin_salida("apagada"), Vpn::default()] {
                for e in menu(&v, false, false, &t) {
                    assert!(
                        !e.texto.is_empty()
                            && !e.texto.contains("bandeja_")
                            && !e.texto.contains("vpn_"),
                        "{e:?}"
                    );
                }
            }
        }
        for o in [
            Orden::Estado,
            Orden::AbrirVpn,
            Orden::Conectar,
            Orden::Desconectar,
            Orden::AbrirGuardiana,
            Orden::Lugar,
            Orden::ApagarVpn,
        ] {
            assert_ne!(o.id(), 0);
            assert_eq!(Orden::de_id(o.id()), Some(o));
        }
        assert_eq!(Orden::de_id(0), None);
    }

    #[test]
    fn un_aviso_por_cambio_y_ninguno_por_lo_que_uno_pulsa() {
        let es = Textos::de(Idioma::Es);
        let av = |a: &Vpn, b: &Vpn, propio: Option<Orden>| {
            aviso(a, b, propio, &es).map(|x| (x.titulo, x.texto))
        };
        let titulo = |a: &Vpn, b: &Vpn, propio| av(a, b, propio).map(|x| x.0);
        // the server stops answering and comes back: always said, with the chip's words and its explanation
        let (t, x) = av(&vpn("conectada"), &vpn("sin_respuesta"), None).unwrap();
        assert_eq!(t, "VPN sin respuesta");
        assert_eq!(x, es.t("vpn_estricto_no_conecta"));
        let sin_estricto = Vpn {
            estricto: false,
            ..vpn("sin_respuesta")
        };
        assert_eq!(
            av(&vpn("conectada"), &sin_estricto, None).unwrap().1,
            es.t("vpn_sin_respuesta_explica")
        );
        assert_eq!(
            titulo(
                &vpn("conectada"),
                &vpn("reconectando"),
                Some(Orden::Conectar)
            )
            .as_deref(),
            Some("VPN: no conecta")
        );
        let vuelve = aviso(&vpn("sin_respuesta"), &vpn("conectada"), None, &es).unwrap();
        assert_eq!(vuelve.titulo, "VPN conectada");
        assert_eq!(vuelve.lugar, "Finlandia · Helsinki");
        assert_eq!(vuelve.texto, es.t("aviso_vpn_vuelve"));
        assert!(aviso(
            &vpn("reconectando"),
            &vpn("conectada"),
            Some(Orden::Conectar),
            &es
        )
        .is_some());
        // connecting and disconnecting: said, unless the person just pressed that very order here
        assert_eq!(
            titulo(&vpn("conectando"), &vpn("conectada"), None).as_deref(),
            Some("VPN conectada")
        );
        assert_eq!(
            titulo(&vpn("conectando"), &vpn("conectada"), Some(Orden::Conectar)),
            None
        );
        assert!(titulo(
            &vpn("conectando"),
            &vpn("conectada"),
            Some(Orden::Desconectar)
        )
        .is_some());
        assert_eq!(
            titulo(&vpn("conectada"), &sin_salida("apagada"), None).as_deref(),
            Some("VPN desconectada")
        );
        assert_eq!(
            titulo(
                &vpn("conectada"),
                &sin_salida("apagada"),
                Some(Orden::Desconectar)
            ),
            None
        );
        // the block taken away from elsewhere: the connection is the normal one again
        assert_eq!(
            av(&bloqueada(), &sin_salida("apagada"), None).map(|x| x.1),
            Some(es.t("aviso_vpn_apagada"))
        );
        // it could not connect, or it stopped with its block in Windows: always said
        assert_eq!(
            titulo(&vpn("conectando"), &vpn("error"), Some(Orden::Conectar)).as_deref(),
            Some("VPN: no se pudo conectar")
        );
        let (t, x) = av(&vpn("reconectando"), &bloqueada(), None).unwrap();
        assert_eq!(t, "VPN parada, con su bloqueo puesto");
        assert_eq!(x, es.t("vpn_bloqueo_puesto_explica"));
        // nothing new, the first reading, a service that stops answering, and an error that ends in nothing: nothing
        assert_eq!(av(&vpn("conectada"), &vpn("conectada"), None), None);
        assert_eq!(av(&vpn("sin_respuesta"), &vpn("reconectando"), None), None);
        assert_eq!(av(&Vpn::default(), &vpn("conectada"), None), None);
        assert_eq!(av(&vpn("conectada"), &Vpn::default(), None), None);
        assert_eq!(av(&sin_salida("apagada"), &vpn("conectando"), None), None);
        assert_eq!(av(&vpn("error"), &sin_salida("apagada"), None), None);
        // the notice opens the VPN
        let a = aviso(&vpn("conectando"), &vpn("conectada"), None, &es).unwrap();
        let x = notificacion(&a, &es);
        assert!(x.starts_with("<toast launch=\"guardiana://vpn\" activationType=\"protocol\""));
        assert!(x.contains("arguments=\"guardiana://vpn\"") && x.contains("Abrir la VPN"));
        assert!(x.contains("<text>Finlandia · Helsinki</text>"));
        for idioma in [Idioma::Es, Idioma::En, Idioma::Pt] {
            let t = Textos::de(idioma);
            for (a, b) in [
                (vpn("conectada"), vpn("sin_respuesta")),
                (vpn("conectada"), sin_estricto.clone()),
                (vpn("sin_respuesta"), vpn("conectada")),
                (vpn("conectando"), vpn("conectada")),
                (vpn("conectada"), sin_salida("apagada")),
                (vpn("conectando"), vpn("error")),
                (vpn("reconectando"), bloqueada()),
            ] {
                let x = aviso(&a, &b, None, &t).unwrap();
                for parte in [&x.titulo, &x.lugar, &x.texto] {
                    assert!(
                        !parte.contains("aviso_")
                            && !parte.contains("vpn_")
                            && !parte.contains("bandeja_")
                            && !parte.contains('{'),
                        "{x:?}"
                    );
                }
            }
        }
    }

    /// The owner's words for every text the icon brings (crates/zero/i18n): «encriptado», never «cifrado»; nothing that
    /// promises what no one can; the strict mode covers «tus programas de Windows», never the whole computer.
    #[test]
    fn los_textos_nuevos_no_prometen_de_mas() {
        let claves = [
            "bandeja_sin_servicio",
            "bandeja_no_disponible",
            "bandeja_desconectada",
            "bandeja_error",
            "bandeja_abrir_vpn",
            "bandeja_conectar",
            "bandeja_abrir_guardiana",
            "aviso_vpn_conectada",
            "aviso_vpn_vuelve",
            "aviso_vpn_apagada",
            "aviso_vpn_error",
            "aviso_vpn_bloqueo_sigue",
            "aviso_vpn_plus",
        ];
        for idioma in [Idioma::Es, Idioma::En, Idioma::Pt] {
            let t = Textos::de(idioma);
            for k in claves {
                let x = t.t(k);
                assert_ne!(x, k, "{idioma:?}: falta {k}");
                let bajo = x.to_lowercase();
                for prohibida in [
                    "cifra",
                    "100 %",
                    "100%",
                    "imposible",
                    "impossible",
                    "nadie puede",
                    "no one can",
                    "ninguém pode",
                    "anónim",
                    "anonym",
                    "anônim",
                    "auditad",
                    "audited",
                    "certificad",
                    "certified",
                ] {
                    assert!(!bajo.contains(prohibida), "{idioma:?} {k}: {x}");
                }
            }
        }
        // the computer's programs, never more: «tus programas de Windows», and «encriptado» (never «cifrado»)
        for (idioma, programas, encriptado) in [
            (Idioma::Es, "tus programas de Windows", "encriptado"),
            (Idioma::En, "your Windows programs", "encrypted"),
            (Idioma::Pt, "seus programas do Windows", "criptografado"),
        ] {
            let t = Textos::de(idioma);
            for k in ["aviso_vpn_conectada", "aviso_vpn_vuelve"] {
                assert!(
                    t.t(k).contains(programas) && t.t(k).contains(encriptado),
                    "{idioma:?} {k}"
                );
            }
            for k in ["aviso_vpn_apagada", "aviso_vpn_bloqueo_sigue"] {
                assert!(t.t(k).contains(programas), "{idioma:?} {k}");
            }
        }
    }

    /// Why GUARDIANA does not answer, told apart without the panel's key, each with its own words; no icon without the
    /// service; and, while it does not answer, the Start menu's way out in the menu, with the Start menu's own words.
    #[test]
    fn por_que_no_contesta_y_su_salida() {
        assert_eq!(
            falta_del_puerto(true, None, None, Some(4040)),
            Falta::SinGuardiana
        );
        assert_eq!(
            falta_del_puerto(false, Some(1), None, Some(812)),
            Falta::Parada
        );
        assert_eq!(
            falta_del_puerto(false, Some(4), Some(812), None),
            Falta::Arrancando
        );
        assert_eq!(
            falta_del_puerto(false, Some(4), Some(812), Some(4040)),
            Falta::OtroEnPuerto
        );
        // while Windows starts or stops it, «not answering yet», never «stopped»; when it cannot be told, the general
        // words (second review of bandeja-vpn-2, point 5)
        for pendiente in [2, 3, 5, 6] {
            assert_eq!(
                falta_del_puerto(false, Some(pendiente), None, None),
                Falta::Arrancando,
                "{pendiente}"
            );
        }
        assert_eq!(falta_del_puerto(false, None, None, None), Falta::Respuesta);
        assert_eq!(
            falta_del_puerto(false, Some(7), None, None),
            Falta::Respuesta
        );
        // the way out runs GUARDIANA's own guardiana.exe, as the service manager names it, and nothing else
        let instalado = r"C:\Program Files\Guardiana\guardiana.exe";
        for imagen in [
            r#""C:\Program Files\Guardiana\guardiana.exe" service run"#,
            r"C:\Program Files\Guardiana\guardiana.exe service run",
            r#"  "C:\Program Files\Guardiana\guardiana.exe"  "#,
        ] {
            assert_eq!(
                guardiana_del_servicio(imagen).as_deref(),
                Some(instalado),
                "{imagen}"
            );
        }
        assert_eq!(
            guardiana_del_servicio(r#""D:\G\GUARDIANA.EXE" service run"#).as_deref(),
            Some(r"D:\G\GUARDIANA.EXE")
        );
        assert_eq!(
            guardiana_del_servicio(r#""\\servidor\c$\Guardiana\guardiana.exe" service run"#)
                .as_deref(),
            Some(r"\\servidor\c$\Guardiana\guardiana.exe")
        );
        for imagen in [
            "",
            r#""C:\Program Files\Guardiana\guardiana.exe service run"#,
            r#""C:\Program Files\Guardiana\otro.exe" service run"#,
            r#""guardiana.exe" service run"#,
            r"Guardiana\guardiana.exe service run",
            r#""C:\Program Files\Guardiana\..\Temp\guardiana.exe" service run"#,
            // without quotes, «.exe» must end the path
            r"C:\G\guardiana.exe.bak service run",
            r"C:\G\guardiana.exe2\guardiana.exe service run",
            r"C:\G\guardiana.exeX",
        ] {
            assert_eq!(guardiana_del_servicio(imagen), None, "{imagen}");
        }
        let causas = [
            Falta::Respuesta,
            Falta::SinGuardiana,
            Falta::Parada,
            Falta::Arrancando,
            Falta::OtroEnPuerto,
            Falta::SinLlave,
        ];
        for idioma in [Idioma::Es, Idioma::En, Idioma::Pt] {
            let t = Textos::de(idioma);
            let mut dichas = Vec::new();
            for f in causas {
                let v = Vpn::sin(f);
                assert_eq!(color(&v), Color::Gris);
                let k = Cara::de(&v).clave();
                assert_ne!(t.t(k), k, "{idioma:?} {k}");
                let l = linea(&v, &t);
                assert!(l.encode_utf16().count() <= LARGO_LINEA, "{l}");
                // causes that are not the same are not said with the same words
                assert!(!dichas.contains(&t.t(k)), "{idioma:?} {k}");
                dichas.push(t.t(k));
            }
        }
        // ZERO alone: no VPN, no icon; any other cause keeps it, grey
        assert!(!se_ve(&Vpn::sin(Falta::SinGuardiana)));
        assert!(se_ve(&Vpn::sin(Falta::Parada)) && se_ve(&vpn("conectada")));
        let es = Textos::de(Idioma::Es);
        assert_eq!(
            estado(&Vpn::sin(Falta::Parada), &es),
            "El servicio de GUARDIANA está parado en este equipo"
        );
        // the way out while GUARDIANA does not answer: only with guardiana.exe there, never without the service
        let ordenes = |v: &Vpn, apagar: bool| {
            menu(v, false, apagar, &es)
                .iter()
                .map(|e| e.orden)
                .collect::<Vec<_>>()
        };
        for f in [
            Falta::Respuesta,
            Falta::Parada,
            Falta::Arrancando,
            Falta::OtroEnPuerto,
            Falta::SinLlave,
        ] {
            assert_eq!(
                ordenes(&Vpn::sin(f), true),
                [
                    Orden::Estado,
                    Orden::AbrirVpn,
                    Orden::ApagarVpn,
                    Orden::AbrirGuardiana
                ],
                "{f:?}"
            );
            assert!(!ordenes(&Vpn::sin(f), false).contains(&Orden::ApagarVpn));
        }
        assert!(!ordenes(&Vpn::sin(Falta::SinGuardiana), true).contains(&Orden::ApagarVpn));
        // with the service answering, «Desconectar» is the way, not this
        assert!(!ordenes(&bloqueada(), true).contains(&Orden::ApagarVpn));
        // the same words as the Start menu's entry (build/wix/loc-*.wxl, `ApagarVpn`)
        for (idioma, wxl) in [
            (Idioma::Es, include_str!("../../../build/wix/loc-es-ES.wxl")),
            (Idioma::En, include_str!("../../../build/wix/loc-en-US.wxl")),
            (Idioma::Pt, include_str!("../../../build/wix/loc-pt-BR.wxl")),
        ] {
            let palabras = Textos::de(idioma)
                .t("bandeja_apagar_vpn")
                .replace('\'', "&apos;");
            assert!(
                wxl.contains(&format!("<String Id=\"ApagarVpn\" Value=\"{palabras}\" />")),
                "{idioma:?}: {palabras}"
            );
        }
    }

    /// An order of the menu the service did not take is said, as the chip and the VPN page say it; one it took, or a
    /// «Conectar» that found it already connecting, is not.
    #[test]
    fn una_orden_que_no_se_pudo_hacer() {
        let es = Textos::de(Idioma::Es);
        let no = Respuesta::No;
        // taken: nothing to say, the next readings tell it
        for o in [Orden::Conectar, Orden::Desconectar] {
            assert_eq!(
                aviso_de_orden(o, &Respuesta::Hecha, &vpn("conectada"), &es),
                None
            );
        }
        let a = aviso_de_orden(Orden::Desconectar, &no, &vpn("conectada"), &es).unwrap();
        assert_eq!(
            (a.titulo.as_str(), a.texto),
            ("VPN conectada", es.t("vpn_no_desconecta"))
        );
        let a = aviso_de_orden(Orden::Desconectar, &no, &vpn("conectando"), &es).unwrap();
        assert_eq!(a.texto, es.t("vpn_espera_conectar"));
        // the service says it is still connecting (the chip's own reading of its «motivo»)
        let a = aviso_de_orden(
            Orden::Desconectar,
            &Respuesta::EnCurso,
            &vpn("conectada"),
            &es,
        )
        .unwrap();
        assert_eq!(a.texto, es.t("vpn_espera_conectar"));
        // GUARDIANA does not answer: the Start menu's way out
        let a = aviso_de_orden(Orden::Desconectar, &no, &Vpn::default(), &es).unwrap();
        assert_eq!(a.texto, es.t("vpn_sin_panel"));
        // «Conectar» not taken, the computer as it was: no tunnel and no cut-off
        let a = aviso_de_orden(Orden::Conectar, &no, &sin_salida("apagada"), &es).unwrap();
        assert_eq!(
            (a.titulo.as_str(), a.texto),
            ("VPN: no se pudo conectar", es.t("aviso_vpn_error"))
        );
        // ... but with its block still in Windows, never «sin corte»: what the VPN page says in that state
        let a = aviso_de_orden(Orden::Conectar, &no, &bloqueada(), &es).unwrap();
        assert_eq!(a.titulo, "VPN parada, con su bloqueo puesto");
        assert_eq!(
            a.texto,
            "El bloqueo sigue puesto en Windows: no sale nada de tus programas de Windows fuera del túnel. «Desconectar» lo quita y vuelve a tu conexión normal."
        );
        assert!(!a.texto.contains("sin corte"));
        // ... and with GUARDIANA not answering, why, without guessing about the block
        let a = aviso_de_orden(Orden::Conectar, &no, &Vpn::default(), &es).unwrap();
        assert_eq!(a.titulo, "VPN: no se pudo conectar");
        assert_eq!(a.texto, estado(&Vpn::default(), &es));
        // GUARDIANA Plus's: the service's own line, or ours when it gave none
        let plus = Respuesta::Plus("Esto forma parte de Plus.".into());
        let a = aviso_de_orden(Orden::Conectar, &plus, &sin_salida("apagada"), &es).unwrap();
        assert_eq!(
            (a.titulo.as_str(), a.texto.as_str()),
            ("VPN desconectada", "Esto forma parte de Plus.")
        );
        let a = aviso_de_orden(
            Orden::Conectar,
            &Respuesta::Plus(String::new()),
            &sin_salida("apagada"),
            &es,
        )
        .unwrap();
        assert_eq!(a.texto, es.t("aviso_vpn_plus"));
        // already connecting (from Órbita or the bar a moment before): nothing, the next reading tells it
        assert_eq!(
            aviso_de_orden(
                Orden::Conectar,
                &Respuesta::EnCurso,
                &sin_salida("apagada"),
                &es
            ),
            None
        );
        assert_eq!(
            aviso_de_orden(Orden::AbrirVpn, &no, &Vpn::default(), &es),
            None
        );
        for idioma in [Idioma::Es, Idioma::En, Idioma::Pt] {
            let t = Textos::de(idioma);
            for (o, v) in [
                (Orden::Conectar, bloqueada()),
                (Orden::Conectar, sin_salida("apagada")),
                (Orden::Conectar, Vpn::default()),
                (Orden::Desconectar, vpn("conectada")),
                (Orden::Desconectar, Vpn::default()),
            ] {
                let a = aviso_de_orden(o, &no, &v, &t).unwrap();
                assert!(
                    !a.titulo.contains("aviso_")
                        && !a.texto.contains("aviso_")
                        && !a.texto.contains("vpn_")
                        && !a.texto.contains("bandeja_"),
                    "{a:?}"
                );
            }
        }
    }

    /// What the service answers to an order, read as the VPN page and the chip read it.
    #[test]
    fn lo_que_contesta_el_servicio_a_una_orden() {
        let r = |codigo: u16, cuerpo: &str| respuesta_de_orden(Some((codigo, cuerpo.to_owned())));
        assert_eq!(r(200, r#"{"ok":true}"#), Respuesta::Hecha);
        assert_eq!(
            r(
                200,
                r#"{"ok":false,"motivo":"ya está conectada, o conectando"}"#
            ),
            Respuesta::EnCurso
        );
        assert_eq!(
            r(
                200,
                r#"{"ok":false,"motivo":"no se sabe la dirección de esa salida"}"#
            ),
            Respuesta::No
        );
        // the panel's own 402 (crates/panel/src/plan.rs, `respuesta_plus`)
        assert_eq!(
            r(
                402,
                r#"{"plan":"plus","texto":"Isto faz parte do Plus.","boton":"Passar para o Plus","enlace":"https://x"}"#
            ),
            Respuesta::Plus("Isto faz parte do Plus.".into())
        );
        assert_eq!(r(402, "no es json"), Respuesta::Plus(String::new()));
        assert_eq!(
            r(
                402,
                &format!(r#"{{"texto":"a\u0007b{}"}}"#, "x".repeat(400))
            ),
            Respuesta::Plus(format!("ab{}", "x".repeat(198)))
        );
        assert_eq!(r(401, "session token required"), Respuesta::No);
        assert_eq!(r(200, "no es json"), Respuesta::No);
        assert_eq!(respuesta_de_orden(None), Respuesta::No);
    }

    #[test]
    fn la_memoria_no_anuncia_lo_que_uno_acaba_de_pulsar() {
        let es = Textos::de(Idioma::Es);
        let t0 = std::time::Instant::now();
        let mut m = Memoria::default();
        // the first reading: nothing
        assert_eq!(m.lectura(sin_salida("apagada"), t0, &es), None);
        m.ordenada(Orden::Conectar, t0);
        m.ordenada(Orden::AbrirVpn, t0);
        assert_eq!(m.propio(t0), Some(Orden::Conectar));
        assert_eq!(m.lectura(vpn("conectando"), t0, &es), None);
        assert_eq!(m.lectura(vpn("conectada"), t0 + CADA, &es), None);
        // a minute later the same change is news again
        assert_eq!(m.propio(t0 + PROPIO), None);
        assert!(m
            .lectura(vpn("sin_respuesta"), t0 + CADA * 2, &es)
            .is_some());
        assert_eq!(m.vpn.fase, Fase::SinRespuesta);
    }

    #[test]
    fn el_idioma_que_eligio_la_persona_en_zero() {
        let d = std::env::temp_dir().join(format!("guardiana-zero-bandeja-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&d);
        let _ = std::fs::remove_file(d.join("preferencias.json"));
        assert_eq!(idioma(&d, "pt-BR"), Idioma::Pt);
        let _ = std::fs::write(d.join("preferencias.json"), r#"{"idioma":"en"}"#);
        assert_eq!(idioma(&d, "pt-BR"), Idioma::En);
        let _ = std::fs::write(d.join("preferencias.json"), r#"{"idioma":""}"#);
        assert_eq!(idioma(&d, "es-CO"), Idioma::Es);
        let _ = std::fs::write(d.join("preferencias.json"), "no es json");
        assert_eq!(idioma(&d, "es-CO"), Idioma::Es);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn las_respuestas_del_servicio() {
        let r = b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 11\r\n\r\n{\"ok\":true}";
        assert_eq!(respuesta(r), Some((200, "{\"ok\":true}".to_owned())));
        let r = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\n{\"ok\r\n7\r\n\":true}\r\n0\r\n\r\n";
        assert_eq!(respuesta(r), Some((200, "{\"ok\":true}".to_owned())));
        assert_eq!(
            respuesta(b"HTTP/1.1 401 Unauthorized\r\n\r\nsession token required"),
            Some((401, "session token required".to_owned()))
        );
        assert_eq!(respuesta(b"no es http"), None);
        assert_eq!(
            respuesta(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nzz\r\n"),
            None
        );
        // without a key, nothing is asked
        assert_eq!(lee(None), Vpn::default());
    }

    /// The key goes only to the service's own process (the CLI's rule): both known and the same.
    #[test]
    fn la_llave_solo_va_al_servicio() {
        // what Windows' service manager answers: nine words, the state (4, running) second and the process eighth
        let estado = |st: u32, pid: u32| {
            let mut b = Vec::new();
            for w in [16, st, 1, 0, 0, 0, 0, pid, 0] {
                b.extend_from_slice(&u32::to_ne_bytes(w));
            }
            b
        };
        assert_eq!(pid_del_servicio(&estado(4, 812)), Some(812));
        assert_eq!(pid_del_servicio(&estado(1, 812)), None);
        assert_eq!(pid_del_servicio(&estado(4, 0)), None);
        assert_eq!(pid_del_servicio(&estado(4, 812)[..20]), None);
        assert_eq!(estado_del_servicio(&estado(2, 0)), Some(2));
        assert_eq!(estado_del_servicio(&estado(1, 0)), Some(1));
        assert_eq!(estado_del_servicio(&estado(4, 812)[..6]), None);
        assert!(es_el_servicio(Some(812), Some(812)));
        assert!(!es_el_servicio(Some(812), Some(4040)));
        // the tests' own panel only where GUARDIANA's service does not exist; with it, the rule holds for them too
        assert!(llave_al_puerto(true, true, None, Some(4040)));
        assert!(!llave_al_puerto(true, false, None, Some(4040)));
        assert!(!llave_al_puerto(true, false, Some(812), Some(4040)));
        assert!(llave_al_puerto(true, false, Some(812), Some(812)));
        assert!(!llave_al_puerto(false, true, None, Some(4040)));
        assert!(llave_al_puerto(false, false, Some(812), Some(812)));
        assert!(!es_el_servicio(None, Some(812)));
        assert!(!es_el_servicio(Some(812), None));
        assert!(!es_el_servicio(Some(0), Some(0)));
        // Windows' table: count, then (state, local address, local port, remote address, remote port, pid) per row,
        // the addresses and ports as the bytes of the network say them
        let ip = |a: [u8; 4]| u32::from_ne_bytes(a);
        let puerto = |p: u16| {
            let b = p.to_be_bytes();
            u32::from_ne_bytes([b[0], b[1], 0, 0])
        };
        let fila = |a: [u8; 4], p: u16, pid: u32| [2, ip(a), puerto(p), 0, 0, pid];
        let tabla = |filas: &[[u32; 6]]| {
            let mut t = vec![u32::try_from(filas.len()).unwrap()];
            for f in filas {
                t.extend_from_slice(f);
            }
            t
        };
        let lo = [127, 0, 0, 1];
        let t = tabla(&[fila([0; 4], 80, 4), fila(lo, 7443, 812)]);
        assert_eq!(pid_que_escucha(&t, lo, 7443), Some(812));
        // another process on every address of the same port: no certainty
        let t = tabla(&[fila(lo, 7443, 812), fila([0; 4], 7443, 999)]);
        assert_eq!(pid_que_escucha(&t, lo, 7443), None);
        // only on every address: not on exactly 127.0.0.1
        let t = tabla(&[fila([0; 4], 7443, 812)]);
        assert_eq!(pid_que_escucha(&t, lo, 7443), None);
        assert_eq!(pid_que_escucha(&[], lo, 7443), None);
        assert_eq!(pid_que_escucha(&[3, 1, 2], lo, 7443), None);
    }

    /// The four icons are GUARDIANA's own (build/wix/iconos_vpn.py), with an image for every size the notification
    /// area asks for at 100 to 200 %.
    #[test]
    fn los_iconos_tienen_cada_tamano() {
        for ico in [
            &include_bytes!("../../../build/wix/vpn-verde.ico")[..],
            &include_bytes!("../../../build/wix/vpn-ambar.ico")[..],
            &include_bytes!("../../../build/wix/vpn-rojo.ico")[..],
            &include_bytes!("../../../build/wix/vpn-gris.ico")[..],
        ] {
            for lado in [16, 20, 24, 32, 40] {
                let png = imagen_ico(ico, lado).unwrap();
                assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
                // the PNG's own width, bytes 16 to 19 of its header
                let ancho = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
                assert_eq!(ancho, lado);
            }
            // bigger than any: the biggest
            let png = imagen_ico(ico, 300).unwrap();
            assert_eq!(u32::from_be_bytes([png[16], png[17], png[18], png[19]]), 64);
        }
        assert_eq!(imagen_ico(b"no es un icono", 16), None);
        assert_eq!(imagen_ico(&[0, 0, 1, 0, 1, 0], 16), None);
    }

    /// The menu's orders reach a service on this computer as the CLI sends them: the method, the address, the key and
    /// the body.
    #[test]
    fn las_ordenes_llegan_con_la_llave() {
        use std::net::TcpListener;
        // only when the panel's port is free here (the service of a developer's computer may be using it)
        let Ok(escucha) = TcpListener::bind(("127.0.0.1", crate::lanzador::PUERTO_PANEL)) else {
            return;
        };
        let hilo = std::thread::spawn(move || {
            let mut vistas = Vec::new();
            for _ in 0..2 {
                let (mut s, _) = escucha.accept().unwrap();
                s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
                // the whole request: its head, then the body its Content-Length says
                let mut b = Vec::new();
                let mut trozo = [0u8; 1024];
                loop {
                    let n = s.read(&mut trozo).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    b.extend_from_slice(&trozo[..n]);
                    let t = String::from_utf8_lossy(&b).into_owned();
                    if let Some((cabeza, cuerpo)) = t.split_once("\r\n\r\n") {
                        let largo = cabeza
                            .lines()
                            .find_map(|l| l.strip_prefix("Content-Length: "))
                            .and_then(|x| x.trim().parse::<usize>().ok())
                            .unwrap_or(0);
                        if cuerpo.len() >= largo {
                            break;
                        }
                    }
                }
                vistas.push(String::from_utf8_lossy(&b).into_owned());
                let cuerpo = "{\"ok\":true}";
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n{cuerpo}",
                    cuerpo.len()
                );
            }
            vistas
        });
        assert_eq!(ordena(Orden::Conectar, "k1", "pt"), Respuesta::Hecha);
        assert_eq!(ordena(Orden::Desconectar, "k1", "x\r\ny"), Respuesta::Hecha);
        assert_eq!(ordena(Orden::AbrirVpn, "k1", "es"), Respuesta::No);
        let vistas = hilo.join().unwrap();
        assert!(vistas[0].starts_with("POST /api/vpn/conecta HTTP/1.1\r\n"));
        assert!(vistas[0].contains("X-Guardiana-Token: k1\r\n"));
        // the words of the answer in the icon's language, as the pages ask for them
        assert!(vistas[0].contains("X-Guardiana-Lang: pt\r\n"));
        assert!(vistas[0].ends_with(CUERPO_CONECTAR));
        assert!(vistas[1].starts_with("POST /api/vpn/desconecta HTTP/1.1\r\n"));
        assert!(vistas[1].contains("Content-Length: 0\r\n"));
        // a language that is not a code is not sent at all
        assert!(!vistas[1].contains("X-Guardiana-Lang"));
    }
}
