//! Per-app firewall engeli (docs/03 §4.5). `netsh advfirewall` ARGV ile (shell yok → enjeksiyon yok).
//! block=false → kuralı kaldır. Gerçek mutasyon yalnız ayrıcalıklı serviste.

use crate::sys::run_os;

/// `id` ipc::validate() ile sanitize edilmiş (alfanümerik+-_); `path` mutlak .exe (validate).
pub fn run_block(id: &str, path: &str, block: bool) -> Result<(), String> {
    let rule = format!("name=evorift-block-{id}");
    // idempotent: önce var olan kuralı kaldır
    let _ = run_os("netsh", &["advfirewall", "firewall", "delete", "rule", &rule]);
    if !block {
        return Ok(());
    }
    if path.is_empty() {
        return Err("uygulama yolu yok — firewall kuralı eklenemedi".into());
    }
    let prog = format!("program={path}");
    run_os("netsh", &["advfirewall", "firewall", "add", "rule", &rule, "dir=out", "action=block", &prog, "enable=yes"])?;
    if let Err(m) = run_os("netsh", &["advfirewall", "firewall", "add", "rule", &rule, "dir=in", "action=block", &prog, "enable=yes"]) {
        // atomiklik: in kuralı başarısızsa out kuralını geri al (tek-yönlü engel kalmasın)
        let _ = run_os("netsh", &["advfirewall", "firewall", "delete", "rule", &rule]);
        return Err(m);
    }
    Ok(())
}
