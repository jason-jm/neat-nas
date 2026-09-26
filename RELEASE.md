# Releasing Neat NAS

## What a release produces

| Platform | Artifact | Notes |
|---|---|---|
| macOS | `Neat NAS_<version>_universal.dmg`, `Neat NAS_<version>_universal-mac.zip` | Universal binary (Apple Silicon + Intel). The zip is the same `.app` without the disk image. |
| Windows | `Neat NAS_<version>_x64-setup.exe` (NSIS), `Neat NAS_<version>_x64-win.zip` | Per-user install in ten languages, WebView2 bootstrapped if missing. The zip holds the same `Neat NAS.exe` without an installer (it relies on the WebView2 runtime built into Windows 11 and current Windows 10). CI releases also carry an `.msi`. |
| Both | `SHA256SUMS.txt` | Checksums of the installable files above. |
| Updater | `Neat NAS.app.tar.gz`, `.sig` files, `latest.json` | What running copies download; `latest.json` only exists when `TAURI_SIGNING_PRIVATE_KEY` is set. |

Every release is also kept on this Mac in `release/<version>/` (gitignored): the installable files and `SHA256SUMS.txt` at the top, the updater files in `updater/`. `scripts/collect-release.sh` fills it, from local builds or from a published GitHub release.

## Cutting a release

The whole flow is one command; the steps below are what it does.

```bash
scripts/release.sh 1.0.1
```

1. Bump the version in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json` (keep them identical), commit and push.
2. Tag and push the tag:
   ```bash
   git tag v1.0.1
   git push origin v1.0.1
   ```
3. The `Release` workflow (`.github/workflows/release.yml`) builds macOS (universal) and Windows (x64) on GitHub's runners and opens a **draft** release with the installers, the zipped apps, the updater artifacts and `latest.json` attached.
4. The script copies the draft into `release/<version>/`, attaches `SHA256SUMS.txt`, and publishes the draft.
5. Publishing makes `latest.json` reachable at `https://github.com/jason-jm/neat-nas/releases/latest/download/latest.json`, which is what running copies poll. The repository and its releases must stay public for that.

If the workflow is unavailable, the same release can be built on this Mac and uploaded with `scripts/publish-release.sh` (see "Building locally").

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
scripts/build-mac.sh            # universal .dmg + zipped .app -> release/<version>/
scripts/build-windows.sh        # installer + zipped .exe      -> release/<version>/ (see below)
scripts/publish-release.sh v1.0.1   # uploads release/1.0.1/ as a GitHub release, with latest.json
```

Both build scripts pick up `~/.tauri/neatnas.key` for the updater signature. `TAURI_SIGNING_PRIVATE_KEY` accepts either the key contents or a path to the key file; without it the bundles are still produced but the build reports an error at the signature step.

### Windows installer from macOS

The NSIS installer (not the MSI) can be cross-built on this Mac; Tauri calls this experimental, and the CI Windows runner remains the reference build:

```bash
rustup target add x86_64-pc-windows-msvc
brew install llvm makensis sevenzip
cargo install cargo-xwin
scripts/build-windows.sh        # -> release/<version>/Neat NAS_<version>_x64-setup.exe and _x64-win.zip
```

The first run downloads the Windows SDK and CRT (about 1 GB) into cargo-xwin's cache. Homebrew's makensis 3.12 aborts with `std::bad_alloc` on recent macOS unless a debugger is attached; the script detects that and runs it under `lldb`. The installer is not Authenticode-signed, so SmartScreen shows a warning on first launch.

A quicker compile-only check is `scripts/check-windows.sh` (it disables the `updater` feature and parks its capability because that check runs without the Windows C toolchain that cargo-xwin provides).

## Platform behaviour to keep in mind

- Drag-out to Finder is macOS only. On Windows the gesture shows a hint and users download instead.
- macOS 14+ prompts for local-network access on the first Bonjour scan; the prompt text comes from `src-tauri/Info.plist`.
- First launch after the rename migrates settings and keychain entries from the old `com.nasdrive.app` identifier.
