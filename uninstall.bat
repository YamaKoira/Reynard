@echo off
reg delete "HKCU\Software\Mozilla\NativeMessagingHosts\io.reynard.host" /f >nul 2>&1
echo Removed Reynard's registration from Firefox.
echo To finish: delete this folder. Your settings are in %APPDATA%\Reynard.
pause
