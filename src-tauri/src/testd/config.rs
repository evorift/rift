//! Test-agent configuration: what to bind, who may talk to it, where the sandbox is.
//!
//! Written by `scripts/testd-install.ps1` at install time and read once at startup. The shared
//! bearer token is deliberately NOT in here — it lives in its own ACL'd file under `state_dir`
//! (see [`super::token`]) so this file stays safe to read, diff and paste into a bug report.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, UdpSocket};
use std::path::{Path, PathBuf};

/// Why a config was refused. Every variant is a hard startup failure: an agent that binds the
/// wrong interface or trusts the wrong peer is worse than an agent that does not start.
#[derive(Debug)]
pub enum ConfigError {
    Unreadable { path: PathBuf, source: std::io::Error },
    Malformed { path: PathBuf, detail: String },
    /// `bind_ip` is 0.0.0.0 / `::` — would expose the agent on every interface.
    BindUnspecified,
    /// `bind_ip` parses but is not a loopback/private/link-local address, i.e. binding it would
    /// put a remote-execution endpoint on a publicly routable address.
    BindNotLocal(IpAddr),
    BindNotAnAddress(String),
    /// `bind_ip` was `"auto"` but the routing table could not name a local address that reaches
    /// the controller — usually the network is down entirely.
    AutoBindFailed { toward: IpAddr },
    /// Port 0 would let the OS pick a port the firewall rule does not cover.
    PortNotFixed,
    AllowlistEmpty,
    AllowlistEntryNotAnAddress(String),
    /// An allowlist entry of 0.0.0.0 / `::` matches nothing useful and signals a confused config.
    AllowlistEntryUnspecified(IpAddr),
    SandboxNotAbsolute(PathBuf),
    StateDirNotAbsolute(PathBuf),
    /// The audit log and token would be reachable through `/pull` — refuse rather than leak.
    StateDirInsideSandbox { state_dir: PathBuf, sandbox_root: PathBuf },
    DeadmanOutOfRange(u64),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable { path, source } => {
                write!(f, "config unreadable at {}: {source}", path.display())
            }
            Self::Malformed { path, detail } => {
                write!(f, "config malformed at {}: {detail}", path.display())
            }
            Self::BindUnspecified => write!(
                f,
                "bind_ip is the unspecified address (0.0.0.0 / ::) — refusing to start; \
                 set bind_ip to this laptop's LAN address"
            ),
            Self::BindNotLocal(ip) => write!(
                f,
                "bind_ip {ip} is not a loopback/private/link-local address — refusing to expose \
                 the test agent on a publicly routable address"
            ),
            Self::BindNotAnAddress(s) => {
                write!(f, "bind_ip {s:?} is not an IP address (use \"auto\" to follow DHCP)")
            }
            Self::AutoBindFailed { toward } => write!(
                f,
                "bind_ip is \"auto\" but no local address routes toward {toward} — is the network up?"
            ),
            Self::PortNotFixed => write!(f, "port must be a fixed non-zero port"),
            Self::AllowlistEmpty => write!(f, "allowlist is empty — no controller could connect"),
            Self::AllowlistEntryNotAnAddress(s) => {
                write!(f, "allowlist entry {s:?} is not an IP address")
            }
            Self::AllowlistEntryUnspecified(ip) => {
                write!(f, "allowlist entry {ip} is the unspecified address")
            }
            Self::SandboxNotAbsolute(p) => {
                write!(f, "sandbox_root {} must be an absolute path", p.display())
            }
            Self::StateDirNotAbsolute(p) => {
                write!(f, "state_dir {} must be an absolute path", p.display())
            }
            Self::StateDirInsideSandbox { state_dir, sandbox_root } => write!(
                f,
                "state_dir {} is inside sandbox_root {} — the token and audit log would be \
                 downloadable via /pull",
                state_dir.display(),
                sandbox_root.display()
            ),
            Self::DeadmanOutOfRange(s) => {
                write!(f, "deadman_secs {s} outside the allowed 10..=3600 range")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

/// On-disk shape. `deny_unknown_fields` is load-bearing security, not tidiness: a typo like
/// `"allow_list"` would otherwise be silently ignored and the real `allowlist` would fall back
/// to its default. There is no default — but the same argument applies to every future field.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    bind_ip: String,
    port: u16,
    allowlist: Vec<String>,
    sandbox_root: String,
    state_dir: String,
    #[serde(default = "default_deadman_secs")]
    deadman_secs: u64,
    #[serde(default = "default_max_upload_bytes")]
    max_upload_bytes: u64,
    #[serde(default = "default_job_timeout_secs")]
    default_job_timeout_secs: u64,
}

fn default_deadman_secs() -> u64 {
    120
}
fn default_max_upload_bytes() -> u64 {
    512 * 1024 * 1024
}
fn default_job_timeout_secs() -> u64 {
    300
}

/// Validated configuration. Constructing one of these is proof the guards below have run.
#[derive(Debug, Clone)]
pub struct Config {
    pub bind_ip: IpAddr,
    /// True when `bind_ip` was resolved from `"auto"` rather than written literally. The server
    /// only watches for address changes in that case — a literal address is the operator saying
    /// "this one, and fail loudly if it is gone".
    pub bind_auto: bool,
    pub port: u16,
    pub allowlist: Vec<AllowEntry>,
    pub sandbox_root: PathBuf,
    pub state_dir: PathBuf,
    pub deadman_secs: u64,
    pub max_upload_bytes: u64,
    pub default_job_timeout_secs: u64,
}

/// Is this an address that can only be reached from the local machine or the local network?
///
/// The agent runs arbitrary (sandbox-confined) commands on request, so binding it to a
/// globally routable address is never what the operator meant, even if a firewall would have
/// caught it. Refusing here means the mistake cannot be made silently.
pub fn is_local_scope(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                // 100.64.0.0/10 — carrier-grade NAT, also what Tailscale hands out.
                || (v4.octets()[0] == 100 && (64..128).contains(&v4.octets()[1]))
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                // fc00::/7 unique-local and fe80::/10 link-local. `Ipv6Addr::is_unique_local`
                // and `is_unicast_link_local` are still unstable, so test the prefixes directly.
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
                // An IPv4-mapped LAN address is still a LAN address.
                || v6.to_ipv4_mapped().map(|m| is_local_scope(IpAddr::V4(m))).unwrap_or(false)
        }
    }
}

/// Which local address would this machine use to reach `toward`?
///
/// The routing table already knows the answer, and asking it is far more robust than hard-coding
/// an address that DHCP will eventually move. A `connect` on a UDP socket sends no packets at
/// all — it only fixes the destination so the OS selects a source address — so this is a pure
/// lookup with no traffic and no dependency on the peer being up.
///
/// This exists because a hard-coded `bind_ip` broke the whole setup the first time the laptop's
/// lease changed: the agent stayed bound to an address the machine no longer had, the firewall
/// rule still pinned the old one, and the controller was dialling a third. One lease renewal,
/// three broken things.
pub fn local_address_toward(toward: IpAddr) -> Option<IpAddr> {
    let bind: &str = match toward {
        IpAddr::V4(_) => "0.0.0.0:0",
        IpAddr::V6(_) => "[::]:0",
    };
    let socket = UdpSocket::bind(bind).ok()?;
    // Port 9 (discard) is arbitrary; nothing is transmitted.
    socket.connect((toward, 9)).ok()?;
    socket.local_addr().ok().map(|a| a.ip())
}

/// An allowlist entry: a single address, or a CIDR range.
///
/// Ranges matter because the *controller* can move too. Widening the source filter to the LAN is
/// acceptable precisely because it was never the security boundary — the bearer token is. A
/// filter that silently stops matching after a DHCP renewal is worse than a slightly broader one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllowEntry {
    Exact(IpAddr),
    Cidr { network: IpAddr, prefix: u8 },
}

impl AllowEntry {
    /// Parse `192.168.1.24` or `192.168.1.0/24`.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        match text.split_once('/') {
            None => text.parse().ok().map(|ip| Self::Exact(normalize_peer(ip))),
            Some((addr, prefix)) => {
                let network: IpAddr = addr.trim().parse().ok()?;
                let prefix: u8 = prefix.trim().parse().ok()?;
                let max = if network.is_ipv4() { 32 } else { 128 };
                if prefix > max {
                    return None;
                }
                Some(Self::Cidr { network, prefix })
            }
        }
    }

    pub fn matches(&self, peer: IpAddr) -> bool {
        let peer = normalize_peer(peer);
        match self {
            Self::Exact(ip) => *ip == peer,
            Self::Cidr { network, prefix } => match (network, peer) {
                (IpAddr::V4(net), IpAddr::V4(p)) => {
                    let (net, p) = (u32::from(*net), u32::from(p));
                    // A /0 must not shift by 32 — that is undefined behaviour for u32.
                    if *prefix == 0 {
                        return true;
                    }
                    let mask = u32::MAX << (32 - prefix);
                    (net & mask) == (p & mask)
                }
                (IpAddr::V6(net), IpAddr::V6(p)) => {
                    let (net, p) = (u128::from(*net), u128::from(p));
                    if *prefix == 0 {
                        return true;
                    }
                    let mask = u128::MAX << (128 - prefix);
                    (net & mask) == (p & mask)
                }
                _ => false,
            },
        }
    }
}

/// The address to aim `"auto"` bind resolution at: the first allowlist entry, using a range's
/// network address, which is on the same link and so selects the same source address.
fn hint_from(entries: &[AllowEntry]) -> Option<IpAddr> {
    entries.first().map(|e| match e {
        AllowEntry::Exact(ip) => *ip,
        AllowEntry::Cidr { network, .. } => *network,
    })
}

/// Collapse `::ffff:a.b.c.d` to `a.b.c.d` so an allowlist written in IPv4 still matches a peer
/// that arrived over a dual-stack socket. Without this, a dual-stack bind would 403 the
/// controller for reasons that look like nothing at all from the outside.
pub fn normalize_peer(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => IpAddr::V6(v6),
        },
        v4 => v4,
    }
}

impl Config {
    /// Read and validate the config file. Any failure is fatal by design.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Unreadable {
            path: path.to_path_buf(),
            source,
        })?;
        // Strip a UTF-8 byte-order mark before parsing.
        //
        // Not defensive padding — this was a real, total failure. Windows PowerShell 5.1's
        // `Out-File -Encoding utf8` always writes a BOM, so the installer's own config could not
        // be parsed: serde_json sees U+FEFF where it expects `{` and rejects the file. The agent
        // then died before opening its audit log, so the service reported "Running" with no log
        // anywhere explaining why nothing was listening. Any hand-edit in Notepad reintroduces it.
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        let raw: RawConfig = serde_json::from_str(text).map_err(|e| ConfigError::Malformed {
            path: path.to_path_buf(),
            detail: e.to_string(),
        })?;
        Self::from_raw(raw)
    }

    fn from_raw(raw: RawConfig) -> Result<Self, ConfigError> {
        if raw.port == 0 {
            return Err(ConfigError::PortNotFixed);
        }

        if raw.allowlist.is_empty() {
            return Err(ConfigError::AllowlistEmpty);
        }
        let mut allowlist = Vec::with_capacity(raw.allowlist.len());
        for entry in &raw.allowlist {
            let parsed = AllowEntry::parse(entry)
                .ok_or_else(|| ConfigError::AllowlistEntryNotAnAddress(entry.clone()))?;
            if let AllowEntry::Exact(ip) = parsed {
                if ip.is_unspecified() {
                    return Err(ConfigError::AllowlistEntryUnspecified(ip));
                }
            }
            allowlist.push(parsed);
        }

        // Resolve the bind address AFTER the allowlist, because "auto" means "whichever local
        // address routes toward the controller" and the controller is the allowlist.
        let wants_auto = raw.bind_ip.trim().eq_ignore_ascii_case("auto");
        let bind_ip: IpAddr = if wants_auto {
            let toward = hint_from(&allowlist).ok_or(ConfigError::AllowlistEmpty)?;
            local_address_toward(toward).ok_or(ConfigError::AutoBindFailed { toward })?
        } else {
            raw.bind_ip
                .trim()
                .parse()
                .map_err(|_| ConfigError::BindNotAnAddress(raw.bind_ip.clone()))?
        };

        if bind_ip.is_unspecified() {
            return Err(ConfigError::BindUnspecified);
        }
        if !is_local_scope(bind_ip) {
            return Err(ConfigError::BindNotLocal(bind_ip));
        }

        let sandbox_root = PathBuf::from(raw.sandbox_root);
        if !sandbox_root.is_absolute() {
            return Err(ConfigError::SandboxNotAbsolute(sandbox_root));
        }
        let state_dir = PathBuf::from(raw.state_dir);
        if !state_dir.is_absolute() {
            return Err(ConfigError::StateDirNotAbsolute(state_dir));
        }
        // The token and the audit log must be structurally unreachable through /pull, which can
        // only ever serve paths under sandbox_root. A lexical check is enough here: both paths
        // come from the installer and neither exists yet at first start, so canonicalising is
        // not possible; the sandbox module does the canonical check per request.
        if state_dir.starts_with(&sandbox_root) {
            return Err(ConfigError::StateDirInsideSandbox { state_dir, sandbox_root });
        }

        if !(10..=3600).contains(&raw.deadman_secs) {
            return Err(ConfigError::DeadmanOutOfRange(raw.deadman_secs));
        }

        Ok(Self {
            bind_ip,
            bind_auto: wants_auto,
            port: raw.port,
            allowlist,
            sandbox_root,
            state_dir,
            deadman_secs: raw.deadman_secs,
            max_upload_bytes: raw.max_upload_bytes,
            default_job_timeout_secs: raw.default_job_timeout_secs.clamp(1, 3600),
        })
    }

    /// Is this peer allowed to talk to the agent at all?
    ///
    /// NOTE (documented, not fixed here): a source-IP allowlist is a filter, not authentication —
    /// a host on the same LAN can spoof the controller's address. It is why every endpoint except
    /// `/health` also demands the bearer token. See docs/REMOTE-TESTING.md.
    pub fn is_allowed(&self, peer: IpAddr) -> bool {
        self.allowlist.iter().any(|entry| entry.matches(peer))
    }

    /// The address the allowlist points at, used to re-resolve an `"auto"` bind. `None` when the
    /// allowlist holds only ranges with no usable target.
    pub fn controller_hint(&self) -> Option<IpAddr> {
        hint_from(&self.allowlist)
    }
}

/// Convenience for tests and for the loopback smoke check in the installer.
pub const LOOPBACK_V4: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
/// Exposed so callers can name the unspecified v6 address without importing `Ipv6Addr`.
pub const UNSPECIFIED_V6: IpAddr = IpAddr::V6(Ipv6Addr::UNSPECIFIED);

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(bind: &str, allow: &[&str]) -> RawConfig {
        RawConfig {
            bind_ip: bind.to_string(),
            port: 8765,
            allowlist: allow.iter().map(|s| s.to_string()).collect(),
            sandbox_root: r"C:\evorift-test".to_string(),
            state_dir: r"C:\ProgramData\evorift-testd".to_string(),
            deadman_secs: 120,
            max_upload_bytes: 1024,
            default_job_timeout_secs: 300,
        }
    }

    #[test]
    fn a_valid_lan_config_is_accepted() {
        let cfg = Config::from_raw(raw("192.168.1.50", &["192.168.1.20"]))
            .expect("a plain LAN config must load");
        assert_eq!(cfg.port, 8765);
        assert_eq!(cfg.deadman_secs, 120);
        assert!(cfg.is_allowed("192.168.1.20".parse::<IpAddr>().expect("literal")));
        assert!(!cfg.is_allowed("192.168.1.21".parse::<IpAddr>().expect("literal")));
    }

    #[test]
    fn refuses_to_bind_all_interfaces() {
        assert!(matches!(
            Config::from_raw(raw("0.0.0.0", &["192.168.1.20"])),
            Err(ConfigError::BindUnspecified)
        ));
        assert!(matches!(
            Config::from_raw(raw("::", &["192.168.1.20"])),
            Err(ConfigError::BindUnspecified)
        ));
    }

    #[test]
    fn refuses_to_bind_a_public_address() {
        assert!(matches!(
            Config::from_raw(raw("8.8.8.8", &["192.168.1.20"])),
            Err(ConfigError::BindNotLocal(_))
        ));
    }

    #[test]
    fn accepts_the_private_and_cgnat_ranges() {
        for ip in ["10.0.0.5", "172.16.3.1", "192.168.1.50", "169.254.7.7", "100.101.0.1", "127.0.0.1"] {
            let parsed: IpAddr = ip.parse().expect("literal");
            assert!(is_local_scope(parsed), "{ip} should count as local scope");
        }
        for ip in ["8.8.8.8", "1.1.1.1", "172.32.0.1", "100.128.0.1"] {
            let parsed: IpAddr = ip.parse().expect("literal");
            assert!(!is_local_scope(parsed), "{ip} should NOT count as local scope");
        }
    }

    /// Tailscale hands out CGNAT v4 (100.64.0.0/10) and ULA v6 out of fd7a:115c:a1e0::/48.
    /// Both must count as local scope or the agent would refuse to bind a tailnet address.
    #[test]
    fn tailscale_addresses_are_accepted() {
        for ip in ["100.120.14.56", "100.103.86.68", "100.64.0.1", "100.127.255.254"] {
            let parsed: IpAddr = ip.parse().expect("literal");
            assert!(is_local_scope(parsed), "Tailscale v4 {ip} must be allowed");
        }
        for ip in ["fd7a:115c:a1e0::634:e39", "fd7a:115c:a1e0::e34:5646"] {
            let parsed: IpAddr = ip.parse().expect("literal");
            assert!(is_local_scope(parsed), "Tailscale v6 {ip} must be allowed");
        }
        // The addresses either side of the CGNAT block are still refused.
        for ip in ["100.63.255.255", "100.128.0.0"] {
            let parsed: IpAddr = ip.parse().expect("literal");
            assert!(!is_local_scope(parsed), "{ip} is outside 100.64/10 and must be refused");
        }
    }

    #[test]
    fn a_tailscale_config_loads_end_to_end() {
        let cfg = Config::from_raw(raw("100.103.86.68", &["100.120.14.56"]))
            .expect("a tailnet config must load");
        assert!(cfg.is_allowed("100.120.14.56".parse::<IpAddr>().expect("literal")));
        assert!(!cfg.is_allowed("100.99.99.99".parse::<IpAddr>().expect("literal")));
    }

    /// The scenario that cost a day: both machines got new DHCP leases at once, so the literal
    /// bind address, the literal allowlist entry and the controller's target were all stale
    /// simultaneously. `"auto"` plus a CIDR allowlist survives exactly that.
    #[test]
    fn auto_bind_follows_the_route_and_a_cidr_allowlist_survives_dhcp() {
        // A range covers the controller wherever DHCP puts it on that LAN.
        let entry = AllowEntry::parse("192.168.1.0/24").expect("valid CIDR");
        assert!(entry.matches("192.168.1.24".parse().expect("literal")), "old controller address");
        assert!(entry.matches("192.168.1.17".parse().expect("literal")), "new controller address");
        assert!(!entry.matches("192.168.2.17".parse().expect("literal")), "a different LAN");

        // "auto" resolves against the real routing table; loopback is always routable, so this
        // asserts the mechanism without depending on which network the test machine is on.
        let resolved = local_address_toward("127.0.0.1".parse().expect("literal"))
            .expect("the routing table must name a source address for loopback");
        assert!(resolved.is_loopback(), "toward loopback the source must be loopback, got {resolved}");

        let mut r = raw("auto", &["127.0.0.0/8"]);
        r.state_dir = r"C:\ProgramData\evorift-testd".to_string();
        let cfg = Config::from_raw(r).expect("an auto config must resolve");
        assert!(cfg.bind_auto, "must remember it was auto, so the watcher runs");
        assert!(cfg.bind_ip.is_loopback());
        assert_eq!(cfg.controller_hint(), Some("127.0.0.0".parse().expect("literal")));
    }

    #[test]
    fn allowlist_entries_parse_as_addresses_or_ranges() {
        assert_eq!(
            AllowEntry::parse("192.168.1.24"),
            Some(AllowEntry::Exact("192.168.1.24".parse().expect("literal")))
        );
        match AllowEntry::parse("10.0.0.0/8").expect("valid") {
            AllowEntry::Cidr { prefix, .. } => assert_eq!(prefix, 8),
            other => panic!("expected a CIDR, got {other:?}"),
        }
        // A /0 must not shift a u32 by 32 (undefined behaviour) -- it matches everything.
        assert!(AllowEntry::parse("0.0.0.0/0").expect("valid").matches("8.8.8.8".parse().expect("literal")));
        // /32 is a single host.
        let single = AllowEntry::parse("192.168.1.24/32").expect("valid");
        assert!(single.matches("192.168.1.24".parse().expect("literal")));
        assert!(!single.matches("192.168.1.25".parse().expect("literal")));
        // Nonsense is rejected rather than silently widened.
        assert_eq!(AllowEntry::parse("192.168.1.0/33"), None);
        assert_eq!(AllowEntry::parse("not-an-ip"), None);
        assert_eq!(AllowEntry::parse("192.168.1.0/abc"), None);
    }

    #[test]
    fn a_cidr_allowlist_still_rejects_outsiders() {
        let cfg = Config::from_raw(raw("192.168.1.18", &["192.168.1.0/24"])).expect("valid");
        assert!(cfg.is_allowed("192.168.1.17".parse().expect("literal")));
        assert!(!cfg.is_allowed("192.168.9.17".parse().expect("literal")));
        assert!(!cfg.is_allowed("8.8.8.8".parse().expect("literal")));
    }

    #[test]
    fn refuses_an_empty_allowlist() {
        assert!(matches!(
            Config::from_raw(raw("192.168.1.50", &[])),
            Err(ConfigError::AllowlistEmpty)
        ));
    }

    #[test]
    fn refuses_a_wildcard_allowlist_entry() {
        assert!(matches!(
            Config::from_raw(raw("192.168.1.50", &["0.0.0.0"])),
            Err(ConfigError::AllowlistEntryUnspecified(_))
        ));
    }

    #[test]
    fn refuses_a_state_dir_that_pull_could_reach() {
        let mut r = raw("192.168.1.50", &["192.168.1.20"]);
        r.state_dir = r"C:\evorift-test\state".to_string();
        assert!(matches!(
            Config::from_raw(r),
            Err(ConfigError::StateDirInsideSandbox { .. })
        ));
    }

    #[test]
    fn refuses_an_absurd_deadman_window() {
        let mut r = raw("192.168.1.50", &["192.168.1.20"]);
        r.deadman_secs = 0;
        assert!(matches!(Config::from_raw(r), Err(ConfigError::DeadmanOutOfRange(0))));
    }

    #[test]
    fn an_ipv4_mapped_peer_matches_an_ipv4_allowlist_entry() {
        let cfg = Config::from_raw(raw("192.168.1.50", &["192.168.1.20"])).expect("valid");
        let mapped: IpAddr = "::ffff:192.168.1.20".parse().expect("literal");
        assert!(cfg.is_allowed(mapped));
    }

    /// Regression: PowerShell 5.1 `Out-File -Encoding utf8` emits a BOM, and the agent could not
    /// read its own installer's config. Cost a full round trip to the laptop to diagnose,
    /// because the failure happened before the audit log existed.
    #[test]
    fn a_config_written_with_a_utf8_bom_still_loads() {
        let dir = std::env::temp_dir().join("evorift-testd-bom-test");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("testd.config.json");

        let json = r#"{"bind_ip":"192.168.1.25","port":8765,"allowlist":["192.168.1.24"],
                       "sandbox_root":"C:\\evorift-test","state_dir":"C:\\ProgramData\\evorift-testd",
                       "deadman_secs":120}"#;

        // Exactly what Out-File -Encoding utf8 produces: EF BB BF then the document.
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(json.as_bytes());
        std::fs::write(&path, &bytes).expect("write BOM config");

        let cfg = Config::load(&path).expect("a BOM-prefixed config must still load");
        assert_eq!(cfg.port, 8765);
        assert_eq!(cfg.bind_ip, "192.168.1.25".parse::<IpAddr>().expect("literal"));

        // And the same document without a BOM must be identical, not merely also-accepted.
        std::fs::write(&path, json.as_bytes()).expect("write plain config");
        let plain = Config::load(&path).expect("plain config loads");
        assert_eq!(plain.bind_ip, cfg.bind_ip);
        assert_eq!(plain.allowlist, cfg.allowlist);

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn unknown_config_keys_are_rejected_rather_than_ignored() {
        // A typo'd allowlist key must not silently produce a config with no allowlist.
        let json = r#"{"bind_ip":"192.168.1.50","port":8765,"allow_list":["192.168.1.20"],
                       "allowlist":["192.168.1.20"],"sandbox_root":"C:\\evorift-test",
                       "state_dir":"C:\\ProgramData\\evorift-testd"}"#;
        let parsed: Result<RawConfig, _> = serde_json::from_str(json);
        assert!(parsed.is_err(), "unknown key must be a parse error");
    }

    #[test]
    fn unspecified_v6_constant_is_actually_unspecified() {
        assert!(UNSPECIFIED_V6.is_unspecified());
        assert!(LOOPBACK_V4.is_loopback());
    }
}
