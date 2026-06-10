//! "Off" modlu uygulamaların ÇALIŞAN PID'lerinin TCP+UDP source port'larını PowerShell ile tara →
//! engine.rs WinwsEngine bunları WinDivert capture filter'ında EXCLUDE eder → o uygulamanın paketleri
//! winws'e ULAŞMAZ → gerçek "off" mod (DPI'dan tamamen hariç).
//!
//! Neden PowerShell? Get-Process + Get-NetTCPConnection + Get-NetUDPEndpoint yerleşik, ek bağımlılık
//! YOK. ~5sn'lik tarama gecikmesi kabul edilebilir (yeni bağlantılarda kısa süreli geçici DPI
//! uygulanabilir, sonra exclusion devreye girer). Tarama atomik: aynı snapshot'tan PID'ler + portlar.
//!
//! Güvenlik: exe yolları $env ile geçer (komut satırında DEĞİL → enjeksiyon yok, P7/A5 deseni).

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ExclusionPorts {
    pub tcp: Vec<u16>,
    pub udp: Vec<u16>,
}

impl ExclusionPorts {
    pub fn is_empty(&self) -> bool {
        self.tcp.is_empty() && self.udp.is_empty()
    }
}

/// Verilen exe yolları için çalışan PID'lerin TCP+UDP source port'larını topla.
/// Yollar yokSayılır (boş, kayıp). Bulamazsa boş döner — engine eski yola düşer.
#[cfg(windows)]
pub fn scan(off_paths: &[String]) -> ExclusionPorts {
    let paths: Vec<&str> = off_paths.iter().filter(|p| !p.is_empty()).map(|s| s.as_str()).collect();
    if paths.is_empty() {
        return ExclusionPorts::default();
    }
    // Yolları | ile birleştir → ENV ile PowerShell'e tek string olarak ver → script -split '\|' ile aç.
    // (Komut satırı argv'sinde DEĞİL → boşluklu/Türkçe yollar bozulmaz, enjeksiyon yok.)
    let joined = paths.join("|");

    // STATİK script. $env değişkeni list edilen exe yollarını taşır. Çıktı: JSON {tcp:[..], udp:[..]}.
    // - Get-Process: tüm süreçler, path'i off listede olanları filtrele.
    // - Get-NetTCPConnection: TCP soketleri PID'e göre eşle → LocalPort.
    // - Get-NetUDPEndpoint: UDP soketleri PID'e göre eşle → LocalPort.
    let script = r#"$ErrorActionPreference='SilentlyContinue'
$wanted = ($env:EVORIFT_OFF_PATHS -split '\|') | Where-Object { $_ }
if ($wanted.Count -eq 0) { '{"tcp":[],"udp":[]}'; return }
$lookup = @{}
foreach ($w in $wanted) { $lookup[$w.ToLower()] = $true }
$pids = @()
foreach ($p in (Get-Process)) {
  try { if ($p.Path -and $lookup[$p.Path.ToLower()]) { $pids += $p.Id } } catch {}
}
$pids = $pids | Sort-Object -Unique
if ($pids.Count -eq 0) { '{"tcp":[],"udp":[]}'; return }
$tcp = @(Get-NetTCPConnection | Where-Object { $pids -contains $_.OwningProcess } | Select-Object -ExpandProperty LocalPort -Unique)
$udp = @(Get-NetUDPEndpoint | Where-Object { $pids -contains $_.OwningProcess } | Select-Object -ExpandProperty LocalPort -Unique)
@{tcp=$tcp; udp=$udp} | ConvertTo-Json -Compress"#;

    use std::os::windows::process::CommandExt;
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .env("EVORIFT_OFF_PATHS", joined)
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW (sessiz tarama)
        .output();
    let Ok(o) = out else { return ExclusionPorts::default() };
    let s = String::from_utf8_lossy(&o.stdout);
    let s = s.trim_start_matches('\u{feff}').trim();
    if s.is_empty() {
        return ExclusionPorts::default();
    }

    // JSON çöz. PowerShell tek elemanı sayı olarak, çoğul'u dizi olarak verir → ikisini de kabul et.
    #[derive(serde::Deserialize)]
    struct Raw {
        #[serde(default)]
        tcp: serde_json::Value,
        #[serde(default)]
        udp: serde_json::Value,
    }
    fn extract(v: &serde_json::Value) -> Vec<u16> {
        let mut out = Vec::new();
        match v {
            serde_json::Value::Array(arr) => {
                for x in arr {
                    if let Some(n) = x.as_u64() {
                        if n > 0 && n <= u16::MAX as u64 {
                            out.push(n as u16);
                        }
                    }
                }
            }
            serde_json::Value::Number(n) => {
                if let Some(v) = n.as_u64() {
                    if v > 0 && v <= u16::MAX as u64 {
                        out.push(v as u16);
                    }
                }
            }
            _ => {}
        }
        out.sort_unstable();
        out.dedup();
        out
    }
    let raw: Raw = match serde_json::from_str(s) {
        Ok(r) => r,
        Err(_) => return ExclusionPorts::default(),
    };
    ExclusionPorts {
        tcp: extract(&raw.tcp),
        udp: extract(&raw.udp),
    }
}

#[cfg(not(windows))]
pub fn scan(_off_paths: &[String]) -> ExclusionPorts {
    ExclusionPorts::default()
}
