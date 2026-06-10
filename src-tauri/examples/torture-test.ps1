# Motor stop/start tork testi: guvenli motor cokmeden + internet kesilmeden defalarca acilip kapanmali.
$ctl = "C:\Users\Evrim\Desktop\projects\net\src-tauri\target\debug\examples\svc_ctl.exe"
[Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12
function Ok($u){ try{ $r=Invoke-WebRequest $u -UseBasicParsing -TimeoutSec 8; ($r.StatusCode -ge 200 -and $r.StatusCode -lt 500) }catch{ $false } }
function SvcAlive(){ [bool]((sc.exe query EvoriftSvc | Select-String "RUNNING")) }

Write-Output "TORK TESTI: 6 tur start/stop, her turda internet+discord+servis kontrol"
$fail = 0
for($i=1; $i -le 6; $i++){
  & $ctl start | Out-Null
  Start-Sleep -Milliseconds 1500
  $netOn = Ok "https://www.microsoft.com"
  $disc  = Ok "https://discord.com/api/v9/gateway"
  $alive = SvcAlive
  if(-not $netOn){ $fail++ }
  if(-not $alive){ $fail++ }
  "tur $i START internet={0} discord={1} servis={2}" -f $netOn,$disc,$alive

  & $ctl stop | Out-Null
  Start-Sleep -Milliseconds 1200
  $netOff = Ok "https://www.microsoft.com"
  $alive2 = SvcAlive
  if(-not $netOff){ $fail++ }
  if(-not $alive2){ $fail++ }
  "tur $i STOP  internet={0} servis={1}" -f $netOff,$alive2
}
Write-Output ""
if($fail -eq 0){ Write-Output "SONUC: GECTI - internet hic kesilmedi, servis hic cokmedi" }
else { Write-Output "SONUC: $fail BASARISIZLIK - internet kesildi veya servis coktu" }
& $ctl start | Out-Null
& $ctl status