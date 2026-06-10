# EvoriftSvc binary'sini guncelle (ACL duzeltmeli yeni evorift-svc.exe). YONETICI.
# Servisi durdur -> yeni exe'yi Program Files'a kopyala -> baslat -> token+pipe dogrula.
$ErrorActionPreference = 'Continue'
$log = "C:\Users\Evrim\Desktop\projects\net\src-tauri\examples\update-svc.log"
function L($m){ $m | Tee-Object -FilePath $log -Append }
"=== update-svc $(Get-Date -Format o) ===" | Set-Content $log

$dir  = "C:\Program Files\evorift"
$base = "C:\Users\Evrim\Desktop\projects\net\src-tauri"

# 1) servisi durdur (exe kilidi birakilsin)
sc.exe stop EvoriftSvc *>&1 | Out-Null
Start-Sleep -Seconds 2
# cakisan app surecleri (gomulu sunuculu) de pipe tutmasin
Get-Process evorift,rift -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue }
Start-Sleep -Seconds 1

# 2) yeni binary'yi kopyala (+ resources/'a da, installer icin)
Copy-Item "$base\target\release\evorift-svc.exe" "$dir\evorift-svc.exe" -Force
Copy-Item "$base\target\release\evorift-svc.exe" "$base\resources\evorift-svc.exe" -Force
L ("Yeni exe: " + (Get-Item "$dir\evorift-svc.exe").LastWriteTime + " (" + (Get-Item "$dir\evorift-svc.exe").Length + " byte)")

# 3) eski token temizle + baslat
Remove-Item "$env:ProgramData\evorift\ipc.token" -ErrorAction SilentlyContinue
Remove-Item "$env:TEMP\evorift-ipc.token" -ErrorAction SilentlyContinue
sc.exe start EvoriftSvc *>&1 | Out-Null
Start-Sleep -Seconds 3

# 4) dogrula
L "--- sc query ---"; (sc.exe query EvoriftSvc) | Tee-Object -FilePath $log -Append | Out-Null
$tok = "$env:ProgramData\evorift\ipc.token"
if (Test-Path $tok) { L "TOKEN VAR ($((Get-Item $tok).Length) byte)" } else { L "TOKEN YOK" }
$pipe = [System.IO.Directory]::GetFiles('\\.\pipe\') | Where-Object {$_ -match 'evorift'}
L ("PIPE: " + ($(if($pipe){$pipe -join ','}else{'YOK'})))
L "=== bitti ==="
