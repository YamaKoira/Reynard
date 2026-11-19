#!/usr/bin/env bash
# =====================================================================
#  Reynard installer for Arch Linux. It is a plain script: read it first.
#
#  What it does, in order:
#    1. Puts reynard into  ~/.local/share/reynard  (the "reynard" file next to this
#       script, put there by make-release.sh)
#    2. Downloads yt-dlp (the official standalone build) into  ~/.local/share/reynard/bin
#       (skipped if it is already there; the popup's Update button keeps it current)
#    3. Tells Firefox where reynard is: one small .json file in
#       ~/.mozilla/native-messaging-hosts. The same for Chrome, Chromium, Brave, Edge and
#       Vivaldi, if you use them (a small .json in each one's NativeMessagingHosts folder)
#  ffmpeg and Deno come from your package manager: the script checks they are there.
#  No root needed. Nothing else on your system is changed.
#  uninstall.sh removes all of it. Run this script again to update.
# =====================================================================
set -euo pipefail

# Chrome, Chromium, Brave, Edge and Vivaldi each read native-host manifests from their own
# config folder. Only browsers that have been run (so the folder exists) are registered.
# write_chrome_hosts NAME DESCRIPTION EXE_PATH ADDON_ID...   (empty IDs are skipped)
write_chrome_hosts() {
    local name="$1" desc="$2" exe="$3" origins="" id d dir count=0 jexe cfg="${XDG_CONFIG_HOME:-$HOME/.config}"
    shift 3
    for id in "$@"; do
        if [ -n "$id" ]; then origins="${origins:+$origins, }\"chrome-extension://$id/\""; fi
    done
    jexe=$(printf '%s' "$exe" | sed 's/\\/\\\\/g; s/"/\\"/g')
    for d in google-chrome chromium BraveSoftware/Brave-Browser microsoft-edge vivaldi; do
        [ -d "$cfg/$d" ] || continue
        dir="$cfg/$d/NativeMessagingHosts"
        mkdir -p "$dir"
        cat > "$dir/$name.json" <<JSON
{
  "name": "$name",
  "description": "$desc",
  "path": "$jexe",
  "type": "stdio",
  "allowed_origins": [$origins]
}
JSON
        echo "      Chrome family: $d"
        count=$((count + 1))
    done
    if [ "$count" -eq 0 ]; then
        echo "      No Chrome, Chromium, Brave, Edge or Vivaldi profile found. Start the browser once and run this again."
    fi
}

# remove_chrome_hosts NAME
remove_chrome_hosts() {
    local d cfg="${XDG_CONFIG_HOME:-$HOME/.config}"
    for d in google-chrome chromium BraveSoftware/Brave-Browser microsoft-edge vivaldi; do
        rm -f "$cfg/$d/NativeMessagingHosts/$1.json"
    done
}

# ---- the only things that ever need editing ----
EXT_ID="reynard@reynard-yamakoira"
CHROME_ID="gcdmloapgnddiblfbcpoonneajgmhpmj"   # the add-on loaded unpacked (see chrome/ids.txt)
HOST_NAME="io.reynard.host"
RF_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/reynard"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

have() { command -v "$1" >/dev/null 2>&1; }
fail() { echo; echo "  $*" >&2; exit 1; }

case "$(uname -m)" in
    x86_64)  YTDLP_URL="https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux" ;;
    aarch64) YTDLP_URL="https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux_aarch64" ;;
    *) fail "There is no standalone yt-dlp build for $(uname -m)." ;;
esac

echo
echo "  Reynard installer"
echo "  -----------------"
echo "  Install folder : $RF_DIR"
echo "  Downloads      : yt-dlp (and reynard, if it is not next to this script)"
echo "  Firefox link   : ~/.mozilla/native-messaging-hosts/$HOST_NAME.json"
echo

# ---- what has to be on the system already ----
missing=()
have curl || missing+=(curl)
{ have ffmpeg && have ffprobe; } || missing+=(ffmpeg)
have deno || missing+=(deno)
if [ ${#missing[@]} -gt 0 ]; then
    echo "  Missing: ${missing[*]}"
    echo "  Install with:  sudo pacman -S --needed ${missing[*]}"
    echo
    have curl || fail "curl is needed to download yt-dlp. Install it and run this script again."
    echo "  Without ffmpeg many downloads cannot be merged or converted, and without"
    echo "  Deno YouTube does not work."
    if [ -t 0 ]; then
        read -r -p "  Install anyway? [y/N] " answer
        case "$answer" in y|Y) ;; *) exit 1 ;; esac
    else
        exit 1
    fi
elif [ -t 0 ]; then
    read -r -p "  Press Enter to install, or Ctrl+C to cancel. "
fi
echo

# ---- 1. reynard ----
echo "[1/3] Installing reynard ..."
mkdir -p "$RF_DIR/bin"
if [ -f "$HERE/reynard" ]; then
    src="$HERE/reynard"
else
    fail "There is no \"reynard\" file next to this script. Run make-release.sh in the project folder and use the install.sh in release-out."
fi
# copy next to it, then rename: this works even while the old one is running
cp "$src" "$RF_DIR/reynard.new"
chmod 755 "$RF_DIR/reynard.new"
mv -f "$RF_DIR/reynard.new" "$RF_DIR/reynard"

# ---- 2. yt-dlp ----
echo "[2/3] Checking yt-dlp ..."
if [ -x "$RF_DIR/bin/yt-dlp" ]; then
    echo "      yt-dlp is already there (use the popup's Update button to update it)."
else
    echo "      Downloading yt-dlp ..."
    curl -L -f --retry 2 --progress-bar -o "$RF_DIR/bin/yt-dlp.part" "$YTDLP_URL" \
        || { rm -f "$RF_DIR/bin/yt-dlp.part"; fail "A download failed. Check your internet connection and try again."; }
    chmod +x "$RF_DIR/bin/yt-dlp.part"
    mv -f "$RF_DIR/bin/yt-dlp.part" "$RF_DIR/bin/yt-dlp"
fi
"$RF_DIR/bin/yt-dlp" --version >/dev/null 2>&1 || fail "yt-dlp was installed but would not start."

# ---- 3. tell Firefox about it ----
echo "[3/3] Registering with your browsers ..."
mkdir -p "$HOME/.mozilla/native-messaging-hosts"
json_path=$(printf '%s' "$RF_DIR/reynard" | sed 's/\\/\\\\/g; s/"/\\"/g')
cat > "$HOME/.mozilla/native-messaging-hosts/$HOST_NAME.json" <<JSON
{
  "name": "$HOST_NAME",
  "description": "Reynard native host",
  "path": "$json_path",
  "type": "stdio",
  "allowed_extensions": ["$EXT_ID"]
}
JSON

write_chrome_hosts "$HOST_NAME" "Reynard native host" "$RF_DIR/reynard" "$CHROME_ID"

# ---- check that it starts (also creates your settings file) ----
"$RF_DIR/reynard" settings || fail "reynard was installed but would not start."

echo
echo "  Reynard is installed."
echo "  Last step: add the Reynard add-on to your browser."
echo "    Firefox: about:addons, the gear icon, Install Add-on From File."
echo "    Chrome family: chrome://extensions, Developer mode, Load unpacked (the extension-chrome"
echo "    folder)."
echo
