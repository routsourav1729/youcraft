$ErrorActionPreference = 'Stop'
try {
    $client = Read-Host 'Connect which installed client? [codex/claude]'
    if ($client -notin @('codex','claude')) { throw 'Choose codex or claude.' }
    if (-not (Get-Command $client -ErrorAction SilentlyContinue)) { throw "Install and sign into $client first, then retry." }
    $cli = Join-Path $PSScriptRoot 'filmcraft-cli.exe'
    if ($client -eq 'codex') {
        & codex mcp add youcraft -- $cli mcp --bridge 127.0.0.1:9876
    } else {
        & claude mcp add --transport stdio --scope user youcraft -- $cli mcp --bridge 127.0.0.1:9876
    }
    if ($LASTEXITCODE -ne 0) { throw 'Registration failed. If youcraft already exists, inspect the client MCP settings before replacing it.' }
    Write-Host 'Close YouCraft, open YouCraft with AI from Start, then restart your AI client.'
    Write-Host 'Ask it to read AI-WORKFLOW.md from this installation folder.'
} catch { Write-Error $_ -ErrorAction Continue; exit 1 }
