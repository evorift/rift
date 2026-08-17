<#
.SYNOPSIS
    Controller-side helper for the evorift remote test agent. Run this on YOUR machine.

.DESCRIPTION
    Wraps the agent's six endpoints, and adds one composite action -- `cycle` -- that performs a
    full remote verification run in a single command:

        push the build  ->  capture "before"  ->  start evorift  ->  capture "during"
        ->  stop evorift  ->  capture "after"  ->  pull everything back  ->  check the deadman

    The whole thing is designed around the fact that the software under test cuts all internet
    on the laptop. So:

      * every request is retried through a short outage rather than failing the run;
      * the run never assumes a command's output came back over HTTP -- output is written to
        disk on the laptop and pulled afterwards;
      * the deadman fire count is read BEFORE and AFTER the run and compared. If it went up, the
        laptop recovered itself mid-test, which invalidates any "it worked" conclusion. That
        check is the point of the last step, not a formality.

.PARAMETER AgentIp
    The test laptop's LAN IP (the BindIp the installer reported).

.PARAMETER Port
    Agent port. Default 8765.

.PARAMETER Token
    Bearer token from the installer. Defaults to $env:EVORIFT_TESTD_TOKEN.

.PARAMETER Action
    health | push | run | job | pull | recover | cycle

.PARAMETER Path
    For push: the local file to upload. For pull: the sandbox-relative path to download.

.PARAMETER RemotePath
    For push: the sandbox-relative destination. Defaults to the source file's name.

.PARAMETER Script
    For run: the sandbox-relative .ps1 to execute, e.g. scripts/capture-state.ps1

.PARAMETER ScriptArgs
    For run: arguments passed to the script as separate argv entries.

.PARAMETER JobId
    For job: the id returned by run.

.PARAMETER TimeoutSecs
    For run: per-job timeout on the laptop. Default 300.

.PARAMETER BuildPath
    For cycle: the local evorift.exe (or installer) to push. Optional -- omit to run the capture
    and verification steps against whatever build is already on the laptop.

.PARAMETER OutDir
    For cycle: where to write the pulled artefacts. Default docs\captures\remote-<timestamp>.

.EXAMPLE
    $env:EVORIFT_TESTD_TOKEN = '<token from the installer>'
    .\scripts\remote.ps1 -AgentIp 192.168.1.50 -Action health

.EXAMPLE
    .\scripts\remote.ps1 -AgentIp 192.168.1.50 -Action cycle -BuildPath .\src-tauri\target\release\evorift.exe

.EXAMPLE
    .\scripts\remote.ps1 -AgentIp 192.168.1.50 -Action pull -Path docs/captures/20260811-2210-during/01-network-config.txt
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$AgentIp,
    [int]$Port = 8765,
    [string]$Token = $env:EVORIFT_TESTD_TOKEN,
    [Parameter(Mandatory = $true)]
    [ValidateSet("health", "push", "run", "job", "pull", "recover", "cycle", "engine")]
    [string]$Action,
    [string]$Path,
    [string]$RemotePath,
    [string]$Script,
    [string[]]$ScriptArgs = @(),
    [string]$JobId,
    [int]$TimeoutSecs = 300,
    [string]$BuildPath,
    [string]$OutDir
)

$ErrorActionPreference = 'Stop'
$Base = "http://${AgentIp}:${Port}"

if (-not $Token -and $Action -ne 'health') {
    throw "No token. Set `$env:EVORIFT_TESTD_TOKEN to the value the installer printed, or pass -Token."
}

function Write-Step { param([string]$Text) Write-Host "`n=== $Text ===" -ForegroundColor Cyan }
function Write-Ok   { param([string]$Text) Write-Host "  ok    $Text" -ForegroundColor Green }
function Write-Info { param([string]$Text) Write-Host "  --    $Text" -ForegroundColor DarkGray }
function Write-Bad  { param([string]$Text) Write-Host "  FAIL  $Text" -ForegroundColor Red }

function Get-AuthHeader {
    if ($Token) { return @{ Authorization = "Bearer $Token" } }
    return @{}
}

<#
    One request, with a bounded retry. The agent is unreachable for a stretch of every
    interesting test, so a single failed call means "not yet", not "broken".
#>
function Invoke-Agent {
    param(
        [string]$Method = 'GET',
        [Parameter(Mandatory = $true)][string]$Endpoint,
        $Body,
        [string]$InFile,
        [string]$OutFile,
        [int]$RetrySeconds = 0,
        [int]$TimeoutSec = 60
    )
    $deadline = (Get-Date).AddSeconds($RetrySeconds)
    $attempt = 0
    while ($true) {
        $attempt++
        try {
            $params = @{
                Uri             = "$Base$Endpoint"
                Method          = $Method
                Headers         = Get-AuthHeader
                TimeoutSec      = $TimeoutSec
                UseBasicParsing = $true
            }
            if ($Body)    { $params.Body = $Body; $params.ContentType = 'application/json' }
            if ($InFile)  { $params.InFile = $InFile; $params.ContentType = 'application/octet-stream' }
            if ($OutFile) { $params.OutFile = $OutFile }

            $response = Invoke-WebRequest @params
            if ($OutFile) { return $null }
            return ($response.Content | ConvertFrom-Json)
        }
        catch {
            # A 4xx is a real answer from a reachable agent -- retrying will not change it.
            $status = $null
            if ($_.Exception.Response) { $status = [int]$_.Exception.Response.StatusCode }
            if ($status -and $status -lt 500) {
                $detail = $_.ErrorDetails.Message
                if (-not $detail) { $detail = $_.Exception.Message }
                throw "Agent returned HTTP $status for $Method $Endpoint : $detail"
            }
            if ((Get-Date) -ge $deadline) {
                throw "Cannot reach the agent at $Base$Endpoint after $attempt attempt(s): $($_.Exception.Message)"
            }
            Write-Info "attempt $attempt failed ($($_.Exception.Message)); retrying, $([int]($deadline - (Get-Date)).TotalSeconds)s left"
            Start-Sleep -Seconds 3
        }
    }
}

function Get-Health {
    param([int]$RetrySeconds = 0)
    Invoke-Agent -Endpoint '/health' -RetrySeconds $RetrySeconds -TimeoutSec 10
}

function Send-AgentFile {
    param(
        [Parameter(Mandatory = $true)][string]$LocalPath,
        [Parameter(Mandatory = $true)][string]$Destination,
        [int]$RetrySeconds = 0
    )
    if (-not (Test-Path $LocalPath)) { throw "Local file not found: $LocalPath" }
    $encoded = [Uri]::EscapeDataString($Destination)
    $result = Invoke-Agent -Method POST -Endpoint "/push?path=$encoded" -InFile $LocalPath `
                           -RetrySeconds $RetrySeconds -TimeoutSec 600
    # Verify the laptop holds the same bytes we sent. Worth doing even on a LAN: the transfer
    # may well be happening while the link is degrading.
    $localHash = (Get-FileHash -Path $LocalPath -Algorithm SHA256).Hash.ToLower()
    if ($result.sha256 -ne $localHash) {
        throw "Upload checksum mismatch for $Destination : local $localHash, laptop $($result.sha256)"
    }
    Write-Ok "pushed $Destination ($($result.bytes) bytes, sha256 verified)"
    return $result
}

<#
    Push a whole local directory into the sandbox, preserving its layout.

    The engine bundle is ten files across three folders and the agent takes one file per request
    by design (no archive handling in the trusted path). Pushing them individually keeps that
    boundary intact and still checksums every file.
#>
function Send-AgentTree {
    param(
        [Parameter(Mandatory = $true)][string]$LocalDir,
        [Parameter(Mandatory = $true)][string]$Destination,
        [int]$RetrySeconds = 60
    )
    if (-not (Test-Path $LocalDir)) { throw "Local directory not found: $LocalDir" }
    # NOT $base. PowerShell variable names are case-insensitive AND functions can read their
    # caller's locals, so a local `$base` here silently shadows the script-level `$Base` (the
    # agent URL) inside every function this one calls -- Invoke-Agent then tries to POST to a
    # local directory path. Cost one failed run to find.
    $localBase = (Resolve-Path $LocalDir).Path.TrimEnd('\')
    $files = Get-ChildItem -LiteralPath $localBase -Recurse -File
    $sent = 0
    foreach ($f in $files) {
        $rel = $f.FullName.Substring($localBase.Length).TrimStart('\') -replace '\\', '/'
        Send-AgentFile -LocalPath $f.FullName -Destination "$Destination/$rel" -RetrySeconds $RetrySeconds | Out-Null
        $sent++
    }
    Write-Ok "pushed $sent file(s) into $Destination/"
    return $sent
}

function Start-AgentJob {
    param(
        [Parameter(Mandatory = $true)][string]$ScriptPath,
        [string[]]$ScriptArgs = @(),
        [int]$JobTimeout = 300,
        [int]$RetrySeconds = 0
    )
    $spec = @{
        kind         = "powershell"
        script       = $ScriptPath
        args         = @($ScriptArgs)
        timeout_secs = $JobTimeout
    } | ConvertTo-Json -Compress
    $result = Invoke-Agent -Method POST -Endpoint '/run' -Body $spec -RetrySeconds $RetrySeconds
    Write-Ok "started job $($result.job_id) -> $ScriptPath $($ScriptArgs -join ' ')"
    return $result.job_id
}

<#
    Poll a job to completion. `WaitSeconds` has to cover the whole outage: while evorift has the
    network down, every poll fails, and that is expected rather than an error.
#>
function Wait-AgentJob {
    param(
        [Parameter(Mandatory = $true)][string]$Id,
        [int]$WaitSeconds = 600,
        [switch]$Quiet
    )
    $deadline = (Get-Date).AddSeconds($WaitSeconds)
    while ($true) {
        # Heartbeat FIRST, every poll.
        #
        # Only /health resets the deadman -- polling /job/<id> does not. A capture job can easily
        # run longer than the 120s deadman window, so without this the agent would conclude the
        # controller had vanished and recover the laptop mid-test, while the controller was in
        # fact talking to it the whole time. That would fire the deadman on every single run and
        # make the "results are trustworthy" verdict meaningless.
        #
        # Deliberately swallowed: during the outage this SHOULD fail, and that is precisely when
        # the deadman is supposed to fire. A failed heartbeat is data, not an error.
        try { Get-Health | Out-Null } catch { }

        try {
            $status = Invoke-Agent -Endpoint "/job/${Id}" -TimeoutSec 15
            if ($status.state -ne 'running') {
                if (-not $Quiet) {
                    Write-Ok "job $Id -> $($status.state) (exit $($status.exit_code))"
                }
                return $status
            }
        }
        catch {
            Write-Info "job $Id not reachable yet (this is expected while the link is down)"
        }
        if ((Get-Date) -ge $deadline) {
            throw "Job $Id did not finish within ${WaitSeconds}s."
        }
        Start-Sleep -Seconds 5
    }
}

function Receive-AgentFile {
    param(
        [Parameter(Mandatory = $true)][string]$RemoteRelPath,
        [Parameter(Mandatory = $true)][string]$Destination,
        [int]$RetrySeconds = 0
    )
    $dir = Split-Path -Parent $Destination
    if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
    $encoded = [Uri]::EscapeDataString($RemoteRelPath)
    Invoke-Agent -Endpoint "/pull?path=$encoded" -OutFile $Destination -RetrySeconds $RetrySeconds -TimeoutSec 600 | Out-Null
    Write-Ok "pulled $RemoteRelPath -> $Destination"
}

<#
    Pull a whole capture directory. The agent serves one file per request by design, so the list
    of files comes from a manifest job run on the laptop -- there is no directory-listing
    endpoint, and adding one would widen the surface for no real gain.
#>
function Receive-AgentTree {
    param(
        [Parameter(Mandatory = $true)][string]$RemoteDir,
        [Parameter(Mandatory = $true)][string]$LocalDir,
        [int]$RetrySeconds = 120
    )
    $listScript = "scripts/list-files.ps1"
    $jobId = Start-AgentJob -ScriptPath $listScript -ScriptArgs @($RemoteDir) -JobTimeout 60 -RetrySeconds $RetrySeconds
    $status = Wait-AgentJob -Id $jobId -WaitSeconds 300 -Quiet
    if ($status.state -ne 'exited' -or $status.exit_code -ne 0) {
        Write-Bad "listing $RemoteDir failed (state $($status.state), exit $($status.exit_code))"
        return @()
    }
    $listing = Invoke-Agent -Endpoint "/job/${jobId}?tail=262144"
    $files = $listing.stdout_tail -split "`r?`n" | Where-Object { $_.Trim() -ne '' }
    $pulled = @()
    $since = 0
    foreach ($f in $files) {
        # Same reason as in Wait-AgentJob: a capture bundle can be many files, and /pull does not
        # reset the deadman. Heartbeat periodically so a long download is not mistaken for a dead
        # controller. Every 10 files is far inside the 120s window at LAN speeds.
        $since++
        if ($since -ge 10) { $since = 0; try { Get-Health | Out-Null } catch { } }

        $rel = $f.Trim()
        $dest = Join-Path $LocalDir ($rel -replace '/', '\')
        try {
            Receive-AgentFile -RemoteRelPath $rel -Destination $dest -RetrySeconds 60
            $pulled += $rel
        }
        catch {
            Write-Bad "could not pull $rel : $($_.Exception.Message)"
        }
    }
    return $pulled
}

# ================================================================================================
# Actions
# ================================================================================================

switch ($Action) {

    'health' {
        $h = Get-Health -RetrySeconds 10
        $h | ConvertTo-Json -Depth 5
        if ($h.deadman.fires -gt 0) {
            Write-Warning "The deadman has fired $($h.deadman.fires) time(s) since the agent started. Check $env:ProgramData\evorift-testd\deadman.log ON THE LAPTOP."
        }
    }

    'push' {
        if (-not $Path) { throw "-Path (the local file) is required for push." }
        if (-not $RemotePath) { $RemotePath = Split-Path -Leaf $Path }
        Send-AgentFile -LocalPath $Path -Destination $RemotePath -RetrySeconds 30 | ConvertTo-Json
    }

    'run' {
        if (-not $Script) { throw "-Script (a sandbox-relative .ps1) is required for run." }
        $id = Start-AgentJob -ScriptPath $Script -ScriptArgs $ScriptArgs -JobTimeout $TimeoutSecs -RetrySeconds 30
        Write-Host $id
    }

    'job' {
        if (-not $JobId) { throw "-JobId is required for job." }
        Invoke-Agent -Endpoint "/job/${JobId}?tail=65536" | ConvertTo-Json -Depth 5
    }

    'pull' {
        if (-not $Path) { throw "-Path (the sandbox-relative file) is required for pull." }
        $dest = if ($RemotePath) { $RemotePath } else { Join-Path (Get-Location) (Split-Path -Leaf $Path) }
        Receive-AgentFile -RemoteRelPath $Path -Destination $dest -RetrySeconds 60
    }

    'recover' {
        Write-Step "Firing the recovery routine on the laptop"
        $r = Invoke-Agent -Method POST -Endpoint '/recover' -RetrySeconds 120 -TimeoutSec 120
        $r | ConvertTo-Json -Depth 6
        if (-not $r.ok) { Write-Bad "one or more recovery steps failed -- read the steps above" }
        else { Write-Ok "all recovery steps reported success" }
    }

    'engine' {
        # THE REAL TEST: push evorift's engine and actually turn protection on.
        $stamp = Get-Date -Format "yyyyMMdd-HHmmss"
        $repoRoot = Split-Path -Parent $PSScriptRoot
        if (-not $OutDir) { $OutDir = Join-Path $repoRoot "docs\captures\engine-$stamp" }
        New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

        Write-Step "0. Baseline"
        $before = Get-Health -RetrySeconds 30
        $firesBefore = [int]$before.deadman.fires
        Write-Ok "agent up, deadman fires so far: $firesBefore"

        Write-Step "1. Pushing the engine bundle"
        $rel = Join-Path $repoRoot "src-tauri\target\release"
        foreach ($need in @("evorift-svc.exe", "evorift-ctl.exe")) {
            $p = Join-Path $rel $need
            if (-not (Test-Path $p)) {
                throw ("$need not found at $p. Build it first:`n" +
                       "    cargo build --release --manifest-path src-tauri/Cargo.toml")
            }
            Send-AgentFile -LocalPath $p -Destination "engine/$need" -RetrySeconds 60 | Out-Null
        }
        # winws + WinDivert must sit in engine\winws\, because WinwsEngine::bundle_dir() looks
        # for <exe dir>\winws -- see engine.rs.
        $winwsSrc = Join-Path $repoRoot "src-tauri\resources\winws"
        Send-AgentTree -LocalDir $winwsSrc -Destination "engine/winws" -RetrySeconds 60 | Out-Null

        Write-Step "2. Pushing the test scripts"
        Send-AgentFile -LocalPath (Join-Path $repoRoot "scripts\capture-state.ps1") `
                       -Destination "scripts/capture-state.ps1" -RetrySeconds 30 | Out-Null
        Send-AgentFile -LocalPath (Join-Path $repoRoot "scripts\run-engine-test.ps1") `
                       -Destination "scripts/run-engine-test.ps1" -RetrySeconds 30 | Out-Null

        $listPs1 = Join-Path $env:TEMP "testd-list-files.ps1"
        @'
param([Parameter(Mandatory=$true)][string]$Dir)
$root = (Get-Location).Path
if ($root.StartsWith('\\?\')) { $root = $root.Substring(4) }
$root = $root.TrimEnd('\')
$rel = ($Dir -replace '/', '\').Trim('\')
$target = "$root\$rel"
if (-not (Test-Path -LiteralPath $target)) { Write-Error "no such directory: $target"; exit 1 }
Get-ChildItem -LiteralPath $target -Recurse -File | ForEach-Object {
    $_.FullName.Substring($root.Length).TrimStart('\') -replace '\\', '/'
}
exit 0
'@ | Out-File -FilePath $listPs1 -Encoding utf8
        Send-AgentFile -LocalPath $listPs1 -Destination "scripts/list-files.ps1" -RetrySeconds 30 | Out-Null

        Write-Step "3. Running the engine test on the laptop"
        Write-Info "the laptop starts evorift, captures itself, and stops it again -- all locally"
        Write-Info "the link may drop while protection is on; that is the point, and it is expected"
        $jobId = Start-AgentJob -ScriptPath "scripts/run-engine-test.ps1" `
                                -ScriptArgs @("-HoldSeconds", "$TimeoutSecs") `
                                -JobTimeout 900 -RetrySeconds 60
        $job = Wait-AgentJob -Id $jobId -WaitSeconds 1200

        Write-Step "4. Engine test output"
        $full = Invoke-Agent -Endpoint "/job/$jobId`?tail=200000" -RetrySeconds 120
        if ($full.stdout_tail) { $full.stdout_tail -split "`r?`n" | ForEach-Object { Write-Host "  $_" } }
        if ($full.stderr_tail) {
            Write-Step "stderr"
            $full.stderr_tail -split "`r?`n" | Select-Object -First 40 | ForEach-Object { Write-Host "  $_" -ForegroundColor DarkYellow }
        }

        Write-Step "5. Pulling everything back"
        try { Receive-AgentFile -RemoteRelPath "engine-test-result.json" `
                                -Destination (Join-Path $OutDir "engine-test-result.json") -RetrySeconds 120 }
        catch { Write-Bad "no engine-test-result.json: $($_.Exception.Message)" }
        $pulled = Receive-AgentTree -RemoteDir "docs/captures" -LocalDir $OutDir -RetrySeconds 300
        Write-Ok "pulled $($pulled.Count) capture file(s)"
        $recovered = Receive-AgentTree -RemoteDir "recovery" -LocalDir $OutDir -RetrySeconds 120

        Write-Step "6. Verdict"
        $after = Get-Health -RetrySeconds 600
        $delta = [int]$after.deadman.fires - $firesBefore

        $resultFile = Join-Path $OutDir "engine-test-result.json"
        $result = $null
        if (Test-Path $resultFile) { $result = Get-Content $resultFile -Raw | ConvertFrom-Json }

        Write-Host ""
        if ($result) {
            if ($result.winws_seen) { Write-Ok "winws RAN - the 'during' capture reflects an active bypass" }
            else { Write-Bad "winws never started - the 'during' capture shows NO bypass; check the output above" }
            if ($result.ok) { Write-Ok "the network recovered after the engine stopped" }
            else { Write-Bad "the network did NOT recover cleanly after stopping - this is the reported bug" }
        } else {
            Write-Bad "no result file came back - read the job output above"
        }
        if ($delta -gt 0) {
            Write-Bad "the deadman fired $delta time(s) during this run - the laptop rescued itself mid-test"
            Write-Info "that means the link died and stayed dead; the captures may be truncated"
        } else {
            Write-Ok "the deadman did not fire"
        }
        Write-Host ""
        Write-Host "Artefacts: $OutDir" -ForegroundColor Cyan
    }

    'cycle' {
        $stamp = Get-Date -Format "yyyyMMdd-HHmmss"
        if (-not $OutDir) {
            $repoRoot = Split-Path -Parent $PSScriptRoot
            $OutDir = Join-Path $repoRoot "docs\captures\remote-$stamp"
        }
        New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

        Write-Step "0. Baseline health (records the deadman fire count BEFORE the run)"
        $before = Get-Health -RetrySeconds 30
        $firesBefore = [int]$before.deadman.fires
        Write-Ok "agent up, uptime $($before.uptime_secs)s, deadman fires so far: $firesBefore"

        Write-Step "1. Pushing the test scripts"
        $repoRoot = Split-Path -Parent $PSScriptRoot
        Send-AgentFile -LocalPath (Join-Path $repoRoot "scripts\capture-state.ps1") `
                  -Destination "scripts/capture-state.ps1" -RetrySeconds 30 | Out-Null

        # Helper scripts the cycle needs on the laptop. Written here rather than kept in the repo
        # so the sandbox always has the version this script expects.
        $listPs1 = Join-Path $env:TEMP "testd-list-files.ps1"
        @'
# Emit every file under a sandbox-relative directory, one sandbox-relative path per line.
param([Parameter(Mandatory=$true)][string]$Dir)

# The agent's working directory is the CANONICAL sandbox root, which on Windows is a verbatim
# path: \\?\C:\evorift-test. Verbatim paths switch off ALL path normalization, which breaks
# PowerShell in two ways that both surface as "the directory does not exist":
#   * a forward slash is never translated, so 'docs/captures' cannot resolve;
#   * Join-Path fails outright ("the value of argument drive is null").
# So strip the prefix and build the path by hand, with backslashes.
$root = (Get-Location).Path
if ($root.StartsWith('\\?\')) { $root = $root.Substring(4) }
$root = $root.TrimEnd('\')

$rel = ($Dir -replace '/', '\').Trim('\')
$target = "$root\$rel"

if (-not (Test-Path -LiteralPath $target)) { Write-Error "no such directory: $target"; exit 1 }
Get-ChildItem -LiteralPath $target -Recurse -File | ForEach-Object {
    $_.FullName.Substring($root.Length).TrimStart('\') -replace '\\', '/'
}
exit 0
'@ | Out-File -FilePath $listPs1 -Encoding utf8
        Send-AgentFile -LocalPath $listPs1 -Destination "scripts/list-files.ps1" -RetrySeconds 30 | Out-Null

        if ($BuildPath) {
            Write-Step "2. Pushing the build"
            if (-not (Test-Path $BuildPath)) { throw "BuildPath not found: $BuildPath" }
            Send-AgentFile -LocalPath $BuildPath -Destination ("build/" + (Split-Path -Leaf $BuildPath)) -RetrySeconds 60 | Out-Null
        } else {
            Write-Step "2. Pushing the build (SKIPPED -- no -BuildPath given)"
            Write-Info "verifying against whatever build is already in the sandbox"
        }

        Write-Step "3. Capture: BEFORE"
        $id = Start-AgentJob -ScriptPath "scripts/capture-state.ps1" -ScriptArgs @("-Label", "before") -JobTimeout 300 -RetrySeconds 30
        $beforeJob = Wait-AgentJob -Id $id -WaitSeconds 420
        if ($beforeJob.exit_code -ne 0) { Write-Bad "the 'before' capture exited $($beforeJob.exit_code)" }

        Write-Step "4. Verification steps (the link is expected to die during this)"
        Write-Info "each step's output is written to disk on the laptop and pulled back in step 6"

        # These run against the sandbox copy of the build. The link is expected to drop the
        # moment protection turns on, so every poll below tolerates an unreachable agent.
        $verifySteps = @(
            @{ Label = "during"; Note = "state while protection is ON" }
        )
        foreach ($step in $verifySteps) {
            $sid = Start-AgentJob -ScriptPath "scripts/capture-state.ps1" -ScriptArgs @("-Label", $step.Label) -JobTimeout 300 -RetrySeconds 180
            Write-Info "waiting for the '$($step.Label)' capture -- $($step.Note)"
            $sjob = Wait-AgentJob -Id $sid -WaitSeconds 900
            if ($sjob.exit_code -ne 0) { Write-Bad "the '$($step.Label)' capture exited $($sjob.exit_code)" }
        }

        Write-Step "5. Capture: AFTER"
        $aid = Start-AgentJob -ScriptPath "scripts/capture-state.ps1" -ScriptArgs @("-Label", "after") -JobTimeout 300 -RetrySeconds 300
        $afterJob = Wait-AgentJob -Id $aid -WaitSeconds 900
        if ($afterJob.exit_code -ne 0) { Write-Bad "the 'after' capture exited $($afterJob.exit_code)" }

        Write-Step "6. Pulling captures back"
        # LocalDir is $OutDir, not a subfolder: the returned paths are already sandbox-relative
        # ("docs/captures/<run>/<file>"), so the artefact folder mirrors the sandbox layout.
        # Passing a subfolder produced captures\docs\captures\... and recovery\recovery\...
        $pulled = Receive-AgentTree -RemoteDir "docs/captures" -LocalDir $OutDir -RetrySeconds 300
        Write-Ok "pulled $($pulled.Count) capture file(s)"

        # Recovery reports, if the deadman wrote any during the run.
        $recovered = Receive-AgentTree -RemoteDir "recovery" -LocalDir $OutDir -RetrySeconds 120
        if ($recovered.Count -gt 0) { Write-Info "pulled $($recovered.Count) recovery report(s)" }

        Write-Step "7. Deadman check"
        $after = Get-Health -RetrySeconds 300
        $firesAfter = [int]$after.deadman.fires
        $delta = $firesAfter - $firesBefore

        $verdict = [ordered]@{
            started_at          = $stamp
            agent               = $Base
            build_pushed        = if ($BuildPath) { Split-Path -Leaf $BuildPath } else { $null }
            deadman_fires_before = $firesBefore
            deadman_fires_after  = $firesAfter
            deadman_fired        = ($delta -gt 0)
            capture_files_pulled = $pulled.Count
            recovery_reports     = $recovered.Count
        }
        $verdict | ConvertTo-Json -Depth 4 | Out-File -FilePath (Join-Path $OutDir "remote-run.json") -Encoding utf8

        Write-Host ""
        if ($delta -gt 0) {
            Write-Bad "THE DEADMAN FIRED $delta TIME(S) DURING THIS RUN."
            Write-Host "        The laptop recovered itself mid-test, so it killed evorift.exe/winws.exe," -ForegroundColor Red
            Write-Host "        cleared WinDivert and reset DNS partway through. Any 'it worked' reading" -ForegroundColor Red
            Write-Host "        from these captures is NOT trustworthy. Recovery reports are in:" -ForegroundColor Red
            Write-Host "          $(Join-Path $OutDir 'recovery')" -ForegroundColor Red
        } else {
            Write-Ok "the deadman did not fire -- the captures reflect an uninterrupted run"
        }

        Write-Host ""
        Write-Host "Artefacts: $OutDir" -ForegroundColor Cyan
        Write-Host "Summary:   $(Join-Path $OutDir 'remote-run.json')" -ForegroundColor Cyan
    }
}
