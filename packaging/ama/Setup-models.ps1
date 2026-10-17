param(
    [ValidateSet('cpu','nvidia')][string]$Mode = 'cpu',
    [string]$ScriptsDirectory = $PSScriptRoot,
    [string]$Root = (Join-Path $env:LOCALAPPDATA 'YouCraft'),
    [string]$StatusFile,
    [switch]$RuntimeOnly
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $Root | Out-Null
function Report([string]$Stage, [string]$State = 'running', [string]$Problem = '') {
    Write-Host $Stage
    if ($StatusFile) {
        $json = @{stage=$Stage;state=$State;error=$Problem} | ConvertTo-Json -Compress
        [IO.File]::WriteAllText("$StatusFile.tmp", $json, (New-Object Text.UTF8Encoding($false)))
        Move-Item "$StatusFile.tmp" $StatusFile -Force
    }
}
function CheckExit([string]$Message) { if ($LASTEXITCODE -ne 0) { throw $Message } }
try {
    Report 'Preparing Python'
    $basePython = Join-Path $Root 'python311/python.exe'
    if (-not (Test-Path $basePython)) {
        if (Get-Command py -ErrorAction SilentlyContinue) {
            try {
                $candidate = & py -3.11 -c 'import sys; assert sys.maxsize > 2**32; print(sys.executable)' 2>$null
                if ($LASTEXITCODE -eq 0 -and (Test-Path $candidate)) { $basePython = $candidate }
            } catch { $candidate = $null }
        }
    }
    if (-not (Test-Path $basePython)) {
        Report 'Downloading Python'
        $installer = Join-Path $Root 'python-3.11.9-amd64.exe'
        Invoke-WebRequest 'https://www.python.org/ftp/python/3.11.9/python-3.11.9-amd64.exe' -OutFile $installer
        $signature = Get-AuthenticodeSignature $installer
        if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'Python Software Foundation') { throw 'Python installer signature check failed.' }
        $target = Join-Path $Root 'python311'
        $install = Start-Process $installer -ArgumentList @('/quiet','InstallAllUsers=0','Include_launcher=0','Include_test=0','Include_doc=0','Include_pip=1','PrependPath=0',"TargetDir=`"$target`"") -PassThru -Wait
        if ($install.ExitCode -notin @(0,3010)) { throw "Python installation failed ($($install.ExitCode))." }
        $basePython = Join-Path $target 'python.exe'
    }
    $runtime = Join-Path $Root "speech-$Mode"
    $python = Join-Path $runtime 'Scripts/python.exe'
    if (-not (Test-Path $python)) {
        & $basePython -m venv $runtime
        CheckExit 'Could not create the speech runtime.'
    }
    Report 'Installing speech runtime'
    & $python -m pip install --upgrade pip
    CheckExit 'pip setup failed.'
    $index = if ($Mode -eq 'nvidia') { 'https://download.pytorch.org/whl/cu128' } else { 'https://download.pytorch.org/whl/cpu' }
    & $python -m pip install 'torch==2.11.0' 'torchaudio==2.11.0' --index-url $index
    CheckExit 'PyTorch installation failed. Check your connection and retry.'
    & $python -m pip install 'https://github.com/huggingface/transformers/archive/536ecc007387a50e77603bb5d92100e9b07514cc.zip' soundfile sentencepiece librosa einops
    CheckExit 'Speech dependencies failed to install.'
    Report 'Checking speech runtime'
    & $python -c 'import torch, torchaudio, soundfile; from transformers import AutoModelForAudioFrameClassification, AutoProcessor; print("CUDA available:", torch.cuda.is_available())'
    CheckExit 'Speech runtime check failed.'
    if ($Mode -eq 'nvidia') {
        & $python -c 'import torch; assert torch.cuda.is_available(), "NVIDIA driver unavailable; update it or select CPU"'
        CheckExit 'NVIDIA GPU unavailable. Update its driver or choose CPU setup.'
    }
    if ($RuntimeOnly) { Report 'Runtime ready' 'succeeded'; exit 0 }
    Report 'Checking media tools'
    $ffmpeg = (Get-Command ffmpeg -ErrorAction SilentlyContinue).Source
    $ffprobe = (Get-Command ffprobe -ErrorAction SilentlyContinue).Source
    if (-not $ffmpeg -or -not $ffprobe) {
        if (-not (Get-Command winget -ErrorAction SilentlyContinue)) { throw 'Windows App Installer is missing. Install it from Microsoft Store, then retry setup.' }
        Report 'Installing media tools'
        & winget install --id Gyan.FFmpeg --exact --silent --accept-source-agreements --accept-package-agreements --disable-interactivity
        CheckExit 'Media tool installation failed. See setup.log for the installer error.'
        $packages = Join-Path $env:LOCALAPPDATA 'Microsoft/WinGet/Packages'
        $ffmpeg = Get-ChildItem "$packages/Gyan.FFmpeg*" -Recurse -Filter ffmpeg.exe | Select-Object -First 1 -ExpandProperty FullName
        $ffprobe = Get-ChildItem "$packages/Gyan.FFmpeg*" -Recurse -Filter ffprobe.exe | Select-Object -First 1 -ExpandProperty FullName
        if (-not $ffmpeg -or -not $ffprobe) { throw 'Media tools were installed but could not be found. Restart Windows and retry setup.' }
    }
    & $ffmpeg -version
    CheckExit 'FFmpeg cannot run.'
    & $ffprobe -version
    CheckExit 'ffprobe cannot run.'
    [IO.File]::WriteAllText((Join-Path $Root 'media-tools.json'), (@{ffmpeg=$ffmpeg;ffprobe=$ffprobe} | ConvertTo-Json), (New-Object Text.UTF8Encoding($false)))
    $models = Join-Path $Root 'models'
    $device = if ($Mode -eq 'nvidia') { 'auto' } else { 'cpu' }
    foreach ($pack in @('nemotron','indic-core')) {
        Report $(if ($pack -eq 'nemotron') { 'Downloading speaker model' } else { 'Downloading Odia model' })
        & $python "$ScriptsDirectory/download_models.py" --directory $models --pack $pack
        if ($LASTEXITCODE -ne 0) {
            if ($pack -eq 'indic-core') { throw 'Odia model access/download failed. Accept its Hugging Face terms and enter a read token, then retry. The speaker model remains installed.' }
            throw 'Speaker model download failed. Check your connection and retry.'
        }
        & $python "$ScriptsDirectory/configure_local_models.py" --directory $models --output "$Root/models.json" --device $device --partial
        CheckExit 'Model configuration failed.'
    }
    Report 'Models ready' 'succeeded'
} catch {
    Report 'Setup failed' 'failed' $_.Exception.Message
    Write-Error $_ -ErrorAction Continue
    exit 1
}
