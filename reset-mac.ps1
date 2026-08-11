# ============================================================
#  MAC sifirlama - Ethernet adaptorune yeni (rastgele) MAC ver
#  Amac: Superbox/ISS'in cihaza uyguladigi throttle'i SIFIRLAMAK.
#  NOT: Bu kalici cozum DEGIL, throttle'i resetler (reboot gibi).
#       Asil cozum: discord-fix-gentle.bat (sessiz strateji).
#  YONETICI PowerShell'de calistir.
# ============================================================

$adapter = "Ethernet"   # aktif adaptor (Intel I226-V)

# yonetici kontrolu
if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
  Write-Host "[HATA] Yonetici PowerShell'de calistir." -ForegroundColor Red
  exit 1
}

# locally-administered + unicast MAC uret (ilk oktet 0x02)
$bytes = @(0x02) + (1..5 | ForEach-Object { Get-Random -Minimum 0 -Maximum 256 })
$mac = ($bytes | ForEach-Object { '{0:X2}' -f $_ }) -join ''
Write-Host "[*] Yeni MAC: $mac" -ForegroundColor Cyan

# eski degeri goster
$old = (Get-NetAdapter -Name $adapter).MacAddress
Write-Host "[*] Eski MAC: $old"

# uygula
Set-NetAdapterAdvancedProperty -Name $adapter -RegistryKeyword "NetworkAddress" -RegistryValue $mac -ErrorAction Stop
Write-Host "[*] Adaptor yeniden baslatiliyor..."
Restart-NetAdapter -Name $adapter -Confirm:$false
Start-Sleep -Seconds 5

# DHCP yenile
ipconfig /release | Out-Null
ipconfig /renew   | Out-Null
ipconfig /flushdns | Out-Null

$new = (Get-NetAdapter -Name $adapter).MacAddress
Write-Host "[OK] Yeni aktif MAC: $new" -ForegroundColor Green
Write-Host ""
Write-Host "Geri almak icin (orijinal MAC'e don):" -ForegroundColor Yellow
Write-Host "  Remove-NetAdapterAdvancedProperty -Name '$adapter' -RegistryKeyword 'NetworkAddress'; Restart-NetAdapter -Name '$adapter' -Confirm:`$false"
