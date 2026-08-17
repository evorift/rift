<#
.SYNOPSIS
    Diagnose why the test agent is unreachable. Run ON THE LAPTOP (CHECK.bat does this for you).

.DESCRIPTION
    Read-only. Checks, in the order things actually break:

      1. did the install finish (binary, config, token on disk)?
      2. does the service exist, and is it Running?
      3. is anything actually LISTENING on the port?
      4. does the inbound firewall rule exist, and is it scoped correctly?
      5. does the configured bind address still exist on this machine?
      6. what do the agent's own logs say?

    Ends with a single VERDICT line naming the most likely cause and the fix, because the raw
    output above it is only useful if you already know what you are looking at.

.PARAMETER OutFile
    Also write everything to this file, so it can be carried back on the USB stick.
#>
[CmdletBinding()]
param([string]$OutFile)

$ErrorActionPreference = 'Continue'

$ServiceName = "evorift-testd"
$StateDir    = Join-Path $env:ProgramData "evorift-testd"
$ConfigPath  = Join-Path $StateDir "testd.config.json"
$TokenPath   = Join-Path $StateDir "testd.token"
$ExePath     = "C:\Program Files\evorift-testd\evorift-testd.exe"
$FwRuleName  = "evorift-testd (controller only)"

$lines = New-Object System.Collections.Generic.List[string]
function Say { param([string]$Text) $lines.Add($Text); Write-Host $Text }
function Head { param([string]$Text) Say ""; Say "=== $Text ===" }

Say "evorift test agent - diagnostic"
Say "host: $env:COMPUTERNAME   time: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')"

$problems = New-Object System.Collections.Generic.List[string]

# --- 1: did the install finish? -----------------------------------------------------------------
Head "1. Installed files"
foreach ($f in @(@{P=$ExePath;N="binary"}, @{P=$ConfigPath;N="config"}, @{P=$TokenPath;N="token"})) {
    if (Test-Path $f.P) { Say "  OK      $($f.N): $($f.P)" }
    else { Say "  MISSING $($f.N): $($f.P)"; $problems.Add("install-incomplete") }
}

$bindIp = $null; $port = $null; $allow = $null
if (Test-Path $ConfigPath) {
    try {
        $cfg = Get-Content $ConfigPath -Raw | ConvertFrom-Json
        $bindIp = $cfg.bind_ip; $port = $cfg.port; $allow = ($cfg.allowlist -join ', ')
        Say "  config: bind_ip=$bindIp port=$port allowlist=[$allow]"
    } catch { Say "  config is NOT valid JSON: $($_.Exception.Message)"; $problems.Add("bad-config") }
}

# --- 2: the service -------------------------------------------------------------------------------
Head "2. Service"
$svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
if (-not $svc) {
    Say "  NOT INSTALLED - the '$ServiceName' service does not exist"
    $problems.Add("no-service")
} else {
    Say "  status: $($svc.Status)   startup: $($svc.StartType)"
    $img = (Get-ItemProperty "HKLM:\SYSTEM\CurrentControlSet\Services\$ServiceName" -Name ImagePath -ErrorAction SilentlyContinue).ImagePath
    Say "  ImagePath: $img"
    if ($svc.Status -ne 'Running') { $problems.Add("service-not-running") }
}

# --- 3: is anything listening? ----------------------------------------------------------------------
Head "3. Listening sockets"
if ($port) {
    $listen = Get-NetTCPConnection -State Listen -LocalPort $port -ErrorAction SilentlyContinue
    if ($listen) {
        $listen | ForEach-Object { Say "  LISTENING on $($_.LocalAddress):$($_.LocalPort)  (pid $($_.OwningProcess))" }
    } else {
        Say "  NOTHING is listening on port $port"
        $problems.Add("not-listening")
    }
} else { Say "  (no port known - config missing)" }

# --- 4: firewall ---------------------------------------------------------------------------------------
Head "4. Firewall rule"
$rule = Get-NetFirewallRule -DisplayName $FwRuleName -ErrorAction SilentlyContinue
if (-not $rule) {
    Say "  MISSING - no inbound rule named '$FwRuleName'"
    $problems.Add("no-firewall-rule")
} else {
    Say "  found: enabled=$($rule.Enabled) action=$($rule.Action) profile=$($rule.Profile)"
    try {
        $addr = $rule | Get-NetFirewallAddressFilter
        $pf   = $rule | Get-NetFirewallPortFilter
        Say "  local address : $($addr.LocalAddress)"
        Say "  remote address: $($addr.RemoteAddress)   <- only this machine may connect"
        Say "  local port    : $($pf.LocalPort)/$($pf.Protocol)"
    } catch { Say "  (could not read the rule's filters: $($_.Exception.Message))" }
    if ($rule.Enabled -ne 'True') { $problems.Add("firewall-rule-disabled") }
}

# --- 5: does the bind address still exist? -----------------------------------------------------------------
Head "5. Addresses on this machine"
Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue |
    Where-Object { $_.IPAddress -ne '127.0.0.1' } |
    ForEach-Object { Say "  $($_.IPAddress)/$($_.PrefixLength) on $($_.InterfaceAlias)" }
if ($bindIp) {
    if (Get-NetIPAddress -IPAddress $bindIp -ErrorAction SilentlyContinue) {
        Say "  OK - configured bind_ip $bindIp exists here"
    } else {
        Say "  PROBLEM - configured bind_ip $bindIp is NOT on this machine any more (DHCP moved it?)"
        $problems.Add("bind-ip-gone")
    }
}
Get-NetConnectionProfile -ErrorAction SilentlyContinue |
    ForEach-Object { Say "  network '$($_.Name)' category=$($_.NetworkCategory)" }

# --- 6: the agent's own logs -----------------------------------------------------------------------------------
# The agent writes here when it cannot even reach the point of opening its audit log. This is
# usually the single most informative line in the whole report.
Head "6a. Startup failures (startup-error.log)"
$startupLog = Join-Path $StateDir "startup-error.log"
if (Test-Path $startupLog) {
    Get-Content $startupLog -Tail 8 | ForEach-Object { Say "  $_" }
    $problems.Add("startup-error")
} else {
    Say "  none recorded"
}

Head "6. Agent log (last 15 lines of audit.log)"
$auditLog = Join-Path $StateDir "audit.log"
if (Test-Path $auditLog) {
    Get-Content $auditLog -Tail 15 | ForEach-Object { Say "  $_" }
} else {
    Say "  no audit.log - the agent has never started successfully"
    $problems.Add("never-started")
}

Head "7. Windows service errors (last 5)"
$evt = Get-WinEvent -FilterHashtable @{LogName='System'; Id=7000,7001,7009,7023,7024,7031,7034} -MaxEvents 5 -ErrorAction SilentlyContinue |
       Where-Object { $_.Message -match 'evorift' }
if ($evt) { $evt | ForEach-Object { Say "  $($_.TimeCreated): $($_.Message.Split("`n")[0])" } }
else { Say "  none mentioning evorift" }

# --- verdict -----------------------------------------------------------------------------------------------------
Head "VERDICT"
if ($problems -contains "install-incomplete" -or $problems -contains "no-service") {
    Say "  The install did not complete. The service was never created."
    Say "  FIX: double-click INSTALL.bat again and read the red text if it stops."
} elseif ($problems -contains "startup-error") {
    Say "  The agent started and then failed. Section 6a above says exactly why -- that"
    Say "  message is the answer, not a symptom."
    Say "  FIX: if it mentions the config, double-click INSTALL.bat again (it rewrites"
    Say "       the config correctly). Otherwise send section 6a back."
} elseif ($problems -contains "service-not-running" -or $problems -contains "not-listening") {
    Say "  The service exists but is not serving. It most likely failed to bind its address."
    Say "  FIX: run this to see the real reason, then send me the output:"
    Say "         & '$ExePath' --console '$ConfigPath'"
    Say "       That runs the agent in the foreground and prints why it stops."
} elseif ($problems -contains "no-firewall-rule") {
    Say "  The agent IS listening, but no firewall rule lets the controller in -- so"
    Say "  connections are dropped and the controller sees a timeout."
    Say "  FIX: double-click INSTALL.bat again; it recreates the rule."
} elseif ($problems -contains "bind-ip-gone") {
    Say "  This laptop's IP changed, so the agent is bound to an address it no longer has."
    Say "  FIX: double-click INSTALL.bat again to re-detect and rebind."
} elseif ($problems.Count -eq 0) {
    Say "  Everything looks correct on this laptop: service running, listening, rule present."
    Say "  If the controller still times out, the block is between the machines"
    Say "  (a second firewall, or the controller's address is not the one in the allowlist)."
} else {
    Say "  Issues found: $($problems -join ', ')"
}

Say ""
Say "-----------------------------------------------------------------"
Say " Send the whole of this output back. The VERDICT above is a guess;"
Say " the sections above it are the evidence."
Say "-----------------------------------------------------------------"

if ($OutFile) {
    try {
        $lines | Out-File -FilePath $OutFile -Encoding utf8
        Write-Host ""
        Write-Host "  Saved to: $OutFile" -ForegroundColor Green
    } catch {
        Write-Host "  Could not save to ${OutFile}: $($_.Exception.Message)" -ForegroundColor Red
    }
}
