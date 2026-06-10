# solve-voice.ps1 — port-BAGIMSIZ ses filtresini uygula + winws yenile + CANLI capture ile dogrula.
# Pencerede "SES KANALINA GIR" yazinca Discord'da ses kanalina gir. Sonunda net VERDICT gosterir.
$ErrorActionPreference='Continue'
$base='C:\Users\Evrim\Desktop\projects\net\src-tauri'
$dir='C:\Program Files\evorift'
$log="$base\examples\solve-voice.log"; $done="$base\examples\solve-voice.done"
$src="$base\resources\winws\windivert.filter\windivert_part.discord_media_wide.txt"
$dst="$dir\winws\windivert.filter\windivert_part.discord_media_wide.txt"
$etl="$env:TEMP\evorift-solve.etl"; $txt="$env:TEMP\evorift-solve.txt"
$pr=[Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if(-not $pr.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){
  Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File',"`"$PSCommandPath`""; exit }
function L($m){ $l="$((Get-Date -Format 'HH:mm:ss')) $m"; Add-Content -Path $log -Value $l; Write-Host $l }
Set-Content $log "=== solve-voice $(Get-Date -Format o) ==="; Set-Content $done 'RUNNING'

Write-Host "`n==================== evorift SES COZUCU v2 ====================" -ForegroundColor Cyan
# 1) port-bagimsiz filtreyi uygula + servisi yenile
Copy-Item $src $dst -Force
L "Filtre uygulandi (imza-tabanli, port-BAGIMSIZ → her ses portu yakalanir)"
L "Servis yeniden baslatiliyor..."
sc.exe stop EvoriftSvc *>&1 | Out-Null; Start-Sleep 2
taskkill /f /im winws.exe *>&1 | Out-Null; Start-Sleep 1
sc.exe start EvoriftSvc *>&1 | Out-Null; Start-Sleep 12
L ("winws: " + @(Get-Process winws -EA SilentlyContinue).Count + " instance")

# 2) capture
pktmon stop *>&1 | Out-Null; pktmon reset *>&1 | Out-Null
Remove-Item $etl,$txt -EA SilentlyContinue
pktmon start --capture --pkt-size 64 -f $etl *>&1 | Out-Null
Write-Host "`n>>>>>>>>>>  SIMDI DISCORD'DA SES KANALINA GIR !!  <<<<<<<<<<" -ForegroundColor Yellow
Write-Host ">>>>>>>>>>  (No Route alsan da kanalda KAL, cik-gir yap)  <<<<<<<<<<`n" -ForegroundColor Yellow
for($i=45;$i -gt 0;$i-=5){ Write-Host ("   capture: $i sn (ses kanalinda kal)") -ForegroundColor DarkGray; Start-Sleep 5 }
pktmon stop *>&1 | Out-Null
pktmon etl2txt $etl -o $txt *>&1 | Out-Null

# 3) analiz: 'sIP.sPort > dIP.dPort: UDP' → public IP basina OUT/IN + voice IP portlari
$raw = Get-Content $txt -Raw -EA SilentlyContinue
$out=@{}; $inn=@{}; $ports=@{}
$priv='^(0\.|127\.|10\.|192\.168\.|172\.(1[6-9]|2[0-9]|3[01])\.|169\.254\.|22[4-9]\.|23[0-9]\.|24[0-9]\.|25[0-5]\.)'
foreach($m in [regex]::Matches($raw,'(\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3})\.(\d+) > (\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3})\.(\d+): UDP')){
  $d=$m.Groups[3].Value; $dp=$m.Groups[4].Value; $s=$m.Groups[1].Value
  if($d -notmatch $priv -and $d -ne '1.1.1.1' -and $d -ne '1.0.0.1'){ $out[$d]=[int]$out[$d]+1; if(-not $ports[$d]){$ports[$d]=@{}}; $ports[$d][$dp]=$true }
  if($s -notmatch $priv -and $s -ne '1.1.1.1' -and $s -ne '1.0.0.1'){ $inn[$s]=[int]$inn[$s]+1 }
}
$voice = ($out.GetEnumerator() | Sort-Object Value -Descending | Select-Object -First 1)
$vip=$voice.Name; $vout=[int]$voice.Value; $vin=[int]$inn[$vip]
$vports = if($vip){ ($ports[$vip].Keys | Sort-Object {[int]$_}) -join ',' } else { '' }
L ("Ses sunucusu: $vip  port(lar): $vports  GIDEN=$vout  GELEN(cevap)=$vin")
foreach($k in ($out.Keys + $inn.Keys | Sort-Object -Unique)){ L ("   {0,-16} OUT={1,-6} IN={2}" -f $k,([int]$out[$k]),([int]$inn[$k])) }

Write-Host ""
if($vin -ge 10){ Write-Host "  ✓ SES CALISIYOR (sunucu cevap veriyor, IN=$vin port=$vports)" -ForegroundColor Green; L "VERDICT: WORKS port=$vports IN=$vin" }
elseif($vout -gt 0){ Write-Host "  ✗ Ses sunucusu ($vip`:$vports) CEVAP VERMIYOR (IN=$vin)." -ForegroundColor Red; Write-Host "    Filtre artik HER portu kapsiyor → bu PORT sorunu DEGIL, ROUTING/region sorunu." -ForegroundColor Red; Write-Host "    COZUM: Discord'da Ses Bolgesi'ni (Voice Region) elle degistir (Almanya/Hollanda)." -ForegroundColor Yellow; L "VERDICT: ROUTING (port=$vports IN=$vin) → region override gerek" }
else { Write-Host "  ? Veri yok — ses kanalina girilmemis olabilir (OUT=0). Tekrar dene." -ForegroundColor Yellow; L "VERDICT: NO-DATA" }
Set-Content $done 'DONE'
Write-Host "`nLog: $log" -ForegroundColor DarkGray
Read-Host "`nBitti. Kapatmak icin ENTER'a bas"