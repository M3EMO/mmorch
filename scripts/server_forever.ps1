# mmorch server watchdog — corre el server y lo relanza si muere (backoff 10s).
# Registrar (una vez, PowerShell normal):
#   schtasks /Create /TN mmorch-server /SC ONLOGON /F /TR "powershell -WindowStyle Hidden -ExecutionPolicy Bypass -File C:\Users\map12\.claude\orchestration\scripts\server_forever.ps1"
# Fix medido 2026-08-14: el server corria como hijo de la sesion de Claude Code
# y moria con cada reinicio de la app (2 veces en un dia).
$repo = "C:\Users\map12\.claude\orchestration"
$py = Join-Path $repo ".venv\Scripts\python.exe"
$errlog = Join-Path $repo "logs\server_forever.err"
$port = if ($env:MMORCH_SERVER_PORT) { [int]$env:MMORCH_SERVER_PORT } else { 8787 }

# Rotacion al arrancar (defecto #4 r3): sin esto el err crece para siempre y el
# server_err_tail de health.report muestra errores historicos ya resueltos
# (10048 viejos), enmascarando los nuevos. Una generacion .1 alcanza de forense.
if ((Test-Path $errlog) -and ((Get-Item $errlog).Length -gt 262144)) {
  Move-Item -Force $errlog "$errlog.1"
}

while ($true) {
  # Fix W3.2 (bind 10048): si el puerto ya esta escuchando, NO relanzar en loop
  # infinito de bind. Un python viejo (huerfano de un watchdog anterior) se mata
  # una vez; cualquier otro proceso = conflicto real -> abortar con mensaje.
  $conn = Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue |
          Select-Object -First 1
  if ($conn) {
    $owner = Get-Process -Id $conn.OwningProcess -ErrorAction SilentlyContinue
    if ($owner -and $owner.ProcessName -match '^python') {
      "$(Get-Date -Format s) puerto $port tomado por python viejo (PID $($owner.Id)) - matando" |
        Add-Content $errlog
      Stop-Process -Id $owner.Id -Force -ErrorAction SilentlyContinue
      Start-Sleep -Seconds 2
      $conn = Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue |
              Select-Object -First 1
    }
    if ($conn) {
      $who = (Get-Process -Id $conn.OwningProcess -ErrorAction SilentlyContinue).ProcessName
      "$(Get-Date -Format s) puerto $port sigue tomado por '$who' (PID $($conn.OwningProcess)) - abortando watchdog" |
        Add-Content $errlog
      exit 1
    }
  }
  try {
    Set-Location $repo
    & $py -m mmorch.server 2>> $errlog
  } catch {}
  Start-Sleep -Seconds 10
}
