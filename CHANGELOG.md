# Changelog

All notable changes to **evorift** are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- _Nothing yet._

### Changed

- _Nothing yet._

### Fixed

- _Nothing yet._

### Security

- _Nothing yet._

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
