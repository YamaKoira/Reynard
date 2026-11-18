#!/usr/bin/env bash
# Removes what install.sh added: the Firefox link and the Reynard folder.
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

HOST_NAME="io.reynard.host"
RF_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/reynard"
CFG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/reynard"
HOST_JSON="$HOME/.mozilla/native-messaging-hosts/$HOST_NAME.json"

echo
echo "  This removes Reynard:"
echo "    - the Firefox link  $HOST_JSON, and the same link for Chrome, Chromium, Brave, Edge and Vivaldi"
echo "    - the folder  $RF_DIR  with reynard, yt-dlp and your download history"
echo "  Your downloaded videos are not touched."
echo
read -r -p "  Continue? [y/N] " answer
case "$answer" in y|Y) ;; *) exit 0 ;; esac

if command -v pgrep >/dev/null 2>&1 && pgrep -f "$RF_DIR/reynard run" >/dev/null 2>&1; then
    echo
    echo "  A download is running right now. Wait for it to finish, then try again."
    exit 1
fi

rm -f "$HOST_JSON"
remove_chrome_hosts "$HOST_NAME"
rm -rf "$RF_DIR"
if [ -e "$RF_DIR" ]; then
    echo "  Some files could not be deleted. Delete the folder $RF_DIR by hand."
    exit 1
fi

if [ -d "$CFG_DIR" ]; then
    echo
    echo "  Your settings file is kept in $CFG_DIR"
    read -r -p "  Delete that too? [y/N] " answer
    case "$answer" in y|Y) rm -rf "$CFG_DIR" ;; esac
fi
echo
echo "  Done. Last step: remove the Reynard add-on in your browser (about:addons, chrome://extensions)."
echo
