<#
.SYNOPSIS
    TEST 3 (toggle downtime) -- MVP acceptance test. Runs ON THE LAPTOP as a single agent job.

.DESCRIPTION
    Measures how long the network is actually down across off->on (evorift-ctl on) and
    on->off (evorift-ctl off), using a continuous local ICMP probe at ~100ms intervals
    logged to disk with absolute UTC timestamps -- the gap is read off that timeline after
    the fact, never inferred from before/after snapshots.

    Design, same as run-engine-test.ps1: everything happens locally. The probe runs in a
    separate background job (its own process) so it is never blocked by evorift-ctl calls
    or Start-Sleep in the main thread, and it writes to disk continuously (AutoFlush) so a
    killed process still leaves a usable partial log. The engine is driven only through
    evorift-ctl -- never a hand-built winws command line.

    Protection is forced to mode=dpi (WARP explicitly off) so this measures the DPI engine's
    own toggle behavior in isolation, not conflated with a WireGuard tunnel coming up/down.

.PARAMETER Reps
    How many off->on / on->off cycles to run. Default 3 -- report all three, not an average.

.PARAMETER HoldSeconds
    How long protection stays on between the off->on and on->off toggle of each rep.

.PARAMETER SettleSeconds
    Baseline settle time (protection off) before each rep's off->on toggle.

.PARAMETER ProbeTarget / ProbeTimeoutMs
    ICMP probe target and per-probe timeout. 1.1.1.1 is outside winws's filtered ports
    (TCP 80/443, UDP/443 QUIC, Discord/STUN UDP) and outside WARP's routed ranges in split
    mode, so it is a clean "is the machine's network path up at all" signal.
#>
[CmdletBinding()]
param(
    [int]$Reps = 3,
    [int]$HoldSeconds = 5,
    [int]$SettleSeconds = 3,
    [string]$ProbeTarget = "1.1.1.1",
    [int]$ProbeTimeoutMs = 200
)

$ErrorActionPreference = 'Continue'

function Iso8601Now {
    [DateTime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ss.fffZ", [System.Globalization.CultureInfo]::InvariantCulture)
}
function ParseIso8601 {
    param([string]$s)
    [DateTime]::Parse($s, [System.Globalization.CultureInfo]::InvariantCulture,
        [System.Globalization.DateTimeStyles]::AdjustToUniversal -bor [System.Globalization.DateTimeStyles]::AssumeUniversal)
}

# The agent's working directory can be a verbatim path (\\?\C:\evorift-test); normalise once.
$root = (Get-Location).Path
if ($root.StartsWith('\\?\')) { $root = $root.Substring(4) }
$root = $root.TrimEnd('\')

$engineDir  = "$root\engine"
$svcExe     = "$engineDir\evorift-svc.exe"
$ctlExe     = "$engineDir\evorift-ctl.exe"
$winwsExe   = "$engineDir\winws\winws.exe"
$probeLog   = "$root\test3-probe.csv"
$stopFlag   = "$root\test3-probe.stop"
$resultPath = "$root\test3-result.json"

$summary = [ordered]@{
    started_at       = Iso8601Now
    reps             = $Reps
    hold_seconds     = $HoldSeconds
    settle_seconds   = $SettleSeconds
    probe_target     = $ProbeTarget
    probe_timeout_ms = $ProbeTimeoutMs
    steps            = @()
    events           = @()
    gaps             = @()
    ok               = $false
}
function Step { param([string]$Name,[bool]$Ok,[string]$Detail)
    $script:summary.steps += [ordered]@{ name=$Name; ok=$Ok; detail=$Detail }
    Write-Host "  $(if($Ok){'ok   '}else{'FAIL '}) $Name -- $Detail"
}
function Save-Summary {
    $summary.finished_at = Iso8601Now
    [System.IO.File]::WriteAllText($resultPath, ($summary | ConvertTo-Json -Depth 8),
        (New-Object System.Text.UTF8Encoding($false)))
    Write-Host ""
    Write-Host "Result written to $resultPath"
}

Write-Host "=== TEST 3: toggle downtime ==="
Write-Host "root: $root"

# --- preflight ---
$missing = @()
foreach ($f in @($svcExe, $ctlExe, $winwsExe)) {
    if (Test-Path -LiteralPath $f) { Step "present" $true $f } else { Step "present" $false "MISSING $f"; $missing += $f }
}
if ($missing.Count -gt 0) {
    Step "preflight" $false "$($missing.Count) required file(s) missing"
    Save-Summary
    exit 1
}
$elevated = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
Step "elevated" $elevated "running elevated = $elevated"

# Exclusive-control guard (P0-e, BACKLOG.md): a pre-existing evorift-svc/winws process means
# unknown prior state is already live -- every gap measurement here requires exclusive control.
$preexisting = Get-Process -Name "evorift-svc", "winws" -ErrorAction SilentlyContinue
if ($preexisting) {
    $list = ($preexisting | ForEach-Object { "$($_.ProcessName) pid=$($_.Id) started=$($_.StartTime)" }) -join "; "
    Step "exclusive control" $false "evorift-svc/winws already running before this job started ($list) -- ABORTING"
    Save-Summary
    exit 1
}
Step "exclusive control" $true "no pre-existing evorift-svc/winws process found"

if (Test-Path -LiteralPath $stopFlag) { Remove-Item -LiteralPath $stopFlag -Force -ErrorAction SilentlyContinue }
if (Test-Path -LiteralPath $probeLog) { Remove-Item -LiteralPath $probeLog -Force -ErrorAction SilentlyContinue }
"ts_iso,elapsed_ms,status,rtt_ms" | Out-File -FilePath $probeLog -Encoding ascii

# --- probe: separate background job (its own process), writes disk-first, never blocked by the main thread ---
$probeScript = {
    param($ProbeLog, $Target, $TimeoutMs, $StopFlagPath)
    $writer = New-Object System.IO.StreamWriter($ProbeLog, $true, [System.Text.Encoding]::ASCII)
    $writer.AutoFlush = $true
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $ping = New-Object System.Net.NetworkInformation.Ping
    while (-not (Test-Path -LiteralPath $StopFlagPath)) {
        $tickStart = $sw.ElapsedMilliseconds
        $ts = [DateTime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ss.fffZ", [System.Globalization.CultureInfo]::InvariantCulture)
        try {
            $reply = $ping.Send($Target, $TimeoutMs)
            if ($reply.Status -eq 'Success') { $status = 'ok'; $rtt = $reply.RoundtripTime }
            else { $status = 'fail'; $rtt = -1 }
        } catch {
            $status = 'error'; $rtt = -1
        }
        $writer.WriteLine("$ts,$tickStart,$status,$rtt")
        $elapsed = $sw.ElapsedMilliseconds - $tickStart
        $sleepFor = 100 - $elapsed
        if ($sleepFor -gt 0) { Start-Sleep -Milliseconds $sleepFor }
    }
    $writer.Flush()
    $writer.Close()
}
$probeJob = Start-Job -ScriptBlock $probeScript -ArgumentList $probeLog, $ProbeTarget, $ProbeTimeoutMs, $stopFlag
Start-Sleep -Seconds 2
Step "probe started" $true "background job $($probeJob.Id) -> $probeLog"

$svcProc = $null
try {
    $env:EVORIFT_PRIVILEGED = "1"
    $svcProc = Start-Process -FilePath $svcExe -ArgumentList "--console" -PassThru -WindowStyle Hidden
    Start-Sleep -Seconds 4
    if ($svcProc.HasExited) {
        Step "evorift-svc" $false "exited immediately with code $($svcProc.ExitCode)"
        throw "svc exited"
    }
    Step "evorift-svc" $true "running as pid $($svcProc.Id)"

    # Force pure DPI-only (WARP explicitly off) so this measures the DPI engine's toggle alone.
    $modeOut = & $ctlExe mode dpi 2>&1 | Out-String
    Step "mode dpi" ($LASTEXITCODE -eq 0) (($modeOut -replace '\s+', ' ').Trim())

    for ($rep = 1; $rep -le $Reps; $rep++) {
        Write-Host ""
        Write-Host "--- rep $rep/${Reps}: settling ${SettleSeconds}s (protection off) ---"
        Start-Sleep -Seconds $SettleSeconds

        $issuedAt = Iso8601Now
        $onOut = & $ctlExe on 2>&1 | Out-String
        $onOk = ($LASTEXITCODE -eq 0)
        $returnedAt = Iso8601Now
        $cmdMs = [int]((ParseIso8601 $returnedAt) - (ParseIso8601 $issuedAt)).TotalMilliseconds
        $summary.events += [ordered]@{
            rep = $rep; direction = "off_to_on"; issued_at = $issuedAt; returned_at = $returnedAt
            cmd_ms = $cmdMs; ok = $onOk; output = (($onOut -replace '\s+', ' ').Trim())
        }
        Step "rep $rep off->on" $onOk "cmd took ${cmdMs}ms"

        Write-Host "--- rep ${rep}: holding protected ${HoldSeconds}s ---"
        Start-Sleep -Seconds $HoldSeconds

        $issuedAt2 = Iso8601Now
        $offOut = & $ctlExe off 2>&1 | Out-String
        $offOk = ($LASTEXITCODE -eq 0)
        $returnedAt2 = Iso8601Now
        $cmdMs2 = [int]((ParseIso8601 $returnedAt2) - (ParseIso8601 $issuedAt2)).TotalMilliseconds
        $summary.events += [ordered]@{
            rep = $rep; direction = "on_to_off"; issued_at = $issuedAt2; returned_at = $returnedAt2
            cmd_ms = $cmdMs2; ok = $offOk; output = (($offOut -replace '\s+', ' ').Trim())
        }
        Step "rep $rep on->off" $offOk "cmd took ${cmdMs2}ms"
    }
    Write-Host ""
    Write-Host "--- trailing settle ${SettleSeconds}s so the last toggle's recovery is fully on the timeline ---"
    Start-Sleep -Seconds $SettleSeconds
}
catch {
    Step "run" $false $_.Exception.Message
}
finally {
    Write-Host ""
    Write-Host "--- teardown ---"
    if (Test-Path -LiteralPath $ctlExe) {
        $off2 = & $ctlExe off 2>&1 | Out-String
        Step "final off" ($LASTEXITCODE -eq 0) (($off2 -replace '\s+', ' ').Trim())
    }
    if ($svcProc -and -not $svcProc.HasExited) {
        Stop-Process -Id $svcProc.Id -Force -ErrorAction SilentlyContinue
        Step "svc stopped" $true "pid $($svcProc.Id) killed"
    }
    $stragglers = Get-Process -Name "winws" -ErrorAction SilentlyContinue
    if ($stragglers) { $stragglers | Stop-Process -Force -ErrorAction SilentlyContinue; Step "winws stopped" $true "killed $($stragglers.Count)" }
    else { Step "winws stopped" $true "none left" }
    # Belt-and-braces: the agent-level deadman does not tear down WireGuardTunnel$warp (see
    # docs/REMOTE-TESTING.md known gap). mode=dpi never brought it up in this test, but clear it
    # defensively in case a prior run on this laptop left it behind.
    $svcW = Get-Service -Name "WireGuardTunnel`$warp" -ErrorAction SilentlyContinue
    if ($svcW) {
        & sc.exe stop "WireGuardTunnel`$warp" | Out-Null
        Start-Sleep -Seconds 1
        & sc.exe delete "WireGuardTunnel`$warp" | Out-Null
        Step "warp tunnel service cleared" $true "WireGuardTunnel`$warp stopped+deleted"
    }

    New-Item -ItemType File -Path $stopFlag -Force | Out-Null
    Start-Sleep -Seconds 1
    Wait-Job $probeJob -Timeout 5 | Out-Null
    Receive-Job $probeJob -ErrorAction SilentlyContinue | Out-Null
    Remove-Job $probeJob -Force -ErrorAction SilentlyContinue
    Step "probe stopped" $true "log at $probeLog"
}

# --- compute gaps from the probe timeline (absolute UTC timestamps on both sides -- the probe
#     job is a separate process, so its own stopwatch is NOT comparable to this thread's). ---
Write-Host ""
Write-Host "--- computing gaps from the probe timeline ---"
$rows = @()
if (Test-Path -LiteralPath $probeLog) {
    Get-Content -LiteralPath $probeLog | Select-Object -Skip 1 | ForEach-Object {
        $parts = $_ -split ','
        if ($parts.Count -ge 3 -and $parts[0]) {
            try { $rows += [ordered]@{ ts = (ParseIso8601 $parts[0]); status = $parts[2] } } catch { }
        }
    }
}
$rows = $rows | Sort-Object { $_.ts }
Step "probe rows parsed" ($rows.Count -gt 0) "$($rows.Count) row(s)"

foreach ($evt in $summary.events) {
    $cmdTs = ParseIso8601 $evt.issued_at
    $before = $rows | Where-Object { $_.ts -le $cmdTs -and $_.status -eq 'ok' } | Select-Object -Last 1
    $windowEnd = $cmdTs.AddSeconds(15)
    $after = $rows | Where-Object { $_.ts -gt $cmdTs -and $_.ts -le $windowEnd }
    $firstFailAfter = $after | Where-Object { $_.status -ne 'ok' } | Select-Object -First 1
    $firstOkAfterFail = $null
    if ($firstFailAfter) {
        $firstOkAfterFail = $after | Where-Object { $_.ts -gt $firstFailAfter.ts -and $_.status -eq 'ok' } | Select-Object -First 1
    }
    $gapMs = $null
    $note = ""
    if (-not $before) { $note = "no successful probe found before the command was issued" }
    elseif (-not $firstFailAfter) { $gapMs = 0; $note = "no failed probe observed in the 15s window after the command -- no visible interruption" }
    elseif (-not $firstOkAfterFail) { $note = "network did not recover within the 15s window" }
    else { $gapMs = [int]($firstOkAfterFail.ts - $before.ts).TotalMilliseconds }

    $summary.gaps += [ordered]@{
        rep = $evt.rep; direction = $evt.direction
        cmd_issued_at = $evt.issued_at; cmd_returned_at = $evt.returned_at; cmd_ms = $evt.cmd_ms
        last_ok_before = if ($before) { $before.ts.ToString("o") } else { $null }
        first_ok_after = if ($firstOkAfterFail) { $firstOkAfterFail.ts.ToString("o") } else { $null }
        gap_ms = $gapMs
        note = $note
    }
    Step "gap $($evt.direction) rep $($evt.rep)" ($null -ne $gapMs) "gap_ms=$gapMs cmd_ms=$($evt.cmd_ms) $note"
}

$summary.ok = ($summary.gaps.Count -gt 0) -and (($summary.gaps | Where-Object { $null -eq $_.gap_ms }).Count -eq 0)
Save-Summary
exit 0
