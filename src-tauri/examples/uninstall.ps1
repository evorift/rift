# uninstall.ps1 — evorift'i makineden TAMAMEN kaldir (temiz kurulum testi icin). SELF-ELEVATING (UAC).
#   NSIS uninstaller (windows/hooks.nsh PREUNINSTALL) ile birebir + install dizini + ProgramData + autostart.
#   Cikti: uninstall.log + uninstall.result (RUNNING/CLEAN/PARTIAL).
$ErrorActionPreference = 'Continue'
$base = 'C:\Users\Evrim\Desktop\projects\net\src-tauri'
$dir  = 'C:\Program Files\evorift'
$pd   = "$env:ProgramData\evorift"
$log  = "$base\examples\uninstall.log"
$res  = "$base\examples\uninstall.result"
$wg   = "$dir\warp\wireguard.exe"

$pr = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $pr.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
  Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File',"`"$PSCommandPath`""
  exit
}
function L($m){ Add-Content -Path $log -Value ("$((Get-Date -Format 'HH:mm:ss')) $m") }
Set-Content $log "=== uninstall $(Get-Date -Format o) ==="; Set-Content $res 'RUNNING'

# 1) servis durdur + sil
sc.exe stop EvoriftSvc *>&1 | Out-Null; Start-Sleep 2
sc.exe delete EvoriftSvc *>&1 | Out-Null
L "EvoriftSvc: stop + delete."

# 2) WARP tunelini kaldir (install dizinini silmeden ONCE wireguard.exe lazim)
if (Test-Path $wg) { & $wg /uninstalltunnelservice warp *>&1 | Out-Null; Start-Sleep 2 }
L "WARP tuneli (WireGuardTunnel`$warp) kaldirildi."

# 3) winws oldur
taskkill /f /im winws.exe *>&1 | Out-Null

# 4) sistem degisikliklerini geri al (hooks.nsh ile ayni) — DNS otomatige + flush
Get-NetAdapter -Physical | ForEach-Object { Set-DnsClientServerAddress -InterfaceIndex $_.InterfaceIndex -ResetServerAddresses -Confirm:$false }
Clear-DnsClientCache
L "DNS otomatige donduruldu + cache temizlendi."

# 5) per-app QoS + firewall kurallari temizle
Get-NetQosPolicy -ErrorAction SilentlyContinue | Where-Object { $_.Name -like 'evorift-*' } | Remove-NetQosPolicy -Confirm:$false
Get-NetFirewallRule -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -like 'evorift-block-*' } | Remove-NetFirewallRule
L "QoS + firewall (evorift-*) kurallari temizlendi."

# 6) autostart (HKCU Run) kaldir
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v evorift /f *>&1 | Out-Null
L "Autostart (HKCU Run\evorift) kaldirildi."

# 7) dizinleri sil (install + ProgramData = tam temiz; installer testi gercek sifirdan baslar)
Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item $pd  -Recurse -Force -ErrorAction SilentlyContinue
L ("Program Files\evorift silindi : " + (-not (Test-Path $dir)))
L ("ProgramData\evorift silindi   : " + (-not (Test-Path $pd)))

# 8) dogrula
$svc = Get-Service EvoriftSvc -ErrorAction SilentlyContinue
$tun = Get-Service 'WireGuardTunnel$warp' -ErrorAction SilentlyContinue
$winws = @(Get-Process winws -ErrorAction SilentlyContinue).Count
L ("EvoriftSvc    : " + $(if($svc){'HALA VAR ('+$svc.Status+')'}else{'YOK (silindi)'}))
L ("WARP tunel    : " + $(if($tun){'HALA VAR ('+$tun.Status+')'}else{'YOK'}))
L ("winws.exe     : $winws (0 olmali)")
L ("Program Files : " + $(if(Test-Path $dir){'HALA VAR'}else{'silindi'}))
L ("ProgramData   : " + $(if(Test-Path $pd){'HALA VAR'}else{'silindi'}))
$ok = (-not $svc) -and (-not $tun) -and ($winws -eq 0) -and (-not (Test-Path $dir)) -and (-not (Test-Path $pd))
Set-Content $res $(if($ok){'CLEAN'}else{'PARTIAL'})
L ("=== bitti (" + $(if($ok){'CLEAN'}else{'PARTIAL'}) + ") ===")
