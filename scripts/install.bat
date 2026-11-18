@echo off
setlocal EnableExtensions
title Reynard installer

rem =====================================================================
rem  Reynard installer for Windows. It is a plain script: read it first.
rem
rem  What it does, in order:
rem    1. Downloads reynard.exe into  %LOCALAPPDATA%\Reynard
rem    2. Downloads yt-dlp, ffmpeg and Deno into  %LOCALAPPDATA%\Reynard\bin
rem       (Deno is the JavaScript runtime that YouTube downloads need)
rem       (skipped if they are already there)
rem    3. Tells Firefox where reynard.exe is: one registry key under
rem       HKEY_CURRENT_USER plus a small .json file next to the exe.
rem  No admin rights needed. Nothing else on your PC is changed.
rem  uninstall.bat removes all of it. Run this script again to update.
rem =====================================================================

rem ---- the only things that ever need editing ----
set "REYNARD_URL=https://github.com/YOUR-GITHUB-NAME/Reynard/releases/latest/download/reynard.exe"
set "ADDON_URL=https://addons.mozilla.org/firefox/addon/YOUR-ADDON-NAME/"
set "EXT_ID=reynard@reynard-yamakoira"
set "CHROME_ID=gcdmloapgnddiblfbcpoonneajgmhpmj"
set "CHROME_STORE_ID="
set "HOST_NAME=io.reynard.host"
set "YTDLP_URL=https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
set "FFMPEG_URL=https://github.com/yt-dlp/FFmpeg-Builds/releases/latest/download/ffmpeg-master-latest-win64-gpl.zip"
set "DENO_URL=https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip"
set "RF_DIR=%LOCALAPPDATA%\Reynard"

echo.
echo   Reynard installer
echo   -----------------
echo   Install folder : "%RF_DIR%"
echo   Downloads      : reynard.exe, yt-dlp, ffmpeg, Deno
echo   Firefox link   : registry key HKCU\Software\Mozilla\NativeMessagingHosts\%HOST_NAME%
echo   Chrome family  : the same under HKCU\Software\Google\Chrome, Microsoft\Edge, BraveSoftware and Chromium
echo.
echo   Press any key to install, or close this window to cancel.
pause >nul
echo.

where curl.exe >nul 2>nul || goto :nocurl
tasklist /FI "IMAGENAME eq reynard.exe" /NH 2>nul | find /I "reynard.exe" >nul && goto :running

mkdir "%RF_DIR%\bin" 2>nul
if not exist "%RF_DIR%\bin" goto :fail

rem ---- 1. reynard.exe (downloaded next to the old one, then swapped in) ----
echo [1/3] Downloading reynard.exe ...
call :download "%REYNARD_URL%" "%RF_DIR%\reynard.exe.new"
if errorlevel 1 goto :dlfail
move /y "%RF_DIR%\reynard.exe.new" "%RF_DIR%\reynard.exe" >nul
if errorlevel 1 goto :running

rem ---- 2. yt-dlp, ffmpeg, ffprobe, Deno: downloaded when missing ----
echo [2/3] Checking yt-dlp, ffmpeg, ffprobe and Deno ...

if exist "%RF_DIR%\bin\yt-dlp.exe" goto :have_ytdlp
where curl.exe >nul 2>nul || goto :nocurl
echo       Downloading yt-dlp ...
call :download "%YTDLP_URL%" "%RF_DIR%\bin\yt-dlp.exe"
if errorlevel 1 goto :dlfail
:have_ytdlp

if not exist "%RF_DIR%\bin\ffmpeg.exe" goto :get_ffmpeg
if not exist "%RF_DIR%\bin\ffprobe.exe" goto :get_ffmpeg
goto :have_ffmpeg
:get_ffmpeg
where curl.exe >nul 2>nul || goto :nocurl
echo       Downloading ffmpeg, about 100 to 150 MB. This can take a few minutes ...
call :download "%FFMPEG_URL%" "%RF_DIR%\bin\_ffmpeg.zip"
if errorlevel 1 goto :dlfail
echo       Unpacking ...
rmdir /s /q "%RF_DIR%\bin\_ffmpeg" 2>nul
powershell -NoProfile -Command "Expand-Archive -LiteralPath (Join-Path $env:RF_DIR 'bin\_ffmpeg.zip') -DestinationPath (Join-Path $env:RF_DIR 'bin\_ffmpeg') -Force"
if errorlevel 1 goto :fail
for /r "%RF_DIR%\bin\_ffmpeg" %%F in (ffmpeg.exe ffprobe.exe) do if exist "%%F" copy /y "%%F" "%RF_DIR%\bin\%%~nxF" >nul
rmdir /s /q "%RF_DIR%\bin\_ffmpeg" 2>nul
del "%RF_DIR%\bin\_ffmpeg.zip" 2>nul
:have_ffmpeg

if exist "%RF_DIR%\bin\deno.exe" goto :have_deno
where curl.exe >nul 2>nul || goto :nocurl
echo       Downloading Deno, about 45 MB. yt-dlp needs it to talk to YouTube ...
call :download "%DENO_URL%" "%RF_DIR%\bin\_deno.zip"
if errorlevel 1 goto :dlfail
powershell -NoProfile -Command "Expand-Archive -LiteralPath (Join-Path $env:RF_DIR 'bin\_deno.zip') -DestinationPath (Join-Path $env:RF_DIR 'bin') -Force"
if errorlevel 1 goto :fail
del "%RF_DIR%\bin\_deno.zip" 2>nul
:have_deno

rem ---- every tool must really be there now, and say so ----
set "MISSING="
for %%T in (yt-dlp.exe ffmpeg.exe ffprobe.exe deno.exe) do if exist "%RF_DIR%\bin\%%T" (echo       OK       %%T) else (echo       MISSING  %%T& set "MISSING=1")
if defined MISSING goto :fail

rem ---- 3. tell Firefox about it ----
echo [3/3] Registering with your browsers ...
powershell -NoProfile -Command "$m = [ordered]@{ name=$env:HOST_NAME; description='Reynard native host'; path=(Join-Path $env:RF_DIR 'reynard.exe'); type='stdio'; allowed_extensions=@($env:EXT_ID) }; $out = Join-Path $env:RF_DIR ($env:HOST_NAME + '.json'); [IO.File]::WriteAllText($out, ($m | ConvertTo-Json), (New-Object Text.UTF8Encoding $false))"
if errorlevel 1 goto :fail
reg add "HKCU\Software\Mozilla\NativeMessagingHosts\%HOST_NAME%" /ve /t REG_SZ /d "%RF_DIR%\%HOST_NAME%.json" /f >nul
if errorlevel 1 goto :fail
rem ---- the same for Chrome, Edge, Brave and Chromium (harmless if you do not use them) ----
powershell -NoProfile -Command "$o = @('chrome-extension://' + $env:CHROME_ID + '/'); if ($env:CHROME_STORE_ID) { $o += 'chrome-extension://' + $env:CHROME_STORE_ID + '/' }; $m = [ordered]@{ name=$env:HOST_NAME; description='Reynard native host'; path=(Join-Path $env:RF_DIR 'reynard.exe'); type='stdio'; allowed_origins=$o }; $out = Join-Path $env:RF_DIR ($env:HOST_NAME + '.chrome.json'); [IO.File]::WriteAllText($out, ($m | ConvertTo-Json), (New-Object Text.UTF8Encoding $false))"
if errorlevel 1 goto :fail
for %%K in ("Google\Chrome" "Microsoft\Edge" "BraveSoftware\Brave-Browser" "Chromium") do reg add "HKCU\Software\%%~K\NativeMessagingHosts\%HOST_NAME%" /ve /t REG_SZ /d "%RF_DIR%\%HOST_NAME%.chrome.json" /f >nul
if errorlevel 1 goto :fail

rem ---- check that the exe starts (also creates your settings file) ----
"%RF_DIR%\reynard.exe" settings
if errorlevel 1 goto :exefail

echo.
echo   Reynard is installed.
echo   Last step: add the Reynard add-on to your browser.
echo   Chrome, Edge, Brave: chrome://extensions, Developer mode, Load unpacked (the extension-chrome folder),
echo   or install it from the Chrome Web Store once it is published.
echo   Firefox:
echo   %ADDON_URL%
echo.
choice /c YN /m "Open that page in your browser now"
if errorlevel 2 goto :done
start "" "%ADDON_URL%"
:done
echo.
pause
exit /b 0

rem ---------------------------------------------------------------------
:download
rem  %1 = address, %2 = file to write.  Shows curl's progress bar.
curl.exe -L -f --retry 2 --progress-bar -o "%~2" "%~1"
exit /b %errorlevel%

:nocurl
echo   curl.exe was not found. It comes with Windows 10 (version 1803) and Windows 11.
echo   Please update Windows, or download the files by hand (see the addresses at the top of this script).
goto :end

:running
echo   reynard.exe is running right now, probably a download in progress.
echo   Wait for it to finish (or close Firefox), then run this installer again.
goto :end

:dlfail
echo.
echo   A download failed. Check your internet connection and try again.
echo   If it keeps failing, the address inside this script may be out of date.
goto :end

:exefail
echo.
echo   reynard.exe was installed but would not start. Antivirus software sometimes
echo   blocks new programs: check its quarantine list, then run this installer again.
goto :end

:fail
echo.
echo   Something went wrong, see the messages above.

:end
echo.
pause
exit /b 1
