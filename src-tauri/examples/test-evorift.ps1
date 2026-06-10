# test-evorift.ps1 — evorift §9 doğrulama harness'i. Admin GEREKMEZ (salt-okuma + TLS el-sıkışma).
#   - EvoriftSvc servis durumu, winws.exe, Motor B WARP tüneli (WireGuardTunnel$warp)
#   - Fiziksel adaptör DNS'leri + discord.com çözümlemesi (162.159.* = zehirsiz)
#   - 6 çekirdek URL'ye TLS el-sıkışma ("ping"): discord.com, gateway.discord.gg, cdn.discordapp.com,
#     discord.media, roblox.com, youtube.com
# Çıkış: net PASS/FAIL + exit kodu 0/1. Çalıştır: powershell -ExecutionPolicy Bypass -File test-evorift.ps1
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$CORE = 'discord.com','gateway.discord.gg','cdn.discordapp.com','discord.media','roblox.com','youtube.com'

function TlsPing([string]$h, [int]$timeoutMs = 5000) {
  $tcp = New-Object Net.Sockets.TcpClient
  try {
    $iar = $tcp.BeginConnect($h, 443, $null, $null)
    if (-not $iar.AsyncWaitHandle.WaitOne($timeoutMs, $false)) { return $false }
    $tcp.EndConnect($iar)
    $ssl = New-Object Net.Security.SslStream($tcp.GetStream(), $false, ([Net.Security.RemoteCertificateValidationCallback]{ param($a,$b,$c,$d) $true }))
    $ssl.AuthenticateAsClient($h)   # SNI TLS handshake — §9 kabul testi
    $ok = $ssl.IsAuthenticated
    $ssl.Dispose()
    return $ok
  } catch { return $false } finally { $tcp.Close() }
}

Write-Output "================ evorift test ($(Get-Date -Format 'HH:mm:ss')) ================"

# 1) Servis ----------------------------------------------------------------
$svc = Get-Service EvoriftSvc -ErrorAction SilentlyContinue
Write-Output ("[svc ] EvoriftSvc        : " + $(if($svc){$svc.Status}else{'KURULU DEGIL'}))

# 2) winws (Motor A) -------------------------------------------------------
$winws = @(Get-Process winws -ErrorAction SilentlyContinue)
Write-Output ("[A   ] winws.exe         : " + $winws.Count + " instance")

# 3) Motor B — WARP tüneli -------------------------------------------------
$tun = Get-Service 'WireGuardTunnel$warp' -ErrorAction SilentlyContinue
Write-Output ("[B   ] WARP tunnel        : " + $(if($tun){$tun.Status}else{'YOK (binary/ag yoksa winws web`i kapsar)'}))

# 4) DNS -------------------------------------------------------------------
$dns = @(Get-DnsClientServerAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue |
         Where-Object {$_.ServerAddresses} | Select-Object -ExpandProperty ServerAddresses -Unique)
$cf  = ($dns -contains '1.1.1.1') -or ($dns -contains '1.0.0.1')
Write-Output ("[dns ] sunucular         : " + ($dns -join ', ') + "  -> Cloudflare: $cf")
try {
  $disc = (Resolve-DnsName discord.com -Type A -ErrorAction Stop | Where-Object {$_.IPAddress} | Select-Object -First 1).IPAddress
  Write-Output ("[dns ] discord.com       : $disc  -> 162.159.* (zehirsiz): " + ($disc -like '162.159.*'))
} catch { Write-Output "[dns ] discord.com       : cozulemedi" }

# 5) TLS el-sıkışma ("ping") -----------------------------------------------
Write-Output "[ping] TLS el-sikisma (SNI handshake):"
$pass = 0
foreach ($h in $CORE) {
  $ok = TlsPing $h
  if ($ok) { $pass++ }
  Write-Output ("        {0,-22} {1}" -f $h, $(if($ok){'OK'}else{'FAIL'}))
}
$total = $CORE.Count
Write-Output "------------------------------------------------------------"
Write-Output ("TLS: $pass/$total basarili")

# Sonuç: çekirdek Discord açıldıysa + en az 5/6 → PASS ("ping alindi")
$discordOk = (TlsPing 'discord.com') -and (TlsPing 'gateway.discord.gg')
$result = if (($pass -ge 5) -and $discordOk) { 'PASS' } else { 'FAIL' }
Write-Output "RESULT: $result"
if ($result -eq 'PASS') { exit 0 } else { exit 1 }
