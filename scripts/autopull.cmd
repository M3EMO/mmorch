@echo off
REM mmorch auto-pull task (esta PC). Pull ff-only y SOLO si el arbol esta limpio (sync.py
REM protege WIP). Lo dispara la Scheduled Task de Windows "mmorch-autopull" cada 15 min.
REM Crear/recrear la task:
REM   schtasks /Create /TN mmorch-autopull /TR "C:\Users\map12\.claude\orchestration\scripts\autopull.cmd" /SC MINUTE /MO 15 /F
REM Borrar:  schtasks /Delete /TN mmorch-autopull /F
cd /d "C:\Users\map12\.claude\orchestration"
".venv\Scripts\python.exe" -m mmorch.sync pull-all >> "logs\autopull.log" 2>&1
