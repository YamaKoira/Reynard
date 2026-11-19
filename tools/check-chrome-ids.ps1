# The Chrome add-on IDs written into the installers must be the ones in chrome/ids.txt.
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)
$ids = @{}
foreach ($line in Get-Content -LiteralPath 'chrome/ids.txt') {
    if ($line -match '^([A-Z_]+)=(.*)$') { $ids[$Matches[1]] = $Matches[2].Trim() }
}
$bad = @()
foreach ($f in 'scripts/install.sh', 'scripts/install-local.bat') {
    $t = Get-Content -Raw -LiteralPath $f
    if (-not $t.Contains($ids['RELEASE_ID'])) { $bad += $f }
}
if ($bad.Count -gt 0) {
    Write-Host ('STOP: these files do not contain the Chrome add-on IDs from chrome\ids.txt: ' + ($bad -join ', '))
    exit 1
}
