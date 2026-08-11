//! Generic Windows service manager (docs/02 §4, docs/05 §3). install/uninstall/status/list for the
//! DPI/tunnel services evorift orchestrates or cleans up. Privileged ops → `sys` (sim when unprivileged);
//! queries are read-only. Dependency-ordered "remove all" lives in item 5.3.

use serde::{Deserialize, Serialize};

/// Services evorift knows about (docs/02 §4 managed-services list + our own `EvoriftSvc`). This order is
/// the canonical set to query/list — NOT the teardown order (see item 5.3 for the dependency-safe order).
pub const MANAGED_SERVICES: &[&str] = &[
    "EvoriftSvc", "zapret", "GoodbyeDPI", "WinDivert", "winws1", "winws2",
    "wiresock-client-service", "ByeDPI", "ProxiFyreService",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub name: String,
    /// "running" | "stopped" | "pending" | "unknown" | "not-installed".
    pub state: String,
}

/// Parse a `sc query <name>` output into a coarse state. Pure → testable.
pub fn parse_state(out: &str) -> &'static str {
    let up = out.to_uppercase();
    if up.contains("RUNNING") {
        "running"
    } else if up.contains("START_PENDING") || up.contains("STOP_PENDING") {
        "pending"
    } else if up.contains("STOPPED") {
        "stopped"
    } else if up.contains("STATE") {
        "unknown"
    } else {
        "not-installed" // no STATE line → service doesn't exist (sc error 1060)
    }
}

/// Query one service's state (read-only; works unprivileged).
pub fn query(name: &str) -> ServiceStatus {
    let out = crate::sys::query_os("sc", &["query", name]);
    ServiceStatus { name: name.to_string(), state: parse_state(&out).to_string() }
}

/// Query all known services.
pub fn list() -> Vec<ServiceStatus> {
    MANAGED_SERVICES.iter().map(|n| query(n)).collect()
}

/// Install an auto-start service:
/// `sc create <name> binPath= "<bin> <args>" start= auto DisplayName= <display>` + description + start.
/// Privileged (sim when unprivileged). `bin`/`args` come from our own bundle. The `binPath=`/`start=`/
/// `DisplayName=` tokens carry a trailing `=` and the value is the next argv token — sc.exe's syntax.
pub fn install(name: &str, bin: &str, args: &[&str], display: &str, description: &str) -> Result<(), String> {
    let bin_path = if args.is_empty() {
        format!("\"{bin}\"")
    } else {
        format!("\"{bin}\" {}", args.join(" "))
    };
    crate::sys::run_os(
        "sc",
        &["create", name, "binPath=", &bin_path, "start=", "auto", "DisplayName=", display],
    )?;
    let _ = crate::sys::run_os("sc", &["description", name, description]);
    crate::sys::run_os("sc", &["start", name])?;
    if crate::sys::privileged() {
        crate::rollback::record(crate::rollback::Change::ServiceCreated { name: name.to_string() });
    }
    Ok(())
}

/// Stop + delete a service (best-effort stop, then delete). Privileged.
pub fn uninstall(name: &str) -> Result<(), String> {
    let _ = crate::sys::run_os("sc", &["stop", name]);
    crate::sys::run_os("sc", &["delete", name])
}

/// Dependency-safe teardown order (docs/05 §3). WinDivert is consumed by zapret/GoodbyeDPI, so those
/// stop FIRST; our orchestration service first of all; the proxy/tunnel services last. Removing in this
/// exact order avoids "service marked for deletion / still in use" failures.
pub const TEARDOWN_ORDER: &[&str] = &[
    "EvoriftSvc", "zapret", "GoodbyeDPI", "WinDivert", "winws1", "winws2",
    "wiresock-client-service", "ByeDPI", "ProxiFyreService",
];

/// Tear down every managed service in dependency order (docs/05 §3 "Remove all"). For each INSTALLED
/// service: `sc stop` → wait until not running (avoids DELETE_PENDING limbo) → `sc delete`. Idempotent:
/// not-installed services are skipped. Returns the names actually acted on (for logging/UI). Privileged
/// → sim when unprivileged.
pub fn remove_all() -> Vec<String> {
    let mut acted = Vec::new();
    for &name in TEARDOWN_ORDER {
        if query(name).state == "not-installed" {
            continue; // idempotent: nothing to do
        }
        let _ = crate::sys::run_os("sc", &["stop", name]);
        for _ in 0..6 {
            std::thread::sleep(std::time::Duration::from_millis(500));
            if query(name).state != "running" {
                break; // stopped (or gone) → safe to delete
            }
        }
        let _ = crate::sys::run_os("sc", &["delete", name]);
        crate::sys::audit(&format!("remove_all: torn down {name}"));
        acted.push(name.to_string());
    }
    acted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_state_cases() {
        assert_eq!(parse_state("STATE              : 4  RUNNING"), "running");
        assert_eq!(parse_state("STATE              : 1  STOPPED"), "stopped");
        assert_eq!(parse_state("STATE              : 2  START_PENDING"), "pending");
        assert_eq!(parse_state("[SC] OpenService FAILED 1060: The specified service does not exist"), "not-installed");
        assert_eq!(parse_state(""), "not-installed");
    }

    #[test]
    fn managed_list_and_install_sim() {
        assert!(MANAGED_SERVICES.contains(&"WinDivert"));
        assert!(MANAGED_SERVICES.contains(&"ProxiFyreService"));
        // list() returns one entry per managed service (read-only; states vary by machine)
        assert_eq!(list().len(), MANAGED_SERVICES.len());
        // install/uninstall are sim no-ops when unprivileged
        assert!(install("EvoriftTestSvc", "C:\\x.exe", &["--flag"], "Test", "desc").is_ok());
        assert!(uninstall("EvoriftTestSvc").is_ok());
    }

    /// Item 5.3: teardown order is dependency-safe (WinDivert after its consumers; proxy last; unique).
    #[test]
    fn teardown_order_dependency_safe() {
        let pos = |n: &str| TEARDOWN_ORDER.iter().position(|&x| x == n).unwrap();
        assert!(pos("WinDivert") > pos("zapret"), "WinDivert removed after zapret");
        assert!(pos("WinDivert") > pos("GoodbyeDPI"), "WinDivert removed after GoodbyeDPI");
        assert_eq!(*TEARDOWN_ORDER.last().unwrap(), "ProxiFyreService", "proxy/tunnel services last");
        let mut v: Vec<&str> = TEARDOWN_ORDER.to_vec();
        let n = v.len();
        v.sort_unstable();
        v.dedup();
        assert_eq!(v.len(), n, "no duplicate services in teardown order");
        for s in TEARDOWN_ORDER {
            assert!(MANAGED_SERVICES.contains(s), "{s} must be a managed service");
        }
    }
}
