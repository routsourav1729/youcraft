param([string]$Client = 'desktop', [string]$StatusFile, [switch]$ToolsOnly)
$ErrorActionPreference = 'Stop'
function Report([string]$Stage, [string]$State = 'running', [string]$Problem = '') {
    Write-Host $Stage
    if ($StatusFile) {
        [IO.File]::WriteAllText("$StatusFile.tmp", (@{stage=$Stage;state=$State;error=$Problem} | ConvertTo-Json -Compress), (New-Object Text.UTF8Encoding($false)))
        Move-Item "$StatusFile.tmp" $StatusFile -Force
    }
}
$process = $null
$started = $false
try {
    $cli = Join-Path $PSScriptRoot 'filmcraft-cli.exe'
    if (-not (Test-Path $cli)) { throw 'MCP executable missing. Reinstall YouCraft.' }
    Report 'Checking MCP tools'
    $info = New-Object Diagnostics.ProcessStartInfo
    $info.FileName = $cli
    $info.Arguments = 'mcp --bridge 127.0.0.1:9876'
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $process = New-Object Diagnostics.Process
    $process.StartInfo = $info
    if (-not $process.Start()) { throw 'Cannot start MCP executable.' }
    $started = $true
    $errors = $process.StandardError.ReadToEndAsync()
    function Send($Message) {
        # Write BOM-free UTF-8 directly: Windows console encodings can add a preamble.
        $bytes = [Text.Encoding]::UTF8.GetBytes($Message + "`n")
        $process.StandardInput.BaseStream.Write($bytes, 0, $bytes.Length)
        $process.StandardInput.BaseStream.Flush()
    }
    function Ask($Id, $Method, $Params) {
        Send (@{jsonrpc='2.0';id=$Id;method=$Method;params=$Params} | ConvertTo-Json -Depth 20 -Compress)
        for ($i=0; $i -lt 30; $i++) {
            $line = $process.StandardOutput.ReadLineAsync()
            if (-not $line.Wait(15000)) { throw "MCP did not answer $Method. Keep YouCraft open and retry." }
            if ($null -eq $line.Result) { throw 'MCP executable stopped before replying.' }
            $reply = $line.Result | ConvertFrom-Json
            if ($reply.error) { throw ($reply.error | ConvertTo-Json -Compress) }
            if ($reply.id -eq $Id) {
                return $reply.result
            }
        }
        throw 'MCP returned too many unrelated messages.'
    }
    $hello = Ask 1 'initialize' @{protocolVersion='2025-03-26';capabilities=@{};clientInfo=@{name='youcraft-check';version='0.1.3'}}
    if (-not $hello.serverInfo.name) { throw 'MCP initialization failed.' }
    Send '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    $tools = Ask 2 'tools/list' @{}
    if ('ui_control' -notin $tools.tools.name) { throw 'Podcast control tool is missing.' }
    if (-not $ToolsOnly) {
        Report 'Checking open episode'
        $result = Ask 3 'tools/call' @{name='ui_control';arguments=@{method='podcast.inspect';params=@{}}}
        if ($result.isError) { throw (($result.content | ForEach-Object { $_.text }) -join ' ') }
        $episode = $result.content[0].text | ConvertFrom-Json
        if ($null -eq $episode.step) { throw 'The running app did not return a podcast workflow.' }
    }
    Report 'Tools verified. Enable YouCraft in Claude chat.' 'succeeded'
} catch {
    Report 'Connection check failed' 'failed' $_.Exception.Message
    Write-Error $_ -ErrorAction Continue
    exit 1
} finally {
    if ($started -and -not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
    if ($process) { $process.Dispose() }
}
