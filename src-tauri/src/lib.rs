pub mod client;
pub mod engine;
pub mod ipc;
pub mod pid_scan;
pub mod service;
pub mod warp;

use ipc::{Command, EngineStatus, Response};
use serde::{Deserialize, Serialize};
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

/// PowerShell çıktısındaki ham satır.
#[derive(Deserialize)]
struct PsProc {
    proc: Option<String>,
    desc: Option<String>,
    company: Option<String>,
    exe: Option<String>,
    path: Option<String>,
}

/// Ağ kullanan çalışan uygulamaları listele (aktif TCP / dinleyen UDP sahibi PID'ler → exe yolu + ad).
/// Yetkisiz salt-okuma; yükseltilmiş süreçlerin yolu alınamazsa zarifçe atlanır. Mock listenin yerine.
#[tauri::command]
async fn list_apps() -> Vec<DetectedApp> {
    // UI komut thread'ini bloke etmemek için PowerShell taramasını ayrı thread'de çalıştır (review LOW-7).
    tauri::async_runtime::spawn_blocking(list_apps_blocking)
        .await
        .unwrap_or_default()
}

fn list_apps_blocking() -> Vec<DetectedApp> {
    let script = r#"$ErrorActionPreference='SilentlyContinue'
$pids=@()
$pids+=(Get-NetTCPConnection -ErrorAction SilentlyContinue | Where-Object {$_.State -eq 'Established' -or $_.State -eq 'Listen'}).OwningProcess
$pids+=(Get-NetUDPEndpoint -ErrorAction SilentlyContinue).OwningProcess
$pids=$pids | Where-Object {$_ -gt 4} | Sort-Object -Unique
$rows=foreach($procId in $pids){ $p=Get-Process -Id $procId -ErrorAction SilentlyContinue; if($p -and $p.Path){ [PSCustomObject]@{proc=$p.ProcessName;desc=$p.Description;company=$p.Company;exe=(Split-Path $p.Path -Leaf);path=$p.Path} } }
@($rows | Sort-Object exe -Unique) | ConvertTo-Json -Compress -Depth 3"#;
    let out = hidden_command("powershell")
        .args(["-NoProfile", "-Command", script])
        .output();
    let mut apps = Vec::new();
    if let Ok(o) = out {
        let s = String::from_utf8_lossy(&o.stdout);
        // PowerShell 5.1 çıktısı baştan UTF-8 BOM taşıyabilir → serde'den önce sıyır (review LOW-8).
        let s = s.trim_start_matches('\u{feff}').trim();
        if !s.is_empty() && s != "null" {
            // ConvertTo-Json tek satırda nesne, çoklu satırda dizi döndürür → ikisini de dene
            let vals: Vec<PsProc> = serde_json::from_str::<Vec<PsProc>>(s)
                .or_else(|_| serde_json::from_str::<PsProc>(s).map(|x| vec![x]))
                .unwrap_or_default();
            for v in vals {
                let exe = v.exe.unwrap_or_default();
                if exe.is_empty() {
                    continue;
                }
                // id, QoS politika adı + netsh kural adı olarak kullanılır → güvenli slug'a indirge
                // (boşluk/Türkçe/sembol → '-'); gerçek eşleştirme `path` ile yapıldığından ad serbest.
                let id = exe
                    .to_lowercase()
                    .trim_end_matches(".exe")
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
                    .collect::<String>();
                let name = v
                    .desc
                    .filter(|d| !d.trim().is_empty())
                    .or(v.proc)
                    .unwrap_or_else(|| exe.clone());
                apps.push(DetectedApp {
                    id,
                    name,
                    exe,
                    path: v.path.unwrap_or_default(),
                    kind: v.company.unwrap_or_default(),
                });
            }
        }
    }
    apps
}

/// Bir uygulamanın ULAŞTIĞI domain'leri tespit et (Apps → otomatik bypass).
/// Yöntem (yetkisiz, salt-okuma): exe'nin çalışan PID'lerinin kurulu (Established) TCP uzak IP'lerini al →
/// DNS istemci önbelleğiyle (Get-DnsClientCache: hostname→IP) eşleştir → hostname listesi.
/// exe ENV ile geçer (komut satırına interpolasyon YOK → enjeksiyon yüzeyi yok, P7/A5 deseni).
#[tauri::command]
async fn detect_app_domains(exe: String) -> Vec<String> {
    tauri::async_runtime::spawn_blocking(move || detect_domains_blocking(&exe))
        .await
        .unwrap_or_default()
}

fn detect_domains_blocking(exe: &str) -> Vec<String> {
    if exe.trim().is_empty() {
        return Vec::new();
    }
    // Statik script; exe yalnız $env:EVORIFT_DETECT_EXE ile gelir (interpolasyon yok).
    let script = r#"$ErrorActionPreference='SilentlyContinue'
$name = ($env:EVORIFT_DETECT_EXE -replace '\.exe$','')
$set=@{}
if($name){
  $procIds = @(Get-Process -Name $name | Select-Object -ExpandProperty Id)
  if($procIds.Count -gt 0){
    $ips = @(Get-NetTCPConnection -OwningProcess $procIds -State Established | Select-Object -ExpandProperty RemoteAddress -Unique) |
      Where-Object { $_ -and $_ -notmatch '^(127\.|10\.|192\.168\.|172\.(1[6-9]|2[0-9]|3[01])\.|169\.254\.|0\.0\.0\.0|::1$|fe80|ff)' }
    if($ips.Count -gt 0){
      $cache = Get-DnsClientCache | Where-Object { $_.Data }
      foreach($ip in $ips){ foreach($e in ($cache | Where-Object { $_.Data -eq $ip })){ if($e.Entry){ $set[$e.Entry.ToLower().TrimEnd('.')]=$true } } }
    }
  }
}
@($set.Keys) | ConvertTo-Json -Compress"#;
    let out = hidden_command("powershell")
        .args(["-NoProfile", "-Command", script])
        .env("EVORIFT_DETECT_EXE", exe)
        .output();
    let mut domains = Vec::new();
    if let Ok(o) = out {
        let s = String::from_utf8_lossy(&o.stdout);
        let s = s.trim_start_matches('\u{feff}').trim();
        if !s.is_empty() && s != "null" {
            // tek hostname → string; çoklu → dizi
            let vals: Vec<String> = serde_json::from_str::<Vec<String>>(s)
                .or_else(|_| serde_json::from_str::<String>(s).map(|x| vec![x]))
                .unwrap_or_default();
            for d in vals {
                let d = d.trim().to_lowercase();
                // alan-adı sağlık kontrolü (ipc::validate hostlist beyaz-listesiyle uyumlu: [a-z0-9.-], nokta var)
                if !d.is_empty()
                    && d.len() <= 253
                    && d.contains('.')
                    && d.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
                    && !domains.contains(&d)
                {
                    domains.push(d);
                }
            }
        }
    }
    domains
}

// ============================================================================
// Anti-cheat watcher (Faz 3.6) — opsiyonel güvenlik ağı.
// Korumalı oyun (Vanguard/EAC/BattlEye) çalışırken DPI müdahalesi anti-cheat
// tarafından şüpheli görülebilir. Watcher süreçleri ~5 sn'de bir tarar ve durum
// değişince "anticheat" Tauri event'i yayınlar. EYLEM (korumayı duraklat/sürdür)
// kararını frontend verir (kullanıcının "Anti-cheat koruması" toggle'ına göre);
// böylece toggle kapalıyken sadece bilgilendirme yapılır, otomatik müdahale olmaz.
// ============================================================================

#[derive(Serialize, Clone)]
struct AntiCheat {
    active: bool,
    name: String,
}

/// Bilinen anti-cheat süreç adları (küçük harf) → kullanıcıya gösterilecek ad.
const ANTICHEAT_PROCS: &[(&str, &str)] = &[
    ("vgc.exe", "Riot Vanguard"),
    ("vgtray.exe", "Riot Vanguard"),
    ("vgk.sys", "Riot Vanguard"),
    ("easyanticheat.exe", "Easy Anti-Cheat"),
    ("easyanticheat_eos.exe", "Easy Anti-Cheat"),
    ("beservice.exe", "BattlEye"),
    ("beservice_x64.exe", "BattlEye"),
    ("bedaisy.sys", "BattlEye"),
    ("faceitservice.exe", "FACEIT AC"),
    ("faceit.exe", "FACEIT AC"),
];

/// Çalışan süreçleri tara; bir anti-cheat tespit edilirse dostça adını döndür.
fn detect_anticheat() -> Option<String> {
    let out = hidden_command("tasklist")
        .args(["/fo", "csv", "/nh"])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout).to_lowercase();
    ANTICHEAT_PROCS
        .iter()
        .find(|(proc_name, _)| s.contains(proc_name))
        .map(|(_, friendly)| friendly.to_string())
}

// UI Tauri komutları → ayrıcalıklı servise pipe IPC ile proxy (docs/05 §1-2).
// Servis ulaşılamazsa UI bloke olmasın diye nazik yedek değer döner.

// ⚠️ Bu komutlar SENKRON pipe IPC + servis tarafında bloke-eden PowerShell/netsh çalıştırır.
// Tauri'de SENKRON komut UI thread'inde koşar → uzun komut pencereyi DONDURUR (Oyun Modu 8+ komut
// gönderince ~5sn donma buradan geliyordu). Hepsini `async fn` + `spawn_blocking` ile thread havuzuna
// taşı → UI asla bloke olmaz (list_apps/detect_app_domains'in zaten kullandığı desen).

/// IPC komutunu bloke-eden thread havuzunda çalıştırıp `EngineStatus` döndüren async sarmalayıcı.
async fn status_cmd(cmd: Command, fallback: EngineStatus) -> EngineStatus {
    tauri::async_runtime::spawn_blocking(move || client::command_status(cmd).unwrap_or(fallback.clone()))
        .await
        .unwrap_or_default()
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
async fn protection_status() -> EngineStatus {
    status_cmd(Command::Status, EngineStatus::default()).await
}

#[tauri::command]
async fn start_protection() -> EngineStatus {
    status_cmd(
        Command::Start,
        EngineStatus { running: true, strategy: "auto".into(), dns: "cloudflare".into() },
    )
    .await
}

#[tauri::command]
async fn stop_protection() -> EngineStatus {
    status_cmd(Command::Stop, EngineStatus::default()).await
}

#[tauri::command]
async fn set_strategy(id: String) -> Result<EngineStatus, String> {
    tauri::async_runtime::spawn_blocking(move || client::command_status(Command::SetStrategy { id }))
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
    // YALNIZ DEV (debug): ayrıcalıklı servis kurulu değilken UI↔IPC uçtan uca çalışsın diye
    // pipe sunucusunu gömülü bir iş parçacığında çalıştır. RELEASE'de bu YOK → gerçek LocalSystem
    // `EvoriftSvc` serve eder; app asla pipe için onunla YARIŞMAZ (aksi halde app önce açılırsa
    // pipe'ı SimEngine ile kapıp gerçek bypass'ı engellerdi). Güvenlik+doğruluk: P7 A10.
    #[cfg(debug_assertions)]
    std::thread::spawn(|| {
        if let Err(e) = service::serve_blocking() {
            eprintln!("[evorift] gömülü dev sunucusu başlamadı (muhtemelen EvoriftSvc çalışıyor): {e}");
        }
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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
            set_dns,
            block_app,
            run_repair,
            set_tweak,
            set_limit,
            set_hostlist,
            set_app_modes,
            connectivity_test,
            dns_status,
            list_apps,
            detect_app_domains,
            set_autostart,
            set_tray_tooltip,
            set_tray_labels,
            is_admin,
            relaunch_as_admin
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
