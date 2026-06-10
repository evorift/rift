# EvoriftSvc'yi baslat + 4 sn sonra hala ayakta mi kontrol et (cokuyor mu?). ELEVATED calisir.
$log = 'C:\Users\Evrim\Desktop\projects\net\src-tauri\examples\start-svc.log'
try {
  Start-Service EvoriftSvc -ErrorAction Stop
  Start-Sleep -Seconds 4
  $s1 = (Get-Service EvoriftSvc).Status
  Start-Sleep -Seconds 4
  $s2 = (Get-Service EvoriftSvc).Status
  $pipe = [System.IO.Directory]::GetFiles('\\.\pipe\') | Where-Object { $_ -match 'evorift' }
  ("4sn: {0} | 8sn: {1} | pipe: {2}" -f $s1, $s2, $(if($pipe){'VAR'}else{'YOK'})) | Set-Content $log
} catch {
  ("HATA: " + $_.Exception.Message) | Set-Content $log
}
