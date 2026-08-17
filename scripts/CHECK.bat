@echo off
setlocal EnableExtensions
title evorift test agent - diagnostic

rem ============================================================================
rem  DOUBLE-CLICK THIS ON THE TEST LAPTOP when the controller cannot reach it.
rem
rem  Read-only: it changes nothing. It reports why the agent is unreachable and
rem  saves the report next to this file (i.e. onto the USB stick you ran it from)
rem  as check-report.txt.
rem ============================================================================

net session >nul 2>&1
if %errorlevel% equ 0 goto elevated

echo.
echo   Asking Windows for administrator rights...
echo   Click YES on the popup. (Some checks need it.)
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
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0check.ps1" -OutFile "%~dp0check-report.txt"

echo.
echo  ============================================================
if exist "%~dp0check-report.txt" (
    echo    Report saved to:
    echo      %~dp0check-report.txt
    echo.
    echo    Take the USB stick back to the desktop and send me that
    echo    file, or just photograph the VERDICT section above.
) else (
    echo    The report file could not be written, but the output is
    echo    on screen above - photograph the VERDICT section.
)
echo  ============================================================
echo.
pause
