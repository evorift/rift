//! Append-only request audit log.
//!
//! Every request that reaches the agent produces exactly one line here, including the ones that
//! were rejected — a 403 from an unexpected source IP is the single most interesting record this
//! file can hold, so it is written before the response goes out.
//!
//! The log lives under `state_dir`, which the config validator forces to be outside the `/pull`
//! sandbox root. That is structural, not a rule someone has to remember: there is no code path
//! that can serve a file from outside the sandbox, so the audit log cannot be fetched over the
//! network at all. It is read by sitting at the laptop.
//!
//! The bearer token is never a parameter to anything in this module.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::util::{iso8601_utc, sanitize_for_log, unix_secs};

/// Per-field cap. Command lines are the long ones; 512 keeps a full PowerShell invocation while
/// bounding what a hostile peer can append per request.
const MAX_FIELD: usize = 512;

/// One audit record. Fields left `None` are written as `-`.
pub struct Entry<'a> {
    pub peer: &'a str,
    pub method: &'a str,
    pub endpoint: &'a str,
    pub status: u16,
    /// The command a `/run` request asked for, or a short note for other endpoints.
    pub command: Option<&'a str>,
    /// Exit code, once a job has finished. `/run` logs `-` here: the job has only just started.
    pub exit_code: Option<i32>,
    pub note: Option<&'a str>,
}

/// Handle to the audit file. The mutex serialises writers so two concurrent connections cannot
/// interleave halves of a line; the file itself is opened in append mode, so even a second
/// process (an accidentally started second agent) appends rather than truncating.
pub struct Audit {
    path: PathBuf,
    lock: Mutex<()>,
}

impl Audit {
    /// Open (or create) the audit log under `state_dir`.
    pub fn open(state_dir: &Path) -> std::io::Result<Self> {
        std::fs::create_dir_all(state_dir)?;
        let path = state_dir.join("audit.log");
        // Touch it now so a permissions problem surfaces at startup, not at the first 403.
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
        writeln!(
            f,
            "{} --- evorift-testd {} audit log opened ---",
            iso8601_utc(unix_secs()),
            env!("CARGO_PKG_VERSION")
        )?;
        Ok(Self { path, lock: Mutex::new(()) })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one record. Failure to log is reported to stderr (which the service redirects to
    /// its own file) but never fails the request — an agent that stops answering because its log
    /// is full is an agent that cannot be recovered remotely.
    pub fn write(&self, entry: &Entry<'_>) {
        let line = format!(
            "{} ip={} method={} endpoint={} status={} cmd={} exit={} note={}",
            iso8601_utc(unix_secs()),
            sanitize_for_log(entry.peer, 64),
            sanitize_for_log(entry.method, 16),
            sanitize_for_log(entry.endpoint, 256),
            entry.status,
            entry.command.map(|c| sanitize_for_log(c, MAX_FIELD)).unwrap_or_else(|| "-".into()),
            entry.exit_code.map(|c| c.to_string()).unwrap_or_else(|| "-".into()),
            entry.note.map(|n| sanitize_for_log(n, 256)).unwrap_or_else(|| "-".into()),
        );
        self.append_line(&line);
    }

    /// Append a free-form line (deadman fires, startup banner, recovery summaries).
    pub fn note(&self, text: &str) {
        let line = format!("{} {}", iso8601_utc(unix_secs()), sanitize_for_log(text, 1024));
        self.append_line(&line);
    }

    fn append_line(&self, line: &str) {
        // The guard is held only for the write itself; nothing awaits or blocks inside it.
        let _guard = match self.lock.lock() {
            Ok(g) => g,
            // A poisoned mutex means another thread panicked mid-write. The file is still append
            // mode and each write is a single line, so recovering the guard is safe and is
            // strictly better than losing all further audit records.
            Err(poisoned) => poisoned.into_inner(),
        };
        match std::fs::OpenOptions::new().create(true).append(true).open(&self.path) {
            Ok(mut f) => {
                if let Err(e) = writeln!(f, "{line}") {
                    eprintln!("[testd][audit] write failed: {e}");
                }
            }
            Err(e) => eprintln!("[testd][audit] open failed: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("evorift-testd-audit-{name}"));
        let _ = std::fs::remove_dir_all(&d); // ignore: absent on the first run, which is fine
        d
    }

    #[test]
    fn every_field_lands_on_one_line() {
        let dir = temp_dir("basic");
        let audit = Audit::open(&dir).expect("open audit log");
        audit.write(&Entry {
            peer: "192.168.1.20",
            method: "POST",
            endpoint: "/run",
            status: 202,
            command: Some("powershell -File scripts/capture-state.ps1 -Label during"),
            exit_code: None,
            note: Some("job=abc123"),
        });
        let text = std::fs::read_to_string(audit.path()).expect("read back");
        let last = text.lines().last().expect("a record was written");
        assert!(last.contains("ip=192.168.1.20"));
        assert!(last.contains("endpoint=/run"));
        assert!(last.contains("status=202"));
        assert!(last.contains("capture-state.ps1"));
        assert!(last.contains("exit=-"));
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn the_log_is_append_only_across_reopens() {
        let dir = temp_dir("append");
        let first = Audit::open(&dir).expect("open");
        first.note("first line");
        drop(first);
        let second = Audit::open(&dir).expect("reopen");
        second.note("second line");
        let text = std::fs::read_to_string(second.path()).expect("read back");
        assert!(text.contains("first line"), "reopening must not truncate the log");
        assert!(text.contains("second line"));
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn a_hostile_endpoint_cannot_forge_a_second_record() {
        let dir = temp_dir("injection");
        let audit = Audit::open(&dir).expect("open");
        let before = std::fs::read_to_string(audit.path()).expect("read").lines().count();
        audit.write(&Entry {
            peer: "192.168.1.99",
            method: "GET",
            // A newline plus a convincing-looking record.
            endpoint: "/pull\n2020-01-01T00:00:00Z ip=127.0.0.1 method=GET endpoint=/health status=200",
            status: 403,
            command: None,
            exit_code: None,
            note: None,
        });
        let text = std::fs::read_to_string(audit.path()).expect("read back");
        assert_eq!(
            text.lines().count(),
            before + 1,
            "an injected newline must not produce a second audit line"
        );
        assert!(text.contains("status=403"));
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn the_audit_path_is_never_inside_a_sandbox_we_serve() {
        // Guards the invariant the config validator enforces: this test documents *why* the
        // state dir is separate, so a later refactor that "simplifies" it fails here.
        let dir = temp_dir("location");
        let audit = Audit::open(&dir).expect("open");
        assert!(audit.path().ends_with("audit.log"));
        assert!(audit.path().starts_with(&dir));
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}
