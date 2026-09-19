#!/usr/bin/env bash
# Compile-check the Rust crate for Windows from macOS.
#
# Needs: rustup target add x86_64-pc-windows-msvc; brew install llvm (for llvm-rc).
# The updater feature is disabled because its HTTP stack compiles C sources
# (ring) that need a Windows C toolchain; the capability list is overridden
# to match. CI builds the full app on a real Windows runner.
set -euo pipefail
cd "$(dirname "$0")/../src-tauri"
export PATH="/opt/homebrew/opt/llvm/bin:$PATH"
command -v llvm-rc >/dev/null || { echo "llvm-rc not found; brew install llvm" >&2; exit 1; }
# The updater capability references a permission that only exists when the
# plugin is compiled in, so park it for the duration of the check.
CAP=capabilities/updater.json
PARKED="$(mktemp -d)/updater.json"
mv "$CAP" "$PARKED"
trap 'mv "$PARKED" "$CAP"' EXIT
cargo check --target x86_64-pc-windows-msvc --no-default-features --tests --examples "$@"
