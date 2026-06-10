# enable-and-test.ps1 — EvoriftSvc'yi yeniden ETKİNLEŞTİR (disabled→auto) + başlat + Motor B doğrula + test.
# (diag: servis 'disabled' idi → 1058; konsol modunda winws+WARP tüneli+warp.conf ÇALIŞIYOR.) SELF-ELEVATING.
$ErrorActionPreference='Continue'
$base='C:\Users\Evrim\Desktop\projects\net\src-tauri'
$dir='C:\Program Files\evorift'
$log="$base\examples\enable-and-test.log"
$resf="$base\examples\deploy-result.txt"
$pr=[Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if(-not $pr.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){
  Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File',"`"$PSCommandPath`""; exit }
function L($m){ "$((Get-Date -Format 'HH:mm:ss')) $m" | Tee-Object -FilePath $log -Append | Out-Null }
"=== enable-and-test $(Get-Date -Format o) ===" | Set-Content $log
Set-Content $resf 'RUNNING'

# 0) WARP binary'leri yerinde mi? (idempotent — eksikse kopyala)
New-Item -ItemType Directory -Force -Path "$dir\warp" | Out-Null
foreach($f in 'wgcf.exe','wireguard.exe','wintun.dll'){ if((Test-Path "$base\resources\warp\$f") -and -not (Test-Path "$dir\warp\$f")){ Copy-Item "$base\resources\warp\$f" "$dir\warp\$f" -Force } }

# 1) servisi YENIDEN ETKINLESTIR (disabled -> auto) — kök neden buydu
$c = sc.exe config EvoriftSvc start= auto 2>&1 | Out-String; L ("sc config start=auto -> " + $c.Trim())
# çökme kurtarma da ekle (crash-proof'landı)
sc.exe failure EvoriftSvc reset= 86400 actions= restart/5000/restart/5000/restart/5000 *>&1 | Out-Null

# 2) temiz başlat
sc.exe stop EvoriftSvc *>&1 | Out-Null; Start-Sleep 2; taskkill /f /im winws.exe *>&1 | Out-Null
Remove-Item "$env:ProgramData\evorift\ipc.token" -ErrorAction SilentlyContinue
$s = sc.exe start EvoriftSvc 2>&1 | Out-String; L ("sc start -> " + $s.Trim())
$svc=Get-Service EvoriftSvc -ErrorAction SilentlyContinue; L ("baslangic durumu: " + $(if($svc){$svc.Status}else{'YOK'}))

# 3) auto-protect otursun (winws + Motor B tünel + DNS). warp.conf varsa tünel hızlı kurulur.
L "auto-protect bekleniyor (~25s)..."
Start-Sleep -Seconds 25

# 4) durum özeti
$svc=Get-Service EvoriftSvc -ErrorAction SilentlyContinue; L ("Servis     : " + $(if($svc){$svc.Status}else{'YOK'}))
$tun=Get-Service 'WireGuardTunnel$warp' -ErrorAction SilentlyContinue; L ("WARP tunel : " + $(if($tun){$tun.Status}else{'YOK'}))
L ("winws.exe  : " + @(Get-Process winws -ErrorAction SilentlyContinue).Count + " instance")
if(Test-Path "$env:ProgramData\evorift\warp.conf"){ L ("warp.conf  : VAR " + (Get-Item "$env:ProgramData\evorift\warp.conf").Length + " byte") } else { L "warp.conf  : YOK" }
$crash=@(Get-WinEvent -FilterHashtable @{LogName='System';Id=7031;StartTime=(Get-Date).AddMinutes(-5)} -ErrorAction SilentlyContinue | Where-Object {$_.Message -match 'vorift'})
L ("SCM 7031   : " + $crash.Count + " (0 beklenir)")

# 5) §9 test
L "test calistiriliyor..."
$o = & powershell -NoProfile -ExecutionPolicy Bypass -File "$base\examples\test-evorift.ps1" 2>&1
$o | Tee-Object -FilePath $log -Append | Out-Null
$result = if($o -match 'RESULT: PASS'){ 'PASS' } else { 'FAIL' }
Set-Content $resf $result
L "SONUC: $result"
L "=== bitti ==="