# One-off: is anything from evorift still running, and is the laptop's general internet up?
Write-Host "=== processes ==="
Get-Process -Name "evorift-svc","evorift","winws" -ErrorAction SilentlyContinue |
    Select-Object Id, ProcessName, StartTime | Format-Table -AutoSize | Out-String | Write-Host

Write-Host "=== services ==="
foreach ($svc in @("EvoriftSvc", "WinDivert", "WinDivert1.4", "WinDivert1.1", "WireGuardTunnel`$warp")) {
    $s = Get-Service -Name $svc -ErrorAction SilentlyContinue
    if ($s) { Write-Host "  $($s.Name): $($s.Status) (StartType via sc query below)" }
    else { Write-Host "  ${svc}: not present" }
}
foreach ($svc in @("EvoriftSvc")) {
    & sc.exe qc $svc 2>&1 | Write-Host
}

Write-Host "=== reachability ==="
try {
    $ping = (New-Object System.Net.NetworkInformation.Ping).Send("1.1.1.1", 3000)
    Write-Host "ping 1.1.1.1: $($ping.Status)"
} catch { Write-Host "ping 1.1.1.1: EXCEPTION $($_.Exception.Message)" }
try {
    $dns = Resolve-DnsName -Name "google.com" -Type A -ErrorAction Stop
    Write-Host "dns google.com resolves: true ($($dns[0].IPAddress))"
} catch { Write-Host "dns google.com resolves: false ($($_.Exception.Message))" }
exit 0
