# Identify what auto-relaunches evorift-svc/winws -- do NOT change anything, read-only.
Write-Host "=== scheduled tasks referencing evorift/winws ==="
try {
    $tasks = Get-ScheduledTask -ErrorAction Stop | Where-Object {
        $_.TaskName -match "evorift|winws" -or
        ($_.Actions | ForEach-Object { "$($_.Execute) $($_.Arguments)" }) -match "evorift|winws"
    }
    if ($tasks) {
        foreach ($t in $tasks) {
            $info = Get-ScheduledTaskInfo -TaskPath $t.TaskPath -TaskName $t.TaskName -ErrorAction SilentlyContinue
            Write-Host "  TaskName : $($t.TaskName)"
            Write-Host "  TaskPath : $($t.TaskPath)"
            Write-Host "  State    : $($t.State)"
            Write-Host "  Triggers :"
            $t.Triggers | ForEach-Object { Write-Host "    $($_.CimClass.CimClassName)  Enabled=$($_.Enabled)  StartBoundary=$($_.StartBoundary)" }
            Write-Host "  Actions  :"
            $t.Actions | ForEach-Object { Write-Host "    Execute='$($_.Execute)' Arguments='$($_.Arguments)' WorkingDirectory='$($_.WorkingDirectory)'" }
            Write-Host "  LastRunTime: $($info.LastRunTime)  LastTaskResult: $($info.LastTaskResult)"
            Write-Host "  RestoreCmd : schtasks /Change /TN `"$($t.TaskPath)$($t.TaskName)`" /Enable"
            Write-Host ""
        }
    } else {
        Write-Host "  none found"
    }
} catch {
    Write-Host "  Get-ScheduledTask failed: $($_.Exception.Message) -- falling back to schtasks.exe"
    & schtasks.exe /query /fo LIST /v 2>&1 | Select-String -Pattern "evorift|winws" -Context 15,0
}

Write-Host ""
Write-Host "=== registry Run / RunOnce keys referencing evorift/winws ==="
$runKeys = @(
    "HKLM:\Software\Microsoft\Windows\CurrentVersion\Run",
    "HKLM:\Software\Microsoft\Windows\CurrentVersion\RunOnce",
    "HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Run",
    "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run",
    "HKCU:\Software\Microsoft\Windows\CurrentVersion\RunOnce"
)
foreach ($k in $runKeys) {
    if (Test-Path $k) {
        $props = Get-ItemProperty -Path $k -ErrorAction SilentlyContinue
        $props.PSObject.Properties | Where-Object { $_.Name -notmatch '^PS' -and $_.Value -match "evorift|winws" } | ForEach-Object {
            Write-Host "  ${k}: $($_.Name) = $($_.Value)"
        }
    }
}

Write-Host ""
Write-Host "=== all user profiles' Run keys (evorift-testd runs as a service account) ==="
Get-ChildItem "Registry::HKEY_USERS" -ErrorAction SilentlyContinue | Where-Object { $_.PSChildName -match '^S-1-5-21-.*[^_Classes]$' } | ForEach-Object {
    $k = "Registry::$($_.PSPath.Split('::')[1])\Software\Microsoft\Windows\CurrentVersion\Run"
    if (Test-Path $k) {
        $props = Get-ItemProperty -Path $k -ErrorAction SilentlyContinue
        $props.PSObject.Properties | Where-Object { $_.Name -notmatch '^PS' -and $_.Value -match "evorift|winws" } | ForEach-Object {
            Write-Host "  ${k}: $($_.Name) = $($_.Value)"
        }
    }
}

Write-Host ""
Write-Host "=== Startup folders ==="
$startupDirs = @(
    "$env:ProgramData\Microsoft\Windows\Start Menu\Programs\StartUp",
    "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\StartUp"
)
Get-ChildItem "C:\Users" -Directory -ErrorAction SilentlyContinue | ForEach-Object {
    $startupDirs += Join-Path $_.FullName "AppData\Roaming\Microsoft\Windows\Start Menu\Programs\StartUp"
}
foreach ($d in $startupDirs) {
    if (Test-Path -LiteralPath $d) {
        Get-ChildItem -LiteralPath $d -ErrorAction SilentlyContinue | Where-Object { $_.Name -match "evorift|winws" } | ForEach-Object {
            Write-Host "  $($_.FullName)"
        }
    }
}

Write-Host ""
Write-Host "=== services (broad match) ==="
Get-CimInstance Win32_Service -ErrorAction SilentlyContinue | Where-Object {
    $_.Name -match "evorift|winws" -or $_.DisplayName -match "evorift|winws" -or $_.PathName -match "evorift|winws"
} | ForEach-Object {
    Write-Host "  Name=$($_.Name) DisplayName=$($_.DisplayName) StartMode=$($_.StartMode) State=$($_.State)"
    Write-Host "  PathName=$($_.PathName)"
}

Write-Host ""
Write-Host "=== currently running evorift/winws processes + their parent + start time ==="
Get-CimInstance Win32_Process -ErrorAction SilentlyContinue | Where-Object { $_.Name -match "evorift|winws" } | ForEach-Object {
    $parent = Get-CimInstance Win32_Process -Filter "ProcessId=$($_.ParentProcessId)" -ErrorAction SilentlyContinue
    Write-Host "  pid=$($_.ProcessId) name=$($_.Name) parentPid=$($_.ParentProcessId) parentName=$($parent.Name) created=$($_.CreationDate) cmd=$($_.CommandLine)"
}
exit 0
