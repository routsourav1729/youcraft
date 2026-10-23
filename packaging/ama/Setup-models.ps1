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
    if (-not $RuntimeOnly) {
        Report 'Preparing media tools'
        & "$ScriptsDirectory/Setup-media.ps1" -Root $Root -StatusFile $StatusFile
        CheckExit 'Media setup failed. Open Setup log for details.'
    }
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
    $runtimeStamp = Join-Path $runtime 'youcraft-runtime-v2.ready'
    if (Test-Path $runtimeStamp) {
        & $python -c 'import torch, torchaudio, soundfile, sentencepiece, librosa, einops; from transformers import AutoModelForAudioFrameClassification, AutoProcessor'
        if ($LASTEXITCODE -ne 0) { Remove-Item $runtimeStamp -Force }
    }
    if (-not (Test-Path $runtimeStamp)) {
        Report 'Installing speech runtime'
        & $python -m pip install --upgrade pip
        CheckExit 'pip setup failed.'
        $index = if ($Mode -eq 'nvidia') { 'https://download.pytorch.org/whl/cu128' } else { 'https://download.pytorch.org/whl/cpu' }
        & $python -m pip install 'torch==2.11.0' 'torchaudio==2.11.0' --index-url $index
        CheckExit 'PyTorch installation failed. Check your connection and retry.'
        & $python -m pip install 'https://github.com/huggingface/transformers/archive/536ecc007387a50e77603bb5d92100e9b07514cc.zip' soundfile sentencepiece librosa einops
        CheckExit 'Speech dependencies failed to install.'
    } else {
        Report 'Speech runtime ready'
    }
    Report 'Checking speech runtime'
    & $python -c 'import torch, torchaudio, soundfile; from transformers import AutoModelForAudioFrameClassification, AutoProcessor; print(torch.cuda.is_available())'
    CheckExit 'Speech runtime check failed.'
    if ($Mode -eq 'nvidia') {
        & $python -c 'import torch; assert torch.cuda.is_available()'
        CheckExit 'NVIDIA GPU unavailable. Update its driver or choose CPU setup.'
    }
    [IO.File]::WriteAllText($runtimeStamp, $Mode)
    if ($RuntimeOnly) { Report 'Runtime ready' 'succeeded'; exit 0 }
    $models = Join-Path $Root 'models'
    $device = if ($Mode -eq 'nvidia') { 'cuda' } else { 'cpu' }
    foreach ($pack in @('nemotron','indic-core')) {
        Report $(if ($pack -eq 'nemotron') { 'Downloading speaker model' } else { 'Downloading Odia model' })
        $downloadArgs = @("$ScriptsDirectory/download_models.py", '--directory', $models, '--pack', $pack)
        if ($StatusFile) { $downloadArgs += @('--status-file', $StatusFile) }
        & $python @downloadArgs
        if ($LASTEXITCODE -ne 0) {
            if ($StatusFile -and (Test-Path $StatusFile)) {
                $downloadStatus = Get-Content $StatusFile -Raw | ConvertFrom-Json
                if ($downloadStatus.error) { throw $downloadStatus.error }
            }
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
