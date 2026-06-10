# Pipe çakışmasını çöz: çakışan evorift APP sürecini durdur (gömülü sunucu pipe'ı tutuyor),
# EvoriftSvc'yi yeniden başlat → servis pipe'ı kapar + token'ı %PROGRAMDATA%\evorift'e yazar. YÖNETİCİ.
$ErrorActionPreference = 'Continue'
$log = "C:\Users\Evrim\Desktop\projects\net\src-tauri\examples\fix-svc.log"
function L($m){ $m | Tee-Object -FilePath $log -Append }
"=== fix-svc $(Get-Date -Format o) ===" | Set-Content $log

# 1) servisi durdur (pipe'ı bıraksın)
sc.exe stop EvoriftSvc *>&1 | Out-Null
Start-Sleep -Seconds 2

# 2) çakışan APP süreçlerini durdur (evorift.exe — gömülü sunuculu; evorift-svc'ye DOKUNMA)
Get-Process evorift -ErrorAction SilentlyContinue | ForEach-Object {
  L "App süreci durduruluyor: PID $($_.Id)"
  Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
}
# eski adlı rift.exe varsa o da pipe tutabilir
Get-Process rift -ErrorAction SilentlyContinue | ForEach-Object {
  L "Eski rift süreci durduruluyor: PID $($_.Id)"
  Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
}
Start-Sleep -Seconds 1

# 3) eski %TEMP% token'ını temizle (kafa karışıklığı olmasın)
Remove-Item "$env:TEMP\evorift-ipc.token" -ErrorAction SilentlyContinue

# 4) servisi yeniden başlat → serve_blocking pipe'ı kapar + token yazar
sc.exe start EvoriftSvc *>&1 | Out-Null
Start-Sleep -Seconds 3

# 5) doğrula
L "--- sc query ---"; (sc.exe query EvoriftSvc) | Tee-Object -FilePath $log -Append | Out-Null
$tok = "$env:ProgramData\evorift\ipc.token"
if (Test-Path $tok) { L "TOKEN VAR: $tok ($((Get-Item $tok).Length) byte)" } else { L "TOKEN HALA YOK" }
$pipe = [System.IO.Directory]::GetFiles('\\.\pipe\') | Where-Object {$_ -match 'evorift'}
L ("PIPE: " + ($(if($pipe){$pipe -join ','}else{'YOK'})))
L "=== bitti ==="
