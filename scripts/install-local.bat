@echo off
setlocal EnableExtensions
title Reynard installer (local)

rem =====================================================================
rem  Reynard installer for Windows, LOCAL version. A plain script: read it first.
rem
rem  Same as install.bat, except reynard.exe is NOT downloaded: it is taken from
rem  the folder this script is in. If yt-dlp.exe, ffmpeg.exe, ffprobe.exe or deno.exe
rem  are in that folder too, they are used instead of being downloaded.
rem
rem  What it does, in order:
rem    1. Copies reynard.exe from this folder into  %LOCALAPPDATA%\Reynard
rem    2. Puts yt-dlp, ffmpeg and Deno into  %LOCALAPPDATA%\Reynard\bin
rem       (copied from this folder, or downloaded from the yt-dlp project if missing)
rem    3. Tells Firefox where reynard.exe is: one registry key under
rem       HKEY_CURRENT_USER plus a small .json file next to the exe.
rem  No admin rights needed. Nothing else on your PC is changed.
rem  uninstall.bat removes all of it. Run this script again to update.
rem =====================================================================

rem ---- the only things that may need editing ----
set "EXT_ID=reynard@reynard-yamakoira"
set "CHROME_ID=gcdmloapgnddiblfbcpoonneajgmhpmj"
set "CHROME_STORE_ID="
set "HOST_NAME=io.reynard.host"
set "ADDON_URL="
set "YTDLP_URL=https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
set "FFMPEG_URL=https://github.com/yt-dlp/FFmpeg-Builds/releases/latest/download/ffmpeg-master-latest-win64-gpl.zip"
set "DENO_URL=https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip"
set "RF_DIR=%LOCALAPPDATA%\Reynard"

cd /d "%~dp0"
set "HERE=%~dp0"

echo.
echo   Reynard installer (local)
echo   -------------------------
echo   Install folder : "%RF_DIR%"
echo   reynard.exe    : copied from "%HERE%"
echo   Firefox link   : registry key HKCU\Software\Mozilla\NativeMessagingHosts\%HOST_NAME%
echo   Chrome family  : the same under HKCU\Software\Google\Chrome, Microsoft\Edge, BraveSoftware and Chromium
echo.
echo   Press any key to install, or close this window to cancel.
pause >nul
echo.

if not exist "%HERE%reynard.exe" goto :noexe

rem a dev build would register fine but never work with the real add-on
set "BUILDLINE="
for /f "delims=" %%L in ('reynard.exe version') do set "BUILDLINE=%%L"
echo "%BUILDLINE%" | find "(release)" >nul || goto :notrelease

tasklist /FI "IMAGENAME eq reynard.exe" /NH 2>nul | find /I "reynard.exe" >nul && goto :running

mkdir "%RF_DIR%\bin" 2>nul
if not exist "%RF_DIR%\bin" goto :fail

rem ---- 1. reynard.exe (copied next to the old one, then swapped in) ----
echo [1/3] Copying reynard.exe ...
copy /y "%HERE%reynard.exe" "%RF_DIR%\reynard.exe.new" >nul
if errorlevel 1 goto :fail
move /y "%RF_DIR%\reynard.exe.new" "%RF_DIR%\reynard.exe" >nul
if errorlevel 1 goto :running

rem ---- 2. yt-dlp, ffmpeg, ffprobe, Deno: copied from this folder if present, else downloaded ----
echo [2/3] Installing yt-dlp, ffmpeg, ffprobe and Deno ...
for %%T in (yt-dlp.exe ffmpeg.exe ffprobe.exe deno.exe) do call :fromhere %%T

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

rem ---- check that the installed exe starts (also creates your settings file) ----
"%RF_DIR%\reynard.exe" settings
if errorlevel 1 goto :exefail

echo.
echo   Reynard is installed.
echo   Last step: add the Reynard add-on to your browser.
echo   Chrome, Edge, Brave: chrome://extensions, Developer mode, Load unpacked (the extension-chrome folder),
echo   or install it from the Chrome Web Store once it is published.
echo   Firefox:
if defined ADDON_URL goto :addon_page
echo   A signed add-on file: drag it into a Firefox window.
echo   Unsigned, for trying it out: in Firefox open about:debugging#/runtime/this-firefox,
echo   choose Load Temporary Add-on and pick extension\manifest.json from the project.
goto :done

:addon_page
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

:fromhere
rem  %1 = a tool's file name. Copy it from this folder over the installed one, if it is here.
if not exist "%HERE%%~1" goto :fromhere_none
copy /y "%HERE%%~1" "%RF_DIR%\bin\%~1" >nul
if errorlevel 1 goto :fromhere_bad
echo       %~1 : copied from this folder
exit /b 0
:fromhere_none
echo       %~1 : not in this folder
exit /b 0
:fromhere_bad
echo       %~1 : COPY FAILED, is it running or blocked by antivirus?
exit /b 0

:noexe
echo   reynard.exe was not found next to this script.
echo   Put reynard.exe in the same folder as install-local.bat and run it again.
goto :end

:notrelease
echo.
echo   This reynard.exe is not a release build, or it would not start. It says:
echo   Build says: %BUILDLINE%
echo   Use the reynard.exe made by make-release.bat. If it would not start at all,
echo   check your antivirus quarantine list.
goto :end

:running
echo   reynard.exe is running right now, probably a download in progress.
echo   Wait for it to finish (or close Firefox), then run this installer again.
goto :end

:nocurl
echo   curl.exe was not found. It comes with Windows 10 version 1803 and Windows 11.
echo   Put yt-dlp.exe, ffmpeg.exe, ffprobe.exe and deno.exe in the same folder as this script instead.
goto :end

:dlfail
echo.
echo   A download failed. Check your internet connection and try again.
echo   Or put yt-dlp.exe, ffmpeg.exe, ffprobe.exe and deno.exe in the same folder as this script.
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
