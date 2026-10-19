# The stable installer AppId upgrades in place, preserving models and projects.
$ErrorActionPreference = 'Stop'
$installed = [version]'0.1.8'
$base = 'https://routsourav1729.github.io/youcraft'
$directory = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'YouCraft\updates'
$partial = Join-Path $directory 'YouCraft-Setup-x64.exe.partial'
$installer = Join-Path $directory 'YouCraft-Setup-x64.exe'
New-Item -ItemType Directory -Force -Path $directory | Out-Null
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$client = $null
$response = $null
$output = $null
try {
    Write-Host "YouCraft $installed - Checking for updates..."
    $manifest = Invoke-RestMethod -Uri "$base/update.json?check=$([DateTime]::UtcNow.Ticks)" -TimeoutSec 30
    $latest = [version]$manifest.version
    if ($latest -le $installed) {
        Write-Host 'YouCraft is up to date.'
        return
    }
    if ($manifest.sha256 -notmatch '^[a-fA-F0-9]{64}$' -or [long]$manifest.size -le 0 -or [long]$manifest.size -gt 1GB) {
        throw 'Invalid update metadata. Nothing was installed.'
    }
    Write-Host "Available: $latest ($([math]::Round($manifest.size / 1MB, 1)) MB)"
    if ((Read-Host 'Download update? [Y/N]') -notmatch '^[yY]') { return }
    # Fixed HTTPS origin: metadata cannot redirect execution to an arbitrary executable.
    Add-Type -AssemblyName System.Net.Http
    $client = New-Object Net.Http.HttpClient
    $client.Timeout = [TimeSpan]::FromMinutes(15)
    $response = $client.GetAsync("$base/YouCraft-Setup-x64.exe?v=$latest", [Net.Http.HttpCompletionOption]::ResponseHeadersRead).GetAwaiter().GetResult()
    $response.EnsureSuccessStatusCode() | Out-Null
    $inputStream = $response.Content.ReadAsStreamAsync().GetAwaiter().GetResult()
    $output = [IO.File]::Create($partial)
    $buffer = New-Object byte[] 262144
    $received = 0L
    $deadline = New-Object Threading.CancellationTokenSource
    $deadline.CancelAfter([TimeSpan]::FromMinutes(15))
    while (($count = $inputStream.ReadAsync($buffer, 0, $buffer.Length, $deadline.Token).GetAwaiter().GetResult()) -gt 0) {
        $received += $count
        if ($received -gt [long]$manifest.size) { throw 'Update size mismatch.' }
        $output.Write($buffer, 0, $count)
        Write-Progress -Activity "Downloading YouCraft $latest" -Status "$([math]::Round($received / 1MB, 1)) / $([math]::Round($manifest.size / 1MB, 1)) MB" -PercentComplete ([math]::Min(100, 100 * $received / $manifest.size))
    }
    $output.Dispose(); $output = $null
    Write-Progress -Activity 'Downloading YouCraft' -Completed
    if ($received -ne [long]$manifest.size -or (Get-FileHash $partial -Algorithm SHA256).Hash -ne $manifest.sha256) {
        throw 'Update verification failed. Nothing was installed; retry Updates.'
    }
    Move-Item $partial $installer -Force
    Write-Host 'Download verified. Save your project and close YouCraft before continuing.'
    if ((Read-Host 'Open installer now? [Y/N]') -match '^[yY]') {
        Start-Process -FilePath $installer -ArgumentList @('/NORESTART', ('/DIR="' + $PSScriptRoot + '"')) -Wait
    }
} catch {
    Write-Host "Update failed: $($_.Exception.Message)" -ForegroundColor Red
    Write-Host 'Your existing installation is unchanged. Retry Updates later.'
} finally {
    if ($output) { $output.Dispose() }
    if ($response) { $response.Dispose() }
    if ($client) { $client.Dispose() }
    if ($deadline) { $deadline.Dispose() }
    Remove-Item $partial -Force -ErrorAction SilentlyContinue
    Read-Host 'Press Enter to close' | Out-Null
}
