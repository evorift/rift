//! ByeDPI (ciadpi) engine — KERNEL-LESS local SOCKS5 proxy (docs/03 §2, docs/04 §4, docs/07 §3).
//!
//! WHY: the WinDivert kernel driver is blocked by Kaspersky/some AVs → desync engines (winws, GoodbyeDPI)
//! can't run. ByeDPI uses no kernel driver: it opens a SOCKS5 proxy (default 127.0.0.1:1080) and applies
//! DPI desync at the proxy level. Routing apps into that proxy needs ProxiFyre (item 3.2) or drover
//! (item 3.3); the engine's own job is to run ciadpi.exe with proven params. Bundle:
//! `<exe_dir>\byedpi\ciadpi.exe` (not yet shipped — absent → logged sim no-op so boot is never broken).

use crate::engine::{BypassEngine, EngineCaps, EngineKind, Strategy};
use crate::pid_scan::ExclusionPorts;

/// ByeDPI SOCKS5 listen port (ciadpi's own default; ProxiFyre/drover point here).
pub const SOCKS_PORT: u16 = 1080;

/// Typed ciadpi parameter set (docs/03 §2.1, docs/04 §4). Each `Option`/flag maps to one ciadpi arg.
/// `port` is omitted from the command line when it equals the ciadpi default (1080) — matching the
/// proven SplitWire invocation byte-for-byte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ByeDpiConfig {
    pub port: u16,
    /// `--split <pos>` (split TLS ClientHello at byte/pos), e.g. "1".
    pub split: Option<String>,
    /// `--disorder <pos>` (reorder segments), e.g. "3+s".
    pub disorder: Option<String>,
    /// `--fake <pos>` (inject a fake packet), e.g. "1".
    pub fake: Option<String>,
    /// `--mod-http=<mods>` (HTTP header tricks), e.g. "h,d".
    pub mod_http: Option<String>,
    /// `--tlsrec <pos>` (split the TLS record layer), e.g. "1+s".
    pub tlsrec: Option<String>,
    /// `--auto=<trigger>` (auto-retry strategy on timeout/RST), e.g. "torst".
    pub auto: Option<String>,
    /// `--ttl <n>` (fake-packet TTL). 0 = omit.
    pub ttl: u32,
    /// `--oob` (out-of-band data trick).
    pub oob: bool,
}

impl ByeDpiConfig {
    /// Proven SplitWire ByeDPI default (docs/03 §2.1):
    /// `--split 1 --disorder 3+s --mod-http=h,d --auto=torst --tlsrec 1+s`.
    pub fn balanced() -> Self {
        Self {
            port: SOCKS_PORT,
            split: Some("1".into()),
            disorder: Some("3+s".into()),
            fake: None,
            mod_http: Some("h,d".into()),
            tlsrec: Some("1+s".into()),
            auto: Some("torst".into()),
            ttl: 0,
            oob: false,
        }
    }

    /// Build the ciadpi command line. Order matches the documented invocation (split, disorder,
    /// mod-http, auto, tlsrec); `--port` is emitted only when non-default (1080).
    pub fn to_args(&self) -> Vec<String> {
        let mut a: Vec<String> = Vec::new();
        if self.port != SOCKS_PORT {
            a.push("--port".into());
            a.push(self.port.to_string());
        }
        if let Some(v) = &self.split {
            a.push("--split".into());
            a.push(v.clone());
        }
        if let Some(v) = &self.disorder {
            a.push("--disorder".into());
            a.push(v.clone());
        }
        if let Some(v) = &self.fake {
            a.push("--fake".into());
            a.push(v.clone());
        }
        if let Some(v) = &self.mod_http {
            a.push(format!("--mod-http={v}"));
        }
        if let Some(v) = &self.auto {
            a.push(format!("--auto={v}"));
        }
        if let Some(v) = &self.tlsrec {
            a.push("--tlsrec".into());
            a.push(v.clone());
        }
        if self.ttl > 0 {
            a.push("--ttl".into());
            a.push(self.ttl.to_string());
        }
        if self.oob {
            a.push("--oob".into());
        }
        a
    }
}

/// A named ByeDPI preset.
#[derive(Clone, Debug)]
pub struct ByeDpiPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub config: ByeDpiConfig,
}

/// ByeDPI preset catalog. `balanced` is the proven SplitWire default (docs/03 §2.1); the others are
/// lighter/heavier variants built from the same param model.
pub fn presets() -> Vec<ByeDpiPreset> {
    vec![
        ByeDpiPreset { id: "balanced", name: "Balanced (proven default)", config: ByeDpiConfig::balanced() },
        ByeDpiPreset {
            id: "split",
            name: "Split only (light)",
            config: ByeDpiConfig {
                port: SOCKS_PORT,
                split: Some("1".into()),
                disorder: None,
                fake: None,
                mod_http: None,
                tlsrec: Some("1+s".into()),
                auto: None,
                ttl: 0,
                oob: false,
            },
        },
        ByeDpiPreset {
            id: "fake",
            name: "Fake + disorder (heavy)",
            config: ByeDpiConfig {
                port: SOCKS_PORT,
                split: Some("1".into()),
                disorder: Some("3+s".into()),
                fake: Some("1".into()),
                mod_http: Some("h,d".into()),
                tlsrec: Some("1+s".into()),
                auto: Some("torst".into()),
                ttl: 0,
                oob: false,
            },
        },
    ]
}

/// How ByeDPI's SOCKS5 proxy is fed traffic (docs/03 §2-3) — the "composite engine mode":
///  - `None`: ciadpi only (the app configures its own proxy, or manual).
///  - `ProxiFyre { browsers }`: system NDIS routing of the listed apps (split tunnel) — privileged service.
///  - `Drover`: Discord-only DLL hijack (user context). Best in serviceless mode (engine runs as the user).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Routing {
    None,
    ProxiFyre { browsers: bool },
    Drover,
}

#[cfg(windows)]
pub struct ByeDpiEngine {
    child: Option<std::process::Child>,
    job: isize,
    /// Active ciadpi config (default = proven balanced preset). `apply_preset`/`set_config` change it.
    config: ByeDpiConfig,
    /// How selected app traffic reaches the proxy (item 3.4). Default `None` (plain ciadpi).
    routing: Routing,
}

#[cfg(windows)]
impl Default for ByeDpiEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(windows)]
impl ByeDpiEngine {
    pub fn new() -> Self {
        Self { child: None, job: 0, config: ByeDpiConfig::balanced(), routing: Routing::None }
    }

    /// Composite engine: ciadpi + a routing method (ProxiFyre split-tunnel or drover Discord-only).
    pub fn with_routing(routing: Routing) -> Self {
        Self { child: None, job: 0, config: ByeDpiConfig::balanced(), routing }
    }

    pub fn set_config(&mut self, config: ByeDpiConfig) {
        self.config = config;
    }

    /// Read access to the active config (used by make_engine_with_params to clone-and-patch).
    pub fn config(&self) -> &ByeDpiConfig {
        &self.config
    }

    pub fn apply_preset(&mut self, p: &ByeDpiPreset) {
        self.config = p.config.clone();
    }

    fn bundle_dir() -> Option<std::path::PathBuf> {
        Some(std::env::current_exe().ok()?.parent()?.join("byedpi"))
    }

    fn exe() -> Option<std::path::PathBuf> {
        Some(Self::bundle_dir()?.join("ciadpi.exe"))
    }
}

#[cfg(windows)]
impl BypassEngine for ByeDpiEngine {
    fn id(&self) -> &'static str {
        match self.routing {
            Routing::None => "byedpi",
            Routing::ProxiFyre { .. } => "byedpi-proxifyre",
            Routing::Drover => "byedpi-drover",
        }
    }
    fn kind(&self) -> EngineKind {
        EngineKind::LocalProxy
    }
    fn caps(&self) -> EngineCaps {
        // kernel-less → the only engine that runs under Kaspersky/AV; split-tunnel via ProxiFyre/drover.
        EngineCaps { split_tunnel: true, requires_windivert: false, kernel_less: true }
    }
    fn is_available(&self) -> bool {
        Self::exe().map(|e| e.exists()).unwrap_or(false)
    }
    fn build_args(&self, _strategy: &Strategy, _hostlist: &[String]) -> Vec<String> {
        self.config.to_args()
    }
    fn start(&mut self, _strategy: &Strategy, _hostlist: &[String]) -> Result<(), String> {
        if let Some(c) = self.child.as_mut() {
            if matches!(c.try_wait(), Ok(None)) {
                return Ok(()); // idempotent
            }
        }
        use std::os::windows::process::CommandExt;
        let exe = match Self::exe() {
            Some(e) if e.exists() => e,
            _ => {
                eprintln!("[evorift][byedpi] bundle missing (ciadpi.exe) — sim (no real proxy)");
                return Ok(()); // fail-safe: boot never broken
            }
        };
        let args = self.config.to_args();
        crate::proc::kill_image("ciadpi.exe"); // single instance
        let mut cmd = std::process::Command::new(&exe);
        cmd.args(&args).creation_flags(crate::proc::CREATE_NO_WINDOW);
        let (child, job) = crate::proc::spawn_with_job(cmd)
            .map_err(|e| format!("ciadpi failed to start: {e}"))?;
        if self.job == 0 {
            self.job = job;
        } else {
            crate::proc::close_job(job); // discard extra handle; old job still covers the child
        }
        self.child = Some(child);

        // Bring up the routing layer (item 3.4) — best-effort; failures are logged, ciadpi stays up.
        match &self.routing {
            Routing::None => {}
            Routing::ProxiFyre { browsers } => {
                if let Err(m) = crate::proxifyre::install(*browsers, self.config.port) {
                    eprintln!("[evorift][byedpi] ProxiFyre routing failed: {m}");
                }
            }
            Routing::Drover => {
                if let Err(m) = crate::drover::install(self.config.port) {
                    eprintln!("[evorift][byedpi] drover routing failed: {m}");
                }
            }
        }
        Ok(())
    }
    fn stop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        crate::proc::kill_image("ciadpi.exe");
        // Tear down the routing layer.
        match &self.routing {
            Routing::None => {}
            Routing::ProxiFyre { .. } => {
                let _ = crate::proxifyre::uninstall();
            }
            Routing::Drover => crate::drover::remove_all(),
        }
    }
    fn is_running(&self) -> bool {
        self.child.is_some()
    }
    fn set_exclusion(&mut self, _excl: &ExclusionPorts) {}
}

#[cfg(windows)]
impl Drop for ByeDpiEngine {
    fn drop(&mut self) {
        self.stop();
        crate::proc::close_job(self.job);
        self.job = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::{ByeDpiConfig, SOCKS_PORT};

    /// Item 3.1: the balanced config matches the documented SplitWire invocation (docs/03 §2.1) exactly,
    /// and omits `--port` at the default 1080.
    #[test]
    fn balanced_matches_doc() {
        let a = ByeDpiConfig::balanced().to_args();
        let got: Vec<&str> = a.iter().map(|s| s.as_str()).collect();
        assert_eq!(
            got,
            vec!["--split", "1", "--disorder", "3+s", "--mod-http=h,d", "--auto=torst", "--tlsrec", "1+s"]
        );
        assert!(!a.iter().any(|s| s == "--port"), "default port 1080 omitted");
    }

    /// Non-default port + ttl + oob are emitted; presets resolve.
    #[test]
    fn nondefault_fields_and_presets() {
        let mut c = ByeDpiConfig::balanced();
        c.port = 1090;
        c.ttl = 4;
        c.oob = true;
        let a = c.to_args();
        assert!(a.windows(2).any(|w| w[0] == "--port" && w[1] == "1090"));
        assert!(a.windows(2).any(|w| w[0] == "--ttl" && w[1] == "4"));
        assert!(a.iter().any(|s| s == "--oob"));

        let ps = super::presets();
        assert!(ps.iter().any(|p| p.id == "balanced"));
        assert_eq!(ps.iter().find(|p| p.id == "balanced").unwrap().config, ByeDpiConfig::balanced());
        assert_eq!(SOCKS_PORT, 1080);
    }

    /// Item 3.4: composite engine ids reflect the routing; build_args stays the ciadpi line; start/stop
    /// in sim (no bundle, unprivileged) is a clean no-op.
    #[cfg(windows)]
    #[test]
    fn composite_modes() {
        use super::{ByeDpiEngine, Routing};
        use crate::engine::{strategy_by_id, BypassEngine};
        assert_eq!(ByeDpiEngine::new().id(), "byedpi");
        assert_eq!(ByeDpiEngine::with_routing(Routing::ProxiFyre { browsers: true }).id(), "byedpi-proxifyre");
        assert_eq!(ByeDpiEngine::with_routing(Routing::Drover).id(), "byedpi-drover");
        let pf = ByeDpiEngine::with_routing(Routing::ProxiFyre { browsers: false });
        assert!(pf.build_args(&strategy_by_id("auto"), &[]).iter().any(|s| s == "--split"));
        let mut e = ByeDpiEngine::with_routing(Routing::Drover);
        assert!(e.start(&strategy_by_id("auto"), &[]).is_ok());
        e.stop();
    }
}
