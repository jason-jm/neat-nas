#!/bin/bash
# Builds the macOS universal release (Apple silicon + Intel) on this Mac, signs
# it with the Developer ID in the login keychain, has Apple notarize it
# (scripts/notarize-mac.sh) and collects it in release/<version>/.
#
#   scripts/build-mac.sh               build, sign, notarize, collect
#   scripts/build-mac.sh --preflight   only check that signing and notarizing will work
#   NO_COLLECT=1 scripts/build-mac.sh  build and notarize without touching release/
#
# Identity: APPLE_SIGNING_IDENTITY, else the first "Developer ID Application"
# in the keychain, else ad-hoc ("-", which cannot be notarized). Notarization
# uses the notarytool keychain profile APPLE_KEYCHAIN_PROFILE (default
# WAVESUBS_NOTARY, the team's saved credentials; create one with
# `xcrun notarytool store-credentials`). The updater signature needs
# ~/.tauri/neatnas.key (or TAURI_SIGNING_PRIVATE_KEY).
set -euo pipefail
cd "$(dirname "$0")/.."

identity="${APPLE_SIGNING_IDENTITY:-$(security find-identity -v -p codesigning | sed -n 's/.*"\(Developer ID Application: [^"]*\)".*/\1/p' | head -n 1)}"
identity="${identity:--}"
profile="${APPLE_KEYCHAIN_PROFILE:-WAVESUBS_NOTARY}"
if [ "$identity" = "-" ]; then
  echo "no Developer ID Application identity in the keychain: signing ad-hoc, without notarization"
elif ! xcrun notarytool history --keychain-profile "$profile" >/dev/null 2>&1; then
  echo "the notarytool keychain profile \"$profile\" does not work; save it with:"
  echo "  xcrun notarytool store-credentials \"$profile\" --apple-id <Apple ID> --team-id <team ID>"
  exit 1
fi
if [ "${1:-}" = --preflight ]; then
  if [ "$identity" = "-" ]; then echo "macOS: ad-hoc signing"; else echo "macOS: $identity, notarized with profile $profile"; fi
  exit 0
fi

[ "${NO_COLLECT:-}" = 1 ] || scripts/collect-release.sh check
source "$HOME/.cargo/env" 2>/dev/null || true
for t in aarch64-apple-darwin x86_64-apple-darwin; do
  rustup target list --installed | grep -qx "$t" || rustup target add "$t"
done
if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ] && [ -f "$HOME/.tauri/neatnas.key" ]; then
  export TAURI_SIGNING_PRIVATE_KEY="$HOME/.tauri/neatnas.key"
  export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"
fi
# Tauri signs with this identity; notarization happens afterwards from the
# keychain profile, so Tauri's own notarization and certificate import stay off.
unset APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID APPLE_API_KEY APPLE_API_ISSUER APPLE_API_KEY_PATH APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD
export APPLE_SIGNING_IDENTITY="$identity"
npm run tauri build -- --target universal-apple-darwin "$@"
if [ "$identity" != "-" ]; then scripts/notarize-mac.sh "$identity" "$profile"; fi
[ "${NO_COLLECT:-}" = 1 ] || scripts/collect-release.sh mac
