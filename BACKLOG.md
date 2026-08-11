# BACKLOG.md — evorift v2 Ready-to-Paste Work Items

Generated from [BACKEND-V2-PLAN.md](BACKEND-V2-PLAN.md), in plan order. Each item below is
meant to be pasted as-is into a fresh session (or `feature-flow`'s "task definition" step).
Cross-references: migration source evidence in [docs/MIGRATION.md](docs/MIGRATION.md),
discovery evidence in [docs/DISCOVERY.md](docs/DISCOVERY.md), open decisions in
[QUESTIONS.md](QUESTIONS.md).

**Standing rules for every item:** follow `feature-flow` (mini plan → small commits →
`test-runner` verification → `evo-review`). Verify order and build discipline: the
`evorift-rust-tauri` skill. Anything network/driver/DNS/firewall-touching: the
`evorift-security-hygiene` skill. Anything about strategy/DPI/layers: the `evorift-dpi`
skill. Anything requiring a live connection isn't a unit test: the
`evorift-live-verification` skill — prepare it, the user runs it. One session = one item
(or a small tight group). `git push` is the user's.

`[x]` done · `[ ]` to do · `[~]` half-done · `[?]` blocked/awaiting a decision — status
markers here should be kept in sync with `BACKEND-V2-PLAN.md`'s own checkboxes.

---

## P0 — Ahead of V0: current-app fixes (surfaced 2026-08-12, resolved 2026-08-13)

These are bugs in the app **as it ships today**, named in `net3/SOLUTION.md` as things
`net3` fixed for its own console app. All three were checked against the actual current
code before any change was made (per this session's recompute-don't-assume discipline) —
two turned out to already be fixed.

### P0-a — Admin manifest / `requireAdministrator` — `[x]` implemented 2026-08-13
Cited: `net3/SOLUTION.md` line 213. Was genuinely missing — `src-tauri/build.rs` now embeds
a `requireAdministrator` manifest (commit `ad278e7`). **Accepted, confirmed tradeoff:**
this removes the previously-intentional unprivileged/"limited" runtime mode and UAC-prompts
on every launch (evorift builds two binaries from one shared `build.rs`; Cargo has no way
to scope the manifest to only the UI binary). Compiles clean, 78/78 tests pass — **not**
live-verified that the prompt actually appears or resolves anything; see the live run below.

### P0-b — EXE-relative path resolution — `[x]` already correct, no change needed
Cited: `net3/SOLUTION.md` line 487. Checked `engine.rs::bundle_dir()` (`:478-480`) directly:
already resolves via `current_exe().parent()`, not cwd. Confirmed Tauri's resource bundling
(`tauri.conf.json:50`, `winws/winws.exe` → `<exe_dir>\winws\winws.exe`) places the bundle at
exactly the path `bundle_dir()` expects, in both dev (`target/debug/winws/winws.exe` exists)
and release — net3's extra fallback branches (shipped-flat/dev-ancestor-walk/cargo-manifest-dir)
exist to work around problems Tauri's build system already solves uniformly here.

### P0-c — WinDivert version conflict / single instance — `[x]` already correct, more thorough than net3
Cited: `net3/SOLUTION.md` lines 163, 465 (not line 335 — that's about a different point,
service-vs-console persistence preference; corrected during triage). Checked `engine.rs`
directly: `kill_all()` (`:501-536`) plus `clear_stale_windivert()` — the latter **goes
further than net3's own reference**, which only does `taskkill /f /im winws.exe` (the
process). evorift's version also stops and deletes the stale `WinDivert`/`WinDivert1.4`/
`WinDivert1.1` **driver services**, with a safe stop→verify→delete sequence, and `start()`
(`:707-727`) already detects instant-exit (the classic conflict symptom) and retries after
cleanup. Feeds CLAUDE.md rule 3b(b) as evidence that this class of problem already has a
working solution in the current app.

---

## DECISIONS — architect-ready briefs (drafted, not yet called)

Per `architect`'s input contract: decision question, options with cost/risk, binding
constraints, evidence summaries (not raw code). **Model: Opus 5 per user directive — brief
only, no broad exploration on that model; any exploration these briefs still need should
run on a Sonnet subagent first and get folded into the brief before calling architect.**

### K1 — Capture layer (RE-REFRAMED 2026-08-12, see QUESTIONS.md Q1, docs/FORENSICS.md B6)

- **Decision question:** WinDivert vs. a TUN-based path for `evosys`'s `PacketSource`
  implementation. In-process capture itself is no longer in question — CLAUDE.md rule 3b
  permits it, since `net3/SOLUTION.md §3.3`'s finding (no desync engine can carry Discord's
  gateway payload) turned out to be about a tunneling gap, not an in-process-capture ban.
- **Options to evaluate (fill in cost/risk before calling architect):**
  1. WinDivert (known, LGPL, AV-flagged) — plan's standing recommendation for V1, since the
     `PacketSource` trait lets a second implementation follow later.
  2. TUN-based path — writing your own TCP/IP stack (weeks) but gives cleaner control for V7.
- **Binding constraint on either option (rule 3b — not optional polish):** (a)
  handle-lifecycle safety (RAII/`Drop`, panic, kill, process exit) proven by test — the old
  engine's was never documented, a gap per docs/FORENSICS.md B2, not a clean record to
  build on; (b) WinDivert version conflict with a co-installed zapret/winws is handled (a
  real, previously-hit failure — net3/SOLUTION.md §4.2/§9). See P2 below — this needs its
  own investigation before the brief is complete.
- **Other binding constraints:** rule 5/11 (strategy change must not drop the connection
  once V2.3 lands), `evorift-security-hygiene` (no silent privilege escalation, rollback
  journal coverage), single-developer maintenance burden.
- **Evidence to attach:** docs/FORENSICS.md B1-B6 in full (the removal-reason excavation),
  `docs/DISCOVERY.md`/`docs/MIGRATION.md` items 1/2/5 (rollback, preflight, IPC).
- **Pre-work before calling architect:** P2 (WinDivert version-conflict handling, below)
  should be at least scoped, since it's now a binding constraint on the decision, not a
  follow-up to it.
- **Blocks:** nothing anymore — V1.1 and everything downstream is unblocked (see below);
  K1 only needs to pick WinDivert vs. TUN before V1.1 locks in its implementation.

### K2 — V7.3 throttling technique

- **Decision question:** WinDivert-based packet queuing vs. Windows QoS policy for
  per-process bandwidth throttling in Game Mode?
- **Options:** (1) WinDivert queuing — more control, latency risk on the hot path
  (`evorift-rust-tauri`'s "no allocation in the hot path" rule applies directly). (2)
  Windows QoS policy — OS-native, less control, likely lower latency risk.
- **Binding constraints:** V7.5's latency budget (accelerator's own added latency must stay
  under a threshold and self-disable if it doesn't), V7.3's "priority process never
  throttled" rule, K1's WinDivert-vs-TUN outcome (does the chosen capture layer already see
  enough traffic to also handle throttling, or is this a separate mechanism entirely?).
- **Pre-work before calling architect:** none yet — this decision isn't urgent until V7
  is reached; revisit the brief once V1-V6 are further along and K1 is resolved (K1's
  outcome may change what's even available here).
- **Blocks:** V7.3 only.

### K3 — Does the tunnel stay?

- **Decision question:** Is V6 (WireGuard/WARP tunnel layer) worth building, or does most
  of the user base not need it if V1-V5 work correctly?
- **Options:** (1) Build V6 as planned. (2) Defer/drop V6, ship V1-V5+V7-V10 without it.
- **Binding constraints:** V6 is explicitly the most expensive phase in the plan; the
  plan's own recommendation is to decide with live data after V5, not now.
- **Pre-work before calling architect:** live-verification data from V5 (autopilot success
  rate without the tunnel) — this brief cannot be usefully filled in yet.
- **Blocks:** V6.x only. Don't call architect on this before V5 has live data — there's
  nothing to evaluate yet.

---

## V0 — Skeleton and contracts

### V0.1 — Workspace split
- **Crate:** new Cargo workspace root (currently a single `src-tauri` package, no
  `[workspace]` table — confirmed by docs-auditor 2026-08-11).
- **Files:** new `Cargo.toml` workspace root, `evocore/`, `evosys/`, `evoapp/` member crates.
- **Task:** split into a Cargo workspace with 3 members. Rule: `evocore` has zero
  dependency on the `windows` crate.
- **Acceptance criteria:** `cargo test -p evocore` passes on a machine with no driver installed.
- **Verify:** `evorift-rust-tauri` skill's 5-step order, step 1-2 only (nothing to clippy/UI-check yet).
- **Skill/subagent:** `feature-flow` for the work; `code-referee` review before commit
  (workspace layout is exactly the kind of thing that's expensive to redo later).

### V0.2 — Result and error types
- **Crate:** `evocore`
- **Files:** new `evocore/src/result.rs` or similar — `Outcome`, `ApplyReport`, error enums.
- **Task:** `Outcome::{Applied, Skipped(reason)}`, `ApplyReport{steps, failed}`, meaningful
  error enums (`NotElevated`, `BundleMissing`, etc.), `#[must_use]` everywhere. No sim mode
  except `EVORIFT_SIM=1` dev-only, shown red in UI.
- **Acceptance criteria:** apply is `Err` in an unprivileged environment; reaching `Active` is impossible.
- **Verify:** `cargo test -p evocore`.
- **Skill/subagent:** `evorift-rust-tauri` (panic/unwrap/Result rules apply directly here — this is the item that sets the pattern every later item follows).

### V0.3 — Injectable environment
- **Crate:** `evocore` (trait definitions), implementations split across `evosys`/`evoapp` as needed.
- **Files:** new trait module for resource root, clock, privilege check, filesystem.
- **Task:** put resource root, clock, privilege check, filesystem behind a trait so
  "bundle missing," "no permission," "timeout" scenarios are unit-testable.
- **Acceptance criteria:** all three scenarios can be set up in a unit test with a fake environment.
- **Verify:** `cargo test -p evocore`.
- **Skill/subagent:** `feature-flow`. This item is a dependency for V1.1's testability, the
  V1 (1) rollback and (3) manifest migration items in `docs/MIGRATION.md`, and V0.4 below — do it early.

### V0.4 — Config schema and migration
- **Crate:** `evocore`
- **Files:** new config schema module; reference `src-tauri/src/profile.rs:1-391` (v1
  profile format, portable per `docs/MIGRATION.md` item 7) as the source format to read.
- **Task:** versioned schema for strategy/profile/settings files; a read path from v1's config.
- **Acceptance criteria:** v1 config loads successfully (use existing test profiles from
  `profile.rs`'s test module as fixtures).
- **Verify:** `cargo test -p evocore`.
- **Skill/subagent:** `feature-flow`; `code-analyst` first if the v1 profile validation
  logic (`profile.rs` rejects bad id/engine/hostlist) needs to be understood in depth before porting.

---

## V1 — Capture and flow layer (L0-L2)

> **Unblocked 2026-08-12** (see DECISIONS above, QUESTIONS.md Q1, docs/FORENSICS.md B6).
> V1.1 may proceed once K1 picks WinDivert vs. TUN; either way, rule 3b's two conditions
> (handle-lifecycle proof, WinDivert version-conflict handling) are mandatory acceptance
> criteria, not optional follow-ups.

### V1.1 — `PacketSource` trait + WinDivert implementation
- **Crate:** `evosys`
- **Files:** new `evosys/src/capture/` module (or wherever K1's implementation choice lands).
- **Task:** RAII-wrapped handle, closed on `Drop`. Filter expression starts narrow (only the
  port/direction of interest) — a broad filter is forbidden, processing game traffic for
  nothing wrecks ping. **Rule 3b acceptance criteria (mandatory, not optional):** (a) test
  proves handle lifecycle is safe across normal drop, panic, `kill`, and process exit —
  the old engine never documented this, don't repeat that gap; (b) test or documented
  procedure proves WinDivert version conflict with a co-installed zapret/winws is detected
  and handled, not silently broken (see P2 below for the investigation this needs first).
- **Acceptance criteria:** the flow engine is testable with a fake source; no handle leaks;
  both rule 3b criteria above pass.
- **Verify:** `cargo test -p evosys`; `evorift-rust-tauri`'s FFI/unsafe rules apply directly
  (every `unsafe` block needs a `// SAFETY:` comment).
- **Skill/subagent:** `feature-flow`; `saboteur` pass specifically on handle lifecycle
  (panic mid-capture, kill during a filter swap, process exit with packets in flight) before
  commit — this is exactly the class of bug the old engine's undocumented lifecycle could
  have hidden.
- **Depends on:** K1 (WinDivert vs. TUN) should be picked first, though the RAII/`Drop`
  structure and the fake-source testability don't otherwise depend on which is chosen.

### V1.2 — Flow table
- **Crate:** `evocore`
- **Files:** new flow-table module.
- **Task:** 5-tuple → state. Lifecycle: SYN → established → FIN/RST → cleanup. Timeout-pruned, hard upper bound on memory.
- **Acceptance criteria:** under a 100k-flow simulation, memory stays bounded and cleanup runs.
- **Verify:** `cargo test -p evocore` with a synthetic load test.
- **Skill/subagent:** `feature-flow`; `saboteur` pass afterward (concurrent flows, boundary
  flow counts, mid-cleanup kill are exactly its remit).
- **Depends on:** nothing K1-related — flow-table logic is capture-source-agnostic if built
  against a trait/mock source; can proceed once V0.1-V0.3 land, in parallel with V1.1.

### V1.3 — PID mapping
- **Crate:** `evosys` (IP Helper tables are Windows-specific)
- **Files:** new PID-mapping module.
- **Task:** flow → process (IP Helper tables, cached). Foundation for per-app rules and V7.
- **Acceptance criteria:** a known socket's PID is found correctly; cache aging is correct.
- **Verify:** `cargo test -p evosys` (Windows-only test target).
- **Skill/subagent:** `feature-flow`.

### V1.4 — Parse layer
- **Crate:** `evocore`
- **Files:** new parse module (TLS ClientHello, QUIC Initial, HTTP Host, DNS question).
- **Task:** extract SNI/Host from TLS ClientHello, QUIC Initial, HTTP Host, DNS question
  name. Must never panic on truncated/malformed input.
- **Acceptance criteria:** fuzz testing (truncated, length-lying, nested-extension inputs) finds no panics.
- **Verify:** `cargo test -p evocore` + a fuzz target.
- **Skill/subagent:** `feature-flow` for the build; `saboteur` specifically for malformed-input testing before commit — this is exactly its remit (boundary/malformed input against a parser).

---

## V2 — Strategy engine (L3-L4)

### V2.1 — Action primitives
- **Crate:** `evocore`
- **Files:** new action-primitives module.
- **Task:** `split`, `disorder`, `fake(n, ttl, variant)`, `seg`, `oob`, `tlsrec`,
  `quicfrag` as pure functions `(packet, parameter) → packet list`. Reference:
  `evorift-dpi` skill's primitive table for what each one does and when it's used.
- **Acceptance criteria:** known input → expected byte output, for every primitive.
- **Verify:** `cargo test -p evocore`.
- **Skill/subagent:** `feature-flow`; consult `evorift-dpi` before writing any primitive — the skill's escalation-order table constrains how these compose later (V3.3).

### V2.2 — Rule model
- **Crate:** `evocore`
- **Files:** new rule-model module.
- **Task:** rule = matcher (domain pattern/IP/port/PID) + action chain, priority list, first
  match wins, serializable as JSON.
- **Acceptance criteria:** a sample rule set dispatches the expected chain to the expected flow.
- **Verify:** `cargo test -p evocore`.
- **Skill/subagent:** `feature-flow`.

### V2.3 — Hot swap
- **Crate:** `evocore`
- **Files:** rule-model module (extends V2.2).
- **Task:** active rule set in an `ArcSwap`-style lock-free structure; existing flows keep
  their old rule set on swap, new flows get the new one. No restart, no lock, no drop.
- **Acceptance criteria:** an open flow's rule reference doesn't change during a swap; a new
  flow gets the new rule; swap takes under 1ms.
- **Verify:** `cargo test -p evocore` with a concurrency/timing test. `evorift-rust-tauri`'s
  concurrency rules apply directly (no `.await` while holding a lock, lock-free config swap).
- **Skill/subagent:** `feature-flow`; this is the item CLAUDE.md hard rule 5/11 and the
  whole plan's "strategy is runtime data" premise hinge on — get `code-referee` review
  before commit regardless of diff size.

### V2.4 — v1 strategy migration
- **Crate:** `evocore` (converter logic), reads from v1 config format defined in V0.4
- **Files:** new converter module; reference `src-tauri/src/engine.rs:572-666`
  (`WinwsEngine::args()`, confirmed by docs-auditor) as the source argument format to convert from.
- **Task:** convert v1 winws argument sets into v2 rule format (V2.2); build a library of known-working strategies.
- **Acceptance criteria:** 5 strategies that work in v1 convert 1:1 into v2 rules.
- **Verify:** `cargo test -p evocore` with the 5 converted strategies as fixtures.
- **Skill/subagent:** `feature-flow`; `code-analyst` first on `engine.rs`'s argument-building logic if the conversion isn't mechanical.

---

## V3 — Live tuning and auto-escalation (L5)

### V3.1 — Parameter channel
- **Crate:** `evoapp` (UI-facing) → `evocore` (via V2.3's hot swap)
- **Task:** a single parameter change from the UI (fake count, split offset, TTL) takes effect immediately through V2.3.
- **Acceptance criteria:** change a parameter with 50 open flows → zero drops, new flows get the new value.
- **Verify:** `cargo test` + prepare a manual reproduction for `evorift-live-verification` once there's a real engine to test against.
- **Skill/subagent:** `feature-flow`.

### V3.2 — Result feedback
- **Crate:** `evocore`
- **Task:** per-flow result classification (TLS completed / RST arrived / timed out /
  reset-then-worked); short per-domain memory of which chain held.
- **Acceptance criteria:** a synthetic RST flow classifies correctly; memory updates correctly.
- **Verify:** `cargo test -p evocore`.
- **Skill/subagent:** `feature-flow`.

### V3.3 — Escalation engine
- **Crate:** `evocore`
- **Task:** auto-strengthen a failing domain's chain in order `split → tlsrec → fake(1) →
  fake(2..N) → disorder → seg → oob` (per `evorift-dpi`'s escalation table); pin the
  winning chain on success. Per-domain, not global. Hard ceiling + backoff — no infinite escalation.
- **Acceptance criteria:** a synthetic target that fails the first N attempts reaches
  success via escalation and stays on that chain.
- **Verify:** `cargo test -p evocore`.
- **Skill/subagent:** `feature-flow`; `saboteur` pass on the backoff/ceiling logic (what
  happens at the ceiling, does it actually stop escalating).

### V3.4 — De-escalation
- **Crate:** `evocore`
- **Task:** lighten the chain gradually once a domain no longer needs the heavy version.
- **Acceptance criteria:** once the block lifts, the chain gets lighter.
- **Verify:** `cargo test -p evocore`.
- **Skill/subagent:** `feature-flow`.

---

## V4 — Honest state and proof (L7)

### V4.1 — Measured state machine
- **Crate:** `evoapp` (or `evocore` if the state machine itself is kept pure and only wired
  to real state by `evoapp`/`evosys` — prefer this for testability, matches V0.2/V0.3's pattern)
- **Task:** states `Off / Applying / Applied-unverified / Verified / Broken(reason)`.
  `Applied` is never shown as "Protected" (CLAUDE.md rule 9).
- **Acceptance criteria:** state can't be `Verified` while verification is failing.
- **Verify:** `cargo test`.
- **Skill/subagent:** `feature-flow`; `code-referee` review — this is a hard-rule-enforcing item.

### V4.2 — Canary probe
- **Crate:** `evosys` or `evocore` depending on how the `docs/MIGRATION.md` item 2
  diagnose()/canary-probe overlap gets resolved — **check that decision before starting this item.**
- **Task:** after apply, a real TLS handshake attempt to 2-3 target domains; time-budgeted, background, non-blocking.
- **Acceptance criteria:** successful/failed probe scenarios testable with a fake network layer (V0.3).
- **Verify:** `cargo test`.
- **Skill/subagent:** `feature-flow`.

### V4.3 — Continuous monitoring
- **Crate:** `evoapp`/`evosys`
- **Task:** periodic lightweight check; state drops on breakage, triggers V3.3 escalation.
- **Acceptance criteria:** state drops to `Broken` when the target goes down.
- **Verify:** `cargo test`.
- **Skill/subagent:** `feature-flow`.

### V4.4 — Liveness measurement
- **Crate:** `evosys`
- **Task:** `is_running` everywhere queries real process/driver/service state, never a
  remembered bool (CLAUDE.md rule 4/10). Stale cleanup on launch.
- **Acceptance criteria:** the next query returns `false` after the process is killed.
- **Verify:** `cargo test -p evosys` (Windows-only, real process kill in test).
- **Skill/subagent:** `feature-flow`; `code-referee` review specifically for any cached-bool pattern this might reintroduce.

### V4.5 — UI proof panel
- **Crate:** frontend (`src/`), backed by `evoapp` commands from V4.1-V4.4
- **Task:** "Protected and verified — 6/6 targets open, measured 14s ago." Broken state shows reason + one-click fix.
- **Acceptance criteria:** `svelte-check` clean.
- **Verify:** `evorift-rust-tauri`'s full 5-step order (this is the first UI-touching item).
- **Skill/subagent:** `feature-flow`.

---

## V5 — Autopilot

### V5.1 — Search space
- **Crate:** `evocore`
- **Task:** candidate generation over rule templates (V2.2), coarse → fine search, only applicable candidates.
- **Acceptance criteria:** candidates ⊆ applicable.
- **Verify:** `cargo test -p evocore`.
- **Skill/subagent:** `feature-flow`.

### V5.2 — Streaming results
- **Crate:** `evoapp`
- **Task:** results stream to the UI line by line, cancelable.
- **Acceptance criteria:** cancel takes effect within 200ms, leaves nothing behind.
- **Verify:** `cargo test` + manual UI check.
- **Skill/subagent:** `feature-flow`.

### V5.3 — ISP profiles
- **Crate:** `evocore` (data), `evoapp`/frontend (selection UI)
- **Files:** reference `src-tauri/src/autopilot.rs:89-100` (v1's hardcoded lowercase ISP
  matching, flagged as brittle in `docs/DISCOVERY.md`) — improve on this, don't port it as-is.
- **Task:** ready-made starting sets for Turkcell/TTNet/Vodafone/Turk.net; user picks ISP, search starts from there.
- **Acceptance criteria:** profile selection changes the search order.
- **Verify:** `cargo test`.
- **Skill/subagent:** `feature-flow`.

### V5.4 — Apply and verify the winner
- **Crate:** `evoapp`, depends on V4's verification stack
- **Task:** a found strategy isn't "found" until it passes V4 verification.
- **Acceptance criteria:** an unverifiable candidate is never declared the winner.
- **Verify:** `cargo test`.
- **Skill/subagent:** `feature-flow`; `code-referee` review (this is a hard-rule item — no
  false "found" claims, mirrors rule 9).
- **Note:** completion of V5 is the trigger for gathering K3's live data (see DECISIONS above).

---

## V6 — Tunnel layer (optional path) — do not start before K3 is answered

### V6.1 — Tunnel abstraction
- **Crate:** `evosys`
- **Task:** WireGuard tunnel behind a trait: setup, handshake state, teardown. Stale tunnel cleaned up on launch.
- **Acceptance criteria:** cached `true` + no service running → `false`.
- **Verify:** `cargo test -p evosys`.
- **Skill/subagent:** `feature-flow`; `evorift-security-hygiene` applies (this is a driver/service/route-touching item).

### V6.2 — Split mode
- **Crate:** `evosys`
- **Task:** only selected domains/IPs through the tunnel; route setup written to the rollback journal.
- **Acceptance criteria:** route table contains expected entries, rolls back cleanly.
- **Verify:** `cargo test -p evosys`.
- **Skill/subagent:** `feature-flow`; `evorift-security-hygiene`.

### V6.3 — Full mode ordering
- **Crate:** `evosys`/`evoapp`
- **Task:** entry: stop DPI engine first, then bring up the tunnel. Exit: tear down tunnel first, then restore the previous engine (CLAUDE.md rule 3's "full tunnel" pitfall).
- **Acceptance criteria:** in full mode `dpi.is_running() == false`; on exit the previous engine is back.
- **Verify:** `cargo test` + `evorift-live-verification`'s Tunnel checklist.
- **Skill/subagent:** `feature-flow`; `saboteur` pass on the ordering (what if the process
  dies mid-sequence between stop-DPI and bring-up-tunnel).

### V6.4 — Handshake verification
- **Crate:** `evosys`
- **Task:** 15s grace/poll window on first connect; 180s freshness threshold for `handshake_ago_secs`. Setup errors surface, never swallowed.
- **Acceptance criteria:** a failed setup produces `Broken` + a message.
- **Verify:** `cargo test -p evosys`.
- **Skill/subagent:** `feature-flow`.

### V6.5 — Kill switch
- **Crate:** `evosys`
- **Task:** optional firewall rule blocking non-tunnel egress if the tunnel drops + immediate UI warning. Off by default, explicit warning on enable.
- **Acceptance criteria:** forcing the tunnel down leaks nothing; disabling removes the rule.
- **Verify:** `evorift-live-verification`'s Tunnel checklist (kill-switch item) — this needs a live network test, not just unit tests.
- **Skill/subagent:** `feature-flow`; `evorift-security-hygiene` (this is exactly the "kill-switch etkinleştirme" permission-gated case).

### V6.6 — MTU / IPv6 conflict resolution
- **Crate:** `evosys`
- **Task:** resolve the IPv6 minimum-MTU-1280-vs-clamp conflict — clamp only in the IPv4
  case, or clamp-below-1280 deliberately disables v6 with the UI saying so.
- **Acceptance criteria:** generated config keeps the v6 address; clamp round-trips consistently.
- **Verify:** `cargo test -p evosys`.
- **Skill/subagent:** `feature-flow`; this is small enough it may not need `architect`, but
  if the IPv4-only-clamp vs explicit-v6-disable choice is contested, write a brief.

---

## V7 — Accelerator — K2 decision needed before V7.3

### V7.1 — Per-process traffic measurement
- **Crate:** `evosys`, via V1.3's PID mapping
- **Task:** instant up/down bandwidth per process.
- **Acceptance criteria:** a known download is attributed to the correct process.
- **Verify:** `cargo test -p evosys`.
- **Skill/subagent:** `feature-flow`.

### V7.2 — Advisor mode (do this first within V7)
- **Crate:** `evoapp`
- **Task:** "Steam is downloading at 42 Mbps, might be hurting your game ping — pause it?"
  One-click pause for known downloaders. Suggestion, not restriction.
- **Acceptance criteria:** suggestion fires past the threshold; false-positive rate is measured.
- **Verify:** `cargo test` + `evorift-live-verification` for the false-positive measurement.
- **Skill/subagent:** `feature-flow`.

### V7.3 — Game mode and throttling — **BLOCKED on K2**
- **Crate:** `evosys` (technique depends on K2's outcome)
- **Task:** placeholder — fill in from K2's resolution (WinDivert queuing vs Windows QoS).
  Priority process never throttled; limit fully lifts when mode ends, no leftovers.
- **Acceptance criteria:** limit shows up in measured Mbps; game flow's latency unaffected; nothing lingers on exit.
- **Verify:** `evorift-live-verification`'s Accelerator checklist.
- **Skill/subagent:** `architect` for K2 first, then `feature-flow`.

### V7.4 — App scan and report
- **Crate:** `evosys`/`evoapp`
- **Task:** periodic report of which app used how much / talks in the background. Local, no telemetry.
- **Acceptance criteria:** report produced, contains no secrets/domain history.
- **Verify:** `cargo test` + `code-referee` review against `evorift-security-hygiene`'s checklist.
- **Skill/subagent:** `feature-flow`.

### V7.5 — Latency budget protection
- **Crate:** `evosys`
- **Task:** measure the accelerator layer's own added latency; self-disable if the budget is exceeded.
- **Acceptance criteria:** added latency under synthetic load stays under the threshold.
- **Verify:** `cargo test -p evosys` with a synthetic load test.
- **Skill/subagent:** `feature-flow`.

---

## V8 — Install, trust, distribution

### V8.1 — Setup wizard
- **Crate:** frontend (`src/`) + `evoapp`
- **Task:** three screens — pick ISP → scan → verify. Works with defaults ("Next, Next, Finish").
- **Acceptance criteria:** measurably shorter than a GoodbyeDPI install (define the measurement before claiming success).
- **Verify:** `evorift-rust-tauri`'s full 5-step order.
- **Skill/subagent:** `feature-flow`.

### V8.2 — Portable mode
- **Crate:** `evoapp`
- **Task:** run without installing, avoid permanent changes where possible.
- **Acceptance criteria:** define and document what "as much as possible" means for this build before marking done.
- **Verify:** manual + `evorift-live-verification`.
- **Skill/subagent:** `feature-flow`.

### V8.3 — Full uninstall
- **Crate:** `evosys`
- **Task:** driver service, firewall, DNS, route, scheduled task, registry, autostart — all removed.
- **Acceptance criteria:** system clean after uninstall (checklist — build the checklist from `evorift-security-hygiene`'s reversibility rules).
- **Verify:** `evorift-live-verification` (this needs a real machine check, not just unit tests).
- **Skill/subagent:** `feature-flow`; `code-referee` review against the security-hygiene checklist specifically.

### V8.4 — Log redaction
- **Crate:** `evocore` (pure masking logic) or shared utility
- **Task:** structural masking layer — keys/tokens/IPs can't be written to a log.
- **Acceptance criteria:** a structure containing a secret gets masked when handed to the logger.
- **Verify:** `cargo test`.
- **Skill/subagent:** `feature-flow`; `code-referee` — secret hygiene is a 🟥-blocker category in every review, verify this item especially hard.

### V8.5 — License compliance and manifest
- **Crate:** `evocore` (verification logic, ports from `docs/MIGRATION.md` item 3)
- **Task:** license text + SHA-256 for every bundled binary; WinDivert dynamically linked, LGPL obligations met.
- **Acceptance criteria:** `verify_manifest()` all `ok`; license folder complete.
- **Verify:** `cargo test`.
- **Skill/subagent:** `feature-flow`; `evorift-security-hygiene`'s license-compliance section directly.

### V8.6 — Signing and updates
- **Crate:** `evoapp` + build tooling
- **Task:** signed installer, signature-verifying updater, published SHA-256s, single official channel, AV-warning explanation page.
- **Acceptance criteria:** define per your actual signing setup — not verifiable in the abstract.
- **Verify:** `evorift-rust-tauri`'s "no release ships until the updater signature verifies" rule.
- **Skill/subagent:** `feature-flow`.

---

## V9 — Licensing and revenue (serverless)

### V9.1 — Offline license verification
- **Crate:** `evocore`
- **Task:** Ed25519-signed license file; app carries only the public key.
- **Acceptance criteria:** valid license accepted, tampered rejected, expired rejected.
- **Verify:** `cargo test -p evocore`.
- **Skill/subagent:** `feature-flow`; `code-referee` — signing-key handling is a secret-hygiene item.

### V9.2 — Free / premium split
- **Crate:** `evocore`/`evoapp`
- **Task:** free tier keeps all access features (bypass, autopilot, verification, basic
  acceleration); premium is game mode, per-process throttling, app profiles, diagnostic
  history, strategy import/export. Access features never locked (CLAUDE.md product constraint).
- **Acceptance criteria:** premium paths closed and UI honest in an unlicensed build.
- **Verify:** `cargo test` + manual check of an unlicensed build.
- **Skill/subagent:** `feature-flow`; `code-referee` for the "never locked" claim specifically.

### V9.3 — Payment flow
- **Crate:** `evoapp` + external service integration (Gumroad/Shopier/LemonSqueezy)
- **Task:** key generation → user pastes key → V9.1 verifies. Donation flow separate and unpushy.
- **Acceptance criteria:** end-to-end key flow manually verified.
- **Verify:** manual.
- **Skill/subagent:** `feature-flow`.

### V9.4 — Upgrade moment
- **Crate:** frontend
- **Task:** premium offer shown only after the V4.5 proof panel turns green, only once. No pestering.
- **Acceptance criteria:** manual verification of the trigger condition and one-time-only behavior.
- **Verify:** manual.
- **Skill/subagent:** `feature-flow`.

---

## V10 — Live verification and release

### V10.1 — Live verification run
- **Task:** full `evorift-live-verification` protocol; results into `docs/LIVE-VERIFICATION.md`.
- **Acceptance criteria:** all targets open + measurable difference from baseline + zero drops on a live setting change.
- **Skill/subagent:** `evorift-live-verification` — user runs it, Claude prepares commands and interprets results.

### V10.2 — Second ISP
- **Task:** repeat V10.1 on at least one different ISP.
- **Acceptance criteria:** same pass criteria as V10.1, on a different ISP.
- **Skill/subagent:** `evorift-live-verification`.

### V10.3 — Unprivileged and broken-install run
- **Task:** verify honest behavior in both cases — no fake "Active."
- **Acceptance criteria:** `evorift-live-verification`'s Honesty checklist passes.
- **Skill/subagent:** `evorift-live-verification`.

### V10.4 — Green gate
- **Task:** 5-step verify order at zero warnings + clean uninstall checklist + manifest ok + signature verifies.
- **Acceptance criteria:** all four conditions met simultaneously.
- **Skill/subagent:** `evorift-rust-tauri` + `evorift-live-verification`.

### V10.5 — Release
- **Task:** sign the installer, publish SHA, fix the CHANGELOG (state explicitly that the
  0.1.0–0.1.2 CPU bug's cause was evorift's own domain-monitoring loop, not a third-party
  client — flagged as a known risk in `docs/DISCOVERY.md` item 2), merge to main.
- **Acceptance criteria:** CHANGELOG corrected, installer signed, SHA published, merged.
- **Skill/subagent:** `feature-flow`; `git push` is the user's, per standing rule.

---

## P1/P2 — Open investigations (surfaced 2026-08-12, NOT solved here)

### P1 — Two different causes claimed for the same Discord-desktop symptom

`src-tauri/src/warp.rs`'s own doc-comment (`warp.rs:3-4`) says the Discord desktop client
sticks on "Starting…" because it prefers QUIC and the ISP kills that with
ICMP-unreachable, plus desync breaking the large JS packets it pulls. `net3/SOLUTION.md`
§3.3 (docs/FORENSICS.md B6) says the client sticks on "Connecting…" because the ISP
inspects and drops the gateway WebSocket's zstd-compressed `READY` payload — and lists
"Block QUIC → force TCP" as a *failed* mitigation attempt, i.e. QUIC wasn't the cause
found there. Both documents agree WARP split-tunnel is the fix; they don't agree on why
it's needed. One of them is wrong, and the current architecture (and this session's rule 3
rewrite) rests on `net3/SOLUTION.md`'s version. **Investigate before V6** (the tunnel
phase this decision actually gates) — not blocking V0-V1.

### P2 — WinDivert version-conflict handling when zapret/winws is co-installed

Named as a real, previously-hit failure in `net3/SOLUTION.md` §4.2/§9 ("WinDivert version
conflict... two apps shipping different versions makes the second fail" /
"`winws` exits instantly... another bypass loaded a different WinDivert version") and now
a binding condition on K1 (rule 3b) and an acceptance criterion on V1.1. Needs at least: how
`net3`/`winws` currently detects and handles this (`net3/src/winws.rs` per the doc), and
whether the same detection approach is available to a v2 in-process capture layer, or needs
its own mechanism. **Feeds the K1 brief directly** — see the K1 pre-work note above. Also
relevant to **P0-c**: whether the current app already bundles a matching WinDivert build
and kills stale `winws` (as `net3` does) affects whether P0-c is still an open bug in the
shipped app or already handled.

---

## P3 — Out-of-plan findings (from docs/DISCOVERY.md, not in BACKEND-V2-PLAN.md)

These surfaced during Phase 1 discovery and aren't part of the numbered plan. Triage them
when convenient — none block V0-V10 directly except where noted.

- **P3.1 — Community profile fetch has no hash pinning.** `profile.rs:216-231`'s
  `fetch_community()` downloads a JSON preset bundle from a user-supplied HTTPS URL with
  strict URL validation but no checksum/pinning on the fetched bundle. Already folded into
  V0/V6 item 6 in `docs/MIGRATION.md` as a required fix during that migration — don't
  treat as a separate follow-up, just don't let it slip when that item is worked.
- **P3.2 — `FRONTEND-CONTRACT.md` is enforced by a test.** v1's `ipc.rs` has a test
  (`contract_doc_covers_all_commands`) that fails if the contract doc doesn't list every
  command. When V1.1(replacement)/V2/V3's IPC surface (`docs/MIGRATION.md` item 5) is
  rebuilt, either port this test or explicitly decide to drop the enforcement — don't let
  it quietly disappear.
- **P3.3 — ISP preset detection is brittle.** `autopilot.rs:89-100` matches ISP names via
  hardcoded lowercase string comparison. Relevant to V5.3 — already noted there; listed
  here too so it isn't missed if V5.3 is worked by someone who skips docs/MIGRATION.md.
- **P3.4 — Game Mode's two recovery paths aren't unified.** UI-side snapshot
  (`state.svelte.ts`, localStorage-persisted) and the service-side rollback journal
  (`rollback.rs`) both exist but recover independently — if the service crashes mid-apply
  while Game Mode is on, only the rollback journal protects system state; the UI snapshot
  doesn't know about that. Worth a design note when V3.1/V4 are built, not urgent before then.
- **P3.5 — `BACKEND-HARDENING-PLAN.md` is stale and untracked.** Sits at repo root,
  untracked in git, explicitly superseded by `BACKEND-V2-PLAN.md`'s first line. Not removed
  this session (out of scope for an infrastructure-only pass) — flag for cleanup whenever
  docs get another pass, so it doesn't get mistaken for live guidance.
- **P3.6 — ByeDPI / GoodbyeDPI engine stubs not reviewed in depth.** `preflight.rs`
  references `make_engine("byedpi")` and similar, but `byedpi.rs`/`goodbyedpi.rs` weren't
  read in detail during discovery. Worth a `code-analyst` pass before V2.4 (v1 strategy
  migration) if either engine's argument format needs converting too, not just winws's.
