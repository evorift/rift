# evorift — Backend Hardening Plan

_Generated 2026-08-11, on project revival. Successor to [BACKEND-MASTER-PLAN.md](BACKEND-MASTER-PLAN.md)
(feature-complete, 78/78 unit tests, never verified live)._

**Premise.** The backend is not missing — it is *unverified and dishonest about failure*. Every phase of
the master plan closed with unit tests, but the code silently degrades to no-op ("sim") whenever it is
unprivileged or a binary is absent, and then reports `Active`. The user sees "protected" while nothing
happened. This plan makes the backend tell the truth, fixes the two real functional bugs, and proves
every path on a live machine.

## How we execute this plan
- **One item per message** (CLAUDE.md cadence). Implement the next unchecked item, verify it, mark it
  `[x]`, stop.
- Verify order: `cargo check --all-targets --target-dir tmp_check` → `cargo test --lib` →
  `cargo clippy --all-targets` → (dev closed) `svelte-check` → `npm run build`.
- `[x]` done · `[ ]` todo · `[~]` partial.

## Baseline verified 2026-08-11
`cargo check --all-targets` clean · `cargo test --lib` **78/78** · `evorift_0.1.3_x64-setup.exe` built
2026-06-26 · bundled binaries: `winws/` + `warp/` + WinDivert **only**.

---

## H1 — Truthful outcomes: kill silent success
_The single highest-impact fix. Today an unprivileged or bundle-less run is indistinguishable from a
working one._

**Evidence:**
- [sys.rs:125](src-tauri/src/sys.rs:125) — `run_os_env` returns `Ok(())` after logging `(sim)` when
  `!privileged()`. Same in `run_os_cwd` ([sys.rs:149](src-tauri/src/sys.rs:149)).
- [engine.rs:703](src-tauri/src/engine.rs:703) — `WinwsEngine::start` returns `Ok(())` when
  `winws.exe` is absent.
- [warp.rs:293](src-tauri/src/warp.rs:293) — `WarpEngine::start` sets **`running = true`** and returns
  `Ok(())` when `wireguard.exe` is absent. A tunnel that does not exist reports as up.
- [service.rs:468](src-tauri/src/service.rs:468) — on `Ok` the state machine goes `Active` regardless.

**Goal.** Distinguish *applied* from *simulated* end-to-end and surface it to the UI.

- [ ] **H1.1 Typed outcome in the system layer.** Add `sys::Outcome { Applied, Simulated(&'static str) }`;
  `run_os*` return `Result<Outcome, String>`. Unprivileged → `Ok(Simulated("not elevated"))`, never a
  bare `Ok`. Update all call sites (dns/firewall/tweak/limit/repair/rollback/services/schtask/wiresock/
  proxifyre) to propagate rather than discard. _Test:_ unprivileged run yields `Simulated`; the
  privileged path is unchanged.
- [ ] **H1.2 Engines report simulation.** `BypassEngine::start` returns `Result<Outcome, String>`.
  winws: bundle missing → `Simulated("winws bundle missing")`. warp: bundle missing → `Simulated`
  **and must not set `running = true`**. _Test:_ start with a bundle-less temp dir → `Simulated`,
  `is_running() == false`.
- [ ] **H1.3 State machine refuses to lie.** Add `RunState::Degraded` (or reuse `Error` with a reason).
  A `Simulated` apply must never reach `Active`. `EngineStatus` gains `simulated: bool` +
  `reason: String`; `HealthSignal.healthy` is false while simulated. _Test:_ apply in a sim environment
  → state is not `active`, `health.healthy == false`.
- [ ] **H1.4 UI surfaces it.** Banner when `status.simulated` — "Not actually protected: <reason>" with
  the fix (run as admin / install the service). Wire through `state.svelte.ts`. _Test:_ `svelte-check`
  clean; banner shows in a non-elevated dev run.

## H2 — Liveness must be measured, not remembered
- [ ] **H2.1 `WinwsEngine::is_running` consults the child.**
  [engine.rs:738](src-tauri/src/engine.rs:738) returns `self.child.is_some()` — a crashed winws still
  reports running until something else notices. Use `try_wait()` and clear the handle on exit.
  _Test:_ kill the child → `is_running()` false on the next call.
- [ ] **H2.2 `WarpEngine::is_running` consults the service.** Prefer the `WireGuardTunnel$warp` service
  state + last handshake over the cached bool. _Test:_ cached-true + service-absent → false.
- [ ] **H2.3 Watchdog reconciles mode, not just running.** [service.rs](src-tauri/src/service.rs) —
  the `_ => {}` arm never repairs a split↔full mismatch, and a leftover tunnel from a crashed run is
  never cleaned. Reconcile mode; force-uninstall a stale `WireGuardTunnel$warp` at startup.
  _Test:_ mismatch is corrected within one watchdog tick.

## H3 — Tam Koruma (full-tunnel) correctness
_Roadmap item 7 of [V0.1.3-BUGFIX-PLAN.md](V0.1.3-BUGFIX-PLAN.md); the DNS-pinning half already landed
at [warp.rs:385](src-tauri/src/warp.rs:385)._

- [ ] **H3.1 Stop the DPI engine while full WARP is active.**
  [service.rs:336](src-tauri/src/service.rs:336) `SetFullWarp` never touches `e.dpi`, so winws/WinDivert
  stays in front of the tunnel and can mangle plaintext packets *before* WireGuard encrypts them
  ("connected but nothing loads"). Stop the DPI engine on entering full mode; restore the previous
  engine + strategy on exit. In full mode the desync is redundant — all traffic is already encrypted.
  _Test:_ enter full → `dpi.is_running()` false; exit → prior engine restored.
- [ ] **H3.2 Verify a real handshake before claiming success.** Do not report full protection until
  `handshake_ago_secs` is `Some(_)` and fresh. Surface `/installtunnelservice` errors instead of
  audit-and-continue. _Test:_ a failing install produces `Error` + a message, not `Active`.
- [ ] **H3.3 IPv6 + MTU levers.** Confirm the v6 `Address` /128 survives config generation and the v6
  default route installs; expose the existing `set_mtu` clamp (1280 → 1200/1180) as a setting for
  PMTUD-blackholing ISPs. _Test:_ generated conf keeps the v6 address; the clamp round-trips.

## H4 — Catalog honesty: no engine the user cannot run
_Bundled today: winws + WARP. Adapters with **no binary**: byedpi (ciadpi), goodbyedpi, proxifyre,
drover, wiresock — all selectable, all sim no-ops._

- [ ] **H4.1 Decide per engine: bundle or hide.** Recommendation — bundle **goodbyedpi** (small, MIT,
  the useful WinDivert alternative) and **byedpi + proxifyre** (the kernel-less path for Kaspersky
  machines, which winws cannot serve); drop **drover** and **wiresock** from the shipped catalog for
  now. Whatever is not bundled must be filtered out of `engine::catalog()` and **rejected** by
  `ipc::validate` — not silently accepted. _Test:_ selecting a non-bundled engine returns an error.
- [ ] **H4.2 Bundle + manifest the chosen binaries.** Add to `resources/`, `tauri.conf.json` resources,
  and `resources/manifest.json` with real SHA-256s (`manifest.rs` already verifies). _Test:_
  `verify_manifest()` → all `ok`.
- [ ] **H4.3 Autopilot only proposes runnable engines.** Already availability-filtered — re-verify after
  H4.1 so a scan can never recommend something the user cannot apply. _Test:_ candidate list ⊆ available.

## H5 — Live verification on a real machine
_The step that was never done. Nothing below is unit-testable; it must be run on your connection._

- [ ] **H5.1 Smoke script via `evorift-ctl`.** Elevated: `preflight` → `services` → `on` →
  `diagnose discord.com discordapp.com discord.media roblox.com youtube.com googlevideo.com` →
  `status` → `off` → verify rollback left nothing behind. Record actual output in
  `docs/LIVE-VERIFICATION.md`. **Pass = 6/6 TLS open with the engine on, and a measurable difference
  with it off.**
- [ ] **H5.2 Discord desktop voice + Roblox join, end to end.** The two cases the whole product exists
  for. Test DPI-only, then WARP split, then Tam Koruma. Record which mode carries which case.
- [ ] **H5.3 Unprivileged run behaves honestly.** Launch without admin: the app must clearly say it is
  not protecting, and offer elevation / service install. This is H1 proven in the real app.
- [ ] **H5.4 Autopilot against the live ISP.** Run quick + standard depth; confirm the winner actually
  works when applied, and that streamed rows arrive incrementally.

## H6 — Release gate for 0.1.4
- [ ] **H6.1 Full green gate.** `cargo check --all-targets` · `cargo test --lib` · `cargo clippy
  --all-targets` 0 warnings · `svelte-check` 0 errors in our own files (BlackHole/three warnings are
  pre-existing and fine) · (dev closed) `npm run build`.
- [ ] **H6.2 Changelog + known-issues correction.** CHANGELOG still blames the Cloudflare WARP client
  for the 0.1.0–0.1.2 CPU bug; per [V0.1.3-BUGFIX-PLAN.md](V0.1.3-BUGFIX-PLAN.md) the dominant cause was
  evorift's own domain-watch loop (fixed). State that plainly.
- [ ] **H6.3 Build + sign the installer, merge to main.** `npm run tauri build` (never plain
  `cargo build` — see the tauri build gotcha), verify the updater signature, then merge
  `backend-rewrite-june` → `main`.

---

## Deliberately out of scope
- A second from-scratch backend rewrite. The layering (engine trait / sys / profile / rollback /
  preflight / autopilot / ipc-service split) is sound and re-deriving it would cost weeks for the same
  result. The defects above are behavioural, not architectural.
- `BlackHole.svelte` — developed in a separate session, do not touch (CLAUDE.md rule 1).
