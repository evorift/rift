//! Per-app YÜKLEME (egress) hız sınırı — QoS / NetQosPolicy (docs/03 §4). NetQosPolicy YALNIZ giden
//! trafiği kısar → burada SADECE `up` uygulanır. İNDİRME (`down`) limiti motor (WinDivert inbound)
//! tarafının işi (winws sidecar'ında no-op). up=0 → politikayı kaldır. Değerler `$env:` ile (enjeksiyon yok).

use crate::sys::run_os_env;

pub fn run_limit(id: &str, path: &str, up: u32) -> Result<(), String> {
    let name = format!("evorift-{id}");
    let app = if path.is_empty() { format!("{id}.exe") } else { path.to_string() };
    let bps = (up as u64 * 1000).to_string(); // kbps → bit/sn
    let script = "$ErrorActionPreference='SilentlyContinue'; \
        $n=$env:EVORIFT_QOS_NAME; $a=$env:EVORIFT_QOS_APP; $b=[int64]$env:EVORIFT_QOS_BPS; \
        Remove-NetQosPolicy -Name $n -Confirm:$false; \
        if($b -gt 0){ New-NetQosPolicy -Name $n -AppPathNameMatchCondition $a -ThrottleRateActionBitsPerSecond $b -Confirm:$false }";
    run_os_env(
        "powershell",
        &["-NoProfile", "-Command", script],
        &[
            ("EVORIFT_QOS_NAME", name.as_str()),
            ("EVORIFT_QOS_APP", app.as_str()),
            ("EVORIFT_QOS_BPS", bps.as_str()),
        ],
    )
}
