# evorift — Single-Shot Implementation Prompt

> Hand this entire file to a fresh capable coding agent. It contains everything needed to make
> Discord (desktop app + voice), Roblox and YouTube work reliably on Turkcell — **like WARP** —
> and ship it as a distributable Windows app. Do NOT re-derive the facts below; they cost a full
> day of live debugging on the target machine.

---

## 0. Goal & environment
- **Product:** evorift — "reach Discord/Roblox/YouTube on aggressive-DPI ISPs without a full VPN."
- **Stack:** Rust + Tauri v2 + SvelteKit. **Repo:** github.com/evorift/rift. **Root:** `C:\Users\Evrim\Desktop\projects\net`.
- **Target ISP:** Turkcell Superbox (Turkey) — aggressive TLS DPI **and** sends ICMP port-unreachable on QUIC/UDP-443.
- **Service:** LocalSystem Windows service `EvoriftSvc` (`evorift-svc.exe`) + non-admin Tauri UI (`evorift.exe`) talking over a named pipe.

## 1. HARD-WON FACTS — do not relitigate
1. **Pure DPI-desync (winws/zapret) is NOT enough for the Discord DESKTOP app.** It makes web Discord, Roblox, YouTube work (TLS handshakes pass, 6/6), but the desktop client **opens to a blank dark screen / hangs on "Starting…"**. Cause: the Electron client prefers QUIC (ICMP-killed here) and pulls large JS bundles that the aggressive desync corrupts. Web works because the browser loads tolerantly.
2. **WARP works** on this line because it tunnels everything. → The reliable fix for the desktop app is a **targeted WireGuard split-tunnel for Discord's IPs only** (everything else stays on DPI-bypass). This is the core structural decision.
3. Blocking QUIC at the firewall (`netsh … UDP remoteport=443`) did **not** fix it and made things worse — **do not block QUIC**.
4. **Licenses (all bundleable with notices):** zapret = **MIT** (bol-van), WinDivert = **LGPLv3**, cygwin1.dll = **LGPLv3**.
5. **`cargo build` produces a BROKEN UI exe** ("localhost refused" — it points at the dev server). The UI exe MUST be built with **`npm run tauri build`**. (`evorift-svc.exe` is fine via `cargo build --release --bin evorift-svc`.)
6. **The old service crashed repeatedly** (`Service Control Manager` event 7031 ×3) when it spawned/managed winws under churn → bypass flapped → Discord broke. The new design MUST be crash-proof (see §4).

## 2. Architecture — 3 engines, one crash-proof service
```
EvoriftSvc (LocalSystem, crash-proof)
 ├─ Engine A: winws (zapret, MIT) child process  → DPI-desync for general TCP/443 (web Discord, Roblox, YouTube, everything else)
 ├─ Engine B: WireGuard split-tunnel (Cloudflare WARP) → Discord IP ranges ONLY → desktop app + voice work like WARP
 └─ DNS: Cloudflare DoH on all physical adapters
UI (evorift.exe): control panel + GitHub-Sponsor popup. Pure status/toggle, no privileged work.
```

## 3. Engine A — winws (DPI-desync), already wired
- Code: `src-tauri/src/engine.rs` → `WinwsEngine` (spawns `<exe_dir>\winws\winws.exe`, `kill_all` + `Drop`, idempotent `start`).
- Bundle (already staged in `src-tauri/resources/winws/`, MIT): `winws.exe`, `cygwin1.dll`, `WinDivert.dll`, `WinDivert64.sys`, `windivert.filter/{discord_media_wide,stun,quic_initial_ietf}.txt`, `files/quic_initial_www_google_com.bin`.
- **Catch-all args (covers all TCP/443 + QUIC-fake + wide voice 50000-65535):**
  ```
  --wf-tcp=80,443
  --wf-raw-part=@<dir>\windivert.filter\windivert_part.discord_media_wide.txt
  --wf-raw-part=@<dir>\windivert.filter\windivert_part.stun.txt
  --wf-raw-part=@<dir>\windivert.filter\windivert_part.quic_initial_ietf.txt
  --filter-tcp=80 --dpi-desync=fake,fakedsplit --dpi-desync-autottl=2 --dpi-desync-fooling=md5sig --new
  --filter-tcp=443 --dpi-desync=fake,multidisorder --dpi-desync-split-pos=1,midsld --dpi-desync-repeats=11 --dpi-desync-fooling=md5sig --dpi-desync-fake-tls-mod=rnd,dupsid,sni=www.google.com --new
  --filter-tcp=443 --dpi-desync=fake,multidisorder --dpi-desync-split-pos=midsld --dpi-desync-repeats=6 --dpi-desync-fooling=badseq,md5sig --new
  --filter-udp=443 --filter-l7=quic --dpi-desync=fake --dpi-desync-repeats=11 --dpi-desync-fake-quic=<dir>\files\quic_initial_www_google_com.bin --new
  --filter-l7=discord,stun --dpi-desync=fake
  ```
- **blockcheck WINNER for discord.com TLS1.3 on Turkcell (USE THIS for the TCP/443 stage — empirically found 2026-06-08):**
  ```
  --filter-tcp=443 --dpi-desync=fake --dpi-desync-ttl=7 --dpi-desync-fake-tls=0x1603 --dpi-desync-fake-tls=!+2 --dpi-desync-fake-tls-mod=rnd,dupsid,rndsni --dpi-desync-fake-tcp-mod=seq
  ```
  **CRITICAL:** this is `fake`-ONLY (no `multidisorder`/split) → it does NOT split/reorder the real ClientHello, so it does **not corrupt Discord's large JS-bundle stream**. The aggressive `fake,multidisorder,repeats=11` strategy was the likely cause of the blank desktop screen. **Try this gentle strategy FIRST — it may fix the desktop app without the WireGuard tunnel.** If the desktop app renders with this, Engine B (§4) becomes optional/voice-only.
  - Re-run `C:\zapret\blockcheck\blockcheck.cmd` if Turkcell changes DPI. `tls_clienthello_iana_org.bin` (the fake-TLS payload it used) is in `zapret/files/fake/` — bundle it.

## 4. Engine B — WireGuard split-tunnel for Discord (THE fix — make it work like WARP)
**This is the new, load-bearing piece.** Route ONLY Discord's IPs through Cloudflare WARP via WireGuard; everything else stays on winws.

### B.1 Generate a WARP WireGuard config (once, at install or first-run)
- Use **wgcf** (https://github.com/ViRb3/wgcf, MIT) — bundle `wgcf.exe`. On first run: `wgcf register --accept-tos` then `wgcf generate` → produces a WireGuard config with:
  - `PrivateKey` (per-install), `Address` (e.g. `172.16.0.2/32`, `2606:4700:...`)
  - `[Peer] PublicKey = bmXOC+F1FxEMF9dyiK2H5/1SUtzH0JuVo51h2wPfgyo=`
  - `Endpoint = engage.cloudflareclient.com:2408` (or `162.159.192.1:2408`)
- Store under `%PROGRAMDATA%\evorift\warp.conf` (hardened ACL).

### B.2 Split-tunnel: override AllowedIPs to Discord only
Do **not** use `AllowedIPs = 0.0.0.0/0` (that's a full VPN). Set:
```
AllowedIPs = 162.159.0.0/16, 66.22.0.0/16
```
- `162.159.0.0/16` = Cloudflare edge → Discord API, gateway, CDN, media (discord.com, gateway.discord.gg, cdn.discordapp.com, discord.media).
- `66.22.0.0/16` = Discord's own AS → voice/RTC servers.
- (Verify/extend ranges with `nslookup` of discord domains + Discord's published voice ranges; add `162.158.0.0/15` if needed.)

### B.3 Bring the tunnel up/down from the service
- Bundle the official **WireGuard for Windows** embeddable (`wireguard.exe` + `wintun.dll`, both can be used as a tunnel service) OR use `wireguard.exe /installtunnelservice warp.conf`.
- Cleanest: `wireguard.exe /installtunnelservice "%PROGRAMDATA%\evorift\warp.conf"` on protection-start, `/uninstalltunnelservice warp` on stop. WireGuard injects only the AllowedIPs routes → split-tunnel automatically.
- Result: Discord's packets go Discord-IP → WARP tunnel (DPI never sees them) → **desktop app + voice connect exactly like WARP**. All other traffic stays on winws DPI-bypass.

## 5. DNS — Cloudflare DoH on ALL physical adapters
In `service.rs run_dns` (and apply on protection-start): for **every** `Get-NetAdapter -Physical` (Up **and** Down):
```
Set-DnsClientServerAddress -InterfaceIndex $i -ServerAddresses 1.1.1.1,1.0.0.1,2606:4700:4700::1111,2606:4700:4700::1001   # v4+v6 in ONE call; -AddressFamily is NOT a valid param
Add-DnsClientDohServerAddress -ServerAddress 1.1.1.1 -DohTemplate https://cloudflare-dns.com/dns-query -AllowFallbackToUdp $false -AutoUpgrade $true   # (and 1.0.0.1)
Clear-DnsClientCache
```

## 6. Crash-proof service (was crashing → flapping → Discord broke)
- `Cargo.toml [profile.release]`: add `panic = "abort"` only if you ALSO wrap every dispatch + the auto-protect block in `std::panic::catch_unwind`; otherwise keep `panic=unwind` and wrap. **A panic must never take down the pipe loop.**
- Put each client connection on its own thread (already so) and never `unwrap()` a poisoned `engine.lock()` — use `lock().unwrap_or_else(|e| e.into_inner())`.
- Manage the winws child + WireGuard tunnel in a **Windows Job Object** (`CREATE_SUSPENDED` + `AssignProcessToJobObject` + `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) so they die with the service and never orphan as SYSTEM.
- Service recovery: keep `sc failure restart` but only after fixing the panic, else it crash-loops.

## 7. UI — control panel + GitHub Sponsor popup (already added)
- `src/routes/+layout.svelte`: `showSponsor=true` onMount → `<Modal>` with "💜 GitHub'da Sponsor Ol" → `openUrl("https://github.com/sponsors/evorift")` (capability `opener:default` already granted). Easy-dismiss. **Confirm the sponsor URL / enable GitHub Sponsors.**
- The dashboard black-hole IS the on/off toggle (`onclick={() => app.toggle()}`). Keep it WARP-like: instant optimistic state.

## 8. Build & package (one shot)
1. `npm run build` (vite) — never `cargo build` for the UI.
2. `cargo build --release --bin evorift-svc` (no `--features windivert` needed; winws brings its own WinDivert).
3. Copy `evorift-svc.exe` → `resources/evorift-svc.exe`; ensure `resources/winws/**` + `resources/warp/{wgcf.exe,wireguard.exe,wintun.dll}` exist.
4. `tauri.conf.json bundle.resources`: map every winws + warp file (explicit per-file mappings — globs are unreliable).
5. `npm run tauri build` → `target/release/bundle/nsis/evorift_0.1.0_x64-setup.exe`.
6. `windows/hooks.nsh`: `sc create EvoriftSvc start=auto`, start it; on uninstall stop+delete + `wireguard /uninstalltunnelservice warp` + remove the DNS/firewall changes.
7. Update `THIRD-PARTY.md`: zapret(MIT), WinDivert(LGPLv3), cygwin(LGPLv3), **wgcf(MIT), WireGuard(MIT/GPLv2 components)**.

## 9. Verification (must pass before shipping)
- Web: `discord.com`, `gateway.discord.gg`, `cdn.discordapp.com`, `discord.media`, `roblox.com`, `youtube.com` → SNI-TLS handshake OK (`SslStream.AuthenticateAsClient`).
- **Discord DESKTOP app: opens fully past "Starting…", text loads, and a voice channel connects** (the real acceptance test — verify via screenshot, not just TCP).
- `EvoriftSvc` survives 6× toggle on/off with **no** `Service Control Manager` 7031 crash event.
- Only ONE `winws.exe` (parent = evorift-svc), one WARP tunnel; no SYSTEM orphans after kill.
- All physical adapters show Cloudflare DNS; `discord.com` resolves to `162.159.*` (no poisoning).

## 10. Order of operations
1. Engine B (WireGuard split-tunnel) — the actual fix; get Discord desktop working FIRST.
2. Crash-proof the service (§6).
3. Re-confirm Engine A winws args (blockcheck) — keep web/Roblox/YouTube working.
4. UI proper build + sponsor popup.
5. Bundle + installer + THIRD-PARTY.
6. Run §9 verification end-to-end.
