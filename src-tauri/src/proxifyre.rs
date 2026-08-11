//! ProxiFyre adapter (docs/03 §2.3-2.5, docs/04 §5) — per-app SOCKS5 routing for the ByeDPI path.
//!
//! ProxiFyre (wiresock/Vadim Smirnov) uses the Windows Packet Filter (NDIS) driver to capture the
//! sockets of named apps and funnel them into a SOCKS5 proxy — here, ByeDPI's `127.0.0.1:1080`. This
//! gives a split tunnel: only the listed apps are desynced, the rest go direct. Bundle:
//! `<exe_dir>\proxifyre\ProxiFyre.exe` (+ generated `app-config.json`). Absent → logged sim no-op.
//!
//! Privileged (service install + firewall) → sys::run_os (sim when unprivileged). Config generation is
//! pure + testable.

use serde::Serialize;

/// ProxiFyre Windows service name (docs/02 §4 managed-services list).
pub const SERVICE_NAME: &str = "ProxiFyreService";

/// Default non-browser apps routed through the proxy (docs/03 §2.3 appNames).
const CORE_APPS: &[&str] = &[
    "discord", "Discord.exe", "DiscordPTB.exe", "Update.exe", "webcord",
    "roblox", "RobloxPlayerBeta.exe", "RobloxPlayerInstaller.exe",
];

/// Browser executables added when the "tunnel browsers too" toggle is on (docs/03 §1.2/2.3).
const BROWSER_APPS: &[&str] = &[
    "chrome.exe", "firefox.exe", "opera.exe", "operagx.exe", "brave.exe", "vivaldi.exe",
    "msedge.exe", "zen.exe", "chromium.exe", "iexplore.exe", "librewolf.exe",
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProxyEntry {
    app_names: Vec<String>,
    socks5_proxy_endpoint: String,
    supported_protocols: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppConfig {
    log_level: String,
    proxies: Vec<ProxyEntry>,
}

/// Build the ProxiFyre `app-config.json` (docs/03 §2.3). `browsers` toggles the browser app names;
/// `socks_port` is the ByeDPI SOCKS5 port (default 1080).
pub fn app_config_json(browsers: bool, socks_port: u16) -> String {
    let mut app_names: Vec<String> = CORE_APPS.iter().map(|s| s.to_string()).collect();
    if browsers {
        app_names.extend(BROWSER_APPS.iter().map(|s| s.to_string()));
    }
    let cfg = AppConfig {
        log_level: "Info".into(),
        proxies: vec![ProxyEntry {
            app_names,
            socks5_proxy_endpoint: format!("127.0.0.1:{socks_port}"),
            supported_protocols: vec!["TCP".into(), "UDP".into()],
        }],
    };
    serde_json::to_string_pretty(&cfg).unwrap_or_else(|_| "{}".into())
}

/// ProxiFyre bundle directory: `<exe_dir>\proxifyre\`.
fn proxifyre_dir() -> Option<std::path::PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.join("proxifyre"))
}

/// Write `app-config.json` into `dir`; returns its path.
pub fn write_config_to(dir: &std::path::Path, browsers: bool, socks_port: u16) -> std::io::Result<std::path::PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join("app-config.json");
    std::fs::write(&path, app_config_json(browsers, socks_port))?;
    Ok(path)
}

/// Allow-rule a program through the firewall (idempotent). Best-effort.
fn allow_firewall(name: &str, program: &str) {
    let rule = format!("name={name}");
    let prog = format!("program={program}");
    let _ = crate::sys::run_os("netsh", &["advfirewall", "firewall", "delete", "rule", &rule]);
    let _ = crate::sys::run_os(
        "netsh",
        &["advfirewall", "firewall", "add", "rule", &rule, "dir=in", "action=allow", &prog, "enable=yes"],
    );
    let _ = crate::sys::run_os(
        "netsh",
        &["advfirewall", "firewall", "add", "rule", &rule, "dir=out", "action=allow", &prog, "enable=yes"],
    );
}

// ---------------------------------------------------------------------------
// Generic app-proxy wizard (item 9.4) — attach any .exe to ProxiFyre's SOCKS5
// ---------------------------------------------------------------------------

/// Return the updated app-config JSON with `exe_name` added to the first proxy entry's appNames.
/// Reads the existing config from `dir/app-config.json` when present (preserving all other apps
/// and settings); falls back to a fresh default config when the file is absent or unparseable.
/// Idempotent: if `exe_name` is already listed the JSON is returned unchanged.
pub fn add_app_to_config(dir: &std::path::Path, exe_name: &str, socks_port: u16) -> String {
    let path = dir.join("app-config.json");
    let mut val: serde_json::Value = path
        .exists()
        .then(|| std::fs::read_to_string(&path).ok())
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::from_str(&app_config_json(false, socks_port)).unwrap());

    // Insert into the first proxy entry's appNames, dedup.
    if let Some(arr) = val
        .get_mut("proxies")
        .and_then(|p| p.as_array_mut())
        .and_then(|v| v.first_mut())
        .and_then(|e| e.get_mut("appNames"))
        .and_then(|a| a.as_array_mut())
    {
        if !arr.iter().any(|n| n.as_str() == Some(exe_name)) {
            arr.push(serde_json::Value::String(exe_name.to_string()));
        }
    }
    serde_json::to_string_pretty(&val).unwrap_or_else(|_| "{}".into())
}

/// Write the updated config (with `exe_name` appended) into `dir/app-config.json`.
pub fn write_app_config_with_app(
    dir: &std::path::Path,
    exe_name: &str,
    socks_port: u16,
) -> std::io::Result<std::path::PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join("app-config.json");
    std::fs::write(&path, add_app_to_config(dir, exe_name, socks_port))?;
    Ok(path)
}

/// Wizard: attach any app (by .exe name) to ProxiFyre's SOCKS5 proxy (docs/03 §2.3).
/// Updates `app-config.json` in the bundle dir, then restarts the ProxiFyre service so the
/// change is live immediately. If the service isn't running the restart no-ops silently.
/// Privileged (sc stop/start). Absent bundle dir → sim no-op.
pub fn proxy_app(exe_name: &str, socks_port: u16) -> Result<(), String> {
    if exe_name.is_empty() {
        return Err("exe_name must not be empty".into());
    }
    let dir = proxifyre_dir().ok_or("proxifyre bundle dir unresolved")?;
    write_app_config_with_app(&dir, exe_name, socks_port)
        .map_err(|e| format!("app-config.json update failed: {e}"))?;
    // Restart so the running service sees the new appNames (best-effort; may not be running).
    let _ = crate::sys::run_os("sc", &["stop", SERVICE_NAME]);
    let _ = crate::sys::run_os("net", &["start", SERVICE_NAME]);
    crate::sys::audit(&format!("proxifyre: added {exe_name} to proxy config (port {socks_port})"));
    Ok(())
}

/// Install ProxiFyre as an auto-start service routing the listed apps into ByeDPI's SOCKS5 (docs/03 §2.3).
/// Writes `app-config.json` next to ProxiFyre.exe, runs `ProxiFyre.exe install` (cwd = bundle so it finds
/// the config), sets the service auto-start, starts it, and opens firewall rules for ProxiFyre + ciadpi.
/// Privileged; absent bundle → logged sim no-op (boot never broken). Records nothing here — the caller
/// (orchestrator) records the service + firewall rules in the rollback log (item 4.x).
pub fn install(browsers: bool, socks_port: u16) -> Result<(), String> {
    let dir = proxifyre_dir().ok_or("proxifyre bundle dir unresolved")?;
    let exe = dir.join("ProxiFyre.exe");
    if !exe.exists() {
        eprintln!("[evorift][proxifyre] bundle missing (ProxiFyre.exe) — sim (no real routing)");
        return Ok(());
    }
    write_config_to(&dir, browsers, socks_port).map_err(|e| format!("app-config.json write failed: {e}"))?;
    crate::sys::run_os_cwd(&exe.to_string_lossy(), &["install"], &dir)?;
    crate::sys::run_os("sc", &["config", SERVICE_NAME, "start=", "auto"])?;
    crate::sys::run_os("net", &["start", SERVICE_NAME])?;
    allow_firewall("evorift-proxifyre", &exe.to_string_lossy());
    if let Some(cia) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("byedpi").join("ciadpi.exe"))) {
        allow_firewall("evorift-ciadpi", &cia.to_string_lossy());
    }
    // Track for rollback (item 7.3) — only when privileged (these are real sc/netsh mutations).
    if crate::sys::privileged() {
        crate::rollback::record(crate::rollback::Change::ServiceCreated { name: SERVICE_NAME.to_string() });
        crate::rollback::record(crate::rollback::Change::FirewallRule { name: "evorift-proxifyre".into() });
        crate::rollback::record(crate::rollback::Change::FirewallRule { name: "evorift-ciadpi".into() });
    }
    Ok(())
}

/// Stop + delete the ProxiFyre service and remove the firewall rules (best-effort).
pub fn uninstall() -> Result<(), String> {
    let _ = crate::sys::run_os("sc", &["stop", SERVICE_NAME]);
    let _ = crate::sys::run_os("sc", &["delete", SERVICE_NAME]);
    let _ = crate::sys::run_os("netsh", &["advfirewall", "firewall", "delete", "rule", "name=evorift-proxifyre"]);
    let _ = crate::sys::run_os("netsh", &["advfirewall", "firewall", "delete", "rule", "name=evorift-ciadpi"]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Item 3.2: config has the SOCKS5 endpoint, TCP+UDP, core apps; valid JSON.
    #[test]
    fn config_json_core() {
        let j = app_config_json(false, 1080);
        assert!(j.contains("\"socks5ProxyEndpoint\""));
        assert!(j.contains("127.0.0.1:1080"));
        assert!(j.contains("\"supportedProtocols\""));
        assert!(j.contains("Discord.exe"));
        assert!(!j.contains("chrome.exe"), "browsers off → no chrome");
        let v: serde_json::Value = serde_json::from_str(&j).expect("valid JSON");
        assert!(v["proxies"].is_array());
        assert_eq!(v["proxies"][0]["socks5ProxyEndpoint"], "127.0.0.1:1080");
    }

    #[test]
    fn browser_toggle_and_custom_port() {
        let on = app_config_json(true, 1090);
        assert!(on.contains("chrome.exe") && on.contains("firefox.exe"));
        assert!(on.contains("127.0.0.1:1090"));
    }

    #[test]
    fn write_config_writes_file() {
        let dir = std::env::temp_dir().join("evorift-test-pf");
        let p = write_config_to(&dir, false, 1080).expect("write config");
        assert!(p.exists());
        let body = std::fs::read_to_string(&p).unwrap();
        assert!(body.contains("socks5ProxyEndpoint"));
        let _ = std::fs::remove_file(&p);
    }

    /// Item 9.4: add_app_to_config inserts the exe into appNames; idempotent on second call.
    #[test]
    fn add_app_to_config_and_idempotent() {
        let dir = std::env::temp_dir().join("evorift-test-pf-wizard");
        // Ensure a clean slate (no pre-existing app-config.json).
        let _ = std::fs::remove_file(dir.join("app-config.json"));
        let _ = std::fs::create_dir_all(&dir);

        // First call: fresh config (no file yet) — app is added.
        let json1 = add_app_to_config(&dir, "MyGame.exe", 1080);
        let v1: serde_json::Value = serde_json::from_str(&json1).expect("valid JSON");
        let names: Vec<&str> = v1["proxies"][0]["appNames"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|n| n.as_str())
            .collect();
        assert!(names.contains(&"MyGame.exe"), "app must be in appNames");
        // Core apps preserved.
        assert!(names.contains(&"Discord.exe"));

        // Write the config, then add again — idempotent (no duplicate).
        let path = dir.join("app-config.json");
        std::fs::write(&path, &json1).unwrap();
        let json2 = add_app_to_config(&dir, "MyGame.exe", 1080);
        let v2: serde_json::Value = serde_json::from_str(&json2).unwrap();
        let count = v2["proxies"][0]["appNames"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|n| n.as_str() == Some("MyGame.exe"))
            .count();
        assert_eq!(count, 1, "duplicate must not be inserted");

        // write_app_config_with_app writes the file and the app appears in it.
        let written = write_app_config_with_app(&dir, "AnotherApp.exe", 1080).expect("write");
        let body = std::fs::read_to_string(&written).unwrap();
        assert!(body.contains("AnotherApp.exe"), "written file must include AnotherApp.exe");

        // Cleanup.
        let _ = std::fs::remove_file(&written);
    }
}
