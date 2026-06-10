# deploy-full.ps1 — UI (evorift.exe) + servis (evorift-svc.exe) + WARP'i tek seferde günceller.
# Ses düzeltmesi (104.29.146.0/24) + adaptif domain izleme (100ms→5sn) DAHIL. warp.conf yeniden üretilir.
# SELF-ELEVATING. UI yeniden başlatmayı çağıran batch (kullanıcı bağlamında) yapar.
$ErrorActionPreference='Continue'
$base='C:\Users\Evrim\Desktop\projects\net\src-tauri'
$dir='C:\Program Files\evorift'
$log="$base\examples\deploy-full.log"; $done="$base\examples\deploy-full.done"
$appSrc="$base\target\release\evorift.exe"; $svcSrc="$base\target\release\evorift-svc.exe"
$ts=Get-Date -Format 'yyyyMMdd-HHmmss'
$pr=[Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if(-not $pr.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){
  Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File',"`"$PSCommandPath`""; exit }
function L($m){ Add-Content -Path $log -Value ("$((Get-Date -Format 'HH:mm:ss')) $m") }
Set-Content $log "=== deploy-full $(Get-Date -Format o) ==="; Set-Content $done 'RUNNING'

if(-not (Test-Path $svcSrc)){ L "HATA: yok $svcSrc"; Set-Content $done 'FAIL'; exit 1 }
if(-not (Test-Path $appSrc)){ L "UYARI: yok $appSrc — UI guncellenmeyecek (yalniz svc)" }
L ("svc kaynak: " + (Get-Item $svcSrc).LastWriteTime); if(Test-Path $appSrc){ L ("ui  kaynak: " + (Get-Item $appSrc).LastWriteTime) }

# 1) durdur: servis + winws + UI + eski tünel
sc.exe stop EvoriftSvc *>&1 | Out-Null; Start-Sleep 2
taskkill /f /im winws.exe *>&1 | Out-Null
Get-Process evorift -EA SilentlyContinue | Stop-Process -Force -EA SilentlyContinue
& "$dir\warp\wireguard.exe" /uninstalltunnelservice warp *>&1 | Out-Null
Start-Sleep 2

# 2) yedekle + kopyala
if(Test-Path "$dir\evorift-svc.exe"){ Copy-Item "$dir\evorift-svc.exe" "$dir\evorift-svc.exe.bak-$ts" -Force }
Copy-Item $svcSrc "$dir\evorift-svc.exe" -Force
Copy-Item $svcSrc "$base\resources\evorift-svc.exe" -Force
if(Test-Path $appSrc){
  if(Test-Path "$dir\evorift.exe"){ Copy-Item "$dir\evorift.exe" "$dir\evorift.exe.bak-$ts" -Force }
  Copy-Item $appSrc "$dir\evorift.exe" -Force
  L ("UI kopyalandi: " + (Get-Item "$dir\evorift.exe").LastWriteTime)
}
# WARP binary'leri (idempotent)
New-Item -ItemType Directory -Force -Path "$dir\warp" | Out-Null
foreach($f in 'wgcf.exe','wireguard.exe','wintun.dll'){ if((Test-Path "$base\resources\warp\$f")){ Copy-Item "$base\resources\warp\$f" "$dir\warp\$f" -Force } }

# 3) warp.conf'u SIL → yeni voice araliklariyla (104.29.146.0/24) regen. token temizle.
Remove-Item "$env:ProgramData\evorift\warp.conf","$env:ProgramData\evorift\ipc.token" -EA SilentlyContinue

# 4) servisi etkin + başlat
sc.exe config EvoriftSvc start= auto *>&1 | Out-Null
sc.exe failure EvoriftSvc reset= 86400 actions= restart/5000/restart/5000/restart/5000 *>&1 | Out-Null
sc.exe start EvoriftSvc *>&1 | Out-Null
L "auto-protect bekleniyor (regen + tunel, ~25s)..."; Start-Sleep 25

# 5) doğrula
$svc=Get-Service EvoriftSvc -EA SilentlyContinue; L ("Servis    : " + $(if($svc){$svc.Status}else{'YOK'}))
$tun=Get-Service 'WireGuardTunnel$warp' -EA SilentlyContinue; L ("WARP tunel: " + $(if($tun){$tun.Status}else{'YOK'}))
L ("winws.exe : " + @(Get-Process winws -EA SilentlyContinue).Count)
if(Test-Path "$env:ProgramData\evorift\warp.conf"){ L ("AllowedIPs: " + ((Get-Content "$env:ProgramData\evorift\warp.conf" | Where-Object {$_ -match 'AllowedIPs'}))) }
$r=Get-NetRoute -DestinationPrefix '104.29.146.0/24' -EA SilentlyContinue
L ("ses rota 104.29.146.0/24: " + $(if($r){'warp = TUNELDE!'}else{'YOK (hala raw)'}))
Set-Content $done 'DONE'; L "=== bitti ==="