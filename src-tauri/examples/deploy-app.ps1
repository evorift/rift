# Yeni app exe'sini Program Files'a kur (ELEVATED calistirilir). Calisan app'i kapatir, kopyalar, loglar.
$ErrorActionPreference = 'Stop'
$src = 'C:\Users\Evrim\Desktop\projects\net\src-tauri\target\release\evorift.exe'
$dst = 'C:\Program Files\evorift\evorift.exe'
$log = 'C:\Users\Evrim\Desktop\projects\net\src-tauri\examples\deploy-app.log'
try {
  Get-Process evorift -ErrorAction SilentlyContinue | Stop-Process -Force
  Start-Sleep -Seconds 2
  Copy-Item $src $dst -Force
  $i = Get-Item $dst
  ("OK: {0} ({1} byte)" -f $i.LastWriteTime, $i.Length) | Set-Content $log
} catch {
  ("HATA: " + $_.Exception.Message) | Set-Content $log
}
