//! Binary manifest: records each bundled tool's expected SHA-256, version, and source repo.
//! `verify_all()` hashes the actual on-disk files and compares — mismatches are logged but NOT
//! fatal (optional bundles like byedpi/goodbyedpi are absent until the user provides them).
//! `manifest.json` lives in the resources dir next to the exe and is included in every build.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// One entry from `resources/manifest.json`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BinaryEntry {
    /// Short display name (e.g. "winws.exe").
    pub name: String,
    /// Semantic version string (e.g. "67.8").
    pub version: String,
    /// Lowercase hex SHA-256 of the binary as shipped.
    pub sha256: String,
    /// Upstream source repository URL.
    pub source: String,
    /// Path relative to the resources bundle dir (e.g. "winws/winws.exe").
    pub path: String,
}

/// Per-binary verification result returned to the UI.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VerifyResult {
    pub name: String,
    pub path: String,
    pub expected_sha256: String,
    /// `None` = file absent (optional bundle not installed).
    pub actual_sha256: Option<String>,
    /// `true` = file present AND hash matches the manifest.
    pub ok: bool,
}

/// Compute the SHA-256 of a file; return lowercase hex string.
pub fn sha256_file(path: &Path) -> Result<String, String> {
    let data = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let result = hasher.finalize();
    Ok(result.iter().map(|b| format!("{b:02x}")).collect())
}

/// Load and parse `manifest.json` from the given path.
pub fn load_manifest(manifest_path: &Path) -> Result<Vec<BinaryEntry>, String> {
    let json = std::fs::read_to_string(manifest_path)
        .map_err(|e| format!("manifest.json not found at {}: {e}", manifest_path.display()))?;
    serde_json::from_str(&json).map_err(|e| format!("manifest.json parse error: {e}"))
}

/// Resolve the resources directory: `<exe_dir>/` (Tauri bundles resources next to the exe).
pub fn resources_dir() -> Option<PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.to_path_buf())
}

/// Verify each listed binary against its expected SHA-256. Missing files get `ok: false,
/// actual_sha256: None` — expected for optional bundles. Returns all entries (ok + not-ok).
pub fn verify_manifest(entries: &[BinaryEntry], base_dir: &Path) -> Vec<VerifyResult> {
    entries
        .iter()
        .map(|e| {
            let file_path = base_dir.join(&e.path);
            let actual = if file_path.exists() {
                sha256_file(&file_path).ok()
            } else {
                None
            };
            let ok = actual.as_deref() == Some(e.sha256.as_str());
            if !ok {
                if actual.is_none() {
                    crate::sys::audit(&format!("manifest: {} absent (optional bundle)", e.name));
                } else {
                    crate::sys::audit(&format!("manifest: {} hash MISMATCH", e.name));
                }
            }
            VerifyResult {
                name: e.name.clone(),
                path: e.path.clone(),
                expected_sha256: e.sha256.clone(),
                actual_sha256: actual,
                ok,
            }
        })
        .collect()
}

/// Load `resources/manifest.json` and verify all listed binaries. Advisory — never panics.
pub fn verify_all() -> Result<Vec<VerifyResult>, String> {
    let base = resources_dir().ok_or("cannot resolve resources dir")?;
    let entries = load_manifest(&base.join("manifest.json"))?;
    Ok(verify_manifest(&entries, &base))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Item 10.3: sha256_file computes the correct hash for a known byte sequence.
    #[test]
    fn sha256_file_correct_hash() {
        let dir = std::env::temp_dir().join("evorift-test-manifest");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("hello.bin");
        std::fs::write(&path, b"hello").unwrap();
        let hash = sha256_file(&path).expect("sha256 should work");
        // SHA-256("hello") = 2cf24dba...
        assert_eq!(hash, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
        let _ = std::fs::remove_file(&path);
    }

    /// Item 10.3: verify_manifest returns ok=false, actual=None for a missing file.
    #[test]
    fn verify_manifest_missing_file_not_ok() {
        let entries = vec![BinaryEntry {
            name: "ghost.exe".into(),
            version: "1.0".into(),
            sha256: "abc123".into(),
            source: "https://example.com".into(),
            path: "no_such_dir/ghost.exe".into(),
        }];
        let results = verify_manifest(&entries, &std::env::temp_dir());
        assert_eq!(results.len(), 1);
        assert!(!results[0].ok, "missing file must not be ok");
        assert!(results[0].actual_sha256.is_none(), "actual_sha256 must be None for absent file");
    }

    /// Item 10.3: verify_manifest returns ok=true when hash matches.
    #[test]
    fn verify_manifest_correct_hash_is_ok() {
        let dir = std::env::temp_dir().join("evorift-test-manifest-ok");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("real.bin");
        std::fs::write(&path, b"hello").unwrap();
        let entries = vec![BinaryEntry {
            name: "real.bin".into(),
            version: "1.0".into(),
            sha256: "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824".into(),
            source: "https://example.com".into(),
            path: "real.bin".into(),
        }];
        let results = verify_manifest(&entries, &dir);
        assert!(results[0].ok, "correct hash must be ok");
        assert_eq!(results[0].actual_sha256.as_deref(),
            Some("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"));
        let _ = std::fs::remove_file(&path);
    }

    /// Item 10.3: load_manifest parses a JSON array of BinaryEntry correctly.
    #[test]
    fn load_manifest_parses_json() {
        let dir = std::env::temp_dir().join("evorift-test-manifest-load");
        std::fs::create_dir_all(&dir).unwrap();
        let json = r#"[{"name":"x.exe","version":"1.0","sha256":"aabbcc","source":"https://ex.com","path":"x.exe"}]"#;
        let p = dir.join("manifest.json");
        std::fs::write(&p, json).unwrap();
        let entries = load_manifest(&p).expect("parse ok");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "x.exe");
        assert_eq!(entries[0].sha256, "aabbcc");
        let _ = std::fs::remove_file(&p);
    }
}
