//! evorift UI ↔ ayrıcalıklı servis IPC protokolü (docs/05 §2).
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
    /// Ağ/sistem tweak'i (docs/04). `key` = ayar, `value` = "on"/"off" veya enum/sayı.
    SetTweak { key: String, value: String },
    /// Per-app hız sınırı (QoS, docs/03 §4). down/up = kbps (0 = sınırsız → politikayı kaldır).
    /// `path` = QoS eşleştirmesi için gerçek exe yolu (boşluklu/Türkçe adlı uygulamalar da eşleşsin);
    /// boşsa servis `{id}.exe` adına düşer (varsayılan placeholder app'ler).
    SetLimit { id: String, path: String, down: u32, up: u32 },
    /// Bypass uygulanacak alan adı listesi (docs/03 §1, Faz 3.4). Motor Start'ta bu listeyi kullanır.
    SetHostlist { domains: Vec<String> },
    /// Uygulama başı koruma modlarının BÜTÜNLÜKLÜ snapshot'ı (UI ↔ servis):
    /// `(id, mode, path)` üçlüleri. path = uygulamanın gerçek exe yolu (boşluklu/Türkçe destekli).
    ///
    /// Servis bu listeden iki şey çıkarır:
    ///  1. **WARP**: en az bir uygulama "warp" → WARP tüneli aktif; aksi halde saf winws.
    ///  2. **Per-PID winws hariç tutma**: "off" modlu uygulamaların ÇALIŞAN PID'lerinin TCP/UDP
    ///     source port'larını ~5sn'de bir tarar → winws WinDivert capture filter'ına
    ///     `tcp.SrcPort != X` clause'larıyla ekler → o uygulamanın paketleri winws'e ULAŞMAZ →
    ///     gerçek "off" (DPI o uygulamada devre dışı). Bkz. service.rs scan loop'u + engine.rs
    ///     `set_exclusion` yöntemi. Boş liste / off PID yok → eski `--wf-tcp` yolu (regresyon yok).
    SetAppModes { modes: Vec<(String, String, String)> },
}

/// Servis → istemci yanıtları.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Ok,
    Status(EngineStatus),
    /// Abonelik akışında ~1 Hz gönderilen canlı metrik.
    Telemetry(Metrics),
    Error { message: String },
}

/// Motorun (servis tarafı) anlık durumu — UI'ya dönen veri.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct EngineStatus {
    pub running: bool,
    pub strategy: String,
    pub dns: String,
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
    match cmd {
        Command::SetStrategy { id } => one_of(id, &["auto", "c1", "multidisorder", "fake"], "strateji"),
        Command::SetDns { profile } => {
            one_of(profile, &["cloudflare", "quad9", "adguard", "google", "auto"], "dns")
        }
        Command::Repair { tool } => one_of(
            tool,
            &["flushdns", "registerdns", "dnscache", "renew", "winsock", "ipreset", "adapter"],
            "onar",
        ),
        Command::BlockApp { id, path, block } => {
            // id netsh kural adına gider → SetLimit ile AYNI charset (nokta YOK → tutarlılık + sessiz hata önlenir)
            if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                return Err(format!("geçersiz app id: {id}"));
            }
            // path elevated servise gidiyor → engelleniyorsa SIKI doğrula (keyfi yol engelini/yetki açığını önle).
            // path argv ile geçtiği için enjeksiyon değil ama servis keyfi yola güvenmemeli (review HIGH-1).
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
            // id güvenli karakterler (QoS politika adına gider) + 0-1000 Mbps
            if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                Err(format!("geçersiz app id: {id}"))
            } else if *down > 1_000_000 || *up > 1_000_000 {
                Err("limit 0-1000000 kbps olmalı".into())
            } else if (*down > 0 || *up > 0) && !path.is_empty() {
                // limit uygulanırken yol verildiyse SIKI doğrula — yol elevated servise QoS eşleştirmesi
                // olarak gidiyor; keyfi yol kabul etme (BlockApp ile aynı disiplin, review HIGH-1).
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
        Command::SetHostlist { domains } => {
            if domains.len() > 500 {
                return Err("çok fazla alan adı (en fazla 500)".into());
            }
            for d in domains {
                // alan adı karakter seti (PowerShell/WinDivert filtre enjeksiyonuna karşı)
                if d.is_empty()
                    || d.len() > 253
                    || !d.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
                {
                    return Err(format!("geçersiz alan adı: {d}"));
                }
            }
            Ok(())
        }
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
                // path elevated servise gidiyor → SetLimit/BlockApp ile AYNI disiplin:
                // boş kabul (henüz exe tespit edilmemiş app'ler için), aksi halde mutlak .exe yolu.
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
        Command::Start | Command::Stop | Command::Status => Ok(()),
    }
}

/// Tweak anahtar + değer beyaz-listesi (docs/04). Bilinmeyen anahtar/değer reddedilir.
pub fn validate_tweak(key: &str, value: &str) -> Result<(), String> {
    // Frontend app.tweaks anahtarlarıyla birebir (camelCase).
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

/// IPC verilerinin paylaşılan dizini: `%PROGRAMDATA%\evorift` (Windows) — LocalSystem servisi
/// ile kullanıcı app'i AYNI yolu görmeli (servis C:\Windows\Temp, kullanıcı %TEMP%'i görür → uyuşmaz).
/// ProgramData her ikisi için sabit (`C:\ProgramData`) ve varsayılan olarak herkes-okunur.
pub fn data_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(pd) = std::env::var("ProgramData") {
            return PathBuf::from(pd).join("evorift");
        }
    }
    std::env::temp_dir().join("evorift")
}

/// Token dosyası yolu: `%PROGRAMDATA%\evorift\ipc.token`. Servis yazar, app okur (paylaşılan).
pub fn token_path() -> PathBuf {
    data_dir().join("ipc.token")
}

/// Tek satır JSON mesaj yaz (`\n` ile sonlandır).
pub fn write_msg<T: Serialize>(mut w: impl Write, msg: &T) -> io::Result<()> {
    let s = serde_json::to_string(msg).map_err(io::Error::other)?;
    w.write_all(s.as_bytes())?;
    w.write_all(b"\n")?;
    w.flush()
}

/// Tek satır JSON mesaj oku/çöz.
pub fn read_msg<T: DeserializeOwned>(r: &mut impl BufRead) -> Result<T, String> {
    let mut line = String::new();
    let n = r.read_line(&mut line).map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("bağlantı kapandı".into());
    }
    serde_json::from_str(line.trim()).map_err(|e| e.to_string())
}
