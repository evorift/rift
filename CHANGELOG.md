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

[Unreleased]: https://github.com/evorift/rift/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/evorift/rift/releases/tag/v0.1.0
