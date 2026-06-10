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

/// WARP anycast endpoint'i (net3/SOLUTION.md §6.2 ile birebir). KRİTİK: endpoint AllowedIPs aralıklarının
/// DIŞINDA olmalı. wgcf varsayılanı `engage.cloudflareclient.com` 162.159.x'e çözülebilir → o ise
/// `162.159.0.0/16` AllowedIPs'in İÇİNDE kalır → WireGuard kendi tünel paketini tünelden geçirmeye çalışır
/// → ROUTING LOOP, el-sıkışma asla tamamlanmaz. `188.114.98.224` hiçbir tünellenen aralıkta değil → güvenli.
pub const WARP_ENDPOINT: &str = "188.114.98.224:2408";

/// WireGuard tünel adı (servis adı `WireGuardTunnel$warp`; conf dosya tabanı ile birebir olmalı).
pub const TUNNEL_NAME: &str = "warp";

/// wgcf'in ürettiği profili split-tunnel'a çevir:
///  1. TÜM `AllowedIPs` satırlarını (tam-tünel `0.0.0.0/0, ::/0`) tek `AllowedIPs = {allowed_ips}` ile değiştir.
///  2. `Endpoint =` satırını sabit `{endpoint}` ile değiştir: wgcf varsayılanı (`engage.cloudflareclient.com`)
///     `162.159.0.0/16` İÇİNE çözülebilir → o AllowedIPs'in içinde kalır → WireGuard kendi handshake paketini
///     tünelden geçirir → ROUTING LOOP. Tünellenen aralıkların DIŞINDAki sabit IP buna engel (net3 §6.2).
///  3. `DNS =` satırını düşür: aksi halde WireGuard-for-Windows tünel açıkken onu SİSTEM çözümleyicisi olarak
///     zorlar ve Cloudflare DoH'umuzu (§5) ezer → sistem DoH yetkili kalsın.
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
            continue; // tünel DNS'ini düşür (sistem DoH yetkili kalsın)
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

/// Discord'u WARP tüneliyle bağlayan motor. Servis Start/Stop'ta winws'in YANINDA çalıştırır.
/// Hatalar ÖLÜMCÜL DEĞİL (servis loglar + devam eder; winws web'i kapsar).
pub struct WarpEngine {
    running: bool,
}

impl Default for WarpEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl WarpEngine {
    pub fn new() -> Self {
        Self { running: false }
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Tüneli aç: config'i garanti et (ilk çalıştırmada wgcf ile üret) → `wireguard.exe
    /// /installtunnelservice`. Idempotent: tünel zaten kuruluysa yeniden kurmaz.
    #[cfg(windows)]
    pub fn start(&mut self) -> Result<(), String> {
        let exe = match Self::wireguard_exe() {
            Some(e) if e.exists() => e,
            _ => {
                // Dev/bundle yok: gerçek tünel yok ama UI↔IPC zinciri çalışsın (sim). Hata DÖNDÜRME.
                eprintln!("[evorift][warp] bundle yok (wireguard.exe) — sim (gerçek tünel yok)");
                self.running = true;
                return Ok(());
            }
        };
        let conf = Self::ensure_config()?; // wgcf ile üret/oku (ağ gerekebilir; başarısızsa Err)
        if Self::tunnel_installed() {
            self.running = true; // önceki örnekten kalan tünel → yeniden kullan (idempotent, kesintisiz)
            return Ok(());
        }
        Self::run_hidden(&exe, &["/installtunnelservice".into(), conf.to_string_lossy().into_owned()], None)?;
        self.running = true;
        Ok(())
    }
    #[cfg(not(windows))]
    pub fn start(&mut self) -> Result<(), String> {
        self.running = true;
        Ok(())
    }

    /// Tüneli kapat: `wireguard.exe /uninstalltunnelservice warp` (best-effort).
    #[cfg(windows)]
    pub fn stop(&mut self) {
        if let Some(exe) = Self::wireguard_exe() {
            if exe.exists() {
                let _ = Self::run_hidden(&exe, &["/uninstalltunnelservice".into(), TUNNEL_NAME.into()], None);
            }
        }
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

    /// Üretilmiş + split-tunnel'a çevrilmiş tünel config'i: `%PROGRAMDATA%\evorift\warp.conf`.
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

    /// warp.conf'u garanti et. Varsa olduğu gibi kullan (per-install özel anahtar kalıcı). Yoksa:
    /// wgcf register (hesap) → wgcf generate (profil) → split-tunnel'a çevir → korumalı ACL ile yaz.
    #[cfg(windows)]
    fn ensure_config() -> Result<std::path::PathBuf, String> {
        let conf = Self::conf_path();
        if conf.exists() {
            return Ok(conf);
        }
        let wgcf = match Self::wgcf_exe() {
            Some(w) if w.exists() => w,
            _ => return Err("wgcf.exe bundle'da yok → WARP config üretilemiyor".into()),
        };
        let dir = crate::ipc::data_dir();
        std::fs::create_dir_all(&dir).map_err(|e| format!("data dizini oluşturulamadı: {e}"))?;

        // 1) Hesap (yalnız yoksa kaydet — wgcf register var olan hesabı reddedebilir; kalıcı tut).
        let account = dir.join("wgcf-account.toml");
        if !account.exists() {
            Self::run_hidden(&wgcf, &["register".into(), "--accept-tos".into()], Some(dir.as_path()))
                .map_err(|e| format!("wgcf register başarısız (ağ?): {e}"))?;
        }
        // 2) Profil üret (AllowedIPs = 0.0.0.0/0, ::/0 içeren tam-tünel config).
        Self::run_hidden(&wgcf, &["generate".into()], Some(dir.as_path()))
            .map_err(|e| format!("wgcf generate başarısız: {e}"))?;
        let profile_path = dir.join("wgcf-profile.conf");
        let profile = std::fs::read_to_string(&profile_path)
            .map_err(|e| format!("wgcf profili okunamadı ({}): {e}", profile_path.display()))?;

        // 3) Split-tunnel'a çevir (Discord IP'leri + loop-güvenli endpoint) + 4) yaz + 5) ACL sıkılaştır.
        let split = split_tunnel_config(&profile, ALLOWED_IPS, WARP_ENDPOINT);
        std::fs::write(&conf, split).map_err(|e| format!("warp.conf yazılamadı: {e}"))?;
        Self::harden_conf_acl(&conf);
        Ok(conf)
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
    #[cfg(windows)]
    fn run_hidden(
        program: &std::path::Path,
        args: &[String],
        cwd: Option<&std::path::Path>,
    ) -> Result<(), String> {
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new(program);
        cmd.args(args).creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        if let Some(d) = cwd {
            cmd.current_dir(d);
        }
        let out = cmd
            .output()
            .map_err(|e| format!("{} çalıştırılamadı: {e}", program.display()))?;
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
}
