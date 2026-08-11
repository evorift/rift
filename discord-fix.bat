@echo off
REM ============================================================
REM  evorift - Discord/genel DPI bypass (standalone winws)
REM  GoodbyeDPI yerine: Turkcell'in yakaladigi GBDPI imzasi YOK.
REM  YONETICI olarak calistir (sag tik > Run as administrator).
REM ============================================================

REM --- yonetici kontrolu ---
net session >nul 2>&1
if %errorlevel% neq 0 (
  echo [HATA] Bu dosyayi YONETICI olarak calistir ^(sag tik ^> Run as administrator^).
  pause
  exit /b 1
)

REM --- GoodbyeDPI'yi tamamen kapat (WinDivert cakismasini onler) ---
echo [*] GoodbyeDPI durduruluyor...
sc stop GoodbyeDPI >nul 2>&1
taskkill /F /IM goodbyedpi.exe >nul 2>&1
taskkill /F /IM winws.exe >nul 2>&1
timeout /t 1 /nobreak >nul

cd /d "%~dp0src-tauri\resources\winws"

echo [*] winws baslatiliyor (catch-all DPI desync)...
echo     Bu pencereyi ACIK BIRAK. Kapatirsan bypass durur.
echo.

winws.exe ^
  --wf-tcp=80,443 ^
  --wf-raw-part=@windivert.filter\windivert_part.discord_media_wide.txt ^
  --wf-raw-part=@windivert.filter\windivert_part.stun.txt ^
  --wf-raw-part=@windivert.filter\windivert_part.quic_initial_ietf.txt ^
  --filter-tcp=80 --dpi-desync=fake,fakedsplit --dpi-desync-autottl=2 --dpi-desync-fooling=md5sig --new ^
  --filter-tcp=443 --dpi-desync=fake,multidisorder --dpi-desync-split-pos=1,midsld --dpi-desync-repeats=11 --dpi-desync-fooling=md5sig --dpi-desync-fake-tls-mod=rnd,dupsid,sni=www.google.com --new ^
  --filter-tcp=443 --dpi-desync=fake,multidisorder --dpi-desync-split-pos=midsld --dpi-desync-repeats=6 --dpi-desync-fooling=badseq,md5sig --new ^
  --filter-l7=quic --dpi-desync=fake --dpi-desync-repeats=11 --dpi-desync-fake-quic=files\quic_initial_www_google_com.bin --new ^
  --filter-l7=discord,stun --dpi-desync=fake --dpi-desync-repeats=6 --dpi-desync-fake-discord=files\quic_initial_www_google_com.bin --dpi-desync-fake-stun=files\quic_initial_www_google_com.bin

echo.
echo [!] winws kapandi. Hata varsa yukaridaki mesaja bak.
pause
