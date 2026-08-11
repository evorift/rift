//! evorift UI ↔ ayrıcalıklı servis IPC protokolü (docs/05 §2, docs/07 §1 — UI/orkestrasyon sınırı).
//!
//! Taşıma: local named pipe (`\\.\pipe\evorift-ipc.sock`), satır-ayrımlı JSON.
//! Güvenlik: ilk handshake'te kısa token (servis üretir, dosyaya yazar);
//! servis yalnız **beyaz-listeli + doğrulanmış** komutları kabul eder.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

/// Namespaced pipe adı (Windows'ta `\\.\pipe\evorift-ipc.sock`).
pub const PIPE_NAME: &str = "evorift-ipc.sock";

/// İstemci → servis mesajları.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    /// İlk mesaj: token handshake.
    Hello { token: String },
    /// Beyaz-listeli komut.
    Command { cmd: Command },
    /// Telemetri akışına abone ol (servis ~1 Hz `Telemetry` yollar).
    Subscribe,
    /// Stream Auto-Pilot ScoreRows live (item 6.5): the server writes one `Response::Data(row_json)` per
    /// candidate as it finishes, then a terminal `Response::Ok`. Beats the blocking `Command::AutoPilot`.
    AutoPilotStream { targets: Vec<String>, depth: String },
}

/// Servisin kabul ettiği **tek** komut kümesi (serbest komut/registry yazımı YOK).
/// Her komut `validate()` ile doğrulanır.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Command {
    Start,
    Stop,
    Status,
    SetStrategy { id: String },
    SetDns { profile: String },
    BlockApp { id: String, path: String, block: bool },
    Repair { tool: String },
    SetTweak { key: String, value: String },
    SetLimit { id: String, path: String, down: u32, up: u32 },
    SetHostlist { domains: Vec<String> },
    SetAppModes { modes: Vec<(String, String, String)> },
    SetFullWarp { enable: bool },

    // ---- Blueprint genişletmesi (docs/07) — yeni orkestrasyon komutları ----
    /// Aktif DPI motorunu değiştir ("zapret" | "byedpi" | "goodbyedpi"). Çalışıyorsa yeniden başlatır.
    SetEngine { id: String },
    /// Bilinen motorların kataloğu + kullanılabilirlik (UI motor seçici). → Response::Data(JSON).
    EngineCatalog,
    /// Profilleri listele. → Response::Data(JSON: Vec<Profile>).
    ListProfiles,
    /// Profili kaydet (içe-aktar dahil) — JSON gövdesi doğrulanır. → Response::Ok.
    SaveProfile { json: String },
    /// Profili sil.
    DeleteProfile { id: String },
    /// Profili dışa aktar (paylaşılabilir JSON). → Response::Data(JSON).
    ExportProfile { id: String },
    /// Profili uygula: motoru/stratejiyi/hostlist'i/DNS'i ayarla + başlat (durum makinesi). → Response::Status.
    ApplyProfile { id: String },
    /// DNS'i DHCP'ye sıfırla + DoH temizle (docs/05 §1).
    ResetDns,
    /// DNS doğrula (aktif sunucular + güvenli mi). → Response::Data(JSON: DnsVerify).
    VerifyDns,
    /// Ön-uçuş kontrolleri (admin/winws/çakışma/WARP/DoH). → Response::Data(JSON: PreflightResult).
    Preflight,
    /// Hedef site teşhisi (DNS/TCP/gecikme). → Response::Data(JSON: Vec<TargetDiag>).
    Diagnose { targets: Vec<String> },
    /// Auto-Pilot: hedefleri her aday motorla test et, skor tablosu döndür. → Response::Data(JSON: Vec<ScoreRow>).
    /// `depth` = "fast" | "full".
    AutoPilot { targets: Vec<String>, depth: String },
    /// Yapılan tüm sistem değişikliklerini ters sırada geri al (transaction log).
    RollbackAll,
    /// WARP tunnel state (item 11.1): returns `TunnelState` as `Response::Data(JSON)`.
    TunnelStatus,
    /// Derived health signal (item 11.2): returns `HealthSignal` as `Response::Data(JSON)`.
    Health,
}

/// Servis → istemci yanıtları.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Ok,
    Status(EngineStatus),
    /// Abonelik akışında ~1 Hz gönderilen canlı metrik.
    Telemetry(Metrics),
    /// Yapılandırılmış sorgu sonucu (profil/katalog/preflight/teşhis/autopilot) — JSON string.
    Data(String),
    Error { message: String },
}

/// Motorun (servis tarafı) anlık durumu — UI'ya dönen veri.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct EngineStatus {
    pub running: bool,
    pub strategy: String,
    pub dns: String,
    /// Aktif DPI motoru id'si ("zapret"|"byedpi"|"goodbyedpi"). Boş = varsayılan.
    #[serde(default)]
    pub engine: String,
    /// Durum makinesi (idle|applying|active|paused|error). Boş = bilinmiyor (eski istemci).
    #[serde(default)]
    pub state: String,
}

/// Canlı telemetri (docs/05 §2 — tek batch ~1 Hz). WinDivert gelene kadar simüle.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Metrics {
    pub running: bool,
    pub ping: u32,   // ms
    pub jitter: u32, // ms
    pub loss: f64,   // %
    pub down: f64,   // Mbps
    pub up: f64,     // Mbps
}

/// Beyaz-liste değer doğrulaması (docs/05 §2: yol/aralık/enum kontrolü).
pub fn validate(cmd: &Command) -> Result<(), String> {
    fn one_of(v: &str, allowed: &[&str], what: &str) -> Result<(), String> {
        if allowed.contains(&v) {
            Ok(())
        } else {
            Err(format!("geçersiz {what}: {v}"))
        }
    }
    fn valid_slug(id: &str) -> bool {
        !id.is_empty()
            && id.len() <= 64
            && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    }
    fn valid_domains(domains: &[String]) -> Result<(), String> {
        if domains.len() > 500 {
            return Err("çok fazla alan adı (en fazla 500)".into());
        }
        for d in domains {
            if d.is_empty()
                || d.len() > 253
                || !d.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
            {
                return Err(format!("geçersiz alan adı: {d}"));
            }
        }
        Ok(())
    }
    match cmd {
        Command::SetStrategy { id } => {
            // Accept "auto" or any known generic strategy / ISP preset id (item 1.5). Derived from the
            // engine catalog so new presets are accepted automatically (no second whitelist to maintain).
            if id == "auto"
                || crate::engine::strategies().iter().chain(crate::engine::presets().iter()).any(|s| s.id == id.as_str())
            {
                Ok(())
            } else {
                Err(format!("geçersiz strateji: {id}"))
            }
        }
        Command::SetDns { profile } => {
            one_of(profile, &["cloudflare", "quad9", "adguard", "google", "auto"], "dns")
        }
        Command::Repair { tool } => one_of(
            tool,
            &["flushdns", "registerdns", "dnscache", "renew", "winsock", "ipreset", "adapter"],
            "onar",
        ),
        Command::BlockApp { id, path, block } => {
            if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                return Err(format!("geçersiz app id: {id}"));
            }
            if *block {
                let p = std::path::Path::new(path);
                let lower = path.to_lowercase();
                let base = p.file_name().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                if path.is_empty()
                    || path.len() > 260
                    || !p.is_absolute()
                    || !lower.ends_with(".exe")
                    || path.contains("..")
                    || base != format!("{id}.exe")
                {
                    return Err(format!("geçersiz/uyumsuz uygulama yolu: {path}"));
                }
            }
            Ok(())
        }
        Command::SetTweak { key, value } => validate_tweak(key, value),
        Command::SetLimit { id, path, down, up } => {
            if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                Err(format!("geçersiz app id: {id}"))
            } else if *down > 1_000_000 || *up > 1_000_000 {
                Err("limit 0-1000000 kbps olmalı".into())
            } else if (*down > 0 || *up > 0) && !path.is_empty() {
                let p = std::path::Path::new(path);
                let lower = path.to_lowercase();
                if path.len() > 260 || !p.is_absolute() || !lower.ends_with(".exe") || path.contains("..") {
                    Err(format!("geçersiz uygulama yolu: {path}"))
                } else {
                    Ok(())
                }
            } else {
                Ok(())
            }
        }
        Command::SetHostlist { domains } => valid_domains(domains),
        Command::SetAppModes { modes } => {
            if modes.len() > 500 {
                return Err("çok fazla uygulama (en fazla 500)".into());
            }
            for (id, mode, path) in modes {
                if id.is_empty()
                    || id.len() > 64
                    || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                {
                    return Err(format!("geçersiz app id: {id}"));
                }
                if !matches!(mode.as_str(), "off" | "dpi" | "warp") {
                    return Err(format!("geçersiz mod: {mode}"));
                }
                if !path.is_empty() {
                    let p = std::path::Path::new(path);
                    let lower = path.to_lowercase();
                    if path.len() > 260 || !p.is_absolute() || !lower.ends_with(".exe") || path.contains("..") {
                        return Err(format!("geçersiz uygulama yolu: {path}"));
                    }
                }
            }
            Ok(())
        }
        // ---- Blueprint komutları ----
        Command::SetEngine { id } => one_of(
            id,
            &["zapret", "byedpi", "byedpi-proxifyre", "byedpi-drover", "goodbyedpi"],
            "motor",
        ),
        Command::DeleteProfile { id } | Command::ExportProfile { id } | Command::ApplyProfile { id } => {
            if valid_slug(id) {
                Ok(())
            } else {
                Err(format!("geçersiz profil id: {id}"))
            }
        }
        Command::SaveProfile { json } => {
            // JSON gövdesini Profile'a çöz + tipli doğrula (id/motor/hostlist). Boyut sınırı (DoS).
            if json.len() > 64 * 1024 {
                return Err("profil JSON çok büyük".into());
            }
            let p: crate::profile::Profile =
                serde_json::from_str(json).map_err(|e| format!("geçersiz profil JSON: {e}"))?;
            p.validate()
        }
        Command::Diagnose { targets } => {
            if targets.is_empty() {
                return Err("hedef listesi boş".into());
            }
            if targets.len() > 50 {
                return Err("çok fazla hedef (en fazla 50)".into());
            }
            valid_domains(targets)
        }
        Command::AutoPilot { targets, depth } => {
            if targets.is_empty() {
                return Err("hedef listesi boş".into());
            }
            if targets.len() > 50 {
                return Err("çok fazla hedef (en fazla 50)".into());
            }
            valid_domains(targets)?;
            // Scan depth (item 6.3). Lenient set (fast/full kept for back-compat with the old CLI).
            one_of(depth, &["quick", "standard", "force", "fast", "full"], "derinlik")
        }
        Command::Start
        | Command::Stop
        | Command::Status
        | Command::SetFullWarp { .. }
        | Command::EngineCatalog
        | Command::ListProfiles
        | Command::ResetDns
        | Command::VerifyDns
        | Command::Preflight
        | Command::RollbackAll
        | Command::TunnelStatus
        | Command::Health => Ok(()),
    }
}

/// Tweak anahtar + değer beyaz-listesi (docs/04). Bilinmeyen anahtar/değer reddedilir.
pub fn validate_tweak(key: &str, value: &str) -> Result<(), String> {
    let bool_keys = ["nagle", "heuristics", "throttleIdx", "nicPower", "highPerf", "rss", "rsc", "offload"];
    match key {
        k if bool_keys.contains(&k) => {
            if value == "on" || value == "off" {
                Ok(())
            } else {
                Err(format!("{key} için değer on/off olmalı: {value}"))
            }
        }
        "autotuning" => {
            if value == "normal" || value == "disabled" {
                Ok(())
            } else {
                Err(format!("geçersiz autotuning: {value}"))
            }
        }
        "congestion" => {
            if ["cubic", "ctcp", "bbr2"].contains(&value) {
                Ok(())
            } else {
                Err(format!("geçersiz congestion: {value}"))
            }
        }
        "mtu" => match value.parse::<u32>() {
            Ok(n) if (1280..=1500).contains(&n) => Ok(()),
            _ => Err(format!("MTU 1280-1500 aralığında olmalı: {value}")),
        },
        other => Err(format!("bilinmeyen tweak: {other}")),
    }
}

// ---------------------------------------------------------------------------
// Request-level validation (item 10.2) — covers variants not routed through Command::validate
// ---------------------------------------------------------------------------

/// Maximum IPC message size in bytes (DoS limit). Named pipe is local, but we still cap to
/// prevent a malformed/compromised client from allocating unbounded memory in the service.
pub const MAX_MSG_BYTES: usize = 256 * 1024; // 256 KiB

/// Maximum token length accepted in a `Hello` handshake (service generates ≤64-char hex tokens;
/// this cap prevents a giant-token string from allocating memory before auth is checked).
const MAX_TOKEN_BYTES: usize = 1024;

/// Validate any incoming `Request` before the service acts on it. Covers:
/// - `Hello`: token size cap.
/// - `Command`: delegates to `validate()`.
/// - `AutoPilotStream`: same target-count + domain + depth checks as `Command::AutoPilot`.
/// - `Subscribe`: always valid.
pub fn validate_request(req: &Request) -> Result<(), String> {
    match req {
        Request::Hello { token } => {
            if token.len() > MAX_TOKEN_BYTES {
                return Err(format!("token too large (> {} bytes)", MAX_TOKEN_BYTES));
            }
            Ok(())
        }
        Request::Command { cmd } => validate(cmd),
        Request::AutoPilotStream { targets, depth } => {
            // Reuse the same checks as Command::AutoPilot (item 6.3/6.5).
            validate(&Command::AutoPilot { targets: targets.clone(), depth: depth.clone() })
        }
        Request::Subscribe => Ok(()),
    }
}

/// Derived health signal for BlackHole gating (item 11.2). `healthy = running && recent_metrics &&
/// loss < 100 && ping > 0`. Returned by `Command::Health` as `Response::Data(JSON)`.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct HealthSignal {
    /// True when protection is active and connectivity metrics look good.
    pub healthy: bool,
    /// True while the engine is transitioning (RunState::Applying).
    pub loading: bool,
    /// True when the engine hit a fatal error (RunState::Error).
    pub error: bool,
    /// Most recent ping to 1.1.1.1 (ms); 0 when not yet measured.
    pub ping_ms: u32,
    /// Most recent packet-loss percentage (0–100).
    pub loss_pct: f64,
}

/// Derive a `HealthSignal` from raw engine state and optional telemetry.
/// `metrics_age_secs` is how old the last cached sample is (`None` = no sample yet).
/// Accepting the age as a parameter instead of computing `Instant::elapsed()` inside makes
/// this function deterministic in tests (no `Instant` mocking needed).
pub fn derive_health(
    running: bool,
    loading: bool,
    error: bool,
    metrics: Option<&Metrics>,
    metrics_age_secs: Option<u64>,
) -> HealthSignal {
    const STALE_SECS: u64 = 5;
    let recent = metrics_age_secs.map(|age| age < STALE_SECS).unwrap_or(false);
    let (ping, loss) = metrics.map(|m| (m.ping, m.loss)).unwrap_or((0, 0.0));
    HealthSignal {
        healthy: running && recent && loss < 100.0 && ping > 0,
        loading,
        error,
        ping_ms: ping,
        loss_pct: loss,
    }
}

/// WARP tunnel health snapshot (item 11.1) — returned by `Command::TunnelStatus`.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct TunnelState {
    /// True if the in-process `WarpEngine` started the tunnel this session.
    pub warp_running: bool,
    /// True if the active tunnel is full-tunnel (all-system); false = split-tunnel (Discord only).
    pub warp_full: bool,
    /// True if the `WireGuardTunnel$warp` Windows service is present on disk (sc query check).
    pub tunnel_installed: bool,
    /// Seconds since the WireGuard peer last completed a handshake, or `None` if unavailable
    /// (tunnel not running, no peers, or WireGuard pipe inaccessible).
    pub handshake_ago_secs: Option<u64>,
}

/// IPC verilerinin paylaşılan dizini: `%PROGRAMDATA%\evorift`.
pub fn data_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(pd) = std::env::var("ProgramData") {
            return PathBuf::from(pd).join("evorift");
        }
    }
    std::env::temp_dir().join("evorift")
}

/// Token dosyası yolu.
/// Dev (debug) → `%LOCALAPPDATA%\evorift\ipc-dev.token` (user-writable; avoids the hardened
/// PROGRAMDATA ACL set by the installed EvoriftSvc, which would block the embedded server).
/// Release → `%PROGRAMDATA%\evorift\ipc.token` (shared between service and UI).
pub fn token_path() -> PathBuf {
    #[cfg(debug_assertions)]
    {
        // User-writable location — no ACL conflict with the installed service binary.
        let base = std::env::var("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir());
        return base.join("evorift").join("ipc-dev.token");
    }
    #[allow(unreachable_code)]
    data_dir().join("ipc.token")
}

/// Tek satır JSON mesaj yaz (`\n` ile sonlandır).
pub fn write_msg<T: Serialize>(mut w: impl Write, msg: &T) -> io::Result<()> {
    let s = serde_json::to_string(msg).map_err(io::Error::other)?;
    w.write_all(s.as_bytes())?;
    w.write_all(b"\n")?;
    w.flush()
}

/// Tek satır JSON mesaj oku/çöz. Rejects messages larger than `MAX_MSG_BYTES` (DoS guard).
pub fn read_msg<T: DeserializeOwned>(r: &mut impl BufRead) -> Result<T, String> {
    let mut line = String::new();
    let n = r.read_line(&mut line).map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("bağlantı kapandı".into());
    }
    if line.len() > MAX_MSG_BYTES {
        return Err(format!("message exceeds {} KiB limit", MAX_MSG_BYTES / 1024));
    }
    serde_json::from_str(line.trim()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Item 10.2: validate_request gates all Request variants.
    #[test]
    fn validate_request_gates_all_variants() {
        // Hello: oversized token rejected, normal token ok.
        assert!(validate_request(&Request::Hello { token: "x".repeat(MAX_TOKEN_BYTES + 1) }).is_err(),
            "oversized token must be rejected");
        assert!(validate_request(&Request::Hello { token: "abc123".into() }).is_ok());

        // Command: delegates to validate() — a known-bad command is rejected.
        assert!(validate_request(&Request::Command { cmd: Command::Status }).is_ok());
        assert!(validate_request(&Request::Command {
            cmd: Command::SetEngine { id: "unknown_engine".into() }
        }).is_err(), "unknown engine must be rejected");

        // AutoPilotStream: targets + depth validation.
        assert!(validate_request(&Request::AutoPilotStream {
            targets: vec!["discord.com".into()],
            depth: "quick".into(),
        }).is_ok());
        assert!(validate_request(&Request::AutoPilotStream {
            targets: vec![],
            depth: "quick".into(),
        }).is_err(), "empty target list must be rejected");
        assert!(validate_request(&Request::AutoPilotStream {
            targets: vec!["discord.com".into()],
            depth: "bogus_depth".into(),
        }).is_err(), "unknown depth must be rejected");
        let many: Vec<String> = (0..51).map(|i| format!("site{i}.com")).collect();
        assert!(validate_request(&Request::AutoPilotStream {
            targets: many,
            depth: "quick".into(),
        }).is_err(), "too many targets must be rejected");

        // Subscribe: always ok.
        assert!(validate_request(&Request::Subscribe).is_ok());
    }

    /// Item 10.2: read_msg rejects messages that exceed MAX_MSG_BYTES.
    #[test]
    fn read_msg_rejects_oversized() {
        let oversized = " ".repeat(MAX_MSG_BYTES + 100) + "\n";
        let mut reader = std::io::BufReader::new(oversized.as_bytes());
        let result: Result<serde_json::Value, String> = read_msg(&mut reader);
        assert!(result.is_err(), "oversized message must be rejected");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("limit") || msg.contains("large") || msg.contains("exceed") || msg.contains("KiB"),
            "error must mention size: {msg}"
        );
    }

    /// Item 11.2: derive_health gates on running + fresh metrics + loss<100 + ping>0.
    #[test]
    fn health_signal_derivation() {
        let good = Metrics { running: true, ping: 50, jitter: 5, loss: 0.0, down: 1.0, up: 0.5 };

        // All conditions met → healthy.
        let h = derive_health(true, false, false, Some(&good), Some(2));
        assert!(h.healthy, "running + fresh + good metrics = healthy");
        assert_eq!(h.ping_ms, 50);
        assert_eq!(h.loss_pct, 0.0);
        assert!(!h.loading);
        assert!(!h.error);

        // Engine not running → not healthy even with good metrics.
        let h2 = derive_health(false, false, false, Some(&good), Some(2));
        assert!(!h2.healthy, "not running → not healthy");

        // Metrics older than 5 s are stale → not healthy.
        let h3 = derive_health(true, false, false, Some(&good), Some(5));
        assert!(!h3.healthy, "age=5s is not fresh (< 5 required)");
        let h3b = derive_health(true, false, false, Some(&good), Some(4));
        assert!(h3b.healthy, "age=4s is still fresh");

        // Full loss → not healthy.
        let lossy = Metrics { loss: 100.0, ..good.clone() };
        let h4 = derive_health(true, false, false, Some(&lossy), Some(2));
        assert!(!h4.healthy, "loss=100 → not healthy");

        // Partial loss below 100 is still healthy.
        let partial = Metrics { loss: 50.0, ..good.clone() };
        let h4b = derive_health(true, false, false, Some(&partial), Some(2));
        assert!(h4b.healthy, "loss=50 < 100 → healthy");

        // Zero ping → not healthy (means no measurement yet).
        let no_ping = Metrics { ping: 0, ..good.clone() };
        let h5 = derive_health(true, false, false, Some(&no_ping), Some(2));
        assert!(!h5.healthy, "ping=0 → not healthy");

        // No metrics at all → not healthy.
        let h6 = derive_health(true, false, false, None, None);
        assert!(!h6.healthy, "no metrics → not healthy");

        // loading / error flags are forwarded regardless of health.
        let h7 = derive_health(false, true, false, None, None);
        assert!(h7.loading, "loading flag forwarded");
        assert!(!h7.error);
        let h8 = derive_health(false, false, true, None, None);
        assert!(h8.error, "error flag forwarded");
        assert!(!h8.loading);
    }

    /// Item 11.2: Command::Health passes validate() and validate_request().
    #[test]
    fn health_command_validates() {
        assert!(validate(&Command::Health).is_ok());
        assert!(validate_request(&Request::Command { cmd: Command::Health }).is_ok());
    }

    /// Item 11.2: HealthSignal serializes and round-trips correctly.
    #[test]
    fn health_signal_roundtrip() {
        let hs = HealthSignal { healthy: true, loading: false, error: false, ping_ms: 25, loss_pct: 10.5 };
        let j = serde_json::to_string(&hs).expect("serialize HealthSignal");
        assert!(j.contains("healthy"));
        assert!(j.contains("loading"));
        assert!(j.contains("ping_ms"));
        assert!(j.contains("loss_pct"));
        let back: HealthSignal = serde_json::from_str(&j).expect("round-trip");
        assert!(back.healthy);
        assert_eq!(back.ping_ms, 25);
        assert!((back.loss_pct - 10.5).abs() < f64::EPSILON);
    }

    /// Item 11.1: TunnelStatus passes validation and TunnelState serializes round-trip correctly.
    #[test]
    fn tunnel_status_validates_and_state_roundtrips() {
        // TunnelStatus requires no parameters — validation must pass.
        assert!(validate(&Command::TunnelStatus).is_ok(), "TunnelStatus must pass validate()");
        assert!(
            validate_request(&Request::Command { cmd: Command::TunnelStatus }).is_ok(),
            "TunnelStatus must pass validate_request()"
        );

        // TunnelState serializes to JSON with all expected fields and round-trips cleanly.
        let state = TunnelState {
            warp_running: false,
            warp_full: false,
            tunnel_installed: false,
            handshake_ago_secs: None,
        };
        let j = serde_json::to_string(&state).expect("serialize TunnelState");
        assert!(j.contains("warp_running"), "JSON must contain warp_running");
        assert!(j.contains("tunnel_installed"), "JSON must contain tunnel_installed");
        assert!(j.contains("handshake_ago_secs"), "JSON must contain handshake_ago_secs");
        let back: TunnelState = serde_json::from_str(&j).expect("round-trip TunnelState");
        assert!(!back.warp_running);
        assert!(!back.warp_full);
        assert!(!back.tunnel_installed);
        assert!(back.handshake_ago_secs.is_none());

        // With handshake present.
        let with_hs = TunnelState { warp_running: true, warp_full: true, tunnel_installed: true, handshake_ago_secs: Some(42) };
        let j2 = serde_json::to_string(&with_hs).expect("serialize with handshake");
        let back2: TunnelState = serde_json::from_str(&j2).expect("round-trip with handshake");
        assert!(back2.warp_running);
        assert_eq!(back2.handshake_ago_secs, Some(42));
    }

    /// Item 1.5: SetStrategy validation accepts "auto" + all generic strategies + all ISP presets,
    /// and rejects unknown ids.
    #[test]
    fn set_strategy_accepts_presets_and_generics() {
        for id in [
            "auto", "c1", "multidisorder", "fake", "tt", "tt-alt", "superonline", "superonline-alt",
            "kablonet", "turkcell-hotspot", "vodafone-hotspot",
        ] {
            assert!(
                validate(&Command::SetStrategy { id: id.to_string() }).is_ok(),
                "strategy id '{id}' should be accepted"
            );
        }
        assert!(validate(&Command::SetStrategy { id: "bogus".into() }).is_err());
        assert!(validate(&Command::SetStrategy { id: String::new() }).is_err());
    }

    /// Item 11.3: FRONTEND-CONTRACT.md exists and mentions every Command op (snake_case).
    /// This test is the machine-checkable "doc lists every command + shape" requirement.
    #[test]
    fn contract_doc_covers_all_commands() {
        // CARGO_MANIFEST_DIR points to src-tauri/; the contract is one level up at the repo root.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let contract_path = dir.join("..").join("FRONTEND-CONTRACT.md");
        let contract = std::fs::read_to_string(&contract_path)
            .unwrap_or_else(|e| panic!("FRONTEND-CONTRACT.md not found at {}: {e}", contract_path.display()));

        // Every Command variant must appear as its snake_case op name in the contract.
        let required = [
            "start", "stop", "status",
            "set_strategy", "set_dns", "block_app", "repair",
            "set_tweak", "set_limit", "set_hostlist", "set_app_modes", "set_full_warp",
            "set_engine", "engine_catalog",
            "list_profiles", "save_profile", "delete_profile", "export_profile", "apply_profile",
            "reset_dns", "verify_dns",
            "preflight", "diagnose",
            "auto_pilot",
            "rollback_all",
            "tunnel_status",
            "health",
        ];
        for op in required {
            assert!(
                contract.contains(op),
                "FRONTEND-CONTRACT.md is missing Command op `{op}` — update the doc"
            );
        }
        // The contract must also document the Response types and key payload shapes.
        for keyword in ["EngineStatus", "Metrics", "HealthSignal", "TunnelState", "ScoreRow", "PreflightResult"] {
            assert!(
                contract.contains(keyword),
                "FRONTEND-CONTRACT.md is missing type `{keyword}`"
            );
        }
    }
}
