@echo off
rem Builds Reynard, fetches yt-dlp + ffmpeg, and registers the native host with Firefox.
setlocal
cd /d "%~dp0"

echo == Building Reynard ==
cargo build --release
if errorlevel 1 goto :fail

if not exist dist mkdir dist
copy /y target\release\reynard.exe dist\reynard.exe >nul
if errorlevel 1 goto :locked

if exist dist\bin\yt-dlp.exe if exist dist\bin\ffmpeg.exe goto :manifest
echo == Fetching yt-dlp and ffmpeg ==
powershell -NoProfile -ExecutionPolicy Bypass -File tools\fetch.ps1
if errorlevel 1 goto :fail

:manifest
echo == Writing native messaging manifest ==
powershell -NoProfile -Command "$exe = (Resolve-Path 'dist\reynard.exe').Path; $m = [ordered]@{ name='io.reynard.host'; description='Reynard native host'; path=$exe; type='stdio'; allowed_extensions=@('reynard@reynard-yamakoira') }; $out = Join-Path $PWD 'dist\io.reynard.host.json'; [IO.File]::WriteAllText($out, ($m | ConvertTo-Json), (New-Object Text.UTF8Encoding $false))"
if errorlevel 1 goto :fail

echo == Registering with Firefox ==
reg add "HKCU\Software\Mozilla\NativeMessagingHosts\io.reynard.host" /ve /t REG_SZ /d "%~dp0dist\io.reynard.host.json" /f >nul
if errorlevel 1 goto :fail

echo.
echo Settings file:
dist\reynard.exe settings
echo.
echo Done. If you haven't yet: load extension\manifest.json in Firefox (about:debugging).
pause
exit /b 0

:locked
echo.
echo Could not replace dist\reynard.exe - it is probably still running.
echo Wait for downloads to finish (or run: dist\reynard.exe jobs), then try again.
pause
exit /b 1

:fail
echo.
echo Something went wrong - see the messages above.
pause
exit /b 1
