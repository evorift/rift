<#
.SYNOPSIS
    The real test: start evorift's engine, capture the machine while it is on, stop it again.
    Runs ON THE LAPTOP as a single agent job.

.DESCRIPTION
    This is the step that reproduces the reported failure -- protection on, all internet gone.

    EVERYTHING HAPPENS LOCALLY, ON PURPOSE. The controller starts this job and then goes away.
    If the engine cuts the network (the whole point), nothing here needs the network to keep
    going: captures are written straight to disk and collected afterwards. Orchestrating the
    start/capture/stop from the controller would mean losing control of the laptop at exactly
    the moment the interesting thing happens.

    The engine is driven through evorift's OWN entry points -- evorift-svc for the privileged
    side and evorift-ctl to turn protection on and off. The winws command line is intricate and
    lives in engine.rs; rebuilding it here in PowerShell would drift from the product the first
    time a flag changed, and the copy that drifts is the one that runs during the test.

    SAFETY. Three independent brakes, because this deliberately breaks the machine's networking:
      * the engine is stopped by this script in a finally block, so a mid-script error still
        tears it down;
      * the agent's job timeout kills this script if it hangs;
      * the agent's deadman switch kills evorift/winws if the controller loses contact entirely.

.PARAMETER HoldSeconds
    How long protection stays on before the 'during' capture is taken. Kept short by default:
    every second here is a second the laptop may be unreachable.

.PARAMETER SkipCaptures
    Start and stop the engine without running the state captures. Useful for a quick smoke test.
#>
[CmdletBinding()]
param(
    [int]$HoldSeconds = 15,
    [switch]$SkipCaptures
)

$ErrorActionPreference = 'Continue'

# The agent's working directory can be a verbatim path; normalise it once (see capture-state.ps1).
$root = (Get-Location).Path
if ($root.StartsWith('\\?\')) { $root = $root.Substring(4) }
$root = $root.TrimEnd('\')

$engineDir = "$root\engine"
$svcExe    = "$engineDir\evorift-svc.exe"
$ctlExe    = "$engineDir\evorift-ctl.exe"
$winwsExe  = "$engineDir\winws\winws.exe"
$captureP  = "$root\scripts\capture-state.ps1"

$summary = [ordered]@{
    started_at   = (Get-Date -Format "yyyy-MM-ddTHH:mm:ss")
    hold_seconds = $HoldSeconds
    steps        = @()
    engine_ran   = $false
    winws_seen   = $false
    ok           = $false
}
function Step { param([string]$Name, [bool]$Ok, [string]$Detail)
    $script:summary.steps += [ordered]@{ name = $Name; ok = $Ok; detail = $Detail }
    $mark = if ($Ok) { "ok   " } else { "FAIL " }
    Write-Host "  $mark $Name -- $Detail"
}

function Save-Summary {
    $summary.finished_at = (Get-Date -Format "yyyy-MM-ddTHH:mm:ss")
    $path = "$root\engine-test-result.json"
    [System.IO.File]::WriteAllText($path, ($summary | ConvertTo-Json -Depth 6),
        (New-Object System.Text.UTF8Encoding($false)))
    Write-Host ""
    Write-Host "Result written to $path"
}

function Invoke-Capture {
    param([string]$Label)
    if ($SkipCaptures) { Step "capture:$Label" $true "skipped (-SkipCaptures)"; return }
    if (-not (Test-Path -LiteralPath $captureP)) {
        Step "capture:$Label" $false "capture-state.ps1 not found at $captureP"
        return
    }
    & powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $captureP -Label $Label 2>&1 |
        ForEach-Object { Write-Host "      $_" }
    Step "capture:$Label" ($LASTEXITCODE -eq 0) "capture-state.ps1 exited $LASTEXITCODE"
}

Write-Host "=== evorift ENGINE TEST ==="
Write-Host "root: $root"
Write-Host ""

# --- 0: preflight. Missing files here are the difference between a real test and a no-op. -------
Write-Host "--- preflight ---"
$missing = @()
foreach ($f in @($svcExe, $ctlExe, $winwsExe)) {
    if (Test-Path -LiteralPath $f) { Step "present" $true $f }
    else { Step "present" $false "MISSING $f"; $missing += $f }
}
if ($missing.Count -gt 0) {
    Step "preflight" $false "$($missing.Count) required file(s) missing - the engine cannot run"
    Save-Summary
    exit 1
}
$elevated = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
Step "elevated" $elevated "running elevated = $elevated (WinDivert needs this)"

$svcProc = $null
try {
    # --- 1: before -------------------------------------------------------------------------------
    Write-Host ""
    Write-Host "--- capture BEFORE ---"
    Invoke-Capture -Label "before"

    # --- 2: start the privileged service in the foreground ---------------------------------------
    Write-Host ""
    Write-Host "--- starting the engine ---"
    # --console keeps it in this process tree so it can be torn down reliably. EVORIFT_PRIVILEGED
    # is what tells sys.rs to run real OS commands instead of simulating them.
    $env:EVORIFT_PRIVILEGED = "1"
    $svcProc = Start-Process -FilePath $svcExe -ArgumentList "--console" -PassThru -WindowStyle Hidden
    Start-Sleep -Seconds 4
    if ($svcProc.HasExited) {
        Step "evorift-svc" $false "exited immediately with code $($svcProc.ExitCode)"
        Save-Summary
        exit 1
    }
    Step "evorift-svc" $true "running as pid $($svcProc.Id)"

    # --- 3: protection ON ---------------------------------------------------------------------------
    $onOut = & $ctlExe on 2>&1 | Out-String
    $onOk = ($LASTEXITCODE -eq 0)
    Step "evorift-ctl on" $onOk (($onOut -replace '\s+', ' ').Trim())
    $summary.engine_ran = $onOk

    # winws is the process that actually carries the DPI bypass. Measure it, do not assume it.
    Start-Sleep -Seconds 3
    $winws = Get-Process -Name "winws" -ErrorAction SilentlyContinue
    $summary.winws_seen = [bool]$winws
    if ($winws) { Step "winws running" $true "pid $($winws.Id -join ', ')" }
    else { Step "winws running" $false "winws.exe is NOT running - no bypass is active, so the 'during' capture will look like 'before'" }

    Write-Host ""
    Write-Host "--- protection is ON, holding ${HoldSeconds}s ---"
    Write-Host "    (the network may be dead right now - that is what we are here to measure)"
    Start-Sleep -Seconds $HoldSeconds

    # --- 4: during ------------------------------------------------------------------------------------
    Write-Host ""
    Write-Host "--- capture DURING ---"
    Invoke-Capture -Label "during"
}
finally {
    # --- 5: stop, whatever happened above ---------------------------------------------------------------
    Write-Host ""
    Write-Host "--- stopping the engine ---"

    if (Test-Path -LiteralPath $ctlExe) {
        $offOut = & $ctlExe off 2>&1 | Out-String
        Step "evorift-ctl off" ($LASTEXITCODE -eq 0) (($offOut -replace '\s+', ' ').Trim())
    }

    if ($svcProc -and -not $svcProc.HasExited) {
        Stop-Process -Id $svcProc.Id -Force -ErrorAction SilentlyContinue
        Step "evorift-svc stopped" $true "pid $($svcProc.Id) killed"
    }

    # Belt and braces: winws must not outlive this test holding the network down.
    $stragglers = Get-Process -Name "winws" -ErrorAction SilentlyContinue
    if ($stragglers) {
        $stragglers | Stop-Process -Force -ErrorAction SilentlyContinue
        Step "winws stopped" $true "killed $($stragglers.Count) leftover winws process(es)"
    } else {
        Step "winws stopped" $true "no winws process left"
    }

    Start-Sleep -Seconds 3
}

# --- 6: after ------------------------------------------------------------------------------------------
Write-Host ""
Write-Host "--- capture AFTER ---"
Invoke-Capture -Label "after"

# --- 7: did the network come back? Measured, because that is the whole question. ------------------------
Write-Host ""
Write-Host "--- recovery check ---"
$ping = $false
try {
    $ping = (New-Object System.Net.NetworkInformation.Ping).Send("1.1.1.1", 2000).Status -eq 'Success'
} catch { }
Step "ping 1.1.1.1 after stop" $ping "reachable = $ping"

$dns = $false
try {
    $null = Resolve-DnsName -Name "google.com" -Type A -ErrorAction Stop
    $dns = $true
} catch { }
Step "dns after stop" $dns "resolves = $dns"

$summary.ok = ($summary.engine_ran -and $ping)
Save-Summary

Write-Host ""
if ($summary.winws_seen) {
    Write-Host "ENGINE RAN. Compare docs/captures/*-before, *-during and *-after."
} else {
    Write-Host "WARNING: winws never started, so the 'during' capture does NOT show a bypass."
}
exit 0
