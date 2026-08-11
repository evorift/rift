# evorift v2 — Backend Rewrite Plan

_Created: 2026-08-11. Supersedes [BACKEND-HARDENING-PLAN.md](BACKEND-HARDENING-PLAN.md)._

**Decision.** Instead of hardening the v1 backend, we're rewriting it from scratch
under the name `evocore`. Rationale: v1's strategy model sits at the wrong layer —
strategy is a **startup argument** to an external process (winws). That's why changing
strategy means restarting the process, which means dropping the connection, and why
individual parameters (like fake count) can't be tuned live. This isn't a bug — it's an
architectural consequence, and patches won't fix it.

**v2 in one sentence:** strategy is not an argument, it's **runtime data**.

---

## Product goal (every item in this plan serves this)

1. **Easier than GoodbyeDPI.** Install, open, it works. No command line, no .bat
   files, no parameter memorization.
2. **Gets stronger without dropping the connection.** A setting change doesn't cut
   existing flows; when it's not enough, it escalates on its own.
3. **Doesn't lie.** "Protected" is only ever written when it's a measured fact.
4. **Accelerates.** Throttles background traffic during a game, watches app usage
   and warns.

## Execution rules

- **One message = one item.** Apply the next unchecked item, verify it, check it
  off, stop.
- **One session = one item group.** When done, `/session-close`, new session.
- Verify order: the 5 steps in the `evorift-rust-tauri` skill.
- Anything requiring the network isn't a unit test → `evorift-live-verification`.
- Every network/driver-touching item is subject to `evorift-security-hygiene` rules.
- `[x]` done · `[ ]` to do · `[~]` half-done · `[?]` awaiting a decision

## What moves from v1 (not rewritten)

These already work, get copied over, and are fitted to v2's contracts:
rollback journal concept · preflight checks · manifest/SHA verification ·
Svelte UI skeleton · Tauri IPC setup · domain lists and profile format.

**Off-limits:** `BlackHole.svelte` (CLAUDE.md hard rule 1).

---

## V0 — Skeleton and contracts

_No behavior yet; just the types everything else sits on. Getting this wrong
means the next 9 phases pay for it._

- [ ] **V0.1 Workspace split.** Cargo workspace: `evocore` (pure logic, testable
  without network or Windows API), `evosys` (Windows/driver/FFI), `evoapp` (Tauri).
  Rule: no `windows` crate dependency inside `evocore`.
  _Test:_ `cargo test -p evocore` passes on a machine with no driver.
- [ ] **V0.2 Result and error types.** `Outcome::{Applied, Skipped(reason)}`,
  `ApplyReport { steps: Vec<StepResult>, failed: Vec<..> }`, meaningful error
  enums. `#[must_use]` everywhere. **No sim mode** — running unprivileged is
  `Err(NotElevated)`, a missing bundle is `Err(BundleMissing)`. Only a developer
  mode gated behind `EVORIFT_SIM=1` exists, and it shows red in the UI.
  _Test:_ apply is `Err` in an unprivileged environment; reaching the `Active`
  state is impossible.
- [ ] **V0.3 Injectable environment.** Resource root, clock, privilege check, and
  filesystem behind a trait. Without this, "bundle missing," "no permission,"
  "timeout" scenarios can't be tested.
  _Test:_ all three scenarios can be set up in a unit test with a fake environment.
- [ ] **V0.4 Config schema and migration.** Versioned schema for strategy/profile/
  settings files; a read path from v1's config. _Test:_ v1 config loads successfully.

## V1 — Capture and flow layer (L0-L2)

> ⚠ **V1.1 is BLOCKED — as written below it violates CLAUDE.md hard rule 3** (no
> in-process WinDivert engine). Resolved 2026-08-11: rule 3 stays a hard ban; V1.1 needs
> a redesign that gets live-tunable strategy control over an **external**-process capture
> layer instead of an in-process one. See QUESTIONS.md Q1 and the K1 decision brief in
> BACKLOG.md's DECISIONS section — this item is not ready to implement as-is.

- [ ] **V1.1 `PacketSource` trait + WinDivert implementation.** RAII-wrapped
  handle, closed on `Drop`. Filter expression starts narrow (only the port/
  direction of interest); a broad filter is forbidden — processing game traffic
  for nothing wrecks ping.
  _Test:_ the flow engine is testable with a fake source; no handle leaks.
- [ ] **V1.2 Flow table.** 5-tuple → state. Lifecycle: SYN → established →
  FIN/RST → cleanup. Pruned by timeout. Hard upper bound (memory can't blow up).
  _Test:_ under a 100k-flow simulation, memory stays bounded and cleanup runs.
- [ ] **V1.3 PID mapping.** Flow → process (IP Helper tables, cached). This is the
  foundation for both per-app rules and the V7 accelerator.
  _Test:_ a known socket's PID is found correctly; cache aging is correct.
- [ ] **V1.4 Parse layer.** TLS ClientHello→SNI, QUIC Initial→SNI, HTTP Host,
  DNS question name. **Resilient to truncated/malformed input** — parsing never
  panics.
  _Test:_ fuzz (truncated, length-lying, nested-extension inputs).

## V2 — Strategy engine (L3-L4)

_This is where v1's real unsolved problem gets solved._

- [ ] **V2.1 Action primitives.** `split`, `disorder`, `fake(n, ttl, variant)`,
  `seg`, `oob`, `tlsrec`, `quicfrag`. Each a pure function: (packet, parameter)
  → packet list. Lives in `evocore`, testable without a driver.
  _Test:_ known input → expected byte output, for every primitive.
- [ ] **V2.2 Rule model.** Rule = matcher (domain pattern / IP / port / PID) +
  action chain. Priority list, first match wins. Serializable as data (JSON),
  hand-writable and shareable.
  _Test:_ a sample rule set dispatches the expected chain to the expected flow.
- [ ] **V2.3 Hot swap.** The active rule set lives in an `ArcSwap`-style lock-free
  structure. When a new set is published: **existing flows keep running on their
  own set**, new flows get the new one. No restart, no lock, no drop.
  _Test:_ an open flow's rule reference doesn't change during a swap; a new flow
  gets the new rule; the swap takes under 1 ms.
- [ ] **V2.4 v1 strategy migration.** A converter from existing winws argument
  sets to v2's rule format, plus a library of known-working strategies.
  _Test:_ 5 strategies that work in v1 convert 1:1 into v2 rules.

## V3 — Live tuning and auto-escalation (L5)

_Your #1 requested feature._

- [ ] **V3.1 Parameter channel.** A single parameter change from the UI (fake
  count, split offset, TTL) takes effect immediately through V2.3.
  _Test:_ change a parameter with 50 open flows → zero drops, new flows get the
  new value.
- [ ] **V3.2 Result feedback.** Per-flow result classification: TLS completed /
  RST arrived / timed out / reset-then-worked. Short per-domain memory (which
  chain held for this domain).
  _Test:_ a synthetic RST flow classifies correctly; memory updates correctly.
- [ ] **V3.3 Escalation engine.** When a domain is failing, auto-strengthen the
  chain in order `split → tlsrec → fake(1) → fake(2..N) → disorder → seg → oob`;
  pin and save the winning chain on success. Escalation is per-domain, not
  global. A hard ceiling and backoff are mandatory — no infinite escalation.
  _Test:_ a synthetic target that fails the first N attempts reaches success via
  escalation and stays on that chain.
- [ ] **V3.4 De-escalation.** If the heavy chain is no longer needed (the domain
  now opens plainly), lighten it gradually — for performance.
  _Test:_ once the block lifts, the chain gets lighter.

## V4 — Honest state and proof (L7)

- [ ] **V4.1 Measured state machine.** States: `Off / Applying / Applied-unverified
  / Verified / Broken(reason)`. **`Applied` is never shown as "Protected."**
  _Test:_ state can't be `Verified` while verification is failing.
- [ ] **V4.2 Canary probe.** After apply, a real TLS handshake attempt to 2-3
  target domains; time-budgeted, in the background, without blocking the user.
  _Test:_ successful/failed probe scenarios with a fake network layer.
- [ ] **V4.3 Continuous monitoring.** Periodic lightweight check; state drops on
  breakage and triggers V3.3 escalation.
  _Test:_ state drops to `Broken` when the target goes down.
- [ ] **V4.4 Liveness measurement.** `is_running` is everywhere the measured
  truth (process/driver/service state), not a remembered bool. Stale cleanup
  on launch.
  _Test:_ the next query returns `false` after the process is killed.
- [ ] **V4.5 UI proof panel.** "Protected and verified — 6/6 targets open,
  measured 14s ago." If broken, show the reason and a one-click fix.
  `svelte-check` clean.

## V5 — Autopilot

- [ ] **V5.1 Search space.** Candidate generation over rule templates; coarse →
  fine search. Only applicable candidates (bundle/permission present).
  _Test:_ candidates ⊆ applicable.
- [ ] **V5.2 Streaming results.** Results stream to the UI line by line as they
  arrive; cancelable.
  _Test:_ cancel takes effect within 200ms, leaves nothing behind.
- [ ] **V5.3 ISP profiles.** Ready-made starting sets for Turkcell / TTNet /
  Vodafone / Turk.net; the user picks their ISP, search starts from there.
  _Test:_ profile selection changes the search order.
- [ ] **V5.4 Apply and verify the winner.** A found strategy isn't called "found"
  until it passes V4 verification.
  _Test:_ an unverifiable candidate is never declared the winner.

## V6 — Tunnel layer (optional path)

_The part of v1 that caused the most trouble. Redone, in the right order._

- [ ] **V6.1 Tunnel abstraction.** WireGuard tunnel behind a trait; setup,
  handshake state, teardown. Stale tunnel gets cleaned up on launch.
  _Test:_ cached `true` + no service running → `false`.
- [ ] **V6.2 Split mode.** Only selected domains/IPs go through the tunnel. Route
  setup is written to the journal.
  _Test:_ the route table contains the expected entries and rolls back cleanly.
- [ ] **V6.3 Full mode (Full Protection) ordering.** Entry: **stop the DPI engine
  first, then bring up the tunnel.** Exit: **tear down the tunnel first, then
  restore the previous engine.**
  _Test:_ in full mode `dpi.is_running() == false`; on exit the previous engine
  is back.
- [ ] **V6.4 Handshake verification.** A 15s grace/poll window on first connect;
  a 180s threshold for `handshake_ago_secs` freshness. Setup errors surface,
  never get swallowed.
  _Test:_ a failed setup produces `Broken` + a message.
- [ ] **V6.5 Kill switch.** If the tunnel drops, an optional firewall rule blocks
  non-tunnel egress + an immediate UI warning. Off by default, explicit warning
  when enabling.
  _Test:_ forcing the tunnel down leaks nothing; turning it off removes the rule.
- [ ] **V6.6 MTU / IPv6 conflict resolution.** IPv6's minimum link MTU is 1280;
  clamping below that can disable v6. Decision: clamp only in the IPv4 case, or
  clamp below 1280 deliberately disables v6 and the UI says so.
  _Test:_ the generated config keeps the v6 address; the clamp round-trips
  consistently.

## V7 — Accelerator

- [ ] **V7.1 Per-process traffic measurement.** Instant up/down bandwidth per
  process, via V1.3. _Test:_ a known download is attributed to the correct process.
- [ ] **V7.2 Advisor mode (do this first).** "Steam is downloading at 42 Mbps,
  might be hurting your game ping — pause it?" One-click pause for known
  downloaders. A suggestion, not a restriction — the safest, highest-value first step.
  _Test:_ the suggestion fires past the threshold; false-positive rate is measured.
- [ ] **V7.3 Game mode and throttling.** When a game process is detected,
  token-bucket bandwidth-limit background processes. **The priority process is
  never throttled.** The limit fully lifts when the mode ends, no leftovers.
  _Test:_ the limit shows up in measured Mbps; the game flow's latency is
  unaffected; nothing lingers on exit.
- [ ] **V7.4 App scan and report.** Periodically: which app used how much,
  which ones keep talking in the background. Local, no telemetry.
  _Test:_ the report is produced and contains no secrets/domain history
  (`evorift-security-hygiene`).
- [ ] **V7.5 Latency budget protection.** The accelerator layer's own added
  latency is measured; if the budget is exceeded, the layer disables itself.
  _Test:_ added latency under synthetic load stays under the threshold.

## V8 — Install, trust, distribution

- [ ] **V8.1 Setup wizard.** Three screens: pick ISP → scan → verify. Works with
  "Next, Next, Finish" using defaults. Target: measurably shorter than a
  GoodbyeDPI install.
- [ ] **V8.2 Portable mode.** Run without installing; tries (as much as possible)
  without leaving a permanent change. An entry point for AV-wary users.
- [ ] **V8.3 Full uninstall.** Driver service, firewall, DNS, route, scheduled
  task, registry, autostart. _Test:_ the system is clean after uninstall (checklist).
- [ ] **V8.4 Log redaction.** A structural masking layer; keys/tokens/IPs can't
  be written to a log. _Test:_ a structure containing a secret gets masked when
  handed to the logger.
- [ ] **V8.5 License compliance and manifest.** License text + SHA-256 for every
  bundled binary. **WinDivert is dynamically linked**, LGPL obligations are met.
  _Test:_ `verify_manifest()` is all `ok`; the license folder is complete.
- [ ] **V8.6 Signing and updates.** Signed installer, a signature-verifying
  updater, published SHA-256s, a single official channel. An explanation page
  for the AV warning.

## V9 — Licensing and revenue (serverless)

_No cloud, no server of our own — this is a design choice, not a constraint._

- [ ] **V9.1 Offline license verification.** An Ed25519-signed license file;
  the app carries only the **public key**. No internet needed, no server needed.
  _Test:_ a valid license is accepted, a tampered one is rejected, an expired
  one is rejected.
- [ ] **V9.2 Free / premium split.** Free: all bypass, autopilot, verification,
  **basic acceleration (single setting)**. Premium: game mode and per-process
  throttling, app profiles, advanced diagnostic history, strategy import/export.
  **Access features are never locked.**
  _Test:_ premium paths are closed and the UI is honest in an unlicensed build.
- [ ] **V9.3 Payment flow.** Gumroad/Shopier/LemonSqueezy key generation → user
  pastes the key → V9.1 verifies it. Donation flow is separate and unpushy.
  _Test:_ the end-to-end key flow is manually verified.
- [ ] **V9.4 Upgrade moment.** The premium offer is shown only **after the proof
  panel turns green**, and only once. No pestering offer of any kind — in this
  category, trust is the only currency.

## V10 — Live verification and release

- [ ] **V10.1 Live verification run.** The full protocol from the
  `evorift-live-verification` skill; results into `docs/LIVE-VERIFICATION.md`.
  **Pass criterion: all targets open + a measurable difference from baseline +
  zero drops on a live setting change.**
- [ ] **V10.2 Second ISP.** Repeat on at least one different ISP. A single-ISP
  verification doesn't mean "it works."
- [ ] **V10.3 Unprivileged and broken-install run.** The app is honest in both
  cases; no fake "Active."
- [ ] **V10.4 Green gate.** The 5-step verify order at zero warnings + a clean
  uninstall checklist + manifest ok + signature verifies.
- [ ] **V10.5 Release.** Sign the installer, publish the SHA, fix the CHANGELOG
  (the 0.1.0–0.1.2 CPU bug's cause was not a third-party client but evorift's
  own domain-monitoring loop — state this explicitly), merge to main.

---

## Awaiting decision (goes to the `architect` subagent)

- [?] **K1 — Capture layer (REFRAMED 2026-08-11, see QUESTIONS.md Q1).** Original framing
  below is superseded — it assumed in-process capture was already decided, which
  contradicts CLAUDE.md hard rule 3 (kept as a hard ban). The real question: **how do you
  get runtime-tunable strategy control over an external-process capture layer** (in the
  spirit of the current `winws` sidecar), without in-process WinDivert? Candidate
  directions to weigh in the brief: a persistent external process with a control
  channel/IPC that accepts live rule updates (vs. today's restart-per-change model), a
  different external tool that already exposes live reconfiguration, or something else.
  Decision needed before V1.1 restarts. Original (rejected) framing, kept for context:
  _"Stick with WinDivert (known, LGPL, AV-flagged), or move to a TUN-based path? TUN means
  writing your own TCP/IP stack (weeks) but gives cleaner control for V7. Recommendation:
  WinDivert for V1, thanks to the `PacketSource` trait a second implementation can follow
  later."_ — this assumed in-process capture, which is now off the table.
- [?] **K2 — V7.3 throttling technique.** WinDivert queuing, or Windows QoS
  policy? The former gives more control but carries latency risk. Decision
  before V7.3.
- [?] **K3 — Does the tunnel stay?** V6 is the most expensive phase, and if
  V1-V5 work correctly, most users may not need it. **Recommendation: decide
  with live data after V5 is done; defer V6 to the end.**

## Out of scope

- Own server/relay infrastructure (legally falls into "service provider"
  territory, a separate decision).
- Mobile client.
- `BlackHole.svelte`.
