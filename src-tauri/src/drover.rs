//! drover adapter (docs/03 §3, docs/04 §6) — Discord-ONLY SOCKS5 routing via DLL hijacking.
//!
//! drover (hdrover) ships a `version.dll`. Dropped into a Discord `app-*` folder, Windows loads it
//! (DLL search-order hijack) when Discord.exe starts and routes Discord's traffic to the SOCKS5 proxy
//! in `drover.ini` — here, ByeDPI's `127.0.0.1:1080`. No ProxiFyre, no kernel driver; Discord only,
//! the lightest path. Runs in the USER context (%LOCALAPPDATA% is the user profile, not LocalSystem's),
//! so this is driven from the UI process (item 3.4 wires it); installs are tracked for rollback removal.
//!
//! Bundle: `<exe_dir>\drover\version.dll` (not yet shipped — absent → logged sim no-op).

use std::path::{Path, PathBuf};

/// `drover.ini` content pointing Discord at the ByeDPI SOCKS5 proxy (docs/03 §3.1).
pub fn drover_ini(socks_port: u16) -> String {
    format!("[drover]\r\nproxy = socks5://127.0.0.1:{socks_port}\r\n")
}

/// Bundled drover DLL: `<exe_dir>\drover\version.dll`.
fn bundle_dll() -> Option<PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.join("drover").join("version.dll"))
}

/// Find Discord `app-*` install dirs under `%LOCALAPPDATA%\{Discord,DiscordPTB,DiscordCanary}`.
pub fn discord_app_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(local) = std::env::var("LOCALAPPDATA") else {
        return out;
    };
    for variant in ["Discord", "DiscordPTB", "DiscordCanary"] {
        let root = Path::new(&local).join(variant);
        if let Ok(rd) = std::fs::read_dir(&root) {
            for e in rd.filter_map(|e| e.ok()) {
                let p = e.path();
                let is_app = p
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|n| n.starts_with("app-"))
                    .unwrap_or(false);
                if p.is_dir() && is_app {
                    out.push(p);
                }
            }
        }
    }
    out
}

/// Copy `src_dll` as `version.dll` + write `drover.ini` into each app dir. Returns the written file
/// paths (so the caller can record them in the rollback log). Propagates the first copy/write error.
pub fn install_to(app_dirs: &[PathBuf], src_dll: &Path, socks_port: u16) -> std::io::Result<Vec<PathBuf>> {
    let ini = drover_ini(socks_port);
    let mut written = Vec::new();
    for dir in app_dirs {
        let dll = dir.join("version.dll");
        std::fs::copy(src_dll, &dll)?;
        written.push(dll);
        let ini_path = dir.join("drover.ini");
        std::fs::write(&ini_path, &ini)?;
        written.push(ini_path);
    }
    Ok(written)
}

/// Resolve the bundled `version.dll` + Discord app dirs and install drover into all of them. Returns the
/// written files for the caller to record in the rollback log. Missing bundle / no Discord → empty Vec.
pub fn install(socks_port: u16) -> Result<Vec<PathBuf>, String> {
    let src = match bundle_dll() {
        Some(p) if p.exists() => p,
        _ => {
            eprintln!("[evorift][drover] bundle missing (version.dll) — sim (no DLL injected)");
            return Ok(Vec::new());
        }
    };
    let dirs = discord_app_dirs();
    if dirs.is_empty() {
        return Ok(Vec::new());
    }
    let written = install_to(&dirs, &src, socks_port).map_err(|e| format!("drover install failed: {e}"))?;
    // Track every copied file for clean removal (item 7.3). These are real user-context writes, so they
    // are recorded regardless of privilege (unlike the sc/netsh sites which only mutate when privileged).
    for f in &written {
        crate::rollback::record(crate::rollback::Change::FileCopied { path: f.to_string_lossy().into_owned() });
    }
    Ok(written)
}

/// Remove drover files from all Discord app dirs (uninstall). Only removes `version.dll` when our
/// `drover.ini` marker is present in the same dir (never touches a non-drover version.dll).
pub fn remove_all() {
    for dir in discord_app_dirs() {
        if dir.join("drover.ini").exists() {
            let _ = std::fs::remove_file(dir.join("version.dll"));
            let _ = std::fs::remove_file(dir.join("drover.ini"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rollback::{Change, RollbackLog};

    #[test]
    fn drover_ini_content() {
        let ini = drover_ini(1080);
        assert!(ini.contains("[drover]"));
        assert!(ini.contains("proxy = socks5://127.0.0.1:1080"));
    }

    /// Item 3.3: install_to copies version.dll + drover.ini into an app dir, and the rollback log
    /// (Change::FileCopied) cleanly removes them.
    #[test]
    fn install_to_copies_and_rollback_removes() {
        let base = std::env::temp_dir().join("evorift-test-drover");
        let app = base.join("app-1.0.0");
        std::fs::create_dir_all(&app).unwrap();
        let src = base.join("version.dll");
        std::fs::write(&src, b"FAKE-DLL").unwrap();

        let written = install_to(std::slice::from_ref(&app), &src, 1080).expect("install");
        assert_eq!(written.len(), 2);
        assert!(app.join("version.dll").exists());
        assert!(app.join("drover.ini").exists());

        let mut log = RollbackLog::new();
        for f in &written {
            log.record(Change::FileCopied { path: f.to_string_lossy().into_owned() });
        }
        let _ = log.rollback_all();
        assert!(!app.join("version.dll").exists(), "rollback removed the DLL");
        assert!(!app.join("drover.ini").exists(), "rollback removed the ini");

        let _ = std::fs::remove_dir_all(&base);
    }
}
