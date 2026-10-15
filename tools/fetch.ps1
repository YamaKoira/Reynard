# Downloads yt-dlp.exe and ffmpeg into dist\bin (next to reynard.exe), if missing.
param([string]$Bin = "dist\bin")
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"   # much faster downloads
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
New-Item -ItemType Directory -Force -Path $Bin | Out-Null

$ytdlp = Join-Path $Bin "yt-dlp.exe"
if (-not (Test-Path $ytdlp)) {
    Write-Host "Downloading yt-dlp.exe ..."
    Invoke-WebRequest -UseBasicParsing "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe" -OutFile $ytdlp
}

$ffmpeg = Join-Path $Bin "ffmpeg.exe"
if (-not (Test-Path $ffmpeg)) {
    Write-Host "Downloading ffmpeg (100-150 MB, this takes a minute) ..."
    $zip = Join-Path $Bin "_ffmpeg.zip"
    $tmp = Join-Path $Bin "_ffmpeg"
    Invoke-WebRequest -UseBasicParsing "https://github.com/yt-dlp/FFmpeg-Builds/releases/latest/download/ffmpeg-master-latest-win64-gpl.zip" -OutFile $zip
    if (Test-Path $tmp) { Remove-Item -Recurse -Force $tmp }
    Expand-Archive -Path $zip -DestinationPath $tmp -Force
    foreach ($name in "ffmpeg.exe", "ffprobe.exe") {
        $f = Get-ChildItem -Path $tmp -Recurse -Filter $name | Select-Object -First 1
        Copy-Item $f.FullName (Join-Path $Bin $name)
    }
    Remove-Item -Recurse -Force $tmp, $zip
}
Write-Host "Tools ready in $Bin"
