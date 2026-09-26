# Releasing Neat NAS

## What a release produces

| Platform | Artifact | Notes |
|---|---|---|
| macOS | `Neat NAS_<version>_universal.dmg`, `Neat NAS_<version>_universal-mac.zip` | Universal binary (Apple Silicon + Intel), signed with the Developer ID and notarized by Apple. The zip is the same `.app` without the disk image. |
| Windows | `Neat NAS_<version>_x64-setup.exe` (NSIS), `Neat NAS_<version>_x64-win.zip` | Per-user install in ten languages, WebView2 bootstrapped if missing. The zip holds the same `Neat NAS.exe` without an installer (it relies on the WebView2 runtime built into Windows 11 and current Windows 10). Only NSIS is built, locally and in CI, so the updater always installs the same way. |
| Both | `SHA256SUMS.txt` | Checksums of the installable files above. |
| Updater | `Neat NAS.app.tar.gz`, `.sig` files, `latest.json` | What running copies download; `latest.json` only exists when `TAURI_SIGNING_PRIVATE_KEY` is set. |

Every release is also kept on this Mac in `release/<version>/` (gitignored): the installable files and `SHA256SUMS.txt` at the top, the updater files in `updater/`. `scripts/collect-release.sh` fills it, from local builds or from a published GitHub release.

## Cutting a release

The whole flow is one command on this Mac; the steps below are what it does.

```bash
scripts/release.sh 1.0.2
```

1. It checks that the Developer ID and the notarization profile work, bumps the version in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`, commits, tags and pushes.
2. The `Release` workflow (`.github/workflows/release.yml`) builds the Windows installer and zip on GitHub and opens a **draft** release with them and `latest.json`. Meanwhile this Mac builds the universal macOS app, signs it with the Developer ID and has Apple notarize the app and the disk image (`scripts/build-mac.sh`).
3. The script uploads the macOS files to the draft and adds them to `latest.json`.
4. It copies the draft into `release/<version>/`, checks `latest.json`, attaches `SHA256SUMS.txt`, and publishes.
5. Publishing makes `latest.json` reachable at `https://github.com/jason-jm/neat-nas/releases/latest/download/latest.json`, which is what running copies poll. The repository and its releases must stay public for that.

Releases need this Mac, because the Developer ID key stays in its keychain and is never uploaded anywhere. If GitHub Actions is unavailable, build both platforms here and upload them with `scripts/publish-release.sh` (see "Building locally").

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

### macOS signing and notarization

`scripts/build-mac.sh` signs with the first "Developer ID Application" identity in the login keychain and notarizes with the notarytool keychain profile `WAVESUBS_NOTARY`, the saved credentials Wave Subs uses too (override with `APPLE_SIGNING_IDENTITY` and `APPLE_KEYCHAIN_PROFILE`). `scripts/notarize-mac.sh` has Apple notarize the app, then rebuilds the disk image around the stapled app, signs it and has it notarized as well, so both open without warnings, even offline. The updater archive keeps the app as Tauri built it: copies installed by the in-app updater are not quarantined, and its minisign signature must stay valid. Because every version carries the same Developer ID, macOS keeps the app's keychain access across updates.

On a Mac without the identity the build falls back to ad-hoc signing (`signingIdentity: "-"` in `tauri.conf.json`), the minimum Apple silicon needs for downloaded apps. Such builds cannot be notarized: users approve them once in System Settings → Privacy & Security, and macOS asks again for keychain access after every update. That is how 1.0.1 shipped; 1.0.0 was not signed at all.

To save the notarization credentials on another Mac: `xcrun notarytool store-credentials WAVESUBS_NOTARY --apple-id <Apple ID> --team-id <team ID>` (it asks for an app-specific password from appleid.apple.com).

### Windows signing (optional)

Unsigned installers trigger SmartScreen ("Windows protected your PC" → More info → Run anyway). To sign, follow the Tauri docs for `bundle.windows.certificateThumbprint` or an Azure Trusted Signing setup and add the corresponding secrets.

## Building locally

```bash
scripts/build-mac.sh            # universal .dmg + zipped .app, signed and notarized -> release/<version>/
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
