## Neat NAS 1.0.0

**Remember your NAS once. Open it forever.** The first release of the lightest NAS file browser for Mac and Windows.

### Highlights

- **Opens straight into your files.** Add a NAS once; every launch connects and shows the last folder you were in, with the password fetched from the macOS Keychain or Windows Credential Manager.
- **Finds your NAS on the network.** Synology, QNAP, TrueNAS, Samba on a Raspberry Pi: anything announcing SMB shows up in the add dialog.
- **Quick Look for everything.** Photos, video, audio, PDF and text stream straight from the NAS. Space opens it, arrows move through the folder.
- **Files both ways.** Download and upload files or whole folders, drag files out into Finder, drop files in to upload. Transfers resume where they stopped, even across restarts.
- **Instant photo grids.** Thumbnails use the previews cameras embed in every photo, so RAW and HEIC folders need only the file headers.
- **Light and dark, ten languages.** English, 简体中文, 繁體中文, 日本語, 한국어, Deutsch, Español, Français, Português, Русский.
- **Tiny and native.** A pure-Rust SMB 2/3 client and the system web view via Tauri 2: about 10 MB to download, no Electron, no mounting, no admin rights.
- **Signed automatic updates.** New versions appear in Settings and install in one click.

### Downloads

| Platform | File | Notes |
| --- | --- | --- |
| macOS 11+ (Apple Silicon and Intel) | `Neat NAS_1.0.0_universal.dmg` | Not yet notarized: right-click → Open on first launch. |
| macOS 11+ (Apple Silicon and Intel) | `Neat NAS_1.0.0_universal-mac.zip` | The same app without the disk image: unzip, move to Applications. |
| Windows 10/11 (x64) | `Neat NAS_1.0.0_x64-setup.exe` | Not yet code-signed: SmartScreen → More info → Run anyway. Installs per user. |
| Windows 10/11 (x64) | `Neat NAS_1.0.0_x64-win.zip` | The same app without an installer: unzip and run `Neat NAS.exe`. Needs WebView2, which Windows 11 and current Windows 10 include. |

`SHA256SUMS.txt` lists the checksums of these four files. `latest.json` and the `.sig` files are what the in-app updater uses; you can ignore them.

### Known limitations

- Drag-out to Explorer is not available on Windows yet; use Download.
- Video preview depends on the system web view's codecs (MKV containers may not play on macOS).
- The macOS build is ad-hoc signed and the Windows installer unsigned, hence the one-time warnings above.
