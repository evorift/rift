//! Engine B — Discord için WireGuard split-tunnel (WARP ayarında asıl çözüm). EVORIFT prompt §4.
//!
//! Saf DPI-desync (winws, Motor A) web Discord / Roblox / YouTube'u açar; ama Discord MASAÜSTÜ
//! (Electron) istemcisi agresif-DPI hatlarında "Starting…"de takılır (QUIC'i tercih eder — ISS bunu
//! ICMP-unreachable ile öldürür — ve desync'in bozduğu büyük JS paketlerini çeker). WARP çalışır çünkü
//! HER ŞEYİ tüneller. Bu yüzden YALNIZ Discord'un IP aralıklarını bir Cloudflare WARP WireGuard tüneline
//! yönlendiririz (AllowedIPs ile split-tunnel); geri kalan her şey winws'te kalır. Sonuç: masaüstü
//! uygulaması + ses tam WARP gibi bağlanır, tam-VPN yükü olmadan.
//!
//! Bundle düzeni (`<exe_dizini>\warp\`): `wgcf.exe` (MIT, github.com/ViRb3/wgcf), resmi WireGuard for
//! Windows `wireguard.exe` + `wintun.dll`. Bundle yoksa (dev) her metot loglanan bir no-op'a iner →
//! UI ↔ IPC zinciri yine çalışır; Motor A web'i açık tutar (fail-safe, boot'u ASLA bozma).

/// Yalnız Discord'un IP'leri tünelden geçer (split-tunnel). `0.0.0.0/0` KULLANMA (o tam VPN olur).
/// net3/SOLUTION.md (2026-06-10, Türkcell — uçtan uca CANLI DOĞRULANDI: metin + SES) ile birebir:
///  - `162.159.0.0/16` = Cloudflare edge → Discord API, gateway (gateway.discord.gg), CDN
///    (cdn.discordapp.com), medya (media.discordapp.net, discord.media) — hepsi Cloudflare arkasında.
///  - `66.22.0.0/16`   = Discord'un kendi AS'i (AS49544) → ses/RTC.
///  - `104.29.0.0/16`  = Cloudflare (AS13335) → Discord SES medya sunucuları (gözlenen 104.29.146.98,
///    104.29.147.39). net3 §3.5: ses bu /16'yı eklemeden ÇALIŞMIYORDU (medya UDP'si tünel DIŞINA çıkıp
///    Türkcell tarafından ICMP-unreachable ile düşürülüyordu → "Hat yok"). Tüm /16 tünellenince ses açıldı
///    (rtt ~70 ms, çift yönlü). Eski dar `104.29.146.0/24` başka bölge medya IP'lerini kaçırıyordu.
pub const ALLOWED_IPS: &str = "162.159.0.0/16, 66.22.0.0/16, 104.29.0.0/16";

/// Tam Koruma (full-tunnel): TÜM sistem trafiği WARP'tan geçer (standart WARP/VPN davranışı —
/// per-app DEĞİL). wireguard.exe AllowedIPs=0.0.0.0/0 görünce endpoint (WARP_ENDPOINT) için /32
/// hariç-tutma route'unu OTOMATİK ekler → el-sıkışma paketi fiziksel arayüzden çıkar (routing loop yok).
pub const FULL_ALLOWED_IPS: &str = "0.0.0.0/0, ::/0";

/// Tam Koruma (full-tunnel) tünel DNS'i. KRİTİK: AllowedIPs `/0` → WireGuard-for-Windows OTOMATİK
/// WFP kill-switch kurar ve YALNIZ config'deki `DNS=` sunucularına çözümlemeye izin verir. Full modda
/// `DNS=` YOKSA tüm DNS bloklanır → "bağlı ama internet yok / sayfa açılmıyor". Cloudflare resolver'ları
/// pinlenir (Cloudflare One client'ın davranışı). Split modda DNS YAZILMAZ (sistem DoH yetkili kalır).
pub const WARP_DNS: &str = "1.1.1.1, 1.0.0.1, 2606:4700:4700::1111, 2606:4700:4700::1001";

/// WARP anycast endpoint'i (net3/SOLUTION.md §6.2 ile birebir). KRİTİK: endpoint AllowedIPs aralıklarının
/// DIŞINDA olmalı. wgcf varsayılanı `engage.cloudflareclient.com` 162.159.x'e çözülebilir → o ise
/// `162.159.0.0/16` AllowedIPs'in İÇİNDE kalır → WireGuard kendi tünel paketini tünelden geçirmeye çalışır
/// → ROUTING LOOP, el-sıkışma asla tamamlanmaz. `188.114.98.224` hiçbir tünellenen aralıkta değil → güvenli.
pub const WARP_ENDPOINT: &str = "188.114.98.224:2408";

/// WireGuard tünel adı (servis adı `WireGuardTunnel$warp`; conf dosya tabanı ile birebir olmalı).
pub const TUNNEL_NAME: &str = "warp";

/// Does the WireGuard tunnel SERVICE exist on this machine? 0 = not asked yet, 1 = yes, 2 = no.
///
/// Cached because the answer only changes when WE install or uninstall the tunnel, and asking costs
/// a child process (`sc query`) on a code path — `Command::Stop` — that must feel instant.
#[cfg(windows)]
static TUNNEL_PRESENCE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

#[cfg(windows)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum TunnelPresence {
    Present,
    Absent,
}

// --- wgcf lifecycle (item 4.2): account/profile management, refresh, register-error handling ---

/// wgcf account file (the WARP account) and profile file (full-tunnel raw conf). Both are persisted
/// under `%PROGRAMDATA%\evorift` and ACL-hardened (they carry the private WARP credentials/key).
pub const WGCF_ACCOUNT: &str = "wgcf-account.toml";
pub const WGCF_PROFILE: &str = "wgcf-profile.conf";

/// Cached-profile staleness threshold (wgcf's own weekly refresh cadence, docs/03 §1.1). Refresh is
/// opt-in/manual — we never auto-download (binaries are bundled).
pub const PROFILE_MAX_AGE_DAYS: u64 = 7;

pub fn account_path() -> std::path::PathBuf {
    crate::ipc::data_dir().join(WGCF_ACCOUNT)
}
pub fn profile_path() -> std::path::PathBuf {
    crate::ipc::data_dir().join(WGCF_PROFILE)
}

/// wgcf should `generate` a profile only when one isn't cached (item 4.2: generate-when-missing,
/// reuse-when-present so the keypair stays stable per install).
pub fn should_generate(profile: &std::path::Path) -> bool {
    !profile.exists()
}

/// Is the cached profile older than `max_age_days`? Best-effort (false if mtime unknown). Advisory —
/// WARP profiles don't hard-expire; drives the optional refresh.
pub fn profile_is_stale(profile: &std::path::Path, max_age_days: u64) -> bool {
    match std::fs::metadata(profile).and_then(|m| m.modified()) {
        Ok(mtime) => mtime
            .elapsed()
            .map(|e| e.as_secs() > max_age_days.saturating_mul(86_400))
            .unwrap_or(false),
        Err(_) => false,
    }
}

/// Turn a wgcf register failure into an actionable message. Cloudflare blocks free registration from
/// some IPs/regions as "abusive usage" (docs/03 §1.1) → tell the user to use a DPI engine instead.
pub fn register_error_message(stderr: &str) -> String {
    let lower = stderr.to_lowercase();
    if lower.contains("abusive")
        || lower.contains("access denied")
        || lower.contains("forbidden")
        || lower.contains("429")
    {
        "WARP registration was rejected by Cloudflare (free-tier block on this IP/region). Use a DPI engine instead, or try again later from a different network.".into()
    } else {
        format!("wgcf register failed (network?): {stderr}")
    }
}

/// wgcf'in ürettiği profili split-tunnel'a çevir:
///  1. TÜM `AllowedIPs` satırlarını (tam-tünel `0.0.0.0/0, ::/0`) tek `AllowedIPs = {allowed_ips}` ile değiştir.
///  2. `Endpoint =` satırını sabit `{endpoint}` ile değiştir: wgcf varsayılanı (`engage.cloudflareclient.com`)
///     `162.159.0.0/16` İÇİNE çözülebilir → o AllowedIPs'in içinde kalır → WireGuard kendi handshake paketini
///     tünelden geçirir → ROUTING LOOP. Tünellenen aralıkların DIŞINDAki sabit IP buna engel (net3 §6.2).
///  3. `DNS =` satırını düşür: aksi halde WireGuard-for-Windows tünel açıkken onu SİSTEM çözümleyicisi olarak
///     zorlar ve Cloudflare DoH'umuzu (§5) ezer → sistem DoH yetkili kalsın.
///
/// Saf string işleme → `cargo test` ile doğrulanır.
pub fn split_tunnel_config(profile: &str, allowed_ips: &str, endpoint: &str) -> String {
    let mut out = String::with_capacity(profile.len());
    let mut allowed_written = false;
    let mut endpoint_written = false;
    for line in profile.lines() {
        let lower = line.trim_start().to_ascii_lowercase();
        if lower.starts_with("allowedips") {
            if !allowed_written {
                out.push_str("AllowedIPs = ");
                out.push_str(allowed_ips);
                out.push('\n');
                allowed_written = true;
            }
            continue; // orijinal tam-tünel AllowedIPs satırlarını at
        }
        if lower.starts_with("endpoint") {
            // loop-güvenli sabit endpoint ile değiştir (wgcf'in alan-adı endpoint'i AllowedIPs içine düşebilir)
            out.push_str("Endpoint = ");
            out.push_str(endpoint);
            out.push('\n');
            endpoint_written = true;
            continue;
        }
        if lower.starts_with("dns") {
            continue; // tünel DNS'ini düşür (split: sistem DoH yetkili; full: DNS sonra inject_interface_dns ile pinlenir)
        }
        out.push_str(line);
        out.push('\n');
    }
    if !allowed_written {
        // wgcf her zaman AllowedIPs yazar; yine de savunmacı: hiç yoksa sona ekle ([Peer] son bölümdür).
        out.push_str("AllowedIPs = ");
        out.push_str(allowed_ips);
        out.push('\n');
    }
    if !endpoint_written {
        // savunmacı: profilde Endpoint yoksa (beklenmez) loop-güvenli sabiti yine de ekle.
        out.push_str("Endpoint = ");
        out.push_str(endpoint);
        out.push('\n');
    }
    out
}

/// `[Interface]` bölümünün hemen ardına `DNS = {dns}` satırı ekle (full-tunnel için). split_tunnel_config
/// DNS'i sildiği için full modda bunu ÇAĞIR → kill-switch DNS'i beyaz-listeler (yoksa tüm DNS bloklanır).
/// İdempotent değil; full modda conf üretiminde bir kez çağrılır. DNS [Interface]'de olmalı ([Peer]'da değil).
pub fn inject_interface_dns(conf: &str, dns: &str) -> String {
    let mut out = String::with_capacity(conf.len() + dns.len() + 8);
    let mut injected = false;
    for line in conf.lines() {
        out.push_str(line);
        out.push('\n');
        if !injected && line.trim().eq_ignore_ascii_case("[interface]") {
            out.push_str("DNS = ");
            out.push_str(dns);
            out.push('\n');
            injected = true;
        }
    }
    out
}

// --- MTU clamp + app-based (WireSock-style) tunnel (item 4.3) ---

/// wgcf's default tunnel MTU (the safe value matching the official WARP app). Clamp lower (1200/1180)
/// on restrictive ISPs where large TLS/QUIC flows hang (PMTUD blackholing) but small packets work.
pub const MTU_DEFAULT: u32 = 1280;

/// Set the `MTU =` line in `[Interface]` to `mtu` (replace if present, else inject right after the
/// `[Interface]` header). Pure string transform → unit-tested. Lets restrictive ISPs clamp 1280→1200/1180.
pub fn set_mtu(conf: &str, mtu: u32) -> String {
    let has_mtu = conf.lines().any(|l| l.trim_start().to_ascii_lowercase().starts_with("mtu"));
    let mut out = String::with_capacity(conf.len() + 16);
    for line in conf.lines() {
        if line.trim_start().to_ascii_lowercase().starts_with("mtu") {
            out.push_str(&format!("MTU = {mtu}\n"));
            continue; // drop the original MTU line (replaced)
        }
        out.push_str(line);
        out.push('\n');
        if !has_mtu && line.trim().eq_ignore_ascii_case("[interface]") {
            out.push_str(&format!("MTU = {mtu}\n")); // inject when none existed
        }
    }
    out
}

/// Default app list for app-based (WireSock) tunneling (docs/03 §1.2 — Discord/Roblox + helpers).
pub fn default_tunnel_apps() -> Vec<String> {
    [
        "Discord.exe", "DiscordPTB.exe", "Update.exe", "webcord.exe",
        "RobloxPlayerBeta.exe", "RobloxPlayerInstaller.exe", "discord", "roblox",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Build a WireSock-style **app-based** tunnel conf (item 4.3): full `AllowedIPs` (everything) plus an
/// `AllowedApps = <names>` line so only the listed apps are tunneled (docs/03 §1.2). Used by the WireSock
/// adapter (item 4.4). Endpoint replaced (loop-safe) + DNS dropped — same discipline as `split_tunnel_config`.
/// NOTE: `AllowedApps` is a WireSock extension; the official `wireguard.exe` ignores it (use IP-split there).
pub fn app_tunnel_config(profile: &str, apps: &[String], endpoint: &str) -> String {
    let mut out = split_tunnel_config(profile, FULL_ALLOWED_IPS, endpoint);
    if !apps.is_empty() {
        out.push_str("AllowedApps = ");
        out.push_str(&apps.join(", "));
        out.push('\n');
    }
    out
}

/// Discord'u WARP tüneliyle bağlayan motor. Servis Start/Stop'ta winws'in YANINDA çalıştırır.
/// Hatalar ÖLÜMCÜL DEĞİL (servis loglar + devam eder; winws web'i kapsar).
pub struct WarpEngine {
    running: bool,
    /// Açık tünel full-tunnel (tüm sistem, Tam Koruma) mı yoksa split-tunnel (Discord IP'leri) mı.
    /// Mod değişiminde tünel kaldırılıp doğru conf ile yeniden kurulur.
    full: bool,
}

impl Default for WarpEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl WarpEngine {
    pub fn new() -> Self {
        Self { running: false, full: false }
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    /// True if the tunnel was started in full-tunnel mode (all-system protection).
    pub fn is_full(&self) -> bool {
        self.full
    }

    /// True if the `WireGuardTunnel$warp` Windows service is present (public wrapper for the
    /// private `tunnel_installed()` used by `start()` idempotency check and health queries).
    pub fn is_installed() -> bool {
        #[cfg(windows)]
        return Self::tunnel_installed();
        #[cfg(not(windows))]
        return false;
    }

    /// Seconds since the most recent WireGuard peer handshake, queried from the WireGuard
    /// userspace pipe (`\\.\pipe\ProtectedPrefix\Localsystem\WireGuard\warp`). Returns `None`
    /// when the tunnel is not running, no handshake has occurred yet, or the pipe is inaccessible.
    #[cfg(windows)]
    pub fn handshake_ago_secs() -> Option<u64> {
        use std::io::{Read, Write};
        use std::time::{SystemTime, UNIX_EPOCH};

        let pipe = format!(r"\\.\pipe\ProtectedPrefix\Localsystem\WireGuard\{TUNNEL_NAME}");
        let mut f = std::fs::OpenOptions::new().read(true).write(true).open(&pipe).ok()?;
        f.write_all(b"get=1\n\n").ok()?;
        let mut buf = [0u8; 8192];
        let n = f.read(&mut buf).ok()?;
        let text = std::str::from_utf8(&buf[..n]).ok()?;
        for line in text.lines() {
            if let Some(v) = line.strip_prefix("last_handshake_time_sec=") {
                let epoch: u64 = v.trim().parse().ok()?;
                if epoch == 0 {
                    return None; // no handshake yet (WireGuard reports 0 before the first one)
                }
                let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
                return Some(now.saturating_sub(epoch));
            }
        }
        None
    }
    #[cfg(not(windows))]
    pub fn handshake_ago_secs() -> Option<u64> {
        None
    }

    /// Tüneli aç: config'i garanti et (ilk çalıştırmada wgcf ile üret) → `wireguard.exe
    /// /installtunnelservice`. Idempotent: tünel zaten kuruluysa yeniden kurmaz.
    #[cfg(windows)]
    pub fn start(&mut self, full: bool) -> Result<(), String> {
        let exe = match Self::wireguard_exe() {
            Some(e) if e.exists() => e,
            _ => {
                // Dev/bundle yok: gerçek tünel yok ama UI↔IPC zinciri çalışsın (sim). Hata DÖNDÜRME.
                eprintln!("[evorift][warp] bundle yok (wireguard.exe) — sim (gerçek tünel yok)");
                self.running = true;
        Self::set_tunnel_presence(TunnelPresence::Present);
                Self::set_tunnel_presence(TunnelPresence::Present);
                self.full = full;
                return Ok(());
            }
        };
        // Zaten istenen moddaysa no-op (idempotent, kesintisiz).
        if self.running && self.full == full && Self::tunnel_installed() {
            return Ok(());
        }
        // Mod değişimi (split↔full) veya bayat tünel: önce mevcut tüneli kaldır → doğru conf ile temiz kur.
        if Self::tunnel_installed() {
            let _ = Self::run_hidden(&exe, &["/uninstalltunnelservice".into(), TUNNEL_NAME.into()], None);
        }
        let conf = Self::ensure_config(full)?; // wgcf ile üret/oku (ağ gerekebilir; başarısızsa Err)
        Self::run_hidden(&exe, &["/installtunnelservice".into(), conf.to_string_lossy().into_owned()], None)?;
        self.running = true;
        Self::set_tunnel_presence(TunnelPresence::Present);
        self.full = full;
        Ok(())
    }
    #[cfg(not(windows))]
    pub fn start(&mut self, full: bool) -> Result<(), String> {
        self.running = true;
        Self::set_tunnel_presence(TunnelPresence::Present);
        self.full = full;
        Ok(())
    }

    /// Tüneli kapat: `wireguard.exe /uninstalltunnelservice warp` (best-effort).
    #[cfg(windows)]
    pub fn stop(&mut self) {
        // SPEED (2026-08-16): this used to spawn `wireguard.exe /uninstalltunnelservice`
        // UNCONDITIONALLY, under the engine lock, on every single `Command::Stop` — including the
        // overwhelmingly common case where no tunnel was ever created (the tunnel is opt-in, and VPN
        // mode is not even enabled today). That is a child process with a 10s timeout on the path
        // the user experiences as "turn protection off".
        //
        // Now: ask the OS once per process whether the tunnel service exists at all, remember the
        // answer, and skip both calls when it does not. Correctness is preserved because the cached
        // "absent" answer is only ever set after an uninstall or a negative query, and `start()`
        // resets it — so a tunnel that DOES exist is always still torn down.
        if !self.running && Self::tunnel_state() == TunnelPresence::Absent {
            return;
        }
        if let Some(exe) = Self::wireguard_exe() {
            if exe.exists() {
                let _ = Self::run_hidden(&exe, &["/uninstalltunnelservice".into(), TUNNEL_NAME.into()], None);
            }
        }
        Self::set_tunnel_presence(TunnelPresence::Absent);
        self.running = false;
    }
    #[cfg(not(windows))]
    pub fn stop(&mut self) {
        self.running = false;
    }

    // ---- Windows yardımcıları ----

    /// Bundle dizini: `<exe_dizini>\warp\`.
    #[cfg(windows)]
    fn bundle_dir() -> Option<std::path::PathBuf> {
        Some(std::env::current_exe().ok()?.parent()?.join("warp"))
    }

    #[cfg(windows)]
    fn wireguard_exe() -> Option<std::path::PathBuf> {
        Some(Self::bundle_dir()?.join("wireguard.exe"))
    }

    #[cfg(windows)]
    fn wgcf_exe() -> Option<std::path::PathBuf> {
        Some(Self::bundle_dir()?.join("wgcf.exe"))
    }

    /// Tünel config yolu — TEK dosya (`warp.conf`). KRİTİK: WireGuard servis adı conf DOSYA ADINDAN
    /// türer (`WireGuardTunnel$warp`). Farklı ad (warp-full) → farklı servis → tunnel_installed()/stop()
    /// (TUNNEL_NAME="warp") onu GÖREMEZ → mod değişiminde iki tünel çakışır. Bu yüzden mod ne olursa olsun
    /// aynı dosyaya yazılır; içerik (AllowedIPs) moda göre ensure_config(full) ile YENİDEN üretilir.
    #[cfg(windows)]
    fn conf_path() -> std::path::PathBuf {
        crate::ipc::data_dir().join("warp.conf")
    }

    /// `WireGuardTunnel$warp` servisi kurulu mu? (idempotent start için).
    #[cfg(windows)]
    fn tunnel_installed() -> bool {
        let svc = format!("WireGuardTunnel${TUNNEL_NAME}");
        Self::run_hidden_status("sc", &["query".into(), svc])
    }

    /// Cached answer to "does the WireGuard tunnel service exist", so the hot Stop path does not
    /// spawn `sc query` (let alone `wireguard.exe`) every time.
    #[cfg(windows)]
    fn tunnel_state() -> TunnelPresence {
        match TUNNEL_PRESENCE.load(std::sync::atomic::Ordering::Relaxed) {
            1 => TunnelPresence::Present,
            2 => TunnelPresence::Absent,
            _ => {
                // Unknown → ask the OS exactly once, then remember.
                let present = Self::tunnel_installed();
                let state = if present { TunnelPresence::Present } else { TunnelPresence::Absent };
                Self::set_tunnel_presence(state);
                state
            }
        }
    }

    #[cfg(windows)]
    fn set_tunnel_presence(state: TunnelPresence) {
        let v = match state {
            TunnelPresence::Present => 1u8,
            TunnelPresence::Absent => 2u8,
        };
        TUNNEL_PRESENCE.store(v, std::sync::atomic::Ordering::Relaxed);
    }

    /// warp.conf'u garanti et. Varsa olduğu gibi kullan (per-install özel anahtar kalıcı). Yoksa:
    /// wgcf register (hesap) → wgcf generate (profil) → split-tunnel'a çevir → korumalı ACL ile yaz.
    /// İstenen moda göre tünel config'ini garanti et (varsa olduğu gibi kullan; yoksa wgcf profilinden üret).
    /// `full=true` → AllowedIPs=0.0.0.0/0,::/0 (tüm sistem · Tam Koruma); `false` → Discord split-tunnel.
    /// İstenen moda göre `warp.conf`'u (YENİDEN) üret — split vs full AllowedIPs farklı olduğundan her
    /// (yeniden) kurulumda doğru içerikle yazılır (bayat conf'u REUSE ETME → modlar karışmasın). wgcf
    /// profili (özel anahtar) önbellekli kalır → yalnız ucuz dönüştürme + yazma tekrarlanır.
    #[cfg(windows)]
    fn ensure_config(full: bool) -> Result<std::path::PathBuf, String> {
        let conf = Self::conf_path();
        let profile = Self::ensure_profile()?;
        // İstenen AllowedIPs + loop-güvenli sabit endpoint ile dönüştür → yaz → ACL sıkılaştır.
        let allowed = if full { FULL_ALLOWED_IPS } else { ALLOWED_IPS };
        let mut cfg = split_tunnel_config(&profile, allowed, WARP_ENDPOINT);
        if full {
            // Full-tunnel: tünel DNS'ini [Interface]'e pinle → /0 kill-switch DNS'i beyaz-listeler
            // (yoksa tüm DNS bloklanır → "bağlı ama internet yok"). Cloudflare One client davranışı.
            cfg = inject_interface_dns(&cfg, WARP_DNS);
        }
        std::fs::write(&conf, cfg).map_err(|e| format!("{} yazılamadı: {e}", conf.display()))?;
        Self::harden_conf_acl(&conf);
        Ok(conf)
    }

    /// wgcf profilini garanti et (hesap kaydı + profil üretimi; ikisi de kalıcı/önbellekli — split + full
    /// conf bu TEK profilden türer, böylece anahtar çifti install başına sabit kalır).
    #[cfg(windows)]
    fn ensure_profile() -> Result<String, String> {
        let wgcf = match Self::wgcf_exe() {
            Some(w) if w.exists() => w,
            _ => return Err("wgcf.exe not in bundle → cannot generate WARP config".into()),
        };
        let dir = crate::ipc::data_dir();
        std::fs::create_dir_all(&dir).map_err(|e| format!("data dir create failed: {e}"))?;
        let account = account_path();
        let profile = profile_path();
        // 1) Account: register only when missing (wgcf rejects re-registering an existing account; keep
        //    persistent). On failure classify the error (Cloudflare abusive-usage block → actionable msg).
        //    Harden the account file ACL — it holds the WARP private account.
        if !account.exists() {
            Self::run_hidden(&wgcf, &["register".into(), "--accept-tos".into()], Some(dir.as_path()))
                .map_err(|e| register_error_message(&e))?;
            Self::harden_conf_acl(&account);
        }
        // 2) Profile: generate only when missing (keypair stays stable per install; split + full conf
        //    derive from this one profile). Harden its ACL — it carries the private key.
        if should_generate(&profile) {
            Self::run_hidden(&wgcf, &["generate".into()], Some(dir.as_path()))
                .map_err(|e| format!("wgcf generate failed: {e}"))?;
            Self::harden_conf_acl(&profile);
        }
        std::fs::read_to_string(&profile)
            .map_err(|e| format!("wgcf profile read failed ({}): {e}", profile.display()))
    }

    /// Force a profile refresh (item 4.2): delete the cached profile so the next ensure regenerates it
    /// (the account is kept). Use when the profile is stale (`profile_is_stale`) or broken.
    #[cfg(windows)]
    pub fn refresh_profile() -> Result<(), String> {
        let p = profile_path();
        if p.exists() {
            std::fs::remove_file(&p).map_err(|e| format!("profile delete failed: {e}"))?;
        }
        Self::ensure_profile().map(|_| ())
    }

    /// warp.conf özel anahtar taşır → ACL'i SYSTEM + Administrators FULL'e indir, miras kes. icacls
    /// LOCALE-BAĞIMSIZ SID'lerle (Türkçe Windows'ta "SYSTEM"/"Administrators" adları farklı):
    /// `*S-1-5-18`=SYSTEM, `*S-1-5-32-544`=Administrators. Best-effort (başarısızsa logla, devam et).
    #[cfg(windows)]
    fn harden_conf_acl(conf: &std::path::Path) {
        let p = conf.to_string_lossy().into_owned();
        if !Self::run_hidden_status(
            "icacls",
            &[
                p,
                "/inheritance:r".into(),
                "/grant:r".into(),
                "*S-1-5-18:(F)".into(),
                "*S-1-5-32-544:(F)".into(),
            ],
        ) {
            eprintln!("[evorift][warp] UYARI: warp.conf ACL sıkılaştırılamadı");
        }
    }

    /// Konsol penceresi AÇMADAN komut çalıştır; başarısızsa stderr ile Err döndür.
    /// Her WARP alt-sürecinin ÜST SINIRI. `wgcf register/generate` ağa çıkar (ve bu uygulamanın
    /// hedefi tam da ağın kurcalandığı hatlar), `wireguard.exe /installtunnelservice` ise Windows
    /// servisi kurar — ikisi de gerçek hayatta asılabiliyor.
    /// 10s, NOT longer: WarpEngine::start() makes two of these calls back to back and the watchdog
    /// still holds the engine lock across them, so the worst case must stay well under the IPC
    /// client's 45s ceiling — otherwise a slow tunnel install starves every other command and the
    /// UI reports "servis yanit vermedi" (seen live on 2026-08-15).
    const CHILD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

    /// Alt süreci çalıştır — ZAMAN AŞIMLI.
    ///
    /// FIXED 2026-08-14 (canlı test): eskiden düz `cmd.output()` idi, yani SINIRSIZ bekleme. Bu
    /// fonksiyon `sync_warp()` üzerinden engine mutex'i TUTULURKEN çağrıldığı için, asılan tek bir
    /// çocuk süreç tüm servisi kalıcı olarak kilitliyordu: sonraki her `dispatch()` aynı kilitte
    /// bloke oluyor, IPC istemcisinin de zaman aşımı olmadığı için UI sonsuza dek "Bağlanıyor"da
    /// kalıyordu. Artık süre dolarsa çocuk öldürülür ve hata döner — kilit her hâlükârda bırakılır.
    #[cfg(windows)]
    fn run_hidden(
        program: &std::path::Path,
        args: &[String],
        cwd: Option<&std::path::Path>,
    ) -> Result<(), String> {
        Self::run_hidden_timeout(program, args, cwd, Self::CHILD_TIMEOUT)
    }

    /// `run_hidden`'ın zaman aşımı parametreli hâli — testler kısa bir süre verip öldürme yolunu
    /// gerçekten çalıştırabilsin diye ayrıldı (25 sn bekleyen bir test işe yaramaz).
    #[cfg(windows)]
    fn run_hidden_timeout(
        program: &std::path::Path,
        args: &[String],
        cwd: Option<&std::path::Path>,
        timeout: std::time::Duration,
    ) -> Result<(), String> {
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new(program);
        cmd.args(args)
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        if let Some(d) = cwd {
            cmd.current_dir(d);
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("{} çalıştırılamadı: {e}", program.display()))?;

        let deadline = std::time::Instant::now() + timeout;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {
                    if std::time::Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(format!(
                            "{} {}s içinde yanıt vermedi — süreç sonlandırıldı",
                            program.display(),
                            timeout.as_secs()
                        ));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                Err(e) => return Err(format!("{} beklenemedi: {e}", program.display())),
            }
        }
        // Süreç bitti → boruları okumak artık bloke etmez.
        let out = child
            .wait_with_output()
            .map_err(|e| format!("{} çıktısı okunamadı: {e}", program.display()))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
        }
    }

    /// run_hidden gibi ama yalnız başarı/başarısızlık (bool) döndürür — query/idempotency kontrolleri için.
    #[cfg(windows)]
    fn run_hidden_status(program: &str, args: &[String]) -> bool {
        use std::os::windows::process::CommandExt;
        std::process::Command::new(program)
            .args(args)
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

impl Drop for WarpEngine {
    /// Servis kapanınca tüneli de kaldır (yetim WireGuardTunnel$warp servisi kalmasın).
    fn drop(&mut self) {
        if self.running {
            self.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "[Interface]\nPrivateKey = AAAA\nAddress = 172.16.0.2/32\nAddress = 2606:4700:110:8::/128\nDNS = 1.1.1.1\nMTU = 1280\n\n[Peer]\nPublicKey = bmXOC+F1FxEMF9dyiK2H5/1SUtzH0JuVo51h2wPfgyo=\nAllowedIPs = 0.0.0.0/0\nAllowedIPs = ::/0\nEndpoint = engage.cloudflareclient.com:2408\n";

    /// REGRESSION (2026-08-14 live test): every WARP child process must be bounded.
    ///
    /// `run_hidden` used a plain `cmd.output()` — an UNBOUNDED wait — and is called from
    /// `sync_warp()` while the service holds the engine mutex. One `wireguard.exe` or `wgcf.exe`
    /// that never returned therefore froze the whole service permanently: every later dispatch
    /// blocked on the same lock, and with no client-side timeout the UI hung on "Bağlanıyor"
    /// forever without so much as an error. This drives the real kill path with a short deadline
    /// against a genuinely long-running child, and asserts we give up rather than wait for it.
    #[cfg(windows)]
    #[test]
    fn a_hanging_child_is_killed_at_its_timeout() {
        // ping -n 30 127.0.0.1 ≈ 29s — far longer than the 1s deadline below.
        let ping = std::path::PathBuf::from("ping");
        let t0 = std::time::Instant::now();
        let r = WarpEngine::run_hidden_timeout(
            &ping,
            &["-n".to_string(), "30".to_string(), "127.0.0.1".to_string()],
            None,
            std::time::Duration::from_secs(1),
        );
        let waited = t0.elapsed();

        assert!(r.is_err(), "a child that outlives its deadline must be an error, not a success");
        assert!(
            r.unwrap_err().contains("yanıt vermedi"),
            "the error must say it timed out, so the cause is visible in logs"
        );
        assert!(
            waited < std::time::Duration::from_secs(10),
            "must return at the deadline (~1s), not wait for the child; waited {waited:?}"
        );
    }

    #[test]
    fn split_tunnel_replaces_allowed_ips_endpoint_and_drops_dns() {
        let out = split_tunnel_config(SAMPLE, ALLOWED_IPS, WARP_ENDPOINT);
        // tam-tünel rotaları gitti, tek split-tunnel satırı var
        assert!(!out.contains("0.0.0.0/0"), "full-tunnel route leaked: {out}");
        assert!(!out.contains("::/0"), "full-tunnel v6 route leaked: {out}");
        assert_eq!(out.matches("AllowedIPs").count(), 1, "exactly one AllowedIPs line");
        assert!(out.contains(&format!("AllowedIPs = {ALLOWED_IPS}")));
        assert!(out.contains("104.29.0.0/16"), "Discord voice media /16 (net3 §3.5) must be present");
        // tünel DNS düşürüldü (sistem DoH yetkili kalsın)
        assert!(!out.contains("DNS ="), "tunnel DNS should be stripped: {out}");
        // wgcf'in alan-adı endpoint'i loop-güvenli sabit IP ile değiştirildi (net3 §6.2)
        assert!(!out.contains("engage.cloudflareclient.com"), "wgcf endpoint must be replaced: {out}");
        assert_eq!(out.matches("Endpoint = ").count(), 1, "exactly one Endpoint line");
        assert!(out.contains(&format!("Endpoint = {WARP_ENDPOINT}")));
        // kritik alanlar korundu
        assert!(out.contains("PrivateKey = AAAA"));
        assert!(out.contains("PublicKey = bmXOC+F1FxEMF9dyiK2H5/1SUtzH0JuVo51h2wPfgyo="));
        assert!(out.contains("Address = 172.16.0.2/32"));
    }

    #[test]
    fn split_tunnel_appends_when_missing() {
        let out = split_tunnel_config("[Interface]\nPrivateKey = X\n", ALLOWED_IPS, WARP_ENDPOINT);
        assert!(out.contains("AllowedIPs = 162.159.0.0/16, 66.22.0.0/16"));
        assert!(out.contains(&format!("Endpoint = {WARP_ENDPOINT}")), "endpoint appended when missing");
    }

    /// Loop-güvenlik sigortası: sabit endpoint, tünellenen HİÇBİR aralıkta olmamalı (aksi halde
    /// WireGuard kendi handshake'ini tünelden geçirir → routing loop, tünel asla kurulmaz; net3 §6.2).
    #[test]
    fn warp_endpoint_is_outside_tunneled_ranges() {
        for prefix in ["162.159.", "66.22.", "104.29."] {
            assert!(
                !WARP_ENDPOINT.starts_with(prefix),
                "WARP_ENDPOINT {WARP_ENDPOINT} falls inside AllowedIPs prefix {prefix} → routing loop"
            );
        }
    }

    /// Item 4.2: profile generated when missing, reused when present; fresh file not stale.
    #[test]
    fn should_generate_missing_reuse_present() {
        let p = std::env::temp_dir().join("evorift-test-wgcf-profile.conf");
        let _ = std::fs::remove_file(&p);
        assert!(should_generate(&p), "missing → generate");
        std::fs::write(&p, "[Interface]\nPrivateKey = X\n").unwrap();
        assert!(!should_generate(&p), "present → reuse");
        assert!(!profile_is_stale(&p, PROFILE_MAX_AGE_DAYS), "fresh file is not stale");
        let _ = std::fs::remove_file(&p);
    }

    /// Register failures are classified: Cloudflare abusive-usage block vs generic network error.
    #[test]
    fn register_error_classification() {
        assert!(register_error_message("Error: Access denied (abusive usage)").contains("Cloudflare"));
        assert!(register_error_message("429 Too Many Requests").contains("Cloudflare"));
        assert!(register_error_message("dial tcp: i/o timeout").contains("network"));
    }

    /// Item 4.3: set_mtu replaces an existing MTU line and injects one (inside [Interface]) when missing.
    #[test]
    fn set_mtu_replace_and_inject() {
        let out = set_mtu(SAMPLE, 1200);
        assert!(out.contains("MTU = 1200"));
        assert!(!out.contains("MTU = 1280"));
        assert_eq!(out.matches("MTU = ").count(), 1, "exactly one MTU line");

        let no_mtu = "[Interface]\nPrivateKey = X\n\n[Peer]\nEndpoint = a:1\n";
        let out2 = set_mtu(no_mtu, 1180);
        let iface = out2.find("[Interface]").unwrap();
        let mtu = out2.find("MTU = 1180").unwrap();
        let peer = out2.find("[Peer]").unwrap();
        assert!(iface < mtu && mtu < peer, "injected MTU sits inside [Interface]");
    }

    /// app-based tunnel conf carries full AllowedIPs + an AllowedApps line (WireSock-style, item 4.3).
    #[test]
    fn app_tunnel_config_has_apps_and_full_ips() {
        let apps = vec!["Discord.exe".to_string(), "roblox".to_string()];
        let out = app_tunnel_config(SAMPLE, &apps, WARP_ENDPOINT);
        assert!(out.contains("AllowedApps = Discord.exe, roblox"));
        assert!(out.contains("0.0.0.0/0"), "full IPs for app-based routing");
        assert!(out.contains(&format!("Endpoint = {WARP_ENDPOINT}")));
        assert!(!out.contains("engage.cloudflareclient.com"), "endpoint replaced (loop-safe)");
        assert!(!default_tunnel_apps().is_empty());
    }

    /// Item 11.1: WarpEngine accessors reflect state set by start()/stop(), is_installed() and
    /// handshake_ago_secs() are callable without panicking (no real tunnel in CI).
    #[test]
    fn warp_engine_accessors_track_state() {
        let mut w = WarpEngine::new();
        assert!(!w.is_running(), "new engine not running");
        assert!(!w.is_full(), "new engine not full-tunnel");

        // Simulate start (non-Windows sim path sets running=true, full=<arg>).
        let _ = w.start(true);
        assert!(w.is_running(), "after start: running");
        assert!(w.is_full(), "after start(true): full-tunnel");

        w.stop();
        assert!(!w.is_running(), "after stop: not running");

        let _ = w.start(false);
        assert!(w.is_running());
        assert!(!w.is_full(), "start(false): split-tunnel");

        // is_installed() and handshake_ago_secs() must not panic (no tunnel in tests).
        let _ = WarpEngine::is_installed();       // on non-Windows always false; on Windows best-effort
        let _ = WarpEngine::handshake_ago_secs(); // None when tunnel not running
    }
}
