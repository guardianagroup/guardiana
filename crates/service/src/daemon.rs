//! Running as a system service (brief §2, §11): registration with the
//! Windows Service Control Manager, with systemd on Linux and with launchd on macOS. The service
//! process is `guardiana service run`; this module installs, removes,
//! starts, stops and reports it. The engine itself lives in the CLI crate.

use std::path::Path;

/// Service name in the SCM and the systemd unit name.
pub const SERVICE_NAME: &str = "guardiana";
/// Display name shown by Windows.
pub const DISPLAY_NAME: &str = "Guardiana";
/// Description shown by Windows and in the unit file.
pub const DESCRIPTION: &str =
    "Guardiana: guardián DNS de la casa. Todo ocurre en este equipo; sin cuenta ni servidor.";

/// Errors of service management.
#[derive(Debug)]
pub enum Error {
    /// Not supported on this platform.
    Unsupported,
    /// A system call or command failed.
    System(String),
    /// Needs administrator / root.
    Privileges,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported => f.write_str("the service is not available on this platform"),
            Self::System(s) => write!(f, "service: {s}"),
            Self::Privileges => f.write_str("administrator privileges required"),
        }
    }
}

impl std::error::Error for Error {}

/// What the service is doing right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Registered and running.
    Running,
    /// Registered, not running.
    Stopped,
    /// Not registered.
    NotInstalled,
}

// ---------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
mod imp {
    use std::ffi::OsString;
    use std::path::Path;
    use std::time::Duration;

    use windows_service::service::{
        ServiceAccess, ServiceAction, ServiceActionType, ServiceErrorControl,
        ServiceFailureActions, ServiceFailureResetPeriod, ServiceInfo, ServiceStartType,
        ServiceState, ServiceType,
    };
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

    use super::{Error, State, DESCRIPTION, DISPLAY_NAME, SERVICE_NAME};

    fn map(e: windows_service::Error) -> Error {
        let text = e.to_string();
        if text.contains("Access is denied") || text.contains("(os error 5)") {
            Error::Privileges
        } else {
            Error::System(text)
        }
    }

    fn manager(access: ServiceManagerAccess) -> Result<ServiceManager, Error> {
        ServiceManager::local_computer(None::<&str>, access).map_err(map)
    }

    pub(super) fn install(exe: &Path) -> Result<(), Error> {
        let manager =
            manager(ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE)?;
        let info = ServiceInfo {
            name: OsString::from(SERVICE_NAME),
            display_name: OsString::from(DISPLAY_NAME),
            service_type: ServiceType::OWN_PROCESS,
            start_type: ServiceStartType::AutoStart,
            error_control: ServiceErrorControl::Normal,
            executable_path: exe.to_path_buf(),
            launch_arguments: vec![OsString::from("service"), OsString::from("run")],
            dependencies: vec![],
            account_name: None, // LocalSystem
            account_password: None,
        };
        let service = manager
            .create_service(&info, ServiceAccess::CHANGE_CONFIG | ServiceAccess::START)
            .map_err(map)?;
        service.set_description(DESCRIPTION).map_err(map)?;
        // Restart on failure (brief §4: the watchdog), waiting 5 s.
        let restart = ServiceAction {
            action_type: ServiceActionType::Restart,
            delay: Duration::from_secs(5),
        };
        service
            .update_failure_actions(ServiceFailureActions {
                reset_period: ServiceFailureResetPeriod::After(Duration::from_secs(86_400)),
                reboot_msg: None,
                command: None,
                actions: Some(vec![restart.clone(), restart.clone(), restart]),
            })
            .map_err(map)?;
        Ok(())
    }

    pub(super) fn uninstall() -> Result<(), Error> {
        let manager = manager(ServiceManagerAccess::CONNECT)?;
        let service = manager
            .open_service(
                SERVICE_NAME,
                ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE,
            )
            .map_err(map)?;
        if service.query_status().map_err(map)?.current_state != ServiceState::Stopped {
            let _ = service.stop();
            std::thread::sleep(Duration::from_secs(2));
        }
        service.delete().map_err(map)
    }

    pub(super) fn start() -> Result<(), Error> {
        let manager = manager(ServiceManagerAccess::CONNECT)?;
        let service = manager
            .open_service(SERVICE_NAME, ServiceAccess::START)
            .map_err(map)?;
        service.start::<&str>(&[]).map_err(map)
    }

    pub(super) fn stop() -> Result<(), Error> {
        let manager = manager(ServiceManagerAccess::CONNECT)?;
        let service = manager
            .open_service(SERVICE_NAME, ServiceAccess::STOP)
            .map_err(map)?;
        service.stop().map(|_| ()).map_err(map)
    }

    pub(super) fn state() -> Result<State, Error> {
        let manager = manager(ServiceManagerAccess::CONNECT)?;
        let service = match manager.open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS) {
            Ok(s) => s,
            Err(windows_service::Error::Winapi(e)) if e.raw_os_error() == Some(1060) => {
                return Ok(State::NotInstalled);
            }
            Err(e) => return Err(map(e)),
        };
        let status = service.query_status().map_err(map)?;
        Ok(if status.current_state == ServiceState::Running {
            State::Running
        } else {
            State::Stopped
        })
    }
}

// ---------------------------------------------------------------------------
// Linux (systemd)
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
mod imp {
    use std::path::Path;

    use super::{Error, State, DESCRIPTION, SERVICE_NAME};
    use crate::sysdns::run_checked;

    const UNIT_PATH: &str = "/etc/systemd/system/guardiana.service";

    fn systemctl(args: &[&str]) -> Result<String, Error> {
        run_checked("systemctl", args).map_err(|e| {
            let text = e.to_string();
            if text.contains("Access denied") || text.contains("Permission denied") {
                Error::Privileges
            } else {
                Error::System(text)
            }
        })
    }

    pub(super) fn install(exe: &Path) -> Result<(), Error> {
        let unit = format!(
            "[Unit]\nDescription={DESCRIPTION}\nAfter=network-online.target\nWants=network-online.target\n\n\
             [Service]\nType=simple\nExecStart={} service run\nRestart=on-failure\nRestartSec=5\n\
             AmbientCapabilities=CAP_NET_BIND_SERVICE\nWorkingDirectory=/var/lib/guardiana\n\
             NoNewPrivileges=yes\nProtectSystem=full\n\
             ReadWritePaths=/var/lib/guardiana /etc/resolv.conf /run/systemd/resolve\n\
             ProtectHome=yes\nPrivateTmp=yes\nProtectKernelTunables=yes\nProtectKernelModules=yes\n\
             ProtectControlGroups=yes\nRestrictSUIDSGID=yes\nRestrictRealtime=yes\nLockPersonality=yes\n\
             SystemCallArchitectures=native\n\n\
             [Install]\nWantedBy=multi-user.target\n",
            exe.display()
        );
        std::fs::create_dir_all("/var/lib/guardiana").map_err(|e| Error::System(e.to_string()))?;
        std::fs::write(UNIT_PATH, unit).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                Error::Privileges
            } else {
                Error::System(e.to_string())
            }
        })?;
        systemctl(&["daemon-reload"])?;
        systemctl(&["enable", "--now", SERVICE_NAME])?;
        Ok(())
    }

    pub(super) fn uninstall() -> Result<(), Error> {
        let _ = systemctl(&["disable", "--now", SERVICE_NAME]);
        let _ = std::fs::remove_file(UNIT_PATH);
        systemctl(&["daemon-reload"])?;
        Ok(())
    }

    pub(super) fn start() -> Result<(), Error> {
        systemctl(&["start", SERVICE_NAME]).map(|_| ())
    }

    pub(super) fn stop() -> Result<(), Error> {
        systemctl(&["stop", SERVICE_NAME]).map(|_| ())
    }

    pub(super) fn state() -> Result<State, Error> {
        // The unit may come from `service install` (/etc/systemd/system) or
        // from the .deb (/lib/systemd/system): ask systemd, not the path.
        let loaded = run_checked(
            "systemctl",
            &["show", "-p", "LoadState", "--value", SERVICE_NAME],
        )
        .map(|o| o.trim() == "loaded")
        .unwrap_or_else(|_| Path::new(UNIT_PATH).exists());
        if !loaded {
            return Ok(State::NotInstalled);
        }
        let out = run_checked("systemctl", &["is-active", SERVICE_NAME])
            .unwrap_or_else(|_| "inactive".to_owned());
        Ok(if out.trim() == "active" {
            State::Running
        } else {
            State::Stopped
        })
    }
}

// ---------------------------------------------------------------------------
// macOS (launchd)
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod imp {
    use std::path::Path;

    use super::{Error, State};
    use crate::sysdns::run_checked;

    /// The same label and path the Mac installer uses, so both agree on what is installed.
    const LABEL: &str = "com.guardianagroup.guardiana";
    const PLIST: &str = "/Library/LaunchDaemons/com.guardianagroup.guardiana.plist";
    const DATA: &str = "/Library/Application Support/Guardiana";

    fn launchctl(args: &[&str]) -> Result<String, Error> {
        run_checked("launchctl", args).map_err(|e| {
            let text = e.to_string();
            if text.contains("Operation not permitted") || text.contains("Permission denied") {
                Error::Privileges
            } else {
                Error::System(text)
            }
        })
    }

    /// The resolvers the Mac uses today, which become the daemon's upstream. Without one,
    /// pointing the Mac at a resolver with nothing behind it would leave it with no names.
    fn upstreams() -> Vec<String> {
        run_checked("scutil", &["--dns"])
            .map(|t| {
                let mut v: Vec<String> = crate::sysdns::parse_scutil_dns(&t)
                    .into_iter()
                    .filter(|ip| !ip.is_loopback() && ip.is_ipv4())
                    .map(|ip| ip.to_string())
                    .collect();
                v.dedup();
                v
            })
            .unwrap_or_default()
    }

    pub(super) fn install(exe: &Path) -> Result<(), Error> {
        let ups = upstreams();
        if ups.is_empty() {
            return Err(Error::System(
                "no current DNS found (scutil --dns): nothing was changed".to_owned(),
            ));
        }
        let args: String = ups
            .iter()
            .map(|u| format!("<string>--upstream</string><string>{u}</string>"))
            .collect();
        let plist = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\"><dict>\n\
             <key>Label</key><string>{LABEL}</string>\n\
             <key>ProgramArguments</key><array><string>{}</string><string>observe</string>\
             <string>--listen</string><string>127.0.0.1:53</string>{args}\
             <string>--panel-listen</string><string>127.0.0.1:7443</string></array>\n\
             <key>EnvironmentVariables</key><dict><key>GUARDIANA_DATA</key><string>{DATA}</string></dict>\n\
             <key>RunAtLoad</key><true/>\n<key>KeepAlive</key><true/>\n\
             <key>StandardOutPath</key><string>{DATA}/guardiana.log</string>\n\
             <key>StandardErrorPath</key><string>{DATA}/guardiana.log</string>\n\
             </dict></plist>\n",
            exe.display()
        );
        std::fs::create_dir_all(DATA).map_err(|e| Error::System(e.to_string()))?;
        std::fs::write(PLIST, plist).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                Error::Privileges
            } else {
                Error::System(e.to_string())
            }
        })?;
        let _ = launchctl(&["bootout", "system", PLIST]);
        launchctl(&["bootstrap", "system", PLIST]).map(|_| ())
    }

    pub(super) fn uninstall() -> Result<(), Error> {
        let _ = launchctl(&["bootout", "system", PLIST]);
        std::fs::remove_file(PLIST).or_else(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Ok(())
            } else if e.kind() == std::io::ErrorKind::PermissionDenied {
                Err(Error::Privileges)
            } else {
                Err(Error::System(e.to_string()))
            }
        })
    }

    pub(super) fn start() -> Result<(), Error> {
        if !Path::new(PLIST).exists() {
            return Err(Error::System("the service is not installed".to_owned()));
        }
        // Already loaded: restart it. Not loaded: load it.
        if launchctl(&["print", &format!("system/{LABEL}")]).is_ok() {
            launchctl(&["kickstart", "-k", &format!("system/{LABEL}")]).map(|_| ())
        } else {
            launchctl(&["bootstrap", "system", PLIST]).map(|_| ())
        }
    }

    pub(super) fn stop() -> Result<(), Error> {
        // launchd restarts a KeepAlive job, so stopping means taking it out of the
        // session; the plist stays, which is what "installed but stopped" means here.
        launchctl(&["bootout", "system", PLIST]).map(|_| ())
    }

    pub(super) fn state() -> Result<State, Error> {
        if !Path::new(PLIST).exists() {
            return Ok(State::NotInstalled);
        }
        Ok(
            if launchctl(&["print", &format!("system/{LABEL}")]).is_ok() {
                State::Running
            } else {
                State::Stopped
            },
        )
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
mod imp {
    use std::path::Path;

    use super::{Error, State};

    pub(super) fn install(_exe: &Path) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    pub(super) fn uninstall() -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    pub(super) fn start() -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    pub(super) fn stop() -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    pub(super) fn state() -> Result<State, Error> {
        Ok(State::NotInstalled)
    }
}

/// Register the service to start with the system, pointing at `exe`.
pub fn install(exe: &Path) -> Result<(), Error> {
    imp::install(exe)
}

/// Stop and unregister the service.
pub fn uninstall() -> Result<(), Error> {
    imp::uninstall()
}

/// Start the registered service.
pub fn start() -> Result<(), Error> {
    imp::start()
}

/// Stop the running service.
pub fn stop() -> Result<(), Error> {
    imp::stop()
}

/// Current state.
pub fn state() -> Result<State, Error> {
    imp::state()
}
