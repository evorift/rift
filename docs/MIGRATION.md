# MIGRATION.md — v1 → v2 Module Migration Inventory

Feeds the V0 phase of [BACKEND-V2-PLAN.md](../BACKEND-V2-PLAN.md). Source evidence:
[DISCOVERY.md](DISCOVERY.md) (repo-scout) + docs-auditor re-verification (2026-08-11).

**Correction (2026-08-12, see [FORENSICS.md](FORENSICS.md) A2):** the 2026-08-11
docs-auditor pass claimed to have "independently re-verified" the line-count citations
below and reported them all matching. It hadn't — it reproduced the same off-by-one error
present in the original citations instead of catching it (every whole-file `path:1-N`
citation was 1 line short of the file's real end). All line-range citations in this
document have since been recounted directly with `awk 'END{print NR}'` against the current
files, not copied from the prior report. **Methodology going forward: a verification pass
must recompute from the source file itself, not re-read the number it's checking.**

No code is moved here — this is the inventory only.

Effort scale: **S** = drop-in with a path/config change, half a session or less.
**M** = needs real adaptation to the new contracts (V0.1-V0.4), roughly one session.
**L** = needs a design decision or significant rework before it can move, multi-session.

## Inventory

### 1. Rollback journal

- **Source:** `src-tauri/src/rollback.rs:1-205` (recounted 2026-08-12, `awk 'END{print NR}'`)
- **Target crate:** `evosys` (the `Change` enum is candidate shared data — could move to
  `evocore` if V0.2's error/result types want to reference it directly; decide when V0.2 lands).
- **What it is today:** `Change` enum (7 variants: ServiceCreated, FirewallRule,
  RegistryWrite, DnsChanged, TunnelInstalled, FileCopied, ScheduledTask). `RollbackLog`:
  in-memory `Vec` + JSON persisted at `%PROGRAMDATA%\evorift\rollback.json` behind a
  global `Mutex`. `rollback_all()` reverses in reverse order, best-effort. Has tests.
- **Contract changes needed:** repoint the data-dir path through V0.3's injectable
  environment trait (currently reads `%PROGRAMDATA%`/`%LOCALAPPDATA%` directly — needs to
  go through the same env abstraction V0.3 introduces, or the "bundle missing"/"no
  permission" test scenarios V0.3 promises can't be set up around it). Global `Mutex` should
  be reviewed against V2.3's lock-free hot-swap discipline if anything in the strategy path
  ever touches it.
- **Effort:** **S** — logic is already decoupled and tested; only the environment wiring changes.

### 2. Preflight checks

- **Source:** `src-tauri/src/preflight.rs:1-314` (recounted 2026-08-12, `awk 'END{print NR}'`)
- **Target crate:** `evosys` for the 10 OS-level checks (admin, winws bundle, WinDivert
  conflict, ByeDPI, WARP, DoH, AV/WinDivert block, VC++ runtime, Packet Filter NDIS, WARP
  API reachability). The `diagnose()` function (DNS→TCP→TLS chain) overlaps conceptually
  with V4.2's canary probe — **flag for a design decision**: either keep `diagnose()` in
  `evosys` as a preflight-only concern and let V4.2 build a separate probe, or factor the
  DNS/TCP/TLS chain logic into `evocore` (behind V0.3's injectable network layer) so both
  preflight and the canary probe share one implementation. Don't duplicate the logic silently.
- **What it is today:** `PreflightResult` with typed results per check; `diagnose()` runs a
  target-specific connectivity chain. Has tests.
- **Contract changes needed:** the engine-catalog path it queries (`make_engine("zapret")`,
  `make_engine("byedpi")`) needs to point at the new bundle layout once V8.5's manifest
  covers the v2 crates' resources.
- **Effort:** **M** — mostly portable, but the diagnose/canary-probe overlap needs a
  decision before it moves cleanly (see above).

### 3. Manifest / SHA verification

- **Source:** `src-tauri/src/manifest.rs:1-164` (recounted 2026-08-12, `awk 'END{print NR}'`)
- **Target crate:** `evocore` — pure Rust (`sha2` crate), no Windows API dependency, fits
  V0.1's rule that `evocore` has no `windows` crate dependency.
- **What it is today:** `BinaryEntry{name, version, sha256, source, path}`. `sha256_file()`
  + `verify_all()` load `resources/manifest.json` and hash bundled binaries in place. Has tests.
- **Contract changes needed:** wire through V0.3's injectable filesystem trait so
  "bundle missing"/"tampered binary" scenarios are unit-testable without a real bundle on
  disk. Generalize the manifest path/schema to the `evocore`/`evosys`/`evoapp` bundle
  layout referenced by V8.5.
- **Effort:** **S** — pure logic, straightforward port once V0.3 lands.

### 4. Svelte UI skeleton

- **Source:** `src/` (routes: dashboard/connection/performance/apps/limits(hidden)/logs/
  settings/advanced/control; components: NavRail, BlackHole (off-limits), TitleBar,
  Toaster, Onboarding, BinaryRain, per-section components; state: `src/lib/state.svelte.ts`, ~840 lines)
- **Target crate:** N/A — this isn't a Rust crate; it's the frontend `evoapp` talks to over
  Tauri IPC. Stays in `src/`, not migrated into the workspace.
- **What it is today:** SvelteKit v2.9 + Svelte 5 runes-based state, localStorage-persisted preferences.
- **Contract changes needed:** none to the UI code itself for V0-V1; every IPC call site
  needs repointing once `evoapp`'s command surface stabilizes (see item 5). Do not regress
  the runes-based store to the older Svelte stores API.
- **Effort:** **S** for the skeleton itself (copy as-is); the real cost is downstream, in
  keeping IPC call sites in sync with `evoapp` as it's built — track that against item 5, not here.

### 5. Tauri IPC setup

- **Source:** `src-tauri/src/ipc.rs:1-675` (recounted 2026-08-12, `awk 'END{print NR}'`),
  `src-tauri/src/lib.rs:1-1086` (recounted 2026-08-12; ~900 lines was an earlier estimate,
  the real file is larger)
- **Target crate:** `evoapp` (command registration, Tauri bridge). Validation logic
  (`ipc::validate`, `ipc::validate_request`) is a candidate for `evocore` if it can be made
  pure (no Windows API) — worth attempting since V2.2's rule model already needs similar
  validation discipline.
- **What it is today:** `Request` enum (Hello/Command/Subscribe/AutoPilotStream); `Command`
  enum with 24 ops (start, stop, status, set_strategy, set_dns, block_app, repair, set_tweak,
  set_limit, set_hostlist, set_app_modes, set_full_warp, set_engine, engine_catalog,
  list/save/delete/export/apply profile, reset_dns, verify_dns, preflight, diagnose,
  autopilot, rollback_all, tunnel_status, health). Transport: named pipe
  `\\.\pipe\evorift-ipc.sock`, line-delimited JSON. A test enforces that
  `FRONTEND-CONTRACT.md` documents every command.
- **Contract changes needed:** **this is the highest-leverage item in the whole inventory** —
  not a copy-paste. Recommend keeping the named-pipe+JSON transport for v1.x↔v2
  compatibility and adding a `protocol_version` field to `Request`/`Response` for the
  future. `evoapp` should publish an updated `FRONTEND-CONTRACT.md`-equivalent (or the
  same file) at build time so the existing contract test keeps its anchor.
- **Effort:** **L** — command surface and transport shape can carry over, but the
  implementation is rebuilt against the new crate boundaries, and the validate-logic split
  (evocore vs evoapp) is a real design call.

### 6. Domain / target lists

- **Source:** `src-tauri/resources/winws/hostlist-discord-roblox.txt` (12 lines, 10
  domains); `src-tauri/src/profile.rs:234-289` `seed_defaults()` (range re-checked
  2026-08-12, unchanged; 3 seeded profiles:
  discord-voice, youtube-fast, everything)
- **Target crate:** `evocore` for the data model (domain list as versioned config); the
  actual file lives in the data dir managed by `evosys`/`evoapp`.
- **What it is today:** flat text file + hardcoded lists baked into seeded profiles, capped
  at 500 domains per profile.
- **Contract changes needed:** externalize to a data-dir JSON/TOML file, version it so it
  can hot-update without a binary rebuild, and reuse (not duplicate) the existing
  `fetch_community()` bundle-download path in `profile.rs:216-231` — note DISCOVERY.md
  flagged that path has **no hash pinning** on the downloaded bundle today; fix that as
  part of this migration, not as a separate follow-up (`evorift-security-hygiene` applies).
- **Effort:** **M** — the data itself is trivial to move; the versioning/hot-update design
  and closing the hash-pinning gap are real work.

### 7. Profile format

- **Source:** `src-tauri/src/profile.rs:1-390` (recounted 2026-08-12, `awk 'END{print NR}'`)
- **Target crate:** `evocore` — directly implements V0.4 (config schema and migration).
- **What it is today:** `Profile{schema_version=1, id, name, engine, isp, scope, dns,
  strategy, hostlist≤500, engine_params:JSON}`. Persisted at
  `%PROGRAMDATA%\evorift\profiles\<id>.json`. serde-backed backward compatibility for
  missing fields. Load/import/export/delete/apply via IPC commands. Has tests (schema
  roundtrip, back-compat, bad-id/engine/hostlist rejection).
- **Contract changes needed:** repoint the data-dir path via V0.3's injectable
  environment; add a one-time migration pass that copies existing v1 profiles into the v2
  location on first v2 boot (V0.4 explicitly requires "v1 config loads successfully" as
  its test — this module is most of that test's implementation).
- **Effort:** **S** — schema is mature and versioned; this is close to a direct port plus
  the migration-pass wrapper.

### 8. Native network/process enumeration (netinfo.rs)

_Added 2026-08-12 — omitted from the original inventory; flagged as a likely gap in
[FORENSICS.md](FORENSICS.md) A3. Feeds plan item V1.3 (PID mapping) directly._

- **Source:** `src-tauri/src/netinfo.rs:1-315` (verified `awk 'END{print NR}'`)
- **Target crate:** `evosys` — Win32 API calls (`GetExtendedTcpTable`/`GetExtendedUdpTable`,
  `QueryFullProcessImageNameW`), not portable to `evocore` as-is.
- **What it is today:** per its own module doc-comment (`netinfo.rs:1-6`): native Win32
  socket/process enumeration replacing PowerShell/`tasklist` subprocess calls. Stated reason
  (quoted): "Eski yol her tarama için `powershell.exe` (+ `conhost.exe`) çağırıyordu. Her
  çağrı tüm .NET CLR'ını yükler (1-3 sn CPU) → döngüde çalışınca yüzlerce kısa-ömürlü süreç
  yığılır → %100 CPU + AV/EDR malware sezgisi." (roughly: the old path spawned
  `powershell.exe` per scan, loading the full .NET CLR each time — looped, this piled up
  hundreds of short-lived processes → 100% CPU + AV/EDR heuristics flagging it). Exports:
  `sockets()` (`netinfo.rs:40`), `pid_exe_path()` (`netinfo.rs:182`), `socket_pids()`
  (`netinfo.rs:212`), `reverse_dns()` (`netinfo.rs:235`). No `#[test]` functions found
  (0 tests, mechanically counted) — this is proven-in-production code (per the squash
  commit `51e925d`'s message: "fixes the 0.1.0-0.1.2 process-pileup / 100% CPU bug"), not
  proven-by-test-suite code; treat accordingly during migration.
- **Contract changes needed:** wire through V0.3's injectable environment for testability
  (currently 0 tests — this is the item's main gap, not its portability). Otherwise a
  near-direct port; it's already pure Win32 API usage, no subprocess spawning, no external
  bundle dependency.
- **Effort:** **S** — small adaptation, but note the 0-test gap should be closed as part of
  the move, not carried forward silently.

### 9. WARP split-tunnel (warp.rs)

_Added 2026-08-12 — omitted from the original inventory; flagged as a likely gap in
[FORENSICS.md](FORENSICS.md) A3. Feeds plan item V6.1-V6.6 (Tunnel layer) directly._

- **Source:** `src-tauri/src/warp.rs:1-620` (verified `awk 'END{print NR}'`)
- **Target crate:** `evosys` (WireGuard/Windows process/service management), with the
  `ALLOWED_IPS`/`FULL_ALLOWED_IPS`/`WARP_DNS`/`WARP_ENDPOINT` constants (`warp.rs:23-40`)
  as candidate `evocore` config data.
- **What it is today:** per its own module doc-comment (`warp.rs:1-8`): a split-tunnel
  WireGuard implementation routing only Discord's IP ranges (`ALLOWED_IPS =
  "162.159.0.0/16, 66.22.0.0/16, 104.29.0.0/16"`, `warp.rs:23`) through Cloudflare WARP.
  The module's own stated reason for existing (quoted, `warp.rs:3-4`): "Discord MASAÜSTÜ
  (Electron) istemcisi agresif-DPI hatlarında 'Starting…'de takılır (QUIC'i tercih eder —
  ISS bunu ICMP-unreachable ile öldürür — ve desync'in bozduğu büyük JS paketlerini çeker)."
  **Note — this is a different causal explanation than the one in
  [net3/SOLUTION.md §3.3](FORENSICS.md#b6--net3solutionmd-33-primary-source)** (which
  describes a gateway-WebSocket zstd-`READY`-payload inspection issue, not QUIC+
  ICMP-unreachable). Both documents agree WARP split-tunnel is the fix and both agree
  handshake-level desync alone doesn't work for desktop Discord; the two explanations of
  *why* aren't identical. Not reconciled here — flagging only, per this item's own
  "quotes and don't interpret" discipline; a further forensics pass would be needed to
  reconcile them if that distinction matters to a decision.
- **Contract changes needed:** significant — bundle/process lifecycle (`wgcf.exe`,
  `wireguard.exe`, `wintun.dll` under `<exe_dir>\warp\`) needs the same injectable-resource
  treatment as the manifest/preflight items. Fail-safe behavior ("Bundle yoksa (dev) her
  metot loglanan bir no-op'a iner" — no bundle → every method degrades to a logged no-op)
  is a design pattern worth carrying forward deliberately, not losing in the port.
- **Effort:** **L** — largest of the three added items; real design work on process/tunnel
  lifecycle management, not a mechanical port.

### 10. Autopilot / strategy auto-finder (autopilot.rs)

_Added 2026-08-12 — omitted from the original inventory; flagged as a likely gap in
[FORENSICS.md](FORENSICS.md) A3. Feeds plan item V5.1-V5.4 (Autopilot) directly._

- **Source:** `src-tauri/src/autopilot.rs:1-446` (verified `awk 'END{print NR}'`)
- **Target crate:** `evocore` for the scoring/candidate logic (`Candidate`, `ScoreRow`
  structs, `candidate_matrix()`, `prioritize_for_isp()` — all pure data/logic per a quick
  read of the exports); `evosys` for the actual engine start/stop/test cycle
  (`run_once_and_score()`, `autopilot.rs:253`) since that needs real process control.
- **What it is today:** per its own module doc-comment (`autopilot.rs:1-3`): tests every
  available engine/strategy candidate against target sites, scores them, proposes the best
  profile. `isp_preset_id()` (`autopilot.rs:89-104`) does the lowercase ISP-name matching
  already flagged as brittle in `docs/DISCOVERY.md`. Stated operational constraint
  (quoted, `autopilot.rs:5-7`): "winws/GoodbyeDPI WinDivert'i SİSTEM GENELİ tutar → aynı
  anda yalnız TEK desync motoru çalışmalı. Bu yüzden çağıran (service.rs AutoPilot kolu)
  önce AKTİF motoru duraklatır..." (winws/GoodbyeDPI hold WinDivert system-wide → only one
  desync engine can run at a time; the caller must pause the active engine first). This is
  directly relevant to K1/K2 — any future in-process engine has the same single-engine
  constraint.
- **Contract changes needed:** the candidate/scoring logic (pure) and the run/pause/resume
  orchestration (needs real engine control) should be split cleanly across `evocore`/
  `evosys` rather than ported as one unit — the plan's V5.1 ("candidate generation") and
  V5.2 ("streaming results") already imply this split.
- **Effort:** **M** — the scoring logic is fairly portable; the pause-active-engine
  orchestration needs to be redesigned against whatever K1 lands on.

## Summary by target crate

| Crate | Items |
|---|---|
| `evocore` | Manifest/SHA verification (3), domain/target list data model (6), profile format (7), possibly IPC validation logic (5, split candidate), autopilot scoring logic (10, split candidate) |
| `evosys` | Rollback journal (1), preflight checks (2), native net/process enumeration (8), WARP split-tunnel (9), autopilot engine orchestration (10, split candidate) |
| `evoapp` | Tauri IPC setup (5) |
| Not a crate (frontend) | Svelte UI skeleton (4) |

## Effort summary

| Effort | Items |
|---|---|
| S | Rollback journal (1), Manifest/SHA verification (3), Svelte UI skeleton (4, copy cost only), Profile format (7), native net/process enumeration (8) |
| M | Preflight checks (2), Domain/target lists (6), Autopilot (10) |
| L | Tauri IPC setup (5), WARP split-tunnel (9) |

## Coverage note (2026-08-12)

This inventory now covers 10 of `src-tauri/src`'s 30 modules. It is **still not
exhaustive** — see [FORENSICS.md](FORENSICS.md) A3 for the full line/test count of every
module and which remain omitted (`engine.rs`, `service.rs`, `goodbyedpi.rs`, `byedpi.rs`,
`repair.rs`, `proxifyre.rs`, `sys.rs`, and everything under 200 lines). Those omissions are
believed intentional (tied to the K1-blocked engine redesign, or genuinely small/
not-yet-triaged utility modules) rather than missed gaps, but that belief hasn't been
checked item-by-item the way items 8-10 were — treat it as a lower-confidence claim than
the rest of this document.

## Note on V1.1 (capture layer)

This inventory only covers what **moves from v1**. The capture layer itself (V1.1) is not
a migration item — v1 has no in-process capture code to move (confirmed in DISCOVERY.md:
`engine/real.rs` was already removed). V1.1 is new-build work, currently BLOCKED pending
the K1 decision (see QUESTIONS.md Q1 and BACKLOG.md's DECISIONS section). The existing
`engine.rs`/`service.rs` winws-spawn code (confirmed at `engine.rs:712-718`,
`service.rs:216-231`) is relevant **reference material** for whatever external-process
control-channel design K1 lands on, even though it isn't migrated as-is.
