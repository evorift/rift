# evorift — Claude Project Guide

**evorift** — Windows DPI-bypass app (reach Discord/Roblox/YouTube without a VPN).
**Stack:** Rust + Tauri v2 + SvelteKit (Svelte 5 + TS). **Repo:** github.com/evorift/rift (user `evorift`, gh installed). **Root:** `C:\Users\Evrim\Desktop\projects\net`.

Read [docs/STATUS.md](docs/STATUS.md) at the start of a session.
Docs: chained set in [docs/README.md](docs/README.md). Dev setup: [.claude/quick-start.md](.claude/quick-start.md).

**The v2 backend rewrite is underway.** Roadmap: [BACKEND-V2-PLAN.md](BACKEND-V2-PLAN.md);
migration inventory: [docs/MIGRATION.md](docs/MIGRATION.md); discovery evidence:
[docs/DISCOVERY.md](docs/DISCOVERY.md). Layer model and DPI term glossary: the
`evorift-dpi` skill — read it before writing code that touches the network layer.

## Architecture (v2 target)

```
evocore   pure logic: flow table, parsing, strategy rules, action
          primitives, escalation. NO Windows API dependency, testable without a driver.
evosys    Windows side: packet capture (WinDivert), service, firewall, DNS,
          routing, process mapping. Everything writes to the rollback journal.
evoapp    Tauri commands, state machine, verification (canary probe), UI bridge.
```

**Resolved 2026-08-12 (see docs/QUESTIONS.md Q1, docs/FORENSICS.md B6):** rule 3 below was
rewritten — the original "no in-process WinDivert" ban rested on a citation
(`net3/SOLUTION.md §3.3`) that was never actually read until the forensics pass; read
directly, it tests `winws` (external), not evorift's own engine, and its finding is that
*no* desync engine can carry Discord's gateway payload — a tunneling problem, not an
in-process-vs-external one. V1.1 and K1 are unblocked as of this commit.

## Critical rules (do not break)
1. **Don't touch `BlackHole.svelte`** — the user develops it in a separate chat; don't "fix" it even if it errors.
2. **Don't run `npm run build`/`check` while `tauri dev` runs** — it kills vite and drops the dev session.
3. **Discord's gateway payload is not a desync problem.** No engine, in-process or
   external, is expected to carry it — `net3/SOLUTION.md` §3.3 established that
   handshake-level desync cannot, tested on `winws`. Discord is carried by tunnel (WARP
   split). Any claim that an engine "opens Discord desktop" must be proven live via
   `evorift-live-verification` before it is written anywhere. (Evidence: docs/FORENSICS.md B6.)
3b. **In-process packet capture is permitted**, under two conditions: (a) handle lifecycle
   (RAII/`Drop`, panic, kill, process exit) is proven by test — the old engine's was never
   documented, which is a gap, not a clean record; (b) WinDivert version conflict with a
   co-installed zapret/winws is handled (a real, previously-hit failure — see net3/SOLUTION.md
   §4.2/§9, "WinDivert version conflict").
4. **Privileged ops** (tweak/DNS/QoS/firewall) need **admin** to apply; as a normal user they are audit-only `(sim)`.
5. **Parallel sessions:** re-read `ipc.rs`/`service.rs`/`Cargo.toml` before editing; run `cargo check --target-dir tmp_check` so you don't break the running dev process.
6. **BlackHole/three type warnings** in `svelte-check` are pre-existing and fine; aim for 0 errors in our own files.
7. **Always respond in English.** All outputs — chat replies, plans, docs, and new code comments — must be in English (user directive, 2026-06-13). Existing Turkish comments may stay; write new code/comments/responses in English.

## HARD RULES — v2 additions (do not merge into or edit the numbered list above)

8. **Silent success is forbidden.** Running unprivileged is `Err(NotElevated)`, a missing
   bundle is `Err(BundleMissing)`. Falling back to "sim" mode while reporting "Active" was
   v1's core bug (see rule 4's `(sim)` marker — v2 makes this an explicit typed error, not
   a silent flag).
9. **Applied ≠ working.** The UI only says "protected" for a state the canary probe verified.
10. **State is measured, not remembered.** `is_running` queries the real process/service
    state; cached-bool status code is rejected.
11. **A strategy change never drops the connection (v2 engine).** Once the v2 rule engine
    exists, it is never restarted to change strategy — the rule set hot-swaps. This rule
    only binds once V2.3 (hot swap) lands; it does not apply to the current winws-sidecar
    engine, which restarts by design today.
12. **Reversibility.** Every change made on the user's machine is journaled first (v1 already
    does this — see `rollback.rs`, portable to v2 largely unchanged per docs/DISCOVERY.md).
    If it can't be written, the change doesn't happen. Detail: `evorift-security-hygiene`.
13. **No secret is ever written to any file/log/report.** WARP private key, license key, the
    user's IP, domain history.
14. **The installer is only produced via `npm run tauri build`.** Not a plain `cargo build`.
15. **`git push` belongs to the user.** Commits happen at phase ends.

## Backend master plan workflow
The v1 hardening plan ([BACKEND-MASTER-PLAN.md](BACKEND-MASTER-PLAN.md)) is superseded by
[BACKEND-V2-PLAN.md](BACKEND-V2-PLAN.md) — v1's backend is being rewritten from scratch as
`evocore`/`evosys`/`evoapp`, not hardened in place. **Execution cadence: one plan item per
user message** — implement exactly one item (the next unchecked one, or the one named),
verify it, mark it done, and stop. The frontend redesign must keep `BlackHole.svelte` and
settings working — the backend exposes the health signals + commands those need.

## Verify order
`cargo check --all-targets --target-dir tmp_check` → `cargo test --lib` →
`cargo clippy --all-targets` → (dev server closed) `svelte-check` → `npm run build`.
Full detail and build pitfalls: `evorift-rust-tauri` skill. Anything requiring the network
isn't a unit test → `evorift-live-verification` skill; the user runs that pass.

## Product constraints
- No server/cloud. License verification is offline (Ed25519-signed file).
- Access features are always free. Premium is only a convenience/acceleration layer.
- Telemetry defaults to off; if on, the user can see the raw data sent.
- WinDivert is LGPL: dynamically linked, license text distributed.

## Distribution
Ship only build output (exe + MSI + portable ZIP + SHA256SUMS). `src/`, `docs/`, `.claude/` stay private (gitignored); user-facing docs = README only.
