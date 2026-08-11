# STATUS — updated: 2026-08-11 (session close)

## Where things stand right now

v1 is a working, tested Windows DPI-bypass app (Rust/Tauri/Svelte) built around a bundled
`winws` sidecar process + optional WARP split-tunnel, with mature rollback/preflight/
manifest/profile modules and real test coverage. v2 is fully at the infrastructure stage:
no v2 code exists yet (no `evocore`/`evosys`/`evoapp` crates), but discovery, a doc audit,
a migration inventory, and a full 51-item ready-to-paste backlog (with 3 decision briefs)
now exist — the next session can start writing code.

## Done this session

- Installed 4 evorift-specific skills (`evorift-dpi`, `evorift-security-hygiene`,
  `evorift-live-verification`, `evorift-rust-tauri`) + 1 new agent (`packet-analyst`) to
  `~/.claude/` — the kit's other agents/skills were skipped as duplicates of the existing
  English roster (`repo-scout`, `code-analyst`, `bug-hunter`, `code-referee`, `architect`,
  `test-runner`, `docs-auditor`, `feature-flow`, `bug-fix`, `evo-review`, `session-close`,
  `docs-audit`, `claude-md-generator`).
- `docs/DISCOVERY.md` — v1→v2 migration discovery report (gitignored by design, `docs/`
  stays private per CLAUDE.md's Distribution section).
- `BACKEND-V2-PLAN.md` added at repo root (commit: `5b0a93b`).
- `CLAUDE.md` merged with v2 hard rules + architecture target (commit: `060347b`).
- `QUESTIONS.md` added, Q1 raised and resolved same session (commits: `1a60fe5`, `b8d7728`).
- Doc audit: 15 factual claims checked across `CLAUDE.md`/`BACKEND-V2-PLAN.md` by
  `docs-auditor`, all 13 confirmed correct, 2 expected-absent (docs not yet created),
  0 wrong, 0 stale.
- **Decision (Q1):** CLAUDE.md hard rule 3 (no in-process WinDivert) stays a hard ban.
  V1.1 and K1 in `BACKEND-V2-PLAN.md` are annotated BLOCKED/REFRAMED (commit: `b8d7728`) —
  they assumed in-process capture, which is now off the table. Real question: how to get
  live-tunable strategy control over an **external**-process capture layer instead.
- `docs/MIGRATION.md` — 7-item v1→v2 module inventory with target crate, contract changes,
  and S/M/L effort per item (gitignored, same as other `docs/` files).
- `docs/STATUS.md`, `docs/LIVE-VERIFICATION.md` created (this file + an empty run-log skeleton).
- `BACKLOG.md` — all 51 `BACKEND-V2-PLAN.md` items as ready-to-paste prompts, plus a
  DECISIONS section with K1/K2/K3 architect briefs (drafted, not called) and a P3 section
  with 6 out-of-plan findings from discovery (commit: `44d5f65`).

## Broken / half-done / known issues

- No v2 code exists yet — everything above is infrastructure (docs, skills, backlog), not implementation.
- V1.1 (and therefore all of V1-V10 downstream of it) is blocked on a capture-layer
  redesign — see `QUESTIONS.md` Q1 and the K1 brief in `BACKLOG.md`. V0 items don't depend
  on this and can proceed first.
- V7.3 is blocked on K2 (throttling technique); not urgent — K2's brief explicitly isn't
  ready to fill in until V7 is reached.
- V6.x shouldn't start before K3 (tunnel keep/drop) has live data from V5 — the plan's own recommendation.
- v1 has known half-done areas carried into `docs/DISCOVERY.md`: bandwidth limiting
  (code-complete, UI hidden, not live-tested), per-app domain auto-detection (not
  stress-tested, tied to a past CPU-spike bug), community profile fetch (no hash pinning
  on the downloaded bundle — see BACKLOG.md P3.1).
- `BACKEND-HARDENING-PLAN.md` still sits untracked at repo root, superseded by
  `BACKEND-V2-PLAN.md` — not touched this session (not mine to remove, see BACKLOG.md
  P3.5); flagged here so it isn't mistaken for live guidance.

## Next 3 steps (ready to paste)

1. Fill in the K1 brief in `BACKLOG.md`'s DECISIONS section (run a `pattern-scout` pass
   first on whether zapret/winws or a fork supports live reconfiguration), then run it
   through `architect` — **Opus 5, brief only, no broad exploration on that model** — to
   unblock V1.1 before starting V1 capture work.
2. In parallel (doesn't depend on K1): start `BACKLOG.md` item **V0.1** (Cargo workspace
   split into `evocore`/`evosys`/`evoapp`), then V0.2-V0.4 in order.
3. Once V0 is done and K1 is resolved, rewrite V1.1's task/acceptance-criteria in
   `BACKLOG.md` to match K1's actual decision, then proceed through V1.2-V1.4 (V1.2's flow
   table doesn't need K1 resolved first if built against a mock capture source).

## Open questions (for the user)

- See `QUESTIONS.md` — Q1 is answered; no other open items as of this session's close.
