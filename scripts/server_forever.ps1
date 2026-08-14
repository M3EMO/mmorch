# mmorch server watchdog — corre el server y lo relanza si muere (backoff 10s).
# Registrar (una vez, PowerShell normal):
#   schtasks /Create /TN mmorch-server /SC ONLOGON /F /TR "powershell -WindowStyle Hidden -ExecutionPolicy Bypass -File C:\Users\map12\.claude\orchestration\scripts\server_forever.ps1"
# Fix medido 2026-08-14: el server corria como hijo de la sesion de Claude Code
# y moria con cada reinicio de la app (2 veces en un dia).
$repo = "C:\Users\map12\.claude\orchestration"
$py = Join-Path $repo ".venv\Scripts\python.exe"
while ($true) {
  try {
    Set-Location $repo
    & $py -m mmorch.server 2>> (Join-Path $repo "logs\server_forever.err")
  } catch {}
  Start-Sleep -Seconds 10
}
