<#
.SYNOPSIS
    Clears a STALE WinDivert kernel-driver registration so evorift's own copy can load.
    Run ELEVATED. Safe to run repeatedly.

.DESCRIPTION
    WinDivert is a kernel driver registered as a Windows service. Only ONE registration can be
    loaded at a time, and Windows keeps whichever one got there first — so if an old registration
    is left behind (a previous dev build, an uninstalled copy, or another DPI tool such as
    GoodbyeDPI/zapret), every NEW winws.exe fails to acquire WinDivert and exits immediately. From
    the outside that looks like "the engine won't start": protection never verifies, mode switches
    fail, the tunnel never comes up. The driver itself is fine; the REGISTRATION is wrong.

    Two conditions make this unrecoverable without help, and both were present on the machine this
    script was written for:

      * START_TYPE = DISABLED — the service can never start, so winws can never load it;
      * a live winws.exe still holding the driver open — `sc stop` then fails, and evorift's own
        clear_stale_windivert() (engine.rs) only deletes the registration if the stop SUCCEEDED,
        so it correctly gives up and the stale entry survives every restart.

    This script does what that cleanup can't: kill the holders FIRST, then stop and delete the
    registration. Deleting is safe and is the point — winws recreates it from its own bundle on the
    next start, with the correct path and start type.
#>
[CmdletBinding()]
param([switch]$Force)

$ErrorActionPreference = 'Continue'

function Step { param([string]$Name, [bool]$Ok, [string]$Detail)
    Write-Host ("  {0} {1}{2}" -f $(if ($Ok) { 'ok   ' } else { 'FAIL ' }), $Name, $(if ($Detail) { " -- $Detail" } else { "" }))
}

$elevated = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $elevated) {
    Write-Host "Bu betik YONETICI olarak calistirilmali (sc stop/delete yetki ister)." -ForegroundColor Red
    Write-Host "PowerShell'i 'Run as administrator' ile acip tekrar calistir."
    exit 1
}

Write-Host "=== evorift: stale WinDivert temizligi ==="

# --- 1) Driver'i acik tutan surecleri oldur -------------------------------------------------
# Bunlar olmadan sc stop calismaz: yuklu bir kernel driver, ona handle tutan bir surec varken
# bosaltilamaz. evorift'in kendi temizligi tam burada takiliyor.
$killed = 0
foreach ($name in @("winws", "goodbyedpi", "ciadpi")) {
    $procs = Get-Process -Name $name -ErrorAction SilentlyContinue
    foreach ($p in $procs) {
        try {
            $started = (Get-CimInstance Win32_Process -Filter "ProcessId=$($p.Id)" -ErrorAction SilentlyContinue).CreationDate
            Stop-Process -Id $p.Id -Force -ErrorAction Stop
            $killed++
            Step "killed $name.exe (pid $($p.Id))" $true "started $started"
        } catch {
            Step "kill $name.exe (pid $($p.Id))" $false $_.Exception.Message
        }
    }
}
if ($killed -eq 0) { Step "holder process" $true "calisan winws/goodbyedpi/ciadpi yok" }
Start-Sleep -Milliseconds 800

# --- 2) Kayitli WinDivert servislerini durdur + sil ------------------------------------------
# Silme KASITLI: winws bir sonraki baslatmada kendi bundle'indan dogru yol + dogru start type ile
# yeniden olusturur. Burada birakilan yanlis kayit tam olarak sorunun kendisi.
$any = $false
foreach ($svc in @("WinDivert", "WinDivert1.4", "WinDivert1.1")) {
    $q = (sc.exe query $svc) 2>&1 | Out-String
    if ($q -notmatch "STATE") {
        Step "$svc" $true "kayitli degil"
        continue
    }
    $any = $true
    $cfg = (sc.exe qc $svc) 2>&1 | Out-String
    if ($cfg -match "BINARY_PATH_NAME\s*:\s*(.+)") { Write-Host "     mevcut yol : $($Matches[1].Trim())" }
    if ($cfg -match "START_TYPE\s*:\s*(.+)")       { Write-Host "     start type : $($Matches[1].Trim())" }

    # DISABLED bir servis stop/başlat edilemez — silmeden once demand-start'a al ki stop islesin.
    sc.exe config $svc start= demand | Out-Null
    sc.exe stop   $svc | Out-Null

    $stopped = $false
    for ($i = 0; $i -lt 8; $i++) {
        Start-Sleep -Milliseconds 500
        $q2 = (sc.exe query $svc) 2>&1 | Out-String
        if ($q2 -notmatch "STATE" -or $q2 -match "STOPPED") { $stopped = $true; break }
    }
    Step "$svc stopped" $stopped

    $del = (sc.exe delete $svc) 2>&1 | Out-String
    if ($del -match "SUCCESS") {
        Step "$svc deleted" $true "winws kendi kopyasini yeniden kaydedecek"
    } elseif ($del -match "1072|marked for deletion") {
        Step "$svc deleted" $true "silme beklemede -> YENIDEN BASLATMA gerekli"
        $script:needReboot = $true
    } else {
        Step "$svc deleted" $false ($del -replace '\s+', ' ').Trim()
    }
}
if (-not $any) { Step "WinDivert" $true "temizlenecek kayit yok" }

# --- 3) Sonuc ---------------------------------------------------------------------------------
Write-Host ""
Write-Host "=== sonuc ==="
foreach ($svc in @("WinDivert", "WinDivert1.4", "WinDivert1.1")) {
    $q = (sc.exe query $svc) 2>&1 | Out-String
    if ($q -match "STATE") {
        if ($q -match "STATE\s*:\s*\d+\s+(\w+)") { Write-Host "  $svc : hala kayitli ($($Matches[1]))" }
    } else {
        Write-Host "  $svc : temiz (kayit yok)"
    }
}
$leftover = Get-Process -Name winws -ErrorAction SilentlyContinue
if ($leftover) { Write-Host "  UYARI: winws.exe hala calisiyor (pid $($leftover.Id -join ','))" -ForegroundColor Yellow }
else { Write-Host "  winws.exe : calismiyor" }

if ($script:needReboot) {
    Write-Host ""
    Write-Host "Silme beklemede: evorift'i test etmeden once PC'yi YENIDEN BASLAT." -ForegroundColor Yellow
} else {
    Write-Host ""
    Write-Host "Temiz. evorift'i simdi baslatabilirsin." -ForegroundColor Green
}
exit 0
