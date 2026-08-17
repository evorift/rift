//! At-rest protection for the small amount of user data this app has to keep on disk.
//!
//! ## What is actually private here
//!
//! Almost everything evorift stores is impersonal: the wide hostlist is a SHIPPED CONSTANT, byte
//! for byte identical on every install, so it says nothing about who is running it. The tuning
//! result is a /24 network fingerprint, a chain id and per-candidate counts — no site names at all
//! (the domain list was deliberately removed from it rather than encrypted, see `tuner::Tuning`).
//!
//! What IS private is the small set of domains the user added themselves. That is the data this
//! module protects.
//!
//! ## What DPAPI here does and does not buy
//!
//! `CryptProtectData` binds the ciphertext to this machine and to the account that wrote it (the
//! `EvoriftSvc` service, i.e. LocalSystem — the service is the only writer; the UI reaches this
//! data over IPC and never touches the file).
//!
//! It PROTECTS against: the file being copied off the machine, a stolen or imaged disk, a backup
//! being read elsewhere, another (non-admin) account on this machine opening the file.
//!
//! It does NOT protect against: anyone with administrator rights on this machine, who can run code
//! as LocalSystem and therefore ask DPAPI to decrypt it exactly as we do. That is not a flaw to be
//! engineered around — the app itself must be able to read this data unattended, at boot, with no
//! user present, so any key it can reach an attacker with the same privileges can reach too.
//!
//! Nothing here is unbreakable, and no comment, document or UI string in this project may say that
//! it is.

/// Encrypt `plain` for this machine + this account. `None` if DPAPI refuses.
#[cfg(windows)]
pub fn protect(plain: &[u8]) -> Option<Vec<u8>> {
    use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CRYPT_INTEGER_BLOB};
    unsafe {
        let in_blob = CRYPT_INTEGER_BLOB { cbData: plain.len() as u32, pbData: plain.as_ptr() as *mut u8 };
        let mut entropy_bytes = ENTROPY.to_vec();
        let entropy = CRYPT_INTEGER_BLOB {
            cbData: entropy_bytes.len() as u32,
            pbData: entropy_bytes.as_mut_ptr(),
        };
        let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        let ok = CryptProtectData(
            &in_blob,
            std::ptr::null(),
            &entropy,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
            &mut out,
        );
        if ok == 0 || out.pbData.is_null() {
            return None;
        }
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        free_blob(out.pbData);
        Some(v)
    }
}

/// Decrypt what `protect` produced. `None` when the blob is absent, corrupt, or was written by a
/// different machine or account — all of which mean "we have no usable stored data", never "trust
/// it anyway".
#[cfg(windows)]
pub fn unprotect(blob: &[u8]) -> Option<Vec<u8>> {
    use windows_sys::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};
    unsafe {
        let in_blob = CRYPT_INTEGER_BLOB { cbData: blob.len() as u32, pbData: blob.as_ptr() as *mut u8 };
        let mut entropy_bytes = ENTROPY.to_vec();
        let entropy = CRYPT_INTEGER_BLOB {
            cbData: entropy_bytes.len() as u32,
            pbData: entropy_bytes.as_mut_ptr(),
        };
        let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        let ok = CryptUnprotectData(
            &in_blob,
            std::ptr::null_mut(),
            &entropy,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
            &mut out,
        );
        if ok == 0 || out.pbData.is_null() {
            return None;
        }
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        free_blob(out.pbData);
        Some(v)
    }
}

/// Application-specific secondary entropy.
///
/// Not a secret — it ships in the binary, and treating it as one would be exactly the kind of claim
/// this module refuses to make. Its only job is to scope the ciphertext to THIS application, so a
/// blob taken from evorift cannot be fed to some other DPAPI consumer running under the same
/// account, and vice versa.
#[cfg(windows)]
const ENTROPY: &[u8] = b"evorift/local-store/v1";

#[cfg(windows)]
unsafe fn free_blob(p: *mut u8) {
    use windows_sys::Win32::Foundation::{LocalFree, HLOCAL};
    LocalFree(p as HLOCAL);
}

#[cfg(not(windows))]
pub fn protect(plain: &[u8]) -> Option<Vec<u8>> {
    // No DPAPI outside Windows. Returning None means callers fall back to "cannot store this
    // securely", which is the honest answer — never a silent plaintext write.
    let _ = plain;
    None
}

#[cfg(not(windows))]
pub fn unprotect(blob: &[u8]) -> Option<Vec<u8>> {
    let _ = blob;
    None
}

/// Restrict a file so only SYSTEM and Administrators can read it.
///
/// This is the answer for data that CANNOT be encrypted: `hostlist.txt` is read from disk by
/// `winws.exe`, a separate bundled binary, so encrypting it would simply stop the engine working.
/// Narrowing the ACL is the protection that remains available.
///
/// PROTECTS against: another standard user account on the same machine reading which domains this
/// user asked to unblock.
/// DOES NOT protect against: an administrator, who can take ownership and rewrite the ACL — the
/// same limitation as the encryption above, for the same unavoidable reason.
///
/// Best-effort: a failure is logged, never fatal. Losing protection on a file is bad; refusing to
/// protect the user's connection because a DACL could not be set is worse.
#[cfg(windows)]
pub fn harden_acl(path: &std::path::Path) {
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

    if !path.exists() {
        return;
    }
    // D:PAI — protected (P) so inherited permissive entries from ProgramData are NOT merged in,
    // which is the whole point: ProgramData grants Users write access by default.
    // (A;;FA;;;SY) SYSTEM full, (A;;FA;;;BA) Administrators full. No entry for Users at all.
    let sddl: Vec<u16> = "D:PAI(A;;FA;;;SY)(A;;FA;;;BA)\0".encode_utf16().collect();
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
            crate::elog::warn("secure", "sddl_parse", "could not build the restrictive DACL");
            return;
        }
        let mut present: BOOL = 0;
        let mut defaulted: BOOL = 0;
        let mut dacl: *mut ACL = std::ptr::null_mut();
        if GetSecurityDescriptorDacl(psd, &mut present, &mut dacl, &mut defaulted) != 0 && present != 0 {
            let rc = SetNamedSecurityInfoW(
                wpath.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                dacl,
                std::ptr::null(),
            );
            if rc != 0 {
                // Expected when running unelevated in dev — the file is simply not ours to re-ACL.
                crate::elog::warn(
                    "secure",
                    "acl_failed",
                    &format!("file permissions could not be restricted (Win32 {rc})"),
                );
            }
        }
        LocalFree(psd as HLOCAL);
    }
}

#[cfg(not(windows))]
pub fn harden_acl(_path: &std::path::Path) {}

/// Write `bytes` to `path` encrypted, with a restrictive ACL, atomically.
///
/// Returns an error rather than falling back to a plaintext write: a store that silently degrades
/// to plaintext when encryption fails is worse than one that says it failed, because nobody would
/// ever find out.
pub fn write_encrypted(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    let blob = protect(bytes).ok_or_else(|| "the local store could not be encrypted".to_string())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("data dir: {e}"))?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, &blob).map_err(|e| format!("store write: {e}"))?;
    // Rename FIRST, harden SECOND.
    //
    // The obvious order — ACL the temp file so the final path is never briefly permissive — locks
    // the writing process out of its own file: the restrictive DACL grants SYSTEM and
    // Administrators only, and `rename` then fails with Access Denied for anyone else (caught by
    // the round-trip test, which ran unelevated).
    //
    // Doing it this way leaves a window of microseconds in which the file exists with inherited
    // permissions — and its CONTENTS ARE ALREADY ENCRYPTED throughout, so what is briefly readable
    // is a DPAPI blob. The ACL here is defence in depth over encryption, not the thing holding the
    // line, so trading a microsecond of exposed ciphertext for a store that actually works is the
    // right way round.
    std::fs::rename(&tmp, path).map_err(|e| format!("store commit: {e}"))?;
    harden_acl(path);
    Ok(())
}

/// Read a file written by `write_encrypted`. `None` for absent, unreadable or undecryptable.
pub fn read_encrypted(path: &std::path::Path) -> Option<Vec<u8>> {
    let blob = std::fs::read(path).ok()?;
    unprotect(&blob)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The store must survive a real round trip through the OS, not just through our own code.
    #[cfg(windows)]
    #[test]
    fn dpapi_round_trips() {
        let secret = b"example-one.com\nexample-two.net";
        let blob = protect(secret).expect("DPAPI should be available on Windows");
        assert_ne!(&blob[..], &secret[..], "the stored bytes must not be the plaintext");
        assert!(
            !String::from_utf8_lossy(&blob).contains("example-one"),
            "a domain is readable in the encrypted blob"
        );
        assert_eq!(unprotect(&blob).as_deref(), Some(&secret[..]));
    }

    /// A corrupt or foreign blob must read as "no data", never as partial data.
    #[cfg(windows)]
    #[test]
    fn a_corrupt_blob_reads_as_nothing() {
        let mut blob = protect(b"example.com").expect("protect");
        let n = blob.len();
        blob[n / 2] ^= 0xFF;
        assert_eq!(unprotect(&blob), None);
        assert_eq!(unprotect(b"not a dpapi blob at all"), None);
    }

    /// End-to-end: what lands on disk is a blob, and the ACL is real.
    ///
    /// This test has TWO correct outcomes, because it asserts on both protections at once:
    ///
    ///   * running privileged (SYSTEM or elevated admin — how the service actually runs), the file
    ///     is readable and must contain ciphertext, and must round-trip; or
    ///   * running unprivileged (a plain `cargo test`), the read is DENIED — which is the ACL doing
    ///     precisely its job, and is therefore a pass, not a skip.
    ///
    /// What is NOT acceptable is a third outcome: a readable file containing the domain.
    #[cfg(windows)]
    #[test]
    fn the_stored_file_is_encrypted_and_access_restricted() {
        let p = std::env::temp_dir().join(format!("evorift-store-test-{}.bin", std::process::id()));
        let _ = std::fs::remove_file(&p);
        write_encrypted(&p, b"secret-site.com").expect("write");

        match std::fs::read(&p) {
            Ok(raw) => {
                assert!(
                    !String::from_utf8_lossy(&raw).contains("secret-site.com"),
                    "the domain is sitting in the file in plaintext"
                );
                assert_eq!(
                    read_encrypted(&p).as_deref(),
                    Some(&b"secret-site.com"[..]),
                    "a privileged reader must still be able to use the store"
                );
            }
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                // The restrictive DACL locked this unelevated process out of a file it just wrote.
                // That is the second protection working; the "never plaintext" property itself is
                // proven independently by `dpapi_round_trips`.
            }
            Err(e) => panic!("unexpected error reading the store back: {e}"),
        }
        // Cleanup may itself be denied by the ACL we just set — that is fine, it is a temp file.
        let _ = std::fs::remove_file(&p);
    }
}
