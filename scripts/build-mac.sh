#!/bin/bash
# Builds the macOS universal release (Apple silicon + Intel) on this Mac and
# collects it in release/<version>/: the .dmg, the zipped .app, SHA256SUMS.txt
# and the updater files (see collect-release.sh).
#
# The updater signature needs ~/.tauri/neatnas.key (or TAURI_SIGNING_PRIVATE_KEY).
set -euo pipefail
cd "$(dirname "$0")/.."
source "$HOME/.cargo/env" 2>/dev/null || true
for t in aarch64-apple-darwin x86_64-apple-darwin; do
  rustup target list --installed | grep -qx "$t" || rustup target add "$t"
done
if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ] && [ -f "$HOME/.tauri/neatnas.key" ]; then
  export TAURI_SIGNING_PRIVATE_KEY="$HOME/.tauri/neatnas.key"
  export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"
fi
npm run tauri build -- --target universal-apple-darwin "$@"
scripts/collect-release.sh mac
