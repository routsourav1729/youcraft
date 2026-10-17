@echo off
set "YOUCRAFT_SETUP=%~dp0..\..\packaging\ama\Setup-models.ps1"
if not exist "%YOUCRAFT_SETUP%" set "YOUCRAFT_SETUP=%~dp0..\vendor\filmcraft\packaging\ama\Setup-models.ps1"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%YOUCRAFT_SETUP%" -ScriptsDirectory "%~dp0..\scripts"
pause
