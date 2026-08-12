//! Real proof-of-protection (evorift-remote-testing: "silent success is the enemy").
//!
//! `running: true` only proves the winws/byedpi/goodbyedpi PROCESS is alive — DPI can still RST or
//! blackhole the connection underneath it, and a UI that reports "Protected" from process liveness
//! alone is exactly the silent-success bug this module exists to close. This runs a real,
//! certificate-validated TLS handshake against live Discord endpoints and answers the harder
//! question: did traffic actually get through. Always runs on a background thread (service.rs
//! spawns it after Start/ApplyProfile/boot auto-start succeeds) — never blocks the caller.

use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, RootCertStore};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

/// Discord alan adları — 3 bağımsız uç (ana site, gateway, CDN), farklı altyapı katmanlarında.
pub const PROBE_TARGETS: &[&str] = &["discord.com", "gateway.discord.gg", "cdn.discordapp.com"];
/// 3 hedeften en az kaçının geçmesi gerekir. Tek ucun geçici arızası "Broken" yalanı üretmesin —
/// ama DPI hâlâ engelliyorsa (çoğu/tümü RST/timeout/sahte sertifika) Broken doğru sonuç olarak kalsın.
const MIN_OK: usize = 2;
const IO_TIMEOUT: Duration = Duration::from_secs(6);

pub struct ProbeOutcome {
    pub ok: bool,
    /// Verified iken boş; aksi halde ilk başarısız hedefin nedeni (kullanıcıya gösterilir).
    pub reason: String,
}

/// `PROBE_TARGETS`'ın tümünü sırayla dener (arka plan thread'inde birkaç saniye sürer, paralelliğe
/// gerek yok). `MIN_OK` kadarı tam TLS handshake + sertifika doğrulamasını geçerse Ok.
pub fn probe_discord() -> ProbeOutcome {
    let roots = match load_roots() {
        Ok(r) => r,
        Err(e) => return ProbeOutcome { ok: false, reason: format!("kök sertifika deposu yüklenemedi: {e}") },
    };
    let mut ok_count = 0usize;
    let mut first_failure = String::new();
    for host in PROBE_TARGETS {
        match probe_one(host, &roots) {
            Ok(()) => ok_count += 1,
            Err(e) if first_failure.is_empty() => first_failure = format!("{host}: {e}"),
            Err(_) => {}
        }
    }
    if ok_count >= MIN_OK {
        ProbeOutcome { ok: true, reason: String::new() }
    } else {
        ProbeOutcome { ok: false, reason: first_failure }
    }
}

fn load_roots() -> Result<Arc<RootCertStore>, String> {
    let native = rustls_native_certs::load_native_certs();
    if native.certs.is_empty() {
        return Err("sistem kök sertifika deposu boş döndü".into());
    }
    let mut store = RootCertStore::empty();
    let (added, _rejected) = store.add_parsable_certificates(native.certs);
    if added == 0 {
        return Err("hiçbir kök sertifika ayrıştırılamadı".into());
    }
    Ok(Arc::new(store))
}

/// Tek hedefe: DNS çözümle → TCP bağlan → TAM TLS handshake (sertifika doğrulamalı) tamamla.
/// DPI'nin RST/blackhole ettiği ya da sahte sertifika enjekte ettiği durumların HERHANGİ biri Err
/// döner — ikisi de "gerçekten geçti mi" sorusunun parçası, yalnız TCP bağlantısı yeterli değil.
fn probe_one(host: &str, roots: &Arc<RootCertStore>) -> Result<(), String> {
    let addr = format!("{host}:443")
        .to_socket_addrs()
        .map_err(|e| format!("DNS: {e}"))?
        .next()
        .ok_or_else(|| "DNS: adres döndürmedi".to_string())?;

    let mut tcp = TcpStream::connect_timeout(&addr, IO_TIMEOUT).map_err(|e| format!("TCP: {e}"))?;
    tcp.set_read_timeout(Some(IO_TIMEOUT)).map_err(|e| format!("TCP: {e}"))?;
    tcp.set_write_timeout(Some(IO_TIMEOUT)).map_err(|e| format!("TCP: {e}"))?;

    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| format!("tls yapılandırma: {e}"))?
        .with_root_certificates(Arc::clone(roots))
        .with_no_client_auth();
    let server_name = ServerName::try_from(host.to_string()).map_err(|e| format!("SNI: {e}"))?;
    let mut conn =
        ClientConnection::new(Arc::new(config), server_name).map_err(|e| format!("tls bağlantı: {e}"))?;

    // rustls kendi I/O'sunu yapmaz — sürücü döngüsü elle sürülür. connect_timeout zaten TCP
    // RST/timeout'u (DPI'nin en olağan tepkisi) yakalar; process_new_packets sahte/geçersiz
    // sertifikayı (MITM/blok sayfası) yakalar.
    while conn.is_handshaking() {
        if conn.wants_write() {
            conn.write_tls(&mut tcp).map_err(|e| format!("tls yazma: {e}"))?;
        }
        if conn.wants_read() {
            let n = conn.read_tls(&mut tcp).map_err(|e| format!("tls okuma: {e}"))?;
            if n == 0 {
                return Err("bağlantı handshake tamamlanmadan kapandı".into());
            }
            conn.process_new_packets().map_err(|e| format!("tls doğrulama: {e}"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A live TLS handshake belongs in evorift-live-verification, not a unit test — but the quorum
    /// arithmetic (MIN_OK against PROBE_TARGETS) is pure and must stay coherent: MIN_OK <= len, and
    /// > half so one target's transient failure can't silently satisfy "verified" alone.
    #[test]
    fn quorum_is_coherent_with_the_target_list() {
        assert!(!PROBE_TARGETS.is_empty());
        assert!(MIN_OK <= PROBE_TARGETS.len(), "MIN_OK can never require more targets than exist");
        assert!(MIN_OK * 2 > PROBE_TARGETS.len(), "MIN_OK must be a majority, not a minority");
    }
}
