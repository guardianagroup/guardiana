//! The VPN next to Windows' clock (5 Nov 2026; the owner, 3 Nov 2026, on the icon and the notices of the VPN: «Hazlo
//! claro»): GUARDIANA's icon in the notification area with the VPN's colour, the line under the pointer, its menu, and
//! Windows' notices when the VPN connects, stops answering, comes back or goes off.
//!
//! Decided here, and tested on every system; `GUARDIANA ZERO.exe --avisos` (zero/navegador/src/avisos.rs, the process
//! without a window that already shows the escudo's notices) draws it. It asks GUARDIANA's service on this computer as
//! the CLI does (`crates/cli/src/vpn_cmd.rs`): `GET /api/vpn` on 127.0.0.1:7443 with the panel's key in
//! `X-Guardiana-Token`, every [`CADA`]; and «Conectar» and «Desconectar» are the panel's own orders.
//!
//! - The colour ([`color`]): green connected, grey off, amber connecting, red when its server does not answer (then
//!   the tunnel's cut-off keeps everything in: [`Fase::Callada`]).
//! - The line under the pointer ([`linea`]): «GUARDIANA · VPN conectada: Finlandia · Helsinki».
//! - The menu ([`menu`]): the state (not clickable), «Abrir la VPN», «Conectar (la más rápida)» or «Desconectar», and
//!   «Abrir GUARDIANA».
//! - The notices ([`aviso`]): one per change of state, so the same one never comes twice in a row, and none for what
//!   the person has just ordered from this menu ([`PROPIO`]).

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use serde_json::Value;

use crate::textos::Textos;

/// What a click on the icon or on a notice opens: Órbita on the VPN.
pub const ENLACE: &str = "guardiana://vpn";
/// How often the icon asks the service.
pub const CADA: Duration = Duration::from_secs(3);
/// After an order from this menu, how long its own outcome is not announced: the person has just pressed it.
pub const PROPIO: Duration = Duration::from_secs(60);
/// The longest line under the pointer Windows keeps (`NOTIFYICONDATAW.szTip`: 128 UTF-16 units with its end).
pub const LARGO_LINEA: usize = 127;

/// In what the VPN is, as the icon tells it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Fase {
    /// The service did not answer (it is starting, stopped, or the key could not be read).
    #[default]
    Desconocida,
    /// Off.
    Apagada,
    /// Connecting.
    Conectando,
    /// Connected and its server answering.
    Conectada,
    /// Its server does not answer (`sin_respuesta`, or `reconectando` in strict mode): nothing leaves the computer.
    Callada,
    /// It could not connect.
    Error,
}

/// The VPN as the service says it (`GET /api/vpn`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Vpn {
    /// In what it is.
    pub fase: Fase,
    /// Strict mode is on.
    pub estricto: bool,
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
            "sin_respuesta" | "reconectando" => Fase::Callada,
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
            estricto: v["estricto"].as_bool() == Some(true),
            pais: if pais.len() == 2 && pais.bytes().all(|b| b.is_ascii_uppercase()) {
                pais
            } else {
                String::new()
            },
            ciudad: texto(&v["salida"]["ciudad"], 40),
            disponible: v["disponible"].as_bool() == Some(true),
        }
    }

    /// The VPN is on: connecting, connected or without its server.
    #[must_use]
    pub fn encendida(&self) -> bool {
        matches!(
            self.fase,
            Fase::Conectando | Fase::Conectada | Fase::Callada
        )
    }
}

/// The icon's colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    /// Connected.
    Verde,
    /// Off, or the service does not answer.
    Gris,
    /// Connecting.
    Ambar,
    /// Its server does not answer.
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

/// The icon's colour for `v`.
#[must_use]
pub fn color(v: &Vpn) -> Color {
    match v.fase {
        Fase::Conectada => Color::Verde,
        Fase::Conectando => Color::Ambar,
        Fase::Callada => Color::Rojo,
        Fase::Desconocida | Fase::Apagada | Fase::Error => Color::Gris,
    }
}

/// Where the VPN goes out, in words: «Finlandia · Helsinki»; empty when it is not one of GUARDIANA's exits.
#[must_use]
pub fn lugar(v: &Vpn, t: &Textos) -> String {
    if v.pais.is_empty() {
        return v.ciudad.clone();
    }
    let clave = format!("pais_{}", v.pais.to_ascii_lowercase());
    let pais = t.t(&clave);
    let pais = if pais == clave { v.pais.clone() } else { pais };
    if v.ciudad.is_empty() {
        pais
    } else {
        format!("{pais} · {}", v.ciudad)
    }
}

/// What the VPN is doing, in words, for the menu's first line and the line under the pointer.
#[must_use]
pub fn estado(v: &Vpn, t: &Textos) -> String {
    let donde = lugar(v, t);
    match v.fase {
        Fase::Desconocida => t.t("bandeja_sin_servicio"),
        Fase::Apagada if !v.disponible => t.t("bandeja_no_disponible"),
        Fase::Apagada => t.t("bandeja_apagada"),
        Fase::Conectando => t.t("bandeja_conectando"),
        Fase::Conectada if donde.is_empty() => t.t("bandeja_conectada_sin_lugar"),
        Fase::Conectada => t.tf("bandeja_conectada", &[("lugar", &donde)]),
        Fase::Callada => t.t("bandeja_callada"),
        Fase::Error => t.t("bandeja_error"),
    }
}

/// The line under the pointer: «GUARDIANA · <state>», cut to what Windows keeps.
#[must_use]
pub fn linea(v: &Vpn, t: &Textos) -> String {
    let s = format!("GUARDIANA · {}", estado(v, t));
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
    /// Disconnect, in any phase.
    Desconectar,
    /// GUARDIANA's window.
    AbrirGuardiana,
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
        ]
        .into_iter()
        .find(|o| o.id() == id)
    }
}

/// One line of the menu: the order, its words, and whether it can be chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entrada {
    /// What it does.
    pub orden: Orden,
    /// Its words.
    pub texto: String,
    /// It can be chosen.
    pub activa: bool,
}

/// The menu for `v`: the state; «Abrir la VPN»; «Desconectar» while it is on, or «Conectar (la más rápida)» when it can
/// connect; «Abrir GUARDIANA».
#[must_use]
pub fn menu(v: &Vpn, t: &Textos) -> Vec<Entrada> {
    let entrada = |orden, clave: &str, activa| Entrada {
        orden,
        texto: t.t(clave),
        activa,
    };
    let mut m = vec![
        Entrada {
            orden: Orden::Estado,
            texto: estado(v, t),
            activa: false,
        },
        entrada(Orden::AbrirVpn, "bandeja_abrir_vpn", true),
    ];
    if v.encendida() {
        m.push(entrada(Orden::Desconectar, "bandeja_desconectar", true));
    } else if v.disponible && v.fase != Fase::Desconocida {
        m.push(entrada(Orden::Conectar, "bandeja_conectar", true));
    }
    m.push(entrada(
        Orden::AbrirGuardiana,
        "bandeja_abrir_guardiana",
        true,
    ));
    m
}

/// A Windows notice of the VPN.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aviso {
    /// Its title.
    pub titulo: String,
    /// Its text.
    pub texto: String,
}

/// The notice for the change from `antes` to `ahora`, if it deserves one. `propio`: the person ordered this change from
/// the menu a moment ago ([`PROPIO`]), so connecting and disconnecting are not announced; a server that stops answering
/// or comes back always is. The first reading (`antes` unknown) and a service that stops answering are not announced.
#[must_use]
pub fn aviso(antes: &Vpn, ahora: &Vpn, propio: bool, t: &Textos) -> Option<Aviso> {
    if antes.fase == ahora.fase || antes.fase == Fase::Desconocida {
        return None;
    }
    let donde = lugar(ahora, t);
    let con = |titulo: &str, texto: &str, huecos: &[(&str, &str)]| {
        Some(Aviso {
            titulo: t.t(titulo),
            texto: t.tf(texto, huecos),
        })
    };
    match (antes.fase, ahora.fase) {
        (Fase::Callada, Fase::Conectada) => con(
            "aviso_vpn_vuelve_t",
            "aviso_vpn_vuelve",
            &[("lugar", &donde)],
        ),
        (_, Fase::Callada) => con("aviso_vpn_callada_t", "aviso_vpn_callada", &[]),
        (_, Fase::Conectada) if !propio => con(
            "aviso_vpn_conectada_t",
            "aviso_vpn_conectada",
            &[("lugar", &donde)],
        ),
        (Fase::Conectada | Fase::Callada | Fase::Conectando, Fase::Apagada) if !propio => {
            con("aviso_vpn_apagada_t", "aviso_vpn_apagada", &[])
        }
        (Fase::Conectando | Fase::Callada, Fase::Error) => {
            con("aviso_vpn_error_t", "aviso_vpn_error", &[])
        }
        _ => None,
    }
}

/// The notice in Windows' toast schema: a click on it, or on its button, opens Órbita on the VPN ([`ENLACE`]).
#[must_use]
pub fn notificacion(a: &Aviso, t: &Textos) -> String {
    crate::avisos::xml_con(ENLACE, &t.t("bandeja_abrir_vpn"), &a.titulo, "", &a.texto)
}

/// The body of «Conectar» from the menu: the fastest exit the service measured.
pub const CUERPO_CONECTAR: &str = "{\"salida\":\"rapida\"}";

/// One request to GUARDIANA's service on this computer, with the panel's key, as the CLI makes it: its status code and
/// its body, or `None` when it does not answer in time.
#[must_use]
pub fn pide(metodo: &str, ruta: &str, cuerpo: &str, llave: &str) -> Option<(u16, String)> {
    let puerto = crate::lanzador::PUERTO_PANEL;
    let dir = SocketAddr::from(([127, 0, 0, 1], puerto));
    let mut s = TcpStream::connect_timeout(&dir, Duration::from_millis(800)).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(4))).ok()?;
    s.set_write_timeout(Some(Duration::from_secs(2))).ok()?;
    let peticion = format!(
        "{metodo} {ruta} HTTP/1.1\r\nHost: 127.0.0.1:{puerto}\r\nX-Guardiana-Token: {llave}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{cuerpo}",
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
    match pide("GET", "/api/vpn", "", llave) {
        Some((200, cuerpo)) => serde_json::from_str::<Value>(&cuerpo)
            .map(|v| Vpn::de_json(&v))
            .unwrap_or_default(),
        _ => Vpn::default(),
    }
}

/// «Conectar» or «Desconectar» from the menu: whether the service took the order (`{"ok": true}`).
#[must_use]
pub fn ordena(orden: Orden, llave: &str) -> bool {
    let (ruta, cuerpo) = match orden {
        Orden::Conectar => ("/api/vpn/conecta", CUERPO_CONECTAR),
        Orden::Desconectar => ("/api/vpn/desconecta", ""),
        _ => return false,
    };
    pide("POST", ruta, cuerpo, llave).is_some_and(|(codigo, cuerpo)| {
        codigo == 200
            && serde_json::from_str::<Value>(&cuerpo).is_ok_and(|v| v["ok"].as_bool() == Some(true))
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::textos::Idioma;
    use serde_json::json;

    fn vpn(fase: &str) -> Vpn {
        Vpn::de_json(&json!({
            "disponible": true, "fase": fase, "estricto": true,
            "salida": { "id": "hel", "pais": "FI", "ciudad": "Helsinki" }
        }))
    }

    #[test]
    fn lo_que_dice_el_servicio() {
        let v = vpn("conectada");
        assert_eq!(v.fase, Fase::Conectada);
        assert!(v.estricto && v.disponible);
        assert_eq!((v.pais.as_str(), v.ciudad.as_str()), ("FI", "Helsinki"));
        assert_eq!(vpn("sin_respuesta").fase, Fase::Callada);
        assert_eq!(vpn("reconectando").fase, Fase::Callada);
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
    }

    #[test]
    fn el_color_y_la_linea() {
        let es = Textos::de(Idioma::Es);
        assert_eq!(color(&vpn("conectada")), Color::Verde);
        assert_eq!(color(&vpn("conectando")), Color::Ambar);
        assert_eq!(color(&vpn("sin_respuesta")), Color::Rojo);
        assert_eq!(color(&vpn("apagada")), Color::Gris);
        assert_eq!(color(&Vpn::default()), Color::Gris);
        assert_eq!(Color::Ambar.nombre(), "ambar");
        assert_eq!(
            linea(&vpn("conectada"), &es),
            "GUARDIANA · VPN conectada: Finlandia · Helsinki"
        );
        assert!(linea(&vpn("conectada"), &Textos::de(Idioma::En)).contains("Finland · Helsinki"));
        assert!(linea(&vpn("conectada"), &Textos::de(Idioma::Pt)).contains("Finlândia · Helsinki"));
        // a country without a name in the texts: its two letters
        let v = Vpn {
            pais: "ZZ".into(),
            ..vpn("conectada")
        };
        assert!(lugar(&v, &es).starts_with("ZZ"));
        // never longer than what Windows keeps
        let v = Vpn {
            ciudad: "x".repeat(40),
            ..vpn("conectada")
        };
        for idioma in [Idioma::Es, Idioma::En, Idioma::Pt] {
            let l = linea(&v, &Textos::de(idioma));
            assert!(l.encode_utf16().count() <= LARGO_LINEA, "{l}");
        }
        for f in ["apagada", "conectando", "sin_respuesta", "error"] {
            let l = linea(&vpn(f), &es);
            assert!(
                l.starts_with("GUARDIANA · ") && !l.contains("bandeja_"),
                "{l}"
            );
        }
        assert!(!linea(&Vpn::default(), &es).contains("bandeja_"));
    }

    #[test]
    fn el_menu_segun_el_estado() {
        let es = Textos::de(Idioma::Es);
        let ordenes = |v: &Vpn| menu(v, &es).iter().map(|e| e.orden).collect::<Vec<_>>();
        assert_eq!(
            ordenes(&vpn("apagada")),
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
                    Orden::AbrirVpn,
                    Orden::Desconectar,
                    Orden::AbrirGuardiana
                ],
                "{f}"
            );
        }
        // without the service, or without the VPN on this computer, nothing to connect
        assert!(!ordenes(&Vpn::default()).contains(&Orden::Conectar));
        let v = Vpn {
            disponible: false,
            ..vpn("apagada")
        };
        assert!(!ordenes(&v).contains(&Orden::Conectar));
        let m = menu(&vpn("conectada"), &es);
        assert!(!m[0].activa && m[1..].iter().all(|e| e.activa));
        assert!(m
            .iter()
            .all(|e| !e.texto.is_empty() && !e.texto.contains("bandeja_")));
        for o in [
            Orden::Estado,
            Orden::AbrirVpn,
            Orden::Conectar,
            Orden::Desconectar,
            Orden::AbrirGuardiana,
        ] {
            assert_ne!(o.id(), 0);
            assert_eq!(Orden::de_id(o.id()), Some(o));
        }
        assert_eq!(Orden::de_id(0), None);
    }

    #[test]
    fn un_aviso_por_cambio_y_ninguno_por_lo_que_uno_pulsa() {
        let es = Textos::de(Idioma::Es);
        let av =
            |a: &str, b: &str, propio: bool| aviso(&vpn(a), &vpn(b), propio, &es).map(|x| x.titulo);
        // the server stops answering and comes back: always said
        assert_eq!(
            av("conectada", "sin_respuesta", false).as_deref(),
            Some("La VPN no responde")
        );
        assert_eq!(
            av("conectada", "reconectando", true).as_deref(),
            Some("La VPN no responde")
        );
        assert_eq!(
            av("sin_respuesta", "conectada", false).as_deref(),
            Some("La VPN ha vuelto")
        );
        // connecting and disconnecting: said, unless the person just pressed it here
        assert_eq!(
            av("conectando", "conectada", false).as_deref(),
            Some("VPN conectada")
        );
        assert_eq!(av("conectando", "conectada", true), None);
        assert_eq!(
            av("conectada", "apagada", false).as_deref(),
            Some("VPN desconectada")
        );
        assert_eq!(av("conectada", "apagada", true), None);
        assert_eq!(
            av("conectando", "error", false).as_deref(),
            Some("La VPN no pudo conectar")
        );
        // nothing new, the first reading and a service that stops answering: nothing
        assert_eq!(av("conectada", "conectada", false), None);
        assert_eq!(av("sin_respuesta", "reconectando", false), None);
        assert_eq!(aviso(&Vpn::default(), &vpn("conectada"), false, &es), None);
        assert_eq!(aviso(&vpn("conectada"), &Vpn::default(), false, &es), None);
        assert_eq!(av("apagada", "conectando", false), None);
        // the text says where, and the notice opens the VPN
        let a = aviso(&vpn("conectando"), &vpn("conectada"), false, &es).unwrap();
        assert!(a.texto.contains("Finlandia · Helsinki"), "{}", a.texto);
        let x = notificacion(&a, &es);
        assert!(x.starts_with("<toast launch=\"guardiana://vpn\" activationType=\"protocol\""));
        assert!(x.contains("arguments=\"guardiana://vpn\"") && x.contains("Abrir la VPN"));
        for idioma in [Idioma::Es, Idioma::En, Idioma::Pt] {
            let t = Textos::de(idioma);
            for (a, b) in [
                ("conectada", "sin_respuesta"),
                ("sin_respuesta", "conectada"),
                ("conectando", "conectada"),
                ("conectada", "apagada"),
                ("conectando", "error"),
            ] {
                let x = aviso(&vpn(a), &vpn(b), false, &t).unwrap();
                assert!(
                    !x.titulo.contains("aviso_")
                        && !x.texto.contains("aviso_")
                        && !x.texto.contains('{'),
                    "{x:?}"
                );
            }
        }
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
        assert!(ordena(Orden::Conectar, "k1"));
        assert!(ordena(Orden::Desconectar, "k1"));
        assert!(!ordena(Orden::AbrirVpn, "k1"));
        let vistas = hilo.join().unwrap();
        assert!(vistas[0].starts_with("POST /api/vpn/conecta HTTP/1.1\r\n"));
        assert!(vistas[0].contains("X-Guardiana-Token: k1\r\n"));
        assert!(vistas[0].ends_with(CUERPO_CONECTAR));
        assert!(vistas[1].starts_with("POST /api/vpn/desconecta HTTP/1.1\r\n"));
        assert!(vistas[1].contains("Content-Length: 0\r\n"));
    }
}
