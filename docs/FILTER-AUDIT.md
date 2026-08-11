# FILTER-AUDIT.md — WinDivert filter / capture blast-radius audit (read-only)

No fixes below. Every claim has file:line evidence; anything outside this repo's evidence
is marked "not determinable from this repo" rather than guessed at.

## Scope-defining finding, before the table

**evorift's own Rust code never opens a WinDivert handle and never captures a packet.**
Confirmed by an exhaustive grep of `src-tauri` (including `Cargo.toml`) for `windivert`,
`WinDivertOpen`, `WinDivertRecv`, `WinDivertSend`, `windivert-sys`, `windivert-rs` — every
hit is either (a) the bundled `WinDivert.dll`/`WinDivert64.sys` referenced as opaque
resource files (`tauri.conf.json:47-57`), (b) `sc.exe query|stop|delete` commands against
the driver's **service name** (`engine.rs:462-494`, `preflight.rs:35-43`,
`testd/recovery.rs:35,137` — service-control, not the WinDivert API), or (c) plain `String`
CLI arguments (`--wf-tcp=`, `--wf-udp=`, `--wf-raw=`, `--wf-raw-part=`) appended to a
`Vec<String>` handed to `std::process::Command::new(winws.exe)` (`engine.rs:585-679`).
`Cargo.toml` has no `windivert`/`windivert-sys`/`windivert-rs` dependency anywhere.

**Consequence for this audit:** the literal question "is a captured packet guaranteed to
be reinjected on every path, including error paths and early returns" has no code in this
repo to answer it against — that guarantee lives entirely inside the external, closed-to-
this-repo `winws.exe` (zapret) and `goodbyedpi.exe` binaries. Stating an answer either way
would be speculation about a binary this repo doesn't contain. What the table below and the
section after it DO answer, from this repo's actual code: what scope each engine's filter
argument covers, whether construction can silently degrade, and whether the WinDivert
**driver service** can be left armed-but-orphaned after `winws.exe` dies unexpectedly.

## Filter / scope-construction paths (all engines)

| File:Line | Expression / concrete example | Scope | Unbounded-scope / silent-degradation risk | Reachable from default Start? |
|---|---|---|---|---|
| `engine.rs:611-619` (modular branch, default) | `--wf-tcp=80,443` + 3× `--wf-raw-part=@<signature file>` (Discord-media/STUN/QUIC) | **All outbound TCP on port 80 or 443, system-wide — every process on the host, not scoped to any app.** | Bounded by port, not app. Empty `strategy.wf_tcp` falls back to `"80,443"` (`engine.rs:611`), never to an actually-empty/catch-all filter. | **Yes — this is the exact path a plain "Start" hits** (default engine `zapret`, default strategy `c1`, `hostlist_only=false`, `excl` empty by default). |
| `engine.rs:608-609`, `554-580` (master-filter branch, exclusion active) | `--wf-raw=@<path>`, file content `(<same catch-all base>) and (tcp.SrcPort != <excluded port>)` | Same catch-all base as above, minus specific excluded app ports | **Silent fail-open on file-read error:** if any of the 3 signature files can't be read, `build_master_filter` returns `None` with **no log line** (`engine.rs:558-563`), and the caller falls through to the modular catch-all branch — the user's exclusion is silently dropped and the "excluded" app's traffic reverts to catch-all capture with no error surfaced. `create_dir_all`/`write` failures are likewise discarded (`let _ =` / `.ok()?`, `engine.rs:577-578`). | No — requires the user to have set a specific app to "off" mode first; only takes effect via a 5s background watchdog once the engine is already running (`service.rs:882,890-891`). |
| `engine.rs:625-634` (`--hostlist=`) | `--hostlist=<path>`, applied to TCP/80, TCP/443-primary, and QUIC groups only — **Discord voice/STUN is never hostlist-gated** (`engine.rs:672-677`, always catch-all) | Restricts desync to listed domains for 3 of 4 traffic groups; voice/STUN stays unrestricted regardless of hostlist | Write failure only `eprintln!`s and still passes the (possibly stale/missing) path to winws as if it succeeded (`engine.rs:628-631`) — not unbounded, but a silent-failure path. | No — requires `ApplyProfile` with split-scope mode and a non-empty hostlist (`service.rs:464`). |
| `engine.rs:611-614` (ISP presets) | e.g. `--wf-tcp=80,443 --wf-udp=443,50000,50100` | Adds UDP/443 + Discord voice port range to the catch-all | Same port-bounded-not-app-bounded characteristic as the default. | No — requires selecting a specific ISP preset explicitly. |
| `goodbyedpi.rs` (no `--wf-*` construction in this repo) | e.g. mode 9 default: `-f 2 -e 2 --wrong-seq --wrong-chksum --reverse-frag --max-payload -q` | GoodbyeDPI's own internal WinDivert filter is **hardcoded inside `goodbyedpi.exe`**, not built by this repo. System-wide by default unless `--blacklist <file>` is set (`goodbyedpi.rs:169-171,186,243-250`). | The actual filter string is internal to the external binary — **not determinable from this repo**. | Not the default engine; requires explicit `SetEngine{id:"goodbyedpi"}`. |
| `byedpi.rs:58-94` | e.g. `--split 1 --disorder 3+s --mod-http=h,d --auto=torst --tlsrec 1+s` | **No packet-capture filter at all** — ByeDPI (`ciadpi.exe`) is a kernel-less local SOCKS5 proxy (`caps: kernel_less:true, requires_windivert:false`, `byedpi.rs:1-7,213-219`); scope is whatever the routing layer sends into it, not a WinDivert expression. | N/A — nothing to be unbounded. | Not the default engine. |
| `proxifyre.rs:45-59` | JSON `appNames` allowlist, e.g. `["discord","Discord.exe",...,"roblox",...]` (`proxifyre.rs:17-20`) | **Different mechanism entirely** — Windows Packet Filter (NDIS) keyed by executable-name allowlist, not WinDivert filter syntax (`proxifyre.rs:3-9,28-41`). | An empty `appNames` list's behavior isn't exercised by any code path found — not determinable from this repo. | Not the default engine; requires `byedpi-proxifyre` selection. |
| `wiresock.rs:15-21` | WireGuard-style conf, `AllowedApps = <names>` | **Different mechanism** — app-name-based filtering via WireSock's own conf format, not a WinDivert expression (`wiresock.rs:1-8`). | Empty `AllowedApps` behavior is external to this repo, not determinable. | Not the default engine (separate Tunnel-kind path). |

No other file in `src-tauri/src` constructs a `--wf-*`/filter argument — confirmed by grep;
the only other hits were `evorift-ctl.rs:60-61`, which just print `wf_tcp`/`wf_udp` for
display, not construction.

## WinDivert driver-service armed-but-orphaned gap (in-repo evidence only)

This is the closest in-scope equivalent to "can a packet be affected without proper
cleanup" that this repo's code can actually answer.

- **Cleanup is reactive, not event-driven.** The only mechanism that notices "winws.exe
  died unexpectedly" is a background loop that **polls every 5 seconds**
  (`service.rs:882`, `std::thread::sleep(Duration::from_secs(5))`), checking
  `child.try_wait()`. There is no Job-Object exit notification, no I/O completion port, no
  callback-on-death anywhere in `engine.rs`/`service.rs` — purely a poll.
- **Quoted worst-case window, from the code's own intervals (not an estimate):** up to
  **~5s** before evorift's own code even notices winws died, then `clear_stale_windivert()`
  adds up to **~2.5s** of its own stop-and-poll (`5×500ms`, `engine.rs:479-487`) before
  attempting `sc delete`. **Worst case ≈ 7.5s** between "winws dies unexpectedly" and "this
  repo's code has even attempted to tear down the stale driver service." What that gap
  means for WinDivert.sys's internal packet handling during that window is internal to the
  driver and **not determinable from this repo**.
- **Asymmetry across engines:** `GoodbyeDpiEngine::start()` (`goodbyedpi.rs:272-304`)
  **never calls `clear_stale_windivert()`** — only `WinwsEngine` does. Switching to or from
  GoodbyeDPI doesn't itself re-arm this cleanup.
- **`set_exclusion()` gap:** kills the child process (`engine.rs:764-766`) but does **not**
  call `clear_stale_windivert()` itself — cleanup for that specific teardown only happens
  on the *next* `start()` call, reached via the same 5-second watchdog, so the same
  worst-case window applies to exclusion-triggered restarts too.

## Flagged for the operator (not fixed here)

1. **`build_master_filter`'s silent fail-open** (`engine.rs:558-563,577-578`) is the one
   place a user-configured exclusion can be silently dropped with zero error surfaced —
   worth knowing about independent of the internet-cut investigation.
2. **The ~7.5s worst-case cleanup window** applies every time winws dies unexpectedly, not
   just during the incident being diagnosed — relevant context for `docs/HYPOTHESES-INTERNET-CUT.md`
   candidate #2 (WinDivert capture/reinjection failure), and for interpreting
   `03-services.txt`/`04-processes.txt` in tomorrow's capture: a WinDivert service still
   `RUNNING` moments after `winws.exe` disappears from `04-processes.txt` is expected
   within this window, not necessarily evidence of a stuck/broken cleanup on its own —
   only a service still `RUNNING` well past ~10s after the process is gone would indicate
   a genuine failure of this path.
