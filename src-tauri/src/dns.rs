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

/// Sistem DNS'ini sağlayıcıya çevir + DoH etkinleştir (docs/05 §1). "auto" → reset_dns (DHCP).
/// Tüm fiziksel adaptörlerde v4+v6 TEK çağrıda; Win11 22H2+ DoH şablonu, eskide netsh.
pub fn run_dns(profile: &str) -> Result<(), String> {
    if profile == "auto" {
        return reset_dns();
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
    let out = query_os(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "(Get-DnsClientServerAddress | Where-Object {$_.ServerAddresses} | Select-Object -ExpandProperty ServerAddresses) -join ','",
        ],
    );
    let mut servers: Vec<String> = Vec::new();
    for ip in out.trim().split(',') {
        let ip = ip.trim().to_string();
        if !ip.is_empty() && !servers.contains(&ip) {
            servers.push(ip);
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
