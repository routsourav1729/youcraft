param(
    [ValidateSet('Prepare','Apply')][string]$Mode = 'Prepare',
    [string]$InstallDirectory = $PSScriptRoot,
    [int]$EditorPid = 0,
    [string]$UpdateDirectory = (Join-Path $env:LOCALAPPDATA 'YouCraft\updates'),
    [string]$Feed = 'https://routsourav1729.github.io/youcraft'
)
$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$InstallDirectory = [IO.Path]::GetFullPath($InstallDirectory).TrimEnd('\')
$UpdateDirectory = [IO.Path]::GetFullPath($UpdateDirectory).TrimEnd('\')
$transaction = Join-Path $UpdateDirectory 'background-transaction'
$lock = $null
$client = $null
. (Join-Path $PSScriptRoot 'Apply-FileUpdate.ps1')
New-Item -ItemType Directory -Force $UpdateDirectory | Out-Null
function Write-Status([string]$State, [string]$Message) {
    @{state=$State;message=$Message} | ConvertTo-Json | Set-Content (Join-Path $UpdateDirectory 'status.tmp') -Encoding UTF8
    Move-Item -LiteralPath (Join-Path $UpdateDirectory 'status.tmp') -Destination (Join-Path $UpdateDirectory 'status.json') -Force
}
function Assert-TransactionPath {
    if ([IO.Path]::GetFullPath($transaction) -ne (Join-Path $UpdateDirectory 'background-transaction')) { throw 'Invalid transaction directory' }
    if ((Test-Path $transaction) -and ((Get-Item $transaction -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Transaction cannot be a link' }
}
function Remove-Transaction {
    Assert-TransactionPath
    if (Test-Path $transaction) { Remove-Item -LiteralPath $transaction -Recurse -Force }
}
function Installation-Processes {
    @(Get-Process YouCraft,BetunBackend,filmcraft-cli -ErrorAction SilentlyContinue) | Where-Object {
        $_.Path -and $_.Path.StartsWith($InstallDirectory + '\', [StringComparison]::OrdinalIgnoreCase)
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
    $lock = [IO.File]::Open((Join-Path $UpdateDirectory 'update.lock'), 'OpenOrCreate', 'ReadWrite', 'None')
    Assert-TransactionPath
    if ($Mode -eq 'Apply') {
        $ready = Get-Content (Join-Path $transaction 'ready.json') -Raw | ConvertFrom-Json
        if ($ready.install -ne $InstallDirectory) { throw 'Prepared update belongs to another installation' }
        if (-not (Test-Path (Join-Path $UpdateDirectory 'resume.json'))) { throw 'Save acknowledgement is missing' }
        Write-Status 'closing' 'Waiting for the saved editor to close…'
        # Never close/kill an editor: it has explicitly saved and requested its own normal exit.
        for ($i=0; $i -lt 90; $i++) {
            if (-not (Get-Process -Id $EditorPid -ErrorAction SilentlyContinue)) { break }
            Start-Sleep -Seconds 1
        }
        if (Get-Process -Id $EditorPid -ErrorAction SilentlyContinue) { throw 'Editor did not close. Downloads are retained; finish the save dialog and retry.' }
        $remaining = @(Installation-Processes)
        if (@($remaining | Where-Object ProcessName -eq 'YouCraft').Count) { throw 'Another YouCraft window is open. Close it before installing.' }
        # Idle helper/bridge processes from this installation may outlive the saved editor.
        $remaining | Stop-Process -ErrorAction Stop
        foreach ($process in $remaining) { $process.WaitForExit(10000) | Out-Null }
        if (Test-Path (Join-Path $transaction 'journal.json')) {
            Restore-FileUpdate $InstallDirectory $transaction
            $env:YOUCRAFT_UPDATE_RESUME = Join-Path $UpdateDirectory 'resume.json'
            Start-Process (Join-Path $InstallDirectory 'YouCraft.exe') -WorkingDirectory $InstallDirectory | Out-Null
            Write-Status 'current' 'The previous version was restored. Your saved session is available.'
            return
        }
        Write-Status 'applying' 'Installing the downloaded update…'
        Install-FileUpdate $InstallDirectory $transaction @($ready.files) -AwaitHealth
        $health = Join-Path $transaction ('healthy-' + [Guid]::NewGuid().ToString('N'))
        $env:YOUCRAFT_UPDATE_RESUME = Join-Path $UpdateDirectory 'resume.json'
        $env:YOUCRAFT_UPDATE_HEALTH = $health
        $app = Start-Process (Join-Path $InstallDirectory 'YouCraft.exe') -WorkingDirectory $InstallDirectory -PassThru
        for ($i=0; $i -lt 90; $i++) {
            if (Test-Path $health) {
                Set-Content (Join-Path $transaction 'committed') 'healthy'
                Restore-FileUpdate $InstallDirectory $transaction
                Write-Status 'current' "YouCraft $($ready.version) is up to date. Your session was restored."
                return
            }
            if ($app.HasExited) {
                Restore-FileUpdate $InstallDirectory $transaction
                Remove-Item Env:YOUCRAFT_UPDATE_HEALTH -ErrorAction SilentlyContinue
                Start-Process (Join-Path $InstallDirectory 'YouCraft.exe') -WorkingDirectory $InstallDirectory | Out-Null
                throw 'The updated app exited before confirming startup. Previous files were restored; your saved session is retained.'
            }
            Start-Sleep -Seconds 1
        }
        throw 'Startup confirmation is taking longer than expected. Backup and saved session are retained; the running app has not been stopped.'
    }
    if (Test-Path (Join-Path $transaction 'journal.json')) {
        if (@(Get-ChildItem $transaction -Filter 'healthy-*' -File).Count) {
            Set-Content (Join-Path $transaction 'committed') 'healthy'
            Restore-FileUpdate $InstallDirectory $transaction
        } else {
            Write-Status 'recovery' 'The previous update did not confirm startup. Restart to restore the previous version and your saved session.'
            return
        }
    }
    Write-Status 'checking' 'Checking for updates…'
    $installed = [version](Get-Content (Join-Path $InstallDirectory 'installed-files.json') -Raw | ConvertFrom-Json).version
    $manifest = Invoke-RestMethod "$Feed/update.json?check=$([DateTime]::UtcNow.Ticks)" -TimeoutSec 30
    if ($manifest.version -notmatch '^\d+\.\d+\.\d+$') { throw 'Invalid version metadata' }
    $latest = [version]$manifest.version
    if ($latest -le $installed) { Write-Status 'current' "YouCraft $installed is up to date."; return }
    $release = Invoke-RestMethod "$Feed/files-$latest.json" -TimeoutSec 30
    if ($release.version -ne $manifest.version -or @($release.files).Count -eq 0 -or @($release.files).Count -gt 10000) { throw 'Invalid file manifest' }
    if (Test-Path (Join-Path $transaction 'version.txt')) {
        if ((Get-Content (Join-Path $transaction 'version.txt') -Raw).Trim() -ne "$latest") { Remove-Transaction }
    }
    New-Item -ItemType Directory -Force (Join-Path $transaction 'staged') | Out-Null
    Set-Content (Join-Path $transaction 'version.txt') "$latest"
    $changed = @(); $seen = @{}; $total = 0L
    foreach ($file in $release.files) {
        $path = Get-SafeUpdatePath $InstallDirectory $file.path
        if ($seen.ContainsKey($file.path) -or $file.path -eq 'installed-files.json' -or $file.sha256 -notmatch '^[a-f0-9]{64}$' -or $file.download_sha256 -notmatch '^[a-f0-9]{64}$' -or [long]$file.size -lt 0 -or [long]$file.size -gt 1GB -or [long]$file.download_size -le 0 -or [long]$file.download_size -gt 1GB) { throw 'Invalid file metadata' }
        $seen[$file.path] = $true
        if ((Test-Path $path -PathType Leaf) -and (Get-Item $path).Length -eq $file.size -and (Get-FileHash $path -Algorithm SHA256).Hash -eq $file.sha256) { continue }
        $changed += $file; $total += [long]$file.download_size
    }
    if ($total -gt 2GB) { throw 'Update exceeds supported size' }
    Add-Type -AssemblyName System.Net.Http
    $client = New-Object Net.Http.HttpClient
    $client.Timeout = [TimeSpan]::FromMinutes(15)
    $index = 0
    foreach ($file in $changed) {
        $index++
        Write-Status 'downloading' "Downloading YouCraft $latest in the background ($index/$($changed.Count) files, $([math]::Round($total/1MB,1)) MB total)…"
        $target = Get-SafeUpdatePath (Join-Path $transaction 'staged') $file.path
        if ((Test-Path $target -PathType Leaf) -and (Get-Item $target).Length -eq $file.size -and (Get-FileHash $target -Algorithm SHA256).Hash -eq $file.sha256) { continue }
        $packed = Join-Path $transaction 'download.gz'
        Receive-VerifiedFile "$Feed/files/$($file.sha256).gz" $packed $file.download_size $file.download_sha256
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
        if ((Get-Item $target).Length -ne $file.size -or (Get-FileHash $target -Algorithm SHA256).Hash -ne $file.sha256) { throw 'Expanded update checksum mismatch' }
    }
    foreach ($old in @((Get-Content (Join-Path $InstallDirectory 'installed-files.json') -Raw | ConvertFrom-Json).files)) {
        if (-not $seen.ContainsKey($old.path)) { Get-SafeUpdatePath $InstallDirectory $old.path | Out-Null; $changed += @{path=$old.path;remove=$true} }
    }
    $record = Join-Path $transaction 'staged/installed-files.json'
    $release | ConvertTo-Json -Depth 8 | Set-Content $record -Encoding UTF8
    $changed += @{path='installed-files.json';size=(Get-Item $record).Length;sha256=(Get-FileHash $record -Algorithm SHA256).Hash}
    @{install=$InstallDirectory;version="$latest";files=@($changed)} | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $transaction 'ready.json') -Encoding UTF8
    Write-Status 'ready' "YouCraft $latest is ready. Restart to update, or keep working and install later."
} catch {
    Write-Status 'error' $_.Exception.Message
    exit 1
} finally {
    if ($client) { $client.Dispose() }
    if ($lock) { $lock.Dispose() }
}
