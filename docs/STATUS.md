# STATUS — updated: 2026-08-16 (engine rewrite, v0.2.0)

## Where things stand right now

**The engine was rewritten after a user report that inverted the two protection modes**: with
"Güçlü Koruma" on, an ordinary HTTPS site would not open at all; switching DOWN to "Hafif" opened
it instantly. That is protection performing worse than no protection, and it was structural, not a
mistuned constant.

Cause: Güçlü ran ONE route-dependent forgery — the `turkcell-hotspot` preset (`fake` + `ttl=1` +
`autottl=3`, **no fooling**) — with `hostlist_only = false`, i.e. catch-all across every TLS/443
flow on the machine. A TTL-limited forged ClientHello with no fooling option only works where the
DPI sits at the assumed hop distance; anywhere else it outlives the DPI, reaches the real server,
and the server tears the connection down. Hafif "worked" only because its hostlist never contained
that domain, so nothing touched it. The preset had been measured on exactly one line, and the
source comment recording that measurement said so.

### What changed (v0.2.0)

- **Layered modes.** Both modes now run an aggressive chain GATED to a hostlist, plus a catch-all
  layer that is provably incapable of corrupting a connection (`Strategy::is_harmless`: a forged
  packet is safe only when a fooling option guarantees the real server discards it). A unit test
  fails the build if any mode ever runs a harmful chain catch-all.
- **Measurement instead of a constant** (`src-tauri/src/tuner.rs`). A candidate ladder ordered
  least-invasive-first — beginning with `off`, i.e. doing nothing — is raced on the actual line
  against blocked targets AND a control set. Any candidate that breaks a control site is rejected
  outright. If nothing wins cleanly the verdict is "touch no packets", which on a DNS-only block is
  the correct answer. Result persisted per network.
- **Verification stopped grading itself on the wrong exam.** The probe tested three Discord
  hostnames only, so Güçlü could report `verified` while everything else was dead. It now probes
  the mode's real targets plus control sites, in one parallel batch (measured: 6 hosts, 422 ms),
  and returns per-site results to the UI.
- **Do-no-harm self-healing.** Harm detected → discard the tuning, fall back to a harmless
  configuration, re-probe. Targets merely blocked → re-measure the line. Both rate-limited, and
  persistent harm STOPS protection rather than restart-looping.
- **Reboot restore.** State is persisted (`%PROGRAMDATA%\evorift\state.json`) and restored behind
  a real user setting, after waiting for actual connectivity. The UI is launched by a scheduled
  task with `-RunLevel Highest` — the old Startup-folder shortcut could never work, because Windows
  will not elevate a `requireAdministrator` binary from that path.
- **Errors are visible.** Structured event log (`elog.rs`) → ring buffer + `engine.log` + a
  Rust→UI event channel + mirroring to `Desktop\evorift-logs`. `winws`'s own stdout/stderr is
  captured for the first time (it was inherited into a Session-0 service with no console).
- **Speed.** `clear_stale_windivert()` and `taskkill` ran on every start as insurance; both are now
  failure-path only behind a sub-millisecond process check. The fixed 700 ms post-spawn sleep is a
  25 ms poll, DNS application runs concurrently with engine start, and DNS state is read via
  `GetAdaptersAddresses` (measured 6.6 ms including lazy init) instead of PowerShell.

### Verified so far

Static + unit: 184/184 tests, `cargo clippy --all-targets` clean, `svelte-check` clean apart from
the known pre-existing `three` declaration error in `BlackHole.svelte`. Live: the new probe was run
against real endpoints from the dev machine (all 6 hosts open, harm=false, 422 ms).

**NOT verified on the test laptop — deferred by the user, 2026-08-16.** The remote pass needs the
testd bearer token; asked for it, and the user chose "just give me the installer for now". So the
central claim — "Güçlü no longer breaks working sites" — rests on code and unit tests, NOT on a
measurement taken on the line where the failure happened. Treat it as unproven until that pass runs.
Scripts are written and staged (`build-020/install-0.2.0.ps1`, `build-020/verify-0.2.0.ps1`).

**Confirmed by the user: the failing test was on the LAPTOP (192.168.1.20)** — the same machine and
line where the previous session measured this exact Güçlü configuration at 400/400 the day before.
Same preset, same line, opposite result one day apart. That is the strongest available evidence
that the route to the DPI changed underneath a hardcoded TTL chain, and therefore that per-line
measurement (not a better constant) is the right fix.

## Broken / half-done / known issues

- **The v0.2.0 engine has not been live-verified on the test laptop.** That is the single most
  important gap: everything above is code-and-unit-test evidence. The remote pass needs the testd
  bearer token (`testd.token`, ACL'd to SYSTEM+Administrators on the laptop, transferred by USB).
  Scripts are written and ready: `install-0.2.0.ps1` + `verify-0.2.0.ps1` (see the session
  scratchpad) — the verification measures control sites with protection OFF and again in Güçlü, and
  FAILS if the Güçlü number is lower. That comparison is the whole point; a status read would not
  have caught the original bug.
- **VPN (WARP full tunnel) is still not working and is now labelled "coming soon"** in the UI and
  made non-interactive. It was live and clickable while its own confirm dialog admitted it had
  never had a passing live run.
- **The wide hostlist is a shipped guess.** `WIDE_HOSTLIST` covers Discord/Roblox/YouTube plus the
  domains commonly blocked on Turkish lines. Sites outside it get only the harmless catch-all layer,
  which may not be enough for a hard block. The user-facing answer is the "add a site" input
  (`SetExtraDomains`), which now actually takes effect — the old `SetHostlist` called `start()` on a
  live child, which is idempotent, so the list was accepted and silently ignored.
- **Network fingerprinting for tuning is coarse** (`tuner::network_key` = the /24 of the outbound
  address). Two different ISPs both handing out 192.168.1.x look identical. A wrong key costs one
  unnecessary re-measure, and the engine re-measures on verification failure anyway.
- **`autopilot.rs` still exists alongside `tuner.rs`.** Autopilot is the older UI-facing score table;
  it has no control set and therefore cannot detect harm. Modes no longer use it. Merging the two is
  outstanding work — two systems that answer the same question is exactly the kind of thing that
  makes a codebase feel unstable.
- Older half-done areas remain from `docs/DISCOVERY.md`: bandwidth limiting (code-complete, UI
  locked), per-app domain auto-detection (not stress-tested), community profile fetch (no hash
  pinning — `BACKLOG.md` P3.1).
- `BACKEND-HARDENING-PLAN.md` still sits untracked at repo root, superseded by
  `BACKEND-V2-PLAN.md` — flagged so it isn't mistaken for live guidance.

## Next 3 steps

1. **Run the remote verification pass** against v0.2.0 on the laptop (needs the testd token). The
   pass fails the build if Güçlü opens fewer control sites than protection-off — that is the
   regression that started this work, and nothing else proves it is gone.
2. If Güçlü still does not open the hard domains on that line, run `evorift-ctl tune` and read the
   score table: the ladder now records, per candidate, how many targets opened and how many control
   sites broke. That table is the evidence for whatever is chosen next — no more single-line presets
   promoted to product defaults.
3. Merge `autopilot.rs` into `tuner.rs` so there is one measurement path with one definition of
   "working", and delete the older one.

## Open questions (for the user)

- The testd bearer token for the test laptop (needed for step 1 above).
