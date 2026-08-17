//! Structured engine event log — the thing that makes an engine failure explainable.
//!
//! Before this, an engine problem produced one of three equally useless outcomes:
//!   * an `eprintln!` into a Session-0 service with no console (discarded by the OS),
//!   * a `Result<_, String>` that some caller dropped with `let _ =` or `.catch(() => {})`,
//!   * nothing at all (winws's own stdout/stderr was never even captured).
//!
//! So "it says applied but nothing works" had no evidence trail on either side of the IPC boundary.
//!
//! This module is the single sink. Every event goes to three places at once:
//!   1. a bounded in-memory ring the UI polls, so errors reach the user's screen live;
//!   2. `%PROGRAMDATA%\evorift\logs\engine.log`, written by whichever process raised it;
//!   3. (via the UI process mirroring that folder) `Desktop\evorift-logs\`, where the user can
//!      actually find it without knowing what ProgramData is.
//!
//! Deliberately global and infallible: a logging call must never be a reason a command fails, and
//! must never need a handle threaded through five layers to be reachable from a leaf function.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

/// How many events stay queryable in memory. ~400 covers a full start/verify/tune cycle several
/// times over; older events remain on disk.
const RING_CAP: usize = 400;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    fn as_str(self) -> &'static str {
        match self {
            Level::Info => "info",
            Level::Warn => "warn",
            Level::Error => "error",
        }
    }
}

/// One logged event. `code` is a stable machine-readable slug — the thing that survived being
/// flattened when everything was a `String`. The UI keys its explanations off `code`, not off the
/// message text, so wording can change without breaking behaviour.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    /// Monotonic sequence number, so a poller can ask for "everything after N" without duplicates.
    pub seq: u64,
    /// Local wall-clock time, human-readable (`2026-08-16 14:03:21`).
    pub at: String,
    pub level: Level,
    /// Subsystem: "engine" | "service" | "dns" | "tuner" | "verify" | "ui" | ...
    pub module: String,
    /// Stable slug, e.g. "winws_instant_exit", "control_regression", "dns_apply_failed".
    pub code: String,
    pub message: String,
}

struct Ring {
    next_seq: u64,
    events: VecDeque<Event>,
}

fn ring() -> &'static Mutex<Ring> {
    static RING: OnceLock<Mutex<Ring>> = OnceLock::new();
    RING.get_or_init(|| Mutex::new(Ring { next_seq: 1, events: VecDeque::with_capacity(RING_CAP) }))
}

pub fn info(module: &str, code: &str, message: &str) {
    push(Level::Info, module, code, message)
}
pub fn warn(module: &str, code: &str, message: &str) {
    push(Level::Warn, module, code, message)
}
pub fn error(module: &str, code: &str, message: &str) {
    push(Level::Error, module, code, message)
}

/// Record an event. Never panics, never blocks on I/O long enough to matter, never fails a caller.
pub fn push(level: Level, module: &str, code: &str, message: &str) {
    let message = scrub(message);
    let ev = Event {
        seq: 0, // assigned under the lock below
        at: local_timestamp(),
        level,
        module: module.to_string(),
        code: code.to_string(),
        message,
    };

    let line = format!("{} [{}] {}/{}: {}", ev.at, ev.level.as_str(), ev.module, ev.code, ev.message);
    // Disk first and unconditionally: if the ring's mutex is somehow contended or poisoned, the
    // evidence still lands somewhere. A poisoned lock recovers via into_inner rather than panicking
    // — an engine that dies because its logger died is worse than a logger with a torn ring.
    crate::sys::log("engine", &line);

    let mut r = ring().lock().unwrap_or_else(|p| p.into_inner());
    let seq = r.next_seq;
    r.next_seq += 1;
    if r.events.len() >= RING_CAP {
        r.events.pop_front();
    }
    r.events.push_back(Event { seq, ..ev });
}

/// Events with `seq > since`, plus the highest sequence number now issued.
///
/// Returning the watermark separately means a caller that polls an empty result still learns where
/// the stream is, so it cannot get stuck replaying from 0 after the ring wraps.
pub fn since(since: u64) -> (u64, Vec<Event>) {
    let r = ring().lock().unwrap_or_else(|p| p.into_inner());
    let watermark = r.next_seq.saturating_sub(1);
    let out: Vec<Event> = r.events.iter().filter(|e| e.seq > since).cloned().collect();
    (watermark, out)
}

/// Most recent errors/warnings only — what the UI shows in its problem banner.
pub fn problems(limit: usize) -> Vec<Event> {
    let r = ring().lock().unwrap_or_else(|p| p.into_inner());
    r.events
        .iter()
        .rev()
        .filter(|e| matches!(e.level, Level::Warn | Level::Error))
        .take(limit)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

/// TLDs a token must END in before we treat it as a hostname worth redacting.
///
/// An allowlist, not a `\.[a-z]+` catch-all, because log lines are FULL of dotted tokens that are
/// not hostnames: `winws.exe`, `windivert_part.stun.txt`, `quic_initial_www_google_com.bin`,
/// `state.json`, `WinDivert64.sys`. Redacting those would shred exactly the diagnostic detail the
/// logs exist for, while protecting nothing.
const HOSTNAME_TLDS: &[&str] = &[
    "com", "net", "org", "gg", "tv", "io", "me", "co", "dev", "app", "xyz", "info", "biz", "media",
    "cloud", "site", "online", "live", "to", "cc", "ru", "tr", "de", "uk", "fr", "nl", "us", "eu",
    "pro", "link", "click", "stream", "video", "porn", "sex", "adult", "cam", "xxx",
];

/// Public DNS resolver addresses that are SHIPPED CONSTANTS, not user data.
///
/// These appear in DNS logs ("applied cloudflare", "drifted, actual [...]"), are identical on every
/// install, and say nothing about what anyone visited. Masking them would make DNS problems
/// undiagnosable to buy no privacy at all.
const PUBLIC_RESOLVERS: &[&str] = &[
    "1.1.1.1", "1.0.0.1", "9.9.9.9", "149.112.112.112", "94.140.14.14", "94.140.15.15",
    "8.8.8.8", "8.8.4.4",
];

/// Remove hostnames and non-resolver IP literals from a line bound for disk.
///
/// WHAT THIS PROTECTS: a log file, a support bundle, or a crash artefact that leaves the machine
/// cannot carry a record of which sites were requested or what they resolved to.
///
/// WHAT IT DOES NOT: it is a filter over text we generate, not a guarantee about text we don't
/// control. Output from bundled third-party binaries is handled separately (the winws `--debug`
/// path was removed outright rather than filtered, precisely because its format is not ours).
pub fn redact(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    // Split on whitespace but keep it, so log lines stay readable after masking.
    let mut rest = line;
    while !rest.is_empty() {
        let end = rest.find(|c: char| c.is_whitespace()).unwrap_or(rest.len());
        let (tok, tail) = rest.split_at(end);
        out.push_str(&redact_token(tok));
        let ws_end = tail.find(|c: char| !c.is_whitespace()).unwrap_or(tail.len());
        out.push_str(&tail[..ws_end]);
        rest = &tail[ws_end..];
    }
    out
}

fn redact_token(tok: &str) -> String {
    // Strip surrounding punctuation so `(discord.com:` still matches, and restore it afterwards.
    let lead: String = tok.chars().take_while(|c| !c.is_ascii_alphanumeric()).collect();
    let core_and_trail = &tok[lead.len()..];
    let trail_start = core_and_trail
        .rfind(|c: char| c.is_ascii_alphanumeric())
        .map(|i| i + 1)
        .unwrap_or(0);
    let core = &core_and_trail[..trail_start];
    let trail = &core_and_trail[trail_start..];
    if core.is_empty() {
        return tok.to_string();
    }

    let masked = if is_hostname(core) {
        "<site>"
    } else if is_ipv4(core) && !PUBLIC_RESOLVERS.contains(&core) {
        // An IP in our logs that is not one of the shipped resolvers is, by elimination, an address
        // something resolved to — exactly the "resolved IP tied to a domain" case.
        "<ip>"
    } else {
        return tok.to_string();
    };
    format!("{lead}{masked}{trail}")
}

fn is_hostname(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    let labels: Vec<&str> = lower.split('.').collect();
    if labels.len() < 2 || labels.iter().any(|l| l.is_empty()) {
        return false;
    }
    if !labels
        .iter()
        .all(|l| l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
    {
        return false;
    }
    // A hostname's last label is never all digits (that would be an IPv4 address).
    let tld = labels[labels.len() - 1];
    HOSTNAME_TLDS.contains(&tld)
}

fn is_ipv4(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 4 && parts.iter().all(|p| !p.is_empty() && p.parse::<u8>().is_ok())
}

/// Drop anything that must never be written to a file (CLAUDE.md rule 13).
///
/// Applied at the sink rather than at each call site on purpose: call sites are where the mistake
/// gets made, so the guarantee has to live somewhere a future careless `format!` still passes
/// through. Matches the two shapes that actually exist in this codebase — a WireGuard key (44-char
/// base64 ending in `=`) and any `PrivateKey =` assignment from a generated tunnel config.
fn scrub(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for (i, line) in s.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let low = line.to_ascii_lowercase();
        if low.contains("privatekey") || low.contains("private_key") {
            out.push_str("<redacted: private key>");
            continue;
        }
        // Bare base64 WireGuard key: exactly 43 base64 chars + '='.
        let mut redacted = String::with_capacity(line.len());
        for tok in line.split(' ') {
            if tok.len() == 44
                && tok.ends_with('=')
                && tok[..43]
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/')
            {
                redacted.push_str("<redacted: key>");
            } else {
                redacted.push_str(tok);
            }
            redacted.push(' ');
        }
        out.push_str(redacted.trim_end());
    }
    out
}

/// Human-readable local timestamp, for anything that needs to record "when" in a file a user reads.
pub fn stamp() -> String {
    local_timestamp()
}

/// `YYYY-MM-DD HH:MM:SS` in the machine's local timezone.
///
/// Local, not UTC: this log's primary reader is the user looking at a folder on their Desktop
/// trying to match a line to "when the site stopped opening". Epoch millis (what `sys::log` stamps)
/// are unusable for that.
#[cfg(windows)]
fn local_timestamp() -> String {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    unsafe {
        let mut st: SYSTEMTIME = std::mem::zeroed();
        GetLocalTime(&mut st);
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond
        )
    }
}

#[cfg(not(windows))]
fn local_timestamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("epoch+{secs}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_is_bounded_and_sequence_keeps_climbing() {
        for i in 0..(RING_CAP + 50) {
            info("test", "fill", &format!("event {i}"));
        }
        let (watermark, all) = since(0);
        assert!(all.len() <= RING_CAP, "ring must stay bounded, got {}", all.len());
        assert!(watermark >= (RING_CAP + 50) as u64, "sequence must keep climbing past the ring size");
        // Asking for everything after the watermark yields nothing — a poller cannot loop forever.
        let (_, none) = since(watermark);
        assert!(none.is_empty(), "nothing may be newer than the watermark");
    }

    #[test]
    fn since_returns_only_newer_events() {
        let (before, _) = since(0);
        error("test", "marker", "boundary");
        let (_, after) = since(before);
        assert!(after.iter().any(|e| e.code == "marker"));
        assert!(after.iter().all(|e| e.seq > before), "no event at or below the cursor may repeat");
    }

    #[test]
    fn problems_filters_to_warnings_and_errors() {
        info("test", "quiet", "nothing to see");
        warn("test", "loud", "something odd");
        error("test", "bad", "something broke");
        let p = problems(50);
        assert!(p.iter().any(|e| e.code == "loud"));
        assert!(p.iter().any(|e| e.code == "bad"));
        assert!(!p.iter().any(|e| e.code == "quiet"), "info must not appear in the problem list");
    }

    /// THE privacy invariant: a domain the user asked for must never reach a log file.
    #[test]
    fn redact_removes_hostnames_but_keeps_diagnostics() {
        // Requested domains — gone, in every shape a log line puts them in.
        assert_eq!(redact("discord.com: TCP: timed out"), "<site>: TCP: timed out");
        assert_eq!(redact("probe (www.pornhub.com) failed"), "probe (<site>) failed");
        assert_eq!(redact("opened cdn.discordapp.com"), "opened <site>");
        assert!(!redact("targets: a.example.com, b.example.net").contains("example"));

        // Resolved addresses — gone.
        assert_eq!(redact("resolved to 195.175.254.2"), "resolved to <ip>");

        // Shipped resolver constants — KEPT, or DNS problems become undiagnosable.
        assert_eq!(redact("dns applied 1.1.1.1, 1.0.0.1"), "dns applied 1.1.1.1, 1.0.0.1");

        // Filenames and paths — KEPT. These carry the whole diagnostic value of the engine log.
        for keep in [
            "winws.exe exited with code 1",
            "--wf-raw-part=@C:\\Program Files\\evorift\\winws\\windivert.filter\\windivert_part.stun.txt",
            "quic_initial_www_google_com.bin",
            "state.json unreadable",
            "WinDivert64.sys",
        ] {
            assert_eq!(redact(keep), keep, "redaction damaged a diagnostic line: {keep}");
        }
    }

    /// The sink is what enforces the rule, so prove the whole path — not just the helper.
    #[test]
    fn a_domain_never_reaches_the_log_file() {
        let dir = std::env::temp_dir().join(format!("evorift-redact-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        crate::sys::log_to_dir_for_test(&dir, "probe", "discord.com: TCP: connection reset by peer");
        let body = std::fs::read_to_string(dir.join("probe.log")).expect("log written");
        assert!(!body.contains("discord.com"), "a requested domain reached the log file:\n{body}");
        assert!(body.contains("<site>"), "the line should still be there, just masked:\n{body}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Rule 13 is absolute, so the scrubber is tested against the exact shapes this codebase can
    /// produce rather than trusted to "no call site does that today".
    #[test]
    fn scrub_removes_key_material() {
        let key = "aGVsbG8gd29ybGQgdGhpcyBpcyBhIGZha2Uga2V5eHg=";
        assert_eq!(key.len(), 44, "test fixture must be a realistic 44-char key");
        assert!(!scrub(&format!("PrivateKey = {key}")).contains(key));
        assert!(!scrub(&format!("PRIVATE_KEY={key}")).contains(key));
        assert!(!scrub(&format!("tunnel came up with {key} attached")).contains(key));
        // Ordinary text is untouched.
        assert_eq!(scrub("winws exited with code 1"), "winws exited with code 1");
    }
}
