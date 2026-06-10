# evorift — complete system cleanup script
# Run as admin: powershell -ExecutionPolicy Bypass -File clean-evorift.ps1

$ErrorActionPreference = 'SilentlyContinue'

Write-Host '>>> 1. Stopping EvoriftSvc service...' -ForegroundColor Yellow
Stop-Service EvoriftSvc -Force
sc.exe stop EvoriftSvc | Out-Null
sc.exe delete EvoriftSvc | Out-Null

Write-Host '>>> 2. Killing processes...' -ForegroundColor Yellow
taskkill /f /im evorift.exe | Out-Null
taskkill /f /im winws.exe | Out-Null
taskkill /f /im wireguard.exe | Out-Null
taskkill /f /im wgcf.exe | Out-Null

Write-Host '>>> 3. Removing WARP tunnel service...' -ForegroundColor Yellow
$wg = 'C:\Program Files\evorift\warp\wireguard.exe'
if (Test-Path $wg) {
    & $wg /uninstalltunnelservice warp | Out-Null
}
Stop-Service 'WireGuardTunnel$warp' -Force
sc.exe stop "WireGuardTunnel`$warp" | Out-Null
sc.exe delete "WireGuardTunnel`$warp" | Out-Null

Write-Host '>>> 4. Stopping & removing WinDivert kernel drivers...' -ForegroundColor Yellow
sc.exe stop WinDivert | Out-Null
sc.exe stop WinDivert1.4 | Out-Null
sc.exe delete WinDivert | Out-Null
sc.exe delete WinDivert1.4 | Out-Null
Start-Sleep -Seconds 3

Write-Host '>>> 5. Deleting install directory...' -ForegroundColor Yellow
Remove-Item 'C:\Program Files\evorift' -Recurse -Force

Write-Host '>>> 6. Removing startup shortcuts...' -ForegroundColor Yellow
Remove-Item 'C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Startup\evorift.lnk' -Force
$userStartup = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Startup\evorift.lnk'
Remove-Item $userStartup -Force

Write-Host '>>> 7. Removing Start Menu entries...' -ForegroundColor Yellow
Remove-Item 'C:\ProgramData\Microsoft\Windows\Start Menu\Programs\evorift.lnk' -Force
Remove-Item 'C:\ProgramData\Microsoft\Windows\Start Menu\Programs\evorift' -Recurse -Force

Write-Host '>>> 8. Cleaning registry...' -ForegroundColor Yellow
$regPaths = @(
    'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\evorift',
    'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\evorift',
    'HKLM:\SOFTWARE\evorift',
    'HKCU:\SOFTWARE\evorift',
    'HKCU:\SOFTWARE\com.evorift.app',
    'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\com.evorift.app'
)
foreach ($rp in $regPaths) {
    if (Test-Path $rp) {
        Remove-Item $rp -Recurse -Force
        Write-Host "  Removed: $rp" -ForegroundColor DarkGray
    }
}

Write-Host '>>> 9. Removing DNS/QoS/Firewall rules...' -ForegroundColor Yellow
Get-NetQosPolicy | Where-Object { $_.Name -like 'evorift-*' } | Remove-NetQosPolicy -Confirm:$false
Get-NetFirewallRule | Where-Object { $_.DisplayName -like 'evorift-*' } | Remove-NetFirewallRule

Write-Host '>>> 10. Removing AppData traces...' -ForegroundColor Yellow
$appDataPaths = @(
    (Join-Path $env:LOCALAPPDATA 'com.evorift.app'),
    (Join-Path $env:APPDATA 'com.evorift.app'),
    (Join-Path $env:LOCALAPPDATA 'evorift'),
    (Join-Path $env:APPDATA 'evorift')
)
foreach ($ap in $appDataPaths) {
    if (Test-Path $ap) {
        Remove-Item $ap -Recurse -Force
        Write-Host "  Removed: $ap" -ForegroundColor DarkGray
    }
}

Write-Host ''
Write-Host '========= VERIFICATION =========' -ForegroundColor Cyan
$checks = @(
    @{ Name='Install dir';     Check=(Test-Path 'C:\Program Files\evorift') },
    @{ Name='EvoriftSvc';      Check=[bool](Get-Service EvoriftSvc -ErrorAction SilentlyContinue) },
    @{ Name='WinDivert drv';   Check=[bool](Get-Service WinDivert -ErrorAction SilentlyContinue) },
    @{ Name='evorift process'; Check=[bool](Get-Process evorift -ErrorAction SilentlyContinue) },
    @{ Name='winws process';   Check=[bool](Get-Process winws -ErrorAction SilentlyContinue) },
    @{ Name='Startup lnk';     Check=(Test-Path 'C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Startup\evorift.lnk') }
)
$allClean = $true
foreach ($c in $checks) {
    if ($c.Check) {
        Write-Host "  STILL EXISTS: $($c.Name)" -ForegroundColor Red
        $allClean = $false
    } else {
        Write-Host "  CLEAN: $($c.Name)" -ForegroundColor Green
    }
}
Write-Host ''
if ($allClean) {
    Write-Host 'ALL EVORIFT TRACES REMOVED!' -ForegroundColor Green
} else {
    Write-Host 'Some traces remain — check above.' -ForegroundColor Red
}
