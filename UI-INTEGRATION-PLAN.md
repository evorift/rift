# evorift — UI Integration Plan

Build on top of the existing Svelte UI. No redesign. Every item below maps a backend
capability to a concrete UI change with the exact file, line, and invoke call.

**Backend status:** 100% complete (78 tests, all 11 phases)
**Frontend status:** Old UI — running on pre-rewrite invoke names + missing new features

---

## PART 1 — Broken invoke calls (fix these first)

These calls exist in the UI right now but are broken or renamed after the backend rewrite.

### 1.1 `dns_status` → `verify_dns`

**File:** `src/lib/components/sections/Connection.svelte`, line 68

```ts
// OLD (broken)
dnsRes = await invoke<DnsRes>("dns_status");

// NEW
const raw = await invoke<string>("verify_dns");
dnsRes = JSON.parse(raw) as DnsRes;
// Shape: { servers: string[], secure: boolean, provider: string }
```

Also change `resetDns()` (line 74–77) to call `reset_dns` (no args) instead of
`set_dns({ profile: "auto" })`:
```ts
function resetDns() {
  app.dns = "cloudflare";
  invoke("reset_dns").catch(() => {});
  toasts.info(t("conn.dnsAutoToast"));
}
```

---

### 1.2 `run_repair` → `repair`

**File:** `src/lib/state.svelte.ts`, line 551

```ts
// OLD (broken)
for (const tool of ["flushdns", "registerdns", "dnscache"]) this.#send("run_repair", { tool });

// NEW
for (const tool of ["flushdns", "registerdns", "dnscache"]) this.#send("repair", { tool });
```

Also check `Advanced.svelte` for any direct `invoke("run_repair", ...)` calls — rename them all to `"repair"`.

Valid `tool` values: `"flushdns"` | `"registerdns"` | `"dnscache"` | `"renew"` | `"winsock"` | `"ipreset"` | `"adapter"`

---

### 1.3 Verify telemetry still works

**File:** `src/lib/state.svelte.ts`, line 169

```ts
await listen<Metrics>("telemetry", (e) => this.#applyTelemetry(e.payload));
```

The backend emits a Tauri event `"telemetry"` from lib.rs. Confirm this event is still
being forwarded after the backend rewrite (check `src-tauri/src/lib.rs` for the emit call).
If the event name changed, update the listener here.

---

### 1.4 Extend strategy dropdown with ISP presets

**File:** `src/lib/components/sections/Connection.svelte`, line 12–17

Add ISP-specific presets to the strategy list. The new backend accepts all of these:

```ts
const strategies = [
  { value: "auto",              labelKey: "conn.stratAuto",         descKey: "conn.stratAutoDesc" },
  { value: "c1",                labelKey: "conn.stratFrag",         descKey: "conn.stratFragDesc" },
  { value: "multidisorder",     labelKey: "conn.stratMulti",        descKey: "conn.stratMultiDesc" },
  { value: "fake",              labelKey: "conn.stratFake",         descKey: "conn.stratFakeDesc" },
  // ISP presets (new — group these under an "ISP Presets" separator)
  { value: "tt",                labelKey: "conn.stratTT",           descKey: "conn.stratTTDesc" },
  { value: "tt-alt",            labelKey: "conn.stratTTAlt",        descKey: "conn.stratTTAltDesc" },
  { value: "superonline",       labelKey: "conn.stratSuperonline",  descKey: "conn.stratSuperonlineDesc" },
  { value: "superonline-alt",   labelKey: "conn.stratSuperonlineAlt", descKey: "conn.stratSuperonlineAltDesc" },
  { value: "kablonet",          labelKey: "conn.stratKablonet",     descKey: "conn.stratKablonetDesc" },
  { value: "turkcell-hotspot",  labelKey: "conn.stratTurkcell",     descKey: "conn.stratTurkcellDesc" },
  { value: "vodafone-hotspot",  labelKey: "conn.stratVodafone",     descKey: "conn.stratVodafoneDesc" },
];
```

Add i18n keys for each new entry in `src/lib/i18n.svelte.ts`.

---

## PART 2 — Health signal (BlackHole gating)

The backend now has `Command::Health` → `HealthSignal { healthy, loading, error, ping_ms, loss_pct }`.

### 2.1 Add health polling to state.svelte.ts

Add to `AppState`:
```ts
healthy   = $state(false);
engineState = $state<"idle"|"applying"|"active"|"paused"|"error">("idle");

async #pollHealth() {
  try {
    const raw = await invoke<string>("health");
    const h = JSON.parse(raw) as { healthy: boolean; loading: boolean; error: boolean; ping_ms: number; loss_pct: number };
    this.healthy = h.healthy;
    if (h.loading) this.engineState = "applying";
    else if (h.error) this.engineState = "error";
    else if (this.status === "on") this.engineState = "active";
    else this.engineState = "idle";
  } catch (_) {}
}
```

Start polling in `init()`:
```ts
setInterval(() => {
  if (this.status !== "off") this.#pollHealth();
}, 2000);
```

### 2.2 Wire to BlackHole + dashboard

**Do NOT touch BlackHole.svelte.** Pass `app.healthy` and `app.activating` as props to it
from `+page.svelte` — check how BlackHole is currently instantiated and pass health through.

In the dashboard, show `engineState` next to the status badge:
- `"applying"` → "Connecting..." (already have this via `activating`)
- `"error"` → show a red "Engine error" toast / banner
- `"active"` → normal "on" state

---

## PART 3 — Engine selector

The backend supports 5 engines. Currently only zapret (winws) is used.

### 3.1 Add `engineId` to state.svelte.ts

```ts
engineId = $state<string>("zapret");

async setEngine(id: string) {
  this.engineId = id;
  await invoke("set_engine", { id }).catch(() => {});
  eventLog.info(`Engine: ${id}`);
}
```

Add `engineId` to `#loadPrefs()` and `#startPersist()`.

### 3.2 Add engine picker to Connection.svelte

Load available engines on mount:
```ts
type EngineInfo = { id: string; name: string; kind: string; available: boolean };
let engines = $state<EngineInfo[]>([]);

onMount(async () => {
  try {
    const raw = await invoke<string>("engine_catalog");
    engines = JSON.parse(raw);
  } catch (_) {}
});
```

Add a segmented control or dropdown above the strategy picker:
```svelte
<section class="card span-2">
  <h3>Engine</h3>
  {#each engines as e}
    <button
      class:active={app.engineId === e.id}
      disabled={!e.available}
      onclick={() => app.setEngine(e.id)}
    >
      {e.name}
    </button>
  {/each}
</section>
```

Engine IDs and their meaning:
| id | Name | Kind |
|---|---|---|
| `zapret` | Zapret (winws) | DPI desync — default |
| `byedpi` | ByeDPI | Local SOCKS5 proxy |
| `byedpi-proxifyre` | ByeDPI + ProxiFyre | Per-app SOCKS5 routing |
| `byedpi-drover` | ByeDPI + Drover | Discord DLL injection |
| `goodbyedpi` | GoodbyeDPI | DPI desync (alt) |

---

## PART 4 — Auto-Pilot

The backend has `Command::AutoPilot { targets, depth }` → ranked ScoreRow[].

### 4.1 Add "Find best" button to Connection.svelte

Add next to the strategy dropdown:
```ts
type ScoreRow = { engine: string; strategy: string; score: number; per_target: [string, boolean, number][] };
let apRunning = $state(false);
let apRows = $state<ScoreRow[]>([]);
let showAp = $state(false);

async function runAutoPilot() {
  apRunning = true;
  apRows = [];
  showAp = true;
  try {
    const raw = await invoke<string>("auto_pilot", {
      targets: app.sites.slice(0, 5),
      depth: "quick",
    });
    apRows = (JSON.parse(raw) as ScoreRow[]).sort((a, b) => b.score - a.score);
  } catch (_) {}
  apRunning = false;
}

function applyBest() {
  if (!apRows.length) return;
  const best = apRows[0];
  app.setEngine(best.engine);
  invoke("set_strategy", { id: best.strategy }).catch(() => {});
  app.strategy = best.strategy;
  showAp = false;
  toasts.success(`Best: ${best.engine} / ${best.strategy} (score ${best.score})`);
}
```

Show results in a simple table (engine | strategy | score | opened/total). Add "Apply best"
button. Use the existing `Modal.svelte` for the panel.

---

## PART 5 — Profiles

The backend has full profile CRUD. Add a "Profiles" card to Connection.svelte.

### 5.1 Profile types
```ts
type Profile = {
  schema_version: number;
  id: string;
  name: string;
  engine: string;
  strategy: string;
  hostlist: string[];
  scope: { mode: "system"|"split"; apps: string[]; browsers: boolean; folders: string[] };
  dns: { enabled: boolean; provider: string };
  engine_params: unknown;
};
```

### 5.2 Profile state + methods (add to state.svelte.ts)
```ts
profiles = $state<Profile[]>([]);

async loadProfiles() {
  try {
    const raw = await invoke<string>("list_profiles");
    this.profiles = JSON.parse(raw);
  } catch (_) {}
}

async applyProfile(id: string) {
  try {
    await invoke("apply_profile", { id });
    toasts.success(`Profile applied: ${id}`);
  } catch (e) { toasts.error(String(e)); }
}

async saveCurrentAsProfile(name: string) {
  const id = name.toLowerCase().replace(/\s+/g, "-").replace(/[^a-z0-9-]/g, "");
  const prof: Profile = {
    schema_version: 1,
    id, name,
    engine: this.engineId,
    strategy: this.strategy,
    hostlist: this.sites,
    scope: { mode: "system", apps: [], browsers: false, folders: [] },
    dns: { enabled: !!this.dns, provider: this.dns },
    engine_params: null,
  };
  await invoke("save_profile", { json: JSON.stringify(prof) });
  await this.loadProfiles();
}

async deleteProfile(id: string) {
  await invoke("delete_profile", { id });
  this.profiles = this.profiles.filter(p => p.id !== id);
}
```

### 5.3 Add Profiles card to Connection.svelte
```svelte
<section class="card span-2">
  <div class="card-head">
    <h3>Profiles</h3>
    <button class="btn" onclick={saveNewProfile}>Save current</button>
  </div>
  {#each app.profiles as p}
    <div class="profile-row">
      <span>{p.name}</span>
      <span class="mono dim">{p.engine} / {p.strategy}</span>
      <button onclick={() => app.applyProfile(p.id)}>Apply</button>
      <button onclick={() => app.deleteProfile(p.id)}>✕</button>
    </div>
  {/each}
</section>
```

Call `app.loadProfiles()` in `init()`.

---

## PART 6 — Diagnostics

### 6.1 Preflight check panel (Advanced.svelte)

Add a "System check" section:
```ts
type Check = { name: string; pass: boolean; hint: string };
type PreflightResult = { ok: boolean; checks: Check[] };
let preflightResult = $state<PreflightResult|null>(null);
let preflightRunning = $state(false);

async function runPreflight() {
  preflightRunning = true;
  try {
    const raw = await invoke<string>("preflight");
    preflightResult = JSON.parse(raw);
  } catch (_) {}
  preflightRunning = false;
}
```

```svelte
<section class="card">
  <div class="card-head">
    <h3>System Check</h3>
    <button class="btn" onclick={runPreflight}>
      {preflightRunning ? "Checking..." : "Run checks"}
    </button>
  </div>
  {#if preflightResult}
    <div class:ok={preflightResult.ok} class:fail={!preflightResult.ok}>
      {preflightResult.ok ? "All checks passed" : "Issues found"}
    </div>
    {#each preflightResult.checks as c}
      <div class="check-row">
        <span class="icon">{c.pass ? "✅" : "❌"}</span>
        <span>{c.name}</span>
        {#if !c.pass}<span class="hint">{c.hint}</span>{/if}
      </div>
    {/each}
  {/if}
</section>
```

### 6.2 Site diagnostics (Connection.svelte)

Add a "Test" button next to the site list:
```ts
type TargetDiag = { target: string; dns_ok: boolean; tcp_ok: boolean; tls_ok: boolean; ms: number; suggestion: string };
let diagResults = $state<TargetDiag[]>([]);
let diagRunning = $state(false);

async function diagSites() {
  diagRunning = true;
  try {
    const raw = await invoke<string>("diagnose", { targets: app.sites.slice(0, 10) });
    diagResults = JSON.parse(raw);
  } catch (_) {}
  diagRunning = false;
}
```

Show results as a small table per domain: DNS | TCP | TLS | ping | suggestion.

---

## PART 7 — Tunnel status

### 7.1 Full Protection enhancement

When `app.fullProtection` is true, show tunnel health from `tunnel_status`:
```ts
type TunnelState = { warp_running: boolean; warp_full: boolean; tunnel_installed: boolean; handshake_ago_secs: number|null };
tunnelState = $state<TunnelState|null>(null);

async #pollTunnel() {
  try {
    const raw = await invoke<string>("tunnel_status");
    this.tunnelState = JSON.parse(raw);
  } catch (_) {}
}
```

In `applyFullProtection()` start polling tunnel every 30s. In `exitFullProtection()` stop polling.

Show in the dashboard when Tam Koruma is active:
- "Tunnel: active" / "Tunnel: installing..." / "Last handshake: Xs ago"
- If `handshake_ago_secs > 180`: show amber warning "Weak tunnel signal"

---

## PART 8 — Misc new features

### 8.1 Log bundle export (Logs.svelte)

Add an "Export logs" button:
```ts
async function exportLogs() {
  try {
    const path = await invoke<string>("create_log_bundle");
    toasts.success(`Log bundle saved: ${path}`);
  } catch (e) { toasts.error(String(e)); }
}
```

### 8.2 Manifest integrity check (Advanced.svelte)

Add to the repair section:
```ts
type VerifyResult = { name: string; path: string; expected_sha256: string; actual_sha256: string|null; ok: boolean };
let manifestResults = $state<VerifyResult[]>([]);

async function verifyIntegrity() {
  try {
    const raw = await invoke<string>("verify_manifest");
    manifestResults = JSON.parse(raw);
    const failed = manifestResults.filter(r => !r.ok);
    if (failed.length === 0) toasts.success("All files verified ✓");
    else toasts.error(`${failed.length} file(s) failed integrity check`);
  } catch (e) { toasts.error(String(e)); }
}
```

### 8.3 Install Discord PTB (Advanced.svelte)

Add to repair section:
```ts
async function installDiscordPTB() {
  try {
    await invoke("install_discord_ptb");
    toasts.success("Discord PTB installation started");
  } catch (e) { toasts.error(String(e)); }
}
```

---

## Complete invoke() reference

### Already working — DO NOT change
| Call | Backend |
|---|---|
| `invoke("start_protection")` | Command::Start |
| `invoke("stop_protection")` | Command::Stop |
| `invoke("set_strategy", { id })` | Command::SetStrategy |
| `invoke("set_dns", { profile })` | Command::SetDns |
| `invoke("set_hostlist", { domains })` | Command::SetHostlist |
| `invoke("set_app_modes", { modes })` | Command::SetAppModes |
| `invoke("set_full_warp", { enable })` | Command::SetFullWarp |
| `invoke("set_tweak", { key, value })` | Command::SetTweak |
| `invoke("set_limit", { id, path, down, up })` | Command::SetLimit |
| `invoke("is_admin")` | Tauri direct |
| `invoke("list_apps")` | Tauri direct |
| `invoke("detect_app_domains", { exe })` | Tauri direct |
| `invoke("get_app_icons", { paths })` | Tauri direct |
| `invoke("set_autostart", { enable, minimized })` | Tauri direct |
| `invoke("relaunch_as_admin")` | Tauri direct |
| `invoke("set_tray_labels", ...)` | Tauri direct |
| `invoke("set_tray_tooltip", { text })` | Tauri direct |

### Renamed — MUST fix
| Old call | New call |
|---|---|
| `invoke("dns_status")` | `invoke("verify_dns")` → parse JSON → DnsVerify |
| `invoke("run_repair", { tool })` | `invoke("repair", { tool })` |

### New — add these
| Call | Returns | Where to use |
|---|---|---|
| `invoke("verify_dns")` | `string` (JSON: DnsVerify) | Connection.svelte — DNS leak check |
| `invoke("reset_dns")` | nothing | Connection.svelte — "Auto / ISP" button |
| `invoke("status")` | `string` (JSON: EngineStatus) | state.svelte.ts — init + poll |
| `invoke("health")` | `string` (JSON: HealthSignal) | state.svelte.ts — 2s poll |
| `invoke("tunnel_status")` | `string` (JSON: TunnelState) | state.svelte.ts — Tam Koruma |
| `invoke("set_engine", { id })` | nothing | Connection.svelte — engine picker |
| `invoke("engine_catalog")` | `string` (JSON: EngineInfo[]) | Connection.svelte — on mount |
| `invoke("auto_pilot", { targets, depth })` | `string` (JSON: ScoreRow[]) | Connection.svelte — autopilot |
| `invoke("list_profiles")` | `string` (JSON: Profile[]) | state.svelte.ts — init |
| `invoke("save_profile", { json })` | nothing | state.svelte.ts |
| `invoke("apply_profile", { id })` | `string` (JSON: EngineStatus) | state.svelte.ts |
| `invoke("delete_profile", { id })` | nothing | state.svelte.ts |
| `invoke("export_profile", { id })` | `string` (JSON: Profile) | Connection.svelte |
| `invoke("rollback_all")` | nothing | Advanced.svelte — emergency reset |
| `invoke("preflight")` | `string` (JSON: PreflightResult) | Advanced.svelte |
| `invoke("diagnose", { targets })` | `string` (JSON: TargetDiag[]) | Connection.svelte |
| `invoke("verify_manifest")` | `string` (JSON: VerifyResult[]) | Advanced.svelte |
| `invoke("install_discord_ptb")` | nothing | Advanced.svelte |
| `invoke("create_log_bundle")` | `string` (file path) | Logs.svelte |

> **Note:** All new IPC-backed commands return `string` because the backend sends
> `Response::Data(json_string)` and lib.rs unwraps it. Parse with `JSON.parse()`.

---

## Implementation checklist

### Phase A — Fix existing (do before committing)
- [x] **A1** `Connection.svelte:68` — rename `dns_status` → `verify_dns`, parse JSON ✅ DONE
- [x] **A2** `Connection.svelte:74` — rename `resetDns` to call `reset_dns` (no profile arg) ✅ DONE (combined with A1)
- [x] **A3** `state.svelte.ts:551` — N/A: `run_repair` is still the Tauri command name in lib.rs
- [x] **A4** `Advanced.svelte` — N/A: `run_repair` still registered, no rename needed
- [x] **A5** `Connection.svelte:12` — add ISP preset strategies (tt, superonline, turkcell, vodafone, kablonet) + i18n keys (all 4 languages) ✅ DONE

### Phase B — Health wiring
- [x] **B1** `state.svelte.ts` — add `healthy`, `engineState` fields + `#pollHealth()` + setInterval in `init()` ✅ DONE
- [x] **B2** `+page.svelte` — BlackHole imports `app` directly, so `app.healthy` is already accessible; no prop wiring needed ✅ DONE
- [x] **B3** Dashboard — added `$effect` in +page.svelte that toasts an error when `engineState === "error"`; also fixed ISP preset labels in STRAT_KEY ✅ DONE

### Phase C — Engine selector
- [x] **C1** `state.svelte.ts` — add `engineId`, `setEngine()`, persist in localStorage ✅ DONE
- [x] **C2** `Connection.svelte` — add engine picker card, load `engine_catalog` on mount, `set_engine` on change ✅ DONE

### Phase D — Auto-Pilot
- [x] **D1** `Connection.svelte` — add "Find best strategy" button + results panel + "Apply best" action ✅ DONE

### Phase E — Profiles
- [x] **E1** `state.svelte.ts` — add `profiles`, `loadProfiles()`, `applyProfile()`, `saveCurrentAsProfile()`, `deleteProfile()` ✅ DONE
- [x] **E2** `state.svelte.ts:init()` — call `loadProfiles()` ✅ DONE
- [x] **E3** `Connection.svelte` — add Profiles card (list + apply + save current + delete) ✅ DONE

### Phase F — Diagnostics
- [x] **F1** `Advanced.svelte` — add System Check card with `preflight` call + result display ✅ DONE
- [x] **F2** `Connection.svelte` — add "Test sites" button with `diagnose` call + per-domain result table ✅ DONE

### Phase G — Tunnel status
- [x] **G1** `state.svelte.ts` — add `tunnelState`, `#pollTunnel()`, start/stop in `applyFullProtection`/`exitFullProtection` ✅ DONE
- [x] **G2** Dashboard — show tunnel health badge when Tam Koruma is active; warn if `handshake_ago_secs > 180` ✅ DONE

### Phase H — Misc
- [x] **H1** `Logs.svelte` — add "Export log bundle" button → `invoke("create_log_bundle")` ✅ DONE
- [x] **H2** `Advanced.svelte` — add "Verify integrity" button → `invoke("verify_manifest")` ✅ DONE
- [x] **H3** `Advanced.svelte` — add "Install Discord PTB" button → `invoke("install_discord_ptb")` ✅ DONE
- [x] **H4** `Advanced.svelte` — add "Rollback all changes" emergency button → `invoke("rollback_all")` ✅ DONE

---

## Data shapes (copy-paste ready TypeScript)

```ts
// From Command::Status
type EngineStatus = {
  running: boolean;
  strategy: string;
  dns: string;
  engine: string;       // "zapret" | "byedpi" | "goodbyedpi" | ...
  state: string;        // "idle" | "applying" | "active" | "paused" | "error"
};

// From Command::Health
type HealthSignal = {
  healthy: boolean;
  loading: boolean;
  error: boolean;
  ping_ms: number;
  loss_pct: number;
};

// From Command::TunnelStatus
type TunnelState = {
  warp_running: boolean;
  warp_full: boolean;
  tunnel_installed: boolean;
  handshake_ago_secs: number | null;
};

// From Command::VerifyDns
type DnsVerify = {
  servers: string[];
  secure: boolean;
  provider: string;
};

// From Command::EngineCatalog
type EngineCaps = { hostlist: boolean; split_tunnel: boolean; requires_windivert: boolean; kernel_less: boolean };
type EngineInfo = {
  id: string;
  name: string;
  kind: "desync" | "local_proxy" | "tunnel";
  caps: EngineCaps;
  available: boolean;
};

// From Command::AutoPilot
type ScoreRow = {
  engine: string;
  strategy: string;
  per_target: [string, boolean, number][]; // [domain, opened, latency_ms]
  score: number;
};

// From Command::Preflight
type Check = { name: string; pass: boolean; hint: string };
type PreflightResult = { ok: boolean; checks: Check[] };

// From Command::Diagnose
type TargetDiag = {
  target: string;
  dns_ok: boolean;
  tcp_ok: boolean;
  tls_ok: boolean;
  ms: number;
  suggestion: string;
};

// From Command::ListProfiles
type Profile = {
  schema_version: number;
  id: string;
  name: string;
  engine: string;
  isp: string;
  scope: { mode: "system" | "split"; apps: string[]; browsers: boolean; folders: string[] };
  dns: { enabled: boolean; provider: string };
  strategy: string;
  hostlist: string[];
  engine_params: unknown;
};

// From verify_manifest Tauri command
type VerifyResult = {
  name: string;
  path: string;
  expected_sha256: string;
  actual_sha256: string | null;
  ok: boolean;
};
```
