<#
.SYNOPSIS
    Repair DNS on the test laptop. Run via FIX-DNS.bat.

.DESCRIPTION
    The deadman switch resets DNS to DHCP on every fire. Left armed and idle it fired dozens of
    times, which on this laptop left the live Wi-Fi adapter with no resolver at all while a stale
    entry sat on a disconnected Ethernet adapter. The machine could ping 1.1.1.1 but resolve
    nothing -- which then showed up in every capture as a property of the laptop rather than as
    damage done by the test agent.

    This resets DNS to DHCP on the adapters that are actually up, renews the lease so the router
    hands its resolver back, flushes the cache, and then VERIFIES by resolving real names. It
    reports failure honestly if resolution still does not work.

    The agent itself no longer causes this: it now stands down after 3 consecutive fires instead
    of firing forever. This script repairs the damage already done.
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Continue'

function Say  { param([string]$T) Write-Host $T }
function Ok   { param([string]$T) Write-Host "  ok    $T" -ForegroundColor Green }
function Bad  { param([string]$T) Write-Host "  FAIL  $T" -ForegroundColor Red }
function Info { param([string]$T) Write-Host "  --    $T" -ForegroundColor DarkGray }

Say ""
Say "=== BEFORE ==="
Get-DnsClientServerAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue |
    ForEach-Object { Info "$($_.InterfaceAlias): [$($_.ServerAddresses -join ', ')]" }

Say ""
Say "=== Repairing ==="

# Only the adapters actually carrying traffic. Touching a disconnected adapter is what left the
# stale entry on Ethernet in the first place.
$live = Get-NetAdapter -Physical -ErrorAction SilentlyContinue | Where-Object { $_.Status -eq 'Up' }
if (-not $live) {
    Bad "no physical adapter is Up - connect Wi-Fi or Ethernet first"
    exit 1
}
foreach ($a in $live) {
    Info "resetting DNS to DHCP on '$($a.Name)'"
    Set-DnsClientServerAddress -InterfaceIndex $a.ifIndex -ResetServerAddresses -Confirm:$false -ErrorAction SilentlyContinue
}

Info "renewing the DHCP lease"
ipconfig /renew | Out-Null
Info "flushing the resolver cache"
Clear-DnsClientCache -ErrorAction SilentlyContinue
Start-Sleep -Seconds 3

Say ""
Say "=== AFTER ==="
Get-DnsClientServerAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue |
    ForEach-Object { Info "$($_.InterfaceAlias): [$($_.ServerAddresses -join ', ')]" }

Say ""
Say "=== Verifying (measured, not assumed) ==="
$names = @('google.com', 'discord.com')
$resolved = 0
foreach ($n in $names) {
    try {
        $r = Resolve-DnsName -Name $n -Type A -ErrorAction Stop | Select-Object -First 1
        Ok "$n -> $($r.IPAddress)"
        $resolved++
    } catch {
        Bad "$n -> still failing: $($_.Exception.Message)"
    }
}

Say ""
if ($resolved -eq $names.Count) {
    Write-Host "  DNS IS WORKING AGAIN." -ForegroundColor Green
    exit 0
}

Write-Host "  DNS IS STILL BROKEN ($resolved of $($names.Count) resolved)." -ForegroundColor Red
Write-Host ""
Write-Host "  Next things to try, in order:" -ForegroundColor Yellow
Write-Host "    1. Turn Wi-Fi off and on again, then re-run this."
Write-Host "    2. Check the router is handing out DNS (another device on the same"
Write-Host "       Wi-Fi should be able to browse)."
Write-Host "    3. As a temporary override, set a public resolver on the live adapter:"
foreach ($a in $live) {
    Write-Host "         Set-DnsClientServerAddress -InterfaceIndex $($a.ifIndex) -ServerAddresses 1.1.1.1,8.8.8.8"
}
Write-Host "       (undo later with -ResetServerAddresses)"
exit 1
