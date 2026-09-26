#!/bin/bash
# Gathers finished bundles into release/<version>/ and writes SHA256SUMS.txt.
#
#   scripts/collect-release.sh [mac|windows|all|sums]     (default: all)
#   scripts/collect-release.sh github vX.Y.Z              (copy a GitHub release)
#   scripts/collect-release.sh check                      (fail if this version is on GitHub)
#
# Result, per version:
#   Neat NAS_<v>_universal.dmg       macOS disk image
#   Neat NAS_<v>_universal-mac.zip   macOS app, zipped (no installer)
#   Neat NAS_<v>_x64-setup.exe       Windows installer
#   Neat NAS_<v>_x64-win.zip         Windows app, zipped (no installer)
#   SHA256SUMS.txt                   checksums of the files above
#   updater/                         what the in-app updater downloads (.app.tar.gz, .sig, latest.json)
#
# build-mac.sh and build-windows.sh call this after building; release.sh uses
# "github" to copy what the Release workflow published. "sums" only rewrites
# SHA256SUMS.txt.
set -euo pipefail
cd "$(dirname "$0")/.."
mode="${1:-all}"
repo="jason-jm/neat-nas"
if [ "$mode" = github ]; then
  tag="${2:?usage: collect-release.sh github vX.Y.Z}"
  version="${tag#v}"
else
  version=$(node -p "require('./package.json').version")
fi
out="release/$version"
got=0

# release/<v> of a published version must keep exactly the published files,
# so local builds of that version are never collected over them.
ensure_unpublished() {
  if gh release view "v$version" --repo "$repo" >/dev/null 2>&1; then
    echo "v$version is already on GitHub, so release/$version keeps the published files."
    echo "Bump the version before building (scripts/release.sh does), or copy it with: scripts/collect-release.sh github v$version"
    exit 1
  fi
}
if [ "$mode" = check ]; then ensure_unpublished; exit 0; fi
mkdir -p "$out/updater"

collect_mac() {
  local b="src-tauri/target/universal-apple-darwin/release/bundle"
  local dmg="$b/dmg/Neat NAS_${version}_universal.dmg" app="$b/macos/Neat NAS.app" f
  if [ ! -f "$dmg" ] || [ ! -d "$app" ]; then
    if [ "$mode" = mac ]; then echo "no macOS $version build in $b (run scripts/build-mac.sh)"; exit 1; fi
    return 0
  fi
  cp "$dmg" "$out/"
  rm -f "$out/Neat NAS_${version}_universal-mac.zip"
  ditto -c -k --sequesterRsrc --keepParent "$app" "$out/Neat NAS_${version}_universal-mac.zip"
  for f in "$b/macos/Neat NAS.app.tar.gz" "$b/macos/Neat NAS.app.tar.gz.sig"; do
    if [ -f "$f" ]; then cp "$f" "$out/updater/"; fi
  done
  got=1
}

collect_windows() {
  local r="src-tauri/target/x86_64-pc-windows-msvc/release"
  local exe="$r/bundle/nsis/Neat NAS_${version}_x64-setup.exe" stage
  if [ ! -f "$exe" ]; then
    if [ "$mode" = windows ]; then echo "no Windows $version build in $r (run scripts/build-windows.sh)"; exit 1; fi
    return 0
  fi
  command -v 7zz >/dev/null || { echo "7zz not found: brew install sevenzip"; exit 1; }
  cp "$exe" "$out/"
  if [ -f "$exe.sig" ]; then cp "$exe.sig" "$out/updater/"; fi
  # The zip holds exactly the exe the installer installs: the bundler marks
  # that copy as an NSIS install for the updater, while the build folder keeps
  # an unmarked one. It is a single self-contained file (the installer adds
  # only an uninstaller and shortcuts); WebView2 ships with Windows 11 and
  # current Windows 10.
  stage=$(mktemp -d)
  7zz e -y -o"$stage" "$exe" neatnas.exe >/dev/null
  mv "$stage/neatnas.exe" "$stage/Neat NAS.exe"
  rm -f "$out/Neat NAS_${version}_x64-win.zip"
  zip -X -q -j "$out/Neat NAS_${version}_x64-win.zip" "$stage/Neat NAS.exe"
  rm -rf "$stage"
  got=1
}

collect_github() {
  local f name local_name
  gh release download "$tag" --repo "$repo" --dir "$out" --clobber
  shopt -s nullglob
  for f in "$out"/*; do
    [ -f "$f" ] || continue
    name=$(basename "$f")
    local_name="${name/#Neat.NAS/Neat NAS}"    # GitHub stores spaces as dots
    case "$name" in
      *.sig|*.app.tar.gz|latest.json) mv -f "$f" "$out/updater/$local_name" ;;
      *) if [ "$name" != "$local_name" ]; then mv -f "$f" "$out/$local_name"; fi ;;
    esac
  done
  shopt -u nullglob
  got=1
}

write_sums() {
  local files=() f pattern
  shopt -s nullglob
  # Updater signatures belong in updater/, not next to the installers.
  for f in "$out"/*.sig; do mv "$f" "$out/updater/"; done
  for pattern in "*.dmg" "*-mac.zip" "*-setup.exe" "*.msi" "*-win.zip"; do
    for f in "$out"/$pattern; do files+=("$(basename "$f")"); done
  done
  shopt -u nullglob
  if [ ${#files[@]} -eq 0 ]; then echo "nothing to checksum in $out"; exit 1; fi
  (cd "$out" && shasum -a 256 "${files[@]}" > SHA256SUMS.txt)
}

case "$mode" in
  mac) ensure_unpublished; collect_mac ;;
  windows) ensure_unpublished; collect_windows ;;
  all)
    ensure_unpublished
    collect_mac
    collect_windows
    if [ "$got" != 1 ]; then echo "no $version builds found; run scripts/build-mac.sh or scripts/build-windows.sh"; exit 1; fi
    ;;
  github) collect_github ;;
  sums) ;;
  *) echo "usage: collect-release.sh [mac|windows|all|sums|check] | github vX.Y.Z"; exit 1 ;;
esac
write_sums
echo "$out:"
(cd "$out" && ls -l | tail -n +2 | awk '{ $1=$2=$3=$4=""; print }' | sed 's/^ *//')
