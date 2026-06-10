# build-app.ps1 — evorift'i DOGRU derle (release). "localhost refused" tuzagini ONLER.
#
# NEDEN: UI exe'sini `cargo build` ile derlersen tauri DEV moduna derler -> pencere acilirken
# devUrl http://localhost:1420'e gider -> dev sunucusu yoksa "localhost refused to connect".
# UI exe ICIN TEK DOGRU YOL: tauri CLI (frontend'i exe'ye GOMER, devUrl kullanmaz).
# `--no-bundle` = NSIS/MSI/updater paketlemeyi atla (hizli; sadece calisan exe lazimsa).
$ErrorActionPreference='Continue'
$root='C:\Users\Evrim\Desktop\projects\net'
$rel="$root\src-tauri\target\release"
Set-Location $root

Write-Host "[1/2] UI exe  -> npx tauri build --no-bundle  (PROD, frontend gomulu)" -ForegroundColor Cyan
& npx tauri build --no-bundle
$uiOk = ($LASTEXITCODE -eq 0)

Write-Host "[2/2] servis  -> cargo build --release --bin evorift-svc" -ForegroundColor Cyan
& cargo build --release --bin evorift-svc --manifest-path "$root\src-tauri\Cargo.toml"
$svcOk = ($LASTEXITCODE -eq 0)

Write-Host "`n=== SONUC ===" -ForegroundColor Cyan
foreach($f in 'evorift.exe','evorift-svc.exe'){
  if(Test-Path "$rel\$f"){ Write-Host ("  $f : " + (Get-Item "$rel\$f").LastWriteTime + " (" + (Get-Item "$rel\$f").Length + " byte)") }
  else { Write-Host "  $f : YOK!" -ForegroundColor Red }
}
if($uiOk -and $svcOk){ Write-Host "OK — simdi deploy-full.ps1 / evorift-guncelle.bat ile kur." -ForegroundColor Green }
else { Write-Host "DERLEME HATASI (uiOk=$uiOk svcOk=$svcOk)" -ForegroundColor Red }
# ASLA: cargo build --bin evorift  (UI dev moduna gider -> localhost refused). Hep tauri CLI kullan.