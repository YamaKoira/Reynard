# Makes the development copy of the add-on: the same code, but with its own ID, its
# own name, and talking to the dev copy of Reynard. Output: dist-dev\extension
param(
    [string]$Out = "dist-dev\extension",
    [Parameter(Mandatory = $true)][string]$ExtId,
    [Parameter(Mandatory = $true)][string]$HostName
)
$ErrorActionPreference = "Stop"

if (Test-Path $Out) { Remove-Item -Recurse -Force $Out }
Copy-Item -Path "extension" -Destination $Out -Recurse

# 1. Which Reynard it talks to, and what it is called on screen.
$config = "var REYNARD_HOST = `"$HostName`";`r`nvar REYNARD_LABEL = `"Reynard (dev)`";`r`n"
[IO.File]::WriteAllText((Join-Path $Out "config.js"), $config, (New-Object Text.UTF8Encoding $false))

# 2. Its own identity in the manifest. Plain text replacement, so nothing else changes.
$path = (Resolve-Path (Join-Path $Out "manifest.json")).Path
$text = [IO.File]::ReadAllText($path)
$new = $text.Replace('"name": "Reynard"', '"name": "Reynard (dev)"').Replace('"default_title": "Reynard"', '"default_title": "Reynard (dev)"')
$idMatch = [regex]::Match($new, '"id":\s*"[^"]+"')
if (-not $idMatch.Success) { throw "no add-on id found in manifest.json" }
$new = $new.Replace($idMatch.Value, '"id": "' + $ExtId + '"')
if ($new -eq $text) { throw "manifest.json was not changed" }
[IO.File]::WriteAllText($path, $new, (New-Object Text.UTF8Encoding $false))

Write-Host "Dev add-on ready in $Out"
