//! `guardiana service install|uninstall|start|stop|status|run` (brief §2,
//! §11). `run` is what the system starts: the engine until the service
//! manager says stop.

use std::error::Error;

use guardiana_core::i18n;
use guardiana_service::daemon::{self, State};

use crate::args::Opts;
#[cfg(not(target_os = "windows"))]
use crate::engine::{self, EngineConfig};

fn report(t: &guardiana_core::i18n::Texts, e: daemon::Error) -> Box<dyn Error> {
    match e {
        daemon::Error::Privileges => t.cli("servicio.privilegios").into(),
        daemon::Error::Unsupported => t.cli("servicio.no_soportado").into(),
        other => Box::new(other),
    }
}

/// Print the service state.
pub fn status() -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    let key = match daemon::state().map_err(|e| report(t, e))? {
        State::Running => "servicio.estado.running",
        State::Stopped => "servicio.estado.stopped",
        State::NotInstalled => "servicio.estado.none",
    };
    println!("{}", t.cli(key));
    Ok(())
}

pub fn run(opts: &Opts) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    let word = opts
        .positional()
        .first()
        .map(String::as_str)
        .unwrap_or("status");
    match word {
        "install" | "instalar" => {
            let exe = std::env::current_exe()?;
            daemon::install(&exe).map_err(|e| report(t, e))?;
            let _ = daemon::start();
            println!("{}", t.cli("servicio.instalado"));
            println!("{}", t.cli("servicio.dns_aviso"));
            Ok(())
        }
        "uninstall" | "quitar" => {
            daemon::uninstall().map_err(|e| report(t, e))?;
            println!("{}", t.cli("servicio.quitado"));
            Ok(())
        }
        "start" => {
            daemon::start().map_err(|e| report(t, e))?;
            println!("{}", t.cli("servicio.arrancado"));
            Ok(())
        }
        "stop" => {
            daemon::stop().map_err(|e| report(t, e))?;
            println!("{}", t.cli("servicio.parado"));
            Ok(())
        }
        "run" => run_service(),
        _ => status(),
    }
}

/// The service body: quiet engine until the manager asks to stop.
fn run_service() -> Result<(), Box<dyn Error>> {
    #[cfg(target_os = "windows")]
    {
        windows::run()
    }
    #[cfg(not(target_os = "windows"))]
    {
        // systemd (or a foreground run): stop on SIGTERM / Ctrl+C.
        let cfg = EngineConfig::default_service();
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(engine::run(cfg, async {
            #[cfg(unix)]
            {
                let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .ok();
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = async { match term.as_mut() { Some(s) => { s.recv().await; } None => std::future::pending::<()>().await } } => {}
                }
            }
            #[cfg(not(unix))]
            {
                let _ = tokio::signal::ctrl_c().await;
            }
        }))
    }
}

#[cfg(target_os = "windows")]
mod windows {
    // The macro below generates the `extern "system"` entry point the SCM
    // calls and converts its raw argument array; that conversion is the one
    // `unsafe` in this program, owned by the windows-service crate (brief §2).
    #![allow(unsafe_code)]

    use std::error::Error;
    use std::ffi::OsString;
    use std::time::Duration;

    use windows_service::service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    };
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
    use windows_service::{define_windows_service, service_dispatcher};

    use crate::engine::{self, EngineConfig};

    define_windows_service!(ffi_service_main, service_main);

    fn status(state: ServiceState, wait_hint: Duration) -> ServiceStatus {
        ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: state,
            controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint,
            process_id: None,
        }
    }

    fn service_main(_args: Vec<OsString>) {
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let handler = move |control| match control {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                let _ = tx.send(());
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        };
        let Ok(handle) = service_control_handler::register(super::SERVICE, handler) else {
            return;
        };
        let _ =
            handle.set_service_status(status(ServiceState::StartPending, Duration::from_secs(10)));
        let Ok(rt) = tokio::runtime::Runtime::new() else {
            let _ = handle.set_service_status(status(ServiceState::Stopped, Duration::ZERO));
            return;
        };
        let _ = handle.set_service_status(status(ServiceState::Running, Duration::ZERO));
        let cfg = EngineConfig::default_service();
        let _ = rt.block_on(engine::run(cfg, async move {
            // The stop request arrives on a plain thread; poll it without blocking the runtime.
            loop {
                if rx.try_recv().is_ok() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        }));
        let _ = handle.set_service_status(status(ServiceState::Stopped, Duration::ZERO));
    }

    pub(super) fn run() -> Result<(), Box<dyn Error>> {
        service_dispatcher::start(super::SERVICE, ffi_service_main)?;
        Ok(())
    }
}

/// Service name, shared with the service crate.
#[allow(dead_code)]
const SERVICE: &str = daemon::SERVICE_NAME;
