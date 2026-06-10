# deploy-svc-warp.ps1 — Motor B'li yeni evorift-svc'yi ÇALIŞAN kuruluma uygular (SELF-ELEVATING/UAC).
#   1) yeni evorift-svc.exe  -> C:\Program Files\evorift\
#   2) WARP binary'leri       -> C:\Program Files\evorift\warp\  (wgcf/wireguard/wintun)
#   3) servisi yeniden başlat -> auto-protect: winws + Motor B (wgcf register + tünel) + DNS
#   4) test-evorift.ps1 çalıştır -> deploy-result.txt (PASS/FAIL) + deploy-svc-warp.log
$ErrorActionPreference = 'Continue'
$base   = 'C:\Users\Evrim\Desktop\projects\net\src-tauri'
$dir    = 'C:\Program Files\evorift'
$log    = "$base\examples\deploy-svc-warp.log"
$resf   = "$base\examples\deploy-result.txt"
$svcSrc = "$base\target\release\evorift-svc.exe"
$warpSrc= "$base\resources\warp"
$ts     = Get-Date -Format 'yyyyMMdd-HHmmss'

# --- self-elevation (UAC) ---
$pr = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $pr.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
  Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File',"`"$PSCommandPath`""
  exit
}

function L($m){ "$((Get-Date -Format 'HH:mm:ss')) $m" | Tee-Object -FilePath $log -Append | Out-Null }
"=== deploy-svc-warp $(Get-Date -Format o) ===" | Set-Content $log
Set-Content $resf 'RUNNING'

if (-not (Test-Path $svcSrc)) { L "HATA: yok $svcSrc"; Set-Content $resf 'FAIL(no-svc-build)'; exit 1 }
L ("Kaynak svc : " + (Get-Item $svcSrc).LastWriteTime + " (" + (Get-Item $svcSrc).Length + " byte)")

# 1) servisi + winws durdur (job object zaten winws'i öldürür; yine de garanti)
L "Servis durduruluyor..."
sc.exe stop EvoriftSvc *>&1 | Out-Null
Start-Sleep -Seconds 2
taskkill /f /im winws.exe *>&1 | Out-Null
Start-Sleep -Seconds 1

# 2) yedekle + kopyala (svc)
if (Test-Path "$dir\evorift-svc.exe") { Copy-Item "$dir\evorift-svc.exe" "$dir\evorift-svc.exe.bak-$ts" -Force; L "Yedek: evorift-svc.exe.bak-$ts" }
Copy-Item $svcSrc "$dir\evorift-svc.exe" -Force
Copy-Item $svcSrc "$base\resources\evorift-svc.exe" -Force   # installer bundle'ı da güncel kalsın
L ("Yeni svc   : " + (Get-Item "$dir\evorift-svc.exe").LastWriteTime + " (" + (Get-Item "$dir\evorift-svc.exe").Length + " byte)")

# 3) WARP binary'lerini exe yanına (<dir>\warp) kopyala — WarpEngine::bundle_dir burayı arar
New-Item -ItemType Directory -Force -Path "$dir\warp" | Out-Null
foreach ($f in 'wgcf.exe','wireguard.exe','wintun.dll') {
  if (Test-Path "$warpSrc\$f") { Copy-Item "$warpSrc\$f" "$dir\warp\$f" -Force }
}
$wc = @(Get-ChildItem "$dir\warp" -File -Include *.exe,*.dll -ErrorAction SilentlyContinue).Count
L ("WARP binary: $dir\warp\ (" + $wc + " dosya)")

# 4) eski token temizle + servisi başlat
Remove-Item "$env:ProgramData\evorift\ipc.token" -ErrorAction SilentlyContinue
# eski warp.conf'u sil → yeni svc temiz config üretsin (split-tunnel + DNS-strip mantığı yeni)
Remove-Item "$env:ProgramData\evorift\warp.conf" -ErrorAction SilentlyContinue
L "Servis baslatiliyor..."
sc.exe start EvoriftSvc *>&1 | Out-Null

# 5) auto-protect'in oturmasını bekle: winws hemen; Motor B wgcf register (ağ) + tünel kurulumu ~10-25 sn
L "auto-protect bekleniyor (winws + Motor B wgcf register + tunel)..."
Start-Sleep -Seconds 25

# durum özeti
$svc = Get-Service EvoriftSvc -ErrorAction SilentlyContinue
L ("Servis durumu : " + $(if($svc){$svc.Status}else{'YOK'}))
$tun = Get-Service 'WireGuardTunnel$warp' -ErrorAction SilentlyContinue
L ("WARP tunel    : " + $(if($tun){$tun.Status}else{'YOK'}))
$winws = @(Get-Process winws -ErrorAction SilentlyContinue).Count
L ("winws.exe     : $winws instance")
if (Test-Path "$env:ProgramData\evorift\warp.conf") { L ("warp.conf     : VAR (" + (Get-Item "$env:ProgramData\evorift\warp.conf").Length + " byte)") } else { L "warp.conf     : YOK (wgcf register basarisiz olabilir)" }
# SCM çökme olayı (7031) son 5 dk içinde var mı? (crash-proof doğrulama)
$crash = @(Get-WinEvent -FilterHashtable @{LogName='System';Id=7031;StartTime=(Get-Date).AddMinutes(-5)} -ErrorAction SilentlyContinue | Where-Object {$_.Message -match 'Evorift'})
L ("SCM 7031 crash: " + $crash.Count + " (0 beklenir)")

# 6) §9 testini çalıştır
L "test-evorift.ps1 calistiriliyor..."
$testOut = & powershell -NoProfile -ExecutionPolicy Bypass -File "$base\examples\test-evorift.ps1" 2>&1
$testOut | Tee-Object -FilePath $log -Append | Out-Null
$result = if ($testOut -match 'RESULT: PASS') { 'PASS' } else { 'FAIL' }
L "SONUC: $result"
Set-Content $resf $result
L "=== bitti ==="
