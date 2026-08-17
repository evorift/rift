<#
.SYNOPSIS
    Build the self-contained remote-test kit: one folder (and one .zip) to carry to the laptop.

.DESCRIPTION
    Run this ON THE CONTROLLER. It builds evorift-testd.exe, gathers every file the laptop needs,
    writes a checksum manifest and a step-by-step README, and zips the lot.

    The point of a kit is that the laptop side needs NO repo, NO toolchain and NO internet --
    which matters, because the laptop is the machine whose internet the software under test
    breaks. Copy the folder over on a USB stick and everything needed is in it.

    Output:
        dist\testd-kit\            the kit folder
        dist\testd-kit.zip         the same thing zipped, for copying

    Contents:
        evorift-testd.exe          the agent
        install.ps1                one-shot entry point: link check -> install -> print token
        testd-install.ps1          the real installer (called by install.ps1)
        link-setup.ps1             IP/link diagnosis and the direct-cable method
        capture-state.ps1          the state capture the agent runs
        README.txt                 what to do, in order
        MANIFEST.txt               SHA-256 of every file in the kit

.PARAMETER ControllerIp
    Your machine's LAN address, baked into README.txt and install.ps1's default so the laptop
    side is a single command with no arguments to remember. Auto-detected if omitted.

.PARAMETER Port
    Agent port to pre-fill. Default 8765.

.PARAMETER SkipBuild
    Use an already-built binary instead of running cargo.

.PARAMETER OutDir
    Where to assemble the kit. Default dist\testd-kit under the repo root.

.EXAMPLE
    .\scripts\make-testd-kit.ps1

.EXAMPLE
    .\scripts\make-testd-kit.ps1 -ControllerIp 192.168.77.1 -Port 8765
#>
[CmdletBinding()]
param(
    [string]$ControllerIp,
    [int]$Port = 8765,
    [switch]$SkipBuild,
    [string]$OutDir
)

$ErrorActionPreference = 'Stop'

$RepoRoot = Split-Path -Parent $PSScriptRoot
if (-not $OutDir) { $OutDir = Join-Path $RepoRoot "dist\testd-kit" }
$ZipPath = "$OutDir.zip"

function Write-Step { param([string]$Text) Write-Host "`n=== $Text ===" -ForegroundColor Cyan }
function Write-Ok   { param([string]$Text) Write-Host "  ok    $Text" -ForegroundColor Green }
function Write-Info { param([string]$Text) Write-Host "  --    $Text" -ForegroundColor DarkGray }

# --- 0: controller address ----------------------------------------------------------------------

Write-Step "Controller address"

if (-not $ControllerIp) {
    $defaultRoute = Get-NetRoute -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue |
        Sort-Object RouteMetric | Select-Object -First 1
    if ($defaultRoute) {
        $candidate = Get-NetIPAddress -AddressFamily IPv4 -InterfaceIndex $defaultRoute.InterfaceIndex -ErrorAction SilentlyContinue |
            Where-Object { $_.IPAddress -ne '127.0.0.1' } | Select-Object -First 1
        if ($candidate) { $ControllerIp = $candidate.IPAddress }
    }
    if (-not $ControllerIp) {
        throw "Could not auto-detect this machine's LAN address; pass -ControllerIp explicitly."
    }
    Write-Info "auto-detected $ControllerIp (from the default-route interface)"

    # Auto-detection cannot know which path you intend to use, and the tailnet address never
    # shows up in the OS address table, so it would never be picked by accident. Surface it.
    $tsExe = @("$env:ProgramFiles\Tailscale\tailscale.exe", "${env:ProgramFiles(x86)}\Tailscale\tailscale.exe") |
        Where-Object { Test-Path $_ } | Select-Object -First 1
    if ($tsExe) {
        $tsIp = (& $tsExe ip -4 2>$null | Select-Object -First 1)
        if ($tsIp) {
            Write-Info "Tailscale is installed here; this machine's tailnet address is $($tsIp.Trim())"
            Write-Info "to pin the kit to the tailnet instead: -ControllerIp $($tsIp.Trim())"
        }
    }
    Write-Info "if you are using the direct-cable method, pass -ControllerIp with the cable address instead"
}
$parsed = [System.Net.IPAddress]::Any
if (-not [System.Net.IPAddress]::TryParse($ControllerIp, [ref]$parsed)) {
    throw "ControllerIp '$ControllerIp' is not a valid IP address."
}
Write-Ok "kit will be pinned to controller $ControllerIp`:$Port"

# --- 1: build ------------------------------------------------------------------------------------

Write-Step "Building evorift-testd"

$ExeSource = Join-Path $RepoRoot "src-tauri\target\release\evorift-testd.exe"
if ($SkipBuild) {
    if (-not (Test-Path $ExeSource)) { throw "-SkipBuild was given but $ExeSource does not exist." }
    Write-Info "skipping the build, using the existing binary"
} else {
    Push-Location $RepoRoot
    try {
        cargo build --release --bin evorift-testd --manifest-path src-tauri/Cargo.toml
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed with exit code $LASTEXITCODE" }
    } finally {
        Pop-Location
    }
    if (-not (Test-Path $ExeSource)) { throw "Build reported success but $ExeSource is missing." }
}
Write-Ok "binary: $ExeSource"

# --- 2: assemble ----------------------------------------------------------------------------------

Write-Step "Assembling the kit"

if (Test-Path $OutDir) { Remove-Item -Path $OutDir -Recurse -Force }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$files = @(
    @{ Source = $ExeSource;                                     Name = "evorift-testd.exe" },
    # The operator-facing entry point: double-click, click Yes on UAC, done.
    @{ Source = (Join-Path $PSScriptRoot "INSTALL.bat");         Name = "INSTALL.bat" },
    # Read-only diagnostic for when the controller cannot reach the agent.
    @{ Source = (Join-Path $PSScriptRoot "CHECK.bat");           Name = "CHECK.bat" },
    @{ Source = (Join-Path $PSScriptRoot "check.ps1");           Name = "check.ps1" },
    # Repairs DNS damage done by an older build's runaway deadman.
    @{ Source = (Join-Path $PSScriptRoot "FIX-DNS.bat");         Name = "FIX-DNS.bat" },
    @{ Source = (Join-Path $PSScriptRoot "fix-dns.ps1");         Name = "fix-dns.ps1" },
    @{ Source = (Join-Path $PSScriptRoot "testd-install.ps1");   Name = "testd-install.ps1" },
    @{ Source = (Join-Path $PSScriptRoot "link-setup.ps1");      Name = "link-setup.ps1" },
    @{ Source = (Join-Path $PSScriptRoot "capture-state.ps1");   Name = "capture-state.ps1" }
)
foreach ($f in $files) {
    if (-not (Test-Path $f.Source)) { throw "Kit input missing: $($f.Source)" }
    Copy-Item -Path $f.Source -Destination (Join-Path $OutDir $f.Name) -Force
    Write-Ok $f.Name
}

# --- 3: install.ps1 -- the laptop's single entry point ----------------------------------------------

# Built here rather than kept in the repo so the controller IP and port are already filled in and
# the operator has nothing to remember or mistype at the laptop.
$installer = @"
<#
.SYNOPSIS
    Remote test agent -- one-shot laptop setup. Run this elevated, on the TEST LAPTOP.

.DESCRIPTION
    Generated by make-testd-kit.ps1 on $(Get-Date -Format "yyyy-MM-dd HH:mm") and pinned to
    controller $ControllerIp`:$Port. It checks that this laptop can actually reach the
    controller, then installs and starts the agent.

.PARAMETER ControllerIp
    Override the baked-in controller address.

.PARAMETER BindIp
    Override the interface to bind. Normally auto-detected as the one on the controller's subnet.

.PARAMETER SkipLinkCheck
    Install even if the controller is unreachable right now.
#>
[CmdletBinding()]
param(
    [string]`$ControllerIp = '$ControllerIp',
    [string]`$BindIp,
    [int]`$Port = $Port,
    [switch]`$SkipLinkCheck
)

`$ErrorActionPreference = 'Stop'
`$here = `$PSScriptRoot

Write-Host ""
Write-Host "evorift remote test agent -- laptop setup" -ForegroundColor Cyan
Write-Host "controller: `$ControllerIp   port: `$Port" -ForegroundColor Cyan
Write-Host ""

`$id = [System.Security.Principal.WindowsIdentity]::GetCurrent()
`$principal = New-Object System.Security.Principal.WindowsPrincipal(`$id)
if (-not `$principal.IsInRole([System.Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw "Run this in an ELEVATED PowerShell (right-click -> Run as administrator)."
}

# Is the controller inside Tailscale's CGNAT range (100.64.0.0/10)?
`$octets = `$ControllerIp.Split('.')
`$controllerIsTailscale = (`$octets.Count -eq 4 -and [int]`$octets[0] -eq 100 -and
                          [int]`$octets[1] -ge 64 -and [int]`$octets[1] -le 127)

# Step 1: can this laptop see the controller at all? Getting this wrong is the most common way
# the whole setup silently does not work, so check before installing rather than after.
if (-not `$SkipLinkCheck) {
    Write-Host "=== Checking the link to the controller ===" -ForegroundColor Cyan

    if (`$controllerIsTailscale) {
        # Subnet matching is meaningless on a tailnet: every node is a /32, so no two nodes ever
        # share a subnet. And the tailnet address is usually missing from Get-NetIPAddress
        # entirely. Ask Tailscale instead, then prove the address is bindable.
        `$tsExe = @("`$env:ProgramFiles\Tailscale\tailscale.exe",
                   "`${env:ProgramFiles(x86)}\Tailscale\tailscale.exe") |
                  Where-Object { Test-Path `$_ } | Select-Object -First 1
        if (-not `$tsExe) { `$tsExe = (Get-Command tailscale.exe -ErrorAction SilentlyContinue).Source }
        if (-not `$tsExe) {
            throw "The controller (`$ControllerIp) is a Tailscale address, but Tailscale is not installed on this laptop. Install it and log in with the same account, then re-run."
        }

        `$selfIp = (& `$tsExe ip -4 2>`$null | Select-Object -First 1)
        if (-not `$selfIp) {
            throw "Tailscale is installed but has no address here. Run 'tailscale up' and log in, then re-run."
        }
        `$selfIp = `$selfIp.Trim()
        Write-Host "  ok    this laptop's tailnet address: `$selfIp" -ForegroundColor Green

        # The decisive check: an address Tailscale reports is not necessarily an address a
        # listener can bind. In userspace-networking mode it is not on any OS interface.
        try {
            `$probe = New-Object System.Net.Sockets.TcpListener([System.Net.IPAddress]::Parse(`$selfIp), 0)
            `$probe.Start(); `$probe.Stop()
            Write-Host "  ok    `$selfIp is bindable" -ForegroundColor Green
        } catch {
            Write-Host ""
            Write-Warning "`$selfIp is NOT bindable, so the agent cannot listen on it."
            Write-Warning "Tailscale reports the address and outbound traffic works, but it is not on"
            Write-Warning "an OS interface -- tailscaled is in userspace-networking mode."
            Write-Host ""
            Write-Host "Check:  Get-NetIPAddress -AddressFamily IPv4      # must list `$selfIp" -ForegroundColor Yellow
            Write-Host "Fix:    Restart-Service Tailscale ; tailscale up" -ForegroundColor Yellow
            Write-Host "Or use the direct cable instead:" -ForegroundColor Yellow
            Write-Host "        .\link-setup.ps1 -Action direct-cable -Role agent" -ForegroundColor Yellow
            Write-Host ""
            throw "Aborting: the agent could not bind `$selfIp. Pass -SkipLinkCheck to install anyway."
        }

        if (-not `$BindIp) { `$BindIp = `$selfIp }

        `$reach = Test-NetConnection -ComputerName `$ControllerIp -InformationLevel Quiet -WarningAction SilentlyContinue
        if (`$reach) { Write-Host "  ok    controller `$ControllerIp is reachable over the tailnet" -ForegroundColor Green }
        else { Write-Warning "controller `$ControllerIp did not answer -- check 'tailscale status' on both machines" }

    } else {
        `$sameSubnet = Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue |
            Where-Object { `$_.IPAddress -ne '127.0.0.1' } |
            Where-Object {
                `$ba = ([System.Net.IPAddress]::Parse(`$_.IPAddress)).GetAddressBytes()
                `$bb = ([System.Net.IPAddress]::Parse(`$ControllerIp)).GetAddressBytes()
                [array]::Reverse(`$ba); [array]::Reverse(`$bb)
                `$ia = [BitConverter]::ToUInt32(`$ba, 0); `$ib = [BitConverter]::ToUInt32(`$bb, 0)
                `$mask = [uint32]([uint32]::MaxValue -shl (32 - `$_.PrefixLength))
                (`$ia -band `$mask) -eq (`$ib -band `$mask)
            } | Select-Object -First 1

        if (-not `$sameSubnet) {
            Write-Warning "No interface on this laptop shares a subnet with the controller (`$ControllerIp)."
            Write-Warning "This laptop's addresses:"
            Get-NetIPAddress -AddressFamily IPv4 | Where-Object { `$_.IPAddress -ne '127.0.0.1' } |
                ForEach-Object { Write-Warning "    `$(`$_.IPAddress)/`$(`$_.PrefixLength) on `$(`$_.InterfaceAlias)" }
            Write-Host ""
            Write-Host "Fix the addressing first:" -ForegroundColor Yellow
            Write-Host "    .\link-setup.ps1 -Action diagnose -PeerIp `$ControllerIp" -ForegroundColor Yellow
            Write-Host "or, for the direct-cable method (recommended):" -ForegroundColor Yellow
            Write-Host "    .\link-setup.ps1 -Action direct-cable -Role agent" -ForegroundColor Yellow
            Write-Host ""
            throw "Aborting: the controller would not be able to reach this agent. Pass -SkipLinkCheck to install anyway."
        }
        Write-Host "  ok    `$(`$sameSubnet.IPAddress) on '`$(`$sameSubnet.InterfaceAlias)' is on the controller's subnet" -ForegroundColor Green
    }
}

# Step 2: install.
`$installArgs = @{
    ControllerIp = `$ControllerIp
    Port         = `$Port
    BinarySource = (Join-Path `$here "evorift-testd.exe")
}
if (`$BindIp) { `$installArgs.BindIp = `$BindIp }
& (Join-Path `$here "testd-install.ps1") @installArgs

# Step 3: put capture-state.ps1 into the sandbox so the very first remote run has it.
`$sandboxScripts = "C:\evorift-test\scripts"
New-Item -ItemType Directory -Force -Path `$sandboxScripts | Out-Null
Copy-Item -Path (Join-Path `$here "capture-state.ps1") -Destination `$sandboxScripts -Force
Write-Host "  ok    capture-state.ps1 placed in the sandbox" -ForegroundColor Green
Write-Host ""
"@

$installer | Out-File -FilePath (Join-Path $OutDir "install.ps1") -Encoding ascii
Write-Ok "install.ps1 (pinned to $ControllerIp`:$Port)"

# --- 4: README ---------------------------------------------------------------------------------------

$readme = @"
================================================================================
 evorift REMOTE TEST KIT
 built $(Get-Date -Format "yyyy-MM-dd HH:mm") for controller $ControllerIp port $Port
================================================================================

WHAT THIS IS
  Everything the test laptop needs to accept builds from your controller and run
  verification on them, with no repo, no Rust toolchain and no internet required on
  the laptop side.

  The software under test cuts all internet on its host until it is killed. So the
  agent is built to lose contact and save itself: if no heartbeat arrives from the
  controller for 120 seconds, it kills evorift.exe and winws.exe, clears stale
  WinDivert services and puts DNS back on DHCP. All command output is written to
  disk on the laptop first, and fetched later.

--------------------------------------------------------------------------------
 STEP 1 -- GET THE TWO MACHINES ON ONE SUBNET
--------------------------------------------------------------------------------

  If your desktop is on a cable and the laptop is on Wi-Fi and they cannot see
  each other, the router is not bridging them. Check what you have:

      .\link-setup.ps1 -Action diagnose -PeerIp $ControllerIp

  RECOMMENDED FIX -- DIRECT CABLE. Plug the network cable straight between the two
  machines (no router, no switch), then run, elevated:

      on the DESKTOP:   .\link-setup.ps1 -Action direct-cable -Role controller
      on this LAPTOP:   .\link-setup.ps1 -Action direct-cable -Role agent

  That gives the desktop 192.168.77.1 and the laptop 192.168.77.2, with no default
  gateway on the link -- so the laptop keeps using Wi-Fi for internet and the cable
  carries only the control channel.

  Why this one is worth the cable: evorift kills the internet path. If your control
  link is on a different physical interface, it survives, and you can watch the
  failure live instead of reconstructing it from captures.

  USING TAILSCALE INSTEAD? Check it end to end first:

      .\link-setup.ps1 -Action tailscale

  It confirms the backend is running, lists peers, checks Tailscale's own ShieldsUp
  setting, and -- the one that decides whether this works at all -- tests whether a
  listener can actually BIND this machine's 100.x address.

  That last check matters. On Windows the tailnet address is often missing from
  ipconfig / Get-NetIPAddress even while the tunnel is up and routing. If tailscaled
  is in userspace-networking mode the address is never put on an OS interface, so
  outbound works but NOTHING CAN LISTEN on it -- the agent will not start. If the
  check says NOT bindable: Restart-Service Tailscale, then 'tailscale up', and
  confirm Get-NetIPAddress now lists the address.

  Trade-off: Tailscale rides the internet, and evorift cuts the internet. The control
  link dies with it, so every run depends on the deadman switch firing. It works --
  that is what the deadman is for -- but you recover blind instead of watching the
  failure. The direct cable does not have this problem.

  NO SPARE ETHERNET PORT AND NO TAILSCALE?
      .\link-setup.ps1 -Action hotspot        (Windows Mobile Hotspot, 192.168.137.x)
  or just put both machines on the same phone hotspot. Both work; both put the
  control link on the same interface as the internet, so every run will depend on
  the deadman switch.

  If you used direct-cable, the controller address changes to 192.168.77.1. Pass it
  in step 2: .\install.ps1 -ControllerIp 192.168.77.1

--------------------------------------------------------------------------------
 STEP 2 -- INSTALL THE AGENT (on this laptop)
--------------------------------------------------------------------------------

      DOUBLE-CLICK  INSTALL.bat

  A Windows popup asks for administrator rights. Click YES. That is the only
  interaction. Nothing has to be typed.

  It verifies this laptop can reach the controller, installs the binary to
  C:\Program Files\evorift-testd\, generates a token, registers the
  "evorift-testd" service (auto-start, auto-restart), opens ONE firewall rule
  scoped to $ControllerIp and port $Port only, starts the service and confirms it
  is listening.

  If it cannot reach the controller it stops and says so. Nothing is half
  installed.

  When it finishes it saves the token to  testd.token  NEXT TO INSTALL.bat --
  so if you ran this from a USB stick, the token is already on that stick.

--------------------------------------------------------------------------------
 STEP 3 -- BACK ON THE DESKTOP
--------------------------------------------------------------------------------

      Plug the USB stick in, then DOUBLE-CLICK  REMOTE-TEST.bat
      (it is in the repo root, next to the scripts folder)

  It finds testd.token on the stick by itself and shows a menu:

      [1] Connection test      -> expect  "ok": true
      [2] RUN THE FULL TEST    -> push build, capture, pull results back
      [3] EMERGENCY rescue     -> tell the laptop to kill evorift now
      [4] Diagnose the network
      [5] Exit

  Start with [1]. If it says "ok": true you are connected; go to [2].

      403 -> the laptop does not have the desktop's address in its allowlist;
             re-run INSTALL.bat with the right controller address
      401 -> wrong token
      no answer -> the laptop is off, or the agent is not running

--------------------------------------------------------------------------------
 STEP 4 -- READING THE RESULT
--------------------------------------------------------------------------------

  Option [2] pushes the build, captures the laptop's state before / during /
  after, pulls everything back, and checks whether the laptop had to rescue
  itself. It takes several minutes and the laptop WILL go unreachable in the
  middle -- that is expected, the script keeps retrying.

  READ THE LAST LINE:
      "the deadman did not fire"  -> results are trustworthy
      "THE DEADMAN FIRED"         -> the laptop rescued itself partway through,
                                     so the captures do NOT describe an
                                     uninterrupted run

--------------------------------------------------------------------------------
 IF THE LAPTOP GOES DARK AND DOES NOT COME BACK
--------------------------------------------------------------------------------

  Physically at the laptop, elevated PowerShell:

      taskkill /f /im evorift.exe; taskkill /f /im winws.exe

  Usually enough. If not, the full manual recovery, the Safe Mode path and how to
  work out why the deadman did not fire are in docs\REMOTE-TESTING.md section 6 in
  the repo. The two logs that explain it never leave the laptop:

      C:\ProgramData\evorift-testd\deadman.log
      C:\ProgramData\evorift-testd\audit.log

--------------------------------------------------------------------------------
 WHAT IS IN THIS KIT
--------------------------------------------------------------------------------

  INSTALL.bat          <<< DOUBLE-CLICK THIS. Everything else is machinery.
  CHECK.bat            <<< double-click this if the desktop cannot reach the
                           agent. Read-only; writes check-report.txt next to it.
  check.ps1            what CHECK.bat runs
  install.ps1          what INSTALL.bat runs; controller address baked in
  testd-install.ps1    the real installer (install.ps1 calls it)
  link-setup.ps1       IP diagnosis, direct-cable setup, revert
  evorift-testd.exe    the agent
  capture-state.ps1    the state capture the agent runs remotely
  MANIFEST.txt         SHA-256 of every file above
  testd.token          appears AFTER install - carry it back to the desktop

  Verify the kit arrived intact:
      Get-FileHash .\evorift-testd.exe -Algorithm SHA256

--------------------------------------------------------------------------------
 UNDOING EVERYTHING
--------------------------------------------------------------------------------

  Stop-Service evorift-testd
  sc.exe delete evorift-testd
  Get-NetFirewallRule -DisplayName "evorift-testd (controller only)" | Remove-NetFirewallRule
  Remove-Item "C:\Program Files\evorift-testd" -Recurse -Force
  Remove-Item "C:\ProgramData\evorift-testd" -Recurse -Force
  Remove-Item "C:\evorift-test" -Recurse -Force
  .\link-setup.ps1 -Action revert          # only if you used direct-cable
"@

$readme | Out-File -FilePath (Join-Path $OutDir "README.txt") -Encoding ascii
Write-Ok "README.txt"

# --- 5: manifest -----------------------------------------------------------------------------------------

Write-Step "Checksums"

$manifestLines = @(
    "evorift remote test kit",
    "built:      $(Get-Date -Format 'yyyy-MM-ddTHH:mm:ssK')",
    "controller: $ControllerIp`:$Port",
    "host:       $env:COMPUTERNAME",
    "",
    "SHA-256:"
)
Get-ChildItem -Path $OutDir -File | Sort-Object Name | ForEach-Object {
    $hash = (Get-FileHash -Path $_.FullName -Algorithm SHA256).Hash.ToLower()
    $manifestLines += ("  {0,-24} {1}  ({2} bytes)" -f $_.Name, $hash, $_.Length)
    Write-Info ("{0,-24} {1}" -f $_.Name, $hash.Substring(0, 16) + "...")
}
$manifestLines | Out-File -FilePath (Join-Path $OutDir "MANIFEST.txt") -Encoding ascii
Write-Ok "MANIFEST.txt"

# --- 6: zip -------------------------------------------------------------------------------------------------

Write-Step "Packaging"

if (Test-Path $ZipPath) { Remove-Item -Path $ZipPath -Force }
Compress-Archive -Path (Join-Path $OutDir '*') -DestinationPath $ZipPath -CompressionLevel Optimal
$zipInfo = Get-Item $ZipPath
Write-Ok "$ZipPath ($([math]::Round($zipInfo.Length / 1MB, 2)) MB)"

Write-Host ""
Write-Host "================================================================" -ForegroundColor Yellow
Write-Host " KIT READY" -ForegroundColor Yellow
Write-Host "================================================================" -ForegroundColor Yellow
Write-Host ""
Write-Host "  Folder : $OutDir"
Write-Host "  Zip    : $ZipPath"
Write-Host ""
Write-Host "  1. Copy the FOLDER above onto a USB stick (not the zip -- the folder,"
Write-Host "     so the token can be written back onto the stick)."
Write-Host ""
Write-Host "  2. On the test laptop:   DOUBLE-CLICK  INSTALL.bat" -ForegroundColor White
Write-Host "     Click YES on the administrator popup. Nothing to type."
Write-Host ""
Write-Host "  3. Bring the stick back here, then:  DOUBLE-CLICK  REMOTE-TEST.bat" -ForegroundColor White
Write-Host "     It finds the token on the stick and gives you a menu."
Write-Host ""
Write-Host "  If the two machines cannot see each other, start with:" -ForegroundColor DarkGray
Write-Host "      .\scripts\link-setup.ps1 -Action diagnose -PeerIp $ControllerIp" -ForegroundColor DarkGray
Write-Host ""
