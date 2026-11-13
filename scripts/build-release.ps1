<#
.SYNOPSIS
  Compila YT Downloader in release e crea setup, zip portabile e SHA256SUMS.txt in dist/.

.PARAMETER FetchTools
  Scarica yt-dlp e ffmpeg aggiornati invece di usare quelli in bin/ (usato dalla CI).

.EXAMPLE
  ./scripts/build-release.ps1
  ./scripts/build-release.ps1 -FetchTools
#>
param(
    [switch]$FetchTools,
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
Write-Host "== YT Downloader $version =="

$dist = Join-Path $root 'dist'
$stage = Join-Path $dist 'stage'
$tools = Join-Path $root 'bin'
if (Test-Path $dist) { Remove-Item -Recurse -Force $dist }
New-Item -ItemType Directory -Force (Join-Path $stage 'bin') | Out-Null

# ── Strumenti esterni ──
if ($FetchTools) {
    $tools = Join-Path $dist 'tools'
    New-Item -ItemType Directory -Force $tools | Out-Null
    Write-Host 'Download di yt-dlp...'
    Invoke-WebRequest 'https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe' -OutFile (Join-Path $tools 'yt-dlp.exe')
    Write-Host 'Download di ffmpeg...'
    $ffzip = Join-Path $dist 'ffmpeg.zip'
    Invoke-WebRequest 'https://github.com/yt-dlp/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip' -OutFile $ffzip
    Expand-Archive $ffzip -DestinationPath (Join-Path $dist 'ffmpeg')
    Get-ChildItem (Join-Path $dist 'ffmpeg') -Recurse -Include ffmpeg.exe, ffprobe.exe | Copy-Item -Destination $tools
}
foreach ($exe in 'yt-dlp.exe', 'ffmpeg.exe', 'ffprobe.exe') {
    $src = Join-Path $tools $exe
    if (-not (Test-Path $src)) { throw "Manca $src (usa -FetchTools oppure copia gli eseguibili in bin/)" }
    Copy-Item $src (Join-Path $stage 'bin')
}

# ── Build ──
if (-not $SkipBuild) {
    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw 'cargo build non riuscito' }
}
Copy-Item 'target\release\YTDownloader.exe' $stage

# ── Installer (Inno Setup) ──
$iscc = @(
    $env:ISCC,
    (Get-Command ISCC.exe -ErrorAction SilentlyContinue).Source,
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe"
) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if (-not $iscc) { throw 'Inno Setup 6 (ISCC.exe) non trovato' }
& $iscc /Q "/DAppVersion=$version" "/DStageDir=$stage" "/DOutputDir=$dist" (Join-Path $root 'installer\setup.iss')
if ($LASTEXITCODE -ne 0) { throw 'compilazione installer non riuscita' }

# ── Zip portabile ──
$portable = Join-Path $dist "YTDownloader-$version-win64"
Copy-Item $stage $portable -Recurse
$zip = Join-Path $dist "YTDownloader-$version-win64.zip"
Compress-Archive -Path $portable -DestinationPath $zip -CompressionLevel Optimal
Remove-Item -Recurse -Force $portable

# ── Checksum ──
$setup = Join-Path $dist "YTDownloader-Setup-$version.exe"
$lines = foreach ($f in $setup, $zip) {
    '{0}  {1}' -f (Get-FileHash $f -Algorithm SHA256).Hash.ToLower(), (Split-Path $f -Leaf)
}
[IO.File]::WriteAllText((Join-Path $dist 'SHA256SUMS.txt'), ($lines -join "`n") + "`n")

Write-Host ''
Get-ChildItem $dist -File | ForEach-Object { '{0,-40} {1,8:N1} MB' -f $_.Name, ($_.Length / 1MB) }
