//! Strict mode («modo estricto»; ronda 91, the owner's top priority: «que la página web no lo pueda detectar… No se
//! puede caer entre medias… Ahí tenemos que ser impecables»; docs/VPN.md, «Modo estricto»): what the person wants of
//! the VPN, kept on disk so that it outlives GUARDIANA's service; the fixed keys of what strict mode leaves in Windows;
//! and what the service does with both when it starts. Pure: the file is read and written with the standard library,
//! and nothing here calls Windows (`guardiana_apps::corte_vpn` does, with these keys).
//!
//! - [`Deseo`]: strict mode (on unless the person turns it off), whether the VPN is meant to be on, its server, and the
//!   server's addresses: with the block on no name resolves outside the tunnel, so a new sign-up goes to the address
//!   known. While the VPN is on, the last address known of each of GUARDIANA's servers too (review of the 1.0.56 VPN):
//!   changing exit with the tunnel down, or going back to the one of before, needs one. Kept in [`ARCHIVO`], under the
//!   data folder's `sistema`, which only the system and the administrators can write
//!   (`guardiana_core::paths::harden_data_dir`).
//! - [`al_arrancar`]: at the service's start, a block nobody wants any more is removed; a VPN meant to be on in strict
//!   mode is brought back, with the block held meanwhile.
//! - [`claves`]: the provider, the sublayer and the filters strict mode adds to Windows, by fixed keys, so the service,
//!   `guardiana vpn apaga` and the uninstaller always find and remove them, whichever version put them there.

use std::collections::BTreeMap;
use std::net::{IpAddr, SocketAddr};
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
    /// The last address known of each of GUARDIANA's servers while the VPN is on, by name (review of the 1.0.56 VPN):
    /// the servers it went to, and what was measured of the exits with the VPN off. With the block on no name resolves
    /// outside the tunnel, so changing exit, or going back to the one of before, goes to the address known here. Only
    /// names of guardianagroup.com, at most [`CONOCIDAS_MAXIMO`], and forgotten with the rest when the VPN is turned
    /// off.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub conocidas: BTreeMap<String, IpAddr>,
}

/// How many servers' addresses [`Deseo::conocidas`] keeps.
pub const CONOCIDAS_MAXIMO: usize = 16;

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
            conocidas: BTreeMap::new(),
        }
    }
}

impl Deseo {
    /// The wish in the file's text. What cannot be read is the VPN off (strict mode on); so is a VPN «on» with no server
    /// of GUARDIANA's or no address to reach it, which would be a block with no way back to the server. Of the addresses
    /// known, only those of GUARDIANA's servers.
    #[must_use]
    pub fn de_texto(texto: &str) -> Self {
        let mut d: Self = serde_json::from_str(texto).unwrap_or_default();
        d.conocidas
            .retain(|nombre, _| crate::alta::es_servidor(nombre));
        while d.conocidas.len() > CONOCIDAS_MAXIMO {
            d.conocidas.pop_last();
        }
        if d.encendida && (!crate::alta::es_servidor(&d.servidor) || d.alta.is_none()) {
            d.apagada()
        } else {
            d
        }
    }

    /// `nombre` is at `ip`, kept among the addresses known when it is one of GUARDIANA's servers and there is room (the
    /// newest address of a name known wins).
    fn conoce(&mut self, nombre: &str, ip: IpAddr) {
        let nombre = nombre.to_ascii_lowercase();
        if !crate::alta::es_servidor(&nombre)
            || (self.conocidas.len() >= CONOCIDAS_MAXIMO && !self.conocidas.contains_key(&nombre))
        {
            return;
        }
        self.conocidas.insert(nombre, ip);
    }

    /// With `direcciones` among the addresses known: each a server's name and its address (what was measured of the
    /// exits with the VPN off). Names not of GUARDIANA's are left out.
    #[must_use]
    pub fn con_conocidas<'a>(
        mut self,
        direcciones: impl IntoIterator<Item = (&'a str, IpAddr)>,
    ) -> Self {
        for (nombre, ip) in direcciones {
            self.conoce(nombre, ip);
        }
        self
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

    /// The VPN on with `servidor`, its HTTPS at `alta`. What was known of its endpoint stays when it is the same server;
    /// with another, the addresses of the one of before stay among the addresses known (until the 1.0.56 review they
    /// were forgotten, and going back to it with the block on and the tunnel down had no address to go to).
    #[must_use]
    pub fn encendida_con(self, servidor: &str, alta: SocketAddr) -> Self {
        let Self {
            estricto,
            servidor: antes,
            alta: alta_antes,
            punto,
            extremo,
            conocidas,
            ..
        } = self;
        let mismo = antes == servidor;
        let mut d = Self {
            estricto,
            encendida: true,
            servidor: servidor.to_owned(),
            alta: Some(alta),
            punto: if mismo { punto.clone() } else { String::new() },
            extremo: if mismo { extremo } else { None },
            conocidas,
        };
        if let Some(a) = alta_antes {
            d.conoce(&antes, a.ip());
        }
        if let Some(e) = extremo {
            d.conoce(&punto, e.ip());
        }
        d.conoce(servidor, alta.ip());
        d
    }

    /// The endpoint a sign-up gave: its name and its address (known from now on too).
    #[must_use]
    pub fn con_extremo(mut self, punto: &str, extremo: SocketAddr) -> Self {
        self.conoce(punto, extremo.ip());
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
    /// asked for (GUARDIANA's servers answer the sign-up and the tunnel on one address), the endpoint's the last
    /// sign-up gave, or the last one known of that name. `None` when none is known.
    #[must_use]
    pub fn sin_resolver(&self, nombre: &str, puerto: u16) -> Option<SocketAddr> {
        if nombre.is_empty() {
            return None;
        }
        let del_servidor = (nombre == self.servidor).then_some(self.alta).flatten();
        let del_punto = (nombre == self.punto).then_some(self.extremo).flatten();
        del_servidor
            .or(del_punto)
            .map(|a| a.ip())
            .or_else(|| self.conocidas.get(&nombre.to_ascii_lowercase()).copied())
            .map(|ip| SocketAddr::new(ip, puerto))
    }
}

/// The wish kept in the data folder `datos`; the VPN off (strict mode on) when there is none, or it cannot be read.
#[must_use]
pub fn lee_de(datos: &Path) -> Deseo {
    std::fs::read_to_string(datos.join(ARCHIVO))
        .map_or_else(|_| Deseo::default(), |t| Deseo::de_texto(&t))
}

/// The wish kept in the data folder `datos`, telling a file that cannot be read right now apart: no file is the VPN off
/// (strict mode on), as for [`lee_de`]; a file there that cannot be read is an error, so that a passing failure (another
/// program holding it a moment) is not taken for the VPN turned off from outside the service.
///
/// # Errors
/// What the file system said, other than the file not being there.
pub fn lee_si_se_puede(datos: &Path) -> std::io::Result<Deseo> {
    match std::fs::read_to_string(datos.join(ARCHIVO)) {
        Ok(t) => Ok(Deseo::de_texto(&t)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Deseo::default()),
        Err(e) => Err(e),
    }
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

    /// Review of the 1.0.56 VPN: changing exit no longer forgets where the one of before is. With the block on and the
    /// tunnel down no name resolves, and going back to it needs its address.
    #[test]
    fn otro_servidor_no_olvida_la_direccion_del_anterior() {
        let otro = encendida().encendida_con(
            "vpn-mia.guardianagroup.com",
            "192.0.2.9:443".parse().unwrap(),
        );
        assert_eq!(
            otro.sin_resolver("vpn.guardianagroup.com", 443),
            Some("62.238.17.121:443".parse().unwrap())
        );
        assert_eq!(
            otro.sin_resolver("VPN-MIA.guardianagroup.com", 51820),
            Some("192.0.2.9:51820".parse().unwrap())
        );
        // and back to Helsinki: its address is known, and Miami's stays
        let vuelta = otro.encendida_con(
            "vpn.guardianagroup.com",
            "62.238.17.121:443".parse().unwrap(),
        );
        assert_eq!(
            vuelta.sin_resolver("vpn-mia.guardianagroup.com", 443),
            Some("192.0.2.9:443".parse().unwrap())
        );
        // the endpoint a sign-up gave is known too
        let con = vuelta.con_extremo(
            "motor-hel.guardianagroup.com",
            "192.0.2.7:51820".parse().unwrap(),
        );
        assert_eq!(
            con.conocidas.get("motor-hel.guardianagroup.com"),
            Some(&"192.0.2.7".parse().unwrap())
        );
        // saved and read alike; turned off, nothing of it stays
        assert_eq!(Deseo::de_texto(&con.texto()), con);
        assert!(con.apagada().conocidas.is_empty());
    }

    /// What was measured of the exits joins the addresses known, only for names of GUARDIANA's, and never more than the
    /// maximum, also when the file says otherwise.
    #[test]
    fn las_direcciones_conocidas_solo_de_guardiana_y_con_tope() {
        let ip: IpAddr = "192.0.2.10".parse().unwrap();
        let d = encendida().con_conocidas([
            ("vpn-nbg.guardianagroup.com", ip),
            ("vpn.example.com", ip),
            ("", ip),
        ]);
        assert_eq!(
            d.sin_resolver("vpn-nbg.guardianagroup.com", 443),
            Some(SocketAddr::new(ip, 443))
        );
        assert_eq!(d.sin_resolver("vpn.example.com", 443), None);
        let muchos: Vec<String> = (0..40)
            .map(|i| format!("vpn-{i}.guardianagroup.com"))
            .collect();
        let lleno = Deseo::default().con_conocidas(muchos.iter().map(|n| (n.as_str(), ip)));
        assert_eq!(lleno.conocidas.len(), CONOCIDAS_MAXIMO);
        // a file with others' names, or too many, is read with GUARDIANA's alone and the maximum
        let mut texto: serde_json::Value = serde_json::from_str(&encendida().texto()).unwrap();
        let mut todas = serde_json::Map::new();
        for n in &muchos {
            todas.insert(n.clone(), serde_json::json!("192.0.2.10"));
        }
        todas.insert("evil.example.com".into(), serde_json::json!("192.0.2.66"));
        texto["conocidas"] = serde_json::Value::Object(todas);
        let leido = Deseo::de_texto(&texto.to_string());
        assert_eq!(leido.conocidas.len(), CONOCIDAS_MAXIMO);
        assert!(!leido.conocidas.contains_key("evil.example.com"));
        // a file of before, with no addresses known: read as it was
        assert!(Deseo::de_texto("{\"estricto\": true}").conocidas.is_empty());
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
        assert!(!lee_si_se_puede(&dir).unwrap().encendida);
        olvida_en(&dir).unwrap();
        olvida_en(&dir).unwrap();
        assert!(!dir.join(ARCHIVO).exists());
        // no file is the VPN off, also for the reading that tells a file it cannot read apart
        assert_eq!(lee_si_se_puede(&dir).unwrap(), Deseo::default());
        // a file there that cannot be read (here, a folder in its place) is not taken for the VPN off
        std::fs::create_dir_all(dir.join(ARCHIVO)).unwrap();
        assert!(lee_si_se_puede(&dir).is_err());
        assert_eq!(lee_de(&dir), Deseo::default());
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
        // what the panel's loop waits after each failed attempt in a row (it counts the failure first, so the first
        // wait is `espera_tras(1)`): 5 s, 15 s, 15 s, and then every 30 s. docs/VPN.md says the same.
        let esperas: Vec<u64> = (1..=6).map(|f| espera_tras(f).as_secs()).collect();
        assert_eq!(esperas, [5, 15, 15, 30, 30, 30]);
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
