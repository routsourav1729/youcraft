@echo off
setlocal
cd /d "%~dp0.."
if not exist .venv\Scripts\python.exe py -3.11 -m venv .venv
if errorlevel 1 goto failed
.venv\Scripts\python.exe -m pip install -e .
if errorlevel 1 goto failed
if exist models\local-config.json set "BETUN_MODEL_CONFIG=%CD%\models\local-config.json"
set "PYTHONPATH=%CD%\backend"
.venv\Scripts\python.exe -m betun --open
if errorlevel 1 goto failed
exit /b 0
:failed
echo Startup failed. Python 3.11 and FFmpeg/ffprobe on PATH are required for processing imports.
pause
exit /b 1
