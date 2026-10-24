param([string]$InstallDirectory = $PSScriptRoot)
$ErrorActionPreference = 'Stop'
$installed = [version]'0.1.11'
$InstallDirectory = [IO.Path]::GetFullPath($InstallDirectory).TrimEnd('\')
$installedRecord = Join-Path $InstallDirectory 'installed-files.json'
if (Test-Path $installedRecord) {
    $installed = [version](Get-Content $installedRecord -Raw | ConvertFrom-Json).version
}
$base = 'https://routsourav1729.github.io/youcraft'
$directory = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'YouCraft\updates'
$transaction = Join-Path $directory 'transaction'
$client = $null
$lock = $null
New-Item -ItemType Directory -Force $directory | Out-Null
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
. (Join-Path $PSScriptRoot 'Apply-FileUpdate.ps1')

function Get-UpdateProcesses {
    @(Get-Process YouCraft,BetunBackend,filmcraft-cli -ErrorAction SilentlyContinue) | Where-Object {
        $_.Path -and $_.Path.StartsWith($InstallDirectory + '\', [StringComparison]::OrdinalIgnoreCase)
    }
}
function Assert-AppClosed {
    # WM_CLOSE follows the application's normal save/exit path. Never force-kill the editor.
    foreach ($process in @(Get-UpdateProcesses)) {
        if ($process.ProcessName -eq 'YouCraft') { $process.CloseMainWindow() | Out-Null }
    }
    Write-Host 'Waiting for YouCraft to close. Finish any save prompt in the app.'
    while ($true) {
        for ($attempt = 0; $attempt -lt 30; $attempt++) {
            $running = @(Get-UpdateProcesses)
            if ($running.Count -eq 0) { return }
            Start-Sleep -Seconds 1
        }
        Write-Host ('Still running: ' + (($running | ForEach-Object { "$($_.ProcessName) (PID $($_.Id))" }) -join ', '))
        Write-Host 'Close these from YouCraft or Task Manager after saving. Your downloaded files are kept.'
        if ((Read-Host 'Check again? [Y/N]') -notmatch '^[yY]') {
            throw 'Installation postponed. Downloads are saved for the next attempt.'
        }
    }
}
function Receive-VerifiedFile($Url, $Destination, [long]$Size, [string]$Hash) {
    $response = $null; $output = $null; $inputStream = $null
    $deadline = New-Object Threading.CancellationTokenSource
    $deadline.CancelAfter([TimeSpan]::FromMinutes(15))
    try {
        $response = $client.GetAsync($Url, [Net.Http.HttpCompletionOption]::ResponseHeadersRead).GetAwaiter().GetResult()
        $response.EnsureSuccessStatusCode() | Out-Null
        $inputStream = $response.Content.ReadAsStreamAsync().GetAwaiter().GetResult()
        $output = [IO.File]::Create($Destination)
        $buffer = New-Object byte[] 262144
        $received = 0L
        while (($count = $inputStream.ReadAsync($buffer, 0, $buffer.Length, $deadline.Token).GetAwaiter().GetResult()) -gt 0) {
            $received += $count
            if ($received -gt $Size) { throw 'Update download exceeds expected size' }
            $output.Write($buffer, 0, $count)
            Write-Progress -Activity 'Downloading changed files' -Status "$([math]::Round($received / 1MB, 1)) MB of current file" -PercentComplete ([math]::Min(100,100 * $received / [math]::Max(1,$Size)))
        }
        $output.Dispose(); $output = $null
        if ($received -ne $Size -or (Get-FileHash $Destination -Algorithm SHA256).Hash -ne $Hash) { throw 'Update checksum mismatch' }
    } finally {
        if ($output) { $output.Dispose() }
        if ($inputStream) { $inputStream.Dispose() }
        if ($response) { $response.Dispose() }
        $deadline.Dispose()
    }
}
try {
    $lock = [IO.File]::Open((Join-Path $directory 'update.lock'), 'OpenOrCreate', 'ReadWrite', 'None')
    if (Test-Path (Join-Path $transaction 'journal.json')) {
        Assert-AppClosed
        Restore-FileUpdate $InstallDirectory $transaction
        Write-Host 'Recovered the previous update. Please run Updates again.'
        return
    }
    # Old updater reused one installer filename; remove that completed-download cache.
    Remove-Item (Join-Path $directory 'YouCraft-Setup-x64.exe') -Force -ErrorAction SilentlyContinue
    Remove-Item (Join-Path $directory 'YouCraft-Setup-x64.exe.partial') -Force -ErrorAction SilentlyContinue
    Write-Host "YouCraft $installed - Checking for updates..."
    $manifest = Invoke-RestMethod "$base/update.json?check=$([DateTime]::UtcNow.Ticks)" -TimeoutSec 30
    if ($manifest.version -notmatch '^\d+\.\d+\.\d+$') { throw 'Invalid version metadata' }
    $latest = [version]$manifest.version
    if ($latest -le $installed) { Write-Host 'YouCraft is up to date.'; return }
    $release = Invoke-RestMethod "$base/files-$latest.json" -TimeoutSec 30
    if ($release.version -ne $manifest.version -or @($release.files).Count -eq 0 -or @($release.files).Count -gt 10000) { throw 'Invalid file manifest' }
    $changed = @(); $seen = @{}; $total = 0L
    foreach ($file in $release.files) {
        $path = Get-SafeUpdatePath $InstallDirectory $file.path
        if ($seen.ContainsKey($file.path) -or $file.path -eq 'installed-files.json' -or $file.sha256 -notmatch '^[a-f0-9]{64}$' -or $file.download_sha256 -notmatch '^[a-f0-9]{64}$' -or [long]$file.size -lt 0 -or [long]$file.size -gt 1GB -or [long]$file.download_size -le 0 -or [long]$file.download_size -gt 1GB) { throw 'Invalid file metadata' }
        $seen[$file.path] = $true
        if ((Test-Path $path -PathType Leaf) -and (Get-Item $path).Length -eq $file.size -and (Get-FileHash $path -Algorithm SHA256).Hash -eq $file.sha256) { continue }
        $changed += $file
        $total += [long]$file.download_size
    }
    if ($total -gt 2GB) { throw 'Update exceeds supported size' }
    Write-Host "$installed -> $latest : $($changed.Count) changed files, $([math]::Round($total / 1MB, 1)) MB. Models and projects are kept."
    if ((Read-Host 'Download changed files? [Y/N]') -notmatch '^[yY]') { return }
    New-Item -ItemType Directory -Force (Join-Path $transaction 'staged') | Out-Null
    Add-Type -AssemblyName System.Net.Http
    $client = New-Object Net.Http.HttpClient
    $client.Timeout = [TimeSpan]::FromMinutes(15)
    foreach ($file in $changed) {
        $target = Get-SafeUpdatePath (Join-Path $transaction 'staged') $file.path
        if ((Test-Path $target -PathType Leaf) -and (Get-Item $target).Length -eq $file.size -and (Get-FileHash $target -Algorithm SHA256).Hash -eq $file.sha256) {
            Write-Host "Reusing verified download: $($file.path)"
            continue
        }
        $packed = Join-Path $transaction 'download.gz'
        Receive-VerifiedFile "$base/files/$($file.sha256).gz" $packed $file.download_size $file.download_sha256
        $target = Get-SafeUpdatePath (Join-Path $transaction 'staged') $file.path
        New-Item -ItemType Directory -Force (Split-Path $target) | Out-Null
        $inputFile = [IO.File]::OpenRead($packed)
        $gzip = New-Object IO.Compression.GZipStream($inputFile, [IO.Compression.CompressionMode]::Decompress)
        $output = [IO.File]::Create($target)
        try {
            $buffer = New-Object byte[] 262144; $expanded = 0L
            while (($count = $gzip.Read($buffer,0,$buffer.Length)) -gt 0) {
                $expanded += $count
                if ($expanded -gt [long]$file.size) { throw 'Expanded update exceeds expected size' }
                $output.Write($buffer,0,$count)
            }
        } finally { $output.Dispose(); $gzip.Dispose(); $inputFile.Dispose() }
    }
    Write-Progress -Activity 'Downloading changed files' -Completed
    # Only files listed in our previous manifest can be removed.
    $previous = Join-Path $InstallDirectory 'installed-files.json'
    if (Test-Path $previous) {
        foreach ($old in @((Get-Content $previous -Raw | ConvertFrom-Json).files)) {
            if (-not $seen.ContainsKey($old.path)) { $changed += @{path=$old.path; remove=$true} }
        }
    }
    $record = Join-Path $transaction 'staged/installed-files.json'
    $release | ConvertTo-Json -Depth 8 | Set-Content $record -Encoding UTF8
    $changed += @{path='installed-files.json'; size=(Get-Item $record).Length; sha256=(Get-FileHash $record -Algorithm SHA256).Hash}
    Write-Host 'Download complete. Save your project. Apply will request a normal YouCraft close and wait for its helper.'
    if ((Read-Host 'Apply update? [Y/N]') -notmatch '^[yY]') { return }
    Assert-AppClosed
    Install-FileUpdate $InstallDirectory $transaction $changed
    Write-Host "Updated to $latest. You can reopen YouCraft. Download and backup files have been removed."
} catch {
    Write-Host "Update stopped: $($_.Exception.Message)" -ForegroundColor Red
    Write-Host 'If interrupted during installation, close YouCraft and run Updates again to recover. A full installer is also available from the download page.'
} finally {
    if ($client) { $client.Dispose() }
    if ($lock) {
        $lock.Dispose()
    }
    Read-Host 'Press Enter to close' | Out-Null
}
