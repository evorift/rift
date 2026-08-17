# Changelog

All notable changes to **evorift** are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Local-data hardening, and a serious cut to how long a first run takes. **Not built into an
installer yet** — these changes exist in the tree only.

### Added

- **Encrypted local store.** The one genuinely personal thing this app keeps — the domains the user
  adds themselves — is now stored via Windows DPAPI, bound to this machine and the account that
  wrote it. Stated plainly in `secure.rs` and in the UI: it protects the file if it is copied off
  the machine, read from a stolen disk or opened by another standard account; it does **not**
  protect against an administrator on this machine, because the app has to be able to read it
  unattended at boot. Nothing in the code, docs or UI claims it is unbreakable. An existing
  plaintext `state.json` is migrated into the store on first read and then **deleted**.
- **Restrictive file permissions** on `hostlist.txt`, the measurement and the store (SYSTEM +
  Administrators only, re-applied on every write). `hostlist.txt` cannot be encrypted — `winws.exe`
  reads it from disk — so narrowing access is the protection that remains available for it.
- **Log redaction at the sink.** Every log file in the app passes through one function, and that is
  where hostnames and non-resolver IPs are masked, rather than trusting ~40 call sites and every
  future one. Public resolver addresses and filenames survive, or DNS and engine faults would become
  undiagnosable. Support bundles zip a redacted **copy**, never the live log directory.
- **Retention limit**: a measurement older than 30 days is deleted at service start, not merely
  ignored.
- **"Tüm verileri sil"** in Settings: one button, one confirmation, and the backend's own report of
  what was removed — a non-empty "could not delete" list is stated as a partial failure rather than
  shown as success.
- **First-run notice** describing what is kept on the device and that it never leaves it.

### Changed

- **Cold start rebuilt.** Three changes: measurement now begins the moment protection starts
  instead of waiting ~9 s for verification to fail; the previously chosen chain is tried first; and
  the search stops at the first candidate that both works and beats doing nothing, rather than
  measuring the whole ladder to find the theoretical best. A stored, in-date measurement for the
  same network skips measurement entirely.
- The measurement reports live progress (`tuning_step`/`tuning_total`) so the wait shows movement
  instead of looking like a hang.
- The persisted measurement no longer stores the target **domain list** at all — it was fully
  re-derivable from the active mode, and removing data is a stronger fix than encrypting it. What
  remains is a /24 network fingerprint, a chain id and per-candidate counts.

### Removed

- **The `ipinfo.io` ISP lookup.** It was already unreachable (every call site passed
  `consent: false`), but an outbound call to a third-party endpoint has no business being in the
  source of a censorship-circumvention tool — "it never runs" is a claim a reader has to verify by
  auditing every branch.
- **`--debug=1` on the winws retry**, added earlier this session to explain why the engine refused
  to start. Its output is per-connection and can name matched hosts, and that file is captured to
  disk and swept into support bundles. A rule that no requested domain is written in plaintext
  cannot have an exception that fires exactly when something is going wrong.
- The window-focus gate on the protection toggle (reverted at the user's request).

## [0.3.0] — 2026-08-16

The MVP build: a UI pass that removes everything the user cannot act on, and makes a failed start
recover itself instead of sitting there.

### Added

- **Automatic recovery.** When protection is on but verification says traffic is not getting
  through, the app now says "Yeniden başlatılıyor (1/2)" and does something about it: stop, wait
  5 s, restart the service, start again, wait for a verified verdict. Two attempts, then it falls
  back to OFF and says so in a dialog rather than leaving a half-working engine in place. Any user
  action supersedes an in-flight attempt.
- **First-run setup page** offering "Windows ile başlat" and "Tepside başlat", both default off,
  both still changeable in Settings. Sequenced after the welcome wizard rather than stacked on it.
- **Service controls in Settings**: reinstall and stop, both reporting the real outcome.
- **Sponsor button on the nav rail**, replacing the modal that opened on every single launch.

### Changed

- The toggle is ignored while the window is not focused — the switch cuts or restores every
  connection on the machine, and it sits where a stray click on a background window could hit it.
- Protection modes moved above game mode; four equal cards (Hafif · Güçlü · Otomatik · VPN) with an
  icon, a one-line description and a "?" for the detail.
- Status now reads simply **"Korumalı"**. The verify gate that decides when it appears is unchanged.
- Site status, the tuning panel and every mention of the strategy are gone from the UI. Engine and
  driver problems surface on the Bağlantı page only.
- Applications, Speed Limit and Advanced are marked "Yakında" and sealed — visible so the user knows
  what is coming, inert so nothing pretends to work.
- Logs moved into `<install dir>\logs`, and the Desktop mirror is gone. One directory, written
  directly by whichever binary produced the line.

### Fixed

- **The UI could be blanked by a single failing backend call.** `TitleBar` resolved the Tauri window
  at component setup; outside the Tauri runtime that throws while reading `__TAURI_INTERNALS__`, and
  because the component is mounted by the root layout it took the entire window down to an empty
  page. All Tauri access now goes through a guard that turns "no runtime" into a rejected promise
  instead of a throw — which is also what makes the frontend runnable in a plain browser for the
  first time.
- `stop_svc` reported success as soon as the SCM accepted the request, while the service was still
  `STOP_PENDING`. It now waits for the service to actually reach STOPPED.
- `install_service` returned Ok even when the service never started, and did not set the crash
  recovery actions the installer configures — so a service reinstalled from the app was quietly
  less resilient than one installed normally.

## [0.2.2] — 2026-08-16

More fixes read out of a real engine log, including two the 0.2.1 changes did not actually cure.

### Fixed

- **The app reported its own version as 0.1.8 after a correct 0.2.1 build.** The version lived in
  FOUR hand-maintained places and only three were bumped — and the file that lied carried a comment
  claiming it was the only one that needed changing. The frontend now takes the version from
  `tauri.conf.json` at build time, and a mismatch between that, `package.json` and `Cargo.toml`
  fails the build. Verified by deliberately breaking it and watching the build stop.
- **The 60-second engine restart was still happening in 0.2.1.** Keying exclusion identity on PIDs
  was not enough: the PID set is derived from processes that *currently hold a socket*, so an app
  that briefly has none drops out and reappears. The real problem was the mechanism itself — port
  exclusion exists to keep an aggressive chain away from chosen apps, and since 0.2.0 nothing
  aggressive runs outside the hostlist. It now pays a guaranteed cost (a restart that drops every
  live connection, immediately followed by a verification reporting 3/9 where the measurement found
  8/9) to prevent a harm that can no longer occur. It is therefore engaged only when the deployed
  catch-all layer is genuinely capable of corrupting traffic — never for either shipped mode.
- **A slow line could be misread as engine damage.** With handshakes legitimately taking 1.3-2.5s,
  one candidate recorded "broke 3 control sites" — all three at once — which is a congested line,
  not a chain that provably cannot corrupt anything suddenly corrupting everything. Harm is now
  CONFIRMED by re-probing the control set with the conclusive budget before anything is reverted,
  and the measurement's per-probe budget is derived from the line's own slowest healthy handshake
  instead of a fixed guess.

### Verified in the field

The 0.2.1 log confirmed the two-pass measurement working end to end: 7 candidates instead of 15,
early exit at 8/9, **24 seconds instead of 91**; and the DNS check dropping from every 5 seconds to
every 60.

## [0.2.1] — 2026-08-16

Fixes found by reading the engine log from a real 0.2.0 run (the logging added in 0.2.0 is what made
all of these visible — none of them were reportable before).

### Fixed

- **The engine restarted itself every 60 seconds, dropping every live connection.** Applying an
  app-exclusion set restarts winws, and the set was compared BY SOURCE PORT. Ports are ephemeral by
  definition, and Steam/OneDrive/VALORANT ship in "off" mode by default, so the set differed on
  essentially every check and the rate limit merely paced the damage. Exclusion identity is now the
  PID set — "an off-app started or exited" — which is a real and infrequent event.
- **`dns verify` ran on every 5-second watchdog tick, forever.** The rate limit was stamped only
  when drift was actually found, so on a healthy machine it never engaged. Before the native
  resolver read landed in 0.2.0 this meant spawning PowerShell every 5 seconds for as long as
  protection was on and unverified.
- **A line measurement could be corrupted by a concurrent `Start`.** Two winws instances fight over
  the single global WinDivert driver; in the captured log this produced two candidate rows measured
  against a dead engine (256-278 ms probes against a ~4100 ms norm) that scored identically to
  doing nothing. `Start` / mode-switch / profile-apply are now refused while a measurement runs, and
  the tuner verifies its engine is alive before trusting a probe.
- **Turning protection OFF spawned `wireguard.exe` even when no tunnel had ever been created** — a
  child process with a 10 s timeout, under the engine lock, on the path the user experiences as a
  single click. The tunnel's existence is now cached per process.
- **`winws.log` was zero bytes for the one failure that mattered.** winws prints nothing on a normal
  run, so capturing its output bought nothing when it exited with code 1. The retry after an
  immediate exit now enables its debug output, so the reason is recorded.

### Changed — speed

Measured from the user's log: first verdict took 19 s, a full line measurement 91 s.

- Probes now take a budget per call. Ranking candidates and the first verification pass use 1.2 s
  (4× headroom over the slowest observed success at 150-300 ms); only the final, conclusive attempt
  — the one allowed to declare a line broken — still uses 4 s.
- Verification publishes its first result immediately instead of after three attempts, and the retry
  gap dropped from 3 s to 1 s. First status: ~19 s → **~1.2 s**.
- Line measurement is two-pass: one representative per bypass mechanism first, the remaining
  variants only if that fails to find something good enough. ~91 s → **~13 s** typically.
- At equal coverage, a chain that cannot corrupt a connection now outranks a route-dependent one, so
  splitting the ladder cannot cost gentleness.
- An untuned Güçlü no longer emits two identical TLS profiles (and no longer pays a per-connection
  hostlist lookup to reach the second one).

## [0.2.0] — 2026-08-16

Engine rewrite, triggered by a report that **"Güçlü Koruma" made an ordinary site unreachable
while "Hafif" opened it instantly** — protection that was worse than no protection.

### Fixed

- **Strong protection no longer breaks working sites.** Güçlü used to run one route-dependent
  forgery (`fake` + `ttl=1` + `autottl=3`, no fooling) catch-all over *every* TLS/443 flow on the
  machine. That chain only works where the DPI sits at the assumed hop distance; anywhere else the
  forged ClientHello reaches the real server and the server tears the connection down. Hafif
  "worked" only because its hostlist never touched that site. Both modes are now **layered**: an
  aggressive chain gated to domains known to need it, plus a catch-all layer that is provably
  incapable of corrupting a connection (`Strategy::is_harmless`). A unit test now fails the build if
  any mode ever runs a harmful chain catch-all.
- **Verification stopped grading itself on the wrong exam.** The proof-of-protection probe tested
  three Discord hostnames only, so Güçlü could report `verified` while every other site was dead.
  It now probes the mode's real target set **and** a control set of sites that must keep working,
  and reports per-site results to the UI.
- **Protection comes back after a reboot.** The service came up idle with no persisted state, and
  the UI's autostart pointed at a `requireAdministrator` executable from the Startup folder — a path
  Windows cannot elevate, so it failed silently. State is now persisted and restored behind a real
  user setting, and the UI is launched by a scheduled task with `-RunLevel Highest`.
- **Engine failures are visible.** `winws`'s own stdout/stderr was never captured (inherited into a
  Session-0 service with no console), most modules only `eprintln!`d into a void, and the UI's state
  layer swallowed errors in a universal `.catch(() => {})` while firing success toasts regardless.
  There is now a structured event log, sidecar output capture, a Rust→UI event channel, and every
  log is mirrored to `Desktop\evorift-logs`.
- **A hostlist change actually takes effect.** `SetHostlist` called `start()` on a live child, which
  is idempotent by contract — so the new list was accepted, reported as applied, and ignored.
- **A watchdog respawn no longer silently reconfigures protection.** It rebuilt the engine from the
  strategy catalog, which drops the runtime fallback layer, quietly narrowing the mode the user
  chose.
- **`is_running()` measures instead of remembering** — it returned `child.is_some()`, which stays
  true forever after the process dies.

### Added

- **Per-line measurement (`tuner`).** A candidate ladder ordered least-invasive-first — starting
  with doing nothing at all — raced against blocked targets and control sites on the actual
  connection. Any candidate that breaks a control site is rejected outright; if nothing wins
  cleanly, the answer is "touch no packets", which on a DNS-only block is the correct one.
- **Do-no-harm self-healing.** If verification finds ordinary sites failing, the engine discards the
  tuning, falls back to a harmless configuration and re-probes; if targets are merely blocked, it
  re-measures the line. Both are rate-limited, and a persistent harm condition stops protection
  rather than restart-looping.
- Per-site status, engine problems and the measurement score table are exposed over IPC and shown in
  the UI; `evorift-ctl` gained `tune`, `events` and `autostart`.

### Changed

- Start is markedly faster. `clear_stale_windivert()` (up to 3 service names × `sc` calls with
  500 ms polls) and a `taskkill` ran on *every* start as insurance; both are now failure-path only,
  guarded by a sub-millisecond process check. The fixed 700 ms post-spawn sleep became a 25 ms poll,
  DNS application now runs concurrently with engine start, and DNS state is read through
  `GetAdaptersAddresses` instead of PowerShell — turning a ~1.5 s round trip into microseconds and
  making "already correct, skip it" cheap enough to be worth doing.
- The VPN mode card is marked "coming soon" and made non-interactive; it was live and clickable
  while its own confirm dialog admitted it had never had a passing live run.

### Security

- _Nothing yet._

## [0.1.8] - 2026-08-16

### Fixed

- **The app could keep showing protection as on after it had actually stopped.** It only re-checked
  whether the connection was verified, never whether the engine was still running — so if the
  engine stopped for any reason the switch stayed on while only the small print admitted something
  was wrong. It now corrects itself against the service every couple of seconds.
- The protection check no longer gives up after a single failed attempt, so a momentary hiccup
  right after starting can't leave it stuck on "not verified".

## [0.1.7] - 2026-08-15

### Fixed

- **Protection kept dropping out.** The engine was being restarted every few seconds in the
  background, killing connections mid-flight, and secure DNS could silently revert without the app
  noticing. Both are fixed; measured stable with no restarts over sustained use.
- **Sites that stayed blocked in Strong Protection now open.** The bypass method (not its strength)
  was the problem for the toughest domains; Strong Protection now uses a method verified to get
  them through — 400/400 successful connections across four blocked sites, zero failures.

## [0.1.6] - 2026-08-15

### Fixed

- **The installer had been shipping an old copy of the background service since June.** The app
  updated but the part that actually does the work did not, so every "fixed" release behaved
  exactly like the old one. The build now verifies what it packages and refuses to build otherwise.
  Verified end to end on a real installation: both protection modes reach Verified.

## [0.1.5] - 2026-08-15

### Fixed

- **Protection could hang on "Connecting…" forever, in every mode.** Turning protection on started
  a VPN tunnel even in DPI-only modes that never mention one, and a slow tunnel install froze the
  whole app with no error and no way out. The tunnel is now only started when you actually ask for
  it, and no internal step can hang indefinitely any more.
- **Secure DNS was shown as active without being applied.** On connections where the provider
  redirects Discord to a dead address, the bypass cannot work at all — it was connecting to the
  wrong place. DNS is now really applied when protection starts, and if it can't be, the app says
  so instead of claiming otherwise. Verified working end to end on a blocked line.
- A leftover network-driver registration from another program (or an older install) could stop the
  engine from ever starting. It's now cleared properly instead of being left in place.

## [0.1.4] - 2026-08-14

### Added

- Four protection modes named for what they do (Light / Strong / Autopilot / VPN), replacing the
  raw engine and strategy pickers.
- The app now says plainly when its protection backend isn't running, instead of letting every
  action fail with a separate unrelated-looking error.

### Changed

- Language follows your Windows display language on first run; picking one by hand still wins.
- "Checking…" is now shown as its own state, distinct from "not verified" — an in-progress check
  no longer looks identical to one that gave up.
- The protection check now runs its three targets in parallel, so a blocked connection gets a
  verdict in seconds rather than up to half a minute.
- Advanced network settings, per-app settings and speed limits are visibly disabled ("Yakında")
  rather than present but unreliable.

### Fixed

- The portable download shipped without the bypass engine or tunnel binaries, so it could never
  actually protect anything.
- Version number bumped to 0.1.4: 0.1.3 was built more than once with different contents, so a
  stale installer was indistinguishable from a current one.

## [0.1.3] - 2026-08-14

### Added

- Real proof-of-protection: "Protected" now requires an actual TLS handshake to succeed, not
  just the engine process staying alive.
- A protection-mode selector on the dashboard (autopilot placeholder, light/strong presets,
  alternate engine when bundled).

### Changed

- Protection no longer auto-starts on app/service launch — it only starts when you ask it to.
- GoodbyeDPI/ByeDPI controls in Control Panel are hidden instead of shown-but-non-functional
  when their engine isn't bundled in this build.
- The auto-updater is fully disabled for this release (its signature chain isn't set up yet —
  no update check runs, rather than one that would silently fail).

### Fixed

- Several places where a failed or missing engine could be silently reported as "running" —
  the app now shows the actual state, including a distinct error state.

### Security

- _Nothing yet._

## [0.1.2] - 2026-06-11

### Added

- **Genişleyen uygulama satırları.** Uygulamalar listesinde bir satıra tıklayınca
  altında o uygulamanın domain listesi açılır. Domain ekleyip kaldırabilirsiniz.
- **Merkezi versiyon sabiti** (`src/lib/version.ts`). Yeni sürüm çıkarırken
  yalnız tek dosyayı değiştirmek yeterli — hardcoded versiyon stringi kalmaması
  için refactoring yapıldı.
- **Tarayıcı domain tespiti** (Chrome, Edge, Firefox, Brave, vb.): Chromium
  tabanlı tarayıcılar kendi DoH'unu kullandığı için OS DNS cache'inde domain
  görünmüyordu. Artık reverse-DNS (PTR) fallback'i ile domain'ler tespit
  edilebilir.

### Fixed

- **Versiyon numarası güncellenmiyordu.** NavRail, Ayarlar, Günlük ve başlatma
  log'unda `v0.1.0` sabit olarak yazıyordu; yeni sürüm kurulsa bile eski
  versiyon gösteriliyordu. Merkezi `APP_VERSION` sabiti ile düzeltildi.
- **Site listesi hata yutuyordu.** `syncHostlist()` servise gönderimdeki hataları
  sessizce yutuyordu — kullanıcı domain eklediğini sanıyordu ama motor
  güncellenmiyordu. Artık hata log'a yazılır.
- **Chrome/tarayıcı domain tespiti çalışmıyordu.** Chromium tarayıcılar kendi
  DoH'unu kullandığı için `Get-DnsClientCache` eşleşme bulamıyordu.

## [0.1.0] — Bilinen Sorunlar / Known Issues

> **⚠️ Cloudflare WARP resmi istemcisi (v2026.4 ve öncesi) ile çakışma**
>
> Sisteminizde **Cloudflare WARP** (Cloudflare One Client) resmi masaüstü
> uygulaması kuruluysa, bu uygulama arka planda sürekli `tasklist /FO CSV`
> komutu çalıştırarak yüzlerce zombi process biriktirebilir ve **CPU kullanımını
> %100'e çıkarabilir**. Bu sorun evorift'in kendisinden kaynaklanmaz — Cloudflare
> WARP istemcisinin bilinen bir hatasıdır.
>
> **evorift'in WARP modu** (uygulama içi "WARP" seçeneği) resmi Cloudflare WARP
> istemcisini **kullanmaz**; kendi `wgcf` + WireGuard split-tunnel altyapısıyla
> çalışır. İki yazılım birbirinden bağımsızdır, ancak aynı anda çalışmaları ağ
> yığınında çakışmaya neden olabilir.
>
> **Çözüm:**
> 1. Resmi Cloudflare WARP istemcisini kaldırın veya güncelleyin.
> 2. Kaldırmak istemiyorsanız, PowerShell'de servisi devre dışı bırakın:
>    ```powershell
>    Set-Service -Name "CloudflareWARP" -StartupType Disabled
>    Stop-Service -Name "CloudflareWARP" -Force
>    ```
> 3. evorift'in WARP modunu güvenle kullanmaya devam edebilirsiniz — evorift
>    kendi WireGuard tünelini yönetir.

## [0.1.0] - 2026-06-08

First public release. evorift unblocks Discord, Roblox, YouTube, and games on
Windows by defeating ISP deep-packet-inspection (DPI) filtering — no VPN, no
external proxy, no account.

### Added

- **WinDivert DPI-bypass engine.** Packet-level bypass over IPv4 and IPv6
  covering TCP (ClientHello `c1` split, multidisorder, fake-packet injection,
  and TCP-MD5 signature options), QUIC (RFC 9001 Initial-packet decrypt/encrypt
  for UDP/443), and Discord voice (STUN / IP-discovery recognition). Ships as a
  privileged service built with the `windivert` cargo feature.
- **Privileged LocalSystem service** (`evorift-svc.exe`). Runs the DPI engine
  with the rights it needs while the UI stays unprivileged. The UI talks to it
  over a named-pipe IPC channel guarded by a token handshake and a strict
  whitelist `validate()` on every request.
- **Desktop UI** (SvelteKit + Svelte 5 + TypeScript, Tauri v2 frameless window)
  with a dashboard, app/limit/settings sections, a first-run onboarding flow,
  and a system-tray icon. Localized in Turkish, English, Spanish, and Russian.
- **System DNS / DoH** with four built-in providers (applied over both IPv4 and
  IPv6), a DNS leak check, and one-click network repair.
- **System tweaks** — 11 admin-gated optimizations applied via `netsh`, the
  registry, and `powercfg`. Without elevation they run audit-only.
- **Per-app QoS rate limiting** with a Solo mode that pauses background uploads
  from heavy apps while gaming to protect ping.
- **Game Mode** — a persistent one-switch toggle that snapshots current state,
  then auto-selects a bypass strategy, Cloudflare DNS, and safe tweaks, and
  suggests a rate limit for detected heavy apps; full restore on exit.
- **Real telemetry** — connectivity test (DNS resolve + TCP connect + latency),
  DNS status, and a copy-diagnostics action that gathers actual system info.
- **App enumeration and per-app firewall** rules for granular control.
- **Anti-cheat watcher** that can pause the engine for sensitive titles
  (disabled by default).
- **Hostlist hot-reload** — apply hostlist changes without restarting.
- **Autostart** on Windows sign-in.
- **NSIS per-machine installer** with hooks that install and uninstall the
  privileged service, plus a portable ZIP distribution.

### Security

- Release-profile hardening: `strip = "symbols"`, fat LTO,
  `codegen-units = 1`, and `overflow-checks = true` (overflow checks kept on in
  release as a safety fuse — `validate()` / rate-limit arithmetic panics instead
  of silently wrapping).
- Strict Content-Security-Policy for the production webview, with a separate
  relaxed `devCsp` used only during development.
- Narrowed Tauri capabilities — dropped the broad `core:default` set in favor of
  an explicit allowlist.
- Named-pipe IPC locked down with an SDDL ACL so only the intended principals
  can connect.
- IPC token relocated to `%PROGRAMDATA%` with a restrictive ACL.
- `freezePrototype` enabled to harden the webview against prototype pollution.

## Versioning policy

evorift follows Semantic Versioning, interpreted for a DPI-bypass tool as:

- **PATCH** (`0.0.x`) — bypass-strategy tuning or hostlist changes; no
  user-facing API or behavior contract change.
- **MINOR** (`0.x.0`) — new features added in a backward-compatible way.
- **MAJOR** (`x.0.0`) — a breaking change to the UI ↔ service IPC protocol (or
  any other incompatible contract change).

[Unreleased]: https://github.com/evorift/rift/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/evorift/rift/compare/v0.1.0...v0.1.2
[0.1.0]: https://github.com/evorift/rift/releases/tag/v0.1.0
[0.1.0 Known Issues]: #010--bilinen-sorunlar--known-issues
