# setup-warp.ps1 — Engine B (WireGuard WARP split-tunnel) binary'lerini indirir → src-tauri\resources\warp\.
#   wgcf.exe      (MIT, github.com/ViRb3/wgcf)            — gh release ile (github.com DPI'da bloklu → gh api/raw)
#   wireguard.exe (MIT, download.wireguard.com)           — resmi MSI'den msiexec /a ile çıkarılır
#   wintun.dll    (WireGuard LLC, wintun.net)             — resmi zip'ten
# Admin GEREKMEZ. İnternet gerekir. Çalıştır: powershell -ExecutionPolicy Bypass -File setup-warp.ps1
$ErrorActionPreference = 'Continue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$base = 'C:\Users\Evrim\Desktop\projects\net\src-tauri'
$warp = "$base\resources\warp"
$tmp  = "$env:TEMP\evorift-warp-setup"
New-Item -ItemType Directory -Force -Path $warp,$tmp | Out-Null
function Ok($f){ (Test-Path $f) -and ((Get-Item $f).Length -gt 50kb) }

# 1) wintun.dll (amd64) -----------------------------------------------------
if (Ok "$warp\wintun.dll") { Write-Output "wintun.dll  : zaten var" } else {
  try {
    Invoke-WebRequest 'https://www.wintun.net/builds/wintun-0.14.1.zip' -OutFile "$tmp\wintun.zip" -UseBasicParsing -TimeoutSec 60
    Expand-Archive "$tmp\wintun.zip" "$tmp\wintun" -Force
    Copy-Item "$tmp\wintun\wintun\bin\amd64\wintun.dll" "$warp\wintun.dll" -Force
    Write-Output ("wintun.dll  : " + $(if(Ok "$warp\wintun.dll"){'OK ' + (Get-Item "$warp\wintun.dll").Length + ' byte'}else{'FAIL'}))
  } catch { Write-Output "wintun.dll  : HATA $_" }
}

# 2) wireguard.exe (amd64) — resmi MSI'den çıkar ------------------------------
if (Ok "$warp\wireguard.exe") { Write-Output "wireguard.exe: zaten var" } else {
  try {
    $idx = (Invoke-WebRequest 'https://download.wireguard.com/windows-client/' -UseBasicParsing -TimeoutSec 60).Links.href |
           Where-Object { $_ -match 'wireguard-amd64-.*\.msi$' } | Sort-Object | Select-Object -Last 1
    Write-Output "wireguard MSI: $idx"
    Invoke-WebRequest "https://download.wireguard.com/windows-client/$idx" -OutFile "$tmp\wg.msi" -UseBasicParsing -TimeoutSec 120
    # administrative install (/a) = elevation'sız dosya çıkarma; /qn sessiz
    & msiexec.exe /a "$tmp\wg.msi" /qn TARGETDIR="$tmp\wg" | Out-Null
    Start-Sleep -Seconds 2
    $exe = Get-ChildItem "$tmp\wg" -Recurse -Filter wireguard.exe -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($exe) { Copy-Item $exe.FullName "$warp\wireguard.exe" -Force }
    # MSI içinde wintun.dll de olabilir → varsa al (yedek)
    $wt = Get-ChildItem "$tmp\wg" -Recurse -Filter wintun.dll -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($wt -and -not (Ok "$warp\wintun.dll")) { Copy-Item $wt.FullName "$warp\wintun.dll" -Force }
    Write-Output ("wireguard.exe: " + $(if(Ok "$warp\wireguard.exe"){'OK ' + (Get-Item "$warp\wireguard.exe").Length + ' byte'}else{'FAIL'}))
  } catch { Write-Output "wireguard.exe: HATA $_" }
}

# 3) wgcf.exe — gh release (github.com bloklu olsa da gh api/raw çalışır) ------
if (Ok "$warp\wgcf.exe") { Write-Output "wgcf.exe    : zaten var" } else {
  try {
    Remove-Item "$tmp\wgcf*" -ErrorAction SilentlyContinue
    & gh release download --repo ViRb3/wgcf --pattern '*windows_amd64.exe' --dir $tmp --clobber 2>&1 | Out-Null
    $w = Get-ChildItem $tmp -Filter '*windows_amd64.exe' -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($w) { Copy-Item $w.FullName "$warp\wgcf.exe" -Force }
    Write-Output ("wgcf.exe    : " + $(if(Ok "$warp\wgcf.exe"){'OK ' + (Get-Item "$warp\wgcf.exe").Length + ' byte'}else{'FAIL'}))
  } catch { Write-Output "wgcf.exe    : HATA $_" }
}

Write-Output "`n=== resources\warp\ ==="
Get-ChildItem $warp -File | Select-Object Name,Length | Format-Table -AutoSize
$have = @('wgcf.exe','wireguard.exe','wintun.dll') | Where-Object { Ok "$warp\$_" }
Write-Output ("Hazir: " + ($have -join ', ') + "  (" + $have.Count + "/3)")