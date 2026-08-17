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

    // ---- Added 2026-08-16 --------------------------------------------------------------------
    /// Active user-facing mode ("hafif" | "guclu"). Lives HERE, not only in the UI's localStorage:
    /// the service is the one that knows what is actually loaded, and the two used to disagree
    /// after any service restart.
    mode: String,
    /// Last full probe report (targets + control). Drives the UI's per-site coverage list and the
    /// harm verdict; `None` until the first probe completes.
    probe: Option<crate::verify::ProtectionReport>,
    /// "" (never measured) | "tuning" | "tuned" | "gave_up".
    tuning: String,
    /// Live measurement progress, so the UI can say "3/7" instead of spinning silently for half a
    /// minute. A user who cannot tell progress from a hang assumes a hang.
    tuning_step: u32,
    tuning_total: u32,
    /// Domains the user added on top of the shipped wide list.
    extra_domains: Vec<String>,
    /// Should protection come back up on its own at boot? Persisted; see `PersistedState`.
    autostart: bool,
    /// Rate-limit for automatic re-tuning, so a genuinely blocked line cannot turn into a
    /// tune-restart-tune loop that never lets a connection live.
    last_auto_tune: Option<Instant>,
    /// Catch-all fallback layer id for the CURRENT plan (see `current_strategy`). Empty = none.
    fallback: &'static str,
    /// How many times harm has forced a fallback in this session. Bounded so a line that is broken
    /// for reasons of its own cannot drive an endless revert/restart loop.
    harm_reverts: u8,
    /// Is a DPI child process supposed to be alive right now?
    ///
    /// False for DNS-only protection — the case where measurement said the honest action is to
    /// touch no packets at all. Without this flag, "is the engine running" collapses into "is
    /// winws alive", and a correctly-configured DNS-only line would report itself broken.
    dpi_expected: bool,
}

/// What survives a reboot.
///
/// The absence of this file was the whole of the "I restarted my PC and evorift did not come back"
/// report: `serve_blocking()` came up Idle with nothing to restore, deliberately, because the
/// earlier unconditional boot-auto-protect had no user setting to gate it (the P0-e fix). The
/// setting is the missing half — with it, restoring the user's last choice is not a surprise, it is
/// the thing they asked for.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct PersistedState {
    /// Was protection ON when we last shut down?
    #[serde(default)]
    running: bool,
    /// Last user-facing mode.
    #[serde(default)]
    mode: String,
    #[serde(default)]
    dns: String,
    /// Master switch for restoring the above at boot. Default TRUE: a user who turned protection on
    /// and rebooted expects it on. Turning it off is one toggle away and is remembered.
    #[serde(default = "yes")]
    autostart: bool,
    #[serde(default)]
    extra_domains: Vec<String>,
}

fn yes() -> bool {
    true
}

/// The persisted state, DPAPI-encrypted (see `secure`). Holds `extra_domains` — the domains the
/// USER added, which is the only genuinely personal data this app keeps.
fn state_path() -> std::path::PathBuf {
    ipc::data_dir().join("state.bin")
}

/// The plaintext file this used to be. Read once, migrated, deleted — never written again.
fn legacy_state_path() -> std::path::PathBuf {
    ipc::data_dir().join("state.json")
}

fn defaults() -> PersistedState {
    PersistedState { autostart: true, ..Default::default() }
}

fn load_state() -> PersistedState {
    if let Some(bytes) = crate::secure::read_encrypted(&state_path()) {
        return serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            crate::elog::warn("service", "state_parse", &format!("stored state unreadable, using defaults: {e}"));
            defaults()
        });
    }

    // MIGRATION: an install from before the store was encrypted left a plaintext state.json
    // containing the user's added domains. Read it once, re-save it encrypted, and DELETE it —
    // leaving it behind would mean the encryption changed nothing for existing users, which is the
    // usual way a privacy fix quietly fails to apply to the people who already have the data.
    let legacy = legacy_state_path();
    if let Ok(s) = std::fs::read_to_string(&legacy) {
        let parsed: PersistedState = serde_json::from_str(&s).unwrap_or_else(|_| defaults());
        if let Ok(json) = serde_json::to_vec(&parsed) {
            if crate::secure::write_encrypted(&state_path(), &json).is_ok() {
                let _ = std::fs::remove_file(&legacy);
                crate::elog::info(
                    "service",
                    "state_migrated",
                    "settings moved into the encrypted local store; the old plaintext file was deleted",
                );
            }
        }
        return parsed;
    }

    // Absent on first run — not an error, just "nothing chosen yet".
    defaults()
}

/// Persist the parts of the engine that must outlive the process. Write-then-rename so a crash
/// mid-write cannot leave a half-file that parses as "protection was off".
fn save_state(e: &Engine) {
    let st = PersistedState {
        running: e.running,
        mode: e.mode.clone(),
        dns: e.dns.clone(),
        autostart: e.autostart,
        extra_domains: e.extra_domains.clone(),
    };
    let Ok(json) = serde_json::to_vec(&st) else { return };
    // Encrypted + ACL-restricted + atomic, all inside write_encrypted. If encryption is unavailable
    // this FAILS rather than falling back to a plaintext write — a store that silently degrades is
    // worse than one that reports it could not protect the data.
    if let Err(err) = crate::secure::write_encrypted(&state_path(), &json) {
        crate::elog::warn("service", "state_save", &format!("could not persist state: {err}"));
    }
}

impl Engine {
    fn new() -> Self {
        let persisted = load_state();
        Self {
            running: false,
            state: RunState::Idle,
            strategy: String::new(),
            dns: persisted.dns.clone(),
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
            mode: if persisted.mode.is_empty() { "hafif".into() } else { persisted.mode },
            probe: None,
            tuning: String::new(),
            tuning_step: 0,
            tuning_total: 0,
            extra_domains: persisted.extra_domains,
            autostart: persisted.autostart,
            last_auto_tune: None,
            fallback: "",
            harm_reverts: 0,
            dpi_expected: false,
        }
    }

    /// Resolve the user-facing mode into the concrete (strategy, hostlist) the engine runs.
    ///
    /// This is the single place a mode becomes packets, which is what the old code lacked — the two
    /// modes each hand-assigned four fields at their call site, and Güçlü's assignment
    /// (`hostlist_only = false`) is what silently turned a route-dependent forgery into a
    /// machine-wide catch-all.
    fn plan_for_mode(&self, mode: &str) -> (engine::Strategy, Vec<String>) {
        match mode {
            "guclu" => {
                let mut hosts: Vec<String> = WIDE_HOSTLIST.iter().map(|s| s.to_string()).collect();
                for d in &self.extra_domains {
                    if !hosts.iter().any(|h| h == d) {
                        hosts.push(d.clone());
                    }
                }
                let id = Self::tuned_strategy_id();
                let mut s = engine::strategy_by_id(&id);
                s.hostlist_only = true;
                // "off" won the measurement, which means the bare line already opened everything —
                // and it was MEASURED bare, with no fallback layer (tuner::candidate_strategy). So
                // deploying a catch-all layer here would ship a configuration nobody tested, and
                // would put TCP segmentation on every HTTPS connection on the machine to solve a
                // problem the measurement says does not exist. Deploy exactly what won.
                s.fallback = fallback_for(&id);
                (s, hosts)
            }
            // "hafif" and anything unrecognised: the narrow, provably safe scope. No catch-all layer
            // at all — outside its hostlist, Hafif touches nothing on the machine.
            _ => {
                let mut s = engine::strategy_by_id(HAFIF_STRATEGY);
                s.hostlist_only = true;
                s.fallback = "";
                (s, CORE_HOSTLIST.iter().map(|s| s.to_string()).collect())
            }
        }
    }

    /// Which aggressive chain Güçlü deploys.
    ///
    /// Until this line has actually been measured, the answer is a HARMLESS chain — never a
    /// route-dependent one. That single rule is what makes "Güçlü breaks a working site"
    /// unreachable: an unmeasured aggressive chain is never deployed in the first place.
    fn tuned_strategy_id() -> String {
        match crate::tuner::load() {
            Some(t) if crate::tuner::is_current(&t) && !t.strategy.is_empty() => t.strategy,
            _ => engine::SAFE_FALLBACK.to_string(),
        }
    }

    /// Is per-app packet EXCLUSION worth what it costs?
    ///
    /// Its cost is not theoretical: applying an exclusion set rebuilds the WinDivert capture filter,
    /// which restarts winws and drops every live connection. The user's log shows that happening on
    /// a 60-second cadence for as long as the app was open, and a verification landing 1.1s after
    /// one of those restarts reported 3/9 targets open where the measurement had found 8/9.
    ///
    /// Its benefit was keeping an aggressive desync chain away from apps the user marked "off". But
    /// under the layered design nothing aggressive runs catch-all any more: outside the hostlist the
    /// only thing applied is a chain that provably cannot corrupt a connection
    /// (`Strategy::is_harmless`). So for both shipped modes this mechanism now pays a guaranteed
    /// cost — engine restarts — to prevent a harm that can no longer occur.
    ///
    /// It stays available for the one case where it still earns its keep: a custom profile that
    /// deliberately runs a non-harmless chain catch-all.
    fn exclusion_matters(&self) -> bool {
        let s = self.current_strategy();
        let catch_all = if s.hostlist_only {
            if s.fallback.is_empty() {
                return false; // nothing runs catch-all at all (this is Hafif)
            }
            engine::strategy_by_id(s.fallback)
        } else {
            s
        };
        !catch_all.is_harmless()
    }

    /// The sites this mode is trying to open — what the probe judges, and what the UI lists.
    fn probe_targets(&self) -> Vec<String> {
        match self.mode.as_str() {
            "guclu" => {
                let mut t: Vec<String> = TUNE_TARGETS.iter().map(|s| s.to_string()).collect();
                // A domain the user added by hand is, by definition, one they care about — probe it.
                for d in self.extra_domains.iter().take(5) {
                    if !t.iter().any(|h| h == d) {
                        t.push(d.clone());
                    }
                }
                t
            }
            _ => crate::verify::PROBE_TARGETS.iter().map(|s| s.to_string()).collect(),
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
        // The catch-all fallback layer is part of the RUNTIME plan, not of the strategy's catalog
        // entry, so it has to be re-applied here.
        //
        // Without this the watchdog silently changed the configuration: it respawns a dead engine
        // from `current_strategy()`, which reads the catalog — and a catalog entry like `safe-split`
        // carries `fallback: ""`. So a Güçlü session whose measured winner happened to be a
        // harmless chain would come back from a respawn with NO catch-all layer, i.e. quietly
        // narrower than the mode the user selected, with nothing reporting the change.
        s.fallback = self.fallback;
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
        let (targets_ok, targets_total, harm, sites) = match &self.probe {
            Some(p) => {
                let (ok, total) = p.target_score();
                let mut sites: Vec<ipc::SiteStatus> = p
                    .targets
                    .iter()
                    .map(|r| ipc::SiteStatus {
                        host: r.host.clone(),
                        ok: r.ok,
                        ms: r.ms,
                        control: false,
                        reason: r.reason.clone(),
                    })
                    .collect();
                sites.extend(p.control.iter().map(|r| ipc::SiteStatus {
                    host: r.host.clone(),
                    ok: r.ok,
                    ms: r.ms,
                    control: true,
                    reason: r.reason.clone(),
                }));
                (ok as u32, total as u32, p.harm, sites)
            }
            None => (0, 0, false, Vec::new()),
        };
        EngineStatus {
            // MEASURED, not remembered (rule 10): when a DPI child is supposed to exist, ask
            // whether it actually does rather than trusting a bool set minutes ago. `running` used
            // to be a plain field, so a winws that died kept the UI showing "on" until a watchdog
            // tick noticed. `dpi_expected` is false for DNS-only protection, where there is
            // legitimately no child process to find.
            running: self.running
                && (!self.dpi_expected || self.dpi.is_running() || self.warp.is_running()),
            strategy: self.strategy.clone(),
            dns: self.dns.clone(),
            engine: self.engine_id.clone(),
            state: self.state.as_str().to_string(),
            verify: self.verify.as_str().to_string(),
            verify_reason: self.verify.reason(),
            mode: self.mode.clone(),
            harm,
            tuning: self.tuning.clone(),
            tuning_step: self.tuning_step,
            tuning_total: self.tuning_total,
            tuned_strategy: Self::tuned_strategy_id(),
            targets_ok,
            targets_total,
            sites,
            problems: crate::elog::problems(8),
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
    /// Gap between retries.
    ///
    /// Was 3s. Combined with a 4s probe budget that made the worst case 19 SECONDS before the user
    /// saw any verdict at all (measured in their log: start 17:47:51, first conclusion 17:48:10).
    /// The retry exists to cover the moment where winws has spawned but WinDivert has not attached
    /// yet — that window is well under a second, so 1s is ample and 3s was just waiting.
    const VERIFY_RETRY_GAP: Duration = Duration::from_secs(1);

    e.verify = VerifyState::Verifying;
    e.verify_gen += 1;
    let my_gen = e.verify_gen;
    let targets = e.probe_targets();
    let engine = Arc::clone(engine);
    std::thread::spawn(move || {
        // Still stale-checked between attempts: a probe from a session the user already stopped or
        // switched away from must never write its result over a newer one.
        let fresh = |gen: u64| -> bool {
            let e = engine.lock().unwrap_or_else(|p| p.into_inner());
            e.verify_gen == gen
        };

        // Attempt 1 uses the FAST budget so a healthy line produces a verdict in ~1.5s instead of
        // ~4s, and publish its per-site results straight away: the user should be able to see which
        // sites opened while the engine is still deciding, not stare at a spinner. Only the FINAL
        // attempt uses the conclusive budget, so "broken" is never declared on an impatient timeout.
        let mut report = crate::verify::probe_protection_within(&targets, crate::verify::IO_TIMEOUT_FAST);
        let mut attempt = 1;
        {
            let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
            if e.verify_gen != my_gen {
                return;
            }
            if report.ok && !report.harm {
                // Fast success: nothing to retry, the UI can say "protected" now.
                e.probe = Some(report.clone());
                e.verify = VerifyState::Verified;
            } else {
                // Not conclusive yet — show the partial picture but keep the state honest.
                e.probe = Some(report.clone());
            }
        }

        // HARM short-circuits the retry loop. Retrying is for a bypass that has not warmed up yet;
        // damage does not warm up, and every extra second spent retrying is a second the user's
        // internet stays broken.
        while !report.ok && !report.harm && attempt < VERIFY_ATTEMPTS {
            if !fresh(my_gen) {
                return;
            }
            std::thread::sleep(VERIFY_RETRY_GAP);
            if !fresh(my_gen) {
                return;
            }
            attempt += 1;
            let budget = if attempt >= VERIFY_ATTEMPTS {
                crate::verify::IO_TIMEOUT // last word: give a slow line every chance
            } else {
                crate::verify::IO_TIMEOUT_FAST
            };
            report = crate::verify::probe_protection_within(&targets, budget);
        }

        let (ok, harm, reason) = (report.ok, report.harm, report.reason.clone());
        let (ok_n, total_n) = report.target_score();

        {
            let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
            if e.verify_gen != my_gen {
                return; // stale — discard
            }
            e.probe = Some(report);
            e.verify = if ok && !harm {
                VerifyState::Verified
            } else {
                VerifyState::Broken(reason.clone())
            };
            audit(&format!(
                "verify ({attempt}/{VERIFY_ATTEMPTS}): {ok_n}/{total_n} targets open, harm={harm}"
            ));
        }

        if harm {
            // ---- DO NO HARM, but CONFIRM first ---------------------------------------------
            //
            // Control sites failing means the deployed chain may be corrupting ordinary traffic,
            // and the reaction (throw the tuning away, redeploy, restart) is expensive and visible.
            // So it must not fire on a blip.
            //
            // It nearly did: on a slow line the user's measurement recorded `safe-fake: broke 3
            // control` — all three, at once — while neighbouring candidates on the same line showed
            // handshakes taking 1.3-2.5s. That is a congested line briefly exceeding the fast probe
            // budget, not a chain that cannot corrupt anything suddenly corrupting everything.
            //
            // Re-probe the control set alone, with the CONCLUSIVE budget. Cheap (3 hosts, parallel)
            // and it separates "the engine is breaking things" from "the line hiccuped".
            std::thread::sleep(Duration::from_millis(800));
            if !fresh(my_gen) {
                return;
            }
            let control: Vec<String> = crate::verify::CONTROL_TARGETS.iter().map(|s| s.to_string()).collect();
            let recheck = crate::verify::probe_hosts_within(&control, crate::verify::IO_TIMEOUT);
            let still_failing = recheck.iter().filter(|r| !r.ok).count();
            if still_failing * 2 <= recheck.len() {
                crate::elog::warn(
                    "verify",
                    "harm_not_confirmed",
                    &format!(
                        "control sites failed once ({reason}) but recovered on recheck — treating it \
                         as a transient network problem, not engine damage"
                    ),
                );
                let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
                if e.verify_gen == my_gen {
                    // Not harm; the honest state is "applied, not verified" rather than "broken".
                    e.verify = VerifyState::Unverified;
                }
                drop(e);
                maybe_auto_tune(&engine);
                return;
            }

            crate::elog::error(
                "verify",
                "harm_detected",
                &format!(
                    "{reason} — confirmed on recheck ({still_failing}/{} control sites still failing) \
                     — reverting to a chain that cannot corrupt traffic",
                    recheck.len()
                ),
            );
            revert_to_harmless(&engine);
            return;
        }

        if !ok {
            // Targets did not open, but nothing broke. That is the case measurement exists for:
            // this line needs a different chain than the one currently deployed.
            maybe_auto_tune(&engine);
        }
    });
}

/// The line changed under a tuned aggressive chain (or the tuning was wrong): throw the tuning
/// away, fall back to a provably harmless configuration, and restart.
///
/// Discarding the tuning file matters — otherwise the next Start would redeploy exactly the chain
/// that just broke the user's internet.
fn revert_to_harmless(engine: &Arc<Mutex<Engine>>) {
    /// Reverting is only a fix when the deployed chain was the cause. If harm persists AFTER we
    /// fell back to a chain that provably cannot corrupt traffic, the cause is elsewhere (the line
    /// itself is down, the control endpoints are unreachable) — and reverting again would redeploy
    /// the identical configuration, re-probe, see harm again, and restart the engine every few
    /// seconds forever. That churn kills every live connection on the machine, which is worse than
    /// the condition it is trying to fix.
    const MAX_REVERTS: u8 = 2;

    let _ = std::fs::remove_file(crate::tuner::path());
    let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
    if !e.running {
        return;
    }
    e.harm_reverts = e.harm_reverts.saturating_add(1);
    if e.harm_reverts > MAX_REVERTS {
        e.dpi.stop();
        e.running = false;
        e.dpi_expected = false;
        e.state = RunState::Error;
        e.tuning = "gave_up".into();
        crate::elog::error(
            "verify",
            "harm_persists",
            "ordinary sites are still failing with a harmless configuration deployed — this is not \
             something the engine is causing. Protection stopped so it cannot be blamed for, or \
             add to, whatever is actually wrong with the connection.",
        );
        save_state(&e);
        return;
    }
    e.tuning = "gave_up".into();
    match load_mode(&mut e) {
        Ok(()) => {
            e.state = RunState::Active;
            audit("harm detected — reloaded with the harmless configuration");
            spawn_verify(engine, &mut e);
        }
        Err(m) => {
            // Could not even bring the safe configuration up: stop entirely rather than leave the
            // damaging one running. Off is a working internet; this is not a close call.
            e.dpi.stop();
            e.running = false;
            e.dpi_expected = false;
            e.state = RunState::Error;
            crate::elog::error("verify", "harm_revert_failed", &format!("stopped protection: {m}"));
        }
    }
}

/// Targets are blocked and nothing is broken → measure this line and deploy what wins.
///
/// Rate-limited hard (10 minutes): a genuinely unbeatable line must not turn into a loop that
/// restarts the engine every few seconds and kills every connection on the machine. That churn is
/// the "works, then stops, then works" pattern this codebase has already been bitten by once.
fn maybe_auto_tune(engine: &Arc<Mutex<Engine>>) {
    const AUTO_TUNE_MIN_GAP: Duration = Duration::from_secs(600);

    let should = {
        let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
        let fresh_enough = e.last_auto_tune.map(|t| t.elapsed() >= AUTO_TUNE_MIN_GAP).unwrap_or(true);
        // Only Güçlü auto-tunes. Hafif's promise is "narrow and safe"; silently escalating it to a
        // measured aggressive chain would be the app deciding to widen its own blast radius.
        let eligible = e.running && e.mode == "guclu" && e.tuning != "tuning" && fresh_enough;
        if eligible {
            e.last_auto_tune = Some(Instant::now());
            e.tuning = "tuning".into();
        }
        eligible
    };
    if !should {
        return;
    }

    crate::elog::info("tuner", "auto", "targets are blocked — measuring this line automatically");
    let engine = Arc::clone(engine);
    std::thread::spawn(move || {
        run_tuning(&engine, Vec::new(), |_| {});
    });
}


// ============================================================================================
// Protection modes.
//
// REWRITTEN 2026-08-16 after a reported failure that inverted the two modes: on "Güçlü Koruma" an
// ordinary HTTPS site would not open at all, and switching DOWN to "Hafif" opened it instantly.
//
// The cause was structural, not a bad constant. Güçlü ran one route-dependent forgery
// (`turkcell-hotspot`: fake + ttl=1 + autottl=3, no fooling) CATCH-ALL over every TLS/443 flow on
// the machine. That chain only works where the DPI sits at the assumed hop distance; everywhere
// else the forged ClientHello outlives the DPI, reaches the real server, and the server drops the
// connection. Hafif "worked" purely because its hostlist did not contain that site, so nothing
// touched it. Strength was buying damage, not coverage.
//
// Both modes are now LAYERED and neither is ever catch-all-aggressive:
//
//   layer 1  aggressive chain, gated to a hostlist of domains known to need it
//   layer 2  a chain that provably cannot corrupt a connection (Strategy::is_harmless), catch-all
//
// and layer 1's chain is chosen by measurement on the actual line (`tuner`), not hardcoded.
// ============================================================================================

/// Which catch-all layer (if any) ships alongside a winning chain.
///
/// Single source of truth for the "deploy exactly what was measured" rule, so the deployment side
/// and the test that guards it cannot drift apart. `tuner::candidate_strategy` applies the same
/// rule when it MEASURES a candidate.
fn fallback_for(strategy_id: &str) -> &'static str {
    if strategy_id == "off" {
        ""
    } else {
        engine::SAFE_FALLBACK
    }
}

/// Hafif Koruma — the narrow, guaranteed-safe scope: Discord + Roblox, and nothing else on the
/// machine is touched at all (no catch-all layer). DEFAULT MODE on first run.
const CORE_HOSTLIST: &[&str] = &[
    "discord.com", "discordapp.com", "discord.gg", "discordapp.net", "discord.media",
    "gateway.discord.gg", "cdn.discordapp.com",
    "roblox.com", "www.roblox.com", "rbxcdn.com",
];

/// Güçlü Koruma — the wide scope: everything Hafif covers, plus the domains that are actually
/// blocked on Turkish consumer lines and that users install this app to reach. These get the
/// measured aggressive chain; every OTHER site on the machine gets the harmless catch-all layer,
/// which is what makes "wide" safe to enable.
const WIDE_HOSTLIST: &[&str] = &[
    // Discord + Roblox (same as CORE)
    "discord.com", "discordapp.com", "discord.gg", "discordapp.net", "discord.media",
    "gateway.discord.gg", "cdn.discordapp.com",
    "roblox.com", "www.roblox.com", "rbxcdn.com",
    // Video / streaming that gets throttled or SNI-filtered
    "youtube.com", "www.youtube.com", "googlevideo.com", "ytimg.com",
    // Commonly DNS-sinkholed or SNI-blocked on TR lines
    "pornhub.com", "www.pornhub.com", "phncdn.com",
    "brazzers.com", "xvideos.com", "xhamster.com", "redtube.com", "youporn.com",
    "onlyfans.com",
];

/// Default targets a tuning run is judged against: a spread across the blocked set, not one family.
/// Discord alone was the old probe set, and that is exactly how Güçlü could report "verified" while
/// every non-Discord site was dead.
const TUNE_TARGETS: &[&str] =
    &["discord.com", "cdn.discordapp.com", "www.roblox.com", "www.pornhub.com", "www.youtube.com"];

/// The chain Hafif uses inside its hostlist. Harmless by construction; Hafif's promise is "never
/// makes anything worse", so it does not get the aggressive/measured treatment.
const HAFIF_STRATEGY: &str = "safe-fake";

/// Delete every file this app keeps on disk, and REPORT what is actually gone.
///
/// Returns both lists on purpose. "Deleted" with no verification is the same class of claim as
/// "protected" with no probe — the rest of this codebase refuses to make it, and a privacy control
/// is the last place to start.
fn wipe_local_data() -> serde_json::Value {
    let data = ipc::data_dir();
    let logs = crate::sys::log_dir();
    let mut targets: Vec<std::path::PathBuf> = vec![
        state_path(),
        legacy_state_path(),
        crate::tuner::path(),
        data.join("hostlist.txt"),
        data.join("winws_master.filter"),
        data.join("gd-blacklist.txt"),
    ];
    // Every log file, plus any staged bundle.
    if let Ok(rd) = std::fs::read_dir(&logs) {
        for entry in rd.filter_map(|e| e.ok()) {
            targets.push(entry.path());
        }
    }

    let mut removed: Vec<String> = Vec::new();
    let mut remaining: Vec<String> = Vec::new();
    for t in targets {
        if !t.exists() {
            continue;
        }
        let name = t.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let res = if t.is_dir() { std::fs::remove_dir_all(&t) } else { std::fs::remove_file(&t) };
        // Check the FILESYSTEM, not the return value: a delete can report success on a path that a
        // running handle keeps alive until close.
        if res.is_ok() && !t.exists() {
            removed.push(name);
        } else {
            remaining.push(name);
        }
    }
    serde_json::json!({ "removed": removed, "remaining": remaining })
}

/// Bring the DPI engine in line with the active mode. Caller holds the lock.
///
/// The one place a mode turns into a running process. Every earlier caller open-coded four field
/// assignments plus a start, which is how Güçlü ended up deploying a catch-all forgery: one of
/// those four assignments (`hostlist_only = false`) meant something very different from what the
/// mode's description claimed.
fn load_mode(e: &mut Engine) -> Result<(), String> {
    let mode = e.mode.clone();
    let (strat, hosts) = e.plan_for_mode(&mode);
    e.strategy = strat.id.to_string();
    e.hostlist = hosts.clone();
    e.hostlist_only = true;
    e.fallback = strat.fallback; // so a watchdog respawn rebuilds the SAME layered plan
    e.repeats_override = None; // modes carry no sweep override; a stale one must not leak in

    let needs_child = engine::has_web_stage(&strat, &hosts);
    // ALWAYS stop first. `start()` is idempotent by contract, so against a live child it returns
    // Ok without applying the new argv — which is exactly how a hostlist/strategy change could be
    // accepted, reported as applied, and silently never take effect.
    e.dpi.stop();
    e.dpi_expected = needs_child;
    if !needs_child {
        crate::elog::info(
            "service",
            "dns_only",
            "measurement says this line needs no packet-level work — running DNS-only and touching \
             no traffic",
        );
        return Ok(());
    }
    e.dpi.start(&strat, &hosts)
}

/// Measure the ladder on this line, persist the winner, and redeploy the mode with it.
///
/// Runs on a TEMPORARY engine instance with the main one stopped, and — critically — WITHOUT
/// holding the engine mutex. Tuning takes tens of seconds; holding the lock across it would block
/// every `dispatch()` and reproduce the "service did not answer in 45s" freeze this codebase has
/// already been bitten by twice (warp install, watchdog re-entry).
fn run_tuning<F: FnMut(&crate::tuner::Row)>(
    engine: &Arc<Mutex<Engine>>,
    targets: Vec<String>,
    on_row: F,
) -> crate::tuner::Tuning {
    let (was_running, targets) = {
        let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
        let wr = e.running;
        let t = if targets.is_empty() { e.probe_targets() } else { targets };
        e.tuning = "tuning".into();
        e.state = RunState::Paused; // watchdog leaves a Paused engine alone
        e.dpi.stop();
        e.running = false;
        e.dpi_expected = false;
        (wr, t)
    };

    // Separate engine instance: two winws processes would fight over the single global WinDivert
    // driver, so the main one is stopped above and this one owns the driver for the run.
    // Publish progress as each candidate finishes. Without this the UI has a 15-20 second window
    // where nothing changes on screen, which reads as a hang — and the whole point of measuring
    // faster is wasted if the user cannot tell that anything is happening.
    let total = engine::tuner_ladder_first_pass().len() as u32;
    {
        let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
        e.tuning_step = 0;
        e.tuning_total = total;
    }
    let progress_engine = Arc::clone(engine);
    let mut step: u32 = 0;
    let mut on_row = on_row;
    let mut temp = engine::make_engine("zapret");
    let result = crate::tuner::run_two_pass(temp.as_mut(), &targets, |row| {
        step += 1;
        let mut e = progress_engine.lock().unwrap_or_else(|p| p.into_inner());
        e.tuning_step = step;
        // The second pass extends past the first pass's length; report the real total rather than
        // letting the counter run past it.
        if step > e.tuning_total {
            e.tuning_total = step;
        }
        drop(e);
        on_row(row);
    });
    temp.stop();
    drop(temp);

    if let Err(m) = crate::tuner::save(&result) {
        // Not fatal — the winner still gets deployed for this session; it just will not survive a
        // restart. Saying so beats a silent re-measure on every boot.
        crate::elog::warn("tuner", "save_failed", &format!("tuning result not persisted: {m}"));
    }

    let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
    e.tuning = if result.gave_up { "gave_up".into() } else { "tuned".into() };
    e.tuning_step = 0;
    e.tuning_total = 0;
    if was_running {
        match load_mode(&mut e) {
            Ok(()) => {
                e.running = true;
                e.state = RunState::Active;
                spawn_verify(engine, &mut e);
            }
            Err(m) => {
                e.running = false;
                e.dpi_expected = false;
                e.state = RunState::Error;
                e.reset_verify();
                crate::elog::error("tuner", "redeploy_failed", &m);
            }
        }
    } else {
        e.state = RunState::Idle;
    }
    save_state(&e);
    result
}

fn dispatch(engine: &Arc<Mutex<Engine>>, cmd: Command) -> Response {
    if let Err(m) = ipc::validate(&cmd) {
        audit(&format!("REJECT {cmd:?}: {m}"));
        return Response::Error { message: m };
    }
    let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());

    // A line measurement owns the WinDivert driver for its duration (it drives a temporary engine,
    // one candidate at a time). Anything that would start a SECOND winws while it runs makes the two
    // fight over the single global driver — the loser exits immediately, and whichever candidate was
    // being measured at that moment gets scored against a dead engine.
    //
    // Seen in the user's log: a `Command::Start` arrived mid-run, and the two candidates either side
    // of it recorded impossibly fast probes (256-278ms against a 4s norm) with results that did not
    // match their neighbours. Those rows were measuring nothing. Refusing the command outright is
    // better than silently corrupting a measurement the user is waiting on.
    if matches!(cmd, Command::Start | Command::SetProtectionMode { .. } | Command::ApplyProfile { .. })
        && e.tuning == "tuning"
    {
        return Response::Error {
            message: "a connection measurement is running — try again when it finishes".into(),
        };
    }

    match cmd {
        Command::Start => {
            e.state = RunState::Applying;

            // Secure DNS is applied HERE, not merely recorded.
            //
            // FIXED 2026-08-15 (live test): this used to just set `e.dns = "cloudflare"` — a field,
            // never applied — so `status` reported dns=cloudflare while the adapters still used the
            // ISP resolver. On the measured line that resolver answers every Discord domain with
            // 195.175.254.2 (a sinkhole), and DPI desync CANNOT fix a wrong destination IP: the
            // connection times out at TCP, before any handshake exists to rewrite.
            //
            // SPEED (2026-08-16): DNS and the engine now come up CONCURRENTLY. They are independent
            // — one rewrites resolver settings, the other attaches a packet filter — but were run
            // strictly one after the other, so every Start paid DNS latency plus engine latency in
            // series. The lock is released for both: `run_dns` shells out across every adapter, and
            // holding the mutex across that is the freeze already fixed once in warp.rs.
            let dns_profile = if e.dns.is_empty() { "cloudflare".to_string() } else { e.dns.clone() };
            drop(e);

            let dns_handle = {
                let p = dns_profile.clone();
                std::thread::spawn(move || crate::dns::run_dns(&p))
            };

            let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
            let engine_result = load_mode(&mut e);
            drop(e);

            // Join DNS before verifying: the probe resolves names, so a probe that races the DNS
            // switch would be measuring the old resolver and reporting nonsense.
            let dns_applied = dns_handle.join().unwrap_or_else(|_| Err("DNS thread panicked".into()));

            let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
            match &dns_applied {
                Ok(()) => {
                    e.dns = dns_profile;
                    audit("start: secure DNS applied");
                }
                Err(m) => {
                    // Do NOT claim a provider we failed to set. Protection still starts (desync
                    // helps domains that aren't DNS-poisoned), but status must not overstate it.
                    e.dns = "auto".into();
                    crate::elog::warn(
                        "dns",
                        "apply_failed",
                        &format!("secure DNS could not be applied, using the system resolver: {m}"),
                    );
                }
            }

            match engine_result {
                Ok(()) => {
                    e.running = true;
                    e.state = RunState::Active;
                    let ll = e.limit_list();
                    e.dpi.set_limits(&ll);
                    e.sync_warp();
                    crate::elog::info(
                        "service",
                        "start",
                        &format!(
                            "protection on — mode={} strategy={} scope={} domains",
                            e.mode,
                            e.strategy,
                            e.hostlist.len()
                        ),
                    );
                    spawn_verify(engine, &mut e);
                    save_state(&e);

                    // MEASURE NOW, don't wait for failure.
                    //
                    // The old sequence was: start → let verification fail (three probes, ~9s) →
                    // only then begin measuring. Those 9 seconds bought nothing: with no usable
                    // stored measurement we already know we are running a placeholder chain, so
                    // waiting to be told is pure latency in front of the user's first impression.
                    //
                    // Only when there is nothing current to deploy. With a stored, in-date
                    // measurement for this network we skip straight to it and never measure at all
                    // — which is what makes every start after the first one take seconds.
                    let needs_measurement = e.mode == "guclu"
                        && !crate::tuner::load().map(|t| crate::tuner::is_current(&t)).unwrap_or(false);
                    if needs_measurement {
                        e.tuning = "tuning".into();
                        let engine2 = Arc::clone(engine);
                        std::thread::spawn(move || {
                            crate::elog::info(
                                "tuner",
                                "on_start",
                                "no current measurement for this network — measuring immediately \
                                 instead of waiting for verification to fail",
                            );
                            run_tuning(&engine2, Vec::new(), |_| {});
                        });
                    }
                    Response::Status(e.status())
                }
                Err(m) => {
                    e.running = false;
                    e.dpi_expected = false;
                    e.state = RunState::Error;
                    e.reset_verify();
                    crate::elog::error("service", "start_failed", &m);
                    Response::Error { message: m }
                }
            }
        }
        Command::Stop => {
            e.warp.stop();
            e.dpi.stop();
            e.running = false;
            e.dpi_expected = false;
            e.state = RunState::Idle;
            e.probe = None;
            e.harm_reverts = 0; // an explicit Stop/Start is a fresh session, not a continued loop
            e.reset_verify();
            crate::elog::info("service", "stop", "protection off (user request)");
            save_state(&e);
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
            // SUPERSEDED by SetExtraDomains. Kept so an older UI build still does something sane
            // instead of erroring, and routed through the same path so it can no longer hit the
            // old bug: this used to call `dpi.start()` against a LIVE child, which is idempotent by
            // contract — so the new list was accepted, reported as applied, and never took effect.
            audit(&format!("set_hostlist ({} domains) → treated as extra domains", domains.len()));
            e.extra_domains = domains;
            let running = e.running && e.mode == "guclu";
            save_state(&e);
            if running {
                match load_mode(&mut e) {
                    Ok(()) => {
                        spawn_verify(engine, &mut e);
                        Response::Status(e.status())
                    }
                    Err(m) => Response::Error { message: m },
                }
            } else {
                Response::Ok
            }
        }
        Command::SetProtectionMode { mode } => {
            if !matches!(mode.as_str(), "hafif" | "guclu") {
                return Response::Error { message: format!("unknown protection mode: {mode}") };
            }
            audit(&format!("set_protection_mode {mode}"));
            let prev_mode = e.mode.clone();
            e.mode = mode.clone();

            if !e.running {
                // Nothing to verify yet — the mode is recorded and takes effect on the next Start.
                save_state(&e);
                return Response::Status(e.status());
            }

            // Live switch: reload so winws actually picks up the new scope, then re-probe. The
            // previous mode's Verified result says nothing about this one.
            match load_mode(&mut e) {
                Ok(()) => {
                    e.running = true;
                    e.state = RunState::Active;
                    e.probe = None; // the old per-site list describes the old mode; don't show it
                    spawn_verify(engine, &mut e);
                    save_state(&e);
                    Response::Status(e.status())
                }
                Err(m) => {
                    // Roll back so status() keeps describing what is actually loaded, not what we
                    // failed to switch to.
                    e.mode = prev_mode;
                    let _ = load_mode(&mut e);
                    e.running = false;
                    e.dpi_expected = false;
                    e.state = RunState::Error;
                    e.reset_verify();
                    crate::elog::error("service", "mode_switch_failed", &m);
                    Response::Error { message: m }
                }
            }
        }
        Command::SetAutoStart { enable } => {
            e.autostart = enable;
            save_state(&e);
            crate::elog::info(
                "service",
                "autostart",
                if enable {
                    "protection will be restored when Windows starts"
                } else {
                    "protection will stay off until started by hand"
                },
            );
            Response::Status(e.status())
        }
        Command::SetExtraDomains { domains } => {
            e.extra_domains = domains;
            let running = e.running;
            save_state(&e);
            // Güçlü's hostlist is built from the shipped set plus these, so a change has to be
            // pushed into the engine. This is the bug the old SetHostlist had: it called start()
            // on a live child, which is idempotent, so the new list was accepted and never applied.
            if running && e.mode == "guclu" {
                match load_mode(&mut e) {
                    Ok(()) => {
                        spawn_verify(engine, &mut e);
                        Response::Status(e.status())
                    }
                    Err(m) => Response::Error { message: m },
                }
            } else {
                Response::Status(e.status())
            }
        }
        Command::WipeLocalData => {
            // Stop first: the engine holds hostlist.txt open and would rewrite it a moment later.
            // Deleting under a running engine is how a "delete everything" leaves everything.
            e.dpi.stop();
            e.running = false;
            e.dpi_expected = false;
            e.state = RunState::Idle;
            e.probe = None;
            e.extra_domains.clear();
            e.tuning = String::new();
            e.reset_verify();
            drop(e);

            let report = wipe_local_data();
            crate::elog::info("service", "wiped", "local data deleted at the user's request");
            match serde_json::to_string(&report) {
                Ok(j) => Response::Data(j),
                Err(err) => Response::Error { message: err.to_string() },
            }
        }
        Command::Events { since } => {
            let (watermark, events) = crate::elog::since(since);
            match serde_json::to_string(&serde_json::json!({ "watermark": watermark, "events": events })) {
                Ok(j) => Response::Data(j),
                Err(err) => Response::Error { message: err.to_string() },
            }
        }
        Command::Tune { targets } => {
            drop(e); // measurement takes tens of seconds — never under the lock
            let result = run_tuning(engine, targets, |_| {});
            match serde_json::to_string(&result) {
                Ok(j) => Response::Data(j),
                Err(err) => Response::Error { message: err.to_string() },
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

/// Block until the machine actually has working connectivity, or `budget` expires.
///
/// At boot the service is started by the SCM long before the network stack is usable — adapters
/// are still initialising, there may be no default route, DHCP may not have answered. Applying DNS
/// and attaching a packet filter into that window fails in ways indistinguishable from an engine
/// fault, which is exactly the kind of "it just doesn't work after restart" report that is
/// impossible to diagnose afterwards.
///
/// Returns whether connectivity was observed. A `false` is not a reason to refuse to start: the
/// user may genuinely be offline, and verification will then report that honestly rather than the
/// service silently deciding not to protect them.
fn wait_for_network(budget: Duration) -> bool {
    use std::net::TcpStream;
    let deadline = Instant::now() + budget;
    let mut delay = Duration::from_millis(250);
    loop {
        if let Ok(addr) = "1.1.1.1:443".parse() {
            if TcpStream::connect_timeout(&addr, Duration::from_millis(1500)).is_ok() {
                return true;
            }
        }
        if Instant::now() >= deadline {
            crate::elog::warn(
                "service",
                "network_wait_timeout",
                "no connectivity within the boot wait window — starting anyway and letting \
                 verification report the real state",
            );
            return false;
        }
        std::thread::sleep(delay);
        // Back off to 2s: a machine that is slow to get online should not be polled 240 times.
        delay = (delay * 2).min(Duration::from_secs(2));
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
    // Enforce the retention limit at startup — actually DELETE what aged out, do not merely stop
    // trusting it. A limit that leaves the data on disk and looks away is not a retention limit,
    // and this call was written and then not wired up on the first pass.
    crate::tuner::purge_if_expired();
    // Recover any half-applied changes from a previous crash into the global rollback log (item 7.3).
    crate::rollback::load_global();
    // Profilleri tohumla (ilk çalıştırma) — UI ListProfiles çağırınca hazır olsun.
    crate::profile::seed_defaults();
    audit("listening");

    // ---- Boot restore (2026-08-16) -----------------------------------------------------------
    //
    // History: protection once auto-started here unconditionally ("service running = protected"),
    // with no persisted preference and no way for `off` to survive a restart. That was removed
    // (P0-e) because unconditional is the wrong answer — but removing it left the OTHER wrong
    // answer: after a reboot the machine always came up unprotected, whatever the user had chosen,
    // with nothing in the UI explaining why. That is what "I restarted my PC and evorift did not
    // start" was.
    //
    // The missing piece was never the trigger, it was the SETTING. `PersistedState` now records
    // both what was running and whether the user wants it restored, so this is neither a surprise
    // nor a silent refusal — it does what the user last asked for, and the choice is a toggle.
    {
        let persisted = load_state();
        if persisted.autostart && persisted.running {
            let engine = Arc::clone(&engine);
            // On a background thread: the pipe listener below must be accepting connections before
            // protection finishes coming up, or the UI's first status call at login times out
            // against a service that is busy applying DNS.
            std::thread::spawn(move || {
                crate::elog::info(
                    "service",
                    "boot_restore",
                    &format!(
                        "restoring protection after boot (mode={}, as last chosen)",
                        if persisted.mode.is_empty() { "hafif" } else { &persisted.mode }
                    ),
                );
                // At boot the network stack is frequently not ready yet: adapters still coming up,
                // no default route, DNS unset. Applying into that fails in ways that look like an
                // engine fault. Wait for actual connectivity, bounded — if the machine is genuinely
                // offline we start anyway and let verification report the truth.
                wait_for_network(Duration::from_secs(60));

                // The wait above can last a minute, and the UI is reachable throughout it. If the
                // user opened the app and turned protection OFF during that window, restoring it
                // now would override a decision they just made — the same surprise the earlier
                // unconditional boot-auto-protect caused. `Stop` persists `running:false`, so
                // re-reading the file is enough to tell.
                if !load_state().running {
                    crate::elog::info(
                        "service",
                        "boot_restore_cancelled",
                        "protection was switched off while waiting for the network — not restoring",
                    );
                    return;
                }
                // Same check against live state, for a Stop that arrived after the file read.
                if engine.lock().unwrap_or_else(|p| p.into_inner()).state != RunState::Idle {
                    return;
                }

                match dispatch(&engine, Command::Start) {
                    Response::Status(_) => {}
                    Response::Error { message } => {
                        crate::elog::error("service", "boot_restore_failed", &message)
                    }
                    _ => {}
                }
            });
        } else {
            audit(&format!(
                "listening idle (autostart={}, last state running={})",
                persisted.autostart, persisted.running
            ));
        }
    }

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
        let mut last_dns_check: Option<std::time::Instant> = None;
        // Say the "per-app off is not enforced" thing ONCE, not every 5 seconds.
        let mut warned_no_exclusion = false;
        loop {
            std::thread::sleep(Duration::from_secs(5));
            let (running, paused, off_paths, excl_matters) = {
                let e = engine.lock().unwrap_or_else(|p| p.into_inner());
                (e.running, e.state == RunState::Paused, e.off_app_paths(), e.exclusion_matters())
            };
            if paused {
                continue; // a measurement is running → do not interfere
            }
            // Scanning at all is gated on the exclusion being worth its cost — see
            // `Engine::exclusion_matters`. Under both shipped modes this is false, so the engine is
            // never restarted for port churn, and `pid_scan` (a full socket-table enumeration) is
            // not run every 5 seconds either.
            let excl = if running && excl_matters && !off_paths.is_empty() {
                crate::pid_scan::scan(&off_paths)
            } else {
                crate::pid_scan::ExclusionPorts::default()
            };
            if running && !excl_matters && !off_paths.is_empty() && !warned_no_exclusion {
                warned_no_exclusion = true;
                crate::elog::info(
                    "watchdog",
                    "exclusion_not_needed",
                    "per-app 'off' is not enforced at the packet layer: nothing that could corrupt a \
                     connection runs outside the hostlist, so excluding those apps would only buy \
                     engine restarts",
                );
            }
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
                    // DNS-only protection legitimately has no child process. Respawning one here
                    // would start a winws the plan never asked for, every 5 seconds, forever.
                    if e.dpi_expected {
                        let strat = e.current_strategy();
                        let hl = e.hostlist.clone();
                        // NEVER discard the respawn Result — if the child is gone and cannot be
                        // restarted (bundle deleted/locked), running:true would keep lying.
                        if let Err(m) = e.dpi.start(&strat, &hl) {
                            e.running = false;
                            e.dpi_expected = false;
                            e.state = RunState::Error;
                            e.reset_verify();
                            crate::elog::error(
                                "watchdog",
                                "respawn_failed",
                                &format!("the engine process is gone and could not be restarted: {m}"),
                            );
                        }
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
            // Rate-limit the CHECK, not just the repair.
            //
            // FIXED 2026-08-16, from the user's audit log: `dns verify: güvenli (Cloudflare)` on
            // EVERY 5-second tick, indefinitely. `last_dns_heal` was only stamped when drift was
            // actually found, so on a machine whose DNS is fine it stayed `None`, `may_heal` stayed
            // true, and the check ran forever at tick rate. Before the native resolver read landed
            // that meant spawning PowerShell every 5 seconds for the entire time protection was on
            // and unverified — which is both the CPU cost and the log noise visible in that file.
            let may_check = last_dns_check
                .map(|t: std::time::Instant| t.elapsed() >= Duration::from_secs(60))
                .unwrap_or(true);
            let may_heal = last_dns_heal
                .map(|t: std::time::Instant| t.elapsed() >= Duration::from_secs(60))
                .unwrap_or(true);
            if needs_dns_check && may_check && may_heal {
                last_dns_check = Some(std::time::Instant::now());
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

    /// THE regression guard for the 2026-08-16 failure: "Güçlü Koruma" made a working site
    /// unreachable, and dropping DOWN to "Hafif" fixed it.
    ///
    /// The mechanism was that Güçlü ran a route-dependent forgery (`fake` + `ttl` with no fooling)
    /// with `hostlist_only = false`, i.e. over EVERY TLS/443 flow on the machine. Where the DPI is
    /// not at the assumed hop distance the forged ClientHello reaches the real server and the
    /// server drops the connection.
    ///
    /// So the invariant is not about which preset wins a sweep. It is: whatever runs catch-all
    /// must be incapable of corrupting a connection. Asserted through the real
    /// `plan_for_mode` → `build_args` path, so any future edit that widens a chain's scope without
    /// making it harmless fails here rather than on a user's machine.
    #[cfg(windows)]
    #[test]
    fn no_mode_may_run_a_harmful_chain_catch_all() {
        let winws = crate::engine::make_engine("zapret");

        for mode in ["hafif", "guclu"] {
            let mut e = Engine::new();
            e.mode = mode.into();
            let (strat, hosts) = e.plan_for_mode(mode);

            // 1. The aggressive layer is ALWAYS gated. Nothing reaches traffic we were not asked
            //    to fix except through the fallback layer checked below.
            assert!(
                strat.hostlist_only,
                "{mode}: the selected chain must be hostlist-gated, never machine-wide"
            );
            assert!(!hosts.is_empty(), "{mode}: a gated chain with an empty hostlist covers nothing");

            let args = winws.build_args(&strat, &hosts);
            assert!(
                args.iter().any(|a| a.starts_with("--hostlist=")),
                "{mode}: the built command line must carry --hostlist"
            );

            // 2. If the mode has a catch-all fallback layer, that layer must be harmless.
            if !strat.fallback.is_empty() {
                let fb = crate::engine::strategy_by_id(strat.fallback);
                assert!(
                    fb.is_harmless(),
                    "{mode}: catch-all fallback '{}' can forge a packet the real server accepts — \
                     this is exactly what broke an unrelated site",
                    fb.id
                );
            }
        }
    }

    /// The watchdog respawns a dead engine from `current_strategy()`, NOT from `plan_for_mode()`.
    /// If those two disagree, a crash silently reconfigures protection into something the user
    /// never chose — and nothing reports it, because from the outside the engine just came back.
    ///
    /// The concrete divergence this guards: catalog entries carry their own `fallback`, and a
    /// harmless one carries none, so a Güçlü session whose measured winner was harmless used to
    /// lose its catch-all layer on every respawn.
    #[test]
    fn a_watchdog_respawn_rebuilds_the_same_plan() {
        for mode in ["hafif", "guclu"] {
            let mut e = Engine::new();
            e.mode = mode.into();
            let (planned, hosts) = e.plan_for_mode(mode);
            // Mirror what load_mode() commits to the Engine.
            e.strategy = planned.id.to_string();
            e.hostlist = hosts;
            e.hostlist_only = true;
            e.fallback = planned.fallback;
            e.repeats_override = None;

            let respawned = e.current_strategy();
            assert_eq!(respawned.id, planned.id, "{mode}: respawn changed the chain");
            assert_eq!(
                respawned.fallback, planned.fallback,
                "{mode}: respawn dropped or changed the catch-all layer"
            );
            assert_eq!(
                respawned.hostlist_only, planned.hostlist_only,
                "{mode}: respawn changed the scope gating"
            );
        }
    }

    /// Layer order is load-bearing: winws hands a flow to the FIRST profile whose filter matches.
    /// The gated aggressive stage must therefore be emitted BEFORE the catch-all fallback, or the
    /// fallback swallows every flow and the hostlist stage becomes unreachable — which is how the
    /// old third `--filter-tcp=443` block ended up as dead code nobody noticed for months.
    #[cfg(windows)]
    #[test]
    fn guclu_emits_the_gated_stage_before_the_catch_all_layer() {
        let winws = crate::engine::make_engine("zapret");
        let mut e = Engine::new();
        e.mode = "guclu".into();
        let (strat, hosts) = e.plan_for_mode("guclu");
        // Force a chain distinguishable from the fallback so the two stages are tellable apart.
        let mut strat = strat;
        strat.desync = "fake,multidisorder";
        strat.fooling = "md5sig";
        let args = winws.build_args(&strat, &hosts);

        let tls_stages: Vec<usize> = args
            .iter()
            .enumerate()
            .filter(|(_, a)| *a == "--filter-tcp=443")
            .map(|(i, _)| i)
            .collect();
        assert!(
            tls_stages.len() >= 2,
            "Güçlü needs both a gated TLS stage and a catch-all fallback stage, found {}",
            tls_stages.len()
        );
        // The gated TLS stage owns a --hostlist between its own --filter-tcp=443 and the next one.
        // (The port-80 stage earlier in the line also carries one, which is why this looks at the
        // window between the two TLS stages rather than at the first --hostlist in the argv.)
        assert!(
            args[tls_stages[0]..tls_stages[1]].iter().any(|a| a.starts_with("--hostlist=")),
            "the first TLS stage must be hostlist-gated — otherwise the aggressive chain is catch-all"
        );
        // Everything from the fallback stage onward must be free of hostlist gating, or the
        // catch-all layer covers nothing and unknown blocked sites get no help at all.
        assert!(
            !args[tls_stages[1]..].iter().any(|a| a.starts_with("--hostlist=")),
            "the fallback layer must stay catch-all"
        );
    }

    /// The other half of the same rule: when the gated chain and the catch-all fallback would emit
    /// the IDENTICAL TLS stage — which is exactly what an untuned Güçlü looks like, since both
    /// layers resolve to the harmless default — only the catch-all is emitted. Two identical
    /// profiles would do the same work twice and charge a per-connection hostlist lookup for the
    /// privilege.
    #[cfg(windows)]
    #[test]
    fn an_identical_gated_and_fallback_chain_collapses_to_one_stage() {
        let winws = crate::engine::make_engine("zapret");
        let mut s = crate::engine::strategy_by_id(crate::engine::SAFE_FALLBACK);
        s.hostlist_only = true;
        s.fallback = crate::engine::SAFE_FALLBACK;
        let hosts: Vec<String> = CORE_HOSTLIST.iter().map(|h| h.to_string()).collect();
        let args = winws.build_args(&s, &hosts);

        let tls_stages = args.iter().filter(|a| **a == "--filter-tcp=443").count();
        assert_eq!(tls_stages, 1, "identical chains must collapse to a single TLS stage");
        // And the one that survives is the CATCH-ALL — dropping the catch-all instead would silently
        // narrow the mode to its hostlist.
        let idx = args.iter().position(|a| a == "--filter-tcp=443").unwrap();
        assert!(
            !args[idx..].iter().take_while(|a| **a != "--new").any(|a| a.starts_with("--hostlist=")),
            "the surviving TLS stage must be the catch-all one"
        );
        // HTTP/80 and QUIC stay gated: the fallback only ever covers TLS, so collapsing those too
        // would drop coverage rather than remove duplication.
        assert!(
            args.iter().any(|a| a.starts_with("--hostlist=")),
            "the non-TLS gated stages must still be present"
        );
    }

    /// Güçlü must never deploy a route-dependent chain that has not been measured ON THIS LINE.
    /// With no tuning file present the selection has to fall back to a harmless chain — the property
    /// that makes "wide" safe to switch on before any measurement has happened.
    #[test]
    fn untuned_guclu_falls_back_to_a_harmless_chain() {
        // No tuning for a fabricated network key → `tuned_strategy_id` must not hand back an
        // aggressive preset just because one is compiled in.
        let id = Engine::tuned_strategy_id();
        let s = crate::engine::strategy_by_id(&id);
        if crate::tuner::load().map(|t| crate::tuner::is_current(&t)).unwrap_or(false) {
            // A real measurement exists on this machine; it was allowed to pick anything it proved.
            return;
        }
        assert!(
            s.is_harmless(),
            "with no measurement for this line, Güçlü selected '{id}', which can corrupt traffic"
        );
    }

    /// The two modes must differ in SCOPE, and Hafif must stay the narrow one — its whole promise
    /// is "cannot make anything worse", which a catch-all layer would quietly break.
    #[test]
    fn hafif_stays_narrow_and_guclu_is_wider() {
        let e = Engine::new();
        let (hs, hhosts) = e.plan_for_mode("hafif");
        let (_gs, ghosts) = e.plan_for_mode("guclu");

        assert_eq!(hs.fallback, "", "Hafif must have NO catch-all layer — outside its list it touches nothing");
        assert!(hs.is_harmless(), "Hafif's own chain must be harmless too");
        assert!(hhosts.iter().any(|d| d.contains("discord")));
        assert!(hhosts.iter().any(|d| d.contains("roblox")));
        assert!(
            !hhosts.iter().any(|d| d.contains("youtube")),
            "Hafif must not quietly widen beyond the scope its label promises"
        );
        assert!(ghosts.len() > hhosts.len(), "Güçlü must actually cover more than Hafif");
        for d in &hhosts {
            assert!(ghosts.contains(d), "Güçlü must be a superset of Hafif; missing {d}");
        }
    }

    /// What gets DEPLOYED must be what was MEASURED.
    ///
    /// The tuner measures "off" as the bare line — no gated stage, no fallback layer. If deployment
    /// then bolted a catch-all layer onto that verdict, the shipped configuration would be one no
    /// measurement ever covered, and every HTTPS connection on the machine would get TCP
    /// segmentation applied to solve a problem the measurement said does not exist.
    #[test]
    fn an_off_verdict_deploys_nothing_at_all() {
        let measured = crate::tuner::candidate_strategy_for_test("off");
        assert_eq!(measured.fallback, "", "the tuner measures 'off' bare");
        assert!(measured.desync.is_empty());

        // The deployment side must agree. `fallback_for` is the production rule itself, not a copy
        // of it — `plan_for_mode` calls exactly this. (Calling `plan_for_mode` directly would make
        // the test depend on whatever tuning file this machine happens to have.)
        let deployed_fallback = fallback_for("off");
        assert_eq!(deployed_fallback, measured.fallback, "deployment must match the measurement");
        assert_eq!(
            fallback_for("c1"),
            crate::engine::SAFE_FALLBACK,
            "any chain that DOES touch packets still gets the harmless catch-all layer"
        );

        let hosts: Vec<String> = WIDE_HOSTLIST.iter().map(|s| s.to_string()).collect();
        let mut s = crate::engine::strategy_by_id("off");
        s.hostlist_only = true;
        s.fallback = deployed_fallback;
        assert!(
            !crate::engine::has_web_stage(&s, &hosts),
            "an 'off' verdict must start no engine process at all — DNS-only means DNS-only"
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
