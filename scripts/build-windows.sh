#!/bin/bash
# Cross-build the Windows x64 release (NSIS installer) from macOS.
#
# Needs: rustup target x86_64-pc-windows-msvc, cargo-xwin, Homebrew llvm,
# makensis and sevenzip (brew install llvm makensis sevenzip; cargo install
# cargo-xwin). The first
# run downloads the Windows SDK/CRT (~1 GB) into cargo-xwin's cache.
#
# Homebrew's makensis 3.12 aborts with std::bad_alloc on recent macOS unless
# a debugger is attached, so when the plain binary fails a self-test the
# script runs it under lldb through a shim placed first on PATH.
#
# The installer, the zipped exe, SHA256SUMS.txt and the updater signature are
# collected in release/<version>/ (see collect-release.sh). The signature is
# produced when ~/.tauri/neatnas.key exists (or TAURI_SIGNING_PRIVATE_KEY
# is already set).
set -euo pipefail
cd "$(dirname "$0")/.."
source "$HOME/.cargo/env" 2>/dev/null || true
export PATH="/opt/homebrew/opt/llvm/bin:$PATH"

for tool in cargo-xwin makensis llvm-rc clang-cl; do
  command -v "$tool" >/dev/null || { echo "missing $tool (see comments at the top of this script)"; exit 1; }
done
rustup target list --installed | grep -q x86_64-pc-windows-msvc || rustup target add x86_64-pc-windows-msvc

if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ] && [ -f "$HOME/.tauri/neatnas.key" ]; then
  export TAURI_SIGNING_PRIVATE_KEY="$HOME/.tauri/neatnas.key"
  export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"
fi

# Self-test makensis; fall back to running it under lldb if it crashes.
shim=$(mktemp -d)
trap 'rm -rf "$shim"' EXIT
printf 'OutFile "%s/t.exe"\nSection\nSectionEnd\n' "$shim" > "$shim/t.nsi"
if ! makensis -V0 "$shim/t.nsi" >/dev/null 2>&1; then
  real=$(command -v makensis)
  cat > "$shim/makensis" <<SHIM
#!/bin/bash
out=\$(lldb --batch -o run -- "$real" "\$@" 2>&1); rc=\$?
printf '%s\n' "\$out" | grep -vE '^\(lldb\) |^Current executable set|^Process [0-9]+ launched'
status=\$(printf '%s\n' "\$out" | sed -n 's/.*exited with status = \([0-9]*\).*/\1/p' | tail -1)
[ -n "\$status" ] && exit "\$status"; exit "\${rc:-1}"
SHIM
  chmod +x "$shim/makensis"
  export PATH="$shim:$PATH"
  echo "makensis crashes on this machine; running it under lldb"
fi

npm run tauri build -- --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis "$@"

scripts/collect-release.sh windows
