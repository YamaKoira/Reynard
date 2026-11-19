#!/usr/bin/env bash
# Makes the Chrome (Manifest V3) version of the add-on from the same sources as the Firefox one.
# Usage: tools/make-chrome-extension.sh OUT_DIR MODE
#   dev      "Reynard (dev)": its own ID, talks to the dev copy of Reynard (for setup.sh)
#   release  the real add-on with the release ID, to load unpacked (Developer mode)
set -euo pipefail
cd "$(dirname "$0")/.."
. tools/common.sh
out="${1:?output folder}"; mode="${2:?dev or release}"
load_chrome_ids

version=$(sed -n 's/.*"version":[[:space:]]*"\([^"]*\)".*/\1/p' extension/manifest.json | head -n 1)
[ -n "$version" ] || { echo "no version found in extension/manifest.json" >&2; exit 1; }

rm -rf "$out"
cp -r extension "$out"
cp chrome/service-worker.js "$out/service-worker.js"
m="$out/manifest.json"
cp chrome/manifest.json "$m"
sed -i "s/__VERSION__/$version/" "$m"
case "$mode" in
    dev)
        sed -i "s|__KEY__|$DEV_KEY|" "$m"
        sed -i -e 's/"name": "Reynard"/"name": "Reynard (dev)"/' -e 's/"default_title": "Reynard"/"default_title": "Reynard (dev)"/' "$m"
        printf 'var REYNARD_HOST = "io.reynard.host.dev";\nvar REYNARD_LABEL = "Reynard (dev)";\n' > "$out/config.js"
        ;;
    release) sed -i "s|__KEY__|$RELEASE_KEY|" "$m" ;;
    *) echo "mode must be dev or release" >&2; exit 1 ;;
esac
# sanity checks
if grep -q '__[A-Z]*__' "$m"; then echo "an unfilled placeholder is left in manifest.json" >&2; exit 1; fi
grep -q '"key": "MII' "$m" || { echo "the key was not put into manifest.json" >&2; exit 1; }
grep -q '"manifest_version": 3' "$m" || { echo "not a Manifest V3 file" >&2; exit 1; }
echo "Chrome add-on ($mode) ready in $out"
