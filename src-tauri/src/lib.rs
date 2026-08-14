pub mod autopilot;
pub mod byedpi;
pub mod client;
pub mod dns;
pub mod drover;
pub mod engine;
pub mod firewall;
pub mod goodbyedpi;
pub mod ipc;
pub mod limit;
pub mod logbundle;
pub mod manifest;
pub mod netinfo;
pub mod pid_scan;
pub mod preflight;
pub mod proc;
pub mod profile;
pub mod proxifyre;
pub mod repair;
pub mod rollback;
pub mod schtask;
pub mod service;
pub mod services;
pub mod svcctl;
pub mod sys;
/// Remote test agent (`evorift-testd` binary only — the app never references it).
pub mod testd;
pub mod tweak;
pub mod verify;
pub mod warp;
pub mod wiresock;

use ipc::{Command, EngineStatus, Response};
use serde::Serialize;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, WindowEvent};

/// Windows'ta bir alt süreci konsol penceresi AÇMADAN çalıştır (CREATE_NO_WINDOW).
/// tasklist/reg/powershell çağrıları aksi halde her seferinde siyah pencere çaktırır.
#[cfg(windows)]
fn hidden_command(program: &str) -> std::process::Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut c = std::process::Command::new(program);
    c.creation_flags(CREATE_NO_WINDOW);
    c
}
#[cfg(not(windows))]
fn hidden_command(program: &str) -> std::process::Command {
    std::process::Command::new(program)
}

/// Hızlı bağlantı tanılaması (yetkisiz, UI sürecinde): DNS çözümleme + TCP erişim + gecikme.
/// Onboarding testi ve sağlık göstergesi için. Gerçek değerler (servise gerek yok).
#[derive(Serialize, Default)]
struct Diag {
    dns_ok: bool,
    reachable: bool,
    ms: u32,
}

/// DNS sızıntı kontrolü (yetkisiz, salt-okuma): sistemin kullandığı IPv4 DNS sunucuları +
/// bunlar bilinen güvenli bir sağlayıcıya mı ait. `secure=false` → ISS DNS (sızıntı).
#[derive(Serialize, Default)]
struct DnsStatus {
    servers: Vec<String>,
    secure: bool,
    provider: String,
}

/// PowerShell taraması bloke eder → ayrı thread'de (UI komut thread'ini dondurmasın, bkz. list_apps).
#[tauri::command]
async fn dns_status() -> DnsStatus {
    tauri::async_runtime::spawn_blocking(dns_status_blocking)
        .await
        .unwrap_or_default()
}

fn dns_status_blocking() -> DnsStatus {
    let out = hidden_command("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "(Get-DnsClientServerAddress | Where-Object {$_.ServerAddresses} | Select-Object -ExpandProperty ServerAddresses) -join ','",
        ])
        .output();

    let mut servers: Vec<String> = Vec::new();
    if let Ok(o) = out {
        let s = String::from_utf8_lossy(&o.stdout);
        for ip in s.trim().split(',') {
            let ip = ip.trim().to_string();
            if !ip.is_empty() && !servers.contains(&ip) {
                servers.push(ip);
            }
        }
    }

    // IPv4 + IPv6 — IPv6 DNS sızıntısını da yakala (Faz P2.9). Get-DnsClientServerAddress (aile filtresiz) ikisini de verir.
    let known: &[(&str, &[&str])] = &[
        ("Cloudflare", &["1.1.1.1", "1.0.0.1", "2606:4700:4700::1111", "2606:4700:4700::1001"]),
        ("Quad9", &["9.9.9.9", "149.112.112.112", "2620:fe::fe", "2620:fe::9"]),
        ("AdGuard", &["94.140.14.14", "94.140.15.15", "2a10:50c0::ad1:ff", "2a10:50c0::ad2:ff"]),
        ("Google", &["8.8.8.8", "8.8.4.4", "2001:4860:4860::8888", "2001:4860:4860::8844"]),
    ];
    let provider = known
        .iter()
        .find(|(_, ips)| servers.iter().any(|s| ips.contains(&s.as_str())))
        .map(|(name, _)| name.to_string())
        .unwrap_or_default();

    DnsStatus {
        secure: !provider.is_empty(),
        provider,
        servers,
    }
}

/// DNS çözümleme + 3sn TCP-timeout içerir → ayrı thread'de (aksi halde UI 3sn'ye kadar donar).
#[tauri::command]
async fn connectivity_test() -> Diag {
    tauri::async_runtime::spawn_blocking(connectivity_test_blocking)
        .await
        .unwrap_or_default()
}

fn connectivity_test_blocking() -> Diag {
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::{Duration, Instant};

    // DNS çözümleme testi (bilinen bir alan adı)
    let dns_ok = "discord.com:443"
        .to_socket_addrs()
        .map(|mut a| a.next().is_some())
        .unwrap_or(false);

    // Cloudflare 1.1.1.1:443'e TCP erişim + gecikme
    let mut reachable = false;
    let mut ms = 0u32;
    if let Ok(addr) = "1.1.1.1:443".parse() {
        let t = Instant::now();
        reachable = TcpStream::connect_timeout(&addr, Duration::from_secs(3)).is_ok();
        ms = t.elapsed().as_millis() as u32;
    }

    Diag { dns_ok, reachable, ms }
}

/// Tespit edilen ağ-kullanan uygulama (gerçek süreç enumerasyonu, Faz P1.3).
#[derive(Serialize)]
struct DetectedApp {
    id: String,
    name: String,
    exe: String,
    path: String,
    kind: String,
}

/// Ağ kullanan çalışan uygulamaları listele (aktif TCP / UDP soketi sahibi PID'ler → exe yolu + ad).
/// NATIVE (alt-süreç YOK): netinfo soketleri → benzersiz PID'ler → pid_exe_path; ad exe dosya adından
/// türetilir. Yetkisiz salt-okuma; yolu alınamayan PID'ler (System/korumalı) zarifçe atlanır.
#[tauri::command]
async fn list_apps() -> Vec<DetectedApp> {
    // UI komut thread'ini bloke etmemek için native taramayı ayrı thread'de çalıştır (review LOW-7).
    tauri::async_runtime::spawn_blocking(list_apps_blocking)
        .await
        .unwrap_or_default()
}

fn list_apps_blocking() -> Vec<DetectedApp> {
    let socks = netinfo::sockets();
    let mut apps: Vec<DetectedApp> = Vec::new();
    let mut seen_exe: std::collections::HashSet<String> = std::collections::HashSet::new();
    for pid in netinfo::socket_pids(&socks) {
        let Some(path) = netinfo::pid_exe_path(pid) else {
            continue; // yolu alınamadı (System/korumalı/yarış) → atla
        };
        // exe = yolun son bileşeni (dosya adı)
        let exe = std::path::Path::new(&path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if exe.is_empty() {
            continue;
        }
        // exe'ye göre tekilleştir (aynı uygulamanın çok PID'i tek satır olsun)
        let exe_lower = exe.to_lowercase();
        if !seen_exe.insert(exe_lower.clone()) {
            continue;
        }
        // id, QoS politika adı + netsh kural adı olarak kullanılır → güvenli slug'a indirge
        // (boşluk/Türkçe/sembol → '-'); gerçek eşleştirme `path` ile yapıldığından ad serbest.
        let id = exe_lower
            .trim_end_matches(".exe")
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
            .collect::<String>();
        // Görünen ad: exe dosya adından ".exe" sıyrılmış hali (örn "Discord.exe" → "Discord"). Eski
        // PowerShell yolu $p.Description okuyordu; native API'de ucuz/güvenilir değil → exe adı yeterli
        // (UI zaten ikon + kullanıcı tercihi gösterir; çekirdek uygulamalar varsayılan listeden adlanır).
        let name = exe.trim_end_matches(".exe").trim_end_matches(".EXE").to_string();
        let name = if name.is_empty() { exe.clone() } else { name };
        apps.push(DetectedApp {
            id,
            name,
            exe,
            path,
            kind: String::new(),
        });
    }
    apps
}

/// Uygulama exe'lerinden ikon çıkar (32×32 PNG, base64). Tek PowerShell çağrısında toplu işler.
/// Frontend açılışta çağırır → AppRow.icon alanına yazar → harf yerine gerçek ikon gösterilir.
#[tauri::command]
async fn get_app_icons(paths: Vec<String>) -> std::collections::HashMap<String, String> {
    tauri::async_runtime::spawn_blocking(move || get_app_icons_blocking(&paths))
        .await
        .unwrap_or_default()
}

fn get_app_icons_blocking(paths: &[String]) -> std::collections::HashMap<String, String> {
    if paths.is_empty() {
        return std::collections::HashMap::new();
    }
    // PowerShell'e yolları | ile ayırarak gönder; her biri için 32×32 PNG base64 döndür.
    let script = r#"Add-Type -AssemblyName System.Drawing
$paths = $env:EVORIFT_ICON_PATHS -split '\|'
$result = @{}
foreach($p in $paths){
  if($p -and (Test-Path $p -PathType Leaf)){
    try {
      $icon = [System.Drawing.Icon]::ExtractAssociatedIcon($p)
      if($icon){
        $bmp = $icon.ToBitmap()
        $thumb = New-Object System.Drawing.Bitmap(32, 32)
        $g = [System.Drawing.Graphics]::FromImage($thumb)
        $g.InterpolationMode = 'HighQualityBicubic'
        $g.DrawImage($bmp, 0, 0, 32, 32)
        $g.Dispose()
        $ms = New-Object System.IO.MemoryStream
        $thumb.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
        $result[$p] = [Convert]::ToBase64String($ms.ToArray())
        $ms.Dispose(); $thumb.Dispose(); $bmp.Dispose(); $icon.Dispose()
      }
    } catch {}
  }
}
$result | ConvertTo-Json -Compress -Depth 2"#;

    let paths_str = paths.join("|");
    let out = hidden_command("powershell")
        .args(["-NoProfile", "-Command", script])
        .env("EVORIFT_ICON_PATHS", &paths_str)
        .output();

    let mut map = std::collections::HashMap::new();
    if let Ok(o) = out {
        let s = String::from_utf8_lossy(&o.stdout);
        let s = s.trim_start_matches('\u{feff}').trim();
        if !s.is_empty() && s != "null" {
            if let Ok(parsed) = serde_json::from_str::<std::collections::HashMap<String, String>>(s) {
                map = parsed;
            }
        }
    }
    map
}

/// Bir uygulamanın ULAŞTIĞI domain'leri tespit et (Apps → otomatik bypass).
///
/// NATIVE (alt-süreç YOK): exe'nin çalışan PID'lerinin kurulu (Established) TCP uzak IP'lerini netinfo
/// ile al → IP'leri önbellekli ters-DNS (GetNameInfoW) ile en-iyi-çaba hostname'e çevir.
///
/// ÖNEMLİ GERÇEK (V0.1.3 plan §b.5/b.6): tarayıcılar kendi DoH stub resolver'larını kullanır → domain'leri
/// OS DNS önbelleğine HİÇ girmez; ters-DNS de CDN-arkası servisler (Discord/YouTube/Roblox = Cloudflare/
/// Google) için kullanışsız `*.1e100.net` PTR'leri döndürür. Bu yüzden bu yol BEST-EFFORT'tur ve sık sık
/// boş döner — gerçek bypass sabit hostlist + IP-aralığı eşleştirmesiyle yapılır (winws/WARP). Eski
/// PowerShell yolu (Get-DnsClientCache + Resolve-DnsName, uygulama başına bir process) KALDIRILDI →
/// süreç-yığılması/CPU bug'ının birincil kaynağıydı.
#[tauri::command]
async fn detect_app_domains(exe: String) -> Vec<String> {
    tauri::async_runtime::spawn_blocking(move || detect_domains_blocking(&exe))
        .await
        .unwrap_or_default()
}

fn detect_domains_blocking(exe: &str) -> Vec<String> {
    let exe = exe.trim().to_lowercase();
    if exe.is_empty() {
        return Vec::new();
    }

    // 1) exe adı eşleşen çalışan PID'leri bul (native; OpenProcess yolun son bileşeni ile eşleştir).
    let socks = netinfo::sockets();
    let mut target_pids: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for pid in netinfo::socket_pids(&socks) {
        if let Some(path) = netinfo::pid_exe_path(pid) {
            let base = std::path::Path::new(&path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            if base == exe {
                target_pids.insert(pid);
            }
        }
    }
    if target_pids.is_empty() {
        return Vec::new();
    }

    // 2) O PID'lerin kurulu TCP uzak IP'lerini topla (public IP'ler; özel/loopback/link-local atla).
    let mut ips: Vec<std::net::IpAddr> = Vec::new();
    for s in &socks {
        if !s.tcp || !target_pids.contains(&s.pid) {
            continue;
        }
        if let Some(ip) = s.remote {
            if is_public_ip(&ip) && !ips.contains(&ip) {
                ips.push(ip);
            }
        }
    }
    if ips.is_empty() {
        return Vec::new();
    }

    // 3) En-iyi-çaba ters-DNS (önbellekli, ilk birkaç IP). CDN PTR'leri çoğu kez kullanışsız → boş kalabilir.
    let mut domains = Vec::new();
    for ip in ips.iter().take(6) {
        if let Some(host) = netinfo::reverse_dns(ip) {
            let d = host.trim().trim_end_matches('.').to_lowercase();
            if is_valid_domain(&d) && !domains.contains(&d) {
                domains.push(d);
            }
        }
    }
    domains
}

/// Genel (public) IP mi? Özel/loopback/link-local/belirsiz aralıkları ele (otomatik bypass'a girmesin).
fn is_public_ip(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            !(v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || v4.octets()[0] == 0)
        }
        std::net::IpAddr::V6(v6) => {
            !(v6.is_loopback() || v6.is_unspecified() || v6.is_multicast()
                // fe80::/10 link-local + fc00::/7 unique-local (is_unique_local stabil değil → bit testi)
                || (v6.segments()[0] & 0xffc0) == 0xfe80
                || (v6.segments()[0] & 0xfe00) == 0xfc00)
        }
    }
}

/// Alan-adı sağlık kontrolü (ipc::validate hostlist beyaz-listesiyle uyumlu: [a-z0-9.-], nokta var).
fn is_valid_domain(d: &str) -> bool {
    !d.is_empty()
        && d.len() <= 253
        && d.contains('.')
        && d.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
}

// UI Tauri komutları → ayrıcalıklı servise pipe IPC ile proxy (docs/05 §1-2).
// Servis ulaşılamazsa UI bloke olmasın diye nazik yedek değer döner.

// ⚠️ Bu komutlar SENKRON pipe IPC + servis tarafında bloke-eden PowerShell/netsh çalıştırır.
// Tauri'de SENKRON komut UI thread'inde koşar → uzun komut pencereyi DONDURUR (Oyun Modu 8+ komut
// gönderince ~5sn donma buradan geliyordu). Hepsini `async fn` + `spawn_blocking` ile thread havuzuna
// taşı → UI asla bloke olmaz (list_apps/detect_app_domains'in zaten kullandığı desen).

/// IPC komutunu bloke-eden thread havuzunda çalıştırıp `EngineStatus` döndüren async sarmalayıcı.
/// IPC/görev hatası olduğunda `running: true` gibi sahte bir başarı ASLA üretilmez — hata olduğu
/// gibi çağırana iletilir (bkz. state.svelte.ts toggle() — dönen `running` alanına göre karar verir).
async fn status_cmd(cmd: Command) -> Result<EngineStatus, String> {
    tauri::async_runtime::spawn_blocking(move || client::command_status(cmd))
        .await
        .map_err(|e| format!("görev hatası: {e}"))?
}

/// IPC komutunu bloke-eden thread havuzunda çalıştırıp `Ok(())`/`Err` döndüren async sarmalayıcı.
async fn unit_cmd(cmd: Command) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || match client::command(cmd)? {
        Response::Error { message } => Err(message),
        _ => Ok(()),
    })
    .await
    .map_err(|e| format!("görev hatası: {e}"))?
}

#[tauri::command]
async fn protection_status() -> Result<EngineStatus, String> {
    status_cmd(Command::Status).await
}

#[tauri::command]
async fn start_protection() -> Result<EngineStatus, String> {
    status_cmd(Command::Start).await
}

#[tauri::command]
async fn stop_protection() -> Result<EngineStatus, String> {
    status_cmd(Command::Stop).await
}

/// Switch the user-facing protection mode ("hafif" | "guclu"). Returns the REAL post-switch status
/// (including the verify state), so the UI can gate "active" on what actually landed.
#[tauri::command]
async fn set_protection_mode(mode: String) -> Result<EngineStatus, String> {
    status_cmd(Command::SetProtectionMode { mode }).await
}

#[tauri::command]
async fn set_strategy(id: String, repeats_override: Option<u32>) -> Result<EngineStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        client::command_status(Command::SetStrategy { id, repeats_override })
    })
        .await
        .map_err(|e| format!("görev hatası: {e}"))?
}

#[tauri::command]
async fn set_dns(profile: String) -> Result<EngineStatus, String> {
    tauri::async_runtime::spawn_blocking(move || client::command_status(Command::SetDns { profile }))
        .await
        .map_err(|e| format!("görev hatası: {e}"))?
}

#[tauri::command]
async fn block_app(id: String, path: String, block: bool) -> Result<(), String> {
    unit_cmd(Command::BlockApp { id, path, block }).await
}

#[tauri::command]
async fn run_repair(tool: String) -> Result<(), String> {
    unit_cmd(Command::Repair { tool }).await
}

#[tauri::command]
async fn set_tweak(key: String, value: String) -> Result<(), String> {
    unit_cmd(Command::SetTweak { key, value }).await
}

#[tauri::command]
async fn set_limit(id: String, path: String, down: u32, up: u32) -> Result<(), String> {
    unit_cmd(Command::SetLimit { id, path, down, up }).await
}

/// Bypass alan adı listesini servise gönder (Faz 3.4). Koruma aktifse motor hot-reload eder.
#[tauri::command]
async fn set_hostlist(domains: Vec<String>) -> Result<(), String> {
    unit_cmd(Command::SetHostlist { domains }).await
}

/// Uygulama başı koruma modlarının BÜTÜNLÜKLÜ snapshot'ı: (id, "off"|"dpi"|"warp", exe path).
/// Servis aggregate edip:
///  - WARP tünelini açar/kapatır (en az bir "warp" → tünel açık; aksi halde saf winws)
///  - "off" modlu uygulamaların ÇALIŞAN PID'lerinin source port'larını ~5sn'de bir tarar →
///    winws WinDivert capture filter'ından HARİÇ tutar → gerçek per-app DPI off (bkz. service.rs
///    watchdog ve pid_scan modülü).
#[tauri::command]
async fn set_app_modes(modes: Vec<(String, String, String)>) -> Result<(), String> {
    unit_cmd(Command::SetAppModes { modes }).await
}

/// Tam Koruma: tüm sistem trafiğini WARP full-tunnel'dan geçir (per-app warp DEĞİL · standart WARP).
/// `enable=false` → uygulama-başı moda geri dön. Frontend applyFullProtection/exitFullProtection çağırır.
#[tauri::command]
async fn set_full_warp(enable: bool) -> Result<(), String> {
    unit_cmd(Command::SetFullWarp { enable }).await
}

// ===========================================================================
// Blueprint genişletmesi (docs/07) — profil / motor / preflight / teşhis / auto-pilot / rollback.
// Sorgu sonuçları JSON string olarak döner (frontend parse eder).
// ===========================================================================

/// Yapılandırılmış sorgu komutunu thread havuzunda çalıştırıp `Data` JSON string'ini döndür.
async fn data_cmd(cmd: Command) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || client::command_data(cmd))
        .await
        .map_err(|e| format!("görev hatası: {e}"))?
}

/// Bilinen DPI motorlarının kataloğu + kullanılabilirlik (UI motor seçici). → JSON Vec<EngineInfo>.
#[tauri::command]
async fn engine_catalog() -> Result<String, String> {
    data_cmd(Command::EngineCatalog).await
}

/// Aktif DPI motorunu değiştir ("zapret"|"byedpi"|"goodbyedpi"). Çalışıyorsa yeniden başlatır.
#[tauri::command]
async fn set_engine(id: String) -> Result<EngineStatus, String> {
    tauri::async_runtime::spawn_blocking(move || client::command_status(Command::SetEngine { id }))
        .await
        .map_err(|e| format!("görev hatası: {e}"))?
}

/// Tüm profilleri listele. → JSON Vec<Profile>.
#[tauri::command]
async fn list_profiles() -> Result<String, String> {
    data_cmd(Command::ListProfiles).await
}

/// Profili kaydet / içe aktar (JSON gövdesi servis tarafında doğrulanır).
#[tauri::command]
async fn save_profile(json: String) -> Result<(), String> {
    unit_cmd(Command::SaveProfile { json }).await
}

/// Profili sil.
#[tauri::command]
async fn delete_profile(id: String) -> Result<(), String> {
    unit_cmd(Command::DeleteProfile { id }).await
}

/// Profili dışa aktar (paylaşılabilir JSON). → JSON Profile.
#[tauri::command]
async fn export_profile(id: String) -> Result<String, String> {
    data_cmd(Command::ExportProfile { id }).await
}

/// Profili uygula: motoru/stratejiyi/hostlist'i/DNS'i ayarla + başlat (durum makinesi).
#[tauri::command]
async fn apply_profile(id: String) -> Result<EngineStatus, String> {
    tauri::async_runtime::spawn_blocking(move || client::command_status(Command::ApplyProfile { id }))
        .await
        .map_err(|e| format!("görev hatası: {e}"))?
}

/// DNS'i DHCP'ye sıfırla + DoH temizle.
#[tauri::command]
async fn reset_dns() -> Result<(), String> {
    unit_cmd(Command::ResetDns).await
}

/// DNS doğrula (aktif sunucular + güvenli mi). → JSON DnsVerify.
#[tauri::command]
async fn verify_dns() -> Result<String, String> {
    data_cmd(Command::VerifyDns).await
}

/// Ön-uçuş kontrolleri (admin/winws/çakışma/WARP/DoH). → JSON PreflightResult.
#[tauri::command]
async fn preflight() -> Result<String, String> {
    data_cmd(Command::Preflight).await
}

/// Hedef site teşhisi (DNS/TCP/gecikme). → JSON Vec<TargetDiag>.
#[tauri::command]
async fn diagnose(targets: Vec<String>) -> Result<String, String> {
    data_cmd(Command::Diagnose { targets }).await
}

/// Auto-Pilot: hedefleri her aday motorla test et, skor tablosu döndür. → JSON Vec<ScoreRow>.
#[tauri::command]
async fn autopilot(targets: Vec<String>, depth: String) -> Result<String, String> {
    data_cmd(Command::AutoPilot { targets, depth }).await
}

/// Yapılan tüm sistem değişikliklerini ters sırada geri al (transaction log).
#[tauri::command]
async fn rollback_all() -> Result<(), String> {
    unit_cmd(Command::RollbackAll).await
}

// ---- KULLANICI bağlamı (UI süreci — servis DEĞİL; %APPDATA%/%LOCALAPPDATA% kullanıcı profili) ----

/// Discord'u onar: süreçleri sonlandır + önbellek klasörlerini temizle ("Checking for updates" takılması).
#[tauri::command]
async fn repair_discord() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(repair::repair_discord)
        .await
        .map_err(|e| format!("görev hatası: {e}"))?
}

/// WebCord kur (Discord alternatifi — GitHub release zip'i %LOCALAPPDATA%\evorift\WebCord altına).
#[tauri::command]
async fn install_webcord() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(repair::install_webcord)
        .await
        .map_err(|e| format!("görev hatası: {e}"))?
}

/// Install Discord PTB via the direct PTB endpoint (docs/05 §2). User context; no admin needed.
#[tauri::command]
async fn install_discord_ptb() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(repair::install_discord_ptb)
        .await
        .map_err(|e| format!("task error: {e}"))?
}

/// Verify bundled binary SHA-256 hashes against resources/manifest.json (item 10.3).
/// Returns JSON array of `VerifyResult`. Advisory — mismatches are logged, never fatal.
#[tauri::command]
async fn verify_manifest() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let results = manifest::verify_all()?;
        serde_json::to_string(&results).map_err(|e| format!("serialize error: {e}"))
    })
    .await
    .map_err(|e| format!("task error: {e}"))?
}

/// Attach any app .exe to the ProxiFyre SOCKS5 proxy (docs/03 §2.3 generic wizard).
/// `exe_name` — bare filename, e.g. "MyGame.exe". Privileged; no-op when bundle absent.
#[tauri::command]
async fn attach_app_to_proxy(exe_name: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || repair::attach_app_to_proxy(&exe_name))
        .await
        .map_err(|e| format!("task error: {e}"))?
}

/// Derived health signal for BlackHole gating (item 11.2). → JSON HealthSignal.
#[tauri::command]
async fn health() -> Result<String, String> {
    data_cmd(Command::Health).await
}

/// WARP tunnel health snapshot (item 11.1). → JSON TunnelState.
#[tauri::command]
async fn tunnel_status() -> Result<String, String> {
    data_cmd(Command::TunnelStatus).await
}

/// Discord kurulum yolunu bul (çalışan süreç → %LOCALAPPDATA%\Discord\app-*).
#[tauri::command]
async fn find_discord_path() -> Option<String> {
    tauri::async_runtime::spawn_blocking(repair::find_discord_path)
        .await
        .unwrap_or(None)
}

// ---- Çalışma modu + servis kontrolü (servissiz ↔ servisli "ekstra koruma") ----

/// UI'ye çalışma modunu bildirir → banner/buton durumu. mode: "service" | "serviceless" | "limited".
#[derive(Serialize)]
struct RuntimeMode {
    /// UI süreci yönetici mi (serviceless için gerekli).
    elevated: bool,
    /// EvoriftSvc durumu: "running" | "stopped" | "absent".
    service: String,
    /// Etkin mod: "service" (kalıcı, UI'siz çalışır) | "serviceless" (yönetici UI motoru taşır) |
    /// "limited" (ne servis ne yönetici → koruma yok; elevate veya servis kur).
    mode: String,
}

/// UI açılışta + durum değişiminde çağırır → hangi modda olduğumuzu göster (banner/buton mantığı).
#[tauri::command]
fn runtime_mode() -> RuntimeMode {
    let elevated = process_is_elevated();
    let service = svcctl::status().to_string();
    let mode = if service == "running" {
        "service"
    } else if elevated {
        "serviceless"
    } else {
        "limited"
    }
    .to_string();
    RuntimeMode { elevated, service, mode }
}

/// EvoriftSvc durumu (UI rozeti): "running" | "stopped" | "absent".
#[tauri::command]
fn service_status() -> String {
    svcctl::status().to_string()
}

/// "Ekstra koruma"yı aç: EvoriftSvc'yi kur + başlat (boot'ta otomatik koru, UI kapalıyken çalış).
/// YÖNETİCİ gerekir (sc create). Kurulumdan sonra UI'nin yeniden başlatılması önerilir (servis pipe'ı devralır).
#[tauri::command]
async fn install_service() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(svcctl::install)
        .await
        .map_err(|e| format!("görev hatası: {e}"))?
}

/// "Ekstra koruma"yı kapat: EvoriftSvc'yi durdur + kaldır (servissiz moda dön). YÖNETİCİ gerekir.
#[tauri::command]
async fn uninstall_service() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(svcctl::uninstall)
        .await
        .map_err(|e| format!("görev hatası: {e}"))?
}

/// Uninstall evorift: undo everything it changed on this machine, then hand off to the Windows
/// uninstaller. Deliberately low-friction — the UI asks once and this does the rest; there is no
/// "are you sure you want to leave", no retention offer, no partial "keep my settings" branch.
///
/// Order matters: system changes are reverted BEFORE the service is removed, because the rollback
/// log is replayed through the service (DNS, firewall rules, tweaks, tunnel). Losing the service
/// first would strand those changes on the machine with nothing left able to undo them. Rollback
/// failure is reported but does NOT abort the uninstall: a user who asked to uninstall must not be
/// trapped in the app because one revert step failed — the message tells them what to check.
#[tauri::command]
async fn uninstall_app(app: tauri::AppHandle) -> Result<(), String> {
    let mut problems: Vec<String> = Vec::new();

    if let Err(m) = unit_cmd(Command::RollbackAll).await {
        problems.push(format!("sistem değişiklikleri geri alınamadı: {m}"));
    }
    if let Err(m) = tauri::async_runtime::spawn_blocking(svcctl::uninstall)
        .await
        .map_err(|e| format!("görev hatası: {e}"))?
    {
        problems.push(format!("servis kaldırılamadı: {m}"));
    }

    // NSIS (perMachine) drops uninstall.exe next to the installed exe. If it isn't there the app is
    // most likely running from the portable zip or a dev build, where there is nothing to uninstall
    // — say so plainly instead of silently doing nothing.
    let uninstaller = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("uninstall.exe")))
        .filter(|p| p.exists());

    match uninstaller {
        Some(path) => {
            std::process::Command::new(&path)
                .spawn()
                .map_err(|e| format!("kaldırıcı başlatılamadı: {e}"))?;
            app.exit(0); // release our own files so the uninstaller can delete them
            Ok(())
        }
        None => {
            let mut msg = String::from(
                "Kaldırıcı bulunamadı (taşınabilir sürüm veya geliştirme derlemesi olabilir). \
                 Sistem değişiklikleri geri alındı; klasörü elle silebilirsin.",
            );
            if !problems.is_empty() {
                msg.push_str(&format!(" Ayrıca: {}", problems.join("; ")));
            }
            Err(msg)
        }
    }
}

/// Create a support bundle ZIP (logs + system summary) and return the path.
/// Returns the absolute path to the created zip file so the frontend can show it / open it.
#[tauri::command]
async fn create_log_bundle() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let out = crate::sys::log_dir().join("evorift-bundle.zip");
        logbundle::create_bundle(&out).map(|p| p.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| format!("görev hatası: {e}"))?
}

/// Start EvoriftSvc (sc start) — requires admin.
#[tauri::command]
async fn start_svc() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| {
        let out = hidden_command("sc")
            .args(["start", svcctl::SERVICE_NAME])
            .output()
            .map_err(|e| format!("sc start çalıştırılamadı: {e}"))?;
        if out.status.success() || String::from_utf8_lossy(&out.stdout).contains("RUNNING") {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stdout).trim().to_string())
        }
    })
    .await
    .map_err(|e| format!("görev hatası: {e}"))?
}

/// Stop EvoriftSvc (sc stop) — requires admin.
#[tauri::command]
async fn stop_svc() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| {
        let out = hidden_command("sc")
            .args(["stop", svcctl::SERVICE_NAME])
            .output()
            .map_err(|e| format!("sc stop çalıştırılamadı: {e}"))?;
        if out.status.success() || String::from_utf8_lossy(&out.stdout).contains("STOPPED") {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stdout).trim().to_string())
        }
    })
    .await
    .map_err(|e| format!("görev hatası: {e}"))?
}

/// Restart the active DPI engine: send Stop then Start over IPC.
/// Useful after changing engine params (GoodbyeDPI mode, ByeDPI flags) to apply them live.
#[tauri::command]
async fn restart_engine() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| {
        // Stop (ignore error — engine might already be stopped)
        let _ = client::command(Command::Stop);
        std::thread::sleep(std::time::Duration::from_millis(400));
        // Start
        match client::command(Command::Start)? {
            Response::Error { message } => Err(message),
            _ => Ok(()),
        }
    })
    .await
    .map_err(|e| format!("görev hatası: {e}"))?
}

/// Windows ile otomatik başlat (Faz 4.7). HKCU\…\Run anahtarına yazar/siler — eklenti gerektirmez,
/// yalnız mevcut kullanıcı (admin gerekmez). `minimized` → autostart'ta `--minimized` argümanı eklenir
/// (setup() bunu görünce pencereyi tray'e gizler).
#[tauri::command]
async fn set_autostart(enable: bool, minimized: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || set_autostart_blocking(enable, minimized))
        .await
        .map_err(|e| format!("görev hatası: {e}"))?
}

fn set_autostart_blocking(enable: bool, minimized: bool) -> Result<(), String> {
    const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    if enable {
        let exe = std::env::current_exe().map_err(|e| format!("exe yolu alınamadı: {e}"))?;
        let exe = exe.to_string_lossy();
        let val = if minimized {
            format!("\"{exe}\" --minimized")
        } else {
            format!("\"{exe}\"")
        };
        let out = hidden_command("reg")
            .args(["add", RUN_KEY, "/v", "evorift", "/t", "REG_SZ", "/d", &val, "/f"])
            .output()
            .map_err(|e| format!("reg çalıştırılamadı: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
        }
    } else {
        // anahtar yoksa hata önemsiz → yut
        let _ = hidden_command("reg")
            .args(["delete", RUN_KEY, "/v", "evorift", "/f"])
            .output();
        Ok(())
    }
}

/// Tray simgesi tooltip'ini güncelle (frontend durum değişince çağırır → dile uygun metin).
#[tauri::command]
fn set_tray_tooltip(app: tauri::AppHandle, text: String) {
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_tooltip(Some(text));
    }
}

/// UI süreci yönetici (elevated) olarak mı çalışıyor? Servis kurulmadan (gömülü sunucu) gerçek
/// tweak/limit/firewall yalnız elevated UI'da uygulanır → banner bunu kullanıcıya bildirir (Faz P2.4).
#[cfg(windows)]
fn process_is_elevated() -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    unsafe {
        let mut token: HANDLE = 0;
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut ret_len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut core::ffi::c_void,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret_len,
        );
        CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}
#[cfg(not(windows))]
fn process_is_elevated() -> bool {
    false
}

#[tauri::command]
fn is_admin() -> bool {
    process_is_elevated()
}

/// Uygulamayı yönetici olarak yeniden başlat (UAC). Kabul → mevcut süreçten çık (elevated kopya devralır);
/// UAC reddedilirse mevcut süreç çalışmaya devam eder (Start-Process hata döndürür).
#[tauri::command]
fn relaunch_as_admin(app: tauri::AppHandle) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.to_string_lossy().replace('\'', "''"); // PowerShell tek-tırnak kaçışı
    let out = hidden_command("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!("Start-Process -FilePath '{exe}' -Verb RunAs"),
        ])
        .output()
        .map_err(|e| format!("yeniden başlatılamadı: {e}"))?;
    if out.status.success() {
        app.exit(0);
        Ok(())
    } else {
        Err("yönetici izni reddedildi".into())
    }
}

/// Tray menü öğelerinin saklanan referansları → dil değişince etiketleri güncellemek için (set_tray_labels).
struct TrayMenuItems {
    show: tauri::menu::MenuItem<tauri::Wry>,
    toggle: tauri::menu::MenuItem<tauri::Wry>,
    quit: tauri::menu::MenuItem<tauri::Wry>,
}

/// Tray menü etiketlerini güncelle (dil değişince frontend çağırır → 4 dil).
#[tauri::command]
fn set_tray_labels(app: tauri::AppHandle, show: String, toggle: String, quit: String) {
    if let Some(items) = app.try_state::<TrayMenuItems>() {
        let _ = items.show.set_text(show);
        let _ = items.toggle.set_text(toggle);
        let _ = items.quit.set_text(quit);
    }
}

/// Ana pencereyi göster + öne getir.
fn show_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Ana pencereyi göster/gizle arasında geçiş yap.
fn toggle_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
        } else {
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // SERVİSSİZ MOD: yönetici UI, motoru (winws + WARP) KENDİ İÇİNDE çalıştırır — ayrı servise GEREK YOK.
    // Karar (öncelik sırası):
    //   • DEV (debug_assertions) → ALWAYS embedded. Stop any running EvoriftSvc first so its old binary
    //     doesn't conflict with the freshly compiled IPC protocol. This prevents the token-mismatch
    //     "handshake reddedildi" loop caused by an installed service running a stale binary.
    //   • RELEASE + EvoriftSvc ÇALIŞIYOR  → ona bırak (pipe'ı o sahiplenir); UI yalnız IPC istemcisi.
    //   • RELEASE + EvoriftSvc KURULU+DURDURULMUŞ → start dene (elevated ise); başlamazsa embedded aç.
    //   • RELEASE + UI YÖNETİCİ + servis YOK (absent) → gömülü serve_blocking aç.
    //   • RELEASE + UI YETKİSİZ + servis YOK → embedded AÇMA; "limited" mod (sim-only).
    let svc_state = svcctl::status();
    let want_embedded = if cfg!(debug_assertions) {
        // Dev mode: always use embedded server so we test the freshly compiled code.
        // Stop EvoriftSvc if running — its stale binary would clash with the freshly compiled IPC.
        // Also handles the "stopped but auto-restarts" race: we keep polling until truly stopped.
        if svc_state != "absent" {
            let _ = hidden_command("sc").args(["stop", svcctl::SERVICE_NAME]).output();
            // Wait up to 3 seconds for the service to fully stop so it can't overwrite our token.
            for _ in 0..6 {
                std::thread::sleep(std::time::Duration::from_millis(500));
                if svcctl::status() != "running" {
                    break;
                }
            }
        }
        true
    } else if svc_state == "running" {
        // Release: service owns the pipe — don't compete.
        false
    } else if svc_state == "stopped" {
        // Release: service installed but not running — try to start it.
        if process_is_elevated() {
            let _ = hidden_command("sc").args(["start", svcctl::SERVICE_NAME]).output();
            std::thread::sleep(std::time::Duration::from_millis(800));
            svcctl::status() != "running"
        } else {
            false // not elevated, not running → limited mode (no embedded in release)
        }
    } else {
        // svc_state == "absent" — no service installed.
        process_is_elevated()
    };
    if want_embedded {
        std::thread::spawn(|| {
            if let Err(e) = service::serve_blocking() {
                eprintln!("[evorift] gömülü sunucu başlamadı (EvoriftSvc çalışıyor olabilir): {e}");
            }
        });
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // Otomatik güncelleme (yalnız release; desktop). Frontend açılışta check() çağırır →
        // varsa imzalı setup'ı indirip kurar, sonra process plugin'iyle yeniden başlatır.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        // Pencere kapatma = tray'e gizle (Faz 4.7). Gerçek çıkış yalnız tray "Çıkış" menüsünden.
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            // ---- sistem tepsisi (tray) simgesi + menü (Faz 4.7) ----
            let show_i = MenuItemBuilder::with_id("show", "Göster / Gizle").build(app)?;
            let toggle_i = MenuItemBuilder::with_id("toggle", "Korumayı Aç/Kapat").build(app)?;
            let quit_i = MenuItemBuilder::with_id("quit", "Çıkış").build(app)?;
            let menu = MenuBuilder::new(app)
                .items(&[&show_i, &toggle_i, &quit_i])
                .build()?;
            let mut tray = TrayIconBuilder::with_id("main")
                .tooltip("evorift")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => toggle_window(app),
                    "toggle" => {
                        let _ = app.emit("tray-toggle", ());
                    }
                    "quit" => {
                        // Servis modeli: app'ten çıkınca korumayı durdur (kullanıcı isteği: "kapatırsam servis durur").
                        // Servis süreci (LocalSystem) çalışmaya devam eder; yalnız motor kapanır.
                        let _ = client::command(Command::Stop);
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_window(tray.app_handle());
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            // tray menü öğelerini sakla → dil değişince set_tray_labels ile etiketleri güncelle
            app.manage(TrayMenuItems {
                show: show_i,
                toggle: toggle_i,
                quit: quit_i,
            });

            // ---- autostart --minimized: başlangıçta pencereyi tray'e gizle ----
            if std::env::args().any(|a| a == "--minimized" || a == "--tray") {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }

            // ---- telemetri aboneliği: servisten gelen ~1 Hz metriği "telemetry" event'i olarak yayınla ----
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                let h = handle.clone();
                let _ = client::subscribe(move |m| {
                    let _ = h.emit("telemetry", m);
                });
                std::thread::sleep(std::time::Duration::from_millis(1000));
            });



            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            protection_status,
            start_protection,
            stop_protection,
            set_strategy,
            set_protection_mode,
            set_dns,
            block_app,
            run_repair,
            set_tweak,
            set_limit,
            set_hostlist,
            set_app_modes,
            set_full_warp,
            connectivity_test,
            dns_status,
            list_apps,
            detect_app_domains,
            get_app_icons,
            set_autostart,
            set_tray_tooltip,
            set_tray_labels,
            is_admin,
            relaunch_as_admin,
            engine_catalog,
            set_engine,
            list_profiles,
            save_profile,
            delete_profile,
            export_profile,
            apply_profile,
            reset_dns,
            verify_dns,
            preflight,
            diagnose,
            autopilot,
            rollback_all,
            repair_discord,
            install_webcord,
            install_discord_ptb,
            attach_app_to_proxy,
            verify_manifest,
            find_discord_path,
            runtime_mode,
            service_status,
            install_service,
            uninstall_service,
            uninstall_app,
            start_svc,
            stop_svc,
            restart_engine,
            create_log_bundle,
            health,
            tunnel_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
