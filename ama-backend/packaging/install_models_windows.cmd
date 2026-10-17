@echo off
setlocal
cd /d "%~dp0.."
py -3.11 -m venv .model-venv
if errorlevel 1 goto failed
.model-venv\Scripts\python.exe -m pip install torch torchaudio --index-url https://download.pytorch.org/whl/cpu
if errorlevel 1 goto failed
.model-venv\Scripts\python.exe -m pip install "transformers @ git+https://github.com/huggingface/transformers.git@536ecc007387a50e77603bb5d92100e9b07514cc" soundfile sentencepiece
if errorlevel 1 goto failed
echo Sign in to your Hugging Face account here. Do not paste tokens into ChatGPT.
.model-venv\Scripts\hf.exe auth login
if errorlevel 1 goto failed
.model-venv\Scripts\python.exe scripts\download_models.py
if errorlevel 1 goto failed
.model-venv\Scripts\python.exe scripts\configure_local_models.py
if errorlevel 1 goto failed
echo Model setup finished. Run Start local backend.cmd after FFmpeg and ffprobe are installed.
pause
exit /b 0
:failed
echo Setup failed. Read the error above. Indic Core requires accepted model access conditions.
pause
exit /b 1
