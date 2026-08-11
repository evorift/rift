//! WireSock adapter (docs/03 §1.3, docs/04 §8) — user-mode WireGuard split-tunnel by app name.
//!
//! WireSock runs WireGuard WITHOUT a kernel driver and filters by application (`AllowedApps`), so only
//! the listed apps ride the WARP tunnel — an alternative to evorift's default IP-based split (official
//! `wireguard.exe`). The config is derived from the same wgcf profile (item 4.2) via the app-based builder
//! (item 4.3). Bundle: `<exe_dir>\wiresock\wiresock-client.exe`. Absent → logged sim no-op (boot safe).
//!
//! Privileged service install → `sys` (sim when unprivileged). Config generation is pure + testable.

/// WireSock Windows service name (docs/02 §4 managed-services list).
pub const SERVICE_NAME: &str = "wiresock-client-service";

/// Build the WireSock app-based tunnel conf from a wgcf profile (item 4.4). Pure → testable.
/// Full `AllowedIPs` + `AllowedApps = <names>` (WireSock routes by app); optional MTU clamp.
pub fn build_conf(profile: &str, apps: &[String], mtu: u32) -> String {
    let mut conf = crate::warp::app_tunnel_config(profile, apps, crate::warp::WARP_ENDPOINT);
    if mtu > 0 {
        conf = crate::warp::set_mtu(&conf, mtu);
    }
    conf
}

fn bundle_dir() -> Option<std::path::PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.join("wiresock"))
}

fn client_exe() -> Option<std::path::PathBuf> {
    Some(bundle_dir()?.join("wiresock-client.exe"))
}

/// WireSock conf path: `%PROGRAMDATA%\evorift\wiresock.conf`.
fn conf_path() -> std::path::PathBuf {
    crate::ipc::data_dir().join("wiresock.conf")
}

/// Is the WireSock client bundled?
pub fn is_available() -> bool {
    client_exe().map(|e| e.exists()).unwrap_or(false)
}

/// Generate `wiresock.conf` from the cached wgcf profile (item 4.2) + the app list. Errors if no wgcf
/// profile exists yet (WARP must be set up first). Hardens the conf ACL (carries the private key).
pub fn write_config(apps: &[String], mtu: u32) -> Result<std::path::PathBuf, String> {
    let profile = std::fs::read_to_string(crate::warp::profile_path())
        .map_err(|_| "no wgcf profile yet — set up WARP first (wgcf register/generate)".to_string())?;
    let conf = build_conf(&profile, apps, mtu);
    let path = conf_path();
    let _ = std::fs::create_dir_all(crate::ipc::data_dir());
    std::fs::write(&path, conf).map_err(|e| format!("wiresock.conf write failed: {e}"))?;
    // Harden the conf ACL — it holds the private key (SID-based, locale-independent; best-effort).
    let _ = crate::sys::run_os(
        "icacls",
        &[&path.to_string_lossy(), "/inheritance:r", "/grant:r", "*S-1-5-18:(F)", "*S-1-5-32-544:(F)"],
    );
    Ok(path)
}

/// Install WireSock as an auto-start (`-start-type 2`) user-mode split-tunnel for `apps` (docs/03 §1.3):
/// `wiresock-client.exe install -start-type 2 -config <conf> -log-level none` + `net start`. Privileged;
/// absent bundle → logged sim no-op (boot never broken).
pub fn install(apps: &[String], mtu: u32) -> Result<(), String> {
    let exe = match client_exe() {
        Some(e) if e.exists() => e,
        _ => {
            eprintln!("[evorift][wiresock] bundle missing (wiresock-client.exe) — sim (no real tunnel)");
            return Ok(());
        }
    };
    let conf = write_config(apps, mtu)?;
    crate::sys::run_os(
        &exe.to_string_lossy(),
        &["install", "-start-type", "2", "-config", &conf.to_string_lossy(), "-log-level", "none"],
    )?;
    crate::sys::run_os("net", &["start", SERVICE_NAME])?;
    if crate::sys::privileged() {
        crate::rollback::record(crate::rollback::Change::ServiceCreated { name: SERVICE_NAME.to_string() });
    }
    Ok(())
}

/// Uninstall the WireSock service (best-effort).
pub fn uninstall() -> Result<(), String> {
    if let Some(exe) = client_exe() {
        if exe.exists() {
            let _ = crate::sys::run_os(&exe.to_string_lossy(), &["uninstall"]);
        }
    }
    let _ = crate::sys::run_os("sc", &["stop", SERVICE_NAME]);
    let _ = crate::sys::run_os("sc", &["delete", SERVICE_NAME]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "[Interface]\nPrivateKey = AAAA\nAddress = 172.16.0.2/32\nDNS = 1.1.1.1\nMTU = 1280\n\n[Peer]\nPublicKey = K\nAllowedIPs = 0.0.0.0/0\nEndpoint = engage.cloudflareclient.com:2408\n";

    /// Item 4.4: the generated conf carries AllowedApps + the clamped MTU + the loop-safe endpoint, and
    /// drops tunnel DNS (split discipline).
    #[test]
    fn build_conf_has_apps_mtu_endpoint() {
        let apps = vec!["Discord.exe".to_string(), "roblox".to_string()];
        let c = build_conf(SAMPLE, &apps, 1200);
        assert!(c.contains("AllowedApps = Discord.exe, roblox"));
        assert!(c.contains("MTU = 1200") && !c.contains("MTU = 1280"));
        assert!(c.contains(&format!("Endpoint = {}", crate::warp::WARP_ENDPOINT)));
        assert!(!c.contains("DNS ="), "split tunnel: tunnel DNS dropped");
        assert_eq!(SERVICE_NAME, "wiresock-client-service");
    }

    /// install/uninstall are clean no-ops in dev (no bundle, unprivileged).
    #[test]
    fn install_uninstall_sim() {
        assert!(install(&["Discord.exe".to_string()], 1280).is_ok());
        assert!(uninstall().is_ok());
    }
}
