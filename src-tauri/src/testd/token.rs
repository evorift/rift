//! Shared-bearer-token authentication.
//!
//! The token is generated once, at install time, by `scripts/testd-install.ps1` and stored in a
//! single ACL'd file under `state_dir` — which the config validator forces to live OUTSIDE the
//! `/pull` sandbox. It is never logged, never echoed in a response, and never included in an
//! error message (evorift hard rule 13). The only thing that ever leaves this module is a bool.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// A token shorter than this is treated as a broken install rather than a weak-but-usable
/// secret. The installer writes 32 bytes of CSPRNG output as 64 hex characters.
pub const MIN_TOKEN_CHARS: usize = 32;

#[derive(Debug)]
pub enum TokenError {
    Unreadable { path: PathBuf, source: std::io::Error },
    /// Deliberately does NOT carry the token, or even its real length beyond the bound.
    TooShort { path: PathBuf, min: usize },
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable { path, source } => write!(
                f,
                "token file unreadable at {}: {source} (run scripts/testd-install.ps1)",
                path.display()
            ),
            Self::TooShort { path, min } => write!(
                f,
                "token file at {} holds fewer than {min} characters — treating this as a \
                 broken install rather than a usable secret",
                path.display()
            ),
        }
    }
}

impl std::error::Error for TokenError {}

/// Read the token file. Surrounding whitespace (a trailing CRLF from PowerShell's `Set-Content`)
/// is trimmed; everything else is taken literally.
pub fn load(path: &Path) -> Result<String, TokenError> {
    let raw = std::fs::read_to_string(path).map_err(|source| TokenError::Unreadable {
        path: path.to_path_buf(),
        source,
    })?;
    let token = raw.trim().to_string();
    if token.chars().count() < MIN_TOKEN_CHARS {
        return Err(TokenError::TooShort { path: path.to_path_buf(), min: MIN_TOKEN_CHARS });
    }
    Ok(token)
}

/// Compare in time independent of the input.
///
/// Hashing both sides first means the byte-wise loop always runs over exactly 32 bytes, so
/// neither the token's length nor the position of the first differing byte is observable in the
/// response time. A naive `==` on the raw strings leaks both, and on a LAN the timing resolution
/// is more than good enough for that to matter.
pub fn matches(expected: &str, presented: &str) -> bool {
    let a = Sha256::digest(expected.as_bytes());
    let b = Sha256::digest(presented.as_bytes());
    let mut diff = 0u8;
    for i in 0..32 {
        diff |= a[i] ^ b[i];
    }
    // `black_box` stops the optimiser from turning the accumulate-then-test into an early exit.
    std::hint::black_box(diff) == 0
}

/// Pull the credential out of an `Authorization: Bearer <token>` header.
///
/// Header names arrive already lowercased from the HTTP parser. The scheme is matched
/// case-insensitively (RFC 7235 says it is case-insensitive); the token itself is not.
pub fn bearer_from_headers(headers: &[(String, String)]) -> Option<&str> {
    let value = headers.iter().find(|(name, _)| name == "authorization").map(|(_, v)| v)?;
    let rest = value.strip_prefix("Bearer ").or_else(|| {
        let (scheme, rest) = value.split_once(' ')?;
        scheme.eq_ignore_ascii_case("bearer").then_some(rest)
    })?;
    let rest = rest.trim();
    (!rest.is_empty()).then_some(rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn identical_tokens_match() {
        assert!(matches(GOOD, GOOD));
    }

    #[test]
    fn a_wrong_token_does_not_match() {
        let mut wrong = GOOD.to_string();
        wrong.replace_range(0..1, "1");
        assert!(!matches(GOOD, &wrong));
        // Differing only in the last character must fail exactly as hard as the first.
        let mut wrong_tail = GOOD.to_string();
        wrong_tail.replace_range(63..64, "0");
        assert!(!matches(GOOD, &wrong_tail));
    }

    #[test]
    fn a_prefix_of_the_token_does_not_match() {
        assert!(!matches(GOOD, &GOOD[..32]));
        assert!(!matches(GOOD, ""));
    }

    #[test]
    fn bearer_is_extracted_case_insensitively() {
        let h = vec![("authorization".to_string(), format!("Bearer {GOOD}"))];
        assert_eq!(bearer_from_headers(&h), Some(GOOD));
        let h = vec![("authorization".to_string(), format!("bearer {GOOD}"))];
        assert_eq!(bearer_from_headers(&h), Some(GOOD));
        let h = vec![("authorization".to_string(), format!("BEARER {GOOD}"))];
        assert_eq!(bearer_from_headers(&h), Some(GOOD));
    }

    #[test]
    fn a_missing_or_malformed_authorization_yields_none() {
        assert_eq!(bearer_from_headers(&[]), None);
        let h = vec![("authorization".to_string(), "Basic abcdef".to_string())];
        assert_eq!(bearer_from_headers(&h), None);
        let h = vec![("authorization".to_string(), "Bearer   ".to_string())];
        assert_eq!(bearer_from_headers(&h), None);
        let h = vec![("authorization".to_string(), "Bearer".to_string())];
        assert_eq!(bearer_from_headers(&h), None);
    }

    #[test]
    fn a_short_token_file_is_rejected() {
        let dir = std::env::temp_dir().join("evorift-testd-token-test");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("short.token");
        std::fs::write(&path, "abc\r\n").expect("write");
        assert!(matches!(load(&path), Err(TokenError::TooShort { .. })));
        std::fs::write(&path, format!("{GOOD}\r\n")).expect("write");
        assert_eq!(load(&path).expect("valid token loads"), GOOD);
        std::fs::remove_file(&path).expect("cleanup");
    }

    #[test]
    fn the_error_message_never_contains_the_token() {
        let err = TokenError::TooShort { path: PathBuf::from("t"), min: 32 };
        let rendered = err.to_string();
        assert!(!rendered.contains(GOOD));
        assert!(!rendered.contains("abc"));
    }
}
