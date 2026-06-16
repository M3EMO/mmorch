# Registra la Scheduled Task "mmorch-autopull": corre scripts/autopull.cmd cada 15 min.
# Uso:  powershell -ExecutionPolicy Bypass -File scripts\register-autopull.ps1
# Borrar: Unregister-ScheduledTask -TaskName mmorch-autopull -Confirm:$false
$action = New-ScheduledTaskAction -Execute "C:\Users\map12\.claude\orchestration\scripts\autopull.cmd"
$trigger = New-ScheduledTaskTrigger -Once -At (Get-Date) -RepetitionInterval (New-TimeSpan -Minutes 15)
$settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -DontStopIfGoingOnBatteries -AllowStartIfOnBatteries
Register-ScheduledTask -TaskName "mmorch-autopull" -Action $action -Trigger $trigger -Settings $settings `
  -Description "mmorch auto-pull ff-only cada 15 min (esta PC); salta arboles sucios" -Force | Out-Null
Write-Output "registered mmorch-autopull"
