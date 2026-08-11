//! Onarım & ağ-onar araçları (docs/05 §2-3).
//!
//! İKİ BAĞLAM:
//!  1. **Ağ onar** (`run_repair`) — flushdns/winsock/ipreset vb. AYRICALIKLI (servis tarafı, sys::run_os).
//!  2. **Discord/WebCord onarımı** — KULLANICI bağlamında çalışır (UI süreci): %APPDATA%/%LOCALAPPDATA%
//!     kullanıcı profiline aittir; LocalSystem servisi yanlış profili görür. Bu yüzden bu fonksiyonlar
//!     ayrıcalık GEREKTİRMEZ ve lib.rs Tauri komutlarından doğrudan çağrılır (docs/05 §2).

use crate::sys::run_os;

/// Ağ Onar araçları (docs/03 §9). Beyaz-liste ipc::validate() ile doğrulandı. Ayrıcalıklı (servis).
pub fn run_repair(tool: &str) -> Result<(), String> {
    match tool {
        "flushdns" => run_os("ipconfig", &["/flushdns"]),
        "registerdns" => run_os("ipconfig", &["/registerdns"]),
        "dnscache" => run_os("powershell", &["-NoProfile", "-Command", "Restart-Service Dnscache -Force"]),
        "renew" => run_os("ipconfig", &["/release"]).and_then(|_| run_os("ipconfig", &["/renew"])),
        "winsock" => run_os("netsh", &["winsock", "reset"]),
        "ipreset" => run_os("netsh", &["int", "ip", "reset"]),
        "adapter" => run_os(
            "powershell",
            &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='SilentlyContinue'; Get-NetAdapter -Physical | Where-Object { $_.Status -eq 'Up' } | Restart-NetAdapter -Confirm:$false",
            ],
        ),
        other => Err(format!("bilinmeyen onar aracı: {other}")),
    }
}

// ---------------------------------------------------------------------------
// Discord / WebCord onarımı — KULLANICI bağlamı (UI süreci). Ayrıcalık gerektirmez.
// ---------------------------------------------------------------------------

#[cfg(windows)]
fn hidden(program: &str) -> std::process::Command {
    use std::os::windows::process::CommandExt;
    let mut c = std::process::Command::new(program);
    c.creation_flags(crate::proc::CREATE_NO_WINDOW);
    c
}
#[cfg(not(windows))]
fn hidden(program: &str) -> std::process::Command {
    std::process::Command::new(program)
}

/// Discord kurulum yolunu bul (docs/05 §2 FindDiscordPath sırası):
///  1. çalışan Discord.exe sürecinin yolu (netinfo, native)
///  2. %LOCALAPPDATA%\Discord\app-*\Discord.exe (en yüksek sürüm)
///  3. %LOCALAPPDATA%\Discord\Discord.exe
pub fn find_discord_path() -> Option<String> {
    // 1) çalışan süreç
    let socks = crate::netinfo::sockets();
    for pid in crate::netinfo::socket_pids(&socks) {
        if let Some(p) = crate::netinfo::pid_exe_path(pid) {
            let base = std::path::Path::new(&p)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            if base == "discord.exe" {
                return Some(p);
            }
        }
    }
    // 2/3) %LOCALAPPDATA%\Discord
    let local = std::env::var("LOCALAPPDATA").ok()?;
    let root = std::path::Path::new(&local).join("Discord");
    // app-* alt klasörlerinde en yüksek sürümü seç
    if let Ok(rd) = std::fs::read_dir(&root) {
        let mut candidates: Vec<std::path::PathBuf> = rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|s| s.to_str())
                    .map(|n| n.starts_with("app-"))
                    .unwrap_or(false)
            })
            .collect();
        candidates.sort(); // app-1.0.x lexicographic ~ sürüm sırası
        if let Some(dir) = candidates.last() {
            let exe = dir.join("Discord.exe");
            if exe.exists() {
                return Some(exe.to_string_lossy().into_owned());
            }
        }
    }
    let flat = root.join("Discord.exe");
    if flat.exists() {
        return Some(flat.to_string_lossy().into_owned());
    }
    None
}

/// Discord'u onar (HAFİF): tüm Discord süreçlerini sonlandır + önbellek klasörlerini temizle
/// (%APPDATA%\discord\{Cache,Code Cache,GPUCache}) → "Checking for updates / Starting" takılmasını
/// çözer (docs/05 §2). Oturum KAPANMAZ (yalnız önbellek). Kullanıcı bağlamında çalışır.
pub fn repair_discord() -> Result<(), String> {
    crate::proc::kill_image("Discord.exe");
    crate::proc::kill_image("DiscordPTB.exe");
    crate::proc::kill_image("DiscordCanary.exe");
    let appdata = std::env::var("APPDATA").map_err(|_| "APPDATA bulunamadı".to_string())?;
    let base = std::path::Path::new(&appdata).join("discord");
    if !base.exists() {
        return Err("Discord kurulu görünmüyor (%APPDATA%\\discord yok)".into());
    }
    let mut cleared = 0;
    for sub in ["Cache", "Code Cache", "GPUCache", "Crashpad"] {
        let p = base.join(sub);
        if p.exists() && std::fs::remove_dir_all(&p).is_ok() {
            cleared += 1;
        }
    }
    crate::sys::audit(&format!("discord repair: {cleared} önbellek klasörü temizlendi"));
    Ok(())
}

/// WebCord kur (docs/05 §2): GitHub release zip'ini indir → %LOCALAPPDATA%\evorift\WebCord altına aç.
/// Discord alternatifi (resmi olmayan istemci, DPI takılmasına dayanıklı). Kullanıcı bağlamı.
pub fn install_webcord() -> Result<(), String> {
    const URL: &str =
        "https://github.com/SpacingBat3/WebCord/releases/download/v4.12.1/WebCord-win32-x64-4.12.1.zip";
    let local = std::env::var("LOCALAPPDATA").map_err(|_| "LOCALAPPDATA bulunamadı".to_string())?;
    let dest = std::path::Path::new(&local).join("evorift").join("WebCord");
    std::fs::create_dir_all(&dest).map_err(|e| format!("WebCord dizini oluşturulamadı: {e}"))?;
    let zip = std::env::temp_dir().join("evorift-webcord.zip");
    // İndir (PowerShell Invoke-WebRequest; HTTPS + resmi GitHub). Değerler $env ile (enjeksiyon yok).
    let script = "$ErrorActionPreference='Stop'; \
        Invoke-WebRequest -Uri $env:EVORIFT_WC_URL -OutFile $env:EVORIFT_WC_ZIP -UseBasicParsing; \
        Expand-Archive -Path $env:EVORIFT_WC_ZIP -DestinationPath $env:EVORIFT_WC_DEST -Force; \
        Remove-Item $env:EVORIFT_WC_ZIP -Force -ErrorAction SilentlyContinue";
    let out = hidden("powershell")
        .args(["-NoProfile", "-Command", script])
        .env("EVORIFT_WC_URL", URL)
        .env("EVORIFT_WC_ZIP", &zip)
        .env("EVORIFT_WC_DEST", &dest)
        .output()
        .map_err(|e| format!("WebCord indirilemedi: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

// ---------------------------------------------------------------------------
// Discord clean reinstall (item 9.2, docs/05 §2) — USER context. Heavy + destructive (re-downloads
// Discord) → user-initiated only. The official installer is silent by default.
// ---------------------------------------------------------------------------

/// Official Discord installer URL for a channel (docs/05 §2). `channel` ∈ stable | ptb | canary.
pub fn installer_url(channel: &str) -> String {
    let channel = normalize_channel(channel);
    format!(
        "https://discord.com/api/downloads/distributions/app/installers/latest?channel={channel}&platform=win&arch=x64"
    )
}

fn normalize_channel(channel: &str) -> &'static str {
    match channel.to_ascii_lowercase().as_str() {
        "ptb" => "ptb",
        "canary" => "canary",
        _ => "stable",
    }
}

/// Where the downloaded installer is written (temp).
fn installer_dest(channel: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("evorift-DiscordSetup-{}.exe", normalize_channel(channel)))
}

/// Clean reinstall of Discord (docs/05 §2): stop processes → clear cache → download the official installer
/// → run it. User context; returns once the installer is launched.
pub fn reinstall_discord(channel: &str) -> Result<(), String> {
    crate::proc::kill_image("Discord.exe");
    crate::proc::kill_image("DiscordPTB.exe");
    crate::proc::kill_image("DiscordCanary.exe");
    crate::proc::kill_image("Update.exe");
    let _ = repair_discord(); // best-effort cache clear

    let url = installer_url(channel);
    let dest = installer_dest(channel);
    let out = hidden("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "$ErrorActionPreference='Stop'; Invoke-WebRequest -Uri $env:EVORIFT_DISCORD_URL -OutFile $env:EVORIFT_DISCORD_DEST -UseBasicParsing",
        ])
        .env("EVORIFT_DISCORD_URL", &url)
        .env("EVORIFT_DISCORD_DEST", &dest)
        .output()
        .map_err(|e| format!("Discord installer download failed: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    hidden(&dest.to_string_lossy())
        .spawn()
        .map_err(|e| format!("Discord installer run failed: {e}"))?;
    crate::sys::audit(&format!("discord reinstall launched ({})", normalize_channel(channel)));
    Ok(())
}

// ---------------------------------------------------------------------------
// Generic app-proxy wizard (item 9.4) — user-facing wrapper around proxifyre::proxy_app
// ---------------------------------------------------------------------------

/// Attach any application to the ProxiFyre SOCKS5 proxy by its .exe name (docs/03 §2.3).
/// `exe_name` must be a bare filename (e.g. "MyGame.exe"), not a full path.
/// Validates input, then delegates to `proxifyre::proxy_app` using the default ByeDPI port 1080.
pub fn attach_app_to_proxy(exe_name: &str) -> Result<(), String> {
    let name = exe_name.trim();
    if name.is_empty() {
        return Err("exe name must not be empty".into());
    }
    // Reject path separators — only bare filenames are accepted (prevents config injection).
    if name.contains('/') || name.contains('\\') {
        return Err("exe name must be a bare filename, not a path".into());
    }
    crate::proxifyre::proxy_app(name, 1080)
}

// ---------------------------------------------------------------------------
// Discord PTB install (item 9.3, docs/05 §2) — direct PTB endpoint.
// Uses the short legacy API that always resolves to the latest PTB build.
// Complements reinstall_discord("ptb") which uses the distributions endpoint.
// ---------------------------------------------------------------------------

/// Direct PTB download URL (docs/05 §2 alternative endpoint).
pub fn ptb_url() -> String {
    "https://discord.com/api/download/ptb?platform=win".to_string()
}

/// Where the PTB installer is written (temp).
fn ptb_dest() -> std::path::PathBuf {
    std::env::temp_dir().join("evorift-DiscordPTBSetup.exe")
}

/// Install Discord PTB (docs/05 §2): stop PTB → download from the direct PTB endpoint → run it.
/// User context; returns once the installer is launched (installer runs its own UI).
pub fn install_discord_ptb() -> Result<(), String> {
    crate::proc::kill_image("DiscordPTB.exe");
    crate::proc::kill_image("Update.exe");

    let url = ptb_url();
    let dest = ptb_dest();
    let out = hidden("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "$ErrorActionPreference='Stop'; Invoke-WebRequest -Uri $env:EVORIFT_PTB_URL -OutFile $env:EVORIFT_PTB_DEST -UseBasicParsing",
        ])
        .env("EVORIFT_PTB_URL", &url)
        .env("EVORIFT_PTB_DEST", &dest)
        .output()
        .map_err(|e| format!("Discord PTB download failed: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    hidden(&dest.to_string_lossy())
        .spawn()
        .map_err(|e| format!("Discord PTB installer launch failed: {e}"))?;
    crate::sys::audit("discord ptb install launched");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Item 9.2: installer URL + dest resolve correctly (dry-run; no download).
    #[test]
    fn discord_installer_url_and_dest() {
        let u = installer_url("stable");
        assert!(u.starts_with("https://discord.com/api/downloads/distributions/app/installers/latest"));
        assert!(u.contains("channel=stable") && u.contains("platform=win") && u.contains("arch=x64"));
        assert!(installer_url("ptb").contains("channel=ptb"));
        assert!(installer_url("canary").contains("channel=canary"));
        assert!(installer_url("bogus").contains("channel=stable"), "unknown channel → stable");
        assert!(installer_dest("ptb").to_string_lossy().contains("DiscordSetup-ptb"));
    }

    /// Item 9.3: PTB-specific URL + dest path (dry-run; no download).
    #[test]
    fn discord_ptb_url_and_dest() {
        let u = ptb_url();
        assert!(u.starts_with("https://discord.com/api/download/ptb"), "must use PTB endpoint");
        assert!(u.contains("platform=win"), "must specify windows platform");
        let d = ptb_dest();
        let name = d.to_string_lossy();
        assert!(name.contains("DiscordPTBSetup"), "dest must be named for PTB");
        assert_eq!(d.extension().and_then(|e| e.to_str()), Some("exe"));
    }
}
