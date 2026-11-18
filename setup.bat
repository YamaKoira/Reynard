@echo off
rem =====================================================================
rem  Development build of Reynard.
rem
rem  Builds into dist-dev\ and registers it as a SEPARATE "dev" Reynard: its own
rem  Firefox host name, its own add-on ID, its own settings and history. An
rem  installed Reynard (install.bat) keeps working untouched next to it.
rem =====================================================================
setlocal
cd /d "%~dp0"

set "DEV_HOST=io.reynard.host.dev"
set "DEV_EXT_ID=reynard-dev@reynard-yamakoira"
rem the exe is compiled so that it answers to the dev add-on only
set "REYNARD_EXT_ID=%DEV_EXT_ID%"

echo == Building Reynard dev ==
cargo build --release --target-dir target-dev
if errorlevel 1 goto :fail

if not exist dist-dev mkdir dist-dev
copy /y target-dev\release\reynard.exe dist-dev\reynard.exe >nul
if errorlevel 1 goto :locked

rem ---- yt-dlp and ffmpeg: borrow the installed copy if there is one, else download ----
mkdir dist-dev\bin 2>nul
for %%T in (yt-dlp.exe ffmpeg.exe ffprobe.exe deno.exe) do if not exist "dist-dev\bin\%%T" if exist "%LOCALAPPDATA%\Reynard\bin\%%T" copy /y "%LOCALAPPDATA%\Reynard\bin\%%T" "dist-dev\bin\" >nul
if exist dist-dev\bin\yt-dlp.exe if exist dist-dev\bin\ffmpeg.exe if exist dist-dev\bin\ffprobe.exe if exist dist-dev\bin\deno.exe goto :tools_ready
echo == Fetching yt-dlp, ffmpeg and Deno ==
powershell -NoProfile -ExecutionPolicy Bypass -File tools\fetch.ps1 -Bin dist-dev\bin
if errorlevel 1 goto :fail
:tools_ready

rem ---- the dev copy keeps its own settings.toml next to the exe ----
if not exist dist-dev\portable.txt echo Reynard dev copy: settings.toml is kept in this folder.> dist-dev\portable.txt
if not exist dist-dev\settings.toml if exist "%APPDATA%\Reynard\settings.toml" copy "%APPDATA%\Reynard\settings.toml" dist-dev\settings.toml >nul

echo == Creating the dev add-on ==
powershell -NoProfile -ExecutionPolicy Bypass -File tools\make-dev-extension.ps1 -Out dist-dev\extension -ExtId %DEV_EXT_ID% -HostName %DEV_HOST%
if errorlevel 1 goto :fail

echo == Creating the dev add-on for Chrome ==
powershell -NoProfile -ExecutionPolicy Bypass -File tools\make-chrome-extension.ps1 -Out dist-dev\extension-chrome -Mode dev
if errorlevel 1 goto :fail

echo == Registering the dev host with Firefox ==
powershell -NoProfile -Command "$exe = (Resolve-Path 'dist-dev\reynard.exe').Path; $m = [ordered]@{ name=$env:DEV_HOST; description='Reynard native host (dev)'; path=$exe; type='stdio'; allowed_extensions=@($env:DEV_EXT_ID) }; $out = Join-Path $PWD ('dist-dev\' + $env:DEV_HOST + '.json'); [IO.File]::WriteAllText($out, ($m | ConvertTo-Json), (New-Object Text.UTF8Encoding $false))"
if errorlevel 1 goto :fail
reg add "HKCU\Software\Mozilla\NativeMessagingHosts\%DEV_HOST%" /ve /t REG_SZ /d "%~dp0dist-dev\%DEV_HOST%.json" /f >nul
if errorlevel 1 goto :fail

echo == Registering the dev host with Chrome, Edge, Brave and Chromium ==
powershell -NoProfile -Command "$id = ((Get-Content 'chrome\ids.txt') | Where-Object { $_ -like 'DEV_ID=*' }) -replace '^DEV_ID=',''; $exe = (Resolve-Path 'dist-dev\reynard.exe').Path; $m = [ordered]@{ name=$env:DEV_HOST; description='Reynard native host (dev)'; path=$exe; type='stdio'; allowed_origins=@('chrome-extension://' + $id + '/') }; $out = Join-Path $PWD ('dist-dev\' + $env:DEV_HOST + '.chrome.json'); [IO.File]::WriteAllText($out, ($m | ConvertTo-Json), (New-Object Text.UTF8Encoding $false))"
if errorlevel 1 goto :fail
for %%K in ("Google\Chrome" "Microsoft\Edge" "BraveSoftware\Brave-Browser" "Chromium") do reg add "HKCU\Software\%%~K\NativeMessagingHosts\%DEV_HOST%" /ve /t REG_SZ /d "%~dp0dist-dev\%DEV_HOST%.chrome.json" /f >nul
if errorlevel 1 goto :fail

echo.
echo Settings file of the dev copy:
dist-dev\reynard.exe settings
echo.
echo Done. An installed Reynard was not touched.
echo To test: in Firefox open about:debugging#/runtime/this-firefox, choose
echo "Load Temporary Add-on" and pick dist-dev\extension\manifest.json
echo The dev add-on is called "Reynard (dev)" and only talks to dist-dev\reynard.exe.
echo.
echo Chrome, Edge, Brave or Chromium: open chrome://extensions, switch on Developer mode,
echo choose "Load unpacked" and pick the folder dist-dev\extension-chrome
pause
exit /b 0

:locked
echo.
echo Could not replace dist-dev\reynard.exe - it is probably still running.
echo Wait for downloads to finish (or run: dist-dev\reynard.exe jobs), then try again.
pause
exit /b 1

:fail
echo.
echo Something went wrong - see the messages above.
pause
exit /b 1
