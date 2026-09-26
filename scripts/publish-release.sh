#!/bin/bash
# Publish a GitHub release from locally built bundles (the fallback when the
# Release workflow is not used).
#
#   scripts/build-mac.sh && scripts/build-windows.sh
#   scripts/publish-release.sh v1.0.0 [--draft]
#
# Uploads release/<version>/: the .dmg, the macOS zip, the Windows installer,
# the Windows zip, SHA256SUMS.txt and the updater files, plus a latest.json
# (written to release/<version>/updater/) that points running copies of the
# app at the new version.
set -euo pipefail
cd "$(dirname "$0")/.."
tag="${1:?usage: publish-release.sh vX.Y.Z [--draft]}"
version="${tag#v}"
repo="jason-jm/neat-nas"
dir="release/$version"
[ "$(node -p "require('./package.json').version")" = "$version" ] || { echo "package.json is not at $version"; exit 1; }

dmg="$dir/Neat NAS_${version}_universal.dmg"
mac_zip="$dir/Neat NAS_${version}_universal-mac.zip"
exe="$dir/Neat NAS_${version}_x64-setup.exe"
win_zip="$dir/Neat NAS_${version}_x64-win.zip"
app_tgz="$dir/updater/Neat NAS.app.tar.gz"
app_sig="$app_tgz.sig"
exe_sig="$dir/updater/Neat NAS_${version}_x64-setup.exe.sig"
for f in "$dmg" "$mac_zip" "$exe" "$win_zip" "$app_tgz" "$app_sig" "$exe_sig"; do
  [ -f "$f" ] || { echo "missing $f (run scripts/build-mac.sh and scripts/build-windows.sh)"; exit 1; }
done
scripts/collect-release.sh sums >/dev/null

# GitHub replaces spaces in asset names with dots.
asset_url() { echo "https://github.com/$repo/releases/download/$tag/$(basename "$1" | sed 's/ /./g')"; }
python3 - "$dir/updater/latest.json" "$version" "$(asset_url "$app_tgz")" "$(cat "$app_sig")" "$(asset_url "$exe")" "$(cat "$exe_sig")" <<'PY'
import json, sys, datetime
path, version, mac_url, mac_sig, win_url, win_sig = sys.argv[1:]
mac = {"signature": mac_sig, "url": mac_url}
manifest = {
    "version": version,
    "notes": f"Neat NAS {version}. See the release page for details.",
    "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "platforms": {
        "darwin-universal": mac,
        "darwin-aarch64": mac,
        "darwin-x86_64": mac,
        "windows-x86_64": {"signature": win_sig, "url": win_url},
    },
}
open(path, "w").write(json.dumps(manifest, indent=2) + "\n")
PY

notes="docs/marketing/release-notes-$tag.md"
[ -f "$notes" ] || notes="RELEASE.md"
draft=""; [ "${2:-}" = "--draft" ] && draft="--draft"
gh release create "$tag" --repo "$repo" --title "Neat NAS $version" --notes-file "$notes" $draft \
  "$dmg" "$mac_zip" "$exe" "$win_zip" "$dir/SHA256SUMS.txt" "$app_tgz" "$app_sig" "$exe_sig" "$dir/updater/latest.json"
echo "published $tag"
