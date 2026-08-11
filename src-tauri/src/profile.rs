//! Profil sistemi (docs/07 §4, docs/06 §2.4) — özelleştirmenin kalbi. Bir profil = {motor, parametreler,
//! hedef uygulamalar/alan adları, DNS}. İçe/dışa aktarılabilir, sürümlenir, topluluk deposundan indirilir.
//!
//! Kalıcılık: `%PROGRAMDATA%\evorift\profiles\<id>.json`. "Bir profil dosyası = paylaşılabilir, tekrar
//! üretilebilir tüm yapılandırma." Servis tarafı okur (ApplyProfile); UI listeler/düzenler/dışa aktarır.

use serde::{Deserialize, Serialize};

/// Profil şema sürümü — ileride alan eklenince geriye-uyum/migrasyon için.
pub const SCHEMA_VERSION: u32 = 1;

/// Kapsam modu: tüm sistem mi, yoksa seçili uygulamalar mı (split).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScopeMode {
    #[default]
    System,
    Split,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Scope {
    #[serde(default)]
    pub mode: ScopeMode,
    /// Split modda hedef uygulamalar (exe adları, ör. "Discord.exe").
    #[serde(default)]
    pub apps: Vec<String>,
    /// Tarayıcıları da kapsa (DoH stub resolver uyarısı: docs/05 — sınırlı etki).
    #[serde(default)]
    pub browsers: bool,
    /// Custom application folders to also route (docs/03 §1.2 "customize folder list", item 7.1).
    #[serde(default)]
    pub folders: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct DnsCfg {
    #[serde(default)]
    pub enabled: bool,
    /// DNS sağlayıcı id'si (cloudflare/quad9/adguard/google/auto). Boş → değiştirme.
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub doh: bool,
}

/// Tek bir bypass profili (docs/07 §4 veri modeli).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default = "default_schema")]
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    /// Motor id'si: "zapret" | "byedpi" | "goodbyedpi" (engine::catalog ile).
    pub engine: String,
    /// Opsiyonel: bilinen ISS preset eşleştirmesi (autopilot önceliklendirmesi).
    #[serde(default)]
    pub isp: String,
    #[serde(default)]
    pub scope: Scope,
    #[serde(default)]
    pub dns: DnsCfg,
    /// DPI strateji id'si (engine::strategies → "c1"/"multidisorder"/"fake"/"auto").
    #[serde(default)]
    pub strategy: String,
    /// Bypass uygulanacak alan adları (winws hostlist / IP-aralığı eşleştirme).
    #[serde(default)]
    pub hostlist: Vec<String>,
    /// Engine-specific parameters (item 7.1), kept as a flexible JSON bag the per-engine apply logic reads
    /// (e.g. goodbyedpi `{ "mode": 9, "set_ttl": 5, "dns": true }`; byedpi `{ "preset": "fake" }`). The
    /// generic zapret engine uses `strategy` above; this carries the rest.
    #[serde(default)]
    pub engine_params: serde_json::Value,
}

fn default_schema() -> u32 {
    SCHEMA_VERSION
}

impl Profile {
    /// id güvenli slug mu (dosya adı + IPC validate ile uyumlu)?
    pub fn valid_id(id: &str) -> bool {
        !id.is_empty()
            && id.len() <= 64
            && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    }

    /// Profil alanlarını doğrula (içe aktarmadan / kaydetmeden önce).
    pub fn validate(&self) -> Result<(), String> {
        if !Self::valid_id(&self.id) {
            return Err(format!("geçersiz profil id: {}", self.id));
        }
        if self.name.is_empty() || self.name.len() > 64 {
            return Err("profil adı 1-64 karakter olmalı".into());
        }
        // "warp" = a tunnel profile (no DPI engine; brings up WARP). The rest are DPI engines.
        if !["zapret", "byedpi", "byedpi-proxifyre", "byedpi-drover", "goodbyedpi", "warp", ""]
            .contains(&self.engine.as_str())
        {
            return Err(format!("bilinmeyen motor: {}", self.engine));
        }
        if self.hostlist.len() > 500 {
            return Err("çok fazla alan adı (en fazla 500)".into());
        }
        for d in &self.hostlist {
            if d.is_empty() || d.len() > 253 || !d.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') {
                return Err(format!("geçersiz alan adı: {d}"));
            }
        }
        Ok(())
    }
}

/// Profiller dizini: `%PROGRAMDATA%\evorift\profiles\`.
fn profiles_dir() -> std::path::PathBuf {
    crate::ipc::data_dir().join("profiles")
}

fn profile_path(id: &str) -> std::path::PathBuf {
    profiles_dir().join(format!("{id}.json"))
}

/// Tüm profilleri diskten oku (bozuk dosyalar atlanır). İlk çağrıda dizin yoksa tohumla.
pub fn load_all() -> Vec<Profile> {
    let dir = profiles_dir();
    if !dir.exists() {
        seed_defaults();
    }
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.filter_map(|e| e.ok()) {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            if let Ok(s) = std::fs::read_to_string(&p) {
                if let Ok(prof) = serde_json::from_str::<Profile>(&s) {
                    out.push(prof);
                }
            }
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// id ile tek profil.
pub fn get(id: &str) -> Option<Profile> {
    if !Profile::valid_id(id) {
        return None;
    }
    let s = std::fs::read_to_string(profile_path(id)).ok()?;
    serde_json::from_str(&s).ok()
}

/// Profili kaydet (doğrula → JSON yaz). Dizini garanti eder.
pub fn save(p: &Profile) -> Result<(), String> {
    p.validate()?;
    let dir = profiles_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("profil dizini oluşturulamadı: {e}"))?;
    let json = serde_json::to_string_pretty(p).map_err(|e| e.to_string())?;
    std::fs::write(profile_path(&p.id), json).map_err(|e| format!("profil yazılamadı: {e}"))
}

/// Profili sil.
pub fn delete(id: &str) -> Result<(), String> {
    if !Profile::valid_id(id) {
        return Err(format!("geçersiz profil id: {id}"));
    }
    let path = profile_path(id);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("profil silinemedi: {e}"))?;
    }
    Ok(())
}

/// Dışa aktar: profili paylaşılabilir JSON string'e çevir.
pub fn export(id: &str) -> Result<String, String> {
    let p = get(id).ok_or_else(|| format!("profil bulunamadı: {id}"))?;
    serde_json::to_string_pretty(&p).map_err(|e| e.to_string())
}

/// İçe aktar: JSON'u çöz → doğrula → kaydet → profili döndür.
pub fn import(json: &str) -> Result<Profile, String> {
    let p: Profile = serde_json::from_str(json).map_err(|e| format!("geçersiz profil JSON: {e}"))?;
    p.validate()?;
    save(&p)?;
    Ok(p)
}

/// Parse a community preset bundle — a JSON array of profiles — validating each (item 7.4). Pure → testable.
pub fn parse_bundle(json: &str) -> Result<Vec<Profile>, String> {
    let profs: Vec<Profile> =
        serde_json::from_str(json).map_err(|e| format!("invalid preset bundle JSON: {e}"))?;
    if profs.len() > 200 {
        return Err("preset bundle too large (max 200)".into());
    }
    for p in &profs {
        p.validate()?;
    }
    Ok(profs)
}

/// Import a whole bundle: parse + validate + save each profile. Returns the imported profiles.
pub fn import_bundle(json: &str) -> Result<Vec<Profile>, String> {
    let profs = parse_bundle(json)?;
    for p in &profs {
        save(p)?;
    }
    Ok(profs)
}

/// OPT-IN community preset fetch (item 7.4) — USER-INITIATED only. Downloads a versioned preset bundle
/// from an HTTPS URL and parses+validates it (does NOT save; the caller decides what to keep). The URL is
/// strictly validated (https + safe chars) so it can be embedded in the PowerShell fetch without injection.
pub fn fetch_community(url: &str) -> Result<Vec<Profile>, String> {
    if !url.starts_with("https://")
        || url.len() > 2048
        || !url.chars().all(|c| c.is_ascii_alphanumeric() || "._-/:?=&%~+".contains(c))
    {
        return Err("invalid community preset URL (must be HTTPS with safe characters)".into());
    }
    let script =
        format!("try {{ (Invoke-WebRequest -Uri '{url}' -UseBasicParsing -TimeoutSec 10).Content }} catch {{ '' }}");
    let out = crate::sys::query_os("powershell", &["-NoProfile", "-Command", &script]);
    let text = out.trim();
    if text.is_empty() {
        return Err("community preset fetch failed (network or URL unreachable)".into());
    }
    parse_bundle(text)
}

/// Hazır profilleri tohumla (ilk çalıştırmada). docs/06 §2.4: "Discord+Ses", "YouTube hızlı", "Her şey".
pub fn seed_defaults() {
    let _ = std::fs::create_dir_all(profiles_dir());
    let discord_hosts = [
        "discord.com", "discordapp.com", "discord.gg", "discordapp.net", "discord.media",
        "gateway.discord.gg", "cdn.discordapp.com",
    ];
    let yt_hosts = ["youtube.com", "googlevideo.com", "ytimg.com"];
    let all_hosts = vec![
        "discord.com", "discordapp.com", "discord.gg", "discordapp.net", "discord.media",
        "gateway.discord.gg", "cdn.discordapp.com", "roblox.com", "www.roblox.com", "rbxcdn.com",
        "youtube.com", "googlevideo.com",
    ];
    let defaults = [
        Profile {
            schema_version: SCHEMA_VERSION,
            id: "discord-voice".into(),
            name: "Discord + Ses".into(),
            engine: "zapret".into(),
            isp: String::new(),
            scope: Scope { mode: ScopeMode::Split, apps: vec!["Discord.exe".into()], browsers: false, folders: vec![] },
            dns: DnsCfg { enabled: true, provider: "cloudflare".into(), doh: true },
            strategy: "c1".into(),
            hostlist: discord_hosts.iter().map(|s| s.to_string()).collect(),
            engine_params: serde_json::Value::Null,
        },
        Profile {
            schema_version: SCHEMA_VERSION,
            id: "youtube-fast".into(),
            name: "YouTube Hızlı".into(),
            engine: "zapret".into(),
            isp: String::new(),
            scope: Scope { mode: ScopeMode::System, apps: vec![], browsers: true, folders: vec![] },
            dns: DnsCfg { enabled: true, provider: "cloudflare".into(), doh: true },
            strategy: "fake".into(),
            hostlist: yt_hosts.iter().map(|s| s.to_string()).collect(),
            engine_params: serde_json::Value::Null,
        },
        Profile {
            schema_version: SCHEMA_VERSION,
            id: "everything".into(),
            name: "Her Şey".into(),
            engine: "zapret".into(),
            isp: String::new(),
            scope: Scope { mode: ScopeMode::System, apps: vec![], browsers: true, folders: vec![] },
            dns: DnsCfg { enabled: true, provider: "cloudflare".into(), doh: true },
            strategy: "auto".into(),
            hostlist: all_hosts.iter().map(|s| s.to_string()).collect(),
            engine_params: serde_json::Value::Null,
        },
    ];
    for p in &defaults {
        if !profile_path(&p.id).exists() {
            let _ = save(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Profile {
        Profile {
            schema_version: SCHEMA_VERSION,
            id: "test-prof".into(),
            name: "Test".into(),
            engine: "zapret".into(),
            isp: String::new(),
            scope: Scope::default(),
            dns: DnsCfg::default(),
            strategy: "c1".into(),
            hostlist: vec!["discord.com".into()],
            engine_params: serde_json::Value::Null,
        }
    }

    /// Item 7.1: full schema (engine_params + scope.folders + composite engine) survives a JSON round-trip,
    /// and an old profile JSON missing the new fields still deserializes (serde defaults).
    #[test]
    fn full_schema_roundtrip_and_back_compat() {
        let mut p = sample();
        p.engine = "goodbyedpi".into();
        p.engine_params = serde_json::json!({ "mode": 9, "set_ttl": 5, "dns": true });
        p.scope.folders = vec!["C:\\Games\\Foo".into()];
        p.isp = "tt".into();
        let json = serde_json::to_string(&p).unwrap();
        let back: Profile = serde_json::from_str(&json).unwrap();
        assert_eq!(back.engine_params["mode"], 9);
        assert_eq!(back.engine_params["dns"], true);
        assert_eq!(back.scope.folders, vec!["C:\\Games\\Foo".to_string()]);
        assert_eq!(back.isp, "tt");
        back.validate().expect("composite/goodbyedpi engine valid");

        // old JSON without the new fields → defaults (Null params, empty folders)
        let old = r#"{"id":"x","name":"X","engine":"zapret"}"#;
        let op: Profile = serde_json::from_str(old).unwrap();
        assert!(op.engine_params.is_null());
        assert!(op.scope.folders.is_empty());

        // composite engine id accepted
        let mut c = sample();
        c.engine = "byedpi-proxifyre".into();
        c.validate().expect("composite engine id valid");
    }

    #[test]
    fn export_import_roundtrip() {
        let p = sample();
        let json = serde_json::to_string(&p).unwrap();
        let back = import_no_save(&json).unwrap();
        assert_eq!(back.id, p.id);
        assert_eq!(back.engine, p.engine);
        assert_eq!(back.hostlist, p.hostlist);
    }

    // Diske yazmadan içe-aktar (test): yalnız çöz + doğrula.
    fn import_no_save(json: &str) -> Result<Profile, String> {
        let p: Profile = serde_json::from_str(json).map_err(|e| e.to_string())?;
        p.validate()?;
        Ok(p)
    }

    /// Item 7.4: bundle parse validates each profile; community-fetch URL is strictly validated.
    #[test]
    fn bundle_parse_and_url_validation() {
        let one = serde_json::to_string(&sample()).unwrap();
        let bundle = parse_bundle(&format!("[{one},{one}]")).expect("parse 2-profile bundle");
        assert_eq!(bundle.len(), 2);
        // a bad profile inside the bundle fails the whole parse
        assert!(parse_bundle(r#"[{"id":"bad id!","name":"x","engine":"zapret"}]"#).is_err());
        assert!(parse_bundle("not json").is_err());
        // fetch URL validation rejects non-HTTPS and injection-unsafe URLs (no network hit)
        assert!(fetch_community("http://evil.example").is_err(), "non-https rejected");
        assert!(fetch_community("https://x.com/p.json'; rm -rf /").is_err(), "unsafe chars rejected");
    }

    #[test]
    fn rejects_bad_id() {
        let mut p = sample();
        p.id = "bad id!".into();
        assert!(p.validate().is_err());
    }

    #[test]
    fn rejects_bad_engine() {
        let mut p = sample();
        p.engine = "hack".into();
        assert!(p.validate().is_err());
    }

    #[test]
    fn rejects_bad_hostlist() {
        let mut p = sample();
        p.hostlist = vec!["bad host".into()];
        assert!(p.validate().is_err());
    }
}
