# shellcheck shell=bash
# Shared helpers for setup.sh and make-release.sh. Sourced, not run.
# (scripts/install.sh is meant to be copied around alone, so it carries its own copies.)

have() { command -v "$1" >/dev/null 2>&1; }

# check_deps build|run
# Lists what is missing and how to get it on Arch. Returns 1 if anything is.
#   run   = what Reynard needs at runtime: curl, ffmpeg (with ffprobe), deno
#   build = the same, plus Rust (rust or rustup) and a C linker
check_deps() {
    local mode="${1:-run}" pkgs=() notes=()
    have curl || pkgs+=(curl)
    { have ffmpeg && have ffprobe; } || pkgs+=(ffmpeg)
    have deno || pkgs+=(deno)
    if [ "$mode" = build ]; then
        if have cargo && cargo --version >/dev/null 2>&1; then
            :
        elif have rustup; then
            notes+=("rustup is installed but has no toolchain yet: run  rustup default stable")
        else
            pkgs+=(rust)
            notes+=("(rustup works too: sudo pacman -S rustup && rustup default stable)")
        fi
        { have cc || have gcc; } || pkgs+=(gcc)
    fi
    if [ ${#pkgs[@]} -eq 0 ] && [ ${#notes[@]} -eq 0 ]; then
        return 0
    fi
    echo
    if [ ${#pkgs[@]} -gt 0 ]; then
        echo "  Missing: ${pkgs[*]}"
        echo "  Install with:  sudo pacman -S --needed ${pkgs[*]}"
    fi
    local n
    for n in "${notes[@]}"; do echo "  $n"; done
    echo
    return 1
}

# The official standalone yt-dlp build (needs no Python), so the popup's Update button works.
ytdlp_url() {
    case "$(uname -m)" in
        x86_64)  echo "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux" ;;
        aarch64) echo "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux_aarch64" ;;
        *) return 1 ;;
    esac
}

# fetch_ytdlp DEST
fetch_ytdlp() {
    local url
    url=$(ytdlp_url) || { echo "  There is no standalone yt-dlp build for $(uname -m)." >&2; return 1; }
    echo "  Downloading yt-dlp ..."
    curl -L -f --retry 2 --progress-bar -o "$1.part" "$url" || { rm -f "$1.part"; return 1; }
    chmod +x "$1.part" && mv -f "$1.part" "$1"
}

# write_host_manifest NAME DESCRIPTION EXE_PATH EXTENSION_ID
# Tells Firefox where the native host is: one small .json file, no admin rights.
write_host_manifest() {
    local dir="$HOME/.mozilla/native-messaging-hosts" exe
    mkdir -p "$dir"
    exe=$(printf '%s' "$3" | sed 's/\\/\\\\/g; s/"/\\"/g')
    cat > "$dir/$1.json" <<JSON
{
  "name": "$1",
  "description": "$2",
  "path": "$exe",
  "type": "stdio",
  "allowed_extensions": ["$4"]
}
JSON
}

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

# shellcheck disable=SC2034  # the variables are read by the scripts that source this file
# Reads chrome/ids.txt into RELEASE_ID, RELEASE_KEY, DEV_ID and DEV_KEY.
load_chrome_ids() {
    local k v
    RELEASE_ID="" RELEASE_KEY="" DEV_ID="" DEV_KEY=""
    while IFS='=' read -r k v; do
        case "$k" in
            RELEASE_ID) RELEASE_ID="$v" ;;
            RELEASE_KEY) RELEASE_KEY="$v" ;;
            DEV_ID) DEV_ID="$v" ;;
            DEV_KEY) DEV_KEY="$v" ;;
        esac
    done < chrome/ids.txt
    { [ -n "$RELEASE_ID" ] && [ -n "$DEV_ID" ] && [ -n "$RELEASE_KEY" ] && [ -n "$DEV_KEY" ]; } || { echo "chrome/ids.txt is incomplete" >&2; return 1; }
}
