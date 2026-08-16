//! Orkestrasyon katmanı (docs/07 §1, §7) — named-pipe sunucusu + komut motoru + durum makinesi.
//!
//! Tüm ayrıcalıklı iş burada (LocalSystem `evorift-svc`); UI yalnız IPC ile niyet bildirir. Motorlar
//! [`crate::engine::BypassEngine`] arkasında; sistem-mutasyonları sys/dns/firewall/tweak/limit/repair
//! modüllerinde; her komut [`ipc::validate`] ile doğrulanır + audit log'a yazılır + (gerekirse)
//! transaction log'a kaydedilir (rollback). Durum makinesi (docs/07 §7): Idle→Applying→Active→Paused/Error.

use crate::engine::{self, BypassEngine};
use crate::ipc::{self, Command, EngineStatus, Metrics, Request, Response, PIPE_NAME};
use std::time::Instant;
use crate::rollback::Change;
use crate::sys::audit;
use crate::warp::WarpEngine;
use interprocess::local_socket::{prelude::*, GenericNamespaced, ListenerOptions, Stream};
use std::io::{self, BufReader};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Global durum makinesi (docs/07 §7). Aynı anda tek aktif profil; çakışma önlenir.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunState {
    Idle,
    Applying,
    Active,
    Paused,
    Error,
}

impl RunState {
    fn as_str(&self) -> &'static str {
        match self {
            RunState::Idle => "idle",
            RunState::Applying => "applying",
            RunState::Active => "active",
            RunState::Paused => "paused",
            RunState::Error => "error",
        }
    }
}

/// Proof-of-protection (item: honesty gate). Orthogonal to `RunState` — `RunState::Active` only
/// means the DPI process is alive; `VerifyState` answers whether a real TLS handshake to live
/// Discord endpoints actually got through. The UI must render "Protected" ONLY on `Verified`;
/// `Unverified`/`Verifying` render as "applied — unverified", never as protected (evorift-remote-testing:
/// "silent success is the enemy").
#[derive(Clone, Debug, PartialEq)]
enum VerifyState {
    Unverified,
    Verifying,
    Verified,
    Broken(String),
}

impl VerifyState {
    fn as_str(&self) -> &'static str {
        match self {
            VerifyState::Unverified => "unverified",
            VerifyState::Verifying => "verifying",
            VerifyState::Verified => "verified",
            VerifyState::Broken(_) => "broken",
        }
    }
    fn reason(&self) -> String {
        match self {
            VerifyState::Broken(r) => r.clone(),
            _ => String::new(),
        }
    }
}

struct Engine {
    /// running = motor (winws/byedpi) gerçekten açık mı. state = kullanıcıya gösterilen durum makinesi.
    running: bool,
    state: RunState,
    strategy: String,
    dns: String,
    /// Aktif DPI motoru id'si ("zapret"|"byedpi"|"goodbyedpi"). SetEngine/ApplyProfile değiştirir.
    engine_id: String,
    hostlist: Vec<String>,
    limits: std::collections::HashMap<String, (String, u32)>,
    app_modes: std::collections::HashMap<String, (String, String)>,
    /// Aktif DPI motoru (BypassEngine soyutlaması). make_engine(engine_id) ile üretilir.
    dpi: Box<dyn BypassEngine>,
    /// Motor B — WARP tüneli (Discord split-tunnel · Tam Koruma full-tunnel).
    warp: WarpEngine,
    full_warp: bool,
    /// Hostlist mode (item 1.3): when true, the DPI desync is restricted to `hostlist` domains
    /// (winws `--hostlist`) instead of catch-all. Set by ApplyProfile when scope.mode == Split.
    hostlist_only: bool,
    /// live-verification-only: overrides the resolved strategy's primary-stage repeats (dpi-desync-repeats
    /// sweep, evorift-ctl `strat <id> --repeats=N`). `None` = use the strategy's own compiled value.
    repeats_override: Option<u32>,
    /// Last telemetry sample cached here by the subscription thread (item 11.2). Used by
    /// `Command::Health` to derive the `HealthSignal` without requiring an active subscriber.
    last_metrics: Option<Metrics>,
    /// When `last_metrics` was last updated. `None` until the first telemetry tick arrives.
    metrics_at: Option<Instant>,
    /// Proof-of-protection state (see `VerifyState`).
    verify: VerifyState,
    /// Bumped on every Start/Stop/restart. A background probe checks this before writing its result
    /// back, so a slow probe from a session the user already stopped/restarted can't clobber newer state.
    verify_gen: u64,
}

impl Engine {
    fn new() -> Self {
        Self {
            running: false,
            state: RunState::Idle,
            strategy: String::new(),
            dns: String::new(),
            engine_id: "zapret".into(),
            hostlist: Vec::new(),
            limits: std::collections::HashMap::new(),
            app_modes: std::collections::HashMap::new(),
            dpi: engine::make_engine("zapret"),
            warp: WarpEngine::new(),
            full_warp: false,
            hostlist_only: false,
            repeats_override: None,
            last_metrics: None,
            metrics_at: None,
            verify: VerifyState::Unverified,
            verify_gen: 0,
        }
    }

    /// Resolve the active strategy for the current engine, applying the runtime hostlist-mode override
    /// (item 1.3). winws reads `Strategy::hostlist_only` to decide catch-all vs `--hostlist` gating.
    fn current_strategy(&self) -> engine::Strategy {
        let mut s = engine::strategy_by_id(&self.strategy);
        if self.hostlist_only {
            s.hostlist_only = true;
        }
        if let Some(r) = self.repeats_override {
            s.repeats = r;
        }
        s
    }

    /// Does anything actually need the WARP tunnel right now?
    ///
    /// FIXED 2026-08-14 (live test): this used to `return true` when `app_modes` was EMPTY — the
    /// default state on a fresh install and in every DPI-only mode. So plain "Hafif Koruma"
    /// (Discord+Roblox desync, no tunnel anywhere in its description) brought up a full WireGuard
    /// tunnel on every single Start. Combined with `sync_warp()` running under the engine mutex and
    /// `run_hidden()` having had no timeout, one slow `wireguard.exe /installtunnelservice` froze
    /// the whole service — which is what left the UI stuck on "Bağlanıyor" in EVERY mode.
    ///
    /// Empty now means "nothing asked for a tunnel", which is the honest reading: the tunnel is
    /// opt-in (VPN mode sets `full_warp`, or a per-app entry explicitly selects "warp").
    fn want_warp(&self) -> bool {
        self.app_modes.values().any(|(m, _)| m == "warp")
    }

    fn warp_target(&self) -> Option<bool> {
        if self.full_warp {
            Some(true)
        } else if self.want_warp() {
            Some(false)
        } else {
            None
        }
    }

    fn off_app_paths(&self) -> Vec<String> {
        self.app_modes
            .values()
            .filter(|(mode, path)| mode == "off" && !path.is_empty())
            .map(|(_, p)| p.clone())
            .collect()
    }

    fn limit_list(&self) -> Vec<(String, u32)> {
        self.limits
            .values()
            .filter(|(p, _)| !p.is_empty())
            .map(|(p, d)| (p.clone(), *d))
            .collect()
    }

    fn status(&self) -> EngineStatus {
        EngineStatus {
            running: self.running,
            strategy: self.strategy.clone(),
            dns: self.dns.clone(),
            engine: self.engine_id.clone(),
            state: self.state.as_str().to_string(),
            verify: self.verify.as_str().to_string(),
            verify_reason: self.verify.reason(),
        }
    }

    /// Engine no longer active (Stop, or a start/respawn failure) → proof-of-protection resets to
    /// Unverified and any in-flight probe from the prior session is invalidated via `verify_gen`.
    fn reset_verify(&mut self) {
        self.verify = VerifyState::Unverified;
        self.verify_gen += 1;
    }

    /// Derive the `HealthSignal` from the engine state + last cached telemetry (item 11.2).
    fn health(&self) -> ipc::HealthSignal {
        let age = self.metrics_at.map(|t| t.elapsed().as_secs());
        ipc::derive_health(
            self.running,
            self.state == RunState::Applying,
            self.state == RunState::Error,
            self.last_metrics.as_ref(),
            age,
        )
    }

    /// WARP tünelini istenen hedefe senkronize et (idempotent). Tünel kurulduysa rollback'e kaydet.
    fn sync_warp(&mut self) {
        match self.warp_target() {
            Some(full) => {
                if let Err(m) = self.warp.start(full) {
                    audit(&format!("warp start atlandı: {m}"));
                } else {
                    crate::rollback::record(Change::TunnelInstalled { name: crate::warp::TUNNEL_NAME.to_string() });
                }
            }
            None => self.warp.stop(),
        }
    }
}

/// Kick off the proof-of-protection probe: mark Verifying immediately (so the UI never keeps showing
/// a stale Verified from before this apply) and run the real TLS handshake on a background thread.
/// Caller must already hold `e`'s lock (`engine` is cloned for the thread; `e` is only touched here
/// synchronously). See `Engine::reset_verify` for the generation-guard rationale.
fn spawn_verify(engine: &Arc<Mutex<Engine>>, e: &mut Engine) {
    /// How many times the probe may fail before the engine is declared Broken.
    ///
    /// The probe used to run EXACTLY ONCE, immediately after start — straight into the window where
    /// winws has just spawned and the WinDivert driver is still attaching. A handshake attempted in
    /// that window loses, and because nothing ever re-probed, the single early failure LATCHED:
    /// the UI kept saying "Uygulandı ama çalışmıyor" indefinitely while the bypass was in fact
    /// working seconds later. Reported from real use ("first unreachable, then connection reset,
    /// then it opens") — the user was watching the engine warm up, and the probe sampled the worst
    /// moment and never looked again.
    const VERIFY_ATTEMPTS: usize = 3;
    /// Gap between retries. Long enough for the driver to attach, short enough that a genuinely
    /// blocked line still reaches Broken quickly rather than sitting on "checking" for a minute.
    const VERIFY_RETRY_GAP: Duration = Duration::from_secs(3);

    e.verify = VerifyState::Verifying;
    e.verify_gen += 1;
    let my_gen = e.verify_gen;
    let engine = Arc::clone(engine);
    std::thread::spawn(move || {
        let mut outcome = crate::verify::probe_discord();
        let mut attempt = 1;
        while !outcome.ok && attempt < VERIFY_ATTEMPTS {
            // Bail out the moment this probe is stale (stopped / restarted / mode switched), both
            // before sleeping and after — otherwise a retry could overwrite a NEWER probe's result.
            {
                let e = engine.lock().unwrap_or_else(|p| p.into_inner());
                if e.verify_gen != my_gen {
                    return;
                }
            }
            std::thread::sleep(VERIFY_RETRY_GAP);
            {
                let e = engine.lock().unwrap_or_else(|p| p.into_inner());
                if e.verify_gen != my_gen {
                    return;
                }
            }
            attempt += 1;
            outcome = crate::verify::probe_discord();
        }

        let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
        if e.verify_gen != my_gen {
            return; // stale — stopped/restarted since this probe was launched, discard the result
        }
        e.verify = if outcome.ok {
            VerifyState::Verified
        } else {
            VerifyState::Broken(outcome.reason)
        };
        audit(&format!(
            "verify ({attempt}/{VERIFY_ATTEMPTS} deneme): {}",
            match &e.verify {
                VerifyState::Verified => "verified (real TLS handshake to Discord succeeded)".to_string(),
                VerifyState::Broken(r) => format!("broken: {r}"),
                VerifyState::Unverified | VerifyState::Verifying => unreachable!(),
            }
        ));
    });
}

/// Boot'ta otomatik açılan korumanın varsayılan hostlist'i (state.svelte.ts CORE_SITES + YouTube).
const DEFAULT_HOSTLIST: &[&str] = &[
    "discord.com", "discordapp.com", "discord.gg", "discordapp.net", "discord.media",
    "gateway.discord.gg", "cdn.discordapp.com", "roblox.com", "www.roblox.com", "rbxcdn.com",
    "youtube.com", "googlevideo.com",
];

// ============================================================================================
// Protection-mode tuning.
//
// ⚠ THESE TWO REPEAT VALUES ARE PROVISIONAL — NOT MEASURED. Do not treat them as tuned.
// The ONLY dpi-desync-repeats value ever actually measured is 1, and that sweep
// (docs/LIVE-VERIFICATION.md, 2026-08-13 (c)) ran against www.google.com / www.microsoft.com /
// www.cloudflare.com — NEVER against Discord, and never against the catch-all path. 6 and 8 are
// picked as conservative middle ground between that single data point and the engine's own
// long-standing c1 default of 11. Anyone tuning these later: re-run the sweep against the real
// target set first, per mode, and record it in LIVE-VERIFICATION.md before changing them here.
// ============================================================================================

/// "Hafif Koruma" — Discord + Roblox only, hostlist-gated. DEFAULT MODE on first run.
const HAFIF_REPEATS: u32 = 6;

/// "Güçlü Koruma" — catch-all (no --hostlist, every TLS/443 flow desynced), using the desync chain
/// that was MEASURED to open the hardest domains on a real blocked line (2026-08-15).
///
/// A 14-config sweep found this to be the only preset that got the hard domains through: TTL-based
/// desync (fake + ttl 1 + autottl 3). Every c1 repeat count (8/11/20) left them at 0% while keeping
/// the control target at 100%, which is what ruled out repeat-count tuning as the answer. Verified
/// afterwards at 400/400 under 50s of sustained load, no engine restarts.
///
/// The preset id names an ISP only because that is where it was originally derived; here it is
/// simply "the chain that measurably works". Evidence is from ONE line — picking this per-line is
/// what Autopilot is meant to automate.
const GUCLU_STRATEGY: &str = "turkcell-hotspot";

/// Hafif Koruma's hostlist: Discord + Roblox only, per the shipped scope. Kept deliberately small —
/// each entry is a per-connection lookup in winws, and a broad list goes stale. Users needing wider
/// coverage switch to Güçlü Koruma (catch-all) rather than growing this.
const HAFIF_HOSTLIST: &[&str] = &[
    "discord.com", "discordapp.com", "discord.gg", "discordapp.net", "discord.media",
    "gateway.discord.gg", "cdn.discordapp.com",
    "roblox.com", "www.roblox.com", "rbxcdn.com",
];

fn dispatch(engine: &Arc<Mutex<Engine>>, cmd: Command) -> Response {
    if let Err(m) = ipc::validate(&cmd) {
        audit(&format!("REJECT {cmd:?}: {m}"));
        return Response::Error { message: m };
    }
    let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
    match cmd {
        Command::Start => {
            e.state = RunState::Applying;
            if e.strategy.is_empty() {
                e.strategy = "auto".into();
            }

            // Secure DNS is applied HERE, not merely recorded.
            //
            // FIXED 2026-08-15 (live test): this used to just set `e.dns = "cloudflare"` — a field,
            // never applied — so `status` reported dns=cloudflare while the adapters still used the
            // ISP resolver. On the measured line that resolver answers every Discord domain with
            // 195.175.254.2 (a sinkhole), and DPI desync CANNOT fix a wrong destination IP: the
            // connection times out at TCP, before any handshake exists to rewrite. Every strategy
            // and repeat value failed identically because of it.
            //
            // Applied with the engine lock RELEASED (run_dns shells out to PowerShell across every
            // adapter) — holding it here would reintroduce the freeze fixed in warp.rs.
            let dns_profile = if e.dns.is_empty() { "cloudflare".to_string() } else { e.dns.clone() };
            drop(e);
            let dns_applied = crate::dns::run_dns(&dns_profile);
            let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
            match &dns_applied {
                Ok(()) => {
                    e.dns = dns_profile;
                    audit("start: secure DNS uygulandı");
                }
                Err(m) => {
                    // Do NOT claim a provider we failed to set. Protection still starts (desync helps
                    // domains that aren't DNS-poisoned), but the status must not overstate it.
                    e.dns = "auto".into();
                    audit(&format!("start: DNS uygulanamadı, sistem DNS'i kullanılıyor: {m}"));
                }
            }
            if e.hostlist.is_empty() {
                // Used to be seeded by the removed boot-auto-protect block (P0-e fix, 2026-08-14) --
                // an explicit Start is now the only path in, so it has to seed this itself, same
                // fallback shape as strategy/dns just above.
                e.hostlist = DEFAULT_HOSTLIST.iter().map(|s| s.to_string()).collect();
            }
            let strat = e.current_strategy();
            let hostlist = e.hostlist.clone();
            match e.dpi.start(&strat, &hostlist) {
                Ok(()) => {
                    e.running = true;
                    e.state = RunState::Active;
                    let ll = e.limit_list();
                    e.dpi.set_limits(&ll);
                    e.sync_warp();
                    audit(&format!("start engine={} strategy={}", e.engine_id, strat.id));
                    spawn_verify(engine, &mut e);
                    Response::Status(e.status())
                }
                Err(m) => {
                    e.state = RunState::Error;
                    e.reset_verify();
                    Response::Error { message: m }
                }
            }
        }
        Command::Stop => {
            e.warp.stop();
            e.dpi.stop();
            e.running = false;
            e.state = RunState::Idle;
            e.reset_verify();
            audit("stop");
            Response::Status(e.status())
        }
        Command::Status => Response::Status(e.status()),
        Command::SetStrategy { id, repeats_override } => {
            audit(&format!("set_strategy {id} repeats_override={repeats_override:?}"));
            e.strategy = id;
            e.repeats_override = repeats_override;
            if e.running {
                // Force a restart so the new strategy actually takes effect: start() alone is idempotent
                // and would no-op on a live winws child. Brief (~1s) gap is fine for an explicit change.
                e.dpi.stop();
                let strat = e.current_strategy();
                let hl = e.hostlist.clone();
                if let Err(m) = e.dpi.start(&strat, &hl) {
                    e.state = RunState::Error;
                    e.running = false;
                    e.reset_verify();
                    return Response::Error { message: m };
                }
                spawn_verify(engine, &mut e); // new strategy → old handshake result no longer proves anything
            }
            Response::Status(e.status())
        }
        Command::SetEngine { id } => {
            audit(&format!("set_engine {id}"));
            if e.engine_id == id {
                return Response::Status(e.status());
            }
            let was_running = e.running;
            // Eski motoru tamamen durdur (WinDivert tek-sürücü: winws↔goodbyedpi çakışmasın).
            e.dpi.stop();
            e.engine_id = id.clone();
            e.dpi = engine::make_engine(&id);
            if was_running {
                e.state = RunState::Applying;
                let strat = e.current_strategy();
                let hl = e.hostlist.clone();
                match e.dpi.start(&strat, &hl) {
                    Ok(()) => {
                        e.state = RunState::Active;
                        spawn_verify(engine, &mut e); // new engine → old handshake result no longer proves anything
                    }
                    Err(m) => {
                        e.state = RunState::Error;
                        e.reset_verify();
                        return Response::Error { message: m };
                    }
                }
            }
            Response::Status(e.status())
        }
        Command::EngineCatalog => match serde_json::to_string(&engine::catalog()) {
            Ok(j) => Response::Data(j),
            Err(e) => Response::Error { message: e.to_string() },
        },
        Command::SetDns { profile } => {
            audit(&format!("set_dns {profile}"));
            // State honesty: apply FIRST, commit to Engine state only on success. This used to set
            // e.dns and snapshot e.status() before run_dns() ran, so a failed DNS change left the
            // engine (and every later Command::Status) reporting a provider that was never applied.
            drop(e);
            match crate::dns::run_dns(&profile) {
                Ok(()) => {
                    crate::rollback::record(Change::DnsChanged);
                    let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
                    e.dns = profile.clone();
                    audit(&format!("set_dns ok (uygulandı): {profile}"));
                    Response::Status(e.status())
                }
                Err(m) => {
                    audit(&format!("set_dns BAŞARISIZ (durum değişmedi): {m}"));
                    Response::Error { message: m }
                }
            }
        }
        Command::ResetDns => {
            audit("reset_dns");
            drop(e);
            match crate::dns::reset_dns() {
                Ok(()) => Response::Ok,
                Err(m) => Response::Error { message: m },
            }
        }
        Command::VerifyDns => {
            drop(e);
            match serde_json::to_string(&crate::dns::verify_dns()) {
                Ok(j) => Response::Data(j),
                Err(e) => Response::Error { message: e.to_string() },
            }
        }
        Command::BlockApp { id, path, block } => {
            audit(&format!("block_app {id} = {block}"));
            if block {
                crate::rollback::record(Change::FirewallRule { name: format!("evorift-block-{id}") });
            }
            drop(e);
            match crate::firewall::run_block(&id, &path, block) {
                Ok(()) => Response::Ok,
                Err(m) => Response::Error { message: m },
            }
        }
        Command::Repair { tool } => {
            drop(e);
            match crate::repair::run_repair(&tool) {
                Ok(()) => Response::Ok,
                Err(m) => Response::Error { message: m },
            }
        }
        Command::SetTweak { key, value } => {
            audit(&format!("set_tweak {key} = {value}"));
            drop(e);
            match crate::tweak::run_tweak(&key, &value) {
                Ok(()) => Response::Ok,
                Err(m) => Response::Error { message: m },
            }
        }
        Command::SetLimit { id, path, down, up } => {
            audit(&format!("set_limit {id} down={down} up={up}"));
            if down > 0 {
                e.limits.insert(id.clone(), (path.clone(), down));
            } else {
                e.limits.remove(&id);
            }
            let ll = e.limit_list();
            e.dpi.set_limits(&ll);
            drop(e);
            match crate::limit::run_limit(&id, &path, up) {
                Ok(()) => Response::Ok,
                Err(m) => Response::Error { message: m },
            }
        }
        Command::SetAppModes { modes } => {
            audit(&format!("set_app_modes ({} uygulama)", modes.len()));
            e.app_modes = modes.into_iter().map(|(id, mode, path)| (id, (mode, path))).collect();
            if e.running {
                e.sync_warp();
            }
            Response::Ok
        }
        Command::SetFullWarp { enable } => {
            audit(&format!("set_full_warp {enable}"));
            e.full_warp = enable;
            e.sync_warp();
            Response::Ok
        }
        Command::SetHostlist { domains } => {
            audit(&format!("set_hostlist ({} domain)", domains.len()));
            let was_running = e.running;
            e.hostlist = domains;
            if was_running {
                let strat = e.current_strategy();
                let hl = e.hostlist.clone();
                match e.dpi.start(&strat, &hl) {
                    Ok(()) => Response::Status(e.status()),
                    Err(m) => Response::Error { message: m },
                }
            } else {
                Response::Ok
            }
        }
        Command::SetProtectionMode { mode } => {
            audit(&format!("set_protection_mode {mode}"));
            // Snapshot everything this command touches, so a failed apply can roll the Engine back
            // to exactly what it was rather than leaving a half-applied mode reported as active.
            let prev = (e.strategy.clone(), e.repeats_override, e.hostlist_only, e.hostlist.clone());

            match mode.as_str() {
                "hafif" => {
                    e.strategy = "c1".into();
                    e.repeats_override = Some(HAFIF_REPEATS);
                    e.hostlist_only = true;
                    e.hostlist = HAFIF_HOSTLIST.iter().map(|s| s.to_string()).collect();
                }
                "guclu" => {
                    // MEASURED, not guessed (2026-08-15, laptop). A 14-config sweep against the
                    // domains that were failing 100% of the time found exactly ONE that opened
                    // them: this preset's TTL-based desync (fake + ttl 1 + autottl 3). Every c1
                    // variant (repeats 8/11/20), plain `fake`, and `superonline` left the hard
                    // domains at 0% while keeping the control target at 100% — so the failure was
                    // the desync method, not the repeat count. `multidisorder`, `tt`, `tt-alt` and
                    // `kablonet` were worse still: 0% on the control target too.
                    //
                    // Verified under load afterwards: 400/400 (100%) across pornhub.com,
                    // brazzers.com, xvideos.com and discord.com over 50s, no winws restarts.
                    //
                    // NOTE the preset id names an ISP because that is where it was first derived;
                    // it is used here purely as "the desync chain that measurably works", and the
                    // evidence is from ONE line. Another line may well need a different one — that
                    // per-line choice is exactly what Autopilot is meant to make automatically.
                    e.strategy = GUCLU_STRATEGY.into();
                    // NO repeats override: the configuration that scored 400/400 ran with the
                    // preset's own value. Forcing a repeat count here would ship something other
                    // than what was actually measured.
                    e.repeats_override = None;
                    // Catch-all: hostlist_only=false means winws gets NO --hostlist flag, so every
                    // TLS/443 flow is desynced (engine.rs build_args). The hostlist field is left
                    // populated but unused — it only matters when hostlist_only is true.
                    e.hostlist_only = false;
                }
                _ => return Response::Error { message: format!("geçersiz koruma modu: {mode}") },
            }

            if !e.running {
                // Nothing to verify yet — the mode is recorded and takes effect on the next Start.
                return Response::Status(e.status());
            }

            // Live switch: restart so winws actually picks up the new args (start() is idempotent
            // and would no-op against a live child), then re-probe — the previous mode's Verified
            // result says nothing about this one.
            e.dpi.stop();
            let strat = e.current_strategy();
            let hl = e.hostlist.clone();
            match e.dpi.start(&strat, &hl) {
                Ok(()) => {
                    e.running = true;
                    e.state = RunState::Active;
                    spawn_verify(engine, &mut e);
                    Response::Status(e.status())
                }
                Err(m) => {
                    // Roll the settings back so status() keeps describing the mode that is actually
                    // loaded, not the one we failed to switch to.
                    e.strategy = prev.0;
                    e.repeats_override = prev.1;
                    e.hostlist_only = prev.2;
                    e.hostlist = prev.3;
                    e.running = false;
                    e.state = RunState::Error;
                    e.reset_verify();
                    audit(&format!("set_protection_mode BAŞARISIZ, geri alındı: {m}"));
                    Response::Error { message: m }
                }
            }
        }
        // ---- Profil sistemi (docs/07 §4) ----
        Command::ListProfiles => match serde_json::to_string(&crate::profile::load_all()) {
            Ok(j) => Response::Data(j),
            Err(e) => Response::Error { message: e.to_string() },
        },
        Command::SaveProfile { json } => match crate::profile::import(&json) {
            Ok(_) => Response::Ok,
            Err(m) => Response::Error { message: m },
        },
        Command::DeleteProfile { id } => match crate::profile::delete(&id) {
            Ok(()) => Response::Ok,
            Err(m) => Response::Error { message: m },
        },
        Command::ExportProfile { id } => match crate::profile::export(&id) {
            Ok(j) => Response::Data(j),
            Err(m) => Response::Error { message: m },
        },
        Command::ApplyProfile { id } => {
            let r = apply_profile(&mut e, &id);
            // Desync/local-proxy AND tunnel profiles land here on success (e.running=true) — same
            // "is it actually working" question applies to both, so probe unconditionally.
            if e.running && matches!(r, Response::Status(_)) {
                spawn_verify(engine, &mut e);
            }
            r
        }
        // ---- Preflight / teşhis (docs/07 §8) ----
        Command::Preflight => {
            drop(e);
            match serde_json::to_string(&crate::preflight::run()) {
                Ok(j) => Response::Data(j),
                Err(e) => Response::Error { message: e.to_string() },
            }
        }
        Command::Diagnose { targets } => {
            drop(e);
            match serde_json::to_string(&crate::preflight::diagnose(&targets)) {
                Ok(j) => Response::Data(j),
                Err(e) => Response::Error { message: e.to_string() },
            }
        }
        // ---- Auto-Pilot (docs/07 §6) ----
        Command::AutoPilot { targets, depth } => run_autopilot(engine, e, targets, depth),
        // ---- Health signals (item 11.1 + 11.2) ----
        Command::Health => {
            match serde_json::to_string(&e.health()) {
                Ok(j) => Response::Data(j),
                Err(err) => Response::Error { message: err.to_string() },
            }
        }
        Command::TunnelStatus => {
            let state = crate::ipc::TunnelState {
                warp_running: e.warp.is_running(),
                warp_full: e.warp.is_full(),
                tunnel_installed: crate::warp::WarpEngine::is_installed(),
                handshake_ago_secs: crate::warp::WarpEngine::handshake_ago_secs(),
            };
            match serde_json::to_string(&state) {
                Ok(j) => Response::Data(j),
                Err(err) => Response::Error { message: err.to_string() },
            }
        }
        // ---- Rollback (docs/07 §5) ----
        Command::RollbackAll => {
            audit("rollback_all");
            // Önce aktif korumayı temiz durdur, sonra tüm sistem değişikliklerini geri al.
            e.warp.stop();
            e.dpi.stop();
            e.running = false;
            e.state = RunState::Idle;
            e.reset_verify();
            match crate::rollback::rollback_all() {
                Ok(()) => Response::Ok,
                Err(m) => Response::Error { message: m },
            }
        }
    }
}

/// Load a profile by id and apply it.
fn apply_profile(e: &mut Engine, id: &str) -> Response {
    match crate::profile::get(id) {
        Some(prof) => apply_profile_obj(e, &prof),
        None => Response::Error { message: format!("profil bulunamadı: {id}") },
    }
}

/// Apply a profile across all engine TYPES (item 7.2, docs/07 §4): desync (zapret/goodbyedpi) and
/// local-proxy (byedpi + routing) start the DPI engine configured from `engine_params`; a tunnel profile
/// (`engine == "warp"`) brings up WARP instead (full-tunnel for System scope, split for Split). DNS is
/// applied if the profile requests it. Runs the state machine (Applying → Active/Error).
fn apply_profile_obj(e: &mut Engine, prof: &crate::profile::Profile) -> Response {
    use crate::profile::ScopeMode;
    audit(&format!("apply_profile (engine={}, strategy={})", prof.engine, prof.strategy));
    e.state = RunState::Applying;
    let engine_id = if prof.engine.is_empty() { "zapret" } else { prof.engine.as_str() };

    // --- TUNNEL profile: no DPI engine; bring up WARP (full = System scope, split = Split scope). ---
    if engine_id == "warp" {
        e.dpi.stop();
        e.engine_id = "warp".into();
        e.full_warp = matches!(prof.scope.mode, ScopeMode::System);
        e.sync_warp();
        e.running = true;
        apply_profile_dns(e, prof);
        e.state = RunState::Active;
        return Response::Status(e.status());
    }

    // --- DESYNC / LOCAL-PROXY profile: (re)build the engine with its params, set strategy/hostlist, start. ---
    e.dpi.stop();
    e.engine_id = engine_id.to_string();
    e.dpi = engine::make_engine_with_params(engine_id, &prof.engine_params);
    e.strategy = if prof.strategy.is_empty() { "auto".into() } else { prof.strategy.clone() };
    e.repeats_override = None; // profiles carry no override concept — a stale sweep value must not leak in

    e.hostlist = prof.hostlist.clone();
    // Split scope → restrict desync to the profile's domains (winws --hostlist); System → catch-all.
    e.hostlist_only = matches!(prof.scope.mode, ScopeMode::Split);

    let strat = e.current_strategy();
    let hl = e.hostlist.clone();
    if let Err(m) = e.dpi.start(&strat, &hl) {
        e.state = RunState::Error;
        return Response::Error { message: m };
    }
    e.running = true;
    e.sync_warp(); // honors app_modes / full_warp for the DPI+tunnel combo
    apply_profile_dns(e, prof);
    e.state = RunState::Active;
    Response::Status(e.status())
}

/// Apply the profile's DNS provider if requested (records a rollback entry; OS call is best-effort).
fn apply_profile_dns(e: &mut Engine, prof: &crate::profile::Profile) {
    if prof.dns.enabled && !prof.dns.provider.is_empty() {
        e.dns = prof.dns.provider.clone();
        crate::rollback::record(Change::DnsChanged);
        if let Err(m) = crate::dns::run_dns(&prof.dns.provider) {
            audit(&format!("apply_profile dns skipped: {m}"));
        }
    }
}

/// Auto-Pilot'u çalıştır (docs/07 §6). Aktif motoru DURAKLAT (watchdog karışmasın) → adayları test et →
/// eski durumu geri yükle. Skor tablosunu Data(JSON) döndür. `e` kilidi tutuluyor → işi kilit DIŞINDA yap.
fn run_autopilot(
    engine: &Arc<Mutex<Engine>>,
    mut e: std::sync::MutexGuard<'_, Engine>,
    targets: Vec<String>,
    depth_str: String,
) -> Response {
    let depth = crate::autopilot::Depth::from_str_lenient(&depth_str);
    // Aktif motoru duraklat: watchdog Paused durumda motora dokunmaz (autopilot geçici motorlarla çakışmasın).
    let was_active = e.running;
    e.state = RunState::Paused;
    e.dpi.stop();
    e.running = false;
    drop(e); // kilidi bırak — autopilot saniyeler sürebilir; diğer komutlar bloke olmasın

    let rows = crate::autopilot::run(&targets, depth, |row| {
        audit(&format!("autopilot aday {} skor={}", row.engine, row.score));
    });

    // Eski durumu geri yükle.
    let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
    if was_active {
        let strat = e.current_strategy();
        let hl = e.hostlist.clone();
        match e.dpi.start(&strat, &hl) {
            Ok(()) => {
                e.running = true;
                e.state = RunState::Active;
            }
            Err(_) => e.state = RunState::Error,
        }
    } else {
        e.state = RunState::Idle;
    }
    drop(e);

    match serde_json::to_string(&rows) {
        Ok(j) => Response::Data(j),
        Err(err) => Response::Error { message: err.to_string() },
    }
}

/// Auto-Pilot STREAMING (item 6.5): like `run_autopilot` but writes each `ScoreRow` to the connection as
/// it completes (`Response::Data(row_json)`), then a terminal `Response::Ok`. Pauses the active engine for
/// the duration (watchdog skips Paused) and restores it afterward.
fn autopilot_stream(conn: &Stream, engine: &Arc<Mutex<Engine>>, targets: Vec<String>, depth_str: String) {
    // Validate via the equivalent command (targets + depth whitelist).
    let cmd = Command::AutoPilot { targets: targets.clone(), depth: depth_str.clone() };
    if let Err(m) = ipc::validate(&cmd) {
        let _ = ipc::write_msg(conn, &Response::Error { message: m });
        return;
    }
    let depth = crate::autopilot::Depth::from_str_lenient(&depth_str);

    // Pause the active engine so the temporary RunOnce engines don't conflict (watchdog skips Paused).
    let was_active = {
        let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
        let wa = e.running;
        e.state = RunState::Paused;
        e.dpi.stop();
        e.running = false;
        wa
    };

    // Stream each row as it finishes.
    crate::autopilot::run(&targets, depth, |row| {
        if let Ok(j) = serde_json::to_string(row) {
            let _ = ipc::write_msg(conn, &Response::Data(j));
        }
    });

    // Restore the previous state.
    {
        let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
        if was_active {
            let strat = e.current_strategy();
            let hl = e.hostlist.clone();
            match e.dpi.start(&strat, &hl) {
                Ok(()) => {
                    e.running = true;
                    e.state = RunState::Active;
                }
                Err(_) => e.state = RunState::Error,
            }
        } else {
            e.state = RunState::Idle;
        }
    }

    let _ = ipc::write_msg(conn, &Response::Ok); // terminal "done" marker
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

#[cfg(windows)]
fn read_net_octets() -> (u64, u64) {
    use windows_sys::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2};
    let mut rx: u64 = 0;
    let mut tx: u64 = 0;
    unsafe {
        let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
        if GetIfTable2(&mut table) != 0 || table.is_null() {
            return (0, 0);
        }
        let n = (*table).NumEntries as usize;
        let rows = (*table).Table.as_ptr();
        for i in 0..n {
            let row = &*rows.add(i);
            if row.OperStatus == 1 && (row.Type == 6 || row.Type == 71) {
                rx = rx.saturating_add(row.InOctets);
                tx = tx.saturating_add(row.OutOctets);
            }
        }
        FreeMibTable(table as *const core::ffi::c_void);
    }
    (rx, tx)
}
#[cfg(not(windows))]
fn read_net_octets() -> (u64, u64) {
    (0, 0)
}

fn tcp_ping() -> (bool, u32) {
    use std::net::TcpStream;
    use std::time::Instant;
    let addr = match "1.1.1.1:443".parse() {
        Ok(a) => a,
        Err(_) => return (false, 0),
    };
    let t = Instant::now();
    match TcpStream::connect_timeout(&addr, Duration::from_millis(1000)) {
        Ok(_) => (true, t.elapsed().as_millis().min(u32::MAX as u128) as u32),
        Err(_) => (false, 0),
    }
}

fn telemetry_loop(conn: &Stream, engine: &Arc<Mutex<Engine>>) {
    use std::collections::VecDeque;
    let mut last_ping: u32 = 0;
    let mut loss_win: VecDeque<bool> = VecDeque::with_capacity(20);
    let mut prev = read_net_octets();
    let mut prev_t = std::time::Instant::now();
    loop {
        std::thread::sleep(Duration::from_secs(1));
        let running = engine.lock().unwrap_or_else(|p| p.into_inner()).running;
        if !running {
            prev = read_net_octets();
            prev_t = std::time::Instant::now();
            if ipc::write_msg(conn, &Response::Telemetry(Metrics::default())).is_err() {
                break;
            }
            continue;
        }
        let now = read_net_octets();
        let now_t = std::time::Instant::now();
        let secs = now_t.duration_since(prev_t).as_secs_f64().max(0.001);
        let drx = now.0.saturating_sub(prev.0);
        let dtx = now.1.saturating_sub(prev.1);
        prev = now;
        prev_t = now_t;
        let down = round1((drx as f64 * 8.0) / 1_000_000.0 / secs);
        let up = round1((dtx as f64 * 8.0) / 1_000_000.0 / secs);
        let (ok, rtt) = tcp_ping();
        if loss_win.len() == 20 {
            loss_win.pop_front();
        }
        loss_win.push_back(ok);
        let fails = loss_win.iter().filter(|s| !**s).count();
        let loss = round1(fails as f64 * 100.0 / loss_win.len().max(1) as f64);
        let ping = if ok { rtt } else { last_ping };
        let jitter = if ok && last_ping > 0 {
            (ping as i64 - last_ping as i64).unsigned_abs() as u32
        } else {
            0
        };
        if ok {
            last_ping = ping;
        }
        let m = Metrics { running: true, ping, jitter, loss, down, up };
        // Cache the latest sample in the engine so Command::Health can read it even when
        // the subscriber has disconnected (last known-good metrics remain accessible).
        {
            let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
            e.last_metrics = Some(m.clone());
            e.metrics_at = Some(Instant::now());
        }
        if ipc::write_msg(conn, &Response::Telemetry(m)).is_err() {
            break;
        }
    }
}

/// Token üret (16 bayt hex) ve dosyaya yaz; başarısız olsa da token'ı döndür.
pub fn ensure_token() -> String {
    let mut bytes = [0u8; 16];
    if getrandom::getrandom(&mut bytes).is_err() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        bytes[..16].copy_from_slice(&nanos.to_le_bytes());
    }
    let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    // Create whatever directory token_path() lives in (dev vs release use different dirs).
    let tok = ipc::token_path();
    if let Some(dir) = tok.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&tok, &token);
    #[cfg(all(windows, not(debug_assertions)))]
    harden_token_acl(&tok); // ACL hardening only in release — embedded dev server runs as normal user
    token
}

/// Sabit-zamanlı token karşılaştırma (P7 A2).
fn ct_token_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    core::hint::black_box(diff) == 0
}

/// Token dosyasının DACL'ini sıkılaştır (SYSTEM=Full, Administrators=Full, Interaktif=Read).
/// Only called in release builds where the full service binary runs as SYSTEM.
#[cfg(all(windows, not(debug_assertions)))]
fn harden_token_acl(path: &std::path::Path) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{LocalFree, BOOL, HLOCAL};
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SetNamedSecurityInfoW, SDDL_REVISION_1,
        SE_FILE_OBJECT,
    };
    use windows_sys::Win32::Security::{
        GetSecurityDescriptorDacl, ACL, DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
        PSECURITY_DESCRIPTOR,
    };
    let sddl: Vec<u16> = "D:PAI(A;;FA;;;SY)(A;;FA;;;BA)(A;;FR;;;IU)\0".encode_utf16().collect();
    let mut wpath: Vec<u16> = path.as_os_str().encode_wide().collect();
    wpath.push(0);
    unsafe {
        let mut psd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut psd,
            std::ptr::null_mut(),
        ) == 0
        {
            audit("UYARI: token SDDL çözümlenemedi (ACL atlandı)");
            return;
        }
        let mut dacl_present: BOOL = 0;
        let mut dacl_defaulted: BOOL = 0;
        let mut pdacl: *mut ACL = std::ptr::null_mut();
        let got = GetSecurityDescriptorDacl(psd, &mut dacl_present, &mut pdacl, &mut dacl_defaulted);
        if got != 0 && dacl_present != 0 {
            let rc = SetNamedSecurityInfoW(
                wpath.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                pdacl,
                std::ptr::null(),
            );
            if rc != 0 {
                audit(&format!("UYARI: token ACL ayarlanamadı (Win32 {rc})"));
            }
        } else {
            audit("UYARI: token DACL alınamadı (ACL atlandı)");
        }
        LocalFree(psd as HLOCAL);
    }
}

fn handle_conn(conn: Stream, engine: Arc<Mutex<Engine>>, token: String) {
    let mut reader = BufReader::new(&conn);
    match ipc::read_msg::<Request>(&mut reader) {
        Ok(Request::Hello { token: t }) if ct_token_eq(&t, &token) => {
            if ipc::write_msg(&conn, &Response::Ok).is_err() {
                return;
            }
        }
        _ => {
            let _ = ipc::write_msg(&conn, &Response::Error { message: "handshake reddedildi".into() });
            audit("REJECT handshake");
            return;
        }
    }
    while let Ok(req) = ipc::read_msg::<Request>(&mut reader) {
        // Item 10.2: validate all incoming requests before dispatch (covers AutoPilotStream size/depth,
        // token size on duplicate Hello, and Command validation as a second gate after dispatch's own check).
        if let Err(m) = ipc::validate_request(&req) {
            audit(&format!("REJECT request: {m}"));
            let _ = ipc::write_msg(&conn, &Response::Error { message: m });
            continue;
        }
        match req {
            Request::Command { cmd } => {
                let resp = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| dispatch(&engine, cmd)))
                    .unwrap_or_else(|_| {
                        audit("PANIC dispatch içinde — kurtarıldı");
                        Response::Error { message: "iç hata (kurtarıldı)".into() }
                    });
                if ipc::write_msg(&conn, &resp).is_err() {
                    break;
                }
            }
            Request::Subscribe => {
                telemetry_loop(&conn, &engine);
                break;
            }
            Request::AutoPilotStream { targets, depth } => {
                autopilot_stream(&conn, &engine, targets, depth);
                break;
            }
            Request::Hello { .. } => {
                let _ = ipc::write_msg(&conn, &Response::Error { message: "beklenmeyen ikinci handshake".into() });
                break;
            }
        }
    }
}

/// Named-pipe sunucusunu çalıştır (bloklar). Servis ikilisi ve dev'de gömülü sunucu kullanır.
pub fn serve_blocking() -> io::Result<()> {
    let name = PIPE_NAME.to_ns_name::<GenericNamespaced>().map_err(io::Error::other)?;
    let opts = ListenerOptions::new().name(name);
    #[cfg(windows)]
    let opts = {
        use interprocess::os::windows::local_socket::ListenerOptionsExt;
        use interprocess::os::windows::security_descriptor::SecurityDescriptor;
        use widestring::U16CString;
        match U16CString::from_str("D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)")
            .ok()
            .and_then(|s| SecurityDescriptor::deserialize(&s).ok())
        {
            Some(sd) => opts.security_descriptor(sd),
            None => {
                audit("UYARI: pipe ACL ayarlanamadı, varsayılan SD ile açılıyor");
                opts
            }
        }
    };
    let listener = opts.create_sync()?;
    let token = ensure_token();
    let engine = Arc::new(Mutex::new(Engine::new()));
    // Recover any half-applied changes from a previous crash into the global rollback log (item 7.3).
    crate::rollback::load_global();
    // Profilleri tohumla (ilk çalıştırma) — UI ListProfiles çağırınca hazır olsun.
    crate::profile::seed_defaults();
    audit("listening");

    // P0-e fix (2026-08-14): protection used to boot-auto-start unconditionally here (servis
    // çalışıyor = korumalı), with no persisted preference to gate it and no way for `off` to
    // survive a service/process restart — Command::Stop only ever touched the in-memory Engine
    // this serve_blocking() call owns, so the NEXT invocation (reboot, crash+SCM-restart, a fresh
    // --console run) always came back up protected regardless of what the user last chose. Opt-in
    // is the smaller fix (no new persistence layer to build and get right tonight): protection now
    // starts ONLY on an explicit Command::Start, same as `Engine::new()`'s own default of
    // running=false/state=Idle a few lines up. See also state.svelte.ts's autoProtect flag.
    audit("listening idle (boot auto-protect kapalı — P0-e, kullanıcı Start demeden korumaya geçmez)");

    // winws watchdog + per-app off PID exclusion (5 sn). Paused durumda (Auto-Pilot) dokunma.
    {
        let engine = Arc::clone(&engine);
        std::thread::spawn(move || {
        // STABILITY (2026-08-15): exclusion changes RESTART winws (there is no filter hot-reload —
        // see WinwsEngine::set_exclusion, which kills the child so the watchdog respawns it with
        // new args). The exclusion set is built from the LIVE SOURCE PORTS of every app in "off"
        // mode, and source ports churn constantly as those apps open connections. So with even one
        // active "off" app, this loop tore the engine down and rebuilt it EVERY 5 SECONDS, and
        // every connection in flight died with it. That is the reported "works, then stops, then
        // works" behaviour: the bypass was fine, it was just never allowed to stay up.
        //
        // Restarts driven by exclusion churn are now rate-limited. A missed port for a few seconds
        // only means one "off" app briefly keeps being bypassed — a far smaller harm than dropping
        // every connection on the machine. Respawn-if-the-engine-died is NOT rate-limited: that
        // path must stay immediate, and it is idempotent when the child is alive.
        const EXCL_MIN_INTERVAL: Duration = Duration::from_secs(60);
        let mut last_excl_change: Option<std::time::Instant> = None;
        let mut last_dns_heal: Option<std::time::Instant> = None;
        loop {
            std::thread::sleep(Duration::from_secs(5));
            let (running, paused, off_paths) = {
                let e = engine.lock().unwrap_or_else(|p| p.into_inner());
                (e.running, e.state == RunState::Paused, e.off_app_paths())
            };
            if paused {
                continue; // Auto-Pilot adayları çalışıyor → karışma
            }
            let excl = if running && !off_paths.is_empty() {
                crate::pid_scan::scan(&off_paths)
            } else {
                crate::pid_scan::ExclusionPorts::default()
            };
            // SCOPED on purpose. std::sync::Mutex is NOT reentrant: this guard must be dropped
            // before the WARP and DNS blocks below take the lock again, or the watchdog deadlocks
            // against itself on its very first tick and holds the engine lock forever — every
            // dispatch() then blocks and the whole service is dead 5 seconds after start. That is
            // exactly what an unscoped `let mut e = engine.lock()` here caused.
            {
                let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
                if e.running && e.state != RunState::Paused {
                    // Only push a new exclusion set when it actually differs AND we haven't just
                    // restarted for the same reason. set_exclusion() is a no-op on an unchanged set,
                    // so an unchanged set costs nothing and never trips the timer.
                    let may_change = last_excl_change
                        .map(|t| t.elapsed() >= EXCL_MIN_INTERVAL)
                        .unwrap_or(true);
                    if may_change {
                        if e.dpi.exclusion_differs(&excl) {
                            last_excl_change = Some(std::time::Instant::now());
                            audit("watchdog: exclusion degisti — winws yeniden baslatiliyor");
                        }
                        e.dpi.set_exclusion(&excl);
                    }
                    let strat = e.current_strategy();
                    let hl = e.hostlist.clone();
                    // Respawn Result'ı ASLA at ma — çocuk süreç kayıp ve yeniden başlatılamıyorsa (bundle
                    // silindi/kilitlendi) running:true yalan söylemeye devam eder (bkz. evorift-remote-testing:
                    // "silent success is the enemy").
                    if let Err(m) = e.dpi.start(&strat, &hl) {
                        e.running = false;
                        e.state = RunState::Error;
                        e.reset_verify();
                        audit(&format!("watchdog: motor kayboldu, yeniden başlatılamadı: {m}"));
                    }
                }
            }

            // ---- WARP reconciliation, OUTSIDE the engine lock -------------------------------
            // This used to run inside the `if e.running` block above, holding the guard. WarpEngine
            // ::start() shells out to wireguard.exe twice (/uninstalltunnelservice then
            // /installtunnelservice, each bounded at CHILD_TIMEOUT=25s), so a slow tunnel install
            // could hold the engine mutex for ~50s. Every dispatch() blocks on that same lock, so a
            // single mode switch then failed with "servis 45s içinde yanıt vermedi" — observed
            // live, and previously (before the client timeout existed) it hung forever instead.
            //
            // Decide under the lock, act without it: only the cheap state read needs exclusivity.
            let warp_action = {
                let e = engine.lock().unwrap_or_else(|p| p.into_inner());
                if e.running && e.state != RunState::Paused {
                    match e.warp_target() {
                        Some(full) if !e.warp.is_running() => Some(Some(full)),
                        None if e.warp.is_running() => Some(None),
                        _ => None,
                    }
                } else {
                    None
                }
            };
            if let Some(target) = warp_action {
                let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
                // Re-check under the lock: the target may have changed while we were unlocked.
                match (target, e.warp_target()) {
                    (Some(full), Some(want)) if want == full && !e.warp.is_running() => {
                        if let Err(m) = e.warp.start(full) {
                            audit(&format!("watchdog: warp baslatilamadi: {m}"));
                        }
                    }
                    (None, None) if e.warp.is_running() => e.warp.stop(),
                    _ => {}
                }
            }

            // ---- DNS drift self-heal -------------------------------------------------------
            // We only ever recorded the DNS we APPLIED; we never re-read the adapters. Anything
            // that changes them behind our back — another network tool, a VPN client, Windows, a
            // user, or (as seen in testing) an external recovery script — leaves the service
            // reporting dns=cloudflare while the machine has silently gone back to the ISP
            // resolver. On a line that sinkholes Discord that is fatal but nearly invisible: the
            // engine is fine, the strategy is fine, and every connection still dies at TCP because
            // the address itself is wrong. The user sees "Uygulandı ama çalışmıyor" and no reason.
            //
            // So when protection is running and the proof-of-protection probe says BROKEN, check
            // whether DNS drifted; if it did, re-apply it and re-probe. Only on Broken (not on
            // every tick) so the cost is paid solely when something is already wrong, and
            // rate-limited so a genuinely blocked line can't turn into a DNS-reapply loop.
            let needs_dns_check = {
                let e = engine.lock().unwrap_or_else(|p| p.into_inner());
                e.running
                    && matches!(e.verify, VerifyState::Broken(_))
                    && !e.dns.is_empty()
                    && e.dns != "auto"
            };
            let may_heal = last_dns_heal
                .map(|t: std::time::Instant| t.elapsed() >= Duration::from_secs(60))
                .unwrap_or(true);
            if needs_dns_check && may_heal {
                let want = {
                    let e = engine.lock().unwrap_or_else(|p| p.into_inner());
                    e.dns.clone()
                };
                // NEVER hold the engine lock across this: verify_dns/run_dns shell out to
                // PowerShell across every adapter and can take seconds (see the warp.rs freeze).
                let actual = crate::dns::verify_dns();
                let drifted = !actual.secure
                    || !actual.provider.eq_ignore_ascii_case(match want.as_str() {
                        "cloudflare" => "Cloudflare",
                        "quad9" => "Quad9",
                        "adguard" => "AdGuard",
                        "google" => "Google",
                        _ => "",
                    });
                if drifted {
                    last_dns_heal = Some(std::time::Instant::now());
                    audit(&format!(
                        "DNS kaymis (beklenen {want}, gerçek [{}]) — yeniden uygulanıyor",
                        actual.servers.join(", ")
                    ));
                    match crate::dns::run_dns(&want) {
                        Ok(()) => {
                            audit("DNS yeniden uygulandı — doğrulama tekrar çalıştırılıyor");
                            let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
                            if e.running {
                                spawn_verify(&engine, &mut e);
                            }
                        }
                        Err(m) => audit(&format!("DNS yeniden uygulanamadı: {m}")),
                    }
                }
            }
        }
        });
    }

    for conn in listener.incoming().filter_map(Result::ok) {
        let e = Arc::clone(&engine);
        let tok = token.clone();
        std::thread::spawn(move || handle_conn(conn, e, tok));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{DnsCfg, Profile, Scope, ScopeMode, SCHEMA_VERSION};

    fn prof(id: &str, engine: &str, mode: ScopeMode) -> Profile {
        Profile {
            schema_version: SCHEMA_VERSION,
            id: id.into(),
            name: "T".into(),
            engine: engine.into(),
            isp: String::new(),
            scope: Scope { mode, apps: vec![], browsers: false, folders: vec![] },
            dns: DnsCfg::default(),
            strategy: "auto".into(),
            hostlist: vec!["discord.com".into()],
            engine_params: serde_json::Value::Null,
        }
    }

    /// Item 11.2: Engine::health() derives from running state + cached metrics.
    #[test]
    fn engine_health_derives_from_state_and_metrics() {
        let mut e = Engine::new();

        // Fresh engine: not running → not healthy, not loading, not error.
        let h = e.health();
        assert!(!h.healthy, "idle engine not healthy");
        assert!(!h.loading);
        assert!(!h.error);

        // Applying state → loading.
        e.state = RunState::Applying;
        assert!(e.health().loading, "Applying → loading");

        // Error state → error.
        e.state = RunState::Error;
        assert!(e.health().error, "Error → error");

        // Inject fresh good metrics: running + recent + ping>0 + loss<100 → healthy.
        e.running = true;
        e.state = RunState::Active;
        e.last_metrics = Some(Metrics { running: true, ping: 30, jitter: 2, loss: 5.0, down: 10.0, up: 2.0 });
        e.metrics_at = Some(Instant::now());
        let h2 = e.health();
        assert!(h2.healthy, "running + fresh good metrics → healthy");
        assert_eq!(h2.ping_ms, 30);
        assert!((h2.loss_pct - 5.0).abs() < f64::EPSILON);

        // Remove metrics → not healthy even while running.
        e.last_metrics = None;
        e.metrics_at = None;
        assert!(!e.health().healthy, "no metrics → not healthy");
    }

    /// Proof-of-protection (item: honesty gate): `EngineStatus.verify` must never claim more than the
    /// state machine actually knows. A fresh engine and a running-but-unverified engine both report
    /// "unverified", never "verified" — the UI's "applied-unverified must never render as protected"
    /// invariant depends on this being true at the source, not patched over in the frontend.
    #[test]
    fn verify_state_never_claims_protection_it_has_not_earned() {
        let mut e = Engine::new();
        assert_eq!(e.status().verify, "unverified");
        assert_eq!(e.status().verify_reason, "");

        // Engine reports running, but nothing probed it yet → still unverified, not "protected".
        e.running = true;
        e.state = RunState::Active;
        assert_eq!(e.status().verify, "unverified");

        e.verify = VerifyState::Verifying;
        assert_eq!(e.status().verify, "verifying");

        e.verify = VerifyState::Verified;
        assert_eq!(e.status().verify, "verified");

        e.verify = VerifyState::Broken("discord.com: TCP: connection reset".into());
        assert_eq!(e.status().verify, "broken");
        assert_eq!(e.status().verify_reason, "discord.com: TCP: connection reset");

        // reset_verify (Stop / failed start / failed respawn) must always fall back to Unverified —
        // a stale "verified" surviving a stop would be exactly the silent-success bug this closes.
        e.verify_gen = 0;
        e.reset_verify();
        assert_eq!(e.verify, VerifyState::Unverified);
        assert_eq!(e.verify_gen, 1, "generation bumps so an in-flight probe from before the reset is discarded");
    }

    /// The two shipped protection modes must map onto the winws arg builder the way the UI claims:
    /// Hafif = hostlist-gated to Discord+Roblox, Güçlü = catch-all (NO --hostlist) using the
    /// MEASURED desync chain. Asserted through the real Strategy → build_args path so a change to
    /// either mode's wiring fails here instead of shipping a label that overstates its scope.
    ///
    /// The two modes no longer share one chain: Güçlü switched to the TTL-based preset after a
    /// sweep showed it was the only configuration that opened the hardest domains (400/400 under
    /// load, 2026-08-15) while every c1 repeat count left them at 0%.
    #[cfg(windows)]
    #[test]
    fn protection_modes_produce_the_scope_the_ui_promises() {
        let winws = crate::engine::make_engine("zapret");

        // --- Hafif: gated to the Discord + Roblox list ---
        let mut e = Engine::new();
        e.strategy = "c1".into();
        e.repeats_override = Some(HAFIF_REPEATS);
        e.hostlist_only = true;
        e.hostlist = HAFIF_HOSTLIST.iter().map(|s| s.to_string()).collect();
        let hafif = winws.build_args(&e.current_strategy(), &e.hostlist);
        assert!(
            hafif.iter().any(|a| a.starts_with("--hostlist=")),
            "Hafif must gate by hostlist — otherwise it silently covers everything"
        );
        assert!(
            hafif.iter().any(|a| a == &format!("--dpi-desync-repeats={HAFIF_REPEATS}")),
            "Hafif must use HAFIF_REPEATS on the primary TLS stage"
        );
        assert!(
            HAFIF_HOSTLIST.contains(&"discord.com") && HAFIF_HOSTLIST.contains(&"roblox.com"),
            "Hafif's scope is Discord + Roblox"
        );
        assert!(
            !HAFIF_HOSTLIST.iter().any(|d| d.contains("youtube")),
            "Hafif must NOT quietly widen beyond the scope its label promises"
        );

        // --- Güçlü: catch-all, TTL-based desync (the measured winner) ---
        let mut g = Engine::new();
        g.strategy = GUCLU_STRATEGY.into();
        g.repeats_override = None; // the 400/400 run used the preset's own value — do not override
        g.hostlist_only = false;
        g.hostlist = HAFIF_HOSTLIST.iter().map(|s| s.to_string()).collect(); // populated but unused
        let guclu = winws.build_args(&g.current_strategy(), &g.hostlist);
        assert!(
            !guclu.iter().any(|a| a.starts_with("--hostlist=")),
            "Güçlü is catch-all: a --hostlist flag here would silently narrow it"
        );
        // The TTL knobs ARE the reason this config beats the hard domains — losing them silently
        // would take the hard sites back to 0% while everything still looked fine.
        assert!(
            guclu.iter().any(|a| a.starts_with("--dpi-desync-ttl=")) ||
            guclu.iter().any(|a| a.starts_with("--dpi-desync-autottl=")),
            "Güçlü's measured config is TTL-based; without a ttl/autottl arg it is not that config"
        );
        assert_ne!(
            crate::engine::strategy_by_id(GUCLU_STRATEGY).desync,
            "",
            "Güçlü's strategy id must resolve to a real preset, not fall through to an empty one"
        );
    }

    /// REGRESSION (2026-08-14 live test): a DPI-only protection start must NEVER ask for a WARP
    /// tunnel. `want_warp()` used to return true whenever `app_modes` was empty — the default on a
    /// fresh install — so plain "Hafif Koruma" ran `wireguard.exe /installtunnelservice` on every
    /// Start, under the engine mutex, with no timeout on the child. One slow tunnel install then
    /// froze the entire service and the UI sat on "Bağlanıyor" forever, in every mode.
    ///
    /// The tunnel is opt-in: only `full_warp` (VPN mode) or an explicit per-app "warp" entry.
    #[test]
    fn dpi_only_start_never_asks_for_a_tunnel() {
        let mut e = Engine::new();
        assert!(e.app_modes.is_empty(), "fresh engine has no per-app modes");
        assert!(!e.want_warp(), "empty app_modes must NOT mean 'everyone wants a tunnel'");
        assert_eq!(e.warp_target(), None, "DPI-only start must not touch the tunnel at all");

        // Per-app "dpi"/"off" entries are still not a tunnel request.
        e.app_modes.insert("a".into(), ("dpi".into(), String::new()));
        e.app_modes.insert("b".into(), ("off".into(), String::new()));
        assert!(!e.want_warp());
        assert_eq!(e.warp_target(), None);

        // An explicit per-app "warp" entry IS a request — split tunnel (full = false).
        e.app_modes.insert("c".into(), ("warp".into(), String::new()));
        assert!(e.want_warp());
        assert_eq!(e.warp_target(), Some(false));

        // VPN mode overrides everything → full tunnel.
        let mut v = Engine::new();
        v.full_warp = true;
        assert_eq!(v.warp_target(), Some(true), "VPN mode must still bring the tunnel up");
    }

    /// Item 7.2: apply_profile_obj handles desync, local-proxy, and tunnel profiles. Test env has no
    /// winws.exe/ciadpi.exe bundle, so desync/local-proxy engines must HONESTLY error rather than take
    /// the old silent-Ok "sim" path (evorift-remote-testing: silent success is the enemy).
    #[test]
    fn apply_three_engine_kinds() {
        let mut e = Engine::new();
        // desync (zapret) — no bundle in test env → honest error, running stays false
        let r = apply_profile_obj(&mut e, &prof("p1", "zapret", ScopeMode::System));
        assert!(matches!(r, Response::Error { .. }));
        assert_eq!(e.engine_id, "zapret");
        assert!(!e.running);
        // local-proxy (byedpi), Split scope → hostlist mode is set before start() is attempted
        apply_profile_obj(&mut e, &prof("p2", "byedpi", ScopeMode::Split));
        assert_eq!(e.engine_id, "byedpi");
        assert!(e.hostlist_only);
        // tunnel (warp), System scope → full tunnel (unconditional path, no binary gate)
        apply_profile_obj(&mut e, &prof("p3", "warp", ScopeMode::System));
        assert_eq!(e.engine_id, "warp");
        assert!(e.full_warp);
        assert!(e.running);
    }
}
