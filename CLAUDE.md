# evorift — Claude Project Guide

**evorift** — Windows DPI-bypass app (reach Discord/Roblox/YouTube without a VPN).
**Stack:** Rust + Tauri v2 + SvelteKit (Svelte 5 + TS). **Repo:** github.com/evorift/rift (user `evorift`, gh installed). **Root:** `C:\Users\Evrim\Desktop\projects\net`.

Docs: chained set in [docs/README.md](docs/README.md). Dev setup: [.claude/quick-start.md](.claude/quick-start.md).

## Critical rules (do not break)
1. **Don't touch `BlackHole.svelte`** — the user develops it in a separate chat; don't "fix" it even if it errors.
2. **Don't run `npm run build`/`check` while `tauri dev` runs** — it kills vite and drops the dev session.
3. **DPI engine = bundled `winws` (zapret) sidecar + WARP split-tunnel** (net3 migration). There is **no in-process WinDivert engine** anymore (`engine/real.rs` + tls/packet/quic/voice removed); don't reintroduce one. Keep `resources/winws/` + the WARP bundle intact.
4. **Privileged ops** (tweak/DNS/QoS/firewall) need **admin** to apply; as a normal user they are audit-only `(sim)`.
5. **Parallel sessions:** re-read `ipc.rs`/`service.rs`/`Cargo.toml` before editing; run `cargo check --target-dir tmp_check` so you don't break the running dev process.
6. **BlackHole/three type warnings** in `svelte-check` are pre-existing and fine; aim for 0 errors in our own files.
7. **Always respond in English.** All outputs — chat replies, plans, docs, and new code comments — must be in English (user directive, 2026-06-13). Existing Turkish comments may stay; write new code/comments/responses in English.

## Backend master plan workflow
The full multi-phase backend roadmap (covers every function in the net3 docs) lives in [BACKEND-MASTER-PLAN.md](BACKEND-MASTER-PLAN.md). Build the backend first, test after each stage, then move to the frontend. **Execution cadence: one plan item per user message** — when the user sends a message, implement exactly one item (the next unchecked one, or the one they name), verify it, mark it done, and stop. The new frontend will be a redesign with many features but must keep `BlackHole.svelte` and the settings working — the backend must expose the health signals + commands those need.

## Verify order
`cargo check` → `svelte-check` → (only with dev closed) `npm run build`.

## Distribution
Ship only build output (exe + MSI + portable ZIP + SHA256SUMS). `src/`, `docs/`, `.claude/` stay private (gitignored); user-facing docs = README only.
