# diag.ps1 — neden EvoriftSvc ayakta kalmıyor + Motor B konsol modunda çalışıyor mu (SELF-ELEVATING).
$ErrorActionPreference='Continue'
$base='C:\Users\Evrim\Desktop\projects\net\src-tauri'
$dir='C:\Program Files\evorift'
$out="$base\examples\diag.log"
$pr=[Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if(-not $pr.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){
  Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File',"`"$PSCommandPath`""; exit }
function L($m){ "$((Get-Date -Format 'HH:mm:ss')) $m" | Tee-Object -FilePath $out -Append | Out-Null }
"=== diag $(Get-Date -Format o) ===" | Set-Content $out

L "--- warp\ icerik (Program Files) ---"
Get-ChildItem "$dir\warp" -File -ErrorAction SilentlyContinue | ForEach-Object { L ("  " + $_.Name + "  " + $_.Length + " byte") }
L ("evorift-svc.exe: " + (Get-Item "$dir\evorift-svc.exe" -ErrorAction SilentlyContinue).Length + " byte, " + (Get-Item "$dir\evorift-svc.exe" -ErrorAction SilentlyContinue).LastWriteTime)

L "--- servisi durdur + temiz baslat ---"
sc.exe stop EvoriftSvc *>&1 | Out-Null; Start-Sleep 2; taskkill /f /im winws.exe *>&1 | Out-Null
Remove-Item "$env:ProgramData\evorift\warp.conf","$env:ProgramData\evorift\ipc.token" -ErrorAction SilentlyContinue
$s = sc.exe start EvoriftSvc 2>&1 | Out-String; L ("sc start ->`n" + $s.Trim())
Start-Sleep 6
$svc=Get-Service EvoriftSvc -ErrorAction SilentlyContinue; L ("durum: " + $(if($svc){$svc.Status}else{'YOK'}))

L "--- Event log (son 20dk, evorift/SCM) ---"
Get-WinEvent -FilterHashtable @{LogName='System';StartTime=(Get-Date).AddMinutes(-20)} -ErrorAction SilentlyContinue |
  Where-Object {$_.Message -match 'vorift'} | Select-Object -First 10 | ForEach-Object {
    $m=($_.Message -replace '\s+',' '); L ("  [" + $_.Id + "] " + $m.Substring(0,[Math]::Min(200,$m.Length))) }
Get-WinEvent -FilterHashtable @{LogName='Application';StartTime=(Get-Date).AddMinutes(-20);Level=2} -ErrorAction SilentlyContinue |
  Where-Object {$_.Message -match 'vorift'} | Select-Object -First 5 | ForEach-Object {
    $m=($_.Message -replace '\s+',' '); L ("  APP-ERR " + $m.Substring(0,[Math]::Min(200,$m.Length))) }

L "--- console mode 40s (audit stderr) ---"
sc.exe stop EvoriftSvc *>&1 | Out-Null; Start-Sleep 2; taskkill /f /im winws.exe *>&1 | Out-Null
Remove-Item "$env:ProgramData\evorift\warp.conf" -ErrorAction SilentlyContinue
Remove-Item "$base\examples\svc-console.err","$base\examples\svc-console.out" -ErrorAction SilentlyContinue
$p = Start-Process "$dir\evorift-svc.exe" -ArgumentList '--console' -RedirectStandardError "$base\examples\svc-console.err" -RedirectStandardOutput "$base\examples\svc-console.out" -PassThru -WindowStyle Hidden
Start-Sleep 40
L ("winws (console): " + @(Get-Process winws -ErrorAction SilentlyContinue).Count)
$tun=Get-Service 'WireGuardTunnel$warp' -ErrorAction SilentlyContinue; L ("WARP tunel (console): " + $(if($tun){$tun.Status}else{'YOK'}))
if(Test-Path "$env:ProgramData\evorift\warp.conf"){ L ("warp.conf: VAR " + (Get-Item "$env:ProgramData\evorift\warp.conf").Length + " byte") } else { L "warp.conf: YOK" }
Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue; Start-Sleep 1
taskkill /f /im winws.exe *>&1 | Out-Null
& "$dir\warp\wireguard.exe" /uninstalltunnelservice warp *>&1 | Out-Null
L "--- console STDERR (audit) ---"
if(Test-Path "$base\examples\svc-console.err"){ Get-Content "$base\examples\svc-console.err" | ForEach-Object { L ("  " + $_) } } else { L "  (stderr yok)" }
L "=== diag bitti ==="