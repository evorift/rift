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
