//! Ön-uçuş (preflight) + teşhis (docs/07 §8, docs/06 §2.5) — "neden çalışmıyor?" sorusunu uygulama
//! kendisi cevaplar. Açılışta + her kurulumdan önce çalışır; her kontrol somut bir öneri üretir.

use serde::{Deserialize, Serialize};
use crate::sys::{is_elevated, query_os};

/// Tek bir ön-uçuş kontrolü.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Check {
    pub name: String,
    pub pass: bool,
    /// Başarısızsa kullanıcıya somut öneri (docs/06 §2.5). Başarılıysa kısa onay.
    pub hint: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreflightResult {
    /// Kritik kontrollerin tamamı geçti mi (kurulum güvenli mi)?
    pub ok: bool,
    pub checks: Vec<Check>,
}

/// Windows build numarası (DoH için ≥22621 = Win11 22H2). Alınamazsa 0.
fn windows_build() -> u32 {
    let out = query_os(
        "powershell",
        &["-NoProfile", "-Command", "[System.Environment]::OSVersion.Version.Build"],
    );
    out.trim().parse().unwrap_or(0)
}

/// Başka bir süreç FARKLI sürüm WinDivert sürücüsü yüklemiş mi (çakışma → winws anında ölür)?
/// `sc query` ile WinDivert servis(ler)inin RUNNING durumunu kontrol et. Bizim winws henüz
/// başlamadan RUNNING bir WinDivert → yabancı (zapret/GoodbyeDPI/eski kurulum) → çakışma riski.
pub fn windivert_conflict() -> Option<String> {
    for name in ["WinDivert", "WinDivert1.4", "WinDivert1.1"] {
        let q = query_os("sc", &["query", name]);
        if q.contains("RUNNING") {
            return Some(name.to_string());
        }
    }
    None
}

/// WARP bundle (wireguard.exe + wgcf.exe) mevcut mu? Yoksa tünel yöntemleri (Tam Koruma / Discord
/// split-tunnel) çalışmaz → UI gri-leştirir, desync önerilir.
fn warp_available() -> bool {
    let dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("warp")));
    match dir {
        Some(d) => d.join("wireguard.exe").exists() && d.join("wgcf.exe").exists(),
        None => false,
    }
}

/// Tüm ön-uçuş kontrollerini çalıştır (docs/07 §8 tablosu).
pub fn run() -> PreflightResult {
    let mut checks = Vec::new();

    // 1) Yönetici mi? (kritik — gerçek mutasyon yalnız elevated/servis)
    let admin = is_elevated() || std::env::var("EVORIFT_PRIVILEGED").is_ok();
    checks.push(Check {
        name: "Yönetici / Servis".into(),
        pass: admin,
        hint: if admin {
            "Ayrıcalıklı çalışıyor.".into()
        } else {
            "Yönetici değil — gerçek koruma için 'Yönetici olarak yeniden başlat' veya servisi kullan.".into()
        },
    });

    // 2) winws (zapret) bundle mevcut mu?
    let winws = crate::engine::make_engine("zapret").is_available();
    checks.push(Check {
        name: "DPI motoru (winws)".into(),
        pass: winws,
        hint: if winws {
            "winws bundle hazır.".into()
        } else {
            "winws bundle bulunamadı — kurulum eksik olabilir (resources/winws).".into()
        },
    });

    // 3) WinDivert çakışması var mı?
    let conflict = windivert_conflict();
    checks.push(Check {
        name: "WinDivert çakışması".into(),
        pass: conflict.is_none(),
        hint: match &conflict {
            None => "Çakışan WinDivert sürücüsü yok.".into(),
            Some(svc) => format!(
                "Yabancı WinDivert sürücüsü çalışıyor ({svc}) — başka bir DPI aracını (zapret/GoodbyeDPI) durdur; evorift başlatınca otomatik temizlenir."
            ),
        },
    });

    // 4) ByeDPI (kernel-siz) mevcut mu? (Kaspersky/AV WinDivert'i engellerse alternatif)
    let byedpi = crate::engine::make_engine("byedpi").is_available();
    checks.push(Check {
        name: "Kernel-siz motor (ByeDPI)".into(),
        pass: byedpi,
        hint: if byedpi {
            "ByeDPI hazır — WinDivert engelliyse (Kaspersky) alternatif var.".into()
        } else {
            "ByeDPI bundle yok — AV WinDivert'i engellerse yedek motor olmaz.".into()
        },
    });

    // 5) WARP tüneli mevcut mu? (Discord masaüstü + Tam Koruma)
    let warp = warp_available();
    checks.push(Check {
        name: "WARP tüneli".into(),
        pass: warp,
        hint: if warp {
            "WARP bundle hazır (Discord split-tunnel + Tam Koruma).".into()
        } else {
            "WARP bundle yok — Discord masaüstü/ses ve Tam Koruma çalışmaz; desync kullanılır.".into()
        },
    });

    // 6) DoH desteği (Win11 22H2+)
    let build = windows_build();
    let doh = build >= 22621;
    checks.push(Check {
        name: "DoH (şifreli DNS)".into(),
        pass: doh,
        hint: if doh {
            format!("Windows build {build} — yerleşik DoH destekleniyor.")
        } else if build == 0 {
            "Windows sürümü okunamadı.".into()
        } else {
            format!("Windows build {build} — yerleşik DoH yok; netsh DoH veya yerel proxy kullanılır.")
        },
    });

    // 7) AV that blocks WinDivert (notably Kaspersky) — item 8.3.
    let av = av_blocks_windivert();
    checks.push(Check {
        name: "AV / WinDivert block".into(),
        pass: av.is_none(),
        hint: match &av {
            None => "No WinDivert-blocking AV detected.".into(),
            Some(name) => format!(
                "{name} detected — it blocks WinDivert, so desync engines (winws/GoodbyeDPI) won't run. Use the kernel-less ByeDPI engine instead."
            ),
        },
    });

    // 8) Visual C++ runtime present? (winws/ciadpi link against it.)
    let vc = vc_redist_present();
    checks.push(Check {
        name: "Visual C++ runtime".into(),
        pass: vc,
        hint: if vc {
            "VC++ runtime present.".into()
        } else {
            "Visual C++ runtime not found — winws/ciadpi may fail to start; install the VC++ redistributable.".into()
        },
    });

    // 9) Windows Packet Filter (NDIS) present? (ProxiFyre needs it.)
    let pf = packet_filter_present();
    checks.push(Check {
        name: "Windows Packet Filter".into(),
        pass: pf,
        hint: if pf {
            "Packet Filter driver present (ProxiFyre routing available).".into()
        } else {
            "Windows Packet Filter (NDIS) not installed — ByeDPI+ProxiFyre split-tunnel won't route; install it or use drover/desync.".into()
        },
    });

    // 10) WARP registration reachability (can wgcf reach Cloudflare?).
    let warp_reach = warp_register_reachable();
    checks.push(Check {
        name: "WARP register reachability".into(),
        pass: warp_reach,
        hint: if warp_reach {
            "Cloudflare WARP API reachable.".into()
        } else {
            "Can't reach the Cloudflare WARP API — registration may fail; use a DPI engine, or retry on another network.".into()
        },
    });

    // Critical checks: admin + winws + no conflict (the rest are informational).
    let ok = admin && winws && conflict.is_none();
    PreflightResult { ok, checks }
}

/// Detect an AV that blocks WinDivert (notably Kaspersky). Checks common Kaspersky service names; returns
/// the matched product label or `None`.
pub fn av_blocks_windivert() -> Option<String> {
    for svc in ["AVP", "AVPSus", "kavfs", "KLIF", "klam", "klflt"] {
        if crate::sys::query_os("sc", &["query", svc]).to_uppercase().contains("RUNNING") {
            return Some(format!("Kaspersky ({svc})"));
        }
    }
    None
}

/// Is the Visual C++ runtime present? (winws/ciadpi link against it.) Checks `vcruntime140.dll` in System32.
fn vc_redist_present() -> bool {
    std::env::var("SystemRoot")
        .ok()
        .map(|r| std::path::Path::new(&r).join("System32").join("vcruntime140.dll").exists())
        .unwrap_or(false)
}

/// Is the Windows Packet Filter (NDIS) driver present? (ProxiFyre needs it.) Checks the `ndisrd` service.
fn packet_filter_present() -> bool {
    crate::sys::query_os("sc", &["query", "ndisrd"]).contains("STATE")
}

/// Can we reach Cloudflare's WARP registration API? TCP probe only (no data sent).
fn warp_register_reachable() -> bool {
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration;
    "api.cloudflareclient.com:443"
        .to_socket_addrs()
        .ok()
        .and_then(|mut a| a.next())
        .map(|addr| TcpStream::connect_timeout(&addr, Duration::from_secs(3)).is_ok())
        .unwrap_or(false)
}

/// Hedef site teşhisi (docs/06 §2.5): her hedef için DNS çözümleme + TCP erişim + gecikme. UI'de
/// "Discord açılmıyor" → hangi basamağın koptuğunu gösterir. Yetkisiz/UI bağlamında da çalışır (salt-okuma).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TargetDiag {
    pub target: String,
    pub dns_ok: bool,
    pub tcp_ok: bool,
    /// Did the TLS ClientHello (with this host's SNI) get a real response, or was it SNI-reset? (item 8.4)
    pub tls_ok: bool,
    pub ms: u32,
    /// Concrete suggestion from the first failing step (docs/06 §2.5).
    pub suggestion: String,
}

/// Concrete suggestion from the first failing step of the DNS→TCP→TLS chain (item 8.4). Pure → testable.
fn diagnose_suggestion(dns_ok: bool, tcp_ok: bool, tls_ok: bool) -> String {
    if !dns_ok {
        "DNS resolution failed — set a secure DNS provider (Cloudflare/Quad9) and flush the cache.".into()
    } else if !tcp_ok {
        "TCP connect blocked — the ISP may be IP-blocking; try the WARP tunnel for this app.".into()
    } else if !tls_ok {
        "TLS handshake was reset — SNI-based DPI block; enable a DPI engine (winws) or add the domain to the bypass list.".into()
    } else {
        "Reachable — DNS, TCP and TLS all succeeded.".into()
    }
}

/// Diagnose each target through DNS → TCP/443 → TLS ClientHello (item 8.4), classifying the failing step
/// and returning a concrete suggestion. Read-only; runs in any context.
pub fn diagnose(targets: &[String]) -> Vec<TargetDiag> {
    use std::io::{Read, Write};
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::{Duration, Instant};
    let mut out = Vec::new();
    for t in targets {
        let host = t.trim();
        if host.is_empty() {
            continue;
        }
        let hostport = format!("{host}:443");
        let (mut dns_ok, mut tcp_ok, mut tls_ok, mut ms) = (false, false, false, 0u32);
        if let Ok(mut addrs) = hostport.to_socket_addrs() {
            if let Some(addr) = addrs.next() {
                dns_ok = true; // 1) DNS
                let st = Instant::now();
                if let Ok(mut s) = TcpStream::connect_timeout(&addr, Duration::from_secs(3)) {
                    tcp_ok = true; // 2) TCP
                    let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
                    let _ = s.set_write_timeout(Some(Duration::from_secs(2)));
                    // 3) TLS ClientHello with SNI → is it answered, or SNI-reset?
                    if s.write_all(&crate::autopilot::client_hello(host)).is_ok() {
                        let mut buf = [0u8; 8];
                        tls_ok = matches!(s.read(&mut buf), Ok(n) if n >= 1 && crate::autopilot::tls_responded(Some(buf[0])));
                    }
                }
                ms = st.elapsed().as_millis().min(u32::MAX as u128) as u32;
            }
        }
        let suggestion = diagnose_suggestion(dns_ok, tcp_ok, tls_ok);
        out.push(TargetDiag { target: host.to_string(), dns_ok, tcp_ok, tls_ok, ms, suggestion });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Item 8.3: preflight includes the AV / VC++ / Packet-Filter / WARP-register checks, and every check
    /// carries a non-empty hint.
    #[test]
    fn preflight_full_checks_have_hints() {
        let r = run();
        let names: Vec<&str> = r.checks.iter().map(|c| c.name.as_str()).collect();
        assert!(names.iter().any(|n| n.contains("AV")), "AV/WinDivert check present");
        assert!(names.iter().any(|n| n.contains("Visual C++")), "VC++ runtime check present");
        assert!(names.iter().any(|n| n.contains("Packet Filter")), "Packet Filter check present");
        assert!(names.iter().any(|n| n.contains("WARP register")), "WARP register check present");
        assert!(r.checks.iter().all(|c| !c.hint.is_empty()), "every check has a hint");
    }

    /// Item 8.4: the diagnose chain yields a per-step suggestion (DNS → TCP → TLS).
    #[test]
    fn diagnose_suggestions_per_step() {
        assert!(diagnose_suggestion(false, false, false).contains("DNS"));
        assert!(diagnose_suggestion(true, false, false).contains("TCP"));
        assert!(diagnose_suggestion(true, true, false).contains("TLS"));
        assert!(diagnose_suggestion(true, true, true).contains("Reachable"));
    }
}
