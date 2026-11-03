//! Strict mode («modo estricto»; ronda 91, the owner's top priority: «que la página web no lo pueda detectar… No se
//! puede caer entre medias… Ahí tenemos que ser impecables»; docs/VPN.md, «Modo estricto»): what the person wants of
//! the VPN, kept on disk so that it outlives GUARDIANA's service; the fixed keys of what strict mode leaves in Windows;
//! and what the service does with both when it starts. Pure: the file is read and written with the standard library,
//! and nothing here calls Windows (`guardiana_apps::corte_vpn` does, with these keys).
//!
//! - [`Deseo`]: strict mode (on unless the person turns it off), whether the VPN is meant to be on, its server, and the
//!   server's addresses: with the block on no name resolves outside the tunnel, so a new sign-up goes to the address
//!   known. Kept in [`ARCHIVO`], under the data folder's `sistema`, which only the system and the administrators can
//!   write (`guardiana_core::paths::harden_data_dir`).
//! - [`al_arrancar`]: at the service's start, a block nobody wants any more is removed; a VPN meant to be on in strict
//!   mode is brought back, with the block held meanwhile.
//! - [`claves`]: the provider, the sublayer and the filters strict mode adds to Windows, by fixed keys, so the service,
//!   `guardiana vpn apaga` and the uninstaller always find and remove them, whichever version put them there.

use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::corte;

/// Where the wish lives, inside GUARDIANA's data folder.
pub const ARCHIVO: &str = "sistema/vpn.json";

/// GUARDIANA's Windows service (`guardiana_service::daemon::SERVICE_NAME`; the panel's tests check they agree). Strict
/// mode's provider names it: at its own start, Windows' filtering service loads a provider's persistent filters only
/// while that service exists and starts with the computer, so an uninstall that could not remove them leaves nothing in
/// force after the next restart.
pub const SERVICIO: &str = "guardiana";

/// The fixed keys of strict mode's objects in Windows (docs/VPN.md, «Modo estricto»). Never change them: a newer
/// version must find and remove what an older one put.
pub mod claves {
    /// The provider, «GUARDIANA — VPN, modo estricto» (`36694406-4925-4a21-9e16-0031b0e144b5`).
    pub const PROVEEDOR: u128 = 0x3669_4406_4925_4a21_9e16_0031_b0e1_44b5;
    /// The sublayer, with the highest weight (`50573eae-5843-4fc2-aefc-12aeda4a7908`).
    pub const SUBCAPA: u128 = 0x5057_3eae_5843_4fc2_aefc_12ae_da4a_7908;
    /// The persistent filters: this key plus the filter's place in [`crate::corte::Estricto::persistentes`]
    /// (`329dccb1-2e23-4244-bf84-61d4a4b7xxxx`).
    pub const PERSISTENTES: u128 = 0x329d_ccb1_2e23_4244_bf84_61d4_a4b7_0000;
    /// The boot-time filters: this key plus the filter's place in [`crate::corte::Estricto::arranque`]
    /// (`53fb6f77-c067-462b-9aca-6a6337d8xxxx`).
    pub const ARRANQUE: u128 = 0x53fb_6f77_c067_462b_9aca_6a63_37d8_0000;
    /// How many places each set has. Removing tries every one of them, so a set that grew or shrank between versions
    /// is still removed whole.
    pub const MAXIMO: u16 = 64;
}

/// What the person wants of the VPN, as kept on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deseo {
    /// Strict mode: on unless the person turned it off.
    #[serde(default = "si")]
    pub estricto: bool,
    /// The VPN is meant to be on: from «Conectar» until «Desconectar», `guardiana vpn apaga` or the uninstall. Kept
    /// only in strict mode: without it, the VPN ends with the service, as always.
    #[serde(default)]
    pub encendida: bool,
    /// The server asked for, a name of guardianagroup.com.
    #[serde(default)]
    pub servidor: String,
    /// The server's HTTPS address, where a new sign-up goes.
    #[serde(default)]
    pub alta: Option<SocketAddr>,
    /// The name of the endpoint the last sign-up gave...
    #[serde(default)]
    pub punto: String,
    /// ...and its address, where the encrypted packets go.
    #[serde(default)]
    pub extremo: Option<SocketAddr>,
}

fn si() -> bool {
    true
}

impl Default for Deseo {
    /// Strict mode on, the VPN off.
    fn default() -> Self {
        Self {
            estricto: true,
            encendida: false,
            servidor: String::new(),
            alta: None,
            punto: String::new(),
            extremo: None,
        }
    }
}

impl Deseo {
    /// The wish in the file's text. What cannot be read is the VPN off (strict mode on); so is a VPN «on» with no server
    /// of GUARDIANA's or no address to reach it, which would be a block with no way back to the server.
    #[must_use]
    pub fn de_texto(texto: &str) -> Self {
        let d: Self = serde_json::from_str(texto).unwrap_or_default();
        if d.encendida && (!crate::alta::es_servidor(&d.servidor) || d.alta.is_none()) {
            d.apagada()
        } else {
            d
        }
    }

    /// The text kept in the file.
    #[must_use]
    pub fn texto(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Whether strict mode's block is wanted in Windows: strict mode on and the VPN meant to be on.
    #[must_use]
    pub fn quiere_bloqueo(&self) -> bool {
        self.estricto && self.encendida
    }

    /// The VPN off: its server and the server's addresses forgotten, strict mode as it was.
    #[must_use]
    pub fn apagada(self) -> Self {
        Self {
            estricto: self.estricto,
            ..Self::default()
        }
    }

    /// The VPN on with `servidor`, its HTTPS at `alta`. What was known of its endpoint stays when it is the same server.
    #[must_use]
    pub fn encendida_con(self, servidor: &str, alta: SocketAddr) -> Self {
        let mismo = self.servidor == servidor;
        Self {
            estricto: self.estricto,
            encendida: true,
            servidor: servidor.to_owned(),
            alta: Some(alta),
            punto: if mismo { self.punto } else { String::new() },
            extremo: if mismo { self.extremo } else { None },
        }
    }

    /// The endpoint a sign-up gave: its name and its address.
    #[must_use]
    pub fn con_extremo(self, punto: &str, extremo: SocketAddr) -> Self {
        Self {
            punto: punto.to_owned(),
            extremo: Some(extremo),
            ..self
        }
    }

    /// Where the block lets the service reach its server.
    #[must_use]
    pub fn servidor_del_bloqueo(&self) -> corte::Servidor {
        corte::Servidor {
            extremo: self.extremo,
            alta: self.alta,
        }
    }

    /// The address of `nombre` at `puerto` without asking any resolver: the HTTPS address's, when it is the server
    /// asked for (GUARDIANA's servers answer the sign-up and the tunnel on one address), or the endpoint's the last
    /// sign-up gave. `None` when neither is known.
    #[must_use]
    pub fn sin_resolver(&self, nombre: &str, puerto: u16) -> Option<SocketAddr> {
        if !nombre.is_empty() && nombre == self.servidor {
            return self.alta.map(|a| SocketAddr::new(a.ip(), puerto));
        }
        if !nombre.is_empty() && nombre == self.punto {
            return self.extremo.map(|e| SocketAddr::new(e.ip(), puerto));
        }
        None
    }
}

/// The wish kept in the data folder `datos`; the VPN off (strict mode on) when there is none, or it cannot be read.
#[must_use]
pub fn lee_de(datos: &Path) -> Deseo {
    std::fs::read_to_string(datos.join(ARCHIVO))
        .map_or_else(|_| Deseo::default(), |t| Deseo::de_texto(&t))
}

/// Keeps `deseo` in the data folder `datos`, whole or not at all: written aside, then put in its place.
///
/// # Errors
/// What the file system said.
pub fn guarda_en(datos: &Path, deseo: &Deseo) -> std::io::Result<()> {
    let ruta = datos.join(ARCHIVO);
    if let Some(carpeta) = ruta.parent() {
        std::fs::create_dir_all(carpeta)?;
    }
    let aparte = ruta.with_extension("json.tmp");
    std::fs::write(&aparte, deseo.texto())?;
    std::fs::rename(&aparte, &ruta)
}

/// Removes the wish from the data folder `datos` (the uninstall): no file is the VPN off.
///
/// # Errors
/// What the file system said, other than the file not being there.
pub fn olvida_en(datos: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(datos.join(ARCHIVO)) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        r => r,
    }
}

/// What the service does with strict mode when it starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlArrancar {
    /// Nothing: no block is wanted, and none is left in Windows.
    Nada,
    /// A block nobody wants any more (strict mode off, or the VPN off) is left in Windows: it is removed, and the VPN
    /// is off.
    Quita,
    /// The VPN is meant to be on in strict mode: the block goes in again (with the service's program file as it is now,
    /// after an update too) and the tunnel is brought back; nothing leaves outside it meanwhile.
    Vuelve,
}

/// What the service does at its start with the wish `deseo`, when Windows has (`quedan`) or has not any of strict
/// mode's objects. When Windows cannot be asked, the caller passes `true`: removing what is not there costs nothing.
#[must_use]
pub fn al_arrancar(deseo: &Deseo, quedan: bool) -> AlArrancar {
    if deseo.quiere_bloqueo() {
        AlArrancar::Vuelve
    } else if quedan {
        AlArrancar::Quita
    } else {
        AlArrancar::Nada
    }
}

/// How long strict mode waits before trying again, after `fallos` attempts in a row did not connect: a few seconds at
/// first, then [`crate::conexion::REINTENTO`]. The block holds all along.
#[must_use]
pub fn espera_tras(fallos: u32) -> Duration {
    match fallos {
        0 | 1 => Duration::from_secs(5),
        2 | 3 => Duration::from_secs(15),
        _ => crate::conexion::REINTENTO,
    }
}

/// From when the server's silence is counted (milliseconds since 1970) at a look at `ahora_ms`, the previous look
/// having been at `antes_ms` and the looks coming every `cada_ms`: a gap much longer than that is the computer waking
/// up from sleep, and the server gets its whole margin again from now. Otherwise, `desde_ms` as it was.
#[must_use]
pub fn desde_tras_vistazo(desde_ms: u64, antes_ms: u64, ahora_ms: u64, cada_ms: u64) -> u64 {
    if ahora_ms.saturating_sub(antes_ms) > cada_ms.saturating_mul(3) {
        desde_ms.max(ahora_ms)
    } else {
        desde_ms
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn encendida() -> Deseo {
        Deseo::default()
            .encendida_con(
                "vpn.guardianagroup.com",
                "62.238.17.121:443".parse().unwrap(),
            )
            .con_extremo(
                "vpn.guardianagroup.com",
                "62.238.17.121:51820".parse().unwrap(),
            )
    }

    #[test]
    fn por_defecto_el_modo_estricto_y_la_vpn_apagada() {
        let d = Deseo::default();
        assert!(d.estricto && !d.encendida && !d.quiere_bloqueo());
        // lo que no se puede leer, igual: nunca un bloqueo sin camino al servidor
        for texto in ["", "{", "[]", "{\"estricto\": 3}"] {
            assert_eq!(Deseo::de_texto(texto), Deseo::default(), "{texto}");
        }
        // un archivo de antes, sin el campo: el modo estricto, encendido
        assert!(Deseo::de_texto("{}").estricto);
        assert!(!Deseo::de_texto("{\"estricto\": false}").estricto);
    }

    #[test]
    fn se_guarda_y_se_lee_igual() {
        let d = encendida();
        assert_eq!(Deseo::de_texto(&d.texto()), d);
        assert!(d.quiere_bloqueo());
        assert_eq!(
            d.servidor_del_bloqueo(),
            corte::Servidor {
                extremo: Some("62.238.17.121:51820".parse().unwrap()),
                alta: Some("62.238.17.121:443".parse().unwrap()),
            }
        );
        // sin modo estricto, la VPN no pide bloqueo
        let normal = Deseo {
            estricto: false,
            ..d.clone()
        };
        assert!(!normal.quiere_bloqueo());
        // apagada: el servidor y sus direcciones, olvidados; el modo estricto, como estaba
        let apagada = d.apagada();
        assert_eq!(apagada, Deseo::default());
        assert!(!normal.apagada().estricto);
    }

    /// A VPN «on» that could never reach its server again would leave the computer blocked for good: read as off.
    #[test]
    fn una_vpn_encendida_sin_camino_al_servidor_se_lee_apagada() {
        let mut d = encendida();
        d.alta = None;
        assert!(!Deseo::de_texto(&d.texto()).encendida);
        let mut d = encendida();
        d.servidor = "vpn.example.com".into();
        assert!(!Deseo::de_texto(&d.texto()).encendida);
        assert!(Deseo::de_texto(&encendida().texto()).encendida);
    }

    #[test]
    fn otro_servidor_olvida_el_extremo_del_anterior() {
        let d = encendida();
        let mismo = d.clone().encendida_con(
            "vpn.guardianagroup.com",
            "62.238.17.121:443".parse().unwrap(),
        );
        assert_eq!(mismo.extremo, d.extremo);
        let otro = d.encendida_con(
            "vpn-mia.guardianagroup.com",
            "192.0.2.9:443".parse().unwrap(),
        );
        assert_eq!(otro.servidor, "vpn-mia.guardianagroup.com");
        assert_eq!((otro.punto.as_str(), otro.extremo), ("", None));
        assert!(otro.estricto && otro.encendida);
    }

    /// With the block on no name resolves outside the tunnel: the server's addresses come from what is known.
    #[test]
    fn las_direcciones_sin_resolver() {
        let d = encendida();
        assert_eq!(
            d.sin_resolver("vpn.guardianagroup.com", 51820),
            Some("62.238.17.121:51820".parse().unwrap())
        );
        let mut otra = d.clone();
        otra.punto = "motor-hel.guardianagroup.com".into();
        otra.extremo = Some("192.0.2.7:51820".parse().unwrap());
        assert_eq!(
            otra.sin_resolver("motor-hel.guardianagroup.com", 51821),
            Some("192.0.2.7:51821".parse().unwrap())
        );
        assert_eq!(d.sin_resolver("otro.guardianagroup.com", 51820), None);
        assert_eq!(Deseo::default().sin_resolver("", 51820), None);
    }

    /// The service's start: a block nobody wants is removed (strict mode off, or the VPN off), whatever is left in
    /// Windows; a VPN meant to be on in strict mode comes back with its block.
    #[test]
    fn al_arrancar_quita_lo_que_nadie_quiere() {
        let on = encendida();
        assert_eq!(al_arrancar(&on, true), AlArrancar::Vuelve);
        assert_eq!(al_arrancar(&on, false), AlArrancar::Vuelve);
        let sin_estricto = Deseo {
            estricto: false,
            ..on.clone()
        };
        assert_eq!(al_arrancar(&sin_estricto, true), AlArrancar::Quita);
        assert_eq!(al_arrancar(&sin_estricto, false), AlArrancar::Nada);
        let apagada = on.apagada();
        assert_eq!(al_arrancar(&apagada, true), AlArrancar::Quita);
        assert_eq!(al_arrancar(&apagada, false), AlArrancar::Nada);
        // sin archivo
        assert_eq!(al_arrancar(&Deseo::default(), true), AlArrancar::Quita);
    }

    #[test]
    fn el_archivo_se_escribe_entero_y_se_olvida() {
        let dir = std::env::temp_dir().join(format!(
            "guardiana-estricto-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(lee_de(&dir), Deseo::default());
        guarda_en(&dir, &encendida()).unwrap();
        assert_eq!(lee_de(&dir), encendida());
        assert!(!dir.join("sistema/vpn.json.tmp").exists());
        guarda_en(&dir, &encendida().apagada()).unwrap();
        assert!(!lee_de(&dir).encendida);
        olvida_en(&dir).unwrap();
        olvida_en(&dir).unwrap();
        assert!(!dir.join(ARCHIVO).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Every set strict mode puts fits the keys it has in Windows, with the server known or not, over IPv4 or IPv6; and
    /// the four keys never overlap.
    #[test]
    fn cada_conjunto_cabe_en_sus_claves() {
        for servidor in [
            corte::Servidor::default(),
            encendida().servidor_del_bloqueo(),
            corte::Servidor {
                extremo: Some("[2001:db8::7]:51820".parse().unwrap()),
                alta: Some("[2001:db8::7]:443".parse().unwrap()),
            },
        ] {
            let e = corte::estricto(servidor);
            assert!(e.persistentes.len() < usize::from(claves::MAXIMO));
            assert!(e.arranque.len() < usize::from(claves::MAXIMO));
            assert!(!e.arranque.is_empty());
        }
        let bases = [
            claves::PROVEEDOR,
            claves::SUBCAPA,
            claves::PERSISTENTES,
            claves::ARRANQUE,
        ];
        for (i, a) in bases.iter().enumerate() {
            for b in bases.iter().skip(i + 1) {
                assert!(a.abs_diff(*b) > u128::from(claves::MAXIMO));
            }
        }
        assert_eq!(claves::PERSISTENTES & 0xffff, 0);
        assert_eq!(claves::ARRANQUE & 0xffff, 0);
    }

    #[test]
    fn se_vuelve_a_intentar_cada_vez_mas_despacio() {
        assert_eq!(espera_tras(1), Duration::from_secs(5));
        assert_eq!(espera_tras(3), Duration::from_secs(15));
        assert_eq!(espera_tras(9), crate::conexion::REINTENTO);
        assert!(espera_tras(0) <= espera_tras(2) && espera_tras(2) <= espera_tras(4));
    }

    /// After a sleep, the clock jumped: the server gets its margin again from the wake-up.
    #[test]
    fn tras_el_reposo_el_servidor_tiene_su_margen_otra_vez() {
        let cada = 10_000;
        // vistazos seguidos: nada cambia
        assert_eq!(desde_tras_vistazo(1_000, 50_000, 60_000, cada), 1_000);
        assert_eq!(desde_tras_vistazo(1_000, 50_000, 80_000, cada), 1_000);
        // un hueco de una hora: el equipo durmió
        assert_eq!(
            desde_tras_vistazo(1_000, 50_000, 3_650_000, cada),
            3_650_000
        );
    }
}
