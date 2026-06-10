# Yeni release build'lerini Program Files'a kur (ELEVATED). evorift.exe + evorift-svc.exe.
# Eski exe'leri zaman damgali yedekler, servisi/surecleri durdurur, kopyalar, servisi baslatir, dogrular.
$ErrorActionPreference = 'Continue'
$base = 'C:\Users\Evrim\Desktop\projects\net\src-tauri'
$dir  = 'C:\Program Files\evorift'
$log  = "$base\examples\redeploy-all.log"
$ts   = Get-Date -Format 'yyyyMMdd-HHmmss'
function L($m){ $m | Tee-Object -FilePath $log -Append | Out-Null }
"=== redeploy-all $(Get-Date -Format o) ===" | Set-Content $log

$appSrc = "$base\target\release\evorift.exe"
$svcSrc = "$base\target\release\evorift-svc.exe"

# 0) kaynaklar var mi?
if (-not (Test-Path $appSrc)) { L "HATA: bulunamadi $appSrc"; exit 1 }
if (-not (Test-Path $svcSrc)) { L "HATA: bulunamadi $svcSrc"; exit 1 }
L ("Kaynak app : " + (Get-Item $appSrc).LastWriteTime + " (" + (Get-Item $appSrc).Length + " byte)")
L ("Kaynak svc : " + (Get-Item $svcSrc).LastWriteTime + " (" + (Get-Item $svcSrc).Length + " byte)")

# 1) servisi + surecleri durdur (exe kilitleri birakilsin)
sc.exe stop EvoriftSvc *>&1 | Out-Null
Start-Sleep -Seconds 2
Get-Process evorift,rift -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue }
Start-Sleep -Seconds 1

# 2) eski exe'leri yedekle
if (Test-Path "$dir\evorift.exe")     { Copy-Item "$dir\evorift.exe"     "$dir\evorift.exe.bak-$ts"     -Force; L "Yedek: evorift.exe.bak-$ts" }
if (Test-Path "$dir\evorift-svc.exe") { Copy-Item "$dir\evorift-svc.exe" "$dir\evorift-svc.exe.bak-$ts" -Force; L "Yedek: evorift-svc.exe.bak-$ts" }

# 3) yeni binary'leri kopyala (svc -> resources/'a da, installer icin)
Copy-Item $appSrc "$dir\evorift.exe"     -Force
Copy-Item $svcSrc "$dir\evorift-svc.exe" -Force
Copy-Item $svcSrc "$base\resources\evorift-svc.exe" -Force
L ("Yeni app   : " + (Get-Item "$dir\evorift.exe").LastWriteTime     + " (" + (Get-Item "$dir\evorift.exe").Length     + " byte)")
L ("Yeni svc   : " + (Get-Item "$dir\evorift-svc.exe").LastWriteTime + " (" + (Get-Item "$dir\evorift-svc.exe").Length + " byte)")

# 4) eski token temizle + servisi baslat
Remove-Item "$env:ProgramData\evorift\ipc.token" -ErrorAction SilentlyContinue
Remove-Item "$env:TEMP\evorift-ipc.token" -ErrorAction SilentlyContinue
sc.exe start EvoriftSvc *>&1 | Out-Null
Start-Sleep -Seconds 3

# 5) dogrula
$svc = (Get-Service EvoriftSvc -ErrorAction SilentlyContinue)
L ("Servis durumu: " + $(if($svc){$svc.Status}else{'YOK'}))
$tok = "$env:ProgramData\evorift\ipc.token"
if (Test-Path $tok) { L "TOKEN VAR ($((Get-Item $tok).Length) byte)" } else { L "TOKEN YOK" }
$pipe = [System.IO.Directory]::GetFiles('\\.\pipe\') | Where-Object {$_ -match 'evorift'}
L ("PIPE: " + ($(if($pipe){$pipe -join ','}else{'YOK'})))
L "=== bitti ==="
