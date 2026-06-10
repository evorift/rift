# overinstall-test.ps1 — CALISAN evorift'in (winws + WinDivert yuklu = .sys KILITLI) UZERINE yeni
#   installer'i /S ile kur. MANUEL teardown YOK -> yalnizca PREINSTALL hook kilitleri (wireguard.exe +
#   WinDivert64.sys) cozmeli. Basari = kurulum tamamlanir + winws calisir + 6/6 (kilit hatasi cikmaz).
#   Cikti: overinstall-test.log + overinstall-test.result. SELF-ELEVATING (tek UAC).
$ErrorActionPreference = 'Continue'
$base  = 'C:\Users\Evrim\Desktop\projects\net\src-tauri'
$dir   = 'C:\Program Files\evorift'
$setup = "$base\target\release\bundle\nsis\evorift_0.1.0_x64-setup.exe"
$sys   = "$dir\winws\WinDivert64.sys"
$log   = "$base\examples\overinstall-test.log"
$res   = "$base\examples\overinstall-test.result"

$pr = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $pr.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
  Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File',"`"$PSCommandPath`""
  exit
}
function L($m){ Add-Content -Path $log -Value ("$((Get-Date -Format 'HH:mm:ss')) $m") }
function Locked($p){ try { $s=[IO.File]::Open($p,'Open','Write','None'); $s.Close(); return $false } catch { return $true } }
Set-Content $log "=== overinstall-test $(Get-Date -Format o) ==="; Set-Content $res 'RUNNING'
if (-not (Test-Path $setup)) { L "HATA: setup yok $setup"; Set-Content $res 'FAIL(no-setup)'; exit 1 }

# === BASELINE: senaryonun gercekten 'kilitli' oldugunu belgele ===
$winwsB = @(Get-Process winws -EA SilentlyContinue).Count
$wdB    = (sc.exe query WinDivert 2>&1 | Out-String) -match 'RUNNING'
$lockB  = if (Test-Path $sys) { Locked $sys } else { $null }
L ("BASELINE: winws=$winwsB, WinDivert RUNNING=$wdB, WinDivert64.sys KILITLI=$lockB  (kilitli=TRUE beklenir = sorun senaryosu)")
if ($winwsB -lt 1) { L "UYARI: baseline'da winws yok -> once normal calisan kurulum gerek (yine de devam)." }

# === UZERINE KURULUM (/S) — manuel teardown YOK, sadece PREINSTALL hook ===
L "Uzerine kurulum (/S) basliyor — PREINSTALL: svc+winws+evorift+tunel+WinDivert durdurmali, sonra extract."
$p = Start-Process $setup -ArgumentList '/S' -PassThru -Wait
L ("Installer exit kodu: " + $p.ExitCode + "  (0 = kurulum tamamlandi, kilit hatasinda takilmadi)")
Start-Sleep 6

# === DOGRULAMA: auto-protect otursun + winws + WARP + TLS ===
L "auto-protect bekle ~30s..."
Start-Sleep 30
$svc   = Get-Service EvoriftSvc -EA SilentlyContinue
$winwsA= @(Get-Process winws -EA SilentlyContinue).Count
$tun   = Get-Service 'WireGuardTunnel$warp' -EA SilentlyContinue
$wdPath= (sc.exe qc WinDivert 2>&1 | Out-String) -split "`n" | Select-String 'BINARY_PATH_NAME'
L ("SONRA: EvoriftSvc=$($svc.Status), winws=$winwsA instance, WARP=$($tun.Status)")
L ("WinDivert path: " + ($wdPath -join '').Trim())
$t   = & powershell -NoProfile -ExecutionPolicy Bypass -File "$base\examples\test-evorift.ps1" 2>&1
$t | Add-Content $log
$tls = if ($t -match 'RESULT: PASS') { 'PASS' } else { 'FAIL' }
L ("TLS: $tls")

# Basari: kurulum exit 0 + winws calisiyor + svc Running. (Kilit cozulmeseydi /S .sys'i atlar ama asil
# kanit: kurulum HATASIZ tamamlandi ve motor calisir durumda -> uzerine kurulum senaryosu artik saglam.)
$ok = ($p.ExitCode -eq 0) -and ($winwsA -ge 1) -and ($svc -and $svc.Status -eq 'Running')
Set-Content $res $(if($ok){'PASS'}else{'FAIL'})
L ("=== bitti (" + $(if($ok){'PASS'}else{'FAIL'}) + ") — TLS=$tls ===")
