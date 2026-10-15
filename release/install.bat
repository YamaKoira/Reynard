@echo off
setlocal EnableExtensions
title Reynard installer

rem =====================================================================
rem  Reynard installer for Windows. It is a plain script: read it first.
rem
rem  What it does, in order:
rem    1. Downloads reynard.exe into  %LOCALAPPDATA%\Reynard
rem    2. Downloads yt-dlp and ffmpeg into  %LOCALAPPDATA%\Reynard\bin
rem       (skipped if they are already there)
rem    3. Tells Firefox where reynard.exe is: one registry key under
rem       HKEY_CURRENT_USER plus a small .json file next to the exe.
rem  No admin rights needed. Nothing else on your PC is changed.
rem  uninstall.bat removes all of it. Run this script again to update.
rem =====================================================================

rem ---- the only things that ever need editing ----
set "REYNARD_URL=https://github.com/YamaKoira/Reynard/releases/download/Release/reynard.exe"
set "ADDON_URL=https://github.com/YamaKoira/Reynard/releases/download/Release/reynard.xpi"
set "EXT_ID=reynard@reynard-yamakoira"
set "HOST_NAME=io.reynard.host"
set "YTDLP_URL=https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
set "FFMPEG_URL=https://github.com/yt-dlp/FFmpeg-Builds/releases/latest/download/ffmpeg-master-latest-win64-gpl.zip"
set "RF_DIR=%LOCALAPPDATA%\Reynard"

echo.
echo   Reynard installer
echo   -----------------
echo   Install folder : "%RF_DIR%"
echo   Downloads      : reynard.exe, yt-dlp, ffmpeg
echo   Firefox link   : registry key HKCU\Software\Mozilla\NativeMessagingHosts\%HOST_NAME%
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

rem ---- 2. yt-dlp and ffmpeg ----
echo [2/3] Checking yt-dlp and ffmpeg ...
if exist "%RF_DIR%\bin\yt-dlp.exe" goto :have_ytdlp
echo       Downloading yt-dlp ...
call :download "%YTDLP_URL%" "%RF_DIR%\bin\yt-dlp.exe"
if errorlevel 1 goto :dlfail
:have_ytdlp

if not exist "%RF_DIR%\bin\ffmpeg.exe" goto :get_ffmpeg
if exist "%RF_DIR%\bin\ffprobe.exe" goto :have_ffmpeg
:get_ffmpeg
echo       Downloading ffmpeg, about 100 to 150 MB. This can take a few minutes ...
call :download "%FFMPEG_URL%" "%RF_DIR%\bin\_ffmpeg.zip"
if errorlevel 1 goto :dlfail
echo       Unpacking ...
rmdir /s /q "%RF_DIR%\bin\_ffmpeg" 2>nul
powershell -NoProfile -Command "Expand-Archive -LiteralPath (Join-Path $env:RF_DIR 'bin\_ffmpeg.zip') -DestinationPath (Join-Path $env:RF_DIR 'bin\_ffmpeg') -Force"
if errorlevel 1 goto :fail
for /r "%RF_DIR%\bin\_ffmpeg" %%F in (ffmpeg.exe ffprobe.exe) do if exist "%%F" copy /y "%%F" "%RF_DIR%\bin\" >nul
rmdir /s /q "%RF_DIR%\bin\_ffmpeg" 2>nul
del "%RF_DIR%\bin\_ffmpeg.zip" 2>nul
if not exist "%RF_DIR%\bin\ffmpeg.exe" goto :fail
:have_ffmpeg

rem ---- 3. tell Firefox about it ----
echo [3/3] Registering with Firefox ...
powershell -NoProfile -Command "$m = [ordered]@{ name=$env:HOST_NAME; description='Reynard native host'; path=(Join-Path $env:RF_DIR 'reynard.exe'); type='stdio'; allowed_extensions=@($env:EXT_ID) }; $out = Join-Path $env:RF_DIR ($env:HOST_NAME + '.json'); [IO.File]::WriteAllText($out, ($m | ConvertTo-Json), (New-Object Text.UTF8Encoding $false))"
if errorlevel 1 goto :fail
reg add "HKCU\Software\Mozilla\NativeMessagingHosts\%HOST_NAME%" /ve /t REG_SZ /d "%RF_DIR%\%HOST_NAME%.json" /f >nul
if errorlevel 1 goto :fail

rem ---- check that the exe starts (also creates your settings file) ----
"%RF_DIR%\reynard.exe" settings
if errorlevel 1 goto :exefail

echo.
echo   Reynard is installed.
echo   Last step: add the Reynard add-on to Firefox:
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
