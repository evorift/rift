<#
.SYNOPSIS
    Local system-state capture for diagnosing the "protection on = internet cut" failure
    (see docs/DIAGNOSE-INTERNET-CUT.md). Read-only probes, no internet required to run.

.DESCRIPTION
    Writes one file per probe into docs/captures/<timestamp>-<label>/. Every probe is
    wrapped so a single failing command (e.g. a service that doesn't exist, a network
    call that times out because there's no network) never aborts the rest of the run --
    a failure IS a valid, useful data point here, not an error to avoid.

    Does not need to run elevated to produce a valid capture. But run it elevated for the
    "during" capture specifically if you can: verified directly (2026-08-13) that an
    unprivileged run's Get-CimInstance Win32_Process query silently returned ZERO results
    for evorift.exe even though it was confirmed running (tasklist saw it fine) -- evorift.exe
    now self-elevates (CLAUDE.md rule 3a/P0-a) and this PowerShell session did not, and
    Get-CimInstance appears to drop higher-integrity processes from the result set rather
    than error on them. winws.exe, spawned by an elevated evorift/EvoriftSvc, will likely
    hit the same gap. tasklist's presence/PID data (section 1 of 04-processes.txt) still
    works unprivileged either way; only the CIM command-line detail (section 2) needs
    elevation to be reliable. The manifest records whether the run was elevated so this is
    checkable after the fact rather than assumed.

.PARAMETER Label
    Which point in the repro sequence this capture represents: before | during | after.

.EXAMPLE
    .\scripts\capture-state.ps1 -Label before
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("before", "during", "after")]
    [string]$Label
)

$ErrorActionPreference = 'Continue'

$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"

# $PSScriptRoot may be a Windows verbatim path (\\?\C:\...) when this script is launched by the
# remote test agent, which resolves paths canonically. Verbatim paths switch off all path
# normalization, and Join-Path then fails outright with "cannot bind argument ... it is null".
# That produced a silent disaster once: every probe below failed, nothing was written, and the
# script still exited 0, so the controller reported a successful capture of nothing.
$repoRoot = Split-Path -Parent $PSScriptRoot
if ($repoRoot.StartsWith('\\?\')) { $repoRoot = $repoRoot.Substring(4) }
$repoRoot = $repoRoot.TrimEnd('\')

$outDir = "$repoRoot\docs\captures\$timestamp-$Label"
New-Item -ItemType Directory -Force -Path $outDir -ErrorAction SilentlyContinue | Out-Null

# Fail loudly if the output directory is not usable. $ErrorActionPreference is 'Continue' on
# purpose -- a failing probe IS a valid data point -- but that must not extend to being unable to
# write results at all. Without this guard the failure is invisible until someone opens an empty
# capture folder days later.
if (-not $outDir -or -not (Test-Path -LiteralPath $outDir)) {
    Write-Error "Cannot create the capture directory: $outDir (repoRoot='$repoRoot', PSScriptRoot='$PSScriptRoot')"
    exit 1
}

function Invoke-Capture {
    param(
        [Parameter(Mandatory = $true)][string]$FileName,
        [Parameter(Mandatory = $true)][scriptblock]$Probe
    )
    $path = Join-Path $outDir $FileName
    try {
        $result = & $Probe 2>&1
        $result | Out-File -FilePath $path -Encoding utf8
        Write-Host "  ok    $FileName"
    }
    catch {
        "PROBE THREW: $($_.Exception.GetType().Name): $($_.Exception.Message)" |
            Out-File -FilePath $path -Encoding utf8
        Write-Host "  ERROR $FileName -- $($_.Exception.Message)"
    }
}

# A ping with an explicit, short timeout. Windows PowerShell 5.1's Test-Connection has no
# -TimeoutSeconds parameter (that's a PS7 addition) -- using .NET's Ping directly instead,
# which works the same on both and never blocks longer than $TimeoutMs.
function Test-PingHost {
    param([string]$TargetHost, [int]$TimeoutMs = 1500)
    try {
        $ping = New-Object System.Net.NetworkInformation.Ping
        $reply = $ping.Send($TargetHost, $TimeoutMs)
        "{0,-16} -> {1,-12} {2} ms" -f $TargetHost, $reply.Status, $reply.RoundtripTime
    }
    catch {
        "{0,-16} -> EXCEPTION: {1}" -f $TargetHost, $_.Exception.Message
    }
}

Write-Host "Capturing '$Label' state to $outDir"

# --- 000: run manifest -- context needed to interpret everything else ---
Invoke-Capture "000-manifest.txt" {
    $isElevated = $false
    try {
        $id = [System.Security.Principal.WindowsIdentity]::GetCurrent()
        $principal = New-Object System.Security.Principal.WindowsPrincipal($id)
        $isElevated = $principal.IsInRole([System.Security.Principal.WindowsBuiltInRole]::Administrator)
    } catch {}
    "label:            $Label"
    "timestamp:        $timestamp"
    "hostname:         $env:COMPUTERNAME"
    "user:             $env:USERDOMAIN\$env:USERNAME"
    "running elevated: $isElevated"
    "os:               $((Get-CimInstance Win32_OperatingSystem).Caption) build $((Get-CimInstance Win32_OperatingSystem).BuildNumber)"
    "powershell:       $($PSVersionTable.PSVersion)"
}

# --- 1: network config (ipconfig/all, route print, DNS servers) ---
Invoke-Capture "01-network-config.txt" {
    "===== ipconfig /all ====="
    ipconfig /all
    ""
    "===== route print ====="
    route print
    ""
    "===== netsh interface ipv4 show dnsservers ====="
    netsh interface ipv4 show dnsservers
}

# --- 2: firewall rules, filtered to evorift/winws (raw kept alongside, in case the
#         filter regex misses something a human eye wouldn't) ---
Invoke-Capture "02-firewall-rules-raw.txt" {
    netsh advfirewall firewall show rule name=all dir=out
}
Invoke-Capture "02-firewall-rules-filtered.txt" {
    $raw = netsh advfirewall firewall show rule name=all dir=out
    # netsh separates each rule with a line of dashes; filtering per-line would split a
    # matching rule's Program/Name fields across unrelated lines, so split into whole
    # rule blocks first and keep any block that mentions evorift or winws anywhere in it.
    $joined = $raw -join "`n"
    $blocks = $joined -split '(?m)^-{5,}\s*$'
    $matching = $blocks | Where-Object { $_ -match 'evorift|winws' }
    if ($matching) {
        $matching -join "`n----------------------------------------------------------------------`n"
    } else {
        "no rule mentioning 'evorift' or 'winws' found in dir=out rules"
    }
}

# --- 3: service states -- WinDivert has 3 possible version-specific service names
#         (see engine.rs::clear_stale_windivert); the app's own service is EvoriftSvc
#         (svcctl.rs::SERVICE_NAME -- not "evorift-svc", that name doesn't exist as a
#         service, corrected here rather than guessed) ---
# IMPORTANT: must call sc.exe explicitly, not bare `sc` -- PowerShell ships a built-in
# alias `sc` -> `Set-Content` that silently wins over the real sc.exe. Found the hard
# way: an early test run of this script produced an empty 03-services.txt and a stray
# `query` file at the repo root containing the last loop value ("EvoriftSvc") -- that
# was `Set-Content -Path query -Value EvoriftSvc` firing instead of `sc.exe query
# EvoriftSvc`. `sc.exe` (with the extension) bypasses the alias correctly.
Invoke-Capture "03-services.txt" {
    foreach ($svc in @("WinDivert", "WinDivert1.4", "WinDivert1.1", "EvoriftSvc")) {
        "===== sc.exe query $svc ====="
        sc.exe query $svc
        ""
    }
}

# --- 4: processes -- tasklist filtered to winws/evorift, plus full command lines via CIM
#         (tasklist alone truncates/omits command-line args; CIM gives the real argv).
#         -OperationTimeoutSec bounds this: a test run of this script took ~3 minutes on
#         this machine for a plain Get-CimInstance Win32_Process call (WMI provider is
#         sometimes just slow to answer, unrelated to the bug being diagnosed) -- without
#         a bound, a live "during" capture could stall the whole repro for minutes. ---
Invoke-Capture "04-processes.txt" {
    "===== tasklist /v (filtered) ====="
    $tl = tasklist /v /fo table
    $tl | Select-Object -First 3   # header rows
    $tl | Select-String -Pattern 'winws|evorift'
    ""
    "===== Get-CimInstance Win32_Process (filtered, with command line) ====="
    try {
        Get-CimInstance Win32_Process -OperationTimeoutSec 20 -ErrorAction Stop |
            Where-Object { $_.Name -match 'winws|evorift' } |
            Select-Object ProcessId, ParentProcessId, Name, CommandLine |
            Format-List |
            Out-String -Width 500
    }
    catch {
        "Get-CimInstance failed or timed out after 20s: $($_.Exception.Message)"
    }
}

# --- 5: reachability -- local-only, never depends on internet succeeding, just measures it ---
Invoke-Capture "05-reachability.txt" {
    "===== default gateway ====="
    $gw = Get-NetRoute -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue |
        Sort-Object -Property RouteMetric |
        Select-Object -First 1 -ExpandProperty NextHop
    if ($gw) {
        "gateway found: $gw"
        Test-PingHost -TargetHost $gw
    } else {
        "no default IPv4 route found (this itself is a data point)"
    }
    ""
    "===== 1.1.1.1 ====="
    Test-PingHost -TargetHost "1.1.1.1"
    ""
    "===== DNS resolve discord.com ====="
    try {
        Resolve-DnsName -Name "discord.com" -Type A -ErrorAction Stop |
            Format-Table -AutoSize | Out-String -Width 300
    }
    catch {
        "DNS resolution FAILED: $($_.Exception.Message)"
    }
}

# --- 6: evorift's own log directory, copied (not dumped) -- actual files preserved.
#         Path resolved from code: ipc.rs::data_dir() = %ProgramData%\evorift, logs
#         written under <data_dir>\logs (sys.rs::log_dir()). Same path in debug and
#         release -- data_dir() is not debug/release-conditional (only the IPC token
#         path is; logs are not). ---
$logSrc = Join-Path $env:ProgramData "evorift\logs"
$logDst = Join-Path $outDir "evorift-logs"
if (Test-Path $logSrc) {
    try {
        Copy-Item -Path $logSrc -Destination $logDst -Recurse -Force -ErrorAction Stop
        Write-Host "  ok    evorift-logs\ (copied from $logSrc)"
    }
    catch {
        "COPY FAILED from $logSrc : $($_.Exception.Message)" |
            Out-File -FilePath (Join-Path $outDir "06-evorift-logs-ERROR.txt") -Encoding utf8
        Write-Host "  ERROR evorift-logs\ -- $($_.Exception.Message)"
    }
} else {
    "log directory not found at: $logSrc" |
        Out-File -FilePath (Join-Path $outDir "06-evorift-logs-NOTFOUND.txt") -Encoding utf8
    Write-Host "  --    evorift-logs\ not found at $logSrc"
}

Write-Host ""
Write-Host "Capture complete: $outDir"
