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

/// The systemd unit the .deb ships. `service install` (the .tar.gz path) writes the same one,
/// built from this text, so there is a single copy to get right. There used to be two: the
/// .deb's was fixed after measuring on Ubuntu 24.04 that `ProtectSystem=full` leaves /etc
/// read-only, so the DNS could never be pointed at the guardian on systemd-resolved; the one
/// in this file kept `full`, and every .tar.gz install got a guardian that could not be
/// switched on (review of 1 Oct 2026, entry 8).
#[cfg(any(target_os = "linux", test))]
const DEB_UNIT: &str = include_str!("../../../build/deb/guardiana.service");

/// The unit text for a program installed at `exe`: the .deb's unit with its `ExecStart` line
/// pointing at `exe`, everything else (hardening, `Restart=always`, the comments that say why)
/// untouched.
#[cfg(any(target_os = "linux", test))]
fn systemd_unit(exe: &Path) -> String {
    // systemd reads `%` as a specifier and splits the command line on spaces.
    let exe = exe.display().to_string().replace('%', "%%");
    let exe = if exe.contains(char::is_whitespace) {
        format!("\"{exe}\"")
    } else {
        exe
    };
    let mut unit: String = DEB_UNIT
        .lines()
        .map(|line| {
            if line.starts_with("ExecStart=") {
                format!("ExecStart={exe} service run")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    unit.push('\n');
    unit
}

#[cfg(target_os = "linux")]
mod imp {
    use std::path::Path;

    use super::{Error, State, SERVICE_NAME};
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
        let unit = super::systemd_unit(exe);
        std::fs::create_dir_all("/var/lib/guardiana").map_err(|e| Error::System(e.to_string()))?;
        std::fs::write(UNIT_PATH, unit).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                Error::Privileges
            } else {
                Error::System(e.to_string())
            }
        })?;
        systemctl(&["daemon-reload"])?;
        systemctl(&["enable", SERVICE_NAME])?;
        // `enable --now` leaves a service that is already running alone, so an update over a
        // running install kept the old program and the old unit's sandbox in memory until the
        // next boot (the 1.0.1 .tar.gz unit is exactly what has to go). `restart` starts it
        // when stopped and replaces it when running; the stop gives the DNS back and the start
        // points the machine at the guardian again (review of 1 Oct 2026, entry 8).
        systemctl(&["restart", SERVICE_NAME])?;
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

/// The data folder of the Mac daemon, where its log goes.
#[cfg(any(target_os = "macos", test))]
const MAC_DATA: &str = "/Library/Application Support/Guardiana";

/// The LaunchDaemon for a program installed at `exe`, the same one `build/mac/instalar.sh`
/// writes: `service run`, which follows the network and gives the DNS back when it stops.
///
/// Until 1.0.2 `service install` wrote the plist of 1.0.0: `observe --upstream <the servers of
/// the day>`, frozen for ever (a Mac installed at home and opened elsewhere forwarded every
/// name to a router that was not there), with no DNS given back on stop and the panel's key
/// written into the log (review of 5 Oct 2026, serious 6). One text now, checked against the
/// installer by a test.
#[cfg(any(target_os = "macos", test))]
fn launchd_plist(exe: &Path) -> String {
    let xml = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\"><dict>\n\
         <key>Label</key><string>{MAC_LABEL}</string>\n\
         <key>ProgramArguments</key><array><string>{}</string><string>service</string><string>run</string></array>\n\
         <key>RunAtLoad</key><true/>\n\
         <key>KeepAlive</key><true/>\n\
         <key>StandardOutPath</key><string>{MAC_DATA}/guardiana.log</string>\n\
         <key>StandardErrorPath</key><string>{MAC_DATA}/guardiana.log</string>\n\
         </dict></plist>\n",
        xml(&exe.display().to_string())
    )
}

/// The launchd label, the same the Mac installer uses.
#[cfg(any(target_os = "macos", test))]
const MAC_LABEL: &str = "com.guardianagroup.guardiana";

#[cfg(target_os = "macos")]
mod imp {
    use std::path::Path;

    use super::{Error, State};
    use crate::sysdns::run_checked;

    /// The same label and path the Mac installer uses, so both agree on what is installed.
    const LABEL: &str = super::MAC_LABEL;
    const PLIST: &str = "/Library/LaunchDaemons/com.guardianagroup.guardiana.plist";
    const DATA: &str = super::MAC_DATA;

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

    pub(super) fn install(exe: &Path) -> Result<(), Error> {
        // No check of today's DNS and no `--upstream`: the daemon does not touch the Mac's DNS
        // (the panel does, with consent) and finds its upstreams by itself, following the
        // network (`service run`).
        std::fs::create_dir_all(DATA).map_err(|e| Error::System(e.to_string()))?;
        std::fs::write(PLIST, super::launchd_plist(exe)).map_err(|e| {
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{launchd_plist, systemd_unit, DEB_UNIT, DESCRIPTION};

    /// The lines systemd reads: no comments, no blanks.
    fn directives(unit: &str) -> Vec<&str> {
        unit.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect()
    }

    /// `service install` on a Mac and the Mac installer write the same daemon: `service run`,
    /// no frozen upstream, no GUARDIANA_DATA (review of 5 Oct 2026, serious 6).
    #[test]
    fn the_mac_plist_runs_the_service_like_the_installer() {
        let plist = launchd_plist(Path::new("/usr/local/guardiana/guardiana"));
        let args = "<key>ProgramArguments</key><array><string>/usr/local/guardiana/guardiana</string><string>service</string><string>run</string></array>";
        assert!(plist.contains(args), "{plist}");
        assert!(!plist.contains("--upstream"));
        assert!(!plist.contains("observe"));
        assert!(!plist.contains("GUARDIANA_DATA"));
        assert!(plist.contains("<key>KeepAlive</key><true/>"));
        let instalador = include_str!("../../../build/mac/instalar.sh");
        assert!(
            instalador.contains(
                "<key>ProgramArguments</key><array><string>$BIN_DIR/guardiana</string><string>service</string><string>run</string></array>"
            ),
            "build/mac/instalar.sh no longer writes `service run`"
        );
        assert!(!instalador.contains("<key>EnvironmentVariables</key>"));
        // A path with characters XML reads as markup stays one string.
        assert!(launchd_plist(Path::new("/opt/a&b/guardiana")).contains("/opt/a&amp;b/guardiana"));
    }

    #[test]
    fn the_written_unit_is_the_deb_unit_with_the_path_changed() {
        // Review of 1 Oct 2026, entry 8: the unit written by `service install` had
        // `ProtectSystem=full` (so /etc was read-only and the DNS could never be pointed at the
        // guardian with systemd-resolved) and `Restart=on-failure`, while the .deb's had been
        // fixed long before. One text now, so they cannot drift again.
        let tarball = systemd_unit(Path::new("/usr/local/bin/guardiana"));
        let got = directives(&tarball);
        assert!(got.contains(&"ExecStart=/usr/local/bin/guardiana service run"));
        assert!(got.contains(&"ProtectSystem=yes"));
        assert!(!got.contains(&"ProtectSystem=full"));
        assert!(got.contains(&"Restart=always"));
        // Each path with «-»: a machine without systemd-resolved has no /run/systemd/resolve,
        // and without the dash systemd refuses to start the service at all (review of 5 Oct
        // 2026: Debian, Arch, openSUSE, status=226/NAMESPACE).
        assert!(got.contains(&"ReadWritePaths=-/var/lib/guardiana -/run/systemd/resolve"));
        let rw = got
            .iter()
            .filter(|l| l.starts_with("ReadWritePaths="))
            .flat_map(|l| l["ReadWritePaths=".len()..].split_whitespace())
            .collect::<Vec<_>>();
        assert!(
            rw.iter().all(|p| p.starts_with('-')),
            "every ReadWritePaths entry must be optional: {rw:?}"
        );
        assert!(
            !got.iter().any(|l| l.contains("/etc/resolv.conf")),
            "no directive may name /etc/resolv.conf: /etc is writable through ProtectSystem=yes"
        );
        assert!(got.contains(&format!("Description={DESCRIPTION}").as_str()));
        // Every directive of the .deb unit but ExecStart is there, unchanged.
        for line in directives(DEB_UNIT)
            .iter()
            .filter(|l| !l.starts_with("ExecStart="))
        {
            assert!(got.contains(line), "missing from the written unit: {line}");
        }
        assert_eq!(got.len(), directives(DEB_UNIT).len());
        // And with the .deb's own path the two texts are identical, comments included.
        assert_eq!(systemd_unit(Path::new("/usr/bin/guardiana")), DEB_UNIT);
    }

    #[test]
    fn a_path_with_spaces_or_percent_is_safe_for_systemd() {
        let unit = systemd_unit(Path::new("/opt/my tools/100%/guardiana"));
        assert!(
            directives(&unit).contains(&"ExecStart=\"/opt/my tools/100%%/guardiana\" service run")
        );
    }
}
