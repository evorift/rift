//! "Off" modlu uygulamaların ÇALIŞAN PID'lerinin TCP+UDP source port'larını NATIVE Win32 ile tara →
//! engine.rs WinwsEngine bunları WinDivert capture filter'ında EXCLUDE eder → o uygulamanın paketleri
//! winws'e ULAŞMAZ → gerçek "off" mod (DPI'dan tamamen hariç).
//!
//! NEDEN PowerShell DEĞİL: Eski yol her ~5sn'de bir `powershell.exe` (+ `conhost.exe`) çağırıyordu →
//! döngüde süreç yığılması + %100 CPU + AV malware sezgisi. Artık `netinfo` ile (GetExtendedTcpTable /
//! GetExtendedUdpTable + QueryFullProcessImageNameW) süreç-İÇİNDE okunur → sıfır alt-süreç. Tarama
//! atomik: tek soket snapshot'ı, sahip PID'lerin yolu çözülüp off listesiyle eşleştirilir.

use crate::netinfo;

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

/// Verilen exe yolları için çalışan PID'lerin TCP+UDP source port'larını topla (native, alt-süreç YOK).
/// Yollar yokSayılır (boş, kayıp). Bulamazsa boş döner — engine eski catch-all yola düşer.
pub fn scan(off_paths: &[String]) -> ExclusionPorts {
    // off yolları küçük harf seti → büyük/küçük harf duyarsız eşleştirme (Windows yolları case-insensitive).
    let wanted: std::collections::HashSet<String> = off_paths
        .iter()
        .filter(|p| !p.is_empty())
        .map(|p| p.to_lowercase())
        .collect();
    if wanted.is_empty() {
        return ExclusionPorts::default();
    }

    let socks = netinfo::sockets();
    if socks.is_empty() {
        return ExclusionPorts::default();
    }

    // 1) Yalnız soket SAHİBİ PID'lerin yolunu çöz (tüm süreçleri taramaktan kaçın) → off listede mi bak.
    //    PID → "off mu" haritası (yol bir kez çözülür; aynı PID'in çok soketi için tekrar OpenProcess yok).
    let mut pid_is_off: std::collections::HashMap<u32, bool> = std::collections::HashMap::new();
    for pid in netinfo::socket_pids(&socks) {
        let is_off = netinfo::pid_exe_path(pid)
            .map(|p| wanted.contains(&p.to_lowercase()))
            .unwrap_or(false);
        pid_is_off.insert(pid, is_off);
    }

    // 2) Off PID'lerinin TCP/UDP yerel (source) port'larını topla.
    let mut tcp: Vec<u16> = Vec::new();
    let mut udp: Vec<u16> = Vec::new();
    for s in &socks {
        if s.local_port == 0 {
            continue;
        }
        if pid_is_off.get(&s.pid).copied().unwrap_or(false) {
            if s.tcp {
                tcp.push(s.local_port);
            } else {
                udp.push(s.local_port);
            }
        }
    }
    tcp.sort_unstable();
    tcp.dedup();
    udp.sort_unstable();
    udp.dedup();
    ExclusionPorts { tcp, udp }
}
