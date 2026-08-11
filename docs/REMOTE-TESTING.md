# Remote testing — pushing builds to the test laptop

How to run evorift verification on a second Windows laptop on your LAN without touching it.

**The constraint everything here is built around:** evorift has been observed to cut *all*
internet on its host until the process is killed (see [DIAGNOSE-INTERNET-CUT.md](DIAGNOSE-INTERNET-CUT.md)).
The test agent therefore loses contact with your controller at exactly the moment a test fails.
That is not designed around — it is designed *for*:

- **Deadman switch.** If no `/health` arrives for 120 s, the laptop recovers *itself*: kills
  `evorift.exe` and `winws.exe`, stops and deletes stale `WinDivert` / `WinDivert1.4` /
  `WinDivert1.1` services, and puts DNS back on DHCP. Every fire is logged.
- **Capture to disk first.** Command output is written to files on the laptop the instant it is
  produced. Nothing waits in memory for an HTTP response that may never be deliverable. You
  fetch the files afterwards, possibly minutes later.

---

## 1. Getting the two machines onto one subnet

Everything else depends on this, and it is where the setup most often stalls: if the desktop is
on a cable and the laptop is on Wi-Fi, the router frequently does **not** bridge them into one
subnet, so no amount of firewall fiddling will make them see each other.

### Diagnose first

`scripts/link-setup.ps1` reads every adapter, its subnet, its gateway and its firewall profile,
and tells you which of those is actually the problem. Run it on **either** machine:

```powershell
.\scripts\link-setup.ps1 -Action diagnose -PeerIp 192.168.1.25
```

It distinguishes the three things that look identical from the outside:

| What it reports                              | What it means                                        |
| -------------------------------------------- | ---------------------------------------------------- |
| `NOT in the same subnet as any interface`    | the router is not bridging cable and Wi-Fi — §1.1/§1.2     |
| networks marked `PUBLIC`                     | Windows is blocking inbound; same subnet, still dead  |
| same subnet, ping ok, TCP 8765 closed        | normal before the agent is installed                  |

### 1.1 Direct cable — the recommended method

Plug the network cable **straight between the two machines**. No router, no switch — modern
NICs auto-negotiate the crossover. Then, elevated on each:

```powershell
# on the desktop
.\scripts\link-setup.ps1 -Action direct-cable -Role controller

# on the laptop
.\link-setup.ps1 -Action direct-cable -Role agent
```

| Machine         | Role       | Direct-link IP  | Internet via |
| --------------- | ---------- | --------------- | ------------ |
| your desktop    | controller | `192.168.77.1`  | Wi-Fi        |
| the test laptop | agent      | `192.168.77.2`  | Wi-Fi        |

**No default gateway is assigned to the link** — that is deliberate and is what keeps the cable
from hijacking the internet route. Both machines keep using Wi-Fi for internet; the cable
carries only the control channel. The script verifies after applying that your default route
still points somewhere else, and says so if it does not.

**Why this is worth a cable rather than just fixing the router:** evorift kills the internet
path. If the control link rides that same interface, you lose the laptop at the exact moment the
test becomes interesting. On a separate physical interface it has a good chance of staying up,
so you can watch the failure live instead of reconstructing it from captures. The deadman switch
still covers you if it does not.

Every change is journaled to `C:\ProgramData\evorift-testd\link-rollback.json` before it is
applied, and `-Action revert` puts the adapter back on DHCP.

### 1.2 Tailscale

Tailscale solves the addressing problem outright: both machines get a stable `100.x` address on
a `/32`, reachable regardless of which router, SSID or subnet they are on, with no router
configuration at all. Check it end to end with:

```powershell
.\scripts\link-setup.ps1 -Action tailscale
```

That reports the backend state, this machine's tailnet address, every peer, whether Tailscale's
own `ShieldsUp` is blocking inbound, and — the one that actually decides whether this can work —
whether a listener can **bind** the tailnet address.

| Machine         | Role       | Tailnet IP        |
| --------------- | ---------- | ----------------- |
| your desktop    | controller | `100.120.14.56`   |
| the test laptop | agent      | `100.103.86.68`   |

Then build the kit pinned to the tailnet and install as usual:

```powershell
.\scripts\make-testd-kit.ps1 -ControllerIp 100.120.14.56
```

The installer detects that the controller is a tailnet address and reads this machine's own
address from `tailscale ip -4` rather than from the OS address table — subnet matching cannot
work here, because two `/32` addresses never share a subnet.

> **The gotcha, and it is a hard blocker.** On Windows, the tailnet address is often absent from
> `Get-NetIPAddress`, `ipconfig` and `netsh` even while the tunnel is fully up and routing. If
> tailscaled is in **userspace-networking mode**, the address is never placed on an OS interface,
> so outbound traffic works but **no process can listen on it** — the agent cannot bind, and the
> service will not start. This is the state your desktop was in when this was written:
> `tailscale ip -4` reported `100.120.14.56`, and binding it failed with *"The requested address
> is not valid in its context"*.
>
> Both `link-setup.ps1 -Action tailscale` and the installer test this by actually binding a
> socket, so you find out in one line instead of debugging a service that will not start. If it
> reports NOT bindable: restart the Tailscale service elevated, run `tailscale up` again, and
> confirm `Get-NetIPAddress -AddressFamily IPv4` now lists the `100.x` address. If it never
> appears there, Tailscale cannot host the agent on this machine and you need §1.1 or §1.3.

**The trade-off, stated plainly.** Tailscale rides the internet, and evorift cuts the internet.
Because your cable and Wi-Fi do not route to each other, Tailscale cannot fall back to a direct
LAN path either — it relays over the internet, which is exactly what dies. So when a test fails
you lose the control link with it, and **every run depends on the deadman switch** to bring the
laptop back. That works, it is what the deadman is for, but you recover blind instead of
watching the failure happen. The direct cable in §1.1 does not have this problem. Having both is
reasonable: Tailscale for convenience, the cable for the runs that matter.

### 1.3 If neither machine has a spare Ethernet port

```powershell
.\scripts\link-setup.ps1 -Action hotspot
```

Guided setup for Windows Mobile Hotspot — both machines end up on `192.168.137.x`, host at
`.1`. (Mobile Hotspot has no supported PowerShell 5.1 API, so the script walks you through the
UI and then verifies the result.) A phone hotspot both machines join works equally well.

Caveat, stated plainly: with either of these the control link and the internet ride the same
adapter, so evorift cuts both, and **every run will depend on the deadman firing**. Workable,
just slower to iterate on.

### 1.4 If they are already on one subnet

Then nothing above is needed. Get each machine's address with:

```powershell
Get-NetRoute -DestinationPrefix '0.0.0.0/0' | Sort-Object RouteMetric | Select-Object -First 1 | ForEach-Object { Get-NetIPAddress -AddressFamily IPv4 -InterfaceIndex $_.InterfaceIndex } | Select-Object IPAddress, InterfaceAlias
```

| Machine         | Role       | LAN IP          |
| --------------- | ---------- | --------------- |
| your desktop    | controller | `192.168.1.24`  |
| the test laptop | agent      | `192.168.1.25`  |

> **DHCP will eventually move these addresses.** Reserve both in your router's DHCP settings, or
> re-run the installer when the laptop's address changes. A stale `bind_ip` makes the agent fail
> to start; a stale allowlist entry makes it 403 you. The direct-cable method does not have this
> problem — those addresses are static.

The agent accepts any of these ranges as a valid bind address: `192.168/16`, `10/8`,
`172.16/12`, `169.254/16` (APIPA), `100.64/10` (CGNAT/Tailscale) and loopback. It refuses
`0.0.0.0` and any publicly routable address.

---

## 2. One-time setup — build the kit, carry it over, run one command

### 2a. On the controller: build the kit

```powershell
.\scripts\make-testd-kit.ps1 -ControllerIp 192.168.77.1
```

(Use the address the laptop will reach you on — `192.168.77.1` if you took the direct-cable
route in §1.1, the tailnet address if §1.2, otherwise your LAN address. It is auto-detected if you omit the flag, which picks
your default-route address — right for a shared subnet, wrong for a direct cable.)

That compiles `evorift-testd`, assembles everything the laptop needs and zips it:

```
dist/testd-kit/
  install.ps1          <- the one command to run on the laptop; controller address baked in
  testd-install.ps1    the real installer
  link-setup.ps1       IP diagnosis / direct-cable / revert
  evorift-testd.exe    the agent
  capture-state.ps1    the capture the agent runs remotely
  README.txt           the whole procedure, standalone
  MANIFEST.txt         SHA-256 of every file
dist/testd-kit.zip
```

The kit is the deliverable: **the laptop needs no repo, no Rust toolchain and no internet.**
That matters, because the laptop is the machine whose internet the software under test breaks.

### 2b. Copy the kit to the laptop

USB stick, file share, anything. Nothing in it needs network access. Verify it arrived intact
against `MANIFEST.txt`:

```powershell
Get-FileHash .\evorift-testd.exe -Algorithm SHA256
```

### 2c. On the laptop: one command, elevated

```powershell
.\install.ps1
```

It checks that this laptop can actually reach the controller **before** installing anything
(the most common way the setup silently fails), then:

1. installs the binary to `C:\Program Files\evorift-testd\`;
2. generates a 256-bit token and locks its ACL to SYSTEM + Administrators;
3. writes `C:\ProgramData\evorift-testd\testd.config.json`;
4. registers the `evorift-testd` service (LocalSystem, auto-start, auto-restart on failure);
5. opens **one** inbound firewall rule: that port, from that one source IP, to that one local
   address, scoped to that one program;
6. starts the service and verifies something is actually listening;
7. places `capture-state.ps1` in the sandbox so the first remote run has it.

If the link check fails it stops and points you at `link-setup.ps1` rather than installing an
agent nobody can reach. Override with `-ControllerIp`, `-BindIp`, or `-SkipLinkCheck`.

`testd-install.ps1` can also be driven directly for the less common knobs: `-Port`,
`-SandboxRoot`, `-DeadmanSecs`, `-RotateToken`.

> **Which interface gets bound.** The installer picks the local address that shares a subnet
> with the controller — *not* the one holding the default route. With the direct-cable setup
> those are deliberately different, and binding the default-route interface would put the agent
> on the Wi-Fi side where the controller cannot reach it. If nothing shares a subnet with the
> controller, it warns loudly instead of quietly binding the wrong one.

### 2d. Carry the token to the controller — by hand

The installer prints the token once, at the end. **This is the one genuinely manual step and it
is deliberate:** the token is never sent over the API, never written to the audit log, and never
included in a `/pull` response. Type it into the controller yourself.

On the **controller**:

```powershell
$env:EVORIFT_TESTD_TOKEN = '<the 64-character token>'
```

To make it persist across shells:

```powershell
[Environment]::SetEnvironmentVariable('EVORIFT_TESTD_TOKEN', '<token>', 'User')
```

### 2e. Verify the pairing

From the **controller**:

```powershell
.\scripts\remote.ps1 -AgentIp 192.168.77.2 -Action health
```

You should get JSON with `"ok": true` and `"fires": 0`. If you get a connection failure, see
§6. If you get HTTP 403, the laptop does not have your address in its allowlist — re-run the
installer with the right `-ControllerIp`. HTTP 401 means the token does not match.

Setup is done. Everything below is routine.

---

## 3. The full remote-run procedure

### One command

```powershell
.\scripts\remote.ps1 -AgentIp 192.168.77.2 -Action cycle -BuildPath .\src-tauri\target\release\evorift.exe
```

That performs the whole cycle:

| Step | What happens                                                             |
| ---- | ------------------------------------------------------------------------ |
| 0    | baseline `/health` — **records the deadman fire count before the run**    |
| 1    | pushes `capture-state.ps1` and a listing helper into the sandbox          |
| 2    | pushes the build to `build/` in the sandbox                              |
| 3    | runs `capture-state.ps1 -Label before`                                    |
| 4    | runs the verification steps (the link is expected to die here)            |
| 5    | runs `capture-state.ps1 -Label after`                                     |
| 6    | pulls `docs/captures/**` and any `recovery/**` reports back               |
| 7    | reads `/health` again and **compares the deadman fire count**             |

Artefacts land in `docs\captures\remote-<timestamp>\`, with a `remote-run.json` summary.

### Reading the result

The last line is the one that matters:

- `ok  the deadman did not fire` — the captures reflect an uninterrupted run. Trust them.
- `FAIL  THE DEADMAN FIRED n TIME(S)` — the laptop recovered itself partway through: it killed
  `evorift.exe`/`winws.exe`, cleared WinDivert and reset DNS mid-test. **Any "it worked" reading
  from those captures is not trustworthy.** The recovery reports pulled into
  `remote-<timestamp>\recovery\` say which step fired and what it found.

### Individual actions

```powershell
# heartbeat + deadman state
.\scripts\remote.ps1 -AgentIp 192.168.77.2 -Action health

# upload a file into the sandbox
.\scripts\remote.ps1 -AgentIp 192.168.77.2 -Action push -Path .\build\evorift.exe -RemotePath build/evorift.exe

# start a script; returns a job id immediately
.\scripts\remote.ps1 -AgentIp 192.168.77.2 -Action run -Script scripts/capture-state.ps1 -ScriptArgs @('-Label','during')

# read a job's status and the tail of its output
.\scripts\remote.ps1 -AgentIp 192.168.77.2 -Action job -JobId <id>

# download one file
.\scripts\remote.ps1 -AgentIp 192.168.77.2 -Action pull -Path docs/captures/20260811-2210-during/01-network-config.txt

# fire the recovery routine on demand
.\scripts\remote.ps1 -AgentIp 192.168.77.2 -Action recover
```

---

## 4. What the agent will and will not do

### Endpoints

| Endpoint          | Token? | Purpose                                                      |
| ----------------- | ------ | ------------------------------------------------------------ |
| `GET /health`     | no     | heartbeat; **resets the deadman**; reports fire count         |
| `POST /push`      | yes    | upload a file, streamed straight to disk, SHA-256 returned    |
| `POST /run`       | yes    | start a command; returns a job id immediately                 |
| `GET /job/<id>`   | yes    | job status read off disk; `?tail=N` for output                |
| `GET /pull`       | yes    | download a file from the sandbox                              |
| `POST /recover`   | yes    | run the recovery routine now                                  |

### `/run` only executes things inside the sandbox

Both job kinds resolve their program through the sandbox root, so `/run` cannot launch an
arbitrary binary from elsewhere on the laptop. To use a system tool (`netsh`, `ipconfig`,
`sc.exe`, …), wrap it in a `.ps1`, push that, and run it. Beyond the security argument, this
means the exact text of what ran ends up on disk next to its output — which is what you want
when you are reading a capture three days later.

Arguments are passed as separate argv entries, never concatenated into a shell string.

### Boundaries

- **Sandbox:** `C:\evorift-test\` and nothing else. Traversal (`..`), absolute paths, UNC paths,
  drive-relative paths (`C:foo`), NTFS alternate data streams (`file:stream`), DOS device names
  (`NUL`, `COM1`, …) and trailing-dot names are all rejected. Containment is decided by
  canonicalising both sides, so a junction or symlink planted inside the sandbox that points out
  of it fails too.
- **The token and the audit log are unreachable over the network.** They live in
  `C:\ProgramData\evorift-testd\`, and the agent *refuses to start* if that directory is inside
  the sandbox root. There is no code path that serves a file from outside the sandbox.
- **Bind address:** one LAN address. The agent refuses to start if it would bind `0.0.0.0`, and
  also refuses a publicly routable address.

### The allowlist is necessary, not sufficient

A host on your LAN can put your controller's address in a packet. The source-IP allowlist is a
filter, not authentication — the bearer token is what actually authenticates, and it is compared
in constant time on every endpoint except `/health`.

The residual consequence, stated plainly: **`/health` takes no token**, so a spoofing LAN peer
could keep the deadman from firing by sending heartbeats. It cannot read, write, or run anything.
If that matters on your network, drop `deadman_secs` and treat the fire count in `remote-run.json`
as the authoritative record of what happened.

### Audit log

Every request — including every rejection — appends one line to
`C:\ProgramData\evorift-testd\audit.log`: timestamp, source IP, method, endpoint, status,
command, exit code. Read it by sitting at the laptop:

```powershell
Get-Content "$env:ProgramData\evorift-testd\audit.log" -Tail 50
```

Newlines and control characters in any field are sanitised, so a hostile request cannot forge a
second record.

---

## 5. Known gap: `EvoriftSvc` is not stopped by recovery

The recovery routine kills `evorift.exe` and `winws.exe` exactly as specified. It does **not**
stop the `EvoriftSvc` service, because stopping a service the operator did not ask about is a
bigger action than the brief called for. If `EvoriftSvc` is installed on the laptop and respawns
the engine, the deadman can fire repeatedly without the machine actually coming back.

The recovery report records the state it measured, so you will see it. If you hit this, either
uninstall `EvoriftSvc` on the test laptop or add a `sc.exe stop EvoriftSvc` step to
`recovery::run` in `src-tauri/src/testd/recovery.rs`.

---

## 6. Recovering the laptop when it is offline **and** the deadman failed

This is the physical fallback. You need it when `/health` times out for well over
`deadman_secs`, `remote.ps1` cannot reach the agent at all, and the laptop has not come back on
its own.

**Go to the laptop.** Everything below is typed on its keyboard.

### 6a. Fastest fix — kill the offender

Open an elevated PowerShell (Win+X → *Terminal (Admin)*). If Windows itself is responsive, this
is usually all it takes:

```powershell
taskkill /f /im evorift.exe; taskkill /f /im winws.exe
```

Internet normally returns within a few seconds. If it does, skip to §6d.

### 6b. Full manual recovery — the same steps the deadman would have run

```powershell
taskkill /f /im evorift.exe
taskkill /f /im winws.exe
foreach ($s in @("WinDivert","WinDivert1.4","WinDivert1.1")) { sc.exe stop $s; Start-Sleep -Seconds 1; sc.exe delete $s }
Get-NetAdapter -Physical | ForEach-Object { Set-DnsClientServerAddress -InterfaceIndex $_.InterfaceIndex -ResetServerAddresses -Confirm:$false }
Clear-DnsClientCache
```

Then confirm the network is actually back — measured, not assumed:

```powershell
Test-NetConnection -ComputerName 1.1.1.1 -InformationLevel Detailed
Resolve-DnsName discord.com -Type A
```

### 6c. If the machine will not respond at all

Hold the power button for 10 seconds, then boot. WinDivert is a kernel driver, so a hard power
cycle is safe from the agent's point of view: nothing it writes is left half-applied across a
reboot, and the service comes back on its own at start-up.

If the network is *still* dead after a clean boot, a WinDivert service is loading at start-up.
Boot into Safe Mode (hold Shift while clicking Restart → *Troubleshoot* → *Advanced options* →
*Startup Settings* → *Restart* → `4`), then run the `sc.exe delete` loop from §6b and reboot
normally.

### 6d. Work out why the deadman did not save you

While you are at the laptop, read the two logs that never leave it:

```powershell
Get-Content "$env:ProgramData\evorift-testd\deadman.log" -Tail 20
Get-Content "$env:ProgramData\evorift-testd\audit.log" -Tail 50
Get-Service evorift-testd
```

The usual causes, in the order worth checking:

| Symptom in the logs                        | Cause                                                          |
| ------------------------------------------ | -------------------------------------------------------------- |
| `deadman.log` empty, service not running   | the agent died; check the restart actions with `sc.exe qfailure evorift-testd` |
| `deadman.log` empty, service running       | the switch was never armed — no `/health` ever arrived, so the agent had no reason to think a test was in progress |
| fires logged, `restore DNS` step failed    | the agent is not elevated; it must run as LocalSystem           |
| fires logged, all steps ok, still no net   | something other than evorift/winws is holding the link — see §5 (`EvoriftSvc`) |

### 6e. Restart the agent

```powershell
Restart-Service evorift-testd
Get-NetTCPConnection -State Listen -LocalPort 8765
```

Then, from the controller, `-Action health` should answer again.

---

## 7. Files

| Path                                   | What it is                                        |
| -------------------------------------- | ------------------------------------------------- |
| `src-tauri/src/testd/`                 | the agent (config, token, sandbox, http, jobs, recovery, server) |
| `src-tauri/src/bin/evorift-testd.rs`   | the binary — service mode + `--console` mode      |
| `scripts/make-testd-kit.ps1`           | **controller:** build the kit to carry to the laptop |
| `scripts/remote.ps1`                   | **controller:** drive a remote run                |
| `scripts/link-setup.ps1`               | **either:** IP diagnosis, direct cable, hotspot, revert |
| `scripts/testd-install.ps1`            | **laptop:** the installer (the kit's `install.ps1` wraps it) |
| `scripts/capture-state.ps1`            | the state capture, run on the laptop by the agent |
| `dist/testd-kit/`, `dist/testd-kit.zip`| the built kit — this is what goes on the USB stick |
| `C:\evorift-test\`                     | sandbox on the laptop (everything `/push`/`/pull`/`/run` can touch) |
| `C:\ProgramData\evorift-testd\`        | config, token, audit log, deadman log, link rollback journal — never served over the network |

The WinDivert teardown in `recovery::run` calls `engine::clear_stale_windivert` — evorift's own
implementation, not a copy. If you add a WinDivert version there, update `WINDIVERT_SERVICES` in
`recovery.rs` too, or the recovery report will claim success while leaving a service behind.
There is a unit test pinning both lists.
