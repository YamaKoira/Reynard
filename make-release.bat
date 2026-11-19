@echo off
rem =====================================================================
rem  Builds everything you upload to a release, into release-out\ :
rem    reynard.exe                       the real build (works with the real add-on)
rem    install-local.bat, uninstall.bat  for your users (install-local.bat uses the exe next to it)
rem    Reynard-extension-VERSION.zip     the add-on, to submit to addons.mozilla.org
rem
rem  Not to be confused with setup.bat, which makes the DEV copy in dist-dev.
rem =====================================================================
setlocal
cd /d "%~dp0"

rem make sure no dev add-on ID leaks into this build
set "REYNARD_EXT_ID="

echo == Building Reynard (release) ==
cargo build --release
if errorlevel 1 goto :fail

if exist release-out rmdir /s /q release-out
mkdir release-out
copy /y target\release\reynard.exe release-out\reynard.exe >nul
if errorlevel 1 goto :fail
copy /y scripts\install-local.bat release-out\install-local.bat >nul
copy /y scripts\uninstall.bat release-out\uninstall.bat >nul

rem safety check: the exe says what it is, and a dev build must never be released
set "BUILDLINE="
for /f "delims=" %%L in ('release-out\reynard.exe version') do set "BUILDLINE=%%L"
echo "%BUILDLINE%" | find "(release)" >nul || goto :devbuild
findstr /m /c:"reynard-dev@" release-out\reynard.exe >nul && goto :devbuild

rem the Chrome add-on IDs written into the installers must be the ones in chrome\ids.txt
powershell -NoProfile -ExecutionPolicy Bypass -File tools\check-chrome-ids.ps1
if errorlevel 1 goto :fail

rem the version number comes from the add-on manifest
set "VER="
for /f "delims=" %%V in ('powershell -NoProfile -Command "(Get-Content -Raw -LiteralPath extension\manifest.json | ConvertFrom-Json).version"') do set "VER=%%V"
if not defined VER goto :fail

echo == Packing the add-on %VER% ==
tar.exe -a -c -f "release-out\Reynard-extension-%VER%.zip" -C extension .
if errorlevel 1 goto :fail

echo == Chrome add-on ==
powershell -NoProfile -ExecutionPolicy Bypass -File tools\make-chrome-extension.ps1 -Out release-out\extension-chrome -Mode release
if errorlevel 1 goto :fail

rem checksum, so people can verify the download (optional, failure is not fatal)
powershell -NoProfile -Command "(Get-FileHash -Algorithm SHA256 release-out\reynard.exe).Hash.ToLower() + '  reynard.exe'" > release-out\SHA256SUMS.txt

echo.
echo Ready in release-out\ :
dir /b release-out
echo.
echo Upload reynard.exe, install-local.bat and uninstall.bat to your release (keep the name reynard.exe).
echo Submit Reynard-extension-%VER%.zip to addons.mozilla.org.
echo Chrome: load the extension-chrome\ folder unpacked (chrome://extensions, Developer mode).
pause
exit /b 0

:devbuild
echo.
echo STOP: this reynard.exe is not a release build, it says: %BUILDLINE%
echo Do not release it. Close this window and run make-release.bat again from a fresh terminal.
goto :end

:fail
echo.
echo Something went wrong - see the messages above.

:end
echo.
pause
exit /b 1
