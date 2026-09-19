# Releasing Neat NAS

## What a release produces

| Platform | Artifact | Notes |
|---|---|---|
| macOS | `Neat NAS_<version>_universal.dmg`, `Neat NAS.app.tar.gz` (+ `.sig`) | Universal binary (Apple Silicon + Intel). The `.tar.gz` is what the in-app updater downloads. |
| Windows | `Neat NAS_<version>_x64-setup.exe` (NSIS), `Neat NAS_<version>_x64_en-US.msi` (+ `.sig`) | Per-user install, Chinese and English installer UI, WebView2 bootstrapped if missing. |
| Both | `latest.json` | Updater manifest, only when `TAURI_SIGNING_PRIVATE_KEY` is set. |

## Cutting a release

1. Bump the version in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json` (keep them identical).
2. Commit, then tag and push:
   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```
3. The `Release` workflow builds macOS and Windows and opens a **draft** GitHub release with the artifacts attached. Review it, then publish.
4. Publishing makes `latest.json` reachable at `https://github.com/jason-jm/neat-nas/releases/latest/download/latest.json`, which is what running copies poll.

## One-time setup

### Updater endpoint

`plugins.updater.endpoints` in `src-tauri/tauri.conf.json` points at the `jason-jm/neat-nas` repository; change it if the releases move elsewhere.

### Updater signing key

The keypair was generated with `tauri signer generate` and lives at:

- private key: `~/.tauri/neatnas.key` — **never commit this**; losing it means existing installs can no longer verify updates.
- public key: `~/.tauri/neatnas.key.pub` — already embedded in `tauri.conf.json`.

Add repository secrets:

- `TAURI_SIGNING_PRIVATE_KEY` = contents of `~/.tauri/neatnas.key`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` = empty (the key has no password)

### macOS signing and notarization (recommended)

Unsigned builds work but Gatekeeper shows "cannot be opened because the developer cannot be verified"; users must right-click → Open once. With an Apple Developer account, add these secrets and the workflow signs and notarizes automatically:

- `APPLE_CERTIFICATE` (base64 of the exported `.p12`), `APPLE_CERTIFICATE_PASSWORD`
- `APPLE_SIGNING_IDENTITY` (e.g. `Developer ID Application: Your Name (TEAMID)`)
- `APPLE_ID`, `APPLE_PASSWORD` (an app-specific password), `APPLE_TEAM_ID`

### Windows signing (optional)

Unsigned installers trigger SmartScreen ("Windows protected your PC" → More info → Run anyway). To sign, follow the Tauri docs for `bundle.windows.certificateThumbprint` or an Azure Trusted Signing setup and add the corresponding secrets.

## Building locally

```bash
# current platform; artifacts land in src-tauri/target/release/bundle/
TAURI_SIGNING_PRIVATE_KEY_PATH=~/.tauri/neatnas.key npm run tauri build
# macOS universal (needs both Rust targets installed)
TAURI_SIGNING_PRIVATE_KEY_PATH=~/.tauri/neatnas.key npm run tauri build -- --target universal-apple-darwin
```

Without the signing key the bundles are still produced; only the updater signature step at the end reports an error.

Windows installers must be built on Windows (or by the workflow); Tauri cannot cross-build NSIS/MSI from macOS. A Windows compile check from macOS is possible with the MSVC target and LLVM's `llvm-rc`:

```bash
rustup target add x86_64-pc-windows-msvc
brew install llvm
scripts/check-windows.sh
```

The script disables the `updater` Cargo feature for the check (its HTTP stack compiles C code that needs a Windows toolchain) and overrides the capability list to match; the Windows CI runner builds the complete app.

## Platform behaviour to keep in mind

- Drag-out to Finder is macOS only. On Windows the gesture shows a hint and users download instead.
- macOS 14+ prompts for local-network access on the first Bonjour scan; the prompt text comes from `src-tauri/Info.plist`.
- First launch after the rename migrates settings and keychain entries from the old `com.nasdrive.app` identifier.
