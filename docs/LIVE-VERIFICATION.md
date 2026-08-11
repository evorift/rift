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
