@echo off
REM ============================================================
REM  evorift - SESSIZ fake-only DPI bypass (anti-detection)
REM  Amac: 5 dk sonra tetiklenen davranissal DPI throttle'ini
REM  ASLA tetiklememek. Gercek veri akisini BOLMEZ (multidisorder
REM  / fakedsplit YOK) -> DPI icin neredeyse gorunmez.
REM  YONETICI olarak calistir.
REM ============================================================

net session >nul 2>&1
if %errorlevel% neq 0 (
  echo [HATA] Bu dosyayi YONETICI olarak calistir.
  pause
  exit /b 1
)

echo [*] Eski bypass'lar durduruluyor...
sc stop GoodbyeDPI >nul 2>&1
taskkill /F /IM goodbyedpi.exe >nul 2>&1
taskkill /F /IM winws.exe >nul 2>&1
timeout /t 2 /nobreak >nul

cd /d "%~dp0src-tauri\resources\winws"

echo [*] SESSIZ winws baslatiliyor (fake-only)...
echo     Bu pencereyi ACIK BIRAK. 5 dakikadan FAZLA bekle ve test et.
echo.

winws.exe ^
  --wf-tcp=80,443 ^
  --wf-raw-part=@windivert.filter\windivert_part.discord_media_wide.txt ^
  --wf-raw-part=@windivert.filter\windivert_part.stun.txt ^
  --wf-raw-part=@windivert.filter\windivert_part.quic_initial_ietf.txt ^
  --filter-tcp=80 --dpi-desync=fake --dpi-desync-ttl=7 --dpi-desync-fooling=md5sig --new ^
  --filter-tcp=443 --dpi-desync=fake --dpi-desync-ttl=7 --dpi-desync-fooling=md5sig --dpi-desync-fake-tls-mod=rnd,dupsid,sni=www.google.com --new ^
  --filter-l7=quic --dpi-desync=fake --dpi-desync-repeats=2 --dpi-desync-fake-quic=files\quic_initial_www_google_com.bin --new ^
  --filter-l7=discord,stun --dpi-desync=fake --dpi-desync-repeats=2 --dpi-desync-fake-discord=files\quic_initial_www_google_com.bin --dpi-desync-fake-stun=files\quic_initial_www_google_com.bin

echo.
echo [!] winws kapandi.
pause
