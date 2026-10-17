param([ValidateSet('cpu','nvidia')][string]$Mode, [string]$ScriptsDirectory = $PSScriptRoot)
$ErrorActionPreference = 'Stop'
try {
    Write-Host 'YouCraft speech model setup'
    Write-Host 'Close YouCraft first. Downloads require internet and several GB of free space.'
    if (-not $Mode) {
        $answer = Read-Host 'Use NVIDIA GPU? [y/N]'
        $Mode = if ($answer -match '^[yY]') { 'nvidia' } else { 'cpu' }
    }
    if (-not (Get-Command py -ErrorAction SilentlyContinue)) {
        throw 'Install Python 3.11 from python.org (include the Python launcher), then run setup again.'
    }
    & py -3.11 -c 'import sys; assert sys.maxsize > 2**32'
    if ($LASTEXITCODE -ne 0) { throw 'Python 3.11 x64 is required. Install it from python.org, then retry.' }
    $root = Join-Path $env:LOCALAPPDATA 'YouCraft'
    $runtime = Join-Path $root "speech-$Mode"
    $python = Join-Path $runtime 'Scripts/python.exe'
    if (-not (Test-Path $python)) {
        & py -3.11 -m venv $runtime
        if ($LASTEXITCODE -ne 0) { throw 'Could not create the speech runtime.' }
    }
    & $python -m pip install --upgrade pip
    if ($LASTEXITCODE -ne 0) { throw 'pip setup failed.' }
    $index = if ($Mode -eq 'nvidia') { 'https://download.pytorch.org/whl/cu128' } else { 'https://download.pytorch.org/whl/cpu' }
    & $python -m pip install 'torch==2.11.0' 'torchaudio==2.11.0' --index-url $index
    if ($LASTEXITCODE -ne 0) { throw 'PyTorch installation failed.' }
    & $python -m pip install 'https://github.com/huggingface/transformers/archive/536ecc007387a50e77603bb5d92100e9b07514cc.zip' soundfile sentencepiece librosa einops
    if ($LASTEXITCODE -ne 0) { throw 'Speech dependencies failed to install.' }
    & $python -c 'import torch, torchaudio, soundfile; from transformers import AutoModelForAudioFrameClassification, AutoProcessor; print("CUDA available:", torch.cuda.is_available())'
    if ($LASTEXITCODE -ne 0) { throw 'Runtime import check failed.' }
    Write-Host 'Accept the model access terms using your Hugging Face account:'
    Write-Host 'https://huggingface.co/bodhan-ai/indic-transcribe-core'
    Write-Host 'If access is already granted and you are logged in, skip login.'
    $login = Read-Host 'Log into Hugging Face now? [y/N]'
    if ($login -match '^[yY]') {
        & "$runtime/Scripts/hf.exe" auth login
        if ($LASTEXITCODE -ne 0) { throw 'Hugging Face login failed.' }
    }
    $models = Join-Path $root 'models'
    & $python "$ScriptsDirectory/download_models.py" --directory $models
    if ($LASTEXITCODE -ne 0) { throw 'Model download failed. Check access and connection, then retry; cached downloads are reused.' }
    $device = if ($Mode -eq 'nvidia') { 'auto' } else { 'cpu' }
    & $python "$ScriptsDirectory/configure_local_models.py" --directory $models --output "$root/models.json" --device $device
    if ($LASTEXITCODE -ne 0) { throw 'Model configuration failed.' }
    if (-not (Get-Command ffmpeg -ErrorAction SilentlyContinue) -or -not (Get-Command ffprobe -ErrorAction SilentlyContinue)) {
        Write-Warning 'Install external FFmpeg and ffprobe, add their bin folder to PATH, then restart YouCraft.'
    }
    Write-Host 'Speech setup complete. Restart YouCraft. Start with the 60-second test audio.'
    Write-Host 'Gaze analysis and Romanized spelling require separate optional model packs.'
} catch {
    Write-Error $_ -ErrorAction Continue
    exit 1
}
