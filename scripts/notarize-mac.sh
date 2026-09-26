#!/bin/bash
# Has Apple notarize the universal build in src-tauri/target and staples the
# tickets, the way Wave Subs does it: first the .app (so the zipped app and the
# app inside the disk image carry their ticket), then the .dmg, rebuilt with
# the stapled app but keeping Tauri's window layout, signed and notarized too.
# The updater archive keeps the app as Tauri built it: its minisign signature
# stays valid, and copies installed by the updater are not quarantined.
#
#   scripts/notarize-mac.sh "<Developer ID Application: ...>" <notarytool keychain profile>
set -euo pipefail
cd "$(dirname "$0")/.."
identity="${1:?usage: notarize-mac.sh <identity> <keychain profile>}"
profile="${2:?usage: notarize-mac.sh <identity> <keychain profile>}"
version=$(node -p "require('./package.json').version")
b="src-tauri/target/universal-apple-darwin/release/bundle"
app="$b/macos/Neat NAS.app"
dmg="$b/dmg/Neat NAS_${version}_universal.dmg"
[ -d "$app" ] && [ -f "$dmg" ] || { echo "no $version build in $b"; exit 1; }
work=$(mktemp -d)
cleanup() { hdiutil detach "$work/mnt" -quiet -force 2>/dev/null || true; rm -rf "$work"; }
trap cleanup EXIT

submit() {  # <file> <label>
  local result id status
  echo "notarizing $2 (this usually takes a few minutes)"
  result=$(xcrun notarytool submit "$1" --keychain-profile "$profile" --wait --timeout 45m --output-format json)
  id=$(printf '%s' "$result" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("id", ""))')
  status=$(printf '%s' "$result" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("status", ""))')
  echo "  $2: $status (submission $id)"
  if [ "$status" != "Accepted" ]; then
    [ -n "$id" ] && xcrun notarytool log "$id" --keychain-profile "$profile" || printf '%s\n' "$result"
    exit 1
  fi
}

# 1. The app.
ditto -c -k --keepParent "$app" "$work/app.zip"
submit "$work/app.zip" "Neat NAS.app"
xcrun stapler staple -q "$app"
xcrun stapler validate -q "$app"

# 2. The disk image: swap in the stapled app, sign, notarize, staple.
hdiutil convert "$dmg" -quiet -format UDRW -o "$work/rw.dmg"
mkdir "$work/mnt"
hdiutil attach "$work/rw.dmg" -quiet -nobrowse -noverify -noautoopen -mountpoint "$work/mnt"
rm -rf "$work/mnt/Neat NAS.app"
ditto "$app" "$work/mnt/Neat NAS.app"
for _ in 1 2 3 4 5; do hdiutil detach "$work/mnt" -quiet && break; sleep 2; done
[ ! -d "$work/mnt/Neat NAS.app" ] || hdiutil detach "$work/mnt" -quiet -force
hdiutil convert "$work/rw.dmg" -quiet -format UDZO -imagekey zlib-level=9 -o "$work/final.dmg"
codesign --force --sign "$identity" --timestamp "$work/final.dmg"
submit "$work/final.dmg" "$(basename "$dmg")"
xcrun stapler staple -q "$work/final.dmg"
xcrun stapler validate -q "$work/final.dmg"
mv -f "$work/final.dmg" "$dmg"

echo "Gatekeeper:"
spctl -a -vv -t exec "$app" 2>&1 | sed 's/^/  /'
spctl -a -vv -t open --context context:primary-signature "$dmg" 2>&1 | sed 's/^/  /'
