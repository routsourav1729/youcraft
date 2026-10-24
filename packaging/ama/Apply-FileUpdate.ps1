# Shared, testable transaction engine. User data and model packs are never managed here.
function Get-SafeUpdatePath($Root, [string]$Relative) {
    if ($Relative -notmatch '^[a-zA-Z0-9_ .()/+\-]+$' -or $Relative -match '(^|/)\.\.?(/|$)|(^|/)[^/]*[ .](/|$)' -or [IO.Path]::IsPathRooted($Relative)) {
        throw "Unsafe update path: $Relative"
    }
    $prefix = [IO.Path]::GetFullPath($Root).TrimEnd('\') + '\'
    $full = [IO.Path]::GetFullPath((Join-Path $Root $Relative))
    if (-not $full.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) { throw 'Update path escaped installation' }
    $parent = $full
    while ($parent -and $parent.Length -ge $prefix.Length) {
        if ((Test-Path $parent) -and ((Get-Item $parent -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Update paths cannot be junctions or symlinks' }
        $parent = Split-Path $parent
    }
    return $full
}

function Restore-FileUpdate($Root, $Transaction) {
    $journal = Join-Path $Transaction 'journal.json'
    if (-not (Test-Path $journal)) { return }
    if (-not (Test-Path (Join-Path $Transaction 'committed'))) {
        $items = Get-Content $journal -Raw | ConvertFrom-Json
        foreach ($item in $items) {
            $target = Get-SafeUpdatePath $Root $item.path
            if ($item.existed) {
                Copy-Item (Get-SafeUpdatePath (Join-Path $Transaction 'backup') $item.path) $target -Force
            } elseif (Test-Path $target) { Remove-Item $target -Force }
        }
    }
    Remove-Item $Transaction -Recurse -Force
}

function Install-FileUpdate($Root, $Transaction, $Files, [switch]$AwaitHealth) {
    if (Test-Path (Join-Path $Transaction 'journal.json')) { throw 'Recover the previous transaction first' }
    $journal = @()
    foreach ($file in $Files) {
        $target = Get-SafeUpdatePath $Root $file.path
        $staged = Get-SafeUpdatePath (Join-Path $Transaction 'staged') $file.path
        if ($file.remove -ne $true) {
            if ((Get-Item $staged).Length -ne $file.size -or (Get-FileHash $staged -Algorithm SHA256).Hash -ne $file.sha256) { throw 'Staged update verification failed' }
        }
        $existed = Test-Path $target
        if ($existed) {
            $backup = Get-SafeUpdatePath (Join-Path $Transaction 'backup') $file.path
            New-Item -ItemType Directory -Force (Split-Path $backup) | Out-Null
            Copy-Item $target $backup -Force
        }
        $journal += @{path=$file.path; existed=$existed}
    }
    # All backups are complete before publishing the journal or changing any installed file.
    ConvertTo-Json -InputObject @($journal) -Depth 5 | Set-Content (Join-Path $Transaction 'journal.tmp') -Encoding UTF8
    Move-Item (Join-Path $Transaction 'journal.tmp') (Join-Path $Transaction 'journal.json') -Force
    try {
        foreach ($file in $Files) {
            $target = Get-SafeUpdatePath $Root $file.path
            if ($file.remove -eq $true) { if (Test-Path $target) { Remove-Item $target -Force } }
            else {
                New-Item -ItemType Directory -Force (Split-Path $target) | Out-Null
                Copy-Item (Get-SafeUpdatePath (Join-Path $Transaction 'staged') $file.path) $target -Force
            }
        }
        if (-not $AwaitHealth) { Set-Content (Join-Path $Transaction 'committed') 'ok' }
    } catch {
        Restore-FileUpdate $Root $Transaction
        throw
    }
    if (-not $AwaitHealth) { Restore-FileUpdate $Root $Transaction }
}
