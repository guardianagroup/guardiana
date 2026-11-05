//! `guardiana vpn apaga | estado` (strict mode, ronda 91; docs/VPN.md, «Modo estricto»): the way out that works with
//! GUARDIANA's service stopped. Strict mode's block lives in Windows apart from the service, so it holds when the
//! service crashes, stops or the computer restarts; this command takes it away whether the service answers or not:
//!
//! 1. when the service answers on this computer, it is asked to switch the VPN off (`POST /api/vpn/desconecta`, with the
//!    panel's key, as «Desconectar» does): it closes the tunnel and removes the block itself. The key goes only when the
//!    system's service manager says GUARDIANA's service is running and the one listening on 127.0.0.1:7443 is the
//!    service's own process (review of the 1.0.56 VPN): otherwise any program of any account could be listening there
//!    (with the service stopped, or having taken the port before the service started) and take the key an
//!    administrator sends;
//! 2. always, and directly: the VPN off in what is kept (`sistema/vpn.json`; `--desinstalando` removes the file) and every
//!    filter, the sublayer and the provider of strict mode out of Windows, by their fixed keys
//!    (`guardiana_panel::quita_el_bloqueo`). That needs an administrator: from the Start menu's entry (`--menu`) the
//!    command asks Windows for the rights itself, and waits for Enter before its window closes.
//!
//! `estado` says what is kept and what Windows has (`--json` for the tests, with whether the panel's port is the
//! service's).
//!
//! Every sentence printed comes from `crates/panel/i18n/es.json` (`cli.vpn.*`).

use std::error::Error;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use guardiana_core::i18n::{self, Texts};
use guardiana_core::paths;

use crate::args::Opts;

/// `guardiana vpn [estado | apaga]`.
pub fn run(opts: &Opts) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    match opts
        .positional()
        .first()
        .map(String::as_str)
        .unwrap_or("estado")
    {
        "apaga" | "apagar" | "off" => apaga(t, opts),
        "estado" | "status" => {
            estado(t, opts.has("json"));
            Ok(())
        }
        _ => {
            println!("{}", t.cli("vpn.uso"));
            Ok(())
        }
    }
}

/// The VPN off, and strict mode's block out of Windows, with the service running or not.
fn apaga(t: &Texts, opts: &Opts) -> Result<(), Box<dyn Error>> {
    let menu = opts.has("menu");
    let por_el_servicio = pide_al_servicio();
    let resultado = guardiana_panel::quita_el_bloqueo(opts.has("desinstalando"));
    let salida = match resultado {
        Ok(q) => {
            println!(
                "{}",
                t.cli(if por_el_servicio || q.filtros > 0 {
                    "vpn.apagada"
                } else {
                    "vpn.nada"
                })
            );
            Ok(())
        }
        // the service already did it; this process only lacked the rights to look
        Err(_) if por_el_servicio => {
            println!("{}", t.cli("vpn.apagada"));
            Ok(())
        }
        Err(e) if e.sin_permiso => {
            if menu && !opts.has("elevado") {
                println!("{}", t.cli("vpn.pide_permiso"));
                if pide_permiso() {
                    // the window with the rights says the rest
                    return Ok(());
                }
            }
            eprintln!("{}", t.cli("vpn.privilegios"));
            Err("".into())
        }
        Err(e) => {
            eprintln!("{}", t.cli("vpn.fallo").replace("{e}", &e.motivo));
            Err("".into())
        }
    };
    if menu {
        println!("{}", t.cli("vpn.intro"));
        let mut linea = String::new();
        let _ = std::io::stdin().read_line(&mut linea);
    }
    salida
}

/// What is kept and what Windows has of strict mode.
fn estado(t: &Texts, json: bool) {
    let e = guardiana_panel::estado_del_modo_estricto();
    if json {
        // and whether the one listening on the panel's port is the service (what `apaga` looks at before sending it the
        // panel's key)
        let mut v = serde_json::to_value(&e).unwrap_or_default();
        if let Some(o) = v.as_object_mut() {
            o.insert(
                "panel_del_servicio".to_owned(),
                serde_json::Value::Bool(panel_del_servicio(guardiana_panel::DEFAULT_PORT)),
            );
        }
        println!("{v}");
        return;
    }
    println!(
        "{}",
        t.cli("vpn.estado")
            .replace(
                "{estricto}",
                t.cli(if e.estricto {
                    "vpn.encendido"
                } else {
                    "vpn.apagado"
                })
            )
            .replace(
                "{vpn}",
                t.cli(if e.encendida {
                    "vpn.la_vpn_encendida"
                } else {
                    "vpn.la_vpn_apagada"
                })
            )
            .replace("{persistentes}", &e.persistentes.to_string())
            .replace("{arranque}", &e.arranque.to_string())
    );
    if let Some(no) = e.error {
        eprintln!("{}", t.cli("vpn.fallo").replace("{e}", &no));
    }
}

/// `POST /api/vpn/desconecta` to GUARDIANA's service on this computer, with the panel's key, as «Desconectar» does:
/// whether it answered that the VPN is off. `false` when the service is not running, does not answer or the key cannot
/// be read.
///
/// Only to the service (review of the 1.0.56 VPN): before the key is even read, the system's service manager must say
/// that GUARDIANA's service is running, and the one listening on 127.0.0.1:7443 must be that service's process (second
/// review: a program can take the port while the service is stopped, and the service then runs without its panel).
/// Otherwise whoever listens there could be any program of any account of this computer, and the key an
/// administrator's `vpn apaga` sends would be theirs; then the way without the service, which needs no key, is taken
/// alone.
fn pide_al_servicio() -> bool {
    let puerto = guardiana_panel::DEFAULT_PORT;
    if !en_marcha(guardiana_service::daemon::state()) || !panel_del_servicio(puerto) {
        return false;
    }
    let Ok(llave) = std::fs::read_to_string(paths::data_dir().join(guardiana_panel::TOKEN_FILE))
    else {
        return false;
    };
    let llave = llave.trim();
    if llave.len() != 64 || !llave.bytes().all(|b| b.is_ascii_hexdigit()) {
        return false;
    }
    let Ok(mut s) = TcpStream::connect_timeout(
        &SocketAddr::from(([127, 0, 0, 1], puerto)),
        Duration::from_secs(2),
    ) else {
        return false;
    };
    // closing the tunnel can take a few seconds
    let _ = s.set_read_timeout(Some(Duration::from_secs(40)));
    let _ = s.set_write_timeout(Some(Duration::from_secs(5)));
    let pedido = format!(
        "POST /api/vpn/desconecta HTTP/1.1\r\nHost: 127.0.0.1:{puerto}\r\nX-Guardiana-Token: {llave}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    if s.write_all(pedido.as_bytes()).is_err() {
        return false;
    }
    let mut respuesta = String::new();
    let _ = s.read_to_string(&mut respuesta);
    contesto_que_si(&respuesta)
}

/// Whether the panel's answer says the VPN is off: `200` and `"ok":true`.
fn contesto_que_si(respuesta: &str) -> bool {
    respuesta.starts_with("HTTP/1.1 200") && respuesta.contains("\"ok\":true")
}

/// Whether the system's service manager says GUARDIANA's service is running: only then does the panel's key go to
/// 127.0.0.1. Not knowing is not running.
fn en_marcha(
    estado: Result<guardiana_service::daemon::State, guardiana_service::daemon::Error>,
) -> bool {
    matches!(estado, Ok(guardiana_service::daemon::State::Running))
}

/// Whether the one listening on 127.0.0.1:`puerto` is GUARDIANA's service: the process the system's service manager says
/// the service runs in (`QueryServiceStatusEx`) against the one Windows' table of listening sockets names
/// (`GetExtendedTcpTable`), which must be the only one listening on that port there or on every address. Off Windows,
/// never (there is no VPN to turn off there).
fn panel_del_servicio(puerto: u16) -> bool {
    es_el_servicio(
        guardiana_service::daemon::process_id(),
        guardiana_apps::vigia::escucha_en(std::net::Ipv4Addr::LOCALHOST, puerto),
    )
}

/// The decision, pure: the service's process (`servicio`) and the one listening (`escucha`) must both be known and be
/// the same. Not knowing either is not the service.
fn es_el_servicio(servicio: Option<u32>, escucha: Option<u32>) -> bool {
    matches!((servicio, escucha), (Some(s), Some(e)) if s == e && s != 0)
}

/// From the Start menu's entry, without an administrator's rights: the same command again, with Windows asked for them
/// (its own window says what happened). Whether Windows started it.
#[cfg(windows)]
fn pide_permiso() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let ruta = exe.to_string_lossy().replace('\'', "''");
    std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!(
                "Start-Process -FilePath '{ruta}' -ArgumentList 'vpn','apaga','--menu','--elevado' -Verb RunAs"
            ),
        ])
        .status()
        .is_ok_and(|s| s.success())
}

/// Only Windows has an entry in a Start menu for this.
#[cfg(not(windows))]
fn pide_permiso() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solo_un_si_del_panel_cuenta() {
        assert!(contesto_que_si(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n\r\n{\"ok\":true}"
        ));
        assert!(!contesto_que_si(
            "HTTP/1.1 200 OK\r\n\r\n{\"ok\":false,\"motivo\":\"x\"}"
        ));
        assert!(!contesto_que_si(
            "HTTP/1.1 401 Unauthorized\r\n\r\nsession token required"
        ));
        assert!(!contesto_que_si(""));
    }

    /// Review of the 1.0.56 VPN: the panel's key goes only to a service the system says is running; stopped, not
    /// installed, or not knowing, never.
    #[test]
    fn la_llave_solo_va_con_el_servicio_en_marcha() {
        use guardiana_service::daemon::{Error, State};
        assert!(en_marcha(Ok(State::Running)));
        assert!(!en_marcha(Ok(State::Stopped)));
        assert!(!en_marcha(Ok(State::NotInstalled)));
        assert!(!en_marcha(Err(Error::Privileges)));
        assert!(!en_marcha(Err(Error::Unsupported)));
    }

    /// Second review of the 1.0.56 VPN: the panel's key goes only when the one listening on 127.0.0.1:7443 is the
    /// service's own process; another process, or not knowing either of them, never.
    #[test]
    fn la_llave_solo_va_al_proceso_del_servicio() {
        assert!(es_el_servicio(Some(1234), Some(1234)));
        assert!(!es_el_servicio(Some(1234), Some(666)));
        assert!(!es_el_servicio(Some(1234), None));
        assert!(!es_el_servicio(None, Some(1234)));
        assert!(!es_el_servicio(None, None));
        assert!(!es_el_servicio(Some(0), Some(0)));
        // off Windows the service's process is never known: the way without the service, alone
        #[cfg(not(windows))]
        assert!(!panel_del_servicio(guardiana_panel::DEFAULT_PORT));
    }

    /// Off Windows there is no block: `apaga` says the VPN was already off, and `estado` reads what is kept.
    #[test]
    fn el_estado_se_lee_sin_windows() {
        let e = guardiana_panel::estado_del_modo_estricto();
        #[cfg(not(windows))]
        assert_eq!(
            (e.persistentes, e.arranque, e.reenvio, e.objetos),
            (0, 0, 0, false)
        );
        let texto = serde_json::to_string(&e).unwrap_or_default();
        assert!(texto.contains("\"estricto\":"));
        assert!(texto.contains("\"persistentes\":"));
        // 1.0.56: cuántos están en las capas de reenvío (lo mira el trabajo «estricto» de vpn.yml)
        assert!(texto.contains("\"reenvio\":"));
    }
}
