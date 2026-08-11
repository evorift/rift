<#
.SYNOPSIS
    Diagnose and establish an IP link between the controller and the test laptop, for the case
    where one machine is on Ethernet and the other is on Wi-Fi and they cannot reach each other.

.DESCRIPTION
    The default assumption of the remote-testing setup is that both machines sit on the same
    router subnet. When they do not -- one on a cable, one on Wi-Fi, and the router not bridging
    the two -- you need a different addressing method. This script provides three, and tells you
    which one you are in a position to use.

      diagnose      Read-only. Lists every adapter, its address, subnet, gateway and firewall
                    profile, and explains WHY the two machines cannot see each other.

      direct-cable  Puts a static address on the Ethernet adapter of each machine, with NO
                    default gateway, and marks the network Private. The cable becomes a
                    dedicated control link; the laptop keeps using Wi-Fi for internet.
                    This is the best option for this project -- see "Why" below.

      hotspot       Guided setup for Windows Mobile Hotspot (both machines land on 192.168.137.x).
                    Use when neither machine has a spare Ethernet port.

      revert        Undo direct-cable: put the adapter back on DHCP and restore its profile.

.NOTES
    WHY THE DIRECT CABLE IS THE RIGHT ANSWER HERE

    The software under test cuts all internet on the laptop. If your control link runs over the
    same interface that evorift is interfering with, you lose the laptop at exactly the moment
    the test gets interesting -- which is what the deadman switch exists to survive.

    A direct cable puts the control link on a *physically different interface* from the one
    carrying internet traffic. When evorift kills the Wi-Fi path, the Ethernet control link has
    a good chance of staying up, so you can watch the failure live instead of reconstructing it
    from captures afterwards. The deadman still guards you if it does not.

    REVERSIBILITY

    Any change this script makes to your network configuration is written to a rollback journal
    BEFORE it is applied (C:\ProgramData\evorift-testd\link-rollback.json). If the journal
    cannot be written, the change does not happen. `-Action revert` reads it back.

.PARAMETER Action
    diagnose | direct-cable | hotspot | revert

.PARAMETER Role
    For direct-cable: controller (gets .1) or agent (gets .2).

.PARAMETER PeerIp
    For diagnose: the other machine's address, so reachability and subnet overlap are checked.

.PARAMETER Subnet
    For direct-cable: first three octets of the private link. Default 192.168.77.
    Must not collide with your router's subnet -- the script refuses if it does.

.PARAMETER AdapterName
    Explicit adapter to configure, if auto-detection picks the wrong one.

.PARAMETER Force
    Skip the confirmation prompt.

.EXAMPLE
    .\scripts\link-setup.ps1 -Action diagnose -PeerIp 192.168.1.25

.EXAMPLE
    # On the controller (desktop):
    .\scripts\link-setup.ps1 -Action direct-cable -Role controller
    # On the test laptop:
    .\link-setup.ps1 -Action direct-cable -Role agent

.EXAMPLE
    .\scripts\link-setup.ps1 -Action revert
#>
[CmdletBinding()]
param(
    [ValidateSet('diagnose', 'direct-cable', 'hotspot', 'tailscale', 'revert')]
    [string]$Action = 'diagnose',
    [ValidateSet('controller', 'agent')]
    [string]$Role,
    [string]$PeerIp,
    [string]$Subnet = '192.168.77',
    [int]$PrefixLength = 24,
    [string]$AdapterName,
    [switch]$Force
)

$ErrorActionPreference = 'Stop'

$StateDir     = Join-Path $env:ProgramData "evorift-testd"
$JournalPath  = Join-Path $StateDir "link-rollback.json"

function Write-Step { param([string]$Text) Write-Host "`n=== $Text ===" -ForegroundColor Cyan }
function Write-Ok   { param([string]$Text) Write-Host "  ok    $Text" -ForegroundColor Green }
function Write-Info { param([string]$Text) Write-Host "  --    $Text" -ForegroundColor DarkGray }
function Write-Bad  { param([string]$Text) Write-Host "  FAIL  $Text" -ForegroundColor Red }
function Write-Hint { param([string]$Text) Write-Host "  ->    $Text" -ForegroundColor Yellow }

function Test-Elevated {
    $id = [System.Security.Principal.WindowsIdentity]::GetCurrent()
    $p = New-Object System.Security.Principal.WindowsPrincipal($id)
    return $p.IsInRole([System.Security.Principal.WindowsBuiltInRole]::Administrator)
}

# Every adapter that is Up, physical or not.
#
# Deliberately NOT `Get-NetAdapter -Physical`: that hides tunnel adapters, and Tailscale is a
# tunnel. Diagnosing a tailnet problem with a tool that cannot see the tailnet adapter wastes an
# afternoon. IsPhysical is carried as a flag instead, so direct-cable can still filter on it.
function Get-Ipv4Adapters {
    $physicalNames = @(Get-NetAdapter -Physical -ErrorAction SilentlyContinue | ForEach-Object { $_.Name })
    Get-NetAdapter -ErrorAction SilentlyContinue | Where-Object { $_.Status -eq 'Up' } | ForEach-Object {
        $adapter = $_
        $ip = Get-NetIPAddress -AddressFamily IPv4 -InterfaceIndex $adapter.ifIndex -ErrorAction SilentlyContinue |
              Where-Object { $_.IPAddress -ne '127.0.0.1' } | Select-Object -First 1
        $route = Get-NetRoute -DestinationPrefix '0.0.0.0/0' -InterfaceIndex $adapter.ifIndex -ErrorAction SilentlyContinue |
                 Select-Object -First 1
        $profile = Get-NetConnectionProfile -InterfaceIndex $adapter.ifIndex -ErrorAction SilentlyContinue
        [PSCustomObject]@{
            Name        = $adapter.Name
            ifIndex     = $adapter.ifIndex
            Media       = $adapter.MediaType
            IsPhysical  = ($physicalNames -contains $adapter.Name)
            IsWifi      = ($adapter.MediaType -match 'Native 802.11' -or $adapter.InterfaceDescription -match 'Wi-Fi|Wireless|802\.11')
            IsTailscale = ($adapter.InterfaceDescription -match 'Tailscale' -or $adapter.Name -match 'Tailscale')
            IPAddress   = if ($ip) { $ip.IPAddress } else { $null }
            PrefixLen   = if ($ip) { $ip.PrefixLength } else { $null }
            IsDhcp      = if ($ip) { $ip.PrefixOrigin -eq 'Dhcp' } else { $null }
            Gateway     = if ($route) { $route.NextHop } else { $null }
            Category    = if ($profile) { $profile.NetworkCategory } else { 'unknown' }
        }
    }
}

function Get-TailscaleExe {
    $exe = @(
        "$env:ProgramFiles\Tailscale\tailscale.exe",
        "${env:ProgramFiles(x86)}\Tailscale\tailscale.exe"
    ) | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $exe) { $exe = (Get-Command tailscale.exe -ErrorAction SilentlyContinue).Source }
    return $exe
}

# Can a listener actually bind this address? The only non-lying test -- see the long note in
# testd-install.ps1 for why reporting tools are not sufficient evidence here.
function Test-Bindable {
    param([string]$Ip)
    try {
        $listener = New-Object System.Net.Sockets.TcpListener([System.Net.IPAddress]::Parse($Ip), 0)
        $listener.Start(); $listener.Stop()
        return @{ Ok = $true; Error = $null }
    } catch {
        $msg = $_.Exception.Message
        if ($_.Exception.InnerException) { $msg = $_.Exception.InnerException.Message }
        return @{ Ok = $false; Error = $msg }
    }
}

# Are two addresses in the same IPv4 subnet, given a prefix length?
function Test-SameSubnet {
    param([string]$A, [string]$B, [int]$Prefix)
    try {
        $ba = ([System.Net.IPAddress]::Parse($A)).GetAddressBytes()
        $bb = ([System.Net.IPAddress]::Parse($B)).GetAddressBytes()
        [array]::Reverse($ba); [array]::Reverse($bb)
        $ia = [BitConverter]::ToUInt32($ba, 0)
        $ib = [BitConverter]::ToUInt32($bb, 0)
        if ($Prefix -le 0)  { return $true }
        if ($Prefix -ge 32) { return $ia -eq $ib }
        $mask = [uint32]([uint32]::MaxValue -shl (32 - $Prefix))
        return (($ia -band $mask) -eq ($ib -band $mask))
    } catch { return $false }
}

# ================================================================================================
# diagnose
# ================================================================================================

if ($Action -eq 'diagnose') {

    Write-Step "Adapters on this machine"
    $adapters = @(Get-Ipv4Adapters)
    if (-not $adapters) { Write-Bad "no adapter is Up"; return }
    $adapters | Format-Table Name, IPAddress, PrefixLen, Gateway, Category, IsWifi, IsTailscale -AutoSize | Out-String | Write-Host

    # Tailscale addresses routinely fail to appear above even when the tunnel is fully up, so
    # report them from the CLI rather than letting the table imply they do not exist.
    $tsExe = Get-TailscaleExe
    if ($tsExe) {
        $tsIp = (& $tsExe ip -4 2>$null | Select-Object -First 1)
        if ($tsIp) {
            $tsIp = $tsIp.Trim()
            $visible = $adapters | Where-Object { $_.IPAddress -eq $tsIp }
            if ($visible) {
                Write-Ok "Tailscale: $tsIp (also present in the OS address table -- listeners can bind it)"
            } else {
                Write-Bad "Tailscale reports $tsIp but Windows does not list it on any interface"
                Write-Hint "Outbound works; an agent CANNOT listen on it. Run '-Action tailscale' for the full check."
            }
        }
    }

    Write-Step "Firewall"
    Get-NetFirewallProfile | Select-Object Name, Enabled | Format-Table -AutoSize | Out-String | Write-Host

    $publicNets = $adapters | Where-Object { $_.Category -eq 'Public' -and $_.IPAddress }
    if ($publicNets) {
        Write-Bad "these networks are marked PUBLIC, which makes Windows block inbound connections:"
        $publicNets | ForEach-Object { Write-Host "          $($_.Name) ($($_.IPAddress))" -ForegroundColor Red }
        Write-Hint "On the LAPTOP this is the single most common reason the agent is unreachable."
        Write-Hint "Fix: Set-NetConnectionProfile -InterfaceIndex <ifIndex> -NetworkCategory Private"
    } else {
        Write-Ok "no active network is marked Public"
    }

    if ($PeerIp) {
        Write-Step "Reachability of $PeerIp"

        $shared = $adapters | Where-Object {
            $_.IPAddress -and (Test-SameSubnet -A $_.IPAddress -B $PeerIp -Prefix $_.PrefixLen)
        }
        if ($shared) {
            Write-Ok "same subnet as $($shared[0].Name) ($($shared[0].IPAddress)/$($shared[0].PrefixLen))"
            Write-Info "so the addressing is fine -- if it still fails, it is a firewall or an AP-isolation problem"
        } else {
            Write-Bad "$PeerIp is NOT in the same subnet as any interface on this machine"
            $adapters | Where-Object { $_.IPAddress } | ForEach-Object {
                Write-Host "          this machine: $($_.IPAddress)/$($_.PrefixLen) on $($_.Name)" -ForegroundColor Red
            }
            Write-Hint "The router is not bridging your cable and Wi-Fi into one subnet."
            Write-Hint "Typical causes: a guest/IoT SSID, AP isolation, or two separate routers."
            Write-Hint "Use: .\link-setup.ps1 -Action direct-cable -Role controller|agent"
        }

        $ping = Test-Connection -ComputerName $PeerIp -Count 2 -Quiet -ErrorAction SilentlyContinue
        if ($ping) { Write-Ok "ICMP ping succeeds" }
        else { Write-Bad "ICMP ping fails (note: many Windows machines block ping even when TCP works)" }

        $tcp = Test-NetConnection -ComputerName $PeerIp -Port 8765 -WarningAction SilentlyContinue
        if ($tcp.TcpTestSucceeded) { Write-Ok "TCP 8765 is open -- the agent is reachable" }
        else { Write-Bad "TCP 8765 is closed or filtered (expected if the agent is not installed yet)" }
    } else {
        Write-Hint "Re-run with -PeerIp <other machine's address> for a reachability verdict."
    }

    Write-Step "Options, best first"
    Write-Host @"
  0. TAILSCALE (if it is installed -- solves addressing, not survivability)
     Stable 100.x addresses with no router configuration at all. Check it with:
         .\link-setup.ps1 -Action tailscale
     Caveat: Tailscale rides the internet, and evorift cuts the internet -- so the control
     link dies with it and every run leans on the deadman switch.

  1. DIRECT CABLE (recommended for this project)
     Plug the network cable straight between the two machines -- no router, no switch. Then:
         controller:  .\link-setup.ps1 -Action direct-cable -Role controller
         laptop:      .\link-setup.ps1 -Action direct-cable -Role agent
     The laptop keeps Wi-Fi for internet; the cable carries only the control link. Because the
     two paths are physically separate, the control link survives evorift killing the internet.

  2. WINDOWS MOBILE HOTSPOT
     If neither machine has a free Ethernet port. One machine shares its connection; both end up
     on 192.168.137.x and can see each other:
         .\link-setup.ps1 -Action hotspot

  3. PHONE HOTSPOT
     Both machines join the same phone hotspot. Simplest of all, no configuration, but the
     control link dies together with the internet when evorift cuts it -- you will be relying on
     the deadman switch every single run.

  4. FIX THE ROUTER
     If both machines are on the same router, put them on the same SSID/VLAN and turn off AP
     isolation ("client isolation" / "guest network"). Then no extra addressing is needed.
"@
    return
}

# ================================================================================================
# tailscale
# ================================================================================================

if ($Action -eq 'tailscale') {

    Write-Step "Tailscale"

    $exe = Get-TailscaleExe
    if (-not $exe) {
        Write-Bad "tailscale.exe not found"
        Write-Hint "Install it from https://tailscale.com/download, then re-run this."
        return
    }
    Write-Ok "CLI: $exe"
    Write-Info ((& $exe version 2>$null | Select-Object -First 1))

    $raw = & $exe status --json 2>$null | Out-String
    if (-not $raw.Trim()) {
        Write-Bad "'tailscale status --json' returned nothing -- the service is not running or you are not logged in"
        Write-Hint "Run: tailscale up"
        return
    }
    $status = $raw | ConvertFrom-Json

    if ($status.BackendState -ne 'Running') {
        Write-Bad "BackendState is '$($status.BackendState)', not 'Running'"
        Write-Hint "Run: tailscale up"
        return
    }
    Write-Ok "backend Running"

    $selfIp4 = @($status.Self.TailscaleIPs | Where-Object { $_ -notmatch ':' }) | Select-Object -First 1
    Write-Ok "this machine: $($status.Self.HostName)  ->  $selfIp4"
    if ($status.CurrentTailnet.MagicDNSSuffix) {
        Write-Info "MagicDNS suffix: $($status.CurrentTailnet.MagicDNSSuffix)"
    }

    Write-Step "Peers"
    $peers = @()
    if ($status.Peer) {
        $peers = $status.Peer.PSObject.Properties | ForEach-Object {
            $p = $_.Value
            [PSCustomObject]@{
                HostName = $p.HostName
                IPv4     = @($p.TailscaleIPs | Where-Object { $_ -notmatch ':' }) | Select-Object -First 1
                OS       = $p.OS
                Online   = $p.Online
            }
        }
    }
    if ($peers) {
        $peers | Format-Table HostName, IPv4, OS, Online -AutoSize | Out-String | Write-Host
    } else {
        Write-Bad "no peers -- the other machine is not on this tailnet yet"
        Write-Hint "Install Tailscale on the laptop and log in with the SAME account."
        return
    }

    # --- the check that actually decides whether this can work -----------------------------------
    Write-Step "Can the agent bind this machine's tailnet address?"

    Write-Info "this matters only on the LAPTOP -- that is where the agent listens"
    $bind = Test-Bindable -Ip $selfIp4
    if ($bind.Ok) {
        Write-Ok "$selfIp4 is bindable -- an agent on this machine could listen on the tailnet"
    } else {
        Write-Bad "$selfIp4 is NOT bindable: $($bind.Error)"
        Write-Host ""
        Write-Host "  Tailscale reports this address and traffic to it routes, but it is not on an" -ForegroundColor Red
        Write-Host "  OS interface, so no process can listen on it. Confirm with:" -ForegroundColor Red
        Write-Host "      Get-NetIPAddress -AddressFamily IPv4" -ForegroundColor Red
        Write-Host "  If $selfIp4 is missing from that list, tailscaled is running in" -ForegroundColor Red
        Write-Host "  userspace-networking mode. Outbound works, inbound listeners do not." -ForegroundColor Red
        Write-Host ""
        Write-Hint "Try: Restart-Service Tailscale   (elevated), then 'tailscale up' and re-run this"
        Write-Hint "If it stays unbindable, use -Action direct-cable instead -- it does not depend on this"
    }

    # Tailscale can block inbound itself, independently of Windows Firewall. Silent when set.
    Write-Step "Shields"
    $prefsRaw = & $exe debug prefs 2>$null | Out-String
    if ($prefsRaw -match '"ShieldsUp"\s*:\s*true') {
        Write-Bad "ShieldsUp is ON -- Tailscale is blocking ALL inbound connections to this machine"
        Write-Hint "On the laptop run: tailscale up --shields-up=false"
    } elseif ($prefsRaw -match '"ShieldsUp"') {
        Write-Ok "ShieldsUp is off (inbound permitted)"
    } else {
        Write-Info "could not read ShieldsUp; if the agent is unreachable but bindable, check 'tailscale debug prefs'"
    }

    Write-Step "Next"
    $laptop = $peers | Where-Object { $_.Online } | Select-Object -First 1
    Write-Host @"
  Tailscale gives you stable addresses with no router configuration, which solves the
  cable-versus-Wi-Fi problem cleanly. Be aware of the trade-off before you rely on it:

    Tailscale needs the internet. evorift cuts the internet. So when a test fails, the
    control link goes down WITH it, and every run depends on the deadman switch firing
    to bring the laptop back. That is exactly what the deadman is for, so this works --
    but you will be recovering blind rather than watching the failure happen.

    The direct cable (-Action direct-cable) puts the control link on a different physical
    interface, so it survives. If you have a spare Ethernet port, prefer it. Nothing stops
    you having both: Tailscale for convenience, the cable for the runs that matter.

  Build the kit pinned to THIS machine's tailnet address, on the controller:
      .\scripts\make-testd-kit.ps1 -ControllerIp $selfIp4

  Then on the laptop, elevated:
      .\install.ps1

  And back on the controller:
"@
    if ($laptop -and $laptop.IPv4) {
        Write-Host "      .\scripts\remote.ps1 -AgentIp $($laptop.IPv4) -Action health"
        Write-Host ""
        Write-Info "('$($laptop.HostName)' looks like the test laptop -- confirm that is the right peer)"
    } else {
        Write-Host "      .\scripts\remote.ps1 -AgentIp <the laptop's 100.x address> -Action health"
    }
    Write-Host ""
    return
}

# ================================================================================================
# revert
# ================================================================================================

if ($Action -eq 'revert') {
    if (-not (Test-Elevated)) { throw "revert changes IP configuration and must run elevated." }
    if (-not (Test-Path $JournalPath)) {
        throw "No rollback journal at $JournalPath -- nothing to revert (or it was never applied from this script)."
    }
    $journal = Get-Content $JournalPath -Raw | ConvertFrom-Json

    Write-Step "Reverting $($journal.AdapterName) to its previous configuration"
    Write-Info "journal written $($journal.AppliedAt)"

    # Remove the static address this script added.
    $existing = Get-NetIPAddress -AddressFamily IPv4 -InterfaceIndex $journal.ifIndex -ErrorAction SilentlyContinue |
                Where-Object { $_.IPAddress -eq $journal.AppliedIp }
    if ($existing) {
        Remove-NetIPAddress -InputObject $existing -Confirm:$false
        Write-Ok "removed static address $($journal.AppliedIp)"
    } else {
        Write-Info "static address $($journal.AppliedIp) is already gone"
    }

    if ($journal.WasDhcp) {
        Set-NetIPInterface -InterfaceIndex $journal.ifIndex -Dhcp Enabled
        Write-Ok "DHCP re-enabled"
    } elseif ($journal.PreviousIp) {
        New-NetIPAddress -InterfaceIndex $journal.ifIndex -IPAddress $journal.PreviousIp `
                         -PrefixLength $journal.PreviousPrefix -ErrorAction SilentlyContinue | Out-Null
        Write-Ok "restored the previous static address $($journal.PreviousIp)/$($journal.PreviousPrefix)"
    }

    if ($journal.PreviousCategory -and $journal.PreviousCategory -ne 'unknown') {
        Set-NetConnectionProfile -InterfaceIndex $journal.ifIndex -NetworkCategory $journal.PreviousCategory -ErrorAction SilentlyContinue
        Write-Ok "network category restored to $($journal.PreviousCategory)"
    }

    Rename-Item -Path $JournalPath -NewName ("link-rollback.reverted-{0}.json" -f (Get-Date -Format "yyyyMMdd-HHmmss"))
    Write-Ok "revert complete; journal archived"
    return
}

# ================================================================================================
# hotspot (guided -- Windows Mobile Hotspot has no supported PowerShell 5.1 API)
# ================================================================================================

if ($Action -eq 'hotspot') {
    Write-Step "Windows Mobile Hotspot"
    Write-Host @"
  Windows Mobile Hotspot cannot be toggled from Windows PowerShell 5.1 in any supported way
  (the old 'netsh wlan set hostednetwork' path is removed on current Windows builds, and the
  replacement is a WinRT API that is not reachable from this shell). So: do it in the UI, then
  come back here and verify.

  ON THE MACHINE THAT WILL SHARE (usually the desktop / controller):
    1. Settings -> Network and Internet -> Mobile hotspot
    2. "Share my Internet connection from": pick the adapter that HAS internet
    3. "Share over": Wi-Fi
    4. Note the network name and password, then turn it On

  ON THE OTHER MACHINE:
    5. Join that Wi-Fi network normally

  THEN, ON BOTH MACHINES:
    6. .\link-setup.ps1 -Action diagnose -PeerIp <other machine's 192.168.137.x address>

  Both should end up on 192.168.137.x -- the host is always 192.168.137.1. Set the profile to
  Private on both if diagnose reports Public.

  CAVEAT worth knowing before you rely on it: the hotspot link and the internet ride the same
  Wi-Fi adapter on the client. When evorift cuts the network, you lose the control link too, and
  every run will depend on the deadman switch firing. The direct cable does not have this
  problem. If you have two Ethernet ports between the machines, prefer -Action direct-cable.
"@
    Write-Step "Current state"
    Get-Ipv4Adapters | Format-Table Name, IPAddress, PrefixLen, Category, IsWifi -AutoSize | Out-String | Write-Host
    return
}

# ================================================================================================
# direct-cable
# ================================================================================================

if (-not $Role) {
    throw "-Role is required for direct-cable: 'controller' on your desktop, 'agent' on the test laptop."
}
if (-not (Test-Elevated)) {
    throw "direct-cable changes IP configuration and must run elevated."
}
if ($Subnet -notmatch '^\d{1,3}\.\d{1,3}\.\d{1,3}$') {
    throw "-Subnet must be the first three octets, e.g. 192.168.77"
}

$myIp   = "$Subnet." + $(if ($Role -eq 'controller') { '1' } else { '2' })
$peerIp = "$Subnet." + $(if ($Role -eq 'controller') { '2' } else { '1' })

Write-Step "Choosing the Ethernet adapter"

$adapters = @(Get-Ipv4Adapters)
if ($AdapterName) {
    $target = $adapters | Where-Object { $_.Name -eq $AdapterName } | Select-Object -First 1
    if (-not $target) {
        # It may be Up but with no IP yet, which is exactly the state of a fresh direct cable.
        $raw = Get-NetAdapter -Name $AdapterName -ErrorAction SilentlyContinue
        if (-not $raw) { throw "No adapter named '$AdapterName'. Available: $((Get-NetAdapter).Name -join ', ')" }
        $target = [PSCustomObject]@{
            Name = $raw.Name; ifIndex = $raw.ifIndex; IsWifi = $false
            IPAddress = $null; PrefixLen = $null; IsDhcp = $true; Gateway = $null; Category = 'unknown'
        }
    }
} else {
    # A directly-cabled adapter typically has no gateway and either no address or an APIPA one,
    # because there is no DHCP server on the other end. Prefer that; it is the giveaway.
    # Physical only -- a tunnel adapter (Tailscale, WireSock) also has "no gateway" and would
    # otherwise be a very plausible-looking wrong answer.
    $ethernet = $adapters | Where-Object { $_.IsPhysical -and -not $_.IsWifi -and -not $_.IsTailscale }
    if (-not $ethernet) {
        throw ("No wired adapter is Up. Plug the cable between the two machines first. " +
               "Adapters seen: " + ((Get-NetAdapter | ForEach-Object { "$($_.Name)=$($_.Status)" }) -join ', '))
    }
    $target = $ethernet | Where-Object { -not $_.Gateway } | Select-Object -First 1
    if (-not $target) { $target = $ethernet | Select-Object -First 1 }
}

Write-Ok "adapter: $($target.Name) (ifIndex $($target.ifIndex))"
Write-Info "current: IP=$($target.IPAddress) prefix=$($target.PrefixLen) gateway=$($target.Gateway) category=$($target.Category)"

if ($target.IsWifi) {
    throw "'$($target.Name)' is a Wi-Fi adapter. direct-cable is for the wired link; pass -AdapterName explicitly if this is wrong."
}

# Refuse to collide with a subnet already in use -- doing so would blackhole the machine's real
# network, which is precisely the kind of unreversible surprise this project forbids.
foreach ($a in $adapters) {
    if ($a.IPAddress -and $a.ifIndex -ne $target.ifIndex) {
        if (Test-SameSubnet -A $a.IPAddress -B $myIp -Prefix $PrefixLength) {
            throw ("Subnet $Subnet.0/$PrefixLength collides with $($a.Name) ($($a.IPAddress)/$($a.PrefixLen)). " +
                   "Pick another with -Subnet, e.g. -Subnet 10.77.77")
        }
    }
}
Write-Ok "subnet $Subnet.0/$PrefixLength does not collide with any existing network"

# Keeping the internet path intact is the whole point of the design, so say so explicitly and
# make it checkable afterwards.
$defaultRouteBefore = Get-NetRoute -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue |
                      Sort-Object RouteMetric | Select-Object -First 1
if ($defaultRouteBefore) {
    Write-Info "internet currently goes via ifIndex $($defaultRouteBefore.InterfaceIndex) (next hop $($defaultRouteBefore.NextHop))"
    if ($defaultRouteBefore.InterfaceIndex -eq $target.ifIndex) {
        Write-Warning "This adapter currently carries your default route. After this change it will not (no gateway is assigned to the link). Make sure Wi-Fi is connected first, or you will lose internet on this machine."
        if (-not $Force) {
            $answer = Read-Host "Continue anyway? [y/N]"
            if ($answer -notmatch '^(y|yes)$') { Write-Host "Aborted."; return }
        }
    }
}

Write-Step "Confirm"
Write-Host "  This machine  : $($target.Name)  ->  $myIp/$PrefixLength   (role: $Role)"
Write-Host "  The other one : must be           $peerIp/$PrefixLength"
Write-Host "  Default gateway on this link: NONE (deliberate -- internet keeps using Wi-Fi)"
Write-Host "  Network category will be set to: Private (so Windows permits inbound)"
Write-Host ""
if (-not $Force) {
    $answer = Read-Host "Apply? [y/N]"
    if ($answer -notmatch '^(y|yes)$') { Write-Host "Aborted. Nothing changed."; return }
}

# --- rollback journal BEFORE the change (hygiene rule 1: if it cannot be written, do not act) ---
New-Item -ItemType Directory -Force -Path $StateDir | Out-Null
$journal = [ordered]@{
    AppliedAt        = (Get-Date -Format "yyyy-MM-ddTHH:mm:ssK")
    Role             = $Role
    AdapterName      = $target.Name
    ifIndex          = $target.ifIndex
    AppliedIp        = $myIp
    AppliedPrefix    = $PrefixLength
    WasDhcp          = [bool]$target.IsDhcp
    PreviousIp       = $target.IPAddress
    PreviousPrefix   = $target.PrefixLen
    PreviousCategory = $target.Category
}
$journal | ConvertTo-Json -Depth 4 | Out-File -FilePath $JournalPath -Encoding utf8
if (-not (Test-Path $JournalPath)) { throw "Rollback journal could not be written -- refusing to change the network." }
Write-Ok "rollback journal written to $JournalPath"

Write-Step "Applying"

# Drop any existing IPv4 address on this adapter so the static one is unambiguous. APIPA
# addresses in particular linger and would leave two addresses on the interface.
Get-NetIPAddress -AddressFamily IPv4 -InterfaceIndex $target.ifIndex -ErrorAction SilentlyContinue |
    Where-Object { $_.IPAddress -ne $myIp } |
    ForEach-Object {
        try { Remove-NetIPAddress -InputObject $_ -Confirm:$false -ErrorAction Stop } catch {}
    }

# No -DefaultGateway. That is what keeps this link from stealing the internet route.
New-NetIPAddress -InterfaceIndex $target.ifIndex -IPAddress $myIp -PrefixLength $PrefixLength -ErrorAction Stop | Out-Null
Write-Ok "address $myIp/$PrefixLength assigned to $($target.Name)"

# Windows blocks most inbound traffic on Public networks; an unidentified direct cable defaults
# to Public, which would silently defeat the whole setup.
try {
    Start-Sleep -Seconds 2
    Set-NetConnectionProfile -InterfaceIndex $target.ifIndex -NetworkCategory Private -ErrorAction Stop
    Write-Ok "network category set to Private"
} catch {
    Write-Warning "Could not set the network category yet: $($_.Exception.Message)"
    Write-Hint "Windows sometimes needs the other end connected first. Re-run after the peer is configured:"
    Write-Hint "  Set-NetConnectionProfile -InterfaceIndex $($target.ifIndex) -NetworkCategory Private"
}

Write-Step "Verifying"

$after = Get-NetIPAddress -AddressFamily IPv4 -InterfaceIndex $target.ifIndex -ErrorAction SilentlyContinue |
         Where-Object { $_.IPAddress -eq $myIp }
if ($after) { Write-Ok "confirmed: $myIp is live on $($target.Name)" }
else { Write-Bad "address did not stick -- check 'Get-NetIPAddress -InterfaceIndex $($target.ifIndex)'" }

$defaultRouteAfter = Get-NetRoute -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue |
                     Sort-Object RouteMetric | Select-Object -First 1
if ($defaultRouteAfter) {
    if ($defaultRouteAfter.InterfaceIndex -eq $target.ifIndex) {
        Write-Bad "the default route now points at the direct link -- that is wrong and will break internet"
        Write-Hint "run '.\link-setup.ps1 -Action revert' and investigate before continuing"
    } else {
        Write-Ok "internet still routes via ifIndex $($defaultRouteAfter.InterfaceIndex), not the direct link"
    }
} else {
    Write-Warning "This machine now has no default route at all -- it has no internet path. Connect Wi-Fi."
}

$peerUp = Test-Connection -ComputerName $peerIp -Count 2 -Quiet -ErrorAction SilentlyContinue
if ($peerUp) { Write-Ok "the other machine answers at $peerIp -- the link is up" }
else { Write-Info "$peerIp does not answer yet (expected until you run this script on the other machine too)" }

Write-Host ""
Write-Host "================================================================" -ForegroundColor Yellow
Write-Host " DIRECT LINK CONFIGURED" -ForegroundColor Yellow
Write-Host "================================================================" -ForegroundColor Yellow
Write-Host ""
Write-Host " This machine ($Role): $myIp"
Write-Host " The other machine   : $peerIp"
Write-Host ""
if ($Role -eq 'agent') {
    Write-Host " Next, on THIS laptop, install the agent bound to the direct link:" -ForegroundColor Cyan
    Write-Host "   .\testd-install.ps1 -ControllerIp $peerIp -BindIp $myIp"
} else {
    Write-Host " Next, on the LAPTOP, run:" -ForegroundColor Cyan
    Write-Host "   .\link-setup.ps1 -Action direct-cable -Role agent -Subnet $Subnet"
    Write-Host "   .\testd-install.ps1 -ControllerIp $myIp -BindIp $peerIp"
    Write-Host ""
    Write-Host " Then back here:" -ForegroundColor Cyan
    Write-Host "   .\scripts\remote.ps1 -AgentIp $peerIp -Action health"
}
Write-Host ""
Write-Host " To undo: .\link-setup.ps1 -Action revert" -ForegroundColor DarkGray
Write-Host ""
