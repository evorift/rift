//! Command execution: start now, answer later.
//!
//! `/run` returns a job id immediately and never blocks on the command finishing. That is not a
//! latency optimisation — it is the whole point. The commands this agent runs are the ones that
//! kill the network, so the HTTP response carrying their output would never arrive. Instead:
//!
//!   * stdout and stderr are redirected by the OS straight into files under the sandbox, so the
//!     agent never holds output in memory waiting for a reader;
//!   * `status.json` is written when the job starts and rewritten (atomically, via a temp file
//!     and a rename) when it ends, so `GET /job/<id>` reads state off disk rather than out of a
//!     process-lifetime map — the answer survives an agent restart;
//!   * every job has a timeout, after which it is killed, because a command that hangs while
//!     holding the network down is indistinguishable from one that succeeded.
//!
//! Both job kinds resolve their program through the sandbox, so `/run` cannot be used to launch
//! an arbitrary binary from elsewhere on the laptop. Anything that needs a system tool
//! (`netsh`, `ipconfig`, …) is wrapped in a `.ps1` that gets pushed into the sandbox first —
//! which also means the exact text of what ran is on disk, next to its output.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::sandbox;
use super::util::{hex, iso8601_utc, unix_secs};

/// How often a running child is polled for exit.
const POLL: Duration = Duration::from_millis(200);
/// Bounds on a caller-supplied timeout.
const MIN_TIMEOUT_SECS: u64 = 1;
const MAX_TIMEOUT_SECS: u64 = 3600;
/// Bounds on the argument vector, so a single request cannot build an unbounded command line.
const MAX_ARGS: usize = 64;
const MAX_ARG_LEN: usize = 4096;

/// What `/run` was asked to do. Untagged variants are rejected by `deny_unknown_fields`, so a
/// misspelled field is an error rather than a silently defaulted job.
#[derive(serde::Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum JobSpec {
    /// Run a PowerShell script that lives inside the sandbox.
    Powershell {
        /// Sandbox-relative path, e.g. `scripts/capture-state.ps1`.
        script: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        timeout_secs: Option<u64>,
    },
    /// Run an executable that lives inside the sandbox, e.g. a freshly pushed build.
    Exec {
        /// Sandbox-relative path, e.g. `build/evorift.exe`.
        program: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        timeout_secs: Option<u64>,
    },
}

#[derive(Debug)]
pub enum JobError {
    BadSpec(String),
    BadPath(sandbox::PathError),
    TooManyArgs { max: usize },
    ArgTooLong { max: usize },
    ArgHasNul,
    Io(std::io::Error),
    Random(String),
}

impl std::fmt::Display for JobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadSpec(d) => write!(f, "invalid job spec: {d}"),
            Self::BadPath(e) => write!(f, "program path rejected: {e}"),
            Self::TooManyArgs { max } => write!(f, "at most {max} arguments allowed"),
            Self::ArgTooLong { max } => write!(f, "each argument must be under {max} characters"),
            Self::ArgHasNul => write!(f, "an argument contains a NUL byte"),
            Self::Io(e) => write!(f, "i/o error: {e}"),
            Self::Random(d) => write!(f, "could not generate a job id: {d}"),
        }
    }
}

impl std::error::Error for JobError {}

impl JobError {
    pub fn status(&self) -> u16 {
        match self {
            Self::Io(_) | Self::Random(_) => 500,
            _ => 400,
        }
    }
}

/// A started job, as handed back to the controller.
pub struct Started {
    pub id: String,
    pub dir: PathBuf,
    /// Human-readable command line, for the audit log. Never re-parsed.
    pub command: String,
}

/// 16 random bytes as hex. A weak fallback is deliberately absent: if the OS RNG is unavailable
/// something is badly wrong, and silently using a predictable id would be worse than failing.
fn new_job_id() -> Result<String, JobError> {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).map_err(|e| JobError::Random(e.to_string()))?;
    Ok(hex(&bytes))
}

fn check_args(args: &[String]) -> Result<(), JobError> {
    if args.len() > MAX_ARGS {
        return Err(JobError::TooManyArgs { max: MAX_ARGS });
    }
    for a in args {
        if a.len() > MAX_ARG_LEN {
            return Err(JobError::ArgTooLong { max: MAX_ARG_LEN });
        }
        if a.contains('\0') {
            return Err(JobError::ArgHasNul);
        }
    }
    Ok(())
}

/// Resolve a spec into an actual command, with the program confined to the sandbox.
///
/// Arguments are passed to `Command::args` as separate argv entries and never concatenated into
/// a shell string, so there is no interpolation surface: an argument containing `; rm -rf` is
/// just a string the script receives.
fn build_command(
    spec: &JobSpec,
    canonical_root: &Path,
    default_timeout_secs: u64,
) -> Result<(Command, String, u64), JobError> {
    match spec {
        JobSpec::Powershell { script, args, timeout_secs } => {
            check_args(args)?;
            let resolved =
                sandbox::resolve_existing(canonical_root, script).map_err(JobError::BadPath)?;
            // Hand the script a normal path, not the canonical `\\?\` one -- see
            // sandbox::strip_verbatim. The canonical form was used for the containment check
            // above; a child process must never see it.
            let script_path = sandbox::strip_verbatim(&resolved);
            let mut cmd = crate::sys::os_command("powershell");
            cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"]);
            cmd.arg(&script_path);
            cmd.args(args);
            let display = format!("powershell -File {} {}", script_path.display(), args.join(" "));
            Ok((cmd, display, clamp_timeout(*timeout_secs, default_timeout_secs)))
        }
        JobSpec::Exec { program, args, timeout_secs } => {
            check_args(args)?;
            let resolved =
                sandbox::resolve_existing(canonical_root, program).map_err(JobError::BadPath)?;
            let program_path = sandbox::strip_verbatim(&resolved);
            let mut cmd = crate::sys::os_command(&program_path.to_string_lossy());
            cmd.args(args);
            let display = format!("{} {}", program_path.display(), args.join(" "));
            Ok((cmd, display, clamp_timeout(*timeout_secs, default_timeout_secs)))
        }
    }
}

/// A job that omits `timeout_secs` gets the configured default, NOT the floor. Getting this
/// wrong is quietly destructive: a capture job would be killed one second in, and the evidence
/// would look like the command failing rather than the agent cutting it short.
fn clamp_timeout(requested: Option<u64>, default_secs: u64) -> u64 {
    requested.unwrap_or(default_secs).clamp(MIN_TIMEOUT_SECS, MAX_TIMEOUT_SECS)
}

/// Write `status.json` atomically: a temp file plus a rename, so a concurrent `GET /job/<id>`
/// never observes a half-written document.
fn write_status(dir: &Path, json: &str) -> std::io::Result<()> {
    let tmp = dir.join("status.json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, dir.join("status.json"))
}

#[allow(clippy::too_many_arguments)]
fn status_json(
    id: &str,
    command: &str,
    state: &str,
    started_at: &str,
    finished_at: Option<&str>,
    exit_code: Option<i32>,
    timeout_secs: u64,
    detail: &str,
) -> String {
    use super::util::json_escape;
    format!(
        "{{\"job_id\":\"{}\",\"state\":\"{}\",\"command\":\"{}\",\"started_at\":\"{}\",\
          \"finished_at\":{},\"exit_code\":{},\"timeout_secs\":{},\"detail\":\"{}\"}}",
        json_escape(id),
        json_escape(state),
        json_escape(command),
        json_escape(started_at),
        finished_at
            .map(|f| format!("\"{}\"", json_escape(f)))
            .unwrap_or_else(|| "null".to_string()),
        exit_code.map(|c| c.to_string()).unwrap_or_else(|| "null".to_string()),
        timeout_secs,
        json_escape(detail),
    )
}

/// Start a job. Returns as soon as the child is spawned and its initial status is on disk.
///
/// `canonical_root` is both the confinement boundary for the program path and the working
/// directory of the child, as specified.
pub fn start(
    spec: &JobSpec,
    canonical_root: &Path,
    default_timeout_secs: u64,
) -> Result<Started, JobError> {
    let (mut cmd, command, timeout_secs) = build_command(spec, canonical_root, default_timeout_secs)?;
    let id = new_job_id()?;

    let dir = canonical_root.join("jobs").join(&id);
    std::fs::create_dir_all(&dir).map_err(JobError::Io)?;

    // OS-level redirection: the child writes to these handles directly. Output never transits
    // the agent's address space, so a job producing hundreds of MB costs nothing here.
    let stdout = std::fs::File::create(dir.join("stdout.txt")).map_err(JobError::Io)?;
    let stderr = std::fs::File::create(dir.join("stderr.txt")).map_err(JobError::Io)?;
    std::fs::write(dir.join("cmd.txt"), &command).map_err(JobError::Io)?;

    // The working directory a script sees must be usable by that script: `Get-Location` returning
    // `\\?\C:\evorift-test` makes Join-Path throw and forward slashes unresolvable.
    cmd.current_dir(sandbox::strip_verbatim(canonical_root))
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));

    let started_at = iso8601_utc(unix_secs());
    write_status(
        &dir,
        &status_json(&id, &command, "running", &started_at, None, None, timeout_secs, "started"),
    )
    .map_err(JobError::Io)?;

    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let detail = format!("spawn failed: {e}");
            // Record the failure on disk too — the controller may only get to ask much later.
            let finished = iso8601_utc(unix_secs());
            if let Err(werr) = write_status(
                &dir,
                &status_json(
                    &id,
                    &command,
                    "spawn_failed",
                    &started_at,
                    Some(&finished),
                    None,
                    timeout_secs,
                    &detail,
                ),
            ) {
                eprintln!("[testd][job {id}] status write failed after spawn error: {werr}");
            }
            return Err(JobError::Io(e));
        }
    };

    spawn_reaper(child, id.clone(), command.clone(), dir.clone(), started_at, timeout_secs);

    Ok(Started { id, dir, command })
}

/// Watch the child to completion or timeout, then write the final status.
fn spawn_reaper(
    mut child: std::process::Child,
    id: String,
    command: String,
    dir: PathBuf,
    started_at: String,
    timeout_secs: u64,
) {
    let builder = std::thread::Builder::new().name(format!("testd-job-{id}"));
    // Kept for the failure message below; the closure takes ownership of the original.
    let id_for_error = id.clone();
    let spawned = builder.spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        let (state, exit_code, detail) = loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    break ("exited", status.code(), "process exited".to_string());
                }
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let killed = child.kill();
                        // Reap it so the handle is not left in the process table.
                        let after = child.wait().ok().and_then(|s| s.code());
                        let detail = match killed {
                            Ok(()) => format!("killed after {timeout_secs}s timeout"),
                            Err(e) => format!("timeout after {timeout_secs}s; kill failed: {e}"),
                        };
                        break ("timed_out", after, detail);
                    }
                    std::thread::sleep(POLL);
                }
                Err(e) => {
                    break ("unknown", None, format!("wait failed: {e}"));
                }
            }
        };

        let finished = iso8601_utc(unix_secs());
        if let Err(e) = write_status(
            &dir,
            &status_json(
                &id,
                &command,
                state,
                &started_at,
                Some(&finished),
                exit_code,
                timeout_secs,
                &detail,
            ),
        ) {
            eprintln!("[testd][job {id}] final status write failed: {e}");
        }
    });
    if let Err(e) = spawned {
        // The child is running but nothing will reap it or record its exit. Say so loudly; the
        // job's status.json stays "running", which is at least honest about what is known.
        eprintln!("[testd][job {id_for_error}] FATAL: reaper thread not started: {e} — job status will stay 'running'");
    }
}

/// Read a job's status document back off disk. `id` is validated as a bare hex token, so it can
/// never be used as a path fragment to reach another directory.
pub fn read_status(canonical_root: &Path, id: &str) -> Option<String> {
    if !is_valid_job_id(id) {
        return None;
    }
    std::fs::read_to_string(canonical_root.join("jobs").join(id).join("status.json")).ok()
}

/// Job ids are 32 lowercase hex characters and nothing else — no separators, so no traversal.
pub fn is_valid_job_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Last `max` bytes of a job's output file, for the `?tail=` convenience on `GET /job/<id>`.
/// Reading from the end keeps a huge log cheap to peek at.
pub fn tail(canonical_root: &Path, id: &str, which: &str, max: u64) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    if !is_valid_job_id(id) {
        return None;
    }
    let name = match which {
        "stdout" => "stdout.txt",
        "stderr" => "stderr.txt",
        _ => return None,
    };
    let path = canonical_root.join("jobs").join(id).join(name);
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let from = len.saturating_sub(max);
    file.seek(SeekFrom::Start(from)).ok()?;
    let mut buf = Vec::with_capacity((len - from) as usize);
    file.take(max).read_to_end(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("evorift-testd-jobs-{name}"));
        let _ = std::fs::remove_dir_all(&d); // ignore: not present on a first run
        std::fs::create_dir_all(&d).expect("create test root");
        d.canonicalize().expect("canonicalise test root")
    }

    #[test]
    fn a_spec_with_an_unknown_field_is_rejected() {
        let json = r#"{"kind":"powershell","script":"a.ps1","timeuot_secs":5}"#;
        let parsed: Result<JobSpec, _> = serde_json::from_str(json);
        assert!(parsed.is_err(), "a typo'd field must not silently default");
    }

    #[test]
    fn a_spec_with_an_unknown_kind_is_rejected() {
        let json = r#"{"kind":"shell","script":"a.ps1"}"#;
        let parsed: Result<JobSpec, _> = serde_json::from_str(json);
        assert!(parsed.is_err());
    }

    #[test]
    fn a_valid_spec_parses_with_defaults() {
        let json = r#"{"kind":"powershell","script":"scripts/capture-state.ps1"}"#;
        let parsed: JobSpec = serde_json::from_str(json).expect("valid spec");
        match parsed {
            JobSpec::Powershell { script, args, timeout_secs } => {
                assert_eq!(script, "scripts/capture-state.ps1");
                assert!(args.is_empty());
                assert_eq!(timeout_secs, None);
            }
            JobSpec::Exec { .. } => panic!("parsed as the wrong variant"),
        }
    }

    #[test]
    fn a_program_outside_the_sandbox_is_refused() {
        let root = root("escape");
        let spec = JobSpec::Exec {
            program: r"..\..\Windows\System32\cmd.exe".to_string(),
            args: vec![],
            timeout_secs: None,
        };
        let err = build_command(&spec, &root, 300).expect_err("traversal must be refused");
        assert!(matches!(err, JobError::BadPath(sandbox::PathError::ParentTraversal)));

        let spec = JobSpec::Exec {
            program: r"C:\Windows\System32\cmd.exe".to_string(),
            args: vec![],
            timeout_secs: None,
        };
        let err = build_command(&spec, &root, 300).expect_err("absolute path must be refused");
        assert!(matches!(err, JobError::BadPath(sandbox::PathError::NotRelative)));
        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn timeouts_are_clamped_into_range() {
        // Omitting timeout_secs must yield the configured default, not the 1s floor -- a job
        // silently killed after one second would look like the command itself failing.
        assert_eq!(clamp_timeout(None, 300), 300);
        assert_eq!(clamp_timeout(Some(0), 300), MIN_TIMEOUT_SECS);
        assert_eq!(clamp_timeout(Some(30), 300), 30);
        assert_eq!(clamp_timeout(Some(999_999), 300), MAX_TIMEOUT_SECS);
        // A nonsensical configured default is clamped too, rather than trusted.
        assert_eq!(clamp_timeout(None, 0), MIN_TIMEOUT_SECS);
        assert_eq!(clamp_timeout(None, 999_999), MAX_TIMEOUT_SECS);
    }

    #[test]
    fn argument_limits_are_enforced() {
        assert!(check_args(&["-Label".into(), "during".into()]).is_ok());
        let many: Vec<String> = (0..MAX_ARGS + 1).map(|i| i.to_string()).collect();
        assert!(matches!(check_args(&many), Err(JobError::TooManyArgs { .. })));
        assert!(matches!(
            check_args(&["x".repeat(MAX_ARG_LEN + 1)]),
            Err(JobError::ArgTooLong { .. })
        ));
        assert!(matches!(check_args(&["a\0b".to_string()]), Err(JobError::ArgHasNul)));
    }

    #[test]
    fn job_ids_are_bare_hex_so_they_cannot_traverse() {
        assert!(is_valid_job_id("0123456789abcdef0123456789abcdef"));
        assert!(!is_valid_job_id("0123456789ABCDEF0123456789abcdef"), "uppercase is not produced");
        assert!(!is_valid_job_id("../../windows/system32"));
        assert!(!is_valid_job_id("0123456789abcdef0123456789abcde"), "31 chars");
        assert!(!is_valid_job_id(""));
        assert!(!is_valid_job_id("0123456789abcdef0123456789abcdeg"));
    }

    #[test]
    fn generated_ids_pass_their_own_validator_and_differ() {
        let a = new_job_id().expect("rng available");
        let b = new_job_id().expect("rng available");
        assert!(is_valid_job_id(&a));
        assert!(is_valid_job_id(&b));
        assert_ne!(a, b);
    }

    #[test]
    fn status_documents_are_valid_json_in_both_states() {
        let running = status_json("id1", "cmd", "running", "T0", None, None, 30, "started");
        let parsed: serde_json::Value = serde_json::from_str(&running).expect("running is JSON");
        assert_eq!(parsed["state"], "running");
        assert!(parsed["exit_code"].is_null());
        assert!(parsed["finished_at"].is_null());

        let done = status_json("id1", "cmd", "exited", "T0", Some("T1"), Some(3), 30, "ok");
        let parsed: serde_json::Value = serde_json::from_str(&done).expect("finished is JSON");
        assert_eq!(parsed["exit_code"], 3);
        assert_eq!(parsed["finished_at"], "T1");
    }

    #[test]
    fn a_command_with_quotes_still_produces_valid_json() {
        let doc = status_json("id", r#"powershell -File "a b.ps1""#, "exited", "T0", Some("T1"), Some(0), 30, "ok");
        let parsed: serde_json::Value = serde_json::from_str(&doc).expect("must stay valid JSON");
        assert!(parsed["command"].as_str().unwrap_or_default().contains("a b.ps1"));
    }

    #[test]
    fn status_writes_are_atomic_and_leave_no_temp_file() {
        let root = root("status");
        write_status(&root, r#"{"state":"running"}"#).expect("write");
        assert!(root.join("status.json").is_file());
        assert!(!root.join("status.json.tmp").exists(), "temp file must be renamed away");
        write_status(&root, r#"{"state":"exited"}"#).expect("overwrite");
        let text = std::fs::read_to_string(root.join("status.json")).expect("read");
        assert!(text.contains("exited"));
        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn tail_returns_the_end_of_a_file_and_rejects_bad_ids() {
        let root = root("tail");
        let id = "0123456789abcdef0123456789abcdef";
        let dir = root.join("jobs").join(id);
        std::fs::create_dir_all(&dir).expect("job dir");
        std::fs::write(dir.join("stdout.txt"), "0123456789ABCDEFGHIJ").expect("write");
        assert_eq!(tail(&root, id, "stdout", 5).as_deref(), Some("FGHIJ"));
        assert_eq!(tail(&root, id, "stdout", 100).as_deref(), Some("0123456789ABCDEFGHIJ"));
        assert_eq!(tail(&root, id, "stderr", 5), None, "missing file yields None");
        assert_eq!(tail(&root, "../../etc", "stdout", 5), None, "bad id yields None");
        assert_eq!(tail(&root, id, "passwd", 5), None, "unknown stream yields None");
        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn read_status_refuses_a_traversing_id() {
        let root = root("read");
        assert_eq!(read_status(&root, r"..\..\secret"), None);
        assert_eq!(read_status(&root, "0123456789abcdef0123456789abcdef"), None, "absent job");
        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    /// End-to-end: a real script runs, output lands on disk, status flips to exited.
    #[test]
    fn a_real_job_writes_its_output_to_disk() {
        let root = root("e2e");
        let scripts = root.join("scripts");
        std::fs::create_dir_all(&scripts).expect("scripts dir");
        std::fs::write(scripts.join("hello.ps1"), "Write-Output 'hello-from-job'\r\nexit 7\r\n")
            .expect("write script");

        let spec = JobSpec::Powershell {
            script: "scripts/hello.ps1".to_string(),
            args: vec![],
            timeout_secs: Some(60),
        };
        let started = start(&spec, &root, 300).expect("job starts");
        assert!(is_valid_job_id(&started.id));

        // Poll for the reaper to record the exit rather than assuming a fixed delay.
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let status = read_status(&root, &started.id).expect("status exists immediately");
            if status.contains("\"state\":\"exited\"") {
                let parsed: serde_json::Value =
                    serde_json::from_str(&status).expect("status is JSON");
                assert_eq!(parsed["exit_code"], 7, "the script's exit code must be recorded");
                break;
            }
            assert!(Instant::now() < deadline, "job never finished: {status}");
            std::thread::sleep(Duration::from_millis(100));
        }

        let out = tail(&root, &started.id, "stdout", 4096).expect("stdout captured");
        assert!(out.contains("hello-from-job"), "stdout was {out:?}");
        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    /// A job that outlives its timeout is killed and reported as such, not left running.
    #[test]
    fn a_hanging_job_is_killed_at_its_timeout() {
        let root = root("timeout");
        let scripts = root.join("scripts");
        std::fs::create_dir_all(&scripts).expect("scripts dir");
        std::fs::write(scripts.join("hang.ps1"), "Start-Sleep -Seconds 120\r\n")
            .expect("write script");

        let spec = JobSpec::Powershell {
            script: "scripts/hang.ps1".to_string(),
            args: vec![],
            timeout_secs: Some(1),
        };
        let started = start(&spec, &root, 300).expect("job starts");

        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let status = read_status(&root, &started.id).expect("status exists");
            if status.contains("\"state\":\"timed_out\"") {
                assert!(status.contains("timeout"), "detail should mention the timeout");
                break;
            }
            assert!(Instant::now() < deadline, "job was never timed out: {status}");
            std::thread::sleep(Duration::from_millis(100));
        }
        std::fs::remove_dir_all(&root).expect("cleanup");
    }
}
