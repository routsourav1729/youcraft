$ErrorActionPreference = 'Stop'
$folder = Join-Path $env:TEMP ('youcraft-update-test-' + [Guid]::NewGuid().ToString('N'))
$root = Join-Path $folder 'app'
$updates = Join-Path $folder 'updates'
$tx = Join-Path $updates 'background-transaction'
try {
    New-Item -ItemType Directory -Force "$tx/staged",$root | Out-Null
    Set-Content "$root/installed-files.json" '{"version":"0.1.11","files":[]}'
    Set-Content "$root/a.txt" 'old'
    Set-Content "$tx/staged/a.txt" 'new'
    $file = @{path='a.txt';size=(Get-Item "$tx/staged/a.txt").Length;sha256=(Get-FileHash "$tx/staged/a.txt").Hash.ToLowerInvariant();download_sha256=('a'*64);download_size=10}
    $release = @{version='0.1.12';files=@($file)}
    function Invoke-RestMethod { param($Uri,$TimeoutSec) return $release }
    $source = (Get-Content "$PSScriptRoot/../ama/Background-Update.ps1" -Raw).Replace(". (Join-Path `$PSScriptRoot 'Apply-FileUpdate.ps1')", ". '$PSScriptRoot/../ama/Apply-FileUpdate.ps1'")
    $source = $source.Replace('Receive-VerifiedFile "$Feed/files/$($file.sha256).gz" $packed $file.download_size $file.download_sha256', "throw 'Unexpected network request for verified cached bytes'")
    $entry = [scriptblock]::Create($source)
    & $entry -InstallDirectory $root -UpdateDirectory $updates -Mode Prepare
    $state = Get-Content "$updates/status.json" -Raw | ConvertFrom-Json
    if ($state.state -ne 'ready' -or (Get-Content "$root/a.txt") -ne 'old') { throw 'Background prepare must not change the installed app' }
    & $entry -InstallDirectory $root -UpdateDirectory $updates -Mode Prepare
    if (-not (Test-Path "$tx/staged/a.txt")) { throw 'Retry lost verified download' }
    . "$PSScriptRoot/../ama/Apply-FileUpdate.ps1"
    $ready = Get-Content "$tx/ready.json" -Raw | ConvertFrom-Json
    Install-FileUpdate $root $tx @($ready.files) -AwaitHealth
    if ((Get-Content "$root/a.txt") -ne 'new' -or -not (Test-Path "$tx/backup/a.txt") -or (Test-Path "$tx/committed")) { throw 'Backup must remain until startup confirmation' }
    Restore-FileUpdate $root $tx
    if ((Get-Content "$root/a.txt") -ne 'old') { throw 'Unhealthy update did not roll back' }
    New-Item -ItemType Directory -Force "$tx/staged" | Out-Null
    Set-Content "$tx/staged/a.txt" 'new'
    Install-FileUpdate $root $tx @($file) -AwaitHealth
    Set-Content "$tx/committed" 'healthy'
    Restore-FileUpdate $root $tx
    if ((Get-Content "$root/a.txt") -ne 'new' -or (Test-Path $tx)) { throw 'Healthy update did not clean backup' }
    Write-Host 'PASS: background preparation, cached retry, retained backup, startup rollback and healthy cleanup.'
} finally {
    $resolved = [IO.Path]::GetFullPath($folder)
    if (-not $resolved.StartsWith([IO.Path]::GetFullPath($env:TEMP).TrimEnd('\') + '\youcraft-update-test-')) { throw 'Unsafe fixture cleanup' }
    Remove-Item -LiteralPath $resolved -Recurse -Force -ErrorAction SilentlyContinue
}
