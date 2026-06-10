# finish-deploy.ps1 — evorift'i TAM BITIR (net3 yontemi: winws + WARP split-tunnel).
#   FAZ 1  teardown  : eski+yeni servisleri TAMAMEN kapat (EvoriftSvc, stray winws, net3 'warp-discord'
#                      tuneli, evorift 'warp' tuneli, eski standalone zapret 'winws' servisi).
#   FAZ 2  arsivle   : eski build'leri (C:\Program Files\evorift\*.bak-*/*.broken*) arsive TASI.
#   FAZ 3  deploy    : bitmis birlesik build'i kur (yeni svc + UI + winws + warp bundle), temiz config.
#   FAZ 4  test      : servisi baslat -> auto-protect (winws + wgcf + WARP tunel) -> test-evorift.ps1.
#   Cikti: finish-deploy.log (ayrintili) + finish-deploy.result (RUNNING/PASS/FAIL). SELF-ELEVATING (UAC).
#   GERI ALMA: net3\warp-discord.conf duruyor -> gerekirse o tunel yeniden kurulabilir; eski exe'ler arsivde.
$ErrorActionPreference = 'Continue'
$base    = 'C:\Users\Evrim\Desktop\projects\net\src-tauri'
$dir     = 'C:\Program Files\evorift'
$ts      = Get-Date -Format 'yyyyMMdd-HHmmss'
$arch    = "C:\Users\Evrim\Desktop\projects\_archive\evorift-old-$ts"
$log     = "$base\examples\finish-deploy.log"
$res     = "$base\examples\finish-deploy.result"
$svcSrc  = "$base\target\release\evorift-svc.exe"
$appSrc  = "$base\target\release\evorift.exe"
$warpSrc = "$base\resources\warp"
$winwsSrc= "$base\resources\winws"

# --- self-elevation (UAC) ---
$pr = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $pr.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
  Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File',"`"$PSCommandPath`""
  exit
}

function L($m){ Add-Content -Path $log -Value ("$((Get-Date -Format 'HH:mm:ss')) $m") }
Set-Content $log "=== finish-deploy $(Get-Date -Format o) ==="; Set-Content $res 'RUNNING'

# guard: bitmis build var mi?
if (-not (Test-Path $svcSrc)) { L "HATA: yok $svcSrc (once: cargo build --release)"; Set-Content $res 'FAIL(no-build)'; exit 1 }
L ("Kaynak svc : " + (Get-Item $svcSrc).LastWriteTime + " (" + (Get-Item $svcSrc).Length + " byte)")
$wg = "$dir\warp\wireguard.exe"

# ===================== FAZ 1 — TEARDOWN =====================
L "--- FAZ 1: teardown (eski+yeni servisleri kapat) ---"
sc.exe stop EvoriftSvc *>&1 | Out-Null; Start-Sleep 2
taskkill /f /im winws.exe *>&1 | Out-Null; Start-Sleep 1
if (Test-Path $wg) {
  sc.exe stop 'WireGuardTunnel$warp-discord' *>&1 | Out-Null
  & $wg /uninstalltunnelservice warp-discord *>&1 | Out-Null     # net3 prototip tuneli
  & $wg /uninstalltunnelservice warp          *>&1 | Out-Null     # evorift kendi tuneli (varsa)
  Start-Sleep 2
}
L "Tuneller kaldirildi: warp-discord + warp (net3 conf net3 klasorunde duruyor -> geri alinabilir)."
sc.exe stop winws *>&1 | Out-Null
sc.exe config winws start= disabled *>&1 | Out-Null
L "Eski standalone zapret 'winws' servisi: stop + disabled."
$svc = Get-Service EvoriftSvc -EA SilentlyContinue; L ("  EvoriftSvc           : " + $(if($svc){$svc.Status}else{'YOK'}))
L ("  winws.exe            : " + @(Get-Process winws -EA SilentlyContinue).Count + " instance (0 beklenir)")
$tunOld = Get-Service 'WireGuardTunnel$warp-discord' -EA SilentlyContinue
L ("  warp-discord tunel   : " + $(if($tunOld){'HALA VAR ('+$tunOld.Status+')'}else{'YOK (iyi)'}))

# ===================== FAZ 2 — ARSIVLE =====================
L "--- FAZ 2: eski build'leri arsivle ---"
New-Item -ItemType Directory -Force -Path $arch | Out-Null
$moved = 0
Get-ChildItem $dir -File -EA SilentlyContinue | Where-Object { $_.Name -match '\.bak-|\.broken' } | ForEach-Object {
  Move-Item $_.FullName (Join-Path $arch $_.Name) -Force -EA SilentlyContinue; $moved++
}
L ("Arsivlenen eski build: $moved dosya -> $arch")

# ===================== FAZ 3 — DEPLOY =====================
L "--- FAZ 3: bitmis birlesik build'i deploy et ---"
New-Item -ItemType Directory -Force -Path $dir | Out-Null
Get-Process evorift -EA SilentlyContinue | Stop-Process -Force -EA SilentlyContinue
# mevcut svc/UI -> arsive yedekle, sonra yenisini koy
if (Test-Path "$dir\evorift-svc.exe") { Move-Item "$dir\evorift-svc.exe" (Join-Path $arch "evorift-svc.exe.prev-$ts") -Force -EA SilentlyContinue }
Copy-Item $svcSrc "$dir\evorift-svc.exe" -Force
Copy-Item $svcSrc "$base\resources\evorift-svc.exe" -Force         # installer bundle de guncel
if (Test-Path $appSrc) {
  if (Test-Path "$dir\evorift.exe") { Move-Item "$dir\evorift.exe" (Join-Path $arch "evorift.exe.prev-$ts") -Force -EA SilentlyContinue }
  Copy-Item $appSrc "$dir\evorift.exe" -Force
}
# winws bundle (exe + filtre + quic bin) ve warp bundle (wgcf/wireguard/wintun) senkronla
New-Item -ItemType Directory -Force -Path "$dir\winws" | Out-Null
if (Test-Path $winwsSrc) { Copy-Item "$winwsSrc\*" "$dir\winws\" -Recurse -Force }
New-Item -ItemType Directory -Force -Path "$dir\warp" | Out-Null
foreach ($f in 'wgcf.exe','wireguard.exe','wintun.dll') { if (Test-Path "$warpSrc\$f") { Copy-Item "$warpSrc\$f" "$dir\warp\$f" -Force } }
# temiz config: yeni split_tunnel (net3 IP araliklari + sabit loop-guvenli endpoint) uretilsin
Remove-Item "$env:ProgramData\evorift\warp.conf","$env:ProgramData\evorift\ipc.token" -EA SilentlyContinue
L ("Deploy edildi -> $dir : yeni svc " + (Get-Item "$dir\evorift-svc.exe").Length + " byte; winws/warp bundle senkron; warp.conf+token temizlendi.")

# servisi (yoksa) kur, etkinlestir, dayanikli yap, baslat
$exists = (sc.exe query EvoriftSvc) -match 'SERVICE_NAME'
if (-not $exists) {
  New-Service -Name 'EvoriftSvc' -BinaryPathName "`"$dir\evorift-svc.exe`"" -DisplayName 'evorift Koruma Servisi' -StartupType Automatic -Description 'evorift DPI-bypass + ag koruma motoru (LocalSystem).' -EA SilentlyContinue | Out-Null
  L "EvoriftSvc YENIDEN olusturuldu (yoktu)."
} else {
  sc.exe config EvoriftSvc binPath= "`"$dir\evorift-svc.exe`"" start= auto *>&1 | Out-Null
}
sc.exe failure EvoriftSvc reset= 86400 actions= restart/5000/restart/5000/restart/5000 *>&1 | Out-Null
sc.exe start EvoriftSvc *>&1 | Out-Null
L "EvoriftSvc start=auto + baslatildi. auto-protect bekleniyor (winws + wgcf register + WARP tunel ~28s)..."
Start-Sleep 28

# ===================== FAZ 4 — TEST =====================
L "--- FAZ 4: test ---"
$svc = Get-Service EvoriftSvc -EA SilentlyContinue;          L ("EvoriftSvc   : " + $(if($svc){$svc.Status}else{'YOK'}))
L ("winws.exe    : " + @(Get-Process winws -EA SilentlyContinue).Count + " instance")
$tun = Get-Service 'WireGuardTunnel$warp' -EA SilentlyContinue; L ("WARP tunnel  : " + $(if($tun){$tun.Status}else{'YOK (wgcf/ag yoksa winws web`i kapsar)'}))
if (Test-Path "$env:ProgramData\evorift\warp.conf") { L ("warp.conf    : VAR (" + (Get-Item "$env:ProgramData\evorift\warp.conf").Length + " byte)") } else { L "warp.conf    : YOK (wgcf register basarisiz olabilir)" }
$r = Get-NetRoute -DestinationPrefix '162.159.0.0/16' -EA SilentlyContinue
L ("162.159/16   : " + $(if($r){'ifIndex '+$r[0].ifIndex+' (Discord tunele gidiyor)'}else{'rota yok'}))
$crash = @(Get-WinEvent -FilterHashtable @{LogName='System';Id=7031;StartTime=(Get-Date).AddMinutes(-5)} -EA SilentlyContinue | Where-Object {$_.Message -match 'Evorift'})
L ("SCM 7031     : " + $crash.Count + " (0 beklenir)")
L "test-evorift.ps1 (TLS §9 kabul testi)..."
$t = & powershell -NoProfile -ExecutionPolicy Bypass -File "$base\examples\test-evorift.ps1" 2>&1
$t | Add-Content $log
$result = if ($t -match 'RESULT: PASS') { 'PASS' } else { 'FAIL' }
L "SONUC: $result"
Set-Content $res $result
L "=== bitti ($result) ==="
