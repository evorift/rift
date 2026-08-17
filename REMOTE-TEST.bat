@echo off
setlocal EnableExtensions EnableDelayedExpansion
title evorift remote test - controller

rem ============================================================================
rem  DOUBLE-CLICK THIS FILE ON YOUR DESKTOP (the controller).
rem
rem  It finds the token the laptop installer saved to your USB stick, then gives
rem  you a menu. No administrator rights are needed here - the controller only
rem  makes HTTP requests. Only the laptop side needs UAC.
rem ============================================================================

set "PORT=8765"
set "REPO=%~dp0"
set "REMOTE=%REPO%scripts\remote.ps1"

if not exist "%REMOTE%" (
    echo.
    echo   ERROR: cannot find scripts\remote.ps1
    echo   This file must stay in the repo root: %REPO%
    echo.
    pause
    exit /b 1
)

rem --- Find the token: next to this file first, then any removable drive ------
rem `if defined` is evaluated at run time, so the first hit wins and later
rem iterations leave it alone.
rem %%~A strips the surrounding quotes; a bare D:\ is unaffected. Do NOT use %%d
rem here -- ~d is the "drive letter only" modifier and would silently mangle it.
set "TOKEN_FILE="
for %%A in ("%~dp0" D:\ E:\ F:\ G:\ H:\ I:\) do (
    if exist "%%~Atestd.token" if not defined TOKEN_FILE set "TOKEN_FILE=%%~Atestd.token"
)

if not defined TOKEN_FILE (
    echo.
    echo  ============================================================
    echo    TOKEN NOT FOUND
    echo.
    echo    Plug in the USB stick you used on the laptop, then run
    echo    this file again.
    echo.
    echo    Looked next to this file and on drives D: to I: for a
    echo    file named  testd.token
    echo.
    echo    If you wrote the token down by hand instead, create a
    echo    file called testd.token next to this .bat and paste the
    echo    64 characters into it.
    echo  ============================================================
    echo.
    pause
    exit /b 1
)

:discover
rem Find the laptop rather than assuming an address. Both machines' DHCP leases moved once, which
rem broke every pinned address at the same time; discovery costs a second and removes that class
rem of failure entirely.
echo.
echo   Looking for the test laptop...
set "AGENT_IP="
for /f "delims=" %%A in ('powershell -NoProfile -ExecutionPolicy Bypass -File "%REPO%scripts\find-agent.ps1" -Port %PORT% -Quiet 2^>nul') do set "AGENT_IP=%%A"

if not defined AGENT_IP (
    echo.
    echo  ============================================================
    echo    COULD NOT FIND THE TEST LAPTOP
    echo.
    echo    Nothing is answering on port %PORT% anywhere on this LAN.
    echo    Usually this means the agent service is not running.
    echo.
    echo    On the laptop, double-click:  CHECK.bat
    echo    then, if needed:              INSTALL.bat
    echo  ============================================================
    echo.
    pause
    exit /b 1
)

:menu
cls
echo.
echo  ============================================================
echo    evorift REMOTE TEST
echo.
echo    Laptop : %AGENT_IP%   (found automatically)
echo    Token  : %TOKEN_FILE%
echo  ============================================================
echo.
echo    [1]  Connection test        (is the laptop answering?)
echo.
echo    [2]  DRY RUN                (capture only - does NOT start evorift)
echo.
echo    [3]  ** THE REAL TEST **    (starts evorift, cuts the internet,
echo         captures it, stops it, checks the network comes back)
echo.
echo    [4]  EMERGENCY: rescue the laptop now
echo         (kills evorift + winws, clears WinDivert, resets DNS)
echo.
echo    [5]  Network problem? Diagnose the link
echo.
echo    [6]  Re-find the laptop (if its IP changed just now)
echo.
echo    [7]  Exit
echo.
set "choice="
set /p "choice=  Type a number and press Enter: "

if "%choice%"=="1" goto health
if "%choice%"=="2" goto cycle
if "%choice%"=="3" goto engine
if "%choice%"=="4" goto rescue
if "%choice%"=="5" goto diagnose
if "%choice%"=="6" goto rediscover
if "%choice%"=="7" exit /b 0
goto menu

:engine
cls
echo.
echo  ============================================================
echo    THE REAL TEST - THIS WILL CUT THE LAPTOP'S INTERNET
echo.
echo    On the laptop it will:
echo      1. capture the machine's state BEFORE
echo      2. start evorift-svc and turn protection ON
echo      3. hold, then capture the state DURING (network likely dead)
echo      4. turn protection OFF and kill any leftover winws
echo      5. capture AFTER, and check the network came back
echo.
echo    All of that runs ON THE LAPTOP as one job, so it completes
echo    even if the link to this desktop drops completely.
echo.
echo    Three brakes: the script stops the engine itself, the agent
echo    times the job out, and the deadman switch kills evorift and
echo    winws if this desktop loses contact for 120 seconds.
echo  ============================================================
echo.
set "hold="
set /p "hold=  Seconds to hold protection ON [15]: "
if not defined hold set "hold=15"
echo.
set "go="
set /p "go=  Start the real test? (y/n): "
if /i not "%go%"=="y" goto menu
echo.
powershell -NoProfile -ExecutionPolicy Bypass -Command "$env:EVORIFT_TESTD_TOKEN=(Get-Content '%TOKEN_FILE%' -Raw).Trim(); & '%REMOTE%' -AgentIp %AGENT_IP% -Action engine -TimeoutSecs %hold%"
echo.
echo   ------------------------------------------------------------
echo    Read the VERDICT section above:
echo      "winws RAN"                  = a real bypass was active
echo      "network recovered"          = it cleaned up correctly
echo      "did NOT recover"            = the reported bug, reproduced
echo   ------------------------------------------------------------
echo.
pause
goto menu

:rediscover
del "%REPO%scripts\.agent-address" >nul 2>&1
goto discover

:health
cls
echo.
echo   Checking the laptop...
echo.
powershell -NoProfile -ExecutionPolicy Bypass -Command "$env:EVORIFT_TESTD_TOKEN=(Get-Content '%TOKEN_FILE%' -Raw).Trim(); & '%REMOTE%' -AgentIp %AGENT_IP% -Action health"
echo.
echo   ------------------------------------------------------------
echo    Look for  "ok": true   above. That means you are connected.
echo    403 = the laptop does not trust this desktop's address.
echo    401 = wrong token.
echo    No answer = the laptop is off, or the agent is not running.
echo   ------------------------------------------------------------
echo.
pause
goto menu

:cycle
cls
echo.
echo  ============================================================
echo    FULL TEST CYCLE
echo.
echo    This pushes the build to the laptop, captures its state
echo    before / during / after, pulls everything back, and checks
echo    whether the laptop had to rescue itself.
echo.
echo    It can take several minutes, and the laptop WILL go
echo    unreachable in the middle. That is expected - the script
echo    keeps retrying. Do not close this window.
echo  ============================================================
echo.

set "BUILD=%REPO%src-tauri\target\release\evorift.exe"
if exist "%BUILD%" (
    echo    Build to push: %BUILD%
    rem SINGLE quotes on purpose: this whole thing is pasted inside a double-quoted
    rem cmd argument below, so a nested double quote would terminate it early.
    rem PowerShell reads single quotes fine, and a build path never contains one.
    set "BUILD_ARG=-BuildPath '%BUILD%'"
) else (
    echo    NOTE: no build found at
    echo          %BUILD%
    echo    Running against whatever build is already on the laptop.
    set "BUILD_ARG="
)
echo.
set "go="
set /p "go=  Start? (y/n): "
if /i not "%go%"=="y" goto menu

echo.
powershell -NoProfile -ExecutionPolicy Bypass -Command "$env:EVORIFT_TESTD_TOKEN=(Get-Content '%TOKEN_FILE%' -Raw).Trim(); & '%REMOTE%' -AgentIp %AGENT_IP% -Action cycle %BUILD_ARG%"
echo.
echo   ------------------------------------------------------------
echo    READ THE LAST LINE ABOVE.
echo.
echo    "the deadman did not fire"  = results are trustworthy.
echo    "THE DEADMAN FIRED"         = the laptop rescued itself
echo                                  mid-test. Results are NOT
echo                                  trustworthy.
echo   ------------------------------------------------------------
echo.
pause
goto menu

:rescue
cls
echo.
echo  ============================================================
echo    EMERGENCY RESCUE
echo.
echo    Tells the laptop to kill evorift.exe and winws.exe, remove
echo    stale WinDivert services, and put DNS back to automatic.
echo.
echo    This only works if the laptop is still reachable. If it is
echo    not, the laptop's own deadman switch does the same thing
echo    by itself after 120 seconds of silence.
echo  ============================================================
echo.
set "go="
set /p "go=  Send the rescue command? (y/n): "
if /i not "%go%"=="y" goto menu
echo.
powershell -NoProfile -ExecutionPolicy Bypass -Command "$env:EVORIFT_TESTD_TOKEN=(Get-Content '%TOKEN_FILE%' -Raw).Trim(); & '%REMOTE%' -AgentIp %AGENT_IP% -Action recover"
echo.
pause
goto menu

:diagnose
cls
echo.
powershell -NoProfile -ExecutionPolicy Bypass -File "%REPO%scripts\link-setup.ps1" -Action diagnose -PeerIp %AGENT_IP%
echo.
pause
goto menu
