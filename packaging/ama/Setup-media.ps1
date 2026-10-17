param(
    [string]$Root = (Join-Path $env:LOCALAPPDATA 'YouCraft'),
    [string]$StatusFile,
    [string]$Archive
)
$ErrorActionPreference = 'Stop'
$version = '9.0.2'
$checksum = '60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba'
$destination = Join-Path $Root "media/$version"
function MediaStatus([string]$Stage) {
    Write-Host $Stage
    if ($StatusFile) {
        [IO.File]::WriteAllText("$StatusFile.tmp", (@{stage=$Stage;state='running';error=''} | ConvertTo-Json -Compress), (New-Object Text.UTF8Encoding($false)))
        Move-Item "$StatusFile.tmp" $StatusFile -Force
    }
}
function CheckMedia([string]$Directory) {
    foreach ($name in @('ffmpeg','ffprobe')) {
        $exe = Join-Path $Directory "$name.exe"
        if (-not (Test-Path $exe)) { return $false }
        & $exe -version | Out-Null
        if ($LASTEXITCODE -ne 0) { return $false }
    }
    return $true
}
New-Item -ItemType Directory -Force $Root | Out-Null
if (-not (CheckMedia $destination)) {
    $downloads = Join-Path $Root 'downloads'
    New-Item -ItemType Directory -Force $downloads | Out-Null
    if (-not $Archive) {
        $Archive = Join-Path $downloads "ffmpeg-$version.zip"
        if (-not (Test-Path $Archive) -or (Get-FileHash $Archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $checksum) {
            $urls = @("https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-$version-essentials_build.zip", "https://github.com/GyanD/codexffmpeg/releases/download/$version/ffmpeg-$version-essentials_build.zip")
            $ok = $false
            foreach ($url in $urls) {
                try {
                    MediaStatus 'Downloading media tools'
                    $partial = "$Archive.partial"
                    if (Test-Path $partial) { Remove-Item $partial -Force }
                    if (Get-Command curl.exe -ErrorAction SilentlyContinue) {
                        $arguments = @('--fail','--location','--retry','2','--connect-timeout','20','--max-time','600','--output',"`"$partial`"",$url)
                        $download = Start-Process curl.exe -ArgumentList $arguments -NoNewWindow -PassThru
                        $null = $download.Handle
                        while (-not $download.HasExited) {
                            if (Test-Path $partial) { MediaStatus ("Downloading media tools - {0:N0} / 110 MB" -f ((Get-Item $partial).Length / 1MB)) }
                            Start-Sleep -Seconds 1
                        }
                        $download.WaitForExit()
                        if ($download.ExitCode -ne 0) { throw "Download failed ($($download.ExitCode)): $url" }
                    } else {
                        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
                        Invoke-WebRequest $url -UseBasicParsing -OutFile $partial -TimeoutSec 600
                    }
                    if ((Get-FileHash $partial -Algorithm SHA256).Hash.ToLowerInvariant() -ne $checksum) { throw 'Media download checksum mismatch.' }
                    Move-Item $partial $Archive -Force
                    $ok = $true
                    break
                } catch { Write-Host $_.Exception.Message }
            }
            if (-not $ok) { throw 'Media download failed on both sources. Check the connection and retry; installed speech packages are retained.' }
        }
    }
    if ((Get-FileHash $Archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $checksum) { throw 'Media archive checksum mismatch.' }
    MediaStatus 'Extracting media tools'
    $staging = Join-Path $downloads ('media-' + [guid]::NewGuid().ToString('N'))
    try {
        Expand-Archive $Archive $staging -Force
        $bin = Join-Path $staging "ffmpeg-$version-essentials_build/bin"
        if (-not (CheckMedia $bin)) { throw 'Downloaded media tools cannot run on this computer.' }
        New-Item -ItemType Directory -Force $destination | Out-Null
        Copy-Item "$bin/ffmpeg.exe","$bin/ffprobe.exe" $destination -Force
        Copy-Item "$staging/ffmpeg-$version-essentials_build/LICENSE" $destination -ErrorAction SilentlyContinue
    } finally { if (Test-Path $staging) { Remove-Item $staging -Recurse -Force } }
}
$settings = @{ffmpeg=(Join-Path $destination 'ffmpeg.exe');ffprobe=(Join-Path $destination 'ffprobe.exe')} | ConvertTo-Json
[IO.File]::WriteAllText((Join-Path $Root 'media-tools.json.tmp'), $settings, (New-Object Text.UTF8Encoding($false)))
Move-Item (Join-Path $Root 'media-tools.json.tmp') (Join-Path $Root 'media-tools.json') -Force
MediaStatus 'Media tools ready'
$global:LASTEXITCODE = 0
