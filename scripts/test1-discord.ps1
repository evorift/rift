<#
.SYNOPSIS
    TEST 1 (Discord desktop) -- MVP acceptance test. Runs ON THE LAPTOP as a single agent job.
    PREPARED ONLY -- do not run until a human is physically watching the laptop screen.

.DESCRIPTION
    One mode per invocation (-Mode dpi|warp-split|warp-full), run in that order, stopping as
    soon as one mode works. Each invocation is its own standalone job -- this is the "if needed"
    escalation from the acceptance criteria: whether warp-split/warp-full get tried at all is a
    human decision made from what was seen in the previous mode, not something this script can
    infer (there is no way to see the Discord window from the controller).

    Everything is scripted except the eyeballing:
      1. Install Discord if absent, via the OFFICIAL installer URL evorift's own
         repair::reinstall_discord() already uses (Command::apply endpoint, distributions API) --
         not a guessed URL.
      2. Quit any running Discord (clean cold-connect attempt).
      3. Apply the mode: `evorift-ctl mode dpi` (+`on`) for DPI-only with WARP explicitly forced
         off, or `evorift-ctl mode warp-split|warp-full` (writes+applies a real WARP profile --
         the only path that actually stops the DPI engine for full-tunnel, see service.rs
         apply_profile_obj).
      4. Launch Discord and announce a hold window on stdout.
      5. Capture supporting network-layer evidence (TLS handshakes to Discord endpoints, and
         Discord's own renderer log if present) -- NOT the verdict. The verdict is what you saw.
      6. Tear down in a finally block, same discipline as TEST 2/3 -- protection does not survive
         past this job on purpose.

.PARAMETER Mode
    dpi | warp-split | warp-full

.PARAMETER HoldSeconds
    How long Discord gets to reach a steady state while you watch. Default 90s. Re-run with a
    larger value if you need more time -- each invocation is independent.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("dpi", "warp-split", "warp-full")]
    [string]$Mode,
    [int]$HoldSeconds = 90,
    [switch]$SkipInstall
)

$ErrorActionPreference = 'Continue'

function Iso8601Now {
    [DateTime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ss.fffZ", [System.Globalization.CultureInfo]::InvariantCulture)
}

$root = (Get-Location).Path
if ($root.StartsWith('\\?\')) { $root = $root.Substring(4) }
$root = $root.TrimEnd('\')

$engineDir  = "$root\engine"
$svcExe     = "$engineDir\evorift-svc.exe"
$ctlExe     = "$engineDir\evorift-ctl.exe"
$winwsExe   = "$engineDir\winws\winws.exe"
$resultPath = "$root\test1-$Mode-result.json"

$summary = [ordered]@{
    started_at   = Iso8601Now
    mode         = $Mode
    hold_seconds = $HoldSeconds
    steps        = @()
    evidence     = @()
    discord_log  = $null
    verdict      = "NOT RECORDED -- fill in from what the human observer reported"
    ok           = $false
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
        # No custom RemoteCertificateValidationCallback: a PS scriptblock callback gets invoked by
        # .NET's async TLS I/O off the PowerShell runspace thread and throws "no Runspace available",
        # failing every handshake regardless of the network (see test2-repeats-sweep.ps1). Real
        # validation against the OS trust store also catches a substituted DPI/MITM certificate.
        $ssl = New-Object System.Net.Security.SslStream($tcp.GetStream(), $false)
        $authTask = $ssl.AuthenticateAsClientAsync($TargetHost)
        if (-not $authTask.Wait($TimeoutMs)) {
            return [ordered]@{ ok = $false; ms = $sw.ElapsedMilliseconds; error = "tls handshake timeout" }
        }
        return [ordered]@{ ok = $ssl.IsAuthenticated; ms = $sw.ElapsedMilliseconds; error = "" }
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

# Mirrors repair.rs find_discord_path(): app-* subfolder with the highest (lexicographically last)
# version, falling back to a flat Discord.exe. Checks $env:LOCALAPPDATA first (the common case when
# this runs interactively), then every real profile under C:\Users -- evorift-testd runs as a
# Windows service, so $env:LOCALAPPDATA in a job it launches is the SERVICE ACCOUNT's profile, not
# the interactive user's, and Discord is installed under the latter (cost a live run to find).
function Find-DiscordExe {
    $roots = @(Join-Path $env:LOCALAPPDATA "Discord")
    Get-ChildItem -LiteralPath "C:\Users" -Directory -ErrorAction SilentlyContinue | ForEach-Object {
        $roots += Join-Path $_.FullName "AppData\Local\Discord"
    }
    foreach ($discordRoot in $roots) {
        if (-not (Test-Path -LiteralPath $discordRoot)) { continue }
        $appDirs = Get-ChildItem -LiteralPath $discordRoot -Directory -Filter "app-*" -ErrorAction SilentlyContinue |
            Sort-Object Name -Descending
        foreach ($d in $appDirs) {
            $exe = Join-Path $d.FullName "Discord.exe"
            if (Test-Path -LiteralPath $exe) { return $exe }
        }
        $flat = Join-Path $discordRoot "Discord.exe"
        if (Test-Path -LiteralPath $flat) { return $flat }
    }
    return $null
}

Write-Host "=== TEST 1: Discord desktop -- mode = $Mode ==="
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

# --- Discord: find, or install via evorift's own official installer URL (repair.rs installer_url) ---
$discordExe = Find-DiscordExe
if (-not $discordExe -and -not $SkipInstall) {
    Write-Host ""
    Write-Host "--- Discord not found -- installing via the official distributions API ---"
    $installerUrl = "https://discord.com/api/downloads/distributions/app/installers/latest?channel=stable&platform=win&arch=x64"
    $installerDest = Join-Path $env:TEMP "evorift-DiscordSetup-stable.exe"
    try {
        Invoke-WebRequest -Uri $installerUrl -OutFile $installerDest -UseBasicParsing -TimeoutSec 120
        Step "discord installer downloaded" $true $installerDest
        Start-Process -FilePath $installerDest | Out-Null
        Write-Host "--- waiting up to 90s for install + first launch ---"
        $deadline = (Get-Date).AddSeconds(90)
        while ((Get-Date) -lt $deadline -and -not $discordExe) {
            Start-Sleep -Seconds 3
            $discordExe = Find-DiscordExe
        }
    } catch {
        Step "discord installer downloaded" $false $_.Exception.Message
    }
}
if (-not $discordExe) {
    Step "discord present" $false "no Discord.exe found (install skipped or failed) -- cannot run TEST 1"
    Save-Summary
    exit 1
}
Step "discord present" $true $discordExe

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

    Get-Process -Name "Discord" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
    Step "discord quit for cold connect" $true "any running Discord.exe stopped"

    Write-Host ""
    Write-Host "--- applying mode: $Mode ---"
    if ($Mode -eq "dpi") {
        $m = & $ctlExe mode dpi 2>&1 | Out-String
        Step "mode dpi" ($LASTEXITCODE -eq 0) (($m -replace '\s+', ' ').Trim())
        $o = & $ctlExe on 2>&1 | Out-String
        $oOk = ($LASTEXITCODE -eq 0)
        Step "on" $oOk (($o -replace '\s+', ' ').Trim())
        if (-not $oOk) { throw "evorift-ctl on failed" }
    } else {
        $m = & $ctlExe mode $Mode 2>&1 | Out-String
        $mOk = ($LASTEXITCODE -eq 0)
        Step "mode $Mode" $mOk (($m -replace '\s+', ' ').Trim())
        if (-not $mOk) { throw "evorift-ctl mode $Mode failed" }
    }
    Start-Sleep -Seconds 3

    Write-Host ""
    Write-Host "--- launching Discord ---"
    Start-Process -FilePath $discordExe | Out-Null
    Step "discord launched" $true $discordExe

    Write-Host ""
    Write-Host "=================================================================="
    Write-Host " LOOK AT THE LAPTOP SCREEN NOW -- mode under test: $Mode"
    Write-Host " Watch the Discord window for up to $HoldSeconds seconds."
    Write-Host " Report back exactly one of:"
    Write-Host "   - stuck on 'Connecting...'"
    Write-Host "   - logged in, servers and DMs load"
    Write-Host "=================================================================="
    Write-Host ""
    Start-Sleep -Seconds $HoldSeconds

    Write-Host "--- supporting evidence (network-layer only -- NOT the verdict) ---"
    foreach ($t in @("discord.com", "gateway.discord.gg", "cdn.discordapp.com", "discordapp.net")) {
        $r = Test-TlsHandshake -TargetHost $t -TimeoutMs 4000
        $summary.evidence += [ordered]@{ target = $t; ok = $r.ok; ms = $r.ms; error = $r.error }
        Step "tls $t" $r.ok "ms=$($r.ms) $($r.error)"
    }
    try {
        $logDir = Join-Path $env:APPDATA "discord\logs"
        if (Test-Path -LiteralPath $logDir) {
            $latest = Get-ChildItem -LiteralPath $logDir -Filter "*.log" -ErrorAction SilentlyContinue |
                Sort-Object LastWriteTime -Descending | Select-Object -First 1
            if ($latest) {
                $tail = Get-Content -LiteralPath $latest.FullName -Tail 300 -ErrorAction SilentlyContinue
                $hasReady = (($tail | Select-String -SimpleMatch "READY").Count -gt 0)
                $hasTimeout = (($tail | Select-String -Pattern "ACK TIMEOUT|WS CLOSED").Count -gt 0)
                $summary.discord_log = [ordered]@{ file = $latest.FullName; has_ready = $hasReady; has_timeout_or_close = $hasTimeout }
                Step "discord renderer log" $true "READY=$hasReady TIMEOUT/CLOSED=$hasTimeout ($($latest.FullName))"
            } else {
                Step "discord renderer log" $false "no *.log under $logDir"
            }
        } else {
            Step "discord renderer log" $false "$logDir does not exist"
        }
    } catch {
        Step "discord renderer log" $false $_.Exception.Message
    }
}
catch {
    Step "run" $false $_.Exception.Message
}
finally {
    Write-Host ""
    Write-Host "--- teardown (protection does not survive past this job) ---"
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

$summary.ok = $true
Save-Summary
Write-Host ""
Write-Host "Now edit $resultPath (or just tell the controller) with the verdict you observed."
exit 0
