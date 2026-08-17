@echo off
setlocal EnableExtensions
title evorift test agent - repair DNS

rem ============================================================================
rem  DOUBLE-CLICK THIS ON THE TEST LAPTOP if DNS is broken (the machine can ping
rem  1.1.1.1 but cannot resolve names).
rem
rem  Cause: the deadman switch resets DNS to DHCP every time it fires, and an
rem  older build fired repeatedly while idle. Newer builds stand down after 3
rem  attempts. This repairs the damage already done.
rem ============================================================================

net session >nul 2>&1
if %errorlevel% equ 0 goto elevated

echo.
echo   Asking Windows for administrator rights...
echo   Click YES on the popup.
echo.
powershell -NoProfile -Command "Start-Process -FilePath '%~f0' -WorkingDirectory '%~dp0' -Verb RunAs"
if %errorlevel% neq 0 (
    echo.
    echo   Could not request administrator rights.
    echo   Right-click this file and choose "Run as administrator" instead.
    echo.
    pause
)
exit /b

:elevated
cd /d "%~dp0"

powershell -NoProfile -ExecutionPolicy Bypass -Command "Get-ChildItem -Path '%~dp0*.ps1' | Unblock-File" >nul 2>&1
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0fix-dns.ps1"
set "RC=%errorlevel%"

echo.
echo  ============================================================
if "%RC%"=="0" (
    echo    DNS repaired. Nothing else to do here.
) else (
    echo    DNS is still broken - read the suggestions above.
)
echo  ============================================================
echo.
pause
