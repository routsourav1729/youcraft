$ErrorActionPreference = 'Stop'
$source = "$PSScriptRoot/../ama/Update-YouCraft.ps1"
$tokens = $null; $errors = $null
$ast = [System.Management.Automation.Language.Parser]::ParseFile($source, [ref]$tokens, [ref]$errors)
if ($errors.Count) { throw ($errors | Out-String) }
# Exercise the actual updater functions without its interactive entry point.
foreach ($name in @('Get-UpdateProcesses','Assert-AppClosed')) {
    $node = $ast.Find({param($n) $n -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq $name}, $true)
    Invoke-Expression $node.Extent.Text
}
$InstallDirectory = 'C:\YouCraft'
$global:closed = $false
$global:held = $false
$global:sleeps = 0
$global:answer = 'N'
$app = [pscustomobject]@{Path='C:\YouCraft\YouCraft.exe';ProcessName='YouCraft';Id=100}
$app | Add-Member ScriptMethod CloseMainWindow { $global:closed = $true; return $true }
$other = [pscustomobject]@{Path='C:\YouCraft-other\YouCraft.exe';ProcessName='YouCraft';Id=200}
function Get-Process { param($Name, $ErrorAction) if (-not $global:closed -or $global:held) { $app }; $other }
function Start-Sleep { param($Seconds) $global:sleeps++ }
function Read-Host { param($Prompt) return $global:answer }
Assert-AppClosed
if (-not $global:closed -or $global:sleeps -ne 0) { throw 'Normal close or unrelated-process filtering failed' }
$global:held = $true
$rejected = $false
try { Assert-AppClosed } catch { $rejected = $_.Exception.Message -like '*postponed*' }
if (-not $rejected -or $global:sleeps -ne 30) { throw 'Save cancellation must leave app running and postpone apply' }
# An orphan helper is stopped only after explicit consent and no editor is running.
$helper = [pscustomobject]@{Path='C:\YouCraft\BetunBackend\BetunBackend.exe';ProcessName='BetunBackend';Id=101}
$global:helperStopped = $false
function Get-Process { param($Name, $ErrorAction) if (-not $global:helperStopped) { $helper }; $other }
function Stop-Process { param([Parameter(ValueFromPipeline=$true)]$InputObject, $ErrorAction) process {
    if ($InputObject.Id -ne 101) { throw 'Attempted to stop unrelated process' }
    $global:helperStopped = $true
} }
$global:answer = 'Y'
Assert-AppClosed
if (-not $global:helperStopped) { throw 'Consented orphan helper cleanup failed' }
Remove-Item Function:Stop-Process
Remove-Item Function:Get-Process
# End-to-end: a previous attempt's verified staged download must survive cancellation,
# then install successfully on retry without any HTTP file download.
$folder = Join-Path $env:TEMP ([Guid]::NewGuid().ToString())
$root = Join-Path $folder 'app'
$tx = Join-Path $folder 'updates/transaction'
try {
    New-Item -ItemType Directory -Force "$tx/staged",$root | Out-Null
    Set-Content "$root/installed-files.json" '{"version":"0.1.10","files":[]}'
    Set-Content "$root/a.txt" 'old'
    Set-Content "$tx/staged/a.txt" 'new'
    $file = @{path='a.txt';size=(Get-Item "$tx/staged/a.txt").Length;sha256=(Get-FileHash "$tx/staged/a.txt").Hash.ToLowerInvariant();download_sha256=('a'*64);download_size=10}
    $release = @{version='0.1.11';files=@($file)}
    function Invoke-RestMethod { param($Uri,$TimeoutSec) return $release }
    $global:apply = 'N'
    function Read-Host { param($Prompt) if ($Prompt -like 'Apply*') { return $global:apply }; return 'Y' }
    # Redirect only the cache location. Production logic, hashing and transaction code stay intact.
    $script = (Get-Content $source -Raw).Replace("`$directory = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'YouCraft\updates'", "`$directory = '$folder\updates'")
    $script = $script.Replace(". (Join-Path `$PSScriptRoot 'Apply-FileUpdate.ps1')", ". '$PSScriptRoot/../ama/Apply-FileUpdate.ps1'")
    $script = $script.Replace('Receive-VerifiedFile "$base/files/$($file.sha256).gz" $packed $file.download_size $file.download_sha256', "throw 'Unexpected network download: verified staging was not reused'")
    $entry = [scriptblock]::Create($script)
    & $entry -InstallDirectory $root
    if (-not (Test-Path "$tx/staged/a.txt") -or (Get-Content "$root/a.txt") -ne 'old') { throw 'Postponed apply discarded downloads or changed installation' }
    $global:apply = 'Y'
    & $entry -InstallDirectory $root
    if ((Get-Content "$root/a.txt") -ne 'new' -or (Test-Path $tx)) { throw 'Cached retry failed to install or clean up' }
    Write-Host 'PASS: graceful close, unrelated process filtering, cancelled exit, cached retry and successful cleanup.'
} finally { Remove-Item $folder -Recurse -Force -ErrorAction SilentlyContinue }
