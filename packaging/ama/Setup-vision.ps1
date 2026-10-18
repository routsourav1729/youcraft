param(
    [string]$Root = (Join-Path $env:LOCALAPPDATA 'YouCraft'),
    [string]$StatusFile,
    [string]$ScriptsDirectory = $PSScriptRoot,
    [string]$Mode = 'cpu'
)
$ErrorActionPreference = 'Stop'
function Report([string]$Stage, [string]$State = 'running', [string]$Problem = '') {
    Write-Host $Stage
    if ($StatusFile) {
        $json = @{stage=$Stage;state=$State;error=$Problem} | ConvertTo-Json -Compress
        [IO.File]::WriteAllText("$StatusFile.tmp", $json, (New-Object Text.UTF8Encoding($false)))
        Move-Item "$StatusFile.tmp" $StatusFile -Force
    }
}
try {
    $base = @('python311/python.exe','speech-cpu/Scripts/python.exe','speech-nvidia/Scripts/python.exe') | ForEach-Object { Join-Path $Root $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $base) { throw 'Set up speech models first, then install reactions.' }
    $runtime = Join-Path $Root 'vision-cpu'
    $python = Join-Path $runtime 'Scripts/python.exe'
    if (-not (Test-Path $python)) {
        Report 'Preparing reaction runtime'
        & $base -m venv $runtime
        if ($LASTEXITCODE -ne 0) { throw 'Could not create reaction runtime.' }
    }
    $stamp = Join-Path $runtime 'youcraft-vision-v1.ready'
    if (Test-Path $stamp) {
        & $python -c 'import mediapipe, cv2, numpy'
        if ($LASTEXITCODE -ne 0) { Remove-Item $stamp -Force }
    }
    if (-not (Test-Path $stamp)) {
        Report 'Installing reaction runtime'
        & $python -m pip install 'mediapipe==0.10.21' 'opencv-contrib-python==4.11.0.86' 'numpy==1.26.4'
        if ($LASTEXITCODE -ne 0) { throw 'Reaction dependencies failed. Check your connection and retry.' }
        [IO.File]::WriteAllText($stamp, 'v1')
    }
    $arguments = @("$ScriptsDirectory/setup_vision.py", '--root', $Root)
    if ($StatusFile) { $arguments += @('--status-file', $StatusFile) }
    & $python @arguments
    if ($LASTEXITCODE -ne 0) { throw 'Reaction model setup failed. Open Setup log for details.' }
    Report 'Reaction model ready' 'succeeded'
} catch {
    Report 'Setup failed' 'failed' $_.Exception.Message
    Write-Error $_ -ErrorAction Continue
    exit 1
}
