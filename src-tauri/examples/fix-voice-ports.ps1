# fix-voice-ports.ps1 — Discord SES port araligi duzeltmesi: winws IP-discovery filtresini 19294'ten
# baslat (eski floor 50000 → 19340'taki ses sunucusu kacirilıyordu). Filtre RUNTIME kaynak → rebuild YOK,
# sadece kopyala + servisi yeniden baslat (winws filtreyi yeniden okur). SELF-ELEVATING.
$ErrorActionPreference='Continue'
$base='C:\Users\Evrim\Desktop\projects\net\src-tauri'
$dir='C:\Program Files\evorift'
$log="$base\examples\fix-voice-ports.log"; $done="$base\examples\fix-voice-ports.done"
$srcFilter="$base\resources\winws\windivert.filter\windivert_part.discord_media_wide.txt"
$dstFilter="$dir\winws\windivert.filter\windivert_part.discord_media_wide.txt"
$pr=[Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if(-not $pr.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){
  Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File',"`"$PSCommandPath`""; exit }
function L($m){ Add-Content -Path $log -Value ("$((Get-Date -Format 'HH:mm:ss')) $m") }
Set-Content $log "=== fix-voice-ports $(Get-Date -Format o) ==="; Set-Content $done 'RUNNING'

if(-not (Test-Path $srcFilter)){ L "HATA: kaynak filtre yok"; Set-Content $done 'FAIL'; exit 1 }
# 1) guncellenmis filtreyi deploy et
Copy-Item $srcFilter $dstFilter -Force
$line = (Get-Content $dstFilter | Where-Object {$_ -match 'DstPort'})
L ("deploy edilen filtre port satiri: " + $line.Trim())
# 2) servisi yeniden baslat -> winws filtreyi yeniden okur
sc.exe stop EvoriftSvc *>&1 | Out-Null; Start-Sleep 2
taskkill /f /im winws.exe *>&1 | Out-Null; Start-Sleep 1
sc.exe start EvoriftSvc *>&1 | Out-Null
L "winws yeniden baslatiliyor (yeni filtre)..."; Start-Sleep 12
# 3) dogrula: filtre hatasi olsaydi winws baslamazdi → 1 instance = filtre parse OK
$svc=Get-Service EvoriftSvc -EA SilentlyContinue; L ("Servis    : " + $(if($svc){$svc.Status}else{'YOK'}))
$w=@(Get-Process winws -EA SilentlyContinue).Count
L ("winws.exe : $w instance " + $(if($w -ge 1){'(filtre parse OK)'}else{'(BASLAMADI — filtre hatali olabilir!)'}))
$tun=Get-Service 'WireGuardTunnel$warp' -EA SilentlyContinue; L ("WARP tunel: " + $(if($tun){'VAR (olmamali)'}else{'YOK (saf winws, dogru)'}))
Set-Content $done 'DONE'; L "=== bitti — Discord'da SES kanalina gir ve dene ==="