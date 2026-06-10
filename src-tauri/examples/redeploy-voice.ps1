# redeploy-voice.ps1 — SES düzeltmesi: yeni svc + warp.conf'u YENIDEN ÜRET (104.29.146.0/24 dahil). SELF-ELEVATING.
$ErrorActionPreference='Continue'
$base='C:\Users\Evrim\Desktop\projects\net\src-tauri'
$dir='C:\Program Files\evorift'
$log="$base\examples\redeploy-voice.log"
$done="$base\examples\redeploy-voice.done"
$svcSrc="$base\target\release\evorift-svc.exe"
$ts=Get-Date -Format 'yyyyMMdd-HHmmss'
$pr=[Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if(-not $pr.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){
  Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File',"`"$PSCommandPath`""; exit }
function L($m){ Add-Content -Path $log -Value ("$((Get-Date -Format 'HH:mm:ss')) $m") }
Set-Content $log "=== redeploy-voice $(Get-Date -Format o) ==="
Set-Content $done 'RUNNING'

L ("Kaynak svc: " + (Get-Item $svcSrc).LastWriteTime + " (" + (Get-Item $svcSrc).Length + " byte)")
# 1) durdur + eski tüneli kaldır
sc.exe stop EvoriftSvc *>&1 | Out-Null; Start-Sleep 2
taskkill /f /im winws.exe *>&1 | Out-Null
& "$dir\warp\wireguard.exe" /uninstalltunnelservice warp *>&1 | Out-Null
Start-Sleep 2
# 2) yeni svc kopyala
if(Test-Path "$dir\evorift-svc.exe"){ Copy-Item "$dir\evorift-svc.exe" "$dir\evorift-svc.exe.bak-$ts" -Force }
Copy-Item $svcSrc "$dir\evorift-svc.exe" -Force
Copy-Item $svcSrc "$base\resources\evorift-svc.exe" -Force
# 3) warp.conf'u SIL → yeni kod yeni AllowedIPs (104.29.146.0/24) ile YENIDEN üretsin. token da temizle.
Remove-Item "$env:ProgramData\evorift\warp.conf","$env:ProgramData\evorift\ipc.token" -EA SilentlyContinue
L "warp.conf silindi -> regen edilecek"
# 4) servisi etkin tut + başlat
sc.exe config EvoriftSvc start= auto *>&1 | Out-Null
sc.exe start EvoriftSvc *>&1 | Out-Null
L "auto-protect bekleniyor (regen + tunel, ~25s)..."
Start-Sleep -Seconds 25
# 5) doğrula
$svc=Get-Service EvoriftSvc -EA SilentlyContinue; L ("Servis    : " + $(if($svc){$svc.Status}else{'YOK'}))
$tun=Get-Service 'WireGuardTunnel$warp' -EA SilentlyContinue; L ("WARP tunel: " + $(if($tun){$tun.Status}else{'YOK'}))
L ("winws.exe : " + @(Get-Process winws -EA SilentlyContinue).Count)
if(Test-Path "$env:ProgramData\evorift\warp.conf"){
  $ai = (Get-Content "$env:ProgramData\evorift\warp.conf" | Where-Object {$_ -match 'AllowedIPs'})
  L ("warp.conf AllowedIPs: " + $ai)
} else { L "warp.conf: YOK (regen basarisiz!)" }
$r=Get-NetRoute -DestinationPrefix '104.29.146.0/24' -EA SilentlyContinue
L ("104.29.146.0/24 rota: " + $(if($r){'ifIndex '+$r[0].ifIndex+' (warp = TUNELDE!)'}else{'YOK (hala raw!)'}))
Set-Content $done 'DONE'
L "=== bitti ==="