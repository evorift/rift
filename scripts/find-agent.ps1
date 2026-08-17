<#
.SYNOPSIS
    Find the test laptop's current IP. Prints one address, or nothing if it cannot be found.

.DESCRIPTION
    Hard-coding the agent's address was the single biggest time sink in getting this working:
    both machines took new DHCP leases on the same day, and every pinned address broke at once.
    This resolves the address fresh, every run, in the order of cheapest-and-most-reliable first:

      1. LAST KNOWN. Whatever worked last time, if TCP on the port still answers. One connect.
      2. TAILSCALE. `tailscale ping` reports the peer's real LAN endpoint -- "pong from laptop
         (100.x) via 192.168.1.18:41641". Tailscale tracks the peer through DHCP changes, so this
         keeps working when everything pinned has gone stale. It is used purely for discovery;
         the control traffic itself goes over plain LAN.
      3. ARP + PORT SWEEP. Every host already in the ARP cache, then the rest of the local /24,
         probed on the agent's port with a short timeout.

    The result is cached next to this script so step 1 usually succeeds next time.

.PARAMETER Port
    The agent's port. Default 8765.

.PARAMETER TailscalePeer
    Peer name or tailnet IP to ask Tailscale about. Default: the first Windows peer that is not
    this machine.

.PARAMETER Quiet
    Print only the address (default prints progress to stderr).
#>
[CmdletBinding()]
param(
    [int]$Port = 8765,
    [string]$TailscalePeer,
    [switch]$Quiet
)

$ErrorActionPreference = 'Continue'
$CachePath = Join-Path $PSScriptRoot ".agent-address"

function Note { param([string]$T) if (-not $Quiet) { Write-Host "  $T" -ForegroundColor DarkGray } }

# A short TCP connect. Test-NetConnection is far too slow to sweep a /24 with.
function Test-Port {
    param([string]$Ip, [int]$P, [int]$TimeoutMs = 350)
    $client = New-Object System.Net.Sockets.TcpClient
    try {
        $async = $client.BeginConnect($Ip, $P, $null, $null)
        if (-not $async.AsyncWaitHandle.WaitOne($TimeoutMs, $false)) { return $false }
        $client.EndConnect($async)
        return $true
    } catch { return $false }
    finally { $client.Close() }
}

# --- 1: last known ------------------------------------------------------------------------------
if (Test-Path $CachePath) {
    $last = (Get-Content $CachePath -Raw).Trim()
    if ($last -and (Test-Port -Ip $last -P $Port)) {
        Note "found at the last known address $last"
        Write-Output $last
        exit 0
    }
    if ($last) { Note "last known address $last no longer answers" }
}

# --- 2: Tailscale ------------------------------------------------------------------------------
$tsExe = @("$env:ProgramFiles\Tailscale\tailscale.exe", "${env:ProgramFiles(x86)}\Tailscale\tailscale.exe") |
    Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $tsExe) { $tsExe = (Get-Command tailscale.exe -ErrorAction SilentlyContinue).Source }

if ($tsExe) {
    try {
        $status = & $tsExe status --json 2>$null | Out-String | ConvertFrom-Json
        $peers = @()
        if ($status.Peer) {
            $peers = $status.Peer.PSObject.Properties | ForEach-Object { $_.Value } |
                     Where-Object { $_.Online }
        }
        if ($TailscalePeer) {
            $peers = $peers | Where-Object {
                $_.HostName -eq $TailscalePeer -or ($_.TailscaleIPs -contains $TailscalePeer)
            }
        }
        foreach ($peer in $peers) {
            $target = @($peer.TailscaleIPs | Where-Object { $_ -notmatch ':' }) | Select-Object -First 1
            if (-not $target) { continue }
            # "pong from laptop (100.103.86.68) via 192.168.1.18:41641 in 4ms"
            $pong = & $tsExe ping --c 1 $target 2>&1 | Out-String
            $m = [regex]::Match($pong, 'via\s+(\d{1,3}(?:\.\d{1,3}){3}):\d+')
            if ($m.Success) {
                $lan = $m.Groups[1].Value
                Note "Tailscale reports '$($peer.HostName)' on the LAN at $lan"
                if (Test-Port -Ip $lan -P $Port) {
                    Set-Content -Path $CachePath -Value $lan -NoNewline -Encoding ascii
                    Write-Output $lan
                    exit 0
                }
                Note "$lan does not answer on port $Port (agent stopped?)"
            }
        }
    } catch { Note "Tailscale lookup failed: $($_.Exception.Message)" }
}

# --- 3: ARP, then sweep the local /24 -----------------------------------------------------------
$localV4 = Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue |
    Where-Object { $_.IPAddress -ne '127.0.0.1' -and $_.PrefixLength -eq 24 } |
    Select-Object -First 1
if (-not $localV4) { Note "no local /24 to sweep"; exit 1 }

$prefix = ($localV4.IPAddress -split '\.')[0..2] -join '.'
Note "sweeping $prefix.0/24 on port $Port"

# ARP first: those hosts are known to exist, so they answer or fail fast.
$arpHosts = (arp -a | Select-String -Pattern "^\s+($([regex]::Escape($prefix))\.\d{1,3})\s" -AllMatches |
    ForEach-Object { $_.Matches.Groups[1].Value }) | Select-Object -Unique
foreach ($ip in $arpHosts) {
    if ($ip -eq $localV4.IPAddress) { continue }
    if (Test-Port -Ip $ip -P $Port) {
        Note "found at $ip (ARP)"
        Set-Content -Path $CachePath -Value $ip -NoNewline -Encoding ascii
        Write-Output $ip
        exit 0
    }
}

foreach ($n in 1..254) {
    $ip = "$prefix.$n"
    if ($ip -eq $localV4.IPAddress -or $arpHosts -contains $ip) { continue }
    if (Test-Port -Ip $ip -P $Port -TimeoutMs 120) {
        Note "found at $ip (sweep)"
        Set-Content -Path $CachePath -Value $ip -NoNewline -Encoding ascii
        Write-Output $ip
        exit 0
    }
}

Note "no agent found on port $Port"
exit 1
