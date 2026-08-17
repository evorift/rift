# One-off diagnostic: tail evorift-svc's own audit log to see why it died mid-sweep.
$logDir = "$env:ProgramData\evorift\logs"
if (Test-Path -LiteralPath $logDir) {
    Get-ChildItem -LiteralPath $logDir -Filter "*.log" -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending | Select-Object -First 2 | ForEach-Object {
            Write-Host "=== $($_.FullName) (last 60 lines) ==="
            Get-Content -LiteralPath $_.FullName -Tail 60
        }
} else {
    Write-Host "no log dir at $logDir"
}
exit 0
