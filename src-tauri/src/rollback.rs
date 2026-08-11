//! Transaction / Rollback log (docs/07 §5, docs/05 §4.2) — garantili geri alma. Yapılan HER sistem
//! değişikliği (firewall, DNS, hizmet, registry, tünel, dosya) önce buraya kaydedilir; geri alma bu
//! logdan ters sırada beslenir. Sabit hizmet listeleriyle temizlikten daha güvenilir (elle eklenen
//! unutulmaz). Kalıcı: `%PROGRAMDATA%\evorift\rollback.json` (servis crash sonrası kurtarma için).

use serde::{Deserialize, Serialize};
use crate::sys::{audit, run_os};

/// Geri alınabilir tek bir sistem değişikliği (docs/07 §5 `IRollbackLog`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Change {
    /// `sc create` ile kurulan hizmet → geri al: stop + delete.
    ServiceCreated { name: String },
    /// `netsh advfirewall` kuralı (tam ad, ör. "evorift-block-discord") → geri al: delete rule.
    FirewallRule { name: String },
    /// Registry yazımı → geri al: eski değer varsa geri yaz, yoksa sil.
    RegistryWrite { path: String, name: String, old: Option<String> },
    /// DNS değişti → geri al: DHCP'ye (otomatik) sıfırla + DoH temizle (docs/05 §1).
    DnsChanged,
    /// WireGuard tüneli kuruldu (servis adı tabanı, ör. "warp") → geri al: /uninstalltunnelservice.
    TunnelInstalled { name: String },
    /// Kopyalanan dosya → geri al: sil.
    FileCopied { path: String },
    /// Zamanlanmış görev kuruldu (item 7.3) → geri al: Unregister-ScheduledTask.
    ScheduledTask { name: String },
}

fn log_path() -> std::path::PathBuf {
    crate::ipc::data_dir().join("rollback.json")
}

/// Sıralı değişiklik günlüğü. Bellekte tutulur + her kayıtta diske yazılır (crash-dayanıklı).
#[derive(Default, Serialize, Deserialize)]
pub struct RollbackLog {
    changes: Vec<Change>,
}

impl RollbackLog {
    /// Empty log (const so it can back the process-global `LOG`).
    pub const fn new() -> Self {
        Self { changes: Vec::new() }
    }

    /// Diskten yükle (servis açılışında: yarım kalan değişiklikleri kurtarmak için). Yoksa boş.
    pub fn load() -> Self {
        std::fs::read_to_string(log_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn persist(&self) {
        let _ = std::fs::create_dir_all(crate::ipc::data_dir());
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(log_path(), json);
        }
    }

    /// Bir değişikliği kaydet (uygulandıktan SONRA çağır → log her zaman gerçeği yansıtsın).
    pub fn record(&mut self, c: Change) {
        audit(&format!("rollback kayıt: {c:?}"));
        self.changes.push(c);
        self.persist();
    }

    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// TÜM değişiklikleri TERS sırada geri al (bağımlılık-farkında: sonra kurulan önce kaldırılır →
    /// ör. WinDivert'i tüketen winws, WinDivert'ten önce; tünel, profil değişiminden önce). Best-effort:
    /// bir adım başarısız olsa diğerleri denenir. Sonunda log temizlenir.
    pub fn rollback_all(&mut self) -> Result<(), String> {
        let mut errors: Vec<String> = Vec::new();
        while let Some(c) = self.changes.pop() {
            if let Err(e) = undo(&c) {
                errors.push(format!("{c:?}: {e}"));
            }
        }
        self.persist(); // boşaldı → diske yaz
        if errors.is_empty() {
            Ok(())
        } else {
            Err(format!("{} geri alma adımı başarısız: {}", errors.len(), errors.join("; ")))
        }
    }

    /// Logu temizle (başarılı tam uygulama sonrası "checkpoint" olarak — geri alınacak bir şey yok).
    pub fn clear(&mut self) {
        self.changes.clear();
        self.persist();
    }
}

/// Tek bir değişikliği geri al (ayrıcalıklı OS komutları; yetkisizse sys (sim)).
fn undo(c: &Change) -> Result<(), String> {
    match c {
        Change::ServiceCreated { name } => {
            let _ = run_os("sc", &["stop", name]);
            run_os("sc", &["delete", name])
        }
        Change::FirewallRule { name } => {
            let rule = format!("name={name}");
            run_os("netsh", &["advfirewall", "firewall", "delete", "rule", &rule])
        }
        Change::RegistryWrite { path, name, old } => match old {
            Some(v) => run_os("reg", &["add", path, "/v", name, "/d", v, "/f"]),
            None => run_os("reg", &["delete", path, "/v", name, "/f"]),
        },
        Change::DnsChanged => crate::dns::reset_dns(),
        Change::TunnelInstalled { name } => {
            // wireguard.exe bundle: <exe_dizini>\warp\wireguard.exe
            let exe = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("warp").join("wireguard.exe")));
            match exe {
                Some(e) if e.exists() => {
                    run_os(&e.to_string_lossy(), &["/uninstalltunnelservice", name])
                }
                _ => Ok(()), // wireguard yok → kaldırılacak tünel de yok (sim/dev)
            }
        }
        Change::FileCopied { path } => {
            if std::path::Path::new(path).exists() {
                std::fs::remove_file(path).map_err(|e| format!("dosya silinemedi: {e}"))
            } else {
                Ok(())
            }
        }
        Change::ScheduledTask { name } => {
            let script =
                format!("Unregister-ScheduledTask -TaskName '{name}' -Confirm:$false -ErrorAction SilentlyContinue");
            run_os("powershell", &["-NoProfile", "-Command", &script])
        }
    }
}

// --- Process-global rollback log (item 7.3) ---
// One log shared by every mutation site (engines call proxifyre/drover directly, with no Engine handle),
// so all real system changes land in one place and `rollback_all` reverses the full apply.
static LOG: std::sync::Mutex<RollbackLog> = std::sync::Mutex::new(RollbackLog::new());

/// Record a change into the global log. Call AFTER a real mutation succeeds.
pub fn record(c: Change) {
    LOG.lock().unwrap_or_else(|p| p.into_inner()).record(c);
}

/// Reverse all recorded changes (ters sıra, best-effort). Clears the log.
pub fn rollback_all() -> Result<(), String> {
    LOG.lock().unwrap_or_else(|p| p.into_inner()).rollback_all()
}

/// Is the global log empty?
pub fn is_empty() -> bool {
    LOG.lock().unwrap_or_else(|p| p.into_inner()).is_empty()
}

/// Load the persisted log into the global (call once at service boot to recover half-applied changes).
pub fn load_global() {
    *LOG.lock().unwrap_or_else(|p| p.into_inner()) = RollbackLog::load();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Item 7.3: rollback_all reverses a full apply — the FileCopied target is really deleted, the rest
    /// (sc/netsh/schtask) sim-succeed when unprivileged, and the log ends empty.
    #[test]
    fn rollback_all_reverses_full_apply() {
        let tmp = std::env::temp_dir().join("evorift-test-rollback-file.bin");
        std::fs::write(&tmp, b"x").unwrap();
        let mut log = RollbackLog::new();
        log.changes.push(Change::ServiceCreated { name: "EvoriftTestSvc".into() });
        log.changes.push(Change::FirewallRule { name: "evorift-test".into() });
        log.changes.push(Change::ScheduledTask { name: "EvoriftTestTask".into() });
        log.changes.push(Change::DnsChanged);
        log.changes.push(Change::FileCopied { path: tmp.to_string_lossy().into_owned() });
        assert!(log.rollback_all().is_ok());
        assert!(!tmp.exists(), "FileCopied target must be removed");
        assert!(log.is_empty(), "log empty after rollback");
    }

    #[test]
    fn changes_pop_in_reverse() {
        let mut log = RollbackLog::new();
        // persist() diske yazar; testte data_dir yazılabilir olmayabilir → yalnız bellek davranışını
        // doğrula (changes vektörü). record persist hatasını yutar (best-effort).
        log.changes.push(Change::FirewallRule { name: "a".into() });
        log.changes.push(Change::FirewallRule { name: "b".into() });
        assert_eq!(log.changes.len(), 2);
        // ters sıra: en son "b" önce çıkmalı
        let last = log.changes.pop().unwrap();
        matches!(last, Change::FirewallRule { ref name } if name == "b").then_some(()).unwrap();
    }

    #[test]
    fn serde_roundtrip() {
        let c = Change::RegistryWrite { path: "HKLM\\X".into(), name: "Y".into(), old: Some("1".into()) };
        let json = serde_json::to_string(&c).unwrap();
        let back: Change = serde_json::from_str(&json).unwrap();
        matches!(back, Change::RegistryWrite { .. }).then_some(()).unwrap();
    }
}
