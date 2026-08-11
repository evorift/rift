//! Native Win32 ağ/süreç enumerasyonu — PowerShell/tasklist alt-süreçlerine SIFIR ihtiyaç.
//!
//! NEDEN: Eski yol her tarama için `powershell.exe` (+ `conhost.exe`) çağırıyordu. Her çağrı tüm .NET
//! CLR'ını yükler (1-3 sn CPU) → döngüde çalışınca yüzlerce kısa-ömürlü süreç yığılır → %100 CPU +
//! AV/EDR malware sezgisi. Aynı veriyi (soket→PID haritası + exe yolu) IP Helper / kernel32 ile
//! SÜREÇ İÇİNDE okuruz → sıfır alt-süreç. service.rs'teki `GetIfTable2` deseninin birebir kardeşi.
//!
//! Sağladıkları:
//!  - [`sockets`] — tüm TCP+UDP soketleri (yerel port, TCP uzak IP, sahip PID) — `GetExtendedTcpTable`
//!    / `GetExtendedUdpTable`, hem AF_INET (2) hem AF_INET6 (23), iki-geçişli boyut deseni.
//!  - [`pid_exe_path`] — PID → tam exe yolu — `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` +
//!    `QueryFullProcessImageNameW` + `CloseHandle` (cross-bitness; elevated servisten başka kullanıcı
//!    süreçleri de okunur).

use std::net::IpAddr;

/// Tek bir soket kaydı (TCP veya UDP). UDP'de `remote` daima `None` (bağlantısız).
#[derive(Debug, Clone)]
pub struct Socket {
    /// Sahip süreç PID'i.
    pub pid: u32,
    /// Yerel (kaynak) port — host byte order.
    pub local_port: u16,
    /// TCP uzak (hedef) IP — yalnız `Established` TCP'de dolu; UDP/dinleyen → `None`.
    pub remote: Option<IpAddr>,
    /// true = TCP, false = UDP.
    pub tcp: bool,
}

#[cfg(windows)]
const AF_INET: u32 = 2;
#[cfg(windows)]
const AF_INET6: u32 = 23;
/// MIB_TCP_STATE_ESTAB = 5 (windows-sys 0.52). Kurulu bağlantı (uzak IP anlamlı).
#[cfg(windows)]
const TCP_STATE_ESTAB: u32 = 5;

/// Tüm TCP + UDP soketlerini (IPv4 + IPv6) süreç-içi enumerate et. Hata/boş → boş vektör (zarif).
#[cfg(windows)]
pub fn sockets() -> Vec<Socket> {
    let mut out = Vec::new();
    unsafe {
        tcp_table(AF_INET, &mut out);
        tcp_table(AF_INET6, &mut out);
        udp_table(AF_INET, &mut out);
        udp_table(AF_INET6, &mut out);
    }
    out
}

#[cfg(not(windows))]
pub fn sockets() -> Vec<Socket> {
    Vec::new()
}

/// İki-geçişli boyut deseni ortak yardımcı: NULL tampon → boyut → gerçek tampon. `f` çağrısı
/// `GetExtended*Table`'ı sarmalar; başarıda doldurulmuş bayt tamponunu döndürür.
#[cfg(windows)]
unsafe fn two_pass<F>(mut call: F) -> Option<Vec<u8>>
where
    F: FnMut(*mut core::ffi::c_void, *mut u32) -> u32,
{
    use windows_sys::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
    let mut size: u32 = 0;
    // 1. geçiş: boyut öğren (NULL tampon). ERROR_INSUFFICIENT_BUFFER beklenir.
    let rc = call(std::ptr::null_mut(), &mut size);
    if rc != ERROR_INSUFFICIENT_BUFFER && rc != NO_ERROR {
        return None;
    }
    if size == 0 {
        return None;
    }
    // 2. geçiş: gerçek tampon. Yarış (sürücü tablosu büyüdü) ihtimaline karşı birkaç deneme.
    for _ in 0..4 {
        let mut buf = vec![0u8; size as usize];
        let rc = call(buf.as_mut_ptr() as *mut _, &mut size);
        if rc == NO_ERROR {
            return Some(buf);
        }
        if rc != ERROR_INSUFFICIENT_BUFFER {
            return None;
        }
        // tampon büyüdü → güncellenmiş `size` ile yeniden dene
    }
    None
}

/// GetExtendedTcpTable → MIB_TCP*TABLE_OWNER_PID satırlarını `out`'a ekle.
#[cfg(windows)]
unsafe fn tcp_table(af: u32, out: &mut Vec<Socket>) {
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCP6TABLE_OWNER_PID, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_ALL,
    };
    let Some(buf) = two_pass(|ptr, sz| GetExtendedTcpTable(ptr, sz, 0, af, TCP_TABLE_OWNER_PID_ALL, 0))
    else {
        return;
    };
    if af == AF_INET {
        let table = &*(buf.as_ptr() as *const MIB_TCPTABLE_OWNER_PID);
        let n = table.dwNumEntries as usize;
        let rows = table.table.as_ptr();
        for i in 0..n {
            let row = &*rows.add(i);
            let remote = if row.dwState == TCP_STATE_ESTAB {
                // dwRemoteAddr ağ-byte-order u32; IPv4 oktetleri doğrudan ham baytlardan.
                let b = row.dwRemoteAddr.to_ne_bytes();
                Some(IpAddr::from([b[0], b[1], b[2], b[3]]))
            } else {
                None
            };
            out.push(Socket {
                pid: row.dwOwningPid,
                local_port: u16::from_be((row.dwLocalPort & 0xFFFF) as u16),
                remote,
                tcp: true,
            });
        }
    } else {
        let table = &*(buf.as_ptr() as *const MIB_TCP6TABLE_OWNER_PID);
        let n = table.dwNumEntries as usize;
        let rows = table.table.as_ptr();
        for i in 0..n {
            let row = &*rows.add(i);
            let remote = if row.dwState == TCP_STATE_ESTAB {
                Some(IpAddr::from(row.ucRemoteAddr))
            } else {
                None
            };
            out.push(Socket {
                pid: row.dwOwningPid,
                local_port: u16::from_be((row.dwLocalPort & 0xFFFF) as u16),
                remote,
                tcp: true,
            });
        }
    }
}

/// GetExtendedUdpTable → MIB_UDP*TABLE_OWNER_PID satırlarını `out`'a ekle (UDP'de uzak IP yok).
#[cfg(windows)]
unsafe fn udp_table(af: u32, out: &mut Vec<Socket>) {
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetExtendedUdpTable, MIB_UDP6TABLE_OWNER_PID, MIB_UDPTABLE_OWNER_PID, UDP_TABLE_OWNER_PID,
    };
    let Some(buf) = two_pass(|ptr, sz| GetExtendedUdpTable(ptr, sz, 0, af, UDP_TABLE_OWNER_PID, 0))
    else {
        return;
    };
    if af == AF_INET {
        let table = &*(buf.as_ptr() as *const MIB_UDPTABLE_OWNER_PID);
        let n = table.dwNumEntries as usize;
        let rows = table.table.as_ptr();
        for i in 0..n {
            let row = &*rows.add(i);
            out.push(Socket {
                pid: row.dwOwningPid,
                local_port: u16::from_be((row.dwLocalPort & 0xFFFF) as u16),
                remote: None,
                tcp: false,
            });
        }
    } else {
        let table = &*(buf.as_ptr() as *const MIB_UDP6TABLE_OWNER_PID);
        let n = table.dwNumEntries as usize;
        let rows = table.table.as_ptr();
        for i in 0..n {
            let row = &*rows.add(i);
            out.push(Socket {
                pid: row.dwOwningPid,
                local_port: u16::from_be((row.dwLocalPort & 0xFFFF) as u16),
                remote: None,
                tcp: false,
            });
        }
    }
}

/// PID → tam exe yolu. `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` + `QueryFullProcessImageNameW`.
/// PROCESS_QUERY_LIMITED_INFORMATION (0x1000) = minimal hak: cross-bitness çalışır, elevated servisten
/// başka kullanıcı/oturum süreçlerinin yolu da alınır. Erişilemeyen (System/Idle/korumalı) PID → None.
#[cfg(windows)]
pub fn pid_exe_path(pid: u32) -> Option<String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    if pid <= 4 {
        return None; // System Idle (0) / System (4) → yol yok
    }
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h == 0 {
            return None; // erişim reddi / artık yaşamıyor
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len);
        CloseHandle(h);
        if ok == 0 || len == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

#[cfg(not(windows))]
pub fn pid_exe_path(_pid: u32) -> Option<String> {
    None
}

/// Soketleri olan benzersiz PID'lerin sıralı listesi (System/Idle hariç). PID→yol çözümleme için.
pub fn socket_pids(socks: &[Socket]) -> Vec<u32> {
    let mut pids: Vec<u32> = socks.iter().map(|s| s.pid).filter(|&p| p > 4).collect();
    pids.sort_unstable();
    pids.dedup();
    pids
}

// ---- En-iyi-çaba ters-DNS (GetNameInfoW), SÜREÇ-İÇİ + ÖNBELLEKLİ ----
//
// UYARI (V0.1.3 plan §b.6): ters-DNS bloke eder, başarısızlıkta zaman aşımına uğrar ve CDN-arkası
// servisler için kullanışsız PTR döndürür → "yalnız ipucu olarak, ASLA bağlantı başına döngüde
// kullanın" (Microsoft). Bu yüzden: sonuçları SÜRECİN ÖMRÜ boyunca önbelleğe alırız (aynı IP iki kez
// sorulmaz) ve çağıran taraf yalnız birkaç IP ile sınırlar. Negatif sonuç da önbeklenir (boş = sorma).

#[cfg(windows)]
static DNS_CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<std::net::IpAddr, Option<String>>>> =
    std::sync::OnceLock::new();
/// WSAStartup yalnız bir kez (ilk reverse_dns çağrısında). GetNameInfoW Winsock init gerektirir.
#[cfg(windows)]
static WSA_INIT: std::sync::Once = std::sync::Once::new();

/// IP → hostname (en-iyi-çaba, önbellekli). Bulamaz/sayısal döner/hata → None. Asla panik etmez.
#[cfg(windows)]
pub fn reverse_dns(ip: &std::net::IpAddr) -> Option<String> {
    let cache = DNS_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Ok(c) = cache.lock() {
        if let Some(hit) = c.get(ip) {
            return hit.clone(); // önbellek (pozitif veya negatif) → tekrar sorma
        }
    }
    let resolved = reverse_dns_uncached(ip);
    if let Ok(mut c) = cache.lock() {
        c.insert(*ip, resolved.clone());
    }
    resolved
}

#[cfg(windows)]
fn reverse_dns_uncached(ip: &std::net::IpAddr) -> Option<String> {
    use windows_sys::Win32::Networking::WinSock::{
        GetNameInfoW, WSAStartup, ADDRESS_FAMILY, AF_INET, AF_INET6, IN6_ADDR, IN6_ADDR_0, IN_ADDR,
        IN_ADDR_0, NI_NAMEREQD, NI_NUMERICSERV, SOCKADDR, SOCKADDR_IN, SOCKADDR_IN6, SOCKADDR_IN6_0,
        WSADATA,
    };
    WSA_INIT.call_once(|| unsafe {
        let mut data: WSADATA = std::mem::zeroed();
        let _ = WSAStartup(0x0202, &mut data); // MAKEWORD(2,2)
    });
    let mut host = [0u16; 256];
    let flags = (NI_NAMEREQD | NI_NUMERICSERV) as i32;
    // GetNameInfoW: sockaddr → host adı. SOCKADDR_IN/IN6'yı yığında tut (pointer çağrı boyunca geçerli).
    let rc = unsafe {
        match ip {
            std::net::IpAddr::V4(v4) => {
                let sa = SOCKADDR_IN {
                    sin_family: AF_INET as ADDRESS_FAMILY,
                    sin_port: 0,
                    sin_addr: IN_ADDR { S_un: IN_ADDR_0 { S_addr: u32::from_ne_bytes(v4.octets()) } },
                    sin_zero: [0; 8],
                };
                GetNameInfoW(
                    &sa as *const _ as *const SOCKADDR,
                    std::mem::size_of::<SOCKADDR_IN>() as i32,
                    host.as_mut_ptr(),
                    host.len() as u32,
                    std::ptr::null_mut(),
                    0,
                    flags,
                )
            }
            std::net::IpAddr::V6(v6) => {
                let sa = SOCKADDR_IN6 {
                    sin6_family: AF_INET6 as ADDRESS_FAMILY,
                    sin6_port: 0,
                    sin6_flowinfo: 0,
                    sin6_addr: IN6_ADDR { u: IN6_ADDR_0 { Byte: v6.octets() } },
                    Anonymous: SOCKADDR_IN6_0 { sin6_scope_id: 0 },
                };
                GetNameInfoW(
                    &sa as *const _ as *const SOCKADDR,
                    std::mem::size_of::<SOCKADDR_IN6>() as i32,
                    host.as_mut_ptr(),
                    host.len() as u32,
                    std::ptr::null_mut(),
                    0,
                    flags,
                )
            }
        }
    };
    if rc != 0 {
        return None;
    }
    let end = host.iter().position(|&c| c == 0).unwrap_or(host.len());
    if end == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&host[..end]))
}

#[cfg(not(windows))]
pub fn reverse_dns(_ip: &std::net::IpAddr) -> Option<String> {
    None
}
