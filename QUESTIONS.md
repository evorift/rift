# QUESTIONS.md

Collected at the end of each phase where the workflow calls for it (Phase 2, Phase 5),
per the v2 infrastructure build. Answered questions are marked and dated; don't delete —
history has value.

## Open

### Q1 — Hard rule 3 vs. the V1.1 plan item (raised end of Phase 2, 2026-08-11)

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

## Answered

(none yet)
