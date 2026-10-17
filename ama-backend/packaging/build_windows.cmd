@echo off
setlocal
cd /d "%~dp0.."
py -3.11 -m venv .venv
if errorlevel 1 goto failed
.venv\Scripts\python.exe -m pip install -e ".[packaging]"
if errorlevel 1 goto failed
.venv\Scripts\python.exe -m PyInstaller packaging\betun.spec --noconfirm
if errorlevel 1 goto failed
echo Built dist\BetunBackend\BetunBackend.exe. Test it on this Windows machine.
pause
exit /b 0
:failed
echo Build failed. Read the error above. Python 3.11 is required by this build script.
pause
exit /b 1
