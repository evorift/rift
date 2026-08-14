@echo off
REM Stale WinDivert registration temizligi — YONETICI olarak calistirir (UAC sorar).
REM Sag tik > "Run as administrator" gerekmez: asagidaki satir kendini yukseltir.
setlocal
net session >nul 2>&1
if %errorlevel% neq 0 (
  powershell -NoProfile -Command "Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-NoExit','-File','%~dp0fix-windivert.ps1'"
  exit /b
)
powershell -NoProfile -ExecutionPolicy Bypass -NoExit -File "%~dp0fix-windivert.ps1"
