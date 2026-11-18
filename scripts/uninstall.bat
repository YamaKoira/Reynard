@echo off
setlocal EnableExtensions
title Reynard uninstaller

rem Removes what install.bat added: the Firefox registry key and the Reynard folder.
set "HOST_NAME=io.reynard.host"
set "RF_DIR=%LOCALAPPDATA%\Reynard"

echo.
echo   This removes Reynard:
echo     - the Firefox link (registry key under HKCU\Software\Mozilla\NativeMessagingHosts)
echo     - the folder "%RF_DIR%" with reynard.exe, yt-dlp, ffmpeg and your download history
echo   Your downloaded videos are not touched.
echo.
choice /c YN /m "Continue"
if errorlevel 2 exit /b 0

tasklist /FI "IMAGENAME eq reynard.exe" /NH 2>nul | find /I "reynard.exe" >nul && goto :running

reg delete "HKCU\Software\Mozilla\NativeMessagingHosts\%HOST_NAME%" /f >nul 2>nul
for %%K in ("Google\Chrome" "Microsoft\Edge" "BraveSoftware\Brave-Browser" "Chromium") do reg delete "HKCU\Software\%%~K\NativeMessagingHosts\%HOST_NAME%" /f >nul 2>nul
rmdir /s /q "%RF_DIR%" 2>nul
if exist "%RF_DIR%" goto :left

echo.
echo   Your settings file is kept in "%APPDATA%\Reynard".
choice /c YN /m "Delete that too"
if errorlevel 2 goto :done
rmdir /s /q "%APPDATA%\Reynard" 2>nul

:done
echo.
echo   Done. Last step: remove the Reynard add-on in your browser (about:addons, chrome://extensions).
echo.
pause
exit /b 0

:running
echo.
echo   reynard.exe is running right now. Wait for downloads to finish, then try again.
echo.
pause
exit /b 1

:left
echo.
echo   Some files could not be deleted, probably because a program is using them.
echo   Close Firefox and delete the folder "%RF_DIR%" by hand.
echo.
pause
exit /b 1
