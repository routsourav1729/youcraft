param([string]$Client = 'desktop', [string]$StatusFile, [switch]$ToolsOnly)
$ErrorActionPreference = 'Stop'
function Report([string]$Stage, [string]$State = 'running', [string]$Problem = '') {
    Write-Host $Stage
    if ($StatusFile) {
        [IO.File]::WriteAllText("$StatusFile.tmp", (@{stage=$Stage;state=$State;error=$Problem} | ConvertTo-Json -Compress), (New-Object Text.UTF8Encoding($false)))
        Move-Item "$StatusFile.tmp" $StatusFile -Force
    }
}
try {
    $cli = Join-Path $PSScriptRoot 'filmcraft-cli.exe'
    $helper = Join-Path $PSScriptRoot 'BetunBackend/BetunBackend.exe'
    if (-not (Test-Path $cli) -or -not (Test-Path $helper)) { throw 'Connection helper missing. Reinstall YouCraft.' }
    Report 'Checking MCP tools'
    $arguments = @('--check-mcp', $cli)
    if ($ToolsOnly) { $arguments += '--tools-only' }
    & $helper @arguments
    if ($LASTEXITCODE -ne 0) { throw 'MCP check failed. Open AI log for the connection error. Keep YouCraft open and retry.' }
    Report 'Tools verified. Enable YouCraft in Claude chat.' 'succeeded'
} catch {
    Report 'Connection check failed' 'failed' $_.Exception.Message
    Write-Error $_ -ErrorAction Continue
    exit 1
}
