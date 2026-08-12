//! GoodbyeDPI (Turkey fork) engine — DPI desync + DNS-redirect (docs/01, docs/04, docs/07 §3).
//!
//! ValdikSS/GoodbyeDPI core; the Turkey fork ships ready `-1..-9` modesets + DNS-redirect flags.
//! Uses WinDivert (like winws → the same single-driver conflict rule applies; the orchestrator runs
//! only one desync engine at a time). Bundle: `<exe_dir>\goodbyedpi\goodbyedpi.exe` (not yet shipped —
//! when absent, start() degrades to a logged no-op so boot is never broken).
//!
//! Item 2.1 models the modesets explicitly so the command line is transparent (docs/06 §1.4) and
//! editable later; DNS-redirect (2.2) and blacklist (2.3) are layered on top.

use crate::engine::{BypassEngine, EngineCaps, EngineKind, Strategy};

/// GoodbyeDPI modeset (docs/01 §3). Each `-N` shorthand expands to a fixed flag combination; we model
/// the expansion explicitly (GoodbyeDPI accepts the explicit flags identically to the shorthand).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GdMode {
    M1,
    M2,
    M3,
    M4,
    M5,
    M6,
    M7,
    M8,
    M9,
}

impl GdMode {
    /// Map 1..=9 to a mode; anything else → M9 (GoodbyeDPI's own default, docs/01 §3).
    pub fn from_u8(n: u8) -> GdMode {
        match n {
            1 => GdMode::M1,
            2 => GdMode::M2,
            3 => GdMode::M3,
            4 => GdMode::M4,
            5 => GdMode::M5,
            6 => GdMode::M6,
            7 => GdMode::M7,
            8 => GdMode::M8,
            _ => GdMode::M9,
        }
    }

    pub fn num(&self) -> u8 {
        match self {
            GdMode::M1 => 1,
            GdMode::M2 => 2,
            GdMode::M3 => 3,
            GdMode::M4 => 4,
            GdMode::M5 => 5,
            GdMode::M6 => 6,
            GdMode::M7 => 7,
            GdMode::M8 => 8,
            GdMode::M9 => 9,
        }
    }

    /// Documented flag expansion (docs/01 §3), verbatim.
    pub fn expand(&self) -> Vec<&'static str> {
        match self {
            // Legacy modes (docs/01 §3)
            GdMode::M1 => vec!["-p", "-r", "-s", "-f", "2", "-k", "2", "-n", "-e", "2"],
            GdMode::M2 => vec!["-p", "-r", "-s", "-f", "2", "-k", "2", "-n", "-e", "40"],
            GdMode::M3 => vec!["-p", "-r", "-s", "-e", "40"],
            GdMode::M4 => vec!["-p", "-r", "-s"],
            // Modern modes (more stable / compatible / faster)
            GdMode::M5 => vec!["-f", "2", "-e", "2", "--auto-ttl", "--reverse-frag", "--max-payload"],
            GdMode::M6 => vec!["-f", "2", "-e", "2", "--wrong-seq", "--reverse-frag", "--max-payload"],
            GdMode::M7 => vec!["-f", "2", "-e", "2", "--wrong-chksum", "--reverse-frag", "--max-payload"],
            GdMode::M8 => vec!["-f", "2", "-e", "2", "--wrong-seq", "--wrong-chksum", "--reverse-frag", "--max-payload"],
            GdMode::M9 => vec!["-f", "2", "-e", "2", "--wrong-seq", "--wrong-chksum", "--reverse-frag", "--max-payload", "-q"],
        }
    }
}

/// WinDivert-based DNS redirect (docs/01 §4 — the Turkey fork's key trick). Redirects UDP DNS to a
/// chosen resolver, optionally on a NON-standard port: ISPs that hijack UDP/53 (forcing their own DNS)
/// can't recognize traffic to e.g. port 1253, so DNS censorship is bypassed without changing Windows DNS.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DnsRedirect {
    pub v4: String,
    pub v4_port: u16,
    pub v6: String,
    pub v6_port: u16,
}

impl DnsRedirect {
    /// Cloudflare on the standard port 53 (SplitWire goodbyedpi preset, docs/03 §5.1).
    pub fn cloudflare() -> Self {
        Self { v4: "1.1.1.1".into(), v4_port: 53, v6: "2606:4700:4700::1111".into(), v6_port: 53 }
    }

    /// Yandex on the NON-standard port 1253 — beats ISPs that hijack UDP/53 (docs/01 §4 Turkey trick).
    pub fn yandex_nonstandard() -> Self {
        Self { v4: "77.88.8.8".into(), v4_port: 1253, v6: "2a02:6b8::feed:0ff".into(), v6_port: 1253 }
    }

    pub fn to_args(&self) -> Vec<String> {
        vec![
            "--dns-addr".into(),
            self.v4.clone(),
            "--dns-port".into(),
            self.v4_port.to_string(),
            "--dnsv6-addr".into(),
            self.v6.clone(),
            "--dnsv6-port".into(),
            self.v6_port.to_string(),
        ]
    }
}

/// Default GoodbyeDPI blacklist (docs/03 §5.2 — SplitWire `Resources/goodbyedpi/blacklist.txt`, verbatim).
/// With `--blacklist`, GoodbyeDPI only desyncs these hosts/subdomains (HTTP Host / TLS SNI) → fewer side
/// effects than system-wide.
pub fn default_blacklist() -> Vec<String> {
    [
        "discord.gg", "discord.com", "discordapp.com", "roblox.com", "arkoselabs.com",
        "rbxcdn.com", "rbxinfra.net", "rbxtrk.com", "amazonaws.com", "wattpad.com",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Write the blacklist domains (one per line) to `path` (used when blacklist mode is on).
fn write_blacklist_to(path: &std::path::Path, domains: &[String]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(path, domains.join("\r\n"))
}

/// A GoodbyeDPI preset = modeset + optional `--set-ttl` + optional Cloudflare DNS-redirect
/// (docs/03 §5.1 — SplitWire `Resources/goodbyedpi/presets.txt`).
#[derive(Clone, Debug)]
pub struct GdPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub mode: GdMode,
    /// `--set-ttl <n>` (fake-packet TTL). 0 = omit.
    pub set_ttl: u32,
    /// Include the Cloudflare DNS-redirect (`--dns-addr 1.1.1.1 …`).
    pub dns: bool,
}

/// GoodbyeDPI preset catalog (docs/03 §5.1).
pub fn presets() -> Vec<GdPreset> {
    vec![
        GdPreset { id: "standard", name: "Standard (-5 ttl5 + DNS)", mode: GdMode::M5, set_ttl: 5, dns: true },
        GdPreset { id: "mode5", name: "Mode 5", mode: GdMode::M5, set_ttl: 0, dns: false },
        GdPreset { id: "mode6", name: "Mode 6", mode: GdMode::M6, set_ttl: 0, dns: false },
        GdPreset { id: "mode7", name: "Mode 7", mode: GdMode::M7, set_ttl: 0, dns: false },
        GdPreset { id: "mode8", name: "Mode 8", mode: GdMode::M8, set_ttl: 0, dns: false },
        GdPreset { id: "mode9", name: "Mode 9 (default)", mode: GdMode::M9, set_ttl: 0, dns: false },
        GdPreset { id: "mode9-dns", name: "Mode 9 + DNS", mode: GdMode::M9, set_ttl: 0, dns: true },
        GdPreset { id: "ttl3", name: "TTL 3 (-9 + set-ttl 3)", mode: GdMode::M9, set_ttl: 3, dns: false },
    ]
}

#[cfg(windows)]
pub struct GoodbyeDpiEngine {
    child: Option<std::process::Child>,
    job: isize,
    /// Active modeset (docs/01 §3). Default M9 (GoodbyeDPI's own default).
    mode: GdMode,
    /// DNS-redirect (item 2.2). `None` = leave DNS untouched; `Some` = append --dns-addr/-port flags.
    /// Default: Cloudflare on port 53 (matches the proven SplitWire goodbyedpi preset).
    dns: Option<DnsRedirect>,
    /// Blacklist mode (item 2.3). `None` = system-wide (all hosts); `Some(domains)` = write a file and
    /// pass `--blacklist <file>` so only those hosts are desynced (docs/03 §5.2). Default None.
    blacklist: Option<Vec<String>>,
    /// `--set-ttl <n>` fake-packet TTL (item 2.4 presets). 0 = omit.
    set_ttl: u32,
}

#[cfg(windows)]
impl Default for GoodbyeDpiEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(windows)]
impl GoodbyeDpiEngine {
    pub fn new() -> Self {
        Self { child: None, job: 0, mode: GdMode::M9, dns: Some(DnsRedirect::cloudflare()), blacklist: None, set_ttl: 0 }
    }

    pub fn with_mode(mode: GdMode) -> Self {
        Self { child: None, job: 0, mode, dns: Some(DnsRedirect::cloudflare()), blacklist: None, set_ttl: 0 }
    }

    /// Apply a preset (docs/03 §5.1): sets mode + `--set-ttl` + DNS-redirect. Blacklist is left as-is.
    pub fn apply_preset(&mut self, p: &GdPreset) {
        self.mode = p.mode;
        self.set_ttl = p.set_ttl;
        self.dns = if p.dns { Some(DnsRedirect::cloudflare()) } else { None };
    }

    /// Override the DNS-redirect (e.g. the nonstandard-port Turkey trick) or disable it (`None`).
    pub fn set_dns_redirect(&mut self, dns: Option<DnsRedirect>) {
        self.dns = dns;
    }

    /// Override the active mode (used by make_engine_with_params individual-field support).
    pub fn set_mode(&mut self, mode: GdMode) {
        self.mode = mode;
    }

    /// Override the fake-packet TTL (0 = omit the flag).
    pub fn set_ttl_val(&mut self, ttl: u32) {
        self.set_ttl = ttl;
    }

    /// Set blacklist mode: `Some(domains)` restricts desync to those hosts; `None` = system-wide.
    pub fn set_blacklist(&mut self, blacklist: Option<Vec<String>>) {
        self.blacklist = blacklist;
    }

    /// Blacklist file path (`%PROGRAMDATA%\evorift\gd-blacklist.txt`).
    fn blacklist_path() -> std::path::PathBuf {
        crate::ipc::data_dir().join("gd-blacklist.txt")
    }

    fn bundle_dir() -> Option<std::path::PathBuf> {
        Some(std::env::current_exe().ok()?.parent()?.join("goodbyedpi"))
    }

    fn exe() -> Option<std::path::PathBuf> {
        Some(Self::bundle_dir()?.join("goodbyedpi.exe"))
    }

    /// Build the GoodbyeDPI command line: mode flags (2.1) + DNS-redirect (2.2) + blacklist (2.3).
    fn gd_args(&self) -> Vec<String> {
        let mut a: Vec<String> = self.mode.expand().into_iter().map(|s| s.to_string()).collect();
        if self.set_ttl > 0 {
            a.push("--set-ttl".into());
            a.push(self.set_ttl.to_string());
        }
        if let Some(dns) = &self.dns {
            a.extend(dns.to_args());
        }
        if let Some(bl) = &self.blacklist {
            let path = Self::blacklist_path();
            if write_blacklist_to(&path, bl).is_err() {
                eprintln!("[evorift][goodbyedpi] blacklist file write failed: {}", path.display());
            }
            a.push("--blacklist".into());
            a.push(path.to_string_lossy().into_owned());
        }
        a
    }
}

#[cfg(windows)]
impl BypassEngine for GoodbyeDpiEngine {
    fn id(&self) -> &'static str {
        "goodbyedpi"
    }
    fn kind(&self) -> EngineKind {
        EngineKind::Desync
    }
    fn caps(&self) -> EngineCaps {
        EngineCaps { split_tunnel: false, requires_windivert: true, kernel_less: false }
    }
    fn is_available(&self) -> bool {
        Self::exe().map(|e| e.exists()).unwrap_or(false)
    }
    fn build_args(&self, _strategy: &Strategy, _hostlist: &[String]) -> Vec<String> {
        self.gd_args()
    }
    fn start(&mut self, _strategy: &Strategy, _hostlist: &[String]) -> Result<(), String> {
        if let Some(c) = self.child.as_mut() {
            if matches!(c.try_wait(), Ok(None)) {
                return Ok(());
            }
        }
        use std::os::windows::process::CommandExt;
        let exe = match Self::exe() {
            Some(e) if e.exists() => e,
            _ => return Err("goodbyedpi.exe bulunamadı — bundle eksik".into()),
        };
        let dir = exe.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let gd = self.gd_args();
        crate::proc::kill_image("goodbyedpi.exe");
        // GoodbyeDPI also uses WinDivert → never run alongside winws (single-driver conflict). The
        // orchestrator (service.rs) runs only one desync engine at a time (state machine).
        let mut cmd = std::process::Command::new(&exe);
        cmd.args(&gd)
            .current_dir(&dir) // blacklist / file paths may be cwd-relative
            .creation_flags(crate::proc::CREATE_NO_WINDOW);
        let (child, job) = crate::proc::spawn_with_job(cmd)
            .map_err(|e| format!("goodbyedpi failed to start: {e}"))?;
        if self.job == 0 {
            self.job = job;
        } else {
            crate::proc::close_job(job); // discard the new handle; old job still covers the child
        }
        self.child = Some(child);
        Ok(())
    }
    fn stop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        crate::proc::kill_image("goodbyedpi.exe");
    }
    fn is_running(&self) -> bool {
        self.child.is_some()
    }
}

#[cfg(windows)]
impl Drop for GoodbyeDpiEngine {
    fn drop(&mut self) {
        self.stop();
        crate::proc::close_job(self.job);
        self.job = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::{default_blacklist, write_blacklist_to, DnsRedirect, GdMode};

    /// Item 2.1: mode 9 (the default) must expand to the documented flag set (docs/01 §3) verbatim.
    #[test]
    fn mode9_matches_doc() {
        assert_eq!(
            GdMode::M9.expand(),
            vec!["-f", "2", "-e", "2", "--wrong-seq", "--wrong-chksum", "--reverse-frag", "--max-payload", "-q"]
        );
    }

    #[test]
    fn mode_roundtrip_and_legacy() {
        assert_eq!(GdMode::from_u8(9).num(), 9);
        assert_eq!(GdMode::from_u8(0).num(), 9, "unknown → default M9");
        assert_eq!(GdMode::from_u8(123).num(), 9, "out of range → default M9");
        assert_eq!(GdMode::M4.expand(), vec!["-p", "-r", "-s"]);
        assert_eq!(GdMode::M1.expand(), vec!["-p", "-r", "-s", "-f", "2", "-k", "2", "-n", "-e", "2"]);
        assert_eq!(GdMode::M5.expand(), vec!["-f", "2", "-e", "2", "--auto-ttl", "--reverse-frag", "--max-payload"]);
    }

    /// Item 2.2: DNS-redirect args — standard Cloudflare:53 + the nonstandard-port Turkey trick.
    #[test]
    fn dns_redirect_args() {
        let cf = DnsRedirect::cloudflare().to_args();
        let got: Vec<&str> = cf.iter().map(|s| s.as_str()).collect();
        assert_eq!(
            got,
            vec!["--dns-addr", "1.1.1.1", "--dns-port", "53", "--dnsv6-addr", "2606:4700:4700::1111", "--dnsv6-port", "53"]
        );
        let yx = DnsRedirect::yandex_nonstandard().to_args();
        assert_eq!(yx.iter().filter(|s| s.as_str() == "1253").count(), 2, "v4+v6 on nonstandard port");
    }

    /// build_args layers the DNS-redirect on top of the mode flags (item 2.1 + 2.2).
    #[cfg(windows)]
    #[test]
    fn build_args_includes_mode_and_dns() {
        use crate::engine::{strategy_by_id, BypassEngine};
        let a = super::GoodbyeDpiEngine::new().build_args(&strategy_by_id("auto"), &[]);
        assert!(a.iter().any(|s| s == "--reverse-frag"), "mode flags present");
        assert!(a.iter().any(|s| s == "--dns-addr"), "dns-redirect present");
        assert!(a.iter().any(|s| s == "1.1.1.1"));
    }

    /// Item 2.3: blacklist file gets written with the documented default content.
    #[test]
    fn blacklist_written_and_default_content() {
        let p = std::env::temp_dir().join("evorift-test-gd-blacklist.txt");
        let _ = std::fs::remove_file(&p);
        write_blacklist_to(&p, &default_blacklist()).expect("write blacklist");
        assert!(p.exists(), "blacklist file must be written");
        let body = std::fs::read_to_string(&p).unwrap();
        assert!(body.contains("discord.com") && body.contains("roblox.com"));
        let _ = std::fs::remove_file(&p);
    }

    /// build_args adds `--blacklist` only when blacklist mode is set; system-wide by default.
    #[cfg(windows)]
    #[test]
    fn build_args_blacklist_toggle() {
        use crate::engine::{strategy_by_id, BypassEngine};
        let mut eng = super::GoodbyeDpiEngine::new();
        eng.set_blacklist(Some(default_blacklist()));
        let on = eng.build_args(&strategy_by_id("auto"), &[]);
        assert!(on.iter().any(|s| s == "--blacklist"), "blacklist flag present when set");
        let off = super::GoodbyeDpiEngine::new().build_args(&strategy_by_id("auto"), &[]);
        assert!(!off.iter().any(|s| s == "--blacklist"), "system-wide by default");
    }

    /// Item 2.4: preset catalog matches docs/03 §5.1 (the Standard preset = -5 + set-ttl 5 + DNS).
    #[test]
    fn preset_catalog_standard() {
        let ps = super::presets();
        assert!(ps.len() >= 8, "at least 8 presets");
        let std = ps.iter().find(|p| p.id == "standard").expect("standard preset");
        assert_eq!(std.mode, GdMode::M5);
        assert_eq!(std.set_ttl, 5);
        assert!(std.dns);
    }

    /// apply_preset wires mode + --set-ttl + DNS into build_args.
    #[cfg(windows)]
    #[test]
    fn apply_preset_build_args() {
        use crate::engine::{strategy_by_id, BypassEngine};
        let mut eng = super::GoodbyeDpiEngine::new();
        let std = super::presets().into_iter().find(|p| p.id == "standard").unwrap();
        eng.apply_preset(&std);
        let a = eng.build_args(&strategy_by_id("auto"), &[]);
        assert!(a.windows(2).any(|w| w[0] == "--set-ttl" && w[1] == "5"), "set-ttl 5 present");
        assert!(a.iter().any(|s| s == "--reverse-frag"), "M5 mode flags present");
        assert!(a.iter().any(|s| s == "--dns-addr"), "dns present");

        let m9 = super::presets().into_iter().find(|p| p.id == "mode9").unwrap();
        eng.apply_preset(&m9);
        let b = eng.build_args(&strategy_by_id("auto"), &[]);
        assert!(!b.iter().any(|s| s == "--set-ttl"), "mode9 preset has no set-ttl");
        assert!(!b.iter().any(|s| s == "--dns-addr"), "mode9 preset has no dns");
    }
}
