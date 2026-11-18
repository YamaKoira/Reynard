# Makes the Chrome (Manifest V3) version of the add-on from the same sources as the Firefox one.
#   -Mode dev      "Reynard (dev)": its own ID, talks to the dev copy of Reynard (used by setup.bat)
#   -Mode release  the real add-on with the release ID, to load unpacked (Developer mode)
#   -Mode store    the same without a "key": the zip for the Chrome Web Store (the store gives the ID)
param(
    [Parameter(Mandatory = $true)][string]$Out,
    [Parameter(Mandatory = $true)][ValidateSet('dev', 'release', 'store')][string]$Mode
)
$ErrorActionPreference = 'Stop'
$Out = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Out)
Set-Location (Split-Path -Parent $PSScriptRoot)

$ids = @{}
foreach ($line in Get-Content -LiteralPath 'chrome/ids.txt') {
    if ($line -match '^([A-Z_]+)=(.*)$') { $ids[$Matches[1]] = $Matches[2].Trim() }
}
foreach ($k in 'RELEASE_ID', 'RELEASE_KEY', 'DEV_ID', 'DEV_KEY') {
    if (-not $ids[$k]) { throw "chrome/ids.txt is missing $k" }
}
$version = (Get-Content -Raw -LiteralPath 'extension/manifest.json' | ConvertFrom-Json).version
if (-not $version) { throw 'no version found in extension/manifest.json' }

if (Test-Path -LiteralPath $Out) { Remove-Item -Recurse -Force -LiteralPath $Out }
Copy-Item -Recurse -LiteralPath 'extension' -Destination $Out
Copy-Item -LiteralPath 'chrome/service-worker.js' -Destination (Join-Path $Out 'service-worker.js')

$m = (Get-Content -Raw -LiteralPath 'chrome/manifest.json').Replace('__VERSION__', $version)
$utf8 = New-Object Text.UTF8Encoding $false
switch ($Mode) {
    'dev' {
        $m = $m.Replace('__KEY__', $ids['DEV_KEY']).Replace('"name": "Reynard"', '"name": "Reynard (dev)"').Replace('"default_title": "Reynard"', '"default_title": "Reynard (dev)"')
        [IO.File]::WriteAllText((Join-Path $Out 'config.js'), "var REYNARD_HOST = `"io.reynard.host.dev`";`nvar REYNARD_LABEL = `"Reynard (dev)`";`n", $utf8)
    }
    'release' { $m = $m.Replace('__KEY__', $ids['RELEASE_KEY']) }
    'store' { $m = (($m -split "`r?`n") | Where-Object { $_ -notmatch '"key":' }) -join "`n" }
}
# sanity checks
if ($m -match '__[A-Z]+__') { throw 'an unfilled placeholder is left in manifest.json' }
if ($m -notmatch '"manifest_version": 3') { throw 'not a Manifest V3 file' }
if ($Mode -eq 'store') {
    if ($m -match '"key"') { throw 'the store build must not contain a key' }
} elseif ($m -notmatch '"key": "MII') {
    throw 'the key was not put into manifest.json'
}
[IO.File]::WriteAllText((Join-Path $Out 'manifest.json'), $m, $utf8)
Write-Host "Chrome add-on ($Mode) ready in $Out"
