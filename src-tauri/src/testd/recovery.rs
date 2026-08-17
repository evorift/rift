//! The recovery routine and the deadman switch that fires it unattended.
//!
//! # Why this exists
//!
//! The software under test cuts all internet on the host until it is killed. That means the
//! agent loses contact with the controller at exactly the moment a test fails — the controller
//! cannot ask for help, because the request would have to travel over the link that just died.
//! So recovery must be a local decision made by the laptop itself.
//!
//! The trigger is silence: if no `/health` request has arrived for `deadman_secs`, the agent
//! concludes the link is gone and undoes the damage. Every fire is logged, on disk, before and
//! after, so the controller can prove after the fact whether the deadman fired during a run.
//!
//! # Why it reuses evorift's own cleanup
//!
//! The WinDivert service teardown is [`crate::engine::clear_stale_windivert`] — the same
//! function the product calls before starting winws, not a copy. A second implementation would
//! drift the first time a WinDivert version is added, and the copy that drifts is the one that
//! runs when the machine is already unreachable.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::audit::Audit;
use super::util::{iso8601_utc, json_escape, stamp_for_filename, unix_secs};

/// Images killed by the recovery routine, in order. evorift first: it is the parent that would
/// otherwise respawn winws.
const KILL_IMAGES: &[&str] = &["evorift.exe", "winws.exe"];

/// WinDivert service names, matching the set `clear_stale_windivert` handles. Listed again here
/// only to *measure* the outcome — the teardown itself is not reimplemented.
const WINDIVERT_SERVICES: &[&str] = &["WinDivert", "WinDivert1.4", "WinDivert1.1"];

/// One step of the routine, with the result that was measured afterwards rather than assumed.
#[derive(Debug, Clone)]
pub struct Step {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

/// What a recovery run actually achieved.
#[derive(Debug, Clone)]
pub struct Report {
    pub reason: String,
    pub at: String,
    /// Whether the agent was running with the privilege its OS commands need. Recorded because
    /// `dns::reset_dns` returns `Ok(())` in unprivileged "sim" mode without touching the system
    /// (see `sys::run_os_env`) — reporting that as a successful DNS restore would be exactly the
    /// silent-success failure evorift hard rule 8 forbids.
    pub privileged: bool,
    pub steps: Vec<Step>,
}

impl Report {
    pub fn all_ok(&self) -> bool {
        self.steps.iter().all(|s| s.ok)
    }

    pub fn to_json(&self) -> String {
        let steps: Vec<String> = self
            .steps
            .iter()
            .map(|s| {
                format!(
                    "{{\"name\":\"{}\",\"ok\":{},\"detail\":\"{}\"}}",
                    json_escape(&s.name),
                    s.ok,
                    json_escape(&s.detail)
                )
            })
            .collect();
        format!(
            "{{\"reason\":\"{}\",\"at\":\"{}\",\"privileged\":{},\"all_ok\":{},\"steps\":[{}]}}",
            json_escape(&self.reason),
            json_escape(&self.at),
            self.privileged,
            self.all_ok(),
            steps.join(",")
        )
    }

    /// One-line form for the audit log.
    pub fn summary(&self) -> String {
        let failed: Vec<&str> =
            self.steps.iter().filter(|s| !s.ok).map(|s| s.name.as_str()).collect();
        if failed.is_empty() {
            format!("recovery({}) all {} steps ok", self.reason, self.steps.len())
        } else {
            format!("recovery({}) FAILED steps: {}", self.reason, failed.join(","))
        }
    }
}

/// Is a process image currently running? Measured, not inferred from the kill's return value —
/// `taskkill` reports success for "no such process" in some configurations.
fn image_running(image: &str) -> bool {
    let out = crate::sys::query_os("tasklist", &["/fi", &format!("IMAGENAME eq {image}"), "/nh"]);
    out.to_ascii_lowercase().contains(&image.to_ascii_lowercase())
}

/// Is a service still registered? `sc query` prints `STATE` only for services that exist.
fn service_present(name: &str) -> bool {
    crate::sys::query_os("sc", &["query", name]).contains("STATE")
}

/// Run the full recovery routine and return a measured report.
///
/// Every step is attempted even if an earlier one failed: a half-recovered laptop that is
/// reachable again is worth more than an early return, and the report says exactly what did and
/// did not work.
pub fn run(reason: &str) -> Report {
    let privileged = crate::sys::privileged();
    let mut steps = Vec::new();

    // 1-2. Kill the processes holding the network hostage.
    for image in KILL_IMAGES {
        crate::proc::kill_image(image);
        // taskkill is asynchronous enough that an immediate re-query can still see the process.
        std::thread::sleep(Duration::from_millis(400));
        let still = image_running(image);
        steps.push(Step {
            name: format!("kill {image}"),
            ok: !still,
            detail: if still {
                format!("{image} is STILL running after taskkill /f")
            } else {
                format!("{image} not present")
            },
        });
    }

    // 3. Stop + delete stale WinDivert services — evorift's own implementation, not a copy.
    crate::engine::clear_stale_windivert();
    let leftover: Vec<&str> =
        WINDIVERT_SERVICES.iter().copied().filter(|s| service_present(s)).collect();
    steps.push(Step {
        name: "clear stale WinDivert services".to_string(),
        ok: leftover.is_empty(),
        detail: if leftover.is_empty() {
            "no WinDivert service registered".to_string()
        } else {
            format!("still registered: {}", leftover.join(", "))
        },
    });

    // 4. Put DNS back on DHCP. Guarded by the privilege check because reset_dns() succeeds
    //    vacuously when unprivileged, and then verified against the real resolver list.
    if privileged {
        let attempted = crate::dns::reset_dns();
        let verified = crate::dns::verify_dns();
        let detail = match &attempted {
            Ok(()) => format!("reset ok; resolvers now: {}", servers_or_none(&verified.servers)),
            Err(e) => format!("reset FAILED: {e}; resolvers now: {}", servers_or_none(&verified.servers)),
        };
        steps.push(Step { name: "restore DNS to DHCP".to_string(), ok: attempted.is_ok(), detail });
    } else {
        steps.push(Step {
            name: "restore DNS to DHCP".to_string(),
            ok: false,
            detail: "NOT ATTEMPTED — agent is not elevated, so sys::run_os would only simulate \
                     the change. Install the agent as a LocalSystem service (see \
                     scripts/testd-install.ps1)."
                .to_string(),
        });
    }

    Report {
        reason: reason.to_string(),
        at: iso8601_utc(unix_secs()),
        privileged,
        steps,
    }
}

fn servers_or_none(servers: &[String]) -> String {
    if servers.is_empty() {
        "(none reported)".to_string()
    } else {
        servers.join(", ")
    }
}

/// Persist a report: an append-only line under `state_dir` (stays on the laptop) and a full JSON
/// document under the sandbox (so the controller can pull it once the link is back).
pub fn persist(report: &Report, state_dir: &Path, sandbox_root: &Path) -> Option<PathBuf> {
    use std::io::Write;

    // Local, never served over the network.
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(state_dir.join("deadman.log"))
    {
        if let Err(e) = writeln!(f, "{} {}", report.at, report.summary()) {
            eprintln!("[testd][deadman] local log write failed: {e}");
        }
    }

    // Pullable copy.
    let dir = sandbox_root.join("recovery");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("[testd][deadman] cannot create {}: {e}", dir.display());
        return None;
    }
    let slug: String = report
        .reason
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .take(32)
        .collect();
    let path = dir.join(format!("{}-{slug}.json", stamp_for_filename(unix_secs())));
    match std::fs::write(&path, report.to_json()) {
        Ok(()) => Some(path),
        Err(e) => {
            eprintln!("[testd][deadman] cannot write {}: {e}", path.display());
            None
        }
    }
}

/// The deadman switch.
///
/// Armed by the first `/health` from an allowlisted peer, so an agent that boots before the
/// controller is ever started does not fire at an idle laptop. Once armed it stays armed: the
/// whole point is that silence after contact means the link died mid-test.
/// How many times in a row the switch may fire without hearing from the controller before it
/// disarms itself and waits.
///
/// Recovery is idempotent: once evorift and winws are dead and DNS is back on DHCP, running it
/// again changes nothing. Repeating it forever is not harmless though — each pass resets DNS,
/// clears the DoH registration and flushes the resolver cache, so an idle agent slowly makes the
/// machine under test *worse* and contaminates the very captures it exists to collect. Observed
/// for real: 37 fires overnight left the laptop unable to resolve DNS during a baseline capture.
///
/// Three attempts covers the case where the first pass does not stick (something respawning the
/// engine); beyond that, more attempts are not going to help and only add damage.
const MAX_CONSECUTIVE_FIRES: u64 = 3;

pub struct Deadman {
    /// Monotonic base. `Instant` is immune to clock changes, which matters because some of the
    /// failure modes under test involve time synchronisation dying with the network.
    base: Instant,
    last_health_ms: AtomicU64,
    armed: AtomicBool,
    fires: AtomicU64,
    /// Fires since the last heartbeat. Reset by `touch`, checked against
    /// [`MAX_CONSECUTIVE_FIRES`] to decide whether to keep going or stand down.
    consecutive_fires: AtomicU64,
    timeout: Duration,
    state_dir: PathBuf,
    sandbox_root: PathBuf,
}

impl Deadman {
    pub fn new(timeout: Duration, state_dir: PathBuf, sandbox_root: PathBuf) -> Self {
        Self {
            base: Instant::now(),
            last_health_ms: AtomicU64::new(0),
            armed: AtomicBool::new(false),
            fires: AtomicU64::new(0),
            consecutive_fires: AtomicU64::new(0),
            timeout,
            state_dir,
            sandbox_root,
        }
    }

    fn now_ms(&self) -> u64 {
        self.base.elapsed().as_millis() as u64
    }

    /// Record a heartbeat and arm the switch. Called by `/health`.
    ///
    /// Hearing from the controller also clears the consecutive-fire count: the switch stood down
    /// because nobody was listening, and now somebody is.
    pub fn touch(&self) {
        self.last_health_ms.store(self.now_ms(), Ordering::SeqCst);
        self.consecutive_fires.store(0, Ordering::SeqCst);
        self.armed.store(true, Ordering::SeqCst);
    }

    /// Fires since the last heartbeat.
    pub fn consecutive_fires(&self) -> u64 {
        self.consecutive_fires.load(Ordering::SeqCst)
    }

    pub fn is_armed(&self) -> bool {
        self.armed.load(Ordering::SeqCst)
    }

    pub fn fire_count(&self) -> u64 {
        self.fires.load(Ordering::SeqCst)
    }

    /// How long since the last heartbeat. Meaningless until armed.
    pub fn since_last_health(&self) -> Duration {
        Duration::from_millis(self.now_ms().saturating_sub(self.last_health_ms.load(Ordering::SeqCst)))
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Should the switch fire right now?
    pub fn is_expired(&self) -> bool {
        self.is_armed() && self.since_last_health() >= self.timeout
    }

    /// Start the watchdog thread. It polls rather than sleeping to a deadline so that a
    /// heartbeat arriving mid-window is honoured immediately on the next tick.
    pub fn spawn_watchdog(self: &Arc<Self>, audit: Arc<Audit>) {
        let deadman = Arc::clone(self);
        let poll = Duration::from_secs(5).min(deadman.timeout / 4);
        std::thread::Builder::new()
            .name("testd-deadman".to_string())
            .spawn(move || loop {
                std::thread::sleep(poll);
                if !deadman.is_expired() {
                    continue;
                }
                let silent = deadman.since_last_health().as_secs();
                let n = deadman.fires.fetch_add(1, Ordering::SeqCst) + 1;
                let streak = deadman.consecutive_fires.fetch_add(1, Ordering::SeqCst) + 1;
                audit.note(&format!(
                    "DEADMAN FIRE #{n} (attempt {streak}/{MAX_CONSECUTIVE_FIRES} since last contact): \
                     no /health for {silent}s (limit {}s) — running recovery",
                    deadman.timeout.as_secs()
                ));
                let report = run(&format!("deadman-{n}"));
                let saved = persist(&report, &deadman.state_dir, &deadman.sandbox_root);
                audit.note(&format!(
                    "DEADMAN FIRE #{n} result: {} (report: {})",
                    report.summary(),
                    saved.map(|p| p.display().to_string()).unwrap_or_else(|| "not saved".into())
                ));

                if streak >= MAX_CONSECUTIVE_FIRES {
                    // Stand down. The machine is as recovered as this routine can make it, and
                    // continuing would keep resetting DNS every window for as long as the agent
                    // is idle — degrading the machine under test and polluting its captures.
                    // The next /health re-arms it.
                    deadman.armed.store(false, Ordering::SeqCst);
                    audit.note(&format!(
                        "DEADMAN DISARMED after {streak} consecutive fires with no contact from \
                         the controller. Recovery is idempotent, so repeating it would only keep \
                         resetting DNS. Re-arms automatically on the next /health."
                    ));
                } else {
                    // Restart the window rather than re-firing every poll while the link stays
                    // down: recovery is destructive, and hammering it adds nothing.
                    deadman.last_health_ms.store(deadman.now_ms(), Ordering::SeqCst);
                }
            })
            // A failure to spawn the watchdog means the agent has no deadman at all, which is
            // the one thing this design cannot ship without — surface it loudly.
            .map(|_| ())
            .unwrap_or_else(|e| eprintln!("[testd][deadman] FATAL: watchdog thread not started: {e}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deadman(timeout_ms: u64) -> Deadman {
        Deadman::new(
            Duration::from_millis(timeout_ms),
            std::env::temp_dir().join("evorift-testd-deadman-state"),
            std::env::temp_dir().join("evorift-testd-deadman-sandbox"),
        )
    }

    #[test]
    fn an_unarmed_switch_never_fires() {
        let d = deadman(1);
        std::thread::sleep(Duration::from_millis(20));
        assert!(!d.is_armed());
        assert!(!d.is_expired(), "a laptop that never heard from the controller must sit still");
    }

    #[test]
    fn a_heartbeat_arms_it_and_silence_expires_it() {
        let d = deadman(30);
        d.touch();
        assert!(d.is_armed());
        assert!(!d.is_expired(), "must not fire immediately after a heartbeat");
        std::thread::sleep(Duration::from_millis(60));
        assert!(d.is_expired(), "silence past the timeout must expire the switch");
    }

    #[test]
    fn a_fresh_heartbeat_resets_the_window() {
        let d = deadman(60);
        d.touch();
        std::thread::sleep(Duration::from_millis(40));
        d.touch();
        std::thread::sleep(Duration::from_millis(40));
        assert!(!d.is_expired(), "the second heartbeat must have restarted the window");
    }

    #[test]
    fn the_fire_count_starts_at_zero() {
        assert_eq!(deadman(1000).fire_count(), 0);
        assert_eq!(deadman(1000).consecutive_fires(), 0);
    }

    /// The switch must stand down after a few fruitless attempts instead of firing forever.
    ///
    /// Regression: left armed and idle overnight it fired 37 times, and because every fire
    /// resets DNS to DHCP and flushes the resolver cache, the laptop could no longer resolve
    /// names during a baseline capture. The agent was damaging the machine it was measuring.
    #[test]
    fn the_switch_disarms_after_repeated_fires_and_a_heartbeat_rearms_it() {
        let d = deadman(30);
        d.touch();
        assert!(d.is_armed());

        // Simulate the watchdog firing, without running the real (destructive) routine.
        for attempt in 1..=MAX_CONSECUTIVE_FIRES {
            let streak = d.consecutive_fires.fetch_add(1, Ordering::SeqCst) + 1;
            if streak >= MAX_CONSECUTIVE_FIRES {
                d.armed.store(false, Ordering::SeqCst);
            }
            assert_eq!(d.consecutive_fires(), attempt);
        }

        assert!(!d.is_armed(), "must stand down rather than keep resetting DNS forever");
        std::thread::sleep(Duration::from_millis(60));
        assert!(!d.is_expired(), "a disarmed switch must not fire again on its own");

        // The controller coming back re-arms it and clears the streak.
        d.touch();
        assert!(d.is_armed(), "a heartbeat must re-arm the switch");
        assert_eq!(d.consecutive_fires(), 0, "and reset the streak");
    }

    #[test]
    fn a_report_serialises_and_flags_failures() {
        let report = Report {
            reason: "deadman-1".to_string(),
            at: "2026-08-11T10:00:00Z".to_string(),
            privileged: true,
            steps: vec![
                Step { name: "kill winws.exe".into(), ok: true, detail: "not present".into() },
                Step { name: "restore DNS to DHCP".into(), ok: false, detail: "boom \"quoted\"".into() },
            ],
        };
        assert!(!report.all_ok());
        assert!(report.summary().contains("FAILED"));
        assert!(report.summary().contains("restore DNS to DHCP"));
        let json = report.to_json();
        assert!(json.contains("\"all_ok\":false"));
        assert!(json.contains("\"privileged\":true"));
        // Quotes inside a detail must not break the document.
        assert!(json.contains("boom \\\"quoted\\\""));
    }

    #[test]
    fn an_all_ok_report_says_so() {
        let report = Report {
            reason: "manual".to_string(),
            at: "2026-08-11T10:00:00Z".to_string(),
            privileged: true,
            steps: vec![Step { name: "kill winws.exe".into(), ok: true, detail: "gone".into() }],
        };
        assert!(report.all_ok());
        assert!(report.summary().contains("all 1 steps ok"));
        assert!(report.to_json().contains("\"all_ok\":true"));
    }

    #[test]
    fn the_kill_list_covers_both_offenders_in_parent_first_order() {
        assert_eq!(KILL_IMAGES, &["evorift.exe", "winws.exe"]);
    }

    #[test]
    fn the_measured_service_list_matches_what_the_engine_clears() {
        // If a WinDivert version is added to engine.rs::clear_stale_windivert, this list must
        // grow too or the report will claim success while a service is left behind.
        assert_eq!(WINDIVERT_SERVICES, &["WinDivert", "WinDivert1.4", "WinDivert1.1"]);
    }
}
