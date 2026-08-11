# evorift — Backend Master Plan

_Generated 2026-06-13. Goal: a backend that can run **every function** described in the net3 docs
(`net3/docs/00–07` + `SOLUTION.md`). Build backend first, **test after each stage**, then move to the
frontend (a redesign with many features that must keep `BlackHole.svelte` + settings working)._

## How we execute this plan
- **One item per message.** When you send a message, I implement exactly **one** item — the next
  unchecked one, or the one you name — verify it (`cargo check --all-targets` + `cargo test` +
  `evorift-ctl` where relevant), mark it `[x]`, and stop.
- Each item lists: **goal**, **files**, **functions/signatures**, **test**.
- `[x]` = done · `[ ]` = todo · `[~]` = partial (needs expansion).
- Verify order per CLAUDE.md: `cargo check` → (frontend later) `svelte-check` → `npm run build`.

## Status legend of what already exists (Phase 0 — DONE)
The blueprint skeleton + net3 core already landed (see `NET3-ARASTIRMA-ENTEGRASYON-PLANI.md`):
- `engine.rs` `BypassEngine` trait + `EngineKind`/`EngineCaps`/`EngineInfo` + `catalog()` + `make_engine(id)`.
- `WinwsEngine` (proven catch-all winws args, Türkcell live-verified), `SimEngine`.
- `byedpi.rs` / `goodbyedpi.rs` scaffolds (need bundled binaries).
- `warp.rs` WARP split-tunnel (Discord IPs) + full-tunnel + DNS pinning.
- `sys.rs`/`dns.rs`/`firewall.rs`/`tweak.rs`/`limit.rs`/`repair.rs` system-ops layer.
- `profile.rs` (CRUD + seed + tests), `rollback.rs` (transaction log + tests), `preflight.rs`
  (preflight + diagnose), `autopilot.rs` (RunOnce skeleton).
- `ipc.rs` (+13 commands, `Response::Data`, `EngineStatus.{engine,state}`), `service.rs` (state machine
  Idle/Applying/Active/Paused/Error + dispatch + watchdog), `client.rs`, `lib.rs` (Tauri commands).
- Serviceless mode (`svcctl.rs` + `runtime_mode`/`install_service`/`uninstall_service`): elevated UI
  runs engine in-process; service is optional "extra protection" (boot persistence).
- Native Win32 `netinfo.rs` + `pid_scan.rs`; IPC token + ACL security; kill-on-close Job objects.

---

## Phase 1 — Strategy model & winws (zapret) full parameterization
_Make winws fully data-driven (docs/03 §4, docs/07 §3 "strategy = data"), not a single fixed catch-all._

- [x] **1.1 Typed strategy model.** ✅ DONE (2026-06-13). Extended `engine::Strategy` with `ttl`,
  `autottl`, `seqovl`, `wf_tcp`, `wf_udp`, `fake_quic`, `hostlist_only` (on top of desync/split_pos/
  repeats/fooling/fake_tls_mod). Added `Strategy::tls_profile_args()` builder (emits only set fields).
  `WinwsEngine::args()` now builds its PRIMARY TLS/443 stage from the selected strategy — default `c1`
  reproduces the live-verified Türkcell args byte-for-byte (test-locked). _Files:_ `engine.rs`.
  _Test:_ 4 unit tests (c1 byte-for-byte, omit-unset, fake, ttl/autottl/seqovl) — `cargo test` 13/13 pass.
- [x] **1.2 ISP preset catalog.** ✅ DONE (2026-06-13). Added `engine::presets()` with all 7 docs/03 §4.1
  presets verbatim (`tt`, `tt-alt`, `superonline`, `superonline-alt`, `kablonet`, `turkcell-hotspot`,
  `vodafone-hotspot`) as `Strategy` constants carrying per-ISP desync/ttl/autottl/fooling + `wf_tcp`/
  `wf_udp` voice ports. `strategy_by_id` now resolves generic strategies **then** presets (so profiles
  can select an ISP preset by id). _Files:_ `engine.rs`, `bin/evorift-ctl.rs`. _Test:_ `evorift-ctl strats`
  dumps all strategies+presets with their built args; 2 unit tests (resolve/tune + unique-slug) — 15/15 pass.
- [x] **1.3 Hostlist mode toggle.** ✅ DONE (2026-06-13). `WinwsEngine::args()` now takes the hostlist;
  when `strategy.hostlist_only && !hostlist.is_empty()` it writes `%PROGRAMDATA%\evorift\hostlist.txt`
  and appends `--hostlist=<file>` to the TCP/80, TLS primary, TLS secondary, and QUIC groups (the
  Discord voice/STUN group stays catch-all — STUN has no hostname). Default off → byte-for-byte the
  proven catch-all. Wired through `service.rs`: new `Engine.hostlist_only` runtime field +
  `Engine::current_strategy()` (overlays the override onto the resolved strategy at every call site);
  `apply_profile` sets it from `scope.mode == Split`. _Files:_ `engine.rs`, `service.rs`. _Test:_
  `hostlist_mode_toggles_flag` (on emits `--hostlist`, off none, voice group never gated) — 16/16 pass.
- [x] **1.4 Fake `.bin` payloads + voice ports.** ✅ DONE (2026-06-13). `args()` resolves the fake QUIC/
  voice payload from `strategy.fake_quic` (QUIC + Discord/STUN fake stages) with a bundle-existence
  fallback to the always-shipped `quic_initial_www_google_com.bin` (our bundle ships only that one
  `.bin`; other SplitWire payloads aren't bundled). `--wf-tcp` now comes from `strategy.wf_tcp`, and
  `--wf-udp` is emitted **only when `strategy.wf_udp` is set**: generic strategies keep `wf_udp=""`
  (raw-part signature capture = proven default, byte-for-byte), ISP presets add their voice port ranges
  (e.g. `--wf-udp=443,50000,50100`). _Files:_ `engine.rs`. _Test:_ `fake_bin_and_voice_ports` (bundled
  `.bin` exists + referenced; generic has no `--wf-udp`; `tt` preset adds voice ports) — 17/17 pass.
- [x] **1.5 `SetStrategy` accepts full preset ids.** ✅ DONE (2026-06-13). `ipc::validate` now accepts
  "auto" + any id in `engine::strategies()`/`presets()` (derived from the catalog — no second whitelist),
  so `tt`, `vodafone-hotspot`, etc. are valid. Dispatch forces a live winws restart on change (start()
  alone is idempotent and would no-op on a live child) and surfaces errors via the state machine.
  `evorift-ctl strat <preset>` applies; status reflects the new strategy. _Files:_ `ipc.rs`, `service.rs`.
  _Test:_ `set_strategy_accepts_presets_and_generics` (all generics+presets accepted, unknown/empty
  rejected) — 18/18 pass.

**✅ Phase 1 complete (1.1–1.5).** winws/zapret is now fully data-driven: typed strategies, 7 ISP presets,
hostlist mode, selectable fake payloads + voice ports, and runtime preset selection — all while the
default `auto`/`c1` reproduces the live-verified Türkcell config byte-for-byte.

## Phase 2 — GoodbyeDPI engine (full)
_docs/01 (all flags + modes -1..-9), docs/03 §5 (presets + blacklist + DNS-redirect)._

- [x] **2.1 Mode + flag model.** ✅ DONE (2026-06-13). Added `GdMode` (M1..M9) with `from_u8`/`num`/
  `expand()` — each modeset expands to its documented flag combination (docs/01 §3) verbatim (legacy
  `-1..-4` = `-p -r -s …`; modern `-5..-9` = `-f 2 -e 2 … --reverse-frag --max-payload [-q]`). The engine
  carries a `mode` field (default M9 = GoodbyeDPI's own default); `build_args`/`start` emit the explicit
  expansion (transparent command line, docs/06 §1.4) — GoodbyeDPI accepts these identically to `-N`.
  _Files:_ `goodbyedpi.rs`. _Test:_ `mode9_matches_doc` + `mode_roundtrip_and_legacy` — 20/20 pass.
- [x] **2.2 DNS-redirect.** ✅ DONE (2026-06-13). Added typed `DnsRedirect { v4, v4_port, v6, v6_port }`
  with `cloudflare()` (standard :53) and `yandex_nonstandard()` (:1253 — the docs/01 §4 Turkey trick that
  beats UDP/53 hijacking) + `to_args()`. Engine carries `dns: Option<DnsRedirect>` (default Cloudflare:53,
  matching the proven SplitWire preset) + `set_dns_redirect()`; `gd_args` appends the `--dns-addr/-port/
  --dnsv6-addr/-port` flags. _Files:_ `goodbyedpi.rs`. _Test:_ `dns_redirect_args` (standard + nonstandard
  port) + `build_args_includes_mode_and_dns` — 22/22 pass.
- [x] **2.3 Blacklist file.** ✅ DONE (2026-06-13). `default_blacklist()` = docs/03 §5.2 list verbatim
  (Discord/Roblox/arkoselabs/…); `write_blacklist_to()` writes it one-per-line. Engine carries
  `blacklist: Option<Vec<String>>` (None = system-wide, default) + `set_blacklist()`; when set, `gd_args`
  writes `%PROGRAMDATA%\evorift\gd-blacklist.txt` and appends `--blacklist <file>`. _Files:_ `goodbyedpi.rs`.
  _Test:_ `blacklist_written_and_default_content` (file written + has discord/roblox) +
  `build_args_blacklist_toggle` (flag present only when set) — 24/24 pass.
- [x] **2.4 Presets.** ✅ DONE (2026-06-13). `GdPreset {mode, set_ttl, dns}` + `presets()` catalog from
  docs/03 §5.1 (`standard` = -5 + set-ttl 5 + DNS, `mode5..mode9`, `mode9-dns`, `ttl3`). Engine gained a
  `set_ttl` field (`--set-ttl <n>`, 0=omit) + `apply_preset()`; start/stop/availability already in place.
  Added `evorift-ctl gd-presets` (local dump of each preset's built args). _Files:_ `goodbyedpi.rs`,
  `bin/evorift-ctl.rs`. _Test:_ `preset_catalog_standard` + `apply_preset_build_args` — 26/26 pass.

**✅ Phase 2 complete (2.1–2.4).** GoodbyeDPI is a full `BypassEngine`: modes -1..-9, DNS-redirect
(incl. nonstandard-port Turkey trick), blacklist, and the docs/03 §5.1 preset catalog — Job-object
managed, available-gated, single-instance (kill-image). Needs `resources/goodbyedpi/goodbyedpi.exe` bundled
to run for real; absent → logged sim no-op (boot never broken).

## Phase 3 — ByeDPI + routing (ProxiFyre / drover)
_docs/03 §2–3, docs/04 §4–6. Kernel-less path for Kaspersky/AV machines._

- [x] **3.1 ByeDPI param model.** ✅ DONE (2026-06-13). Added typed `ByeDpiConfig { port, split, disorder,
  fake, mod_http, tlsrec, auto, ttl, oob }` + `to_args()` (matches the documented invocation order;
  `--port` omitted at the default 1080). `ByeDpiConfig::balanced()` = the proven SplitWire default
  (`--split 1 --disorder 3+s --mod-http=h,d --auto=torst --tlsrec 1+s`, docs/03 §2.1). `ByeDpiPreset` +
  `presets()` (balanced/split/fake). Engine carries `config` (default balanced) + `set_config`/
  `apply_preset`; `build_args` emits `config.to_args()`. _Files:_ `byedpi.rs`. _Test:_ `balanced_matches_doc`
  (byte-for-byte) + `nondefault_fields_and_presets` — 28/28 pass.
- [x] **3.2 ProxiFyre adapter.** ✅ DONE (2026-06-13). New `proxifyre.rs`: `app_config_json(browsers,
  port)` builds the docs/03 §2.3 config (serde `camelCase` → `appNames`/`socks5ProxyEndpoint`/
  `supportedProtocols` TCP+UDP); core apps (Discord/Roblox/WebCord) + optional browser list toggle;
  `write_config_to()` writes `app-config.json` next to ProxiFyre.exe. `install(browsers, port)` writes
  the config, runs `ProxiFyre.exe install` (cwd=bundle), `sc config ProxiFyreService start= auto`,
  `net start`, and opens allow firewall rules for ProxiFyre + ciadpi; `uninstall()` reverses it. All
  privileged ops via `sys` (+ new `sys::run_os_cwd`) → sim when unprivileged; absent bundle → sim no-op.
  _Files:_ `proxifyre.rs`, `sys.rs`, `lib.rs`. _Test:_ `config_json_core` + `browser_toggle_and_custom_port`
  + `write_config_writes_file` — 31/31 pass.
- [x] **3.3 drover adapter.** ✅ DONE (2026-06-13). New `drover.rs`: `drover_ini(port)` =
  `[drover]\nproxy = socks5://127.0.0.1:<port>`; `discord_app_dirs()` finds `app-*` under
  `%LOCALAPPDATA%\{Discord,DiscordPTB,DiscordCanary}`; `install_to()` copies `version.dll` + writes
  `drover.ini` into each (returns the written paths for rollback); `install(port)` resolves the bundled
  DLL + app dirs; `remove_all()` deletes them (only when our `drover.ini` marker is present → never
  touches a foreign version.dll). Runs in the USER context (user profile). _Files:_ `drover.rs`, `lib.rs`.
  _Test:_ `drover_ini_content` + `install_to_copies_and_rollback_removes` (install into a temp app-* dir,
  then `RollbackLog`/`Change::FileCopied` removes both files) — 33/33 pass.
- [x] **3.4 Composite modes.** ✅ DONE (2026-06-13). Added `byedpi::Routing { None, ProxiFyre{browsers},
  Drover }` + `ByeDpiEngine::with_routing()`; `id()` reflects routing (`byedpi`/`byedpi-proxifyre`/
  `byedpi-drover`). `start()` brings up ciadpi then the routing layer (ProxiFyre install or drover inject,
  best-effort); `stop()` tears it down (proxifyre::uninstall / drover::remove_all). Factory `make_engine`,
  `catalog()`, display names, and `ipc::validate` SetEngine all accept the two composite ids → selectable
  via `evorift-ctl engine byedpi-proxifyre|byedpi-drover` and profiles. _Files:_ `byedpi.rs`, `engine.rs`,
  `ipc.rs`. _Test:_ `composite_modes` + `catalog_includes_composite_byedpi` — 35/35 pass.

**✅ Phase 3 complete (3.1–3.4).** Kernel-less ByeDPI path is whole: typed ciadpi params + presets, the
ProxiFyre split-tunnel adapter, the drover Discord-only DLL-hijack adapter, and both composite modes
selectable as engines. Needs `resources/{byedpi,proxifyre,drover}/` binaries bundled to run for real;
absent → sim no-ops (boot never broken).

## Phase 4 — Tunnel engines (WARP full; WireSock; app-based)
_docs/03 §1, docs/04 §7–8. WARP IP-split is done; add app-based + WireSock + wgcf lifecycle._

- [x] **4.1 WARP split-tunnel (Discord IPs) + full-tunnel + DNS pinning** — done (`warp.rs`).
- [x] **4.2 wgcf lifecycle.** ✅ DONE (2026-06-13). Added `account_path`/`profile_path` + `WGCF_ACCOUNT`/
  `WGCF_PROFILE` consts; `should_generate()` (generate-when-missing, reuse-when-present so the keypair stays
  stable) + `profile_is_stale(max_age_days)` (`PROFILE_MAX_AGE_DAYS=7`, advisory) + `refresh_profile()`
  (delete cached profile → regenerate, keep account). `register_error_message()` classifies the Cloudflare
  abusive-usage/429/forbidden block into an actionable message vs a generic network error. `ensure_profile`
  now uses these + **hardens the account & profile file ACLs** (they hold the private WARP credentials/key),
  not just `warp.conf`. _Files:_ `warp.rs`. _Test:_ `should_generate_missing_reuse_present` +
  `register_error_classification` — 37/37 pass.
- [x] **4.3 MTU clamp + AllowedApps option.** ✅ DONE (2026-06-13). `set_mtu(conf, mtu)` rewrites the
  `[Interface]` MTU line (replace if present, inject after the header if missing) so restrictive ISPs can
  clamp 1280→1200/1180 (PMTUD blackholing); `MTU_DEFAULT=1280`. `app_tunnel_config(profile, apps, endpoint)`
  builds the WireSock-style **app-based** conf (full `AllowedIPs` + `AllowedApps = <names>`, endpoint
  replaced loop-safe, DNS dropped) for the WireSock adapter (4.4); `default_tunnel_apps()` = docs/03 §1.2
  Discord/Roblox list. (`AllowedApps` is a WireSock extension — official `wireguard.exe` keeps IP-split.)
  _Files:_ `warp.rs`. _Test:_ `set_mtu_replace_and_inject` + `app_tunnel_config_has_apps_and_full_ips` — 39/39 pass.
- [x] **4.4 WireSock adapter.** ✅ DONE (2026-06-13). New `wiresock.rs`: `build_conf(profile, apps, mtu)`
  derives a WireSock app-based conf from the cached wgcf profile via `warp::app_tunnel_config` + optional
  `set_mtu` (pure). `write_config()` writes `wiresock.conf` (ACL-hardened — private key) from the wgcf
  profile (errors if WARP not set up yet). `install(apps, mtu)` runs `wiresock-client.exe install
  -start-type 2 -config <conf> -log-level none` + `net start wiresock-client-service`; `uninstall()`
  reverses it. `SERVICE_NAME`/`is_available()` for the manager (5.x). Absent bundle/unprivileged → sim.
  _Files:_ `wiresock.rs`, `lib.rs`. _Test:_ `build_conf_has_apps_mtu_endpoint` + `install_uninstall_sim` — 41/41 pass.
- [x] **4.5 Tunnel refresh task.** ✅ DONE (2026-06-13). New `schtask.rs`: `register_script(interval_min)`
  builds the `Register-ScheduledTask` PowerShell (restarts `$env:EVORIFT_REFRESH_SVC` every N min,
  SYSTEM/Highest, ~10y repetition — docs/03 §1.5 WireSockRefresh); service name via `$env` (no injection).
  `create_refresh_task(service, interval_min)` / `delete_task()` / `task_exists()`. Privileged → sim when
  unprivileged. _Files:_ `schtask.rs`, `lib.rs`. _Test:_ `register_script_content` + `create_delete_sim` — 43/43 pass.

**✅ Phase 4 complete (4.1–4.5).** Tunnel layer is whole: WARP IP-split + full-tunnel + DNS pinning, wgcf
lifecycle, MTU clamp, WireSock-style app-based tunneling, the WireSock adapter, and an opt-in refresh task.

## Phase 5 — Service manager (multi-service) + WinDivert conflict
_docs/02 §4, docs/05 §3. Generic, dependency-ordered._

- [x] **5.1 evorift-svc install/uninstall/status** — done (`svcctl.rs`).
- [x] **5.2 Generic service manager.** ✅ DONE (2026-06-13). New `services.rs`: `MANAGED_SERVICES` (docs/02
  §4 list + `EvoriftSvc`); `parse_state()` (pure: `sc query` → running/stopped/pending/unknown/not-installed);
  `query()`/`list()` (read-only) → `ServiceStatus`; `install(name, bin, args, display, desc)` (`sc create
  binPath= "<bin> <args>" start= auto` + description + start) / `uninstall(name)` (stop+delete), privileged
  → sim. `evorift-ctl services` dumps all states. _Files:_ `services.rs`, `lib.rs`, `bin/evorift-ctl.rs`.
  _Test:_ `parse_state_cases` + `managed_list_and_install_sim` — 45/45 pass.
- [x] **5.3 Remove-all (dependency order).** ✅ DONE (2026-06-13). `TEARDOWN_ORDER` (docs/05 §3:
  EvoriftSvc→zapret→GoodbyeDPI→WinDivert→winws1/2→wiresock→ByeDPI→ProxiFyre — consumers before WinDivert)
  + `remove_all()`: for each INSTALLED service, `sc stop` → wait until not-running (avoids DELETE_PENDING)
  → `sc delete`, audit-logged; skips not-installed (idempotent); returns the acted-on names. Privileged → sim.
  _Files:_ `services.rs`. _Test:_ `teardown_order_dependency_safe` (WinDivert after consumers, proxy last,
  no dups, all managed) — 46/46 pass.

**✅ Phase 5 complete (5.1–5.4).** Multi-service management: evorift-svc lifecycle, a generic
install/uninstall/status/list manager, dependency-ordered remove-all, and WinDivert conflict scan+clean.
- [x] **5.4 WinDivert conflict scan + clean** — done (`preflight::windivert_conflict` +
  `engine::clear_stale_windivert`). _(Verify only.)_

## Phase 6 — Auto-Pilot (blockcheck-equivalent), full
_docs/03 §4.4, docs/06 §2.2, docs/07 §6. Native strategy finder (no Cygwin/bash)._

- [x] **6.1 Candidate matrix.** ✅ DONE (2026-06-13). `candidate_matrix()` now enumerates every base DPI
  engine × all its strategies/presets — zapret: `strategies()`+`presets()` (generic + 7 ISP presets);
  goodbyedpi: `goodbyedpi::presets()`; byedpi: `byedpi::presets()`. Deterministic (not availability-gated);
  `candidates()` applies the `is_available` filter so Auto-Pilot only tests runnable engines. Tunnels and
  composite routing modes excluded (Auto-Pilot finds a desync strategy, not routing). _Files:_ `autopilot.rs`.
  _Test:_ `candidate_matrix_covers_all_presets` (count = zapret+gd+byedpi presets, ISP preset present) — 47/47 pass.
- [x] **6.2 Real connectivity scoring.** ✅ DONE (2026-06-13). Replaced the TCP-only probe with a
  TLS-handshake-aware one (std-only, no new deps): DNS → TCP/443 → send a hand-built **ClientHello with the
  target SNI** → classify the response — `0x16` ServerHello / `0x15` Alert = reachable (SNI not reset = open),
  RST/silence/timeout = blocked. This is the real DPI signal (TCP connects but the censor resets the TLS).
  `compute_score()` extracted (opened-count dominant, latency penalty). _Files:_ `autopilot.rs`. _Test:_
  `client_hello_is_valid_tls_with_sni` + `tls_response_classification` + `score_distinguishes_open_from_blocked` — 50/50 pass.
- [x] **6.3 Scan depth.** ✅ DONE (2026-06-13). `Depth` is now `Quick`/`Standard`/`Force` with
  `from_str_lenient` (quick/fast→Quick, force/full→Force, else Standard) + `early_stop()`. `candidates_for(depth)`:
  Quick = curated shortlist (zapret `c1` + 7 ISP presets), Standard/Force = full matrix; `run()` early-stops
  at first all-open for Quick/Standard, tries every candidate for Force. Wired through `service.rs`
  (from_str_lenient), `ipc.rs` (depth whitelist), and `evorift-ctl auto [quick|standard|force] <targets>`.
  _Files:_ `autopilot.rs`, `service.rs`, `ipc.rs`, `bin/evorift-ctl.rs`. _Test:_ `depth_levels_differ` — 51/51 pass.
- [x] **6.4 ISP detection (opt-in).** ✅ DONE (2026-06-13). `detect_isp(consent)` returns `None` without
  consent (never sends the user's IP anywhere); with consent, a one-shot `ipinfo.io/org` lookup (best-effort).
  `isp_preset_id()` maps an org name → ISP preset (Türk Telekom→tt, SuperOnline→superonline checked before
  Turkcell→turkcell-hotspot, Vodafone→vodafone-hotspot, Kablo→kablonet); `prioritize_for_isp()` reorders the
  candidate list to try the matching preset first (stable otherwise). `best_as_profile` uses `detect_isp(false)`.
  _Files:_ `autopilot.rs`. _Test:_ `isp_detection_opt_in_and_mapping` (None without consent + mapping + prioritization) — 52/52 pass.
- [x] **6.5 Streaming progress.** ✅ DONE (2026-06-13). `run()` refactored into a testable `run_with()`
  streaming driver (scores each candidate → fires `on_progress` immediately → honors early-stop). New
  `Request::AutoPilotStream { targets, depth }`: the service pauses the active engine, runs the scan writing
  one `Response::Data(row_json)` per candidate as it finishes, then a terminal `Response::Ok`, and restores
  the engine. `client::autopilot_stream(targets, depth, on_row)` consumes the live rows. The blocking
  `Command::AutoPilot` stays for the CLI. _Files:_ `autopilot.rs`, `ipc.rs`, `service.rs`, `client.rs`.
  _Test:_ `run_with_streams_incrementally` (rows arrive one-by-one; early-stop) — 53/53 pass.

**✅ Phase 6 complete (6.1–6.5).** Native Auto-Pilot (no Cygwin/bash blockcheck): full engine×preset
candidate matrix, TLS-handshake-aware scoring, quick/standard/force depth, opt-in ISP prioritization, and
live streamed results.

## Phase 7 — Profiles (full) + rollback coverage
_docs/07 §4–5, docs/06 §2.4._

- [x] **7.1 Full profile schema.** ✅ DONE (2026-06-13). Added `Profile.engine_params` (flexible
  `serde_json::Value` bag the per-engine apply logic reads, e.g. goodbyedpi `{mode,set_ttl,dns}`),
  `Scope.folders` (custom app folders, docs/03 §1.2), and widened the engine whitelist to the composite
  ByeDPI ids. `isp` preset link + `scope.browsers` already present. All new fields `#[serde(default)]` →
  old profile JSON still loads. _Files:_ `profile.rs`, `autopilot.rs`. _Test:_
  `full_schema_roundtrip_and_back_compat` (engine_params/folders/isp round-trip + back-compat + composite
  engine valid) — 54/54 pass.
- [x] **7.2 Apply across engine types.** ✅ DONE (2026-06-13). Split into `apply_profile` (loads by id) +
  `apply_profile_obj` which branches by engine TYPE: **tunnel** (`engine=="warp"`) brings up WARP instead of
  a DPI engine (full-tunnel for System scope, split for Split); **desync/local-proxy** rebuild the engine via
  the new `engine::make_engine_with_params(id, engine_params)` (applies goodbyedpi/byedpi presets from
  `{"preset":...}` + ByeDPI routing) then start with strategy/hostlist/hostlist-mode. DNS factored into
  `apply_profile_dns`. _Files:_ `service.rs`, `engine.rs`, `profile.rs`. _Test:_ `apply_three_engine_kinds`
  (zapret desync / byedpi local-proxy + hostlist mode / warp tunnel + full) — 55/55 pass.
- [x] **7.3 Rollback coverage for all mutations.** ✅ DONE (2026-06-13). Made `rollback` a **process-global**
  log (`static Mutex<RollbackLog>` + `record`/`rollback_all`/`is_empty`/`load_global`) so every mutation site
  records into one place (engines call proxifyre/drover directly, with no Engine handle). Added
  `Change::ScheduledTask` (+ Unregister undo). Records added: drover `FileCopied` (per file, always — real
  user writes), proxifyre `ServiceCreated`+`FirewallRule×2`, schtask `ScheduledTask`, wiresock/services
  `ServiceCreated` (gated on `privileged()` so sim no-ops don't record); `service.rs` `DnsChanged`/
  `FirewallRule`/`TunnelInstalled` now use the global; `RollbackAll` reverses it; boot `load_global()` recovers
  crashes. _Files:_ `rollback.rs`, `service.rs`, `proxifyre.rs`, `drover.rs`, `schtask.rs`, `wiresock.rs`,
  `services.rs`. _Test:_ `rollback_all_reverses_full_apply` (FileCopied really deleted, rest sim, log empty) — 56/56 pass.
- [x] **7.4 Import/export + community presets (opt-in).** ✅ DONE (2026-06-13). `parse_bundle()` (JSON array
  of profiles, validates each, ≤200) + `import_bundle()` (parse+save). `fetch_community(url)` — USER-INITIATED
  opt-in download of a versioned preset bundle: strict URL validation (HTTPS + safe chars → no injection),
  `Invoke-WebRequest` fetch, parse+validate (does NOT auto-save). export/import already present. _Files:_
  `profile.rs`. _Test:_ `bundle_parse_and_url_validation` (2-profile bundle parses, bad profile/JSON rejected,
  non-HTTPS + unsafe URLs rejected without network) — 57/57 pass.

**✅ Phase 7 complete (7.1–7.4).** Profiles are whole: full schema (engine_params/folders/isp), apply across
desync/local-proxy/tunnel engine types, global rollback covering every mutation site, and import/export +
opt-in community preset fetch.

## Phase 8 — Diagnostics, logging, preflight (full)
_docs/02 §5, docs/06 §2.5, docs/07 §8._

- [x] **8.1 File logging.** ✅ DONE (2026-06-13). Added a rotating file logger in `sys.rs`:
  `write_log_to(dir, category, line)` appends to `%PROGRAMDATA%\evorift\logs\<category>.log`, rotates to
  `<category>.old.log` at 1 MB (`LOG_MAX_BYTES`), and writes a `=== evorift <version> <category> log [ts] ===`
  header on a fresh/rotated file. `audit()` now also writes `audit.log` (kept stderr); `log(category, line)`
  for per-operation logs (dns/repair/setup). _Files:_ `sys.rs`. _Test:_ `log_writes_header_and_rotation_predicate`
  (file written, header once, rotation predicate) — 58/58 pass.
- [x] **8.2 Log bundle.** ✅ DONE (2026-06-13). New `logbundle.rs`: `system_summary()` gathers a
  secret-free plaintext report (version, OS, elevation, preflight checks, DNS state, engine availability,
  service states); `write_summary(dir)` drops `system-summary.txt`; `create_bundle(out_zip)` writes the
  summary into the logs dir then zips it via `Compress-Archive` (paths via `$env` → no injection; no zip
  crate). _Files:_ `logbundle.rs`, `lib.rs`. _Test:_ `summary_write_and_compress_script` (sections present,
  file written, injection-safe compress script) — 59/59 pass.
- [x] **8.3 Preflight (full).** ✅ DONE (2026-06-13). Added 4 checks to `preflight::run()`:
  `av_blocks_windivert()` (Kaspersky service names → "use ByeDPI" hint), `vc_redist_present()` (vcruntime140.dll
  → winws/ciadpi need it), `packet_filter_present()` (ndisrd service → ProxiFyre needs it),
  `warp_register_reachable()` (TCP probe to the Cloudflare WARP API). Each yields a concrete hint; criticals
  unchanged (admin+winws+conflict). _Files:_ `preflight.rs`. _Test:_ `preflight_full_checks_have_hints`
  (all 4 present + every check has a hint) — 60/60 pass.
- [x] **8.4 Diagnose chain.** ✅ DONE (2026-06-13). `diagnose()` now runs DNS → TCP/443 → **TLS ClientHello
  (with SNI)** per target (reusing autopilot's `client_hello`/`tls_responded`, now `pub(crate)`); `TargetDiag`
  gained `tls_ok` + a `suggestion` string from the first failing step via pure `diagnose_suggestion()`
  (DNS→provider, TCP→WARP, TLS→DPI-engine, all-ok→reachable). HTTP-over-TLS isn't separately probed (needs a
  full TLS lib); TLS reachability is the meaningful DPI signal. _Files:_ `preflight.rs`, `autopilot.rs`.
  _Test:_ `diagnose_suggestions_per_step` — 61/61 pass.

**✅ Phase 8 complete (8.1–8.4).** Diagnostics: rotating file logs, support log bundle, full preflight
(admin/winws/conflict/AV/VC++/packet-filter/WARP/DoH), and a DNS→TCP→TLS diagnose chain with concrete fixes.

## Phase 9 — Repair & helpers (full)
_docs/05 §2._

- [x] **9.1 Discord cache repair + find_discord_path + WebCord** — done (`repair.rs`).
- [x] **9.2 Discord reinstall flow.** ✅ DONE (2026-06-13). `installer_url(channel)` builds the official
  docs/05 §2 URL (`discord.com/api/downloads/distributions/app/installers/latest?channel=…&platform=win&arch=x64`,
  channel normalized to stable/ptb/canary); `reinstall_discord(channel)` kills Discord processes → clears
  cache (`repair_discord`) → downloads the installer (`Invoke-WebRequest`, URL/dest via `$env`) → launches it
  (silent by default). User context. _Files:_ `repair.rs`. _Test:_ `discord_installer_url_and_dest` (dry-run
  URL/dest for all channels + unknown→stable) — 62/62 pass.
- [x] **9.3 Discord PTB install.** ✅ DONE (2026-06-14). Added `ptb_url()` (direct `discord.com/api/download/ptb?platform=win`
  endpoint — the short legacy URL that always resolves to the latest PTB build, complementing
  `reinstall_discord("ptb")` which uses the distributions API) + `ptb_dest()` + `install_discord_ptb()`:
  kills DiscordPTB.exe/Update.exe → downloads via `Invoke-WebRequest` (URL/dest via `$env` → no injection)
  → launches the installer. Exposed as Tauri command `install_discord_ptb`. _Files:_ `repair.rs`, `lib.rs`.
  _Test:_ `discord_ptb_url_and_dest` (URL prefix + `platform=win` + dest named `DiscordPTBSetup` + `.exe`
  extension) — 63/63 pass.
- [x] **9.4 Generic app-proxy wizard.** ✅ DONE (2026-06-14). `add_app_to_config(dir, exe_name, port)` reads the
  existing `app-config.json` (or creates a fresh default), inserts `exe_name` into the first proxy entry's
  `appNames` (idempotent — no duplicate), and returns the updated JSON. `write_app_config_with_app()` writes it
  to disk. `proxy_app(exe_name, port)` — privileged wizard: writes the config then `sc stop`/`net start`
  ProxiFyreService so the change is live immediately (absent service → silent no-op). `repair::attach_app_to_proxy(exe_name)`
  — user-facing wrapper: validates that `exe_name` is a bare filename (rejects path separators → no config injection)
  then delegates to `proxy_app`. Exposed as Tauri command `attach_app_to_proxy`. _Files:_ `proxifyre.rs`, `repair.rs`,
  `lib.rs`. _Test:_ `add_app_to_config_and_idempotent` (fresh config adds app + preserves core apps; second call
  no-dups; write_app_config_with_app writes file with new app) — 64/64 pass.

## Phase 10 — Hardening & integration
- [x] **10.1 Generalize kill-on-close Job.** ✅ DONE (2026-06-14). Added `proc::spawn_with_job(cmd)` —
  spawns a `Command`, creates a KILL_ON_JOB_CLOSE job, assigns the child, returns `(child, job)`. All
  adapters that spawn long-running children now use it: `goodbyedpi.rs` and `byedpi.rs` migrated from the
  old 3-step pattern (spawn → create_job → assign). `WinwsEngine` (`engine.rs`) `ensure_job`/`assign_to_job`
  methods replaced with thin wrappers around `proc::create_kill_on_close_job` + `proc::assign_to_job`; inline
  `kill_all` replaced with `proc::kill_image`. Short-lived helpers (sc/netsh/wgcf/wireguard installs) use
  `.output()` and complete synchronously — no Job needed. _Files:_ `proc.rs`, `engine.rs`, `goodbyedpi.rs`,
  `byedpi.rs`. _Test:_ `spawn_with_job_assigns_to_job` — spawns `cmd /c exit 0`, checks `IsProcessInJob`
  API returns in_job=1 (Windows Job API verified end-to-end) — 65/65 pass.
- [x] **10.2 IPC validate for all new commands + DoS limits.** ✅ DONE (2026-06-14). Added `MAX_MSG_BYTES=256KiB`
  enforced in `read_msg` (rejects oversized lines before parsing). Added `MAX_TOKEN_BYTES=1024` cap for `Hello`
  token. New `validate_request(req: &Request) -> Result<(), String>` covers all variants: `Hello` (token size),
  `Command` (delegates to existing `validate()`), `AutoPilotStream` (reuses `Command::AutoPilot` target-count/
  domain/depth checks), `Subscribe` (always ok). Wired as the first gate in `service.rs`'s dispatch loop —
  invalid requests are audit-logged and receive an error response before reaching any handler. _Files:_ `ipc.rs`,
  `service.rs`. _Test:_ `validate_request_gates_all_variants` (Hello size / Command delegation / AutoPilotStream
  empty+bad-depth+51-targets / Subscribe) + `read_msg_rejects_oversized` — 67/67 pass.
- [x] **10.3 Binary manifest + SHA-256.** ✅ DONE (2026-06-14). New `manifest.rs`: `BinaryEntry {name,
  version, sha256, source, path}` + `VerifyResult {name, path, expected/actual sha256, ok}`. `sha256_file()`
  uses the `sha2` crate (RustCrypto, pure Rust). `load_manifest(path)` parses `manifest.json`. `verify_manifest(entries,
  base_dir)` hashes each listed file and compares — absent files → `ok=false, actual=None` (expected for optional
  bundles); mismatches audit-logged but NOT fatal. `verify_all()` resolves resources dir + loads manifest + verifies.
  Created `resources/manifest.json` with real SHA-256s of all 8 bundled binaries (computed from actual files on disk).
  Exposed as Tauri command `verify_manifest()` → JSON `Vec<VerifyResult>`. Added `sha2 = "0.10"` dep. _Files:_
  `manifest.rs`, `lib.rs`, `Cargo.toml`, `resources/manifest.json`. _Test:_ `sha256_file_correct_hash` (b"hello"
  → known hash), `verify_manifest_missing_file_not_ok`, `verify_manifest_correct_hash_is_ok`, `load_manifest_parses_json`
  — 71/71 pass.
- [x] **10.4 Final gate.** ✅ DONE (2026-06-14). `cargo check --all-targets` ✓ · `cargo test --lib` 71/71 ✓ ·
  `cargo clippy --all-targets` **0 warnings** ✓ · `grep wmic` → no matches ✓. Fixed all 6 clippy warnings:
  `sort_by` → `sort_by_key(Reverse)` (`autopilot.rs`); derived `Default` for `ScopeMode` (`profile.rs`); `vec![…]`
  → array literals × 2 (`profile.rs`); `loop`+match-break → `while let` (`service.rs`); doc blank-line
  (`warp.rs`); `&[x.clone()]` → `std::slice::from_ref(&x)` (`drover.rs`). _Files:_ `autopilot.rs`, `profile.rs`,
  `service.rs`, `warp.rs`, `drover.rs`.

## Phase 11 — Frontend handoff contract (backend side)
_New UI will be a redesign with many features but must keep `BlackHole.svelte` + settings working._

- [x] **11.1 WARP-up signal over IPC.** ✅ DONE (2026-06-14). Added `Command::TunnelStatus` → `Response::Data(TunnelState JSON)`.
  `TunnelState { warp_running, warp_full, tunnel_installed, handshake_ago_secs }`. New accessors on
  `WarpEngine`: `is_full()`, `is_installed()` (public wrapper for `tunnel_installed()` sc-query check),
  `handshake_ago_secs()` (queries WireGuard userspace pipe `\\.\pipe\ProtectedPrefix\Localsystem\WireGuard\warp`
  for `last_handshake_time_sec`; returns `None` when tunnel not running). `validate()` no-op arm covers
  `TunnelStatus`. Dispatch in `service.rs` returns JSON-serialized state. _Files:_ `ipc.rs`, `warp.rs`,
  `service.rs`. _Test:_ `tunnel_status_validates_and_state_roundtrips` (validate + round-trip, with/without
  handshake) + `warp_engine_accessors_track_state` (start/stop toggles + no-panic calls) — 73/73 pass.
- [x] **11.2 Derived health signal.** ✅ DONE (2026-06-14). Added `Command::Health` → `Response::Data(HealthSignal JSON)`.
  `HealthSignal { healthy, loading, error, ping_ms, loss_pct }`. Pure `derive_health(running, loading,
  error, metrics, age_secs)` function in `ipc.rs` (age injected, no `Instant` mocking needed in tests).
  `Engine` caches `last_metrics: Option<Metrics>` + `metrics_at: Option<Instant>` — the subscription
  thread's `telemetry_loop` updates them each tick. `Engine::health()` reads the cached state and calls
  `derive_health()` with `Instant::elapsed()`. `healthy = running && age<5s && loss<100 && ping>0`.
  _Files:_ `ipc.rs`, `service.rs`. _Test:_ `health_signal_derivation` (8 cases: running/stale/loss/ping/
  no-metrics/loading/error), `health_command_validates`, `health_signal_roundtrip`, `engine_health_derives_from_state_and_metrics`
  (idle/loading/error/healthy/no-metrics cases) — 77/77 pass.
- [x] **11.3 Contract doc.** ✅ DONE (2026-06-14). Created `FRONTEND-CONTRACT.md` — full command/event
  reference covering all 27 `Command` ops with parameters, response types, JSON shapes, constraint tables,
  example payloads; `Response` variants + field tables; `BlackHole` and Settings binding guidance;
  Tauri-command sidebar. Machine-checked by `ipc::tests::contract_doc_covers_all_commands` (reads the file
  at test time, asserts every op name + 6 payload type names present). _Files:_ `FRONTEND-CONTRACT.md`,
  `ipc.rs`. _Test:_ `contract_doc_covers_all_commands` — 78/78 pass.

---

## Coverage map — every net3 function → where it lands
| net3 doc | Capability | Phase |
|---|---|---|
| 01 | GoodbyeDPI modes -1..-9, all flags, DNS-redirect, blacklist | 2 |
| 03 §1 | WireSock/WARP wgcf, AllowedApps, refresh task | 4 |
| 03 §2 | ByeDPI ciadpi params + ProxiFyre app-config + firewall | 3 |
| 03 §3 | drover DLL hijack (Discord-only) | 3.3 |
| 03 §4 | Zapret presets, fooling/ttl/split, fake `.bin`, blockcheck | 1, 6 |
| 03 §5 | GoodbyeDPI presets + blacklist | 2 |
| 04 | WinDivert conflict, tool roles, NDIS packet filter | 5, 3.2 |
| 05 §1 | DNS + DoH set/reset/verify | done + 8 |
| 05 §2 | Discord/PTB/WebCord repair, find path | 9 |
| 05 §3 | service list, remove-all (ordered), uninstall | 5 |
| 06 | Auto-Pilot, profiles, diagnostics, simple/expert | 6, 7, 8 |
| 07 | IBypassEngine, state machine, rollback, preflight | done + 1,7,8 |
| SOLUTION.md | winws catch-all + WARP Discord split | done (Phase 0) |
