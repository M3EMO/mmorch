@echo off
cd /d "%~dp0.."
".venv\Scripts\python.exe" "scripts\nightly.py" %* >> "logs\nightly_task.out" 2>&1
exit /b %errorlevel%
