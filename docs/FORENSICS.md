# FORENSICS.md — Session Verification + Rule 3 Origin

_Forensics-only pass, 2026-08-12. No code written, no changes to BACKLOG.md,
BACKEND-V2-PLAN.md, or CLAUDE.md. Evidence is git history and file contents only — where
the repo doesn't say, this file says UNPROVEN rather than inferring a plausible story._

---

## PART A — verifying yesterday's session

### A1. Persistence check

`git log --stat -8` (see raw output captured during this session) confirms 5 commits from
yesterday's session (`5b0a93b`…`44d5f65`) plus the pre-existing `51e925d` squash commit and
older history. `.gitignore` line 29 is a bare `docs/` entry — the entire `docs/` directory
is gitignored by design (this predates yesterday's session; it's project convention, not a
mistake).

| File | Status |
|---|---|
| `docs/DISCOVERY.md` | **GITIGNORED** (`.gitignore:29`) — present on disk, never committed, by design |
| `docs/STATUS.md` | **GITIGNORED** — same |
| `docs/MIGRATION.md` | **GITIGNORED** — same |
| `docs/LIVE-VERIFICATION.md` | **GITIGNORED** — same |
| `BACKLOG.md` | **COMMITTED** (`44d5f65`) |
| `QUESTIONS.md` | **COMMITTED** (`1a60fe5`, updated `b8d7728`) |

**`git clean -fdx` would destroy all 4 `docs/*.md` files above** — they exist only on this
machine's working tree, nowhere else. This is consistent with the rest of `docs/` (all
15+ other files there are equally gitignored, per project convention set at repo
scaffolding), so it isn't a new risk introduced yesterday — but it is a real one: if this
machine is lost or the working tree is cleaned, `docs/DISCOVERY.md`, `docs/MIGRATION.md`,
`docs/STATUS.md`, and `docs/LIVE-VERIFICATION.md` are gone with no git recovery path.

### A2. Citation spot-check (10 picked: 5 from DISCOVERY.md, 5 from MIGRATION.md)

| # | Source | Claimed reference | What's actually there | Verdict |
|---|---|---|---|---|
| 1 | DISCOVERY | `rollback.rs:1-206` | File's real last line (via `awk 'END{print NR}'`, trailing-newline-safe) is **205**, not 206 | **MISMATCH** (off by 1) |
| 2 | DISCOVERY | `warp.rs:15` | Line 15: `/// net3/SOLUTION.md (2026-06-10, Türkcell — uçtan uca CANLI DOĞRULANDI...` — exact content claimed | MATCH |
| 3 | DISCOVERY | `autopilot.rs:89-100` | Lines 89-100 are exactly the `isp_preset_id()` function with the lowercase ISP-name matching described | MATCH |
| 4 | DISCOVERY | `profile.rs:216-231` | Lines 216-231 are exactly the `fetch_community()` function, bounds precise | MATCH |
| 5 | DISCOVERY | `engine.rs:5-15` | Lines 5-12 are the architecture-layer doc list; the actual "old engine removed" claim is specifically at lines 13-15, inside the cited range but not evenly distributed across it | MATCH (imprecise but the claim is within range) |
| 6 | MIGRATION | `preflight.rs:1-315` | Real last line: **314** | **MISMATCH** (off by 1) |
| 7 | MIGRATION | `manifest.rs:1-165` | Real last line: **164** | **MISMATCH** (off by 1) |
| 8 | MIGRATION | `ipc.rs:1-676` | Real last line: **675** | **MISMATCH** (off by 1) |
| 9 | MIGRATION | `profile.rs:1-391` | Real last line: **390** | **MISMATCH** (off by 1) |
| 10 | MIGRATION | `engine.rs:712-718` + `service.rs:216-231` (cited together for "external process, forced restart") | `engine.rs:713-718` is exactly `std::process::Command::new(&exe).args(...).spawn()`; `service.rs:216-232` is exactly the `SetStrategy` handler with the "Force a restart..." comment at 220-221 | MATCH |

**Score: 5/10 MATCH, 5/10 MISMATCH, 0/10 FILE MISSING.** All 5 mismatches are the *same*
narrow error class: every whole-file `"path:1-N"` citation is off by exactly +1 versus the
file's real last line (confirmed independently via `awk 'END{print NR}'`, not just `wc -l`,
and all 5 files end with a trailing newline so that's not the cause either). Every
mid-file, precisely-bounded citation (the ones that actually matter for engineering
decisions — the process-spawn, the restart-comment, the ISP matcher, the fetch function)
was accurate. **Root cause of the off-by-one is UNPROVEN** — I can confirm the pattern
exists and is consistent, not which counting method produced it.

One additional, unprompted finding while re-checking: the earlier same-session
`docs-auditor` pass (see `QUESTIONS.md`/commit history) claimed to have "independently
re-verified" these exact same EOF line counts and reported them as matching. It reproduced
the same off-by-one instead of catching it — its "independent" check wasn't actually
independent of whatever produced the original number.

### A3. Migration inventory sanity

`src-tauri/src/*.rs` has **30 files** (29 modules + `main.rs`), **78 `#[test]` functions**
total (mechanically counted via `grep -rc '#\[test\]'`, matches the 78 claimed). `docs/MIGRATION.md`
covers **5 of those 30 files** as standalone modules (`rollback.rs`, `preflight.rs`,
`manifest.rs`, `ipc.rs`, `profile.rs`) plus 2 non-module items (a hostlist text file, the
Svelte UI tree) — 7 items total, matching what was asked of it, but far from all 30.

| Module | Lines | Tests | Status |
|---|---|---|---|
| rollback.rs | 205 | 3 | IN-INVENTORY |
| preflight.rs | 314 | 2 | IN-INVENTORY |
| manifest.rs | 164 | 4 | IN-INVENTORY |
| ipc.rs | 675 | 8 | IN-INVENTORY |
| profile.rs | 390 | 6 | IN-INVENTORY |
| lib.rs | 1086 | 0 | OMITTED (as a standalone item — but its `#[tauri::command]` surface is discussed as part of the `ipc.rs` inventory item, not a clean omission) |
| **engine.rs** | **938** | 9 | **OMITTED** — this is v1's core DPI engine (`WinwsEngine`, `SimEngine`, factory). Not a carry-over candidate: it's the thing V2.x is explicitly rewriting (strategy-as-runtime-data), so "moves from v1" doesn't apply to it the way it does to rollback/preflight/manifest. |
| **service.rs** | **996** | 2 | **OMITTED** — service-side dispatch/orchestration, directly tied to the now-BLOCKED V1.1/K1 decision (its `SetStrategy` restart logic is the exact thing K1 has to replace). Its fate isn't decidable until K1 resolves, so it wasn't listed as a clean migration candidate. |
| **warp.rs** | **620** | 8 | **OMITTED, and this looks like a real gap** — this is the WARP/WireGuard tunnel implementation, directly relevant to plan phase V6 ("Tunnel abstraction"). Nothing in `docs/MIGRATION.md` covers it, and nothing in `BACKEND-V2-PLAN.md`'s "what moves from v1" list mentions it either — that list was scoped to 6 specific items the plan named, and this wasn't one of them, but V6.1-V6.6 in `BACKLOG.md` would clearly benefit from a migration entry for this file. |
| **goodbyedpi.rs** | **428** | 8 | OMITTED — alternate engine adapter, same reasoning as `engine.rs` (tied to the engine/strategy redesign, not a simple carry-over). |
| **autopilot.rs** | **446** | 7 | **OMITTED, likely gap** — strategy auto-finder, directly relevant to plan phase V5 ("Autopilot"). Same situation as `warp.rs`: not in the plan's "what moves" list, but relevant to a later plan phase and not flagged anywhere as a migration candidate. |
| **byedpi.rs** | **350** | 3 | OMITTED — same reasoning as `goodbyedpi.rs`. |
| **netinfo.rs** | **315** | 0 | **OMITTED, likely gap** — native Win32 PID/socket enumeration; per the squash commit message (`51e925d`) this file is what "fixes the 0.1.0-0.1.2 process-pileup / 100% CPU bug." Directly relevant to plan item V1.3 ("PID mapping"), and arguably the single highest-value omission — it's proven-working code (fixed a real shipped bug) that the plan's own V1.3 will need equivalent logic for. |
| **repair.rs** | 295 | 2 | OMITTED — network repair wizard; loosely relevant to V8, not named. |
| **proxifyre.rs** | 266 | 4 | OMITTED — routing adapter used by `byedpi.rs`; same reasoning as the other engine adapters. |
| **sys.rs** | 206 | 1 | OMITTED — general OS utility grab-bag; likely portable, not called out specifically. |

**Every other module is under 200 lines** (`client.rs`, `dns.rs`, `drover.rs`, `firewall.rs`,
`limit.rs`, `logbundle.rs`, `main.rs`, `pid_scan.rs`, `proc.rs`, `schtask.rs`, `services.rs`,
`svcctl.rs`, `tweak.rs`, `wiresock.rs`, both `bin/` files) — omitted from the per-item
reasoning above per the task's 200-line threshold, but listed here for completeness: none
of them appear in `docs/MIGRATION.md` either.

**Honest assessment:** `docs/MIGRATION.md` is accurate for what it claims to cover (the 6
items `BACKEND-V2-PLAN.md`'s "what moves from v1" section explicitly named), but it is
**not a complete migration inventory** of the codebase, and it was never presented as one —
the gap is real. Three omissions in particular (`warp.rs`, `autopilot.rs`, `netinfo.rs`)
look like they should have been included given they map directly onto named future plan
phases (V6, V5, V1.3 respectively) that `BACKLOG.md` already references. This wasn't
caught during yesterday's session.

### A4. Backlog quality (3 items, picked across different plan sections)

| Item | Acceptance criterion (verbatim) | Verdict |
|---|---|---|
| V0.2 | "apply is `Err` in an unprivileged environment; reaching `Active` is impossible." | **CHECKABLE, with a caveat** — the first clause is a concrete test (mock unprivileged env, assert `Err`). The second clause ("impossible") is a stronger universal claim; a single test can show one path can't reach `Active`, but "impossible" as stated isn't something one command decides pass/fail on exhaustively. |
| V3.3 | "a synthetic target that fails the first N attempts reaches success via escalation and stays on that chain." | CHECKABLE — concrete scenario, deterministic pass/fail. |
| V8.4 | "a structure containing a secret gets masked when handed to the logger." | CHECKABLE — concrete scenario, deterministic pass/fail. |

**2/3 cleanly CHECKABLE, 1/3 CHECKABLE-with-caveat** (not VAGUE — it's actionable, just
broader than a single test can fully close out).

---

## PART B — why the in-process WinDivert engine was removed

### B1. Locate the removal

**There is no removal commit in this repository's git history.** Exhaustive check across
all 52 commits in `git rev-list --all` (`git ls-tree -r <sha> --name-only` per commit,
grepped for `engine/` or `real.rs`) found **zero** commits, ever, containing a file at
`engine/real.rs` or any `engine/` subdirectory. `git log -S windivert -i --all` and
`git log -S WinDivert --all --stat` likewise return no historical removal — only this
session's docs commits and 4 earlier ones (`a006361`, `2e883a9`, `b7f4e93`, `37316c1`),
none of which delete a WinDivert engine.

The first commit that mentions the removal is `2e883a9` ("v0.1.0: dark green installer,
i18n brand-neutral, language selector, NSIS hooks", 2026-06-08 per repo dating), which
*adds* `src-tauri/src/engine.rs` from scratch, already containing `WinwsEngine` and a
doc-comment describing something already gone:

```
+//! NOT: Eski saf-Rust WinDivert motoru (`engine/real.rs` + tls/packet/quic/voice byte-cerrahisi) net3'e
+//! geçişle KALDIRILDI — winws QUIC/gateway desync'ini gerçek yapıyor, eski motor masaüstü Discord'u
+//! açamıyordu (bkz. net3/SOLUTION.md §3.3). Discord'un kendisi artık WARP split-tunnel ile (warp.rs)
+//! taşınıyor; winws'in canlı işi Roblox + genel HTTPS SNI-desync'i.
```

The same note, reworded, is still present verbatim in today's `engine.rs:13-15`. **The
actual removal happened before this repository's first commit (`37316c1`,
"chore: initial repo scaffold", 2026-06-06) or entirely outside version control** — the
in-process engine was built, tested, and removed during a pre-git research phase, not as a
tracked change in this repo.

### B2. Read the dead code

**Not possible from this repository — the file was never committed here, at any point, in
any commit (see B1).** `git show <sha>^:src-tauri/src/engine/real.rs` cannot be run
because no `<sha>` exists where that path is present in the tree.

What follows is **not from git**, but from `docs/_archive/originals-tr/12-BACKEND-BUILD-LOG.md`
(a locally-present, gitignored build log — the pre-git development record), which is the
closest available evidence and is explicitly marked as historical documentation, not code:

- **Threading:** worker thread, not main thread — the log describes a "recv → apply_strategy
  → recalculate_checksums → send" loop running in a spawned thread (line ~132 of that log).
- **Filter breadth:** `WinDivert::network("outbound and tcp.DstPort==443 ...", 0, flags)` —
  narrow (outbound, port 443 only), not the "filter too broad, hurts game ping" failure
  mode described elsewhere in this project's docs for a *different* component.
- **Packet handling:** later log entries (line ~189-191) describe a working
  `desync_c1`-based fragment/reinject implementation ("Orijinali GÖNDERME" — do not send the
  original packet) that reportedly compiled (`cargo check/build --features windivert` →
  `WD_EXIT=0`).
- **TODO markers:** an earlier log entry explicitly marks the real SNI-split/multidisorder/
  fake-injection surgery as `TODO` at the scaffold stage, before the later entry shows it implemented.
- **RAII / `Drop` on the WinDivert handle:** **UNPROVEN.** Grepped the same build log for
  `Drop`, `panic`, `kill`, `crash` — no matches. The log does not document handle lifecycle,
  panic behavior, or exit/kill behavior for the in-process engine.
- **Behavior on app exit / process kill:** **UNPROVEN**, same reason.

### B3. Find the stated reason

**Found, verbatim, in committed code** (not just archived docs) — `src-tauri/src/engine.rs`
today, lines 13-15, present since the file's first commit (`2e883a9`):

> "NOT: Eski saf-Rust WinDivert motoru (`engine/real.rs` + tls/packet/quic/voice byte-cerrahisi)
> net3'e geçişle KALDIRILDI — winws QUIC/gateway desync'ini gerçek yapıyor, eski motor
> masaüstü Discord'u açamıyordu (bkz. net3/SOLUTION.md §3.3). Discord'un kendisi artık WARP
> split-tunnel ile (warp.rs) taşınıyor; winws'in canlı işi Roblox + genel HTTPS SNI-desync'i."

Translation: *"NOTE: The old pure-Rust WinDivert engine (`engine/real.rs` + tls/packet/quic/
voice byte-surgery) was REMOVED with the migration to net3 — winws does the QUIC/gateway
desync for real, the old engine couldn't open desktop Discord (see net3/SOLUTION.md §3.3).
Discord itself is now carried via WARP split-tunnel (warp.rs); winws's live job is Roblox +
general HTTPS SNI-desync."*

This is **not a crash/leak/AV/CPU complaint** — it names a specific functional failure:
**the in-process engine could not successfully open desktop Discord**, and was replaced by
a proven external tool (winws) plus a separate mechanism (WARP tunnel) that could.
`net3/SOLUTION.md` is referenced as the source of this finding but lives outside this
repo's accessible directories (`C:\Users\Evrim\Desktop\projects\net3\`) — **its contents are
UNPROVEN from here; only the citation to it is confirmed to exist.**

**A separate, unrelated incident exists and must not be conflated with this:**
`docs/_archive/done-bsod.md` documents one `0x00000133 DPC_WATCHDOG_VIOLATION` crash. That
analysis's own conclusion: *"WinDivert likely innocent"* — the WinDivert loaded at crash
time belonged to zapret/winws (external), not evorift's own build, and evorift's own
in-process code "was compiled but not installed/running" at the time, so it could not have
triggered the crash. The suspected causes named instead are the ASIX USB-Ethernet driver
and the Riot Vanguard anti-cheat driver. One mitigation *was* taken from this incident —
hiding the Speed-Limit UI to remove one inbound WinDivert capture path — but this is
separate from, and predates by a description standpoint, the actual `real.rs` removal, and
the BSOD write-up does not claim the BSOD caused that removal.

No CHANGELOG entry, commit message, or other doc in this repo names crash, leak, antivirus,
or driver-load-failure as the reason for removing the in-process engine specifically.

### B4. Classification

**(f) does not apply — the reason is found and verbatim, so this is not a case of "history
doesn't say."** But **none of (a)-(e) as given cleanly fit either.** The stated reason is a
functional/capability failure — the engine didn't work for a specific real target, not a
runtime failure (a, b, c) and not a maintainability retreat with no failure (e). It's
closest in spirit to (d) PERFORMANCE if that category is stretched to mean "didn't meet the
functional bar," but that would blur a working-vs-broken distinction the actual evidence is
precise about, and I'm not going to force-fit it to avoid saying so plainly.

**Classification: none of (a)-(e), and not (f) either. The evidence names a sixth
category not offered: FUNCTIONAL FAILURE — the in-process engine could not bypass DPI for
its own hardest real target (desktop Discord's QUIC/gateway connection), and was replaced
by a tool proven to.**

**Confidence: HIGH** that this is the stated reason (verbatim, committed, corroborated
independently by the archived build log's own trajectory). **UNPROVEN** whether this is the
*complete* reason — `net3/SOLUTION.md §3.3`, the document actually cited as the source of
this finding, is outside this repo and was not read for this report.

**Strongest evidence:** `src-tauri/src/engine.rs:13-15`, present verbatim since the file's
first commit and unchanged in today's code.

### B5. Consequences (testable requirements, not a design)

Since the documented failure was **functional** (couldn't bypass DPI for a specific hard
target), not crash/leak/AV/performance, a future in-process design's requirements are
different from what a stability post-mortem would produce. Based only on B1-B3 evidence:

- Any future in-process capture/desync implementation must be validated against the
  **same specific target that broke the old one** — desktop Discord's QUIC/gateway
  connection — not just the easier HTTP/TLS-SNI cases the old engine's own build log
  shows it got further on.
- The validation must be a **live pass/fail test against that target**, using the same
  method the plan already prescribes (`evorift-live-verification`'s baseline → apply →
  measure protocol), not a unit test — the old engine's failure was only visible against
  real network conditions, per the archived evidence (`cargo build` succeeded; the failure
  was functional, not a build/test failure).
- Whatever replaces `engine/real.rs`'s approach to QUIC/gateway desync must be compared
  directly against `winws`'s QUIC/gateway handling on the same target, since that's the
  specific capability gap the removal note names — "winws does the QUIC/gateway desync for
  real" is the standard to match or exceed, not a generic DPI-bypass benchmark.
- Because B2 could not establish the old engine's RAII/Drop/panic/exit behavior (UNPROVEN,
  not "it was fine"), any future in-process design should independently prove its own
  handle-lifecycle safety — this is a gap in what's known, not a clean bill of health to build on.
- If the new design still can't clear the desktop-Discord/QUIC bar, the fallback the
  project already uses today (WARP split-tunnel for Discord specifically, winws for
  everything else) is proven working per the current codebase and shouldn't be assumed
  obsolete just because K1 might reintroduce in-process capture for other traffic.

No architecture is proposed here and no recommendation is made on whether to lift rule 3 —
per the task, that decision belongs to the user.

---

## Unproven (complete list)

- The root cause of the systematic +1 off-by-one in whole-file `"path:1-N"` line-count
  citations in `docs/DISCOVERY.md`/`docs/MIGRATION.md` (A2).
- Whether the `docs-auditor` pass that "re-verified" those same citations actually
  re-counted independently, or reproduced the same source of error (A2).
- Any git-history evidence for the in-process engine's removal — none exists in this repo
  at any commit (B1). The removal predates or lies entirely outside this repo's version
  control.
- The in-process engine's RAII/`Drop` handling on the WinDivert handle (B2) — not
  documented in the one available pre-git source (the archived build log).
- The in-process engine's behavior on panic, process kill, or app exit (B2) — same reason.
- The full contents of `net3/SOLUTION.md §3.3`, cited as the primary source for "the old
  engine couldn't open desktop Discord" — that file lives at
  `C:\Users\Evrim\Desktop\projects\net3\`, outside this repo and outside the directories
  available to this session. Only the citation to it, not its content, is confirmed.
- Whether the BSOD incident (`docs/_archive/done-bsod.md`) had any causal role in the
  `real.rs` removal decision beyond the one UI-hiding mitigation it documents — the two
  events are both real and both documented, but no source ties them together as
  cause-and-effect for the engine's removal specifically.
