$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/../ama/Apply-FileUpdate.ps1"
$folder = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid().ToString())
$root = Join-Path $folder 'app'
$tx = Join-Path $folder 'transaction'
function Stage-Fixture {
    New-Item -ItemType Directory -Force "$tx/staged" | Out-Null
    Set-Content "$tx/staged/a.txt" 'new'
    Set-Content "$tx/staged/b.txt" 'new'
    return @('a.txt','b.txt') | ForEach-Object { @{path=$_;size=(Get-Item "$tx/staged/$_").Length;sha256=(Get-FileHash "$tx/staged/$_").Hash} }
}
try {
    New-Item -ItemType Directory -Force $root | Out-Null
    Set-Content "$root/a.txt" 'old'
    Set-Content "$root/b.txt" 'old'
    foreach ($unsafe in @('../outside','/absolute','C:/absolute','sub/../outside','bad:stream','a.txt.')) {
        $rejected = $false
        try { Get-SafeUpdatePath $root $unsafe | Out-Null } catch { $rejected = $true }
        if (-not $rejected) { throw "Accepted unsafe path $unsafe" }
    }
    $files = @(Stage-Fixture)
    Set-Content "$tx/staged/a.txt" 'tampered'
    $rejected = $false
    try { Install-FileUpdate $root $tx $files } catch { $rejected = $true }
    if (-not $rejected -or (Get-Content "$root/a.txt") -ne 'old') { throw 'Corrupt staging modified installation' }
    $files = @(Stage-Fixture)
    # A locked second destination forces an apply error after the first replacement.
    $handle = [IO.File]::Open("$root/b.txt",'Open','Read','Read')
    $rejected = $false
    try { Install-FileUpdate $root $tx $files } catch { $rejected = $true } finally { $handle.Dispose() }
    if (-not $rejected) { throw 'Expected locked-file failure' }
    Restore-FileUpdate $root $tx
    if ((Get-Content "$root/a.txt") -ne 'old' -or (Get-Content "$root/b.txt") -ne 'old') { throw 'Recovery failed' }
    $files = @(Stage-Fixture)
    Install-FileUpdate $root $tx $files
    if ((Get-Content "$root/a.txt") -ne 'new' -or (Test-Path $tx)) { throw 'Update or cleanup failed' }
    # Crash recovery uses the persisted journal, including files newly introduced by an update.
    New-Item -ItemType Directory -Force "$tx/backup" | Out-Null
    Copy-Item "$root/a.txt" "$tx/backup/a.txt"
    @(@{path='a.txt';existed=$true},@{path='new.txt';existed=$false}) | ConvertTo-Json | Set-Content "$tx/journal.json"
    Set-Content "$root/a.txt" 'interrupted'
    Set-Content "$root/new.txt" 'interrupted'
    Restore-FileUpdate $root $tx
    if ((Get-Content "$root/a.txt") -ne 'new' -or (Test-Path "$root/new.txt")) { throw 'Interrupted transaction recovery failed' }
    Write-Host 'File update: verified staging, path safety, locked-file rollback, interrupted recovery and cleanup passed.'
} finally { Remove-Item $folder -Recurse -Force -ErrorAction SilentlyContinue }
