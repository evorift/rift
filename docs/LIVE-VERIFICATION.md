# LIVE-VERIFICATION.md

Live-machine verification log for evorift v2. Per the `evorift-live-verification` skill:
a unit test verifies the code's own claim, this log verifies **reality**. No phase counts
as "done" without an entry here. The user runs the actual commands; Claude prepares them
and interprets the result. Failed runs are recorded too — they're the most valuable data
point there is.

**Corrected 2026-08-14 — this line was stale.** It used to say "empty today, no v2 code
exists yet" — still true for **v2** (V0 hasn't started), but this file now holds a real
**v1** run below (the P0-a/b/c validation pass), which is exactly what it's for: this log
isn't v2-exclusive, it's whatever gets live-verified, on whichever version. The next *v2*
entry lands once V0-V1 produce a strategy engine with observable behavior; the run below
is unrelated to that milestone.

## Run log

| Date | Phase | ISP | Version | Baseline | Result | Live-tweak result | Pass/Fail | Notes |
|---|---|---|---|---|---|---|---|---|
| 2026-08-13 | P0-a/b/c validation (pre-V0, v1) | not recorded | 0.1.3 | Not cleanly established — see `docs/HYPOTHESES-INTERNET-CUT.md` "known unknowns" | **FAIL** — turning protection on cut ALL internet (not just target domains); Discord never opened | Not reached — run aborted at the outage | **FAIL** | Recovery only on closing the app (process-bound, no manual cleanup needed). Root cause not yet determined — see `docs/HYPOTHESES-INTERNET-CUT.md` for the ranked candidate list and `docs/DIAGNOSE-INTERNET-CUT.md`/`scripts/capture-state.ps1` for the capture procedure still to be run. |
| 2026-08-15 | MVP UI freeze + DNS root-cause (0.1.5) | user's home network | 0.1.4→0.1.5 | Protection stuck on "Bağlanıyor/test edilmedi" in every mode; ISP resolver answers Discord with sinkhole 195.175.254.2 | **PASS** — freeze fixed (start 1038ms, mode switch 1898ms, verify settles); DNS now actually applied → **hafif verified in 31ms, güçlü verified in 19ms** | Not run this pass | **PASS** | Two independent causes: (1) DPI-only started a WARP tunnel under the engine lock with no child timeout → whole service froze; (2) `Command::Start` reported `dns=cloudflare` without applying it, so Discord resolved to a sinkhole and no strategy could work. Repeats 6/8 still UNMEASURED — DNS masked all four configs. Single line, single run. |
| 2026-08-13 | Task 3 acceptance measurements, post Task-1/2 honesty fixes (commits `9dce8b3`,`f61d3be`,`edd6ac9`,`4b5d80f`,`ac123f5`) | user's home network (`sweet home 5`, Public profile) | 0.1.3 | Protection OFF: `discord.com`/`gateway.discord.gg` TLS handshake genuinely times out (~6.2-6.3s); `cdn.discordapp.com` reachable | **PASS (a)** Discord desktop logs in fully under DPI-only, not stuck on "Connecting…". **(b)** toggle gap 212–1743ms (see below). **(c)** confirmed minimum `dpi-desync-repeats`=1, 9/9 handshakes held | Not run this pass | **PASS** (a), data recorded (b)(c) | Single run, one network — not yet confirmed on a second ISP/time per this file's own interpretation rules. Test 2's *first* run falsely reported "no repeats value works" — root cause was a bug in the test script itself (see below), not the product. |

## 2026-08-15 (night) — instability root-caused; hard domains opened; Güçlü retuned (0.1.7)

User report: Discord and some blocked sites worked in Güçlü, but not reliably — the UI kept
flipping to "Uygulandı ama çalışmıyor". Measured with a purpose-built harness (N attempts per
target over a fixed window, `winws` PID sampled every second so a restart can't be mistaken for a
bypass failure), against ISP-blocked targets — unblocked hosts prove nothing about a bypass.

### Three independent causes of the flapping

1. **The engine restarted every 5 seconds.** `set_exclusion` kills the winws child whenever the
   excluded-port set changes, and that set is built from the LIVE SOURCE PORTS of every app in
   "off" mode — which churn constantly. With 39 app modes synced (new apps default to "off"), the
   watchdog tore the engine down and rebuilt it on a 5s beat, killing every in-flight connection.
   Exclusion-driven restarts are now rate-limited to once a minute; crash-respawn stays immediate.
2. **DNS silently reverted.** The service recorded the DNS it applied and never re-read the
   adapters. Caught red-handed: service reporting `dns=cloudflare` while the machine was on
   `192.168.1.1` with `discord.com → 195.175.254.2`. On a sinkholing line that is fatal but nearly
   invisible — the engine and strategy are fine and every connection still dies at TCP. Now
   re-checked and re-applied when the verify probe reports Broken.
3. **The service wedged on tunnel work.** The watchdog held the engine mutex while running
   `wireguard.exe` (two calls, each bounded at 25s), so a mode switch failed with
   "servis 45s içinde yanıt vermedi". WARP reconciliation moved outside the lock; child timeout
   cut to 10s so two calls stay well under the IPC ceiling.

Self-inflicted, recorded for honesty: the first attempt at fix (2) called `engine.lock()` while the
watchdog already held the guard. `std::sync::Mutex` is not reentrant, so the watchdog deadlocked
against itself on its first tick and wedged the service 5s after every start — every command,
including `status`, timed out. Fixed by scoping the guard.

### Result: flapping → deterministic

| Target | Before (guclu/c1) | After fixes (guclu/c1) |
|---|---|---|
| xvideos.com | — | **100/100** |
| discord.com | — | **100/100** |
| pornhub.com | — | 0/23 (`tls timeout`) |
| brazzers.com | — | 0/23 (`tls timeout`) |

`winws` PID: **no changes**, failures spread evenly across all 5s buckets. So this was no longer
instability — two domains passed 100% and two failed 100%, which is a tuning problem, not a race.

### Strategy sweep: it was never the repeat count

14 configs, 8 attempts per target, with a known-good control target so a config that simply breaks
everything can't look like progress:

| Config | Hard domains | Control |
|---|---|---|
| c1 r8 / r11 / r20 | 0% | 100% |
| fake, fake r20 | 0% | 100% |
| superonline | 0% | 100% |
| multidisorder, multidisorder r11, tt, tt-alt, superonline-alt, kablonet | 0% | **0%** (worse) |
| **turkcell-hotspot** | **100% (16/16)** | **100%** |

Every c1 repeat count failed identically, which rules out repeat-count tuning as the answer — the
difference is the DESYNC METHOD: `fake + ttl 1 + autottl 3` (TTL-based; the fake packet expires
before the real server but the DPI box still sees it).

### Güçlü Koruma retuned + verified as shipped

`GUCLU_STRATEGY = "turkcell-hotspot"`, no repeats override (the measured run used the preset's own
value). Verified by selecting the mode the way a user does — `protmode guclu`, nothing else — so
this tests the shipped code path, not a hand-applied config:

```
PROTMODE OK -> guclu (strategy=turkcell-hotspot)   verify=verified
pornhub.com   100/100  ort 201ms
brazzers.com  100/100  ort 201ms
xvideos.com   100/100  ort 216ms
discord.com   100/100  ort 184ms
TOPLAM        400/400 = 100%     winws restarts: 0, errors per 5s bucket: 0
```

**Caveats.** ONE line, one session. The winning preset names an ISP because that is where it was
derived — another line may need a different chain, and choosing it per-line is precisely what
Autopilot is for. HAFIF_REPEATS (6) remains unmeasured; the sweep only tested the catch-all path.
VPN/WARP still unfixed and untested. Also: the test agent's deadman fired 21 times today and its
recovery kills winws AND resets DNS, so part of the originally reported instability was the harness
rather than the product — its window was widened from 120s to 3600s for manual testing.

## 2026-08-15 (evening) — the INSTALLED build verified end to end (0.1.6)

Everything below the previous entry was tested by running `evorift-svc.exe --console` out of the
sandbox. **That bypassed the installer**, so it proved the code worked while saying nothing about
what was actually shipped — and what was shipped was broken in a way no amount of code fixing could
reach.

### The real defect: the installer shipped a ten-week-old service binary

`resources/evorift-svc.exe` is what the installer and portable zip package — NOT
`target/release/evorift-svc.exe`. It had been the **26 June binary since 26 June**:

```
BUILT:    target/release/evorift-svc.exe    2,541,568 B   15.08.2026
BUNDLED:  resources/evorift-svc.exe         1,213,952 B   26.06.2026   <- stale
```

`beforeBundleCommand` was supposed to refresh it. It was an inline
`powershell -Command "Copy-Item ... -Force"` which the build log shows merely **echoing** the
command text rather than executing it (quoting mangled), and which could not have failed the build
anyway: `Copy-Item` raises a NON-terminating error, so PowerShell exits 0 and Tauri bundles
whatever stale file is present. Every shipped build was therefore **new UI + June service**, and
the service is what runs the engine. That reproduces the original symptoms exactly: no
`SetProtectionMode` (mode switch fails), no verify state machine ("test edilmedi" forever), old
WARP path (VPN fails), boot auto-protect back. Confirmed on the laptop: the running service logged
`auto-start ok (boot koruması açık)` — a line that exists **only** in the old code.

Fixed: `beforeBundleCommand` is now `scripts/stage-svc.ps1` — `ErrorActionPreference=Stop`, a retry
for file locks, and a **SHA-256 comparison that fails the build** if the staged bytes don't match.
It proved itself immediately: the first 0.1.6 build **failed** on a bad script path instead of
silently shipping a stale binary.

### Installed-build verification (0.1.6, silent `/S` install via the test agent)

| Check | Result |
|---|---|
| `evorift-svc.exe` before install | 1,213,952 B (26.06.2026) |
| `evorift-svc.exe` after install | **2,541,568 B, SHA-256 matches shipped** |
| Uninstall registry | `evorift v0.1.6` |
| Service startup line | `listening idle (boot auto-protect kapalı — P0-e…)` = **new code** |

Then driven through the **installed** `evorift-ctl.exe` → **installed** `EvoriftSvc`:

```
start:   running=false state=idle verify=unverified     (no auto-start — P0-e holds)
DNS before: 192.168.1.1 (ISP)

HAFIF    protmode   38ms | on 4996ms | VERIFY -> verified in  453ms
         discord.com -> 162.159.138.232, 162.159.128.233, ...   (DNS actually applied)
GUCLU    protmode 1967ms |            VERIFY -> verified in  851ms
HAFIF    (back)   1892ms |            VERIFY -> verified in  435ms
```

**Both shipped modes reach `verified` on the installed build.** Teardown reset DNS to DHCP.

Caveats: single line, single run. The repeats values (6/8) are still unmeasured — see the previous
entry; DNS masked them and this run did not re-sweep. Two "FAIL" lines in the install-verify output
were artifacts of that script, not defects: `evorift.exe`'s hash legitimately differs from
`target/release` because Tauri patches it with bundle metadata, and its old-code regex matched log
lines predating the install.

## 2026-08-15 — "stuck on Bağlanıyor / test edilmedi" root-caused and fixed (laptop, measured)

Three consecutive UI test rounds reported: protection stuck on "Bağlanıyor / test edilmedi"
forever in EVERY mode (including Hafif), "Güçlü Koruma" failing with a mode-switch error, and VPN
never handshaking. Reproduced on the laptop through `evorift-ctl` only. **Two independent causes.**

### Cause 1 — the freeze: a tunnel nobody asked for, under the engine lock, with no timeout

`want_warp()` returned **true when `app_modes` is empty** — the default on a fresh install and in
every DPI-only mode. So plain "Hafif Koruma" ran `wireguard.exe /installtunnelservice` on every
Start. That call happens inside `sync_warp()`, which `dispatch()` invokes **while holding the
engine mutex**, and `run_hidden()` used a plain `cmd.output()` with **no timeout**. One slow tunnel
install therefore froze the entire service: every later dispatch blocked on the same lock,
including the 2s health/verify poll. `spawn_verify()` runs *after* `sync_warp()`, so `verify` never
left `Unverified` — "never Korumalı, never Sorunlu". With no client-side IPC timeout either, the UI
waited forever instead of erroring.

Fixed: tunnel is opt-in (`want_warp` no longer true on empty), child processes bounded at 25s
(`run_hidden_timeout`), IPC calls bounded at 45s. Measured after the fix:

| Step | Before | After |
|---|---|---|
| `protmode hafif` | error | **179 ms** |
| `on` (start_protection) | hung indefinitely | **1038 ms** |
| verify settles | never | **6257 ms** (to `broken` — see cause 2) |
| `protmode guclu` | "mod değiştirilemedi" | **1898 ms** |

### Cause 2 — DNS was reported but never applied; the ISP answers Discord with a sinkhole

With the freeze gone, verify settled to `broken: discord.com: TCP: connection timed out` — and a
sweep of four configs failed **identically**, including the 2026-08-13 proven one (catch-all,
repeats 11). Identical failure across a known-good config means it was never a tuning problem:

| Config | Result |
|---|---|
| hafif (hostlist + r6) | broken — `discord.com: TCP: connection timed out` |
| guclu (catch-all + r8) | broken — same |
| catch-all + r11 (2026-08-13 proven) | broken — same |
| catch-all + r6 | broken — same |

Root cause: `Command::Start` set `e.dns = "cloudflare"` **as a field and never applied it**, so
`status` claimed dns=cloudflare while the adapters still used the ISP resolver — which answers
every Discord domain with **195.175.254.2**, a sinkhole. DPI desync cannot fix a wrong destination
IP: the connection dies at TCP, before any handshake exists to rewrite.

```
BEFORE:  adapters 192.168.1.1        discord.com -> 195.175.254.2
AFTER:   adapters 1.1.1.1,1.0.0.1    discord.com -> 162.159.138.232, 162.159.128.233, ...
                                     gateway.discord.gg -> 162.159.130.234, ...
                                     cdn.discordapp.com -> 162.159.133.233, ...
VERIFY: verified in 31ms   (hafif)
VERIFY: verified in 19ms   (guclu, catch-all)
```

Fixed: Start now applies DNS with the engine lock released, and on failure records `dns=auto`
rather than claiming a provider it did not set.

**Both shipped modes reach `verified` on this line.** Note this makes secure DNS load-bearing, not
cosmetic: on a DNS-poisoned line the bypass is useless without it. The repeats values (6/8) remain
unmeasured — the sweep above could not discriminate between them because DNS masked everything, so
they stay provisional and the comment at their definition still stands.

Caveat: single line, single run, DNS reset to DHCP afterwards so the laptop was left clean.

## 2026-08-13 — Task 3 acceptance measurements (post Task 1/2 honesty fixes)

Context: today's mandate was "honesty + measurement, nothing else." Task 1 removed six silent-
success paths (UI reporting Active unconditionally, IPC failure resolving to `running:true`,
missing-binary "sim" fallbacks in three engines, a watchdog that discarded a failed respawn
Result, a hardcoded engine-list fallback). Task 2 added a real TLS-handshake proof-of-protection
gate (`src-tauri/src/verify.rs`) so the UI can no longer show "Protected" without a live,
certificate-validated handshake to Discord actually succeeding. This is the first measurement
run taken with both landed — everything below is real, on the second laptop, via the
`evorift-remote-testing` harness (`evorift-ctl` only, never a hand-built winws command line).

Engine bundle: `evorift-svc.exe`/`evorift-ctl.exe` built fresh today (`cargo build --release`,
includes the Task 1/2 fixes); `winws/` bundle already present on the laptop from a prior push,
byte-identical (sha256-verified) to the local copy.

### (a) Discord desktop — DPI-only

**Baseline (protection OFF), real cert-validated TLS handshake:**

| Target | Result | ms | Note |
|---|---|---|---|
| `discord.com` | **FAIL** | 6177 | TLS handshake timeout (TCP connects, DNS resolves — the handshake itself never completes) |
| `gateway.discord.gg` | **FAIL** | 6348 | Same — this is the WebSocket gateway Discord needs to actually log in |
| `cdn.discordapp.com` | ok | 366 | Not blocked |
| `discordapp.net` | n/a | — | DNS does not resolve — looks like a stale/retired domain, not a blocking signal |

**With protection ON** (`evorift-ctl mode dpi` — WARP explicitly forced off — then `evorift-ctl on`, strategy `auto`):

| Target | Result | ms |
|---|---|---|
| `discord.com` | ok | 351 |
| `gateway.discord.gg` | ok | 1548 |
| `cdn.discordapp.com` | ok | 642 |

**Functional verdict (human-observed, the part TLS numbers can't tell us):** with DPI-only
protection on, Discord desktop was quit and relaunched, and **logged in fully — not stuck on
"Connecting…"**. Confirmed directly by the person at the laptop during the hold window.

Job log (`evorift-ctl mode dpi` → `on` → 120s hold → `off`), full teardown reported clean:
```
ok evorift-svc pid 9340
MODE OK -> dpi (warp forced off)
hostlist: 12 domain gonderildi
START OK running=true strategy=auto dns=cloudflare
PROTECTION IS ON (mode=dpi) -- holding 120s. Quit and relaunch Discord now.
--- teardown ---
STOP OK running=false
```

Note: the prepared `scripts/test1-discord.ps1` (auto-installs Discord, applies the mode, launches
Discord, holds for a human to watch) could not run end-to-end as designed — the laptop's
`evorift-testd` runs as a Windows service (Session 0), so any GUI process it launches is not
visible on the interactive desktop, and Discord was not yet installed under the service context's
profile (it was under `C:\Users\Huseyin\...`, found by searching all profiles). Protection was
applied via a session-safe variant instead; Discord was quit/relaunched by the person at the
laptop directly. `test1-discord.ps1`'s `Find-DiscordExe` should be widened to search
`C:\Users\*\AppData\Local\Discord` for future runs launched through the agent.

### (b) Toggle downtime

`scripts/test3-toggle.ps1`, mode=dpi (WARP off), continuous ICMP probe to `1.1.1.1` at ~100ms
intervals logged to disk (348 rows captured), 3 reps, gap read off the probe timeline (not
inferred from before/after snapshots). Raw: `docs/captures/test3-probe.csv`, `test3-result.json`.

| Rep | Direction | Command latency | Network gap |
|---|---|---|---|
| 1 | off→on | 20ms | **212ms** |
| 1 | on→off | 157ms | **749ms** |
| 2 | off→on | 1691ms | **406ms** |
| 2 | on→off | 103ms | **740ms** |
| 3 | off→on | 1743ms | **1168ms** |
| 3 | on→off | 173ms | **400ms** |

off→on gap: 212–1168ms (avg ~595ms). on→off gap: 400–749ms (avg ~630ms). Command latency itself
is noisy (20ms–1743ms) and doesn't track the actual network gap — the gap is the real number,
read from the probe, not the command's own return time.

### (c) Fake-packet minimum (`dpi-desync-repeats` sweep)

**First run gave a false negative — recorded because failed runs are the most valuable data
point here.** `scripts/test2-repeats-sweep.ps1` swept `evorift-ctl strat c1 --repeats=N` from 1
to 20 against `www.google.com`/`www.microsoft.com`/`www.cloudflare.com` and reported **zero**
values held. Every single attempt failed in ~150–400ms with the same generic error. Before
recording that as a product finding, a baseline check (protection OFF) showed the identical
failure on all four targets including plain `discord.com` — inconsistent with `gateway.discord.gg`
and `cdn.discordapp.com` behaving normally elsewhere. Root cause: the script's `Test-TlsHandshake`
passed a PowerShell scriptblock as the `RemoteCertificateValidationCallback`; .NET's async TLS I/O
invokes that callback off the PowerShell runspace thread, which throws "There is no Runspace
available to run scripts in this thread" — failing every handshake regardless of what was actually
happening on the wire. Fixed in `scripts/test1-discord.ps1` and `scripts/test2-repeats-sweep.ps1`
by dropping the accept-any callback and using real certificate validation instead (methodologically
better anyway — it also catches a substituted DPI/MITM certificate, which accept-any would mask).

**Re-run with the fix:**
```
strat c1 --repeats=1 (sweep)    -> all 3 targets ok (253ms, 374ms, 266ms)
strat c1 --repeats=1 (confirm1) -> all 3 targets ok (556ms, 562ms, 166ms)
strat c1 --repeats=1 (confirm2) -> all 3 targets ok (171ms, 435ms, 169ms)
confirmed minimum repeats = 1 (3/3 runs held)
```

**Confirmed minimum `dpi-desync-repeats` = 1**, verified twice (9/9 individual TLS handshakes
succeeded). Raw: `docs/captures/test2-sweep.csv`, `test2-result.json`.

### Second, independent run (same evening) — (b) and (c) only

A second session ran TEST 2 and TEST 3 independently against this same laptop (address moved
`.18`→`.24`→`.20` across two laptop sleep/wake cycles). Recorded here per the interpretation
rule below ("repeat at least at two different times") — this is that second data point, not a
replacement for the run above.

**(b) Toggle downtime, 3 reps, mode=dpi:**

| Rep | Direction | Command latency | Network gap |
|---|---|---|---|
| 1 | off→on | 25ms | **1137ms** |
| 1 | on→off | 175ms | **1042ms** |
| 2 | off→on | 1597ms | 421ms |
| 2 | on→off | 175ms | 874ms |
| 3 | off→on | 1604ms | 503ms |
| 3 | on→off | 104ms | 941ms |

Same shape as the first run (gap does not track command latency; off→on and on→off both land
mostly under ~1s with occasional excursions above it) — rep 1 exceeded 1s in both directions
this time instead of rep 3. Raw: `docs/captures/test3-20260813/`.

**(c) Fake-packet minimum:** first attempt this session also produced a false negative — same
`Task.Wait()`-swallows-the-real-exception bug independently rediscovered, already fixed by the
other session before this one's clean re-run (see BACKLOG.md P0-e for the fix). Clean re-run
after the fix: **confirmed minimum `dpi-desync-repeats` = 1, 3/3 runs held** (9/9 TLS
handshakes), matching the run above exactly. Raw: `docs/captures/test2-20260813-clean/`.

**New this session — a harness-exclusivity bug, filed as BACKLOG.md P0-e:** partway through
this session's first (contaminated) TEST 2 attempt, `evorift-svc.exe`/`winws.exe` were found
running minutes after the job that should have owned them had exited, with no scheduled
task/registry Run key/service found to explain it. `evorift-ctl off` does not verify the DPI
process it stopped is actually gone. All three test scripts now refuse to start if a prior
evorift-svc/winws instance is already running — see "Exclusive control" in
`docs/REMOTE-TESTING.md`. **Open question, not resolved:** this session was told no one else
was using the laptop, yet this file already contained a full three-test write-up (including a
human-observed TEST 1) from what must be a separate, concurrent effort — worth reconciling
before trusting "exclusive control" fully going forward.

### Caveats (per this file's own interpretation rules)

- Single run, one network, one point in time. Not yet repeated on a different ISP or at a
  different time — do not read this as "solved."
- `repeats=1` being sufficient is itself a signal worth double-checking on a second network:
  an unusually low minimum can mean the strategy is working well, or that this specific
  network's blocking is lighter than the target this strategy was tuned against.
- The Test 2 script bug above is a reminder that a measurement claiming "it doesn't work" needs
  the same scrutiny as one claiming "it works" — both were wrong today until traced to a cause.
- Deadman check: the laptop's controller link dropped twice during this session (DHCP moved its
  address mid-run, once from `.18`→`.24`, once `.24`→`.20` — the harness's known "both machines'
  leases moved on the same day" failure class) and the deadman fired several times, but cross-
  checked against `deadman.log` timestamps, none of the fires overlap the four jobs whose data is
  recorded above (test3, the corrected test2, the protected-Discord TLS check, the human-verified
  Discord session) — all fired in the reconnect gaps between jobs, not during one.

## Prepared run: P0-a/b/c validation (2026-08-13, not yet executed)

Targets SOLUTION.md §3.2's case — every Discord/Roblox (+YouTube) domain reaching TLS-OK
through the bypass. 5-step protocol (baseline/apply/measure/60s-persistence/revert) as
requested; no live-tweak step this run. Commands and expected output below; user executes,
fills in the actual output, marks Pass/Fail, appends a row to the table above.

**Target list caveat (2026-08-13):** of the 12 CORE targets, only the `discord.*` ones
discriminate anything. `youtube.com`/`googlevideo.com` are not blocked in Turkey — expect
`tls_ok:true` on these even at baseline; that's not evidence the bypass did anything.
`roblox.com` appears to have been unblocked around June 2026 per `net3/SOLUTION.md` — same
caveat. Don't read a baseline pass on these three as a bypass working, and don't count them
toward the pass criterion below.

**Why `tls_ok:true` on Discord domains is NOT sufficient by itself (this is the important
part):** `net3/SOLUTION.md` §3.2 already recorded every Discord domain — including
`gateway.discord.gg` — reaching TLS-OK through the bypass, and even the gateway WebSocket
returning a live hello frame (`op:10 {"heartbeat_interval":41250}`). §3.3, on the exact same
run, records the Discord **desktop app stuck on "Connecting…"** the whole time. A
`tls_ok:true` result on `discord.*` is therefore consistent with either Discord actually
working OR Discord still being broken — this run's original pass criterion could be fully
satisfied by a run that fixes nothing. Step 3b below is what actually discriminates.

```powershell
cd C:\Users\Evrim\Desktop\projects\net\src-tauri
cargo build --bin evorift-ctl --bin evorift
cd target\debug

# 1) BASELINE — protection off
.\evorift-ctl.exe off
.\evorift-ctl.exe diag
# Expect: JSON array, 12 targets (discord.com, discordapp.com, discord.gg, discordapp.net,
# discord.media, gateway.discord.gg, cdn.discordapp.com, roblox.com, www.roblox.com,
# rbxcdn.com, youtube.com, googlevideo.com). youtube.com/googlevideo.com/roblox.com may
# already show tls_ok:true here — expected, not a bypass signal (see caveat above). If any
# discord.* target is ALSO already tls_ok:true at baseline, this network isn't a valid
# baseline for the discord.* targets either — note that explicitly, don't force a result.

# 2) APPLY — launch evorift.exe; P0-a means it now self-elevates (accept the UAC prompt)
Start-Process .\evorift.exe
# wait for the tray icon to appear, then:
.\evorift-ctl.exe on
# Expect: "hostlist: 12 domain gonderildi" then "START OK running=true strategy=... dns=..."

# 3) MEASURE
.\evorift-ctl.exe diag
# Expect: all discord.* targets tls_ok:true, suggestion "Reachable — DNS, TCP and TLS all
# succeeded." This alone does NOT mean Discord works — see 3b.

# 3b) FUNCTIONAL CHECK — the thing tls_ok cannot tell us
# Launch the Discord desktop client. Record which of these it reaches:
#   - stuck on "Connecting…"        → gateway payload still dropped (SOLUTION.md §3.3 state)
#   - logs in, servers and DMs load  → actually working
# If Discord's renderer log is accessible, record whether READY arrives or whether it shows
# [ACK TIMEOUT] / [WS CLOSED] (false, 1006, ) — that distinction is the whole finding.
# (Renderer log, if enabled: %APPDATA%\discord\logs\ — not guaranteed present/enabled.)

# 4) 60s PERSISTENCE
Start-Sleep -Seconds 60
.\evorift-ctl.exe diag
# Expect: identical to step 3 — same targets still Reachable (durability, not a one-off)

# 5) REVERT
.\evorift-ctl.exe off
.\evorift-ctl.exe diag
# Expect: back to step 1's baseline result
```

**Pass criterion (rewritten 2026-08-13):** TLS-OK on `discord.*` targets is **necessary but
not sufficient**. The run passes only if **3b** shows the Discord desktop client actually
reaching a logged-in state (servers/DMs load) — not merely `tls_ok:true` in step 3/4.
`youtube.com`/`googlevideo.com`/`roblox.com` results don't count toward pass/fail either
way (see target-list caveat). **If it fails:** the failure mode matters — which targets, at
which step (dns/tcp/tls) in the JSON, and which of the two 3b states Discord reached — is
more informative than pass/fail alone; record the raw JSON and the 3b observation, not just
a verdict.

**What each outcome means (for after the run, not to decide now):**
- Discord logs in successfully → the engine works; whatever gave the impression it doesn't
  is elsewhere (state-reporting honesty is the first suspect). v2's priority becomes the
  honesty/verification layer, not the engine.
- 12/12 `discord.*` TLS-OK but Discord stuck on "Connecting…" → SOLUTION.md §3.3's state
  reproduced exactly. Discord's only path is the tunnel; V6 can't stay deferred to the end.
- TLS-OK doesn't even happen → the strategy/engine genuinely isn't working, and that's the
  actual case for the v2 engine rewrite.


## Measurement protocol (same order every run)

1. **Baseline.** Protection OFF. Run `diagnose` against the target domain list. Record the
   result. No "it worked" claim is valid without this.
2. **Apply.** Turn protection on. Record the apply time and the reported status.
3. **Measure.** Same `diagnose` again. Pass criterion: TLS handshake completes on ALL
   targets **and** there's a measurable difference from step 1.
4. **Durability.** Wait 60s, repeat step 3. Should get the same result.
5. **Live tweak.** Change a strategy parameter (e.g. fake count). Expected: existing
   connections don't drop, new flows pick up the new setting. If they drop, the phase
   doesn't pass.
6. **Roll back.** Turn protection off. Confirm the baseline is restored and nothing is
   left on the machine (filter, service, DNS, firewall, route).

## Phase-based checklists (copy the relevant block into a run entry when used)

**Strategy engine (V1-V3)**
- [ ] Target domains blocked at baseline, open after apply
- [ ] Live parameter change doesn't drop connections
- [ ] Auto-escalation fires: start with a deliberately weak strategy, watch it self-strengthen
- [ ] Traffic outside the filter's scope (game UDP) is unaffected — ping measured before/after

**Tunnel (V6)**
- [ ] Split mode: only selected domains route through the tunnel (route table check)
- [ ] Full mode: DPI engine is STOPPED
- [ ] No "Protected" claim before the handshake actually happens
- [ ] Kill switch: force the tunnel down → no traffic leaks, UI warns immediately
- [ ] On exit, the prior engine and routes are restored

**Accelerator (V7)**
- [ ] Background downloads are actually throttled while game mode is on (measured Mbps)
- [ ] Game traffic's ping is unaffected by the throttle
- [ ] Limit lifts when the mode turns off, no persistent QoS leftover

**Honesty (every phase)**
- [ ] Run unprivileged: app clearly says "no protection," no fake "Active"
- [ ] Corrupt the bundle: errors, doesn't silently fall back to a no-op
- [ ] Kill the process: the next status query tells the truth

## Interpretation rules

- One run is not evidence. Repeat at least at two different times.
- Don't mistake the line's own fluctuation for the strategy's success — that's what the
  baseline is for.
- "Worked for me" is a single ISP. Not "solved" until confirmed on a different ISP (V10.2).
- Unexpected success is also suspect: the block might have lifted on its own — rule that out first.
