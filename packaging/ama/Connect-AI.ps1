param([ValidateSet('desktop','codex','claude')][string]$Client, [string]$StatusFile)
$ErrorActionPreference = 'Stop'
function Report([string]$Stage, [string]$State = 'running', [string]$Problem = '') {
    Write-Host $Stage
    if ($StatusFile) {
        [IO.File]::WriteAllText("$StatusFile.tmp", (@{stage=$Stage;state=$State;error=$Problem} | ConvertTo-Json -Compress), (New-Object Text.UTF8Encoding($false)))
        Move-Item "$StatusFile.tmp" $StatusFile -Force
    }
}
try {
    if (-not $Client) { $Client = Read-Host 'Client [desktop/claude/codex]' }
    if ($Client -notin @('desktop','codex','claude')) { throw 'Choose Claude Desktop, Claude Code, or Codex.' }
    $cli = Join-Path $PSScriptRoot 'filmcraft-cli.exe'
    if (-not (Test-Path $cli)) { throw 'YouCraft MCP helper missing. Reinstall YouCraft.' }
    Report 'Registering YouCraft'
    if ($Client -eq 'desktop') {
        $folder = Join-Path $env:APPDATA 'Claude'
        New-Item -ItemType Directory -Force $folder | Out-Null
        $path = Join-Path $folder 'claude_desktop_config.json'
        $config = if (Test-Path $path) { Get-Content $path -Raw -Encoding utf8 | ConvertFrom-Json } else { [PSCustomObject]@{} }
        if ($null -eq $config -or $config -is [Array] -or $config -is [string]) { throw 'Claude config is invalid. Your existing file was left unchanged.' }
        if (-not $config.PSObject.Properties['mcpServers']) { $config | Add-Member -NotePropertyName mcpServers -NotePropertyValue ([PSCustomObject]@{}) }
        if ($null -eq $config.mcpServers -or $config.mcpServers -is [Array] -or $config.mcpServers -is [string]) { throw 'Claude MCP configuration is invalid. Your existing file was left unchanged.' }
        $server = [PSCustomObject]@{command=$cli;args=@('mcp','--bridge','127.0.0.1:9876')}
        $config.mcpServers | Add-Member -NotePropertyName youcraft -NotePropertyValue $server -Force
        if (Test-Path $path) { Copy-Item $path "$path.youcraft-backup" -Force }
        [IO.File]::WriteAllText("$path.tmp", ($config | ConvertTo-Json -Depth 100), (New-Object Text.UTF8Encoding($false)))
        Move-Item "$path.tmp" $path -Force
        Report "Registered in $path. Restart Claude; open a Local Code session and enable YouCraft." 'succeeded'
    } else {
        $clientCommand = Get-Command "$Client.exe", "$Client.cmd" -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
        if (-not $clientCommand) { throw "$Client terminal CLI was not found. If you use the Code tab inside Claude, choose Register Claude app instead." }
        $clientPath = $clientCommand.Source
        if ($Client -eq 'codex') {
            & $clientPath mcp add youcraft -- $cli mcp --bridge 127.0.0.1:9876
        } else {
            & $clientPath mcp add --transport stdio --scope user youcraft -- $cli mcp --bridge 127.0.0.1:9876
        }
        if ($LASTEXITCODE -ne 0) { throw 'Registration failed. Check setup log; if YouCraft is already registered, verify it in your client.' }
        Report "Registered. Restart $Client." 'succeeded'
    }
} catch { Report 'Connection setup failed' 'failed' $_.Exception.Message; Write-Error $_ -ErrorAction Continue; exit 1 }
