//! `evorift-testd` — remote test agent for the second Windows laptop.
//!
//! Two modes, mirroring `evorift-svc`:
//!   * started by the SCM (the installed `evorift-testd` service) → runs under
//!     `service_dispatcher`, so the SCM gets its status report and the start does not time out;
//!   * started from a console (`--console`, or any run where no SCM is attached) → runs in the
//!     foreground, which is how you smoke-test a config before installing it.
//!
//! Usage:
//!   evorift-testd                    # service mode, default config path
//!   evorift-testd --console          # foreground, default config path
//!   evorift-testd --console <path>   # foreground, explicit config
//!
//! Deliberately thin: everything worth testing lives in `evorift_lib::testd`, so it is covered
//! by `cargo test --lib`. Installed by `scripts/testd-install.ps1`; see docs/REMOTE-TESTING.md.

use std::path::PathBuf;

use evorift_lib::testd;

/// Resolve the config path from argv, ignoring the mode flags.
fn config_path_from_args() -> PathBuf {
    std::env::args()
        .skip(1)
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(testd::default_config_path)
}

/// Record a startup failure where a human will actually find it.
///
/// A service's stderr goes nowhere. When the agent failed to parse its config it died before
/// opening the audit log, so the only evidence anywhere was "service Running, nothing
/// listening" — which is indistinguishable from a firewall problem from the outside. This file
/// is the breadcrumb that makes that case self-diagnosing; `scripts/check.ps1` prints it.
fn record_startup_failure(message: &str) {
    use std::io::Write;
    let dir = testd::default_state_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("[testd] cannot create {} to record startup failure: {e}", dir.display());
        return;
    }
    let path = dir.join("startup-error.log");
    match std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        Ok(mut f) => {
            let stamp = evorift_lib::testd::util::iso8601_utc(evorift_lib::testd::util::unix_secs());
            if let Err(e) = writeln!(f, "[{stamp}] {message}") {
                eprintln!("[testd] could not write {}: {e}", path.display());
            }
        }
        Err(e) => eprintln!("[testd] could not open {}: {e}", path.display()),
    }
}

/// Load config + token and serve. Returns only on a fatal error.
fn serve() -> Result<(), testd::StartupError> {
    // This process runs as LocalSystem when installed as a service, which is what lets the
    // recovery routine actually change DNS instead of simulating it (see sys::privileged).
    // Setting the marker explicitly means a console run under an elevated shell behaves the
    // same way, rather than silently no-op'ing the DNS step.
    std::env::set_var("EVORIFT_PRIVILEGED", "1");
    let path = config_path_from_args();
    eprintln!("[testd] starting with config {}", path.display());
    testd::start(&path)
}

#[cfg(windows)]
mod svc {
    use std::ffi::OsString;
    use std::sync::mpsc;
    use std::time::Duration;
    use windows_service::service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    };
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
    use windows_service::{define_windows_service, service_dispatcher};

    /// Must match the name `scripts/testd-install.ps1` passes to `sc.exe create`.
    pub const SERVICE_NAME: &str = "evorift-testd";

    define_windows_service!(ffi_service_main, service_main);

    fn service_main(_args: Vec<OsString>) {
        if let Err(e) = run_service() {
            eprintln!("[testd] service error: {e}");
        }
    }

    /// Why the service loop ended. The distinction matters to the SCM: a clean stop must report
    /// success, a startup failure must report failure so the configured restart action fires and
    /// `Get-Service` does not claim the agent is fine.
    enum Ended {
        Stopped,
        Failed,
    }

    fn run_service() -> windows_service::Result<()> {
        let (stop_tx, stop_rx) = mpsc::channel::<Ended>();
        let fail_tx = stop_tx.clone();

        let status_handle = service_control_handler::register(SERVICE_NAME, move |control| {
            match control {
                ServiceControl::Stop | ServiceControl::Shutdown => {
                    // A send failure means the main thread is already gone, which is the state
                    // the sender wanted anyway.
                    let _ = stop_tx.send(Ended::Stopped);
                    ServiceControlHandlerResult::NoError
                }
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        })?;

        let status = |state: ServiceState, accept: ServiceControlAccept| ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: state,
            controls_accepted: accept,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        };

        status_handle.set_service_status(status(
            ServiceState::Running,
            ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        ))?;

        // The listener blocks, so it gets its own thread; this one waits for the SCM's STOP.
        std::thread::spawn(move || {
            if let Err(e) = super::serve() {
                // Do NOT keep reporting Running with nothing listening — that is the exact
                // failure that made a BOM in the config file take a trip to the laptop to
                // diagnose. Write the reason down and take the service down with a failure
                // exit code so the SCM's restart action fires and Get-Service tells the truth.
                let message = format!("FATAL: {e}");
                eprintln!("[testd] {message}");
                super::record_startup_failure(&message);
                let _ = fail_tx.send(Ended::Failed);
            }
        });

        // Blocks until the SCM asks us to stop, or the listener gives up.
        let ended = stop_rx.recv().unwrap_or(Ended::Stopped);
        let exit_code = match ended {
            Ended::Stopped => ServiceExitCode::Win32(0),
            // A service-specific non-zero code marks the service as failed.
            Ended::Failed => ServiceExitCode::ServiceSpecific(1),
        };
        status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code,
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })?;
        Ok(())
    }

    pub fn main_impl() -> std::process::ExitCode {
        if std::env::args().any(|a| a == "--console") {
            return match super::serve() {
                Ok(()) => std::process::ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("[testd] FATAL: {e}");
                    std::process::ExitCode::FAILURE
                }
            };
        }
        // Try the SCM first; if there is none we were run from a shell, so fall to foreground.
        if service_dispatcher::start(SERVICE_NAME, ffi_service_main).is_err() {
            eprintln!("[testd] no SCM attached — running in the foreground (same as --console)");
            return match super::serve() {
                Ok(()) => std::process::ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("[testd] FATAL: {e}");
                    std::process::ExitCode::FAILURE
                }
            };
        }
        std::process::ExitCode::SUCCESS
    }
}

fn main() -> std::process::ExitCode {
    if std::env::args().any(|a| a == "--help" || a == "-h") {
        println!("evorift-testd [--console] [config-path]");
        println!();
        println!("Remote test agent. Default config: {}", testd::default_config_path().display());
        println!("Endpoints: POST /push, POST /run, GET /job/<id>, GET /pull, GET /health, POST /recover");
        return std::process::ExitCode::SUCCESS;
    }

    #[cfg(windows)]
    {
        svc::main_impl()
    }
    #[cfg(not(windows))]
    {
        match serve() {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("[testd] FATAL: {e}");
                std::process::ExitCode::FAILURE
            }
        }
    }
}
