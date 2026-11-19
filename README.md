# Reynard

Right-click anything in your browser and have it downloaded by [yt-dlp](https://github.com/yt-dlp/yt-dlp), into the folder and with the options you chose for that site. No terminal, no download manager, no copy-pasting links.

- **One click.** Right-click a link, image, video or page, then choose *Reynard*, then *Download* (or *as audio / GIF / video*).
- **Per-site rules.** Twitter goes to one folder, YouTube to another, each with its own options (convert to mp4, embed thumbnails, use your Firefox cookies, and so on).
- **A fox that tells you what's happening.** The toolbar icon shows idle, downloading, done or failed. No pop-up notifications needed.
- **Stays out of your way.** Nothing runs in the background while you're not downloading, and there is no tray icon.
- **Everything in one folder.** No installer, no admin rights, and one clean uninstall.

Windows and Arch Linux, with Firefox, or with Chrome, Edge, Brave or Chromium. (Other Linux distributions will probably work too, but only Arch is tested.)

## Install

You need Windows 10 (version 1803 or later) or Windows 11, and Firefox.

1. Put **`reynard.exe`** and **`install-local.bat`** in the same folder and double-click `install-local.bat`. It's a plain text script: it tells you exactly what it will do and waits for a key press before touching anything. It copies Reynard from its own folder (and refuses an exe that isn't a release build), and puts yt-dlp, ffmpeg and Deno into `%LOCALAPPDATA%\Reynard` and tells Firefox where to find Reynard. No admin rights are needed.
2. Add the **Reynard add-on** to Firefox. The installer offers to open the page for you.
3. Pin the fox to the toolbar: puzzle-piece menu, then *Pin to Toolbar*.

Windows may show a "protected your PC" warning for files downloaded from the internet. Choose *More info*, then *Run anyway*. Read the script first if you like; that's why it's a script. Windows Security (Defender) may also flag `reynard.exe` as a trojan (for example `Trojan:Win32/Bearfoos.A!ml`). That `!ml` means a machine-learning guess, and new unsigned programs that start other programs (here yt-dlp and ffmpeg) get it often. If it happens: *Windows Security, Virus & threat protection, Protection history*, restore the file and choose *Allow on device*. You can also report it as a false positive at <https://www.microsoft.com/wdsi/filesubmission>. The source is all in this repository, so you can build it yourself instead.

If `yt-dlp.exe`, `ffmpeg.exe`, `ffprobe.exe` and `deno.exe` are in that folder too, it uses them instead of downloading them.

Run `install-local.bat` again at any time to update Reynard. yt-dlp and ffmpeg are kept; yt-dlp updates from inside the add-on (see below).

## Install on Arch Linux

You need Firefox from the repositories (the Snap and Flatpak builds can't start native hosts) and three packages:

```
sudo pacman -S --needed curl ffmpeg deno
```

1. Put **`reynard`**, **`install.sh`** and **`uninstall.sh`** in one folder and run `./install.sh`. It checks that curl, ffmpeg and Deno are installed (and prints the pacman command if not), puts Reynard in `~/.local/share/reynard`, downloads yt-dlp (the official standalone build, which the popup can update), and tells Firefox where Reynard is with one small `.json` file in `~/.mozilla/native-messaging-hosts`. No root needed.
2. Add the **Reynard add-on** to Firefox: `about:addons`, the gear icon, *Install Add-on From File*.
3. Pin the fox to the toolbar.

Your settings are in `~/.config/reynard/settings.toml`. The *Browse...* button uses `zenity` or `kdialog` when one is installed (`sudo pacman -S zenity`); without them, type the folder path. Run `./install.sh` again to update Reynard, `./uninstall.sh` to remove it. Run the scripts from a terminal; double-clicking them in a file manager is not supported. Everything below applies to both systems; where it differs, it says so.

## Install in Chrome, Edge, Brave or Chromium

It is the same Reynard program with a Chrome version of the add-on. The installers (`install-local.bat` and `install.sh`) already tell Chrome, Edge, Brave and Chromium where Reynard is (registry keys on Windows, one `.json` file per browser on Linux, for the browsers that exist), so only the add-on is left:

1. Open `chrome://extensions` (or `edge://extensions`) and switch on **Developer mode**.
2. Choose **Load unpacked** and pick the **`extension-chrome`** folder (from `release-out`).

Its ID is fixed by a key in its manifest, which is what lets the installer's registration match it. Chrome keeps reminding you that an unpacked add-on is in developer mode; publishing it in the Chrome Web Store removes that. Differences from Firefox: Chrome puts the background to sleep when nothing is happening, so the toolbar fox can take up to about 30 seconds to notice a download that was started elsewhere; during your own downloads it updates every 2 seconds as usual.

## Using it

### Right-click

Right-click a link, an image, a video, audio, or the page itself, then choose **Reynard**. Its submenu has:

- **Download**: your settings exactly as they are (the quick one).
- **Download as audio**: audio only, in the format set on the settings page (mp3 by default). Flags that only make sense for video are dropped.
- **Download as GIF**: downloads the video, then ffmpeg turns it into a GIF next to where the video would have gone. By default the video is not kept.
- **Download as video**: your settings, with audio-only switched off.

Every entry still follows your site rules for the folder and flags. If a format you asked for (say a resolution limit) isn't available, Reynard retries once and lets yt-dlp pick the best it can find; conversions such as mp3 or GIF still happen afterwards. (Why a submenu? Firefox groups all of an add-on's context-menu items into one submenu named after the add-on, so a separate "quick" entry beside it isn't possible. Download comes first in it.)

Reynard picks the address in this order: the link, then the media source, then the page. Addresses that yt-dlp can't use (`blob:` video elements, for example) are skipped, so on YouTube it falls back to the page address. A playlist is not downloaded unless you turn that off (the default flag `--no-playlist`).

**On x.com / twitter.com** videos are `blob:` elements too, so a small content script (`extension/x-post.js`) remembers which post you right-clicked in. When the normal pick would be the page address, Reynard uses that post's own address (`/user/status/ID`) instead, so it works in threads, quote posts and on pages like `/i/history`. It only reads the page, and it is the reason the add-on asks for access to those two sites. It also works out which videos are meant: yt-dlp lists a post's own videos first and the quoted post's after them, so the add-on counts the videos on the page and tells Reynard to download only those (`--playlist-items`); with *single video* on, that is the first one. Right-clicking the quoted post's video gets the quoted one. Every right-click prints one `[Reynard] right-click:` line to the page console (F12) that shows what it found. After installing or reloading the add-on, reload open X tabs, or the script isn't there yet. If it can't tell which post you meant (X changes its page now and then), you get today's behaviour: the page address. Right-clicking a link, or a GIF, is unchanged.

### The toolbar fox

| Fox | Meaning |
| --- | --- |
| Grey, eyes closed | Idle |
| Orange (badge with a number if several) | Downloading; hover for progress |
| Green, for 5 seconds | Finished |
| Red, for 10 seconds | A download failed |
| Red, with a tooltip | Reynard can't be reached; see [Troubleshooting](#troubleshooting) |

### The popup (click the fox)

- **Downloads**: every download with its site, a badge for what it was (`VIDEO`, `AUDIO` or `GIF`), link, folder and progress. *Cancel* stops a running download (yt-dlp and ffmpeg together). *Open* opens a finished file, *Folder* shows it in Explorer, and *Clear Downloads* forgets finished ones.
- **Status**: the add-on's version, whether the core (`reynard.exe`) is reachable with its version and build type (release or dev), your yt-dlp version (tagged *outdated* when a newer one exists) with an *Update yt-dlp* button, and where your settings file is.
- **Settings**: opens the settings page in its own tab.

### The settings page

- **General**: the default download folder (with a *Browse* button), the default flags, the *Download as* options (audio format; GIF frames per second, width, and whether to keep the video), *File names* (include the video ID, and rename a file if its name is already taken), and under *Advanced* the yt-dlp program, the ffmpeg folder and the file name template.
- **Sites**: one rule per site. A rule has *patterns*, a *folder*, its own *flags* and a *Video ID* choice (follow the general setting, include the ID, or leave it out). The first rule that matches a link wins, so order matters; use the arrows to reorder.
- **Try a link**: paste a link to see which rule it would use, with the final folder and flags. It reflects your unsaved edits too.

Flags are switches and dropdowns:

| Option | yt-dlp flag |
| --- | --- |
| Single video only | `--no-playlist` |
| Max resolution (2160p to 480p) | `-f bv*[height<=N]+ba/b[height<=N]` |
| Convert video to mp4, mkv or webm | `--recode-video` |
| Audio only (best, mp3, m4a, opus, flac) | `-x --audio-format` |
| Embed thumbnail, metadata, subtitles | `--embed-thumbnail`, `--embed-metadata`, `--embed-subs` |
| Cookies from a browser (Firefox, Chrome, Edge, ...) | `--cookies-from-browser` |

Switch on **Custom flags** to paste any yt-dlp options yourself. Flags the switches don't know are never dropped; they're kept and shown as "Other flags, kept as they are". A rule's flags are *added on top of* the default flags unless you untick **Start from the default flags**.

## The settings file

The settings page edits this file; you can edit it by hand too. Your comments survive a save from the page, and the previous version is backed up to `data\settings.backup.toml` in the Reynard folder.

Location: `%APPDATA%\Reynard\settings.toml` (the only file Reynard keeps outside its own folder, apart from your downloads).

```toml
[general]
yt_dlp = 'bin\yt-dlp.exe'        # relative paths are relative to the Reynard folder
ffmpeg_dir = 'bin'
default_dir = '%USERPROFILE%\Downloads'
default_args = "--no-playlist --recode-video mp4 --embed-thumbnail --embed-metadata"
output_template = "%(title).150B [%(id)s].%(ext)s"
include_id = true                # false = plain "Title.mp4", without the [video id]
rename_existing = false          # true = if "Title.mp4" exists, save "Title (1).mp4", "Title (2).mp4" ...
audio_format = "mp3"           # Download as audio: mp3, m4a, opus, flac, or best (keep the original)
gif_fps = 15                     # Download as GIF: frames per second
gif_width = 480                  # ... width in pixels, 0 = the video's own size (never scaled up)
gif_keep_video = false           # ... keep the downloaded video next to the GIF

# The first matching rule wins.
[[site]]
patterns = ["*youtube.com*", "*youtu.be*"]
dir = 'C:\Users\you\Videos\YouTube'
args = "--cookies-from-browser firefox"   # added after default_args
# inherit_args = false                    # use only this rule's args instead
# include_id = false                      # this site only: no video ID in file names
```

- **Patterns** are matched against the whole link, case-insensitively. `*` means anything and `?` means one character, so you usually want a `*` at both ends. In the settings page, separate several patterns with commas or spaces.
- **Folders** can use Windows variables such as `%USERPROFILE%`, so no user name needs to be written into the file.
- Paths in single quotes are taken literally, so backslashes need no escaping.
- **File names:** `include_id` and `rename_existing` only apply while `output_template` is the stock one shown above. If you write your own template, it is used exactly as written. Without the ID, two different videos with the same title share a name, so yt-dlp skips the second one (Reynard says so in the popup). Turn on `rename_existing` to keep both. It downloads into a scratch folder first, then moves the file into your folder under the first free name. A failed or cancelled download leaves no half-finished files in your folder, but it also can't be resumed.
- **yt-dlp's own config is ignored:** Reynard runs yt-dlp with `--ignore-config`, so your personal yt-dlp config (`%APPDATA%\yt-dlp\config`) or a `yt-dlp.conf` in the download folder never changes what Reynard does. Put your flags in Reynard's settings instead. A `--config-locations` in your own flags still works.
- **Portable mode:** if a file named `portable.txt` sits next to `reynard.exe`, `settings.toml` is kept there too, instead of in `%APPDATA%\Reynard`.

## Command line

Everything is also available from a terminal (run it from the Reynard folder, or add that folder to your PATH):

```
reynard download <url> [--as video|audio|gif]
                         download now, with progress (uses your site rules)
reynard jobs             list downloads and their status
reynard kill <id>        stop a download (yt-dlp and ffmpeg)
reynard clear            forget finished downloads
reynard match <url>      show which site rule a link would use
reynard settings         print where settings.toml is (creates it if missing)
reynard version          show which build this is (release or dev)
```

## How it works, and what it can touch

Firefox add-ons can't run programs, so Reynard is two small parts:

- the **add-on** (right-click menu, fox icon, popup, settings page), and
- **`reynard.exe`** (just `reynard` on Linux), which Firefox starts on demand through *native messaging*. It runs yt-dlp, keeps your download history and edits your settings.

Because of that:

- **Nothing listens on a network port.** Only the Reynard add-on, identified by its ID in the registration, can talk to `reynard.exe`. Web pages can't.
- **The add-on only ever sends a link.** The flags always come from your settings file, so a page can't inject options.
- Open / Show-in-folder work on **job numbers** from Reynard's own history, never on paths supplied by the add-on.
- Downloads run as **separate background processes**, so they carry on if you close Firefox.
- There is **no telemetry**, and the add-on declares that it collects no data. The network traffic is yt-dlp talking to the sites you download from, the downloads done by `install-local.bat`, and the popup's *Status* tab asking `api.github.com` (at most every 6 hours) what the newest yt-dlp version is.

What lives where:

```
%LOCALAPPDATA%\Reynard\
    reynard.exe
    io.reynard.host.json     tells Firefox where reynard.exe is
    bin\                     yt-dlp.exe, ffmpeg.exe, ffprobe.exe, deno.exe
    data\                    history.json, reynard.log, settings.backup.toml
%APPDATA%\Reynard\settings.toml
HKCU\Software\Mozilla\NativeMessagingHosts\io.reynard.host     (one registry key)
```

## Troubleshooting

- **Red fox, "can't reach reynard.exe"**: run `install-local.bat` again, then restart Firefox. Check that `%LOCALAPPDATA%\Reynard\io.reynard.host.json` exists.
- **Right-click does nothing**: press Ctrl+Shift+J in Firefox and look for lines starting with `[Reynard]`.
- **"ffmpeg not found"**: check that `ffmpeg.exe` and `ffprobe.exe` are in the `bin` folder, and that `ffmpeg_dir` in your settings points there.
- **Open or Folder does nothing**: look at `data\reynard.log` for lines such as `open #3: ...`.
- **YouTube: "Requested format is not available" or "Only images are available"**: yt-dlp needs a JavaScript runtime to see YouTube's formats, and that is what **Deno** (`bin\deno.exe`) is for. Run `install-local.bat` again to add it, then update yt-dlp (popup, *Status*, *Update yt-dlp*). Reynard tells yt-dlp where Deno is.
- **YouTube: "The page needs to be reloaded"**: update yt-dlp first; this has been fixed upstream more than once. If it persists, try the *Cookies from browser* option off for YouTube.
- **Reddit: "Your IP address is unable to access the Reddit API"**: Reddit is refusing your internet address, which Reynard can't change. Another network (or a VPN off) sometimes helps, and so can *Cookies from browser* for Reddit while logged in there.
- **"Making the GIF failed"**: GIFs need ffmpeg. Check that `ffmpeg.exe` is in the `bin` folder. The video itself is kept in your download folder when this happens.
- **A site stopped working**: update yt-dlp (popup, *Status*, *Update yt-dlp*). Sites change often.
- **Your settings file has an error**: Reynard shows the message and leaves the file alone. The last good copy is `data\settings.backup.toml`.
- **Antivirus or SmartScreen complains**: new, unsigned programs often trigger it. Check your antivirus quarantine list and allow `reynard.exe`.

## Uninstall

Run `uninstall.bat`. It removes the registry key and the Reynard folder (and asks before deleting your settings). Then remove the add-on in Firefox (`about:addons`). Your downloaded videos are never touched. On Linux, run `./uninstall.sh`: it removes `~/.local/share/reynard` and the `.json` file in `~/.mozilla/native-messaging-hosts`, and asks before deleting `~/.config/reynard`.

## Building from source

You need [Rust](https://rust-lang.org/learn/get-started/) and the Visual Studio Build Tools ("Desktop development with C++"). [Firefox Developer Edition](https://www.mozilla.org/firefox/developer/) is handy for loading the unsigned add-on.

```
setup.bat
```

This builds a **development copy** into `dist-dev\`. It is a separate Reynard: its own Firefox host name (`io.reynard.host.dev`), its own add-on ID (`reynard-dev@...`), its own settings and its own history. An installed Reynard keeps working untouched, so you can test a new build next to it.

To test a build:

1. Run `setup.bat`. If an installed Reynard exists, its yt-dlp and ffmpeg are reused, so nothing big is downloaded.
2. In Firefox open `about:debugging#/runtime/this-firefox`, choose *Load Temporary Add-on*, and pick `dist-dev\extension\manifest.json`. It shows up as **Reynard (dev)**, with its own fox, popup and a right-click entry called *Download with Reynard (dev)*. The normal add-on keeps its own.
3. Try it from the terminal first, without Firefox: `dist-dev\reynard.exe download <url>`, then `jobs`, `kill`, `match`. Then use the dev add-on the normal way.
4. The dev copy's settings are `dist-dev\settings.toml`, copied from your installed settings the first time. Edit it freely; the installed Reynard never sees it.

`cargo test` runs the unit tests. `uninstall.bat` in the project root removes only the dev registration.

```
src\            reynard.exe: host mode, command line, jobs, settings
extension\      the add-on (background, popup, options page, x-post.js)
scripts\        install-local.bat and uninstall.bat for end users
tools\          fetch.ps1 (downloads yt-dlp and ffmpeg), make-dev-extension.*, make-chrome-extension.*
chrome\         the Chrome (Manifest V3) manifest, service worker and add-on IDs
setup.bat       build and register the development copy (dist-dev\)
```

### Building on Arch Linux

```
sudo pacman -S --needed rust curl ffmpeg deno
./setup.sh
```

(`rustup` instead of `rust` works too: `sudo pacman -S rustup && rustup default stable`.) `setup.sh` is the counterpart of `setup.bat`: it builds the same source into `dist-dev/` as a separate dev copy (host `io.reynard.host.dev`, add-on ID `reynard-dev@...`, its own settings and history), downloads yt-dlp, and registers the dev host in `~/.mozilla/native-messaging-hosts`. Test it exactly like on Windows: *Load Temporary Add-on* with `dist-dev/extension/manifest.json`, or from the terminal with `dist-dev/reynard download <url>`. An installed Reynard is not touched.

`./make-release.sh` builds the real thing into `release-out/`: `reynard`, `install.sh`, `uninstall.sh`, the add-on zip and `SHA256SUMS.txt`. Hand over `reynard`, `install.sh` and `uninstall.sh` together; `install.sh` uses the `reynard` next to it (or `REYNARD_URL` at the top of the script, if you host it). It refuses to continue if the binary turns out to be a dev build. The add-on is the same on both systems, so one signed add-on serves both.

The platform differences live in a few `cfg` blocks in `src/`: Windows values are pinned by tests, Cancel stops yt-dlp's whole process group on Linux (`taskkill /T` on Windows), and the folder dialog is `zenity`/`kdialog` instead of PowerShell. `.gitattributes` keeps `.sh` files on Unix line endings.

## Credits and responsibility

Reynard is a front end for [yt-dlp](https://github.com/yt-dlp/yt-dlp) and uses [FFmpeg](https://ffmpeg.org/) builds from [yt-dlp/FFmpeg-Builds](https://github.com/yt-dlp/FFmpeg-Builds). It is not affiliated with either project. Only download content you have the right to download, and respect the terms of the sites you use.

## License

MIT. See the `LICENSE` file.
