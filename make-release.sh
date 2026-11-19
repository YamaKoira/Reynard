#!/usr/bin/env bash
# =====================================================================
#  Builds everything you hand over, into release-out/ (counterpart of make-release.bat):
#    reynard                          the real build (works with the real add-on)
#    install.sh, uninstall.sh         for your users (install.sh uses the reynard next to it)
#    Reynard-extension-VERSION.zip    the add-on, to submit to addons.mozilla.org
#
#  Not to be confused with setup.sh, which makes the DEV copy in dist-dev/.
# =====================================================================
set -euo pipefail
SELF="$(readlink -f "$0")"
cd "$(dirname "$SELF")"
. tools/common.sh

echo "== Checking what is needed =="
check_deps build || exit 1

echo "== Building Reynard (release) =="
# make sure no dev add-on ID leaks into this build
env -u REYNARD_EXT_ID cargo build --release
rm -rf release-out
mkdir release-out
cp target/release/reynard release-out/reynard
cp scripts/install.sh scripts/uninstall.sh release-out/
chmod +x release-out/reynard release-out/install.sh release-out/uninstall.sh

# safety check: the binary says what it is, and a dev build must never be released
line=$(release-out/reynard version)
devbuild() {
    echo
    echo "STOP: this reynard is not a release build, it says: $line"
    echo "Do not release it. Run make-release.sh again from a fresh terminal."
    exit 1
}
case "$line" in *"(release)"*) ;; *) devbuild ;; esac
if grep -q 'reynard-dev@' release-out/reynard; then devbuild; fi

# the Chrome add-on IDs written into the installers must be the ones in chrome/ids.txt
load_chrome_ids
for f in scripts/install.sh scripts/install-local.bat; do
    grep -q "$RELEASE_ID" "$f" || { echo "STOP: $f does not contain the release Chrome ID $RELEASE_ID (see chrome/ids.txt)" >&2; exit 1; }
    if [ -n "$STORE_ID" ]; then grep -q "$STORE_ID" "$f" || { echo "STOP: $f does not contain the store ID $STORE_ID (see chrome/ids.txt)" >&2; exit 1; }; fi
done

# the version number comes from the add-on manifest
ver=$(sed -n 's/.*"version":[[:space:]]*"\([^"]*\)".*/\1/p' extension/manifest.json | head -n 1)
[ -n "$ver" ] || { echo "no version found in extension/manifest.json" >&2; exit 1; }
echo "== Packing the add-on $ver =="
zipfile="$PWD/release-out/Reynard-extension-$ver.zip"
if have bsdtar; then
    bsdtar -a -c -f "$zipfile" -C extension .
elif have zip; then
    (cd extension && zip -q -r -X "$zipfile" .)
else
    echo "Need bsdtar (package libarchive) or zip to pack the add-on." >&2
    exit 1
fi
echo "== Chrome add-on =="
tools/make-chrome-extension.sh release-out/extension-chrome release
tools/make-chrome-extension.sh /tmp/reynard-chrome-store.$$ store
chrome_zip="$PWD/release-out/Reynard-chrome-$ver.zip"
if have bsdtar; then
    bsdtar -a -c -f "$chrome_zip" -C /tmp/reynard-chrome-store.$$ .
else
    (cd /tmp/reynard-chrome-store.$$ && zip -q -r -X "$chrome_zip" .)
fi
rm -rf /tmp/reynard-chrome-store.$$
# checksum, so people can verify the download
(cd release-out && sha256sum reynard > SHA256SUMS.txt)

echo
echo "Ready in release-out/ :"
ls release-out
echo
echo "Handing it over directly? Send reynard, install.sh and uninstall.sh together."
echo "Submit Reynard-extension-$ver.zip to addons.mozilla.org."
echo "Chrome: Reynard-chrome-$ver.zip is for the Chrome Web Store; extension-chrome/ loads unpacked."
