@echo off
setlocal EnableExtensions
title evorift remote test agent - installer

rem ============================================================================
rem  DOUBLE-CLICK THIS FILE ON THE TEST LAPTOP.
rem
rem  It asks Windows for administrator rights (one popup - click Yes), installs
rem  the test agent, and then saves the access token next to this file so you
rem  can carry it back to the controller on the same USB stick.
rem
rem  Nothing needs to be typed. Nothing here needs internet.
rem
rem  This .bat is the user-facing entry point; the actual work is done by
rem  install.ps1 in the same folder. PowerShell is only ever invoked by this
rem  file, never by the operator.
rem ============================================================================

rem --- Are we already elevated? `net session` fails for non-admins. ------------
net session >nul 2>&1
if %errorlevel% equ 0 goto elevated

echo.
echo   Asking Windows for administrator rights...
echo   Click YES on the popup that appears.
echo.

rem Relaunch this same file elevated. -WorkingDirectory keeps the kit folder as
rem the working directory; without it the elevated copy starts in system32 and
rem the relative paths below would miss.
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

echo.
echo  ============================================================
echo    evorift remote test agent
echo    Installing on this laptop...
echo  ============================================================
echo.

rem Files copied from a USB stick or downloaded are often flagged by Windows as
rem "from another computer" and refuse to run. Clear that first.
powershell -NoProfile -ExecutionPolicy Bypass -Command "Get-ChildItem -Path '%~dp0*.ps1' | Unblock-File" >nul 2>&1

powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0install.ps1"
if %errorlevel% neq 0 goto failed

rem Carry the token back on the same stick this was run from, so nobody has to
rem retype 64 hex characters. It stays on removable media the operator holds.
copy /y "C:\ProgramData\evorift-testd\testd.token" "%~dp0testd.token" >nul 2>&1

echo.
echo  ============================================================
if exist "%~dp0testd.token" (
    echo    DONE - the agent is installed and running.
    echo.
    echo    The token was saved to:
    echo      %~dp0testd.token
    echo.
    echo    NEXT: unplug this USB stick, plug it into the desktop,
    echo          and double-click REMOTE-TEST.bat there.
) else (
    echo    DONE - the agent is installed and running.
    echo.
    echo    WARNING: the token file could not be copied here.
    echo    Write down the 64-character code shown above by hand.
)
echo  ============================================================
echo.
pause
exit /b 0

:failed
echo.
echo  ============================================================
echo    INSTALL FAILED
echo.
echo    Read the red text above - it says exactly what to fix.
echo    The most common cause is that this laptop cannot reach
echo    the controller. Nothing was half-installed.
echo  ============================================================
echo.
pause
exit /b 1
