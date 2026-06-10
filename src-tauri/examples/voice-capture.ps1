# voice-capture.ps1 — Discord SES sunucusunun gerçek UDP IP'sini bul (pktmon). SELF-ELEVATING.
# Akış: 8 sn hazırlık → 40 sn yakalama. Bu sürede SES kanalına gir + dene (No Route alsan da birak denesin).
$ErrorActionPreference='Continue'
$base='C:\Users\Evrim\Desktop\projects\net\src-tauri'
$log="$base\examples\voice-capture.log"
$done="$base\examples\voice-capture.done"
$etl="$env:TEMP\evorift-voice.etl"; $txt="$env:TEMP\evorift-voice.txt"
$pr=[Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if(-not $pr.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){
  Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File',"`"$PSCommandPath`""; exit }
# Add-Content = her satir aninda flush (Tee-Object bufferliyordu → satirlar kayboluyordu)
function L($m){ Add-Content -Path $log -Value ("$((Get-Date -Format 'HH:mm:ss')) $m") }
Set-Content $log "=== voice-capture $(Get-Date -Format o) ==="
Set-Content $done 'RUNNING'

if(-not (Get-Command pktmon -EA SilentlyContinue)){ L "HATA: pktmon yok"; Set-Content $done 'FAIL'; exit 1 }
$wgEp = (Resolve-DnsName engage.cloudflareclient.com -Type A -EA SilentlyContinue | Where-Object {$_.IPAddress} | Select-Object -First 1).IPAddress
L "WARP endpoint (elenecek): $wgEp"

pktmon stop *>&1 | Out-Null
pktmon reset *>&1 | Out-Null
Remove-Item $etl,$txt -EA SilentlyContinue
$st = pktmon start --capture --pkt-size 64 -f $etl 2>&1 | Out-String
L ("pktmon start -> " + ($st -replace '\s+',' ').Trim())
L "HAZIRLIK 8 sn — Discord ses kanalina GIRMEYE BASLA..."
Start-Sleep -Seconds 8
L "YAKALAMA 40 sn — SES kanalinda kal / denemeye devam et..."
Start-Sleep -Seconds 40
pktmon stop *>&1 | Out-Null
pktmon etl2txt $etl -o $txt *>&1 | Out-Null
L ("etl boyut: " + (Get-Item $etl -EA SilentlyContinue).Length + " byte; txt boyut: " + (Get-Item $txt -EA SilentlyContinue).Length + " byte")

if(-not (Test-Path $txt)){ L "HATA: etl2txt cikti yok"; Set-Content $done 'FAIL'; exit 1 }
$raw = Get-Content $txt -Raw
L "--- txt format ornegi (ilk 3 paket satiri) ---"
($raw -split "`n" | Where-Object {$_ -match '\d+\.\d+\.\d+\.\d+'} | Select-Object -First 3) | ForEach-Object { L ("  " + ($_ -replace '\s+',' ').Trim().Substring(0,[Math]::Min(160,($_ -replace '\s+',' ').Trim().Length))) }

$ips = [regex]::Matches($raw,'\b(\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3})\b') | ForEach-Object { $_.Groups[1].Value }
$pub = $ips | Where-Object {
  $_ -notmatch '^(0\.|127\.|10\.|192\.168\.|172\.(1[6-9]|2[0-9]|3[01])\.|169\.254\.|22[4-9]\.|23[0-9]\.|24[0-9]\.|25[0-5]\.)' -and
  $_ -ne '1.1.1.1' -and $_ -ne '1.0.0.1' -and $_ -ne $wgEp
}
$top = $pub | Group-Object | Sort-Object Count -Descending | Select-Object -First 20
L ("--- En cok gorulen PUBLIC IP (toplam public paket: " + @($pub).Count + ") ---")
foreach($g in $top){
  $ip=$g.Name
  $inTunnel = ($ip -like '162.159.*') -or ($ip -like '66.22.*')
  $tag = if($inTunnel){'TUNELDE'}else{'>>> RAW (AllowedIPs disinda)'}
  try{$h=[System.Net.Dns]::GetHostEntry($ip).HostName}catch{$h=''}
  L ("  {0,5}x  {1,-16} {2}  {3}" -f $g.Count, $ip, $tag, $h)
}
Set-Content $done 'DONE'
L "=== bitti ==="