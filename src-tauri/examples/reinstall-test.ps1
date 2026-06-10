# reinstall-test.ps1 — bozuk kurulumu kaldir + YENI installer'i sessiz kur + dayaniklilik fix'lerini dogrula.
#   Ozellikle: (1) WinDivert auto-clear (zapret surucusu bilerek birakiliyor -> winws yine de calismali),
#              (2) WARP bundle (warp/ klasoru + tunel), (3) autostart kisayolu, (4) TLS 6/6.
#   Cikti: reinstall-test.log + reinstall-test.result (RUNNING/PASS/FAIL). SELF-ELEVATING (tek UAC).
$ErrorActionPreference = 'Continue'
$base  = 'C:\Users\Evrim\Desktop\projects\net\src-tauri'
$dir   = 'C:\Program Files\evorift'
$pd    = "$env:ProgramData\evorift"
$setup = "$base\target\release\bundle\nsis\evorift_0.1.0_x64-setup.exe"
$lnk   = "$env:ProgramData\Microsoft\Windows\Start Menu\Programs\Startup\evorift.lnk"
$log   = "$base\examples\reinstall-test.log"
$res   = "$base\examples\reinstall-test.result"
$wg    = "$dir\warp\wireguard.exe"

$pr = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $pr.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
  Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File',"`"$PSCommandPath`""
  exit
}
function L($m){ Add-Content -Path $log -Value ("$((Get-Date -Format 'HH:mm:ss')) $m") }
Set-Content $log "=== reinstall-test $(Get-Date -Format o) ==="; Set-Content $res 'RUNNING'
if (-not (Test-Path $setup)) { L "HATA: setup yok $setup"; Set-Content $res 'FAIL(no-setup)'; exit 1 }
L ("Setup: " + (Get-Item $setup).LastWriteTime + " (" + [math]::Round((Get-Item $setup).Length/1MB,2) + " MB)")

# === FAZ A: mevcut (bozuk) kurulumu kaldir — zapret WinDivert'e DOKUNMA (auto-clear'i test edecegiz) ===
L "--- FAZ A: eski kurulumu kaldir ---"
sc.exe stop EvoriftSvc *>&1 | Out-Null; Start-Sleep 2
sc.exe delete EvoriftSvc *>&1 | Out-Null
if (Test-Path $wg) { & $wg /uninstalltunnelservice warp *>&1 | Out-Null; Start-Sleep 2 }
taskkill /f /im winws.exe   *>&1 | Out-Null
taskkill /f /im evorift.exe *>&1 | Out-Null
Remove-Item $lnk -ErrorAction SilentlyContinue
Remove-Item $dir, $pd -Recurse -Force -ErrorAction SilentlyContinue
$preWD = sc.exe query WinDivert 2>&1 | Out-String
L ("Eski kurulum silindi. Kurulum ONCESI WinDivert surucu: " + $(if($preWD -match 'RUNNING'){'RUNNING (zapret cakismasi DURUYOR)'}elseif($preWD -match 'STOPPED'){'STOPPED'}else{'YOK'}))

# === FAZ B: yeni installer'i SESSIZ kur (zaten elevated -> ek UAC yok) ===
L "--- FAZ B: yeni installer (sessiz /S) ---"
$p = Start-Process $setup -ArgumentList '/S' -PassThru -Wait
L ("Installer exit kodu: " + $p.ExitCode)
Start-Sleep 6

# === FAZ C: auto-protect bekle + dogrula ===
L "--- FAZ C: auto-protect (winws auto-clear + wgcf + tunel) bekle ~30s ---"
Start-Sleep 30
$svc   = Get-Service EvoriftSvc -ErrorAction SilentlyContinue
$winws = @(Get-Process winws -ErrorAction SilentlyContinue).Count
$tun   = Get-Service 'WireGuardTunnel$warp' -ErrorAction SilentlyContinue
L ("EvoriftSvc        : " + $(if($svc){$svc.Status}else{'YOK'}))
L ("winws.exe         : $winws instance   <-- WinDivert AUTO-CLEAR KANITI (>=1 beklenir)")
L ("WARP tunnel       : " + $(if($tun){$tun.Status}else{'YOK'}))
L ("warp\ klasoru     : " + $(if(Test-Path $wg){'VAR (bundle dogru)'}else{'YOK <-- bundle hatasi'}))
L ("warp.conf         : " + $(if(Test-Path "$pd\warp.conf"){'VAR (' + (Get-Item "$pd\warp.conf").Length + ' byte)'}else{'YOK'}))
L ("autostart kisayolu: " + $(if(Test-Path $lnk){'VAR (Windows ile acilir)'}else{'YOK'}))
$postWD = (sc.exe qc WinDivert 2>&1 | Out-String) -split "`n" | Select-String 'BINARY_PATH_NAME'
L ("Kurulum SONRASI WinDivert path: " + ($postWD -join '').Trim() + "  (artik evorift\winws olmali, zapret degil)")
$t = & powershell -NoProfile -ExecutionPolicy Bypass -File "$base\examples\test-evorift.ps1" 2>&1
$t | Add-Content $log
$tls = if ($t -match 'RESULT: PASS') { 'PASS' } else { 'FAIL' }
L ("TLS testi (6 cekirdek domain): $tls")

$ok = $svc -and ($svc.Status -eq 'Running') -and ($winws -ge 1) -and $tun -and ($tls -eq 'PASS')
Set-Content $res $(if($ok){'PASS'}else{'FAIL'})
L ("=== bitti (" + $(if($ok){'PASS'}else{'FAIL'}) + ") ===")
