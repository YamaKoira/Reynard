Reynard 0.4 - core + popup + settings page
==================
Setup (Windows)
  1. Run setup.bat. It builds Reynard, downloads yt-dlp and ffmpeg into dist\bin,
     and registers the native host with Firefox.
  2. Firefox Developer Edition: about:debugging#/runtime/this-firefox
     -> "Load Temporary Add-on..." -> extension\manifest.json
     (reload it there whenever you change the extension)
  3. Right-click a page, link, image or video -> "Download with Reynard".
  4. Pin the fox to the toolbar (puzzle-piece menu -> Pin to Toolbar). Click it for the
     popup: Downloads (cancel, open file, show in folder, clear) and Status (versions,
     yt-dlp update). The fox changes color: grey idle, orange downloading, green done,
     red failed / Reynard not reachable.
  5. Settings tab in the popup opens the settings page (sites, folders, flag toggles,
     "Custom flags" for pasting your own, a Try-a-link box). It edits settings.toml in
     place, so your comments stay. A backup of the previous file is kept in data\.

Terminal (from the dist folder)
  reynard download <url>   download now, with progress
  reynard jobs             list downloads
  reynard kill <id>        stop one (yt-dlp and ffmpeg)
  reynard clear            forget finished downloads
  reynard match <url>      which site rule would this URL use?
  reynard settings         prints the path of settings.toml

Folders in settings can use Windows variables, e.g. %USERPROFILE%\Downloads (the default).

Where things live
  dist\                       reynard.exe, bin\ (yt-dlp, ffmpeg), data\ (history, log)
  %APPDATA%\Reynard\settings.toml   the only file outside the folder
  uninstall.bat removes the Firefox registration.

Releasing (for the maintainer)
  1. Build: setup.bat (or cargo build --release). Upload target\release\reynard.exe to a
     GitHub release under exactly that name, so .../releases/latest/download/reynard.exe works.
  2. Sign the extension on addons.mozilla.org (unlisted = self-distributed) and publish the
     link or the .xpi. Release Firefox only keeps signed add-ons.
  3. Edit the top of release\install.bat: REYNARD_URL and ADDON_URL.
     EXT_ID there must match gecko.id in extension\manifest.json and EXTENSION_ID in
     src\main.rs (all three say reynard@reynard-yamakoira today; change them together, once).
  4. Attach release\install.bat and release\uninstall.bat to the release.
     Users double-click install.bat; it installs into %LOCALAPPDATA%\Reynard.
  Note: install.bat and setup.bat use the same Firefox registration, so whichever ran last wins.
