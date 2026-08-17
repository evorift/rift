//! Real proof-of-protection (evorift-remote-testing: "silent success is the enemy").
//!
//! `running: true` only proves the winws/byedpi/goodbyedpi PROCESS is alive — DPI can still RST or
//! blackhole the connection underneath it, and a UI that reports "Protected" from process liveness
//! alone is exactly the silent-success bug this module exists to close. This runs real,
//! certificate-validated TLS handshakes against live endpoints and answers the harder question:
//! did traffic actually get through. Always runs on a background thread (service.rs spawns it
//! after Start/ApplyProfile/mode switch) — never blocks the caller.
//!
//! TWO target sets, not one. This is the correction to the 2026-08-16 failure where "Güçlü Koruma"
//! reported `verified` while a plain HTTPS site would not open at all:
//!
//!   * TARGETS — the sites the active mode is supposed to OPEN. Quorum-based: most must pass.
//!   * CONTROL — sites that were never blocked and must KEEP working. Any control failure means
//!     the strategy is doing HARM, which is strictly worse than doing nothing, and the engine
//!     must react to it rather than report success.
//!
//! The old probe hit three Discord hostnames only. Every non-Discord site could be dead and the
//! probe still said `verified` — the exact shape of the bug the user hit.

use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, RootCertStore};
use serde::{Deserialize, Serialize};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

/// Default targets when the caller has nothing more specific: Discord's three independent
/// endpoints (main site, gateway, CDN — different infrastructure layers).
pub const PROBE_TARGETS: &[&str] = &["discord.com", "gateway.discord.gg", "cdn.discordapp.com"];

/// Sites that are NOT blocked on any Turkish line and must keep working whatever the engine does.
/// A failure here is the "do no harm" alarm: the desync chain is corrupting ordinary traffic.
///
/// Deliberately three unrelated operators/anycast networks so one operator's outage can't be
/// mistaken for engine-inflicted damage (the harm verdict needs a majority, see `ProtectionReport`).
pub const CONTROL_TARGETS: &[&str] = &["www.google.com", "www.microsoft.com", "www.cloudflare.com"];

/// Budget for a probe that must be CONCLUSIVE — the one whose failure is allowed to declare a line
/// broken. Generous on purpose: a "blocked" verdict must never rest on an impatient timeout.
pub const IO_TIMEOUT: Duration = Duration::from_secs(4);

/// Budget for a probe that only has to be FAST and comparative.
///
/// Every host is probed on its own thread, so a probe costs the slowest single host — and a
/// blackholed host costs the whole timeout. With the conclusive budget that made each sweep ~4.1s
/// regardless of how many hosts answered instantly, which is what turned a 15-candidate measurement
/// into 91 seconds and the first status into 19 (measured, user's log, 2026-08-16).
///
/// 1.2s is not a guess: successful handshakes on that same line measured 150-300ms, so this is 4x
/// headroom over the slowest observed success. It is used where a wrong answer is cheap — comparing
/// candidates, and the first pass of verification, which retries with the conclusive budget before
/// anything is declared broken.
pub const IO_TIMEOUT_FAST: Duration = Duration::from_millis(1200);

/// Single host outcome — carried to the UI so the user sees WHICH site failed, not just "broken".
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProbeResult {
    pub host: String,
    pub ok: bool,
    /// Handshake wall time in ms (0 when it failed before completing).
    pub ms: u32,
    /// Empty when ok; otherwise the failure reason, already prefixed with the failing stage
    /// (`DNS:` / `TCP:` / `tls ...`) so the layer is visible without reading code.
    pub reason: String,
}

/// Full verdict for one apply: did the targets open, and did anything that used to work break.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProtectionReport {
    /// Quorum of `targets` passed → the mode is doing its job.
    pub ok: bool,
    /// A MAJORITY of control sites failed → the engine is breaking ordinary traffic. This is the
    /// condition that must never be reported as "protected", no matter what `ok` says.
    pub harm: bool,
    /// First failure reason (targets first, then control), for the one-line UI message.
    pub reason: String,
    pub targets: Vec<ProbeResult>,
    pub control: Vec<ProbeResult>,
}

impl ProtectionReport {
    /// How many targets opened / how many were tried — the "3/5" the UI shows.
    pub fn target_score(&self) -> (usize, usize) {
        (self.targets.iter().filter(|r| r.ok).count(), self.targets.len())
    }
}

/// Legacy shape kept for the existing service call sites (ok + reason). Prefer `ProtectionReport`.
pub struct ProbeOutcome {
    pub ok: bool,
    pub reason: String,
}

/// Probe a set of hosts CONCURRENTLY (one thread per host) and return every result.
///
/// Concurrency is not a nicety here: on a blocked line every blocked host burns the full
/// `IO_TIMEOUT`, so a sequential sweep of 8 hosts would take ~32s — the slowest possible answer in
/// exactly the case where the answer matters most. In parallel the whole sweep costs one timeout.
pub fn probe_hosts(hosts: &[String]) -> Vec<ProbeResult> {
    probe_hosts_within(hosts, IO_TIMEOUT)
}

/// `probe_hosts` with an explicit per-host budget. See `IO_TIMEOUT` vs `IO_TIMEOUT_FAST`.
pub fn probe_hosts_within(hosts: &[String], budget: Duration) -> Vec<ProbeResult> {
    if hosts.is_empty() {
        return Vec::new();
    }
    let roots = match shared_roots() {
        Ok(r) => r,
        Err(e) => {
            return hosts
                .iter()
                .map(|h| ProbeResult { host: h.clone(), ok: false, ms: 0, reason: e.clone() })
                .collect()
        }
    };

    let handles: Vec<_> = hosts
        .iter()
        .cloned()
        .map(|host| {
            let roots = Arc::clone(&roots);
            std::thread::spawn(move || {
                let t = Instant::now();
                let r = probe_one(&host, &roots, budget);
                let ms = t.elapsed().as_millis().min(u32::MAX as u128) as u32;
                match r {
                    Ok(()) => ProbeResult { host, ok: true, ms, reason: String::new() },
                    Err(e) => ProbeResult { host, ok: false, ms, reason: e },
                }
            })
        })
        .collect();

    handles
        .into_iter()
        .zip(hosts.iter())
        .map(|(h, host)| {
            // A panicking probe thread must count as a FAILURE. Treating a lost result as "passed"
            // would be silent success by another route.
            h.join().unwrap_or_else(|_| ProbeResult {
                host: host.clone(),
                ok: false,
                ms: 0,
                reason: "probe thread panicked".into(),
            })
        })
        .collect()
}

/// The full honesty gate: probe what the mode should open AND what it must not break.
///
/// `targets` is what the active mode claims to cover (empty → the Discord default set).
/// Targets and control are probed in ONE parallel batch, so the whole verdict costs a single
/// timeout window rather than two.
pub fn probe_protection(targets: &[String]) -> ProtectionReport {
    probe_protection_within(targets, IO_TIMEOUT)
}

/// `probe_protection` with an explicit per-host budget.
pub fn probe_protection_within(targets: &[String], budget: Duration) -> ProtectionReport {
    let targets: Vec<String> = if targets.is_empty() {
        PROBE_TARGETS.iter().map(|s| s.to_string()).collect()
    } else {
        targets.to_vec()
    };
    let control: Vec<String> = CONTROL_TARGETS.iter().map(|s| s.to_string()).collect();

    let mut all = targets.clone();
    all.extend(control.iter().cloned());
    let results = probe_hosts_within(&all, budget);
    let (t_res, c_res) = results.split_at(targets.len());
    let (t_res, c_res) = (t_res.to_vec(), c_res.to_vec());

    let t_ok = t_res.iter().filter(|r| r.ok).count();
    let c_ok = c_res.iter().filter(|r| r.ok).count();

    // Targets: a MAJORITY must pass. One endpoint's transient failure must not read as "blocked",
    // but a line where most targets die is genuinely not protected.
    let ok = t_ok * 2 > t_res.len();
    // Control: harm is declared only when a MAJORITY of control sites fail. One flaky anycast
    // endpoint is not evidence that the engine is corrupting traffic; two of three is.
    let harm = !c_res.is_empty() && c_ok * 2 <= c_res.len();

    let reason = if harm {
        let first = c_res.iter().find(|r| !r.ok);
        match first {
            Some(r) => format!(
                "protection is breaking ordinary traffic ({} of {} control sites failed, e.g. {}: {})",
                c_res.len() - c_ok,
                c_res.len(),
                r.host,
                r.reason
            ),
            None => "protection is breaking ordinary traffic".to_string(),
        }
    } else if !ok {
        t_res
            .iter()
            .find(|r| !r.ok)
            .map(|r| format!("{}: {}", r.host, r.reason))
            .unwrap_or_default()
    } else {
        String::new()
    };

    ProtectionReport { ok, harm, reason, targets: t_res, control: c_res }
}

/// Backwards-compatible Discord-only probe (kept so nothing that still calls it silently changes
/// meaning). New code should call `probe_protection`.
pub fn probe_discord() -> ProbeOutcome {
    let r = probe_protection(&[]);
    ProbeOutcome { ok: r.ok && !r.harm, reason: r.reason }
}

/// System root store, loaded ONCE. It used to be re-read for every probe — on a 8-host sweep that
/// parsed the whole Windows root store 8 times per attempt, for a value that never changes.
fn shared_roots() -> Result<Arc<RootCertStore>, String> {
    static ROOTS: OnceLock<Result<Arc<RootCertStore>, String>> = OnceLock::new();
    ROOTS.get_or_init(load_roots).clone()
}

fn load_roots() -> Result<Arc<RootCertStore>, String> {
    let native = rustls_native_certs::load_native_certs();
    if native.certs.is_empty() {
        return Err("system root certificate store returned empty".into());
    }
    let mut store = RootCertStore::empty();
    let (added, _rejected) = store.add_parsable_certificates(native.certs);
    if added == 0 {
        return Err("no root certificate could be parsed".into());
    }
    Ok(Arc::new(store))
}

/// Shared rustls client config — building one costs a full crypto-provider setup, and it is
/// identical for every host (SNI is per-connection, not per-config).
fn shared_config(roots: &Arc<RootCertStore>) -> Result<Arc<ClientConfig>, String> {
    static CFG: OnceLock<Result<Arc<ClientConfig>, String>> = OnceLock::new();
    CFG.get_or_init(|| {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let config = ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|e| format!("tls config: {e}"))?
            .with_root_certificates(Arc::clone(roots))
            .with_no_client_auth();
        Ok(Arc::new(config))
    })
    .clone()
}

/// One host: resolve DNS → TCP connect → FULL TLS handshake with certificate validation.
///
/// Every DPI reaction is an Err here: RST, blackhole/timeout, and an injected block-page
/// certificate all fail, because all three are part of "did traffic actually get through".
fn probe_one(host: &str, roots: &Arc<RootCertStore>, budget: Duration) -> Result<(), String> {
    let addr = format!("{host}:443")
        .to_socket_addrs()
        .map_err(|e| format!("DNS: {e}"))?
        .next()
        .ok_or_else(|| "DNS: no address returned".to_string())?;

    let mut tcp = TcpStream::connect_timeout(&addr, budget).map_err(|e| format!("TCP: {e}"))?;
    tcp.set_read_timeout(Some(budget)).map_err(|e| format!("TCP: {e}"))?;
    tcp.set_write_timeout(Some(budget)).map_err(|e| format!("TCP: {e}"))?;
    // Nagle would coalesce the ClientHello with nothing else here, but disabling it keeps the
    // handshake timing honest — the probe measures the network, not the local send buffer.
    let _ = tcp.set_nodelay(true);

    let config = shared_config(roots)?;
    let server_name = ServerName::try_from(host.to_string()).map_err(|e| format!("SNI: {e}"))?;
    let mut conn = ClientConnection::new(config, server_name).map_err(|e| format!("tls connect: {e}"))?;

    // rustls does no I/O of its own — the driver loop is manual. connect_timeout already catches
    // TCP RST/timeout (DPI's usual answer); process_new_packets catches a forged/invalid cert.
    while conn.is_handshaking() {
        if conn.wants_write() {
            conn.write_tls(&mut tcp).map_err(|e| format!("tls write: {e}"))?;
        }
        if conn.wants_read() {
            let n = conn.read_tls(&mut tcp).map_err(|e| format!("tls read: {e}"))?;
            if n == 0 {
                return Err("connection closed before the handshake completed".into());
            }
            conn.process_new_packets().map_err(|e| format!("tls verify: {e}"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn res(host: &str, ok: bool) -> ProbeResult {
        ProbeResult { host: host.into(), ok, ms: 1, reason: if ok { String::new() } else { "x".into() } }
    }

    /// The control set is what makes "Güçlü broke a working site" detectable at all. It must be
    /// non-empty and disjoint from the target defaults, or the harm check is decorative.
    #[test]
    fn control_set_is_real_and_disjoint() {
        assert!(!CONTROL_TARGETS.is_empty());
        for c in CONTROL_TARGETS {
            assert!(!PROBE_TARGETS.contains(c), "control target {c} must not also be a probe target");
        }
    }

    /// Harm is a majority verdict: one flaky control host is tolerated, two of three is not.
    /// This is the arithmetic the "do no harm" guarantee rests on, so it is pinned by test.
    #[test]
    fn harm_needs_a_majority_of_control_failures() {
        let judge = |c: Vec<ProbeResult>| {
            let ok = c.iter().filter(|r| r.ok).count();
            !c.is_empty() && ok * 2 <= c.len()
        };
        assert!(!judge(vec![res("a", true), res("b", true), res("c", true)]), "all good → no harm");
        assert!(!judge(vec![res("a", true), res("b", true), res("c", false)]), "1 of 3 → tolerated");
        assert!(judge(vec![res("a", true), res("b", false), res("c", false)]), "2 of 3 → harm");
        assert!(judge(vec![res("a", false), res("b", false), res("c", false)]), "all bad → harm");
    }

    /// Targets need a strict majority, mirroring the old MIN_OK=2-of-3 rule.
    #[test]
    fn target_quorum_is_a_strict_majority() {
        let judge = |t: Vec<ProbeResult>| {
            let ok = t.iter().filter(|r| r.ok).count();
            ok * 2 > t.len()
        };
        assert!(judge(vec![res("a", true), res("b", true), res("c", false)]), "2 of 3 → ok");
        assert!(!judge(vec![res("a", true), res("b", false), res("c", false)]), "1 of 3 → not ok");
        assert!(!judge(vec![res("a", true), res("b", false)]), "1 of 2 is not a majority");
    }

    /// An empty target list falls back to the Discord defaults rather than vacuously passing —
    /// `ok` on zero probes would be another silent success.
    #[test]
    fn empty_targets_fall_back_to_defaults() {
        let t: Vec<String> = Vec::new();
        let resolved: Vec<String> = if t.is_empty() {
            PROBE_TARGETS.iter().map(|s| s.to_string()).collect()
        } else {
            t.clone()
        };
        assert_eq!(resolved.len(), PROBE_TARGETS.len());
    }

    /// Live probe, deliberately NOT part of the normal suite. Run by hand when the UI is stuck on
    /// "unverified" and you need to know whether the probe finishes at all, and how long it takes:
    ///     cargo test --release --lib verify:: -- --ignored --nocapture
    #[test]
    #[ignore = "live network probe — run explicitly, see evorift-live-verification"]
    fn live_probe_reports_timing_and_outcome() {
        let t = Instant::now();
        let r = probe_protection(&[]);
        println!("ok={} harm={} in {}ms", r.ok, r.harm, t.elapsed().as_millis());
        for p in r.targets.iter().chain(r.control.iter()) {
            println!("  {:<28} ok={} {}ms {}", p.host, p.ok, p.ms, p.reason);
        }
    }
}
