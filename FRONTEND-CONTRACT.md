# evorift — Frontend IPC Contract

This document defines the complete command/event surface that the UI (SvelteKit) and
`BlackHole.svelte` consume from the privileged service (`evorift-svc`). Any UI redesign
and any new component must bind against these shapes — no free-form commands exist.

**Last updated:** 2026-06-14 (item 11.3, all phases complete)

---

## Transport

| Property | Value |
|---|---|
| Protocol | Local named pipe, line-delimited JSON (one JSON object per `\n`) |
| Pipe name | `evorift-ipc.sock` → resolves to `\\.\pipe\evorift-ipc.sock` on Windows |
| Message size cap | 256 KiB per line (DoS guard — `ipc::MAX_MSG_BYTES`) |
| Connection model | One connection per logical session; `Subscribe` takes over the connection for streaming |
| Auth | First message on every connection **must** be `Hello { token }`. Token is in `%PROGRAMDATA%\evorift\ipc.token` |

---

## Client → Service (`Request`)

All requests are JSON objects with a `"type"` discriminant (snake_case).

```json
{ "type": "hello", "token": "<hex-string>" }
{ "type": "command", "cmd": { "op": "<command-op>", ... } }
{ "type": "subscribe" }
{ "type": "auto_pilot_stream", "targets": ["discord.com"], "depth": "quick" }
```

### `hello`
Must be the first message on every connection.

| Field | Type | Notes |
|---|---|---|
| `token` | `string` | Read from `%PROGRAMDATA%\evorift\ipc.token`; max 1024 bytes |

### `command`
Wraps a `Command` object (see section below). Returns a `Response`.

### `subscribe`
Takes over the connection: service streams `Telemetry` at ~1 Hz until the client
disconnects. No further requests are accepted on this connection.

### `auto_pilot_stream`
Streams Auto-Pilot results live. Service writes one `Response::Data(row_json)` per
candidate as it finishes, then a terminal `Response::Ok`.

| Field | Type | Constraints |
|---|---|---|
| `targets` | `string[]` | 1–50 domain names, ASCII alphanumeric + `.` + `-`, max 253 chars each |
| `depth` | `string` | `"quick"` \| `"standard"` \| `"force"` |

---

## Commands (`Command.op`)

All commands are sent as `{ "type": "command", "cmd": { "op": "<op>", ... } }`.
Every command is validated server-side; invalid arguments return `Response::Error`.

### Protection control

#### `start`
Start DPI bypass with the current strategy + DNS. Brings up WARP if any app is in
`"warp"` mode or `full_warp` is set.

→ `Response::Status(EngineStatus)`

#### `stop`
Stop all protection (DPI engine + WARP tunnel).

→ `Response::Status(EngineStatus)`

#### `status`
Read current engine state without mutating anything.

→ `Response::Status(EngineStatus)`

---

### Strategy / engine

#### `set_strategy`
Set the active DPI strategy id. Restarts the engine if running.

| Field | Type | Values |
|---|---|---|
| `id` | `string` | `"auto"` or any id from `EngineCatalog` strategies: `c1`, `multidisorder`, `fake`, `tt`, `tt-alt`, `superonline`, `superonline-alt`, `kablonet`, `turkcell-hotspot`, `vodafone-hotspot` |

→ `Response::Status(EngineStatus)`

#### `set_engine`
Switch the active DPI engine. Restarts with the same strategy if currently running.

| Field | Type | Values |
|---|---|---|
| `id` | `string` | `"zapret"` \| `"byedpi"` \| `"byedpi-proxifyre"` \| `"byedpi-drover"` \| `"goodbyedpi"` |

→ `Response::Status(EngineStatus)`

#### `engine_catalog`
List all known engines with their availability and capabilities.

→ `Response::Data(JSON: EngineInfo[])`

```json
[
  {
    "id": "zapret",
    "name": "Zapret (winws)",
    "kind": "desync",
    "caps": { "hostlist": true, "split_tunnel": false, "requires_windivert": true, "kernel_less": false },
    "available": true
  }
]
```

`kind` values: `"desync"` | `"local_proxy"` | `"tunnel"`

---

### DNS

#### `set_dns`
Set the DoH/DNS provider. Applied to the system (requires admin).

| Field | Type | Values |
|---|---|---|
| `profile` | `string` | `"cloudflare"` \| `"quad9"` \| `"adguard"` \| `"google"` \| `"auto"` |

→ `Response::Status(EngineStatus)`

#### `reset_dns`
Reset DNS to DHCP and clear DoH (undo `set_dns`).

→ `Response::Ok`

#### `verify_dns`
Read the system's active DNS servers and classify them (no mutation).

→ `Response::Data(JSON: DnsVerify)`

```json
{ "servers": ["1.1.1.1", "1.0.0.1"], "secure": true, "provider": "cloudflare" }
```

---

### Profiles

#### `list_profiles`
Return all saved profiles.

→ `Response::Data(JSON: Profile[])`

```json
[
  {
    "schema_version": 1,
    "id": "my-profile",
    "name": "My Profile",
    "engine": "zapret",
    "isp": "",
    "scope": { "mode": "system", "apps": [], "browsers": false, "folders": [] },
    "dns": { "enabled": false, "provider": "" },
    "strategy": "c1",
    "hostlist": ["discord.com", "roblox.com"],
    "engine_params": null
  }
]
```

`scope.mode`: `"system"` | `"split"`

#### `save_profile`
Validate and persist a profile (create or replace by `id`).

| Field | Type | Notes |
|---|---|---|
| `json` | `string` | Serialized `Profile` object, max 64 KiB |

→ `Response::Ok`

#### `delete_profile`
Delete a profile by id.

| Field | Type | Notes |
|---|---|---|
| `id` | `string` | Slug: alphanumeric + `-` + `_`, max 64 chars |

→ `Response::Ok`

#### `export_profile`
Export a profile as a shareable JSON string.

| Field | Type |
|---|---|
| `id` | `string` |

→ `Response::Data(JSON: Profile)`

#### `apply_profile`
Load a saved profile and apply it end-to-end (engine + strategy + hostlist + DNS + WARP).
Runs the state machine: `Idle → Applying → Active | Error`.

| Field | Type |
|---|---|
| `id` | `string` |

→ `Response::Status(EngineStatus)`

---

### Per-app / routing

#### `set_app_modes`
Set protection mode for each tracked app. Replaces the entire app-mode map.

| Field | Type | Notes |
|---|---|---|
| `modes` | `[string, string, string][]` | Triples of `(app_id, mode, exe_path)`. `mode`: `"off"` \| `"dpi"` \| `"warp"`. Path can be empty when `mode == "off"`. Max 500 entries. |

→ `Response::Ok` (WARP sync happens immediately if running)

#### `set_full_warp`
Enable or disable full-tunnel WARP (all-system Tam Koruma mode). Takes effect immediately if running.

| Field | Type |
|---|---|
| `enable` | `bool` |

→ `Response::Ok`

#### `set_hostlist`
Replace the active domain hostlist (used in split-scope/hostlist mode).

| Field | Type | Constraints |
|---|---|---|
| `domains` | `string[]` | Max 500 domains, ASCII alphanumeric + `.` + `-`, max 253 chars each |

→ `Response::Ok`

#### `block_app`
Add or remove a firewall block for an app (requires admin).

| Field | Type | Notes |
|---|---|---|
| `id` | `string` | App slug (alphanumeric + `-` + `_`) |
| `path` | `string` | Absolute `.exe` path; must match `<id>.exe`. Empty when `block=false`. |
| `block` | `bool` | `true` = add block rule; `false` = remove |

→ `Response::Ok`

#### `set_limit`
Set per-app bandwidth limit.

| Field | Type | Notes |
|---|---|---|
| `id` | `string` | App slug |
| `path` | `string` | Absolute `.exe` path |
| `down` | `u32` | Download cap in kbps (0 = remove); max 1 000 000 |
| `up` | `u32` | Upload cap in kbps (0 = remove); max 1 000 000 |

→ `Response::Ok`

---

### System tweaks

#### `set_tweak`
Apply a network stack tuning parameter (requires admin; sim read-only otherwise).

| Field | Type | Values |
|---|---|---|
| `key` | `string` | `nagle` \| `heuristics` \| `throttleIdx` \| `nicPower` \| `highPerf` \| `rss` \| `rsc` \| `offload` → `"on"/"off"` ; `autotuning` → `"normal"/"disabled"` ; `congestion` → `"cubic"/"ctcp"/"bbr2"` ; `mtu` → `"1280"`–`"1500"` |
| `value` | `string` | See key constraints above |

→ `Response::Ok`

---

### Diagnostics / preflight

#### `preflight`
Run all preflight checks: admin elevation, winws binary, WinDivert conflict, WARP API
reachability, AV interference, VC++ Redist, packet filter, DoH probe.

→ `Response::Data(JSON: PreflightResult)`

```json
{
  "ok": true,
  "checks": [
    { "name": "admin", "pass": true, "hint": "Running as administrator" },
    { "name": "winws", "pass": true, "hint": "winws.exe present" }
  ]
}
```

#### `diagnose`
Test specific target domains: DNS resolution + TCP connect + TLS ClientHello probe.

| Field | Type | Constraints |
|---|---|---|
| `targets` | `string[]` | 1–50 domain names |

→ `Response::Data(JSON: TargetDiag[])`

```json
[
  { "target": "discord.com", "dns_ok": true, "tcp_ok": true, "tls_ok": false, "ms": 42, "suggestion": "SNI blocked — try a DPI strategy" }
]
```

#### `repair`
Run a network repair tool (does not require restart).

| Field | Type | Values |
|---|---|---|
| `tool` | `string` | `"flushdns"` \| `"registerdns"` \| `"dnscache"` \| `"renew"` \| `"winsock"` \| `"ipreset"` \| `"adapter"` |

→ `Response::Ok`

---

### Auto-Pilot (blocking)

#### `auto_pilot`
Run blockcheck against all engine×strategy candidates and return a ranked score table.
Prefer `auto_pilot_stream` (Request-level) for live UI updates.

| Field | Type | Values |
|---|---|---|
| `targets` | `string[]` | 1–50 domain names |
| `depth` | `string` | `"quick"` \| `"standard"` \| `"force"` |

→ `Response::Data(JSON: ScoreRow[])`

```json
[
  {
    "engine": "zapret",
    "strategy": "c1",
    "per_target": [["discord.com", true, 38]],
    "score": 62
  }
]
```

`per_target` tuples: `[domain, opened, latency_ms]`. `score = opened_count×100 − avg_latency_penalty`.

---

### Health signals

#### `tunnel_status`
Read WARP tunnel state snapshot (item 11.1). Does not block.

→ `Response::Data(JSON: TunnelState)`

```json
{
  "warp_running": false,
  "warp_full": false,
  "tunnel_installed": false,
  "handshake_ago_secs": null
}
```

`handshake_ago_secs`: seconds since the WireGuard peer last completed a handshake, or
`null` when unavailable (tunnel not running, no peer handshake yet, or pipe inaccessible).

#### `health`
Derived health signal for BlackHole gating (item 11.2). Synthesises engine state +
cached telemetry into a single `healthy` boolean.

`healthy = running && metrics_age < 5s && loss < 100% && ping > 0`

→ `Response::Data(JSON: HealthSignal)`

```json
{
  "healthy": true,
  "loading": false,
  "error": false,
  "ping_ms": 32,
  "loss_pct": 0.0
}
```

`loading`: true while in `Applying` state (state machine transitioning).
`error`: true when the engine hit a fatal error and stopped.

---

### Rollback

#### `rollback_all`
Reverse all system mutations recorded in the transaction log (DNS, firewall rules, tunnel
install, proxifyre service, scheduled task, drover DLL copies). Stops protection first.

→ `Response::Ok`

---

## Service → Client (`Response`)

All responses have a `"type"` discriminant (snake_case).

```json
{ "type": "ok" }
{ "type": "status", "running": true, "strategy": "c1", "dns": "cloudflare", "engine": "zapret", "state": "active" }
{ "type": "telemetry", "running": true, "ping": 32, "jitter": 4, "loss": 0.0, "down": 12.4, "up": 1.1 }
{ "type": "data", ... }
{ "type": "error", "message": "..." }
```

### `EngineStatus` (inside `status`)

| Field | Type | Notes |
|---|---|---|
| `running` | `bool` | DPI engine is actively protecting |
| `strategy` | `string` | Active strategy id |
| `dns` | `string` | Active DNS provider id |
| `engine` | `string` | Active engine id |
| `state` | `string` | State machine: `"idle"` \| `"applying"` \| `"active"` \| `"paused"` \| `"error"` |

### `Metrics` (inside `telemetry`, ~1 Hz on Subscribe)

| Field | Type | Notes |
|---|---|---|
| `running` | `bool` | Engine running when this sample was taken |
| `ping` | `u32` | RTT to 1.1.1.1:443 in ms |
| `jitter` | `u32` | \|current − previous ping\| in ms |
| `loss` | `f64` | Packet loss % (20-sample rolling window) |
| `down` | `f64` | Download throughput in Mbps |
| `up` | `f64` | Upload throughput in Mbps |

### `data`
Carries a JSON-encoded string payload. The actual type depends on the command:

| Command | Payload type |
|---|---|
| `engine_catalog` | `EngineInfo[]` |
| `list_profiles` | `Profile[]` |
| `export_profile` | `Profile` |
| `verify_dns` | `DnsVerify` |
| `preflight` | `PreflightResult` |
| `diagnose` | `TargetDiag[]` |
| `auto_pilot` | `ScoreRow[]` |
| `auto_pilot_stream` | one `ScoreRow` per write, then terminal `ok` |
| `tunnel_status` | `TunnelState` |
| `health` | `HealthSignal` |

---

## BlackHole binding

`BlackHole.svelte` gates its animation on a single `healthy: bool` polled from
`Command::Health`. Recommended polling pattern:

1. Open a new IPC connection, send `Hello`.
2. Send `Command::Health` once per second (or on-demand).
3. Parse `Response::Data` → `HealthSignal.healthy`.
4. Set `animating = healthy` on the BlackHole component.

Do **not** use the `Subscribe` stream for BlackHole gating — that connection is exclusively
for the live telemetry panel. Keep them on separate connections.

---

## Settings binding

The Settings panel reads/writes via these commands:

| Setting | Read | Write |
|---|---|---|
| DNS provider | `Status.dns` | `set_dns` |
| Strategy | `Status.strategy` | `set_strategy` |
| Engine | `Status.engine` | `set_engine` |
| DNS verify | `verify_dns` | — |
| Per-app modes | (store in profile) | `set_app_modes` |
| Full WARP | (store in profile) | `set_full_warp` |
| Hostlist | (store in profile) | `set_hostlist` |
| Network tweaks | — | `set_tweak` |
| Bandwidth limits | — | `set_limit` |
| Profiles | `list_profiles` | `save_profile` / `delete_profile` / `apply_profile` |

---

## Tauri commands (UI process)

These are available directly in the SvelteKit frontend via `invoke()` without going
through the named pipe (Tauri IPC, not the service pipe):

| Tauri command | Module | Notes |
|---|---|---|
| `install_discord_ptb` | `repair` | Download + install Discord PTB (repair flow) |
| `attach_app_to_proxy` | `repair` / `proxifyre` | Add an app to ProxiFyre SOCKS5 config |
| `verify_manifest` | `manifest` | Verify SHA-256 of all bundled binaries |
| `create_log_bundle` | `logbundle` | Zip logs for support |

---

*This contract is auto-checked by `ipc::tests::contract_doc_covers_all_commands`.*
