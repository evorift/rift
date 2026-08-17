//! Paylaşılan süreç yardımcıları (motor adaptörleri ortak kullanır). Konsol penceresi açmadan spawn,
//! kill-on-close Job Object (servis ölünce kernel çocukları öldürür → yetim "virüs kalıntısı" yok),
//! ve image-adıyla toplu öldürme. winws Job mantığının genelleştirilmiş hâli (V0.1.3 plan §c.2).

#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Belirli bir image adındaki TÜM süreçleri öldür (ör. "ciadpi.exe", "goodbyedpi.exe") — tek instance
/// garantisi + yetim temizliği. Best-effort + sessiz (pencere yok).
#[cfg(windows)]
pub fn kill_image(name: &str) {
    use std::os::windows::process::CommandExt;
    let _ = std::process::Command::new("taskkill")
        .args(["/f", "/im", name])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
}
#[cfg(not(windows))]
pub fn kill_image(_name: &str) {}

/// Is any process with this image name running? Sub-millisecond, fully in-process.
///
/// Exists so the hot start path stops paying for a `taskkill` spawn (~100ms of process creation)
/// just to discover there was nothing to kill. Enumerating the snapshot and comparing names is
/// cheaper than creating one process, and it is the check `taskkill` would do anyway.
#[cfg(windows)]
pub fn image_running(name: &str) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            // Cannot tell → assume it might be running, so the caller still does its cleanup.
            // Guessing "no" here would skip a kill that was actually needed.
            return true;
        }
        let mut e: PROCESSENTRY32W = std::mem::zeroed();
        e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut found = false;
        if Process32FirstW(snap, &mut e) != 0 {
            loop {
                let end = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
                let exe = String::from_utf16_lossy(&e.szExeFile[..end]);
                if exe.eq_ignore_ascii_case(name) {
                    found = true;
                    break;
                }
                if Process32NextW(snap, &mut e) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snap);
        found
    }
}
#[cfg(not(windows))]
pub fn image_running(_name: &str) -> bool {
    false
}

/// Is this PID still alive? Used by `is_running()` so engine state is MEASURED rather than
/// remembered (CLAUDE.md rule 10) without needing `&mut` access to the `Child`.
///
/// A process that exited but has not been reaped is a zombie whose handle still opens; on Windows
/// `GetExitCodeProcess` distinguishes the two via STILL_ACTIVE, which is what this checks.
#[cfg(windows)]
pub fn pid_alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    const STILL_ACTIVE: u32 = 259;
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h == 0 {
            return false; // gone, or not ours to query — either way not a live engine we own
        }
        let mut code: u32 = 0;
        let ok = GetExitCodeProcess(h, &mut code);
        CloseHandle(h);
        ok != 0 && code == STILL_ACTIVE
    }
}
#[cfg(not(windows))]
pub fn pid_alive(_pid: u32) -> bool {
    false
}

/// KILL_ON_JOB_CLOSE bayraklı bir Job Object oluştur (handle isize; 0 = başarısız). Son handle kapanınca
/// kernel atanan tüm süreçleri öldürür. [`assign_to_job`] ile çocuk atanır, [`close_job`] ile kapatılır.
#[cfg(windows)]
pub fn create_kill_on_close_job() -> isize {
    use windows_sys::Win32::System::JobObjects::{
        CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    unsafe {
        let h = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if h == 0 {
            return 0;
        }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            h,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        h
    }
}

/// Doğan süreci kill-on-close job'a ata (best-effort).
#[cfg(windows)]
pub fn assign_to_job(job: isize, child: &std::process::Child) {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
    if job == 0 {
        return;
    }
    unsafe {
        AssignProcessToJobObject(job as _, child.as_raw_handle() as _);
    }
}

/// Job handle'ını kapat (Drop'ta çağrılır → KILL_ON_JOB_CLOSE atanan süreçleri temizler).
#[cfg(windows)]
pub fn close_job(job: isize) {
    if job != 0 {
        unsafe { windows_sys::Win32::Foundation::CloseHandle(job as _) };
    }
}

/// Spawn `cmd`, create a KILL_ON_JOB_CLOSE job, assign the child to it, return `(child, job)`.
/// One-liner for any adapter that spawns a long-running helper process — no duplicate boilerplate.
/// The job handle must be closed by the caller (via `close_job`) when the engine is dropped.
/// Job handle is 0 if job creation fails (assignment is then a no-op; process still starts).
#[cfg(windows)]
pub fn spawn_with_job(
    mut cmd: std::process::Command,
) -> Result<(std::process::Child, isize), String> {
    let child = cmd.spawn().map_err(|e| format!("spawn failed: {e}"))?;
    let job = create_kill_on_close_job();
    assign_to_job(job, &child);
    Ok((child, job))
}

#[cfg(not(windows))]
pub fn spawn_with_job(
    mut cmd: std::process::Command,
) -> Result<(std::process::Child, isize), String> {
    let child = cmd.spawn().map_err(|e| format!("spawn failed: {e}"))?;
    Ok((child, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Item 10.1: spawn_with_job returns a live child and a non-zero job handle on Windows.
    #[cfg(windows)]
    #[test]
    fn spawn_with_job_assigns_to_job() {
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new("cmd");
        cmd.args(["/c", "exit", "0"]).creation_flags(CREATE_NO_WINDOW);
        let (mut child, job) = spawn_with_job(cmd).expect("spawn");
        assert!(job != 0, "job handle must be non-zero on Windows");
        // Verify via IsProcessInJob API.
        #[allow(non_snake_case)]
        unsafe {
            use std::os::windows::io::AsRawHandle;
            let mut in_job: windows_sys::Win32::Foundation::BOOL = 0;
            let ok = windows_sys::Win32::System::JobObjects::IsProcessInJob(
                child.as_raw_handle() as _,
                job as _,
                &mut in_job,
            );
            assert!(ok != 0, "IsProcessInJob API call must succeed");
            assert_eq!(in_job, 1, "child must be reported as inside the job");
        }
        let _ = child.wait();
        close_job(job);
    }
}
