# DIAGNOSE-INTERNET-CUT.md — Procedure for the "protection on = internet cut" failure

**This is a procedure, not a conclusion.** The live run on 2026-08-13 produced a hard
failure: turning protection on cut all internet, Discord never opened, and connectivity
returned only when the app was closed. That the failure is process-bound (tied to the
app's own lifetime, not a lingering system change) makes a persistent config change
(DNS/firewall/route left behind) look less likely than a packet-filter problem — but
that is a hypothesis to check against the captures below, not something to assume going
in. Do not skip straight to "it's the filter" — measure, then read the table.

## What this captures, and why

`scripts/capture-state.ps1` takes a system-state snapshot: network config, firewall
rules (filtered to `evorift`/`winws`), the three possible WinDivert service names plus
`EvoriftSvc`, running `winws`/`evorift*` processes with full command lines, a local-only
reachability probe (gateway/1.1.1.1/DNS), and a copy of evorift's own log files. It does
not fix anything and does not touch any state — every command it runs is read-only.

## Run order

Run all three captures from the same **elevated** PowerShell window (Run as Administrator)
if at all possible, so the environment (elevation, working directory) is consistent across
them. This isn't just tidiness: verified directly while testing this script that an
unprivileged capture's `Get-CimInstance Win32_Process` query silently returned zero results
for `evorift.exe` even though `tasklist` confirmed it was running — `evorift.exe` now
self-elevates (P0-a's `requireAdministrator` manifest), and an unprivileged CIM query appears to drop
higher-integrity processes from the result set rather than error on them. `winws.exe`,
spawned by an elevated process, will likely hit the same gap — an unprivileged `during`
capture may show `winws.exe` present via `tasklist` but with no command line, which would
wrongly look like "process exists but couldn't confirm what filter it's running" instead
of a clean answer. Each run creates a new timestamped folder under `docs/captures/` —
nothing gets overwritten, so re-running elevated after an unprivileged attempt is harmless.

1. **Before.** Protection off, app not yet started (or started but protection not
   applied).
   ```powershell
   .\scripts\capture-state.ps1 -Label before
   ```
2. **Start protection.** Apply it exactly as the failing run did. Give it a few seconds
   to settle — if internet cuts immediately, that itself is useful timing information,
   note when you ran step 3 relative to when you applied protection.
3. **During.** While protection is (believed to be) on and the cut is (believed to be)
   happening.
   ```powershell
   .\scripts\capture-state.ps1 -Label during
   ```
   If the machine is far enough offline that even this local script can't finish
   (unlikely — it doesn't need network — but if `Resolve-DnsName`/ping hang longer than
   expected, that's fine, they're timeout-bounded and will return "failed" rather than
   hang forever), let it finish; a slow reachability section is itself data.
4. **Close the app** (the same way you did when internet came back before).
5. **After.** Confirm recovery and capture the restored state.
   ```powershell
   .\scripts\capture-state.ps1 -Label after
   ```

## If the machine gets stuck offline

The prior run already showed internet returns when the app is closed — so closing the
app (Task Manager → End Task on `evorift.exe`, or Ctrl+C if run from a console) is the
first recovery step, and per the prior run it should be sufficient. If it is not this
time:

1. `sc stop EvoriftSvc` (if the service is installed and running — this is the most
   likely thing still holding a filter open after the UI process exits).
2. `taskkill /f /im winws.exe` — kills the desync engine process directly.
3. If still offline after both: `netsh advfirewall firewall show rule name=all dir=out`
   (from a machine with network, or from memory of step 2's filtered capture) to find any
   `evorift-*` rule and `netsh advfirewall firewall delete rule name=<rule-name>` it.
4. `ipconfig /release` + `ipconfig /renew` as a last resort if DNS/DHCP state looks
   disturbed in the `after` capture compared to `before`.

None of this is a fix for the underlying bug — it's how to get back online to keep
working. Do not conclude the bug is "solved" because one of these steps restored
connectivity; that would conflate a manual recovery action with the root cause.

## What to look for in each diff (before vs. during vs. after)

Compare `before` against `during` file-by-file. Then compare `during` against `after` —
what reverted on its own when the app closed vs. what needed a manual step in the
recovery section above is itself informative (auto-revert-on-exit is consistent with a
process-held resource; something that needed manual cleanup is consistent with a
persistent change).

| File | What "no change" looks like | What a change would mean | Suspect it implicates |
|---|---|---|---|
| `01-network-config.txt` | Same DNS server list, same default gateway, same interface IPs in `before`/`during` | DNS servers changed, gateway changed, or an interface got a new/different address | DNS/route reconfiguration — **not** a pure packet-filter problem |
| `02-firewall-rules-filtered.txt` | Same `evorift-*` rules present (or same "no rules found") in `before`/`during` | A new `evorift-*` block-rule appears in `during` that wasn't in `before` | Firewall rule — check if it's an overly broad block (e.g. blocking more than the intended per-app scope) |
| `03-services.txt` | `WinDivert`/`WinDivert1.4`/`WinDivert1.1` all show the same STATE in `before` vs `during` (usually STOPPED/not-installed at baseline) | A WinDivert service transitions to RUNNING in `during` and stays RUNNING in `after` (not just during) | Stale driver left running after the process exits — would explain persistence, but the prior run says internet came back on close, so check this first since it would contradict that observation |
| `04-processes.txt` | `winws.exe` absent in `before`, present in `during`, absent again in `after` | `winws.exe` (or a stale copy) still present in `after` | Orphaned process — the kill-on-close Job object (`engine.rs::assign_to_job`/`ensure_job`) not actually working |
| `04-processes.txt` (command line) | — | The `winws.exe` command line in `during` shows a filter scope (`--wf-tcp=`/`--wf-raw=`) far broader than expected, or missing an exclusion that should be there | **Packet-filter problem** — the filter itself is catching traffic it shouldn't |
| `05-reachability.txt` | Gateway ping succeeds in `before` | Gateway ping *also fails* in `during` (not just DNS/discord.com) | Broad — this is layer 0/1 (link-level), points at the packet filter dropping essentially everything, not a DNS-specific or app-specific issue |
| `05-reachability.txt` | — | Gateway ping **succeeds** but 1.1.1.1 and DNS resolution **fail** in `during` | More specific — local link fine, WAN/DNS path blocked; check DNS config change (file 1) before blaming the filter |
| `evorift-logs\` | — | Log lines around the time of the cut mention DNS changes, tunnel/WARP setup, or a specific filter string | Whatever the log actually says — read it before the table above, it may name the mechanism directly instead of requiring inference from diffs |

**Reading the table as a whole:** if `01-network-config.txt` and `02-firewall-rules-filtered.txt`
are IDENTICAL between `before` and `during` (no new rule, no DNS/route change) while
`03-services.txt`/`04-processes.txt` show WinDivert/`winws.exe` newly running with a
broad filter, and `05-reachability.txt` shows even the gateway ping failing — that
combination is the packet-filter hypothesis the report's own reasoning leaned toward,
now actually measured rather than assumed. Any other combination (a firewall rule
appearing, a DNS server changing, a service still running in `after`) means the
process-bound assumption was wrong and the real mechanism is elsewhere — follow the
table, not the initial hunch.

## Verification

`.\scripts\capture-state.ps1 -Label before` was run standalone to confirm the script
itself completes without a fixed system state to diff against yet (see commit for the
actual output/timing). Per `evorift-rust-tauri`, this is a PowerShell script rather than
Rust — no `cargo check`/`test`/`clippy` applies; the check that applies here is "does it
run and produce the expected files without throwing," verified directly.
