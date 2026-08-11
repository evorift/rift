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
    /// Last telemetry sample cached here by the subscription thread (item 11.2). Used by
    /// `Command::Health` to derive the `HealthSignal` without requiring an active subscriber.
    last_metrics: Option<Metrics>,
    /// When `last_metrics` was last updated. `None` until the first telemetry tick arrives.
    metrics_at: Option<Instant>,
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
            last_metrics: None,
            metrics_at: None,
        }
    }

    /// Resolve the active strategy for the current engine, applying the runtime hostlist-mode override
    /// (item 1.3). winws reads `Strategy::hostlist_only` to decide catch-all vs `--hostlist` gating.
    fn current_strategy(&self) -> engine::Strategy {
        let mut s = engine::strategy_by_id(&self.strategy);
        if self.hostlist_only {
            s.hostlist_only = true;
        }
        s
    }

    fn want_warp(&self) -> bool {
        if self.app_modes.is_empty() {
            return true;
        }
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
        }
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

/// Boot'ta otomatik açılan korumanın varsayılan hostlist'i (state.svelte.ts CORE_SITES + YouTube).
const DEFAULT_HOSTLIST: &[&str] = &[
    "discord.com", "discordapp.com", "discord.gg", "discordapp.net", "discord.media",
    "gateway.discord.gg", "cdn.discordapp.com", "roblox.com", "www.roblox.com", "rbxcdn.com",
    "youtube.com", "googlevideo.com",
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
            if e.dns.is_empty() {
                e.dns = "cloudflare".into();
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
                    Response::Status(e.status())
                }
                Err(m) => {
                    e.state = RunState::Error;
                    Response::Error { message: m }
                }
            }
        }
        Command::Stop => {
            e.warp.stop();
            e.dpi.stop();
            e.running = false;
            e.state = RunState::Idle;
            audit("stop");
            Response::Status(e.status())
        }
        Command::Status => Response::Status(e.status()),
        Command::SetStrategy { id } => {
            audit(&format!("set_strategy {id}"));
            e.strategy = id;
            if e.running {
                // Force a restart so the new strategy actually takes effect: start() alone is idempotent
                // and would no-op on a live winws child. Brief (~1s) gap is fine for an explicit change.
                e.dpi.stop();
                let strat = e.current_strategy();
                let hl = e.hostlist.clone();
                if let Err(m) = e.dpi.start(&strat, &hl) {
                    e.state = RunState::Error;
                    e.running = false;
                    return Response::Error { message: m };
                }
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
                    Ok(()) => e.state = RunState::Active,
                    Err(m) => {
                        e.state = RunState::Error;
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
            e.dns = profile.clone();
            crate::rollback::record(Change::DnsChanged);
            let status = e.status();
            drop(e);
            match crate::dns::run_dns(&profile) {
                Ok(()) => Response::Status(status),
                Err(m) => Response::Error { message: m },
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
        Command::ApplyProfile { id } => apply_profile(&mut e, &id),
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

    // Boot auto-protect (servis çalışıyor = korumalı). Panik servis döngüsünü düşürmesin (fail-safe).
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
        e.state = RunState::Applying;
        e.strategy = "auto".into();
        e.dns = "cloudflare".into();
        e.hostlist = DEFAULT_HOSTLIST.iter().map(|s| s.to_string()).collect();
        let strat = e.current_strategy();
        let hl = e.hostlist.clone();
        match e.dpi.start(&strat, &hl) {
            Ok(()) => {
                e.running = true;
                e.state = RunState::Active;
                e.sync_warp();
                audit("auto-start ok (boot koruması açık)");
            }
            Err(m) => {
                e.state = RunState::Error;
                audit(&format!("auto-start başarısız (UI Start gönderene dek kapalı): {m}"));
            }
        }
        drop(e);
        if let Err(m) = crate::dns::run_dns("cloudflare") {
            audit(&format!("auto-start DNS uygulanamadı: {m}"));
        }
    }));

    // winws watchdog + per-app off PID exclusion (5 sn). Paused durumda (Auto-Pilot) dokunma.
    {
        let engine = Arc::clone(&engine);
        std::thread::spawn(move || loop {
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
            let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
            if e.running && e.state != RunState::Paused {
                e.dpi.set_exclusion(&excl);
                let strat = e.current_strategy();
                let hl = e.hostlist.clone();
                let _ = e.dpi.start(&strat, &hl);
                match e.warp_target() {
                    Some(full) if !e.warp.is_running() => {
                        let _ = e.warp.start(full);
                    }
                    None if e.warp.is_running() => {
                        e.warp.stop();
                    }
                    _ => {}
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

    /// Item 7.2: apply_profile_obj handles desync, local-proxy, and tunnel profiles (sim engines in dev).
    #[test]
    fn apply_three_engine_kinds() {
        let mut e = Engine::new();
        // desync (zapret)
        let r = apply_profile_obj(&mut e, &prof("p1", "zapret", ScopeMode::System));
        assert!(matches!(r, Response::Status(_)));
        assert_eq!(e.engine_id, "zapret");
        assert!(e.running);
        // local-proxy (byedpi), Split scope → hostlist mode
        apply_profile_obj(&mut e, &prof("p2", "byedpi", ScopeMode::Split));
        assert_eq!(e.engine_id, "byedpi");
        assert!(e.hostlist_only);
        // tunnel (warp), System scope → full tunnel
        apply_profile_obj(&mut e, &prof("p3", "warp", ScopeMode::System));
        assert_eq!(e.engine_id, "warp");
        assert!(e.full_warp);
        assert!(e.running);
    }
}
