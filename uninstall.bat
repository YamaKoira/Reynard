@echo off
rem Removes the Firefox registration of the DEVELOPMENT copy (setup.bat).
rem An installed Reynard has its own uninstall.bat in the release folder.
reg delete "HKCU\Software\Mozilla\NativeMessagingHosts\io.reynard.host.dev" /f >nul 2>&1
echo Removed the registration of the dev copy from Firefox.
echo To finish: delete the folders dist-dev and target-dev, and remove "Reynard (dev)" in about:addons.
pause
