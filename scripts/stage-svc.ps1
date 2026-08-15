<#
.SYNOPSIS
    Stage the freshly built evorift-svc.exe into src-tauri/resources/ before bundling.
    Called by tauri.conf.json's beforeBundleCommand. FAILS THE BUILD if the copy does not land.

.DESCRIPTION
    The installer and the portable zip ship resources/evorift-svc.exe, NOT the binary under
    target/release. If that copy silently does not happen, the app binary is new while the SERVICE
    binary is old - and since the service is what actually runs the engine, the shipped build
    behaves exactly like the old version no matter how much code changed.

    Not hypothetical: this happened for a full day on 2026-08-15. beforeBundleCommand was an inline
    powershell -Command "Copy-Item ... -Force" which (a) got mangled by quoting and merely echoed
    the command text instead of running it, and (b) would not have failed the build anyway, because
    Copy-Item raises a NON-TERMINATING error - PowerShell still exits 0, so Tauri happily bundled a
    service binary that was ten weeks stale. Three "fixed" releases shipped the old engine and every
    live test faithfully reproduced the original bug.

    Hence: a real script file (no inline-quoting minefield), ErrorActionPreference Stop, and a
    post-copy HASH COMPARISON so a lock, an AV hold or a partial write fails the build loudly
    instead of quietly shipping the wrong binary.

    ASCII ONLY on purpose: Windows PowerShell 5.1 reads .ps1 as ANSI unless the file has a BOM, so
    non-ASCII characters here turn into parse errors on exactly the machines that run the build.
#>
$ErrorActionPreference = 'Stop'

$src = Join-Path $PSScriptRoot "..\src-tauri\target\release\evorift-svc.exe"
$dst = Join-Path $PSScriptRoot "..\src-tauri\resources\evorift-svc.exe"

if (-not (Test-Path -LiteralPath $src)) {
    throw "evorift-svc.exe not built: $src -- run: cargo build --release --bin evorift-svc"
}

# The destination is very often locked: EvoriftSvc may be running from a previous install, or an AV
# scanner may still hold the file cargo just wrote. Retry briefly - but NEVER continue on failure.
$copied = $false
for ($i = 1; $i -le 5; $i++) {
    try {
        Copy-Item -LiteralPath $src -Destination $dst -Force
        $copied = $true
        break
    } catch {
        if ($i -eq 5) {
            throw ("evorift-svc.exe copy failed after $i attempts: " + $_.Exception.Message +
                   " -- EvoriftSvc may be running (sc stop EvoriftSvc)")
        }
        Start-Sleep -Milliseconds 600
    }
}
if (-not $copied) { throw "evorift-svc.exe copy failed" }

# Belt and braces: prove the bytes actually match. A copy that "succeeded" but left the old file
# (or half a file) is precisely the failure this script exists to prevent.
$hs = (Get-FileHash -LiteralPath $src -Algorithm SHA256).Hash
$hd = (Get-FileHash -LiteralPath $dst -Algorithm SHA256).Hash
if ($hs -ne $hd) {
    throw ("evorift-svc.exe STAGE VERIFICATION FAILED. build=" + $hs + " bundle=" + $hd +
           " -- the bundle would have shipped a STALE service binary; build stopped.")
}

$len = (Get-Item -LiteralPath $dst).Length
Write-Host ("stage-svc: evorift-svc.exe staged, " + $len + " bytes, sha256 " + $hd.Substring(0, 16) + "...")
