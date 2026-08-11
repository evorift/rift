//! `evorift-testd` — the remote test agent that runs on the second Windows laptop.
//!
//! # The constraint this is built around
//!
//! The software under test cuts all internet on its host until the process is killed. The agent
//! therefore loses contact with the controller *at exactly the moment a test fails* — the
//! interesting moment. Two consequences shape every design decision in this module:
//!
//! **The laptop must be able to save itself.** [`recovery::Deadman`] fires an unattended
//! recovery routine after `deadman_secs` of silence from the controller — killing `evorift.exe`
//! and `winws.exe`, tearing down stale WinDivert services via evorift's own
//! [`crate::engine::clear_stale_windivert`], and putting DNS back on DHCP.
//!
//! **Results must survive the outage.** Command output and state captures are written to files
//! on the laptop the instant they are produced, never held in memory waiting for an HTTP
//! response that may never be deliverable. The controller fetches them afterwards, possibly
//! minutes later, over a link that has since come back.
//!
//! # Security posture
//!
//! * binds one LAN address, refuses to start on `0.0.0.0` ([`config`]);
//! * source-IP allowlist — necessary, explicitly **not** sufficient, because a LAN peer can
//!   spoof an address;
//! * shared bearer token compared in constant time, required on every endpoint but `/health`
//!   ([`token`]);
//! * every network-supplied path confined to one sandbox root by canonical resolution, so
//!   traversal, absolute paths and escaping symlinks all fail ([`sandbox`]);
//! * append-only audit log of every request, kept outside the sandbox alongside the token, so
//!   neither can be fetched through `/pull` ([`audit`]).
//!
//! # Deployment
//!
//! Not started by the evorift application — nothing in the product's call graph references this
//! module. It runs only as the separate `evorift-testd` binary, installed as a Windows service
//! by `scripts/testd-install.ps1`. See `docs/REMOTE-TESTING.md`.

pub mod audit;
pub mod config;
pub mod http;
pub mod jobs;
pub mod recovery;
pub mod sandbox;
pub mod server;
pub mod token;
pub mod util;

use std::path::{Path, PathBuf};

/// Everything that can stop the agent from starting. Each one is fatal: a test agent that half
/// starts is worse than one that does not, because the operator would trust it.
#[derive(Debug)]
pub enum StartupError {
    Config(config::ConfigError),
    Token(token::TokenError),
    Serve(server::ServeError),
}

impl std::fmt::Display for StartupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(e) => write!(f, "configuration rejected: {e}"),
            Self::Token(e) => write!(f, "token rejected: {e}"),
            Self::Serve(e) => write!(f, "server failed: {e}"),
        }
    }
}

impl std::error::Error for StartupError {}

/// Default install location for the config and token, matching `scripts/testd-install.ps1`.
pub fn default_state_dir() -> PathBuf {
    // %ProgramData% is machine-scoped and outside the sandbox root by construction.
    let base = std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".to_string());
    Path::new(&base).join("evorift-testd")
}

pub fn default_config_path() -> PathBuf {
    default_state_dir().join("testd.config.json")
}

/// Load config + token and serve. Returns only on a fatal error; a healthy agent blocks here.
pub fn start(config_path: &Path) -> Result<(), StartupError> {
    let cfg = config::Config::load(config_path).map_err(StartupError::Config)?;
    let token_path = cfg.state_dir.join("testd.token");
    let shared_token = token::load(&token_path).map_err(StartupError::Token)?;
    server::run(cfg, shared_token).map_err(StartupError::Serve)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_state_dir_is_absolute_and_outside_any_plausible_sandbox() {
        let dir = default_state_dir();
        assert!(dir.is_absolute(), "state dir must be absolute: {}", dir.display());
        assert!(!dir.starts_with(r"C:\evorift-test"), "state dir must not sit in the sandbox");
        assert!(default_config_path().starts_with(&dir));
    }

    #[test]
    fn startup_errors_render_a_cause() {
        let e = StartupError::Config(config::ConfigError::BindUnspecified);
        let text = e.to_string();
        assert!(text.contains("configuration rejected"));
        assert!(text.contains("0.0.0.0"));
    }
}
