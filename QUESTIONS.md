# QUESTIONS.md

Collected at the end of each phase where the workflow calls for it (Phase 2, Phase 5),
per the v2 infrastructure build. Answered questions are marked and dated; don't delete —
history has value.

## Open

(none currently — Q1 below was answered end of Phase 2)

## Answered

### Q1 (SUPERSEDED 2026-08-12 — see "Q1 — revised" below) — Hard rule 3 vs. the V1.1 plan item

> ⚠ **This decision was reversed the next day, once its own cited evidence was actually
> read.** Kept below for history; do not act on it. Current state is "Q1 — revised" further
> down this file.

**Original decision (2026-08-11): keep rule 3 as a hard ban.** In-process WinDivert capture stays off-limits.
V1.1 as currently written in BACKEND-V2-PLAN.md ("`PacketSource` trait + WinDivert
implementation" = in-process capture) is **rejected** and needs to be redesigned to
achieve live strategy control while keeping the capture layer external-process-based
(consistent with the existing `winws`-sidecar approach). K1's framing ("WinDivert vs.
TUN," which assumed in-process capture either way) is invalidated along with it — the
real open question becomes: *how do you get runtime-tunable strategy on an external
process* (e.g. a control channel/IPC to a long-lived external capture process that
accepts live rule updates, vs. some other mechanism) — not which in-process driver to use.

**Follow-through:** V1.1 is marked BLOCKED in BACKLOG.md pending an `architect` decision
(see the K1 brief in BACKLOG.md's DECISIONS section, rewritten to reflect this). Original
plan text in BACKEND-V2-PLAN.md is annotated in place, not rewritten — the actual
technical redesign of V1.1 is architect/user work, not something invented here.

### Q1 — revised (closed 2026-08-12, see docs/FORENSICS.md B6)

**What changed:** the 2026-08-11 decision rested on a citation
(`engine.rs:13-15` → "old engine couldn't open desktop Discord, see net3/SOLUTION.md §3.3")
that nobody had actually read. Read directly (docs/FORENSICS.md B6, quotes-only, no
interpretation): §3.3 tests `winws` (external zapret), never evorift's own `real.rs`
engine — no source names or tests that specific implementation. Its finding is that **no**
desync engine, in-process or external, can carry Discord's gateway WebSocket payload
(zstd-compressed `READY`, dropped by deep inspection) — SNI/handshake-level blocking is
explicitly confirmed working via the same desync approach in the same document, for
Discord, Roblox, and general HTTPS alike. The failure is a tunneling gap, not an
in-process-capture hazard. The 2026-08-11 decision had treated a citation as evidence
without reading it — the same failure class as the A2 off-by-one finding (see
`docs/MIGRATION.md`'s "recompute, don't re-read" methodology note), generalized: **an
unread citation is not verified evidence, regardless of which document it's in.**

**Revised decision:** CLAUDE.md rule 3 rewritten (not just annotated) — Discord's gateway
payload isn't a desync problem for any engine; it's carried by WARP split-tunnel, as the
product already does. New rule 3b permits in-process packet capture under two proven
conditions: handle-lifecycle safety (RAII/`Drop`, panic, kill, exit — undocumented for the
old engine, a gap not a clean record) and WinDivert version-conflict handling against a
co-installed zapret/winws (a real, previously-hit failure per `net3/SOLUTION.md` §4.2/§9).

**Follow-through:** V1.1 and K1 unblocked in `BACKLOG.md`/`BACKEND-V2-PLAN.md` (K1 reframed
as WinDivert-vs-TUN, with 3b as a binding constraint either way). Two new unsolved items
opened: P1 (warp.rs and net3/SOLUTION.md give different causes for the same Discord-desktop
symptom — investigate before V6) and P2 (WinDivert version-conflict handling mechanics —
feeds K1 directly). Neither is resolved here.

<details><summary>Original question text (for context)</summary>

`CLAUDE.md`'s existing critical rule 3 says: *"There is no in-process WinDivert engine
anymore ... don't reintroduce one."* This was written for v1, after an earlier in-process
engine (`engine/real.rs`) was deliberately removed in favor of the bundled `winws` sidecar.

`BACKEND-V2-PLAN.md`'s **V1.1** explicitly plans to build a `PacketSource` trait with
WinDivert as the first implementation — i.e. an in-process capture engine — inside the new
`evosys` crate. **K1** in the same plan frames the open question as "WinDivert vs. a
TUN-based path," which already assumes in-process capture is the direction, not something
still up for debate.

**docs-auditor confirmed** (2026-08-11) both sides of this factually: rule 3 accurately
describes today's code (`engine.rs`/`service.rs` — winws is launched as an external
process, strategy changes force a restart), and V1.1/K1 in the plan really do call for
in-process capture. This is not a doc error — it's a genuine, unresolved tension between
standing project doctrine and the new plan's direction. Not touched or resolved by
`docs-audit`/`md-denetim` per its own rule (hard rules are user intent, not derived from code).

**Question:** was rule 3 meant as permanent doctrine, or as v1-specific doctrine that the
v2 rewrite is expected to supersede (in which case it should be retired/reworded once V1.1
lands, not before)? This doesn't block any of the Phase 3-5 infrastructure work — it's
carried forward into the K1 decision brief in BACKLOG.md's DECISIONS section — but it should
be resolved before V1.1 actually starts, since it changes what V1.1 is allowed to build.

</details>
