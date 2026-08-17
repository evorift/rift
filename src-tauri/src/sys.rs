//! Ayrıcalıklı OS-komut primitive'leri (docs/07 §1 sistem katmanı). Tüm sistem-mutasyon modülleri
//! (dns, firewall, tweak, limit, repair, rollback) bunları kullanır.
//!
//! GÜVENLİK: gerçek mutasyon YALNIZ ayrıcalıklı süreçte (LocalSystem `evorift-svc` env VEYA admin).
//! Yetkisiz dev süreçte komutlar `(sim)` loglanır, sistem değişmez (docs/05 §5 — UI asla zorla
//! ayrıcalıklı iş yapmaz). Kullanıcı-etkili değerler komut satırına KONMAZ → `$env:` ile geçer
//! (interpolasyon/enjeksiyon yüzeyi yok, review/P7 A5).

use std::process::Command as OsCommand;

// --- File logging (item 8.1): rotating per-category logs under %PROGRAMDATA%\evorift\logs ---

/// Rotate a log file once it reaches this size (docs/02 §5: SplitWire rotates at 1 MB).
const LOG_MAX_BYTES: u64 = 1024 * 1024;

/// Log directory: `<install dir>\logs`, i.e. beside the executables.
///
/// Was `%PROGRAMDATA%\evorift\logs`, mirrored to the user's Desktop by the UI process. Moved here
/// on request: the logs belong with the app's own files, not scattered across two other locations.
/// This also removes the mirror entirely — one directory, written directly by whoever produced the
/// line, instead of a copy that could lag or diverge.
///
/// Both binaries live in the same directory, so the service (SYSTEM) and the UI (elevated, per the
/// requireAdministrator manifest) write to the SAME file set — which is what makes a single folder
/// answer "what happened" without cross-referencing.
///
/// Falls back to the ProgramData path if the executable's location cannot be resolved, so logging
/// never silently stops.
pub fn log_dir() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("logs")))
        .unwrap_or_else(|| crate::ipc::data_dir().join("logs"))
}

fn ts_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn should_rotate(size: u64) -> bool {
    size >= LOG_MAX_BYTES
}

/// Append `line` to `<dir>/<category>.log`, rotating to `<category>.old.log` at 1 MB and writing a
/// version+timestamp header on a fresh/rotated file. Best-effort — logging must never fail a command.
fn write_log_to(dir: &std::path::Path, category: &str, line: &str) {
    use std::io::Write;
    // REDACT AT THE SINK, not at the call sites.
    //
    // The rule is absolute: no requested domain, and no IP resolved for one, is ever written to
    // disk in plaintext. Enforcing that at each `log()`/`audit()` call would mean auditing ~40 call
    // sites today and trusting every future one — and the failure mode is silent, because a leaked
    // hostname looks exactly like a useful log line. This is the ONE function every log file in the
    // app passes through, so it is the only place the guarantee can actually hold.
    //
    // Deliberately NOT applied to the in-memory event ring: that feeds the UI, which is the user's
    // own screen showing the user their own blocked site. "Never written" is about persistence and
    // anything that can leave the machine, not about what the user is allowed to see.
    let line = &crate::elog::redact(line);
    let _ = std::fs::create_dir_all(dir);
    let path = dir.join(format!("{category}.log"));
    let mut fresh = !path.exists();
    if let Ok(meta) = std::fs::metadata(&path) {
        if should_rotate(meta.len()) {
            let _ = std::fs::rename(&path, dir.join(format!("{category}.old.log")));
            fresh = true;
        }
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        if fresh {
            let _ = writeln!(
                f,
                "=== evorift {} {} log [{}] ===",
                env!("CARGO_PKG_VERSION"),
                category,
                ts_millis()
            );
        }
        let _ = writeln!(f, "[{}] {}", ts_millis(), line);
    }
}

/// Append a timestamped line to a per-operation log file (e.g. `dns`, `repair`, `setup`).
pub fn log(category: &str, line: &str) {
    write_log_to(&log_dir(), category, line);
}

/// Test-only: write through the REAL sink into a chosen directory, so the redaction guarantee can
/// be proven end-to-end rather than only on the helper function.
#[cfg(test)]
pub fn log_to_dir_for_test(dir: &std::path::Path, category: &str, line: &str) {
    write_log_to(dir, category, line);
}

/// Audit log → stderr + `%PROGRAMDATA%\evorift\logs\audit.log` (rotating).
pub fn audit(action: &str) {
    eprintln!("[evorift-svc][audit] {action}");
    write_log_to(&log_dir(), "audit", action);
}

/// Süreç yönetici (admin/elevated) olarak mı çalışıyor?
#[cfg(windows)]
pub fn is_elevated() -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    unsafe {
        let mut token: HANDLE = 0;
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut ret_len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut core::ffi::c_void,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret_len,
        );
        CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}
#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    false
}

/// Ayrıcalıklı mı? Gerçek OS komutları yalnız: LocalSystem `evorift-svc` (env) VEYA admin süreç.
pub fn privileged() -> bool {
    std::env::var("EVORIFT_PRIVILEGED").is_ok() || is_elevated()
}

/// Konsol penceresi AÇMADAN OS komutu nesnesi (CREATE_NO_WINDOW).
#[cfg(windows)]
pub fn os_command(program: &str) -> OsCommand {
    use std::os::windows::process::CommandExt;
    let mut c = OsCommand::new(program);
    c.creation_flags(0x0800_0000);
    c
}
#[cfg(not(windows))]
pub fn os_command(program: &str) -> OsCommand {
    OsCommand::new(program)
}

/// Bir OS komutunu çalıştır (audit log'lu). Yetkisizse `(sim)`.
pub fn run_os(program: &str, args: &[&str]) -> Result<(), String> {
    run_os_env(program, args, &[])
}

/// run_os gibi ama ek ENV değişkenleriyle: kullanıcı-etkili değerleri komut satırına KOYMADAN
/// PowerShell'e `$env:...` ile geçir → komut enjeksiyonu yüzeyi yok (review/P7 A5).
pub fn run_os_env(program: &str, args: &[&str], envs: &[(&str, &str)]) -> Result<(), String> {
    if !privileged() {
        audit(&format!("(sim) {program} {}", args.join(" ")));
        return Ok(());
    }
    audit(&format!("exec {program} {}", args.join(" ")));
    let mut cmd = os_command(program);
    cmd.args(args);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("{program} çalıştırılamadı: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(format!("{program} hata ({:?}): {}", out.status.code(), err.trim()))
    }
}

/// Like run_os but with an explicit working directory (for tools that read their config from cwd,
/// e.g. ProxiFyre reading `app-config.json`). Sim when unprivileged.
pub fn run_os_cwd(program: &str, args: &[&str], cwd: &std::path::Path) -> Result<(), String> {
    if !privileged() {
        audit(&format!("(sim) [cwd={}] {program} {}", cwd.display(), args.join(" ")));
        return Ok(());
    }
    audit(&format!("exec [cwd={}] {program} {}", cwd.display(), args.join(" ")));
    let mut cmd = os_command(program);
    cmd.args(args).current_dir(cwd);
    let out = cmd
        .output()
        .map_err(|e| format!("{program} çalıştırılamadı: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(format!("{program} hata ({:?}): {}", out.status.code(), err.trim()))
    }
}

/// run_os gibi ama stdout'u döndürür (sorgu komutları için — preflight, verify). Yetkisizse boş String.
pub fn query_os(program: &str, args: &[&str]) -> String {
    if !privileged() {
        // Sorgular salt-okuma → yetkisizken de çalıştırılabilir (mutasyon yok). Yine de pencere açma.
    }
    let out = os_command(program).args(args).output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).into_owned(),
        Err(_) => String::new(),
    }
}

/// &[&str] → Vec<String> kısayolu (komut argümanı kurarken).
pub fn svec(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Item 8.1: a fresh log gets a header + the line; a second write reuses the file (one header);
    /// the rotation predicate triggers at the size cap.
    #[test]
    fn log_writes_header_and_rotation_predicate() {
        let dir = std::env::temp_dir().join("evorift-test-logs-8-1");
        let _ = std::fs::remove_dir_all(&dir);
        write_log_to(&dir, "audit", "first line");
        let path = dir.join("audit.log");
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("=== evorift"), "header on fresh file");
        assert!(body.contains("first line"));
        write_log_to(&dir, "audit", "second line");
        let body2 = std::fs::read_to_string(&path).unwrap();
        assert_eq!(body2.matches("=== evorift").count(), 1, "header written only once");
        assert!(body2.contains("second line"));
        assert!(should_rotate(LOG_MAX_BYTES) && !should_rotate(0));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
