# STATUS — updated: 2026-08-14 (overnight session)

## Where things stand right now

**v1's current build does not reliably provide internet when protection is turned on** —
2026-08-13's live-verification run cut ALL internet (not just target domains), Discord
never opened, and recovery only happened when the app was closed. Root cause is **not yet
determined** — `docs/HYPOTHESES-INTERNET-CUT.md` has a ranked candidate list (WARP
split-tunnel misfire on default Start is the top candidate, WinDivert filter/reinjection a
close second) and `scripts/capture-state.ps1` + `docs/DIAGNOSE-INTERNET-CUT.md` are ready
for the operator to run the actual capture. **This blocks V0 by explicit instruction** — no
v2 engine code starts until the cause is known. A remote test/recovery agent
(`src-tauri/src/testd/`, see `docs/REMOTE-TESTING.md`) now exists specifically so this kind
of failure can be diagnosed and recovered from without needing physical access to the
laptop, but has not been deployed/started.

v1 itself is otherwise a working, tested Windows DPI-bypass app (Rust/Tauri/Svelte) built
around a bundled `winws` sidecar process + optional WARP split-tunnel, with mature
rollback/preflight/manifest/profile modules and real test coverage (150/150 unit tests
pass as of this session, up from 78 — the new `testd` module added ~72). v2 is still fully
at the infrastructure stage: no v2 code exists (no `evocore`/`evosys`/`evoapp` crates),
discovery/doc-audit/migration-inventory/backlog all exist from the 2026-08-11 session, but
**V0 is deliberately unstarted** pending the diagnosis above.

## Done this session (overnight, 2026-08-14 — earlier 2026-08-11/12/13 history in git log)

- **P0-a landed** (forced admin elevation via manifest — accepted UX tradeoff, user
  confirmed). **P0-b and P0-c were found already implemented** when checked against actual
  code before writing anything — no change needed for either (`engine.rs`'s `bundle_dir()`
  was already EXE-relative; `clear_stale_windivert()` already handled stale WinDivert
  services more thoroughly than the net3 reference).
- Live-verification run executed by the user: **FAILED** — total internet cut on
  "protection on," Discord never opened, recovered only on app close. Logged in
  `docs/LIVE-VERIFICATION.md`'s run table.
- **Finished `src-tauri/src/testd/`** (a parallel session had started it; completed +
  independently verified this session — 150/150 tests pass, `cargo build --bin
  evorift-testd` actually links, not just check/clippy). A LAN-only, token-authed,
  sandboxed remote test/recovery agent with a deadman switch (no `/health` for 120s →
  kills evorift.exe/winws.exe, reuses `engine::clear_stale_windivert()`, resets DNS to
  DHCP). See `docs/REMOTE-TESTING.md`. Not deployed, not started, no firewall rule opened.
- **`docs/HYPOTHESES-INTERNET-CUT.md`** — evidence gathered by 2 independent Sonnet
  subagent passes, ranked by `architect` (Opus 5, brief-only). Top candidate: WARP
  split-tunnel silently engaging on default Start (`sync_warp()` fires whenever
  `app_modes` is empty, which is the default) — matches Discord-specific history from
  `net3/SOLUTION.md` and has a real close-triggers-teardown path via tray Quit. Close
  second: WinDivert filter/reinjection failure. Decision rule given for reading tomorrow's
  capture.
- **`docs/FILTER-AUDIT.md`** — confirmed evorift's Rust code never touches WinDivert
  directly (only builds CLI args for the external `winws.exe`/`goodbyedpi.exe` binaries),
  so "packet reinjection guaranteed" has no in-repo code to verify against — stated
  plainly. Found one real silent-degradation path (`build_master_filter` drops exclusions
  to catch-all on a signature-file read failure, zero error surfaced) and a ~7.5s
  worst-case window (from the code's own poll intervals) before stale-driver cleanup
  starts after `winws.exe` dies unexpectedly.
- Fixed this file's and `docs/LIVE-VERIFICATION.md`'s stale headers.

## Process note — parallel-session collision (2026-08-13/14)

A prior session left `src-tauri/src/testd/` half-built and `engine.rs` modified,
uncommitted, discovered mid-turn by a different session that hadn't been told about it.
No damage resulted (the work was completed and independently verified rather than
discarded), but it was pure luck that the two sessions' changes were compatible rather
than conflicting. **Going forward: one session working on this repo at a time.** If a
second session's work is found uncommitted, stop and surface it rather than silently
building on top of or around it.

## Broken / half-done / known issues

- **Internet-cut root cause is unresolved** — this blocks all of V0-V10, not just V1.1's
  capture-layer question. See `docs/HYPOTHESES-INTERNET-CUT.md` for what to check first.
- V1.1/K1 (capture layer): unblocked as a *design* question (in-process capture is
  permitted per rule 3b), but practically gated behind the internet-cut diagnosis above —
  don't start V1.1 work until that's resolved, the two are likely related.
- `testd`'s self-disclosed gaps (see `docs/REMOTE-TESTING.md`): `EvoriftSvc` isn't stopped
  by the deadman recovery, only `evorift.exe`/`winws.exe`; `/health` itself is unaudited
  per-request (volume reasons).
- v1 has known half-done areas from `docs/DISCOVERY.md`: bandwidth limiting (code-complete,
  UI hidden, not live-tested), per-app domain auto-detection (not stress-tested), community
  profile fetch (no hash pinning — `BACKLOG.md` P3.1).
- `BACKEND-HARDENING-PLAN.md` still sits untracked at repo root, superseded by
  `BACKEND-V2-PLAN.md` — not mine to remove, flagged so it isn't mistaken for live guidance.

## Next 3 steps (ready to paste)

1. Run `scripts/capture-state.ps1 -Label before` (elevated PowerShell), reproduce the
   internet-cut, capture `during` and `after`, then read `docs/HYPOTHESES-INTERNET-CUT.md`'s
   decision rule against the results — start with `01-network-config.txt` (WARP tunnel
   adapter present or not) as the gate between the top two candidates.
2. Once the cause is confirmed, decide the fix as its own task — not started, not guessed
   at in any doc this session.
3. Only after that: resume `BACKLOG.md` V0.1 (Cargo workspace split) — deliberately not
   started this session per explicit instruction, even though it doesn't technically depend
   on the internet-cut cause.

## Open questions (for the user)

- See `QUESTIONS.md` — Q1 is answered; no other open items.
- Which close method was used during the 2026-08-13 failing run (tray Quit / window X /
  Task Manager)? Materially changes how strongly the top-ranked hypothesis is supported —
  see `docs/HYPOTHESES-INTERNET-CUT.md`.
