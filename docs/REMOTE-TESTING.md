# Remote testing — running evorift on the test laptop from your desk

Push a build to the second Windows laptop, turn protection on, capture what happens, and pull the
evidence back — without touching the laptop.

**Why this exists.** evorift has been reported to cut all internet on its host until the process
is killed ([DIAGNOSE-INTERNET-CUT.md](DIAGNOSE-INTERNET-CUT.md)). You therefore lose contact with
the machine at exactly the moment the test gets interesting. The whole design follows from that:

- **The laptop can save itself.** A deadman switch fires after 120 s of silence from the
  controller: kills `evorift.exe` and `winws.exe`, tears down stale WinDivert services via
  evorift's own [`engine::clear_stale_windivert`](../src-tauri/src/engine.rs), and puts DNS back
  on DHCP. Every fire is logged.
- **Results survive the outage.** Output is written to disk on the laptop as it is produced, never
  held in memory awaiting an HTTP response. You collect it afterwards.
- **The interesting step runs entirely on the laptop.** Start → capture → stop is one local job, so
  it completes even if the link dies the instant protection comes on.

---

## Exclusive control (added 2026-08-13, see BACKLOG.md P0-e)

Every MVP test script (`test1-discord.ps1`, `test2-repeats-sweep.ps1`, `test3-toggle.ps1`) now
checks, before doing anything else, that no `evorift-svc`/`winws` process is already running.
If one is found, the job aborts immediately rather than measuring against unknown prior state.

This exists because a live run found `evorift-svc.exe`/`winws.exe` still running on the test
laptop several minutes after the job that should have owned them had already exited and its own
`evorift-ctl off` had reported success — see BACKLOG.md P0-e for the full evidence. **A
read-only scan (Scheduled Tasks, registry `Run`/`RunOnce` across every user hive, Startup
folders, `Win32_Service`) found NO configured auto-start mechanism on this laptop** — nothing to
disable, no restore command needed. The root cause is unconfirmed; treat any future recurrence
as a live repro opportunity for P0-e, not as this scan's job to explain.

If a genuine auto-start mechanism (task/service/Run key) IS found on a future laptop, disable it
— don't delete it — and record here the exact re-enable command (e.g.
`schtasks /Change /TN "<path>" /Enable`, or `sc.exe config <name> start=auto` followed by
`sc.exe start <name>`) before running any measurement.

---

## Daily use

Double-click **`REMOTE-TEST.bat`** in the repo root. It finds the laptop by itself and shows:

| | What it does |
| --- | --- |
| **1** | Connection test — is the agent answering? |
| **2** | Dry run — captures state 3×, does **not** start evorift |
| **3** | **The real test** — starts evorift, captures it, stops it, checks the network came back |
| **4** | Emergency rescue — kill evorift/winws, clear WinDivert, reset DNS |
| **5** | Diagnose the link |
| **6** | Re-find the laptop (if its IP just changed) |
| **7** | Exit |

Option **3** is the one that matters. It asks how long to hold protection on (default 15 s), then:

1. pushes `evorift-svc.exe`, `evorift-ctl.exe` and `resources/winws/` into the sandbox
2. runs [`run-engine-test.ps1`](../scripts/run-engine-test.ps1) **on the laptop** as a single job:
   capture `before` → `evorift-ctl on` → hold → capture `during` → `evorift-ctl off` → capture
   `after` → verify the network recovered
3. pulls every capture back to `docs/captures/engine-<timestamp>/`
4. prints a verdict

Read the verdict:

- `winws RAN` — a real bypass was active, so the `during` capture means something. If this says
  winws never started, `during` looks identical to `before` and proves nothing.
- `the network recovered` — evorift cleaned up after itself.
- `the deadman did not fire` — the run was uninterrupted. **If it fired, the laptop rescued itself
  mid-test and the captures are truncated; do not draw conclusions from them.**

### Reading the captures

Each of `before` / `during` / `after` holds the same 7 probes. The one that answers "did it work"
is `05-reachability.txt`:

```
before   discord.com A -> 195.175.254.2       <- ISP DNS poisoning (sinkhole)
during   discord.com A -> 162.159.128.233     <- real Cloudflare address
```

`01-network-config.txt` (adapters, routes, DNS), `03-services.txt` (WinDivert + EvoriftSvc state)
and `04-processes.txt` (the full `winws` command line) are the next places to look.

> The engine's arguments are built by `engine.rs` and read back out of `04-processes.txt`. Nothing
> in this harness reconstructs them — a second copy would drift from the product, and the copy that
> drifts is the one that runs during the test.

---

## Nothing is pinned to an IP

Both machines took new DHCP leases on the same day once, which simultaneously invalidated the bind
address, the allowlist and the address the controller was dialling. That class of failure is gone:

| | How it resolves now |
| --- | --- |
| Agent bind address | `bind_ip: "auto"` — asks the routing table which local address reaches the controller |
| If the lease moves | the agent notices within 30 s, exits, and the service restart policy rebinds it |
| Allowlist | the controller's `/24`, so the controller may move too |
| Firewall rule | scoped by remote address + port + program, **not** local address |
| Controller → laptop | [`find-agent.ps1`](../scripts/find-agent.ps1): last-known → Tailscale → ARP → `/24` sweep |

Tailscale is used **only for discovery** — `tailscale ping` reports the peer's real LAN endpoint,
which keeps working through DHCP changes. The control traffic itself is plain LAN.

Widening the source filter to the LAN is safe because it was never the security boundary — the
bearer token is.

---

## One-time setup

### On the controller

```powershell
.\scripts\make-testd-kit.ps1
```

Builds `evorift-testd`, assembles `dist/testd-kit/`, and zips it. Pass `-ControllerIp` if
auto-detection picks the wrong interface.

Copy the **`testd-kit` folder** (not the zip) to a USB stick — the laptop writes the token back
into it.

### On the laptop

Double-click, in order, clicking **Yes** on each UAC prompt:

| File | When |
| --- | --- |
| **`INSTALL.bat`** | always — installs the agent, registers the service, opens one firewall rule |
| **`CHECK.bat`** | when the controller cannot reach the agent (read-only; writes `check-report.txt`) |
| **`FIX-DNS.bat`** | if the laptop can ping `1.1.1.1` but cannot resolve names |

`INSTALL.bat` verifies it can reach the controller **before** installing anything, so it stops
cleanly rather than half-installing.

It prints a 64-character token once and saves it to `testd.token` beside itself. Carry that stick
back; `REMOTE-TEST.bat` finds the token automatically.

### Pairing check

Back on the controller, double-click `REMOTE-TEST.bat` → press `1`. Expect `"ok": true`.

- `403` — the laptop does not trust this desktop's address; re-run `INSTALL.bat`
- `401` — wrong token
- no answer — the agent is not running; run `CHECK.bat` on the laptop

---

## The agent

`evorift-testd`, a Windows service running as LocalSystem. Source: [`src-tauri/src/testd/`](../src-tauri/src/testd/).

| Endpoint | Token? | Purpose |
| --- | --- | --- |
| `GET /health` | no | heartbeat; **resets the deadman**; reports fire count |
| `POST /push` | yes | upload a file, streamed to disk, SHA-256 returned |
| `POST /run` | yes | start a command; returns a job id immediately |
| `GET /job/<id>` | yes | status read off disk; `?tail=N` for output |
| `GET /pull` | yes | download a file from the sandbox |
| `POST /recover` | yes | run the recovery routine now |

### Security

- **Binds one LAN address**, refuses `0.0.0.0` and any publicly routable address.
- **Source-IP allowlist** — necessary, deliberately **not** sufficient: a LAN peer can spoof an
  address. The bearer token, compared in constant time, is what authenticates.
- **Sandbox**: `C:\evorift-test\` only. Traversal, absolute/UNC paths, drive-relative (`C:foo`),
  alternate data streams, DOS device names and trailing-dot names are all rejected; containment is
  decided by canonicalising both sides, so a junction pointing out of the sandbox fails too.
- **Token and audit log are unreachable over the network** — they live in
  `C:\ProgramData\evorift-testd\`, and the agent refuses to start if that sits inside the sandbox.
- **`/run` only executes programs inside the sandbox.** System tools must be wrapped in a `.ps1`
  and pushed, which also puts the exact text of what ran next to its output.
- **Append-only audit log** of every request including rejections, with control characters
  sanitised so a hostile request cannot forge a record.

Residual risk, stated plainly: `/health` takes no token, so a LAN peer spoofing the controller's
address could hold the deadman open. It cannot read, write or run anything.

### Deadman switch

Armed by the first `/health`. Fires after `deadman_secs` (120) of silence, and **stands down after
3 consecutive fires**, re-arming on the next heartbeat.

That last part matters: recovery is idempotent, but every pass resets DNS and flushes the resolver
cache. Left firing forever, an idle agent degraded the laptop until it could no longer resolve
names — the agent was damaging the machine it was measuring. 37 fires in one afternoon.

> `remote.ps1` sends a heartbeat on **every** poll of a running job. Only `/health` resets the
> deadman, so without that a long capture would look like a vanished controller and every run would
> report a spurious fire.

---

## When it breaks

**Start here:** on the laptop, double-click **`CHECK.bat`**. It reports installed files, service
state, whether anything is listening, the firewall rule, current addresses, `startup-error.log`,
the audit log tail, and ends with a plain-language verdict. Read the verdict, but trust the
evidence above it.

### The laptop is unreachable and the deadman did not save it

Physically at the laptop, elevated PowerShell:

```powershell
taskkill /f /im evorift.exe; taskkill /f /im winws.exe
```

Usually enough. If not:

```powershell
foreach ($s in @("WinDivert","WinDivert1.4","WinDivert1.1")) { sc.exe stop $s; Start-Sleep -Seconds 1; sc.exe delete $s }
Get-NetAdapter -Physical | ForEach-Object { Set-DnsClientServerAddress -InterfaceIndex $_.InterfaceIndex -ResetServerAddresses -Confirm:$false }
Clear-DnsClientCache
```

Then confirm it actually came back, rather than assuming:

```powershell
Test-NetConnection -ComputerName 1.1.1.1 -InformationLevel Detailed
```

Still dead after a clean reboot? A WinDivert service is loading at start-up: boot into Safe Mode
(Shift + Restart → Troubleshoot → Advanced options → Startup Settings → Restart → `4`), run the
`sc.exe delete` loop, reboot.

### Two logs that never leave the laptop

```powershell
Get-Content "$env:ProgramData\evorift-testd\audit.log" -Tail 50
Get-Content "$env:ProgramData\evorift-testd\startup-error.log" -Tail 20
```

`startup-error.log` exists because a failure before the audit log opened was previously invisible:
the service reported `Running` with nothing listening, which looks identical to a firewall problem
from outside.

| Symptom | Cause |
| --- | --- |
| service `Running`, nothing listening, `startup-error.log` has a line | read that line — it is the answer |
| `deadman.log` empty, service not running | the agent died; `sc.exe qfailure evorift-testd` |
| `deadman.log` empty, service running | never armed — no `/health` ever arrived |
| fires logged, `restore DNS` step failed | the agent is not elevated; it must run as LocalSystem |
| fires logged, all steps ok, still no network | something else holds the link — see below |

### Known gap

Recovery does **not** stop the `EvoriftSvc` service. If that is installed on the laptop and
respawns the engine, the deadman can fire repeatedly without the machine coming back. The recovery
report records the state it measured, so you will see it. Either uninstall `EvoriftSvc` on the test
laptop or add a `sc.exe stop EvoriftSvc` step to `recovery::run`.

---

## Files

| Path | Runs on | What it is |
| --- | --- | --- |
| `REMOTE-TEST.bat` | controller | **the launcher** — double-click this |
| `scripts/make-testd-kit.ps1` | controller | builds the kit for the USB stick |
| `scripts/remote.ps1` | controller | the endpoint wrapper; actions incl. `cycle`, `engine` |
| `scripts/find-agent.ps1` | controller | locates the laptop |
| `scripts/run-engine-test.ps1` | laptop | the real test, as one local job |
| `scripts/capture-state.ps1` | laptop | the 7 state probes |
| `scripts/testd-install.ps1` | laptop | the installer (`INSTALL.bat` wraps it) |
| `scripts/check.ps1` | laptop | diagnostic (`CHECK.bat` wraps it) |
| `scripts/fix-dns.ps1` | laptop | DNS repair (`FIX-DNS.bat` wraps it) |
| `scripts/link-setup.ps1` | either | link diagnosis, direct cable, hotspot, Tailscale, revert |
| `src-tauri/src/testd/` | laptop | the agent |
| `C:\evorift-test\` | laptop | sandbox — everything `/push`, `/pull` and `/run` can touch |
| `C:\ProgramData\evorift-testd\` | laptop | config, token, audit + deadman + startup logs |

---

## Traps worth remembering

Each of these cost real time, and each is now guarded in code:

- **`Out-File -Encoding utf8` writes a BOM** in PowerShell 5.1, which makes a JSON config
  unparseable. Use `[System.IO.File]::WriteAllText` with `UTF8Encoding($false)`. The agent strips a
  leading BOM defensively, and the installer verifies the bytes it wrote.
- **`.ps1` files must be pure ASCII** unless saved with a BOM. A UTF-8 em-dash in a BOM-less script
  is read as CP1252, and its third byte is a smart quote that silently breaks string parsing.
- **`canonicalize()` returns `\\?\` verbatim paths.** Never hand one to a child process: forward
  slashes stop resolving and `Join-Path` throws. Use `sandbox::strip_verbatim` — canonical for the
  containment check, plain for the child.
- **`sc.exe create` fails with 1639** when PowerShell mangles a quoted `binPath=`. Use `New-Service`.
- **PowerShell variables are case-insensitive and visible to called functions.** A local `$base`
  silently overwrites a script-level `$Base` inside everything it calls.
- **A script that writes nothing can still exit 0.** `capture-state.ps1` now fails loudly if it
  cannot create its output directory.
