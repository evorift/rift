//! Scheduled-task helper (docs/03 §1.5 — the WireSockRefresh pattern). An opt-in task that periodically
//! restarts a tunnel service so flaky WireSock/WireGuard tunnels self-heal. Created via PowerShell
//! `Register-ScheduledTask` (SYSTEM / Highest). Privileged → `sys` (sim when unprivileged).

/// Our scheduled-task name (fixed; not user input → safe to embed in scripts).
pub const REFRESH_TASK_NAME: &str = "EvoriftTunnelRefresh";

/// Build the `Register-ScheduledTask` PowerShell script (pure → testable). Restarts the service named by
/// `$env:EVORIFT_REFRESH_SVC` every `interval_min` minutes, SYSTEM/Highest, repeating for ~10 years
/// (docs/03 §1.5). The service name goes through `$env` (no command injection); the task name + interval
/// are our own constants/number (safe to embed).
pub fn register_script(interval_min: u32) -> String {
    format!(
        "$ErrorActionPreference='Stop'; \
         $svc=$env:EVORIFT_REFRESH_SVC; \
         $cmd='sc.exe stop ' + $svc + ' & timeout /t 5 /nobreak & sc.exe start ' + $svc; \
         $a=New-ScheduledTaskAction -Execute 'cmd.exe' -Argument ('/c ' + $cmd); \
         $t=New-ScheduledTaskTrigger -Once -At (Get-Date) -RepetitionInterval (New-TimeSpan -Minutes {interval_min}) -RepetitionDuration (New-TimeSpan -Days 3650); \
         $p=New-ScheduledTaskPrincipal -UserId 'SYSTEM' -RunLevel Highest; \
         Register-ScheduledTask -TaskName '{REFRESH_TASK_NAME}' -Action $a -Trigger $t -Principal $p -Force | Out-Null",
        interval_min = interval_min,
        REFRESH_TASK_NAME = REFRESH_TASK_NAME,
    )
}

/// Create the refresh task (opt-in). `service` = the tunnel service to restart (e.g.
/// `wiresock-client-service`); `interval_min` = cadence. Privileged; sim when unprivileged.
pub fn create_refresh_task(service: &str, interval_min: u32) -> Result<(), String> {
    let script = register_script(interval_min);
    crate::sys::run_os_env(
        "powershell",
        &["-NoProfile", "-Command", &script],
        &[("EVORIFT_REFRESH_SVC", service)],
    )?;
    // Track for rollback (item 7.3) — only when privileged (the task is really registered then).
    if crate::sys::privileged() {
        crate::rollback::record(crate::rollback::Change::ScheduledTask { name: REFRESH_TASK_NAME.to_string() });
    }
    Ok(())
}

/// Delete the refresh task (best-effort).
pub fn delete_task() -> Result<(), String> {
    let script = format!(
        "Unregister-ScheduledTask -TaskName '{REFRESH_TASK_NAME}' -Confirm:$false -ErrorAction SilentlyContinue"
    );
    crate::sys::run_os("powershell", &["-NoProfile", "-Command", &script])
}

/// Does the refresh task exist? (read-only; works unprivileged).
pub fn task_exists() -> bool {
    let script = format!(
        "if (Get-ScheduledTask -TaskName '{REFRESH_TASK_NAME}' -ErrorAction SilentlyContinue) {{ 'yes' }}"
    );
    crate::sys::query_os("powershell", &["-NoProfile", "-Command", &script]).contains("yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Item 4.5: the Register-ScheduledTask script restarts the env-named service on the given cadence,
    /// SYSTEM/Highest, and never injects the service name into the command line literally.
    #[test]
    fn register_script_content() {
        let s = register_script(3);
        assert!(s.contains("Register-ScheduledTask"));
        assert!(s.contains("New-TimeSpan -Minutes 3"), "cadence embedded");
        assert!(s.contains("'SYSTEM'") && s.contains("Highest"), "runs as SYSTEM/Highest");
        assert!(s.contains("EvoriftTunnelRefresh"));
        assert!(s.contains("$env:EVORIFT_REFRESH_SVC"), "service via env (no injection)");
    }

    /// create/delete are clean no-ops in dev (unprivileged → sim).
    #[test]
    fn create_delete_sim() {
        assert!(create_refresh_task("wiresock-client-service", 3).is_ok());
        assert!(delete_task().is_ok());
    }
}
