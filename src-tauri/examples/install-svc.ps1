# EvoriftSvc'yi bu makinede kur (installer'ın yapacağının aynısı). YÖNETİCİ gerekir.
# Program Files'a evorift-svc.exe + WinDivert.dll + WinDivert64.sys kopyalar, LocalSystem servisi
# olarak start=auto kurar, başlatır. Sonuç %TEMP%\evorift-install.log'a yazılır.
$ErrorActionPreference = 'Continue'
$log = "C:\Users\Evrim\Desktop\projects\net\src-tauri\examples\install-svc.log"
function L($m){ $m | Tee-Object -FilePath $log -Append }
"=== evorift servis kurulumu $(Get-Date -Format o) ===" | Set-Content $log

$dir = "C:\Program Files\evorift"
$base = "C:\Users\Evrim\Desktop\projects\net\src-tauri"
New-Item -ItemType Directory -Force -Path $dir | Out-Null
Copy-Item "$base\target\release\evorift-svc.exe" $dir -Force
Copy-Item "$base\resources\WinDivert.dll"        $dir -Force
Copy-Item "$base\resources\WinDivert64.sys"      $dir -Force
L "Kopyalandi -> $dir :"
L ((Get-ChildItem $dir | Select-Object Name,Length | Out-String).Trim())

# eski servisi temizle (idempotent)
sc.exe stop EvoriftSvc   *>&1 | Out-Null
sc.exe delete EvoriftSvc *>&1 | Out-Null
Start-Sleep -Milliseconds 500

# LocalSystem servisi, boot'ta otomatik. -BinaryPathName tirnakli (bosluklu yol guvenligi).
try {
  New-Service -Name "EvoriftSvc" -BinaryPathName "`"$dir\evorift-svc.exe`"" `
    -DisplayName "evorift Koruma Servisi" -StartupType Automatic `
    -Description "evorift DPI-bypass + ag koruma motoru (LocalSystem)." -ErrorAction Stop | Out-Null
  L "New-Service OK"
} catch { L "New-Service HATA: $($_.Exception.Message)" }

sc.exe start EvoriftSvc *>&1 | Tee-Object -FilePath $log -Append | Out-Null
Start-Sleep -Seconds 3
L "--- sc query ---"
(sc.exe query EvoriftSvc) | Tee-Object -FilePath $log -Append | Out-Null
L "--- sc qc (binPath) ---"
(sc.exe qc EvoriftSvc) | Tee-Object -FilePath $log -Append | Out-Null
L "--- token dosyasi ---"
$tok = "$env:ProgramData\evorift\ipc.token"
if (Test-Path $tok) { L "TOKEN VAR: $tok ($((Get-Item $tok).Length) byte)" } else { L "TOKEN YOK: $tok" }
L "=== bitti ==="
