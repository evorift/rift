//! Path confinement for `/push`, `/pull` and `/run`.
//!
//! Every filesystem path that reaches the agent from the network is resolved through here. The
//! rule is absolute: the result must live under the sandbox root, and "under" is decided by
//! canonicalising both sides — not by string prefix — so a junction or symlink planted inside
//! the sandbox cannot be used to read or write outside it.
//!
//! Windows-specific traps handled below, each of which defeats a naive `..` filter:
//!   * `C:\...` and `\\server\share` (absolute / UNC) never join to a root the way a caller expects.
//!   * `C:foo` is a *drive-relative* path, not a folder called `C:`.
//!   * `file.txt:hidden` addresses an NTFS alternate data stream on `file.txt`.
//!   * `CON`, `NUL`, `COM1`… are device names at every directory level, with or without extension.
//!   * A trailing dot or space (`evil.exe.`) is silently stripped by Win32, so it resolves to a
//!     different file than the one that was validated.

use std::path::{Component, Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub enum PathError {
    Empty,
    /// Absolute, drive-relative (`C:foo`) or UNC.
    NotRelative,
    /// Contains a `..` component.
    ParentTraversal,
    /// NTFS alternate data stream, device name, trailing dot/space, or a NUL byte.
    IllegalComponent(String),
    /// Canonicalised to somewhere outside the sandbox root — the symlink/junction case.
    EscapesSandbox,
    Io(String),
}

impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "path is empty"),
            Self::NotRelative => {
                write!(f, "path must be relative to the sandbox root (no drive, root or UNC prefix)")
            }
            Self::ParentTraversal => write!(f, "path contains a '..' component"),
            Self::IllegalComponent(c) => write!(f, "path component {c:?} is not allowed"),
            Self::EscapesSandbox => {
                write!(f, "path resolves outside the sandbox root (symlink or junction)")
            }
            Self::Io(e) => write!(f, "path could not be resolved: {e}"),
        }
    }
}

impl std::error::Error for PathError {}

/// Reserved DOS device names. These resolve to devices from *any* directory, so
/// `captures\NUL` is still the null device — the check has to run per component.
const DEVICE_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

fn is_device_name(component: &str) -> bool {
    // The device is addressed with or without an extension: `NUL` and `NUL.txt` are both it.
    let stem = component.split('.').next().unwrap_or(component);
    DEVICE_NAMES.iter().any(|d| stem.eq_ignore_ascii_case(d))
}

/// Syntactic validation, before the filesystem is touched at all.
///
/// Returns the cleaned relative path (with `.` components dropped). Both `/` and `\` are accepted
/// as separators so the controller can write either.
pub fn validate_relative(rel: &str) -> Result<PathBuf, PathError> {
    if rel.trim().is_empty() {
        return Err(PathError::Empty);
    }
    if rel.contains('\0') {
        return Err(PathError::IllegalComponent("<NUL byte>".to_string()));
    }
    let normalized = rel.replace('/', "\\");
    let path = Path::new(&normalized);

    let mut cleaned = PathBuf::new();
    for component in path.components() {
        match component {
            // A drive prefix (`C:`), a UNC prefix, or a leading `\` all mean this was never a
            // path relative to the sandbox.
            Component::Prefix(_) | Component::RootDir => return Err(PathError::NotRelative),
            Component::ParentDir => return Err(PathError::ParentTraversal),
            Component::CurDir => continue,
            Component::Normal(part) => {
                let part = part.to_str().ok_or_else(|| {
                    PathError::IllegalComponent("<non-UTF-8>".to_string())
                })?;
                // `C:foo` parses as a single Normal component on some inputs — catch the colon
                // directly, which also covers NTFS alternate data streams (`file.txt:evil`).
                if part.contains(':') {
                    return Err(PathError::IllegalComponent(part.to_string()));
                }
                // Win32 strips trailing dots and spaces, so a validated `a.exe.` opens `a.exe`.
                if part.ends_with('.') || part.ends_with(' ') {
                    return Err(PathError::IllegalComponent(part.to_string()));
                }
                if is_device_name(part) {
                    return Err(PathError::IllegalComponent(part.to_string()));
                }
                cleaned.push(part);
            }
        }
    }
    if cleaned.as_os_str().is_empty() {
        return Err(PathError::Empty);
    }
    Ok(cleaned)
}

/// Canonicalise `root` once at startup. Every later check compares against this value, so a
/// verbatim (`\\?\C:\...`) prefix on one side and not the other can never cause a false mismatch.
pub fn canonical_root(root: &Path) -> Result<PathBuf, PathError> {
    std::fs::create_dir_all(root).map_err(|e| PathError::Io(e.to_string()))?;
    root.canonicalize().map_err(|e| PathError::Io(e.to_string()))
}

/// Resolve a path that must already exist (`/pull`, `/run`'s program).
///
/// The canonicalisation is the real boundary: it follows symlinks and junctions, so a link
/// planted inside the sandbox pointing at `C:\Windows` fails here even though its *syntax* was
/// perfectly innocent.
pub fn resolve_existing(canonical_root: &Path, rel: &str) -> Result<PathBuf, PathError> {
    let cleaned = validate_relative(rel)?;
    let joined = canonical_root.join(cleaned);
    let resolved = joined.canonicalize().map_err(|e| PathError::Io(e.to_string()))?;
    if !resolved.starts_with(canonical_root) {
        return Err(PathError::EscapesSandbox);
    }
    Ok(resolved)
}

/// Resolve a path that does not exist yet (`/push`), creating its parent directories.
///
/// The target itself cannot be canonicalised (it is not there yet), so the *parent* is
/// canonicalised after creation and the file name appended to that. An attacker who pre-planted
/// a junction as one of the parent directories is caught, because the junction is resolved before
/// the containment test runs.
pub fn resolve_for_create(canonical_root: &Path, rel: &str) -> Result<PathBuf, PathError> {
    let cleaned = validate_relative(rel)?;
    let joined = canonical_root.join(&cleaned);
    let parent = joined.parent().ok_or(PathError::NotRelative)?;
    std::fs::create_dir_all(parent).map_err(|e| PathError::Io(e.to_string()))?;
    let parent_resolved = parent.canonicalize().map_err(|e| PathError::Io(e.to_string()))?;
    if !parent_resolved.starts_with(canonical_root) {
        return Err(PathError::EscapesSandbox);
    }
    let name = joined.file_name().ok_or(PathError::NotRelative)?;
    let target = parent_resolved.join(name);
    // If the target already exists it may itself be a link out of the sandbox; re-check it.
    if target.exists() {
        let existing = target.canonicalize().map_err(|e| PathError::Io(e.to_string()))?;
        if !existing.starts_with(canonical_root) {
            return Err(PathError::EscapesSandbox);
        }
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_nested_path_is_accepted() {
        assert_eq!(
            validate_relative(r"docs\captures\01.txt").expect("valid"),
            PathBuf::from(r"docs\captures\01.txt")
        );
        // Forward slashes are accepted and normalised.
        assert_eq!(
            validate_relative("docs/captures/01.txt").expect("valid"),
            PathBuf::from(r"docs\captures\01.txt")
        );
        // `.` components are dropped, not rejected.
        assert_eq!(
            validate_relative(r".\build\evorift.exe").expect("valid"),
            PathBuf::from(r"build\evorift.exe")
        );
    }

    #[test]
    fn traversal_is_rejected_in_every_position() {
        for bad in [r"..\windows\system32\config\sam", r"docs\..\..\secret", r"a\..\b", ".."] {
            assert_eq!(
                validate_relative(bad),
                Err(PathError::ParentTraversal),
                "{bad} must be rejected"
            );
        }
    }

    #[test]
    fn absolute_and_unc_paths_are_rejected() {
        for bad in [
            r"C:\Windows\System32\drivers\etc\hosts",
            r"\Windows",
            r"\\server\share\file",
            r"\\?\C:\Windows",
            "/etc/passwd",
        ] {
            assert_eq!(validate_relative(bad), Err(PathError::NotRelative), "{bad} must be rejected");
        }
    }

    #[test]
    fn drive_relative_and_alternate_data_streams_are_rejected() {
        // `C:foo` means "foo, relative to the current directory on drive C:".
        assert!(matches!(validate_relative("C:foo"), Err(PathError::NotRelative)));
        // NTFS alternate data stream on an otherwise legal file name.
        assert!(matches!(
            validate_relative("report.txt:hidden"),
            Err(PathError::IllegalComponent(_))
        ));
    }

    #[test]
    fn dos_device_names_are_rejected_at_any_depth() {
        for bad in ["NUL", "con", r"docs\NUL", r"docs\nul.txt", "COM1", r"a\LPT9.log", "PRN"] {
            assert!(
                matches!(validate_relative(bad), Err(PathError::IllegalComponent(_))),
                "{bad} must be rejected"
            );
        }
        // A name that merely starts with the same letters is fine.
        assert!(validate_relative("console.log").is_ok());
        assert!(validate_relative("nullable.txt").is_ok());
    }

    #[test]
    fn trailing_dot_or_space_is_rejected() {
        // Win32 strips these, so the validated name and the opened name would differ.
        assert!(matches!(validate_relative("evil.exe."), Err(PathError::IllegalComponent(_))));
        assert!(matches!(validate_relative("evil.exe "), Err(PathError::IllegalComponent(_))));
    }

    #[test]
    fn empty_and_nul_bytes_are_rejected() {
        assert_eq!(validate_relative(""), Err(PathError::Empty));
        assert_eq!(validate_relative("   "), Err(PathError::Empty));
        assert_eq!(validate_relative("."), Err(PathError::Empty));
        assert!(matches!(validate_relative("a\0b"), Err(PathError::IllegalComponent(_))));
    }

    /// End-to-end containment against a real directory: the canonical-prefix test is what
    /// actually stops an escape, so exercise it rather than trusting the syntax filter alone.
    #[test]
    fn resolution_is_confined_to_the_canonical_root() {
        let base = std::env::temp_dir().join("evorift-testd-sandbox-test");
        let root = base.join("root");
        let outside = base.join("outside");
        std::fs::create_dir_all(&root).expect("root");
        std::fs::create_dir_all(&outside).expect("outside");
        std::fs::write(outside.join("secret.txt"), b"secret").expect("write");
        std::fs::write(root.join("inside.txt"), b"ok").expect("write");

        let canonical = canonical_root(&root).expect("canonicalise root");

        let resolved = resolve_existing(&canonical, "inside.txt").expect("inside resolves");
        assert!(resolved.starts_with(&canonical));

        // Traversal is refused before the filesystem is consulted.
        assert_eq!(
            resolve_existing(&canonical, r"..\outside\secret.txt"),
            Err(PathError::ParentTraversal)
        );

        // A create target lands under the canonical root, parents and all.
        let created = resolve_for_create(&canonical, r"docs\captures\new.txt").expect("create ok");
        assert!(created.starts_with(&canonical));
        assert!(created.parent().map(|p| p.is_dir()).unwrap_or(false));

        std::fs::remove_dir_all(&base).expect("cleanup");
    }
}
