param([string]$Repository)
$ErrorActionPreference = 'Stop'
$root = Join-Path $env:RUNNER_TEMP 'youcraft-ai-registration-test'
New-Item -ItemType Directory -Force $root | Out-Null
Copy-Item "$Repository/packaging/ama/Connect-AI.ps1" $root
[IO.File]::WriteAllText((Join-Path $root 'filmcraft-cli.exe'), 'path-only fixture')
$before = $env:APPDATA
try {
    $env:APPDATA = Join-Path $root 'profile'
    $folder = Join-Path $env:APPDATA 'Claude'
    New-Item -ItemType Directory -Force $folder | Out-Null
    $path = Join-Path $folder 'claude_desktop_config.json'
    [IO.File]::WriteAllText($path, '{"preferences":{"theme":"dark"},"mcpServers":{"existing":{"command":"keep.exe","args":[]}}}')
    & powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$root/Connect-AI.ps1" -Client desktop -StatusFile "$root/status.json"
    if ($LASTEXITCODE -ne 0) { throw 'Claude Desktop registration failed' }
    $config = Get-Content $path -Raw | ConvertFrom-Json
    if ($config.preferences.theme -ne 'dark' -or $config.mcpServers.existing.command -ne 'keep.exe') { throw 'Registration changed unrelated Claude settings' }
    if ($config.mcpServers.youcraft.command -ne "$root\filmcraft-cli.exe") { throw 'Wrong MCP executable path' }
    if (($config.mcpServers.youcraft.args -join ' ') -ne 'mcp --bridge 127.0.0.1:9876') { throw 'Wrong MCP bridge arguments' }
    if (-not (Test-Path "$path.youcraft-backup")) { throw 'Config backup missing' }
    [IO.File]::WriteAllText($path, 'invalid-json')
    & powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$root/Connect-AI.ps1" -Client desktop -StatusFile "$root/status.json"
    if ($LASTEXITCODE -eq 0 -or (Get-Content $path -Raw) -ne 'invalid-json') { throw 'Malformed config was overwritten' }
    Write-Host 'Claude Desktop registration preserves existing settings and rejects corrupt files.'
} finally { $env:APPDATA = $before }

$global:LASTEXITCODE = 0
