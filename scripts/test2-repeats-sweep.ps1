<#
.SYNOPSIS
    TEST 2 (fake-packet sweep) -- MVP acceptance test. Runs ON THE LAPTOP as a single agent job.

.DESCRIPTION
    Sweeps dpi-desync-repeats from 1 upward via `evorift-ctl strat c1 --repeats=N`, which routes
    through engine.rs's own Strategy/tls_profile_args() builder (see ipc.rs Command::SetStrategy
    repeats_override) -- this script never constructs a winws command line itself.

    At each value, tests whether GENERAL HTTPS still works using real TLS handshakes (TCP connect
    + TLS AuthenticateAsClient) to a few non-Discord targets -- not ping. This measures general
    HTTPS only; Discord's own minimum needs TEST 1's result first.

    Finds the lowest value whose first pass holds, then re-tests that value twice more (3/3 total)
    before calling it confirmed. If a "confirmed" value turns out to be a one-off on a later
    confirmation run, the sweep continues upward and repeats the confirmation procedure. The full
    sweep -- including every failure -- is recorded, not just the winner.

    Protection is forced to mode=dpi (WARP explicitly off) so this isolates the DPI engine's
    repeat count from any WARP tunnel effect.
#>
[CmdletBinding()]
param(
    [int]$MinRepeats = 1,
    [int]$MaxRepeats = 20,
    [int]$SettleMs = 1500,
    [string[]]$Targets = @("www.google.com", "www.microsoft.com", "www.cloudflare.com"),
    [int]$TlsTimeoutMs = 4000,
    [int]$ConfirmRuns = 2
)

$ErrorActionPreference = 'Continue'

function Iso8601Now {
    [DateTime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ss.fffZ", [System.Globalization.CultureInfo]::InvariantCulture)
}

# The agent's working directory can be a verbatim path (\\?\C:\evorift-test); normalise once.
$root = (Get-Location).Path
if ($root.StartsWith('\\?\')) { $root = $root.Substring(4) }
$root = $root.TrimEnd('\')

$engineDir  = "$root\engine"
$svcExe     = "$engineDir\evorift-svc.exe"
$ctlExe     = "$engineDir\evorift-ctl.exe"
$winwsExe   = "$engineDir\winws\winws.exe"
$sweepLog   = "$root\test2-sweep.csv"
$resultPath = "$root\test2-result.json"

$summary = [ordered]@{
    started_at            = Iso8601Now
    targets               = $Targets
    min_repeats           = $MinRepeats
    max_repeats           = $MaxRepeats
    confirm_runs          = $ConfirmRuns
    steps                 = @()
    sweep                 = @()
    candidate             = $null
    confirmed_min_repeats = $null
    ok                    = $false
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

# Real TLS handshake, not a ping: TCP connect + TLS AuthenticateAsClient, with REAL certificate
# validation against the OS trust store (no accept-any callback -- a PS scriptblock passed as
# RemoteCertificateValidationCallback gets invoked by .NET's async TLS I/O off the PowerShell
# runspace thread, which throws "There is no Runspace available to run scripts in this thread" and
# makes every single handshake fail regardless of what's actually happening on the wire -- cost a
# full sweep run to find). Real validation is also methodologically the right call here: it also
# catches a DPI/MITM box presenting a substituted certificate, which accept-any would have masked.
function Test-TlsHandshake {
    param([string]$TargetHost, [int]$Port = 443, [int]$TimeoutMs = 4000)
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $tcp = $null
    $ssl = $null
    try {
        $tcp = New-Object System.Net.Sockets.TcpClient
        $connectTask = $tcp.ConnectAsync($TargetHost, $Port)
        if (-not $connectTask.Wait($TimeoutMs)) {
            return [ordered]@{ ok = $false; ms = $sw.ElapsedMilliseconds; error = "tcp connect timeout" }
        }
        $ssl = New-Object System.Net.Security.SslStream($tcp.GetStream(), $false)
        $authTask = $ssl.AuthenticateAsClientAsync($TargetHost)
        if (-not $authTask.Wait($TimeoutMs)) {
            return [ordered]@{ ok = $false; ms = $sw.ElapsedMilliseconds; error = "tls handshake timeout" }
        }
        $ok = $ssl.IsAuthenticated
        return [ordered]@{ ok = $ok; ms = $sw.ElapsedMilliseconds; error = "" }
    } catch {
        $e = $_.Exception
        $chain = @()
        while ($e) { $chain += $e.Message; $e = $e.InnerException }
        return [ordered]@{ ok = $false; ms = $sw.ElapsedMilliseconds; error = ($chain -join " <- ") }
    } finally {
        if ($ssl) { $ssl.Close() }
        if ($tcp) { $tcp.Close() }
    }
}

function Test-AtRepeats {
    param([int]$N, [string]$Label)
    $stratOut = & $ctlExe strat c1 --repeats=$N 2>&1 | Out-String
    $stratOk = ($LASTEXITCODE -eq 0)
    Step "strat c1 --repeats=$N ($Label)" $stratOk (($stratOut -replace '\s+', ' ').Trim())
    if (-not $stratOk) { return $false }
    Start-Sleep -Milliseconds $SettleMs
    $allOk = $true
    foreach ($t in $Targets) {
        $r = Test-TlsHandshake -TargetHost $t -TimeoutMs $TlsTimeoutMs
        $script:summary.sweep += [ordered]@{ repeats = $N; label = $Label; target = $t; ok = $r.ok; ms = $r.ms; error = $r.error }
        Add-Content -LiteralPath $sweepLog -Value "$N,$Label,$t,$($r.ok),$($r.ms),$($r.error)"
        if (-not $r.ok) { $allOk = $false }
    }
    Step "repeats=$N ($Label) all targets" $allOk "$($Targets.Count) target(s)"
    return $allOk
}

Write-Host "=== TEST 2: fake-packet (dpi-desync-repeats) sweep ==="
Write-Host "root: $root"

$missing = @()
foreach ($f in @($svcExe, $ctlExe, $winwsExe)) {
    if (Test-Path -LiteralPath $f) { Step "present" $true $f } else { Step "present" $false "MISSING $f"; $missing += $f }
}
if ($missing.Count -gt 0) {
    Step "preflight" $false "$($missing.Count) required file(s) missing"
    Save-Summary
    exit 1
}
# Exclusive-control guard: every measurement here assumes THIS job is the only thing driving
# evorift-svc/winws. A pre-existing instance this job didn't start means an unknown prior state
# (unknown strategy, unknown app_modes/full_warp) is already live -- proceeding would silently
# mix that state into the sweep. Abort rather than produce an invalid number (P0-e, BACKLOG.md).
$preexisting = Get-Process -Name "evorift-svc", "winws" -ErrorAction SilentlyContinue
if ($preexisting) {
    $list = ($preexisting | ForEach-Object { "$($_.ProcessName) pid=$($_.Id) started=$($_.StartTime)" }) -join "; "
    Step "exclusive control" $false "evorift-svc/winws already running before this job started ($list) -- ABORTING, every measurement here requires exclusive control"
    Save-Summary
    exit 1
}
Step "exclusive control" $true "no pre-existing evorift-svc/winws process found"

"repeats,label,target,ok,ms,error" | Out-File -FilePath $sweepLog -Encoding ascii

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

    $modeOut = & $ctlExe mode dpi 2>&1 | Out-String
    Step "mode dpi" ($LASTEXITCODE -eq 0) (($modeOut -replace '\s+', ' ').Trim())

    $onOut = & $ctlExe on 2>&1 | Out-String
    $onOk = ($LASTEXITCODE -eq 0)
    Step "on" $onOk (($onOut -replace '\s+', ' ').Trim())
    if (-not $onOk) { throw "evorift-ctl on failed" }

    Write-Host ""
    Write-Host "--- sweeping repeats from $MinRepeats to $MaxRepeats ---"
    $n = $MinRepeats
    $confirmed = $null
    while ($n -le $MaxRepeats -and $null -eq $confirmed) {
        $pass = Test-AtRepeats -N $n -Label "sweep"
        if ($pass) {
            $summary.candidate = $n
            Step "candidate" $true "repeats=$n held on first pass; confirming $ConfirmRuns more time(s)"
            $allConfirmsPass = $true
            for ($c = 1; $c -le $ConfirmRuns; $c++) {
                if (-not (Test-AtRepeats -N $n -Label "confirm$c")) { $allConfirmsPass = $false; break }
            }
            if ($allConfirmsPass) {
                $confirmed = $n
            } else {
                Step "confirm" $false "repeats=$n was a one-off on confirmation -- continuing the sweep upward"
            }
        }
        $n++
    }
    $summary.confirmed_min_repeats = $confirmed
    if ($null -eq $confirmed) {
        Step "sweep" $false "no repeats value in $MinRepeats..$MaxRepeats held $(1+$ConfirmRuns)/$(1+$ConfirmRuns) confirmation runs"
    } else {
        Step "sweep" $true "confirmed minimum repeats = $confirmed ($(1+$ConfirmRuns)/$(1+$ConfirmRuns) runs held)"
    }
}
catch {
    Step "run" $false $_.Exception.Message
}
finally {
    Write-Host ""
    Write-Host "--- teardown ---"
    if (Test-Path -LiteralPath $ctlExe) {
        $offOut = & $ctlExe off 2>&1 | Out-String
        Step "off" ($LASTEXITCODE -eq 0) (($offOut -replace '\s+', ' ').Trim())
    }
    if ($svcProc -and -not $svcProc.HasExited) {
        Stop-Process -Id $svcProc.Id -Force -ErrorAction SilentlyContinue
        Step "svc stopped" $true "pid $($svcProc.Id) killed"
    }
    $stragglers = Get-Process -Name "winws" -ErrorAction SilentlyContinue
    if ($stragglers) { $stragglers | Stop-Process -Force -ErrorAction SilentlyContinue; Step "winws stopped" $true "killed $($stragglers.Count)" }
    else { Step "winws stopped" $true "none left" }
    $svcW = Get-Service -Name "WireGuardTunnel`$warp" -ErrorAction SilentlyContinue
    if ($svcW) {
        & sc.exe stop "WireGuardTunnel`$warp" | Out-Null
        Start-Sleep -Seconds 1
        & sc.exe delete "WireGuardTunnel`$warp" | Out-Null
        Step "warp tunnel service cleared" $true "WireGuardTunnel`$warp stopped+deleted"
    }
}

$summary.ok = ($null -ne $summary.confirmed_min_repeats)
Save-Summary
exit 0
