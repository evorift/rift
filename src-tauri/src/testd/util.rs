//! Small dependency-free helpers shared by the test agent.
//!
//! Everything here must work with ZERO internet and zero extra crates — the agent's whole reason
//! to exist is to keep functioning on a laptop whose network stack the software under test has
//! just killed. That rules out a time crate (chrono is not a dependency of this workspace), so
//! the UTC formatting below is done by hand.

use std::time::{SystemTime, UNIX_EPOCH};

/// Seconds since the Unix epoch. Returns 0 if the system clock is set before 1970 — a nonsense
/// clock is a logging cosmetic here, never a reason to fail a recovery.
pub fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Civil date from a day count relative to 1970-01-01 (Howard Hinnant's `civil_from_days`).
/// Returns `(year, month, day)`. All arithmetic is signed so pre-epoch days cannot underflow.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11], March-based
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

/// Format epoch seconds as `YYYY-MM-DDTHH:MM:SSZ` (UTC). Used for audit lines, capture directory
/// names and job timestamps, so the controller and the laptop agree on ordering without a
/// timezone argument between them.
pub fn iso8601_utc(secs: u64) -> String {
    let secs = secs as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, mo, d) = civil_from_days(days);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Same instant, formatted for use inside a file name (`YYYYMMDD-HHMMSS`) — `:` is not a legal
/// Windows path character, so the ISO form cannot be used there.
pub fn stamp_for_filename(secs: u64) -> String {
    let secs = secs as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, mo, d) = civil_from_days(days);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    format!("{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}")
}

/// Make an attacker-supplied string safe to write into a single audit-log line.
///
/// Two distinct threats, both real for a log that is meant to be evidence:
/// newline injection (forging extra audit records) and control characters (terminal escape
/// sequences that rewrite what a human sees when they `cat` the log). Everything outside
/// printable ASCII becomes `?`, and the result is length-capped.
pub fn sanitize_for_log(s: &str, max: usize) -> String {
    let mut out = String::with_capacity(s.len().min(max));
    for ch in s.chars() {
        if out.chars().count() >= max {
            out.push_str("...");
            break;
        }
        // Printable ASCII only. Non-ASCII is transliterated rather than dropped so the
        // presence of unusual input is still visible in the log.
        if ch.is_ascii_graphic() || ch == ' ' {
            out.push(ch);
        } else {
            out.push('?');
        }
    }
    if out.is_empty() {
        out.push('-');
    }
    out
}

/// Lowercase hex of a byte slice (job ids, SHA-256 digests).
pub fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

/// Escape a string for embedding in a JSON string literal. The agent hand-builds a few small
/// JSON responses (serde_json is available and used for parsing, but responses assembled here
/// stay allocation-light and never fail).
pub fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_zero_is_unix_day_one() {
        assert_eq!(iso8601_utc(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn known_instants_round_trip() {
        // Independently checkable reference points, not values read back out of this function.
        assert_eq!(iso8601_utc(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(iso8601_utc(946_684_800), "2000-01-01T00:00:00Z");
        // 2024 is a leap year: day 60 of the year must be Feb 29, not Mar 1.
        assert_eq!(iso8601_utc(1_709_164_800), "2024-02-29T00:00:00Z");
    }

    #[test]
    fn filename_stamp_has_no_colons() {
        let s = stamp_for_filename(1_700_000_000);
        assert_eq!(s, "20231114-221320");
        assert!(!s.contains(':'));
    }

    #[test]
    fn sanitize_strips_newlines_so_a_log_line_cannot_be_forged() {
        let forged = "ok\n2020-01-01T00:00:00Z ip=1.2.3.4 endpoint=/run status=200";
        let clean = sanitize_for_log(forged, 200);
        assert!(!clean.contains('\n'));
        assert!(clean.starts_with("ok?"));
    }

    #[test]
    fn sanitize_strips_terminal_escapes_and_caps_length() {
        assert_eq!(sanitize_for_log("a\x1b[31mred", 100), "a?[31mred");
        assert_eq!(sanitize_for_log(&"x".repeat(50), 10), "xxxxxxxxxx...");
        assert_eq!(sanitize_for_log("", 10), "-");
    }

    #[test]
    fn hex_is_lowercase_and_padded() {
        assert_eq!(hex(&[0x00, 0x0f, 0xa0, 0xff]), "000fa0ff");
    }

    #[test]
    fn json_escape_handles_quotes_and_controls() {
        assert_eq!(json_escape("a\"b\\c"), "a\\\"b\\\\c");
        assert_eq!(json_escape("l1\nl2"), "l1\\nl2");
        assert_eq!(json_escape("\u{1}"), "\\u0001");
    }
}
