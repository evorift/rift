//! "Off" modlu uygulamaların ÇALIŞAN PID'lerinin TCP+UDP source port'larını NATIVE Win32 ile tara →
//! engine.rs WinwsEngine bunları WinDivert capture filter'ında EXCLUDE eder → o uygulamanın paketleri
//! winws'e ULAŞMAZ → gerçek "off" mod (DPI'dan tamamen hariç).
//!
//! NEDEN PowerShell DEĞİL: Eski yol her ~5sn'de bir `powershell.exe` (+ `conhost.exe`) çağırıyordu →
//! döngüde süreç yığılması + %100 CPU + AV malware sezgisi. Artık `netinfo` ile (GetExtendedTcpTable /
//! GetExtendedUdpTable + QueryFullProcessImageNameW) süreç-İÇİNDE okunur → sıfır alt-süreç. Tarama
//! atomik: tek soket snapshot'ı, sahip PID'lerin yolu çözülüp off listesiyle eşleştirilir.

use crate::netinfo;

#[derive(Debug, Default, Clone, Eq)]
pub struct ExclusionPorts {
    pub tcp: Vec<u16>,
    pub udp: Vec<u16>,
    /// PIDs of the "off" apps these ports were collected from.
    ///
    /// THIS is the identity of an exclusion set, not the ports — see the `PartialEq` impl below.
    pub pids: Vec<u32>,
}

/// Two exclusion sets are "the same" when they describe the same PROCESSES, even if the individual
/// source ports differ.
///
/// FIXED 2026-08-16, from a log the user sent: `winws yeniden baslatiliyor` every 60 seconds, for as
/// long as the app was open. Applying an exclusion set restarts winws (there is no filter hot
/// reload), and the set was compared BY PORT. Source ports are ephemeral by definition — Steam and
/// OneDrive ship "off" by default and open and close connections constantly — so the set differed on
/// essentially every check, and the engine tore itself down and rebuilt once per rate-limit window,
/// killing every live connection on the machine each time. That is the "works, then stops, then
/// works" behaviour, and it was structural: no rate limit fixes a comparison that is always unequal.
///
/// Comparing by PID makes the trigger "an off-app started or exited", which is a real, infrequent
/// event. The cost is bounded and small: ports a tracked process opens BETWEEN restarts are not
/// excluded until the next one, so that app keeps being bypassed for a while. Being bypassed for a
/// few minutes is a far smaller harm than dropping every connection on the machine every minute.
impl PartialEq for ExclusionPorts {
    fn eq(&self, other: &Self) -> bool {
        self.pids == other.pids
    }
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
    // Sorted + deduped so the PID set is a stable identity: the enumeration order of sockets must
    // not make an unchanged set of processes look like a changed one.
    let mut pids: Vec<u32> = pid_is_off.iter().filter(|(_, off)| **off).map(|(pid, _)| *pid).collect();
    pids.sort_unstable();
    pids.dedup();
    ExclusionPorts { tcp, udp, pids }
}
