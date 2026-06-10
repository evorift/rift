//! Ayrıcalıklı servis tarafı: named-pipe sunucusu + komut motoru iskeleti (docs/05 §1-2).
//!
//! Bu iskelette motor yalnız durum tutar; gerçek ayrıcalıklı işler (WinDivert aç/kapat,
//! netsh/registry/DoH) Faz 3'te `dispatch()` içindeki kollara eklenecek. Her komut
//! `ipc::validate()` ile doğrulanır ve audit log'a yazılır.

use crate::engine::{self, DpiEngine};
use crate::ipc::{self, Command, EngineStatus, Metrics, Request, Response, PIPE_NAME};
use crate::warp::WarpEngine;
use interprocess::local_socket::{prelude::*, GenericNamespaced, ListenerOptions, Stream};
use std::io::{self, BufReader};
use std::process::Command as OsCommand;
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct Engine {
    running: bool,
    strategy: String,
    dns: String,
    hostlist: Vec<String>,
    /// Per-app İNDİRME limitleri: id → (gerçek exe yolu, kbps). Motora (WinDivert inbound throttle)
    /// iletilir. Yükleme (egress) limiti ayrı yoldan (QoS / run_limit) uygulanır.
    limits: std::collections::HashMap<String, (String, u32)>,
    /// Uygulama başı koruma modu: id → (mode, exe path). WARP tüneli yalnız EN AZ BİR uygulama "warp"
    /// modundaysa açılır → kullanıcı hiçbir uygulamayı maks-koruma'ya almadıysa saf winws (en az gecikme).
    /// Path: PID tarama için gerekli — "off" modlu uygulamaların ÇALIŞAN PID'lerinin source port'larını
    /// pid_scan ile bulup winws WinDivert capture filter'ından HARİÇ tutar (gerçek per-app off).
    /// UI SetAppModes ile bütünüklü snapshot gönderir; servis aggregate eder.
    app_modes: std::collections::HashMap<String, (String, String)>,
    dpi: Box<dyn DpiEngine>,
    /// Motor B — Discord IP'leri için WARP WireGuard split-tunnel (masaüstü Discord + ses). winws'in
    /// (Motor A) YANINDA çalışır; Start/Stop'ta birlikte aç/kapanır. Hataları ölümcül değil.
    warp: WarpEngine,
}

impl Engine {
    fn new() -> Self {
        Self {
            running: false,
            strategy: String::new(),
            dns: String::new(),
            hostlist: Vec::new(),
            limits: std::collections::HashMap::new(),
            app_modes: std::collections::HashMap::new(),
            dpi: engine::make_engine(),
            warp: WarpEngine::new(),
        }
    }

    /// EN AZ BİR uygulama "warp" modundaysa true → WARP tüneli açık olmalı. Boş harita (UI hiç bildirmedi)
    /// → varsayılan davranış: WARP açık (boot auto-protect'te masaüstü Discord güvenilir bağlansın).
    fn want_warp(&self) -> bool {
        if self.app_modes.is_empty() {
            return true; // boot/dev: UI bağlanmadıysa güvenli varsayılan
        }
        self.app_modes.values().any(|(m, _)| m == "warp")
    }

    /// "Off" modlu uygulamaların exe yolları — pid_scan'a beslenir. Path'i olmayanlar (henüz tespit
    /// edilmemiş) atlanır (PID bulunamaz → o app için exclusion uygulanmaz, zararı yok).
    fn off_app_paths(&self) -> Vec<String> {
        self.app_modes
            .values()
            .filter(|(mode, path)| mode == "off" && !path.is_empty())
            .map(|(_, p)| p.clone())
            .collect()
    }

    /// Mevcut indirme limitlerini motorun beklediği `(yol, kbps)` listesine indir (yol boş olanlar atlanır —
    /// inbound paket→PID→exe eşlemesi gerçek yol ister).
    fn limit_list(&self) -> Vec<(String, u32)> {
        self.limits
            .values()
            .filter(|(p, _)| !p.is_empty())
            .map(|(p, d)| (p.clone(), *d))
            .collect()
    }
    fn status(&self) -> EngineStatus {
        EngineStatus {
            running: self.running,
            strategy: self.strategy.clone(),
            dns: self.dns.clone(),
        }
    }
}

/// Audit log (iskelet: stderr). Üretimde `%PROGRAMDATA%\evorift\audit.log`.
fn audit(action: &str) {
    eprintln!("[evorift-svc][audit] {action}");
}

/// Servis boot'ta otomatik açılan korumanın varsayılan hostlist'i (state.svelte.ts CORE_SITES + YouTube
/// ile birebir). UI bağlanınca kullanıcının tam site listesini SetHostlist ile bunun ÜSTÜNE ekler.
const DEFAULT_HOSTLIST: &[&str] = &[
    "discord.com", "discordapp.com", "discord.gg", "discordapp.net", "discord.media",
    "gateway.discord.gg", "cdn.discordapp.com", "roblox.com", "www.roblox.com", "rbxcdn.com",
    "youtube.com", "googlevideo.com",
];

// Motor B (WARP split-tunnel) açık/kapalı kararı artık UI'dan gelen uygulama başı modlara bağlıdır
// (SetAppModes → Engine::want_warp). En az bir uygulama "warp" modunda → WARP tüneli açılır; aksi
// halde saf winws (en az gecikme). UI hiç bildirmediyse (boot auto-protect / dev) güvenli varsayılan:
// WARP açık → masaüstü Discord ses dahil hemen bağlansın.

fn dispatch(engine: &Arc<Mutex<Engine>>, cmd: Command) -> Response {
    if let Err(m) = ipc::validate(&cmd) {
        audit(&format!("REJECT {cmd:?}: {m}"));
        return Response::Error { message: m };
    }
    // §6: poison-safe kilit — bir thread motor kilidini tutarken panik ederse Mutex zehirlenir; `unwrap()`
    // burada da panik eder → tüm sonraki komutlar düşer. `into_inner()` ile zehirli kilidi KURTAR.
    let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
    match cmd {
        Command::Start => {
            if e.strategy.is_empty() {
                e.strategy = "auto".into();
            }
            if e.dns.is_empty() {
                e.dns = "cloudflare".into();
            }
            let strat = engine::strategy_by_id(&e.strategy);
            // hostlist: UI'dan SetHostlist ile gelen kalıcı liste (boşsa motor tüm seçili trafiğe uygular)
            let hostlist = e.hostlist.clone();
            match e.dpi.start(&strat, &hostlist) {
                Ok(()) => {
                    e.running = true;
                    // mevcut indirme limitlerini motora (yeniden) bildir — (re)start'ta inbound throttle hazır olsun
                    let ll = e.limit_list();
                    e.dpi.set_limits(&ll);
                    // Motor B (WARP split-tunnel): UI'dan gelen uygulama-başı modlar agg edilir
                    // (en az bir "warp" → tünel açık). Boşsa varsayılan AÇIK (boot/dev). Aksi halde
                    // çalışıyorsa kapat (kullanıcı tüm uygulamaları off/dpi'a aldı → saf winws).
                    if e.want_warp() {
                        if let Err(m) = e.warp.start() {
                            audit(&format!("warp start atlandı (winws aktif): {m}"));
                        }
                    } else {
                        e.warp.stop();
                    }
                    // DNS/DoH BURADA uygulanmaz: UI toggle'ı ayrı SetDns gönderir (çift uygulama/yarış olmasın);
                    // boot auto-protect'te (UI yok) DNS, serve_blocking içindeki auto-protect bloğunda uygulanır (§5).
                    audit(&format!("start strategy={}", strat.id));
                    Response::Status(e.status())
                }
                Err(m) => Response::Error { message: m },
            }
        }
        Command::Stop => {
            e.warp.stop(); // önce WARP tünelini kaldır (winws'ten önce — sıra önemsiz ama tutarlı)
            e.dpi.stop();
            e.running = false;
            audit("stop");
            Response::Status(e.status())
        }
        Command::Status => Response::Status(e.status()),
        Command::SetStrategy { id } => {
            audit(&format!("set_strategy {id}"));
            e.strategy = id;
            // Motor çalışıyorsa yeni stratejiyi anında uygula (güvenli motor: start() yalnız cfg günceller,
            // handle teardown YOK → kesintisiz). Çalışmıyorsa sonraki Start'ta geçerli olur.
            if e.running {
                let strat = engine::strategy_by_id(&e.strategy);
                let hl = e.hostlist.clone();
                let _ = e.dpi.start(&strat, &hl);
            }
            Response::Status(e.status())
        }
        Command::SetDns { profile } => {
            audit(&format!("set_dns {profile}"));
            e.dns = profile.clone();
            let status = e.status();
            drop(e); // kilidi bırak — DNS/DoH OS komutları kilit dışında çalışsın
            match run_dns(&profile) {
                Ok(()) => Response::Status(status),
                Err(m) => Response::Error { message: m },
            }
        }
        Command::BlockApp { id, path, block } => {
            audit(&format!("block_app {id} = {block}"));
            match run_block(&id, &path, block) {
                Ok(()) => Response::Ok,
                Err(m) => Response::Error { message: m },
            }
        }
        Command::Repair { tool } => {
            // Faz 3: gerçek OS komutları. Reset'ler (winsock/ipreset/dnscache) admin ister;
            // dev'de gömülü (yetkisiz) sunucuda zarif şekilde hata döner, LocalSystem servisinde çalışır.
            match run_repair(&tool) {
                Ok(()) => Response::Ok,
                Err(m) => Response::Error { message: m },
            }
        }
        Command::SetTweak { key, value } => {
            audit(&format!("set_tweak {key} = {value}"));
            match run_tweak(&key, &value) {
                Ok(()) => Response::Ok,
                Err(m) => Response::Error { message: m },
            }
        }
        Command::SetLimit { id, path, down, up } => {
            audit(&format!("set_limit {id} down={down} up={up}"));
            // İNDİRME (ingress): motora ilet (gerçek WinDivert inbound throttle). 0 → kaldır.
            if down > 0 {
                e.limits.insert(id.clone(), (path.clone(), down));
            } else {
                e.limits.remove(&id);
            }
            let ll = e.limit_list();
            e.dpi.set_limits(&ll);
            drop(e); // QoS PowerShell'i kilit DIŞINDA çalıştır (SetDns ile aynı disiplin — diğer komutlar bloke olmasın)
            // YÜKLEME (egress): QoS / NetQosPolicy.
            match run_limit(&id, &path, up) {
                Ok(()) => Response::Ok,
                Err(m) => Response::Error { message: m },
            }
        }
        Command::SetAppModes { modes } => {
            audit(&format!("set_app_modes ({} uygulama)", modes.len()));
            e.app_modes = modes
                .into_iter()
                .map(|(id, mode, path)| (id, (mode, path)))
                .collect();
            // WARP tünelini agregasyona göre senkronize et (idempotent: zaten istenen durumdaysa no-op).
            if e.running {
                if e.want_warp() {
                    if !e.warp.is_running() {
                        if let Err(m) = e.warp.start() {
                            audit(&format!("set_app_modes warp start atlandı: {m}"));
                        }
                    }
                } else if e.warp.is_running() {
                    e.warp.stop();
                }
            }
            Response::Ok
        }
        Command::SetHostlist { domains } => {
            audit(&format!("set_hostlist ({} domain)", domains.len()));
            let was_running = e.running;
            e.hostlist = domains;
            // KESİNTİSİZ hostlist güncelleme: güvenli motorda `start()` handle'ları KAPATMADAN yalnız
            // cfg'yi (strateji+hostlist) atomik değiştirir → KURULU BAĞLANTILAR KOPMAZ. (ESKİ `stop()+start()`
            // hot-reload'u domain-watch her 15 sn çağırınca Discord gateway'ini kopartıyordu — bug.)
            if was_running {
                let strat = engine::strategy_by_id(&e.strategy);
                let hl = e.hostlist.clone();
                match e.dpi.start(&strat, &hl) {
                    Ok(()) => Response::Status(e.status()),
                    Err(m) => Response::Error { message: m },
                }
            } else {
                Response::Ok
            }
        }
    }
}

/// Per-app YÜKLEME (egress) hız sınırı (QoS / NetQosPolicy). NetQosPolicy YALNIZ giden (egress)
/// trafiği kısabilir → burada SADECE `up` uygulanır. İNDİRME (`down`) limiti QoS ile YAPILAMAZ;
/// onu motor (WinDivert inbound throttle, `engine::DpiEngine::set_limits`) gerçek olarak uygular.
/// (Eski kod `down.max(up)` ile bir indirme limitini yanlışlıkla UPLOAD throttle olarak uyguluyordu — bug.)
/// up=0 → politikayı kaldır (sınırsız). id `validate()` ile sanitize edildi.
fn run_limit(id: &str, path: &str, up: u32) -> Result<(), String> {
    let name = format!("evorift-{id}");
    // Eşleştirme tercihen gerçek tam yol (boşluklu/Türkçe adlı exe'ler de eşleşsin); yoksa exe adına düş.
    let app = if path.is_empty() { format!("{id}.exe") } else { path.to_string() };
    let bps = (up as u64 * 1000).to_string(); // kbps → bit/sn (yalnız egress)
    // STATİK script; değerler $env ile gelir (komut satırında DEĞİL → interpolasyon/enjeksiyon yok, review/P7 A5).
    let script = "$ErrorActionPreference='SilentlyContinue'; \
        $n=$env:EVORIFT_QOS_NAME; $a=$env:EVORIFT_QOS_APP; $b=[int64]$env:EVORIFT_QOS_BPS; \
        Remove-NetQosPolicy -Name $n -Confirm:$false; \
        if($b -gt 0){ New-NetQosPolicy -Name $n -AppPathNameMatchCondition $a -ThrottleRateActionBitsPerSecond $b -Confirm:$false }";
    run_os_env(
        "powershell",
        &["-NoProfile", "-Command", script],
        &[
            ("EVORIFT_QOS_NAME", name.as_str()),
            ("EVORIFT_QOS_APP", app.as_str()),
            ("EVORIFT_QOS_BPS", bps.as_str()),
        ],
    )
}

/// Per-app firewall engeli (Faz 4.5). `netsh advfirewall` ARGV ile (shell yok → komut enjeksiyonu yok);
/// program yolu `list_apps`'ten gelir (gerçek süreç). block=false → kuralı kaldır.
/// Gerçek mutasyon yalnız elevated serviste (`run_os` privileged kapısı); dev'de `(sim)`.
fn run_block(id: &str, path: &str, block: bool) -> Result<(), String> {
    let rule = format!("name=evorift-block-{id}");
    // idempotent: önce var olan kuralı kaldır (yoksa hatayı yut)
    let _ = run_os("netsh", &["advfirewall", "firewall", "delete", "rule", &rule]);
    if !block {
        return Ok(());
    }
    if path.is_empty() {
        return Err("uygulama yolu yok — firewall kuralı eklenemedi".into());
    }
    let prog = format!("program={path}");
    run_os("netsh", &["advfirewall", "firewall", "add", "rule", &rule, "dir=out", "action=block", &prog, "enable=yes"])?;
    if let Err(m) = run_os("netsh", &["advfirewall", "firewall", "add", "rule", &rule, "dir=in", "action=block", &prog, "enable=yes"]) {
        // atomiklik: in kuralı başarısızsa out kuralını geri al → tek-yönlü engel kalmasın (review LOW-5)
        let _ = run_os("netsh", &["advfirewall", "firewall", "delete", "rule", &rule]);
        return Err(m);
    }
    Ok(())
}

/// Süreç yönetici (admin/elevated) olarak mı çalışıyor?
#[cfg(windows)]
fn is_elevated() -> bool {
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
fn is_elevated() -> bool {
    false
}

/// Ayrıcalıklı mı? Gerçek OS komutları yalnız: LocalSystem `evorift-svc` (env) VEYA admin süreç.
/// Aksi halde (yetkisiz dev) simüle edilir — UI asla zorla ayrıcalıklı iş yapmaz (docs/05 §5).
fn privileged() -> bool {
    std::env::var("EVORIFT_PRIVILEGED").is_ok() || is_elevated()
}

/// Konsol penceresi AÇMADAN OS komutu nesnesi (CREATE_NO_WINDOW) — netsh/reg/powershell
/// her çağrıda siyah pencere çaktırmasın (admin serviste fark edilir).
#[cfg(windows)]
fn os_command(program: &str) -> OsCommand {
    use std::os::windows::process::CommandExt;
    let mut c = OsCommand::new(program);
    c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    c
}
#[cfg(not(windows))]
fn os_command(program: &str) -> OsCommand {
    OsCommand::new(program)
}

/// Bir OS komutunu çalıştır (audit log'lu).
fn run_os(program: &str, args: &[&str]) -> Result<(), String> {
    run_os_env(program, args, &[])
}

/// run_os gibi ama ek ENV değişkenleriyle: kullanıcı-etkili değerleri komut satırına KOYMADAN
/// PowerShell'e `$env:...` ile geçir → interpolasyon yok, komut enjeksiyonu yüzeyi yok (review/P7 A5).
fn run_os_env(program: &str, args: &[&str], envs: &[(&str, &str)]) -> Result<(), String> {
    if !privileged() {
        audit(&format!("(sim) {program} {}", args.join(" ")));
        return Ok(()); // gömülü dev sunucusu: gerçek mutasyon yok
    }
    audit(&format!("exec {program} {}", args.join(" ")));
    let mut cmd = os_command(program);
    cmd.args(args);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("{program} çalıştırılamadı: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(format!(
            "{program} hata ({:?}): {}",
            out.status.code(),
            err.trim()
        ))
    }
}

fn svec(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

/// Secure DNS: sistem DNS'ini sağlayıcıya çevir + DoH etkinleştir (docs/03 §2).
/// IP'ler + DoH şablonları `docs/12` DNS workflow'unda adversarial doğrulandı.
/// Tek PowerShell çağrısı (yalnız tek-tırnak → Rust arg kaçışı sorunsuz):
///  - aktif fiziksel adapter'larda IPv4 sonra IPv6 DNS ayarla
///  - Win11 22H2+ (build ≥ 22621) ise `Add-DnsClientDohServerAddress`, değilse `netsh dns add encryption` ile DoH
/// Gerçek mutasyon yalnız ayrıcalıklı serviste (`run_os` privileged kapısı).
fn run_dns(profile: &str) -> Result<(), String> {
    // "auto" → sistem DNS'ini DHCP'ye (otomatik) sıfırla. Sonunda DNS önbelleğini temizle: aksi halde
    // eski (zehirli/engelli) kayıtlar TTL dolana kadar kalır → DNS değişimi "etki etmemiş" gibi görünür.
    if profile == "auto" {
        // §5: TÜM fiziksel adaptörler (Up VE Down) — set ederken kapsadığımız her adaptörü geri al.
        let script = "$ErrorActionPreference='SilentlyContinue'; \
            Get-NetAdapter -Physical | ForEach-Object { \
            Set-DnsClientServerAddress -InterfaceIndex $_.InterfaceIndex -ResetServerAddresses -Confirm:$false }; \
            Clear-DnsClientCache";
        return run_os("powershell", &["-NoProfile", "-Command", script]);
    }
    let (v4, v6, tpl): (&[&str], &[&str], &str) = match profile {
        "cloudflare" => (
            &["1.1.1.1", "1.0.0.1"],
            &["2606:4700:4700::1111", "2606:4700:4700::1001"],
            "https://cloudflare-dns.com/dns-query",
        ),
        "quad9" => (
            &["9.9.9.9", "149.112.112.112"],
            &["2620:fe::fe", "2620:fe::9"],
            "https://dns.quad9.net/dns-query",
        ),
        "adguard" => (
            &["94.140.14.14", "94.140.15.15"],
            &["2a10:50c0::ad1:ff", "2a10:50c0::ad2:ff"],
            "https://dns.adguard-dns.com/dns-query",
        ),
        "google" => (
            &["8.8.8.8", "8.8.4.4"],
            &["2001:4860:4860::8888", "2001:4860:4860::8844"],
            "https://dns.google/dns-query",
        ),
        other => return Err(format!("bilinmeyen dns profili: {other}")),
    };
    let ps_list = |a: &[&str]| a.iter().map(|s| format!("'{s}'")).collect::<Vec<_>>().join(",");
    // §5: TÜM fiziksel adaptörler (Up VE Down → reboot/uyku sonrası bağlanan adaptör de güvenli DNS alır).
    // v4+v6 TEK Set-DnsClientServerAddress çağrısında: `-AddressFamily` GEÇERLİ BİR PARAMETRE DEĞİL → eski
    // ikinci çağrı her seferinde sessizce hata yutuyordu, IPv6 DNS hiç set edilmiyordu (IPv6 sızıntısı).
    // Birleşik dizi: @($v4) + @($v6). DoH şablonu Win11 22H2+ (build≥22621)'de, eskide netsh ile.
    let script = format!(
        "$ErrorActionPreference='SilentlyContinue'; $dns=@({v4}) + @({v6}); $tpl='{tpl}'; \
         $ad=@(Get-NetAdapter -Physical); \
         foreach($a in $ad){{ Set-DnsClientServerAddress -InterfaceIndex $a.InterfaceIndex -ServerAddresses $dns -Confirm:$false }}; \
         $b=[System.Environment]::OSVersion.Version.Build; \
         if($b -ge 22621){{ foreach($ip in @({v4})){{ Add-DnsClientDohServerAddress -ServerAddress $ip -DohTemplate $tpl -AllowFallbackToUdp $false -AutoUpgrade $true -Confirm:$false }} }} \
         else {{ foreach($ip in @({v4})){{ netsh dns add encryption server=$ip dohtemplate=$tpl autoupgrade=yes | Out-Null }} }}; \
         Clear-DnsClientCache",
        v4 = ps_list(v4),
        v6 = ps_list(v6),
        tpl = tpl,
    );
    run_os("powershell", &["-NoProfile", "-Command", &script])
}

/// Sistem/ağ tweak'leri (docs/04). Komutlar `docs/12` workflow'unda adversarial doğrulandı.
/// `value`: bool tweak'lerde "on"/"off"; autotuning normal/disabled; congestion cubic/ctcp/bbr2; mtu sayı.
/// Enumerasyon gereken (per-adapter/GUID) tweak'ler tek `powershell -Command` içinde çalışır.
fn run_tweak(key: &str, value: &str) -> Result<(), String> {
    let on = value == "on";
    // PowerShell yardımcıları (raw string → ters-bölü/tırnak kaçışı yok)
    let ps_nagle_on = r#"Get-NetAdapter | ForEach-Object { $p = "HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\$($_.InterfaceGuid)"; if (Test-Path $p) { Set-ItemProperty -Path $p -Name TcpAckFrequency -Value 1 -Type DWord; Set-ItemProperty -Path $p -Name TCPNoDelay -Value 1 -Type DWord } }"#;
    let ps_nagle_off = r#"Get-NetAdapter | ForEach-Object { $p = "HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\$($_.InterfaceGuid)"; if (Test-Path $p) { Remove-ItemProperty -Path $p -Name TcpAckFrequency -ErrorAction SilentlyContinue; Remove-ItemProperty -Path $p -Name TCPNoDelay -ErrorAction SilentlyContinue } }"#;
    // NDIS standardı: *EEE / *PowerSavingMode için 0=Disabled, 1=Enabled.
    // "NIC güç yönetimini kapat" ON → güç tasarrufu özelliklerini KAPAT → değer 0.
    // (Sürücüye göre değişebilir; gerçek NIC'te doğrulanmalı — bkz. docs/12.)
    let ps_nicpower = |v: i32| -> String {
        format!("$ErrorActionPreference='SilentlyContinue'; Get-NetAdapter | ForEach-Object {{ Set-NetAdapterAdvancedProperty -Name $_.Name -RegistryKeyword '*PowerSavingMode' -RegistryValue {v}; Set-NetAdapterAdvancedProperty -Name $_.Name -RegistryKeyword '*EEE' -RegistryValue {v} }}")
    };
    // -ErrorAction SilentlyContinue: desteklemeyen adapter'larda sessiz geç (yine de exit 0 döner,
    // ama hata stderr'i kirletmez). Bkz. docs/12 review bulgusu.
    let ps_rss = |verb: &str| format!("$ErrorActionPreference='SilentlyContinue'; Get-NetAdapter -Physical | ForEach-Object {{ {verb}-NetAdapterRss -Name $_.Name -Confirm:$false }}");
    let ps_offload = |verb: &str| format!("$ErrorActionPreference='SilentlyContinue'; Get-NetAdapter -Physical | Where-Object {{ $_.Status -eq 'Up' }} | ForEach-Object {{ {verb}-NetAdapterLso -Name $_.Name -IPv4 -IPv6; {verb}-NetAdapterChecksumOffload -Name $_.Name }}");
    let ps_mtu = format!("Get-NetAdapter -Physical | Where-Object {{ $_.Status -eq 'Up' }} | ForEach-Object {{ netsh interface ipv4 set subinterface \"$($_.Name)\" mtu={value} store=persistent }}");

    let cmds: Vec<(&str, Vec<String>)> = match key {
        // 🟢 statik, güvenli
        // Etiket "TCP heuristics'i kapat" → toggle ON = heuristics DISABLED (kasıtlı ters eşleme).
        "heuristics" => vec![(
            "netsh",
            svec(&["int", "tcp", "set", "heuristics", if on { "disabled" } else { "enabled" }]),
        )],
        "throttleIdx" => vec![(
            "reg",
            svec(&[
                "add", r"HKLM\SYSTEM\CurrentControlSet\Services\Multimedia\SystemProfile",
                "/v", "NetworkThrottlingIndex", "/t", "REG_DWORD",
                "/d", if on { "4294967295" } else { "10" }, "/f",
            ]),
        )],
        "highPerf" => vec![(
            "powercfg",
            svec(&[
                "/setactive",
                if on { "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c" } else { "381b4222-f694-41f0-9685-ff5bb260df2e" },
            ]),
        )],
        "autotuning" => vec![(
            "netsh",
            svec(&["int", "tcp", "set", "global", &format!("autotuninglevel={value}")]),
        )],
        "congestion" => vec![(
            // 'set supplemental' zorunlu template parametresi ister → "custom" (review bulgusu).
            "netsh",
            svec(&["int", "tcp", "set", "supplemental", "custom", &format!("congestionprovider={value}")]),
        )],
        "rsc" => vec![(
            "netsh",
            svec(&["int", "tcp", "set", "global", if on { "rsc=enabled" } else { "rsc=disabled" }]),
        )],
        // 🟡/🔴 per-adapter enumerasyon (tek powershell çağrısı içinde)
        "nagle" => vec![(
            "powershell",
            svec(&["-NoProfile", "-Command", if on { ps_nagle_on } else { ps_nagle_off }]),
        )],
        "nicPower" => vec![(
            "powershell",
            svec(&["-NoProfile", "-Command", &ps_nicpower(if on { 0 } else { 1 })]),
        )],
        "rss" => vec![(
            "powershell",
            svec(&["-NoProfile", "-Command", &ps_rss(if on { "Enable" } else { "Disable" })]),
        )],
        "offload" => vec![(
            "powershell",
            svec(&["-NoProfile", "-Command", &ps_offload(if on { "Enable" } else { "Disable" })]),
        )],
        "mtu" => vec![("powershell", svec(&["-NoProfile", "-Command", &ps_mtu]))],
        other => return Err(format!("bilinmeyen tweak: {other}")),
    };

    for (prog, args) in &cmds {
        let argrefs: Vec<&str> = args.iter().map(String::as_str).collect();
        run_os(prog, &argrefs)?;
    }
    Ok(())
}

/// Ağ Onar araçları (docs/03 §9, docs/04). Beyaz-liste `ipc::validate()` ile zaten doğrulandı.
fn run_repair(tool: &str) -> Result<(), String> {
    match tool {
        "flushdns" => run_os("ipconfig", &["/flushdns"]),
        "registerdns" => run_os("ipconfig", &["/registerdns"]),
        "dnscache" => run_os(
            "powershell",
            &["-NoProfile", "-Command", "Restart-Service Dnscache -Force"],
        ),
        "renew" => run_os("ipconfig", &["/release"]).and_then(|_| run_os("ipconfig", &["/renew"])),
        "winsock" => run_os("netsh", &["winsock", "reset"]),
        "ipreset" => run_os("netsh", &["int", "ip", "reset"]),
        // Ağ kartını yeniden başlat (kullanıcı onaylı; kısa internet kesintisi). Restart-NetAdapter
        // kullanıcı-modu cmdlet'idir (kernel değil) → güvenli; yalnız Up fiziksel adapter'lara uygulanır.
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

/// Sabit-zamanlı token karşılaştırma (P7 A2): `==` ilk farklı baytta kısa-devre yapar →
/// teorik zamanlama yan-kanalı. Uzunluk gizli değil (token sabit 32 hex) → erken dönüş sorun
/// değil; eşit uzunlukta TÜM baytlar taranır, `black_box` ile kısa-devre optimizasyonu engellenir.
/// NOT: mevcut ACL modelinde token zaten IU-okunur olduğundan marjinal; pipe ACL fail-open olur
/// veya token ACL ileride tek SID'e daraltılırsa derinlemesine savunma sağlar.
fn ct_token_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    core::hint::black_box(diff) == 0
}

fn handle_conn(conn: Stream, engine: Arc<Mutex<Engine>>, token: String) {
    let mut reader = BufReader::new(&conn);

    // 1) handshake — ilk mesaj geçerli token'lı Hello olmalı
    match ipc::read_msg::<Request>(&mut reader) {
        Ok(Request::Hello { token: t }) if ct_token_eq(&t, &token) => {
            if ipc::write_msg(&conn, &Response::Ok).is_err() {
                return;
            }
        }
        _ => {
            let _ = ipc::write_msg(
                &conn,
                &Response::Error {
                    message: "handshake reddedildi".into(),
                },
            );
            audit("REJECT handshake");
            return;
        }
    }

    // 2) komut döngüsü (veya telemetri aboneliği)
    loop {
        let req = match ipc::read_msg::<Request>(&mut reader) {
            Ok(r) => r,
            Err(_) => break, // bağlantı kapandı
        };
        match req {
            Request::Command { cmd } => {
                // §6: dispatch içindeki bir panik (motor / OS komutu) ASLA bu bağlantıyı veya servisi
                // düşürmesin → catch_unwind ile yakala, istemciye nazik hata dön. panic=unwind (release
                // profilinde panic=abort YOK) olduğundan catch_unwind paniği gerçekten yakalar.
                let resp = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| dispatch(&engine, cmd)))
                    .unwrap_or_else(|_| {
                        audit("PANIC dispatch içinde — kurtarıldı");
                        Response::Error { message: "iç hata (kurtarıldı)".into() }
                    });
                if ipc::write_msg(&conn, &resp).is_err() {
                    break;
                }
            }
            Request::Subscribe => {
                telemetry_loop(&conn, &engine); // bağlantı kapanana kadar akıtır
                break;
            }
            Request::Hello { .. } => {
                let _ = ipc::write_msg(
                    &conn,
                    &Response::Error {
                        message: "beklenmeyen ikinci handshake".into(),
                    },
                );
                break;
            }
        }
    }
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// GERÇEK ağ sayaçları: tüm Up durumdaki Ethernet(6)/Wi-Fi(71) arayüzlerinin toplam (alınan, gönderilen)
/// baytı. GetIfTable2 (iphlpapi) — loopback/sanal/filtre arayüzleri sayılmaz (çift sayım önlenir).
#[cfg(windows)]
fn read_net_octets() -> (u64, u64) {
    use windows_sys::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2};
    let mut rx: u64 = 0;
    let mut tx: u64 = 0;
    unsafe {
        let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
        if GetIfTable2(&mut table) != 0 || table.is_null() {
            return (0, 0);
        }
        let n = (*table).NumEntries as usize;
        let rows = (*table).Table.as_ptr();
        for i in 0..n {
            let row = &*rows.add(i);
            // OperStatus 1 = IfOperStatusUp; Type 6 = Ethernet, 71 = IEEE 802.11 (Wi-Fi)
            if row.OperStatus == 1 && (row.Type == 6 || row.Type == 71) {
                rx = rx.saturating_add(row.InOctets);
                tx = tx.saturating_add(row.OutOctets);
            }
        }
        FreeMibTable(table as *const core::ffi::c_void);
    }
    (rx, tx)
}
#[cfg(not(windows))]
fn read_net_octets() -> (u64, u64) {
    (0, 0)
}

/// GERÇEK gecikme: 1.1.1.1:443'e TCP el-sıkışma RTT'si (ms). Başarısızlıkta (false, 0).
/// ICMP raw-socket gerektirmez; TCP connect ağ gidiş-dönüşünü (SYN→SYN/ACK) gerçek ölçer.
fn tcp_ping() -> (bool, u32) {
    use std::net::TcpStream;
    use std::time::Instant;
    let addr = match "1.1.1.1:443".parse() {
        Ok(a) => a,
        Err(_) => return (false, 0),
    };
    let t = Instant::now();
    match TcpStream::connect_timeout(&addr, Duration::from_millis(1000)) {
        Ok(_) => (true, t.elapsed().as_millis().min(u32::MAX as u128) as u32),
        Err(_) => (false, 0),
    }
}

/// Telemetri akışı: ~1 Hz **GERÇEK** `Metrics` yollar (rastgele DEĞİL, docs/05 §2):
///  - down/up: GetIfTable2 bayt sayaçlarının ~1 sn'lik deltası → Mbps.
///  - ping: 1.1.1.1:443 TCP RTT · jitter: ardışık ping farkı · loss: son ~20 denemenin başarısız oranı (%).
/// Bağlantı kapanınca (write hatası) sonlanır. Koruma kapalıyken sıfır gönderir (UI yalnız açıkken gösterir).
fn telemetry_loop(conn: &Stream, engine: &Arc<Mutex<Engine>>) {
    use std::collections::VecDeque;
    let mut last_ping: u32 = 0;
    let mut loss_win: VecDeque<bool> = VecDeque::with_capacity(20);
    let mut prev = read_net_octets();
    let mut prev_t = std::time::Instant::now();
    loop {
        std::thread::sleep(Duration::from_secs(1));
        // §6: poison-safe (dispatch zehirleyebilir) — telemetri thread'i ASLA panik etmesin.
        let running = engine.lock().unwrap_or_else(|p| p.into_inner()).running;
        if !running {
            // taban çizgisini taze tut → koruma açılınca ilk örnek devasa delta olmasın
            prev = read_net_octets();
            prev_t = std::time::Instant::now();
            if ipc::write_msg(conn, &Response::Telemetry(Metrics::default())).is_err() {
                break;
            }
            continue;
        }

        // ---- gerçek throughput (bayt sayaç deltası → Mbps) ----
        let now = read_net_octets();
        let now_t = std::time::Instant::now();
        let secs = now_t.duration_since(prev_t).as_secs_f64().max(0.001);
        let drx = now.0.saturating_sub(prev.0);
        let dtx = now.1.saturating_sub(prev.1);
        prev = now;
        prev_t = now_t;
        let down = round1((drx as f64 * 8.0) / 1_000_000.0 / secs);
        let up = round1((dtx as f64 * 8.0) / 1_000_000.0 / secs);

        // ---- gerçek ping / jitter / loss ----
        let (ok, rtt) = tcp_ping();
        if loss_win.len() == 20 {
            loss_win.pop_front();
        }
        loss_win.push_back(ok);
        let fails = loss_win.iter().filter(|s| !**s).count();
        let loss = round1(fails as f64 * 100.0 / loss_win.len().max(1) as f64);
        let ping = if ok { rtt } else { last_ping };
        let jitter = if ok && last_ping > 0 {
            (ping as i64 - last_ping as i64).unsigned_abs() as u32
        } else {
            0
        };
        if ok {
            last_ping = ping;
        }

        let m = Metrics { running: true, ping, jitter, loss, down, up };
        if ipc::write_msg(conn, &Response::Telemetry(m)).is_err() {
            break;
        }
    }
}

/// Token üret (16 bayt hex) ve dosyaya yaz; başarısız olsa da token'ı döndür.
pub fn ensure_token() -> String {
    let mut bytes = [0u8; 16];
    if getrandom::getrandom(&mut bytes).is_err() {
        // çok düşük olasılık; zaman tabanlı zayıf yedek
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        bytes[..16].copy_from_slice(&nanos.to_le_bytes());
    }
    let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    // Paylaşılan dizini oluştur (%PROGRAMDATA%\evorift) — yoksa write başarısız olur.
    let _ = std::fs::create_dir_all(ipc::data_dir());
    let _ = std::fs::write(ipc::token_path(), &token);
    // P7 A1 — token dosyasına KORUMALI DACL uygula: yalnız SYSTEM + Administrators yazar,
    // interaktif kullanıcı SALT-OKUR. Aksi halde dosya ProgramData'nın geniş (kullanıcı-yazılabilir)
    // ACL'ini miras alır → normal kullanıcı token'ı ezip/önceden yazıp handshake'i ele geçirebilir.
    // En-iyi-çaba: dev gömülü sunucuda (normal kullanıcı) başarısız olursa logla ve geç; gerçek
    // LocalSystem servisinde her zaman uygulanır (güvenlik için tek önemli yol).
    #[cfg(windows)]
    harden_token_acl(&ipc::token_path());
    token
}

/// P7 A1 — token dosyasının DACL'ini sıkılaştır (SYSTEM=Full, Administrators=Full,
/// Interaktif kullanıcı=Read). PROTECTED bayrağı ProgramData'dan miras gelen geniş
/// (CREATOR/Users yazma) ACE'lerini söker → yalnız bu üç giriş kalır. Pipe ACL'iyle (serve_blocking)
/// aynı SDDL deseni ama dosya hak maskeleri (FA/FR) + korumalı DACL.
#[cfg(windows)]
fn harden_token_acl(path: &std::path::Path) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{LocalFree, BOOL, HLOCAL};
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SetNamedSecurityInfoW, SDDL_REVISION_1,
        SE_FILE_OBJECT,
    };
    use windows_sys::Win32::Security::{
        GetSecurityDescriptorDacl, ACL, DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
        PSECURITY_DESCRIPTOR,
    };

    // SDDL: P=protected (mirası kes), AI=auto-inherited bayrağı; SY/BA=Full (FA), IU=Read (FR).
    // Everyone/Anonymous KASITLI YOK; normal kullanıcıya YAZMA YOK.
    let sddl: Vec<u16> = "D:PAI(A;;FA;;;SY)(A;;FA;;;BA)(A;;FR;;;IU)\0".encode_utf16().collect();
    // Yol → geniş NUL-sonlu (boşluklu/Türkçe ProgramData yolları için kayıpsız).
    let mut wpath: Vec<u16> = path.as_os_str().encode_wide().collect();
    wpath.push(0);

    unsafe {
        let mut psd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut psd,
            std::ptr::null_mut(),
        ) == 0
        {
            audit("UYARI: token SDDL çözümlenemedi (ACL atlandı)");
            return;
        }
        // SD'den DACL'i çıkar.
        let mut dacl_present: BOOL = 0;
        let mut dacl_defaulted: BOOL = 0;
        let mut pdacl: *mut ACL = std::ptr::null_mut();
        let got = GetSecurityDescriptorDacl(psd, &mut dacl_present, &mut pdacl, &mut dacl_defaulted);
        if got != 0 && dacl_present != 0 {
            // Dosyaya KORUMALI DACL yaz (miras sökülür, yalnız 3 ACE kalır).
            let rc = SetNamedSecurityInfoW(
                wpath.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(), // owner değişmesin
                std::ptr::null_mut(), // group değişmesin
                pdacl,
                std::ptr::null(), // SACL değişmesin
            );
            if rc != 0 {
                audit(&format!("UYARI: token ACL ayarlanamadı (Win32 {rc})"));
            }
        } else {
            audit("UYARI: token DACL alınamadı (ACL atlandı)");
        }
        // ConvertString... LocalAlloc ile ayırdı → serbest bırak.
        LocalFree(psd as HLOCAL);
    }
}

/// Named-pipe sunucusunu çalıştır (bloklar). Servis ikilisi ve dev'de gömülü sunucu kullanır.
pub fn serve_blocking() -> io::Result<()> {
    // Önce pipe'ı sahiplen; ancak sahiplenince token üret (başka bir servis
    // çalışıyorsa create_sync hata verir ve token dosyasını EZMEYİZ).
    let name = PIPE_NAME
        .to_ns_name::<GenericNamespaced>()
        .map_err(io::Error::other)?;
    let opts = ListenerOptions::new().name(name);
    // P7 A0 — pipe SDDL ACL'i: LocalSystem servisi pipe'ı açtığında VARSAYILAN güvenlik tanımı
    // normal (non-admin) kullanıcının bağlanmasını ENGELLER ("Access is denied"). Açıkça izin ver:
    //   SY (LocalSystem) + BA (Administrators) = full; IU (interaktif kullanıcı) = read+write (bağlan).
    // Everyone/Anonymous kasıtlı YOK. SD set edilemezse (eski Windows/hata) yine de aç (dev'de gömülü
    // sunucu kullanıcı token'ıyla açar → zaten erişilebilir; sadece LocalSystem servisinde ACL şart).
    #[cfg(windows)]
    let opts = {
        use interprocess::os::windows::local_socket::ListenerOptionsExt;
        use interprocess::os::windows::security_descriptor::SecurityDescriptor;
        use widestring::U16CString;
        match U16CString::from_str("D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)")
            .ok()
            .and_then(|s| SecurityDescriptor::deserialize(&s).ok())
        {
            Some(sd) => opts.security_descriptor(sd),
            None => {
                audit("UYARI: pipe ACL ayarlanamadı, varsayılan SD ile açılıyor");
                opts
            }
        }
    };
    let listener = opts.create_sync()?;
    let token = ensure_token();
    let engine = Arc::new(Mutex::new(Engine::new()));
    audit("listening");

    // Servis başlar başlamaz korumayı OTOMATİK aç (çekirdek hostlist ile) → "servis çalışıyor = korumalı".
    // Reboot sonrası, UI hiç açılmasa veya autostart kapalı olsa bile Discord/Roblox/YouTube çalışır
    // (eski davranışta motor pasif başlıyor, açılması UI'nin Start göndermesine bağlıydı → reboot sonrası
    // "Discord açılmıyor" şikâyetinin asıl nedeni buydu). UI bağlanınca kullanıcının TAM site listesini
    // SetHostlist ile üstüne ekler; kullanıcı UI'dan kapatabilir (sonraki servis restart'ına dek kapalı).
    // WinDivert açılamazsa (beklenmez; LocalSystem yetkili) running=false kalır → fail-safe, internet açık.
    // §6: boot auto-protect bloğunu catch_unwind ile sar — buradaki bir panik (motor/OS) servisi
    // (pipe döngüsünü) ASLA başlamadan düşürmesin; en kötü ihtimalle koruma kapalı başlar (fail-safe).
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
        e.strategy = "auto".into();
        e.dns = "cloudflare".into();
        e.hostlist = DEFAULT_HOSTLIST.iter().map(|s| s.to_string()).collect();
        let strat = engine::strategy_by_id(&e.strategy);
        let hl = e.hostlist.clone();
        match e.dpi.start(&strat, &hl) {
            Ok(()) => {
                e.running = true;
                // Motor B (WARP): boot anında UI bağlanmamıştır → app_modes BOŞ → want_warp()=true →
                // güvenli varsayılan açık (masaüstü Discord ses dahil hemen bağlansın). UI bağlanınca
                // SetAppModes ile gerçek tercih gelir, gerekirse WARP kapatılır.
                if e.want_warp() {
                    if let Err(m) = e.warp.start() {
                        audit(&format!("auto-start warp atlandı: {m}"));
                    }
                } else {
                    e.warp.stop(); // idempotent: tünel yoksa no-op; varsa kaldır → saf winws
                }
                audit("auto-start ok (boot koruması açık)");
            }
            Err(m) => audit(&format!("auto-start başarısız (UI Start gönderene dek kapalı): {m}")),
        }
        drop(e); // kilidi bırak → DNS OS komutu kilit DIŞINDA
        // §5: boot'ta UI YOK → güvenli DNS + DoH'u BURADA uygula (tüm fiziksel adaptörler). best-effort.
        if let Err(m) = run_dns("cloudflare") {
            audit(&format!("auto-start DNS uygulanamadı: {m}"));
        }
    }));

    // winws watchdog (dayanıklılık): koruma AÇIKKEN winws ölürse (WinDivert hiccup / yabancı sürücü
    // çakışması / geçici hata) otomatik yeniden başlat. `dpi.start()` idempotent: winws canlıysa no-op,
    // ölmüşse WinDivert'i temizleyip respawn eder. Böylece "bir kez öldü → kalıcı bozuk" durumu olmaz
    // (net3'teki foreground supervisor'ın servis içi karşılığı). WARP tüneli düşmüşse onu da yeniden kur.
    //
    // EK: per-app "off" mod gerçek-zamanlı PID exclusion. Her tur "off" uygulamaların exe yollarını
    // pid_scan ile çalışan PID'lerin TCP/UDP source port'larına çevir → engine.set_exclusion. Liste
    // değişirse engine winws'i düşürür, bir sonraki idempotent start yeni args'la respawn eder. Aynı
    // liste → no-op (gereksiz restart yok). Pahalı PowerShell taramayı kilit DIŞINDA yap.
    {
        let engine = Arc::clone(&engine);
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(5));

            // 1) Kilit içinde: off uygulamalarının yollarını ve "running" durumunu al, hemen bırak.
            let (running, off_paths) = {
                let e = engine.lock().unwrap_or_else(|p| p.into_inner());
                (e.running, e.off_app_paths())
            };

            // 2) Kilit DIŞINDA: pahalı PowerShell PID taramasını yap (kilit asıldığında dispatch bloke
            //    olmasın — set_dns / set_app_modes vb. paralel girişler bekleyemez).
            let excl = if running && !off_paths.is_empty() {
                crate::pid_scan::scan(&off_paths)
            } else {
                crate::pid_scan::ExclusionPorts::default()
            };

            // 3) Kilit içinde: engine durumunu uygula. set_exclusion idempotent (aynı liste → no-op).
            let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
            if e.running {
                e.dpi.set_exclusion(&excl); // değişti ise winws child düşer (kayıp ≤ sonraki start)
                let strat = engine::strategy_by_id(&e.strategy);
                let hl = e.hostlist.clone();
                let _ = e.dpi.start(&strat, &hl); // idempotent (canlıysa dokunmaz, ölmüşse respawn)
                if e.want_warp() && !e.warp.is_running() {
                    let _ = e.warp.start();
                } else if !e.want_warp() && e.warp.is_running() {
                    e.warp.stop(); // kullanıcı tüm uygulamaları off/dpi'a aldıysa tüneli düşür
                }
            }
        });
    }

    for conn in listener.incoming().filter_map(Result::ok) {
        let e = Arc::clone(&engine);
        let tok = token.clone();
        std::thread::spawn(move || handle_conn(conn, e, tok));
    }
    Ok(())
}
