#!/usr/bin/env bash
# Makes the development copy of the add-on: the same code, but with its own ID, its own
# name, and talking to the dev copy of Reynard.
# Usage: tools/make-dev-extension.sh OUT_DIR EXTENSION_ID HOST_NAME
set -euo pipefail
cd "$(dirname "$0")/.."
out="${1:?output folder}"; ext_id="${2:?extension id}"; host="${3:?host name}"

rm -rf "$out"
cp -r extension "$out"
# 1. Which Reynard it talks to, and what it is called on screen.
printf 'var REYNARD_HOST = "%s";\nvar REYNARD_LABEL = "Reynard (dev)";\n' "$host" > "$out/config.js"
# 2. Its own identity in the manifest. Plain text replacement, so nothing else changes.
m="$out/manifest.json"
sed -i -e 's/"name": "Reynard"/"name": "Reynard (dev)"/' \
       -e 's/"default_title": "Reynard"/"default_title": "Reynard (dev)"/' \
       -e '0,/"id":[[:space:]]*"[^"]*"/s//"id": "'"$ext_id"'"/' "$m"
cmp -s "$m" extension/manifest.json && { echo "manifest.json was not changed" >&2; exit 1; }
grep -q "\"id\": \"$ext_id\"" "$m" || { echo "no add-on id found in manifest.json" >&2; exit 1; }
echo "Dev add-on ready in $out"
