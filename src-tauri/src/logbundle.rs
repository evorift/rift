//! Support log bundle (item 8.2, docs/02 §5) — zip the logs + a plaintext system summary for support.
//! Uses PowerShell `Compress-Archive` (no zip crate). User-initiated; runs in the UI/user context (no
//! privilege needed). The summary contains NO secrets (no tokens/keys) — just versions + status.

/// Gather a plaintext system summary: version, OS, elevation, preflight checks, DNS state, engine
/// availability, and managed-service states. Read-only; safe to share for troubleshooting.
pub fn system_summary() -> String {
    let mut s = String::new();
    s.push_str(&format!("evorift {} system summary\n", env!("CARGO_PKG_VERSION")));
    s.push_str(&format!("os: {}\n", os_version()));
    s.push_str(&format!("elevated: {}\n\n", crate::sys::is_elevated()));

    let pf = crate::preflight::run();
    s.push_str(&format!("preflight_ok: {}\n", pf.ok));
    for c in &pf.checks {
        s.push_str(&format!("  [{}] {}: {}\n", if c.pass { "OK" } else { "!!" }, c.name, c.hint));
    }
    s.push('\n');

    let dv = crate::dns::verify_dns();
    s.push_str(&format!("dns_secure: {} ({}) servers={:?}\n\n", dv.secure, dv.provider, dv.servers));

    s.push_str("engines:\n");
    for e in crate::engine::catalog() {
        s.push_str(&format!("  {} available={}\n", e.id, e.available));
    }
    s.push('\n');

    s.push_str("services:\n");
    for sv in crate::services::list() {
        s.push_str(&format!("  {} {}\n", sv.name, sv.state));
    }
    s
}

fn os_version() -> String {
    crate::sys::query_os(
        "powershell",
        &["-NoProfile", "-Command", "[System.Environment]::OSVersion.VersionString"],
    )
    .trim()
    .to_string()
}

/// Write the system summary into `dir` as `system-summary.txt`. Returns its path.
pub fn write_summary(dir: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
    std::fs::create_dir_all(dir)?;
    let p = dir.join("system-summary.txt");
    std::fs::write(&p, system_summary())?;
    Ok(p)
}

/// PowerShell script that zips the logs dir into the destination. Paths are passed via `$env` (no
/// injection); the script is constant → unit-testable.
fn compress_script() -> &'static str {
    "Compress-Archive -Path ($env:EVORIFT_LOGS + '\\*') -DestinationPath $env:EVORIFT_ZIP -Force"
}

#[cfg(windows)]
fn hidden(program: &str) -> std::process::Command {
    use std::os::windows::process::CommandExt;
    let mut c = std::process::Command::new(program);
    c.creation_flags(0x0800_0000);
    c
}
#[cfg(not(windows))]
fn hidden(program: &str) -> std::process::Command {
    std::process::Command::new(program)
}

/// Create a support bundle: write the summary into the logs dir, then zip the whole logs dir to
/// `out_zip` (Compress-Archive). User-initiated; returns the zip path. Best-effort cleanup not needed.
pub fn create_bundle(out_zip: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let logs = crate::sys::log_dir();
    write_summary(&logs).map_err(|e| format!("summary write failed: {e}"))?;

    // Zip a REDACTED COPY, never the live log directory.
    //
    // Our own log writer already redacts at the sink, so in principle this is a second pass over
    // clean text. It exists because the bundle also carries files we did NOT write — `winws.log` is
    // a bundled third-party binary's stdout, and its format is not ours to guarantee. A support
    // bundle is the one artefact explicitly meant to be sent to someone else, so it is the last
    // place to assume the inputs were already safe.
    //
    // Also drops the previous bundle: `evorift-bundle.zip` lives inside the folder being archived,
    // so a second run would otherwise pack the first run's zip inside the new one.
    let stage = logs.join("_bundle_stage");
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir_all(&stage).map_err(|e| format!("bundle staging failed: {e}"))?;

    let entries = std::fs::read_dir(&logs).map_err(|e| format!("logs unreadable: {e}"))?;
    for e in entries.filter_map(|e| e.ok()) {
        let p = e.path();
        if !p.is_file() {
            continue;
        }
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if name.ends_with(".zip") {
            continue; // never nest a previous bundle inside this one
        }
        let body = std::fs::read_to_string(&p).unwrap_or_default();
        let cleaned: String =
            body.lines().map(crate::elog::redact).collect::<Vec<_>>().join("\r\n");
        let _ = std::fs::write(stage.join(&name), cleaned);
    }

    let result = hidden("powershell")
        .args(["-NoProfile", "-Command", compress_script()])
        .env("EVORIFT_LOGS", &stage)
        .env("EVORIFT_ZIP", out_zip)
        .output()
        .map_err(|e| format!("Compress-Archive failed to run: {e}"));

    // Staging is removed whatever happened — it holds a full copy of the logs, and leaving it
    // behind would quietly double the footprint of the thing we just took care to minimise.
    let _ = std::fs::remove_dir_all(&stage);

    let out = result?;
    if out.status.success() {
        Ok(out_zip.to_path_buf())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Item 8.2: the summary has the expected sections, write_summary creates the file, and the compress
    /// script uses Compress-Archive with injection-safe `$env` paths.
    #[test]
    fn summary_write_and_compress_script() {
        let s = system_summary();
        assert!(s.contains("evorift"));
        assert!(s.contains("preflight_ok"));
        assert!(s.contains("dns_secure"));
        assert!(s.contains("engines:") && s.contains("services:"));

        let dir = std::env::temp_dir().join("evorift-test-bundle");
        let p = write_summary(&dir).unwrap();
        assert!(p.exists());
        assert!(std::fs::read_to_string(&p).unwrap().contains("evorift"));

        assert!(compress_script().contains("Compress-Archive"));
        assert!(compress_script().contains("$env:EVORIFT_ZIP"));
        assert!(compress_script().contains("$env:EVORIFT_LOGS"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
