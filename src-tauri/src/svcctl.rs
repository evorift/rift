//! evorift-svc (LocalSystem) servis kontrolü — KULLANICI/UI bağlamından (elevated) çağrılır.
//!
//! MİMARİ (iki çalışma modu):
//!  - **SERVİSSİZ (varsayılan):** UI yönetici olarak çalışır → motoru (winws + WARP) KENDİ İÇİNDE
//!    (gömülü serve_blocking) çalıştırır. Servis kurmaya GEREK YOK. winws/WireGuard zaten ayrı
//!    binary'ler; tek gereken yönetici hakkı (WinDivert + installtunnelservice için).
//!  - **SERVİSLİ (ekstra koruma):** kullanıcı "açılışta otomatik koru / ben kapalıyken de çalış"
//!    isterse evorift-svc bir kez kurulur (boot'ta auto-start LocalSystem). Kalıcılık + UI'siz çalışma.
//!
//! Bu modül DPI/VPN motorlarını YAZMAZ — onlar bundled binary (winws=zapret, wireguard.exe). Yalnız
//! bizim ince orkestrasyon servisimizi (evorift-svc.exe) sc ile kurar/kaldırır. İnternetten indirme YOK.

/// Servis adı (bin/evorift-svc.rs SERVICE_NAME ile birebir).
pub const SERVICE_NAME: &str = "EvoriftSvc";

#[cfg(windows)]
fn hidden(program: &str) -> std::process::Command {
    use std::os::windows::process::CommandExt;
    let mut c = std::process::Command::new(program);
    c.creation_flags(crate::proc::CREATE_NO_WINDOW);
    c
}

/// evorift-svc.exe yolu: UI exe'sinin yanındaki kardeş binary (installer aynı dizine koyar).
#[cfg(windows)]
fn svc_exe() -> Option<std::path::PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.join("evorift-svc.exe"))
}

/// Servis durumu: "running" | "stopped" | "absent". Salt-okuma (yönetici gerekmez).
#[cfg(windows)]
pub fn status() -> &'static str {
    let out = hidden("sc").args(["query", SERVICE_NAME]).output();
    match out {
        Ok(o) => {
            let s = String::from_utf8_lossy(&o.stdout);
            if !s.contains("STATE") {
                "absent" // "servis yok" (1060) → STATE satırı dönmez
            } else if s.contains("RUNNING") {
                "running"
            } else {
                "stopped"
            }
        }
        Err(_) => "absent",
    }
}
#[cfg(not(windows))]
pub fn status() -> &'static str {
    "absent"
}

/// Servisi kur + başlat (boot'ta auto-start). YÖNETİCİ gerektirir (sc create). evorift-svc.exe bundle'da
/// olmalı. Zaten kuruluysa yalnız başlatır. Başarısızsa (yetki yok) hata döner → UI elevate önerir.
#[cfg(windows)]
pub fn install() -> Result<(), String> {
    let exe = svc_exe().filter(|p| p.exists()).ok_or_else(|| {
        "evorift-svc.exe bundle'da bulunamadı (kurulum eksik olabilir)".to_string()
    })?;
    let bin = exe.to_string_lossy().into_owned();
    // Zaten kurulu değilse oluştur (sc create). binPath= ve start= sonrası BOŞLUK sc.exe sözdizimi gereği.
    if status() == "absent" {
        let out = hidden("sc")
            .args(["create", SERVICE_NAME, "binPath=", &bin, "start=", "auto", "DisplayName=", "evorift Koruma Servisi"])
            .output()
            .map_err(|e| format!("sc create çalıştırılamadı: {e}"))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            let so = String::from_utf8_lossy(&out.stdout);
            return Err(format!("servis kurulamadı (yönetici gerekli?): {} {}", err.trim(), so.trim()));
        }
        // Servis açıklaması (best-effort, kozmetik).
        let _ = hidden("sc")
            .args(["description", SERVICE_NAME, "VPN'siz DPI atlatma — boot koruması (winws + WARP)."])
            .output();
        // Crash recovery, IDENTICAL to what the NSIS installer configures. Without this, a service
        // reinstalled from the app's own button would silently be less resilient than one installed
        // by the installer — the same name, the same binary, quietly different behaviour.
        let _ = hidden("sc")
            .args(["failure", SERVICE_NAME, "reset=", "86400", "actions=", "restart/5000/restart/5000/restart/5000"])
            .output();
    }
    // Başlat (idempotent: zaten RUNNING ise sc start hata verir ama önemsiz → durumu kontrol et).
    let _ = hidden("sc").args(["start", SERVICE_NAME]).output();

    // The SCM starts services asynchronously, so an immediate status read can legitimately still
    // say "stopped". This used to return Ok REGARDLESS — so a service that never came up produced
    // a success message and a UI that claimed a working backend. Poll briefly, then tell the truth.
    for _ in 0..10 {
        if status() == "running" {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    Err(format!(
        "service was created but did not start (state: {}). Check the Windows event log, or reinstall the app.",
        status()
    ))
}

/// Servisi durdur + kaldır (sc stop + delete). YÖNETİCİ gerektirir. Servissiz moda dönüş.
#[cfg(windows)]
pub fn uninstall() -> Result<(), String> {
    if status() == "absent" {
        return Ok(()); // zaten yok
    }
    let _ = hidden("sc").args(["stop", SERVICE_NAME]).output();
    // STOPPED olmasını kısa bekle (delete açık handle'da DELETE_PENDING yaratmasın).
    for _ in 0..6 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        if status() != "running" {
            break;
        }
    }
    let out = hidden("sc")
        .args(["delete", SERVICE_NAME])
        .output()
        .map_err(|e| format!("sc delete çalıştırılamadı: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(format!("servis kaldırılamadı (yönetici gerekli?): {}", err.trim()))
    }
}

#[cfg(not(windows))]
pub fn install() -> Result<(), String> {
    Err("servis yalnız Windows'ta".into())
}
#[cfg(not(windows))]
pub fn uninstall() -> Result<(), String> {
    Ok(())
}
