# restore-zapret.ps1 — Motor B (WARP) KAPALI svc'yi kur + WARP tünelini KALDIR → SAF winws/zapret.
# Kullanıcı: "normal zapret mükemmel calisiyordu" → ses RTC icin kanitlanmis yol. SELF-ELEVATING.
$ErrorActionPreference='Continue'
$base='C:\Users\Evrim\Desktop\projects\net\src-tauri'
$dir='C:\Program Files\evorift'
$log="$base\examples\restore-zapret.log"; $done="$base\examples\restore-zapret.done"
$svcSrc="$base\target\release\evorift-svc.exe"
$ts=Get-Date -Format 'yyyyMMdd-HHmmss'
$pr=[Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if(-not $pr.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){
  Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File',"`"$PSCommandPath`""; exit }
function L($m){ Add-Content -Path $log -Value ("$((Get-Date -Format 'HH:mm:ss')) $m") }
Set-Content $log "=== restore-zapret $(Get-Date -Format o) ==="; Set-Content $done 'RUNNING'

L ("svc kaynak: " + (Get-Item $svcSrc -EA SilentlyContinue).LastWriteTime + " (" + (Get-Item $svcSrc -EA SilentlyContinue).Length + " byte)")
# 1) durdur + WARP tünelini KALDIR (Engine B'nin yarattigi WireGuardTunnel$warp servisi)
sc.exe stop EvoriftSvc *>&1 | Out-Null; Start-Sleep 2
taskkill /f /im winws.exe *>&1 | Out-Null
& "$dir\warp\wireguard.exe" /uninstalltunnelservice warp *>&1 | Out-Null
Start-Sleep 2
L "WARP tuneli kaldirildi (varsa)."
# 2) yeni (WARP kapali) svc kopyala + UI (prod, tauri ile derlenmis) varsa guncelle
Get-Process evorift -EA SilentlyContinue | Stop-Process -Force -EA SilentlyContinue
if(Test-Path "$dir\evorift-svc.exe"){ Copy-Item "$dir\evorift-svc.exe" "$dir\evorift-svc.exe.bak-$ts" -Force }
Copy-Item $svcSrc "$dir\evorift-svc.exe" -Force
Copy-Item $svcSrc "$base\resources\evorift-svc.exe" -Force
$appSrc="$base\target\release\evorift.exe"
if(Test-Path $appSrc){
  if(Test-Path "$dir\evorift.exe"){ Copy-Item "$dir\evorift.exe" "$dir\evorift.exe.bak-$ts" -Force }
  Copy-Item $appSrc "$dir\evorift.exe" -Force
  L ("UI guncellendi (prod): " + (Get-Item "$dir\evorift.exe").LastWriteTime + " (" + (Get-Item "$dir\evorift.exe").Length + " byte)")
}
# 3) warp.conf + token temizle (tünel config'ine gerek yok)
Remove-Item "$env:ProgramData\evorift\warp.conf","$env:ProgramData\evorift\ipc.token" -EA SilentlyContinue
# 4) servisi etkin + başlat (saf winws auto-protect)
sc.exe config EvoriftSvc start= auto *>&1 | Out-Null
sc.exe failure EvoriftSvc reset= 86400 actions= restart/5000/restart/5000/restart/5000 *>&1 | Out-Null
sc.exe start EvoriftSvc *>&1 | Out-Null
L "winws auto-protect bekleniyor (~12s)..."; Start-Sleep 12
# 5) doğrula: servis + winws AÇIK, WARP tüneli YOK, Discord IP'leri tünele GİTMİYOR (saf winws)
$svc=Get-Service EvoriftSvc -EA SilentlyContinue; L ("Servis    : " + $(if($svc){$svc.Status}else{'YOK'}))
L ("winws.exe : " + @(Get-Process winws -EA SilentlyContinue).Count + " instance")
$tun=Get-Service 'WireGuardTunnel$warp' -EA SilentlyContinue; L ("WARP tunel: " + $(if($tun){'HALA VAR! ('+$tun.Status+')'}else{'YOK (iyi — saf winws)'}))
$r=Get-NetRoute -DestinationPrefix '162.159.0.0/16' -EA SilentlyContinue
L ("162.159/16 rota: " + $(if($r){'ifIndex '+$r[0].ifIndex+' (tunel kalmis olabilir!)'}else{'normal (tunel yok)'}))
Set-Content $done 'DONE'; L "=== bitti ==="