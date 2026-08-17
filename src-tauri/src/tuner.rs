//! Per-line strategy selection, by measurement.
//!
//! WHY THIS EXISTS. "Güçlü Koruma" shipped ONE hardcoded chain — the `turkcell-hotspot` preset
//! (`fake` + `ttl=1` + `autottl=3`, no fooling) — applied catch-all to every TLS/443 flow. That
//! chain was measured once, on one line, and the source comment recording that measurement said so
//! plainly. On a different line the forged ClientHello survives past the DPI hop, reaches the real
//! server, and the server tears the connection down: protection made an ordinary site UNREACHABLE
//! that had been fine with the app turned off. Meanwhile "Hafif", which is gated to a Discord/Roblox
//! hostlist and therefore never touched that site, appeared to "work" — the inversion the user hit.
//!
//! No amount of retuning a constant fixes that, because the right chain is a property of the line,
//! not of the product. So the engine measures it:
//!
//!   * a ladder ordered LEAST-INVASIVE FIRST (`engine::tuner_ladder`), starting with doing nothing;
//!   * every candidate judged on TWO sets at once — did the blocked targets open, AND did the
//!     control sites keep working (`verify::probe_protection`);
//!   * any candidate that harms the control set is REJECTED no matter how well it scores, and
//!   * if nothing passes, the winner is "off". Doing nothing is a legitimate answer and a strictly
//!     better one than shipping damage.
//!
//! The result is persisted per network, so this runs once per line rather than once per start.

use crate::engine::{self, BypassEngine};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// How long to let a candidate's winws attach WinDivert before probing through it.
///
/// The old Auto-Pilot slept a flat 1200ms per candidate, which across a 15-candidate ladder is 18
/// seconds of pure waiting. Measured cold-start on the test laptop had winws live and filtering in
/// well under 100ms; 350ms keeps a comfortable margin without paying for it fifteen times.
const SETTLE: Duration = Duration::from_millis(350);

/// One candidate's measured result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Row {
    pub strategy: String,
    /// Blocked targets that opened.
    pub opened: usize,
    pub total: usize,
    /// Control sites this candidate BROKE. Non-zero disqualifies the candidate outright.
    pub broke: usize,
    /// Mean handshake time over the targets that opened (0 when none did).
    pub avg_ms: u32,
    /// Passed both gates: opened a majority of targets and broke nothing.
    pub accepted: bool,
    /// Is this chain incapable of corrupting a connection (`Strategy::is_harmless`)?
    /// Used as a tie-break so "the gentlest chain that works" stays true even when the two passes
    /// mean a risky candidate happens to be measured before a harmless one.
    #[serde(default)]
    pub harmless: bool,
}

impl Row {
    /// Ranking key. Coverage dominates; latency is a tie-break, never a reason to prefer a
    /// candidate that opens fewer sites. Harmful candidates are ranked below everything.
    fn score(&self) -> i64 {
        if self.broke > 0 {
            return i64::MIN + self.opened as i64;
        }
        // Coverage dominates. Among equal coverage a HARMLESS chain wins outright (the bonus is
        // larger than any latency difference can be), and only then does speed decide. Without this
        // the two-pass ladder could deploy a route-dependent TTL chain when a chain that cannot
        // corrupt anything scored identically.
        (self.opened as i64) * 1_000_000
            + if self.harmless { 10_000 } else { 0 }
            - (self.avg_ms as i64)
    }
}

/// The persisted outcome of a tuning run for one network.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Tuning {
    /// Coarse network fingerprint (see `network_key`) — used to notice "different line, re-tune".
    pub network: String,
    /// Local wall-clock time of the run, for display.
    pub at: String,
    /// Unix seconds, for the retention check. `at` is human-readable and locale-shaped; comparing
    /// it as a date would mean parsing our own formatting back, which is how off-by-a-timezone
    /// retention bugs happen.
    #[serde(default)]
    pub at_epoch: u64,
    /// Winning strategy id. `"off"` means: measured, and the honest answer is to touch nothing.
    pub strategy: String,

    // NOTE: the target DOMAIN LIST used to be persisted here.
    //
    // It was removed on 2026-08-16, before reaching for encryption, because it did not need to
    // exist: the list is fully re-derivable from the active mode (`Engine::probe_targets`), so
    // storing it bought nothing and put the exact category of data we most want off disk — which
    // sites this user cares about — into a plain JSON file. Encrypting data you did not need to
    // keep is the weaker fix.
    //
    // What remains is deliberately impersonal: a /24 network fingerprint, a timestamp, the winning
    // chain's id, and per-candidate COUNTS. None of it names a site.
    /// How many targets the run was judged against — the count, never the names.
    #[serde(default)]
    pub target_count: usize,
    /// Every candidate tried, best first — this is what the UI shows so the choice is inspectable.
    pub rows: Vec<Row>,
    /// True when NO candidate opened the targets without collateral damage.
    pub gave_up: bool,
}

/// Where the tuning result lives.
pub fn path() -> std::path::PathBuf {
    crate::ipc::data_dir().join("tuning.json")
}

pub fn load() -> Option<Tuning> {
    let s = std::fs::read_to_string(path()).ok()?;
    serde_json::from_str(&s).ok()
}

pub fn save(t: &Tuning) -> Result<(), String> {
    let dir = crate::ipc::data_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("data dir: {e}"))?;
    let json = serde_json::to_string_pretty(t).map_err(|e| e.to_string())?;
    // Write-then-rename so a crash mid-write cannot leave a truncated file that parses as an empty
    // tuning and silently reverts the line to untuned.
    let tmp = path().with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| format!("tuning write: {e}"))?;
    std::fs::rename(&tmp, path()).map_err(|e| format!("tuning commit: {e}"))?;
    // No domain names live in here any more (see the note on `Tuning`), so this is not protecting
    // site history — it is keeping one account's measured network profile away from other accounts.
    crate::secure::harden_acl(&path());
    Ok(())
}

/// How long a measurement stays valid before it is discarded and re-measured.
///
/// A retention limit, not just a cache policy. Keeping a record of this machine's network
/// behaviour indefinitely is data we have no ongoing reason to hold: DPI configurations change,
/// ISPs change routing, and a result from months ago is more likely to be wrong than useful. 30
/// days is long enough that a normal user measures roughly once a month, short enough that nothing
/// accumulates a long-term profile.
pub const RETENTION_DAYS: u64 = 30;

fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Is the stored tuning still about the network we are on now, AND still fresh enough to trust?
pub fn is_current(t: &Tuning) -> bool {
    if t.network.is_empty() || t.network != network_key() {
        return false;
    }
    // `at_epoch == 0` means "written before retention existed, or the clock was unreadable" — treat
    // that as expired rather than as eternally valid. Erring toward re-measuring costs seconds;
    // erring toward keeping costs a stale config and an unbounded retention window.
    if t.at_epoch == 0 {
        return false;
    }
    let age = now_epoch().saturating_sub(t.at_epoch);
    age <= RETENTION_DAYS * 86_400
}

/// Delete a measurement that has aged out, so expiry actually REMOVES the data rather than merely
/// ignoring it. Called on load; a retention limit that only hides old records is not a retention
/// limit.
pub fn purge_if_expired() {
    if let Some(t) = load() {
        let expired = t.at_epoch == 0 || now_epoch().saturating_sub(t.at_epoch) > RETENTION_DAYS * 86_400;
        if expired {
            let _ = std::fs::remove_file(path());
            crate::elog::info(
                "tuner",
                "retention",
                "the stored measurement aged past the retention limit and was deleted",
            );
        }
    }
}

/// Coarse fingerprint of the line we are attached to: the /24 of the address the OS picks for
/// outbound traffic (e.g. `192.168.1`).
///
/// Deliberately cheap and dependency-free — no adapter enumeration, no privileges, no FFI. The
/// UDP socket is never sent on; `connect` on UDP only fixes the route so `local_addr` reveals which
/// interface would be used.
///
/// LIMITATION, stated rather than hidden: two different ISPs both handing out 192.168.1.x look
/// identical to this. That is tolerable because a wrong key only costs one unnecessary re-tune —
/// and the engine re-tunes anyway whenever verification fails, which is the case that matters.
pub fn network_key() -> String {
    use std::net::UdpSocket;
    let Ok(sock) = UdpSocket::bind("0.0.0.0:0") else {
        return String::new();
    };
    if sock.connect("1.1.1.1:53").is_err() {
        return String::new();
    }
    match sock.local_addr() {
        Ok(std::net::SocketAddr::V4(a)) => {
            let o = a.ip().octets();
            format!("{}.{}.{}", o[0], o[1], o[2])
        }
        Ok(std::net::SocketAddr::V6(a)) => format!("v6:{}", a.ip()),
        Err(_) => String::new(),
    }
}

/// Measure the ladder on this line and return the verdict.
///
/// `dpi` must already be STOPPED by the caller (the service pauses the live engine first) — this
/// drives it directly, one candidate at a time, and leaves it stopped on the way out.
///
/// `on_row` is called as each candidate finishes so the UI can stream the table instead of staring
/// at a spinner for the whole run.
pub fn run<F: FnMut(&Row)>(
    dpi: &mut dyn BypassEngine,
    targets: &[String],
    ladder: &[&str],
    mut on_row: F,
) -> Tuning {
    let targets: Vec<String> = targets.to_vec();
    let mut rows: Vec<Row> = Vec::new();
    let mut budget: Option<Duration> = None;
    let winner = measure(dpi, &targets, ladder, &mut rows, &mut budget, &mut on_row);
    dpi.stop();
    decide(targets.len(), rows, winner)
}

/// Two-pass measurement: the mechanism representatives first, the remaining variants only if the
/// first pass did not find something good enough.
///
/// Why: measuring all 15 candidates took 91 SECONDS on the user's line (their log, 2026-08-16), and
/// most of that was spent separating variants of a mechanism that had already been shown not to
/// work. Candidates cannot be measured concurrently — each needs its own winws, and two winws
/// instances fight over the single global WinDivert driver — so the only lever is measuring fewer
/// of them.
pub fn run_two_pass<F: FnMut(&Row)>(
    dpi: &mut dyn BypassEngine,
    targets: &[String],
    mut on_row: F,
) -> Tuning {
    /// Fraction of targets a first-pass result must open for the second pass to be skipped.
    const GOOD_ENOUGH: f64 = 0.8;

    let targets: Vec<String> = targets.to_vec();
    let mut rows: Vec<Row> = Vec::new();

    let mut budget: Option<Duration> = None;

    // MOST LIKELY FIRST. Whatever won last time is the single best guess for what will win now —
    // even if it was measured on a different network or has aged past retention, it beats starting
    // from the middle of a generic ladder. It goes SECOND, after `off`: the baseline still has to be
    // established first, or there is nothing to judge "better than doing nothing" against.
    let previous = load().map(|t| t.strategy).filter(|s| s != "off" && !s.is_empty());
    let mut first_pass: Vec<&str> = engine::tuner_ladder_first_pass().to_vec();
    if let Some(prev) = previous.as_deref() {
        if let Some(pos) = first_pass.iter().position(|c| *c == prev) {
            let c = first_pass.remove(pos);
            first_pass.insert(1.min(first_pass.len()), c);
            crate::elog::info("tuner", "prior", &format!("trying the previously chosen chain '{prev}' first"));
        }
    }

    let mut winner = measure(dpi, &targets, &first_pass, &mut rows, &mut budget, &mut on_row);

    if winner.is_none() {
        let best = rows.iter().filter(|r| r.broke == 0).map(|r| r.opened).max().unwrap_or(0);
        let total = rows.first().map(|r| r.total).unwrap_or(targets.len()).max(1);
        let good_enough = best as f64 >= total as f64 * GOOD_ENOUGH;
        if good_enough {
            crate::elog::info(
                "tuner",
                "second_pass_skipped",
                &format!(
                    "first pass opened {best}/{total} with nothing broken — the remaining {} variants \
                     are not measured. Re-run the measurement to try them.",
                    engine::tuner_ladder_second_pass().len()
                ),
            );
        } else {
            crate::elog::info(
                "tuner",
                "second_pass",
                &format!("first pass only reached {best}/{total} — measuring the remaining variants"),
            );
            winner =
                measure(dpi, &targets, engine::tuner_ladder_second_pass(), &mut rows, &mut budget, &mut on_row);
        }
    }

    dpi.stop();
    decide(targets.len(), rows, winner)
}

/// Measure one ladder, appending to `rows`. Returns `Some(id)` if a candidate opened EVERY target
/// (in which case there is nothing left worth trying, because the ladder is gentlest-first).
fn measure<F: FnMut(&Row)>(
    dpi: &mut dyn BypassEngine,
    targets: &[String],
    ladder: &[&str],
    rows: &mut Vec<Row>,
    budget: &mut Option<Duration>,
    on_row: &mut F,
) -> Option<String> {
    let targets: Vec<String> = targets.to_vec();
    let mut winner: Option<String> = None;

    crate::elog::info(
        "tuner",
        "start",
        &format!("measuring {} candidates against {} targets", ladder.len(), targets.len()),
    );

    for id in ladder {
        let s = candidate_strategy(id);
        let has_stage = engine::has_web_stage(&s, &targets);

        // "off" (and anything else with no web stage) is measured with the engine genuinely
        // stopped, not with a winws running that happens to do nothing. Otherwise the row would be
        // measuring the driver's presence rather than the strategy.
        dpi.stop();
        if has_stage {
            if let Err(e) = dpi.start(&s, &targets) {
                crate::elog::warn("tuner", "candidate_failed", &format!("{id}: {e}"));
                let row = Row {
                    strategy: (*id).into(),
                    opened: 0,
                    total: targets.len(),
                    broke: 0,
                    avg_ms: 0,
                    accepted: false,
                    harmless: s.is_harmless(),
                };
                on_row(&row);
                rows.push(row);
                continue;
            }
            std::thread::sleep(SETTLE);

            // The engine must still be ALIVE at probe time, or the row measures the bare line while
            // claiming to measure this candidate.
            //
            // This is not hypothetical: in the user's log two candidates recorded impossibly fast
            // probes (256-278ms against a ~4100ms norm) immediately after `winws exited immediately
            // (code 1)`, and both scored 3/9 — exactly the "off" baseline. Those rows were noise
            // presented as data, and one of them could have been the right answer.
            if !dpi.is_running() {
                crate::elog::warn(
                    "tuner",
                    "candidate_died",
                    &format!("{id}: the engine exited before it could be measured — retrying once"),
                );
                dpi.stop();
                if dpi.start(&s, &targets).is_ok() {
                    std::thread::sleep(SETTLE);
                }
                if !dpi.is_running() {
                    crate::elog::warn(
                        "tuner",
                        "candidate_unmeasurable",
                        &format!("{id}: engine would not stay up — recorded as unmeasured, not as a failure"),
                    );
                    let row = Row {
                        strategy: (*id).into(),
                        opened: 0,
                        total: targets.len(),
                        // `broke: 1` keeps this row out of the winner ranking entirely (see
                        // Row::score). An unmeasured candidate must never be *selected*, and it
                        // must never be silently scored as "opened nothing" either.
                        broke: 1,
                        avg_ms: 0,
                        accepted: false,
                        harmless: s.is_harmless(),
                    };
                    on_row(&row);
                    rows.push(row);
                    continue;
                }
            }
        }

        let t0 = Instant::now();
        // ADAPTIVE budget, derived from this line rather than assumed.
        //
        // A fixed fast budget was wrong in both directions. Too long and every candidate costs a
        // full timeout (the 91-second run). Too short and a merely SLOW line reads as a broken one:
        // on the user's second run, handshakes legitimately took 1.3-2.5s and a 1.2s budget made
        // one candidate look like it had broken all three control sites at once.
        //
        // So the first candidate — always "off", i.e. the bare line — is measured with the
        // conclusive budget to learn what "normal" costs here, and every candidate after it gets
        // 3x that, clamped to something sane.
        let this_budget = budget.unwrap_or(crate::verify::IO_TIMEOUT);
        let report = crate::verify::probe_protection_within(&targets, this_budget);
        if budget.is_none() {
            let slowest = report
                .targets
                .iter()
                .chain(report.control.iter())
                .filter(|r| r.ok)
                .map(|r| r.ms)
                .max()
                .unwrap_or(0);
            let derived = Duration::from_millis(((slowest as u64) * 3).clamp(1_200, 4_000));
            *budget = Some(derived);
            crate::elog::info(
                "tuner",
                "budget",
                &format!(
                    "this line's slowest healthy handshake is {slowest}ms — allowing {}ms per probe                      for the remaining candidates",
                    derived.as_millis()
                ),
            );
        }
        let opened = report.targets.iter().filter(|r| r.ok).count();
        let broke = report.control.iter().filter(|r| !r.ok).count();
        let avg_ms = {
            let oks: Vec<u32> = report.targets.iter().filter(|r| r.ok).map(|r| r.ms).collect();
            if oks.is_empty() { 0 } else { (oks.iter().map(|v| *v as u64).sum::<u64>() / oks.len() as u64) as u32 }
        };
        // A candidate is accepted only if it opens a MAJORITY of targets and breaks NOTHING.
        // `report.harm` is the majority-of-control verdict; here the bar is stricter — a single
        // broken control site is enough to disqualify, because the alternative ("off") is free.
        let accepted = report.ok && broke == 0;
        let row = Row {
            strategy: (*id).into(),
            opened,
            total: report.targets.len(),
            broke,
            avg_ms,
            accepted,
            harmless: s.is_harmless(),
        };

        crate::elog::info(
            "tuner",
            "candidate",
            &format!(
                "{id}: opened {}/{}, broke {} control, avg {}ms, probe took {}ms{}",
                opened,
                row.total,
                broke,
                avg_ms,
                t0.elapsed().as_millis(),
                if accepted { " → ACCEPTED" } else { "" }
            ),
        );
        on_row(&row);

        // STOP AT THE FIRST CANDIDATE THAT WORKS — not the best one available.
        //
        // The old rule only stopped on a clean sweep (every target open), so on a line where
        // nothing reaches 100% it measured the entire ladder every time. That is where most of the
        // 33-second cold start went, and a user waiting on a blank screen does not benefit from us
        // finding a marginally better chain.
        //
        // "Works" has to mean more than "accepted", though. `off` is measured first to establish
        // the baseline, and on a lightly-filtered line it can open a majority all by itself — if
        // that counted as a win we would stop at "do nothing" and never try the chain that opens
        // the rest. So a candidate must ALSO beat the baseline, unless it opens everything (in
        // which case there is nothing left to beat).
        let baseline = rows.first().filter(|r| r.strategy == "off").map(|r| r.opened);
        let good_enough = is_good_enough(accepted, opened, row.total, baseline);
        rows.push(row);
        if good_enough {
            winner = Some((*id).to_string());
            crate::elog::info(
                "tuner",
                "early_stop",
                &format!("'{id}' works on this line — stopping here rather than measuring the rest"),
            );
            break;
        }
    }

    winner
}

/// Should the measurement STOP at this candidate?
///
/// This single rule is what the cold-start time depends on, so it lives on its own and is tested
/// directly rather than only through a run that needs a live engine and a network.
///
/// A candidate stops the search when it is `accepted` (opened a majority of targets AND broke no
/// control site) and either:
///   * it opened EVERYTHING, so there is nothing left to improve on; or
///   * it beat the baseline — the `off` row measured first. Without this second clause the search
///     would stop at "do nothing" on any line where a majority of targets are reachable anyway,
///     and never discover the chain that opens the rest.
///
/// `baseline == None` means this pass has no `off` row to compare against (the second pass), where
/// any accepted candidate is already better than what the first pass found.
fn is_good_enough(accepted: bool, opened: usize, total: usize, baseline: Option<usize>) -> bool {
    if !accepted {
        return false;
    }
    if opened == total {
        return true;
    }
    match baseline {
        Some(b) => opened > b,
        None => true,
    }
}

/// Turn the measured rows into a verdict.
fn decide(target_count: usize, rows: Vec<Row>, winner: Option<String>) -> Tuning {
    let gave_up;
    let strategy = match winner {
        Some(w) => {
            gave_up = false;
            w
        }
        None => {
            // No clean sweep. Take the best candidate that at least broke nothing; if even that
            // opened nothing, choose "off" — protection that damages traffic is worse than none,
            // and saying so is more useful than pretending.
            let mut ranked: Vec<&Row> = rows.iter().filter(|r| r.broke == 0).collect();
            ranked.sort_by_key(|r| std::cmp::Reverse(r.score()));
            match ranked.first() {
                Some(best) if best.opened > 0 => {
                    gave_up = false;
                    best.strategy.clone()
                }
                _ => {
                    gave_up = true;
                    "off".to_string()
                }
            }
        }
    };

    if gave_up {
        crate::elog::error(
            "tuner",
            "no_working_strategy",
            "no strategy opened the targets without breaking ordinary sites — falling back to \
             DNS-only protection rather than damaging traffic",
        );
    } else {
        crate::elog::info("tuner", "winner", &format!("selected '{strategy}' for this line"));
    }

    let mut sorted = rows;
    sorted.sort_by_key(|r| std::cmp::Reverse(r.score()));

    Tuning {
        network: network_key(),
        at: crate::elog::stamp(),
        at_epoch: now_epoch(),
        strategy,
        target_count,
        rows: sorted,
        gave_up,
    }
}

/// Build the strategy a candidate is measured with — the SAME shape it will be deployed in.
///
/// This matters: measuring a chain catch-all and then shipping it hostlist-gated (or the reverse)
/// means the measurement does not describe the thing that runs. Candidates are therefore always
/// gated to the target list with the harmless catch-all layer behind them, exactly as
/// `SetProtectionMode` deploys them.
/// Test-only view of `candidate_strategy`, so the deployment side (`service.rs`) can assert that
/// what it ships matches what was measured without duplicating the rule.
#[cfg(test)]
pub fn candidate_strategy_for_test(id: &str) -> engine::Strategy {
    candidate_strategy(id)
}

fn candidate_strategy(id: &str) -> engine::Strategy {
    let mut s = engine::strategy_by_id(id);
    s.hostlist_only = true;
    // "off" means off: no gated stage AND no fallback layer, so the row measures the bare line.
    s.fallback = if id == "off" { "" } else { engine::SAFE_FALLBACK };
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(strategy: &str, opened: usize, broke: usize, avg_ms: u32) -> Row {
        Row {
            strategy: strategy.into(),
            opened,
            total: 4,
            broke,
            avg_ms,
            accepted: broke == 0 && opened >= 3,
            harmless: engine::strategy_by_id(strategy).is_harmless(),
        }
    }

    /// The whole point of the module: a candidate that breaks ordinary sites can never outrank one
    /// that does not, however many targets it opens.
    #[test]
    fn harmful_candidates_rank_below_everything_clean() {
        let harmful = row("turkcell-hotspot", 4, 2, 100);
        let clean_weak = row("off", 1, 0, 900);
        assert!(clean_weak.score() > harmful.score(), "a harmless partial win must beat a harmful full win");
    }

    #[test]
    fn coverage_dominates_latency() {
        let slow_broad = row("c1", 4, 0, 900);
        let fast_narrow = row("safe-split", 2, 0, 50);
        assert!(slow_broad.score() > fast_narrow.score(), "opening more sites must outrank being faster");
        // Latency only breaks ties at equal coverage.
        let fast_broad = row("safe-fake", 4, 0, 90);
        assert!(fast_broad.score() > slow_broad.score(), "at equal coverage, faster wins");
    }

    /// The ladder must start with "off" — otherwise a DNS-only block gets packets mangled for no
    /// reason, which is exactly how the reported failure happened.
    #[test]
    fn ladder_starts_with_doing_nothing() {
        let ladder = engine::tuner_ladder();
        assert_eq!(ladder.first().copied(), Some("off"), "the gentlest candidate must be tried first");
        // And every id in the ladder must actually resolve to a distinct known strategy.
        for id in ladder {
            let s = engine::strategy_by_id(id);
            assert_eq!(s.id, *id, "ladder entry '{id}' does not resolve to a real strategy");
        }
    }

    /// Harmless chains must come before route-dependent TTL ones, or the "gentlest that works"
    /// guarantee is just a comment.
    #[test]
    fn ladder_is_ordered_least_invasive_first() {
        let ladder = engine::tuner_ladder();
        let pos = |id: &str| ladder.iter().position(|c| *c == id).expect("id in ladder");
        let last_harmless = ladder
            .iter()
            .enumerate()
            .filter(|(_, id)| engine::strategy_by_id(id).is_harmless())
            .map(|(i, _)| i)
            .max()
            .expect("at least one harmless candidate");
        let first_harmful = ladder
            .iter()
            .enumerate()
            .filter(|(_, id)| !engine::strategy_by_id(id).is_harmless())
            .map(|(i, _)| i)
            .min()
            .expect("at least one harmful candidate to guard against");
        assert!(
            last_harmless < first_harmful,
            "every harmless candidate must be tried before the first one that can break a working site"
        );
        assert!(pos("off") < pos("turkcell-hotspot"));
    }

    /// A candidate is measured in the exact shape it ships in — gated, with the safe layer behind
    /// it — except "off", which must be bare.
    #[test]
    fn candidates_are_measured_as_deployed() {
        let c1 = candidate_strategy("c1");
        assert!(c1.hostlist_only, "candidates must be measured hostlist-gated, as deployed");
        assert_eq!(c1.fallback, engine::SAFE_FALLBACK);
        let off = candidate_strategy("off");
        assert_eq!(off.fallback, "", "'off' must measure the bare line, with no fallback layer");
        assert!(off.desync.is_empty());
    }

    /// The two passes must together cover the ENTIRE ladder. A strategy that is in neither list is
    /// one the tuner will never try — it would sit in the catalog looking supported while silently
    /// never being measured, which is the quietest possible way to lose the right answer.
    #[test]
    fn the_two_passes_cover_every_candidate_exactly_once() {
        let full = engine::tuner_ladder();
        let mut covered: Vec<&str> = engine::tuner_ladder_first_pass()
            .iter()
            .chain(engine::tuner_ladder_second_pass().iter())
            .copied()
            .collect();
        let n = covered.len();
        covered.sort_unstable();
        covered.dedup();
        assert_eq!(covered.len(), n, "a candidate appears in both passes");

        let mut all: Vec<&str> = full.to_vec();
        all.sort_unstable();
        assert_eq!(covered, all, "the two passes must cover exactly the full ladder");
    }

    /// The first pass is what most runs will ever execute, so IT is the list that has to honour
    /// "everything harmless before anything that can break a working site".
    #[test]
    fn the_first_pass_is_ordered_least_invasive_first() {
        let pass = engine::tuner_ladder_first_pass();
        assert_eq!(pass.first().copied(), Some("off"));
        let last_harmless = pass
            .iter()
            .enumerate()
            .filter(|(_, id)| engine::strategy_by_id(id).is_harmless())
            .map(|(i, _)| i)
            .max()
            .expect("first pass must contain harmless candidates");
        let first_harmful = pass
            .iter()
            .enumerate()
            .filter(|(_, id)| !engine::strategy_by_id(id).is_harmless())
            .map(|(i, _)| i)
            .min()
            .expect("first pass must also cover the risky mechanism, or it cannot find it");
        assert!(last_harmless < first_harmful, "first pass must try every harmless chain first");
    }

    /// At equal coverage the chain that cannot corrupt a connection must win. Without this the
    /// two-pass split could deploy a route-dependent TTL chain over an equally effective harmless
    /// one purely because of measurement order.
    #[test]
    fn harmless_wins_ties() {
        let risky = Row { strategy: "turkcell-hotspot".into(), opened: 4, total: 5, broke: 0, avg_ms: 100, accepted: true, harmless: false };
        let safe = Row { strategy: "safe-fake".into(), opened: 4, total: 5, broke: 0, avg_ms: 180, accepted: true, harmless: true };
        assert!(safe.score() > risky.score(), "at equal coverage the harmless chain must win, even if slower");
        // But coverage still dominates: a harmless chain that opens LESS does not win.
        let safe_weak = Row { opened: 3, ..safe.clone() };
        assert!(risky.score() > safe_weak.score(), "harmlessness must not outrank actually working");
    }

    /// An unmeasurable candidate (engine would not stay up) must be excluded from selection rather
    /// than scored as "opened nothing" — otherwise a transient driver conflict silently demotes a
    /// candidate that might have been the right answer.
    #[test]
    fn an_unmeasured_candidate_can_never_be_selected() {
        let unmeasured = Row { strategy: "c1".into(), opened: 0, total: 5, broke: 1, avg_ms: 0, accepted: false, harmless: true };
        let nothing = Row { strategy: "off".into(), opened: 0, total: 5, broke: 0, avg_ms: 0, accepted: false, harmless: true };
        assert!(nothing.score() > unmeasured.score(), "an unmeasured row must rank below even a useless measured one");
        let t = decide(1, vec![unmeasured, nothing], None);
        assert_eq!(t.strategy, "off");
        assert!(t.gave_up, "no candidate opened anything → say so");
    }

    /// The rule that decides how long a user waits on first run.
    #[test]
    fn the_search_stops_at_the_first_candidate_that_actually_helps() {
        // Doing nothing opened 3 of 9. A candidate that opens 7 and breaks nothing is a real win.
        assert!(is_good_enough(true, 7, 9, Some(3)), "should stop at the first clear improvement");
        // ...but one that merely matches the baseline is not: stopping there would deploy a chain
        // that achieves exactly what doing nothing achieves.
        assert!(!is_good_enough(true, 3, 9, Some(3)));
        assert!(!is_good_enough(true, 2, 9, Some(3)), "worse than doing nothing is never a stop");
        // A clean sweep always stops, even with no baseline to compare against.
        assert!(is_good_enough(true, 9, 9, None));
        assert!(is_good_enough(true, 9, 9, Some(9)));
        // Not accepted (minority opened, or it broke a control site) can never stop the search.
        assert!(!is_good_enough(false, 9, 9, Some(0)));
    }

    /// `off` is measured FIRST so a baseline exists at all — the rule above is meaningless without
    /// it, and "stop at the first thing that works" would then stop at doing nothing.
    #[test]
    fn the_baseline_is_always_measured_first() {
        assert_eq!(engine::tuner_ladder_first_pass().first().copied(), Some("off"));
    }

    #[test]
    fn network_key_is_stable_within_a_run() {
        let a = network_key();
        let b = network_key();
        assert_eq!(a, b, "the fingerprint must not flap between calls");
    }

    /// A torn/absent tuning file must read as "untuned", never as a valid empty selection.
    #[test]
    fn missing_tuning_is_not_a_silent_empty_selection() {
        let t = Tuning::default();
        assert!(!is_current(&t), "an empty network key must never match the live network");
    }
}
