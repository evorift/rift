# LIVE-VERIFICATION.md

Live-machine verification log for evorift v2. Per the `evorift-live-verification` skill:
a unit test verifies the code's own claim, this log verifies **reality**. No phase counts
as "done" without an entry here. The user runs the actual commands; Claude prepares them
and interprets the result. Failed runs are recorded too — they're the most valuable data
point there is.

**Empty today.** No v2 code exists yet to verify (see `docs/STATUS.md`). The first entry
should land once V0-V1 produce a strategy engine with observable behavior — not before.

## Run log

| Date | Phase | ISP | Version | Baseline | Result | Live-tweak result | Pass/Fail | Notes |
|---|---|---|---|---|---|---|---|---|
| _(none yet)_ | | | | | | | | |

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
