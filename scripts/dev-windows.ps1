param(
    [switch]$SkipBuild,
    [int]$BackendPort = 18765,
    [int]$ControlPort = 19876
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
Set-Location $repo
$cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
$python = Join-Path $repo '.venv\Scripts\python.exe'
if (-not (Test-Path $python)) { throw 'Create the Python 3.11 .venv and install ama-backend[test] first.' }
$env:PATH = "$(Split-Path $cargo);$env:PATH"
$env:CARGO_BUILD_JOBS = '3'
$env:CARGO_PROFILE_DEV_DEBUG = '0'
$env:BETUN_DATA_DIR = Join-Path $repo '.dev-data\backend'
$env:YOUCRAFT_UI_CACHE_DIR = Join-Path $repo '.dev-data\workflow'
$env:FILMCRAFT_DATA_DIR = Join-Path $repo '.dev-data\native'
$env:YOUCRAFT_BACKEND_URL = "http://127.0.0.1:$BackendPort"
$env:BETUN_MODEL_CONFIG = Join-Path $env:LOCALAPPDATA 'YouCraft\models.json'
$env:BETUN_NO_BROWSER = '1'
$logs = Join-Path $repo 'work'
New-Item -ItemType Directory -Force $logs,$env:BETUN_DATA_DIR,$env:YOUCRAFT_UI_CACHE_DIR,$env:FILMCRAFT_DATA_DIR | Out-Null
if (-not $SkipBuild) {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path $vswhere)) { throw 'Install Visual Studio C++ Build Tools and the Windows SDK first.' }
    $vsRoot = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $vsRoot) { throw 'MSVC x64 build tools are unavailable.' }
    Import-Module "$vsRoot\Common7\Tools\Microsoft.VisualStudio.DevShell.dll"
    Enter-VsDevShell -VsInstallPath $vsRoot -SkipAutomaticLocation -DevCmdArguments '-arch=x64 -host_arch=x64'
    & $cargo +1.95.0 build --locked -p filmcraft
    if ($LASTEXITCODE -ne 0) { throw 'Native build failed.' }
}
$app = Join-Path $repo 'target\debug\filmcraft.exe'
if (-not (Test-Path $app)) { throw 'Build filmcraft before using -SkipBuild.' }
foreach ($port in @($BackendPort,$ControlPort)) {
    $probe = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback,$port)
    try { $probe.Start() } finally { $probe.Stop() }
}
$helper = Start-Process -FilePath $python -ArgumentList @('-m','betun','--port',"$BackendPort") -WindowStyle Hidden -PassThru -RedirectStandardOutput "$logs\dev-backend.log" -RedirectStandardError "$logs\dev-backend-error.log"
try {
    $ready = $false
    for ($attempt=0; $attempt -lt 60; $attempt++) {
        if ($helper.HasExited) { throw 'Source helper exited; inspect work/dev-backend-error.log.' }
        try {
            $health = Invoke-RestMethod "$env:YOUCRAFT_BACKEND_URL/health" -TimeoutSec 2
            if ($health.status -eq 'ok' -and $health.service -eq 'youcraft') { $ready = $true; break }
        } catch { Start-Sleep -Milliseconds 500 }
    }
    if (-not $ready) { throw 'Source helper did not become ready.' }
    Write-Host "Source backend ready; launching editor with isolated development data. Control port: $ControlPort"
    # Debug builds use the console subsystem. CREATE_NO_WINDOW suppresses only
    # that console, unlike WindowStyle Hidden which can hide the editor itself.
    $editorStart = New-Object Diagnostics.ProcessStartInfo
    $editorStart.FileName = $app
    $editorStart.Arguments = "--empty --no-recover --control $ControlPort --data-dir `"$env:FILMCRAFT_DATA_DIR`""
    $editorStart.UseShellExecute = $false
    $editorStart.CreateNoWindow = $true
    $editor = [Diagnostics.Process]::Start($editorStart)
    $editor.WaitForExit()
    if ($editor.ExitCode -ne 0) { throw "Source editor exited with code $($editor.ExitCode)." }
} finally {
    if (-not $helper.HasExited) { Stop-Process -Id $helper.Id }
}
