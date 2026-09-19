#!/bin/bash
# Publish a GitHub release from locally built bundles.
#
#   scripts/publish-release.sh v1.0.0 [--draft]
#
# Expects the macOS universal build and the Windows build to exist (see
# RELEASE.md). Uploads the .dmg, the updater .app.tar.gz + .sig, the Windows
# -setup.exe + .sig, and a latest.json manifest that points at those assets,
# which is what running copies of the app poll for updates.
set -euo pipefail
cd "$(dirname "$0")/.."
tag="${1:?usage: publish-release.sh vX.Y.Z [--draft]}"
version="${tag#v}"
repo="jason-jm/neat-nas"
mac="src-tauri/target/universal-apple-darwin/release/bundle"
win="src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis"

dmg=$(ls "$mac"/dmg/*.dmg)
app_tgz=$(ls "$mac"/macos/*.app.tar.gz)
app_sig="$app_tgz.sig"
exe=$(ls "$win"/*-setup.exe)
exe_sig="$exe.sig"
for f in "$dmg" "$app_tgz" "$app_sig" "$exe" "$exe_sig"; do [ -f "$f" ] || { echo "missing $f"; exit 1; }; done

# GitHub replaces spaces in asset names with dots.
asset_url() { echo "https://github.com/$repo/releases/download/$tag/$(basename "$1" | sed 's/ /./g')"; }
out=$(mktemp -d); trap 'rm -rf "$out"' EXIT
python3 - "$out/latest.json" "$version" "$(asset_url "$app_tgz")" "$(cat "$app_sig")" "$(asset_url "$exe")" "$(cat "$exe_sig")" <<'PY'
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
  "$dmg" "$app_tgz" "$app_sig" "$exe" "$exe_sig" "$out/latest.json"
echo "published $tag"
