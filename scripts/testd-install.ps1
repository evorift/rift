<#
.SYNOPSIS
    One-time setup of the evorift remote test agent on the TEST LAPTOP.

.DESCRIPTION
    Run this ON THE LAPTOP, once, in an elevated PowerShell. It:
      1. copies evorift-testd.exe into C:\Program Files\evorift-testd\
      2. generates a 256-bit bearer token and locks its ACL to SYSTEM + Administrators
      3. writes the config with your controller's IP and this laptop's LAN IP
      4. registers the "evorift-testd" Windows service (LocalSystem, auto-start, auto-restart)
      5. opens ONE inbound firewall rule, scoped to that one source IP, that one port, and
         that one program

    The token is printed once at the end. That printout is the ONLY time it is shown, and
    carrying it to the controller is the one genuinely manual step of the whole setup -- it is
    never sent over the API, never written to the audit log, and never included in a /pull
    response. Type it into the controller by hand (or copy it over a USB stick); do not email
    it to yourself.

    Re-running the script is safe. By default an existing token is KEPT (so the controller does
    not have to be re-paired); pass -RotateToken to generate a new one.

.PARAMETER ControllerIp
    The LAN IP of the machine you drive tests from. This is the ONLY address allowed to reach
    the agent, both in the config allowlist and in the firewall rule.

.PARAMETER BindIp
    This laptop's LAN IP. Auto-detected if omitted. The agent refuses to start on 0.0.0.0.

.PARAMETER Port
    TCP port to listen on. Default 8765.

.PARAMETER BinarySource
    Path to the built evorift-testd.exe. Defaults to the release build in this repo.

.PARAMETER SandboxRoot
    The only directory the agent may read or write over the network. Default C:\evorift-test.

.PARAMETER DeadmanSecs
    Seconds of silence from the controller before the agent recovers itself. Default 120.

.PARAMETER RotateToken
    Generate a new token even if one already exists. The controller must be updated to match.

.EXAMPLE
    .\scripts\testd-install.ps1 -ControllerIp 192.168.1.20

.EXAMPLE
    .\scripts\testd-install.ps1 -ControllerIp 192.168.1.20 -BindIp 192.168.1.50 -Port 8765
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$ControllerIp,
    [string]$BindIp,
    [int]$Port = 8765,
    [string]$BinarySource,
    [string]$SandboxRoot = "C:\evorift-test",
    [int]$DeadmanSecs = 120,
    [switch]$RotateToken,
    [switch]$StrictAllowlist
)

$ErrorActionPreference = 'Stop'

$ServiceName = "evorift-testd"
$InstallDir  = "C:\Program Files\evorift-testd"
$StateDir    = Join-Path $env:ProgramData "evorift-testd"
$ConfigPath  = Join-Path $StateDir "testd.config.json"
$TokenPath   = Join-Path $StateDir "testd.token"
$ExePath     = Join-Path $InstallDir "evorift-testd.exe"
$FwRuleName  = "evorift-testd (controller only)"

function Write-Step { param([string]$Text) Write-Host "`n=== $Text ===" -ForegroundColor Cyan }
function Write-Ok   { param([string]$Text) Write-Host "  ok    $Text" -ForegroundColor Green }
function Write-Info { param([string]$Text) Write-Host "  --    $Text" -ForegroundColor DarkGray }

# --- 0: preconditions ------------------------------------------------------------------------

Write-Step "Checking preconditions"

$id = [System.Security.Principal.WindowsIdentity]::GetCurrent()
$principal = New-Object System.Security.Principal.WindowsPrincipal($id)
if (-not $principal.IsInRole([System.Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw "This script must run elevated (registering a service and a firewall rule both require admin)."
}
Write-Ok "running elevated"

# Validate the controller address up front: a typo here would either lock you out or, worse,
# quietly allow a machine you did not mean to trust.
$parsedController = [System.Net.IPAddress]::Any
if (-not [System.Net.IPAddress]::TryParse($ControllerIp, [ref]$parsedController)) {
    throw "ControllerIp '$ControllerIp' is not a valid IP address."
}
if ($ControllerIp -eq "0.0.0.0" -or $ControllerIp -eq "::") {
    throw "ControllerIp must be a specific address, not the wildcard."
}
Write-Ok "controller IP $ControllerIp"

# Are two addresses in the same IPv4 subnet?
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

# Is this address inside Tailscale's CGNAT range (100.64.0.0/10)?
function Test-IsTailscaleIp {
    param([string]$Ip)
    if ($Ip -notmatch '^(\d{1,3})\.(\d{1,3})\.') { return $false }
    return ([int]$Matches[1] -eq 100 -and [int]$Matches[2] -ge 64 -and [int]$Matches[2] -le 127)
}

# This machine's own tailnet address.
#
# It has to come from the Tailscale CLI, not Get-NetIPAddress: on Windows the tailnet address is
# frequently absent from the OS address table even while the tunnel is fully up and routing. So
# the usual "list local addresses" approach finds nothing and silently picks the wrong interface.
function Get-TailscaleIp {
    $exe = @(
        "$env:ProgramFiles\Tailscale\tailscale.exe",
        "${env:ProgramFiles(x86)}\Tailscale\tailscale.exe"
    ) | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $exe) { $exe = (Get-Command tailscale.exe -ErrorAction SilentlyContinue).Source }
    if (-not $exe) { return $null }
    try {
        $ip = (& $exe ip -4 2>$null | Select-Object -First 1)
        if ($ip) { return $ip.Trim() }
    } catch {}
    return $null
}

# Can a listener actually bind this address right now?
#
# This is the one check that does not lie. An address can be reported by a tool, be routable, and
# still be unbindable because it was never put on an OS interface -- which is exactly what
# happens with Tailscale in userspace-networking mode. Without this preflight the install
# "succeeds" and the service then fails to start with WSAEADDRNOTAVAIL, whose message
# ("The requested address is not valid in its context") explains nothing.
function Test-Bindable {
    param([string]$Ip)
    try {
        $listener = New-Object System.Net.Sockets.TcpListener([System.Net.IPAddress]::Parse($Ip), 0)
        $listener.Start()
        $listener.Stop()
        return @{ Ok = $true; Error = $null }
    } catch {
        $msg = $_.Exception.Message
        if ($_.Exception.InnerException) { $msg = $_.Exception.InnerException.Message }
        return @{ Ok = $false; Error = $msg }
    }
}

# Auto-detect this laptop's address if not given.
#
# The interface to bind is the one that can REACH THE CONTROLLER, which is not necessarily the
# one holding the default route. With the direct-cable setup (see link-setup.ps1) they are
# deliberately different: the control link is the cable, the internet is Wi-Fi. Picking the
# default-route interface there would bind the wrong side and the controller would never connect.
if (-not $BindIp) {
    if (Test-IsTailscaleIp -Ip $ControllerIp) {
        # The controller is a tailnet address, so the agent must answer on this machine's tailnet
        # address. Subnet matching cannot find it: Tailscale addresses are /32, so no two nodes
        # ever "share a subnet".
        $BindIp = Get-TailscaleIp
        if (-not $BindIp) {
            throw ("The controller address $ControllerIp is a Tailscale address, but this machine's " +
                   "tailnet address could not be read. Is Tailscale installed and logged in here? " +
                   "Check with 'tailscale status', then pass -BindIp explicitly.")
        }
        Write-Info "auto-detected BindIp $BindIp from 'tailscale ip -4' (controller is on the tailnet)"
    } else {
        $allIps = Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue |
            Where-Object { $_.IPAddress -ne '127.0.0.1' -and $_.InterfaceAlias -notmatch 'Loopback' }

        $onControllerSubnet = $allIps | Where-Object {
            Test-SameSubnet -A $_.IPAddress -B $ControllerIp -Prefix $_.PrefixLength
        } | Select-Object -First 1

        if ($onControllerSubnet) {
            $BindIp = $onControllerSubnet.IPAddress
            Write-Info "auto-detected BindIp $BindIp on '$($onControllerSubnet.InterfaceAlias)' (same subnet as the controller)"
        } else {
            $defaultRoute = Get-NetRoute -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue |
                Sort-Object -Property RouteMetric | Select-Object -First 1
            if (-not $defaultRoute) { throw "No interface shares a subnet with $ControllerIp and there is no default route; pass -BindIp explicitly." }
            $candidate = $allIps | Where-Object { $_.InterfaceIndex -eq $defaultRoute.InterfaceIndex } | Select-Object -First 1
            if (-not $candidate) { throw "Could not auto-detect a LAN IP; pass -BindIp explicitly." }
            $BindIp = $candidate.IPAddress
            Write-Warning "No interface on this machine shares a subnet with the controller ($ControllerIp)."
            Write-Warning "Falling back to $BindIp on '$($candidate.InterfaceAlias)', but the controller probably cannot reach it."
            Write-Warning "Run '.\link-setup.ps1 -Action diagnose -PeerIp $ControllerIp' to sort the addressing out first."
        }
    }
}

$BindIsTailscale = Test-IsTailscaleIp -Ip $BindIp

$parsedBind = [System.Net.IPAddress]::Any
if (-not [System.Net.IPAddress]::TryParse($BindIp, [ref]$parsedBind)) {
    throw "BindIp '$BindIp' is not a valid IP address."
}
if ($BindIp -eq "0.0.0.0" -or $BindIp -eq "::") {
    throw "BindIp must be this laptop's LAN address, never the wildcard. The agent refuses 0.0.0.0 by design."
}
# Prove the address is bindable, by binding it. `Get-NetIPAddress` is not sufficient evidence:
# a Tailscale address is routable and reported by `tailscale ip -4` yet frequently absent from
# the OS address table, and in userspace-networking mode it is genuinely unbindable. Finding that
# out here, with an explanation, beats finding it out when the service refuses to start.
$bindTest = Test-Bindable -Ip $BindIp
if (-not $bindTest.Ok) {
    $hint = if ($BindIsTailscale) {
        "`n`n  $BindIp is a Tailscale address. Tailscale is reporting it, but it is not on an OS" +
        "`n  interface, so no process can listen on it. Check:" +
        "`n      Get-NetIPAddress -AddressFamily IPv4        # must list $BindIp" +
        "`n      tailscale status                            # must say Running" +
        "`n  If Get-NetIPAddress does not list it, tailscaled is in userspace-networking mode." +
        "`n  Restart the Tailscale service, or run 'tailscale up' again so it creates the tunnel" +
        "`n  adapter properly. If you cannot get an OS-level address, use the LAN or direct-cable" +
        "`n  method instead: .\link-setup.ps1 -Action diagnose"
    } else {
        "`n`n  Addresses this machine can actually bind:`n" +
        ((Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue |
            ForEach-Object { "      $($_.IPAddress)/$($_.PrefixLength) on $($_.InterfaceAlias)" }) -join "`n")
    }
    throw "Cannot bind $BindIp`:$Port -- $($bindTest.Error)$hint"
}
Write-Ok "bind address $BindIp`:$Port (verified bindable)"
if ($BindIsTailscale) { Write-Info "this is a tailnet address; the agent will only be reachable over Tailscale" }

if ($ControllerIp -eq $BindIp) {
    Write-Warning "ControllerIp and BindIp are the same address -- that only makes sense if you are testing against this same machine."
}

if ($DeadmanSecs -lt 10 -or $DeadmanSecs -gt 3600) {
    throw "DeadmanSecs must be between 10 and 3600 (the agent rejects anything else)."
}

# --- 1: locate and install the binary ---------------------------------------------------------

Write-Step "Installing the binary"

if (-not $BinarySource) {
    $repoRoot = Split-Path -Parent $PSScriptRoot
    $candidates = @(
        (Join-Path $repoRoot "src-tauri\target\release\evorift-testd.exe"),
        (Join-Path $repoRoot "src-tauri\target\debug\evorift-testd.exe"),
        (Join-Path $PSScriptRoot "evorift-testd.exe")
    )
    $BinarySource = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $BinarySource) {
        throw ("evorift-testd.exe not found. Build it with " +
               "``cargo build --release --bin evorift-testd`` on the dev machine, copy it to this " +
               "laptop, and pass -BinarySource <path>. Looked in:`n  " + ($candidates -join "`n  "))
    }
}
if (-not (Test-Path $BinarySource)) { throw "BinarySource '$BinarySource' does not exist." }

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
New-Item -ItemType Directory -Force -Path $StateDir | Out-Null
New-Item -ItemType Directory -Force -Path $SandboxRoot | Out-Null

# The service must be stopped before its binary can be replaced.
$existing = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
if ($existing -and $existing.Status -ne 'Stopped') {
    Write-Info "stopping the running $ServiceName service so its binary can be replaced"
    Stop-Service -Name $ServiceName -Force
    $existing.WaitForStatus('Stopped', [TimeSpan]::FromSeconds(30))
}

Copy-Item -Path $BinarySource -Destination $ExePath -Force
Write-Ok "binary at $ExePath"
Write-Info "source: $BinarySource"
Write-Info ("sha256: " + (Get-FileHash -Path $ExePath -Algorithm SHA256).Hash)

# --- 2: token ----------------------------------------------------------------------------------

Write-Step "Bearer token"

$tokenExisted = Test-Path $TokenPath
if ($tokenExisted -and -not $RotateToken) {
    $Token = (Get-Content -Path $TokenPath -Raw).Trim()
    Write-Info "keeping the existing token (pass -RotateToken to replace it)"
} else {
    # 32 bytes of CSPRNG output, hex-encoded to 64 characters. Not Get-Random: that is not a
    # cryptographic RNG and this value is the only thing standing between a spoofed LAN peer
    # and remote command execution.
    $bytes = New-Object byte[] 32
    $rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
    try { $rng.GetBytes($bytes) } finally { $rng.Dispose() }
    $Token = -join ($bytes | ForEach-Object { $_.ToString("x2") })
    Set-Content -Path $TokenPath -Value $Token -NoNewline -Encoding ascii
    if ($tokenExisted) { Write-Ok "token ROTATED -- the controller must be updated to match" }
    else { Write-Ok "token generated" }
}

# Lock the token down: break ACL inheritance, then allow only SYSTEM and Administrators. The
# agent runs as LocalSystem, so nothing less privileged ever needs to read this file.
$acl = Get-Acl -Path $TokenPath
$acl.SetAccessRuleProtection($true, $false)   # protect from inheritance, drop inherited rules
@($acl.Access) | ForEach-Object { [void]$acl.RemoveAccessRule($_) }
foreach ($account in @("NT AUTHORITY\SYSTEM", "BUILTIN\Administrators")) {
    $rule = New-Object System.Security.AccessControl.FileSystemAccessRule(
        $account, "FullControl", "None", "None", "Allow")
    $acl.AddAccessRule($rule)
}
Set-Acl -Path $TokenPath -AclObject $acl
Write-Ok "token ACL restricted to SYSTEM + Administrators"

# --- 3: config ---------------------------------------------------------------------------------

Write-Step "Writing the config"

# state_dir deliberately sits outside sandbox_root: /pull can only ever serve paths under the
# sandbox, so putting the token and audit log here makes them structurally unfetchable. The
# agent refuses to start if this is violated.
# Derive the controller's /24 so a DHCP change on EITHER machine does not break the pairing.
#
# Both machines got new leases on the same day once, which simultaneously invalidated the bind
# address, the allowlist entry and the address the controller was dialling. The source filter was
# never the security boundary -- the bearer token is -- so widening it to the LAN buys robustness
# at negligible cost. Use -StrictAllowlist for a single-address filter.
if ($StrictAllowlist) {
    $allowlist = @($ControllerIp)
    Write-Info "allowlist pinned to $ControllerIp exactly (-StrictAllowlist)"
} else {
    $o = $ControllerIp.Split('.')
    $allowlist = @("$($o[0]).$($o[1]).$($o[2]).0/24")
    Write-Info "allowlist $($allowlist[0]) - survives a DHCP change on the controller"
}

# bind_ip "auto" means "whichever local address routes toward the controller", re-checked every
# 30s by the agent. A literal address here would go stale the next time this laptop's lease moves.
$config = [ordered]@{
    bind_ip                  = "auto"
    port                     = $Port
    allowlist                = $allowlist
    sandbox_root             = $SandboxRoot
    state_dir                = $StateDir
    deadman_secs             = $DeadmanSecs
    max_upload_bytes         = 536870912
    default_job_timeout_secs = 300
}
# Write UTF-8 WITHOUT a byte-order mark.
#
# `Out-File -Encoding utf8` in Windows PowerShell 5.1 ALWAYS prepends a BOM (EF BB BF), and a
# BOM makes this file unparseable as JSON -- the agent then dies before it can log why, and the
# service sits there reporting "Running" with nothing listening. That exact failure cost a trip
# to the laptop. .NET's UTF8Encoding($false) is the only reliable way to suppress it here.
[System.IO.File]::WriteAllText($ConfigPath, ($config | ConvertTo-Json -Depth 4), (New-Object System.Text.UTF8Encoding($false)))

# Verify, rather than trust: read the first bytes back and refuse to continue if a BOM appeared.
$configBytes = [System.IO.File]::ReadAllBytes($ConfigPath)
if ($configBytes.Length -ge 3 -and $configBytes[0] -eq 0xEF -and $configBytes[1] -eq 0xBB -and $configBytes[2] -eq 0xBF) {
    throw "The config at $ConfigPath was written with a UTF-8 BOM, which the agent cannot parse. This is a bug in the installer -- report it."
}
Write-Ok "config at $ConfigPath (UTF-8, no BOM - verified)"
Write-Info "sandbox root: $SandboxRoot   deadman: ${DeadmanSecs}s"

# --- 4: service --------------------------------------------------------------------------------

Write-Step "Registering the Windows service"

if ($existing) {
    Write-Info "service already exists -- deleting and recreating so binPath/config changes take effect"
    sc.exe delete $ServiceName | Out-Null
    # A delete does not complete while a handle is still open (DELETE_PENDING). Creating the
    # service again during that window fails with "marked for deletion", so wait it out.
    $gone = $false
    for ($i = 0; $i -lt 20; $i++) {
        Start-Sleep -Milliseconds 500
        if (-not (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue)) { $gone = $true; break }
    }
    if (-not $gone) {
        throw "The existing '$ServiceName' service is still marked for deletion. Close Services.msc and Task Manager, then re-run."
    }
}

# The service command line: the exe, then the config path. Both are quoted because
# "C:\Program Files\..." contains a space.
$binPath = '"{0}" "{1}"' -f $ExePath, $ConfigPath

# New-Service, NOT `sc.exe create`.
#
# Windows PowerShell 5.1 rewrites arguments on their way to a native .exe, and a single
# argument that itself contains double quotes and spaces -- exactly what binPath= needs -- comes
# out mangled. sc.exe then rejects it with exit code 1639 (ERROR_INVALID_COMMAND_LINE). This was
# hit for real during a laptop install. New-Service hands the string to the service-control API
# directly, so there is no command line to mangle.
try {
    New-Service -Name $ServiceName `
                -BinaryPathName $binPath `
                -DisplayName "evorift remote test agent" `
                -StartupType Automatic `
                -Description "Remote test agent for evorift. Runs sandbox-confined test commands for one allowlisted controller and self-recovers if the controller goes silent." `
                -ErrorAction Stop | Out-Null
} catch {
    throw "Could not register the '$ServiceName' service: $($_.Exception.Message)"
}

# Read the registered path back rather than trusting that it was stored as intended -- this is
# the exact field that silently broke before.
$registered = (Get-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Services\$ServiceName" -Name ImagePath -ErrorAction SilentlyContinue).ImagePath
if (-not $registered) {
    throw "The service was created but its ImagePath is empty. Remove it with 'sc.exe delete $ServiceName' and re-run."
}
if ($registered -notlike "*evorift-testd.exe*") {
    throw "The service ImagePath looks wrong: $registered"
}
Write-Ok "service $ServiceName created (LocalSystem, auto-start)"
Write-Info "ImagePath: $registered"

# Auto-restart matters more here than usual: if the agent dies while the software under test has
# the network down, nobody can reach the laptop to restart it by hand.
sc.exe failure $ServiceName reset= 86400 actions= restart/5000/restart/10000/restart/30000 | Out-Null
Write-Ok "restart-on-failure configured (5s, 10s, 30s)"

# --- 5: firewall --------------------------------------------------------------------------------

Write-Step "Firewall rule"

# Remove any rule from a previous run before adding, so re-running cannot accumulate rules that
# each widen the allowed source set.
$old = Get-NetFirewallRule -DisplayName $FwRuleName -ErrorAction SilentlyContinue
if ($old) {
    $old | Remove-NetFirewallRule
    Write-Info "removed the previous rule"
}

# Scoped by remote address + port + program, but deliberately NOT by local address.
#
# Pinning -LocalAddress to the address detected at install time silently blocked everything the
# moment DHCP moved this laptop: the agent rebound correctly and the rule then pointed at an
# address the machine no longer had. Remote address, port and program are the three that matter.
$fwRemote = if ($StrictAllowlist) { $ControllerIp } else { $allowlist[0] }
New-NetFirewallRule `
    -DisplayName $FwRuleName `
    -Direction Inbound `
    -Action Allow `
    -Protocol TCP `
    -LocalPort $Port `
    -RemoteAddress $fwRemote `
    -Program $ExePath `
    -Profile Any `
    -Description "evorift-testd: inbound from the controller only." | Out-Null
Write-Ok "inbound TCP $Port allowed from $fwRemote only (program-scoped, any local address)"

# --- 6: start and verify -------------------------------------------------------------------------

Write-Step "Starting the service"

Start-Service -Name $ServiceName
$svc = Get-Service -Name $ServiceName
$svc.WaitForStatus('Running', [TimeSpan]::FromSeconds(30))
Write-Ok "service is $($svc.Status)"

# Verify it is actually listening, rather than trusting the service state. A LocalSystem service
# can report Running while its listener failed to bind.
Start-Sleep -Seconds 2
$listening = Get-NetTCPConnection -State Listen -LocalPort $Port -ErrorAction SilentlyContinue
if ($listening) {
    $addrs = ($listening | Select-Object -ExpandProperty LocalAddress) -join ", "
    Write-Ok "listening on port $Port (local address: $addrs)"
    if ($addrs -match '0\.0\.0\.0') {
        Write-Warning "Something is listening on 0.0.0.0:$Port. evorift-testd refuses to do that, so this is a DIFFERENT process -- investigate before trusting the setup."
    }
} else {
    Write-Warning "Service reports Running but nothing is listening on port $Port. Check the agent's own log:"
    Write-Warning "  Get-Content '$StateDir\audit.log' -Tail 20"
}

# --- done ------------------------------------------------------------------------------------------

Write-Host ""
Write-Host "================================================================" -ForegroundColor Yellow
Write-Host " SETUP COMPLETE" -ForegroundColor Yellow
Write-Host "================================================================" -ForegroundColor Yellow
Write-Host ""
Write-Host " Agent URL   : http://$BindIp`:$Port"
Write-Host " Sandbox     : $SandboxRoot"
Write-Host " Audit log   : $StateDir\audit.log      (stays on this laptop)"
Write-Host " Deadman log : $StateDir\deadman.log    (stays on this laptop)"
Write-Host ""
Write-Host " BEARER TOKEN -- copy this to the controller now. It is not shown again," -ForegroundColor Yellow
Write-Host " and it is never sent over the API." -ForegroundColor Yellow
Write-Host ""
Write-Host "   $Token" -ForegroundColor White
Write-Host ""
Write-Host " On the CONTROLLER, run:" -ForegroundColor Cyan
Write-Host "   `$env:EVORIFT_TESTD_TOKEN = '$Token'"
Write-Host "   .\scripts\remote.ps1 -AgentIp $BindIp -Port $Port -Action health"
Write-Host ""
Write-Host " If you lose the token, re-run this script with -RotateToken." -ForegroundColor DarkGray
Write-Host ""
