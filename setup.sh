#!/usr/bin/env bash
# =====================================================================
#  Development build of Reynard (Linux, the counterpart of setup.bat).
#
#  Builds into dist-dev/ and registers it as a SEPARATE "dev" Reynard: its own
#  Firefox host name, its own add-on ID, its own settings and history. An
#  installed Reynard (scripts/install.sh) keeps working untouched next to it.
# =====================================================================
set -euo pipefail
SELF="$(readlink -f "$0")"
cd "$(dirname "$SELF")"
. tools/common.sh

DEV_HOST="io.reynard.host.dev"
DEV_EXT_ID="reynard-dev@reynard-yamakoira"
INSTALLED="${XDG_DATA_HOME:-$HOME/.local/share}/reynard"

echo "== Checking what is needed =="
check_deps build || exit 1

echo "== Building Reynard dev =="
# the binary is compiled so that it answers to the dev add-on only
REYNARD_EXT_ID="$DEV_EXT_ID" cargo build --release --target-dir target-dev
mkdir -p dist-dev/bin
# copy next to it, then rename: that works even while the old one is running
cp target-dev/release/reynard dist-dev/reynard.new
mv -f dist-dev/reynard.new dist-dev/reynard

echo "== yt-dlp: borrow the installed copy if there is one, else download =="
if [ ! -x dist-dev/bin/yt-dlp ]; then
    if [ -x "$INSTALLED/bin/yt-dlp" ]; then
        cp "$INSTALLED/bin/yt-dlp" dist-dev/bin/yt-dlp
    else
        fetch_ytdlp dist-dev/bin/yt-dlp
    fi
fi

# the dev copy keeps its own settings.toml next to the binary
[ -f dist-dev/portable.txt ] || echo "Reynard dev copy: settings.toml is kept in this folder." > dist-dev/portable.txt
cfg="${XDG_CONFIG_HOME:-$HOME/.config}/reynard/settings.toml"
if [ ! -f dist-dev/settings.toml ] && [ -f "$cfg" ]; then cp "$cfg" dist-dev/settings.toml; fi

echo "== Creating the dev add-on =="
tools/make-dev-extension.sh dist-dev/extension "$DEV_EXT_ID" "$DEV_HOST"

echo "== Creating the dev add-on for Chrome =="
tools/make-chrome-extension.sh dist-dev/extension-chrome dev

echo "== Registering the dev host with Firefox and the Chrome family =="
write_host_manifest "$DEV_HOST" "Reynard native host (dev)" "$PWD/dist-dev/reynard" "$DEV_EXT_ID"
load_chrome_ids
write_chrome_hosts "$DEV_HOST" "Reynard native host (dev)" "$PWD/dist-dev/reynard" "$DEV_ID"

echo
echo "Settings file of the dev copy:"
./dist-dev/reynard settings
echo
echo "Done. An installed Reynard was not touched."
echo "To test: in Firefox open about:debugging#/runtime/this-firefox, choose"
echo "\"Load Temporary Add-on\" and pick dist-dev/extension/manifest.json"
echo "The dev add-on is called \"Reynard (dev)\" and only talks to dist-dev/reynard."
echo
echo "Chrome, Chromium, Brave or Edge: open chrome://extensions, switch on Developer mode,"
echo "choose \"Load unpacked\" and pick the folder dist-dev/extension-chrome"
