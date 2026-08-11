# HYPOTHESES-INTERNET-CUT.md — Ranked candidates, read this before running the capture

Produced overnight (2026-08-13/14) by gathering evidence with two independent code-reading
passes (WinDivert/capture-lifecycle; DNS/firewall/route/proxy) and handing a self-contained
brief to `architect` (Opus 5) to rank. **No fixes below — this is what to look for, not
what to build.** Full reasoning kept in the brief/response; this file is the operational
summary for reading before you run `scripts/capture-state.ps1`.

## Read this first, before anything else

1. **Read `evorift-logs\` in the capture before diffing any other file.** It may state
   outright which engines/tunnel started — inference from the other files is the fallback,
   not the first move.
2. **Record which close method you use, and use the SAME one you used on 2026-08-13.**
   Tray "Çıkış"/Quit, the window's X button, and Task Manager kill are NOT equivalent —
   only tray Quit explicitly tears the WARP tunnel down before exiting (see #1 below). If
   you don't remember which you used originally, that's itself worth noting — it changes
   how much weight rank #1 vs #2 below should get.
3. **Don't read "all internet was down" as literal until `05-reachability.txt` says so.**
   Ping 1.1.1.1 specifically (it's outside every candidate's affected IP ranges below) and
   the gateway. If those succeed while browsing fails, the outage was
   Cloudflare/443-shaped, not literally everything — that distinction is exactly what
   separates candidate #1 from #2.

## Ranked candidates

### 1. WARP split-tunnel silently engaging on a default "Start"

**Why it's ranked first:** it's the only candidate with a mechanism *already proven*
Discord-specific by prior project history (`net3/SOLUTION.md` already established Discord
needs tunnel routing, not desync, on this network — this isn't a new guess), and the only
one with a real code path that ties recovery to the app closing.

- `service.rs::sync_warp()` runs on **every** plain `Command::Start` (`service.rs:197`),
  not just a special tunnel mode. `want_warp()` returns `true` whenever `app_modes` is
  empty (`service.rs:97-102`), which is the default state on a fresh session
  (`service.rs:77`) unless the frontend explicitly populates it first — **unverified
  whether it does**, frontend code wasn't read in this pass.
- If triggered, it installs a real WireGuard-for-Windows tunnel service routing
  `162.159.0.0/16` (Cloudflare edge), `66.22.0.0/16` (Discord's own AS), and
  `104.29.0.0/16` (more Cloudflare) — `warp.rs:23`. A failed handshake to
  `188.114.98.224:2408` black-holes traffic to those ranges rather than falling back to
  direct routing.
- Recovery path: tray Quit sends `Command::Stop` synchronously before `app.exit(0)`
  (`lib.rs:983-987`), which reaches `WarpEngine::stop()` (`warp.rs:322-331`,
  `/uninstalltunnelservice`). **The window's X button does NOT do this** — it only hides to
  tray (`lib.rs:960-965`); the tunnel would keep running.

**Discriminating file: `01-network-config.txt`.** Look for a WireGuard/wintun adapter and
routes for the three `/16`s above, in `ipconfig /all` and `route print`. Presence = this
candidate is at least a contributor. Absence = this candidate is eliminated outright, no
further argument needed — promote candidate #2 to first place.

**Hardest fact for this candidate to explain:** total outage. It can only black-hole three
specific `/16`s by its own code — calling it "all internet" requires accepting that was a
subjective impression of everything you happened to try that day (a lot of the modern web
is Cloudflare-fronted), not a literal measurement. If `05-reachability.txt` shows
non-Cloudflare destinations also failed, this candidate alone isn't sufficient.

### 2. WinDivert capture filter / reinjection failure while `winws.exe` runs

**Why it's close behind #1, not a distant second:** TCP 80/443 is not a narrow scope from
where you're sitting — browsing, app updates, and Discord are essentially all port 443, so
"everything I tried stopped working" is fully consistent with a 443-scoped capture that
fails to reinject packets correctly. This may not need to be reinjection failure specifically -
what matters observably is whether traffic on 80/443 broke while other ports didn't, or
whether the whole gateway/adapter went dark.

- Every filter-construction path found (`engine.rs:554-679`) scopes to TCP 80/443 + 3
  narrow UDP signature predicates — no deny-all/wildcard bug found in the code that builds
  the filter argument string.
- What happens to already-captured packets if `winws.exe` dies abruptly (killed, or the
  Job-object kill-on-close fires) — whether WinDivert fails open (passes them through) or
  fails closed (drops them, possibly more broadly than just the matched flows) — is
  **Windows/WinDivert kernel behavior not visible in this repo's source** (WinDivert is an
  external bundled driver). This is a real, unresolved unknown, not something the code
  proves either way.
- Recovery fits by construction: winws only holds the capture handle for the app's
  lifetime; closing the app (any way) eventually kills winws via the Job object, which
  releases the capture.

**Discriminating file: `04-processes.txt`.** The actual `winws.exe` command line (via the
CIM section) shows the filter that really ran. If it's materially broader than TCP 80/443 +
3 UDP signatures, this candidate moves to rank 1 regardless of what `01-network-config.txt`
shows for WARP — the two can compound, since both fire on the same Start action.

**Hardest fact for this candidate to explain:** Discord failing *specifically and
repeatably*, including in earlier tests that weren't a total outage. A 443-wide break
explains Discord failing *along with* everything else, but doesn't on its own explain why
Discord in particular kept failing when other things didn't.

### 3. Job-object / process-death cleanup gap (amplifier, not standalone)

Governs how reliably #1 and #2's side effects actually get cleaned up — has no independent
story for total outage or Discord-specificity on its own.

- `SetInformationJobObject`/`AssignProcessToJobObject` return values are silently discarded
  (`proc.rs:36-41,55`). If either failed, the "kill winws when the app exits" safety net
  would never have been armed — but that predicts the **opposite** of what was observed
  (closing the app would have fixed nothing). Recovery on close is mild evidence this net
  *was* armed, i.e. an argument against this being the primary cause.
- `WinwsEngine::stop()` (`engine.rs:744-750`) can return success while `winws.exe` is still
  alive — `kill()`/`wait()`/the `taskkill` fallback all silently discard failures. Real gap,
  but only matters for the *explicit* stop action, not the close-triggered recovery observed.

**Discriminating file: `04-processes.txt`.** Whether any `winws*` process survives in the
`after` capture (once the app is closed) directly tests whether the kill was armed.

### 4. DNS override — near-eliminated by the recovery fact itself, not just unsupported

- Not on the plain Start path at all (`service.rs:261-271,480-488` — needs an explicit
  `SetDns` command or a DNS-enabled profile). Nothing reverts DNS on process exit — only an
  explicit `ResetDns`/rollback does. **If DNS had been the cause, closing the app would not
  have fixed it — but it did.** Keep on the list only if you recall having a DNS-enabled
  profile active during the test.

**Discriminating file: `01-network-config.txt`** — the `netsh interface ipv4 show
dnsservers` section.

### 5. Per-app firewall block — ruled out by scope

`BlockApp` requires an explicit command never issued by Start, and is scoped to one
program's path — cannot cause a total outage by construction. Kept only as a cheap negative
control.

**Discriminating file: `02-firewall-rules-filtered.txt`** — any `evorift-*` rule present at all.

### 6. ProxiFyre / WireSock alternate engines — ruled out absent explicit selection

`byedpi-proxifyre` requires deliberate engine selection (not the default); `wiresock.rs` has
no call sites anywhere outside its own tests — dead code as currently wired. "Protection on"
reads as the default action, which routes to the WinDivert/winws engine (candidate #2), not
these.

**Discriminating file: `04-processes.txt`** — which engine process is actually running and
with what arguments.

## The decision rule between #1 and #2 (the two live candidates)

1. Check `01-network-config.txt` for a WireGuard/wintun adapter + routes to the three
   WARP `/16`s.
   - **Not present** → candidate #1 is eliminated. Candidate #2 is the answer, full stop.
   - **Present** → candidate #1 is confirmed as at least a contributor. Continue to step 2
     regardless, since both can fire from the same Start action.
2. Check `04-processes.txt` for the actual `winws.exe` filter arguments.
   - **Broader than TCP 80/443 + 3 UDP signatures** → candidate #2 is also live; both are
     likely compounding.
   - **Matches the narrow expected scope** → candidate #1 is the primary explanation.

## Known unknowns this ranking depends on (record these tomorrow, don't guess them)

- Which close method was used on 2026-08-13 (tray Quit / window X / Task Manager) — changes
  how strongly the recovery fact supports candidate #1.
- Whether `EvoriftSvc` (the separate LocalSystem service) was installed and running
  independently of `evorift.exe` during the test — if so, closing the UI window wouldn't
  have touched the engine at all, and both candidates' recovery story needs re-deriving
  through the service instead. `03-services.txt` resolves this.
- Whether the frontend populates `app_modes` before calling Start (which would suppress
  candidate #1's default WARP trigger) — not determinable from the backend code read in
  this pass.

## Accepted risk in this ranking

Zero cost to being wrong here — this is read-order guidance for interpreting a capture, not
a change to anything. The one real risk is an unmodelled failure this brief didn't
consider: if `05-reachability.txt` shows even the **gateway ping** failing, that points at
something link-level (layer 2), which none of the six candidates above predict — treat that
as a signal to stop trusting this ranking and start over rather than forcing a fit.
