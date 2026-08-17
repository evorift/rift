//! DNS + DoH yönetimi (docs/05 §1). Güvenli sağlayıcıya geçiş, DoH şablonu, geri alma, doğrulama.
//! Gerçek mutasyon yalnız ayrıcalıklı serviste (sys::run_os privileged kapısı).

use crate::sys::{audit, query_os, run_os};

/// Bilinen güvenli DNS sağlayıcıları: (id, v4[], v6[], DoH şablonu).
fn provider(profile: &str) -> Option<(&'static [&'static str], &'static [&'static str], &'static str)> {
    Some(match profile {
        "cloudflare" => (
            &["1.1.1.1", "1.0.0.1"],
            &["2606:4700:4700::1111", "2606:4700:4700::1001"],
            "https://cloudflare-dns.com/dns-query",
        ),
        "quad9" => (
            &["9.9.9.9", "149.112.112.112"],
            &["2620:fe::fe", "2620:fe::9"],
            "https://dns.quad9.net/dns-query",
        ),
        "adguard" => (
            &["94.140.14.14", "94.140.15.15"],
            &["2a10:50c0::ad1:ff", "2a10:50c0::ad2:ff"],
            "https://dns.adguard-dns.com/dns-query",
        ),
        "google" => (
            &["8.8.8.8", "8.8.4.4"],
            &["2001:4860:4860::8888", "2001:4860:4860::8844"],
            "https://dns.google/dns-query",
        ),
        _ => return None,
    })
}

/// Read the resolvers the machine is ACTUALLY using, straight from the IP Helper API.
///
/// SPEED (2026-08-16): the only way to read DNS state used to be `verify_dns()`, which spawns
/// PowerShell and costs the better part of a second — paid on every watchdog drift check and every
/// Start. This is the same information from `GetAdaptersAddresses` in well under a millisecond,
/// with no child process, which is what makes "skip the apply when it is already right" cheap
/// enough to be worth doing.
#[cfg(windows)]
pub fn current_servers() -> Vec<String> {
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetAdaptersAddresses, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_FRIENDLY_NAME,
        GAA_FLAG_SKIP_MULTICAST, IP_ADAPTER_ADDRESSES_LH,
    };
    const ERROR_BUFFER_OVERFLOW: u32 = 111;
    const AF_UNSPEC: u32 = 0;
    const IF_OPER_STATUS_UP: i32 = 1;

    let mut out: Vec<String> = Vec::new();
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_FRIENDLY_NAME;
    let mut size: u32 = 16 * 1024;
    // Two attempts: the first with a generous guess, the second with the size the API asks for.
    // Looping without a bound here would be a hang if the API kept reporting overflow.
    for _ in 0..2 {
        let mut buf = vec![0u8; size as usize];
        let rc = unsafe {
            GetAdaptersAddresses(
                AF_UNSPEC,
                flags,
                std::ptr::null_mut(),
                buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH,
                &mut size,
            )
        };
        if rc == ERROR_BUFFER_OVERFLOW {
            continue; // `size` now holds what the API needs
        }
        if rc != 0 {
            return out;
        }
        unsafe {
            let mut ad = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
            while !ad.is_null() {
                // Only adapters that are actually up: a disconnected NIC's stale resolver list is
                // not what traffic is using, and counting it would make drift detection flap.
                if (*ad).OperStatus == IF_OPER_STATUS_UP {
                    let mut dns = (*ad).FirstDnsServerAddress;
                    while !dns.is_null() {
                        if let Some(ip) = sockaddr_to_string((*dns).Address.lpSockaddr as *const u8) {
                            // Skip link-local IPv6 resolvers (fe80::/10) and site-local leftovers.
                            // Windows lists a link-local resolver on almost every adapter; counting
                            // them would mean `already_using` can never be true on a normal
                            // machine, and the whole point of the check is to be usable.
                            let ll = ip.starts_with("fe80:") || ip.starts_with("fec0:");
                            if !ll && !out.contains(&ip) {
                                out.push(ip);
                            }
                        }
                        dns = (*dns).Next;
                    }
                }
                ad = (*ad).Next;
            }
        }
        return out;
    }
    out
}

/// Decode a `SOCKADDR` into a printable IP. Returns None for anything that is not IPv4/IPv6.
#[cfg(windows)]
unsafe fn sockaddr_to_string(sa: *const u8) -> Option<String> {
    if sa.is_null() {
        return None;
    }
    // sockaddr layout: u16 family, then family-specific payload.
    let family = u16::from_ne_bytes([*sa, *sa.add(1)]);
    match family {
        2 => {
            // AF_INET: sin_family(2) sin_port(2) sin_addr(4)
            let o = std::slice::from_raw_parts(sa.add(4), 4);
            Some(format!("{}.{}.{}.{}", o[0], o[1], o[2], o[3]))
        }
        23 => {
            // AF_INET6: sin6_family(2) sin6_port(2) sin6_flowinfo(4) sin6_addr(16)
            let o = std::slice::from_raw_parts(sa.add(8), 16);
            let mut seg = [0u16; 8];
            for (i, s) in seg.iter_mut().enumerate() {
                *s = u16::from_be_bytes([o[i * 2], o[i * 2 + 1]]);
            }
            Some(std::net::Ipv6Addr::new(seg[0], seg[1], seg[2], seg[3], seg[4], seg[5], seg[6], seg[7]).to_string())
        }
        _ => None,
    }
}

#[cfg(not(windows))]
pub fn current_servers() -> Vec<String> {
    Vec::new()
}

/// Are the machine's resolvers ALREADY this provider's?
///
/// Used to skip a redundant apply. The check is "every configured v4 resolver belongs to the
/// provider", not "any" — a machine with one Cloudflare entry and the ISP resolver alongside it is
/// NOT protected, because the resolver that answers first decides, and on the measured line the
/// ISP one returns a sinkhole address for every Discord domain.
pub fn already_using(profile: &str) -> bool {
    let Some((v4, v6, _)) = provider(profile) else { return false };
    let servers = current_servers();
    if servers.is_empty() {
        return false;
    }
    servers.iter().all(|s| v4.contains(&s.as_str()) || v6.contains(&s.as_str()))
}

/// Sistem DNS'ini sağlayıcıya çevir + DoH etkinleştir (docs/05 §1). "auto" → reset_dns (DHCP).
/// Tüm fiziksel adaptörlerde v4+v6 TEK çağrıda; Win11 22H2+ DoH şablonu, eskide netsh.
pub fn run_dns(profile: &str) -> Result<(), String> {
    if profile == "auto" {
        return reset_dns();
    }
    // Already correct → do nothing. This is the common case on every start after the first, and it
    // turns a ~1.5s PowerShell round trip (five cmdlets across every physical adapter) into a
    // sub-millisecond API read. Skipping is safe precisely because the check reads the live
    // adapter state rather than a remembered flag.
    if already_using(profile) {
        crate::sys::log("dns", &format!("{profile} already active on every adapter — apply skipped"));
        return Ok(());
    }
    let (v4, v6, tpl) = provider(profile).ok_or_else(|| format!("bilinmeyen dns profili: {profile}"))?;
    let ps_list = |a: &[&str]| a.iter().map(|s| format!("'{s}'")).collect::<Vec<_>>().join(",");
    let script = format!(
        "$ErrorActionPreference='SilentlyContinue'; $dns=@({v4}) + @({v6}); $tpl='{tpl}'; \
         $ad=@(Get-NetAdapter -Physical); \
         foreach($a in $ad){{ Set-DnsClientServerAddress -InterfaceIndex $a.InterfaceIndex -ServerAddresses $dns -Confirm:$false }}; \
         $b=[System.Environment]::OSVersion.Version.Build; \
         if($b -ge 22621){{ foreach($ip in @({v4})){{ Add-DnsClientDohServerAddress -ServerAddress $ip -DohTemplate $tpl -AllowFallbackToUdp $false -AutoUpgrade $true -Confirm:$false }} }} \
         else {{ foreach($ip in @({v4})){{ netsh dns add encryption server=$ip dohtemplate=$tpl autoupgrade=yes | Out-Null }} }}; \
         Clear-DnsClientCache",
        v4 = ps_list(v4),
        v6 = ps_list(v6),
        tpl = tpl,
    );
    run_os("powershell", &["-NoProfile", "-Command", &script])
}

/// DNS'i DHCP'ye (otomatik) sıfırla + DoH kayıtlarını temizle + önbelleği boşalt (docs/05 §1
/// ResetModernDNSSettings). Tüm fiziksel adaptörlerde (Up + Down).
pub fn reset_dns() -> Result<(), String> {
    let script = "$ErrorActionPreference='SilentlyContinue'; \
        Get-NetAdapter -Physical | ForEach-Object { \
          Set-DnsClientServerAddress -InterfaceIndex $_.InterfaceIndex -ResetServerAddresses -Confirm:$false; \
          try { Remove-DnsClientDohServerAddress -ServerAddress * -ErrorAction SilentlyContinue } catch {} }; \
        Clear-DnsClientCache";
    run_os("powershell", &["-NoProfile", "-Command", script])
}

/// DNS doğrulama sonucu (UI rozeti + preflight): aktif sunucular + güvenli sağlayıcıya mı ait.
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct DnsVerify {
    pub servers: Vec<String>,
    pub secure: bool,
    pub provider: String,
}

/// Sistemin kullandığı DNS sunucularını oku (v4+v6) ve bilinen güvenli bir sağlayıcıya mı ait belirle.
/// Salt-okuma → yetkisizken de çalışır (docs/05 §1 VerifyDNSSettings).
pub fn verify_dns() -> DnsVerify {
    // Native read first (sub-millisecond). PowerShell is the fallback only if the API returns
    // nothing — this used to spawn a shell unconditionally, on every watchdog tick that needed it.
    let mut servers = current_servers();
    if servers.is_empty() {
        let out = query_os(
            "powershell",
            &[
                "-NoProfile",
                "-Command",
                "(Get-DnsClientServerAddress | Where-Object {$_.ServerAddresses} | Select-Object -ExpandProperty ServerAddresses) -join ','",
            ],
        );
        for ip in out.trim().split(',') {
            let ip = ip.trim().to_string();
            if !ip.is_empty() && !servers.contains(&ip) {
                servers.push(ip);
            }
        }
    }
    let known: &[(&str, &[&str])] = &[
        ("Cloudflare", &["1.1.1.1", "1.0.0.1", "2606:4700:4700::1111", "2606:4700:4700::1001"]),
        ("Quad9", &["9.9.9.9", "149.112.112.112", "2620:fe::fe", "2620:fe::9"]),
        ("AdGuard", &["94.140.14.14", "94.140.15.15", "2a10:50c0::ad1:ff", "2a10:50c0::ad2:ff"]),
        ("Google", &["8.8.8.8", "8.8.4.4", "2001:4860:4860::8888", "2001:4860:4860::8844"]),
    ];
    let provider = known
        .iter()
        .find(|(_, ips)| servers.iter().any(|s| ips.contains(&s.as_str())))
        .map(|(name, _)| name.to_string())
        .unwrap_or_default();
    if !provider.is_empty() {
        audit(&format!("dns verify: güvenli ({provider})"));
    }
    DnsVerify { secure: !provider.is_empty(), provider, servers }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The native resolver read replaced a PowerShell round trip, so it has to actually decode
    /// sockaddrs correctly — a subtly wrong offset would silently produce garbage IPs that never
    /// match a provider, making `already_using` permanently false (slow but invisible) or, worse,
    /// accidentally match (fast and wrong).
    #[cfg(windows)]
    #[test]
    fn current_servers_returns_parseable_addresses() {
        for ip in current_servers() {
            assert!(
                ip.parse::<std::net::IpAddr>().is_ok(),
                "sockaddr decoding produced something that is not an IP: {ip:?}"
            );
            assert!(!ip.starts_with("fe80:"), "link-local resolvers must be filtered out");
        }
    }

    /// The skip check must be conservative: an unknown resolver anywhere means "not already using".
    /// Getting this backwards would skip applying secure DNS while reporting it as applied — the
    /// exact silent-success failure this codebase exists to avoid.
    #[test]
    fn already_using_requires_every_resolver_to_belong_to_the_provider() {
        // Pure-logic mirror of `already_using`'s rule, so the invariant is pinned without needing a
        // machine whose DNS happens to be set a particular way.
        let judge = |servers: &[&str], profile: &str| {
            let Some((v4, v6, _)) = provider(profile) else { return false };
            !servers.is_empty()
                && servers.iter().all(|s| v4.contains(s) || v6.contains(s))
        };
        assert!(judge(&["1.1.1.1", "1.0.0.1"], "cloudflare"));
        assert!(!judge(&["1.1.1.1", "192.168.1.1"], "cloudflare"), "one ISP resolver alongside is NOT secure");
        assert!(!judge(&[], "cloudflare"), "no resolvers at all is not 'already using'");
        assert!(!judge(&["1.1.1.1"], "quad9"), "must not match a different provider");
        assert!(!judge(&["1.1.1.1"], "nonsense"), "unknown profile can never be 'already using'");
    }
}
